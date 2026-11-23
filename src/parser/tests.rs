use super::*;

#[test]
fn named_pointer_and_reference_ambiguities_follow_cdt_bindings() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/logical_declaration_boundaries.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        for name in [
            "logical_unknown",
            "logical_unknown_bare",
            "logical_unknown_both",
            "logical_parameter",
            "logical_global",
            "logical_calls",
            "logical_shadowed_alias",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert!(
                matches!(&body[0].kind,
                StmtKind::Expression(Expr {kind: ExprKind::Binary {op, ..}, ..})
                if op == "&&"),
                "{name}: {:?}",
                body[0]
            );
        }
        for name in [
            "pointer_known",
            "pointer_unknown",
            "pointer_unknown_uninitialized",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert!(matches!(&body[0].kind, StmtKind::Declaration(values)
                if values.len() == 1 && values[0].name == "pointer"));
        }
        for name in [
            "lvalue_reference_known",
            "lvalue_reference_unknown",
            "rvalue_reference_known",
            "rvalue_reference_unknown",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert_eq!(
                matches!(&body[0].kind, StmtKind::Declaration(_)),
                cpp,
                "{name}: {:?}",
                body[0]
            );
        }
    }
}

#[test]
fn reference_declarations_preserve_fresh_names_and_typed_prototypes() {
    let source =
        include_str!("../../tests/fixtures/declaration-recovery/reference_binding_boundaries.c");
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        for name in [
            "typed_logical_fresh",
            "unknown_logical_fresh",
            "typed_bitwise_fresh",
            "unknown_bitwise_fresh",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert_eq!(
                matches!(&body[0].kind, StmtKind::Declaration(_)),
                cpp,
                "{name}: {:?}",
                body[0]
            );
        }
        for name in [
            "typed_logical_bound",
            "unknown_logical_bound",
            "typed_bitwise_bound",
            "unknown_bitwise_bound",
            "typed_pointer_bound",
            "unknown_pointer_bound",
            "typed_pointer_comparison",
            "unknown_pointer_comparison",
            "value_call_rhs",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert!(
                matches!(&body[0].kind, StmtKind::Expression(_)),
                "{name}: {:?}",
                body[0]
            );
        }
        assert!(unit.functions.iter().any(|f| f.name == "make_pointer"));
        assert_eq!(
            unit.functions.iter().any(|f| f.name == "make_reference"),
            cpp
        );
        assert_eq!(unit.functions.iter().any(|f| f.name == "make_lvalue"), cpp);
    }
}

#[test]
fn qualified_pointer_declarators_override_ordinary_name_bindings() {
    let source = include_str!(
        "../../tests/fixtures/declaration-recovery/qualified_pointer_binding_boundaries.c"
    );
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        for name in [
            "record_const",
            "record_volatile",
            "unknown_const",
            "unknown_volatile",
            "scalar_shadow_const",
            "record_shadow_const",
            "unknown_shadow_const",
            "global_shadow_const",
            "bound_target_const",
            "nested_const",
            "nested_volatile",
            "plain_object_assignment",
            "fresh_pointer",
            "exact_timer_shape",
            "exact_ready_list_shape",
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert!(
                matches!(&body[0].kind, StmtKind::Declaration(values)
                if values.len() == 1 && !values[0].problem),
                "{name}: {:?}",
                body[0]
            );
        }
        for name in ["plain_object_product", "plain_global_product"] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!("{name}")
            };
            assert!(
                matches!(&body[0].kind,
                StmtKind::Expression(Expr{kind:ExprKind::Binary{op,..},..}) if op == "*"),
                "{name}: {:?}",
                body[0]
            );
        }
    }
}

#[test]
fn preprocessed_types_pointers_and_control_flow() {
    let source = "# 1 \"x.c\"\ntypedef unsigned long size_t;\nint helper(int);\nint f(int x, char *p) { size_t n = x, j; int (*fn)(int) = helper;\nL: for (j=0; j<n; ++j) { if (fn(j) && p[j]) continue; else break; }\nswitch (x) { case 1: x++; case 2 ... 4: goto L; default: return (int)n; } }";
    let unit = parse(source, false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["f", "helper"]
    );
    assert!(matches!(unit.functions[1].body.kind, StmtKind::Empty));
    assert_eq!(unit.functions[0].parameters[1].type_name, "char*");
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(declarations) = &body[1].kind else {
        panic!()
    };
    assert_eq!(declarations[0].name, "fn");
    assert!(
        matches!(&body[2].kind, StmtKind::Label { statement, .. } if matches!(statement.kind, StmtKind::For { .. }))
    );
}

#[test]
fn precedence_and_statement_expressions() {
    let unit = parse(
        "int f(int a, int b) { int x = ({ int t = b; t + 1; }); return a ? x : b || a && x; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Return(Some(Expr {
        kind: ExprKind::Conditional { alternative, .. },
        ..
    })) = &body[1].kind
    else {
        panic!()
    };
    assert!(
        matches!(&alternative.kind, ExprKind::Binary { op, right, .. } if op == "||" && matches!(&right.kind, ExprKind::Binary { op, .. } if op == "&&"))
    );
}

#[test]
fn namespace_methods_and_cpp_conditions() {
    let unit = parse(
        "namespace n { class A { public: int f(int x) { if (int y=x) return y; for (auto z : xs) x += z; try { throw x; } catch (...) { return 0; } return x; } }; template<class T> T id(T x) { return x; } }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 2);
    assert_eq!(unit.functions[0].full_name, "n.A.f:int(int)");
    assert_eq!(unit.functions[1].return_type, "T");
}

#[test]
fn parentheses_keep_operand_source_spans() {
    let source = "int f(int x) { return (x) + 1; }";
    let unit = parse(source, false);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Return(Some(Expr {
        kind: ExprKind::Binary { left, .. },
        span,
    })) = &body[0].kind
    else {
        panic!()
    };
    assert_eq!(&source[left.span.start..left.span.end], "x");
    assert_eq!(&source[span.start..span.end], "(x) + 1");
}

#[test]
fn unknown_types_gnu_ternary_and_computed_goto() {
    let unit = parse(
        "__int64 f(_DWORD *a) { unknown_t v = (_DWORD)*a; int x = v ?: 1; goto *a; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(matches!(&body[2].kind, StmtKind::Goto(name) if name == "*"));
}

#[test]
fn knr_definitions_use_parameter_declarations() {
    let unit = parse(
        "void f(x,p) unsigned x; char *p; { while (x--) *p++=0; } unsigned g() { return 1; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 2);
    assert_eq!(unit.functions[0].parameters[0].name, "x");
    assert_eq!(unit.functions[0].parameters[0].type_name, "unsigned");
    assert_eq!(unit.functions[0].parameters[1].type_name, "char*");
    assert!(matches!(unit.functions[0].body.kind, StmtKind::Block(_)));
}

#[test]
fn arrays_default_parameters_and_operator_methods() {
    let unit = parse(
        "int sum(int a[4]); struct A { bool operator==(A x) { return 1; } int operator()(int x=3) { return x; } }; int f(int x=1) { return x; }",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(
        unit.functions
            .iter()
            .map(|f| f.name.as_str())
            .collect::<Vec<_>>(),
        ["==", "()", "f", "sum"]
    );
    assert_eq!(unit.functions[3].parameters[0].type_name, "int[4]");
}

#[test]
fn unknown_parenthesized_identifier_remains_binary_operand() {
    let unit = parse("int f(void) { return (MAX) + 1; }", false);
    assert!(unit.diagnostics.is_empty());
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind, StmtKind::Return(Some(Expr {kind: ExprKind::Binary {op,..},..})) if op == "+")
    );
}

#[test]
fn sizeof_keeps_call_operands_in_logical_expression() {
    let unit = parse(
        "int foo(int); int bar(int); int f(int n) { return sizeof(foo(n) && bar(n)); }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Return(Some(Expr {
        kind: ExprKind::Unary { op, argument, .. },
        ..
    })) = &body[0].kind
    else {
        panic!()
    };
    assert_eq!(op, "sizeof");
    assert!(
        matches!(&argument.kind, ExprKind::Binary { op, left, right } if op == "&&" && matches!(left.kind, ExprKind::Call {..}) && matches!(right.kind, ExprKind::Call {..}))
    );
}

#[test]
fn unknown_calls_with_address_arguments_are_expressions() {
    let source = "int f(char *src, int *p) { char filename[20]; char *sa=filename; strcat(&filename, &src[(int)(sa - &filename)]); if (cond) strcat(&filename, \".bz2\"); consume(*p); Unknown (*callback)(int); return (uch)(*p); }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(matches!(
            &body[2].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        assert!(
            matches!(&body[3].kind, StmtKind::If {consequence,..} if matches!(consequence.kind, StmtKind::Expression(Expr {kind: ExprKind::Call {..},..})))
        );
        assert!(matches!(
            &body[4].kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Call { .. },
                ..
            })
        ));
        assert!(
            matches!(&body[5].kind, StmtKind::Declaration(declarations) if declarations[0].name == "callback")
        );
        assert!(matches!(
            &body[6].kind,
            StmtKind::Return(Some(Expr {
                kind: ExprKind::Call { .. },
                ..
            }))
        ));
    }
}

#[test]
fn cannot_silently_drop_control_flow() {
    let unit = parse(
        "int f(int x) { co_await x; asm goto (\"\" : : : : done); done: return x; }",
        true,
    );
    assert!(!unit.diagnostics.is_empty());
    assert_eq!(unit.functions.len(), 1);
}
