//! C++ expression-name regressions. These check syntax, call operands and CFG
//! ordering; they are not snapshots generated from this implementation.
use rust_joern::{
    analyze,
    graph::{Analysis, FunctionGraph, Options},
};

fn parse(source: &str) -> Analysis {
    analyze(
        source,
        "qualified.cpp",
        &Options {
            language: Some("cpp".into()),
            strict: true,
            preprocessed: true,
            ..Options::default()
        },
    )
    .unwrap()
}
fn function(analysis: &Analysis) -> &FunctionGraph {
    analysis.functions.iter().find(|f| f.name == "f").unwrap()
}
fn call(function: &FunctionGraph, name: &str) -> u32 {
    let ids: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL" && n.name.as_deref() == Some(name))
        .map(|n| n.id)
        .collect();
    assert_eq!(ids.len(), 1, "expected exactly one {name}: {ids:?}");
    ids[0]
}
fn path(function: &FunctionGraph, from: u32, to: u32) -> bool {
    let mut pending = vec![from];
    let mut seen = std::collections::HashSet::new();
    while let Some(node) = pending.pop() {
        if node == to {
            return true;
        }
        if seen.insert(node) {
            pending.extend(
                function
                    .cpg
                    .edges
                    .iter()
                    .filter(|e| e.kind == "CFG" && e.source == node)
                    .map(|e| e.target),
            );
        }
    }
    false
}
fn argument_code(function: &FunctionGraph, call: u32, index: usize) -> &str {
    let edge = function
        .cpg
        .edges
        .iter()
        .find(|e| {
            e.kind == "ARGUMENT"
                && e.source == call
                && e.label.as_deref() == Some(&index.to_string())
        })
        .unwrap();
    &function.cpg.nodes[edge.target as usize].code
}

#[test]
fn declared_template_calls_remain_calls_in_all_expression_positions() {
    let context = "namespace demo { template<class T> struct Box { static int get(int); static void touch(int); }; } int sink(int);";
    for body in [
        "demo::Box<int>::touch(x);",
        "return demo::Box<int>::get(x);",
        "return sink(demo::Box<int>::get(x));",
    ] {
        let analysis = parse(&format!("{context} int f(int x) {{ {body} }}"));
        let f = function(&analysis);
        let name = if body.starts_with("demo") {
            "touch"
        } else {
            "get"
        };
        let id = call(f, name);
        assert_eq!(argument_code(f, id, 1), "x");
        assert!(
            f.cpg.nodes[id as usize]
                .code
                .starts_with("demo::Box<int>::")
        );
        if body.contains("sink") {
            assert!(path(f, id, call(f, "sink")));
        }
    }
}

#[test]
fn nested_template_ids_and_global_scope_keep_callee_and_operands() {
    let a = parse(
        "namespace demo { template<class T> struct Box { static int get(int,int); }; } int left(); int right(); int f() { return ::demo::Box<demo::Box<int>>::get(left(),right()); }",
    );
    let f = function(&a);
    let id = call(f, "get");
    assert_eq!(argument_code(f, id, 1), "left()");
    assert_eq!(argument_code(f, id, 2), "right()");
    assert!(path(f, call(f, "left"), id));
    assert!(path(f, call(f, "right"), id));
    assert_eq!(
        f.cpg.nodes[id as usize].code,
        "::demo::Box<demo::Box<int>>::get(left(),right())"
    );
}

#[test]
fn explicitly_qualified_subscript_keeps_receiver_and_argument_effects() {
    let a = parse(
        "namespace demo { struct Box { int operator[](int); }; } demo::Box &receiver(); int tick(); int f() { return receiver().demo::Box::operator[](tick()); }",
    );
    let f = function(&a);
    let id = call(f, "operator[]");
    assert_eq!(argument_code(f, id, 0), "receiver()");
    assert_eq!(argument_code(f, id, 1), "tick()");
    assert!(path(f, call(f, "receiver"), call(f, "tick")));
    assert!(path(f, call(f, "tick"), id));
}

#[test]
fn explicit_member_qualification_does_not_retarget_receiver_method() {
    let a = parse(
        "struct Base { int get(int x) { return x; } }; struct Derived : Base { int get(int x) { return x+1; } }; int f(Derived &x) { return x.Base::get(1); }",
    );
    let f = function(&a);
    let id = call(f, "get");
    assert!(f.cpg.nodes[id as usize].method_full_name.is_none());
    assert_eq!(f.cpg.nodes[id as usize].code, "x.Base::get(1)");
    assert_eq!(argument_code(f, id, 0), "x");
}

#[test]
fn explicit_operator_ids_do_not_become_binary_operators_or_index_expressions() {
    for (name, context, body) in [
        (
            "operator+",
            "namespace demo { struct Object {}; Object operator+(Object,Object); }",
            "demo::operator+(x,x)",
        ),
        (
            "operator[]",
            "namespace demo { struct Object { int operator[](int); }; }",
            "x.operator[](1)",
        ),
        (
            "operator()",
            "namespace demo { struct Object { int operator()(int); }; }",
            "x.operator()(1)",
        ),
    ] {
        let a = parse(&format!(
            "{context} int f(demo::Object x) {{ {body}; return 0; }}"
        ));
        let f = function(&a);
        call(f, name);
        assert!(!f.cpg.nodes.iter().any(|n| matches!(
            n.name.as_deref(),
            Some("<operator>.addition" | "<operator>.indirectIndexAccess")
        )));
    }
}

#[test]
fn comparisons_with_bound_identifiers_are_not_template_calls() {
    let a = parse("int f(int a,int b,int c) { return a < b > (c); }");
    let f = function(&a);
    call(f, "<operator>.lessThan");
    call(f, "<operator>.greaterThan");
}

#[test]
fn template_parameter_class_and_defaults_do_not_hide_real_record_body() {
    let a = parse(
        "template<class T = int> struct Box { static int get(int x) { return x; } }; int f(int x) { return Box<int>::get(x); }",
    );
    assert!(a.functions.iter().any(|f| f.name == "get"));
    call(function(&a), "get");
}

#[test]
fn invalid_qualified_names_keep_existing_strict_errors() {
    for source in [
        "int f(int x) { return ::0x1234::call(x); }",
        "int f(int x) { return demo::Box<int>::(x); }",
    ] {
        assert!(
            analyze(
                source,
                "bad.cpp",
                &Options {
                    language: Some("cpp".into()),
                    strict: true,
                    preprocessed: true,
                    ..Options::default()
                }
            )
            .is_err(),
            "accepted {source}"
        );
    }
}

#[test]
fn unrelated_local_bindings_do_not_shadow_qualified_template_components() {
    let a = parse(
        "namespace demo { template<class T> struct Box { static int get(int); }; struct Object { template<class T> int get(int); }; } int f(demo::Object &x,int get,int Box) { return demo::Box<int>::get(Box) + x.template get<int>(get); }",
    );
    let f = function(&a);
    let calls: Vec<_> = f
        .cpg
        .nodes
        .iter()
        .filter(|n| {
            n.kind == "CALL"
                && (n.code == "demo::Box<int>::get(Box)" || n.code == "x.template get<int>(get)")
        })
        .collect();
    assert_eq!(calls.len(), 2);
    for n in calls {
        assert!(!n.name.as_deref().unwrap().starts_with("<operator>"));
    }
}

#[test]
fn comparison_does_not_capture_a_later_shift_and_cast_as_template_arguments() {
    let a = parse(
        "int f(int x,int y) { int result; result = x < 0; if (y) { x = y >> (unsigned long long)x; } return result+x; }",
    );
    let f = function(&a);
    call(f, "<operator>.lessThan");
    call(f, "<operator>.arithmeticShiftRight");
    assert!(
        f.cpg
            .nodes
            .iter()
            .any(|n| n.kind == "RETURN" && n.code == "return result+x;")
    );
}

#[test]
fn explicit_operator_template_arguments_preserve_calls_and_returns() {
    let context = "struct Box { int value; }; template<class T> bool operator!=(const T &a,const T &b) { return a.value != b.value; }";
    for spelling in ["operator!=<>", "operator!=<Box>", "::operator!=<Box>"] {
        let a = parse(&format!(
            "{context} int f(Box a,Box b) {{ return {spelling}(a,b); }}"
        ));
        let f = function(&a);
        let id = call(f, spelling.trim_start_matches("::"));
        assert_eq!(argument_code(f, id, 1), "a");
        assert_eq!(argument_code(f, id, 2), "b");
        let returns: Vec<_> = f.cpg.nodes.iter().filter(|n| n.kind == "RETURN").collect();
        assert_eq!(returns.len(), 1);
        assert!(path(f, id, returns[0].id));
        assert!(!f.cpg.nodes.iter().any(|n| n.kind == "UNKNOWN"));
    }
}

#[test]
fn operator_template_call_in_comma_condition_keeps_executable_operands() {
    let a = parse(
        "struct Box { int value; }; template<class T> bool operator!=(const T &a,const T &b) { return a.value != b.value; } int touch(); int f(Box a,Box b) { int x; while ((x = operator!=<>(a,b), x && touch())) { a.value--; } return x; }",
    );
    let f = function(&a);
    let comparison = call(f, "operator!=<>");
    let assignment = call(f, "<operator>.assignment");
    assert!(path(f, comparison, assignment));
    assert!(path(f, assignment, call(f, "touch")));
    call(f, "<operator>.logicalAnd");
    assert!(!f.cpg.nodes.iter().any(|n| n.kind == "UNKNOWN"));
}

#[test]
fn qualified_and_member_operator_templates_keep_nested_arguments() {
    for (source, spelling, argument) in [
        (
            "namespace demo { template<class T> struct Box { int value; }; template<class T> int operator+(Box<T> a,Box<T> b) { return a.value+b.value; } } int f(demo::Box<int> a,demo::Box<int> b) { return demo::operator+<int>(a,b); }",
            "operator+<int>",
            "a",
        ),
        (
            "struct Box { template<class T> int operator()(int x) { return x; } }; int f(Box &box,int x) { return box.template operator()<int>(x); }",
            "operator()<int>",
            "x",
        ),
        (
            "template<class T> struct Box { int value; }; template<class T> bool operator!=(T a,T b) { return a.value!=b.value; } int f(Box<Box<int>> a,Box<Box<int>> b) { return operator!=<Box<Box<int>>>(a,b); }",
            "operator!=<Box<Box<int>>>",
            "a",
        ),
    ] {
        let a = parse(source);
        let f = function(&a);
        let id = call(f, spelling);
        assert_eq!(argument_code(f, id, 1), argument);
        assert!(f.cpg.nodes.iter().any(|n| n.kind == "RETURN"));
        assert!(!f.cpg.nodes.iter().any(|n| n.kind == "UNKNOWN"));
    }
}

#[test]
fn operator_call_template_accepts_empty_runtime_arguments() {
    let a = parse(
        "struct Box { template<class T> int operator()(); }; int f(Box &box) { return box.template operator()<int>(); }",
    );
    let f = function(&a);
    call(f, "operator()<int>");
    assert!(f.cpg.nodes.iter().any(|n| n.kind == "RETURN"));
    assert!(!f.cpg.nodes.iter().any(|n| n.kind == "UNKNOWN"));
}

#[test]
fn scopes_inside_operator_template_arguments_do_not_rename_the_callee() {
    let a = parse(
        "namespace types { struct Tag { int value; }; } namespace demo { template<class T> int operator+(T a,T b) { return a.value+b.value; } } int f(types::Tag a,types::Tag b) { return demo::operator+<types::Tag>(a,b); }",
    );
    let f = function(&a);
    let id = call(f, "operator+<types::Tag>");
    assert_eq!(argument_code(f, id, 1), "a");
    assert_eq!(argument_code(f, id, 2), "b");
}

#[test]
fn operator_template_names_do_not_hide_invalid_runtime_arguments() {
    for source in [
        "int f(int a,int b) { return operator!=<>(a,,b); }",
        "int f(int a,int b) { return operator!=<>(a +,b); }",
        "int f(int a,int b) { return operator!=<>(a @,b); }",
    ] {
        let parsed = analyze(
            source,
            "bad.cpp",
            &Options {
                language: Some("cpp".into()),
                strict: true,
                preprocessed: true,
                ..Options::default()
            },
        );
        if let Ok(analysis) = parsed {
            let f = function(&analysis);
            assert!(
                !f.cpg
                    .nodes
                    .iter()
                    .any(|n| n.kind == "CALL" && n.name.as_deref() == Some("operator!=<>")),
                "retained malformed call in {source}"
            );
        }
    }
}
