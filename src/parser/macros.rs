//! Macro expansion, builtins, attributes and inline assembly.

use super::*;

impl Parser<'_> {
    pub(super) fn record_retained_macro_call(&mut self, callee: &Expr, _arguments: &[Expr]) {
        if !self.retained_macro_invocations.is_empty()
            && matches!(callee.kind, ExprKind::Member { .. })
        {
            self.retained_macro_call_ends.insert(callee.span.end);
        }
    }

    /// Recovers the rest of a body that CDT closed early, as the file-scope
    /// declarations it is to CDT, and returns the object names they declare.
    pub(super) fn asm_tail_declarations(&mut self, start: usize, end: usize) -> Vec<usize> {
        let mut objects = Vec::new();
        if self.cpp {
            return objects;
        }
        let saved_limit = self.limit;
        self.limit = end;
        self.pos = start;
        let mut recovered_variables = HashSet::new();
        while self.pos < end {
            let start = self.pos;
            if self.tokens[start].kind == TokenKind::Identifier && self.text(start + 1) == "=" {
                // Assignment-shaped orphan declarations create global C
                // variable bindings, unlike locals from the truncated body.
                recovered_variables.insert(self.text(start).to_string());
            }
            if matches!(self.text(start), "asm" | "__asm" | "__asm__")
                && self.text(start + 1) == "{"
                && let Some(close) = self.matching(start + 1, end)
            {
                // A problem assembly declaration ends at its own brace, not
                // the semicolon of the next recovered declaration.
                self.pos = close + 1;
                self.eat(";");
                continue;
            }
            if self.tokens[start].kind == TokenKind::Identifier
                && self.text(start + 1) == "("
                && !recovered_variables.contains(self.text(start))
                && !self.type_word(self.text(start))
                && !qualifier(self.text(start))
                && !matches!(
                    self.text(start),
                    "if" | "while"
                        | "for"
                        | "switch"
                        | "sizeof"
                        | "_Alignof"
                        | "alignof"
                        | "__alignof__"
                        | "return"
                        | "asm"
                        | "__asm"
                        | "__asm__"
                )
                && let Some(close) = self.matching(start + 1, end)
                && matches!(self.text(close + 1), ";" | "=")
            {
                let declaration_end = self.declaration_end(close + 1);
                if self.text(declaration_end) != ";" {
                    break;
                }
                let ranges = self.split_ranges(start + 2, close, ",");
                if ranges.iter().all(|&(a, b)| {
                    a == b
                        || b == a + 1
                            && self.tokens[a].kind == TokenKind::Identifier
                            && !self.type_word(self.text(a))
                            && !qualifier(self.text(a))
                }) {
                    // These are recovered K&R parameter names, not named type
                    // specifiers. Sending them through parameters() would leak
                    // each argument spelling into the translation-unit types.
                    let parameters: Vec<_> = ranges
                        .into_iter()
                        .filter(|(a, b)| a < b)
                        .map(|(a, b)| Parameter {
                            name: self.raw(a, b),
                            type_name: "ANY".into(),
                            function_pointer: false,
                            knr: None,
                            span: self.span(a, b),
                        })
                        .collect();
                    let name = self.text(start).to_string();
                    Rc::make_mut(&mut self.variables).insert(name.clone());
                    self.function_declarations.push(Function {
                        full_name: name.clone(),
                        name,
                        return_type: "ANY".into(),
                        binding_return_type: "ANY".into(),
                        signature: format!("ANY({})", vec!["ANY"; parameters.len()].join(",")),
                        extern_c: false,
                        implicit_this: None,
                        lambda_parent: None,
                        implicit_fields: Vec::new(),
                        member_cv_qualified: false,
                        lambda: false,
                        is_static: false,
                        is_virtual: false,
                        inherited_bindings: Vec::new(),
                        inherited_closures: Vec::new(),
                        parameters,
                        constructor_initializers: Vec::new(),
                        body: Stmt {
                            kind: StmtKind::Empty,
                            span: self.span(declaration_end, declaration_end),
                        },
                        span: self.span(start, declaration_end + 1),
                    });
                    self.pos = declaration_end + 1;
                    continue;
                }
            }
            if self.cdt_failures
                && let Some((semicolon, names)) = self.cdt_object_declaration(start, end)
            {
                // By default the names stay local to the body they left.
                objects.extend(names);
                self.pos = semicolon + 1;
                continue;
            }
            self.pos = self.problem_declaration_end(start).max(start + 1);
        }
        self.limit = saved_limit;
        objects
    }

    pub(super) fn skip_attribute(&self, i: usize, end: usize) -> Option<usize> {
        if matches!(
            self.text(i),
            "__attribute__" | "__attribute" | "__declspec" | "alignas" | "_Alignas"
        ) {
            return Some(if self.text(i + 1) == "(" {
                self.matching(i + 1, end).map_or(i + 1, |n| n + 1)
            } else {
                i + 1
            });
        }
        if self.text(i) == "[" && self.text(i + 1) == "[" {
            return self.matching(i, end).map(|n| n + 1);
        }
        None
    }

    pub(super) fn reparse_macro_declaration(&mut self, start: usize, end: usize) -> Stmt {
        // Joern reparses macro-origin problem statements as a new translation
        // unit without the surrounding macro definitions. An erased initializer
        // can therefore recover its original, now ordinary, function call.
        let after = end + usize::from(self.text(end) == ";");
        let original = self.span(start, after);
        let (mut tokens, _) = lex_preprocessed(&self.raw(start, after), true);
        for token in &mut tokens {
            let first_line = token.span.line == 1;
            token.span.start += original.start;
            token.span.end += original.start;
            token.span.line += original.line - 1;
            token.span.end_line += original.line - 1;
            if first_line {
                token.span.column += original.column - 1;
            }
        }
        let old_limit = self.limit;
        self.limit = tokens.len();
        let previous_tokens = std::mem::replace(&mut self.tokens, tokens);
        self.pos = 0;
        let statement = self.statement();
        self.tokens = previous_tokens;
        self.limit = old_limit;
        self.pos = after;
        statement
    }

    pub(super) fn builtin_macro_call(&mut self, name: String, start: usize) -> Option<Expr> {
        let close = self.matching(self.pos, self.limit)?;
        let ranges = self.split_ranges(self.pos + 1, close, ",");
        let arity = if name == "__offsetof__" { 1 } else { 2 };
        if ranges.len() != arity {
            return None;
        }
        let arguments: Vec<_> = ranges.iter().map(|&(a, b)| self.raw(a, b)).collect();
        let expansion = match name.as_str() {
            // CDT's type-id signature renders typeof's operand as 'typeof'.
            // The unparenthesized first argument preserves replacement-list
            // precedence, including a conditional or binary argument.
            "__builtin_va_arg" => format!("*(typeof *){}", arguments[0]),
            "__builtin_offsetof" if self.cpp => format!(
                "reinterpret_cast<size_t>(&reinterpret_cast<const volatile char &>(static_cast<{}*>(0)->{}))",
                arguments[0], arguments[1]
            ),
            "__builtin_offsetof" => format!("((size_t)&(({} *)0)->{})", arguments[0], arguments[1]),
            "__builtin_types_compatible_p" => format!(
                "__builtin_types_compatible_p(sizeof({}),sizeof({}))",
                arguments[0], arguments[1]
            ),
            "__offsetof__" => format!("({})", arguments[0]),
            _ => return None,
        };
        self.pos = close + 1;
        let span = self.span(start, self.pos);
        Some(self.expanded_macro(name, arity, &ranges, &expansion, span))
    }

    pub(super) fn expanded_macro(
        &mut self,
        name: String,
        arity: usize,
        arguments: &[(usize, usize)],
        replacement: &str,
        span: Span,
    ) -> Expr {
        let (tokens, diagnostics) = lex_preprocessed(replacement, true);
        let limit = tokens.len();
        let mut parser = Parser {
            source: replacement,
            tokens,
            pos: 0,
            limit,
            cpp: self.cpp,
            cdt_failures: self.cdt_failures,
            diagnostics,
            types: self.types.clone(),
            explicit_types: self.explicit_types.clone(),
            supplied_type_names: self.supplied_type_names.clone(),
            type_aliases: self.type_aliases.clone(),
            class_alias_undo: Vec::new(),
            variables: self.variables.clone(),
            imports: self.imports.clone(),
            extern_c: self.extern_c,
            extern_c_names: self.extern_c_names.clone(),
            class_scopes: self.class_scopes.clone(),
            asm_problem_recovery: false,
            in_function_body: self.in_function_body,
            function_declarations: Vec::new(),
            variable_types: self.variable_types.clone(),
            variable_closures: self.variable_closures.clone(),
            function_returns: self.function_returns.clone(),
            object_bindings: HashSet::new(),
            global_pointers: HashSet::new(),
            external_functions: HashSet::new(),
            later_functions: HashSet::new(),
            record_depth: 0,
            function_full_name: self.function_full_name.clone(),
            using_namespaces: self.using_namespaces.clone(),
            lexical_scope: self.lexical_scope.clone(),
            class_fields: self.class_fields.clone(),
            scalar_constructor_classes: self.scalar_constructor_classes.clone(),
            constructor_initializers: self.constructor_initializers,
            function_fields: self.function_fields.clone(),
            function_member_cv: self.function_member_cv,
            global_expressions: Vec::new(),
            // The replacement for types_compatible_p contains its own name;
            // a disabled macro is not expanded a second time by CDT.
            macro_expansion: true,
            current_method: self.current_method.clone(),
            lambda_counter: self.lambda_counter,
            defined_cpp_methods: self.defined_cpp_methods.clone(),
            lambda_ast_parent: self.lambda_ast_parent.clone(),
            retained_macro_invocations: Vec::new(),
            retained_macro_call_ends: HashSet::new(),
        };
        let mut expansion = parser.expression(0);
        if parser.pos < parser.limit {
            parser.diagnose(
                parser.pos,
                parser.limit,
                "unparsed builtin macro expansion tokens",
            );
        }
        for diagnostic in &mut parser.diagnostics {
            diagnostic.span = span.clone();
        }
        self.diagnostics.append(&mut parser.diagnostics);
        self.lambda_counter = parser.lambda_counter;
        for mut function in parser.function_declarations {
            generated_statement(&mut function.body, replacement, &span);
            function.span = span.clone();
            for parameter in &mut function.parameters {
                parameter.span = span.clone();
            }
            self.function_declarations.push(function);
        }
        let mut cloned = Vec::new();
        for &(a, b) in arguments {
            let code: String = (a..b).map(|i| self.text(i)).collect();
            if let Some(mut expression) = expanded_argument(&expansion, replacement, &code) {
                generated_expression(&mut expression, replacement, &span);
                cloned.push(expression);
            }
        }
        generated_expression(&mut expansion, replacement, &span);
        Expr {
            kind: ExprKind::MacroCall {
                name,
                arguments: cloned,
                expansion: Box::new(expansion),
                arity,
            },
            span,
        }
    }
}
