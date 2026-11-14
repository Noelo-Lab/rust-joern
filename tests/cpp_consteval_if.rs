use rust_joern::{analyze, graph::Options};

fn options() -> Options {
    Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    }
}

#[test]
fn runtime_graph_keeps_only_the_selected_arm() {
    let source = "int runtime(int);
        constexpr int pick(int x) {
            if consteval { return x + 100; }
            else { return runtime(x); }
        }
        constexpr int reversed(int x) {
            if not consteval { return runtime(x); }
            else { return x + 200; }
        }
        constexpr int fallthrough(int x) {
            if consteval { return x + 300; }
            return runtime(x);
        }";
    let analysis = analyze(source, "consteval.cpp", &options()).unwrap();
    for name in ["pick", "reversed", "fallthrough"] {
        let function = analysis.functions.iter().find(|f| f.name == name).unwrap();
        let nodes = &function.cpg.nodes;
        assert!(nodes.iter().all(|n| n.kind != "CONTROL_STRUCTURE"));
        let returns: Vec<_> = nodes.iter().filter(|n| n.kind == "RETURN").collect();
        assert_eq!(returns.len(), 1);
        assert_eq!(returns[0].code, "return runtime(x);");
        let call = nodes.iter().find(|n| n.code == "runtime(x)").unwrap();
        assert!(
            function
                .cpg
                .edges
                .iter()
                .any(|e| { e.kind == "CFG" && e.source == call.id && e.target == returns[0].id })
        );
    }
}

#[test]
fn nested_consteval_keeps_break_and_fallthrough_targets() {
    let source = "void step(); void done();
        void run(int n) {
            while (n > 0) {
                if !consteval { break; }
                else { step(); }
            }
            done();
        }";
    let analysis = analyze(source, "nested.cpp", &options()).unwrap();
    let function = analysis.functions.iter().find(|f| f.name == "run").unwrap();
    let nodes = &function.cpg.nodes;
    assert!(!nodes.iter().any(|n| n.name.as_deref() == Some("step")));
    let stop = nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("BREAK"))
        .unwrap();
    let done = nodes.iter().find(|n| n.code == "done()").unwrap();
    assert!(
        function
            .cpg
            .edges
            .iter()
            .any(|e| { e.kind == "CFG" && e.source == stop.id && e.target == done.id })
    );
}

#[test]
fn compatibility_omits_both_arms_and_recovers_the_successor() {
    let source = "int runtime();
        constexpr int f() {
            if consteval { return 1; }
            else { return runtime(); }
        }
        int g() {
            if consteval { auto unused = []() { return 2; }; }
            else { auto unused = []() { return 4; }; }
            return 3;
        }";
    let analysis = analyze(
        source,
        "compat.cpp",
        &Options {
            joern_cpp_control_flow: true,
            ..options()
        },
    )
    .unwrap();
    let f = analysis.functions.iter().find(|f| f.name == "f").unwrap();
    assert!(
        f.cpg
            .nodes
            .iter()
            .all(|n| n.kind != "RETURN" && n.kind != "CALL")
    );
    let g = analysis.functions.iter().find(|f| f.name == "g").unwrap();
    assert!(g.cpg.nodes.iter().any(|n| n.code == "return 3;"));
    assert_eq!(analysis.functions.len(), 3);
}

#[test]
fn local_runtime_alias_does_not_change_later_function_identity() {
    let source = "using Element = int;
        void run() {
            if !consteval { using Element = long; Element storage = 0; }
        }
        Element convert(Element x) { return x; }";
    let analysis = analyze(source, "scope.cpp", &options()).unwrap();
    let function = analysis
        .functions
        .iter()
        .find(|f| f.name == "convert")
        .unwrap();
    assert_eq!(function.fullname, "convert:int(int)");
}

#[test]
fn incomplete_and_unbraced_arms_are_diagnosed() {
    for source in [
        "int f() { if consteval return 1; return 3; }",
        "int f() { if !consteval { return 1; } else return 2; }",
    ] {
        let error = analyze(source, "bad.cpp", &options()).unwrap_err();
        assert!(error.contains("consteval if requires complete compound statements"));
    }
}
