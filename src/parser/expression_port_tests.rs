use super::*;

#[test]
fn consecutive_casts_keep_the_prefix_increment_operand() {
    for cpp in [false, true] {
        let unit = parse(
            "int f(int x) { short *p; p = (short *)(unsigned char)++x; return x; }",
            cpp,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        let StmtKind::Expression(Expr {
            kind: ExprKind::Binary { right, .. },
            ..
        }) = &body[1].kind
        else {
            panic!()
        };
        let ExprKind::Cast { argument, .. } = &right.kind else {
            panic!()
        };
        let ExprKind::Cast { argument, .. } = &argument.kind else {
            panic!()
        };
        assert!(matches!(&argument.kind,
            ExprKind::Unary {op,argument,postfix:false}
                if op == "++" && matches!(&argument.kind,ExprKind::Identifier(name) if name == "x")));
        assert!(matches!(body[2].kind, StmtKind::Return(_)));
    }
}

#[test]
fn unfinished_switch_retains_its_body_and_complete_inner_switch() {
    for cpp in [false, true] {
        let unit = parse(
            "int f(int n) { n++; switch(n) { case 1: n--; switch(n) { case 2: return n; default: n++; }",
            cpp,
        );
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert_eq!(body.len(), 2);
        let StmtKind::Block(recovered) = &body[1].kind else {
            panic!()
        };
        assert!(
            recovered
                .iter()
                .any(|s| matches!(s.kind, StmtKind::Switch { .. }))
        );
    }
}

#[test]
fn indirect_calls_retain_cast_and_dereference_receiver() {
    let unit = parse(
        "typedef void code(void); int f(int *p) { (*(code *)(p))(); return *p; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit
        .functions
        .iter()
        .find(|f| f.name == "f")
        .unwrap()
        .body
        .kind
    else {
        panic!()
    };
    assert!(matches!(&body[0].kind, StmtKind::Expression(Expr {
        kind: ExprKind::Call { callee, .. }, ..
    }) if matches!(&callee.kind, ExprKind::Unary {op, argument,..} if op == "*" && matches!(argument.kind, ExprKind::Cast {..}))));
}

#[test]
fn nested_designators_repeat_operand_as_cdt_assignments() {
    let unit = parse(
        "struct B { int n; }; struct A { struct B b; }; int f(void) { struct A a = { .b.n = 3 }; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(declarations) = &body[0].kind else {
        panic!()
    };
    let ExprKind::ArrayInitializer(values) = &declarations[0].initializer.as_ref().unwrap().kind
    else {
        panic!()
    };
    let ExprKind::Block(assignments) = &values[0].kind else {
        panic!()
    };
    assert_eq!(assignments.len(), 2);
    for (assignment, name) in assignments.iter().zip(["b", "n"]) {
        assert!(
            matches!(&assignment.kind, ExprKind::Binary {op, left, right} if op == "=" && matches!(&left.kind, ExprKind::Identifier(n) if n == name) && matches!(&right.kind, ExprKind::Literal(n) if n == "3"))
        );
    }
}

#[test]
fn terminal_problem_label_keeps_closing_brace_for_enclosing_block() {
    let unit = parse(
        "int f(int n) { while (n--) { goto end; end: } return n; } int g(void) { return 1; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert_eq!(unit.functions.len(), 2);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert_eq!(body.len(), 2);
    assert!(matches!(body[1].kind, StmtKind::Return(_)));
}

#[test]
fn assembly_is_explicit_opaque_cdt_node() {
    let unit = parse(
        "void f(void) { __asm__ __volatile__(\"x\" : : : \"memory\"); asm goto(\"y\" : : : : end); end: ; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    for statement in &body[..2] {
        assert!(matches!(
            &statement.kind,
            StmtKind::Expression(Expr {
                kind: ExprKind::Assembly(_),
                ..
            })
        ));
    }
}

#[test]
fn following_cast_disambiguates_unknown_type_alias() {
    let unit = parse(
        "int f(int n) { return (outer_type)(inner_type)(long double)n; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Return(Some(expression)) = &body[0].kind else {
        panic!()
    };
    let mut expression = expression;
    for name in ["outer_type", "inner_type", "long double"] {
        let ExprKind::Cast {
            type_name,
            argument,
        } = &expression.kind
        else {
            panic!("{expression:?}")
        };
        assert_eq!(type_name, name);
        expression = argument;
    }
    assert!(matches!(&expression.kind,ExprKind::Identifier(name) if name == "n"));
}

#[test]
fn gnu_initializer_designators_are_assignment_blocks() {
    let unit = parse(
        "struct A {int x;}; int f(void) { struct A a={ x: 1 }; int b[8] = { [1 ... 3] = 7 }; return 0; }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    let StmtKind::Declaration(declarations) = &body[1].kind else {
        panic!()
    };
    let ExprKind::ArrayInitializer(values) = &declarations[0].initializer.as_ref().unwrap().kind
    else {
        panic!()
    };
    let ExprKind::Block(assignments) = &values[0].kind else {
        panic!()
    };
    assert!(
        matches!(&assignments[0].kind,ExprKind::Binary {left,..} if matches!(&left.kind,ExprKind::ArrayInitializer(bounds) if bounds.len()==2))
    );
}

#[test]
fn microsoft_assembly_recovers_cdt_compound_boundary() {
    let source =
        "int f(int x) { x++; __asm { mov eax, ebx } x+=2; return x; } int g(void) {return 1;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        assert_eq!(unit.functions.len(), 2);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert_eq!(body.len(), 2);
        assert!(
            matches!(&body[0].kind,StmtKind::Expression(Expr {kind:ExprKind::Unary {op,..},..}) if op=="++")
        );
        assert!(matches!(body[1].kind, StmtKind::Problem));
        assert_eq!(
            unit.functions[0].span.end,
            source.find(" } x+=2").unwrap() + 2
        );
    }
}

#[test]
fn c_qualified_names_follow_problem_recovery_context() {
    let unit = parse(
        "int f(int x) { ::global=x; if (::global && x) return 1; return ::foo(x); }",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert_eq!(body.len(), 3);
    assert!(matches!(body[0].kind, StmtKind::Problem));
    assert!(
        matches!(&body[1].kind,StmtKind::If {condition,..} if matches!(&condition.kind,ExprKind::Problem(text) if text=="::global && x"))
    );
    assert!(matches!(body[2].kind, StmtKind::Problem));
}

#[test]
fn ambiguous_casts_use_real_typedef_bindings_and_object_shadowing() {
    let unit = parse(
        "typedef int T; void decl(Unbound x); int f(int T,int x){return (T)&x;} int g(int x){return (Unbound)&x;} int h(int x){return (T)&x;}",
        false,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    for name in ["f", "g", "h"] {
        let function = unit.functions.iter().find(|f| f.name == name).unwrap();
        let StmtKind::Block(body) = &function.body.kind else {
            panic!()
        };
        let StmtKind::Return(Some(expression)) = &body[0].kind else {
            panic!()
        };
        if name == "h" {
            assert!(matches!(expression.kind, ExprKind::Cast { .. }));
        } else {
            assert!(matches!(&expression.kind,ExprKind::Binary{op,..} if op=="&"));
        }
    }
}

#[test]
fn ordinary_type_arguments_recover_whole_problem_statements() {
    let source = "typedef int T; int f(int ap,int n){ n=arbitrary(ap,long long); int a=0,b=arbitrary(ap,char*); if(n)n=arbitrary(ap,T*); while(n)n=arbitrary(ap,); return arbitrary(ap,int); }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert_eq!(body.len(), 5);
        assert!(body.iter().all(|s| matches!(s.kind, StmtKind::Problem)));
    }
}

#[test]
fn valid_cast_sizeof_alias_and_builtin_arguments_remain_executable() {
    let source = "typedef int T; int f(int ap,int n){ n=arbitrary(ap,T); n=arbitrary(ap,(long long)n); n=arbitrary(ap,sizeof(long long)); n=__builtin_va_arg(ap,int); return n; }";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert_eq!(body.len(), 5);
        assert!(
            body[..4]
                .iter()
                .all(|s| matches!(s.kind, StmtKind::Expression(_)))
        );
    }
}

#[test]
fn grouped_calls_with_pointer_operands_remain_executable() {
    fn unbracketed(expression: &Expr) -> &Expr {
        if let ExprKind::Bracketed(inner) = &expression.kind {
            unbracketed(inner)
        } else {
            expression
        }
    }
    let source = "int compare(char **s,char **t){return (strcoll(*s,*t));} int checked(char *s){return (legal(s,(int*)((void*)0)));} int rounded(int x){return (rounder(x*10000));} int loop(int *p,int x){while((*(int*)((long)x*8+42) && (compare((char*)((long)x*8+42),p))))x++;return x;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        for name in ["compare", "checked", "rounded"] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!()
            };
            assert!(
                matches!(&body[0].kind, StmtKind::Return(Some(value)) if matches!(unbracketed(value).kind, ExprKind::Call { .. })),
                "{name}: {:?}",
                body[0]
            );
        }
        let function = unit.functions.iter().find(|f| f.name == "loop").unwrap();
        let StmtKind::Block(body) = &function.body.kind else {
            panic!()
        };
        assert!(
            matches!(&body[0].kind, StmtKind::While { condition, .. } if matches!(&unbracketed(condition).kind, ExprKind::Binary { op, .. } if op=="&&"))
        );
    }
}

#[test]
fn grouped_products_and_array_bounds_remain_expressions() {
    fn unbracketed(expression: &Expr) -> &Expr {
        if let ExprKind::Bracketed(inner) = &expression.kind {
            unbracketed(inner)
        } else {
            expression
        }
    }
    let source = "typedef unsigned int I; typedef unsigned int U; int product(int n){return(n*(2+sizeof(int)));} int bounds(char*p,char*a){return p==&(a[60*1024-1]);} int cast_bounds(char*p,char*a){return ((char*)p>=&(a[0]))&&((char*)p<=&(a[(I)(60*1024)-1]));} int cast_product(int bits){return ((U)(84000000*0.000125/(U)((84000000/0xFFFF)*0.0005+1))+((bits*(U)(84000000*0.000125/(U)((84000000/0xFFFF)*0.0005+1)))/0xFFFF));}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        for (name, expected_operator) in [
            ("product", "*"),
            ("bounds", "=="),
            ("cast_bounds", "&&"),
            ("cast_product", "+"),
        ] {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!()
            };
            assert!(
                matches!(&body[0].kind, StmtKind::Return(Some(value)) if matches!(&unbracketed(value).kind, ExprKind::Binary { op, .. } if op == expected_operator)),
                "{name}: {:?}",
                body[0]
            );
        }
    }
}

#[test]
fn cast_statement_expressions_keep_their_inner_statements() {
    let source = "int direct(int x){return (int)({x++;x;});} int assigned(int x){x=(int)({if(x)x--;x;});return x;} int argument(int x){known((int)({if(x)x--;x;}));return x;} int pointer(int*x){int*p;p=(int*)({x++;x;});return *p;} int bare(int x){known({0});return x;} int bad_condition(int x){if(!({0}<x))x++;return x;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let body = |name| {
            let function = unit.functions.iter().find(|f| f.name == name).unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!()
            };
            body
        };
        assert!(
            matches!(&body("direct")[0].kind, StmtKind::Return(Some(Expr { kind: ExprKind::Cast { argument, .. }, .. })) if matches!(argument.kind, ExprKind::Statement(_)))
        );
        for name in ["assigned", "argument"] {
            assert!(
                matches!(body(name)[0].kind, StmtKind::Expression(_)),
                "{name}: {:?}",
                body(name)
            );
        }
        assert!(matches!(body("pointer")[1].kind, StmtKind::Expression(_)));
        if !cpp {
            assert!(matches!(body("bare")[0].kind, StmtKind::Problem));
        }
        assert!(
            matches!(&body("bad_condition")[0].kind, StmtKind::If { condition, .. } if matches!(condition.kind, ExprKind::Problem(_)))
        );
    }
}

#[test]
fn abstract_pointer_types_remain_distinct_from_grouped_calls() {
    for cpp in [false, true] {
        for type_id in [
            "int(*)(int)",
            "int(*)[4]",
            "int[sizeof(int)]",
            "int(*)(int n, char *p)",
            "int(*const)[60*1024-1]",
            "int(*(*)(int))[4]",
            "Unknown(*)(int)",
            "Unknown**const",
        ] {
            let source = format!(
                "int f(void*p){{return ({type_id})p;}} int missing(void){{return ({type_id});}}"
            );
            let unit = parse(&source, cpp);
            assert!(
                unit.diagnostics.is_empty(),
                "{type_id}: {:?}",
                unit.diagnostics
            );
            let function = unit.functions.iter().find(|f| f.name == "f").unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!()
            };
            assert!(
                matches!(&body[0].kind, StmtKind::Return(Some(value)) if matches!(value.kind, ExprKind::Cast { .. })),
                "{type_id}: {:?}",
                body[0]
            );
            let function = unit.functions.iter().find(|f| f.name == "missing").unwrap();
            let StmtKind::Block(body) = &function.body.kind else {
                panic!()
            };
            assert!(
                matches!(body[0].kind, StmtKind::Problem),
                "{type_id}: {:?}",
                body[0]
            );
        }
    }
    let unit = parse(
        "struct Box{}; int f(void*p){return (int(Box::*)(int))p;}",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind, StmtKind::Return(Some(value)) if matches!(value.kind, ExprKind::Cast { .. }))
    );
}

#[test]
fn cpp_invalid_condition_recovers_reference_function_declaration() {
    let source = "int f(int ap,int n){ if(n&&arbitrary(ap,long long)) return 1; return 2; }";
    let unit = parse(source, true);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert!(
        unit.functions
            .iter()
            .any(|f| f.name == "arbitrary" && f.return_type == "n&&")
    );
    let unit = parse(source, false);
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    assert!(!unit.functions.iter().any(|f| f.name == "arbitrary"));
    let StmtKind::Block(body) = &unit.functions[0].body.kind else {
        panic!()
    };
    assert!(
        matches!(&body[0].kind,StmtKind::If {condition,..} if matches!(condition.kind,ExprKind::Problem(_)))
    );
}

#[test]
fn lambda_methods_keep_tu_names_and_parent_context() {
    let unit = parse(
        "int outer(int x){auto f=[x](int y){auto g=[y](){return 2;};return x+y;};return f(x);} int next(int x){return [x](int y) mutable noexcept -> int {return x+y;}(x);}",
        true,
    );
    assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
    for (name, parent, fullname, signature) in [
        (
            "<lambda>0",
            "outer:int(int)",
            "outer:int(int).<lambda>0",
            "ANY(int)",
        ),
        (
            "<lambda>1",
            "outer:int(int).<lambda>0",
            "outer:int(int).<lambda>1",
            "ANY()",
        ),
        (
            "<lambda>2",
            "next:int(int)",
            "next:int(int).<lambda>2",
            "ANY(int)",
        ),
    ] {
        let function = unit.functions.iter().find(|f| f.name == name).unwrap();
        assert_eq!(function.lambda_parent.as_deref(), Some(parent));
        assert_eq!(function.full_name, fullname);
        let explicit = name == "<lambda>2";
        assert_eq!(function.return_type, if explicit { "int" } else { "ANY" });
        assert_eq!(
            function.signature,
            if explicit { "int(int)" } else { signature }
        );
    }
    let outer = unit.functions.iter().find(|f| f.name == "outer").unwrap();
    let StmtKind::Block(body) = &outer.body.kind else {
        panic!()
    };
    let StmtKind::Declaration(declarations) = &body[0].kind else {
        panic!()
    };
    assert!(
        matches!(&declarations[0].initializer.as_ref().unwrap().kind,ExprKind::Lambda(closure) if closure.parameter_types==["int"]&&closure.return_type=="int")
    );
}

#[test]
fn invalid_abstract_pointer_expression_preserves_valid_types_and_calls() {
    let source = "typedef int T; int f(int *p){p=(T*__madeup*)p; if((T*__madeup*)p) return 1; p=(T**)p; __ptr32(p); return 0;}";
    for cpp in [false, true] {
        let unit = parse(source, cpp);
        assert!(unit.diagnostics.is_empty(), "{:?}", unit.diagnostics);
        let StmtKind::Block(body) = &unit.functions[0].body.kind else {
            panic!()
        };
        assert!(matches!(body[0].kind, StmtKind::Problem));
        assert!(
            matches!(&body[1].kind,StmtKind::If {condition,..} if matches!(condition.kind,ExprKind::Problem(_)))
        );
        assert!(matches!(body[2].kind, StmtKind::Expression(_)));
        assert!(matches!(body[3].kind, StmtKind::Expression(_)));
    }
}
