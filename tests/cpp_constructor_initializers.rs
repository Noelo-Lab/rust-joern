//! The opt-in profile models scalar initialization on normal completion only.
use rust_joern::{analyze, graph::Options};

fn options(enabled: bool) -> Options {
    Options {
        language: Some("cpp".into()),
        preprocessed: true,
        strict: true,
        cpp_constructor_initializers: enabled,
        ..Options::default()
    }
}

#[test]
fn initialization_follows_declarations_and_preserves_rhs_and_body_calls() {
    let source = "int first(); int second(); void body(); struct A {
        int x; int y; A() : y(second()), x(first()) { body(); }
    };";
    assert!(analyze(source, "order.cpp", &options(false)).is_err());
    let result = analyze(source, "order.cpp", &options(true)).unwrap();
    let method = result.functions.iter().find(|f| f.name == "A").unwrap();
    let calls: Vec<_> = method
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL")
        .collect();
    let id = |name: &str| {
        let nodes: Vec<_> = calls
            .iter()
            .filter(|n| n.name.as_deref() == Some(name))
            .collect();
        assert_eq!(nodes.len(), 1, "exactly one {name}");
        nodes[0].id
    };
    let inits: Vec<_> = calls
        .iter()
        .filter(|n| n.name.as_deref() == Some("<operator>.fieldInitialization"))
        .collect();
    assert_eq!(inits.len(), 2);
    assert_eq!(inits[0].code, "x(first())");
    assert_eq!(inits[1].code, "y(second())");
    let mut reached = std::collections::HashSet::new();
    let mut pending = vec![id("first")];
    while let Some(node) = pending.pop() {
        if reached.insert(node) {
            pending.extend(
                method
                    .cpg
                    .edges
                    .iter()
                    .filter(|e| e.kind == "CFG" && e.source == node)
                    .map(|e| e.target),
            );
        }
    }
    assert!(reached.contains(&inits[0].id));
    assert!(reached.contains(&id("second")));
    assert!(reached.contains(&inits[1].id));
    assert!(reached.contains(&id("body")));
    let first_init_successors: Vec<_> = method
        .cpg
        .edges
        .iter()
        .filter(|e| e.kind == "CFG" && e.source == inits[0].id)
        .map(|e| e.target)
        .collect();
    assert!(!first_init_successors.contains(&id("first")));
}

#[test]
fn incomplete_or_non_scalar_plans_fail_closed() {
    for source in [
        "struct A { int x; int y; A(): x(1) {} };",
        "struct A { int x; A(): x(1), x(2) {} };",
        "struct A { int x; A(): absent(1) {} };",
        "struct A { int& x; A(int& v): x(v) {} };",
        "struct B {}; struct A { B b; A(): b(1) {} };",
        "struct B {}; struct A : B { int x; A(): x(1) {} };",
        "union A { int x; A(): x(1) {} };",
        "struct A { int x=1; A(): x(2) {} };",
        "struct A { int x:2; A(): x(1) {} };",
        "struct A { int x[2]; A(): x(1) {} };",
        "struct A { int x; virtual void f(); A(): x(1) {} };",
        "struct A { int x; A(): x{1} {} };",
        "struct A { int x; A(): x() {} };",
        "A::A(): x(1) {}",
        "struct A { int x; A(): x(unknown()) {} };",
        "struct B { operator int(); }; struct A { int x; A(B v): x(v) {} };",
        "struct A { int x; A(): x(1_tag) {} };",
        "int effect(); struct A { int x=effect(); A() {} };",
        "struct B { B(); }; struct A { B b; A() {} };",
        "struct B {}; struct D: virtual B {}; struct A { B* x; A(D* p): x(p) {} };",
    ] {
        assert!(
            analyze(source, "unsupported.cpp", &options(true)).is_err(),
            "{source}"
        );
    }
}

#[test]
fn profile_refuses_unqualified_dataflow_and_c_input() {
    let mut config = options(true);
    config.data_flow = true;
    assert!(analyze("struct A { int x; A(): x(1) {} };", "a.cpp", &config).is_err());
    config.data_flow = false;
    config.language = Some("c".into());
    assert!(analyze("int f() { return 1; }", "a.c", &config).is_err());
}

#[test]
fn saved_parameter_and_prior_member_reads_keep_distinct_bindings() {
    let result = analyze(
        "struct A { int x; int y; A(int x): y(this_x), x(x) {} };",
        "bad-read.cpp",
        &options(true),
    );
    assert!(result.is_err());
    let result = analyze(
        "struct A { int x; int y; A(int value): y(x), x(value) {} };",
        "member-read.cpp",
        &options(true),
    )
    .unwrap();
    let method = result.functions.iter().find(|f| f.name == "A").unwrap();
    let initializers: Vec<_> = method
        .cpg
        .nodes
        .iter()
        .filter(|n| n.name.as_deref() == Some("<operator>.fieldInitialization"))
        .collect();
    assert_eq!(
        initializers
            .iter()
            .map(|n| n.code.as_str())
            .collect::<Vec<_>>(),
        ["x(value)", "y(x)"]
    );
    let rhs = method
        .cpg
        .edges
        .iter()
        .find(|e| {
            e.kind == "ARGUMENT"
                && e.source == initializers[1].id
                && e.label.as_deref() == Some("2")
        })
        .unwrap();
    assert_eq!(
        method.cpg.nodes[rhs.target as usize].name.as_deref(),
        Some("<operator>.indirectFieldAccess")
    );
}

#[test]
fn literal_and_pointer_conversions_require_proven_compatibility() {
    for source in [
        "struct A { int x; A(): x(nullptr) {} };",
        "struct A { int x; A(): x(\"abc\") {} };",
        "struct A { int x; A(int* p): x(p) {} };",
        "struct A { int* x; A(const int* p): x(p) {} };",
        // A valid pointer copy is also rejected until pointee qualifiers are
        // preserved well enough to distinguish it from the case above.
        "struct A { int* x; A(int* p): x(p) {} };",
    ] {
        assert!(
            analyze(source, "invalid.cpp", &options(true)).is_err(),
            "{source}"
        );
    }
    for source in [
        "struct A { int* x; A(): x(nullptr) {} };",
        "struct A { int x; A(): x(42) {} };",
        "struct A { bool x; A(): x(true) {} };",
        "struct A { char x; A(): x('a') {} };",
    ] {
        assert!(
            analyze(source, "valid.cpp", &options(true)).is_ok(),
            "{source}"
        );
    }
}
