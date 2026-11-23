//! Recovery that reproduces CDT's problem statements and expressions.

use super::*;

impl Parser<'_> {
    pub(super) fn problem_initializer_end(&self, open: usize) -> Option<usize> {
        let mut depth = 1usize;
        for i in open + 1..self.limit {
            if self.text(i) == "\\"
                || (self.tokens[i].kind == TokenKind::Literal
                    && unclosed_quoted_literal(self.text(i)))
            {
                // AbstractGNUSourceCodeParser.skipProblemDeclaration starts
                // at the failed operand, then balances only subsequent braces.
                return Some(self.problem_declaration_end(i));
            }
            match self.text(i) {
                "{" | "<%" => depth += 1,
                "}" | "%>" => {
                    depth -= 1;
                    if depth == 0 {
                        return None;
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn problem_declaration_end(&self, start: usize) -> usize {
        let mut balance = 0isize;
        for end in start..self.limit {
            match self.text(end) {
                ";" if balance == 0 => return end + 1,
                "{" | "<%" => balance += 1,
                "}" | "%>" => {
                    balance -= 1;
                    if balance <= 0 {
                        let after = end + 1;
                        return after + usize::from(self.text(after) == ";");
                    }
                }
                _ => {}
            }
        }
        self.limit
    }

    pub(super) fn cdt_problem_parameters(&self, header: &Header) -> bool {
        self.split_ranges(header.parameters_start, header.parameters_end, ",")
            .into_iter()
            .any(|(start, end)| {
                if start == end || self.text(start) != "(" {
                    return false;
                }
                if self
                    .matching(start, end)
                    .is_some_and(|close| close + 1 < end)
                    && self.grouped_declarator_problem(start, end)
                {
                    // Formal parameters may contain grouped declarators, but
                    // a cast type-id followed by an address is an expression.
                    return true;
                }
                if self.cpp {
                    return true;
                }
                let specifier = self.text(start + 1);
                self.text(start + 2) == "("
                    && !self.type_word(specifier)
                    && !qualifier(specifier)
                    && !self.explicit_types.contains(specifier)
                    && !matches!(specifier, "typeof" | "__typeof__" | "__typeof" | "_Atomic")
            })
    }

    /// The token at `i`, past the macros CDT's GNU scanner defines as empty.
    pub(super) fn cdt_token(&self, mut i: usize, end: usize) -> (usize, &str) {
        while i < end && matches!(self.text(i), "__extension__" | "__stdcall" | "__thread") {
            i += 1;
        }
        (i, if i < end { self.text(i) } else { "" })
    }

    pub(super) fn cdt_identifier(&self, i: usize, end: usize) -> bool {
        i < end && self.tokens[i].kind == TokenKind::Identifier && !cdt_c_keyword(self.text(i))
    }

    /// CDT `__attribute_decl_seq`. c2cpg's default defines expand the
    /// calling conventions `__cdecl`, `__fastcall` and `__pascal` to attributes.
    pub(super) fn cdt_attributes(&self, i: usize, end: usize) -> usize {
        let (mut i, mut text) = self.cdt_token(i, end);
        while cdt_attribute(text) {
            let arguments = !matches!(text, "__cdecl" | "__fastcall" | "__pascal");
            i += 1;
            if arguments && i < end && self.text(i) == "(" {
                i = self.matching(i, end).map_or(end, |close| close + 1);
            }
            (i, text) = self.cdt_token(i, end);
        }
        i
    }

    /// CDT's C `declSpecifierSeq`: the end of the specifiers and, when an
    /// identifier read as the type name may instead be the declarator, the
    /// token to which the second reading returns.
    pub(super) fn cdt_decl_specifiers(
        &self,
        start: usize,
        end: usize,
        allow_empty: bool,
    ) -> Option<(usize, Option<usize>)> {
        let (mut i, mut raw, mut named, mut any, mut alternative) =
            (start, false, false, false, None);
        loop {
            let (at, text) = self.cdt_token(i, end);
            i = at;
            match text {
                "auto" | "register" | "static" | "extern" | "typedef" | "inline" | "__inline"
                | "__inline__" | "const" | "__const" | "__const__" | "volatile" | "__volatile"
                | "__volatile__" | "restrict" | "__restrict" | "__restrict__" => i += 1,
                "void" | "char" | "short" | "int" | "__int128" | "long" | "float" | "double"
                | "__float128" | "_Decimal32" | "_Decimal64" | "_Decimal128" | "signed"
                | "__signed" | "__signed__" | "unsigned" | "_Bool" => {
                    if named {
                        break;
                    }
                    raw = true;
                    i += 1;
                }
                "_Complex" | "_Imaginary" | "__complex__" => {
                    if named {
                        break;
                    }
                    i += 1;
                }
                "struct" | "union" | "enum" => {
                    if named || raw {
                        break;
                    }
                    let mut tag = self.cdt_attributes(i + 1, end);
                    let name = self.cdt_identifier(tag, end);
                    tag = self.cdt_attributes(tag + usize::from(name), end);
                    if tag < end && self.text(tag) == "{" {
                        tag = self.matching(tag, end)? + 1;
                    } else if !name {
                        return None;
                    }
                    named = true;
                    i = tag;
                }
                "typeof" | "__typeof" | "__typeof__" => {
                    if raw || named {
                        return alternative.map(|at| (at, None));
                    }
                    i += 1;
                    i = if i < end && self.text(i) == "(" {
                        self.matching(i, end)? + 1
                    } else {
                        i + 1
                    };
                    named = true;
                }
                "_Alignas" => {
                    i = self.matching(i + 1, end).map_or(i + 1, |close| close + 1);
                    continue;
                }
                _ if cdt_attribute(text) => {
                    i = self.cdt_attributes(i, end);
                    continue;
                }
                _ if self.cdt_identifier(i, end) => {
                    if named || raw {
                        break;
                    }
                    if any || allow_empty {
                        alternative = Some(i);
                    }
                    named = true;
                    i += 1;
                }
                _ => break,
            }
            any = true;
        }
        (any || allow_empty).then_some((i, alternative))
    }

    /// CDT's C `declarator`: its end and whether it declares a name.
    pub(super) fn cdt_declarator(
        &self,
        start: usize,
        end: usize,
        abstract_: bool,
    ) -> Option<(usize, bool)> {
        let mut i = start;
        loop {
            i = self.cdt_attributes(i, end);
            if i >= end || self.text(i) != "*" {
                break;
            }
            i += 1;
            while let (
                at,
                "const" | "__const" | "__const__" | "volatile" | "__volatile" | "__volatile__"
                | "restrict" | "__restrict" | "__restrict__",
            ) = self.cdt_token(i, end)
            {
                i = at + 1;
            }
        }
        if self.cdt_identifier(i, end) {
            return Some((self.cdt_declarator_suffix(i + 1, end, true)?, true));
        }
        if i < end && self.text(i) == "(" {
            // An abstract function declarator and a nested declarator are
            // both tried; the longer one wins.
            let function = abstract_
                .then(|| self.cdt_declarator_suffix(i, end, false))
                .flatten();
            let nested = (|| {
                let (inner, text) = self.cdt_token(i + 1, end);
                if text == ")" {
                    return None;
                }
                let (close, named) = self.cdt_declarator(inner, end, abstract_)?;
                (close < end && self.text(close) == ")")
                    .then(|| self.cdt_declarator_suffix(close + 1, end, named))
                    .flatten()
                    .map(|after| (after, named))
            })();
            return match (function, nested) {
                (Some(function), Some(nested)) if nested.0 < function => Some((function, false)),
                (Some(function), None) => Some((function, false)),
                (_, nested) => nested,
            };
        }
        if !abstract_ {
            return None;
        }
        Some((self.cdt_declarator_suffix(i, end, false)?, false))
    }

    /// The function or array suffix, attributes and asm label of a declarator.
    pub(super) fn cdt_declarator_suffix(
        &self,
        start: usize,
        end: usize,
        named: bool,
    ) -> Option<usize> {
        let mut i = self.cdt_attributes(start, end);
        match if i < end { self.text(i) } else { "" } {
            "(" => i = self.cdt_parameter_list(i, end, named)?,
            "[" => {
                while i < end && self.text(i) == "[" {
                    i = self.matching(i, end)? + 1;
                }
            }
            // Only members may be bit-fields.
            ":" => return None,
            _ => {}
        }
        i = self.cdt_attributes(i, end);
        if i < end && matches!(self.text(i), "asm" | "__asm" | "__asm__") {
            i += 1;
            if i < end && self.text(i) == "(" {
                i = self.matching(i, end)? + 1;
            }
            i = self.cdt_attributes(i, end);
        }
        Some(i)
    }

    /// CDT's C `functionDeclarator` parameter loop, from the opening
    /// parenthesis to past the closing one. `None` is a backtrack: a
    /// parameter must be followed by a comma, an ellipsis or the close.
    pub(super) fn cdt_parameter_list(&self, open: usize, end: usize, named: bool) -> Option<usize> {
        let (mut i, mut seen) = (open + 1, false);
        loop {
            let (at, text) = self.cdt_token(i, end);
            i = at;
            match text {
                ")" => return Some(i + 1),
                "..." => i += 1,
                "," => {
                    i += 1;
                    seen = false;
                }
                _ if seen || i >= end => return None,
                _ => {
                    // The parameters of a named function may omit specifiers.
                    let (specifiers, alternative) = self.cdt_decl_specifiers(i, end, named)?;
                    i = self
                        .cdt_init_declarator(specifiers, end)
                        .max(alternative.and_then(|at| self.cdt_init_declarator(at, end)))?;
                    seen = true;
                }
            }
        }
    }

    pub(super) fn cdt_init_declarator(&self, start: usize, end: usize) -> Option<usize> {
        let (mut i, _) = self.cdt_declarator(start, end, true)?;
        if i + 1 < end && self.text(i) == "=" && self.text(i + 1) != "{" {
            while i < end && !matches!(self.text(i), "," | ")") {
                i = if matches!(self.text(i), "(" | "[" | "{") {
                    self.matching(i, end)? + 1
                } else {
                    i + 1
                };
            }
        }
        Some(i)
    }

    /// The end of a C local declaration `specifiers name` that an
    /// identifier follows, where CDT's `simpleDeclaration` stops.
    pub(super) fn cdt_unterminated_declarator(&self, start: usize) -> Option<usize> {
        if self.cpp {
            return None;
        }
        let end = self.limit;
        let (specifiers, alternative) = self.cdt_decl_specifiers(start, end, false)?;
        let declarator = |from: usize| {
            let mut name = from;
            while name < end && self.text(name) == "*" {
                name += 1;
            }
            self.cdt_identifier(name, end).then_some(name + 1)
        };
        let after = declarator(specifiers).max(alternative.and_then(declarator))?;
        self.cdt_identifier(after, end).then_some(after)
    }

    /// A statement without its semicolon, the problem CDT reports for it,
    /// and the statement that starts at the current token.
    pub(super) fn missing_semicolon(
        &mut self,
        start: usize,
        kind: StmtKind,
        span: Span,
        enclosing_fails: bool,
    ) -> Stmt {
        let rest = self.recovered_statement();
        self.unterminated(start, kind, span, rest, enclosing_fails)
    }

    /// A statement without its semicolon and the one parsed after it. CDT's
    /// problem there also fails an enclosing label or control statement, so
    /// that the two become its siblings; `enclosing_fails` marks that.
    pub(super) fn unterminated(
        &self,
        start: usize,
        kind: StmtKind,
        span: Span,
        rest: Stmt,
        enclosing_fails: bool,
    ) -> Stmt {
        let mut statements = Vec::with_capacity(3);
        if enclosing_fails {
            statements.push(Stmt {
                kind: StmtKind::Problem,
                span: span.clone(),
            });
        }
        statements.extend([Stmt { kind, span }, rest]);
        Stmt {
            span: self.span(start, self.pos),
            kind: StmtKind::Sequence(statements),
        }
    }

    /// The statement after a missing semicolon. One that reaches a brace
    /// before its own semicolon is no whole statement for the type-directed
    /// path; CDT's two attempts decide it instead. An operand chain that an
    /// identifier follows is an expression statement, again without its
    /// semicolon. A call whose group holds adjacent names and is no
    /// declarator fails both attempts, and the problem ends with the block.
    pub(super) fn recovered_statement(&mut self) -> Stmt {
        let start = self.pos;
        let mut end = start;
        while end < self.limit && !matches!(self.text(end), ";" | "{" | "}" | "<%" | "%>") {
            end = if matches!(self.text(end), "(" | "[") {
                self.matching(end, self.limit)
                    .map_or(end + 1, |close| close + 1)
            } else {
                end + 1
            };
        }
        if self.cpp
            || end >= self.limit
            || !matches!(self.text(end), "{" | "<%")
            || !self.cdt_identifier(start, end)
            || self.cdt_unterminated_declarator(start).is_some()
        {
            return self.statement();
        }
        let operand = |i: usize| {
            self.cdt_identifier(i, end) || (i < end && self.tokens[i].kind == TokenKind::Literal)
        };
        let mut stop = start + 1;
        while matches!(
            self.text(stop),
            "." | "->"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "&"
                | "|"
                | "^"
                | "<<"
                | ">>"
                | "<"
                | ">"
                | "<="
                | ">="
                | "=="
                | "!="
                | "&&"
                | "||"
                | "="
        ) && operand(stop + 1)
        {
            stop += 2;
        }
        if stop > start + 1 && self.cdt_identifier(stop, end) {
            let diagnostics = self.diagnostics.len();
            let expression = self.expression_range(start, stop);
            if self.diagnostics.len() == diagnostics
                && !matches!(expression.kind, ExprKind::Problem(_))
            {
                self.pos = stop;
                let span = expression.span.clone();
                return self.missing_semicolon(
                    start,
                    StmtKind::Expression(expression),
                    span,
                    self.cdt_failures,
                );
            }
            self.diagnostics.truncate(diagnostics);
        } else if stop == start + 1
            && self.text(stop) == "("
            && let Some(close) = self.matching(stop, end)
            && close + 1 == end
            && (stop + 1..close.saturating_sub(1)).any(|i| {
                self.tokens[i].kind == TokenKind::Identifier
                    && self.tokens[i + 1].kind == TokenKind::Identifier
            })
            && self.cdt_declarator(stop, end, false).is_none()
        {
            self.pos = self.problem_declaration_end(start);
            return Stmt {
                kind: StmtKind::Problem,
                span: self.span(start, self.pos),
            };
        }
        self.pos = start;
        self.statement()
    }

    /// CDT's C expression ends at a literal that follows a complete operand,
    /// as inside `"a "%s" b"`, and at a name that follows a literal. In an
    /// expression statement or a local initializer the semicolon is then
    /// missing and the rest is parsed as the next statement.
    pub(super) fn cdt_literal_recovery(&mut self, start: usize, end: usize) -> Option<Stmt> {
        if self.cpp || self.pos != start || !self.cdt_adjacent_operands(start, end) {
            return None;
        }
        let diagnostics = self.diagnostics.len();
        let (kind, span) = if self.looks_declaration(start, end) {
            let (specifiers, _) = self.cdt_decl_specifiers(start, end, false)?;
            let (at, _) = self
                .split_ranges(specifiers, end, ",")
                .into_iter()
                .find_map(|(a, b)| {
                    let (value, _) = self.cdt_declarator(a, b, false)?;
                    (value + 1 < b && self.text(value) == "=" && self.text(value + 1) != "{")
                        .then(|| self.cdt_expression_stop(value + 1, b))
                        .flatten()
                })?;
            self.remember_types(start, at);
            let declarations = self.declarations_range(start, at);
            if self.diagnostics.len() > diagnostics {
                self.diagnostics.truncate(diagnostics);
                self.pos = start;
                return None;
            }
            self.pos = at;
            (StmtKind::Declaration(declarations), self.span(start, at))
        } else {
            let (at, expression) = self.cdt_expression_stop(start, end)?;
            let declarator = |from: usize| Some(self.cdt_declarator(from, end, false)?.0);
            if self
                .cdt_decl_specifiers(start, end, false)
                .and_then(|(specifiers, alternative)| {
                    declarator(specifiers).max(alternative.and_then(declarator))
                })
                .is_some_and(|after| after >= at)
            {
                // A declaration that reaches as far is CDT's reading.
                return None;
            }
            self.pos = at;
            let span = expression.span.clone();
            (StmtKind::Expression(expression), span)
        };
        // CDT's problem also fails an enclosing label or control statement.
        // By default that stays.
        Some(self.missing_semicolon(start, kind, span, self.cdt_failures))
    }

    /// Whether a literal follows a complete operand, or a name a literal,
    /// outside the groups of `start..end`.
    pub(super) fn cdt_adjacent_operands(&self, start: usize, end: usize) -> bool {
        let literal = |i: usize| self.tokens[i].kind == TokenKind::Literal;
        let string = |i: usize| string_literal_token(self.text(i));
        let mut i = start + 1;
        while i < end {
            if literal(i) && unclosed_quoted_literal(self.text(i)) {
                return false;
            }
            let after_operand = (literal(i - 1) && !(string(i - 1) && string(i)))
                || self.cdt_identifier(i - 1, end)
                || matches!(self.text(i - 1), ")" | "]");
            if (literal(i) && after_operand) || (self.cdt_identifier(i, end) && literal(i - 1)) {
                return true;
            }
            i = match self.text(i) {
                "(" | "[" | "{" => self.matching(i, end).map_or(end, |close| close + 1),
                _ => i + 1,
            };
        }
        false
    }

    /// The expression from `from` and its end, where that end comes before
    /// `to` at a literal or at a name after a literal.
    pub(super) fn cdt_expression_stop(&mut self, from: usize, to: usize) -> Option<(usize, Expr)> {
        let diagnostics = self.diagnostics.len();
        let (pos, limit) = (self.pos, std::mem::replace(&mut self.limit, to));
        self.pos = from;
        let expression = self.expression(0);
        let at = std::mem::replace(&mut self.pos, pos);
        self.limit = limit;
        let literal = |i: usize| self.tokens[i].kind == TokenKind::Literal;
        let ended = self.diagnostics.len() == diagnostics
            && at > from
            && at < to
            && !matches!(expression.kind, ExprKind::Problem(_))
            && (literal(at) || (self.cdt_identifier(at, to) && literal(at - 1)));
        self.diagnostics.truncate(diagnostics);
        ended.then_some((at, expression))
    }

    /// CDT parses a local declaration without type bindings. One whose
    /// declarator fails is an expression statement, or a problem if no
    /// expression parses there; one that ends early is followed by another
    /// statement. `None` leaves the statement to the type-directed path.
    pub(super) fn cdt_declaration_recovery(&mut self, start: usize, end: usize) -> Option<Stmt> {
        if !self.cdt_failures && self.function_header(start, end).is_some() {
            // By default a prototype keeps its method even where CDT
            // backtracks on the parameter list.
            return None;
        }
        let (specifiers, alternative) = self.cdt_decl_specifiers(start, end, false)?;
        let declarator = |from: usize| Some(self.cdt_declarator(from, end, false)?.0);
        let named = declarator(specifiers);
        let renamed = alternative.and_then(declarator);
        let diagnostics = self.diagnostics.len();
        // CDT's missing semicolon fails an enclosing label or control
        // statement. By default that stays as the type-directed path left
        // it: failed only where the whole statement was a problem.
        let enclosing_fails = self.cdt_failures
            || self.declaration_shape_problem(start, end)
            || self.declaration_expression_problem(start, end);
        match named.max(renamed) {
            None if specifiers < end => {
                let mut kind = StmtKind::Problem;
                if self.cdt_identifier(start, end) {
                    let limit = std::mem::replace(&mut self.limit, end);
                    let expression = self.expression(0);
                    self.limit = limit;
                    if self.diagnostics.len() == diagnostics && self.pos > start && self.pos < end {
                        // An expression that ends early lacks its semicolon.
                        let span = expression.span.clone();
                        return Some(self.missing_semicolon(
                            start,
                            StmtKind::Expression(expression),
                            span,
                            enclosing_fails,
                        ));
                    }
                    self.pos = start;
                    let expression = if self.diagnostics.len() == diagnostics {
                        self.expression_range(start, end)
                    } else {
                        expression
                    };
                    if self.diagnostics.len() > diagnostics {
                        self.diagnostics.truncate(diagnostics);
                        return None;
                    }
                    if !matches!(expression.kind, ExprKind::Problem(_)) {
                        kind = StmtKind::Expression(expression);
                    }
                }
                if matches!(kind, StmtKind::Problem) && !self.cdt_failures {
                    // By default no statement is dropped for CDT's failure.
                    return None;
                }
                self.pos = end;
                self.finish_statement(start);
                Some(Stmt {
                    kind,
                    span: self.span(start, self.pos),
                })
            }
            Some(after) if after < end && !matches!(self.text(after), "=" | "," | ":" | "{") => {
                let declarations = match alternative {
                    // Only the reading that declares the type name itself
                    // reached this far.
                    Some(name) if renamed > named && after == name + 1 => {
                        let name = self.text(name).to_string();
                        Rc::make_mut(&mut self.variables).insert(name.clone());
                        vec![Declaration {
                            type_name: joern_clean_type(
                                &(start..after - 1)
                                    .map(|i| self.text(i))
                                    .collect::<Vec<_>>()
                                    .join(" "),
                            ),
                            name,
                            initializer: None,
                            dimensions: Vec::new(),
                            problem: false,
                            is_typedef: false,
                            nested_declarator: false,
                            span: self.span(start, after),
                        }]
                    }
                    _ => self.declarations_range(start, after),
                };
                if self.diagnostics.len() > diagnostics {
                    self.diagnostics.truncate(diagnostics);
                    self.pos = start;
                    return None;
                }
                let span = self.span(start, after);
                self.pos = after;
                // The type-directed path already failed the enclosing
                // statement of a declarator that no group follows.
                Some(self.missing_semicolon(
                    start,
                    StmtKind::Declaration(declarations),
                    span,
                    enclosing_fails || self.text(after) != "(",
                ))
            }
            _ => None,
        }
    }

    /// Whether CDT's C parser backtracks out of this header's parameter
    /// lists, which makes the declaration and any body one problem node.
    pub(super) fn cdt_parameter_backtrack(&self, header: &Header) -> bool {
        if self.cpp {
            return false;
        }
        let (open, close) = (header.parameters_start - 1, header.parameters_end);
        // An identifier list followed by declarations is a K&R definition.
        let knr = close > open + 1
            && (open + 1..close).all(|i| {
                if (i - open) % 2 == 1 {
                    self.cdt_identifier(i, close)
                } else {
                    self.text(i) == ","
                }
            })
            && (close - open) % 2 == 0
            && !matches!(self.text(close + 1), ";" | "{");
        let outer = header.outer_parameters.is_some_and(|(start, close)| {
            self.cdt_parameter_list(start - 1, close + 1, true) != Some(close + 1)
        });
        outer || (!knr && self.cdt_parameter_list(open, close + 1, true) != Some(close + 1))
    }

    /// `name (declarator) suffix` on one line: CDT keeps the reading with
    /// the type `name` and a nested declarator. Only a function declarator
    /// may precede a missing semicolon there, so the declaration backtracks.
    pub(super) fn cdt_nested_declarator_problem(
        &self,
        start: usize,
        end: usize,
        header: &Header,
    ) -> bool {
        let (open, close) = (header.parameters_start - 1, header.parameters_end);
        !self.cpp
            && header.name_start + 1 == open
            && self.cdt_decl_specifiers(start, end, true) == Some((open, Some(header.name_start)))
            && self.cdt_declarator(open, close + 1, false) == Some((close + 1, true))
            && self
                .unknown_function_suffix(close + 1, end)
                .is_some_and(|marker| {
                    self.tokens[marker].span.line == self.tokens[close].span.end_line
                })
    }

    pub(super) fn cdt_problem_return_prefix(&self, start: usize, header: &Header) -> bool {
        // Shared declaration specifiers precede all comma-separated declarators.
        // A later declarator must not reparse an earlier function's parameter list
        // as part of its own return type.
        let start = if header.return_type.is_some() {
            self.split_ranges(start, header.name_start, ",")
                .last()
                .map_or(start, |&(from, _)| from)
        } else {
            start
        };
        let (start, _) = self.return_specifier_start(start, header.name_start);
        let mut cursor = self.specifier_end(start, header.name_start);
        while cursor < header.name_start {
            if let Some(after) = self.skip_attribute(cursor, header.name_start) {
                cursor = after;
            } else if matches!(self.text(cursor), "*" | "&" | "&&" | "(" | ")")
                || qualifier(self.text(cursor))
            {
                cursor += 1;
            } else if self.cpp
                && self.tokens[cursor].kind == TokenKind::Identifier
                && self.text(cursor + 1) == "::"
            {
                // A C++ member-pointer operator owns a qualified scope.
                cursor += 2;
            } else {
                return true;
            }
        }
        false
    }

    pub(super) fn unknown_function_suffix(&self, start: usize, end: usize) -> Option<usize> {
        if (start..end).any(|i| self.text(i) == ";") {
            return None;
        }
        let mut i = start;
        while i < end {
            if self.text(i) == "," {
                return None;
            }
            if let Some(after) = self.skip_attribute(i, end) {
                i = after;
            } else if matches!(
                self.text(i),
                "__cdecl" | "__fastcall" | "__pascal" | "__stdcall" | "__thread"
            ) {
                // Joern's default defines and the GNU scanner expand these
                // calling convention/storage macros to attributes or nothing.
                i += 1;
            } else if matches!(self.text(i), "__asm__" | "__asm" | "asm") {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(end, |close| close + 1);
                }
            } else if self.cpp
                && matches!(
                    self.text(i),
                    "const" | "volatile" | "override" | "final" | "throw" | "noexcept"
                )
            {
                i += 1;
                if self.text(i) == "(" {
                    i = self.matching(i, end).map_or(end, |close| close + 1);
                }
            } else if matches!(self.text(i), "->" | ":" | "requires") {
                return None;
            } else if matches!(self.text(i), "(" | "[") {
                i = self.matching(i, end).map_or(end, |close| close + 1);
            } else if matches!(self.text(i), ")" | "*" | "&" | "&&") {
                i += 1;
            } else {
                return Some(i);
            }
        }
        None
    }

    pub(super) fn cdt_problem_declaration(&self, start: usize, end: usize) -> bool {
        if (start..end).any(|i| self.tokens[i].macro_recovery) {
            return true;
        }
        if !self.cpp {
            let mut cursor = start;
            while cursor < end {
                if let Some(after) = self.skip_attribute(cursor, end) {
                    cursor = after;
                } else if self.text(cursor) == "[" {
                    cursor = self.matching(cursor, end).map_or(end, |close| close + 1);
                } else if matches!(self.text(cursor), "&" | "&&") {
                    // C reference declarators are CDT problem declarations;
                    // expressions within valid array bounds remain supported.
                    return true;
                } else {
                    cursor += 1;
                }
            }
        }
        let mut specifier_start = start;
        while let Some(after) = self.skip_attribute(specifier_start, end) {
            specifier_start = after;
        }
        (start..end).any(|i| {
            (self.text(i) == "_Noreturn"
                && if self.cpp {
                    i != start
                } else {
                    !self.in_function_body
                        && self.tokens[specifier_start].span.line == self.tokens[i + 1].span.line
                })
                || self.text(i) == "@"
                || (!self.cpp && self.text(i) == "::")
                || (!self.cpp && self.text(i) == "[" && self.text(i + 1) == "[")
                || (matches!(self.text(i), "__attribute__" | "__attribute")
                    && self.text(i + 1) == "("
                    && self.text(i + 2) == "("
                    && self.text(i + 3) == ")"
                    && self.text(i + 4) == ")")
        })
    }

    pub(super) fn cdt_problem_record_header(&self, start: usize, end: usize) -> bool {
        let mut cursor = start;
        while cursor < end {
            if let Some(after) = self.skip_attribute(cursor, end) {
                cursor = after;
                continue;
            }
            if matches!(self.text(cursor), "struct" | "union" | "class" | "enum") {
                let mut tag = cursor + 1;
                while let Some(after) = self.skip_attribute(tag, end) {
                    tag = after;
                }
                if self
                    .tokens
                    .get(tag)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                {
                    let name = tag;
                    tag += 1;
                    while let Some(after) = self.skip_attribute(tag, end) {
                        tag = after;
                    }
                    if self.text(tag) == "(" {
                        // A parenthesized function declarator can follow a
                        // record return type. Its name belongs inside that
                        // declarator, rather than to the preceding record tag.
                        return self
                            .function_header(start, end)
                            .is_none_or(|header| header.name_start <= name);
                    }
                }
            }
            cursor = if matches!(self.text(cursor), "(" | "[") {
                self.matching(cursor, end).map_or(end, |close| close + 1)
            } else {
                cursor + 1
            };
        }
        false
    }

    pub(super) fn cdt_problem_function_header(&self, start: usize, end: usize) -> bool {
        if self.cdt_problem_record_header(start, end) {
            return true;
        }
        if (start..end).any(|i| self.tokens[i].macro_recovery) {
            return true;
        }
        if self.specifier_end(start, end) == end || (start..end).any(|i| self.text(i) == ":") {
            // A type-only brace header has no declarator. A global label or
            // colon-bearing problem preamble likewise creates no function.
            return true;
        }
        if end >= start + 2
            && !self.type_word(self.text(start))
            && !qualifier(self.text(start))
            && (start..end).all(|i| self.tokens[i].kind == TokenKind::Identifier)
        {
            // An unresolved name-only brace header has no function
            // declarator. CDT recovers its body as one problem declaration.
            return true;
        }
        let mut cursor = start;
        while cursor < end {
            if let Some(after) = self.skip_attribute(cursor, end) {
                cursor = after;
                continue;
            }
            if self.text(cursor) == "@"
                && (!self.cpp
                    || (start..end)
                        .find(|i| self.text(*i) == "(")
                        .is_some_and(|open| (open..end).any(|i| self.text(i) == "@")))
            {
                return true;
            }
            if self.tokens[cursor].kind == TokenKind::Identifier && self.text(cursor + 1) == "[" {
                let mut suffix = cursor + 1;
                while self.text(suffix) == "[" {
                    let Some(close) = self.matching(suffix, end) else {
                        break;
                    };
                    suffix = close + 1;
                }
                // C and C++ cannot define an array of functions. The old
                // CDT parser recovers its entire brace body as a problem.
                if self.text(suffix) == "(" {
                    return true;
                }
            }
            if self.text(cursor) == "("
                && let Some(close) = self.matching(cursor, end)
            {
                cursor = close + 1;
            } else {
                cursor += 1;
            }
        }
        false
    }

    /// A C file-scope `simpleDeclaration` that reaches its semicolon: that
    /// semicolon and the names of the objects it declares. The specifiers
    /// may be empty, and a leading identifier may be the declarator.
    pub(super) fn cdt_object_declaration(
        &self,
        start: usize,
        end: usize,
    ) -> Option<(usize, Vec<usize>)> {
        let (specifiers, alternative) = self.cdt_decl_specifiers(start, end, true)?;
        let typedef = (start..specifiers).any(|i| self.text(i) == "typedef");
        let declarators = |mut i: usize| {
            let mut names = Vec::new();
            loop {
                let (after, _) = self.cdt_declarator(i, end, false)?;
                if !typedef
                    && self.function_header(i, after).is_none()
                    && let (Some(name), _) = self.declarator_name(i, after)
                {
                    names.push(name);
                }
                i = after;
                if i < end && self.text(i) == "=" {
                    // Only a whole initializer may be a brace list.
                    let value = i + 1;
                    i = value;
                    while i < end && !matches!(self.text(i), "," | ";") {
                        i = match self.text(i) {
                            "{" if i > value => return None,
                            "(" | "[" | "{" => self.matching(i, end)? + 1,
                            _ => i + 1,
                        };
                    }
                    if i == value {
                        return None;
                    }
                }
                match if i < end { self.text(i) } else { "" } {
                    "," => i += 1,
                    ";" => return Some((i, names)),
                    _ => return None,
                }
            }
        };
        declarators(specifiers).or_else(|| alternative.and_then(declarators))
    }

    pub(super) fn local_problem_enum_statement(&mut self, start: usize) -> Option<Stmt> {
        if !self.in_function_body {
            return None;
        }
        let mut head = start;
        loop {
            if let Some(after) = self.skip_attribute(head, self.limit) {
                head = after;
            } else if qualifier(self.text(head)) {
                head += 1;
            } else {
                break;
            }
        }
        if self.text(head) != "enum" {
            return None;
        }
        let mut open = head + 1;
        while open < self.limit && !matches!(self.text(open), "{" | ";" | "}" | "=") {
            if let Some(after) = self.skip_attribute(open, self.limit) {
                open = after;
            } else {
                open += 1;
            }
        }
        if self.text(open) != "{" {
            return None;
        }
        let close = self.matching(open, self.limit)?;
        let invalid = self
            .split_ranges(open + 1, close, ",")
            .iter()
            .any(|&(a, b)| a < b && self.tokens[a].kind != TokenKind::Identifier);
        if !invalid {
            return None;
        }
        let mut after = close + 1;
        if self.text(after) == ";" {
            after += 1;
        }
        // In a compound statement CDT's enum failure has already advanced
        // past its terminator when skipProblemStatement resumes. It therefore
        // consumes the next statement too; translation-unit recovery differs.
        self.pos = if after == self.limit || matches!(self.text(after), "}" | "%>") {
            after
        } else {
            self.problem_declaration_end(after)
        };
        Some(Stmt {
            kind: StmtKind::Problem,
            span: self.span(start, self.pos),
        })
    }

    pub(super) fn grouped_declarator_problem(&self, start: usize, end: usize) -> bool {
        let mut core = start;
        while core < end {
            if let Some(after) = self.skip_attribute(core, end) {
                core = after;
            } else if matches!(self.text(core), "*" | "&" | "&&") || qualifier(self.text(core)) {
                core += 1;
            } else if self.cpp && self.text(core + 1) == "::" {
                let mut pointer = core;
                while pointer + 2 < end && self.text(pointer + 1) == "::" {
                    pointer += 2;
                }
                if self.text(pointer) == "*" {
                    core = pointer + 1;
                } else {
                    break;
                }
            } else {
                break;
            }
        }
        if self.text(core) == "("
            && let Some(close) = self.matching(core, end)
            && self.grouped_declarator_problem(core + 1, close)
        {
            return true;
        }
        let (name, mut suffix) = self.declarator_name(core, end);
        if name.is_none() {
            return true;
        }
        while suffix < end {
            if let Some(after) = self.skip_attribute(suffix, end) {
                suffix = after;
            } else if matches!(self.text(suffix), "(" | "[") {
                let Some(close) = self.matching(suffix, end) else {
                    return true;
                };
                suffix = close + 1;
            } else if self.cpp && self.text(suffix) == "<" {
                let Some(close) = self.angle_close(suffix, end) else {
                    return true;
                };
                suffix = close + 1;
            } else if self.cpp && qualifier(self.text(suffix)) {
                suffix += 1;
            } else {
                // A name may have array/function suffixes, but a trailing
                // pointer or address operand belongs to a cast expression.
                return true;
            }
        }
        false
    }

    pub(super) fn declaration_shape_problem(&self, start: usize, end: usize) -> bool {
        let base_end = self.specifier_end(start, end);
        for i in start..base_end {
            if matches!(self.text(i), "struct" | "union" | "class" | "enum") {
                let mut tag = i + 1;
                while let Some(after) = self.skip_attribute(tag, end) {
                    tag = after;
                }
                if self.text(tag) == "<" {
                    // A record type-id needs a tag before template arguments.
                    // Decompiled angle-delimited anonymous names are problems.
                    return true;
                }
            }
        }
        if self.text(base_end) == "["
            && !(self.cpp && (start..base_end).any(|i| self.text(i) == "auto"))
        {
            return true;
        }
        if !self.cpp
            && self.text(base_end) == "("
            && self.matching(base_end, end) == Some(base_end + 1)
        {
            return true;
        }
        for (a, b) in self.split_ranges(base_end, end, ",") {
            let mut grouped = a;
            while grouped < b {
                if let Some(after) = self.skip_attribute(grouped, b) {
                    grouped = after;
                } else if matches!(self.text(grouped), "*" | "&" | "&&")
                    || qualifier(self.text(grouped))
                {
                    grouped += 1;
                } else {
                    break;
                }
            }
            if self.text(grouped) == "("
                && let Some(close) = self.matching(grouped, b)
                && self.grouped_declarator_problem(grouped + 1, close)
            {
                return true;
            }
        }
        let mut core = base_end;
        while core < end {
            if let Some(after) = self.skip_attribute(core, end) {
                core = after;
            } else if matches!(self.text(core), "*" | "&" | "&&" | "(")
                || qualifier(self.text(core))
            {
                core += 1;
            } else {
                break;
            }
        }
        // A literal cannot name a declarator, including the fabricated
        // address form void (*0x1234)(...). Casted pointer calls start with
        // an outer expression group and never enter this declaration path.
        self.tokens
            .get(core)
            .is_some_and(|token| token.kind == TokenKind::Literal)
    }

    pub(super) fn unclosed_literal_expression_end(
        &self,
        start: usize,
        end: usize,
        require_assignment: bool,
    ) -> Option<usize> {
        let mut cursor = start;
        let mut assignment = !require_assignment;
        while cursor < end {
            if matches!(self.text(cursor), "(" | "[" | "{") {
                cursor = self.matching(cursor, end)? + 1;
                continue;
            }
            assignment |= matches!(
                self.text(cursor),
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
            );
            if assignment
                && self.tokens[cursor].kind == TokenKind::Literal
                && unclosed_quoted_literal(self.text(cursor))
                && !(require_assignment && matches!(self.text(cursor + 1), "++" | "--"))
            {
                // The scanner's newline boundary terminates the recovered
                // assignment, without discarding the following statement.
                // Postfix operators still attach to the literal normally.
                return Some(cursor + 1);
            }
            cursor += 1;
        }
        None
    }

    pub(super) fn unclosed_literal_call(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        (start + 1..end).find_map(|open| {
            (self.text(open) == "("
                && self.tokens[open - 1].kind == TokenKind::Identifier
                && !self.type_word(self.text(open - 1))
                && !qualifier(self.text(open - 1))
                && self.matching(open, end).is_none())
            .then(|| {
                (open + 1..end)
                    .find(|&i| {
                        self.tokens[i].kind == TokenKind::Literal
                            && unclosed_quoted_literal(self.text(i))
                    })
                    .map(|literal| (open, literal))
            })
            .flatten()
        })
    }

    pub(super) fn unclosed_conditional_literal(&self, start: usize, end: usize) -> bool {
        let mut pending = 0usize;
        let mut cursor = start;
        while cursor < end {
            match self.text(cursor) {
                "(" | "[" | "{" => {
                    if let Some(close) = self.matching(cursor, end) {
                        cursor = close + 1;
                        continue;
                    }
                }
                "?" => pending += 1,
                ":" => pending = pending.saturating_sub(1),
                _ => {}
            }
            if pending > 0
                && self.tokens[cursor].kind == TokenKind::Literal
                && unclosed_quoted_literal(self.text(cursor))
            {
                return true;
            }
            cursor += 1;
        }
        false
    }

    pub(super) fn unclosed_condition_boundary(&self, open: usize) -> Option<usize> {
        if self.text(open) == "(" && self.matching(open, self.limit).is_none() {
            let mut unclosed_literal = false;
            let mut cursor = open + 1;
            while cursor < self.limit && !matches!(self.text(cursor), "}" | "%>") {
                if self.tokens[cursor].kind == TokenKind::Literal {
                    unclosed_literal |= unclosed_quoted_literal(self.text(cursor));
                }
                cursor += 1;
            }
            if unclosed_literal && cursor < self.limit {
                return Some(cursor);
            }
        }
        None
    }

    /// CDT's `unaryExpression` backtracks when `*` is applied directly to an
    /// integer, floating or character literal, which fails the expression.
    pub(super) fn star_literal_problem(&self, start: usize, end: usize) -> bool {
        // By default such an expression keeps its nodes.
        if !self.cdt_failures {
            return false;
        }
        (start..end.min(self.tokens.len()).saturating_sub(1)).any(|i| {
            self.text(i) == "*"
                && self.tokens[i + 1].kind == TokenKind::Literal
                && !string_literal_token(self.text(i + 1))
                // A postfix operator makes the operand a larger expression.
                && !matches!(self.text(i + 2), "(" | "[" | "++" | "--" | "." | "->")
                // After an operand the star multiplies.
                && (i == start
                    || match self.tokens[i - 1].kind {
                        TokenKind::Literal => false,
                        TokenKind::Identifier => matches!(
                            self.text(i - 1),
                            "return" | "case" | "sizeof" | "else" | "do" | "throw"
                        ),
                        TokenKind::Punctuation => {
                            !matches!(self.text(i - 1), ")" | "]" | "++" | "--")
                        }
                    })
        })
    }

    pub(super) fn detached_identifier_suffix(&self, start: usize, end: usize) -> Option<usize> {
        let mut cursor = start;
        while cursor < end {
            if cursor > start
                && self.tokens[cursor].kind == TokenKind::Identifier
                && (matches!(self.text(cursor - 1), ")" | "]")
                    || (cursor > start + 1
                        && self.tokens[cursor - 1].kind == TokenKind::Identifier
                        && (start..cursor - 1).all(|i| matches!(self.text(i), "*" | "&"))))
                && matches!(
                    self.text(cursor + 1),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
                )
                && !self.cast_group_ends_at(start, cursor - 1, end)
                && (!self.cpp || alternative_operator(self.text(cursor)) == self.text(cursor))
            {
                // A completed operand followed by a new assignment recovers
                // as two statements, including fabricated signedness suffixes.
                return Some(cursor);
            }
            cursor = if matches!(self.text(cursor), "(" | "[" | "{") {
                self.matching(cursor, end).map_or(cursor + 1, |i| i + 1)
            } else {
                cursor + 1
            };
        }
        if self.text(end.saturating_sub(1)) == ")" {
            let mut cursor = start;
            while cursor < end {
                if cursor > start
                    && self.tokens[cursor].kind == TokenKind::Identifier
                    && matches!(self.text(cursor - 1), ")" | "]")
                    && self.text(cursor + 1) == "("
                    && self.matching(cursor + 1, end) == Some(end - 1)
                    && !self.cast_group_ends_at(start, cursor - 1, end)
                    && (!self.cpp || alternative_operator(self.text(cursor)) == self.text(cursor))
                    && !expression_type_keyword(self.text(cursor), self.cpp)
                {
                    // A completed top-level operand and a following ordinary
                    // call recover as two statements, not an infix operator.
                    return Some(cursor);
                }
                cursor = if matches!(self.text(cursor), "(" | "[" | "{") {
                    self.matching(cursor, end).map_or(cursor + 1, |i| i + 1)
                } else {
                    cursor + 1
                };
            }
        }
        if end >= start + 4
            && matches!(self.text(end - 2), "++" | "--")
            && self.text(end - 3) == ")"
            && (start..end - 3).rev().any(|open| {
                let preceding_cast = open > start
                    && self.text(open - 1) == ")"
                    && (start..open - 1).rev().any(|previous| {
                        self.text(previous) == "("
                            && self.matching(previous, end) == Some(open - 1)
                            && self.is_cast(previous + 1, open - 1)
                    });
                self.text(open) == "("
                    && self.matching(open, end) == Some(end - 3)
                    && (open == start
                        || (self.tokens[open - 1].kind != TokenKind::Identifier
                            && (preceding_cast || !matches!(self.text(open - 1), ")" | "]"))))
                    && self.is_cast(open + 1, end - 3)
            })
        {
            // '(T)++value' is a cast of a prefix increment, not a
            // completed postfix expression followed by a stray identifier.
            return None;
        }
        if end >= start + 3
            && self.tokens[end - 1].kind == TokenKind::Identifier
            && self.tokens[end - 2].kind == TokenKind::Identifier
            && (!self.cpp
                || (alternative_operator(self.text(end - 1)) == self.text(end - 1)
                    && alternative_operator(self.text(end - 2)) == self.text(end - 2)))
            && (matches!(
                self.tokens[end - 3].kind,
                TokenKind::Identifier | TokenKind::Literal
            ) || matches!(self.text(end - 3), ")" | "]" | "++" | "--"))
            && !self.cast_group_ends_at(start, end - 3, end)
            && self.looks_declaration(end - 2, end)
        {
            // A complete operand followed by two names recovers a separate
            // local declaration, rather than an invented infix operator.
            return Some(end - 2);
        }
        (end >= start + 2
            && self.tokens[end - 1].kind == TokenKind::Identifier
            && ((self.tokens[end - 2].kind == TokenKind::Identifier
                && !matches!(
                    self.text(end - 2),
                    "sizeof"
                        | "alignof"
                        | "_Alignof"
                        | "__alignof__"
                        | "__alignof"
                        | "typeof"
                        | "__typeof__"
                        | "__typeof"
                        | "__extension__"
                        | "__real__"
                        | "__imag__"
                        | "noexcept"
                        | "throw"
                        | "delete"
                        | "new"
                )
                && (!self.cpp || alternative_operator(self.text(end - 2)) == self.text(end - 2)))
                || (self.tokens[end - 2].kind == TokenKind::Literal
                    && self.text(end - 2).starts_with(|c: char| c.is_ascii_digit()))
                || (matches!(self.text(end - 2), "++" | "--")
                    && end > start + 2
                    && (matches!(
                        self.tokens[end - 3].kind,
                        TokenKind::Identifier | TokenKind::Literal
                    ) || matches!(self.text(end - 3), ")" | "]"))))
            && (!self.cpp || alternative_operator(self.text(end - 1)) == self.text(end - 1)))
        .then_some(end - 1)
    }

    pub(super) fn recovered_suffix_statement(&mut self, start: usize, end: usize) -> Stmt {
        let kind = if self.looks_declaration(start, end) {
            self.remember_types(start, end);
            StmtKind::Declaration(self.declarations_range(start, end))
        } else {
            StmtKind::Expression(self.expression_range(start, end))
        };
        Stmt {
            kind,
            span: self.span(start, end),
        }
    }

    pub(super) fn stray_postfix_operator(&self, start: usize, end: usize) -> Option<usize> {
        (end > start + 1
            && matches!(self.text(end - 1), "!" | "~")
            && (matches!(
                self.tokens[end - 2].kind,
                TokenKind::Identifier | TokenKind::Literal
            ) || matches!(self.text(end - 2), ")" | "]"))
            && !self.cast_group_ends_at(start, end - 2, end))
        .then_some(end - 1)
    }

    pub(super) fn macro_problem_origin(&self, start: usize, end: usize) -> bool {
        self.tokens[start..(end + 1).min(self.tokens.len())]
            .iter()
            .any(|token| token.macro_recovery || token.macro_expansion)
    }

    pub(super) fn problem_statement(&self, start: usize, from_macro: bool) -> Stmt {
        let span = self.span(start, self.pos);
        Stmt {
            kind: if from_macro {
                StmtKind::Expression(Expr {
                    kind: ExprKind::Problem(self.raw(start, self.pos)),
                    span: span.clone(),
                })
            } else {
                StmtKind::Problem
            },
            span,
        }
    }

    pub(super) fn unpaired_expression_colon(&self, start: usize, end: usize) -> Option<usize> {
        if !(start..end).any(|i| self.text(i) == ":") {
            return None;
        }
        let mut conditionals = 0usize;
        let mut i = start;
        while i < end {
            match self.text(i) {
                "(" | "[" | "{" => {
                    if let Some(close) = self.matching(i, end) {
                        i = close + 1;
                        continue;
                    }
                }
                "?" => conditionals += 1,
                ":" if conditionals == 0 => return Some(i),
                ":" => conditionals -= 1,
                _ => {}
            }
            i += 1;
        }
        None
    }

    pub(super) fn c_qualification_problem(&self, start: usize, end: usize) -> bool {
        if start < end && self.text(start) != "using" {
            let last = if self.cpp {
                alternative_operator(self.text(end - 1))
            } else {
                self.text(end - 1)
            };
            if (binary_precedence(last).is_some()
                && !matches!(last, "," | "...")
                && !(self.cpp && matches!(last, ">" | ">>") && self.template_id_end(start, end)))
                || matches!(last, "?" | "!" | "~" | "." | "->" | "::")
            {
                return true;
            }
        }
        let mut i = start;
        // Conditional colons belong to their own expression delimiter. A
        // colon in a nested group cannot close an outer conditional, and
        // record bit-fields and initializer designators belong to braces.
        let mut delimiters = (start..end)
            .any(|token| self.text(token) == ":")
            .then(|| vec![("", 0usize)]);
        while i < end {
            if let Some(after) = self.lambda_end(i, end) {
                i = after;
                continue;
            }
            // An operator-function-id (and its template arguments) is one
            // name. Its punctuation is not a sequence of binary operators.
            if self.cpp
                && self.text(i) == "operator"
                && let Some(after) = self.cpp_expression_name_end(i, end, false)
                && after > i + 1
            {
                // Empty parentheses after a complete operator name are its
                // argument list, including when the name ends in `>` or `]`.
                i = after
                    + if self.text(after) == "(" && self.text(after + 1) == ")" {
                        2
                    } else {
                        0
                    };
                continue;
            }
            if matches!(self.text(i), "@" | "#" | "##")
                || (!self.cpp
                    && (self.text(i) == "::"
                        || (self.text(i) == "_Generic" && self.text(i + 1) == "(")))
            {
                return true;
            }
            if self.text(i) == "..."
                && (i == start || matches!(self.text(i - 1), "=" | "("))
                && (i + 1 == end || matches!(self.text(i + 1), ")" | ","))
                && !(start..i).rev().any(|open| {
                    self.text(open) == "("
                        && self
                            .matching(open, end)
                            .is_some_and(|close| close > i && self.is_cast(open + 1, close))
                })
            {
                return true;
            }
            let text = self.text(i);
            if matches!(text, "." | "->")
                && !self.tokens.get(i + 1).is_some_and(|token| {
                    token.kind == TokenKind::Identifier
                        || (self.cpp
                            && self.text(i + 1) == "~"
                            && self
                                .tokens
                                .get(i + 2)
                                .is_some_and(|name| name.kind == TokenKind::Identifier))
                })
            {
                return true;
            }
            if text == "(" && self.text(i + 1) == ")" {
                let previous = i.checked_sub(1).map(|index| self.text(index));
                let callable = i > start
                    && (self.tokens[i - 1].kind == TokenKind::Identifier
                        || self.numeric_callee(i - 1)
                        || matches!(previous, Some(")" | "]")))
                    && !matches!(
                        previous,
                        Some(
                            "sizeof"
                                | "alignof"
                                | "_Alignof"
                                | "__alignof__"
                                | "__alignof"
                                | "typeof"
                                | "__typeof__"
                                | "__typeof"
                        )
                    );
                if !callable {
                    return true;
                }
            }
            let operator = if self.cpp {
                alternative_operator(text)
            } else {
                text
            };
            let infix_only = binary_precedence(operator).is_some()
                && !matches!(operator, "," | "+" | "-" | "*" | "&" | "&&" | "...")
                && !(self.cpp && matches!(operator, ">" | ">>"));
            if (operator == "?" || infix_only)
                && (i == start
                    || !(matches!(
                        self.tokens[i - 1].kind,
                        TokenKind::Identifier | TokenKind::Literal
                    ) || matches!(self.text(i - 1), ")" | "]" | "}" | "++" | "--")))
            {
                return true;
            }
            if let Some(delimiters) = &mut delimiters {
                match text {
                    "(" | "[" | "{" => delimiters.push((text, 0)),
                    ")" | "]" | "}" if delimiters.len() > 1 => {
                        delimiters.pop();
                    }
                    "?" => delimiters.last_mut().unwrap().1 += 1,
                    ":" => {
                        let (delimiter, conditionals) = delimiters.last_mut().unwrap();
                        if *conditionals > 0 {
                            *conditionals -= 1;
                        } else if matches!(*delimiter, "(" | "[") {
                            return true;
                        }
                    }
                    _ => {}
                }
            }
            if self.tokens[i].kind == TokenKind::Literal {
                let string = string_literal_token(text);
                if !self.cpp
                    && string
                    && !unclosed_quoted_literal(text)
                    && text.rfind('"').is_some_and(|last| last + 1 < text.len())
                {
                    return true;
                }
                if i > start {
                    let previous = self.text(i - 1);
                    let previous_string = string_literal_token(previous);
                    let previous_suffix = self.cpp
                        && i > start + 1
                        && string_literal_token(self.text(i - 2))
                        && self.tokens[i - 2].span.end == self.tokens[i - 1].span.start;
                    if (self.tokens[i - 1].kind == TokenKind::Literal
                        && !(string && previous_string))
                        || (self.tokens[i - 1].kind == TokenKind::Identifier
                            && !(previous_suffix && string)
                            && !matches!(
                                previous,
                                "sizeof"
                                    | "alignof"
                                    | "_Alignof"
                                    | "__alignof__"
                                    | "__alignof"
                                    | "typeof"
                                    | "__typeof__"
                                    | "__typeof"
                                    | "__extension__"
                                    | "__real__"
                                    | "__imag__"
                                    | "noexcept"
                                    | "throw"
                            )
                            && (!self.cpp || alternative_operator(previous) == previous))
                    {
                        return true;
                    }
                    if previous == "]"
                        || (previous == ")"
                            && !(start..i - 1).rev().any(|open| {
                                self.text(open) == "("
                                    && self.matching(open, end) == Some(i - 1)
                                    && self.is_cast(open + 1, i - 1)
                            }))
                    {
                        return true;
                    }
                }
            } else if self.tokens[i].kind == TokenKind::Identifier
                && i > start
                && self.tokens[i - 1].kind == TokenKind::Literal
                && (!self.cpp || alternative_operator(text) == text)
                && !(self.cpp
                    && string_literal_token(self.text(i - 1))
                    && self.tokens[i - 1].span.end == self.tokens[i].span.start)
            {
                return true;
            }
            // A GNU statement-expression has its own recovery boundaries.
            // An invalid inner statement does not turn the enclosing
            // expression into a problem expression.
            if self.text(i) == "{"
                && i > start
                && self.text(i - 1) == "("
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
                if let Some(delimiters) = &mut delimiters {
                    delimiters.pop();
                }
            } else {
                i += 1;
            }
        }
        false
    }

    pub(super) fn ordinary_call_problem(&self, start: usize, end: usize) -> bool {
        if self.call_brace(start, end).is_some()
            || (!self.cpp && self.operator_brace(start, end).is_some())
        {
            return true;
        }
        let mut i = start;
        while i < end {
            if let Some(after) = self.lambda_end(i, end) {
                i = after;
                continue;
            }
            if self.cpp
                && self.text(i) == "operator"
                && let Some(after) = self.cpp_expression_name_end(i, end, false)
                && after > i + 1
            {
                // Do not reinterpret the name's `()` or an empty call after
                // its template-id as a grouped operand. Nonempty arguments
                // still pass through the ordinary expression checks below.
                i = after;
                if self.text(i) == "(" && self.text(i + 1) == ")" {
                    i += 2;
                }
                continue;
            }
            if expression_type_keyword(self.text(i), self.cpp)
                && i > start
                && matches!(
                    self.text(i - 1),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
                )
                && !(self.cpp
                    && self.type_word(self.text(i))
                    && matches!(self.text(i + 1), "(" | "{"))
            {
                // A reserved type specifier is not an ordinary RHS operand.
                // Cast and sizeof type-ids are consumed as complete groups.
                return true;
            }
            if self.text(i) == "(" && self.matching(i, end).is_none() {
                return true;
            }
            if self.text(i) == "["
                && i > start
                && (self.tokens[i - 1].kind == TokenKind::Identifier
                    || matches!(self.text(i - 1), ")" | "]"))
                && !matches!(self.text(i - 1), "operator" | "delete" | "new")
                && !self.looks_declaration(start, end)
                && !(self.cpp && (start..i).any(|j| self.text(j) == "new"))
                && let Some(close) = self.matching(i, end)
                && self.call_argument_problem(i + 1, close)
            {
                // An array subscript needs a complete operand expression.
                // A bad index invalidates its containing statement rather
                // than leaving a partial address or assignment in the CFG.
                return true;
            }
            if self.text(i) == "("
                && let Some(close) = self.matching(i, end)
                && close > i + 1
                && matches!(
                    self.text(close - 1),
                    "*" | "&"
                        | "&&"
                        | "+"
                        | "-"
                        | "/"
                        | "%"
                        | "!"
                        | "~"
                        | "|"
                        | "||"
                        | "^"
                        | "="
                        | "?"
                        | "."
                        | "->"
                        | "::"
                )
                && !(i > start
                    && matches!(
                        self.text(i - 1),
                        "__builtin_va_arg"
                            | "__builtin_offsetof"
                            | "__builtin_types_compatible_p"
                            | "__offsetof__"
                    ))
                && !self.is_type_range(i + 1, close)
            {
                // A malformed abstract declarator is neither a cast type-id
                // nor a complete grouped expression. CDT recovers its entire
                // expression container, as for an invalid call argument.
                return true;
            }
            if self.text(i) == "{"
                && i > start
                && self.text(i - 1) == "("
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
                continue;
            }
            if self.text(i) == "("
                && i > start
                && let Some(close) = self.matching(i, end)
            {
                let callee = self.text(i - 1);
                let previous_cast = callee == ")"
                    && (start..i).rev().any(|open| {
                        self.text(open) == "("
                            && self.matching(open, end) == Some(i - 1)
                            && self.is_cast(open + 1, i - 1)
                    });
                if previous_cast
                    && self.text(i + 1) == "{"
                    && self.matching(i + 1, close) == Some(close - 1)
                {
                    // A cast can take a GNU statement-expression operand.
                    // Its compound has independent statement recovery, rather
                    // than being a bare brace argument to an ordinary call.
                    i = close + 1;
                    continue;
                }
                if close + 1 < end
                    && self.is_cast(i + 1, close)
                    && (self.tokens[i - 1].kind != TokenKind::Identifier
                        && !matches!(callee, ")" | "]")
                        || previous_cast)
                {
                    i = close + 1;
                    continue;
                }
                if matches!(
                    callee,
                    "__builtin_va_arg"
                        | "__builtin_offsetof"
                        | "__builtin_types_compatible_p"
                        | "__offsetof__"
                        | "__attribute__"
                        | "__attribute"
                        | "__declspec"
                        | "__extension__"
                ) || (matches!(
                    callee,
                    "sizeof"
                        | "alignof"
                        | "_Alignof"
                        | "__alignof__"
                        | "__alignof"
                        | "typeof"
                        | "__typeof__"
                        | "__typeof"
                        | "decltype"
                ) && self.is_type_range(i + 1, close))
                {
                    i = close + 1;
                    continue;
                }
                if self.tokens[i - 1].kind == TokenKind::Identifier
                    || self.numeric_callee(i - 1)
                    || matches!(callee, ")" | "]")
                {
                    if !self.cpp && expression_type_keyword(callee, false) {
                        return true;
                    }
                    if close > i + 1
                        && self
                            .split_ranges(i + 1, close, ",")
                            .into_iter()
                            .any(|(a, b)| self.call_argument_problem(a, b))
                    {
                        return true;
                    }
                }
            }
            if self.text(i) == "("
                && let Some(close) = self.matching(i, end)
                && (i == start
                    || (self.tokens[i - 1].kind != TokenKind::Identifier
                        && !self.numeric_callee(i - 1)
                        && !matches!(self.text(i - 1), ")" | "]")))
            {
                let type_id = self.is_type_range(i + 1, close);
                if (!type_id && self.call_argument_problem(i, close + 1))
                    || (type_id
                        && !self.can_start_expression(close + 1)
                        && (self.type_word(self.text(i + 1))
                            || qualifier(self.text(i + 1))
                            || (i + 1..close).any(|j| self.text(j) == "*")))
                {
                    // A malformed grouped expression or a cast with no
                    // operand invalidates its whole expression container.
                    // GNU statement blocks keep their own problem boundary.
                    return true;
                }
            }
            if i == start
                && self.text(i) == "("
                && let Some(close) = self.matching(i, end)
                && close + 1 < end
                && self.is_cast(i + 1, close)
            {
                i = close + 1;
            } else {
                i += 1;
            }
        }
        false
    }

    pub(super) fn call_argument_problem(&self, mut start: usize, mut end: usize) -> bool {
        let mut statement_expression = false;
        while self.text(start) == "(" && self.matching(start, end) == Some(end.saturating_sub(1)) {
            start += 1;
            end = end.saturating_sub(1);
            statement_expression |= self.text(start) == "{";
        }
        if start >= end {
            return true;
        }
        if statement_expression
            && self.text(start) == "{"
            && self.matching(start, end) == Some(end - 1)
        {
            // Each statement in a GNU expression block has its own CDT
            // problem boundary; a bad inner statement does not invalidate
            // the enclosing argument, condition or return expression.
            return false;
        }
        if self.c_qualification_problem(start, end) {
            return true;
        }
        let first = self.text(start);
        if first == "," {
            return true;
        }
        if first == "{" && !self.cpp && !statement_expression {
            // A bare braced list is an argument in C++, while C requires a
            // compound literal's explicit type or a GNU statement-expression.
            return true;
        }
        if expression_type_keyword(first, self.cpp) {
            // C++ has functional casts to a simple type; a bare type-id is
            // never an ordinary call argument. Typedef spellings themselves
            // remain identifier expressions, as in CDT.
            if !(self.cpp && self.type_word(first) && matches!(self.text(start + 1), "(" | "{")) {
                return true;
            }
        }
        let mut i = start;
        while i < end {
            if self.text(i) == "," && matches!(self.text(i.saturating_sub(1)), "(" | "[") {
                // A comma needs a left operand. CDT replaces its complete
                // containing condition rather than retaining partial calls.
                return true;
            }
            if let Some(after) = self.lambda_end(i, end) {
                i = after;
                continue;
            }
            if matches!(
                self.text(i),
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
                    | "+"
                    | "-"
                    | "*"
                    | "/"
                    | "%"
                    | "&"
                    | "&&"
                    | "|"
                    | "||"
                    | "^"
            ) && (i + 1 == end || matches!(self.text(i + 1), "," | ")" | "]"))
            {
                return true;
            }
            if self.text(i) == "("
                && self.text(i + 1) == "{"
                && (i == start
                    || (self.tokens[i - 1].kind != TokenKind::Identifier
                        && !matches!(self.text(i - 1), ")" | "]")))
                && let Some(close) = self.matching(i, end)
                && let Some(brace_close) = self.matching(i + 1, close)
                && brace_close + 1 != close
            {
                // A GNU statement expression ends immediately after its
                // compound. A brace literal followed by an infix operator
                // inside that group is a problem expression in both modes.
                return true;
            }
            if self.text(i) == "("
                && i > start
                && matches!(
                    self.text(i - 1),
                    "__builtin_va_arg"
                        | "__builtin_offsetof"
                        | "__builtin_types_compatible_p"
                        | "__offsetof__"
                )
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
                continue;
            }
            if self.text(i) == "("
                && let Some(close) = self.matching(i, end)
                && (self.cast_group_ends_at(start, close, end)
                    || (i > start
                        && matches!(
                            self.text(i - 1),
                            "sizeof"
                                | "alignof"
                                | "_Alignof"
                                | "__alignof__"
                                | "__alignof"
                                | "typeof"
                                | "__typeof__"
                                | "__typeof"
                                | "decltype"
                        )
                        && self.is_type_range(i + 1, close)))
            {
                i = close + 1;
                continue;
            }
            if i > start
                && matches!(self.text(i), "!" | "~")
                && (matches!(
                    self.tokens[i - 1].kind,
                    TokenKind::Identifier | TokenKind::Literal
                ) || matches!(self.text(i - 1), ")" | "]"))
                && !self.cast_group_ends_at(start, i - 1, end)
                && !matches!(
                    self.text(i - 1),
                    "sizeof"
                        | "alignof"
                        | "_Alignof"
                        | "__alignof__"
                        | "__alignof"
                        | "typeof"
                        | "__typeof__"
                        | "__typeof"
                        | "__extension__"
                        | "__real__"
                        | "__imag__"
                        | "noexcept"
                        | "throw"
                        | "operator"
                )
                && !(self.cpp && alternative_operator(self.text(i - 1)) != self.text(i - 1))
            {
                return true;
            }
            if i > start
                && self.tokens[i].kind == TokenKind::Identifier
                && matches!(self.text(i - 1), ")" | "]")
                && !self.cast_group_ends_at(start, i - 1, end)
                && (!self.cpp || alternative_operator(self.text(i)) == self.text(i))
            {
                // An absent operator inside an argument or grouped operand
                // cannot recover a second top-level statement.
                return true;
            }
            if self.text(i) == "{"
                && (i == start || self.text(i - 1) == "(")
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
                continue;
            }
            if i > start
                && self.tokens[i].kind == TokenKind::Identifier
                && self.tokens[i - 1].kind == TokenKind::Identifier
            {
                let previous = self.text(i - 1);
                if !matches!(
                    previous,
                    "sizeof"
                        | "alignof"
                        | "_Alignof"
                        | "__alignof__"
                        | "__alignof"
                        | "typeof"
                        | "__typeof__"
                        | "__typeof"
                        | "__extension__"
                        | "__real__"
                        | "__imag__"
                        | "noexcept"
                        | "throw"
                ) && !(self.cpp
                    && (alternative_operator(previous) != previous
                        || alternative_operator(self.text(i)) != self.text(i)
                        || matches!(previous, "new" | "delete" | "typename")
                        || (previous == "template"
                            && i >= start + 2
                            && matches!(self.text(i - 2), "::" | "." | "->")
                            && !expression_type_keyword(self.text(i), true))
                        || expression_type_keyword(previous, true)))
                {
                    return true;
                }
            }
            i += 1;
        }
        matches!(
            self.text(end - 1),
            "*" | "&"
                | "&&"
                | "+"
                | "-"
                | "/"
                | "%"
                | "!"
                | "~"
                | "^"
                | "|"
                | "||"
                | "="
                | "?"
                | ":"
                | "."
                | "->"
                | "::"
        )
    }

    pub(super) fn declaration_expression_problem(&self, start: usize, end: usize) -> bool {
        if self.cpp && self.unpaired_expression_colon(start, end).is_some() {
            return true;
        }
        let base_end = self.specifier_end(start, end);
        for (a, b) in self.split_ranges(base_end, end, ",") {
            let (_, mut i) = self.declarator_name(a, b);
            while i < b {
                if let Some(after) = self.skip_attribute(i, b) {
                    i = after;
                    continue;
                }
                match self.text(i) {
                    "=" | "{" => {
                        if self.ordinary_call_problem(i + usize::from(self.text(i) == "="), b) {
                            return true;
                        }
                        break;
                    }
                    "[" => {
                        if let Some(close) = self.matching(i, b) {
                            if self.ordinary_call_problem(i + 1, close) {
                                return true;
                            }
                            i = close + 1;
                        } else {
                            break;
                        }
                    }
                    "(" => {
                        let close = self.matching(i, b).unwrap_or(b.saturating_sub(1));
                        if self.cpp
                            && self.function_header(a, b).is_none()
                            && self.ordinary_call_problem(i + 1, close)
                        {
                            return true;
                        }
                        i = close + 1;
                    }
                    _ => i += 1,
                }
            }
        }
        false
    }

    pub(super) fn c_attribute_problem(&self, start: usize, end: usize) -> bool {
        if self.cpp {
            return false;
        }
        let mut i = start;
        while i < end {
            if self.text(i) == "[" && self.text(i + 1) == "[" {
                return true;
            }
            if self.text(i) == "{"
                && i > start
                && self.text(i - 1) == "("
                && let Some(close) = self.matching(i, end)
            {
                i = close + 1;
            } else {
                i += 1;
            }
        }
        false
    }

    /// CDT skips a statement that starts with a stray closer like any problem
    /// statement: it ends after a terminator or after a brace group that
    /// returns to the starting depth, leaving later statements intact.
    pub(super) fn stray_closer_problem_end(&self, start: usize) -> usize {
        let mut i = start + 1;
        while i < self.limit {
            match self.text(i) {
                ";" => return i + 1,
                "}" | "%>" => return i,
                "{" | "<%" => {
                    return self
                        .brace_end(i, self.limit)
                        .map_or(self.limit, |close| close + 1);
                }
                "(" | "[" => {
                    i = self
                        .matching(i, self.limit)
                        .map_or(i + 1, |close| close + 1)
                }
                _ => i += 1,
            }
        }
        i
    }

    pub(super) fn problem_statement_end(&self, start: usize) -> usize {
        if start >= self.limit {
            return self.limit;
        }
        match self.text(start) {
            "{" | "<%" => self
                .matching(start, self.limit)
                .map_or(self.limit, |n| n + 1),
            "if" | "while" | "for" | "switch" => {
                let keyword = self.text(start);
                let open = start + 1 + usize::from(self.text(start + 1) == "constexpr");
                let Some(close) = self.matching(open, self.limit) else {
                    return self.limit;
                };
                let mut end = self.problem_statement_end(close + 1);
                if keyword == "if" && self.text(end) == "else" {
                    end = self.problem_statement_end(end + 1);
                }
                end
            }
            "do" => {
                let body_end = self.problem_statement_end(start + 1);
                if self.text(body_end) == "while" {
                    self.matching(body_end + 1, self.limit)
                        .map_or(self.limit, |n| n + 1 + usize::from(self.text(n + 1) == ";"))
                } else {
                    body_end
                }
            }
            _ if self.tokens[start].kind == TokenKind::Identifier
                && self.text(start + 1) == ":" =>
            {
                self.problem_statement_end(start + 2)
            }
            _ => {
                if let Some(open) = self.direct_call_brace(start, self.limit) {
                    return self
                        .matching(open, self.limit)
                        .map_or(self.limit, |close| close + 1);
                }
                let end = self.statement_end(start);
                end + usize::from(self.text(end) == ";")
            }
        }
    }
}
