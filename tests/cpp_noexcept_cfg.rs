use rust_joern::{analyze, graph::Options};

#[test]
fn noexcept_operand_calls_remain_ast_only() {
    let source = "int effect() noexcept; bool f() { return noexcept(effect()); }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let result = analyze(source, "noexcept.cpp", &options).unwrap();
    let f = result.functions.iter().find(|f| f.name == "f").unwrap();
    let call = f
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "CALL" && n.name.as_deref() == Some("effect"))
        .unwrap();
    assert!(
        f.cpg
            .edges
            .iter()
            .any(|e| e.kind == "AST" && e.target == call.id)
    );
    assert!(
        !f.cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && (e.source == call.id || e.target == call.id)),
        "unevaluated operand was added to CFG"
    );
    assert!(
        !f.cfg
            .nodes
            .iter()
            .any(|block| block.statements.contains(&call.id)),
        "unevaluated operand was added to the normalized CFG"
    );
}

#[test]
fn only_noexcept_operands_are_excluded_from_control_flow() {
    let source = "bool effect(); bool f() { return noexcept(effect() && effect()) && effect(); }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let result = analyze(source, "noexcept.cpp", &options).unwrap();
    let f = result.functions.iter().find(|f| f.name == "f").unwrap();
    let calls: Vec<_> = f
        .cpg
        .nodes
        .iter()
        .filter(|n| n.kind == "CALL" && n.name.as_deref() == Some("effect"))
        .collect();
    assert_eq!(calls.len(), 3);
    let in_cfg = |id| {
        f.cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && (e.source == id || e.target == id))
    };
    assert!(!in_cfg(calls[0].id));
    assert!(!in_cfg(calls[1].id));
    assert!(in_cfg(calls[2].id));
}
