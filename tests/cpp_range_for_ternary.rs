use rust_joern::{analyze, graph::Options};

#[test]
fn range_for_keeps_unparenthesized_ternary_and_body() {
    let source = "int a[2], b[2], c[2];
        int probe(bool choose, bool second) {
            int sum = 0;
            for (int value : choose ? a : second ? b : c) { sum += value; }
            return sum;
        }";
    let options = Options {
        language: Some("cpp".into()),
        strict: true,
        ..Options::default()
    };
    let result = analyze(source, "range.cpp", &options).unwrap();
    let function = result.functions.iter().find(|f| f.name == "probe").unwrap();
    let nodes = &function.cpg.nodes;
    let find = |code: &str| nodes.iter().find(|n| n.code == code).unwrap().id;
    let range = nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("FOR"))
        .unwrap();
    let expression = find("choose ? a : second ? b : c");
    assert!(
        function
            .cpg
            .edges
            .iter()
            .any(|e| e.kind == "AST" && e.source == range.id && e.target == expression)
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|n| n.name.as_deref() == Some("<operator>.conditional"))
            .count(),
        2
    );
    let update = find("sum += value");
    assert!(
        function
            .cfg
            .nodes
            .iter()
            .any(|b| b.statements.contains(&update))
    );
    assert!(
        function
            .cpg
            .edges
            .iter()
            .any(|e| e.kind == "CFG" && e.target == find("return sum;"))
    );
}
