//! Statements and control flow syntax.

use super::*;

impl Parser<'_> {
    pub(super) fn brace_end(&self, start: usize, limit: usize) -> Option<usize> {
        if !matches!(self.text(start), "{" | "<%") {
            return None;
        }
        let mut depth = 0usize;
        for i in start..limit {
            match self.text(i) {
                "{" | "<%" => depth += 1,
                "}" | "%>" => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    pub(super) fn statement(&mut self) -> Stmt {
        let start = self.pos;
        let empty_initializer = self.text(start + 2) == "=" && self.text(start + 3) == ";";
        if !self.cpp
            && self.cdt_identifier(start, self.limit)
            && self.cdt_identifier(start + 1, self.limit)
            && (self.text(start + 2) == ":" || empty_initializer)
        {
            // `T name : ...` is no C declaration, as only members may be
            // bit-fields. CDT keeps the expression statement `T`, reports its
            // missing semicolon, and parses a labeled statement from `name`.
            // After `T name = ;` the rest is one problem statement instead.
            let expression = self.expression_range(start, start + 1);
            self.pos = start + 1;
            let labeled = if empty_initializer {
                self.pos = start + 4;
                Stmt {
                    kind: StmtKind::Problem,
                    span: self.span(start + 1, self.pos),
                }
            } else {
                self.statement()
            };
            let span = expression.span.clone();
            return self.unterminated(start, StmtKind::Expression(expression), span, labeled, true);
        }
        if let Some(after) = self.cdt_unterminated_declarator(start) {
            // CDT ends a local declaration at an identifier that follows its
            // declarator, reports the missing semicolon, and parses the rest
            // as the next statement.
            // By default a label or control statement before the declaration
            // stays, although CDT fails it with the missing semicolon.
            let diagnostics = self.diagnostics.len();
            let declarations = self.declarations_range(start, after);
            if self.diagnostics.len() == diagnostics {
                let span = self.span(start, after);
                self.pos = after;
                return self.missing_semicolon(
                    start,
                    StmtKind::Declaration(declarations),
                    span,
                    self.cdt_failures,
                );
            }
            // Its declarator has no name for the type-directed conversion,
            // which then keeps the whole statement.
            self.diagnostics.truncate(diagnostics);
            self.pos = start;
        }
        if let Some(statement) = self.local_problem_enum_statement(start) {
            return statement;
        }
        if let Some(statement) = self.nested_function_statement(start) {
            return statement;
        }
        let kind = match self.peek() {
            "{" | "<%" => {
                self.pos += 1;
                let old_variables = self.variables.clone();
                let old_variable_types = self.variable_types.clone();
                let old_variable_closures = self.variable_closures.clone();
                let mut statements = Vec::new();
                while self.pos < self.limit && !matches!(self.peek(), "}" | "%>") {
                    let before = self.pos;
                    statements.push(self.statement());
                    if self.pos == before {
                        self.diagnose(before, before + 1, "cannot recover statement");
                        self.pos += 1;
                    }
                }
                if !self.eat("}") && self.pos < self.limit {
                    self.expect("%>");
                }
                self.variables = old_variables;
                self.variable_types = old_variable_types;
                self.variable_closures = old_variable_closures;
                StmtKind::Block(statements)
            }
            ";" => {
                self.pos += 1;
                StmtKind::Empty
            }
            ")" | "]" => {
                self.pos = self.stray_closer_problem_end(self.pos);
                StmtKind::Problem
            }
            "else" => {
                self.pos += 1;
                self.pos = self.problem_statement_end(self.pos);
                StmtKind::Problem
            }
            "[" if self.cpp && self.text(start + 1) == "[" => {
                while let Some(after) = self.skip_attribute(self.pos, self.limit) {
                    self.pos = after;
                }
                return self.statement();
            }
            "if" => {
                self.pos += 1;
                self.eat("constexpr");
                if !self.at("(") {
                    // CDT recovers a missing if-condition across the next
                    // complete statement, without turning it into a condition.
                    self.pos = self.problem_statement_end(self.pos);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                if let Some(boundary) = self.unclosed_condition_boundary(self.pos) {
                    self.pos = (self.pos..boundary)
                        .find(|&i| self.text(i) == "{")
                        .map_or(boundary, |brace| self.problem_declaration_end(brace + 1));
                    self.asm_problem_recovery = true;
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let end = self.matching(self.pos, self.limit).unwrap_or(self.limit);
                if let Some(brace) = self.operator_brace(self.pos + 1, end) {
                    // IF consumes the mismatched opening brace before its
                    // problem statement stops at the closing brace. That
                    // brace consequently closes the surrounding compound.
                    self.pos = self.matching(brace, end).unwrap_or(brace);
                    self.asm_problem_recovery = true;
                    return self.problem_statement(start, false);
                }
                let condition = self.condition();
                if matches!(self.peek(), ")" | "]") {
                    let from_macro = self.macro_problem_origin(start, self.pos);
                    self.pos = self.problem_statement_end(self.pos);
                    return self.problem_statement(start, from_macro);
                }
                if matches!(self.peek(), "}" | "%>") {
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let unclosed_consequence =
                    self.at("{") && self.matching(self.pos, self.limit).is_none();
                let consequence = Box::new(self.statement());
                if unclosed_consequence && self.pos == self.limit {
                    return *consequence;
                }
                if recovered_statement_siblings(&consequence) {
                    if self.eat("else") {
                        self.pos = self.problem_statement_end(self.pos);
                    }
                    return *consequence;
                }
                let alternative = if self.eat("else") {
                    Some(Box::new(self.statement()))
                } else {
                    None
                };
                if alternative
                    .as_ref()
                    .is_some_and(|s| recovered_statement_siblings(s))
                {
                    return *alternative.unwrap();
                }
                if matches!(consequence.kind, StmtKind::Problem)
                    || alternative
                        .as_ref()
                        .is_some_and(|s| matches!(s.kind, StmtKind::Problem))
                {
                    StmtKind::Problem
                } else {
                    StmtKind::If {
                        condition,
                        consequence,
                        alternative,
                    }
                }
            }
            "while" => {
                self.pos += 1;
                if let Some(boundary) = self.unclosed_condition_boundary(self.pos) {
                    self.pos = (self.pos..boundary)
                        .find(|i| self.text(*i) == ";")
                        .map_or(boundary, |i| i + 1);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let end = self.matching(self.pos, self.limit).unwrap_or(self.limit);
                if self.operator_brace(self.pos + 1, end).is_some() {
                    // WHILE checks its closing parenthesis without consuming
                    // the brace, so recovery crosses the complete loop body.
                    self.pos = self.problem_statement_end(start);
                    return self.problem_statement(start, false);
                }
                let condition = self.condition();
                if matches!(self.peek(), ")" | "]") {
                    let from_macro = self.macro_problem_origin(start, self.pos);
                    self.pos = self.problem_statement_end(self.pos);
                    return self.problem_statement(start, from_macro);
                }
                if matches!(self.peek(), "}" | "%>") {
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let body = Box::new(self.statement());
                if recovered_statement_siblings(&body) {
                    return *body;
                }
                if matches!(body.kind, StmtKind::Problem) {
                    StmtKind::Problem
                } else {
                    StmtKind::While { condition, body }
                }
            }
            "do" => {
                self.pos += 1;
                let body = Box::new(self.statement());
                if recovered_statement_siblings(&body) {
                    return *body;
                }
                if !self.eat("while") {
                    // An incomplete do statement crosses the next complete
                    // statement during CDT problem recovery. Its body is not
                    // recovered as executable siblings.
                    self.pos = self.problem_statement_end(self.pos);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                if self.eat(";") {
                    // A missing do condition consumes its terminator and the
                    // following statement before recovery resumes.
                    self.pos = self.problem_statement_end(self.pos);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let condition = self.condition();
                if !self.eat(";") {
                    self.pos = self.problem_statement_end(self.pos);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                if matches!(body.kind, StmtKind::Problem) {
                    // After a bad scalar body CDT recovers the trailing
                    // while as a new statement, with an empty body.
                    StmtKind::While {
                        body: Box::new(Stmt {
                            kind: StmtKind::Empty,
                            span: body.span.clone(),
                        }),
                        condition,
                    }
                } else {
                    StmtKind::DoWhile { body, condition }
                }
            }
            "for" => return self.for_statement(start),
            "switch" => {
                self.pos += 1;
                if let Some(boundary) = self.unclosed_condition_boundary(self.pos) {
                    let first =
                        (self.pos..boundary).find(|i| matches!(self.text(*i), "{" | "<%" | ";"));
                    self.pos = match first {
                        Some(open) if matches!(self.text(open), "{" | "<%") => self
                            .matching(open, self.limit)
                            .map_or(boundary, |close| close + 1),
                        Some(semicolon) => semicolon + 1,
                        None => boundary,
                    };
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let condition = self.condition();
                if matches!(self.peek(), "}" | "%>") {
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let case_body = matches!(self.peek(), "case" | "default");
                let unclosed_body = self.at("{") && self.matching(self.pos, self.limit).is_none();
                let body = if case_body {
                    let body_start = self.pos;
                    let mut statements = Vec::new();
                    while matches!(self.peek(), "case" | "default") {
                        statements.push(self.statement());
                    }
                    statements.push(self.statement());
                    Box::new(Stmt {
                        kind: StmtKind::Block(statements),
                        span: self.span(body_start, self.pos),
                    })
                } else {
                    Box::new(self.statement())
                };
                if unclosed_body && self.pos == self.limit {
                    return *body;
                }
                if recovered_statement_siblings(&body) {
                    return *body;
                }
                if case_body
                    && let StmtKind::Block(statements) = &body.kind
                    && statements.last().is_some_and(recovered_statement_siblings)
                {
                    return statements.last().unwrap().clone();
                }
                StmtKind::Switch { condition, body }
            }
            "case" => {
                self.pos += 1;
                let expression = self.expression(0);
                self.expect(":");
                StmtKind::Case(Some(expression))
            }
            "default" => {
                self.pos += 1;
                self.expect(":");
                StmtKind::Case(None)
            }
            "goto" => {
                self.pos += 1;
                let name = if self.eat("*") {
                    let operand = self.expression(0);
                    if !matches!(operand.kind, ExprKind::Identifier(_)) {
                        self.diagnose(start, self.pos, "computed-goto operand CFG is unsupported; sanitize decompiled computed gotos");
                    }
                    "*".into()
                } else {
                    let name = self.peek().to_string();
                    if !name.is_empty() {
                        self.pos += 1;
                    }
                    name
                };
                self.expect(";");
                StmtKind::Goto(name)
            }
            "break" => {
                self.pos += 1;
                self.expect(";");
                StmtKind::Break
            }
            "continue" => {
                self.pos += 1;
                self.expect(";");
                StmtKind::Continue
            }
            "return" | "throw" if self.cpp || self.at("return") => {
                let throwing = self.at("throw");
                self.pos += 1;
                let end = self.statement_end(self.pos);
                if self.c_literal_call_crosses_brace(self.pos, end) {
                    // GNU C's argument parser consumes the mismatched brace
                    // before its return-statement failure propagates. Problem
                    // recovery therefore starts at the following statement.
                    self.pos = self.problem_declaration_end(end + 1);
                    return self.problem_statement(start, false);
                }
                if end == self.limit || matches!(self.text(end), "}" | "%>") {
                    // A return without its terminator is a CDT problem
                    // statement; unlike recovered expression prefixes it
                    // does not survive as an executable block sibling.
                    self.pos = end;
                    return self.problem_statement(start, false);
                }
                if self
                    .unclosed_literal_expression_end(self.pos, end, false)
                    .is_some()
                {
                    // A newline-terminated unclosed literal in a return
                    // crosses the next statement's semicolon in CDT.
                    self.pos = end;
                    self.finish_statement(start);
                    return self.problem_statement(start, false);
                }
                if self.unmatched_expression_close(self.pos, end).is_some() {
                    let from_macro = self.macro_problem_origin(start, end);
                    self.pos = end;
                    self.finish_statement(start);
                    return self.problem_statement(start, from_macro);
                }
                if self.c_qualification_problem(self.pos, end)
                    || self.c_attribute_problem(self.pos, end)
                    || self.ordinary_call_problem(self.pos, end)
                    || (self.pos < end && self.call_argument_problem(self.pos, end))
                    || self.detached_identifier_suffix(self.pos, end).is_some()
                    || self.unpaired_expression_colon(self.pos, end).is_some()
                    || self.star_literal_problem(self.pos, end)
                {
                    self.pos = end;
                    self.finish_statement(start);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                let expression = if self.at(";") {
                    None
                } else if self.cpp && self.at("{") {
                    // A C++ return initializer-clause has no return-value
                    // expression for Joern's expression conversion.
                    self.expression(0);
                    None
                } else {
                    Some(self.expression(0))
                };
                self.finish_statement(start);
                if throwing {
                    StmtKind::Throw(expression)
                } else {
                    StmtKind::Return(expression)
                }
            }
            "try" if self.cpp => {
                self.pos += 1;
                let body = Box::new(self.statement());
                let mut catches = Vec::new();
                while self.eat("catch") {
                    if self.at("(") {
                        self.pos = self
                            .matching(self.pos, self.limit)
                            .map_or(self.pos + 1, |n| n + 1);
                    }
                    catches.push(self.statement());
                }
                StmtKind::Try { body, catches }
            }
            "co_await" | "co_yield" | "co_return" if self.cpp => {
                self.pos += 1;
                let operands = if self.at(";") {
                    Vec::new()
                } else {
                    vec![self.expression(0)]
                };
                self.diagnose(start, self.pos, "coroutine control flow is unsupported");
                self.finish_statement(start);
                StmtKind::Unknown(operands)
            }
            "asm" | "__asm" | "__asm__" => {
                self.pos += 1;
                while qualifier(self.peek()) || self.at("goto") {
                    self.pos += 1;
                }
                let braced = self.at("{");
                if self.at("(") || braced {
                    if let Some(close) = self.matching(self.pos, self.limit) {
                        if braced {
                            // CDT rejects Microsoft assembly syntax. Its
                            // problem-statement recovery leaves this brace to
                            // close the current enclosing compound statement.
                            self.asm_problem_recovery = true;
                            self.pos = close;
                            return Stmt {
                                kind: StmtKind::Problem,
                                span: self.span(start, self.pos),
                            };
                        }
                        self.pos = close + 1;
                    } else {
                        self.diagnose(start, self.pos + 1, "unterminated assembly declaration");
                        self.pos += 1;
                    }
                } else {
                    self.diagnose(
                        start,
                        self.pos,
                        "assembly declaration requires an operand block",
                    );
                }
                if braced {
                    self.eat(";");
                } else {
                    self.finish_statement(start);
                }
                StmtKind::Expression(Expr {
                    kind: ExprKind::Assembly(self.raw(start, self.pos)),
                    span: self.span(start, self.pos),
                })
            }
            "" => StmtKind::Empty,
            _ if self.tokens[self.pos].kind == TokenKind::Identifier
                && self.text(self.pos + 1) == ":" =>
            {
                let name = self.peek().to_string();
                self.pos += 2;
                if matches!(self.peek(), "}" | "%>" | "") {
                    // CDT represents a label without a following statement as
                    // a problem statement, which Joern omits from the AST.
                    StmtKind::Problem
                } else {
                    let statement = Box::new(self.statement());
                    if matches!(statement.kind, StmtKind::Problem) {
                        StmtKind::Problem
                    } else if matches!(&statement.kind, StmtKind::Sequence(statements)
                        if statements.first().is_some_and(|s| matches!(s.kind, StmtKind::Problem)))
                    {
                        // A recovered problem child cannot bind this label.
                        // Following recovered statements belong to the outer
                        // compound statement, rather than to the label.
                        return *statement;
                    } else {
                        StmtKind::Label { name, statement }
                    }
                }
            }
            _ => {
                let end = self.statement_end(self.pos);
                if self.looks_declaration(self.pos, end)
                    && (self.pos..end).any(|i| self.text(i) == "=")
                    && self.c_literal_call_crosses_brace(self.pos, end)
                {
                    // A C declaration initializer propagates the same
                    // consumed-brace argument failure as a return expression.
                    self.pos = self.problem_declaration_end(end + 1);
                    return self.problem_statement(start, false);
                }
                if self.cdt_failures
                    && !self.cpp
                    && let Some((open, close)) = self.call_brace(self.pos, end)
                    && self.problem_declaration_end(start) > close
                    && self
                        .unclosed_literal_call(self.pos, end)
                        .is_some_and(|(_, literal)| literal < open)
                {
                    // An unclosed literal ran this call into a later
                    // statement with a brace group. CDT's skip from here
                    // ends with that group; it is no call that the brace
                    // follows. By default the problem runs on as it did.
                    self.pos = self.problem_declaration_end(start);
                    return Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                }
                if let Some(literal_end) = self.unclosed_literal_expression_end(self.pos, end, true)
                {
                    if self.unclosed_conditional_literal(self.pos, literal_end) {
                        // The literal swallowed a required conditional colon
                        // and the terminator. CDT drops this expression and
                        // crosses the next complete statement's semicolon.
                        self.pos = end;
                        self.eat(";");
                        return self.problem_statement(start, false);
                    }
                    let kind = if self.looks_declaration(self.pos, literal_end) {
                        self.remember_types(self.pos, literal_end);
                        StmtKind::Declaration(self.declarations_range(self.pos, literal_end))
                    } else {
                        StmtKind::Expression(self.expression_range(self.pos, literal_end))
                    };
                    self.pos = literal_end;
                    self.eat(";");
                    let span = self.span(start, self.pos);
                    return Stmt {
                        kind: StmtKind::Sequence(vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: span.clone(),
                            },
                            Stmt {
                                kind,
                                span: span.clone(),
                            },
                        ]),
                        span,
                    };
                }
                if let Some(statement) = self.terminal_expression_statement(start, end) {
                    return statement;
                }
                if let Some(after) = self.call_statement_separator(self.pos) {
                    let close = after - 1;
                    let grouped_name = self.declarator_name(self.pos + 2, close);
                    let grouped_declaration = grouped_name.0.is_some_and(|name| {
                        grouped_name.1 == close
                            && !expression_type_keyword(self.text(name), self.cpp)
                            && !qualifier(self.text(name))
                    });
                    let kind = if grouped_declaration {
                        StmtKind::Declaration(self.declarations_range(self.pos, after))
                    } else if self.c_qualification_problem(self.pos, after)
                        || self.ordinary_call_problem(self.pos, after)
                    {
                        // A malformed first expression still recovers through
                        // the shared semicolon; it cannot invent a separator.
                        let end = self.statement_end(self.pos);
                        self.pos = end;
                        if matches!(self.peek(), "}" | "%>") {
                            // CDT's problem-statement recovery stops before
                            // the compound brace even without a terminator.
                            return self.problem_statement(start, false);
                        }
                        self.finish_statement(start);
                        return self.problem_statement(start, false);
                    } else {
                        StmtKind::Expression(self.expression_range(self.pos, after))
                    };
                    self.pos = after;
                    let span = self.span(start, self.pos);
                    return Stmt {
                        kind: StmtKind::Sequence(vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: span.clone(),
                            },
                            Stmt {
                                kind,
                                span: span.clone(),
                            },
                        ]),
                        span,
                    };
                }
                let end = self
                    .direct_call_brace(self.pos, self.limit)
                    .unwrap_or_else(|| self.statement_end(self.pos));
                if self.looks_declaration(self.pos, end)
                    && (self.pos..end).any(|i| {
                        self.text(i) == "="
                            && (i + 1 == end || self.text(i + 1) == ",")
                            && self
                                .tokens
                                .get(i + 1)
                                .is_some_and(|token| token.macro_expansion)
                    })
                {
                    return self.reparse_macro_declaration(start, end);
                }
                if !self.cpp
                    && self.text(start + 1) == "("
                    && self.cdt_identifier(start, end)
                    && !self.explicit_types.contains(self.text(start))
                    && !self.variables.contains(self.text(start))
                    && let Some((after, _)) = self.cdt_declarator(start + 1, end, false)
                    && !(start + 2..after).any(|i| cdt_c_keyword(self.text(i)))
                    && (after == end
                        || (self.text(after) == "="
                            && self.text(after - 1) == "]"
                            && !(start + 2..after)
                                .any(|i| self.text(i) == "[" && self.text(i + 1) == "]")
                            && after + 1 < end
                            && self.text(after + 1) != "{"))
                {
                    // `T (declarator);` also reads as a call, and so does
                    // `T (declarator)[n] = v;`, whose left side is no call.
                    // Resolving that reading binds an undeclared `T` as an
                    // external function.
                    self.external_functions.insert(self.text(start).to_string());
                }
                if self.star_literal_problem(self.pos, end) {
                    // Neither an expression nor a declaration parses.
                    self.pos = end;
                    self.finish_statement(start);
                    let problem = Stmt {
                        kind: StmtKind::Problem,
                        span: self.span(start, self.pos),
                    };
                    if !self.cpp
                        && self.cdt_identifier(start, end)
                        && self.cdt_identifier(start + 1, end)
                    {
                        // The expression `T` of `T name = ...` does parse.
                        // It lacks its semicolon, and the rest is the problem.
                        let expression = self.expression_range(start, start + 1);
                        let span = expression.span.clone();
                        let kind = StmtKind::Expression(expression);
                        return self.unterminated(start, kind, span, problem, true);
                    }
                    return problem;
                }
                if let Some(statement) = self.cdt_literal_recovery(start, end) {
                    return statement;
                }
                let colon = self.unpaired_expression_colon(self.pos, end);
                // CDT can recover a complete expression followed by a
                // detached identifier as two block siblings. The
                // same suffix inside a return or call argument is a problem
                // expression, rather than a second statement.
                let suffix = self
                    .detached_identifier_suffix(self.pos, end)
                    .filter(|_| !self.looks_declaration(self.pos, end));
                let mut attributes_end = self.pos;
                if matches!(self.peek(), "__attribute__" | "__attribute") {
                    while let Some(after) = self.skip_attribute(attributes_end, end) {
                        attributes_end = after;
                    }
                }
                if attributes_end > self.pos && attributes_end == end {
                    self.pos = end;
                    self.finish_statement(start);
                    if self.cpp {
                        StmtKind::Empty
                    } else {
                        StmtKind::Problem
                    }
                } else if self.c_qualification_problem(
                    self.pos,
                    colon
                        .or(suffix)
                        .or_else(|| self.detached_identifier_suffix(self.pos, end))
                        .or_else(|| self.stray_postfix_operator(self.pos, end))
                        .unwrap_or(end),
                ) || self.c_attribute_problem(self.pos, end)
                {
                    self.pos = end;
                    if matches!(self.peek(), "}" | "%>") {
                        // This grammar problem was established before parsing
                        // an executable expression. The brace closes its
                        // compound rather than requiring another semicolon.
                        return self.problem_statement(start, false);
                    }
                    self.finish_statement(start);
                    StmtKind::Problem
                } else if self.looks_declaration(self.pos, end)
                    || (attributes_end > self.pos && {
                        let base_end = self.specifier_end(self.pos, end);
                        let (name, suffix) = self.declarator_name(base_end, end);
                        self.text(base_end) == "("
                            && suffix == end
                            && name
                                .is_some_and(|i| !expression_type_keyword(self.text(i), self.cpp))
                    })
                {
                    if !self.cpp
                        && let Some(statement) = self.cdt_declaration_recovery(start, end)
                    {
                        return statement;
                    }
                    if self.declaration_shape_problem(self.pos, end)
                        || self.declaration_expression_problem(self.pos, end)
                    {
                        self.pos = end;
                        self.finish_statement(start);
                        return Stmt {
                            kind: StmtKind::Problem,
                            span: self.span(start, self.pos),
                        };
                    }
                    self.remember_types(self.pos, end);
                    let alias = (self.pos..end).any(|i| self.text(i) == "using");
                    let mut declarations = if alias {
                        Vec::new()
                    } else {
                        self.declarations_range(self.pos, end)
                    };
                    let mut recovered_declarator = false;
                    let mut suffixes = Vec::new();
                    for (a, b) in self.split_ranges(self.specifier_end(start, end), end, ",") {
                        let (_, mut after) = self.declarator_name(a, b);
                        while let Some(next) = self.skip_attribute(after, b) {
                            after = next;
                        }
                        if self.text(after) == "=" {
                            let suffix = self.detached_identifier_suffix(after + 1, b);
                            let problem_end =
                                suffix.or_else(|| self.stray_postfix_operator(after + 1, b));
                            if let Some(end) = problem_end {
                                let start_offset = self.span(a, b).start;
                                if let Some(declaration) = declarations
                                    .iter_mut()
                                    .find(|d| d.span.start == start_offset)
                                {
                                    declaration.span = self.span(a, end);
                                }
                                recovered_declarator = true;
                            }
                            if let Some(suffix) = suffix {
                                suffixes.push(self.recovered_suffix_statement(suffix, b));
                            }
                        } else {
                            recovered_declarator |=
                                after < b && !matches!(self.text(after), "{" | "(" | "[");
                        }
                    }
                    self.pos = end;
                    self.expect(";");
                    if alias {
                        StmtKind::Empty
                    } else if recovered_declarator {
                        // CDT retains the parsed declaration prefix as a
                        // sibling after dropping an invalid scalar parent.
                        let span = self.span(start, self.pos);
                        let mut statements = vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: span.clone(),
                            },
                            Stmt {
                                kind: StmtKind::Declaration(declarations),
                                span,
                            },
                        ];
                        statements.extend(suffixes);
                        StmtKind::Sequence(statements)
                    } else {
                        StmtKind::Declaration(declarations)
                    }
                } else if self.ordinary_call_problem(self.pos, end) {
                    if let Some((_, close)) = self.call_brace(self.pos, end) {
                        self.pos = close;
                        self.asm_problem_recovery = true;
                        return Stmt {
                            kind: StmtKind::Problem,
                            span: self.span(start, self.pos),
                        };
                    }
                    self.pos = end;
                    if matches!(self.peek(), "}" | "%>")
                        && (start..end).any(|i| {
                            self.tokens[i].kind == TokenKind::Literal
                                && unclosed_quoted_literal(self.text(i))
                        })
                    {
                        // An unclosed literal can consume the call's closing
                        // delimiter and terminator. CDT ends this problem
                        // statement at the compound boundary, leaving the
                        // brace to close its surrounding statement block.
                        return self.problem_statement(start, false);
                    }
                    self.finish_statement(start);
                    StmtKind::Problem
                } else if let Some(colon) = colon {
                    let expression = self.expression_range(self.pos, colon);
                    self.pos = end;
                    self.finish_statement(start);
                    StmtKind::Expression(expression)
                } else if let Some(suffix) = self.stray_postfix_operator(self.pos, end) {
                    let expression = self.expression_range(self.pos, suffix);
                    self.pos = end;
                    self.finish_statement(start);
                    StmtKind::Sequence(vec![
                        Stmt {
                            kind: StmtKind::Problem,
                            span: expression.span.clone(),
                        },
                        Stmt {
                            span: expression.span.clone(),
                            kind: StmtKind::Expression(expression),
                        },
                    ])
                } else if let Some(suffix) = suffix {
                    let first = self.expression_range(self.pos, suffix);
                    let second = self.recovered_suffix_statement(suffix, end);
                    self.pos = end;
                    self.finish_statement(start);
                    StmtKind::Sequence(vec![
                        Stmt {
                            span: first.span.clone(),
                            kind: StmtKind::Problem,
                        },
                        Stmt {
                            span: first.span.clone(),
                            kind: StmtKind::Expression(first),
                        },
                        second,
                    ])
                } else {
                    let expression = self.expression(0);
                    if matches!(self.peek(), ")" | "]")
                        && (self.pos..end).all(|i| matches!(self.text(i), ")" | "]"))
                    {
                        self.pos = end;
                        self.finish_statement(start);
                        StmtKind::Sequence(vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: expression.span.clone(),
                            },
                            Stmt {
                                span: expression.span.clone(),
                                kind: StmtKind::Expression(expression),
                            },
                        ])
                    } else if self.at("{") && matches!(expression.kind, ExprKind::Call { .. }) {
                        let call = Stmt {
                            span: expression.span.clone(),
                            kind: StmtKind::Expression(expression),
                        };
                        let block = self.statement();
                        StmtKind::Sequence(vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: call.span.clone(),
                            },
                            call,
                            block,
                        ])
                    } else if self
                        .tokens
                        .get(self.pos)
                        .is_some_and(|t| t.kind == TokenKind::Identifier)
                        && matches!(expression.kind, ExprKind::Member { .. })
                    {
                        // With no separator, CDT recovers the member operand
                        // and following statement as compound siblings.
                        StmtKind::Sequence(vec![
                            Stmt {
                                kind: StmtKind::Problem,
                                span: expression.span.clone(),
                            },
                            Stmt {
                                span: expression.span.clone(),
                                kind: StmtKind::Expression(expression),
                            },
                        ])
                    } else if self
                        .tokens
                        .get(self.pos)
                        .is_some_and(|t| t.kind == TokenKind::Identifier)
                        && self.text(self.pos + 1) == "("
                        && matches!(&expression.kind, ExprKind::Binary { op, .. } if op == "=")
                    {
                        StmtKind::Expression(expression)
                    } else if self.pos + 2 == end
                        && self.tokens[self.pos].kind == TokenKind::Identifier
                        && self.tokens[self.pos + 1].kind == TokenKind::Identifier
                        && matches!(&expression.kind, ExprKind::Binary { op, .. } if op == "=")
                        && self.looks_declaration(self.pos, end)
                    {
                        // The completed assignment and a following two-name
                        // declaration are recovered as compound siblings.
                        // The declaration is parsed on the next iteration.
                        StmtKind::Expression(expression)
                    } else if self
                        .tokens
                        .get(self.pos)
                        .is_some_and(|t| t.kind == TokenKind::Identifier)
                        && self.text(self.pos + 1) == "("
                        && self.text(start + 1) == "("
                        && self.pos == start + 4
                        && self.tokens[start + 2].kind == TokenKind::Identifier
                    {
                        StmtKind::Declaration(self.declarations_range(start, self.pos))
                    } else if !self.at(";") && !matches!(self.peek(), "}" | "") {
                        let mut expressions = vec![expression];
                        self.diagnose(start, end, "unsupported expression statement syntax");
                        while self.pos < end {
                            let before = self.pos;
                            expressions.push(self.expression(0));
                            if self.pos <= before {
                                self.pos += 1;
                            }
                        }
                        self.finish_statement(start);
                        StmtKind::Unknown(expressions)
                    } else {
                        self.finish_statement(start);
                        StmtKind::Expression(expression)
                    }
                }
            }
        };
        Stmt {
            kind,
            span: self.span(start, self.pos),
        }
    }

    pub(super) fn statement_end(&self, start: usize) -> usize {
        let mut i = start;
        while i < self.limit {
            if matches!(self.text(i), ";" | "}") {
                break;
            }
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, self.limit).map_or(i + 1, |n| n + 1);
            } else {
                i += 1;
            }
        }
        i
    }

    pub(super) fn c_literal_call_crosses_brace(&self, start: usize, end: usize) -> bool {
        if self.cpp || !matches!(self.text(end), "}" | "%>") {
            return false;
        }
        let Some((open, mut literal)) = self.unclosed_literal_call(start, end) else {
            return false;
        };
        while literal > open + 1
            && string_literal_token(self.text(literal))
            && self.tokens[literal - 1].kind == TokenKind::Literal
            && string_literal_token(self.text(literal - 1))
        {
            literal -= 1;
        }
        // Invalid tokens before the unclosed literal fail the argument parse
        // before CDT reaches the compound brace. Only a parsed argument can
        // trigger its consuming-comma mismatch at that brace.
        matches!(self.text(literal - 1), "(" | ",")
    }

    pub(super) fn finish_statement(&mut self, start: usize) {
        if !self.eat(";") {
            self.diagnose(start, self.pos, "missing semicolon after statement");
            while self.pos < self.limit && !matches!(self.peek(), ";" | "}") {
                self.pos += 1;
            }
            self.eat(";");
        }
    }

    pub(super) fn condition(&mut self) -> Expr {
        self.expect("(");
        let start = self.pos;
        let end = self
            .matching(start.saturating_sub(1), self.limit)
            .unwrap_or(self.limit);
        if !self.cpp
            && let Some((_, close)) = self.call_brace(start, end)
        {
            self.pos = close;
            self.asm_problem_recovery = true;
            return Expr {
                kind: ExprKind::Problem(String::new()),
                span: self.span(start, close),
            };
        }
        let ranges = self.split_ranges(start, end, ";");
        let declaration_end = ranges[0].1;
        let recovered_prototype = self.cpp
            && self.ordinary_call_problem(start, declaration_end)
            && self
                .function_header(start, declaration_end)
                .is_some_and(|header| {
                    let base_end = self.specifier_end(start, declaration_end);
                    base_end < header.name_start
                        && (base_end..header.name_start).all(|i| {
                            matches!(self.text(i), "*" | "&" | "&&") || qualifier(self.text(i))
                        })
                });
        let condition_declaration = self.cpp
            && (recovered_prototype
                || (self.looks_declaration(start, declaration_end) && {
                    let base_end = self.specifier_end(start, declaration_end);
                    let (_, after_name) = self.declarator_name(base_end, declaration_end);
                    matches!(self.text(after_name), "=" | "{")
                }));
        let expression = if start == end {
            Expr {
                kind: ExprKind::Problem(String::new()),
                span: self.span(start, end),
            }
        } else if condition_declaration {
            let first = self.declarations_range(start, ranges[0].1);
            let declaration = Stmt {
                kind: StmtKind::Declaration(first),
                span: self.span(start, ranges[0].1),
            };
            if ranges.len() == 1 {
                Expr {
                    kind: ExprKind::Statement(Box::new(declaration)),
                    span: self.span(start, end),
                }
            } else {
                let value = self.expression_range(ranges[1].0, end);
                Expr {
                    kind: ExprKind::Block(vec![
                        Expr {
                            span: declaration.span.clone(),
                            kind: ExprKind::Statement(Box::new(declaration)),
                        },
                        value,
                    ]),
                    span: self.span(start, end),
                }
            }
        } else if self.call_argument_problem(start, end) {
            Expr {
                kind: ExprKind::Problem(self.raw(start, end)),
                span: self.span(start, end),
            }
        } else {
            self.expression_range(start, end)
        };
        self.pos = end;
        self.expect(")");
        expression
    }

    pub(super) fn for_statement(&mut self, start: usize) -> Stmt {
        self.pos += 1;
        self.expect("(");
        let inside = self.pos;
        let end = self
            .matching(inside.saturating_sub(1), self.limit)
            .unwrap_or(self.limit);
        let clauses = self.split_ranges(inside, end, ";");
        if self.cpp
            && let [range] | [_, range] = clauses.as_slice()
        {
            // In C++20 the range declaration may follow an init-statement.
            // The delimiter is outside any conditional expression.
            let (range_start, range_end) = *range;
            if let Some(separator) = self.unpaired_expression_colon(range_start, range_end)
                && self.unpaired_expression_colon(separator + 1, end).is_none()
            {
                let saved_scope = (clauses.len() == 2).then(|| {
                    (
                        self.variables.clone(),
                        self.variable_types.clone(),
                        self.variable_closures.clone(),
                    )
                });
                let initializer = if let [(a, b), _] = clauses.as_slice() {
                    self.for_initializer(*a, *b)
                } else {
                    None
                };
                // The range expression can see the initializer's variables.
                // The iteration variable only enters scope for the body.
                let iterable = self.expression_range(separator + 1, end);
                let mut declarations = self.declarations_range(range_start, separator);
                let declaration = if declarations.is_empty() {
                    self.diagnose(
                        range_start,
                        separator,
                        "cannot recover range-for declaration",
                    );
                    Declaration {
                        name: "<unknown>".into(),
                        type_name: "ANY".into(),
                        initializer: None,
                        dimensions: Vec::new(),
                        problem: false,
                        is_typedef: false,
                        nested_declarator: false,
                        span: self.span(range_start, separator),
                    }
                } else {
                    declarations.remove(0)
                };
                self.pos = end;
                self.expect(")");
                let body = Box::new(self.statement());
                if let Some((variables, types, closures)) = saved_scope {
                    self.variables = variables;
                    self.variable_types = types;
                    self.variable_closures = closures;
                }
                return Stmt {
                    kind: StmtKind::RangeFor {
                        initializer,
                        declaration,
                        iterable,
                        body,
                    },
                    span: self.span(start, self.pos),
                };
            }
        }
        if clauses.len() != 3 {
            self.pos = self.problem_statement_end(end.saturating_add(1));
            return Stmt {
                kind: StmtKind::Problem,
                span: self.span(start, self.pos),
            };
        }
        if let Some(problem) = clauses.iter().enumerate().rposition(|(index, &(a, b))| {
            if a == b {
                return false;
            }
            let declaration = index == 0 && self.looks_declaration(a, b);
            self.c_qualification_problem(a, b)
                || self.ordinary_call_problem(a, b)
                || self.unpaired_expression_colon(a, b).is_some()
                || self.star_literal_problem(a, b)
                || if declaration {
                    let base = self.specifier_end(a, b);
                    let (_, after) = self.declarator_name(base, b);
                    self.declaration_shape_problem(a, b)
                        || self.declaration_expression_problem(a, b)
                        || (after < b
                            && !matches!(self.text(after), "=" | "{" | "(" | "[" | ",")
                            && self.skip_attribute(after, b).is_none())
                } else {
                    self.call_argument_problem(a, b)
                }
        }) {
            // CDT's expression-error recovery crosses the invalid for header
            // up to its last problem clause. Later clauses survive as plain
            // expression statements; the for control structure and body do
            // not. Retain that recovery rather than inventing loop edges.
            let mut statements = vec![Stmt {
                kind: StmtKind::Problem,
                span: self.span(start, clauses[problem].1),
            }];
            for &(a, b) in &clauses[problem + 1..] {
                if a < b {
                    statements.push(Stmt {
                        kind: StmtKind::Expression(self.expression_range(a, b)),
                        span: self.span(a, b),
                    });
                }
            }
            self.pos = self.problem_statement_end(end.saturating_add(1));
            return Stmt {
                kind: StmtKind::Sequence(statements),
                span: self.span(start, self.pos),
            };
        }
        let initializer = self.for_initializer(clauses[0].0, clauses[0].1);
        let condition = clauses
            .get(1)
            .filter(|(a, b)| a < b)
            .map(|(a, b)| self.expression_range(*a, *b));
        let update = clauses
            .get(2)
            .filter(|(a, b)| a < b)
            .map(|(a, b)| self.expression_range(*a, *b));
        self.pos = end;
        self.expect(")");
        let body = Box::new(self.statement());
        if recovered_statement_siblings(&body) {
            return *body;
        }
        Stmt {
            kind: StmtKind::For {
                initializer,
                condition,
                update,
                body,
            },
            span: self.span(start, self.pos),
        }
    }

    pub(super) fn direct_call_brace(&self, start: usize, end: usize) -> Option<usize> {
        if self.tokens.get(start)?.kind != TokenKind::Identifier || self.text(start + 1) != "(" {
            return None;
        }
        let close = self.matching(start + 1, end)?;
        (self.text(close + 1) == "{").then_some(close + 1)
    }

    pub(super) fn call_statement_separator(&self, start: usize) -> Option<usize> {
        if self.tokens.get(start)?.kind != TokenKind::Identifier
            || self.text(start + 1) != "("
            || qualifier(self.text(start))
            || matches!(
                self.text(start),
                "sizeof"
                    | "alignof"
                    | "_Alignof"
                    | "__alignof__"
                    | "__alignof"
                    | "typeof"
                    | "__typeof__"
                    | "__typeof"
                    | "decltype"
                    | "noexcept"
            )
            || self.skip_attribute(start, self.limit).is_some()
        {
            return None;
        }
        let next = self.matching(start + 1, self.limit)? + 1;
        let text = self.text(next);
        let keyword = matches!(
            text,
            "if" | "while"
                | "for"
                | "switch"
                | "do"
                | "return"
                | "throw"
                | "break"
                | "continue"
                | "goto"
                | "try"
                | "case"
                | "default"
        );
        let ordinary = self
            .tokens
            .get(next)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                self.text(next + 1),
                "(" | "."
                    | "->"
                    | "["
                    | "="
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "&="
                    | "|="
                    | "^="
                    | ">>="
                    | "<<="
                    | "++"
                    | "--"
            );
        (keyword
            || ordinary
            || (self
                .tokens
                .get(next)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && self.text(next + 1) == ":")
            || self.type_word(text)
            || qualifier(text)
            || (self
                .tokens
                .get(next)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && self.looks_declaration(next, self.statement_end(next)))
            || matches!(text, "++" | "--"))
        .then_some(next)
    }

    pub(super) fn terminal_expression_statement(
        &mut self,
        start: usize,
        end: usize,
    ) -> Option<Stmt> {
        let eof = end == self.limit;
        if !eof && !matches!(self.text(end), "}" | "%>") {
            return None;
        }
        if self.direct_call_brace(start, end).is_some() {
            // This is a call followed by a compound sibling, whose missing
            // separator is recovered before the compound's own boundary.
            return None;
        }
        if self
            .call_statement_separator(start)
            .is_some_and(|next| next < end)
        {
            // A call followed directly by another statement is recovered as
            // two siblings even when no later terminator precedes the brace.
            return None;
        }
        let direct_call = self.tokens.get(start)?.kind == TokenKind::Identifier
            && !self.type_word(self.text(start))
            && !qualifier(self.text(start))
            && self.text(start + 1) == "("
            && self.matching(start + 1, end) == end.checked_sub(1);
        let declaration = !eof && self.looks_declaration(start, end);
        if !direct_call
            && !declaration
            && (eof
                || !(start..end).any(|i| {
                    matches!(
                        self.text(i),
                        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
                    )
                }) && !matches!(self.text(end.saturating_sub(1)), "++" | "--"))
        {
            return None;
        }
        self.pos = end;
        let span = self.span(start, end);
        if eof
            || self.c_qualification_problem(start, end)
            || self.ordinary_call_problem(start, end)
            || declaration
                && (self.declaration_shape_problem(start, end)
                    || self.declaration_expression_problem(start, end))
        {
            // A completed call at literal EOF is omitted. Before a closing
            // brace a malformed expression is still a whole problem node.
            return Some(Stmt {
                kind: StmtKind::Problem,
                span,
            });
        }
        let grouped_name = direct_call.then(|| self.declarator_name(start + 2, end - 1));
        let kind = if declaration
            || grouped_name.is_some_and(|(name, after)| {
                after == end - 1
                    && name.is_some_and(|i| {
                        !expression_type_keyword(self.text(i), self.cpp) && !qualifier(self.text(i))
                    })
            }) {
            self.remember_types(start, end);
            StmtKind::Declaration(self.declarations_range(start, end))
        } else {
            StmtKind::Expression(self.expression_range(start, end))
        };
        Some(Stmt {
            kind: StmtKind::Sequence(vec![
                Stmt {
                    kind: StmtKind::Problem,
                    span: span.clone(),
                },
                Stmt {
                    kind,
                    span: span.clone(),
                },
            ]),
            span,
        })
    }

    pub(super) fn call_brace(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        let mut open = start;
        while open < end {
            if let Some(after) = self.lambda_end(open, end) {
                open = after;
                continue;
            }
            if self.text(open) == "{"
                && open > start
                && self.text(open - 1) == "("
                && let Some(close) = self.matching(open, end)
            {
                open = close + 1;
                continue;
            }
            if self.text(open) == "("
                && open > start
                && self.tokens[open - 1].kind == TokenKind::Identifier
                && !matches!(
                    self.text(open - 1),
                    "sizeof" | "alignof" | "typeof" | "__typeof__" | "__typeof" | "__extension__"
                )
                && let Some(close) = self.matching(open, end)
                && self.text(close + 1) == "{"
                && let Some(brace_close) = self.matching(close + 1, end)
            {
                return Some((open, brace_close));
            }
            open += 1;
        }
        None
    }

    pub(super) fn operator_brace(&self, start: usize, end: usize) -> Option<usize> {
        let mut i = start;
        while i < end {
            if self.text(i) == "("
                && self.text(i + 1) == "{"
                && let Some(close) = self.matching(i, end)
            {
                // GNU compounds have their own statement recovery boundary.
                i = close + 1;
                continue;
            }
            if self.text(i) == "{" {
                if i > start
                    && binary_precedence(self.text(i - 1)).is_some_and(|(precedence, _)| {
                        precedence > 0 && (!self.cpp || precedence != 1)
                    })
                {
                    return Some(i);
                }
                // Declaration initializers and typed compound literals keep
                // their complete initializer, including nested brace lists.
                i = self.matching(i, end)? + 1;
                continue;
            }
            i += 1;
        }
        None
    }
}
