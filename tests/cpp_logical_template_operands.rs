use rust_joern::{analyze, graph::Options};

#[test]
fn qualified_template_operands_keep_logical_short_circuiting() {
    let source = r#"
namespace traits {
template<class T> constexpr bool enabled = true;
constexpr int value = 1;
}
template<class T> struct Box {};
bool rhs();
bool both() { return traits::enabled<int> && rhs(); }
bool either() { return traits::enabled<int> or rhs(); }
bool nested() { return traits::enabled<Box<int>> and rhs(); }
bool chain() { return rhs() && traits::enabled<int> && traits::enabled<Box<int>>; }
bool comparisons(int a, int b) { return traits::value < a && b > 0; }
"#;
    let analysis = analyze(
        source,
        "template-logic.cpp",
        &Options {
            strict: true,
            preprocessed: true,
            ..Options::default()
        },
    )
    .unwrap();

    for (name, logical, lhs) in [
        ("both", "<operator>.logicalAnd", "traits::enabled<int>"),
        ("either", "<operator>.logicalOr", "traits::enabled<int>"),
        (
            "nested",
            "<operator>.logicalAnd",
            "traits::enabled<Box<int>>",
        ),
    ] {
        let function = analysis.functions.iter().find(|f| f.name == name).unwrap();
        let call = |name: &str| {
            function
                .cpg
                .nodes
                .iter()
                .find(|node| node.kind == "CALL" && node.name.as_deref() == Some(name))
                .unwrap()
        };
        let operand = call("<operator>.fieldAccess");
        let join = call(logical);
        let rhs = call("rhs");
        assert_eq!(operand.code, lhs);
        for (source, target) in [(operand, rhs), (operand, join), (rhs, join)] {
            assert!(function.cpg.edges.iter().any(|edge| {
                edge.kind == "CFG" && edge.source == source.id && edge.target == target.id
            }));
        }
    }
    let chain = analysis
        .functions
        .iter()
        .find(|f| f.name == "chain")
        .unwrap();
    let operands: Vec<_> = chain
        .cpg
        .nodes
        .iter()
        .filter(|node| node.name.as_deref() == Some("<operator>.fieldAccess"))
        .map(|node| node.code.as_str())
        .collect();
    assert_eq!(
        operands,
        ["traits::enabled<int>", "traits::enabled<Box<int>>"]
    );

    let comparisons = analysis
        .functions
        .iter()
        .find(|f| f.name == "comparisons")
        .unwrap();
    let calls: Vec<_> = comparisons
        .cpg
        .nodes
        .iter()
        .filter(|node| node.kind == "CALL")
        .filter_map(|node| node.name.as_deref())
        .collect();
    assert!(calls.contains(&"<operator>.lessThan"));
    assert!(calls.contains(&"<operator>.greaterThan"));
}

#[test]
fn logical_operator_does_not_hide_an_invalid_template_argument() {
    let analysis = analyze(
        "bool f(bool rhs) { return traits::enabled<@> || rhs; }",
        "template-logic.cpp",
        &Options {
            preprocessed: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(
        !analysis.functions[0]
            .cpg
            .nodes
            .iter()
            .any(|node| node.name.as_deref() == Some("<operator>.logicalOr"))
    );
}
