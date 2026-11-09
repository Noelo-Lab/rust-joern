//! Joern's `dotDdg` caches each node's displayed in-edges while following
//! hidden sources, so its output depends on the stored order of a node's
//! incoming REACHING_DEF edges. The expectations below are that order in an
//! unchanged Joern 4.0.150 capture of the fixture (scripts/ddg_replay.sc).
use rust_joern::{
    analyze,
    graph::{FunctionGraph, Options},
    project_ddg,
};

const SOURCE: &str = include_str!("fixtures/ddg-decbench/definition-order.c");

fn function(name: &str) -> FunctionGraph {
    let analysis = analyze(
        SOURCE,
        "definition-order.c",
        &Options {
            data_flow: true,
            strict: true,
            ..Default::default()
        },
    )
    .unwrap();
    analysis
        .functions
        .into_iter()
        .find(|function| function.name == name)
        .unwrap()
}

/// `(code, line)` of the sources of `target`'s incoming edges, in stored order.
fn incoming(function: &FunctionGraph, code: &str, line: i64) -> Vec<(String, i64)> {
    let node = |id| {
        function
            .cpg
            .nodes
            .iter()
            .find(|node| node.id == id)
            .unwrap()
    };
    let target = function
        .cpg
        .nodes
        .iter()
        .find(|node| node.kind == "CALL" && node.code == code && node.line == line)
        .unwrap();
    function
        .cpg
        .edges
        .iter()
        .filter(|edge| edge.kind == "REACHING_DEF" && edge.target == target.id)
        .map(|edge| node(edge.source))
        .filter(|source| source.kind != "METHOD")
        .map(|source| (source.code.clone(), source.line))
        .collect()
}

fn expected(sources: &[(&str, i64)]) -> Vec<(String, i64)> {
    sources
        .iter()
        .map(|&(code, line)| (code.into(), line))
        .collect()
}

#[test]
fn incoming_definitions_are_stored_in_definition_number_order() {
    // Definitions are numbered along ReachingDefFlowGraph's reverse post
    // order, so an argument precedes the call it belongs to: `cursor[1]`
    // before `&cursor[1]`, although the call comes first in the AST.
    let walk = function("walk_suffix");
    assert_eq!(
        incoming(&walk, "cursor[1]", 7),
        expected(&[
            ("cursor", 4),
            ("cursor", 8),
            ("cursor[1]", 8),
            ("&cursor[1]", 8)
        ])
    );
    assert_eq!(
        incoming(&walk, "cursor[1]", 8),
        expected(&[
            ("cursor", 4),
            ("cursor[1]", 7),
            ("cursor", 8),
            ("&cursor[1]", 8)
        ])
    );
    let shift = function("shift_window");
    assert_eq!(
        incoming(&shift, "head[1]", 20),
        expected(&[
            ("head", 15),
            ("head[1]", 18),
            ("head", 20),
            ("&head[1]", 20)
        ])
    );
}

#[test]
fn projection_request_reproduces_the_analysis_projection() {
    // The replay entry point runs the same projection as the analysis, and
    // reports the destination order it evaluated: entry, exit, parameters,
    // then the method's CFG nodes.
    for name in ["walk_suffix", "shift_window", "fold_pairs"] {
        let function = function(name);
        let request = serde_json::json!({"function": function, "cpp": false});
        let response: serde_json::Value =
            serde_json::from_str(&project_ddg(&request.to_string()).unwrap()).unwrap();
        let view = serde_json::to_value(function.ddg_view.as_ref().unwrap()).unwrap();
        assert_eq!(response["nodes"], view["nodes"], "{name}");
        assert_eq!(response["edges"], view["edges"], "{name}");
        let kinds: Vec<_> = response["order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| {
                function.cpg.nodes[id.as_u64().unwrap() as usize]
                    .kind
                    .as_str()
            })
            .collect();
        assert_eq!(kinds[..2], ["METHOD", "METHOD_RETURN"], "{name}");
        let parameters = kinds
            .iter()
            .skip(2)
            .take_while(|&&kind| kind == "METHOD_PARAMETER_IN")
            .count();
        assert!(parameters >= 2, "{name}");
        assert!(
            kinds[2 + parameters..].iter().all(|kind| !matches!(
                *kind,
                "METHOD_PARAMETER_IN" | "METHOD_RETURN" | "CONTROL_STRUCTURE" | "JUMP_TARGET"
            )),
            "{name}"
        );
    }
}

#[test]
fn projection_request_rejects_malformed_input() {
    assert!(
        project_ddg("{}")
            .unwrap_err()
            .starts_with("invalid request")
    );
}
