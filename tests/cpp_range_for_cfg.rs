use rust_joern::{analyze, graph::Options};

#[test]
fn range_for_evaluates_range_once_and_routes_loop_exits() {
    let source =
        "struct Range { int *begin(); int *end(); }; Range get(); void visit(int); void after();
        void probe() {
            for (int x : get()) {
                if (x == 1) continue;
                if (x == 3) break;
                visit(x);
            }
            after();
        }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let analysis = analyze(source, "range.cpp", &options).unwrap();
    let function = analysis
        .functions
        .iter()
        .find(|f| f.name == "probe")
        .unwrap();
    let node = |code: &str| {
        function
            .cpg
            .nodes
            .iter()
            .find(|n| n.code == code)
            .unwrap()
            .id
    };
    let edge = |a, b| {
        function
            .cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && e.source == a && e.target == b)
    };
    let get = node("get()");
    let test = node("<range-for test>");
    let advance = node("<range-for advance>");
    let after = node("after()");
    assert!(edge(get, test));
    assert!(edge(test, after), "an empty range skips the body");
    assert!(edge(node("break;"), after));
    assert!(edge(node("continue;"), advance));
    assert!(edge(node("visit(x)"), advance));
    assert!(edge(advance, test));
    assert!(!edge(advance, get));
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|b| b.statements.contains(&test))
    );
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|b| b.statements.contains(&get))
    );
}

#[test]
fn range_for_without_a_backedge_does_not_create_an_orphan_advance() {
    let source = "void probe() { int a[1] = {1}; for (int x : a) break; }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let analysis = analyze(source, "range.cpp", &options).unwrap();
    let function = analysis
        .functions
        .iter()
        .find(|f| f.name == "probe")
        .unwrap();
    assert!(
        function
            .cpg
            .nodes
            .iter()
            .any(|n| n.code == "<range-for test>")
    );
    assert!(
        !function
            .cpg
            .nodes
            .iter()
            .any(|n| n.code == "<range-for advance>")
    );
}

#[test]
fn joern_cpp_control_flow_keeps_the_body_as_the_condition() {
    let source = "int get(); void f() { for (int x : get()) { if (x) break; } }";
    let options = Options {
        language: Some("cpp".into()),
        joern_cpp_control_flow: true,
        ..Options::default()
    };
    let result = analyze(source, "range.cpp", &options).unwrap();
    let f = result.functions.iter().find(|f| f.name == "f").unwrap();
    let flows = |code: &str| {
        let id = f.cpg.nodes.iter().find(|n| n.code == code).unwrap().id;
        f.cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && (e.source == id || e.target == id))
    };
    assert!(!flows("get()"), "Joern never evaluates the range");
    assert!(!f.cpg.nodes.iter().any(|n| n.code == "<range-for test>"));
}
