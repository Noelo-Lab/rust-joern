use super::*;

#[test]
fn cpp_implicit_return_problems_preserve_explicit_functions_and_constructors() {
    let source = include_str!(
        "../../tests/fixtures/declaration-recovery/implicit_return_function_declarations.cpp"
    );
    let unit = parse(source, true);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 7);
    assert_eq!(
        unit.functions.iter().filter(|f| f.name == "Object").count(),
        2
    );
    assert!(
        unit.functions
            .iter()
            .all(|f| !matches!(f.body.kind, StmtKind::Empty))
    );
    assert!(
        !unit
            .functions
            .iter()
            .any(|f| f.name.ends_with("_only") || f.name.ends_with("_definition"))
    );
}

#[test]
fn orphan_global_closing_braces_preserve_following_function_definitions() {
    let source = "int known(char*); int work(int); int condition(int x){if(known(\"broken)){\nwork(x);\n}work(x);return x;}\nint after(int x){return x;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert!(
            unit.functions
                .iter()
                .any(|f| f.name == "after" && !matches!(f.body.kind, StmtKind::Empty))
        );
    }
}

#[test]
fn address_expressions_cannot_be_grouped_formal_parameters() {
    let source = include_str!(
        "../../tests/fixtures/declaration-recovery/grouped_address_parameter_recovery.c"
    );
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let names: std::collections::HashSet<_> =
            unit.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "after_address",
                "after_builtin",
                "ordinary",
                "callback",
                "grouped",
                "grouped_parameter"
            ]
            .into_iter()
            .collect()
        );
        assert!(
            unit.functions
                .iter()
                .all(|f| !f.parameters.iter().any(|p| p.name == "Small"))
        );
    }
}

#[test]
fn grouped_function_names_keep_cdt_full_name_fallback_and_distinct_stubs() {
    let source = "int ordinary(int); int (ordinary)(int x) {return x;} int (grouped)(int); int (grouped)(int x) {return x;} int (inner(int x)) {return x;}";
    let c = parse(source, false);
    assert!(c.diagnostics.is_empty(), "{:?}", c.diagnostics);
    let ordinary: Vec<_> = c
        .functions
        .iter()
        .filter(|f| f.name == "ordinary")
        .collect();
    assert_eq!(ordinary.len(), 2);
    assert_eq!(ordinary[0].full_name, "<unresolvedNamespace>.ordinary");
    assert_eq!(ordinary[1].full_name, "ordinary");
    let grouped: Vec<_> = c.functions.iter().filter(|f| f.name == "grouped").collect();
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[0].full_name, "<unresolvedNamespace>.grouped");
    assert_eq!(
        c.functions
            .iter()
            .find(|f| f.name == "inner")
            .unwrap()
            .full_name,
        "inner"
    );
    let cpp = parse(source, true);
    assert!(cpp.diagnostics.is_empty(), "{:?}", cpp.diagnostics);
    assert_eq!(cpp.functions.len(), 3);
    assert!(
        cpp.functions
            .iter()
            .all(|f| f.full_name == format!("{}:int(int)", f.name))
    );
}

#[test]
fn cast_operands_cannot_name_grouped_declarations_or_shadow_type_bindings() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/address_declarator_recovery.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let address = unit
            .functions
            .iter()
            .find(|f| f.name == "address_name")
            .unwrap();
        let StmtKind::Block(body) = &address.body.kind else {
            panic!("expected function body");
        };
        assert!(matches!(body[0].kind, StmtKind::Problem));
        assert!(matches!(
            &body[1].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        let shadow = unit
            .functions
            .iter()
            .find(|f| f.name == "shadowed_type")
            .unwrap();
        let StmtKind::Block(body) = &shadow.body.kind else {
            panic!("expected function body");
        };
        assert!(
            matches!(&body[0].kind, StmtKind::Declaration(declarations) if declarations[0].name == "Alias")
        );
        assert!(
            matches!(&body[1].kind, StmtKind::Return(Some(Expr { kind: ExprKind::Binary { op, .. }, .. })) if op == "+")
        );
    }
}

#[test]
fn problem_enums_use_compound_recovery_without_changing_global_recovery() {
    let source = "int host(int n) { enum { 2=2 }; int absent(int); enum { VALID=1 }; int kept(int); return n; } enum { 2=2 }; int global_after(int n) { return n; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let names: std::collections::HashSet<_> =
            unit.functions.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            ["host", "kept", "global_after"].into_iter().collect()
        );
        let host = unit.functions.iter().find(|f| f.name == "host").unwrap();
        let StmtKind::Block(body) = &host.body.kind else {
            panic!("expected compound function body");
        };
        assert!(matches!(body[0].kind, StmtKind::Problem));
        assert!(matches!(body.last().unwrap().kind, StmtKind::Return(_)));
    }
}

#[test]
fn local_noreturn_recovery_does_not_apply_global_storage_restrictions() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/nested_noreturn_headers.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), if cpp { 9 } else { 13 });
        assert!(unit.functions.iter().any(|f| f.name == "leading_proto"));
        for name in ["leading_child", "storage_child", "local_proto"] {
            assert_eq!(unit.functions.iter().any(|f| f.name == name), !cpp);
        }
    }
}

#[test]
fn record_fields_distinguish_problem_macros_and_function_declarations() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/record_function_shaped_fields.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 4);
        assert!(!unit.functions.iter().any(|f| f.name == "WRAP"));
        let callback = unit
            .functions
            .iter()
            .find(|f| f.name == "callback")
            .unwrap();
        assert_eq!(
            callback.full_name,
            if cpp {
                "Global.callback:int(int)"
            } else {
                "callback"
            }
        );
    }
}

#[test]
fn problem_record_parentheses_preserve_valid_attributes_and_grouped_returns() {
    for cpp in [false, true] {
        let problem = parse(
            include_str!("../../tests/fixtures/declaration-recovery/problem_record_parentheses.c"),
            cpp,
        );
        let valid = parse(
            include_str!("../../tests/fixtures/declaration-recovery/record_return_declarators.c"),
            cpp,
        );
        assert!(problem.diagnostics.is_empty() && valid.diagnostics.is_empty());
        assert_eq!(problem.functions.len(), 5);
        assert_eq!(valid.functions.len(), 3);
        assert!(valid.functions.iter().any(|f| f.name == "grouped"));
        assert!(valid.functions.iter().any(|f| f.name == "factory"));
    }
}

#[test]
fn c_and_cpp_loop_separators_use_object_bindings_instead_of_inferred_types() {
    let source = "typedef int i; int work(int); int f(int n){int i=0;for(;i<((n<<2)+3);i=(n>>i)&7)work(i);return i;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let f = unit.functions.iter().find(|f| f.name == "f").unwrap();
        let StmtKind::Block(body) = &f.body.kind else {
            panic!()
        };
        assert!(matches!(
            &body[1].kind,
            StmtKind::For {
                condition: Some(_),
                update: Some(_),
                ..
            }
        ));
    }
}

#[test]
fn gnu_nested_functions_keep_lexical_methods_and_parent_ast_markers() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/nested_function_declarations.c");
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    assert_eq!(c.functions.len(), 7);
    assert_eq!(cpp.functions.len(), 6);
    let nested = c.functions.iter().find(|f| f.name == "nested").unwrap();
    assert_eq!(nested.full_name, "nested");
    assert_eq!(nested.lambda_parent.as_deref(), Some("valid_outer"));
    assert!(!nested.lambda);
    assert!(
        nested
            .inherited_bindings
            .contains(&("n".into(), "int".into()))
    );
    let parent = c
        .functions
        .iter()
        .find(|f| f.name == "valid_outer")
        .unwrap();
    let StmtKind::Block(body) = &parent.body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind, StmtKind::FunctionDefinition { name, full_name } if name == "nested" && full_name == "nested")
    );
    assert!(matches!(&body[1].kind, StmtKind::Return(_)));
    assert!(!cpp.functions.iter().any(|f| f.name == "nested"));
}

#[test]
fn missing_outer_braces_keep_c_nested_definitions_without_control_headers() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/missing_outer_functions.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), if cpp { 3 } else { 5 });
        assert!(
            !unit
                .functions
                .iter()
                .any(|f| matches!(f.name.as_str(), "if" | "while" | "for" | "switch"))
        );
        let outer = unit
            .functions
            .iter()
            .find(|f| f.name == "unclosed_outer")
            .unwrap();
        let StmtKind::Block(body) = &outer.body.kind else {
            panic!()
        };
        assert!(matches!(&body[0].kind, StmtKind::Expression(_)));
        assert!(matches!(&body[1].kind, StmtKind::Block(_)));
    }
}

#[test]
fn unfinished_translation_unit_declarations_keep_completed_methods() {
    for tail in [
        "int incomplete(int n)",
        "Unexpected words! Failed to recover int32_t absent(int argc) due to error during bad-pass.",
    ] {
        for cpp in [false, true] {
            let unit = parse(&format!("int before(int n) {{ return n; }} {tail}"), cpp);
            assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
            assert_eq!(
                unit.functions
                    .iter()
                    .map(|f| f.name.as_str())
                    .collect::<Vec<_>>(),
                ["before"]
            );
        }
    }
}

#[test]
fn malformed_numeric_and_nameless_declarators_keep_later_statements() {
    let source = "int work(int); int f(int n) { void (*0x1160)() (); int 0x1160; long() ** value; work(n); return n; } int after(int n) { ((void (*)(int))0x1160)(n); return n; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let method = unit.functions.iter().find(|f| f.name == "f").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        assert!(
            body[..2]
                .iter()
                .all(|s| matches!(s.kind, StmtKind::Problem))
        );
        if cpp {
            assert!(
                matches!(&body[2].kind, StmtKind::Expression(Expr { kind: ExprKind::Binary { op, .. }, .. }) if op == "*")
            );
        } else {
            assert!(matches!(body[2].kind, StmtKind::Problem));
        }
        assert!(matches!(body[3].kind, StmtKind::Expression(_)));
        let method = unit.functions.iter().find(|f| f.name == "after").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        assert!(matches!(
            &body[0].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
    }
}

#[test]
fn problem_headers_distinguish_duplicate_parameters_from_pointer_returns() {
    let source = "int bad(int n)(int n) { return n; } define pseudo { work(1); } other bare { work(2); } int (*valid(int n))(int) { return 0; } int after(int n) { return n; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["valid", "after", "bad"]
        );
        assert!(matches!(unit.functions[2].body.kind, StmtKind::Empty));
    }
}

#[test]
fn ordinary_function_names_do_not_become_operator_overloads() {
    for cpp in [false, true] {
        let unit = parse(
            "int operators(int n) { return n; } int operator_names(int n) { return n; }",
            cpp,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["operators", "operator_names"]
        );
    }
    let unit = parse("int operator(int n) { return n; }", false);
    assert_eq!(unit.functions[0].name, "operator");
}

#[test]
fn named_type_declaration_does_not_replace_a_function_binding() {
    let source = "struct sigaction { int handler; }; int sigaction(int, void *, void *); int f(int n) { sigaction local; sigaction(n, &local, 0); return n; } int multiply(int a, int b) { a*b; return a; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let method = unit.functions.iter().find(|f| f.name == "f").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        assert!(matches!(&body[0].kind, StmtKind::Declaration(values)
            if values[0].name == "local" && values[0].type_name == "sigaction"));
        assert!(matches!(
            &body[1].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        let method = unit
            .functions
            .iter()
            .find(|f| f.name == "multiply")
            .unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        assert!(matches!(&body[0].kind, StmtKind::Expression(Expr {
            kind: ExprKind::Binary { op, .. }, ..
        }) if op == "*"));
    }
}

#[test]
fn recovered_assembly_tail_parameter_names_do_not_become_types() {
    let unit = parse(
        "int broken(int v10) { __asm { nop } orphan(v10); } int after(int x) { int (*v10)(int); v10(x); v10(x); return x; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let orphan = unit.functions.iter().find(|f| f.name == "orphan").unwrap();
    assert_eq!(orphan.signature, "ANY(ANY)");
    assert_eq!(orphan.parameters[0].name, "v10");
    assert_eq!(orphan.parameters[0].type_name, "ANY");
    let after = unit.functions.iter().find(|f| f.name == "after").unwrap();
    let StmtKind::Block(body) = &after.body.kind else {
        panic!()
    };
    assert!(body[1..3].iter().all(|s| matches!(
        s.kind,
        StmtKind::Expression(Expr {
            kind: ExprKind::Call { .. },
            ..
        })
    )));
}

#[test]
fn call_shaped_assignment_lhs_recovers_a_grouped_declarator() {
    let source = "int declared_get(int); int f(int x) { get(x)=x+1; declared_get(x)=x+2; get(x); return x; } int after(int x) { get(x); declared_get(x); return x; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let method = unit.functions.iter().find(|f| f.name == "f").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        if cpp {
            assert!(body[..2].iter().all(|s| matches!(&s.kind, StmtKind::Expression(Expr {kind:ExprKind::Binary{op,..},..}) if op == "=")));
        } else {
            assert!(
                matches!(&body[0].kind, StmtKind::Declaration(values) if values[0].name == "x" && values[0].type_name == "get" && values[0].initializer.is_some())
            );
            assert!(
                matches!(&body[1].kind, StmtKind::Declaration(values) if values[0].name == "x" && values[0].type_name == "declared_get" && values[0].initializer.is_some())
            );
        }
        assert!(matches!(
            &body[2].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        let after = unit.functions.iter().find(|f| f.name == "after").unwrap();
        let StmtKind::Block(body) = &after.body.kind else {
            panic!()
        };
        assert!(body[..2].iter().all(|s| matches!(
            &s.kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        )));
    }
}

#[test]
fn typedef_grouped_name_recovery_distinguishes_new_and_bound_objects() {
    let source = "typedef int T; int f(int x) { T(y)=x; T(x); return x; } int loop(int x) { while(x)T(x)=1;return x; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let method = unit.functions.iter().find(|f| f.name == "f").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[0].kind, StmtKind::Declaration(values) if values[0].name == "y" && values[0].nested_declarator)
        );
        assert!(matches!(
            &body[1].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        let method = unit.functions.iter().find(|f| f.name == "loop").unwrap();
        let StmtKind::Block(body) = &method.body.kind else {
            panic!()
        };
        let StmtKind::While { body, .. } = &body[0].kind else {
            panic!()
        };
        assert_eq!(matches!(body.kind, StmtKind::Declaration(_)), !cpp);
    }
}

#[test]
fn const_first_knr_parameters_keep_all_function_bodies() {
    let unit = parse(
        "struct global { int p; int x; }; int first(p) const char *p; { if (*p) return 1; return 0; } int second(x) int x; { return x; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(unit.functions[0].parameters[0].name, "p");
    assert_eq!(unit.functions[0].parameters[0].type_name, "char*");
    assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
}

#[test]
fn object_initializers_and_array_bounds_do_not_hide_following_functions() {
    let unit = parse(
        "struct item { int value; }; static struct item configuration = { 0 }; static char options[2 * sizeof(configuration)]; int after(int x) { if (x) return 1; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].name, "after");
}

#[test]
fn anonymous_record_initializer_and_enum_alias_preserve_later_bodies() {
    let unit = parse(
        "static const struct { int x; } pairs[] = { { 1 }, { 2 } }; typedef enum { first, second } Choice; int f(Choice x) { if (x) return 1; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].name, "f");
    assert_eq!(unit.functions[0].parameters[0].type_name, "Choice");
}

#[test]
fn typedef_function_parameters_are_types_without_executable_expressions() {
    let unit = parse(
        "typedef long read_fn(void *cookie, char *buf, unsigned long count); typedef void (*callback_fn)(int); int f(read_fn *reader, callback_fn callback) { callback(1); return 0; }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].parameters[0].type_name, "read_fn*");
}

#[test]
fn elaborated_return_declarations_follow_cdt_type_declaration_conversion() {
    let unit = parse(
        "struct item; struct item *declaration(void); struct item *definition(void) { return 0; } int ordinary(void); int ordinary(void) { return 1; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["definition", "ordinary"]
    );
}

#[test]
fn operator_definitions_replace_matching_declaration_stubs() {
    let unit = parse(
        include_str!("../../tests/fixtures/decbench-regressions/cpp_operator_definitions.cpp"),
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["==", "!=", "="]
    );
    let declarations = parse(
        include_str!("../../tests/fixtures/decbench-regressions/cpp_operator_declarations.cpp"),
        true,
    );
    assert_eq!(
        declarations
            .functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["operator ==", "operator !=", "operator ="]
    );
}

#[test]
fn inferred_typenames_do_not_turn_object_assignment_into_declarations() {
    let unit = parse(
        "typedef int array; int f(void) { array = 1; array[0] = 2; array.member = 3; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        body[..3]
            .iter()
            .all(|s| matches!(s.kind, StmtKind::Expression(_)))
    );
}

#[test]
fn named_type_function_pointer_fields_are_objects() {
    let unit = parse(
        "typedef long size; struct callbacks { size (*read)(int); size (*write)(int); }; size (*global_callback)(int); size f(int x) { return x; }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].name, "f");
}

#[test]
fn using_declaration_resolves_method_owner_and_removes_stub() {
    let unit = parse(
        "namespace n { struct A { int f(); }; } using n::A; int A::f() { return 0; }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].full_name, "n.A.f:int()");
}

#[test]
fn cpp_method_names_and_signatures_match_original_joern() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/ddg-lambdas/method-names.json"
    ))
    .unwrap();
    assert_eq!(reference["generator"], "Original Joern 4.0.150 / c2cpg");
    for (filename, source) in [
        (
            "using.cpp",
            include_str!("../../tests/fixtures/ddg-lambdas/using.cpp"),
        ),
        (
            "namespaced.cpp",
            include_str!("../../tests/fixtures/ddg-lambdas/namespaced.cpp"),
        ),
    ] {
        let unit = parse(source, true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let methods = reference["methods"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|method| method["filename"] == filename)
            .collect::<Vec<_>>();
        assert_eq!(unit.functions.len(), methods.len());
        for function in &unit.functions {
            let original = methods
                .iter()
                .find(|method| method["name"] == function.name)
                .unwrap();
            assert_eq!(function.full_name, original["fullname"].as_str().unwrap());
            assert_eq!(function.signature, original["signature"].as_str().unwrap());
            assert_eq!(
                function.return_type,
                original["return_type"].as_str().unwrap()
            );
        }
    }
}

#[test]
fn cpp_declaration_iteration_matches_original_constructor_selection() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/ddg-classes/joern-4.0.150.json"
    ))
    .unwrap();
    let unit = parse(
        include_str!("../../tests/fixtures/ddg-classes/constructors.cpp"),
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let expected: Vec<_> = reference["methods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|method| method["fullname"].as_str().unwrap())
        .collect();
    assert_eq!(
        unit.functions
            .iter()
            .map(|function| function.full_name.as_str())
            .collect::<Vec<_>>(),
        expected
    );
    // Every reduced constructor has one CFG node, so PyJoern keeps the
    // last declaration on ties, including Plain's default overload.
    for name in ["type_info", "Plain"] {
        let selected = unit
            .functions
            .iter()
            .rev()
            .find(|function| function.name == name)
            .unwrap();
        let original = reference["methods"]
            .as_array()
            .unwrap()
            .iter()
            .find(|method| method["name"] == name && method["selected_by_from_many"] == true)
            .unwrap();
        assert_eq!(selected.full_name, original["fullname"].as_str().unwrap());
    }
}

#[test]
fn c_and_cpp_declaration_orders_match_original_joern() {
    let reference: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/ddg-classes/method-order-oracle.json"
    ))
    .unwrap();
    for case in reference["cases"].as_array().unwrap() {
        let unit = parse(case["source"].as_str().unwrap(), case["language"] == "cpp");
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let original = case["ordered_methods"].as_array().unwrap();
        assert_eq!(unit.functions.len(), original.len());
        for (function, method) in unit.functions.iter().zip(original) {
            assert_eq!(function.name, method["name"].as_str().unwrap());
            assert_eq!(function.full_name, method["fullname"].as_str().unwrap());
            assert_eq!(function.signature, method["signature"].as_str().unwrap());
            assert_eq!(
                function.return_type,
                method["return_type"].as_str().unwrap()
            );
        }
    }
}

#[test]
fn c_symbol_definition_replaces_array_parameter_declaration() {
    let unit = parse(
        "int f(int values[]); int f(int *values) { return values[0]; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
}

#[test]
fn file_scope_assembly_preserves_following_method_and_asm_symbol_alias() {
    let unit = parse(
        "__asm__(\".section .text\"); __asm { nop } int declared(void) __asm__(\"symbol\"); int f(void) { return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["f", "declared"]
    );
}

#[test]
fn cdt_problem_attributes_recover_later_methods_with_language_specific_rules() {
    let source = "int empty(int x) __attribute__(()); int empty_body(int x) __attribute__(()) { return x; } [[__nodiscard__]] int cpp_only([[__maybe_unused__]] int x) { return x; } int valid(int x) __attribute__((unused)); int after(int x) { return x; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let names = unit
            .functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            names,
            if cpp {
                vec!["cpp_only", "after", "valid"]
            } else {
                vec!["after", "valid"]
            }
        );
    }
}

#[test]
fn cdt_noreturn_problem_recovery_depends_on_declaration_order() {
    let source = "_Noreturn void prefix(int x) { return; } static _Noreturn void after_static(int x) { return; } void _Noreturn after_type(int x) { return; } __attribute__((noreturn)) void gnu(int x) { return; } int after(void) { return 0; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["prefix", "gnu", "after"]
        );
    }
}

#[test]
fn attributes_before_local_declaration_keep_initializers() {
    for source in [
        "int f(int x) { __attribute__((unused)) int value = x + 1; return value; }",
        "int f(int x) { [[__maybe_unused__]] int value = x + 1; return value; }",
    ] {
        let unit = parse(source, true);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[0].kind, StmtKind::Declaration(values) if values[0].initializer.is_some())
        );
    }
}

#[test]
fn c_record_tags_do_not_classify_function_calls_as_declarations() {
    let unit = parse(
        "struct sigaction { int value; }; int f(int x) { struct sigaction state; sigaction(x, 0, &state); return x; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(matches!(
        body[1].kind,
        StmtKind::Expression(Expr {
            kind: ExprKind::Call { .. },
            ..
        })
    ));
}

#[test]
fn array_allocation_and_deallocation_operator_headers_keep_exception_specs() {
    let unit = parse(
        "void *operator new[](unsigned long size) { return 0; } void operator delete[](void *ptr) throw() { release(ptr); }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["new[]", "delete[]"]
    );
}

#[test]
fn block_scope_prototypes_emit_method_stubs_without_local_initializers() {
    let source = "int f(int x) { extern void start(void) __attribute__((__noreturn__)); int first(int), second(int); start(); return first(x); }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let expected = if cpp {
            ["f", "first", "start"]
        } else {
            ["f", "start", "first"]
        };
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            unit.functions[1..]
                .iter()
                .all(|f| matches!(f.body.kind, StmtKind::Empty))
        );
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(matches!(&body[0].kind, StmtKind::Declaration(values) if values.is_empty()));
    }
}

#[test]
fn grouped_declarators_keep_definitions_and_exclude_pointer_objects() {
    let source = "typedef int Number; Number (*callback)(int); static inline Number (*arrays(void))[4] { return 0; } int (digit)(int x) { return x; } static int (inside(int x)) { return x; } void (handler)(void) { return; }";
    let unit = parse(source, false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["arrays", "digit", "inside", "handler"]
    );
    // A named return specifier keeps only its simple name: unchanged
    // PyJoern exports `(METHOD_RETURN,Number)` for this definition.
    assert_eq!(unit.functions[0].return_type, "Number");
    assert_eq!(unit.functions[1].parameters[0].name, "x");
}

#[test]
fn unnamed_declared_parameters_keep_empty_binding_names() {
    let unit = parse("int sink(int, const char *);", false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions[0].parameters[0].name, "");
    assert_eq!(unit.functions[0].parameters[1].name, "");
}

#[test]
fn cpp_keyword_spelling_in_c_parameter_does_not_hide_later_definitions() {
    let unit = parse(
        "struct block { int value; }; static struct block *using_blocks(struct block *using[2]) { return using[0]; } int after(int x) { if (x) return 1; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["using_blocks", "after"]
    );
}

#[test]
fn object_shadowing_typedef_keeps_mutations_and_identifier_condition() {
    let unit = parse(
        "typedef int count; int f(int count) { count--; count -= 1; if (count) return 1; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(matches!(
        body[0].kind,
        StmtKind::Expression(Expr {
            kind: ExprKind::Unary { postfix: true, .. },
            ..
        })
    ));
    assert!(
        matches!(&body[1].kind, StmtKind::Expression(Expr {kind: ExprKind::Binary {op,..},..}) if op == "-=")
    );
    assert!(matches!(
        body[2].kind,
        StmtKind::If {
            condition: Expr {
                kind: ExprKind::Identifier(_),
                ..
            },
            ..
        }
    ));
}

#[test]
fn first_function_pointer_uses_cdt_binding_conversion_and_ignores_siblings() {
    let source = "int target(int x); void f(int x) { int (*pointer)(int) = x ? target : second, ignored = call(); }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Declaration(values) = &body[0].kind else {
            panic!()
        };
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].name, "pointer");
        assert!(values[0].initializer.is_none());
        assert_eq!(values[0].problem, cpp);
    }
}

#[test]
fn plain_first_declarator_keeps_later_function_pointer_initializer() {
    let unit = parse(
        "void f(int x) { int first = call(), (*pointer)(int) = x ? target : second; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(values) = &body[0].kind else {
        panic!()
    };
    assert_eq!(values.len(), 2);
    assert!(values.iter().all(|d| d.initializer.is_some() && !d.problem));
}

#[test]
fn an_unsized_array_modifier_disables_all_allocation_bounds() {
    let unit = parse("void f(int n) { extern int values[][n + 1]; }", false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(values) = &body[0].kind else {
        panic!()
    };
    assert!(values[0].dimensions.is_empty());
}

#[test]
fn genuine_type_bindings_govern_ambiguous_address_casts() {
    let inferred = parse("int f(uint32_t n) { return (uint32_t)&target; }", false);
    let declared = parse(
        "typedef unsigned int uint32_t; int f(uint32_t n) { return (uint32_t)&target; }",
        false,
    );
    for unit in [&inferred, &declared] {
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    }
    let StmtKind::Block(inferred_body) = &inferred.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Block(declared_body) = &declared.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&inferred_body[0].kind, StmtKind::Return(Some(e)) if matches!(&e.kind, ExprKind::Binary {op,..} if op=="&"))
    );
    assert!(
        matches!(&declared_body[0].kind, StmtKind::Return(Some(e)) if matches!(e.kind, ExprKind::Cast {..}))
    );
}

#[test]
fn top_level_declarations_convert_each_function_declarator() {
    let unit = parse(
        "void first(void), second(int); char *third(void), *fourth(int);",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["third", "fourth", "first", "second"]
    );
    assert_eq!(unit.functions[0].return_type, "char*");
    assert_eq!(unit.functions[1].return_type, "char*");
}

#[test]
fn record_attributes_and_grouped_arrays_do_not_create_methods() {
    let unit = parse(
        "typedef unsigned char byte; struct __attribute__((aligned(8))) R { byte (mask)[8]; }; int after(void) { return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].name, "after");
}

#[test]
fn attributed_unknown_function_pointer_is_a_declaration() {
    let unit = parse(
        "int f(void) { LONG (__attribute__((__stdcall__)) *query)(HANDLE handle); return query(0); }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(values) = &body[0].kind else {
        panic!()
    };
    assert_eq!(values[0].name, "query");
    assert_eq!(values.len(), 1);
}

#[test]
fn void_and_function_pointer_parameters_preserve_cdt_properties() {
    let unit = parse(
        "int empty(void) { return 0; } int f(int (*call)(char *), char *p) { return call(p); }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions[0].signature, "int(void)");
    assert_eq!(unit.functions[0].parameters[0].type_name, "void");
    assert!(unit.functions[0].parameters[0].name.is_empty());
    assert_eq!(unit.functions[1].parameters[0].type_name, "int");
    assert!(unit.functions[1].parameters[0].function_pointer);
    assert_eq!(unit.functions[1].parameters[1].type_name, "char*");
    assert!(!unit.functions[1].parameters[1].function_pointer);
}

#[test]
fn cxx_binding_names_linkage_and_implicit_this_follow_cdt() {
    let source = "typedef unsigned long Size; using Num = int; extern \"C\" { int cfun(void *); } int cfun(void *p) { return 0; } extern \"C\" int single(void) { return 1; } namespace N { struct S { static int stat(int x) { return x; } int decl(void); }; int S::decl(void) { return 1; } int local(Num n) { return n; } } int alias(Size n) { return 1; } int arrays(int a[4], int b[][3]) { return a[0]; } int fp(int (*f)(int), int x) { return f(x); }";
    let unit = parse(source, true);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.full_name.as_str())
            .collect::<Vec<_>>(),
        [
            "cfun",
            "single",
            "N.S.stat:int(int)",
            "N.S.decl:int()",
            "N.local:int(int)",
            "alias:int(unsigned longint)",
            "arrays:int(int*,int(*)[3])",
            "fp:int(int(*)(int),int)"
        ]
    );
    assert!(unit.functions[0].extern_c && unit.functions[1].extern_c);
    assert_eq!(unit.functions[2].implicit_this.as_deref(), Some("N.S"));
    assert_eq!(unit.functions[3].implicit_this.as_deref(), Some("N.S"));
    assert!(unit.functions[4].implicit_this.is_none());
    assert_eq!(unit.functions[4].signature, "int(Num)");
}

#[test]
fn cdt_noreturn_recovery_uses_c_header_line_boundaries() {
    let source = "extern _Noreturn void same_line(void); extern\n_Noreturn void before_keyword(void); extern _Noreturn\nvoid after_keyword(void); static\n_Noreturn\nvoid definition(void) { return; } int after(void) { return 0; }";
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    assert_eq!(
        c.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["definition", "after", "after_keyword", "before_keyword"]
    );
    assert_eq!(
        cpp.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["after"]
    );
    assert_eq!(c.functions[0].return_type, "void");
}

#[test]
fn emitted_tag_type_properties_omit_reserved_prefix() {
    let unit = parse(
        "struct Box { int x; }; int fields(struct Box *box) { return box->x; }",
        false,
    );
    assert!(unit.diagnostics.is_empty());
    assert_eq!(unit.functions[0].signature, "int(Box*)");
    assert_eq!(unit.functions[0].parameters[0].type_name, "Box*");
}

#[test]
fn recovered_assembly_tails_have_c_implicit_declarations() {
    let source = include_str!(
        "../../tests/fixtures/decbench-regressions/microsoft_asm_orphan_declarations.c"
    );
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    let mut names = c
        .functions
        .iter()
        .map(|f| f.name.as_str())
        .collect::<Vec<_>>();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "after_arguments",
            "after_do",
            "after_nested",
            "after_simple",
            "asm_tail_arguments",
            "asm_tail_do",
            "asm_tail_nested",
            "asm_tail_simple",
            "nested_outer",
            "other",
            "tail_variable",
            "tail_zero",
            "undeclared"
        ]
    );
    assert_eq!(cpp.functions.len(), 8);
    let other = c.functions.iter().find(|f| f.name == "other").unwrap();
    assert_eq!(other.signature, "ANY(ANY)");
    assert_eq!(other.parameters[0].name, "x");
    assert_eq!(other.span.line, 5);
    assert_eq!(other.span.end_line, 5);
    assert_eq!(c.functions[0].span.end_line, 3);
}

#[test]
fn global_initializers_and_array_bounds_preserve_builtin_macro_trees() {
    let unit = parse(
        "struct S { int x; }; unsigned offset = __builtin_offsetof(struct S, x); char bounds[1 + __builtin_types_compatible_p(int,int)]; int ordinary(void) { return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.global_expressions.len(), 2);
    assert!(
        matches!(&unit.global_expressions[0].kind, ExprKind::MacroCall {name, arity:2,..} if name=="__builtin_offsetof")
    );
    assert!(
        matches!(&unit.global_expressions[1].kind, ExprKind::Binary {right,..} if matches!(&right.kind,ExprKind::MacroCall {name,arity:2,..} if name=="__builtin_types_compatible_p"))
    );
}

#[test]
fn record_member_names_do_not_shadow_typedefs_outside_the_record() {
    let unit = parse(
        "typedef unsigned short u16; union values { u16 u16; }; int f(void *p) { return *(u16*)p; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind,StmtKind::Return(Some(e)) if matches!(&e.kind,ExprKind::Unary {argument,..} if matches!(argument.kind,ExprKind::Cast {..})))
    );
}

#[test]
fn malformed_function_header_recovery_keeps_later_definitions() {
    let unit = parse(
        "int broken@<eax>(int x) { return x; } int after(int x) { if (x) return 1; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty());
    assert!(unit.functions.iter().any(|f| f.name == "after"));
    assert!(!unit.functions.iter().any(|f| f.name == "broken"));
}

#[test]
fn cdt_problem_headers_keep_duplicate_specifier_definitions() {
    let source = "int registers@<eax>(int x@<eax>) { return x; } int after_register(int x) { return x; } unknown_type duplicated_type *duplicate(int x) { return x; } int after_duplicate(int x) { return x; } unknown_type *array[8](int x) { return x; } int after_array(int x) { return x; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            [
                "after_register",
                "duplicate",
                "after_duplicate",
                "after_array"
            ]
        );
    }
}

#[test]
fn constructors_without_return_specifiers_survive_declarations() {
    let unit = parse(
        "namespace n { class S { public: S(); S(const S&); virtual ~S(); }; }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["~S", "S", "S"]
    );
    assert!(unit.functions.iter().all(|f| f.implicit_this.is_none()));
}

#[test]
fn global_assertion_operands_do_not_create_function_bindings() {
    let source = "void declaration(time_t value); _Static_assert(((time_t)0)<=1,\"range\"); int f(void) { return (time_t)0; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["f", "declaration"]
        );
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[0].kind, StmtKind::Return(Some(e)) if matches!(&e.kind, ExprKind::Cast {type_name,..} if type_name=="time_t"))
        );
    }
}

#[test]
fn local_typedef_array_bounds_keep_alias_and_executable_expression() {
    let source = "int g(int); int f(int n) { typedef char T[n&&g(n)?1:-1]; return *(T*)0; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Declaration(declarations) = &body[0].kind else {
            panic!()
        };
        assert_eq!(declarations.len(), 1);
        assert!(declarations[0].is_typedef);
        assert_eq!(declarations[0].name, "T");
        assert_eq!(declarations[0].dimensions.len(), 1);
        assert!(
            matches!(&declarations[0].dimensions[0].kind, ExprKind::Conditional {condition,..} if matches!(&condition.kind, ExprKind::Binary {op,..} if op=="&&"))
        );
        assert!(
            matches!(&body[1].kind, StmtKind::Return(Some(e)) if matches!(&e.kind, ExprKind::Unary {argument,..} if matches!(&argument.kind, ExprKind::Cast {type_name,..} if type_name=="T*")))
        );
    }
}

#[test]
fn typeof_operand_identifiers_do_not_become_declared_types() {
    let source = "int f(int x) { __typeof__(x) copy=x; return __extension__({x>0?1:-1;}); }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[1].kind, StmtKind::Return(Some(e)) if matches!(&e.kind, ExprKind::Statement(s) if matches!(&s.kind, StmtKind::Block(statements) if matches!(&statements[0].kind, StmtKind::Expression(Expr {kind:ExprKind::Conditional{..},..})))))
        );
    }
}

#[test]
fn qualified_parameter_type_problem_declarations_recover_following_methods() {
    let source =
        include_str!("../../tests/fixtures/decbench-regressions/qualified_parameter_types.c");
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    assert_eq!(c.functions.len(), 4);
    assert!(c.functions.iter().all(|f| f.name.starts_with("after_")));
    assert_eq!(cpp.functions.len(), 8);
}

#[test]
fn recovered_c_assembly_tails_allow_function_shaped_initializers() {
    let source = include_str!("../../tests/fixtures/decbench-regressions/asm_tail_assignments.c");
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    for (name, parameter_count) in [("tail_identifier", 1), ("tail_zero", 0)] {
        let method = c.functions.iter().find(|f| f.name == name).unwrap();
        assert!(matches!(method.body.kind, StmtKind::Empty));
        assert_eq!(method.parameters.len(), parameter_count);
    }
    assert!(!c.functions.iter().any(|f| f.name == "tail_number"));
    assert!(!cpp.functions.iter().any(|f| f.name.starts_with("tail_")));
}

#[test]
fn leading_attributes_do_not_move_cdt_noreturn_recovery_to_a_new_line() {
    let source = "__attribute__((format(printf,1,2)))\nstatic _Noreturn void problem(const char *format,...) { return; } int after(void) { return 1; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(
            unit.functions
                .iter()
                .map(|f| f.name.as_str())
                .collect::<Vec<_>>(),
            ["after"]
        );
    }
}

#[test]
fn recovered_assembly_declarations_keep_separate_block_boundaries() {
    let source =
        include_str!("../../tests/fixtures/decbench-regressions/asm_repeated_projection.c");
    let unit = parse(source, false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    for name in [
        "top_first",
        "top_second",
        "top_third",
        "second_inside",
        "after_inner",
    ] {
        assert!(
            unit.functions
                .iter()
                .any(|f| f.name == name && matches!(f.body.kind, StmtKind::Empty)),
            "{name}"
        );
    }
}

#[test]
fn orphan_assignments_bind_global_variables_without_reusing_method_locals() {
    let source = include_str!("../../tests/fixtures/decbench-regressions/asm_recovered_controls.c");
    let unit = parse(source, false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert!(unit.functions.iter().any(|f| f.name == "undeclared"));
    assert!(!unit.functions.iter().any(|f| matches!(
        f.name.as_str(),
        "value" | "pointer" | "after_if" | "assign_if" | "after_loop" | "assign_loop"
    )));
}

#[test]
fn template_named_return_specifiers_cannot_follow_a_primitive_type() {
    let source = "void unknown<R1,R2> problem(int x){return;} unknown<R1,R2> named(int x){return x;} int after(void){return 0;}";
    let c = parse(source, false);
    let cpp = parse(source, true);
    assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
    assert_eq!(
        c.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["after"]
    );
    assert_eq!(
        cpp.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["named", "after"]
    );
}

#[test]
fn cpp_duplicate_function_bindings_follow_problem_fullname_fallback() {
    let unit = parse(
        "namespace N { int f(int x){return x;} int f(int x){return x;} int f(double x){return 0;} } extern \"C\" {int g(int x){return x;} int g(int x){return x;}} long h(int x){return x;} int h(int x){return x;}",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.full_name.as_str())
            .collect::<Vec<_>>(),
        [
            "N.f:int(int)",
            "f:<unresolvedSignature>",
            "N.f:int(double)",
            "g",
            "g:<unresolvedSignature>",
            "h:long int(int)",
            "h:int(int)"
        ]
    );
}

#[test]
fn cpp_method_bindings_keep_cv_overloads_and_qualified_repeated_definitions() {
    let unit = parse(
        "struct S {int f(int x){return x;} int f(int x){return x;} int cv(void){return 1;} int cv(void) const{return 2;} int ref(void)&{return 1;} int ref(void)&&{return 2;} int outside(int);}; int S::outside(int x){return x;} int S::outside(int x){return x;}",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.full_name.as_str())
            .collect::<Vec<_>>(),
        [
            "S.f:int(int)",
            "f:<unresolvedSignature>",
            "S.cv:int()",
            "S.cv:int()",
            "S.ref:int()",
            "S.ref:int()",
            "S.outside:int(int)",
            "S.outside:int(int)"
        ]
    );
    assert!(unit.functions[1].implicit_this.is_none());
}

#[test]
fn repeated_prototypes_keep_the_first_registered_declaration() {
    let unit = parse("int f(int first); int f(int second);", false);
    assert!(unit.diagnostics.is_empty());
    assert_eq!(unit.functions.len(), 1);
    assert_eq!(unit.functions[0].parameters[0].name, "first");
}

#[test]
fn retained_macro_descriptors_only_follow_executable_member_calls() {
    let source = "#define mirror(x) env->ops->mirror(env,x)\nextern __typeof__(env->ops->mirror(env,9)) declaration; void f(void) { env->ops->mirror(env,5); }\n";
    let unit = parse_preprocessed(source, false, true);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.retained_macro_calls.len(), 1);
    let call = &unit.retained_macro_calls[0];
    assert_eq!(call.name, "mirror");
    assert_eq!(call.formal_arity, 1);
    assert_eq!(call.parameter_count, 1);
    assert_eq!(call.definition_span.line, 1);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind,StmtKind::Expression(Expr {kind:ExprKind::Call {arguments,..},..}) if arguments.len()==2)
    );
}
