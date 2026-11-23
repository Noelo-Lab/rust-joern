//! Type specifiers, declarators and type bindings.

use super::*;

impl Parser<'_> {
    /// A builtin type specifier of the parsed language. C has no `bool` or
    /// `wchar_t` keyword: without a typedef they are ordinary identifiers, so
    /// `(bool)(x)` is a call through a parenthesized name, as for any
    /// unknown `(T)(x)`.
    pub(super) fn type_word(&self, text: &str) -> bool {
        type_word(text) && (self.cpp || !c_typedef_name(text))
    }

    pub(super) fn return_specifier_start(&self, start: usize, name_start: usize) -> (usize, bool) {
        let mut cursor = start;
        if self.text(cursor) == "template"
            && self.text(cursor + 1) == "<"
            && let Some(close) = self.angle_close(cursor + 1, name_start)
        {
            cursor = close + 1;
        }
        let original = cursor;
        if !self.cpp {
            // CDT retries a failed declaration after each leading identifier
            // and keeps the first start from which the declarator is reached.
            let reaches = |mut from: usize| loop {
                from = self.cdt_attributes(from, name_start);
                if from >= name_start {
                    break true;
                }
                if !matches!(
                    self.text(from),
                    "*" | "(" | "const" | "volatile" | "restrict" | "__const" | "__restrict"
                ) {
                    break false;
                }
                from += 1;
            };
            let mut retry = cursor;
            while retry < name_start {
                if self
                    .cdt_decl_specifiers(retry, name_start, true)
                    .is_some_and(|(end, alternative)| {
                        reaches(end) || alternative.is_some_and(reaches)
                    })
                {
                    return (retry, retry != original);
                }
                if !self.cdt_identifier(retry, name_start) {
                    break;
                }
                retry += 1;
            }
        }
        while cursor + 1 < name_start
            && self.tokens[cursor].kind == TokenKind::Identifier
            && self.tokens[cursor + 1].kind == TokenKind::Identifier
            && !self.explicit_types.contains(self.text(cursor))
            && !self.type_word(self.text(cursor))
            && !qualifier(self.text(cursor))
            && !self.type_word(self.text(cursor + 1))
            && !qualifier(self.text(cursor + 1))
            && self.skip_attribute(cursor + 1, name_start).is_none()
        {
            // Leading unbound specifiers become separate CDT problem nodes.
            cursor += 1;
        }
        (cursor, cursor != original)
    }

    pub(super) fn remember_types(&mut self, start: usize, end: usize) {
        for i in start..end {
            if self.cpp
                && matches!(self.text(i), "struct" | "union" | "enum" | "class")
                && self
                    .tokens
                    .get(i + 1)
                    .is_some_and(|t| t.kind == TokenKind::Identifier)
            {
                self.types.insert(self.text(i + 1).to_string());
                self.explicit_types.insert(self.text(i + 1).to_string());
            }
        }
        if self.text(start) == "using" && self.text(start + 2) == "=" {
            let alias = self.text(start + 1).to_string();
            self.remember_class_alias(&alias);
            self.types.insert(self.text(start + 1).to_string());
            self.explicit_types.insert(self.text(start + 1).to_string());
            self.type_aliases.insert(
                self.text(start + 1).to_string(),
                self.clean_type(start + 3, end),
            );
        }
        if self.cpp
            && self.text(start) == "using"
            && self.text(start + 1) != "namespace"
            && self.text(start + 2) != "="
            && end > start + 2
            && self.text(end - 2) == "::"
        {
            let name = self.text(end - 1).to_string();
            let qualified = (start + 1..end).map(|i| self.text(i)).collect::<String>();
            self.imports.insert(name, qualified);
        }
        if self.cpp && self.text(start) == "using" && self.text(start + 1) == "namespace" {
            self.using_namespaces
                .push((start + 2..end).map(|i| self.text(i)).collect());
        }
        if (start..end).any(|i| self.text(i) == "typedef") {
            let base_end = self.specifier_end(start, end);
            self.remember_inferred_type(start, base_end);
            for (a, b) in self.split_ranges(base_end, end, ",") {
                if let (Some(name), _) = self.declarator_name(a, b) {
                    let alias = self.text(name).to_string();
                    self.remember_class_alias(&alias);
                    self.types.insert(self.text(name).to_string());
                    self.explicit_types.insert(self.text(name).to_string());
                    self.type_aliases.insert(
                        self.text(name).to_string(),
                        self.declarator_type(start, base_end, Some(name), b),
                    );
                }
            }
        }
    }

    pub(super) fn binding_parameter_type(&self, start: usize, end: usize) -> String {
        if self.text(start) == "..." {
            return "...".into();
        }
        let base_end = self.specifier_end(start, end);
        let (name, suffix) = self.declarator_name(base_end, end);
        let mut group = base_end;
        while group < end
            && (matches!(self.text(group), "*" | "&" | "&&") || qualifier(self.text(group)))
        {
            group += 1;
        }
        let grouped = self.text(group) == "(";
        let pointer = if grouped {
            (group + 1..name.unwrap_or(group + 1))
                .filter(|i| matches!(self.text(*i), "*" | "&" | "&&"))
                .map(|i| self.text(i))
                .collect::<String>()
        } else {
            String::new()
        };
        if self.text(suffix) == "(" {
            let close = self.matching(suffix, end).unwrap_or(end);
            let parameters = self
                .split_ranges(suffix + 1, close, ",")
                .into_iter()
                .filter(|(a, b)| a < b)
                .map(|(a, b)| self.binding_parameter_type(a, b))
                .filter(|t| t != "void" && t != "...")
                .collect::<Vec<_>>()
                .join(",");
            let return_type = format!(
                "{}{}",
                self.clean_type(start, base_end),
                (base_end..group)
                    .filter(|i| matches!(self.text(*i), "*" | "&" | "&&"))
                    .map(|i| self.text(i))
                    .collect::<String>()
            );
            return format!(
                "{}({})({parameters})",
                self.binding_type(&return_type),
                if pointer.is_empty() { "*" } else { &pointer }
            );
        }
        if self.text(suffix) == "[" {
            let mut after = suffix;
            while self.text(after) == "[" {
                after = self.matching(after, end).map_or(end, |close| close + 1);
            }
            let base = self.binding_type(&self.clean_type(start, base_end));
            if grouped {
                return format!(
                    "{base}({pointer}){}",
                    self.raw(suffix, after).replace(' ', "")
                );
            }
            let first_end = self.matching(suffix, end).map_or(after, |close| close + 1);
            return if first_end == after {
                format!("{base}*")
            } else {
                format!("{base}(*){}", self.raw(first_end, after).replace(' ', ""))
            };
        }
        self.binding_type(&self.declarator_type(start, base_end, name, end))
    }

    pub(super) fn binding_type(&self, raw_type: &str) -> String {
        let mut raw = raw_type.trim().to_string();
        for _ in 0..16 {
            let split = raw.find(['*', '&', '[', '(']).unwrap_or(raw.len());
            let base = raw[..split].trim();
            let Some(alias) = self.type_aliases.get(base) else {
                break;
            };
            let replaced = format!("{alias}{}", &raw[split..]);
            if replaced == raw {
                break;
            }
            raw = replaced;
        }
        let split = raw.find(['*', '&', '[', '(']).unwrap_or(raw.len());
        let mut base = raw[..split].trim().to_string();
        for keyword in ["struct ", "class ", "union ", "enum ", "const "] {
            if let Some(rest) = base.strip_prefix(keyword) {
                base = rest.to_string();
            }
        }
        let class_type = self.cpp.then(|| self.resolve_class_name(&base)).flatten();
        if let Some(qualified) = &class_type {
            base.clone_from(qualified);
        }
        base = match base.as_str() {
            "signed" | "signed int" => "int".into(),
            "unsigned" => "unsigned int".into(),
            "short" | "signed short" | "signed short int" => "short int".into(),
            "unsigned short" => "unsigned short int".into(),
            "long" | "signed long" | "signed long int" => "long int".into(),
            "unsigned long" => "unsigned long int".into(),
            "long long" | "signed long long" | "signed long long int" => "long long int".into(),
            "unsigned long long" => "unsigned long long int".into(),
            _ => base,
        };
        if !base.split_whitespace().all(|word| self.type_word(word))
            && !self.explicit_types.contains(&base)
            && !self.class_scopes.contains(&base)
            && !base.contains("::")
        {
            return "ANY".into();
        }
        let mut result = format!("{base}{}", &raw[split..]);
        if result.starts_with("unsigned ") || result.starts_with("volatile ") {
            let space = result.find(' ').unwrap();
            result = format!(
                "{}{}",
                &result[..=space],
                result[space + 1..].replace(' ', "")
            );
        } else if result.contains(['*', '[']) || base.contains("::") {
            result = result.replace(' ', "");
        } else if raw[split..].contains('&') {
            result = format!("{base} {}", raw[split..].trim());
        }
        result.replace("::", ".")
    }

    pub(super) fn specifier_end(&self, start: usize, end: usize) -> usize {
        let mut i = start;
        let mut has_type = false;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            let t = self.text(i);
            if i > start
                && self.text(i - 1) == "extern"
                && self.tokens[i].kind == TokenKind::Literal
            {
                i += 1;
                continue;
            }
            if qualifier(t) {
                i += 1;
                continue;
            }
            // `class` is an ordinary identifier in C.
            if matches!(t, "struct" | "union" | "enum") || (self.cpp && t == "class") {
                has_type = true;
                i += 1;
                while let Some(after) = self.skip_attribute(i, end) {
                    i = after;
                }
                if self
                    .tokens
                    .get(i)
                    .is_some_and(|t| t.kind == TokenKind::Identifier)
                {
                    i += 1;
                }
                if self.text(i) == "{" {
                    i = self.matching(i, end).map_or(i + 1, |n| n + 1);
                }
                continue;
            }
            if self.type_word(t) {
                has_type = true;
                i += 1;
                continue;
            }
            if matches!(
                t,
                "typeof" | "__typeof__" | "__typeof" | "decltype" | "_Atomic"
            ) && self.text(i + 1) == "("
            {
                has_type = true;
                i = self.matching(i + 1, end).map_or(i + 1, |n| n + 1);
                continue;
            }
            if !has_type && (self.tokens[i].kind == TokenKind::Identifier || t == "::") {
                has_type = true;
                i += 1;
                while i + 1 < end && self.text(i) == "::" {
                    i += 2;
                }
                if self.cpp && self.text(i) == "<" {
                    i = self.angle_close(i, end).map_or(i, |n| n + 1);
                }
                continue;
            }
            break;
        }
        i
    }

    pub(super) fn declarator_name(&self, start: usize, end: usize) -> (Option<usize>, usize) {
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            if matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i)) {
                i += 1;
                continue;
            }
            if self.text(i) == "(" {
                if let Some(close) = self.matching(i, end) {
                    let (name, _) = self.declarator_name(i + 1, close);
                    return (name, close + 1);
                }
                return (None, i);
            }
            if self.tokens[i].kind == TokenKind::Identifier {
                while i + 2 < end && self.text(i + 1) == "::" {
                    i += 2;
                }
                return (Some(i), i + 1);
            }
            break;
        }
        (None, i)
    }

    pub(super) fn nested_declarator(&self, start: usize, end: usize) -> bool {
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
            } else if matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i)) {
                i += 1;
            } else {
                break;
            }
        }
        self.text(i) == "("
    }

    /// `ASTStringUtil.getSignatureString` of an elaborated type specifier
    /// such as `const struct S`, which is Joern's return type for a definition.
    pub(super) fn elaborated_return_display(&self, start: usize, end: usize) -> Option<String> {
        let start = if self.text(start) == "template" && self.text(start + 1) == "<" {
            self.angle_close(start + 1, end)? + 1
        } else {
            start
        };
        let keyword = (start..end)
            .find(|&i| matches!(self.text(i), "struct" | "union" | "enum" | "class"))?;
        let mut name = String::new();
        let mut i = keyword + 1;
        while let Some(after) = self.skip_attribute(i, end) {
            i = after;
        }
        while i < end
            && (self.tokens[i].kind == TokenKind::Identifier && !qualifier(self.text(i))
                || self.text(i) == "::")
            && self.skip_attribute(i, end).is_none()
        {
            name.push_str(self.text(i));
            i += 1;
        }
        if name.is_empty() {
            return None;
        }
        if self.text(i) == "<"
            && let Some(close) = self.angle_close(i, end)
        {
            name.push_str(&(i..=close).map(|at| self.text(at)).collect::<String>());
        }
        if (start..end).any(|i| self.text(i) == "{") {
            // A composite or enumeration specifier has only its simple name.
            return Some(simple_type_name(&name).to_string());
        }
        let mut out = String::new();
        for (word, spellings) in [
            ("const", &["const"][..]),
            ("volatile", &["volatile", "__volatile__"][..]),
        ] {
            if (start..end).any(|i| spellings.contains(&self.text(i))) {
                out.push_str(word);
                out.push(' ');
            }
        }
        out.push_str(self.text(keyword));
        out.push(' ');
        out.push_str(&name);
        Some(joern_register_type(&out))
    }

    /// The `(*)(int)` that CDT's return type string appends for a definition
    /// whose grouped declarator returns a function pointer.
    pub(super) fn function_pointer_return(
        &self,
        group: usize,
        name: usize,
        limit: usize,
    ) -> Option<String> {
        if self.text(group) != "(" {
            return None;
        }
        let close = self.matching(group, limit)?;
        let pointers: String = (group + 1..name)
            .map(|i| self.text(i))
            .filter(|text| matches!(*text, "*" | "&" | "&&"))
            .collect();
        if pointers.is_empty() || self.text(close + 1) != "(" {
            return None;
        }
        let parameters_close = self.matching(close + 1, limit)?;
        let parameters = self
            .split_ranges(close + 2, parameters_close, ",")
            .into_iter()
            .filter(|(a, b)| a < b)
            .map(|(a, b)| {
                let (name, _) = self.declarator_name(self.specifier_end(a, b), b);
                let mut out = String::new();
                for i in (a..b).filter(|&i| Some(i) != name) {
                    let text = self.text(i);
                    if !out.is_empty() && !matches!(text, "*" | "&" | "&&" | "[" | "]" | ")") {
                        out.push(' ');
                    }
                    out.push_str(text);
                }
                out
            })
            .collect::<Vec<_>>()
            .join(", ");
        Some(format!("({pointers})({parameters})"))
    }

    /// returnTypeForIASTFunctionDeclarator: the declaration specifier's
    /// signature and the declarator's raw pointer operators, each cleaned.
    pub(super) fn prototype_return_display(
        &self,
        start: usize,
        base_end: usize,
        declarator: usize,
    ) -> Option<String> {
        let mut storage = None;
        let (mut constant, mut inline, mut restrict, mut volatile) = (false, false, false, false);
        let mut words = Vec::new();
        let mut named = String::new();
        let mut i = start;
        while i < base_end {
            if let Some(after) = self.skip_attribute(i, base_end) {
                i = after;
                continue;
            }
            let text = self.text(i);
            match text {
                "struct" | "union" | "enum" | "class" | "typename" | "decltype" | "(" | "{" => {
                    return None;
                }
                "static" | "extern" | "register" | "typedef" | "mutable" => {
                    if text == "extern" && self.tokens[i + 1].kind == TokenKind::Literal {
                        i += 1;
                    } else {
                        storage.get_or_insert(text);
                    }
                }
                "const" => constant = true,
                "inline" | "__inline" | "__inline__" => inline = true,
                "restrict" | "__restrict" | "__restrict__" => restrict = true,
                "volatile" | "__volatile__" => volatile = true,
                "__extension__" | "_Noreturn" | "__noreturn" | "__cdecl" | "__fastcall"
                | "__stdcall" | "__thiscall" | "constexpr" | "consteval" | "friend" | "virtual"
                | "explicit" => {}
                "<" if !named.is_empty() => {
                    // A template-id keeps its arguments without spaces.
                    let close = self.angle_close(i, base_end)?;
                    named.push_str(&(i..=close).map(|at| self.text(at)).collect::<String>());
                    i = close;
                }
                _ if self.type_word(text) => words.push(text),
                _ => {
                    if !named.is_empty()
                        && !named.ends_with("::")
                        && !matches!(text, "::" | "<" | ">" | ",")
                        && !named.ends_with(['<', ','])
                    {
                        return None;
                    }
                    named.push_str(text);
                }
            }
            i += 1;
        }
        let specifier = if named.is_empty() {
            // An empty C specifier has the signature of its modifiers alone.
            if (words.is_empty() && self.cpp)
                || words
                    .iter()
                    .any(|word| matches!(*word, "auto" | "__auto_type"))
            {
                return None;
            }
            // ASTSignatureUtil.getSignature for a simple declaration specifier.
            let mut out: Vec<&str> = storage.into_iter().collect();
            for (present, word) in [
                (constant, "const"),
                (inline, "inline"),
                (restrict, "restrict"),
                (volatile, "volatile"),
            ] {
                if present {
                    out.push(word);
                }
            }
            let longs = words.iter().filter(|word| **word == "long").count();
            if longs >= 2 {
                out.push("long long");
            }
            for word in ["_Complex", "_Imaginary"] {
                if words.contains(&word) {
                    out.push(word);
                }
            }
            if longs == 1 {
                out.push("long");
            }
            for word in ["short", "signed", "unsigned"] {
                if words.contains(&word) {
                    out.push(word);
                }
            }
            for word in &words {
                // A repeated specifier such as dewolf's `void void` is one type.
                if !matches!(
                    *word,
                    "long" | "short" | "signed" | "unsigned" | "_Complex" | "_Imaginary"
                ) && !out.contains(word)
                {
                    out.push(word);
                }
            }
            out.join(" ")
        } else if words.is_empty() {
            // ASTStringUtil.getReturnTypeString for a named type specifier.
            format!(
                "{}{}{named}",
                if constant { "const " } else { "" },
                if volatile { "volatile " } else { "" }
            )
        } else {
            return None;
        };
        let mut out = joern_clean_type(&specifier);
        let mut i = declarator;
        loop {
            if let Some(after) = self.skip_attribute(i, self.limit) {
                i = after;
                continue;
            }
            let text = self.text(i);
            if matches!(text, "*" | "&" | "&&") {
                out.push_str(text);
            } else if matches!(
                text,
                "__cdecl" | "__fastcall" | "__pascal" | "__stdcall" | "__extension__"
            ) {
                // Macros of the scanner configuration: attributes or nothing.
            } else if qualifier(text) {
                // A pointer operator's raw signature keeps its qualifiers.
                out.push(' ');
                out.push_str(text);
            } else if text != "__thiscall" {
                // The declarator's name, or the group that nests it.
                break;
            }
            i += 1;
        }
        Some(joern_register_type(&joern_clean_type(out.trim())))
    }

    pub(super) fn clean_type(&self, start: usize, end: usize) -> String {
        self.type_code(start, end, false)
    }

    pub(super) fn type_code(&self, start: usize, end: usize, keep_qualifiers: bool) -> String {
        let mut pieces = Vec::new();
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            let t = self.text(i);
            if i > start
                && self.text(i - 1) == "extern"
                && self.tokens[i].kind == TokenKind::Literal
            {
                i += 1;
                continue;
            }
            if !matches!(
                t,
                "static"
                    | "extern"
                    | "inline"
                    | "__inline"
                    | "__inline__"
                    | "register"
                    | "typedef"
                    | "constexpr"
                    | "consteval"
                    | "friend"
                    | "virtual"
                    | "__extension__"
                    | "__cdecl"
                    | "__fastcall"
                    | "__stdcall"
                    | "__thiscall"
                    | "__noreturn"
                    | "_Noreturn"
                    | "struct"
                    | "class"
                    | "enum"
                    | "__restrict"
                    | "__restrict__"
                    | "restrict"
            ) && (keep_qualifiers || !matches!(t, "const" | "volatile" | "__volatile__"))
            {
                pieces.push(if t == "__volatile__" { "volatile" } else { t });
            }
            i += 1;
        }
        let mut out = String::new();
        for t in pieces {
            if !out.is_empty()
                && !matches!(t, "*" | "&" | "&&" | "[" | "]" | "::" | ">" | "," | ")")
                && !out.ends_with([':', '<', '(', '['])
            {
                out.push(' ');
            }
            out.push_str(t);
        }
        out
    }

    pub(super) fn declarator_type(
        &self,
        start: usize,
        base_end: usize,
        name: Option<usize>,
        end: usize,
    ) -> String {
        let mut out = self.clean_type(start, base_end);
        let suffix_end = (base_end..end)
            .find(|i| matches!(self.text(*i), "=" | "{"))
            .unwrap_or(end);
        for i in base_end..suffix_end {
            if Some(i) != name && matches!(self.text(i), "*" | "&" | "&&") {
                out.push_str(self.text(i));
            }
        }
        let mut i = name.map_or(suffix_end, |n| n + 1);
        while i < suffix_end {
            if self.text(i) == "["
                && let Some(close) = self.matching(i, suffix_end)
            {
                out.push_str(&self.raw(i, close + 1));
                i = close + 1;
                continue;
            }
            i += 1;
        }
        if out.is_empty() { "ANY".into() } else { out }
    }

    /// C statements shaped `T (*x)(args)...;` with an identifier `T` are
    /// ambiguous. CDT resolves them by `T`'s binding: without a typedef they
    /// are call expressions, and with one they stay declarations unless
    /// consecutive parameter lists declare a function returning a function.
    /// Groups that cannot be call arguments, such as type keywords or
    /// adjacent names, keep the declaration.
    pub(super) fn grouped_pointer_call_declaration(
        &self,
        start: usize,
        end: usize,
    ) -> Option<bool> {
        let t = self.text(start);
        if self.cpp
            || self.tokens[start].kind != TokenKind::Identifier
            || self.type_word(t)
            || qualifier(t)
            || self.text(start + 1) != "("
            || self.text(start + 2) != "*"
        {
            return None;
        }
        let close = self.matching(start + 1, end)?;
        let (name, after) = self.declarator_name(start + 2, close);
        if name.is_none() || after != close || !matches!(self.text(close + 1), "(" | "[") {
            return None;
        }
        let mut i = close + 1;
        let mut previous_call = false;
        let mut consecutive_calls = false;
        while i < end {
            let open = i;
            if self.text(open) == "=" {
                // An initializer follows. Only a left side that also reads
                // as an expression is ambiguous: no pointer qualifier and
                // no empty subscript. A C call is no lvalue, so CDT keeps
                // the declaration when that expression ends in one.
                if (start + 2..close).any(|k| qualifier(self.text(k)))
                    || (close + 1..open).any(|k| self.text(k) == "[" && self.text(k + 1) == "]")
                {
                    return None;
                }
                if previous_call {
                    return Some(true);
                }
                break;
            }
            let group_end = self.matching(open, end)?;
            if self.text(open) == "(" {
                let adjacent_names = (open + 1..group_end - 1).any(|k| {
                    self.tokens[k].kind == TokenKind::Identifier
                        && self.tokens[k + 1].kind == TokenKind::Identifier
                });
                if adjacent_names
                    || (open + 1..group_end).any(|k| {
                        let text = self.text(k);
                        text == "..." || self.type_word(text) || qualifier(text)
                    })
                {
                    return None;
                }
                consecutive_calls |= previous_call;
                previous_call = true;
            } else if self.text(open) == "[" {
                previous_call = false;
            } else {
                return None;
            }
            i = group_end + 1;
        }
        Some(self.explicit_types.contains(t) && !consecutive_calls)
    }

    pub(super) fn is_type_range(&self, start: usize, end: usize) -> bool {
        if start == end {
            return false;
        }
        let first = self.text(start);
        if first == "__extension__" {
            // CDT's predefined extension marker has an empty replacement;
            // it does not make the following statement-expression a type-id.
            return self.is_type_range(start + 1, end);
        }
        // A type-id begins with a declaration specifier, never with an
        // expression's dereference or parenthesized operand. In particular,
        // '(*(code *)(p))()' must retain its pointer-call receiver.
        if self.tokens[start].kind != TokenKind::Identifier && first != "::" {
            return false;
        }
        let known = self.type_word(first)
            || qualifier(first)
            || self.types.contains(first)
            || matches!(
                first,
                "struct" | "union" | "enum" | "class" | "typeof" | "__typeof__" | "decltype"
            );
        let base_end = self.specifier_end(start, end);
        let Some(pointer) = self.abstract_type_declarator(base_end, end) else {
            return false;
        };
        if !known && !pointer && self.variables.contains(first) {
            return false;
        }
        if !known && !pointer {
            return end == start + 1
                && self.tokens[start].kind == TokenKind::Identifier
                && !self.variables.contains(first);
        }
        true
    }

    pub(super) fn abstract_type_declarator(&self, start: usize, end: usize) -> Option<bool> {
        let mut groups = vec![(start, end)];
        let mut pointer = false;
        while let Some((start, end)) = groups.pop() {
            let mut i = start;
            // Pointer operators precede the direct declarator. Stars inside
            // array bounds or function parameters are not pointer evidence.
            while i < end {
                if let Some(after) = self.skip_attribute(i, end) {
                    i = after;
                } else if self.cpp && self.text(i + 1) == "::" {
                    while i + 2 < end && self.text(i + 1) == "::" {
                        i += 2;
                    }
                    if self.text(i) != "*" {
                        return None;
                    }
                    pointer = true;
                    i += 1;
                } else if self.text(i) == "*" || (self.cpp && matches!(self.text(i), "&" | "&&")) {
                    pointer = true;
                    i += 1;
                } else if qualifier(self.text(i)) {
                    i += 1;
                } else {
                    break;
                }
            }
            if self.text(i) == "(" {
                let close = self.matching(i, end)?;
                let first = self.text(i + 1);
                if matches!(first, "*" | "(" | "[")
                    || (self.cpp && (matches!(first, "&" | "&&") || self.text(i + 2) == "::"))
                {
                    groups.push((i + 1, close));
                } else if !self.abstract_type_parameters(i + 1, close) {
                    return None;
                }
                i = close + 1;
            }
            // A completed direct declarator has only function/array suffixes;
            // a subsequent multiplication belongs to an expression.
            while i < end {
                if let Some(after) = self.skip_attribute(i, end) {
                    i = after;
                    continue;
                }
                match self.text(i) {
                    "(" | "[" => {
                        let close = self.matching(i, end)?;
                        if self.text(i) == "(" && !self.abstract_type_parameters(i + 1, close) {
                            return None;
                        }
                        i = close + 1;
                    }
                    "&" | "&&" if self.cpp => {
                        pointer = true;
                        i += 1;
                    }
                    t if self.cpp && qualifier(t) => i += 1,
                    _ => return None,
                }
            }
        }
        Some(pointer)
    }

    pub(super) fn abstract_type_parameters(&self, start: usize, end: usize) -> bool {
        self.split_ranges(start, end, ",")
            .into_iter()
            .all(|(a, b)| {
                if a == b || self.text(a) == "..." || self.is_type_range(a, b) {
                    return true;
                }
                let base_end = self.specifier_end(a, b);
                self.tokens[a].kind == TokenKind::Identifier
                    && !self.variables.contains(self.text(a))
                    && base_end < b
                    && !self.grouped_declarator_problem(base_end, b)
            })
    }

    pub(super) fn remember_inferred_type(&mut self, start: usize, end: usize) {
        let mut i = start;
        while i < end {
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
                continue;
            }
            if matches!(
                self.text(i),
                "typeof" | "__typeof" | "__typeof__" | "decltype"
            ) {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(end, |close| close + 1);
                }
                continue;
            }
            if !self.cpp && matches!(self.text(i), "struct" | "union" | "enum") {
                i += 2;
                continue;
            }
            if self.tokens[i].kind == TokenKind::Identifier
                && !self.type_word(self.text(i))
                && !qualifier(self.text(i))
                && !matches!(
                    self.text(i),
                    "struct" | "class" | "union" | "enum" | "typeof" | "__typeof__" | "decltype"
                )
            {
                self.types.insert(self.text(i).to_string());
                break;
            }
            i += 1;
        }
    }
}
