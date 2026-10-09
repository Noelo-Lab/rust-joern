//! The C/C++ syntax used by the CDT-to-CPG conversion, without a parser runtime.
//! Unsupported executable syntax is reported, rather than silently dropping its CFG.
use std::collections::{HashMap, HashSet};

use crate::lexer::{
    RetainedMacroInvocation, Token, TokenKind, lex_preprocessed,
    lex_preprocessed_with_macros_for_language,
};
use crate::syntax::*;

#[cfg(test)]
pub fn parse(source: &str, cpp: bool) -> TranslationUnit {
    parse_preprocessed(source, cpp, false)
}

pub fn parse_preprocessed(source: &str, cpp: bool, preprocessed: bool) -> TranslationUnit {
    let (tokens, diagnostics, retained_macro_invocations) =
        lex_preprocessed_with_macros_for_language(source, preprocessed, cpp);
    let limit = tokens.len();
    let mut parser = Parser {
        source,
        tokens,
        pos: 0,
        limit,
        cpp,
        diagnostics,
        types: HashSet::new(),
        explicit_types: HashSet::new(),
        type_aliases: HashMap::new(),
        extern_c: false,
        extern_c_names: HashSet::new(),
        class_scopes: HashSet::new(),
        class_fields: HashMap::new(),
        function_fields: Vec::new(),
        function_member_cv: false,
        variables: HashSet::new(),
        variable_types: HashMap::new(),
        variable_closures: HashMap::new(),
        function_returns: HashMap::new(),
        function_full_name: String::new(),
        imports: HashMap::new(),
        using_namespaces: Vec::new(),
        lexical_scope: String::new(),
        asm_problem_recovery: false,
        in_function_body: false,
        function_declarations: Vec::new(),
        global_expressions: Vec::new(),
        macro_expansion: false,
        current_method: String::new(),
        lambda_counter: 0,
        defined_cpp_methods: HashSet::new(),
        lambda_ast_parent: String::new(),
        retained_macro_invocations,
        retained_macro_call_ends: HashSet::new(),
    };
    let mut functions = Vec::new();
    parser.translation_scope("", &mut functions, false);
    functions.append(&mut parser.function_declarations);
    let declaration_count = functions
        .iter()
        .filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.as_str())
        .collect::<HashSet<_>>()
        .len();
    // Joern collects declarations separately, and emits only those without a
    // matching definition after visiting every translation-unit declaration.
    let definitions: HashSet<_> = functions
        .iter()
        .filter(|f| !matches!(f.body.kind, StmtKind::Empty))
        .map(|f| f.full_name.clone())
        .collect();
    let mut declarations = HashSet::new();
    functions.retain(|f| {
        !matches!(f.body.kind, StmtKind::Empty)
            || !definitions.contains(&f.full_name) && declarations.insert(f.full_name.clone())
    });
    let declaration_names: Vec<_> = functions
        .iter()
        .filter(|function| matches!(function.body.kind, StmtKind::Empty))
        .map(|function| function.full_name.clone())
        .collect();
    let ranks = joern_declaration_order(&declaration_names, declaration_count);
    functions.sort_by_key(|f| {
        if matches!(f.body.kind, StmtKind::Empty) {
            (true, ranks[&f.full_name])
        } else {
            (false, 0)
        }
    });
    let retained_macro_calls = parser
        .retained_macro_invocations
        .into_iter()
        .filter(|event| parser.retained_macro_call_ends.contains(&event.callee_end))
        .map(|event| event.descriptor)
        .collect();
    TranslationUnit {
        functions,
        global_expressions: parser.global_expressions,
        retained_macro_calls,
        diagnostics: parser.diagnostics,
    }
}

fn joern_declaration_order(names: &[String], registered_count: usize) -> HashMap<String, usize> {
    // AstCreationPass copies its ConcurrentHashMap into Scala's immutable Map,
    // then removes definitions. Small Maps retain Java's bucket order; larger
    // Maps iterate the hash trie, visiting inline data before child nodes.
    fn java_hash(name: &str) -> u32 {
        name.encode_utf16().fold(0u32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(unit as u32)
        })
    }
    fn improved_hash(name: &str) -> u32 {
        let hash = java_hash(name);
        let hash = hash.wrapping_add(!(hash << 9));
        let hash = hash ^ (hash >> 14);
        let hash = hash.wrapping_add(hash << 4);
        hash ^ (hash >> 10)
    }
    fn trie_order<'a>(names: Vec<&'a String>, shift: u32, output: &mut Vec<&'a String>) {
        if shift >= 32 {
            output.extend(names);
            return;
        }
        let mut slots: [Vec<&String>; 32] = std::array::from_fn(|_| Vec::new());
        for name in names {
            slots[((improved_hash(name) >> shift) & 31) as usize].push(name);
        }
        output.extend(
            slots
                .iter()
                .filter(|slot| slot.len() == 1)
                .map(|slot| slot[0]),
        );
        for slot in slots.into_iter().filter(|slot| slot.len() > 1) {
            trie_order(slot, shift + 5, output);
        }
    }
    let mut order: Vec<_> = names.iter().collect();
    if registered_count <= 4 {
        order.sort_by_key(|name| {
            let hash = java_hash(name);
            (hash ^ (hash >> 16)) & 15
        });
    } else {
        let mut sorted = Vec::with_capacity(order.len());
        trie_order(order, 0, &mut sorted);
        order = sorted;
    }
    order
        .into_iter()
        .enumerate()
        .map(|(index, name)| (name.clone(), index))
        .collect()
}

#[cfg(test)]
mod expression_port_tests {
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
        let ExprKind::ArrayInitializer(values) =
            &declarations[0].initializer.as_ref().unwrap().kind
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
        let ExprKind::ArrayInitializer(values) =
            &declarations[0].initializer.as_ref().unwrap().kind
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
}

struct Parser<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    pos: usize,
    limit: usize,
    cpp: bool,
    diagnostics: Vec<ParseDiagnostic>,
    types: HashSet<String>,
    explicit_types: HashSet<String>,
    type_aliases: HashMap<String, String>,
    extern_c: bool,
    extern_c_names: HashSet<String>,
    class_scopes: HashSet<String>,
    class_fields: HashMap<String, Vec<(String, String)>>,
    function_fields: Vec<(String, String)>,
    function_member_cv: bool,
    variables: HashSet<String>,
    variable_types: HashMap<String, String>,
    variable_closures: HashMap<String, Closure>,
    function_returns: HashMap<String, String>,
    function_full_name: String,
    imports: HashMap<String, String>,
    using_namespaces: Vec<String>,
    lexical_scope: String,
    asm_problem_recovery: bool,
    in_function_body: bool,
    function_declarations: Vec<Function>,
    global_expressions: Vec<Expr>,
    macro_expansion: bool,
    current_method: String,
    lambda_counter: usize,
    defined_cpp_methods: HashSet<String>,
    lambda_ast_parent: String,
    retained_macro_invocations: Vec<RetainedMacroInvocation>,
    retained_macro_call_ends: HashSet<usize>,
}

struct Header {
    name: String,
    name_start: usize,
    parameters_start: usize,
    parameters_end: usize,
    return_type: Option<String>,
}

impl Parser<'_> {
    fn text(&self, i: usize) -> &str {
        self.tokens
            .get(i)
            .map_or("", |t| &self.source[t.span.start..t.span.end])
    }

    fn peek(&self) -> &str {
        if self.pos < self.limit {
            self.text(self.pos)
        } else {
            ""
        }
    }

    fn at(&self, text: &str) -> bool {
        self.peek() == text
    }

    fn eat(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, text: &str) {
        if !self.eat(text) {
            self.diagnose(
                self.pos,
                self.pos.saturating_add(1),
                format!("expected '{text}', found '{}'", self.peek()),
            );
        }
    }

    fn span(&self, start: usize, end: usize) -> Span {
        if let Some(first) = self.tokens.get(start) {
            if end > start
                && let Some(last) = self.tokens.get(end - 1)
            {
                return Span {
                    start: first.span.start,
                    end: last.span.end,
                    line: first.span.line,
                    column: first.span.column,
                    end_line: last.span.end_line,
                };
            }
            return Span {
                end: first.span.start,
                ..first.span.clone()
            };
        }
        self.tokens.last().map_or(Span::default(), |last| Span {
            start: self.source.len(),
            end: self.source.len(),
            line: last.span.end_line,
            column: last.span.column + last.span.end - last.span.start,
            end_line: last.span.end_line,
        })
    }

    fn raw(&self, start: usize, end: usize) -> String {
        let span = self.span(start, end);
        self.source[span.start..span.end].to_string()
    }

    fn diagnose(&mut self, start: usize, end: usize, message: impl Into<String>) {
        self.diagnostics.push(ParseDiagnostic {
            message: message.into(),
            span: self.span(start, end),
        });
    }

    fn matching(&self, start: usize, limit: usize) -> Option<usize> {
        let close = match self.text(start) {
            "(" => ")",
            "[" | "<:" => "]",
            "{" | "<%" => "}",
            _ => return None,
        };
        let mut stack = vec![close];
        for i in start + 1..limit {
            match self.text(i) {
                "(" => stack.push(")"),
                "[" | "<:" => stack.push("]"),
                "{" | "<%" => stack.push("}"),
                ")" | "]" | "}" | ":>" | "%>" => {
                    let t = match self.text(i) {
                        ":>" => "]",
                        "%>" => "}",
                        other => other,
                    };
                    if stack.last() != Some(&t) {
                        return None;
                    }
                    stack.pop();
                    if stack.is_empty() {
                        return Some(i);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn brace_end(&self, start: usize, limit: usize) -> Option<usize> {
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

    fn problem_initializer_end(&self, open: usize) -> Option<usize> {
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

    fn problem_declaration_end(&self, start: usize) -> usize {
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

    fn translation_scope(&mut self, prefix: &str, functions: &mut Vec<Function>, enclosed: bool) {
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
            let header = if is_typedef {
                None
            } else {
                self.function_header(start, cursor)
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
                        for (a, b) in self.split_ranges(base_end, cursor, ",") {
                            if let Some(mut header) = self.function_header(a, b) {
                                let declarator_type = header
                                    .return_type
                                    .take()
                                    .unwrap_or_else(|| self.clean_type(a, header.name_start));
                                header.return_type = Some(format!("{base_type}{declarator_type}"));
                                self.add_function(start, cursor, header, prefix, false, functions);
                            } else if let (Some(name), _) = self.declarator_name(a, b) {
                                self.variables.insert(self.text(name).to_string());
                            }
                        }
                    }
                } else {
                    self.remember_types(start, cursor);
                    if !is_typedef {
                        let base_end = self.specifier_end(start, cursor);
                        for (a, b) in self.split_ranges(base_end, cursor, ",") {
                            if let (Some(name), _) = self.declarator_name(a, b) {
                                self.variables.insert(self.text(name).to_string());
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
                        self.class_fields.insert(nested.clone(), fields);
                    }
                    let previous_linkage = self.extern_c;
                    let outer_variables = class.map(|_| self.variables.clone());
                    if self.cpp && linkage {
                        self.extern_c = self.text(start + 1) == "\"C\"";
                    }
                    self.pos = cursor + 1;
                    self.translation_scope(&nested, functions, true);
                    self.extern_c = previous_linkage;
                    if let Some(variables) = outer_variables {
                        self.variables = variables;
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
                                    self.variables.insert(self.text(name).to_string());
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

    fn record_retained_macro_call(&mut self, callee: &Expr, _arguments: &[Expr]) {
        if !self.retained_macro_invocations.is_empty()
            && matches!(callee.kind, ExprKind::Member { .. })
        {
            self.retained_macro_call_ends.insert(callee.span.end);
        }
    }

    fn declaration_end(&self, start: usize) -> usize {
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

    fn knr_body_start(&self, header: &Header) -> Option<usize> {
        let parameters = self.split_ranges(header.parameters_start, header.parameters_end, ",");
        if parameters.is_empty()
            || parameters.iter().any(|&(a, b)| {
                b != a + 1
                    || self.tokens[a].kind != TokenKind::Identifier
                    || type_word(self.text(a))
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

    fn record_definition(&self, start: usize, end: usize) -> Option<usize> {
        if (start..end).any(|i| self.text(i) == "=") {
            return None;
        }
        let introducer =
            (start..end).find(|i| matches!(self.text(*i), "struct" | "class" | "union"))?;
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

    fn function_header(&self, start: usize, end: usize) -> Option<Header> {
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
                || type_word(self.text(name_start))
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
            if self.split_ranges(i + 1, close, ",").iter().any(|(a, b)| {
                a < b
                    && (self.tokens[*a].kind == TokenKind::Literal
                        || matches!(
                            self.text(*a),
                            "{" | "+" | "-" | "!" | "~" | "*" | "&" | "&&"
                        ))
            }) {
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
            });
        }
        None
    }

    fn grouped_function_header(&self, start: usize, end: usize) -> Option<Header> {
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
        let mut header = if let Some(header) = self.function_header(group + 1, close) {
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

    fn collect_class_fields(&self, start: usize, end: usize) -> Vec<(String, String)> {
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

    fn add_function(
        &mut self,
        start: usize,
        body_start: usize,
        header: Header,
        prefix: &str,
        definition: bool,
        functions: &mut Vec<Function>,
    ) {
        if self.cdt_problem_declaration(start, body_start)
            || self.cdt_problem_record_header(start, body_start)
            || self.cdt_problem_return_prefix(start, &header)
            || self.cdt_problem_parameters(&header)
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
                && let Some(mut recovered) = self.function_header(marker, body_start)
            {
                recovered.return_type = Some("ANY".into());
                self.add_function(marker, body_start, recovered, prefix, true, functions);
            } else {
                self.pos = body_end.map_or(self.limit, |close| close + 1);
            }
            return;
        }
        let (return_start, recovered_return_type) =
            self.return_specifier_start(start, header.name_start);
        let base_end = self.specifier_end(return_start, header.name_start);
        let primitive_return = (return_start..base_end).any(|i| type_word(self.text(i)));
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
            let constructor = owner.as_ref().is_some_and(|owner| {
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
            } else {
                return_binding_type.clone()
            }
        } else if definition {
            let base = return_type.trim_end_matches(['*', '&']).trim();
            if base.split_whitespace().all(type_word) {
                primitive_return_display(&self.type_code(return_start, header.name_start, true))
            } else if self.explicit_types.contains(base)
                || self.class_scopes.contains(base)
                || base.contains("::")
            {
                simple_type_name(base).to_string()
            } else {
                return_type
            }
        } else {
            return_type
        };
        let old_variables = self.variables.clone();
        let old_variable_types = self.variable_types.clone();
        let old_variable_closures = self.variable_closures.clone();
        let old_function_full_name = self.function_full_name.clone();
        let old_function_fields = self.function_fields.clone();
        let old_function_member_cv = self.function_member_cv;
        let old_body_context = self.in_function_body;
        self.in_function_body = false;
        let parameters = if definition
            && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ";")
        {
            let mut declared = Vec::new();
            for (a, b) in self.split_ranges(header.parameters_end + 1, body_start, ";") {
                if a < b {
                    let span = self.span(a, b + 1);
                    declared.extend(
                        self.declarations_range(a, b)
                            .into_iter()
                            .map(|d| (d, span.clone())),
                    );
                }
            }
            self.split_ranges(header.parameters_start, header.parameters_end, ",")
                .into_iter()
                .filter(|(a, b)| a < b)
                .map(|(a, b)| {
                    let name = self.raw(a, b);
                    let declaration = declared.iter().find(|(d, _)| d.name == name);
                    let type_name = declaration.map_or("int".into(), |(d, _)| d.type_name.clone());
                    Parameter {
                        name,
                        type_name,
                        function_pointer: false,
                        span: declaration.map_or_else(|| self.span(a, b), |(_, span)| span.clone()),
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
                            && !type_word(self.text(a))
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
                    span: self.span(a, b),
                })
                .collect()
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
            self.variables.insert(parameter.name.clone());
            self.variable_types
                .insert(parameter.name.clone(), parameter.type_name.clone());
        }
        self.variables.insert(header.name.clone());
        if definition && (header.parameters_end + 1..body_start).any(|i| self.text(i) == ":") {
            self.diagnose(
                header.parameters_end + 1,
                body_start,
                "constructor member-initializer CFG is unsupported",
            );
        }
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
            self.variables.insert("this".into());
            self.variable_types.insert("this".into(), owner.clone());
        }
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
        if self.asm_problem_recovery
            && let Some(body_end) = original_body_end
        {
            // CDT closes the enclosing compound at a problem assembly brace.
            // The balanced source body still delimits recovery of later methods.
            self.asm_tail_declarations(end, body_end);
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
        self.variables.insert(header.name.clone());
        let mut span = self.span(start, end);
        if !definition {
            let name_span = self.span(header.name_start, header.name_start + 1);
            span.line = name_span.line;
            span.column = name_span.column;
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
            inherited_bindings: Vec::new(),
            inherited_closures: Vec::new(),
            parameters,
            body,
            span,
        });
    }

    fn return_specifier_start(&self, start: usize, name_start: usize) -> (usize, bool) {
        let mut cursor = start;
        if self.text(cursor) == "template"
            && self.text(cursor + 1) == "<"
            && let Some(close) = self.angle_close(cursor + 1, name_start)
        {
            cursor = close + 1;
        }
        let original = cursor;
        while cursor + 1 < name_start
            && self.tokens[cursor].kind == TokenKind::Identifier
            && self.tokens[cursor + 1].kind == TokenKind::Identifier
            && !self.explicit_types.contains(self.text(cursor))
            && !type_word(self.text(cursor))
            && !qualifier(self.text(cursor))
            && !type_word(self.text(cursor + 1))
            && !qualifier(self.text(cursor + 1))
        {
            // Leading unbound specifiers become separate CDT problem nodes.
            cursor += 1;
        }
        (cursor, cursor != original)
    }

    fn cdt_problem_parameters(&self, header: &Header) -> bool {
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
                    && !type_word(specifier)
                    && !qualifier(specifier)
                    && !self.explicit_types.contains(specifier)
                    && !matches!(specifier, "typeof" | "__typeof__" | "__typeof" | "_Atomic")
            })
    }

    fn cdt_problem_return_prefix(&self, start: usize, header: &Header) -> bool {
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

    fn unknown_function_suffix(&self, start: usize, end: usize) -> Option<usize> {
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

    fn cdt_problem_declaration(&self, start: usize, end: usize) -> bool {
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

    fn cdt_problem_record_header(&self, start: usize, end: usize) -> bool {
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

    fn cdt_problem_function_header(&self, start: usize, end: usize) -> bool {
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
            && !type_word(self.text(start))
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

    fn asm_tail_declarations(&mut self, start: usize, end: usize) {
        if self.cpp {
            return;
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
                && !type_word(self.text(start))
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
                            && !type_word(self.text(a))
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
                            span: self.span(a, b),
                        })
                        .collect();
                    let name = self.text(start).to_string();
                    self.variables.insert(name.clone());
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
                        inherited_bindings: Vec::new(),
                        inherited_closures: Vec::new(),
                        parameters,
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
            self.pos = self.problem_statement_end(start).max(start + 1);
        }
        self.limit = saved_limit;
    }

    fn collect_global_expressions(&mut self, start: usize, end: usize) {
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

    fn remember_types(&mut self, start: usize, end: usize) {
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

    fn parameters(&mut self, start: usize, end: usize) -> Vec<Parameter> {
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
                && !type_word(self.text(b - 1))
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
            result.push(Parameter {
                name,
                type_name,
                function_pointer: self.text(suffix) == "(",
                span: self.span(a, b),
            });
        }
        result
    }

    fn binding_parameter_type(&self, start: usize, end: usize) -> String {
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

    fn binding_type(&self, raw_type: &str) -> String {
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
        if !base.split_whitespace().all(type_word)
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

    fn resolve_class_name(&self, name: &str) -> Option<String> {
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

    fn angle_close(&self, start: usize, end: usize) -> Option<usize> {
        let mut depth = 0;
        for i in start..end {
            match self.text(i) {
                "<" => depth += 1,
                ">" => depth -= 1,
                ">>" => depth -= 2,
                _ => {}
            }
            if depth <= 0 {
                return Some(i);
            }
        }
        None
    }

    fn split_ranges(&self, start: usize, end: usize, separator: &str) -> Vec<(usize, usize)> {
        let mut result = Vec::new();
        let (mut from, mut i) = (start, start);
        while i < end {
            if matches!(self.text(i), "(" | "[" | "{") {
                i = self.matching(i, end).map_or(i + 1, |n| n + 1);
            } else if self.cpp
                && self.text(i) == "<"
                && i > start
                && (!self.variables.contains(self.text(i - 1))
                    || (self.function_returns.contains_key(self.text(i - 1))
                        && !self.variable_types.contains_key(self.text(i - 1))))
                && (self.types.contains(self.text(i - 1))
                    || self.text(i - 1) == "template"
                    || (i >= 2 && self.text(i - 2) == "::"))
            {
                i = self.angle_close(i, end).map_or(i + 1, |n| n + 1);
            } else if self.text(i) == separator {
                result.push((from, i));
                from = i + 1;
                i += 1;
            } else {
                i += 1;
            }
        }
        result.push((from, end));
        result
    }

    fn skip_attribute(&self, i: usize, end: usize) -> Option<usize> {
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

    fn specifier_end(&self, start: usize, end: usize) -> usize {
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
            if matches!(t, "struct" | "union" | "enum" | "class") {
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
            if type_word(t) {
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

    fn declarator_name(&self, start: usize, end: usize) -> (Option<usize>, usize) {
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

    fn nested_declarator(&self, start: usize, end: usize) -> bool {
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

    fn clean_type(&self, start: usize, end: usize) -> String {
        self.type_code(start, end, false)
    }

    fn type_code(&self, start: usize, end: usize, keep_qualifiers: bool) -> String {
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

    fn declarator_type(
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

    fn declarations_range(&mut self, start: usize, end: usize) -> Vec<Declaration> {
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
                    self.variables.insert(name.clone());
                    self.variable_types.insert(name.clone(), type_name.clone());
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
                self.variables.insert(name.clone());
                self.variable_types.insert(name.clone(), type_name.clone());
                if let Some(closure) = initializer
                    .as_ref()
                    .and_then(|value| self.closure_expression(value))
                    .cloned()
                {
                    self.variable_closures.insert(name.clone(), closure);
                } else {
                    self.variable_closures.remove(&name);
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
                span: self.span(a, b),
            });
        }
        result
    }

    fn local_problem_enum_statement(&mut self, start: usize) -> Option<Stmt> {
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

    fn nested_function_statement(&mut self, start: usize) -> Option<Stmt> {
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

    fn grouped_declarator_problem(&self, start: usize, end: usize) -> bool {
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

    fn declaration_shape_problem(&self, start: usize, end: usize) -> bool {
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

    fn looks_declaration(&self, start: usize, end: usize) -> bool {
        let t = self.text(start);
        if start < end
            && self.tokens[start].kind == TokenKind::Identifier
            && !type_word(t)
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
                if (self.variables.contains(t) && !matches!(self.text(after), "=" | "{"))
                    || (after == end && self.variable_types.contains_key(self.text(name)))
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
        if !type_word(t)
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
            && after == close
            && !type_word(self.text(name))
            && !qualifier(self.text(name))
        {
            if self.text(close + 1) == "=" {
                // C resolves T(x)=value as a declaration even when T binds
                // to a function. C++ instead chooses a call expression when
                // the grouped name already has an object binding.
                return type_word(t)
                    || qualifier(t)
                    || !self.cpp
                    || !self.variables.contains(self.text(name));
            }
            if close + 1 == end && !type_word(t) && !qualifier(t) {
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
            && !type_word(t)
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
            let mut after = start + 1;
            while after + 1 < end && self.text(after) == "::" {
                after += 2;
            }
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
            || type_word(t)
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

    /// C statements shaped `T (*x)(args)...;` with an identifier `T` are
    /// ambiguous. CDT resolves them by `T`'s binding: without a typedef they
    /// are call expressions, and with one they stay declarations unless
    /// consecutive parameter lists declare a function returning a function.
    /// Groups that cannot be call arguments, such as type keywords or
    /// adjacent names, keep the declaration.
    fn grouped_pointer_call_declaration(&self, start: usize, end: usize) -> Option<bool> {
        let t = self.text(start);
        if self.cpp
            || self.tokens[start].kind != TokenKind::Identifier
            || type_word(t)
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
            let group_end = self.matching(open, end)?;
            if self.text(open) == "(" {
                let adjacent_names = (open + 1..group_end - 1).any(|k| {
                    self.tokens[k].kind == TokenKind::Identifier
                        && self.tokens[k + 1].kind == TokenKind::Identifier
                });
                if adjacent_names
                    || (open + 1..group_end).any(|k| {
                        let text = self.text(k);
                        text == "..." || type_word(text) || qualifier(text)
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

    fn statement(&mut self) -> Stmt {
        let start = self.pos;
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

    fn statement_end(&self, start: usize) -> usize {
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

    fn unclosed_literal_expression_end(
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

    fn c_literal_call_crosses_brace(&self, start: usize, end: usize) -> bool {
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

    fn unclosed_literal_call(&self, start: usize, end: usize) -> Option<(usize, usize)> {
        (start + 1..end).find_map(|open| {
            (self.text(open) == "("
                && self.tokens[open - 1].kind == TokenKind::Identifier
                && !type_word(self.text(open - 1))
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

    fn unclosed_conditional_literal(&self, start: usize, end: usize) -> bool {
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

    fn finish_statement(&mut self, start: usize) {
        if !self.eat(";") {
            self.diagnose(start, self.pos, "missing semicolon after statement");
            while self.pos < self.limit && !matches!(self.peek(), ";" | "}") {
                self.pos += 1;
            }
            self.eat(";");
        }
    }

    fn unclosed_condition_boundary(&self, open: usize) -> Option<usize> {
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

    fn condition(&mut self) -> Expr {
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

    fn for_statement(&mut self, start: usize) -> Stmt {
        self.pos += 1;
        self.expect("(");
        let inside = self.pos;
        let end = self
            .matching(inside.saturating_sub(1), self.limit)
            .unwrap_or(self.limit);
        let clauses = self.split_ranges(inside, end, ";");
        if clauses.len() == 1 && self.cpp {
            let range = self.split_ranges(inside, end, ":");
            if range.len() == 2 {
                let mut declarations = self.declarations_range(range[0].0, range[0].1);
                let declaration = if declarations.is_empty() {
                    self.diagnose(inside, end, "cannot recover range-for declaration");
                    Declaration {
                        name: "<unknown>".into(),
                        type_name: "ANY".into(),
                        initializer: None,
                        dimensions: Vec::new(),
                        problem: false,
                        is_typedef: false,
                        nested_declarator: false,
                        span: self.span(range[0].0, range[0].1),
                    }
                } else {
                    declarations.remove(0)
                };
                let iterable = self.expression_range(range[1].0, range[1].1);
                self.pos = end;
                self.expect(")");
                let body = Box::new(self.statement());
                return Stmt {
                    kind: StmtKind::RangeFor {
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
        let initializer = clauses.first().filter(|(a, b)| a < b).map(|(a, b)| {
            let kind = if self.looks_declaration(*a, *b) {
                StmtKind::Declaration(self.declarations_range(*a, *b))
            } else {
                StmtKind::Expression(self.expression_range(*a, *b))
            };
            Box::new(Stmt {
                kind,
                span: self.span(*a, *b),
            })
        });
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

    fn expression_range(&mut self, start: usize, end: usize) -> Expr {
        if self.c_qualification_problem(start, end)
            || self.ordinary_call_problem(start, end)
            || self.unpaired_expression_colon(start, end).is_some()
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

    fn detached_identifier_suffix(&self, start: usize, end: usize) -> Option<usize> {
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

    fn recovered_suffix_statement(&mut self, start: usize, end: usize) -> Stmt {
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

    fn initializer_expression_range(&mut self, start: usize, end: usize) -> Expr {
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

    fn stray_postfix_operator(&self, start: usize, end: usize) -> Option<usize> {
        (end > start + 1
            && matches!(self.text(end - 1), "!" | "~")
            && (matches!(
                self.tokens[end - 2].kind,
                TokenKind::Identifier | TokenKind::Literal
            ) || matches!(self.text(end - 2), ")" | "]"))
            && !self.cast_group_ends_at(start, end - 2, end))
        .then_some(end - 1)
    }

    fn cast_group_ends_at(&self, start: usize, mut close: usize, end: usize) -> bool {
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

    fn unmatched_expression_close(&self, start: usize, end: usize) -> Option<usize> {
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

    fn macro_problem_origin(&self, start: usize, end: usize) -> bool {
        self.tokens[start..(end + 1).min(self.tokens.len())]
            .iter()
            .any(|token| token.macro_recovery || token.macro_expansion)
    }

    fn problem_statement(&self, start: usize, from_macro: bool) -> Stmt {
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

    fn reparse_macro_declaration(&mut self, start: usize, end: usize) -> Stmt {
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

    fn unpaired_expression_colon(&self, start: usize, end: usize) -> Option<usize> {
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

    fn c_qualification_problem(&self, start: usize, end: usize) -> bool {
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

    fn template_id_end(&self, start: usize, end: usize) -> bool {
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

    fn direct_call_brace(&self, start: usize, end: usize) -> Option<usize> {
        if self.tokens.get(start)?.kind != TokenKind::Identifier || self.text(start + 1) != "(" {
            return None;
        }
        let close = self.matching(start + 1, end)?;
        (self.text(close + 1) == "{").then_some(close + 1)
    }

    fn call_statement_separator(&self, start: usize) -> Option<usize> {
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
            || type_word(text)
            || qualifier(text)
            || (self
                .tokens
                .get(next)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && self.looks_declaration(next, self.statement_end(next)))
            || matches!(text, "++" | "--"))
        .then_some(next)
    }

    fn terminal_expression_statement(&mut self, start: usize, end: usize) -> Option<Stmt> {
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
            && !type_word(self.text(start))
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

    fn call_brace(&self, start: usize, end: usize) -> Option<(usize, usize)> {
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

    /// CDT's postfix grammar calls any primary expression, including the
    /// numeric literals decompilers emit for unresolved absolute call targets.
    fn numeric_callee(&self, i: usize) -> bool {
        self.tokens[i].kind == TokenKind::Literal
            && self.text(i).starts_with(|c: char| c.is_ascii_digit())
    }

    fn ordinary_call_problem(&self, start: usize, end: usize) -> bool {
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
            if expression_type_keyword(self.text(i), self.cpp)
                && i > start
                && matches!(
                    self.text(i - 1),
                    "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>="
                )
                && !(self.cpp && type_word(self.text(i)) && matches!(self.text(i + 1), "(" | "{"))
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
                        && (type_word(self.text(i + 1))
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

    fn operator_brace(&self, start: usize, end: usize) -> Option<usize> {
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

    fn lambda_end(&self, start: usize, end: usize) -> Option<usize> {
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

    fn call_argument_problem(&self, mut start: usize, mut end: usize) -> bool {
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
            if !(self.cpp && type_word(first) && matches!(self.text(start + 1), "(" | "{")) {
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

    fn declaration_expression_problem(&self, start: usize, end: usize) -> bool {
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

    fn c_attribute_problem(&self, start: usize, end: usize) -> bool {
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
    fn stray_closer_problem_end(&self, start: usize) -> usize {
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

    fn problem_statement_end(&self, start: usize) -> usize {
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

    fn expression(&mut self, minimum: u8) -> Expr {
        let start = self.pos;
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
                && matches!(&left.kind, ExprKind::Identifier(name) if self.explicit_types.contains(name))
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
                if matches!(left.kind, ExprKind::Identifier(_))
                    && self.span(start, start + 1).start < left.span.start
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
                let name = self.peek().to_string();
                if !name.is_empty() {
                    self.pos += 1;
                }
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

    fn builtin_macro_call(&mut self, name: String, start: usize) -> Option<Expr> {
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

    fn expanded_macro(
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
            diagnostics,
            types: self.types.clone(),
            explicit_types: self.explicit_types.clone(),
            type_aliases: self.type_aliases.clone(),
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
            function_full_name: self.function_full_name.clone(),
            using_namespaces: self.using_namespaces.clone(),
            lexical_scope: self.lexical_scope.clone(),
            class_fields: self.class_fields.clone(),
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

    fn closure_expression<'a>(&'a self, expression: &'a Expr) -> Option<&'a Closure> {
        closure_expression(expression, &self.variable_closures)
    }

    fn lambda_expression(&mut self) -> Expr {
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
            self.variables.insert(parameter.name.clone());
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
        let mut types = self.variable_types.clone();
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
            &mut self.variable_closures.clone(),
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
            inherited_bindings,
            inherited_closures,
            parameters,
            body,
            span: self.span(start, self.pos),
        });
        Expr {
            kind: ExprKind::Lambda(closure),
            span: self.span(start, self.pos),
        }
    }

    fn prefix(&mut self) -> Expr {
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
                self.pos += 2;
                while self.eat("::") {
                    self.eat("template");
                    if self.pos < self.limit {
                        self.pos += 1;
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
                        while self.eat("::") {
                            self.eat("template");
                            if self.pos < self.limit {
                                self.pos += 1;
                            }
                        }
                        // Template calls: include type arguments in the callee name.
                        if self.cpp
                            && self.at("<")
                            && let Some(close) = self.angle_close(self.pos, self.limit)
                            && (self.text(close + 1) == "(" || self.text(close + 1) == "::")
                        {
                            self.pos = close + 1;
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

    fn can_start_expression(&self, i: usize) -> bool {
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

    fn is_cast(&self, start: usize, end: usize) -> bool {
        let next = self.text(end + 1);
        if end == start + 1
            && self.tokens[start].kind == TokenKind::Identifier
            && !type_word(self.text(start))
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
        if type_word(first)
            || qualifier(first)
            || self.explicit_types.contains(first)
            || matches!(first, "struct" | "union" | "enum" | "class")
            || (start..end).any(|i| self.text(i) == "*")
        {
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

    fn is_type_range(&self, start: usize, end: usize) -> bool {
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
        let known = type_word(first)
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

    fn abstract_type_declarator(&self, start: usize, end: usize) -> Option<bool> {
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

    fn abstract_type_parameters(&self, start: usize, end: usize) -> bool {
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

    fn remember_inferred_type(&mut self, start: usize, end: usize) {
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
                && !type_word(self.text(i))
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

fn closure_expression<'a>(
    expression: &'a Expr,
    closures: &'a HashMap<String, Closure>,
) -> Option<&'a Closure> {
    match &expression.kind {
        ExprKind::Lambda(closure) => Some(closure),
        ExprKind::Identifier(name) => closures.get(name),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => closure_expression(inner, closures),
        ExprKind::Unary { op, argument, .. } if matches!(op.as_str(), "*" | "&") => {
            closure_expression(argument, closures)
        }
        _ => None,
    }
}

fn common_type(left: String, right: String) -> String {
    if left == right {
        return left;
    }
    if left == "ANY" || right == "ANY" {
        return "ANY".into();
    }
    for ty in [
        "long double",
        "double",
        "float",
        "unsigned long long",
        "long long",
        "unsigned long",
        "long",
        "unsigned int",
        "int",
    ] {
        if left == ty || right == ty {
            return ty.into();
        }
    }
    left
}

fn infer_value_type(
    expression: &Expr,
    types: &HashMap<String, String>,
    closures: &HashMap<String, Closure>,
    functions: &HashMap<String, String>,
) -> String {
    match &expression.kind {
        ExprKind::Identifier(name) => types
            .get(name)
            .map(|ty| ty.trim_end_matches('&').to_string())
            .unwrap_or_else(|| "ANY".into()),
        ExprKind::Literal(value) => {
            if matches!(value.as_str(), "true" | "false") {
                "bool".into()
            } else if value.starts_with('"') {
                "char*".into()
            } else if value.starts_with('\'') {
                "char".into()
            } else if value.contains('.')
                || (!value.starts_with("0x") && value.to_lowercase().contains('e'))
            {
                if value.ends_with(['f', 'F']) {
                    "float".into()
                } else {
                    "double".into()
                }
            } else if value.ends_with(['l', 'L']) {
                "long".into()
            } else if value.ends_with(['u', 'U']) {
                "unsigned int".into()
            } else {
                "int".into()
            }
        }
        ExprKind::Cast { type_name, .. } => type_name.clone(),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => infer_value_type(inner, types, closures, functions),
        ExprKind::Unary { op, argument, .. } => {
            let ty = infer_value_type(argument, types, closures, functions);
            match op.as_str() {
                "!" => "bool".into(),
                "*" => ty.strip_suffix('*').unwrap_or("ANY").into(),
                "&" => format!("{ty}*"),
                "sizeof" | "alignof" => "unsigned long".into(),
                _ => ty,
            }
        }
        ExprKind::Binary { op, left, right } => {
            let left = infer_value_type(left, types, closures, functions);
            let right = infer_value_type(right, types, closures, functions);
            match op.as_str() {
                "==" | "!=" | "<" | ">" | "<=" | ">=" | "&&" | "||" => "bool".into(),
                "," => right,
                "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=" => left,
                "+" | "-" if left.ends_with('*') => left,
                _ => common_type(left, right),
            }
        }
        ExprKind::Conditional {
            consequence,
            alternative,
            ..
        } => common_type(
            infer_value_type(consequence, types, closures, functions),
            infer_value_type(alternative, types, closures, functions),
        ),
        ExprKind::Call { callee, .. } => {
            if let Some(closure) = closure_expression(callee, closures) {
                closure.return_type.clone()
            } else if let ExprKind::Identifier(name) = &callee.kind {
                functions
                    .get(name.rsplit("::").next().unwrap())
                    .cloned()
                    .unwrap_or_else(|| "ANY".into())
            } else {
                "ANY".into()
            }
        }
        ExprKind::Index { base, .. } => {
            let ty = infer_value_type(base, types, closures, functions);
            ty.strip_suffix('*')
                .or_else(|| ty.split_once('[').map(|(ty, _)| ty))
                .unwrap_or("ANY")
                .into()
        }
        ExprKind::List(expressions) | ExprKind::Block(expressions) => expressions
            .last()
            .map(|value| infer_value_type(value, types, closures, functions))
            .unwrap_or_else(|| "void".into()),
        _ => "ANY".into(),
    }
}

fn infer_return_types(
    statement: &Stmt,
    types: &mut HashMap<String, String>,
    closures: &mut HashMap<String, Closure>,
    functions: &HashMap<String, String>,
    returns: &mut Vec<String>,
) {
    match &statement.kind {
        StmtKind::Block(statements) => {
            let mut types = types.clone();
            let mut closures = closures.clone();
            for statement in statements {
                infer_return_types(statement, &mut types, &mut closures, functions, returns);
            }
        }
        StmtKind::Sequence(statements) => {
            for statement in statements {
                infer_return_types(statement, types, closures, functions, returns);
            }
        }
        StmtKind::Declaration(declarations) => {
            for declaration in declarations {
                let ty = if declaration.type_name.starts_with("auto") {
                    declaration
                        .initializer
                        .as_ref()
                        .map(|value| infer_value_type(value, types, closures, functions))
                        .unwrap_or_else(|| "ANY".into())
                } else {
                    declaration.type_name.clone()
                };
                if let Some(closure) = declaration
                    .initializer
                    .as_ref()
                    .and_then(|value| closure_expression(value, closures))
                    .cloned()
                {
                    closures.insert(declaration.name.clone(), closure);
                }
                types.insert(declaration.name.clone(), ty);
            }
        }
        StmtKind::Return(value) => returns.push(
            value
                .as_ref()
                .map(|value| infer_value_type(value, types, closures, functions))
                .unwrap_or_else(|| "void".into()),
        ),
        StmtKind::If {
            consequence,
            alternative,
            ..
        } => {
            infer_return_types(
                consequence,
                &mut types.clone(),
                &mut closures.clone(),
                functions,
                returns,
            );
            if let Some(alternative) = alternative {
                infer_return_types(
                    alternative,
                    &mut types.clone(),
                    &mut closures.clone(),
                    functions,
                    returns,
                );
            }
        }
        StmtKind::While { body, .. }
        | StmtKind::DoWhile { body, .. }
        | StmtKind::Switch { body, .. }
        | StmtKind::For { body, .. }
        | StmtKind::RangeFor { body, .. }
        | StmtKind::Label {
            statement: body, ..
        } => infer_return_types(
            body,
            &mut types.clone(),
            &mut closures.clone(),
            functions,
            returns,
        ),
        StmtKind::Try { body, catches } => {
            infer_return_types(
                body,
                &mut types.clone(),
                &mut closures.clone(),
                functions,
                returns,
            );
            for catch in catches {
                infer_return_types(
                    catch,
                    &mut types.clone(),
                    &mut closures.clone(),
                    functions,
                    returns,
                );
            }
        }
        _ => (),
    }
}

// CDT uses a formatted AST signature for expanded syntax, rather than the
// original macro call's source. This also governs which original arguments
// MacroHandler can find and clone in the expansion AST.
fn expanded_code(expression: &Expr, source: &str) -> String {
    let code = |e: &Expr| {
        if let ExprKind::Bracketed(inner) = &e.kind {
            format!("({})", expanded_code(inner, source))
        } else {
            expanded_code(e, source)
        }
    };
    match &expression.kind {
        ExprKind::Identifier(name) | ExprKind::Literal(name) => name.clone(),
        ExprKind::TypeSpecifier { code, .. } => code.clone(),
        ExprKind::Binary { op, left, right } => format!("{} {op} {}", code(left), code(right)),
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => format!(
            "{} ? {} : {}",
            code(condition),
            code(consequence),
            code(alternative)
        ),
        ExprKind::Cast {
            type_name,
            argument,
        } => {
            let raw = &source[expression.span.start..expression.span.end];
            let cast = [
                "static_cast",
                "reinterpret_cast",
                "const_cast",
                "dynamic_cast",
            ]
            .into_iter()
            .find(|cast| raw.starts_with(cast));
            if let Some(cast) = cast {
                format!("{cast}<{}>({})", macro_type_code(type_name), code(argument))
            } else {
                format!("({}){}", macro_type_code(type_name), code(argument))
            }
        }
        ExprKind::Unary {
            op,
            argument,
            postfix,
        } => {
            if *postfix {
                format!("{}{op}", code(argument))
            } else if matches!(
                op.as_str(),
                "sizeof"
                    | "alignof"
                    | "_Alignof"
                    | "__alignof__"
                    | "__alignof"
                    | "typeof"
                    | "__typeof__"
            ) {
                let raw = &source[expression.span.start..expression.span.end];
                // ASTSignatureUtil supplies the parentheses for a type-id;
                // an expression operand already retains its bracketed syntax.
                if !matches!(argument.kind, ExprKind::TypeSpecifier { .. }) {
                    return format!("{op} {}", code(argument));
                }
                let value = if matches!(&argument.kind,ExprKind::TypeSpecifier {code,..} if code=="typeof"||code=="__typeof__"||code=="decltype")
                {
                    code(argument)
                } else if let Some(inner) = raw
                    .strip_prefix(op)
                    .and_then(|s| s.trim().strip_prefix('('))
                    .and_then(|s| s.strip_suffix(')'))
                {
                    macro_type_code(inner)
                } else {
                    code(argument)
                };
                format!("{op} ({value})")
            } else {
                format!("{op}{}", code(argument))
            }
        }
        ExprKind::Call { callee, arguments } => format!(
            "{}({})",
            code(callee),
            arguments.iter().map(code).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Bracketed(inner) => expanded_code(inner, source),
        ExprKind::Generated { code, .. } => code.clone(),
        ExprKind::Member {
            base,
            name,
            indirect,
        } => format!("{}{}{name}", code(base), if *indirect { "->" } else { "." }),
        ExprKind::Index { base, index } => format!("{}[{}]", code(base), code(index)),
        ExprKind::List(values) | ExprKind::Block(values) => {
            values.iter().map(code).collect::<Vec<_>>().join(", ")
        }
        ExprKind::ArrayInitializer(values) => format!(
            "{{{}}}",
            values.iter().map(code).collect::<Vec<_>>().join(", ")
        ),
        _ => source[expression.span.start..expression.span.end].into(),
    }
}

fn macro_type_code(type_name: &str) -> String {
    let type_name = type_name.trim();
    let type_name = type_name
        .strip_prefix("struct ")
        .or_else(|| type_name.strip_prefix("union "))
        .or_else(|| type_name.strip_prefix("enum "))
        .unwrap_or(type_name);
    type_name
        .replace('*', " *")
        .replace('&', " &")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn expanded_argument(expression: &Expr, source: &str, argument: &str) -> Option<Expr> {
    if expanded_code(expression, source) == argument {
        return Some(expression.clone());
    }
    let children: Vec<&Expr> = match &expression.kind {
        ExprKind::Unary { argument, .. }
        | ExprKind::Cast { argument, .. }
        | ExprKind::Bracketed(argument)
        | ExprKind::Generated {
            expression: argument,
            ..
        } => vec![argument],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => vec![condition, consequence, alternative],
        // Static call names and FIELD_IDENTIFIERs have no expression AST.
        ExprKind::Call { arguments, .. } => arguments.iter().collect(),
        ExprKind::Member { base, .. } => vec![base],
        ExprKind::Index { base, index } => vec![base, index],
        ExprKind::List(values) | ExprKind::Block(values) | ExprKind::ArrayInitializer(values) => {
            values.iter().collect()
        }
        _ => Vec::new(),
    };
    children
        .into_iter()
        .find_map(|child| expanded_argument(child, source, argument))
}

fn generated_expression(expression: &mut Expr, source: &str, span: &Span) {
    let code = expanded_code(expression, source);
    match &mut expression.kind {
        ExprKind::Unary { argument, .. }
        | ExprKind::Bracketed(argument)
        | ExprKind::Generated {
            expression: argument,
            ..
        } => generated_expression(argument, source, span),
        ExprKind::Cast {
            argument,
            type_name,
        } => {
            *type_name = macro_type_code(type_name);
            generated_expression(argument, source, span);
        }
        ExprKind::Binary { left, right, .. } => {
            generated_expression(left, source, span);
            generated_expression(right, source, span);
        }
        ExprKind::Conditional {
            condition,
            consequence,
            alternative,
        } => {
            generated_expression(condition, source, span);
            generated_expression(consequence, source, span);
            generated_expression(alternative, source, span);
        }
        ExprKind::Call { callee, arguments } => {
            if matches!(callee.kind, ExprKind::Identifier(_)) {
                callee.span = span.clone();
            } else {
                generated_expression(callee, source, span);
            }
            for argument in arguments {
                generated_expression(argument, source, span);
            }
        }
        ExprKind::MacroCall {
            arguments,
            expansion,
            ..
        } => {
            for argument in arguments {
                generated_expression(argument, source, span);
            }
            generated_expression(expansion, source, span);
        }
        ExprKind::Member { base, .. } => generated_expression(base, source, span),
        ExprKind::Index { base, index } => {
            generated_expression(base, source, span);
            generated_expression(index, source, span);
        }
        ExprKind::List(values) | ExprKind::Block(values) | ExprKind::ArrayInitializer(values) => {
            for value in values {
                generated_expression(value, source, span);
            }
        }
        ExprKind::Statement(statement) => generated_statement(statement, source, span),
        _ => (),
    }
    let kind = std::mem::replace(&mut expression.kind, ExprKind::Unknown(String::new()));
    expression.kind = ExprKind::Generated {
        expression: Box::new(Expr {
            kind,
            span: span.clone(),
        }),
        code,
    };
    expression.span = span.clone();
}

fn generated_statement(statement: &mut Stmt, source: &str, span: &Span) {
    match &mut statement.kind {
        StmtKind::Block(statements) | StmtKind::Sequence(statements) => {
            for statement in statements {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::Expression(expression) => generated_expression(expression, source, span),
        StmtKind::Declaration(declarations) => {
            for declaration in declarations {
                declaration.span = span.clone();
                if let Some(expression) = &mut declaration.initializer {
                    generated_expression(expression, source, span);
                }
                for expression in &mut declaration.dimensions {
                    generated_expression(expression, source, span);
                }
            }
        }
        StmtKind::If {
            condition,
            consequence,
            alternative,
        } => {
            generated_expression(condition, source, span);
            generated_statement(consequence, source, span);
            if let Some(statement) = alternative {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::While { condition, body }
        | StmtKind::Switch { condition, body }
        | StmtKind::DoWhile { condition, body } => {
            generated_expression(condition, source, span);
            generated_statement(body, source, span);
        }
        StmtKind::For {
            initializer,
            condition,
            update,
            body,
        } => {
            if let Some(statement) = initializer {
                generated_statement(statement, source, span);
            }
            for expression in [condition, update].into_iter().flatten() {
                generated_expression(expression, source, span);
            }
            generated_statement(body, source, span);
        }
        StmtKind::RangeFor {
            declaration,
            iterable,
            body,
        } => {
            declaration.span = span.clone();
            generated_expression(iterable, source, span);
            generated_statement(body, source, span);
        }
        StmtKind::Case(expression) | StmtKind::Return(expression) | StmtKind::Throw(expression) => {
            if let Some(expression) = expression {
                generated_expression(expression, source, span);
            }
        }
        StmtKind::Label { statement, .. } => generated_statement(statement, source, span),
        StmtKind::Try { body, catches } => {
            generated_statement(body, source, span);
            for statement in catches {
                generated_statement(statement, source, span);
            }
        }
        StmtKind::Unknown(expressions) => {
            for expression in expressions {
                generated_expression(expression, source, span);
            }
        }
        _ => (),
    }
    statement.span = span.clone();
}

fn qualifier(t: &str) -> bool {
    matches!(
        t,
        "const"
            | "volatile"
            | "restrict"
            | "static"
            | "extern"
            | "inline"
            | "register"
            | "typedef"
            | "constexpr"
            | "consteval"
            | "constinit"
            | "friend"
            | "virtual"
            | "mutable"
            | "thread_local"
            | "_Thread_local"
            | "__inline"
            | "__inline__"
            | "__volatile"
            | "__volatile__"
            | "__const"
            | "__const__"
            | "__restrict"
            | "__restrict__"
            | "__extension__"
            | "__cdecl"
            | "__fastcall"
            | "__stdcall"
            | "__thiscall"
            | "_Noreturn"
    )
}

fn string_literal_token(text: &str) -> bool {
    text.find('"')
        .is_some_and(|quote| !text[..quote].contains('\''))
}

fn recovered_statement_siblings(statement: &Stmt) -> bool {
    matches!(&statement.kind, StmtKind::Sequence(statements) if statements.first().is_some_and(|s| matches!(s.kind, StmtKind::Problem)))
}

fn unclosed_quoted_literal(text: &str) -> bool {
    let Some(quote) = text.find(['\"', '\'']) else {
        return false;
    };
    if !matches!(&text[..quote], "" | "L" | "u" | "U" | "u8") {
        return false;
    }
    let bytes = text.as_bytes();
    let mut at = quote + 1;
    while at < bytes.len() {
        if bytes[at] == b'\\' {
            at += 2;
        } else if bytes[at] == bytes[quote] {
            return false;
        } else {
            at += 1;
        }
    }
    true
}

fn expression_type_keyword(t: &str, cpp: bool) -> bool {
    (type_word(t)
        && (cpp || !matches!(t, "bool" | "wchar_t" | "char8_t" | "char16_t" | "char32_t")))
        || matches!(
            t,
            "struct"
                | "union"
                | "enum"
                | "const"
                | "volatile"
                | "restrict"
                | "__const"
                | "__const__"
                | "__volatile"
                | "__volatile__"
                | "__restrict"
                | "__restrict__"
        )
        || (cpp && matches!(t, "class" | "typename"))
}

fn type_word(t: &str) -> bool {
    matches!(
        t,
        "void"
            | "char"
            | "short"
            | "int"
            | "long"
            | "float"
            | "double"
            | "signed"
            | "unsigned"
            | "_Bool"
            | "bool"
            | "wchar_t"
            | "char8_t"
            | "char16_t"
            | "char32_t"
            | "auto"
            | "__auto_type"
            | "__int8"
            | "__int16"
            | "__int32"
            | "__int64"
            | "__int128"
            | "__uint128_t"
            | "__float128"
            | "_Float16"
            | "_Float32"
            | "_Float64"
            | "_Float128"
            | "_Complex"
            | "_Imaginary"
            | "_Atomic"
            | "__complex__"
    )
}

fn binary_precedence(op: &str) -> Option<(u8, bool)> {
    Some(match op {
        "," => (0, false),
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "<<=" | ">>=" | "&=" | "^=" | "|=" => (1, true),
        "||" => (3, false),
        "&&" => (4, false),
        "|" => (5, false),
        "^" => (6, false),
        "&" => (7, false),
        "==" | "!=" => (8, false),
        "<" | ">" | "<=" | ">=" | "<=>" | "..." => (9, false),
        "<<" | ">>" => (10, false),
        "+" | "-" => (11, false),
        "*" | "/" | "%" => (12, false),
        ".*" | "->*" => (13, false),
        _ => return None,
    })
}

// CDT's ASTStringUtil renders simple return specifiers from their flags, so
// base qualifiers and signedness precede the type regardless of source order.
// Qualifiers on pointer operators keep their position after the pointer.
fn canonical_primitive_specifiers(code: &str) -> String {
    let split = code.find(['*', '&', '[']).unwrap_or(code.len());
    let words: Vec<_> = code[..split].split_whitespace().collect();
    if words.is_empty() || !words.iter().all(|word| type_word(word)) {
        return code.into();
    }
    let mut result = Vec::new();
    for modifier in ["signed", "unsigned", "short"] {
        if words.contains(&modifier) {
            result.push(modifier);
        }
    }
    result.extend(std::iter::repeat_n(
        "long",
        words.iter().filter(|&&word| word == "long").count().min(2),
    ));
    for word in words {
        if !matches!(word, "signed" | "unsigned" | "short" | "long") && !result.contains(&word) {
            result.push(word);
        }
    }
    format!("{}{}", result.join(" "), &code[split..])
}

fn primitive_return_display(code: &str) -> String {
    let split = code.find(['*', '&', '[']).unwrap_or(code.len());
    let words: Vec<_> = code[..split].split_whitespace().collect();
    let modifiers = ["const", "volatile", "signed", "unsigned"];
    let mut base: Vec<_> = modifiers
        .iter()
        .copied()
        .filter(|modifier| words.contains(modifier))
        .collect();
    base.extend(words.into_iter().filter(|word| !modifiers.contains(word)));
    format!("{}{}", base.join(" "), &code[split..]).replace("_Bool", "bool")
}

fn simple_type_name(name: &str) -> &str {
    let mut depth = 0usize;
    let mut start = 0;
    for (at, ch) in name.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            ':' if depth == 0 && name[at..].starts_with("::") => start = at + 2,
            _ => {}
        }
    }
    &name[start..]
}

fn alternative_operator(op: &str) -> &str {
    match op {
        "and" => "&&",
        "or" => "||",
        "not" => "!",
        "bitand" => "&",
        "bitor" => "|",
        "xor" => "^",
        "compl" => "~",
        "and_eq" => "&=",
        "or_eq" => "|=",
        "xor_eq" => "^=",
        "not_eq" => "!=",
        other => other,
    }
}

#[cfg(test)]
mod declaration_tests {
    use super::*;

    #[test]
    fn cpp_implicit_return_problems_preserve_explicit_functions_and_constructors() {
        let source = include_str!(
            "../tests/fixtures/declaration-recovery/implicit_return_function_declarations.cpp"
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
            "../tests/fixtures/declaration-recovery/grouped_address_parameter_recovery.c"
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
            include_str!("../tests/fixtures/declaration-recovery/address_declarator_recovery.c");
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
            include_str!("../tests/fixtures/declaration-recovery/nested_noreturn_headers.c");
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
            include_str!("../tests/fixtures/declaration-recovery/record_function_shaped_fields.c");
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
                include_str!("../tests/fixtures/declaration-recovery/problem_record_parentheses.c"),
                cpp,
            );
            let valid = parse(
                include_str!("../tests/fixtures/declaration-recovery/record_return_declarators.c"),
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
            include_str!("../tests/fixtures/declaration-recovery/nested_function_declarations.c");
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
            include_str!("../tests/fixtures/declaration-recovery/missing_outer_functions.c");
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
            include_str!("../tests/fixtures/decbench-regressions/cpp_operator_definitions.cpp"),
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
            include_str!("../tests/fixtures/decbench-regressions/cpp_operator_declarations.cpp"),
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
            "../tests/fixtures/ddg-lambdas/method-names.json"
        ))
        .unwrap();
        assert_eq!(reference["generator"], "Original Joern 4.0.150 / c2cpg");
        for (filename, source) in [
            (
                "using.cpp",
                include_str!("../tests/fixtures/ddg-lambdas/using.cpp"),
            ),
            (
                "namespaced.cpp",
                include_str!("../tests/fixtures/ddg-lambdas/namespaced.cpp"),
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
            "../tests/fixtures/ddg-classes/joern-4.0.150.json"
        ))
        .unwrap();
        let unit = parse(
            include_str!("../tests/fixtures/ddg-classes/constructors.cpp"),
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
            "../tests/fixtures/ddg-classes/method-order-oracle.json"
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
        assert_eq!(unit.functions[0].return_type, "Number*[4]");
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
            "../tests/fixtures/decbench-regressions/microsoft_asm_orphan_declarations.c"
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
            include_str!("../tests/fixtures/decbench-regressions/qualified_parameter_types.c");
        let c = parse(source, false);
        let cpp = parse(source, true);
        assert!(c.diagnostics.is_empty() && cpp.diagnostics.is_empty());
        assert_eq!(c.functions.len(), 4);
        assert!(c.functions.iter().all(|f| f.name.starts_with("after_")));
        assert_eq!(cpp.functions.len(), 8);
    }

    #[test]
    fn recovered_c_assembly_tails_allow_function_shaped_initializers() {
        let source = include_str!("../tests/fixtures/decbench-regressions/asm_tail_assignments.c");
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
            include_str!("../tests/fixtures/decbench-regressions/asm_repeated_projection.c");
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
        let source =
            include_str!("../tests/fixtures/decbench-regressions/asm_recovered_controls.c");
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn named_pointer_and_reference_ambiguities_follow_cdt_bindings() {
        let source =
            include_str!("../tests/fixtures/declaration-recovery/logical_declaration_boundaries.c");
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
            include_str!("../tests/fixtures/declaration-recovery/reference_binding_boundaries.c");
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
            "../tests/fixtures/declaration-recovery/qualified_pointer_binding_boundaries.c"
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
}
