use rust_joern::{
    analyze,
    graph::{FunctionGraph, Node, NodeId, Options},
};
use std::collections::{BTreeMap, HashMap, HashSet};

fn fixtures() -> [(&'static str, &'static str, &'static str); 2] {
    [
        (
            include_str!("fixtures/ddg-builtins/builtins.c"),
            "builtins.c",
            include_str!("fixtures/ddg-builtins/joern-4.0.150-c.json"),
        ),
        (
            include_str!("fixtures/ddg-builtins/builtins.cpp"),
            "builtins.cpp",
            include_str!("fixtures/ddg-builtins/joern-4.0.150-cpp.json"),
        ),
    ]
}

fn node_keys(function: &FunctionGraph) -> HashMap<NodeId, String> {
    let nodes: HashMap<_, _> = function
        .cpg
        .nodes
        .iter()
        .map(|node| (node.id, node))
        .collect();
    let parents: HashMap<_, _> = function
        .cpg
        .edges
        .iter()
        .filter(|edge| edge.kind == "AST")
        .map(|edge| (edge.target, edge.source))
        .collect();
    fn key(
        id: NodeId,
        nodes: &HashMap<NodeId, &Node>,
        parents: &HashMap<NodeId, NodeId>,
    ) -> String {
        let node = nodes[&id];
        let symbol = matches!(
            node.kind.as_str(),
            "CALL" | "IDENTIFIER" | "METHOD" | "METHOD_PARAMETER_IN" | "METHOD_PARAMETER_OUT"
        )
        .then(|| node.name.as_deref().unwrap_or(""));
        let parent = parents.get(&id).map(|id| key(*id, nodes, parents));
        serde_json::to_string(&(
            parent,
            &node.kind,
            &node.code,
            symbol,
            node.line,
            node.column,
        ))
        .unwrap()
    }
    let keys: HashMap<_, _> = nodes
        .keys()
        .map(|&id| (id, key(id, &nodes, &parents)))
        .collect();
    // Macro clones can share CODE and source locations. Include their AST
    // ancestry so equal identities cannot conceal a misplaced argument.
    assert_eq!(keys.len(), keys.values().collect::<HashSet<_>>().len());
    keys
}

fn edge_counts(
    function: &FunctionGraph,
    keys: &HashMap<NodeId, String>,
    kind: &str,
) -> BTreeMap<(String, String, String), usize> {
    let mut edges = BTreeMap::new();
    for edge in function.cpg.edges.iter().filter(|edge| edge.kind == kind) {
        *edges
            .entry((
                keys[&edge.source].clone(),
                keys[&edge.target].clone(),
                edge.label.clone().unwrap_or_default(),
            ))
            .or_default() += 1;
    }
    edges
}

#[test]
fn builtin_cpg_lowering_and_cfg_match_original_joern() {
    for (source, filename, snapshot) in fixtures() {
        let oracle: serde_json::Value = serde_json::from_str(snapshot).unwrap();
        assert_eq!(
            oracle["generator"],
            "Original Joern 4.0.150 / dataflowOss and ReachingDefProblem"
        );
        let expected: Vec<FunctionGraph> =
            serde_json::from_value(oracle["methods"].clone()).unwrap();
        assert_eq!(expected.len(), 9);
        let actual = analyze(
            source,
            filename,
            &Options {
                data_flow: true,
                strict: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            !actual
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity == "error")
        );
        for original in expected {
            let native = actual
                .functions
                .iter()
                .find(|f| f.fullname == original.fullname)
                .unwrap();
            let old = node_keys(&original);
            let new = node_keys(native);
            assert_eq!(
                old.values().collect::<HashSet<_>>(),
                new.values().collect::<HashSet<_>>(),
                "{} source CODE, operator, and AST ancestry",
                original.fullname
            );
            for kind in ["AST", "CFG", "REF", "PARAMETER_LINK", "REACHING_DEF"] {
                assert_eq!(
                    edge_counts(&original, &old, kind),
                    edge_counts(native, &new, kind),
                    "{} {kind}",
                    original.fullname
                );
            }
            if original.name == "alignof_type" {
                let specifier = native
                    .cpg
                    .nodes
                    .iter()
                    .find(|n| n.kind == "IDENTIFIER" && n.code == "int")
                    .unwrap();
                assert_eq!(specifier.type_name.as_deref(), Some("int"));
            }
        }
    }
}
