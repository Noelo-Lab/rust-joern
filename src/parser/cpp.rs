//! C++ names, templates, classes, lambdas and scopes.

use super::*;

impl Parser<'_> {
    pub(super) fn collect_class_fields(&self, start: usize, end: usize) -> Vec<(String, String)> {
        let mut fields = Vec::new();
        let mut at = start;
        while at < end {
            if matches!(self.text(at), "public" | "private" | "protected")
                && self.text(at + 1) == ":"
            {
                at += 2;
                continue;
            }
            let from = at;
            while at < end && !matches!(self.text(at), ";" | "{") {
                if matches!(self.text(at), "(" | "[") {
                    at = self.matching(at, end).map_or(end, |close| close + 1);
                } else {
                    at += 1;
                }
            }
            if at >= end {
                break;
            }
            let method = self.function_header(from, at).is_some();
            if self.text(at) == "{" {
                at = self.matching(at, end).map_or(end, |close| close + 1);
                if method {
                    continue;
                }
                while at < end && self.text(at) != ";" {
                    at += 1;
                }
            }
            if !method
                && !(from..at).any(|i| {
                    matches!(
                        self.text(i),
                        "static" | "typedef" | "using" | "struct" | "class" | "union" | "enum"
                    )
                })
            {
                let base = self.specifier_end(from, at);
                for (a, b) in self.split_ranges(base, at, ",") {
                    if let (Some(name), _) = self.declarator_name(a, b) {
                        fields.push((
                            self.text(name).into(),
                            self.declarator_type(from, base, Some(name), b),
                        ));
                    }
                }
            }
            at += 1;
        }
        fields
    }

    pub(super) fn scalar_constructor_class(
        &self,
        start: usize,
        open: usize,
        end: usize,
        fields: &[(String, String)],
    ) -> bool {
        if (start..open).any(|i| matches!(self.text(i), ":" | "union" | "template")) {
            return false;
        }
        let mut collected = Vec::new();
        let mut at = open + 1;
        while at < end {
            if matches!(self.text(at), "public" | "private" | "protected")
                && self.text(at + 1) == ":"
            {
                at += 2;
                continue;
            }
            if self.text(at) == ";" {
                at += 1;
                continue;
            }
            let from = at;
            while at < end && !matches!(self.text(at), ";" | "{") {
                if matches!(self.text(at), "(" | "[") {
                    let Some(close) = self.matching(at, end) else {
                        return false;
                    };
                    at = close + 1;
                } else {
                    at += 1;
                }
            }
            if at == end || (from..at).any(|i| self.text(i) == "virtual") {
                return false;
            }
            if (from..at).any(|i| self.text(i) == "(" && self.text(i + 1) == "*") {
                return false;
            }
            if self.function_header(from, at).is_some() {
                if self.text(at) == "{" {
                    let Some(close) = self.matching(at, end) else {
                        return false;
                    };
                    at = close;
                }
                at += 1;
                continue;
            }
            if (from..at).any(|i| matches!(self.text(i), "static" | "typedef" | "using")) {
                if self.text(at) != ";" {
                    return false;
                }
                at += 1;
                continue;
            }
            if self.text(at) != ";"
                || (from..at).any(|i| {
                    matches!(
                        self.text(i),
                        "[" | "]"
                            | "&"
                            | "&&"
                            | ":"
                            | "="
                            | "("
                            | "struct"
                            | "class"
                            | "union"
                            | "enum"
                            | "auto"
                    )
                })
            {
                return false;
            }
            let base = self.specifier_end(from, at);
            for (a, b) in self.split_ranges(base, at, ",") {
                let (Some(name), _) = self.declarator_name(a, b) else {
                    return false;
                };
                let ty = self.declarator_type(from, base, Some(name), b);
                let scalar = ty.split_whitespace().all(|word| {
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
                });
                // Only plain pointer declarators are admitted for nominal pointees.
                if !scalar && !(a..name).any(|i| self.text(i) == "*") {
                    return false;
                }
                collected.push((self.text(name).to_string(), ty));
            }
            at += 1;
        }
        collected == fields
            && fields
                .iter()
                .map(|(name, _)| name)
                .collect::<HashSet<_>>()
                .len()
                == fields.len()
    }

    pub(super) fn remember_class_alias(&mut self, name: &str) {
        if let Some(undo) = self.class_alias_undo.last_mut() {
            undo.entry(name.to_string()).or_insert_with(|| AliasUndo {
                alias: self.type_aliases.get(name).cloned(),
                was_type: self.types.contains(name),
                was_explicit: self.explicit_types.contains(name),
            });
        }
    }

    pub(super) fn resolve_class_name(&self, name: &str) -> Option<String> {
        let raw = name.trim_start_matches("::");
        if name.starts_with("::") {
            return self.class_scopes.contains(raw).then(|| raw.to_string());
        }
        let mut scope = self.lexical_scope.as_str();
        while !scope.is_empty() {
            let candidate = format!("{scope}::{raw}");
            if self.class_scopes.contains(&candidate) {
                return Some(candidate);
            }
            scope = scope.rsplit_once("::").map_or("", |(owner, _)| owner);
        }
        if self.class_scopes.contains(raw) {
            return Some(raw.to_string());
        }
        let (head, suffix) = raw
            .split_once("::")
            .map_or((raw, ""), |(head, tail)| (head, tail));
        if let Some(imported) = self.imports.get(head) {
            let candidate = if suffix.is_empty() {
                imported.clone()
            } else {
                format!("{imported}::{suffix}")
            };
            if self.class_scopes.contains(&candidate) {
                return Some(candidate);
            }
        }
        let candidates: HashSet<_> = self
            .using_namespaces
            .iter()
            .map(|namespace| format!("{namespace}::{raw}"))
            .filter(|candidate| self.class_scopes.contains(candidate))
            .collect();
        (candidates.len() == 1).then(|| candidates.into_iter().next().unwrap())
    }

    pub(super) fn template_id_end(&self, start: usize, end: usize) -> bool {
        let mut depth = 0usize;
        for i in (start..end).rev() {
            match self.text(i) {
                ">" => depth += 1,
                ">>" => depth += 2,
                "<" if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        return i > start && self.tokens[i - 1].kind == TokenKind::Identifier;
                    }
                }
                ";" | "=" => return false,
                _ => {}
            }
        }
        false
    }

    pub(super) fn lambda_end(&self, start: usize, end: usize) -> Option<usize> {
        if !self.cpp || self.text(start) != "[" {
            return None;
        }
        let mut i = self.matching(start, end)? + 1;
        if self.text(i) == "(" {
            i = self.matching(i, end)? + 1;
        }
        while matches!(
            self.text(i),
            "mutable" | "constexpr" | "consteval" | "noexcept"
        ) {
            let noexcept = self.text(i) == "noexcept";
            i += 1;
            if noexcept && self.text(i) == "(" {
                i = self.matching(i, end)? + 1;
            }
        }
        while let Some(after) = self.skip_attribute(i, end) {
            i = after;
        }
        if self.text(i) == "->" {
            i += 1;
            while i < end && self.text(i) != "{" {
                i = if self.text(i) == "(" {
                    self.matching(i, end)? + 1
                } else {
                    i + 1
                };
            }
        }
        (self.text(i) == "{")
            .then(|| self.matching(i, end).map(|close| close + 1))
            .flatten()
    }

    /// Consume a C++ expression name as components, including template-ids
    /// between scope separators, operator-function-ids and member destructor-ids.
    /// Template arguments are not executable comparison nodes.
    pub(super) fn cpp_expression_name_end(
        &self,
        start: usize,
        end: usize,
        member: bool,
    ) -> Option<usize> {
        self.cpp_expression_name(start, end, member)
            .map(|name| name.end)
    }

    pub(super) fn cpp_expression_name(
        &self,
        start: usize,
        end: usize,
        member: bool,
    ) -> Option<CppExpressionName> {
        if !self.cpp {
            return None;
        }
        let mut cursor = start + usize::from(self.text(start) == "::");
        loop {
            let destructor_marker = cursor;
            let destructor = member && alternative_operator(self.text(cursor)) == "~";
            if destructor {
                cursor += 1;
            }
            if cursor >= end
                || self.tokens.get(cursor)?.kind != TokenKind::Identifier
                || (destructor && self.text(cursor) == "operator")
            {
                return None;
            }
            let component = cursor;
            if self.text(cursor) == "operator" {
                cursor += 1;
                match self.text(cursor) {
                    "[" if self.text(cursor + 1) == "]" => cursor += 2,
                    "(" if self.text(cursor + 1) == ")" => cursor += 2,
                    "new" | "delete" => {
                        cursor += 1;
                        if self.text(cursor) == "[" && self.text(cursor + 1) == "]" {
                            cursor += 2;
                        }
                    }
                    "+" | "-" | "*" | "/" | "%" | "^" | "&" | "|" | "~" | "!" | "=" | "<" | ">"
                    | "+=" | "-=" | "*=" | "/=" | "%=" | "^=" | "&=" | "|=" | "<<" | ">>"
                    | "<<=" | ">>=" | "==" | "!=" | "<=" | ">=" | "<=>" | "&&" | "||" | "++"
                    | "--" | "," | "->*" | "->" => cursor += 1,
                    _ => return None,
                }
                if self.text(cursor) == "<"
                    && let Some(close) = self.angle_close(cursor, end)
                    && self.text(close + 1) == "("
                {
                    cursor = close + 1;
                }
            } else {
                cursor += 1;
                if self.text(cursor) == "<"
                    && (member
                        || component > start
                        || !self.variables.contains(self.text(component))
                        || self.function_returns.contains_key(self.text(component)))
                    && let Some(close) = self.angle_close(cursor, end)
                    && matches!(self.text(close + 1), "::" | "(")
                {
                    cursor = close + 1;
                }
            }
            if destructor || self.text(cursor) != "::" {
                return (cursor <= end).then_some(CppExpressionName {
                    end: cursor,
                    destructor: destructor.then_some(destructor_marker),
                });
            }
            cursor += 1;
            if self.text(cursor) == "template" {
                cursor += 1;
            }
        }
    }

    pub(super) fn closure_expression<'a>(&'a self, expression: &'a Expr) -> Option<&'a Closure> {
        closure_expression(expression, &self.variable_closures)
    }

    pub(super) fn lambda_expression(&mut self) -> Expr {
        let start = self.pos;
        let Some(capture_end) = self.matching(start, self.limit) else {
            self.diagnose(start, start + 1, "unterminated lambda capture list");
            self.pos += 1;
            return Expr {
                kind: ExprKind::Unknown("[".into()),
                span: self.span(start, self.pos),
            };
        };
        // Captures are not expression AST children in c2cpg 4.0.150, including
        // init captures. Their body identifiers use the surrounding scope.
        self.pos = capture_end + 1;
        let inherited_bindings = self
            .variable_types
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect();
        let inherited_closures = self
            .variable_closures
            .iter()
            .map(|(name, closure)| (name.clone(), closure.clone()))
            .collect();
        let name = format!("<lambda>{}", self.lambda_counter);
        self.lambda_counter += 1;
        let full_name = if self.function_full_name.is_empty() {
            name.clone()
        } else {
            format!("{}.{}", self.function_full_name, name)
        };
        let mut parameter_types = Vec::new();
        let parameters = if self.at("(") {
            let open = self.pos;
            let close = self.matching(open, self.limit).unwrap_or(self.limit);
            parameter_types = self
                .split_ranges(open + 1, close, ",")
                .into_iter()
                .filter(|(a, b)| a < b)
                .map(|(a, b)| self.binding_parameter_type(a, b))
                .filter(|ty| ty != "void" && ty != "...")
                .collect();
            let parameters = self.parameters(open + 1, close);
            self.pos = close.saturating_add(1).min(self.limit);
            parameters
        } else {
            Vec::new()
        };
        let mut declared_return = None;
        let mut callable_return = None;
        while self.pos < self.limit && !self.at("{") {
            if let Some(after) = self.skip_attribute(self.pos, self.limit) {
                self.pos = after;
            } else if self.eat("->") {
                let return_start = self.pos;
                let specifier_end = self.specifier_end(return_start, self.limit);
                self.pos = specifier_end;
                while matches!(self.peek(), "*" | "&" | "&&") || qualifier(self.peek()) {
                    self.pos += 1;
                }
                declared_return = Some(self.clean_type(return_start, specifier_end));
                callable_return = Some(self.binding_type(&self.clean_type(return_start, self.pos)));
            } else if self.at("(") {
                self.pos = self
                    .matching(self.pos, self.limit)
                    .map_or(self.limit, |close| close + 1);
            } else {
                self.pos += 1;
            }
        }
        if !self.at("{") {
            self.diagnose(start, self.pos, "lambda requires a compound body");
            return Expr {
                kind: ExprKind::Unknown(self.raw(start, self.pos)),
                span: self.span(start, self.pos),
            };
        }
        let old_variables = self.variables.clone();
        for parameter in &parameters {
            Rc::make_mut(&mut self.variables).insert(parameter.name.clone());
        }
        let parent = self.current_method.clone();
        let old_method = std::mem::replace(&mut self.current_method, full_name.clone());
        let body = self.statement();
        self.current_method = old_method;
        self.variables = old_variables;
        let return_type = declared_return.unwrap_or_else(|| "ANY".into());
        let signature = format!(
            "{}({})",
            return_type,
            parameters
                .iter()
                .map(|parameter| parameter.type_name.as_str())
                .collect::<Vec<_>>()
                .join(",")
        );
        let mut types = self.variable_types.as_ref().clone();
        for (name, ty) in &self.function_fields {
            types.entry(name.clone()).or_insert_with(|| ty.clone());
        }
        for parameter in &parameters {
            types.insert(parameter.name.clone(), parameter.type_name.clone());
        }
        let mut returns = Vec::new();
        infer_return_types(
            &body,
            &mut types,
            &mut self.variable_closures.as_ref().clone(),
            &self.function_returns,
            &mut returns,
        );
        let call_return = callable_return.unwrap_or_else(|| {
            returns
                .into_iter()
                .reduce(common_type)
                .unwrap_or_else(|| "void".into())
        });
        let closure = Closure {
            full_name: full_name.clone(),
            return_type: self.binding_type(&call_return),
            parameter_types,
        };
        self.function_declarations.push(Function {
            name,
            full_name,
            binding_return_type: self.binding_type(&return_type),
            return_type,
            signature,
            extern_c: false,
            lambda_parent: Some(parent),
            implicit_this: None,
            implicit_fields: self.function_fields.clone(),
            member_cv_qualified: self.function_member_cv,
            lambda: true,
            is_static: false,
            is_virtual: false,
            inherited_bindings,
            inherited_closures,
            parameters,
            constructor_initializers: Vec::new(),
            body,
            span: self.span(start, self.pos),
        });
        Expr {
            kind: ExprKind::Lambda(closure),
            span: self.span(start, self.pos),
        }
    }
}
