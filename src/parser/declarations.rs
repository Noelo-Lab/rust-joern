//! Declarations, function headers, records and translation-unit scope.

use super::*;

impl Parser<'_> {
    pub(super) fn translation_scope(
        &mut self,
        prefix: &str,
        functions: &mut Vec<Function>,
        enclosed: bool,
    ) {
        let old_scope = std::mem::replace(&mut self.lexical_scope, prefix.to_string());
        let old_using_namespaces = self.using_namespaces.clone();
        while self.pos < self.limit {
            if self.at("}") {
                if enclosed {
                    break;
                }
                // At translation-unit scope a stray closing brace is a CDT
                // problem declaration; it does not end the translation unit.
                self.pos += 1;
                continue;
            }
            if self.eat(";") {
                continue;
            }
            // This CDT version treats a leading _Noreturn as a separate
            // non-executable problem token, before parsing the declaration.
            if self.eat("_Noreturn") {
                continue;
            }
            if matches!(self.peek(), "asm" | "__asm" | "__asm__") {
                let start = self.pos;
                self.pos += 1;
                while matches!(
                    self.peek(),
                    "volatile" | "__volatile" | "__volatile__" | "inline" | "goto"
                ) {
                    self.pos += 1;
                }
                if matches!(self.peek(), "(" | "{")
                    && let Some(close) = self.matching(self.pos, self.limit)
                {
                    self.pos = close + 1;
                    self.eat(";");
                } else {
                    self.diagnose(
                        start,
                        self.pos,
                        "cannot recover assembly declaration operand",
                    );
                }
                continue;
            }
            if matches!(self.peek(), "public" | "private" | "protected")
                && self.text(self.pos + 1) == ":"
            {
                self.pos += 2;
                continue;
            }
            let start = self.pos;
            let mut cursor = start;
            while cursor < self.limit {
                match self.text(cursor) {
                    "(" | "[" => {
                        if let Some(close) = self.matching(cursor, self.limit) {
                            cursor = close + 1;
                        } else {
                            break;
                        }
                    }
                    ";" | "{" | "}" => break,
                    _ => cursor += 1,
                }
            }
            if cursor == self.limit {
                // An unterminated translation-unit declaration is a CDT
                // problem declaration, including a header without its final
                // semicolon. It contributes no method or executable AST.
                self.pos = cursor;
                break;
            }
            if self.text(cursor) == "}" {
                if enclosed {
                    break;
                }
                self.pos = cursor + 1;
                continue;
            }
            if matches!(self.text(cursor), "(" | "[") {
                // An unmatched declaration group is a CDT problem, whose
                // recovery ignores group delimiters and skips its brace body.
                self.pos = self.problem_declaration_end(cursor);
                continue;
            }
            let is_typedef = (start..cursor).any(|i| self.text(i) == "typedef");
            // A group that is no declarator makes CDT declare a function by
            // the type's name, which turns later `T *x;` into expressions.
            // By default such a declaration stays a problem.
            let file_scope =
                self.cdt_failures && !self.cpp && self.record_depth == 0 && prefix.is_empty();
            let header = if is_typedef {
                None
            } else {
                self.function_header_in(start, cursor, file_scope)
            };
            // CDT accepts K&R definitions. Their parameter declarations contain
            // semicolons before the function body, unlike ordinary prototypes.
            if self.text(cursor) == ";"
                && let Some(ref header) = header
                && let Some(body) = self.knr_body_start(header)
            {
                cursor = body;
            }
            if self.text(cursor) == ";" {
                if matches!(self.text(start), "static_assert" | "_Static_assert") {
                    // These are assertions, not declarations with a grouped
                    // function declarator inside their type-id operands.
                    if (start..cursor).any(|i| {
                        matches!(
                            self.text(i),
                            "__builtin_va_arg"
                                | "__builtin_offsetof"
                                | "__builtin_types_compatible_p"
                                | "__offsetof__"
                        )
                    }) {
                        let expression = self.expression_range(start, cursor);
                        self.global_expressions.push(expression);
                    }
                    self.pos = cursor + 1;
                    continue;
                }
                if !is_typedef && header.is_none() && self.declaration_shape_problem(start, cursor)
                {
                    // A cast/address expression cannot name a global object.
                    // Avoid creating bindings from omitted problem declarators.
                    self.pos = cursor + 1;
                    continue;
                }
                // CDT's elaborated-type declaration path creates declarator
                // locals, including function declarators, rather than methods.
                let elaborated = (start..self.specifier_end(start, cursor))
                    .any(|i| matches!(self.text(i), "struct" | "union" | "enum" | "class"));
                self.remember_types(start, cursor);
                if self.cpp && self.text(start) == "using" {
                    self.pos = cursor + 1;
                    continue;
                }
                if !is_typedef {
                    self.collect_global_expressions(start, cursor);
                }
                if !is_typedef && !elaborated {
                    // A simple declaration shares its specifiers across every
                    // declarator; Joern converts each function declarator.
                    // A constructor's name is also a known type. It belongs to
                    // the declarator, rather than the shared type specifiers.
                    let base_end = header.as_ref().map_or_else(
                        || self.specifier_end(start, cursor),
                        |header| self.specifier_end(start, cursor).min(header.name_start),
                    );
                    let base_type = self.clean_type(start, base_end);
                    let record_scope = if self.cpp {
                        self.class_scopes.contains(prefix)
                    } else {
                        !prefix.is_empty()
                    };
                    if record_scope
                        && header.as_ref().is_some_and(|header| {
                            header.name_start < self.specifier_end(start, cursor)
                                && self
                                    .unknown_function_suffix(header.parameters_end + 1, cursor)
                                    .is_some()
                        })
                    {
                        // A function-shaped macro followed by a field name
                        // is one CDT problem member, not a method declaration.
                        self.pos = cursor + 1;
                        continue;
                    }
                    if let Some(header) = header.filter(|h| h.name_start < base_end) {
                        // A C++ constructor has no return decl-specifier;
                        // specifier recovery may consume its class-name token.
                        self.add_function(start, cursor, header, prefix, false, functions);
                    } else {
                        let declarators = self.split_ranges(base_end, cursor, ",");
                        let first = declarators.first().map_or(base_end, |range| range.0);
                        let several = declarators.len() > 1;
                        for (a, b) in declarators {
                            if let Some(mut header) = self.function_header_in(a, b, file_scope) {
                                let declarator_type = header
                                    .return_type
                                    .take()
                                    .unwrap_or_else(|| self.clean_type(a, header.name_start));
                                header.return_type = Some(format!("{base_type}{declarator_type}"));
                                if several {
                                    header.shared = Some((base_end, first, a));
                                }
                                self.add_function(start, cursor, header, prefix, false, functions);
                            } else if let (Some(name), _) = self.declarator_name(a, b) {
                                self.bind_object(name, prefix);
                            }
                        }
                    }
                } else {
                    self.remember_types(start, cursor);
                    if !is_typedef {
                        let base_end = self.specifier_end(start, cursor);
                        for (a, b) in self.split_ranges(base_end, cursor, ",") {
                            if let (Some(name), _) = self.declarator_name(a, b) {
                                if self.function_header(a, b).is_some() {
                                    // Still a function binding to CDT.
                                    let name = self.text(name).to_string();
                                    Rc::make_mut(&mut self.variables).insert(name);
                                } else {
                                    self.bind_object(name, prefix);
                                }
                            }
                        }
                    }
                }
                self.pos = cursor + 1;
            } else if let Some(header) = header {
                self.add_function(start, cursor, header, prefix, true, functions);
            } else {
                let namespace = (start..cursor).find(|i| self.cpp && self.text(*i) == "namespace");
                let class = self.record_definition(start, cursor);
                let linkage = self.text(start) == "extern"
                    && self
                        .tokens
                        .get(start + 1)
                        .is_some_and(|t| t.kind == TokenKind::Literal);
                if namespace.is_some() || class.is_some() || linkage {
                    let introducer = namespace.or(class);
                    let name = introducer
                        .map(|i| {
                            let mut after = i + 1;
                            while let Some(next) = self.skip_attribute(after, cursor) {
                                after = next;
                            }
                            after
                        })
                        .filter(|i| {
                            self.tokens
                                .get(*i)
                                .is_some_and(|t| t.kind == TokenKind::Identifier)
                        })
                        .map(|i| self.text(i).to_string())
                        .unwrap_or_default();
                    if self.cpp && class.is_some() && !name.is_empty() {
                        self.types.insert(name.clone());
                        self.explicit_types.insert(name.clone());
                    }
                    let nested = if name.is_empty() {
                        prefix.to_string()
                    } else if prefix.is_empty() {
                        name
                    } else {
                        format!("{prefix}::{name}")
                    };
                    if self.cpp && class.is_some() && !nested.is_empty() {
                        self.class_scopes.insert(nested.clone());
                        let close = self.matching(cursor, self.limit).unwrap_or(self.limit);
                        let fields = self.collect_class_fields(cursor + 1, close);
                        self.scalar_constructor_classes.remove(&nested);
                        if self.constructor_initializers
                            && self.scalar_constructor_class(start, cursor, close, &fields)
                        {
                            self.scalar_constructor_classes.insert(nested.clone());
                        }
                        self.class_fields.insert(nested.clone(), fields);
                    }
                    let previous_linkage = self.extern_c;
                    let outer_variables = class.map(|_| self.variables.clone());
                    if self.cpp && class.is_some() {
                        self.class_alias_undo.push(HashMap::new());
                    }
                    if self.cpp && linkage {
                        self.extern_c = self.text(start + 1) == "\"C\"";
                    }
                    self.pos = cursor + 1;
                    self.record_depth += usize::from(class.is_some());
                    self.translation_scope(&nested, functions, true);
                    self.record_depth -= usize::from(class.is_some());
                    self.extern_c = previous_linkage;
                    if let Some(variables) = outer_variables {
                        self.variables = variables;
                    }
                    if self.cpp && class.is_some() {
                        for (name, undo) in self.class_alias_undo.pop().unwrap() {
                            if let Some(alias) = undo.alias {
                                self.type_aliases.insert(name.clone(), alias);
                            } else {
                                self.type_aliases.remove(&name);
                            }
                            if !undo.was_type {
                                self.types.remove(&name);
                            }
                            if !undo.was_explicit {
                                self.explicit_types.remove(&name);
                            }
                        }
                    }
                    self.expect("}");
                    // Names after an anonymous record, including typedef aliases.
                    let after = self.pos;
                    if class.is_some() {
                        self.pos = self.declaration_end(after);
                        if !is_typedef {
                            self.collect_global_expressions(after, self.pos);
                        }
                        for (a, b) in self.split_ranges(after, self.pos, ",") {
                            if let (Some(name), _) = self.declarator_name(a, b) {
                                if is_typedef {
                                    self.types.insert(self.text(name).to_string());
                                    self.explicit_types.insert(self.text(name).to_string());
                                } else {
                                    self.bind_object(name, prefix);
                                }
                            }
                        }
                        self.eat(";");
                    }
                } else if let Some(close) = self.matching(cursor, self.limit) {
                    // Aggregate initializer, enum, or non-executable record data.
                    if !(start..cursor).any(|i| matches!(self.text(i), "=" | "enum"))
                        && !self.cdt_problem_function_header(start, cursor)
                    {
                        self.diagnose(
                            start,
                            close + 1,
                            "unrecognized top-level brace; possible omitted function definition",
                        );
                    }
                    let aggregate = (start..cursor).any(|i| matches!(self.text(i), "=" | "enum"));
                    self.pos = if aggregate {
                        self.declaration_end(close + 1)
                    } else {
                        close + 1
                    };
                    if aggregate {
                        self.remember_types(start, self.pos);
                        if !is_typedef {
                            self.collect_global_expressions(start, self.pos);
                        }
                    }
                    self.eat(";");
                } else if (start..cursor).any(|i| self.text(i) == "=")
                    && let Some(after) = self.problem_initializer_end(cursor)
                {
                    self.pos = after;
                } else {
                    self.diagnose(cursor, cursor + 1, "unclosed top-level brace");
                    self.pos = self.limit;
                }
            }
            if self.pos <= start {
                self.diagnose(start, start + 1, "cannot recover top-level syntax");
                self.pos = start + 1;
            }
        }
        self.lexical_scope = old_scope;
        self.using_namespaces = old_using_namespaces;
    }

    pub(super) fn bind_object(&mut self, at: usize, prefix: &str) {
        let name = self.text(at).to_string();
        if !self.cpp
            && prefix.is_empty()
            && self.record_depth == 0
            && !self.function_returns.contains_key(&name)
        {
            let mut before = at;
            while before > 0 && qualifier(self.text(before - 1)) {
                before -= 1;
            }
            // CDT types the name by its first declarator: a pointer unless
            // an array or function suffix binds to the name first.
            if !self.variables.contains(&name)
                && before > 0
                && self.text(before - 1) == "*"
                && !matches!(self.text(at + 1), "[" | "(")
            {
                self.global_pointers.insert(name.clone());
            }
            self.object_bindings.insert(name.clone());
        }
        Rc::make_mut(&mut self.variables).insert(name);
    }

    pub(super) fn declaration_end(&self, start: usize) -> usize {
        let mut i = start;
        while i < self.limit && !matches!(self.text(i), ";" | "}") {
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self
                    .matching(i, self.limit)
                    .map_or(i + 1, |close| close + 1);
            } else {
                i += 1;
            }
        }
        i
    }

    pub(super) fn knr_body_start(&self, header: &Header) -> Option<usize> {
        let parameters = self.split_ranges(header.parameters_start, header.parameters_end, ",");
        if parameters.is_empty()
            || parameters.iter().any(|&(a, b)| {
                b != a + 1
                    || self.tokens[a].kind != TokenKind::Identifier
                    || self.type_word(self.text(a))
                    || qualifier(self.text(a))
            })
        {
            return None;
        }
        let mut from = header.parameters_end + 1;
        let mut declared = false;
        while from < self.limit {
            if self.text(from) == "{" {
                return declared.then_some(from);
            }
            let mut end = from;
            while end < self.limit && !matches!(self.text(end), ";" | "{" | "}" | "=") {
                if matches!(self.text(end), "(" | "[") {
                    end = self.matching(end, self.limit)? + 1;
                } else {
                    end += 1;
                }
            }
            if self.text(end) != ";" {
                return None;
            }
            let base_end = self.specifier_end(from, end);
            if base_end == from
                || base_end == end
                || self.split_ranges(base_end, end, ",").iter().any(|&(a, b)| {
                    self.declarator_name(a, b).0.is_none_or(|name| {
                        !parameters
                            .iter()
                            .any(|&(p, _)| self.text(p) == self.text(name))
                    })
                })
            {
                return None;
            }
            declared = true;
            from = end + 1;
        }
        None
    }

    pub(super) fn record_definition(&self, start: usize, end: usize) -> Option<usize> {
        // A template parameter's `class` is not the record introducer, and
        // its default argument is not an object initializer.
        let mut start = start;
        while self.cpp && self.text(start) == "template" && self.text(start + 1) == "<" {
            start = self.angle_close(start + 1, end)? + 1;
        }
        if (start..end).any(|i| self.text(i) == "=") {
            return None;
        }
        let introducer =
            (start..end).find(|i| matches!(self.text(*i), "struct" | "class" | "union"))?;
        // In C++, the class/struct token in an enum-key does not introduce a
        // record scope. Its brace body contains enumerators, not members.
        if self.cpp && introducer > start && self.text(introducer - 1) == "enum" {
            return None;
        }
        let mut after = introducer + 1;
        while let Some(next) = self.skip_attribute(after, end) {
            after = next;
        }
        if after < end && self.tokens[after].kind == TokenKind::Identifier {
            after += 1;
        }
        while after < end {
            if let Some(next) = self.skip_attribute(after, end) {
                after = next;
            } else if self.text(after) == "final" {
                after += 1;
            } else {
                break;
            }
        }
        (after == end || self.text(after) == ":").then_some(introducer)
    }

    pub(super) fn function_header(&self, start: usize, end: usize) -> Option<Header> {
        self.function_header_in(start, end, false)
    }

    /// `c_file_scope` marks the start of a C declaration at file scope.
    pub(super) fn function_header_in(
        &self,
        start: usize,
        end: usize,
        c_file_scope: bool,
    ) -> Option<Header> {
        if matches!(self.text(start), "static_assert" | "_Static_assert")
            || (self.cpp && self.text(start) == "using")
        {
            return None;
        }
        if let Some(header) = self.grouped_function_header(start, end) {
            return Some(header);
        }
        let mut i = start;
        while i < end {
            if matches!(
                self.text(i),
                "__attribute__"
                    | "__attribute"
                    | "__declspec"
                    | "alignas"
                    | "__asm__"
                    | "asm"
                    | "typeof"
                    | "__typeof__"
                    | "decltype"
                    | "sizeof"
                    | "alignof"
                    | "_Alignof"
                    | "__alignof__"
            ) {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                }
                continue;
            }
            if self.cpp && matches!(self.text(i), "throw" | "noexcept") {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(i + 1, |close| close + 1);
                }
                continue;
            }
            if self.text(i) == "[" {
                i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                continue;
            }
            if self.text(i) != "(" || i == start {
                i += 1;
                continue;
            }
            let close = self.matching(i, end)?;
            // A typedef followed by a grouped object declarator is not an
            // implicit function name (e.g. byte (mask)[8]).
            if i == start + 1
                && self.explicit_types.contains(self.text(start))
                && (!self.cpp || self.text(close + 1) == "[")
            {
                i = close + 1;
                continue;
            }
            if self.text(i - 1) == "operator" && close == i + 1 && self.text(close + 1) == "(" {
                i = close + 1;
                continue;
            }
            let mut name_end = i;
            let mut name_start = i - 1;
            if name_start > start && self.text(name_start - 1) == "operator" {
                name_start -= 1;
            } else if matches!(self.text(name_start), ")" | "]")
                && name_start > start
                && matches!(self.text(name_start - 1), "(" | "[")
                && name_start >= 2
                && self.text(name_start - 2) == "operator"
            {
                name_start -= 2;
            } else if self.text(name_start) == "]"
                && name_start >= start + 3
                && self.text(name_start - 1) == "["
                && matches!(self.text(name_start - 2), "new" | "delete")
                && self.text(name_start - 3) == "operator"
            {
                name_start -= 3;
            } else if self.tokens[name_start].kind != TokenKind::Identifier
                || self.type_word(self.text(name_start))
                || qualifier(self.text(name_start))
            {
                i += 1;
                continue;
            }
            if name_start > start && self.text(name_start - 1) == "~" {
                name_start -= 1;
            }
            while name_start >= start + 2
                && self.text(name_start - 1) == "::"
                && self.tokens[name_start - 2].kind == TokenKind::Identifier
            {
                name_start -= 2;
            }
            if (start..name_start).any(|n| self.text(n) == "=" || self.text(n) == ":") {
                i = close + 1;
                continue;
            }
            // A direct initializer with values is not a function prototype.
            // At file scope CDT's C parser has no such initializer: a list
            // it accepts, as of abstract pointers, declares a function
            // unless the group reads further as a nested declarator, or as
            // far and the name before it is a type.
            if self.split_ranges(i + 1, close, ",").iter().any(|(a, b)| {
                a < b
                    && (self.tokens[*a].kind == TokenKind::Literal
                        || matches!(
                            self.text(*a),
                            "{" | "+" | "-" | "!" | "~" | "*" | "&" | "&&"
                        ))
            }) && (!c_file_scope
                || self.cdt_parameter_list(i, close + 1, true).is_none()
                || self
                    .cdt_declarator(i, end, false)
                    .is_some_and(|(nested, _)| {
                        nested > close + 1 || self.explicit_types.contains(self.text(name_start))
                    }))
            {
                i = close + 1;
                continue;
            }
            if self.cpp && close == i + 2 && self.variables.contains(self.text(i + 1)) {
                i = close + 1;
                continue;
            }
            // Handle operator names without spaces; ordinary qualified names use ::.
            if name_start > start && self.text(name_start - 1) == "operator" {
                name_start -= 1;
            }
            if name_start >= name_end {
                name_end = i;
            }
            let name = (name_start..name_end)
                .map(|n| self.text(n))
                .collect::<String>();
            return Some(Header {
                name,
                name_start,
                parameters_start: i + 1,
                parameters_end: close,
                return_type: None,
                shared: None,
                outer_parameters: None,
            });
        }
        None
    }

    pub(super) fn grouped_function_header(&self, start: usize, end: usize) -> Option<Header> {
        let base_end = self.specifier_end(start, end);
        let mut group = base_end;
        while group < end
            && (matches!(self.text(group), "*" | "&" | "&&") || qualifier(self.text(group)))
        {
            group += 1;
        }
        if self.text(group) != "(" {
            return None;
        }
        let close = self.matching(group, end)?;
        let mut header = if let Some(mut header) = self.function_header(group + 1, close) {
            if self.text(close + 1) == "(" {
                header.outer_parameters = Some((close + 2, self.matching(close + 1, end)?));
            }
            header
        } else {
            let (name, suffix) = self.declarator_name(group + 1, close);
            let name = name?;
            if suffix != close
                || (group + 1..name).any(|i| matches!(self.text(i), "*" | "&" | "&&"))
                || self.text(close + 1) != "("
            {
                return None;
            }
            let mut name_start = name;
            while name_start >= group + 3 && self.text(name_start - 1) == "::" {
                name_start -= 2;
            }
            Header {
                name: (name_start..=name).map(|i| self.text(i)).collect(),
                name_start,
                parameters_start: close + 2,
                parameters_end: self.matching(close + 1, end)?,
                return_type: None,
                shared: None,
                outer_parameters: None,
            }
        };
        let mut return_type = self.clean_type(start, base_end);
        for i in base_end..header.name_start {
            if matches!(self.text(i), "*" | "&" | "&&") {
                return_type.push_str(self.text(i));
            }
        }
        let mut i = close + 1;
        while i < end {
            if self.text(i) == "[" {
                let close = self.matching(i, end)?;
                return_type.push_str(&self.raw(i, close + 1));
                i = close + 1;
            } else if self.text(i) == "(" {
                i = self.matching(i, end)? + 1;
            } else {
                i += 1;
            }
        }
        header.return_type = Some(return_type);
        Some(header)
    }

    pub(super) fn scalar_initialization_value(
        &self,
        value: &Expr,
        expected_type: &str,
        fields: &[(String, String)],
        functions: &[Function],
    ) -> bool {
        let scalar_type = |ty: &str| {
            !ty.is_empty()
                && !ty.contains(['&', '(', '[', '<'])
                && (ty.contains('*')
                    || ty.split_whitespace().all(|word| {
                        matches!(
                            word,
                            "bool"
                                | "char"
                                | "signed"
                                | "unsigned"
                                | "short"
                                | "int"
                                | "long"
                                | "float"
                                | "double"
                                | "const"
                                | "volatile"
                        )
                    }))
        };
        let compatible = |ty: &str| {
            // Binding types do not preserve pointee cv qualifiers, so pointer
            // conversions cannot yet be proved safe from this information.
            scalar_type(ty) && !expected_type.contains('*') && !ty.contains('*')
        };
        match &value.kind {
            ExprKind::Identifier(name) => self
                .variable_types
                .get(name)
                .or_else(|| {
                    fields
                        .iter()
                        .find(|(field, _)| field == name)
                        .map(|(_, ty)| ty)
                })
                .is_some_and(|ty| compatible(ty)),
            ExprKind::Literal(value) => {
                if expected_type.contains('*') {
                    value == "nullptr"
                } else if scalar_type(expected_type) {
                    let integral = !expected_type
                        .split_whitespace()
                        .any(|word| matches!(word, "float" | "double"));
                    !value.is_empty() && value.bytes().all(|c| c.is_ascii_digit())
                        || integral && matches!(value.as_str(), "true" | "false")
                        || integral && value.starts_with('\'') && value.ends_with('\'')
                } else {
                    false
                }
            }
            ExprKind::Bracketed(inner) => {
                self.scalar_initialization_value(inner, expected_type, fields, functions)
            }
            ExprKind::Call { callee, arguments } if arguments.is_empty() => {
                let ExprKind::Identifier(name) = &callee.kind else {
                    return false;
                };
                let matches: Vec<_> = functions
                    .iter()
                    .filter(|f| {
                        f.full_name.split(':').next() == Some(name.as_str())
                            && f.parameters.is_empty()
                            && f.implicit_this.is_none()
                    })
                    .collect();
                !matches.is_empty() && matches.iter().all(|f| compatible(&f.binding_return_type))
            }
            _ => false,
        }
    }

    pub(super) fn add_function(
        &mut self,
        start: usize,
        body_start: usize,
        header: Header,
        prefix: &str,
        definition: bool,
        functions: &mut Vec<Function>,
    ) {
        let initializer_guard = if self.cpp
            && self.constructor_initializers
            && definition
            && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ":")
        {
            let index = self.diagnostics.len();
            self.diagnose(header.parameters_end + 1, body_start,
                "constructor member-initializer CFG is unsupported: complete explicit scalar-member plan required");
            Some(index)
        } else {
            None
        };
        if self.cdt_problem_declaration(start, body_start)
            || self.cdt_problem_record_header(start, body_start)
            || self.cdt_problem_return_prefix(start, &header)
            || self.cdt_problem_parameters(&header)
            || (self.cdt_failures
                && (self.cdt_parameter_backtrack(&header)
                    || self.cdt_nested_declarator_problem(start, body_start, &header)))
        {
            // The original CDT version recovers these as problem declarations;
            // Joern's declaration conversion emits no method for them.
            if definition {
                self.pos = self
                    .brace_end(body_start, self.limit)
                    .map_or(self.limit, |close| close + 1);
            }
            return;
        }
        if !definition
            && prefix.is_empty()
            && self.record_depth == 0
            && self.object_bindings.contains(&header.name)
        {
            // CDT binds a C name to its first declaration: a later function
            // declarator of an object's name is not an IFunction, so
            // astForFunctionDeclarator emits no method.
            return;
        }
        if !definition
            && let Some(marker) =
                self.unknown_function_suffix(header.parameters_end + 1, body_start)
        {
            let declaration_end = header.parameters_end;
            self.add_function(start, declaration_end, header, prefix, false, functions);
            if !self.cpp
                && let Some(mut recovered) = self.function_header(marker, body_start)
            {
                recovered.return_type = Some("ANY".into());
                self.add_function(marker, body_start, recovered, prefix, false, functions);
            }
            return;
        }
        let mut direct_parameters = header.parameters_end + 1;
        while self.cpp && matches!(self.text(direct_parameters), "const" | "volatile") {
            direct_parameters += 1;
        }
        if definition && self.text(direct_parameters) == "(" {
            // CDT ends the first function declarator before a second direct
            // parameter list, leaving a method declaration and no body.
            let body_end = self.brace_end(body_start, self.limit);
            self.add_function(
                start,
                header.parameters_end,
                header,
                prefix,
                false,
                functions,
            );
            self.pos = body_end.map_or(self.limit, |close| close + 1);
            return;
        }
        if definition
            && let Some(marker) =
                self.unknown_function_suffix(header.parameters_end + 1, body_start)
        {
            // CDT finishes the initial declarator before an unknown suffix.
            // Its following brace body belongs to a recovered declaration,
            // not to that method's declaration stub. Problem-declaration
            // recovery balances braces only, so unclosed groups stay inside.
            let declaration_end = header.parameters_end;
            let body_end = self.brace_end(body_start, self.limit);
            self.add_function(start, declaration_end, header, prefix, false, functions);
            if !self.cpp
                && let Some(recovered) = self.function_header(marker, body_start)
            {
                self.add_function(marker, body_start, recovered, prefix, true, functions);
            } else {
                self.pos = body_end.map_or(self.limit, |close| close + 1);
            }
            return;
        }
        let (return_start, recovered_return_type) = match header.shared {
            // An earlier declarator is not part of this one's specifiers.
            Some((shared_end, _, _)) => (self.return_specifier_start(start, shared_end).0, false),
            None => self.return_specifier_start(start, header.name_start),
        };
        let base_end = match header.shared {
            Some((shared_end, _, _)) => shared_end,
            None => self.specifier_end(return_start, header.name_start),
        };
        let primitive_return = (return_start..base_end).any(|i| self.type_word(self.text(i)));
        if (!self.cpp && self.text(base_end) == "<")
            || (primitive_return
                && self
                    .tokens
                    .get(base_end)
                    .is_some_and(|t| t.kind == TokenKind::Identifier)
                && self.text(base_end + 1) == "<"
                && base_end < header.name_start)
        {
            // CDT cannot combine a primitive return specifier with a second
            // template-like named specifier; C also has no template type-ids.
            if definition {
                self.pos = self
                    .brace_end(body_start, self.limit)
                    .map_or(self.limit, |close| close + 1);
            }
            return;
        }
        let return_type = if recovered_return_type {
            self.clean_type(return_start, header.name_start)
        } else {
            header
                .return_type
                .clone()
                .unwrap_or_else(|| self.clean_type(return_start, header.name_start))
        };
        let return_type = canonical_primitive_specifiers(&return_type);
        if self.cpp && return_type.is_empty() {
            let method = header.name.rsplit("::").next().unwrap_or(&header.name);
            let owner = header
                .name
                .rsplit_once("::")
                .and_then(|(owner, _)| self.resolve_class_name(owner))
                .or_else(|| {
                    self.class_scopes
                        .contains(prefix)
                        .then(|| prefix.to_string())
                });
            let qualified_special_member = definition
                && header
                    .name
                    .rsplit_once("::")
                    .is_some_and(|(spelled_owner, _)| {
                        let leaf = spelled_owner.rsplit("::").next().unwrap_or(spelled_owner);
                        !leaf.contains('<') && method.strip_prefix('~').unwrap_or(method) == leaf
                    });
            let constructor = qualified_special_member
                || owner.as_ref().is_some_and(|owner| {
                    method.strip_prefix('~').unwrap_or(method)
                        == owner.rsplit("::").next().unwrap_or(owner)
                });
            let conversion = owner.is_some()
                && (header.name_start..header.parameters_start - 1).any(|i| {
                    self.text(i) == "operator"
                        && self
                            .tokens
                            .get(i + 1)
                            .is_some_and(|t| t.kind == TokenKind::Identifier)
                        && !matches!(self.text(i + 1), "new" | "delete" | "co_await")
                });
            if !constructor && !conversion {
                // Unlike C, CDT's C++ parser does not accept an omitted
                // return specifier on an ordinary function declaration.
                if definition {
                    self.pos = self
                        .brace_end(body_start, self.limit)
                        .map_or(self.limit, |close| close + 1);
                }
                return;
            }
        }
        let return_binding_type = if return_type.is_empty() && self.cpp {
            "ANY".into()
        } else if return_type.is_empty() {
            "int".into()
        } else {
            return_type.clone()
        };
        let return_type = if return_type.is_empty() {
            if !definition
                && self.cpp
                && (return_start..header.name_start).any(|i| self.text(i) == "virtual")
            {
                "virtual".into()
            } else if self.cpp {
                return_binding_type.clone()
            } else if definition {
                // CDT's return type string of an empty C specifier keeps
                // only its cv-qualifiers; Joern reports an empty one as ANY.
                let display =
                    primitive_return_display(&self.type_code(return_start, base_end, true));
                if display.is_empty() {
                    "ANY".into()
                } else {
                    display
                }
            } else {
                self.prototype_return_display(return_start, base_end, base_end)
                    .unwrap_or_else(|| "ANY".into())
            }
        } else if definition {
            // returnTypeForIASTFunctionDefinition does not clean its type. A
            // simple specifier keeps CDT's return type string, an elaborated
            // one its signature, and a named one only its simple name: the
            // last two lose the declarator's pointer operators.
            let base = return_type
                .split(['*', '&', '['])
                .next()
                .unwrap_or("")
                .trim();
            if self.cpp && base == "auto" {
                // CDT's return type string of a deduced type is empty.
                "ANY".into()
            } else if base.split_whitespace().all(|word| self.type_word(word)) {
                // The pointer operators end where a grouped declarator begins.
                let pointers_end = (base_end..header.name_start)
                    .find(|&i| {
                        !matches!(self.text(i), "*" | "&" | "&&") && !qualifier(self.text(i))
                    })
                    .unwrap_or(header.name_start);
                let display =
                    primitive_return_display(&self.type_code(return_start, pointers_end, true));
                self.function_pointer_return(pointers_end, header.name_start, body_start)
                    .map_or(display.clone(), |suffix| format!("{display}{suffix}"))
            } else if let Some(elaborated) = self.elaborated_return_display(start, base_end) {
                elaborated
            } else {
                simple_type_name(base).replace(" <", "<")
            }
        } else {
            // typeForDeclSpecifier reads the pointer operators of the
            // declaration's first declarator for every function it declares.
            self.prototype_return_display(
                return_start,
                base_end,
                header.shared.map_or(base_end, |(_, first, _)| first),
            )
            .unwrap_or(return_type)
        };
        let old_variables = self.variables.clone();
        let old_variable_types = self.variable_types.clone();
        let old_variable_closures = self.variable_closures.clone();
        let old_function_full_name = self.function_full_name.clone();
        let old_function_fields = self.function_fields.clone();
        let old_function_member_cv = self.function_member_cv;
        let old_body_context = self.in_function_body;
        self.in_function_body = false;
        let parameters =
            if definition && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ";") {
                let mut declared = Vec::new();
                for (a, b) in self.split_ranges(header.parameters_end + 1, body_start, ";") {
                    if a < b {
                        let span = self.span(a, b + 1);
                        declared.extend(
                            self.declarations_range(a, b)
                                .into_iter()
                                .enumerate()
                                .map(|(position, d)| (d, span.clone(), position == 0)),
                        );
                    }
                }
                self.split_ranges(header.parameters_start, header.parameters_end, ",")
                    .into_iter()
                    .filter(|(a, b)| a < b)
                    .map(|(a, b)| {
                        let name = self.raw(a, b);
                        let declaration = declared.iter().find(|(d, _, _)| d.name == name);
                        let type_name =
                            declaration.map_or("int".into(), |(d, _, _)| d.type_name.clone());
                        Parameter {
                            name,
                            type_name,
                            function_pointer: false,
                            knr: Some(if declaration.is_some_and(|(_, _, first)| *first) {
                                KnrParameter::First
                            } else {
                                KnrParameter::Unlisted
                            }),
                            span: declaration
                                .map_or_else(|| self.span(a, b), |(_, span, _)| span.clone()),
                        }
                    })
                    .collect()
            } else if definition
                && !self.cpp
                && self
                    .split_ranges(header.parameters_start, header.parameters_end, ",")
                    .iter()
                    .all(|&(a, b)| {
                        a == b
                            || b == a + 1
                                && self.tokens[a].kind == TokenKind::Identifier
                                && !self.type_word(self.text(a))
                                && !qualifier(self.text(a))
                                && !self.explicit_types.contains(self.text(a))
                    })
            {
                self.split_ranges(header.parameters_start, header.parameters_end, ",")
                    .into_iter()
                    .filter(|(a, b)| a < b)
                    .map(|(a, b)| Parameter {
                        name: self.raw(a, b),
                        type_name: "ANY".into(),
                        function_pointer: false,
                        knr: None,
                        span: self.span(a, b),
                    })
                    .collect()
            } else if let Some((outer_start, outer_end)) =
                header.outer_parameters.filter(|_| !definition)
            {
                // A declared method's outer function declarator lists its own
                // parameters before those of the declarator it nests.
                let mut parameters = self.parameters(outer_start, outer_end);
                parameters.extend(self.parameters(header.parameters_start, header.parameters_end));
                parameters
            } else {
                self.parameters(header.parameters_start, header.parameters_end)
            };
        let signature = format!(
            "{}({})",
            return_type,
            parameters
                .iter()
                .map(|p| p.type_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        let lexical_name = header
            .name
            .rsplit("::")
            .next()
            .unwrap_or(&header.name)
            .to_string();
        let name = if let Some(operator) = lexical_name.strip_prefix("operator")
            && self.cpp
            && (header.name_start..header.parameters_start).any(|i| self.text(i) == "operator")
        {
            if definition {
                operator.to_string()
            } else {
                format!("operator {operator}")
            }
        } else {
            lexical_name
        };
        let qualified_name = if let Some((owner, method)) = header.name.rsplit_once("::")
            && let Some(owner) = self.resolve_class_name(owner)
        {
            format!("{owner}::{method}")
        } else {
            let resolved_name = if let Some((owner, method)) = header.name.split_once("::") {
                self.imports.get(owner).map_or_else(
                    || header.name.clone(),
                    |qualified| format!("{qualified}::{method}"),
                )
            } else {
                header.name.clone()
            };
            if prefix.is_empty() {
                resolved_name
            } else {
                format!("{prefix}::{resolved_name}")
            }
        };
        let extern_c = self.cpp
            && (self.extern_c
                || (start..header.name_start)
                    .any(|i| self.text(i) == "extern" && self.text(i + 1) == "\"C\"")
                || self.extern_c_names.contains(&qualified_name));
        if extern_c {
            self.extern_c_names.insert(qualified_name.clone());
        }
        let mut implicit_this = (self.cpp && definition)
            .then(|| {
                qualified_name
                    .rsplit_once("::")
                    .filter(|(owner, _)| self.class_scopes.contains(*owner))
                    .map(|(owner, _)| owner.replace("::", "."))
            })
            .flatten();
        let binding_signature = self.cpp.then(|| {
            let parameter_types = self
                .split_ranges(header.parameters_start, header.parameters_end, ",")
                .into_iter()
                .filter(|(a, b)| a < b)
                .map(|(a, b)| self.binding_parameter_type(a, b))
                .filter(|t| t != "void" && t != "...")
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{}({parameter_types})",
                self.binding_type(&return_binding_type)
            )
        });
        let duplicate_binding = if definition && self.cpp && !header.name.contains("::") {
            let mut qualifiers = String::new();
            let mut cursor = header.parameters_end + 1;
            while cursor < body_start {
                if let Some(after) = self.skip_attribute(cursor, body_start) {
                    cursor = after;
                    continue;
                }
                if matches!(self.text(cursor), "(" | "[") {
                    cursor = self
                        .matching(cursor, body_start)
                        .map_or(body_start, |close| close + 1);
                } else {
                    if matches!(self.text(cursor), "const" | "volatile" | "&" | "&&") {
                        qualifiers.push_str(self.text(cursor));
                    }
                    cursor += 1;
                }
            }
            !self.defined_cpp_methods.insert(format!(
                "{qualified_name}:{}:{qualifiers}",
                binding_signature.as_deref().unwrap_or_default()
            ))
        } else {
            false
        };
        if duplicate_binding {
            implicit_this = None;
        }
        let implicit_fields = if duplicate_binding {
            Vec::new()
        } else {
            qualified_name
                .rsplit_once("::")
                .and_then(|(owner, _)| self.class_fields.get(owner))
                .cloned()
                .unwrap_or_default()
        };
        let full_name = if duplicate_binding {
            format!("{}:<unresolvedSignature>", header.name.replace("::", "."))
        } else if self.cpp && !extern_c {
            format!(
                "{}:{}",
                qualified_name.replace("::", "."),
                binding_signature.as_deref().unwrap_or_default()
            )
        } else if extern_c {
            qualified_name
                .rsplit("::")
                .next()
                .unwrap_or(&qualified_name)
                .to_string()
        } else if (header.name_start..header.parameters_start - 1).any(|i| self.text(i) == ")") {
            // A C function's outer declarator has no name when only its name
            // is grouped. FullNameProvider falls back to unresolvedNamespace;
            // a parameter list inside the group still names the declarator.
            format!("<unresolvedNamespace>.{name}")
        } else {
            // C function bindings have a bare name, including declarations
            // encountered while converting a record's members.
            name.clone()
        };
        for parameter in &parameters {
            Rc::make_mut(&mut self.variables).insert(parameter.name.clone());
            Rc::make_mut(&mut self.variable_types)
                .insert(parameter.name.clone(), parameter.type_name.clone());
        }
        Rc::make_mut(&mut self.variables).insert(header.name.clone());
        self.function_full_name.clone_from(&full_name);
        self.function_fields.clone_from(&implicit_fields);
        let mut suffix = header.parameters_end + 1;
        let mut member_cv_qualified = false;
        while suffix < body_start && !matches!(self.text(suffix), "->" | ":") {
            if matches!(self.text(suffix), "const" | "volatile") {
                member_cv_qualified = true;
            }
            suffix = if matches!(self.text(suffix), "(" | "[") {
                self.matching(suffix, body_start)
                    .map_or(body_start, |close| close + 1)
            } else {
                suffix + 1
            };
        }
        self.function_member_cv = member_cv_qualified;
        if let Some(owner) = &implicit_this {
            Rc::make_mut(&mut self.variables).insert("this".into());
            Rc::make_mut(&mut self.variable_types).insert("this".into(), owner.clone());
        }
        if self.constructor_initializers
            && definition
            && !((header.parameters_end + 1..body_start).any(|i| self.text(i) == ":"))
            && qualified_name.rsplit_once("::").is_some_and(|(owner, _)| {
                owner.rsplit("::").next() == Some(name.as_str())
                    && (!self.scalar_constructor_classes.contains(owner)
                        || !implicit_fields.is_empty())
            })
        {
            self.diagnose(
                header.parameters_end + 1,
                body_start,
                "implicit constructor initialization effects are unsupported",
            );
        }
        let constructor_initializers = if definition {
            if let Some(colon) =
                (header.parameters_end + 1..body_start).find(|i| self.text(*i) == ":")
            {
                let owner = qualified_name.rsplit_once("::").map(|(owner, _)| owner);
                let plan = if self.constructor_initializers
                    && owner.is_some_and(|owner| {
                        self.scalar_constructor_classes.contains(owner)
                            && owner.rsplit("::").next() == Some(name.as_str())
                    }) {
                    self.scalar_member_initializers(
                        colon + 1,
                        body_start,
                        &implicit_fields,
                        functions,
                    )
                } else {
                    None
                };
                if let Some(plan) = plan {
                    if let Some(index) = initializer_guard {
                        self.diagnostics.remove(index);
                    }
                    plan
                } else {
                    if initializer_guard.is_none() {
                        self.diagnose(
                            header.parameters_end + 1,
                            body_start,
                            "constructor member-initializer CFG is unsupported",
                        );
                    }
                    Vec::new()
                }
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        self.function_returns
            .insert(name.clone(), return_type.clone());
        self.pos = body_start;
        let old_method = std::mem::replace(&mut self.current_method, full_name.clone());
        let old_ast_parent = std::mem::replace(&mut self.lambda_ast_parent, full_name.clone());
        self.in_function_body = definition;
        let original_body_end = definition
            .then(|| {
                self.matching(body_start, self.limit)
                    .or_else(|| self.brace_end(body_start, self.limit))
            })
            .flatten();
        self.asm_problem_recovery = false;
        let body = if definition {
            self.statement()
        } else {
            Stmt {
                kind: StmtKind::Empty,
                span: self.span(body_start, body_start),
            }
        };
        let end = if definition { self.pos } else { body_start + 1 };
        let mut leaked = Vec::new();
        if self.asm_problem_recovery
            && let Some(body_end) = original_body_end
        {
            // CDT closes the enclosing compound at a problem assembly brace.
            // The balanced source body still delimits recovery of later methods.
            leaked = self.asm_tail_declarations(end, body_end);
            self.pos = body_end + 1;
        }
        self.variables = old_variables;
        self.variable_types = old_variable_types;
        self.variable_closures = old_variable_closures;
        self.function_full_name = old_function_full_name;
        self.function_fields = old_function_fields;
        self.function_member_cv = old_function_member_cv;
        self.in_function_body = old_body_context;
        self.current_method = old_method;
        self.lambda_ast_parent = old_ast_parent;
        Rc::make_mut(&mut self.variables).insert(header.name.clone());
        for name in leaked {
            self.bind_object(name, prefix);
        }
        // Leading identifiers that CDT skipped are a separate problem node.
        let mut span = self.span(
            if recovered_return_type {
                return_start
            } else {
                start
            },
            end,
        );
        if !definition {
            // A declared method takes the position of its declarator, which
            // starts at the pointer operators or group before its name.
            let lower = header.shared.map_or(start, |(_, _, own)| own);
            let (mut declarator, mut candidate) = (header.name_start, header.name_start);
            while candidate > lower {
                let previous = self.text(candidate - 1);
                if matches!(previous, "*" | "&" | "&&" | "(") {
                    candidate -= 1;
                    declarator = candidate;
                } else if qualifier(previous) {
                    candidate -= 1;
                } else {
                    break;
                }
            }
            let declarator_span = self.span(declarator, declarator + 1);
            span.line = declarator_span.line;
            span.column = declarator_span.column;
        }
        functions.push(Function {
            name,
            full_name,
            binding_return_type: self.binding_type(&return_binding_type),
            return_type,
            signature,
            implicit_this,
            extern_c,
            lambda_parent: None,
            implicit_fields,
            member_cv_qualified,
            lambda: false,
            is_static: (start..header.name_start).any(|i| self.text(i) == "static"),
            is_virtual: (start..header.name_start).any(|i| self.text(i) == "virtual"),
            inherited_bindings: Vec::new(),
            inherited_closures: Vec::new(),
            parameters,
            constructor_initializers,
            body,
            span,
        });
    }

    pub(super) fn collect_global_expressions(&mut self, start: usize, end: usize) {
        let base_end = self.specifier_end(start, end);
        for (a, b) in self.split_ranges(base_end, end, ",") {
            let mut cursor = a;
            let mut ranges = Vec::new();
            while cursor < b {
                if self.text(cursor) == "=" {
                    ranges.push((cursor + 1, b));
                    break;
                }
                if matches!(self.text(cursor), "(" | "[" | "{") {
                    let Some(close) = self.matching(cursor, b) else {
                        break;
                    };
                    if self.text(cursor) == "[" {
                        ranges.push((cursor + 1, close));
                    }
                    if self.text(cursor) == "{" {
                        ranges.push((cursor, b));
                        break;
                    }
                    cursor = close + 1;
                } else {
                    cursor += 1;
                }
            }
            for (a, b) in ranges {
                if (a..b).any(|i| {
                    matches!(
                        self.text(i),
                        "__builtin_va_arg"
                            | "__builtin_offsetof"
                            | "__builtin_types_compatible_p"
                            | "__offsetof__"
                    )
                }) {
                    let expression = self.expression_range(a, b);
                    self.global_expressions.push(expression);
                }
            }
        }
    }

    pub(super) fn parameters(&mut self, start: usize, end: usize) -> Vec<Parameter> {
        let mut result = Vec::new();
        for (a, b) in self.split_ranges(start, end, ",") {
            if a == b {
                continue;
            }
            if self.text(a) == "..." {
                result.push(Parameter {
                    name: "...".into(),
                    type_name: "...".into(),
                    function_pointer: false,
                    knr: None,
                    span: self.span(a, b),
                });
                continue;
            }
            let base_end = self.specifier_end(a, b);
            self.remember_inferred_type(a, base_end);
            let (name_index, suffix) = self.declarator_name(base_end, b);
            let mut name = name_index
                .map(|i| self.text(i).to_string())
                .unwrap_or_default();
            let mut type_name = if self.text(suffix) == "(" {
                // CDT's parameter type helper reads only the outer
                // declarator's pointer operators, not those nested in a
                // function-pointer declarator or its parameter list.
                let mut type_name = self.clean_type(a, base_end);
                let mut i = base_end;
                while i < b && (matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i)))
                {
                    if matches!(self.text(i), "*" | "&" | "&&") {
                        type_name.push_str(self.text(i));
                    }
                    i += 1;
                }
                type_name
            } else {
                self.declarator_type(a, base_end, name_index, b)
            };
            // Without a typedef binding, CDT's C parser interprets a bare
            // identifier parameter as an old-style parameter name.
            if !self.cpp
                && name_index.is_none()
                && b > a
                && self.tokens[b - 1].kind == TokenKind::Identifier
                && !self.type_word(self.text(b - 1))
                && !self.explicit_types.contains(self.text(b - 1))
                && (a..b - 1).all(|i| qualifier(self.text(i)))
            {
                name = self.text(b - 1).into();
                type_name = if b == a + 1 {
                    "ANY".into()
                } else {
                    self.type_code(a, b - 1, true)
                };
            }
            // The parameter declaration ends with its declarator: attributes
            // after it are not part of CDT's node or of Joern's CODE.
            let mut code_end = b;
            'trailing: loop {
                let mut i = a + 1;
                while i < code_end {
                    match self.skip_attribute(i, code_end) {
                        Some(after) if after == code_end => {
                            code_end = i;
                            continue 'trailing;
                        }
                        Some(after) => i = after,
                        None => i += 1,
                    }
                }
                break;
            }
            result.push(Parameter {
                name,
                type_name,
                function_pointer: self.text(suffix) == "(",
                knr: None,
                span: self.span(a, code_end),
            });
        }
        result
    }

    pub(super) fn declarations_range(&mut self, start: usize, end: usize) -> Vec<Declaration> {
        if start == end {
            return Vec::new();
        }
        if self.declaration_shape_problem(start, end) {
            return Vec::new();
        }
        let base_end = self.specifier_end(start, end);
        let is_typedef = (start..base_end).any(|i| self.text(i) == "typedef");
        let (_, first_suffix) = self.declarator_name(base_end, end);
        // An ambiguous grouped object declaration does not establish a type
        // binding for its leading spelling, which may name a later callee.
        if !self.nested_declarator(base_end, end) || self.text(first_suffix) == "(" {
            self.remember_inferred_type(start, base_end);
        }
        let ranges = self.split_ranges(base_end, end, ",");
        if self.in_function_body
            && let Some(&(a, b)) = ranges.first()
        {
            let (name, suffix) = self.declarator_name(a, b);
            let header = self.function_header(a, b);
            let function_pointer = self.text(a) == "("
                && self.text(suffix) == "("
                && (a..suffix).any(|i| matches!(self.text(i), "*" | "&" | "&&"));
            if self.text(suffix) == "(" && (header.is_some() || function_pointer) {
                // AstForStatementsCreator routes a function-shaped first
                // declarator separately and ignores every following declarator.
                if (start..base_end).any(|i| self.text(i) == "typedef") {
                    self.remember_types(start, end);
                    return Vec::new();
                }
                if let Some(mut header) = header {
                    let base_type = self.clean_type(start, base_end);
                    let pointer_type = self.clean_type(a, header.name_start);
                    header.return_type = Some(format!("{base_type}{pointer_type}"));
                    let saved_pos = self.pos;
                    let mut declarations = Vec::new();
                    self.add_function(start, b, header, "", false, &mut declarations);
                    self.function_declarations.extend(declarations);
                    self.pos = saved_pos;
                    return Vec::new();
                }
                if let Some(name) = name {
                    let type_name = self.declarator_type(start, base_end, Some(name), b);
                    let name = self.text(name).to_string();
                    Rc::make_mut(&mut self.variables).insert(name.clone());
                    Rc::make_mut(&mut self.variable_types).insert(name.clone(), type_name.clone());
                    return vec![Declaration {
                        name,
                        type_name,
                        initializer: None,
                        dimensions: Vec::new(),
                        problem: self.cpp,
                        is_typedef: false,
                        nested_declarator: true,
                        span: self.span(a, b),
                    }];
                }
            }
        }
        let mut result = Vec::new();
        for (a, b) in ranges {
            if a == b {
                continue;
            }
            let (name_index, suffix_start) = self.declarator_name(a, b);
            let Some(name_index) = name_index else {
                // An unnamed struct/enum declaration has no executable declarator.
                if base_end < end {
                    self.diagnose(a, b, "cannot recover declaration name");
                }
                continue;
            };
            let name = self.text(name_index).to_string();
            let mut i = suffix_start;
            let mut dimension_ranges = Vec::new();
            let mut valid_dimensions = true;
            let mut initializer_start = None;
            while i < b {
                if let Some(after) = self.skip_attribute(i, b) {
                    i = after;
                    continue;
                }
                match self.text(i) {
                    "[" => {
                        if let Some(close) = self.matching(i, b) {
                            if close > i + 1 && self.text(i + 1) != "*" {
                                dimension_ranges.push((i + 1, close));
                            } else {
                                valid_dimensions = false;
                            }
                            i = close + 1;
                        } else {
                            break;
                        }
                    }
                    "=" => {
                        initializer_start = Some(i + 1);
                        break;
                    }
                    "{" => {
                        initializer_start = Some(i);
                        break;
                    }
                    "(" => {
                        let close = self.matching(i, b).unwrap_or(b.saturating_sub(1));
                        // C++ direct initialization; a pointer declarator's suffix is a type.
                        if self.cpp && a == name_index && self.function_header(a, b).is_none() {
                            initializer_start = Some(i + 1);
                        }
                        i = close + 1;
                        if initializer_start.is_some() {
                            break;
                        }
                    }
                    _ => i += 1,
                }
            }
            let dimensions = if valid_dimensions {
                dimension_ranges
                    .into_iter()
                    .map(|(a, b)| self.expression_range(a, b))
                    .collect()
            } else {
                Vec::new()
            };
            let initializer = initializer_start.map(|init| {
                let init_end = if init > a && self.text(init - 1) == "(" {
                    b.saturating_sub(1)
                } else {
                    b
                };
                self.initializer_expression_range(init, init_end)
            });
            let type_name = self.declarator_type(start, base_end, Some(name_index), b);
            if !is_typedef {
                Rc::make_mut(&mut self.variables).insert(name.clone());
                Rc::make_mut(&mut self.variable_types).insert(name.clone(), type_name.clone());
                if let Some(closure) = initializer
                    .as_ref()
                    .and_then(|value| self.closure_expression(value))
                    .cloned()
                {
                    Rc::make_mut(&mut self.variable_closures).insert(name.clone(), closure);
                } else {
                    Rc::make_mut(&mut self.variable_closures).remove(&name);
                }
            }
            // CDT ends a declarator before its trailing attributes; only an
            // initializer extends the node past them.
            let mut code_end = b;
            if initializer.is_none() && !self.cpp {
                let (mut i, mut last) = (a, a);
                while i < b {
                    if matches!(self.text(i), "__attribute__" | "__attribute")
                        && let Some(after) = self.skip_attribute(i, b)
                    {
                        i = after;
                        continue;
                    }
                    i = if matches!(self.text(i), "(" | "[" | "{") {
                        self.matching(i, b).map_or(i + 1, |close| close + 1)
                    } else {
                        i + 1
                    };
                    last = i;
                }
                if last > a {
                    code_end = last;
                }
            }
            result.push(Declaration {
                name,
                type_name,
                initializer,
                dimensions,
                problem: false,
                is_typedef,
                nested_declarator: self.nested_declarator(a, b),
                span: self.span(a, code_end),
            });
        }
        result
    }

    pub(super) fn nested_function_statement(&mut self, start: usize) -> Option<Stmt> {
        if !self.in_function_body {
            return None;
        }
        let mut head = start;
        loop {
            if let Some(after) = self.skip_attribute(head, self.limit) {
                head = after;
            } else if self.text(head) == "__extension__" {
                head += 1;
            } else {
                break;
            }
        }
        if matches!(
            self.text(head),
            "if" | "else"
                | "while"
                | "do"
                | "for"
                | "switch"
                | "case"
                | "default"
                | "return"
                | "break"
                | "continue"
                | "goto"
                | "try"
                | "catch"
                | "throw"
                | "asm"
                | "__asm"
                | "__asm__"
        ) || self.text(head + 1) == ":"
        {
            return None;
        }
        let mut body_start = start;
        while body_start < self.limit {
            match self.text(body_start) {
                "(" | "[" => body_start = self.matching(body_start, self.limit)? + 1,
                ";" | "=" | "}" | "%>" => return None,
                "{" | "<%" => break,
                _ => body_start += 1,
            }
        }
        if body_start == self.limit || (start..body_start).any(|i| self.text(i) == "typedef") {
            return None;
        }
        if !self.looks_declaration(head, body_start) {
            return None;
        }
        let header = self.function_header(start, body_start)?;
        let mut kind = StmtKind::Empty;
        if self.cpp {
            // A C++ local function definition is a problem declaration. CDT
            // skips its body without creating a method or executable node.
            self.pos = self.problem_declaration_end(body_start);
        } else {
            let parent = self.current_method.clone();
            let inherited_bindings: Vec<_> = self
                .variable_types
                .iter()
                .map(|(name, type_name)| (name.clone(), type_name.clone()))
                .collect();
            let inherited_closures: Vec<_> = self
                .variable_closures
                .iter()
                .map(|(name, closure)| (name.clone(), closure.clone()))
                .collect();
            let recovery = self.asm_problem_recovery;
            let mut functions = Vec::new();
            self.add_function(start, body_start, header, "", true, &mut functions);
            self.asm_problem_recovery = recovery;
            for function in &mut functions {
                function.lambda_parent = Some(parent.clone());
                function.inherited_bindings = inherited_bindings.clone();
                function.inherited_closures = inherited_closures.clone();
            }
            if let Some(function) = functions.first() {
                kind = StmtKind::FunctionDefinition {
                    name: function.name.clone(),
                    full_name: function.full_name.clone(),
                };
            }
            self.function_declarations.extend(functions);
        }
        Some(Stmt {
            kind,
            span: self.span(start, self.pos),
        })
    }

    /// Whether `T *name;` is a product for the binding of `name`: a local
    /// in scope or, in C, a file-scope object declared before or a function
    /// (`CScope`). A declared type `T` keeps the declaration.
    pub(super) fn resolves(&self, t: &str, name: &str) -> bool {
        self.variable_types.contains_key(name)
            || (!self.cpp
                && !self.explicit_types.contains(t)
                && (self.object_bindings.contains(name)
                    || self.function_returns.contains_key(name)
                    || self.later_functions.contains(name)))
    }

    /// The functions that a C `T *x;` statement names, as `T` or as `x`,
    /// before the first declaration among `functions`.
    pub(super) fn later_function_names(&self, functions: &[Function]) -> HashSet<String> {
        let mut names = HashSet::new();
        if self.cpp || !self.later_functions.is_empty() {
            return names;
        }
        let mut first = HashMap::new();
        for function in functions {
            let start = first.entry(function.name.as_str()).or_insert(usize::MAX);
            *start = (*start).min(function.span.start);
        }
        let end = self.tokens.len();
        for star in 1..end {
            if self.text(star) != "*"
                || !self.cdt_identifier(star - 1, end)
                || (star > 1
                    && !matches!(
                        self.text(star - 2),
                        ";" | "{" | "}" | ")" | ":" | "else" | "do"
                    ))
            {
                continue;
            }
            let mut name = star;
            while name < end && self.text(name) == "*" {
                name += 1;
            }
            if self.cdt_identifier(name, end) && self.text(name + 1) == ";" {
                let at = self.tokens[star - 1].span.start;
                names.extend(
                    [star - 1, name]
                        .into_iter()
                        .filter(|&i| first.get(self.text(i)).is_some_and(|&start| at < start))
                        .map(|i| self.text(i).to_string()),
                );
            }
        }
        names
    }

    pub(super) fn looks_declaration(&self, start: usize, end: usize) -> bool {
        let t = self.text(start);
        if start < end
            && self.tokens[start].kind == TokenKind::Identifier
            && !self.type_word(t)
            && !qualifier(t)
            && matches!(self.text(start + 1), "*" | "&" | "&&")
        {
            // Named C types cannot introduce reference declarators. CDT
            // retries those forms as bitwise or logical expressions.
            if !self.cpp && matches!(self.text(start + 1), "&" | "&&") {
                return false;
            }
            if let (Some(name), after) = self.declarator_name(start + 1, end) {
                // A pointer qualifier cannot be an expression operand, even
                // when the leading name also has an ordinary binding.
                if (start + 1..name).any(|i| qualifier(self.text(i)))
                    && (after == end || matches!(self.text(after), "=" | "{" | "[" | "("))
                {
                    return true;
                }
                let function = self.variables.contains(t) || self.later_functions.contains(t);
                if (function && !matches!(self.text(after), "=" | "{"))
                    || (after == end && self.resolves(t, self.text(name)))
                    // `T *x;` ties between its two readings once `T` binds
                    // to an external function, and the expression is first.
                    // By default it stays a declaration.
                    || (after == end
                        && self.cdt_failures
                        && !self.cpp
                        && self.external_functions.contains(t))
                    || matches!(
                        self.text(after),
                        "==" | "!="
                            | "<"
                            | ">"
                            | "<="
                            | ">="
                            | "&&"
                            | "||"
                            | "+"
                            | "-"
                            | "/"
                            | "%"
                            | "^"
                            | "|"
                            | "?"
                            | "."
                            | "->"
                    )
                {
                    return false;
                }
                if self.variables.contains(self.text(name))
                    && self.text(after) == "("
                    && let Some(close) = self.matching(after, end)
                    && close + 1 == end
                    && self
                        .split_ranges(after + 1, close, ",")
                        .iter()
                        .any(|&(a, b)| b == a + 1 && self.variable_types.contains_key(self.text(a)))
                {
                    return false;
                }
            }
        }
        if self.cpp && start + 1 < end {
            let operator = alternative_operator(self.text(start + 1));
            if operator != self.text(start + 1)
                && (!matches!(operator, "&" | "&&") || self.variables.contains(t))
            {
                return false;
            }
        }
        if !self.type_word(t)
            && !qualifier(t)
            && matches!(self.text(start + 1), "*" | "&" | "&&")
            && self.declarator_name(start + 2, end).0.is_none()
        {
            return false;
        }
        if self.cpp && matches!(t, "new" | "delete" | "noexcept") {
            return false;
        }
        if (self.cpp || t != "[")
            && let Some(after) = self.skip_attribute(start, end)
        {
            return self.looks_declaration(after, end);
        }
        if t == "__extension__" {
            return self.looks_declaration(start + 1, end);
        }
        if self.cpp {
            let base_end = self.specifier_end(start, end);
            if self.text(base_end) == "(" && self.matching(base_end, end) == Some(base_end + 1) {
                return false;
            }
        }
        if start < end
            && self.tokens[start].kind == TokenKind::Identifier
            && self.text(start + 1) == "("
            && let Some(close) = self.matching(start + 1, end)
            && let (Some(name), after) = self.declarator_name(start + 2, close)
            // CDT's C declarator may carry array suffixes inside the group.
            && (after == close
                || (!self.cpp
                    && self.text(close + 1) == "="
                    && self.cdt_declarator(start + 1, close + 1, false)
                        == Some((close + 1, true))))
            && !self.type_word(self.text(name))
            && !qualifier(self.text(name))
        {
            if self.text(close + 1) == "=" {
                // C resolves T(x)=value as a declaration even when T binds
                // to a function. C++ instead chooses a call expression when
                // the grouped name already has an object binding.
                return self.type_word(t)
                    || qualifier(t)
                    || !self.cpp
                    || !self.variables.contains(self.text(name));
            }
            if close + 1 == end && !self.type_word(t) && !qualifier(t) {
                // A syntactically named but unresolved type does not establish
                // a C typedef binding. C++ also accepts the declaration form
                // when neither its callee nor grouped name already binds.
                return !self.variables.contains(self.text(name))
                    && if self.cpp {
                        !self.variables.contains(t)
                    } else {
                        self.explicit_types.contains(t)
                    };
            }
        }
        if start < end
            && self.tokens[start].kind == TokenKind::Identifier
            && !self.type_word(t)
            && !qualifier(t)
            && self.text(start + 1) == "("
            && self
                .matching(start + 1, end)
                .is_some_and(|close| close + 1 == end)
        {
            // Empty, literal and multiple argument lists are calls rather
            // than grouped object declarators, including after named locals.
            return false;
        }
        if self.cpp {
            let after = self
                .cpp_expression_name_end(start, end, false)
                .unwrap_or(start + 1);
            if after > start + 1
                && matches!(
                    self.text(after),
                    "=" | "+="
                        | "-="
                        | "*="
                        | "/="
                        | "%="
                        | "&="
                        | "|="
                        | "^="
                        | "<<="
                        | ">>="
                        | "("
                        | "["
                        | "++"
                        | "--"
                )
            {
                return false;
            }
        }
        // A typedef and an object may share a spelling after CDT recovery.
        // Assignment, member selection and subscripting still start expressions.
        if start + 1 < end
            && matches!(
                self.text(start + 1),
                "=" | "."
                    | "->"
                    | "["
                    | "++"
                    | "--"
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | "<<="
                    | ">>="
                    | "+"
                    | "-"
                    | "/"
                    | "%"
                    | "=="
                    | "!="
                    | "<="
                    | ">="
                    | "||"
                    | "?"
            )
        {
            return false;
        }
        if start + 1 == end && self.variables.contains(t) {
            return false;
        }
        if let Some(declaration) = self.grouped_pointer_call_declaration(start, end) {
            return declaration;
        }
        if qualifier(t)
            || self.type_word(t)
            || matches!(
                t,
                "struct"
                    | "union"
                    | "enum"
                    | "class"
                    | "typeof"
                    | "__typeof__"
                    | "decltype"
                    | "using"
            )
            || self.types.contains(t)
        {
            return true;
        }
        if start < end && self.tokens[start].kind == TokenKind::Identifier {
            let mut i = start + 1;
            while i + 1 < end && self.text(i) == "::" {
                i += 2;
            }
            if self.cpp && self.text(i) == "<" {
                i = self.angle_close(i, end).map_or(i, |n| n + 1);
            }
            let mut pointer = i + 1;
            while let Some(after) = self.skip_attribute(pointer, end) {
                pointer = after;
            }
            let grouped_declarator = self.text(i) == "("
                && (self.text(pointer) == "*"
                    || (self.cpp && matches!(self.text(pointer), "&" | "&&")))
                && self
                    .matching(i, end)
                    .is_some_and(|close| matches!(self.text(close + 1), "(" | "["))
                && self
                    .split_ranges(i + 1, self.matching(i, end).unwrap_or(end), ",")
                    .len()
                    == 1;
            return i < end
                && ((self.tokens[i].kind == TokenKind::Identifier
                    && (!self.cpp || alternative_operator(self.text(i)) == self.text(i)))
                    || (!self.variables.contains(t)
                        && (matches!(self.text(i), "*" | "&" | "&&") || grouped_declarator)));
        }
        false
    }
}
