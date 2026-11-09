use rust_joern::{analyze, graph::Options};

#[test]
fn block_static_assert_is_ast_only_in_cpp() {
    let source = "constexpr int check(int x) { return x + 1; }
        int f(int x) { static_assert(check(2) == 3, \"checked\"); return check(x); }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let result = analyze(source, "assert.cpp", &options).unwrap();
    let function = result.functions.iter().find(|f| f.name == "f").unwrap();
    let find = |code: &str| {
        function
            .cpg
            .nodes
            .iter()
            .find(|n| n.code == code)
            .unwrap()
            .id
    };
    let assertion = find("static_assert(check(2) == 3, \"checked\")");
    let compile_time = find("check(2)");
    let runtime = find("check(x)");
    for id in [assertion, compile_time] {
        assert!(
            function
                .cpg
                .edges
                .iter()
                .any(|e| e.kind == "AST" && e.target == id)
        );
        assert!(
            !function
                .cpg
                .edges
                .iter()
                .any(|e| e.kind == "CFG" && (e.target == id || e.source == id))
        );
        assert!(
            !function
                .cfg
                .nodes
                .iter()
                .any(|b| b.statements.contains(&id))
        );
    }
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|b| b.statements.contains(&runtime))
    );
}

#[test]
fn c_function_named_static_assert_is_still_executed() {
    let source = "void static_assert(int); void f(int x) { static_assert(x); }";
    let options = Options {
        language: Some("c".into()),
        strict: true,
        ..Options::default()
    };
    let result = analyze(source, "assert.c", &options).unwrap();
    let function = result.functions.iter().find(|f| f.name == "f").unwrap();
    let call = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.code == "static_assert(x)")
        .unwrap();
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|b| b.statements.contains(&call.id))
    );
}

#[test]
fn joern_cpp_control_flow_keeps_the_assertion_in_the_cfg() {
    let source = "constexpr int check(int x) { return x; } void f() { static_assert(check(2) == 2, \"checked\"); }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        joern_cpp_control_flow: true,
        ..Options::default()
    };
    let result = analyze(source, "assert.cpp", &options).unwrap();
    let f = result.functions.iter().find(|f| f.name == "f").unwrap();
    let call = f
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "CALL" && n.name.as_deref() == Some("check"))
        .unwrap();
    assert!(
        f.cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && (e.source == call.id || e.target == call.id))
    );
}
