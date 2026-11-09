use rust_joern::{analyze, graph::Options};

#[test]
fn sizeof_pack_preserves_unevaluated_operand_and_cfg() {
    let source = "template<class... T> unsigned count() { return sizeof...(T); }";
    let options = Options {
        strict: true,
        language: Some("cpp".into()),
        ..Options::default()
    };
    let result = analyze(source, "pack.cpp", &options).unwrap();
    let function = result.functions.iter().find(|f| f.name == "count").unwrap();
    let calls: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL")
        .collect();
    assert_eq!(
        calls
            .iter()
            .filter(|n| n.name.as_deref() == Some("<operator>.sizeOf"))
            .count(),
        1
    );
    assert!(calls.iter().any(|n| n.code == "sizeof...(T)"));
    assert_eq!(
        function
            .cfg
            .nodes
            .iter()
            .filter(|n| n.is_entrypoint)
            .count(),
        1
    );
    assert_eq!(
        function.cfg.nodes.iter().filter(|n| n.is_exitpoint).count(),
        1
    );
}

#[test]
fn sizeof_value_pack_preserves_source_and_no_runtime_call() {
    let source = "template<class... T> unsigned count(T... xs) { return sizeof...(xs); }";
    let options = Options {
        strict: true,
        language: Some("cpp".into()),
        ..Options::default()
    };
    let result = analyze(source, "pack.cpp", &options).unwrap();
    let function = result.functions.iter().find(|f| f.name == "count").unwrap();
    let calls: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL")
        .collect();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name.as_deref(), Some("<operator>.sizeOf"));
    assert_eq!(calls[0].code, "sizeof...(xs)");
}
