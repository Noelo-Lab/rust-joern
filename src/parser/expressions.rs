//! Expressions, calls and operators.

use super::*;

impl Parser<'_> {
    pub(super) fn scalar_member_initializers(
        &mut self,
        start: usize,
        end: usize,
        fields: &[(String, String)],
        functions: &[Function],
    ) -> Option<Vec<MemberInitialization>> {
        let mut entries = HashMap::new();
        for (a, b) in self.split_ranges(start, end, ",") {
            if a + 2 >= b
                || self.tokens[a].kind != TokenKind::Identifier
                || self.text(a + 1) != "("
                || self.matching(a + 1, b) != Some(b - 1)
                || self.split_ranges(a + 2, b - 1, ",").len() != 1
                || a + 2 == b - 1
            {
                return None;
            }
            let name = self.text(a).to_string();
            if !fields.iter().any(|(field, _)| field == &name)
                || entries.insert(name, (a, b)).is_some()
            {
                return None;
            }
        }
        if entries.len() != fields.len() {
            return None;
        }
        let mut plan = Vec::new();
        for (member, expected_type) in fields {
            let &(a, b) = entries.get(member)?;
            let value = self.expression_range(a + 2, b - 1);
            if !self.scalar_initialization_value(&value, expected_type, fields, functions) {
                return None;
            }
            plan.push(MemberInitialization {
                member: member.clone(),
                value,
                span: self.span(a, b),
                member_span: self.span(a, a + 1),
            });
        }
        Some(plan)
    }

    pub(super) fn for_initializer(&mut self, start: usize, end: usize) -> Option<Box<Stmt>> {
        if start == end {
            return None;
        }
        let kind = if self.looks_declaration(start, end) {
            StmtKind::Declaration(self.declarations_range(start, end))
        } else {
            StmtKind::Expression(self.expression_range(start, end))
        };
        Some(Box::new(Stmt {
            kind,
            span: self.span(start, end),
        }))
    }

    pub(super) fn expression_range(&mut self, start: usize, end: usize) -> Expr {
        if self.c_qualification_problem(start, end)
            || self.ordinary_call_problem(start, end)
            || self.unpaired_expression_colon(start, end).is_some()
            || self.star_literal_problem(start, end)
        {
            return Expr {
                kind: ExprKind::Problem(self.raw(start, end)),
                span: self.span(start, end),
            };
        }
        let (old_pos, old_limit) = (self.pos, self.limit);
        self.pos = start;
        self.limit = end;
        let expression = self.expression(0);
        if self.pos < end {
            self.diagnose(self.pos, end, "unparsed executable expression tokens");
        }
        self.pos = old_pos;
        self.limit = old_limit;
        expression
    }

    pub(super) fn initializer_expression_range(&mut self, start: usize, end: usize) -> Expr {
        let end = self
            .detached_identifier_suffix(start, end)
            .or_else(|| self.stray_postfix_operator(start, end))
            .or_else(|| self.unmatched_expression_close(start, end))
            .unwrap_or(end);
        let end = if self.cpp {
            end
        } else {
            self.unpaired_expression_colon(start, end).unwrap_or(end)
        };
        self.expression_range(start, end)
    }

    pub(super) fn cast_group_ends_at(&self, start: usize, mut close: usize, end: usize) -> bool {
        while self.text(close) == ")" {
            let Some(open) = (start..close)
                .rev()
                .find(|&open| self.text(open) == "(" && self.matching(open, end) == Some(close))
            else {
                return false;
            };
            if !self.is_cast(open + 1, close) {
                return false;
            }
            if open == start {
                return true;
            }
            let previous = self.text(open - 1);
            if self.tokens[open - 1].kind == TokenKind::Identifier {
                return self.cpp && alternative_operator(previous) != previous;
            }
            if previous != ")" {
                return previous != "]";
            }
            close = open - 1;
        }
        false
    }

    pub(super) fn unmatched_expression_close(&self, start: usize, end: usize) -> Option<usize> {
        let mut cursor = start;
        while cursor < end {
            match self.text(cursor) {
                ")" | "]" => return Some(cursor),
                "(" | "[" | "{" => {
                    if let Some(close) = self.matching(cursor, end) {
                        cursor = close + 1;
                        continue;
                    }
                }
                _ => {}
            }
            cursor += 1;
        }
        None
    }

    /// CDT's postfix grammar calls any primary expression, including the
    /// numeric literals decompilers emit for unresolved absolute call targets.
    pub(super) fn numeric_callee(&self, i: usize) -> bool {
        self.tokens[i].kind == TokenKind::Literal
            && self.text(i).starts_with(|c: char| c.is_ascii_digit())
    }

    pub(super) fn expression(&mut self, minimum: u8) -> Expr {
        let mut start = self.pos;
        // CDT's scanner expands `__extension__` to nothing: no node starts there.
        while start + 1 < self.limit && self.text(start) == "__extension__" {
            start += 1;
        }
        let mut left = self.prefix();
        loop {
            if self.pos >= self.limit {
                break;
            }
            let op = if self.cpp {
                alternative_operator(self.peek())
            } else {
                self.peek()
            }
            .to_string();
            // All postfix forms bind more tightly than unary/binary operators.
            if self.cpp
                && minimum <= 15
                && op == "{"
                && matches!(&left.kind, ExprKind::Identifier(name) if self.explicit_types.contains(name)
                        && (!self.supplied_type_names.contains(name) || !self.variables.contains(name)))
            {
                let initializer = self.prefix();
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments: vec![initializer],
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && op == "(" {
                if !self.macro_expansion
                    && let ExprKind::Identifier(name) = &left.kind
                    && matches!(
                        name.as_str(),
                        "__builtin_va_arg"
                            | "__builtin_offsetof"
                            | "__builtin_types_compatible_p"
                            | "__offsetof__"
                    )
                    && let Some(call) = self.builtin_macro_call(name.clone(), start)
                {
                    left = call;
                    continue;
                }
                self.pos += 1;
                let mut arguments = Vec::new();
                while self.pos < self.limit && !self.at(")") {
                    let before = self.pos;
                    arguments.push(self.expression(1));
                    if !self.eat(",") || self.pos <= before {
                        break;
                    }
                }
                self.expect(")");
                self.record_retained_macro_call(&left, &arguments);
                // A call through an identifier that CDT types as a pointer
                // is a pointer call with that identifier as its receiver.
                let global_pointer = matches!(&left.kind, ExprKind::Identifier(name)
                    if self.global_pointers.contains(name)
                        && !self.variable_types.contains_key(name));
                if matches!(left.kind, ExprKind::Identifier(_))
                    && (global_pointer || self.span(start, start + 1).start < left.span.start)
                {
                    let span = left.span.clone();
                    left = Expr {
                        kind: ExprKind::Bracketed(Box::new(left)),
                        span,
                    };
                }
                left = Expr {
                    kind: ExprKind::Call {
                        callee: Box::new(left),
                        arguments,
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && op == "[" {
                self.pos += 1;
                let index = self.expression(0);
                self.expect("]");
                left = Expr {
                    kind: ExprKind::Index {
                        base: Box::new(left),
                        index: Box::new(index),
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && matches!(op.as_str(), "." | "->") {
                self.pos += 1;
                if self.cpp {
                    self.eat("template");
                }
                let name_start = self.pos;
                let mut destructor = None;
                if let Some(name) = self.cpp_expression_name(name_start, self.limit, true) {
                    self.pos = name.end;
                    destructor = name.destructor;
                    if destructor.is_some() && !(self.at("(") && self.text(self.pos + 1) == ")") {
                        self.diagnose(
                            name_start,
                            self.pos,
                            "unsupported destructor-id; an empty destructor call is required",
                        );
                    }
                } else if self.cpp && alternative_operator(self.peek()) == "~" {
                    self.pos += 1;
                    self.diagnose(
                        name_start,
                        self.pos,
                        "unsupported destructor-id; an empty destructor call is required",
                    );
                } else if !self.peek().is_empty() {
                    self.pos += 1;
                }
                let name = if let Some(marker) = destructor {
                    let owner = if marker > name_start {
                        self.raw(name_start, marker)
                    } else {
                        String::new()
                    };
                    format!("{owner}~{}", self.raw(marker + 1, self.pos))
                } else {
                    self.raw(name_start, self.pos)
                };
                left = Expr {
                    kind: ExprKind::Member {
                        base: Box::new(left),
                        name,
                        indirect: op == "->",
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if minimum <= 15 && matches!(op.as_str(), "++" | "--") {
                self.pos += 1;
                left = Expr {
                    kind: ExprKind::Unary {
                        op,
                        argument: Box::new(left),
                        postfix: true,
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            if op == "?" && minimum <= 2 {
                self.pos += 1;
                let consequence = if self.at(":") {
                    Expr {
                        kind: ExprKind::Unknown(String::new()),
                        span: self.span(self.pos, self.pos),
                    }
                } else {
                    self.expression(0)
                };
                self.expect(":");
                let alternative = self.expression(1);
                left = Expr {
                    kind: ExprKind::Conditional {
                        condition: Box::new(left),
                        consequence: Box::new(consequence),
                        alternative: Box::new(alternative),
                    },
                    span: self.span(start, self.pos),
                };
                continue;
            }
            let Some((precedence, right_associative)) = binary_precedence(&op) else {
                break;
            };
            if precedence < minimum {
                break;
            }
            self.pos += 1;
            let initializer_clause = self.cpp && precedence == 1 && self.at("{");
            let mut right = self.expression(if right_associative {
                precedence
            } else {
                precedence + 1
            });
            if initializer_clause {
                // CPP initializer-clauses are not operand expressions in
                // Joern's binary-expression AST conversion.
                right.kind = ExprKind::Unknown(String::new());
            }
            if op == "," {
                let mut values = match left.kind {
                    ExprKind::List(v) => v,
                    _ => vec![left],
                };
                values.push(right);
                left = Expr {
                    kind: ExprKind::List(values),
                    span: self.span(start, self.pos),
                };
            } else {
                left = Expr {
                    kind: ExprKind::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span: self.span(start, self.pos),
                };
            }
        }
        left
    }

    pub(super) fn prefix(&mut self) -> Expr {
        let start = self.pos;
        let token = if self.cpp {
            alternative_operator(self.peek())
        } else {
            self.peek()
        }
        .to_string();
        let kind = match token.as_str() {
            "true" | "false" | "nullptr" if self.cpp => {
                self.pos += 1;
                ExprKind::Literal(token)
            }
            "__null" if !self.macro_expansion => {
                self.pos += 1;
                let replacement = if self.cpp { "0" } else { "(void *)0" };
                return self.expanded_macro(token, 0, &[], replacement, self.span(start, self.pos));
            }
            "__imag__" | "__real__" if !self.macro_expansion => {
                self.pos += 1;
                let operand_start = self.pos;
                self.expression(14);
                let replacement = format!("(int){}", self.raw(operand_start, self.pos));
                return self.expanded_macro(
                    token,
                    0,
                    &[],
                    &replacement,
                    self.span(start, self.pos),
                );
            }
            "__extension__" => {
                self.pos += 1;
                return self.expression(14);
            }
            "[" if self.cpp => return self.lambda_expression(),
            "(" => {
                self.pos += 1;
                if self.at("{") {
                    let statement = self.statement();
                    self.expect(")");
                    return Expr {
                        span: self.span(start, self.pos),
                        kind: ExprKind::Statement(Box::new(statement)),
                    };
                }
                let close = self.matching(start, self.limit);
                if close.is_some_and(|end| self.is_cast(start + 1, end)) {
                    let close = close.unwrap();
                    let type_name = if self.macro_expansion {
                        macro_type_code(&self.raw(start + 1, close))
                    } else {
                        self.clean_type(start + 1, close)
                    };
                    self.pos = close + 1;
                    let argument = self.expression(14);
                    ExprKind::Cast {
                        type_name,
                        argument: Box::new(argument),
                    }
                } else {
                    let inner = self.expression(0);
                    self.expect(")");
                    // CDT's bracketed-primary conversion retains only the operand AST.
                    return if self.macro_expansion || matches!(inner.kind, ExprKind::List(_)) {
                        // Expanded parent CODE still contains these brackets,
                        // although their operand has no extra CPG AST vertex.
                        let span = if matches!(inner.kind, ExprKind::List(_)) {
                            self.span(start, self.pos)
                        } else {
                            inner.span.clone()
                        };
                        Expr {
                            span,
                            kind: ExprKind::Bracketed(Box::new(inner)),
                        }
                    } else {
                        inner
                    };
                }
            }
            "{" => {
                self.pos += 1;
                let mut values = Vec::new();
                while self.pos < self.limit && !self.at("}") {
                    let before = self.pos;
                    let old_field_designator = self.tokens[self.pos].kind == TokenKind::Identifier
                        && self.text(self.pos + 1) == ":";
                    if self.at(".") || self.at("[") || old_field_designator {
                        let mut designators = Vec::new();
                        if old_field_designator {
                            let field = self.pos;
                            self.pos += 2;
                            designators.push(Expr {
                                kind: ExprKind::Identifier(self.text(field).into()),
                                span: self.span(field, field + 1),
                            });
                        }
                        while self.at(".") || self.at("[") {
                            if self.eat(".") {
                                let field = self.pos;
                                if self.pos < self.limit
                                    && self.tokens[self.pos].kind == TokenKind::Identifier
                                {
                                    self.pos += 1;
                                } else {
                                    self.diagnose(
                                        field,
                                        field + 1,
                                        "field designator requires a name",
                                    );
                                }
                                designators.push(Expr {
                                    kind: ExprKind::Identifier(self.raw(field, self.pos)),
                                    span: self.span(field, self.pos),
                                });
                            } else {
                                let open = self.pos;
                                self.pos += 1;
                                let index = self.expression(0);
                                self.expect("]");
                                let index = match index.kind {
                                    ExprKind::Binary { op, left, right } if op == "..." => Expr {
                                        kind: ExprKind::ArrayInitializer(vec![*left, *right]),
                                        span: self.span(open, self.pos),
                                    },
                                    _ => index,
                                };
                                designators.push(index);
                            }
                        }
                        // CDT's GNU array designator permits omission of '='.
                        self.eat("=");
                        let value = self.expression(1);
                        let assignments = designators
                            .into_iter()
                            .map(|target| Expr {
                                kind: ExprKind::Binary {
                                    op: "=".into(),
                                    left: Box::new(target),
                                    right: Box::new(value.clone()),
                                },
                                span: self.span(before, self.pos),
                            })
                            .collect();
                        values.push(Expr {
                            kind: ExprKind::Block(assignments),
                            span: self.span(before, self.pos),
                        });
                    } else {
                        values.push(self.expression(1));
                    }
                    if self.pos <= before {
                        self.pos += 1;
                    }
                    if !self.eat(",") {
                        break;
                    }
                }
                self.expect("}");
                ExprKind::ArrayInitializer(values)
            }
            "sizeof"
                if self.cpp
                    && self.text(start + 1) == "..."
                    && self.text(start + 2) == "("
                    && self
                        .tokens
                        .get(start + 3)
                        .is_some_and(|t| t.kind == TokenKind::Identifier)
                    && self.text(start + 4) == ")" =>
            {
                // A pack-size operand is an unevaluated identifier, not an
                // expression or a regular sizeof type-id. Preserve the full
                // source span while matching CDT's sizeOf operator family.
                self.pos = start + 5;
                let argument = Expr {
                    kind: ExprKind::Identifier(self.text(start + 3).to_string()),
                    span: self.span(start + 3, start + 4),
                };
                ExprKind::Unary {
                    op: "sizeof...".into(),
                    argument: Box::new(argument),
                    postfix: false,
                }
            }
            "++" | "--" | "+" | "-" | "*" | "&" | "!" | "~" | "&&" | "sizeof" | "alignof"
            | "_Alignof" | "__alignof__" | "__alignof" | "typeof" | "__typeof__" | "noexcept"
            | "delete" | "throw"
                if self.cpp || !matches!(token.as_str(), "delete" | "throw" | "noexcept") =>
            {
                self.pos += 1;
                if token == "delete" && self.eat("[") {
                    self.expect("]");
                }
                let argument = if self.at("(")
                    && self
                        .matching(self.pos, self.limit)
                        .is_some_and(|end| self.is_type_range(self.pos + 1, end))
                    && matches!(
                        token.as_str(),
                        "sizeof"
                            | "alignof"
                            | "_Alignof"
                            | "__alignof__"
                            | "__alignof"
                            | "typeof"
                            | "__typeof__"
                    ) {
                    let open = self.pos;
                    let end = self.matching(open, self.limit).unwrap();
                    // astForTypeIdExpression converts only the declaration
                    // specifier: an abstract pointer is not an operand, and a
                    // typeof's internal expression is not evaluated here.
                    let specifier = self.specifier_end(open + 1, end);
                    let typeof_specifier = matches!(
                        self.text(open + 1),
                        "typeof" | "__typeof__" | "__typeof" | "decltype"
                    );
                    let argument_end = if typeof_specifier {
                        open + 2
                    } else {
                        specifier
                    };
                    let type_name = self.clean_type(open + 1, argument_end);
                    let type_name = type_name
                        .strip_prefix("struct ")
                        .or_else(|| type_name.strip_prefix("union "))
                        .or_else(|| type_name.strip_prefix("enum "))
                        .unwrap_or(&type_name)
                        .to_string();
                    let argument = Expr {
                        kind: ExprKind::TypeSpecifier {
                            code: self.raw(open + 1, argument_end),
                            type_name,
                        },
                        span: self.span(open + 1, argument_end),
                    };
                    self.pos = end + 1;
                    argument
                } else {
                    self.expression(14)
                };
                ExprKind::Unary {
                    op: token,
                    argument: Box::new(argument),
                    postfix: false,
                }
            }
            "static_cast" | "reinterpret_cast" | "const_cast" | "dynamic_cast"
                if self.cpp && self.text(start + 1) == "<" =>
            {
                let close = self.angle_close(start + 1, self.limit).unwrap_or(start + 1);
                let type_name = if self.macro_expansion {
                    macro_type_code(&self.raw(start + 2, close))
                } else {
                    self.clean_type(start + 2, close)
                };
                self.pos = close + 1;
                self.expect("(");
                let argument = self.expression(0);
                self.expect(")");
                ExprKind::Cast {
                    type_name,
                    argument: Box::new(argument),
                }
            }
            "new" if self.cpp => {
                self.pos += 1;
                let type_start = self.pos;
                self.pos = self.specifier_end(self.pos, self.limit);
                let type_end = self.pos;
                while self.eat("*") {}
                let type_name = self.clean_type(type_start, type_end).replace("::", ".");
                let mut arguments = vec![Expr {
                    kind: ExprKind::TypeSpecifier {
                        code: self.raw(type_start, type_end),
                        type_name,
                    },
                    span: self.span(type_start, type_end),
                }];
                while self.eat("[") {
                    arguments.push(self.expression(0));
                    self.expect("]");
                }
                if self.eat("(") {
                    while self.pos < self.limit && !self.at(")") {
                        arguments.push(self.expression(1));
                        if !self.eat(",") {
                            break;
                        }
                    }
                    self.expect(")");
                } else if self.at("{") {
                    arguments.push(self.prefix());
                }
                ExprKind::Call {
                    callee: Box::new(Expr {
                        kind: ExprKind::Identifier("<operator>.new".into()),
                        span: self.span(start, start + 1),
                    }),
                    arguments,
                }
            }
            "::" if self
                .tokens
                .get(start + 1)
                .is_some_and(|t| t.kind == TokenKind::Identifier) =>
            {
                if self.cpp {
                    self.pos = self
                        .cpp_expression_name_end(start, self.limit, false)
                        .unwrap_or(start + 2);
                } else {
                    self.pos += 2;
                    while self.eat("::") {
                        self.eat("template");
                        if self.pos < self.limit {
                            self.pos += 1;
                        }
                    }
                }
                ExprKind::Identifier(self.raw(start, self.pos))
            }
            "" | ")" | "]" | "}" | ";" | ":" => {
                self.diagnose(start, start, "missing expression");
                ExprKind::Unknown(String::new())
            }
            _ => {
                self.pos += 1;
                match self.tokens[start].kind {
                    TokenKind::Literal => {
                        // Adjacent string literals are one CDT literal expression.
                        if self.cpp
                            && string_literal_token(&token)
                            && self.pos < self.limit
                            && self.tokens[self.pos].kind == TokenKind::Identifier
                            && self.tokens[self.pos - 1].span.end
                                == self.tokens[self.pos].span.start
                        {
                            self.pos += 1;
                        }
                        while self.pos < self.limit
                            && self.tokens[self.pos].kind == TokenKind::Literal
                            && self.text(self.pos).contains('"')
                            && token.contains('"')
                        {
                            self.pos += 1;
                            if self.cpp
                                && self.pos < self.limit
                                && self.tokens[self.pos].kind == TokenKind::Identifier
                                && self.tokens[self.pos - 1].span.end
                                    == self.tokens[self.pos].span.start
                            {
                                self.pos += 1;
                            }
                        }
                        ExprKind::Literal(self.raw(start, self.pos))
                    }
                    TokenKind::Identifier => {
                        if self.cpp {
                            self.pos = self
                                .cpp_expression_name_end(start, self.limit, false)
                                .unwrap_or(start + 1);
                        } else {
                            while self.eat("::") {
                                self.eat("template");
                                if self.pos < self.limit {
                                    self.pos += 1;
                                }
                            }
                        }
                        ExprKind::Identifier(self.raw(start, self.pos))
                    }
                    TokenKind::Punctuation => {
                        self.diagnose(
                            start,
                            self.pos,
                            format!("unsupported expression token '{token}'"),
                        );
                        ExprKind::Unknown(token)
                    }
                }
            }
        };
        Expr {
            kind,
            span: self.span(start, self.pos),
        }
    }

    pub(super) fn can_start_expression(&self, i: usize) -> bool {
        if i >= self.limit {
            return false;
        }
        self.tokens[i].kind != TokenKind::Punctuation
            || matches!(
                self.text(i),
                "(" | "{" | "::" | "++" | "--" | "+" | "-" | "*" | "&" | "!" | "~"
            )
            || self.cpp && self.text(i) == "["
    }

    pub(super) fn is_cast(&self, start: usize, end: usize) -> bool {
        let next = self.text(end + 1);
        if end == start + 1
            && self.tokens[start].kind == TokenKind::Identifier
            && !self.type_word(self.text(start))
            && !qualifier(self.text(start))
        {
            // CDT chooses the syntactically complete cast/prefix form even
            // when this spelling also has an ordinary object binding. With
            // no prefix operand it instead chooses a grouped postfix form.
            if matches!(next, "++" | "--") {
                return self.can_start_expression(end + 2);
            }
            if (matches!(next, "!" | "~") && self.can_start_expression(end + 2))
                || (self
                    .tokens
                    .get(end + 1)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && !expression_type_keyword(next, self.cpp)
                    && (!self.cpp || alternative_operator(next) == next))
                || self
                    .tokens
                    .get(end + 1)
                    .is_some_and(|token| token.kind == TokenKind::Literal)
            {
                return true;
            }
            if self.variables.contains(self.text(start)) {
                if next == "("
                    && let Some(close) = self.matching(end + 1, self.limit)
                    && self.is_cast(end + 2, close)
                {
                    // A following complete cast is not an ordinary call
                    // argument, even when the outer name binds to an object.
                    return true;
                }
                // A real object keeps the binary or call interpretation in
                // ambiguous forms such as '(T)+x' and '(T)(x) in both modes.
                return false;
            }
        }
        if !self.is_type_range(start, end) || !self.can_start_expression(end + 1) {
            return false;
        }
        let first = self.text(start);
        if self.type_word(first)
            || qualifier(first)
            || self.explicit_types.contains(first)
            || matches!(first, "struct" | "union" | "enum" | "class")
            || (start..end).any(|i| self.text(i) == "*")
        {
            return true;
        }
        if matches!(first, "typeof" | "__typeof__" | "__typeof") {
            // The group is a type-id whatever its operand names. Before
            // `(` CDT tries the call reading only when a name in the type-id
            // has no binding, and keeps it only for a callee of function
            // type, which a typeof group never has.
            return true;
        }
        // An absent typedef is common after DecBench removes system headers.
        // '+'/'-'/'*'/'&' after an unknown bracketed identifier are ambiguous
        // binary expressions; retain that interpretation unless it is a type.
        if next == "("
            && let Some(close) = self.matching(end + 1, self.limit)
            && self.is_cast(end + 2, close)
        {
            // A following cast resolves the otherwise ambiguous '(T)(...)'
            // form: its type-id is not a valid call argument expression.
            return true;
        }
        self.tokens[end + 1].kind != TokenKind::Punctuation
            || matches!(next, "{" | "!" | "~")
            || (matches!(next, "++" | "--") && self.can_start_expression(end + 2))
    }
}
