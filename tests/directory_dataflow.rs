use rust_joern::{
    analyze, analyze_sources,
    graph::{FunctionGraph, Node, Options, PropertyGraph},
};
use std::collections::{BTreeSet, HashMap};

const BODY: &str = include_str!("fixtures/ddg-directory/body.c");
const CALLER: &str = include_str!("fixtures/ddg-directory/caller.c");

type NodeKey = (String, String, i64, usize);
type EdgeKey = (NodeKey, NodeKey, String);

fn node_key(node: &Node) -> NodeKey {
    (node.kind.clone(), node.code.clone(), node.line, node.column)
}

fn edges(graph: &PropertyGraph, kind: &str) -> BTreeSet<EdgeKey> {
    let nodes: HashMap<_, _> = graph.nodes.iter().map(|n| (n.id, node_key(n))).collect();
    assert_eq!(nodes.values().collect::<BTreeSet<_>>().len(), nodes.len());
    graph
        .edges
        .iter()
        .filter(|edge| edge.kind == kind)
        .map(|edge| {
            (
                nodes[&edge.source].clone(),
                nodes[&edge.target].clone(),
                edge.label.clone().unwrap_or_default(),
            )
        })
        .collect()
}

fn oracle() -> Vec<FunctionGraph> {
    let snapshot: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/ddg-directory/joern-4.0.150.json")).unwrap();
    serde_json::from_value(snapshot["methods"].clone()).unwrap()
}

fn options() -> Options {
    Options {
        data_flow: true,
        reaching_definitions: true,
        strict: true,
        ..Default::default()
    }
}

#[test]
fn directory_ddg_matches_original_joern_for_both_source_methods() {
    let analysis = analyze_sources(&[(BODY, "body.c"), (CALLER, "caller.c")], &options()).unwrap();
    assert!(!analysis.diagnostics.iter().any(|d| d.severity == "error"));
    let oracle = oracle();
    assert_eq!(oracle.len(), 2);
    for expected in oracle {
        let actual = analysis
            .functions
            .iter()
            .find(|f| f.fullname == expected.fullname && f.filename == expected.filename)
            .unwrap();
        assert_eq!(
            edges(&actual.cpg, "REACHING_DEF"),
            edges(&expected.cpg, "REACHING_DEF"),
            "raw {}",
            expected.fullname,
        );
        assert_eq!(
            edges(actual.ddg_view.as_ref().unwrap(), "DDG"),
            edges(expected.ddg_view.as_ref().unwrap(), "DDG"),
            "projection {}",
            expected.fullname,
        );
        assert!(actual.reaching_definitions.is_some());
    }
}

#[test]
fn global_callee_context_is_confined_to_each_analysis() {
    let joined = analyze_sources(&[(CALLER, "caller.c"), (BODY, "body.c")], &options()).unwrap();
    let isolated = analyze_sources(&[(CALLER, "caller.c")], &options()).unwrap();
    let legacy = analyze(CALLER, "caller.c", &options()).unwrap();
    let caller =
        |functions: Vec<FunctionGraph>| functions.into_iter().find(|f| f.name == "caller").unwrap();
    let joined_edges = edges(&caller(joined.functions).cpg, "REACHING_DEF");
    let isolated_edges = edges(&caller(isolated.functions).cpg, "REACHING_DEF");
    assert_eq!(isolated_edges.difference(&joined_edges).count(), 2);
    assert_eq!(joined_edges.difference(&isolated_edges).count(), 0);
    assert_eq!(
        isolated_edges,
        edges(&caller(legacy.functions).cpg, "REACHING_DEF")
    );
}

fn argument_flows_to_call(function: &FunctionGraph) -> bool {
    let call = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "CALL" && n.name.as_deref() == Some("free"))
        .unwrap();
    let argument = function
        .cpg
        .edges
        .iter()
        .find(|e| e.source == call.id && e.kind == "ARGUMENT")
        .unwrap()
        .target;
    function
        .cpg
        .edges
        .iter()
        .any(|e| e.kind == "REACHING_DEF" && e.source == argument && e.target == call.id)
}

#[test]
fn batch_language_is_resolved_per_file_and_honors_an_explicit_override() {
    let sources = [
        (
            "extern void free(void *); void c_use(void *p) { free(p); }",
            "input.c",
        ),
        (
            "extern void free(void *); void cpp_use(void *p) { free(p); }",
            "input.cpp",
        ),
    ];
    for language in [None, Some("c"), Some("cpp")] {
        let mut options = options();
        options.language = language.map(str::to_string);
        let analysis = analyze_sources(&sources, &options).unwrap();
        for function in analysis
            .functions
            .iter()
            .filter(|f| f.name.ends_with("_use"))
        {
            let expected = language.map_or(function.name == "cpp_use", |l| l == "cpp");
            assert_eq!(
                argument_flows_to_call(function),
                expected,
                "{} {language:?}",
                function.name
            );
        }
    }
}

#[test]
fn batch_preserves_disabled_dataflow_strictness_and_preprocessed_options() {
    let disabled = analyze_sources(
        &[(BODY, "body.c"), (CALLER, "caller.c")],
        &Options::default(),
    )
    .unwrap();
    assert!(
        disabled
            .functions
            .iter()
            .all(|f| f.ddg.is_none() && f.ddg_view.is_none() && f.reaching_definitions.is_none())
    );
    let source = "#define value 7\nint f(void) { return value; }\n";
    let strict = Options {
        strict: true,
        ..Default::default()
    };
    assert!(analyze_sources(&[(source, "raw.c")], &strict).is_err());
    let suffix = analyze_sources(&[(source, "prepared.i")], &strict).unwrap();
    assert!(suffix.diagnostics.is_empty());
    let explicit = analyze_sources(
        &[(source, "raw.c")],
        &Options {
            preprocessed: true,
            ..strict
        },
    )
    .unwrap();
    assert!(explicit.diagnostics.is_empty());
}
