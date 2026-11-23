use super::*;
use crate::graph::Cfg;
use std::collections::{BTreeMap, BTreeSet};

#[derive(serde::Deserialize)]
struct Oracle {
    methods: Vec<FunctionGraph>,
    internal_methods: HashMap<String, HashSet<String>>,
}

#[derive(serde::Deserialize)]
struct ProjectionOracle {
    methods: Vec<ProjectionMethod>,
    all_internal_methods: HashMap<String, HashSet<String>>,
}

#[derive(serde::Deserialize)]
struct ProjectionMethod {
    filename: String,
    fullname: String,
    nodes: Vec<u32>,
    edges: Vec<Edge>,
}

fn projection_oracles() -> (Oracle, ProjectionOracle) {
    let mut oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow/joern-4.0.150.json"
    ))
    .unwrap();
    let builtins: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow/joern-4.0.150-builtins.json"
    ))
    .unwrap();
    oracle.methods.extend(builtins.methods);
    oracle.internal_methods.extend(builtins.internal_methods);
    let projection = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow/joern-4.0.150-ddg-projection.json"
    ))
    .unwrap();
    (oracle, projection)
}

fn assert_projection(
    actual: &FunctionGraph,
    projected: &PropertyGraph,
    expected: &FunctionGraph,
    reference: &ProjectionMethod,
) {
    let (actual_keys, expected_keys) = (ast_keys(actual), ast_keys(expected));
    let nodes: BTreeSet<_> = projected
        .nodes
        .iter()
        .map(|n| &actual_keys[&n.id])
        .collect();
    let reference_nodes: BTreeSet<_> = reference
        .nodes
        .iter()
        .map(|id| &expected_keys[id])
        .collect();
    assert_eq!(
        nodes, reference_nodes,
        "displayed DDG nodes {}",
        expected.fullname
    );
    let edges = |edges: &[Edge], keys: &HashMap<u32, String>| -> BTreeSet<_> {
        edges
            .iter()
            .map(|e| {
                (
                    keys[&e.source].clone(),
                    keys[&e.target].clone(),
                    e.label.clone().unwrap_or_default(),
                )
            })
            .collect()
    };
    assert_eq!(
        edges(&projected.edges, &actual_keys),
        edges(&reference.edges, &expected_keys),
        "displayed DDG edges {}",
        expected.fullname,
    );
    assert!(projected.edges.iter().all(|e| e.kind == "DDG"));
}

#[test]
fn original_joern_displayed_ddg_matches_by_expression_role() {
    let (oracle, projection) = projection_oracles();
    assert_eq!(projection.methods.len(), 60);
    for reference in &projection.methods {
        let expected = oracle
            .methods
            .iter()
            .find(|f| f.filename == reference.filename && f.fullname == reference.fullname)
            .unwrap();
        let mut actual = expected.clone();
        let ids: HashMap<_, _> = actual
            .cpg
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.id, 99_001 + i as u32 * 11))
            .collect();
        for node in &mut actual.cpg.nodes {
            node.id = ids[&node.id];
        }
        for edge in &mut actual.cpg.edges {
            edge.source = ids[&edge.source];
            edge.target = ids[&edge.target];
        }
        let projected = project_ddg(
            &actual,
            &oracle.internal_methods[&reference.filename],
            &projection.all_internal_methods[&reference.filename],
            reference.filename.ends_with(".cpp"),
        );
        assert_projection(&actual, &projected, expected, reference);
    }
}

#[test]
fn native_frontend_displayed_ddg_matches_original_c_and_cpp() {
    let (oracle, projection) = projection_oracles();
    let mut count = 0;
    for (filename, source) in [
        (
            "flow.c",
            include_str!("../../tests/fixtures/dataflow/flow.c"),
        ),
        (
            "semantics.c",
            include_str!("../../tests/fixtures/dataflow/semantics.c"),
        ),
        (
            "extra.cpp",
            include_str!("../../tests/fixtures/dataflow/extra.cpp"),
        ),
        (
            "builtins.c",
            include_str!("../../tests/fixtures/dataflow/builtins.c"),
        ),
    ] {
        let analysis = crate::analyze(
            source,
            filename,
            &crate::graph::Options {
                data_flow: true,
                reaching_definitions: true,
                ..Default::default()
            },
        )
        .unwrap();
        for reference in projection
            .methods
            .iter()
            .filter(|f| f.filename == filename || filename == "builtins.c" && f.filename.is_empty())
        {
            let expected = oracle
                .methods
                .iter()
                .find(|f| f.filename == reference.filename && f.fullname == reference.fullname)
                .unwrap();
            let actual = analysis
                .functions
                .iter()
                .find(|f| f.fullname == expected.fullname)
                .unwrap_or_else(|| panic!("missing {}", expected.fullname));
            assert_native_dataflow(actual, expected);
            assert_projection(
                actual,
                actual.ddg_projection.as_ref().expect("DDG projection"),
                expected,
                reference,
            );
            if let Some((parent_name, _)) = actual.fullname.rsplit_once(".<lambda>") {
                let parent = analysis
                    .functions
                    .iter()
                    .find(|f| f.fullname == parent_name)
                    .unwrap();
                let (actual_keys, expected_keys) = (ast_keys(actual), ast_keys(expected));
                for expected_node in expected.cpg.nodes.iter().filter(|n| n.kind == "IDENTIFIER") {
                    let node = actual
                        .cpg
                        .nodes
                        .iter()
                        .find(|n| actual_keys[&n.id] == expected_keys[&expected_node.id])
                        .unwrap();
                    assert_eq!(
                        node.type_name, expected_node.type_name,
                        "lambda {}",
                        node.code
                    );
                    if name(node) == "x" {
                        let target = node.external_ref.as_ref().expect("captured x provenance");
                        assert_eq!(target.method_full_name, parent.fullname);
                        let binding = parent
                            .cpg
                            .nodes
                            .iter()
                            .find(|n| n.id == target.node)
                            .unwrap();
                        assert!(input(binding));
                        assert_eq!(name(binding), "x");
                    } else if name(node) == "y" {
                        assert!(
                            node.external_ref.is_none(),
                            "original lambda parameter scope is popped before its body"
                        );
                    }
                }
            }
            count += 1;
        }
    }
    assert_eq!(count, 60);
}

#[test]
fn displayed_ddg_hidden_loop_paths_fit_a_small_worker_stack() {
    let source = format!(
        "int f(int x) {{ x+2; while(x) {{ {} }} return x; }}",
        "x+1;".repeat(512)
    );
    let function = crate::analyze(
        &source,
        "hidden-loop.c",
        &crate::graph::Options {
            data_flow: true,
            ..Default::default()
        },
    )
    .unwrap()
    .functions
    .remove(0);
    // A loop back edge creates a hidden chain through the read-only `x`
    // arguments. Re-expanding it must not use one Rust stack frame per read.
    let projected = std::thread::Builder::new()
        .stack_size(128 * 1024)
        .spawn(move || project_ddg(&function, &HashSet::new(), &HashSet::new(), false))
        .unwrap()
        .join()
        .unwrap();
    assert_eq!(projected.nodes.iter().filter(|n| call(n)).count(), 513);
    assert!(
        projected
            .edges
            .iter()
            .any(|e| e.label.as_deref() == Some("x"))
    );
    assert!(projected.edges.iter().all(|e| e.kind == "DDG"));
}

// IDs vary across Joern and Rust, so compare expression/operator/argument
// roles, source positions and variable labels. The candidate's IDs are
// deliberately sparse and unrelated to the saved oracle's IDs.
fn node_keys(function: &FunctionGraph) -> HashMap<u32, String> {
    let nodes: HashMap<_, _> = function.cpg.nodes.iter().map(|n| (n.id, n)).collect();
    let arguments: HashMap<_, _> = function
        .cpg
        .edges
        .iter()
        .filter(|e| e.kind == "ARGUMENT")
        .map(|e| (e.target, (e.source, e.label.clone())))
        .collect();
    nodes
        .iter()
        .map(|(&id, node)| {
            let mut role = Vec::new();
            let mut at = id;
            while let Some((parent, position)) = arguments.get(&at) {
                let node = nodes[parent];
                role.push((&node.kind, &node.name, &node.code, position));
                at = *parent;
            }
            (
                id,
                serde_json::to_string(&(
                    &node.kind,
                    &node.name,
                    &node.code,
                    node.line,
                    node.column,
                    role,
                ))
                .unwrap(),
            )
        })
        .collect()
}

fn definitions(function: &FunctionGraph) -> BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> {
    let keys = node_keys(function);
    definitions_by_keys(function, &keys)
}

fn definitions_by_keys(
    function: &FunctionGraph,
    keys: &HashMap<u32, String>,
) -> BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> {
    function
        .reaching_definitions
        .as_ref()
        .unwrap()
        .iter()
        .map(|d| {
            (
                keys[&d.node].clone(),
                (
                    d.incoming.iter().map(|id| keys[id].clone()).collect(),
                    d.outgoing.iter().map(|id| keys[id].clone()).collect(),
                ),
            )
        })
        .collect()
}

fn dependencies(function: &FunctionGraph) -> BTreeSet<(String, String, String)> {
    let keys = node_keys(function);
    edges_by_keys(function, &keys, "REACHING_DEF")
}

fn edges_by_keys(
    function: &FunctionGraph,
    keys: &HashMap<u32, String>,
    kind: &str,
) -> BTreeSet<(String, String, String)> {
    function
        .cpg
        .edges
        .iter()
        .filter(|e| e.kind == kind)
        .map(|e| {
            (
                keys[&e.source].clone(),
                keys[&e.target].clone(),
                e.label.clone().unwrap_or_default(),
            )
        })
        .collect()
}

// Frontends may allocate and order nodes differently. Match a node by its
// exact AST position, call operator/code and argument role. Code/type/line
// metadata on other nodes is a separate CPG property comparison; this test
// compares the actual CFG, bindings, data dependencies and RD sets.
fn ast_keys(function: &FunctionGraph) -> HashMap<u32, String> {
    let nodes: HashMap<_, _> = function.cpg.nodes.iter().map(|n| (n.id, n)).collect();
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut parent = HashMap::new();
    let mut args = HashMap::new();
    for edge in &function.cpg.edges {
        if edge.kind == "AST" {
            children.entry(edge.source).or_default().push(edge.target);
            parent.insert(edge.target, edge.source);
        } else if edge.kind == "ARGUMENT" {
            args.insert((edge.source, edge.target), edge.label.as_deref());
        }
    }
    nodes
        .iter()
        .map(|(&id, node)| {
            let mut role = Vec::new();
            let mut at = id;
            loop {
                let n = nodes[&at];
                let position = parent.get(&at).map(|&p| {
                    let siblings = &children[&p];
                    if call(nodes[&p])
                        && let Some(index) = args.get(&(p, at))
                    {
                        format!("argument:{index:?}")
                    } else {
                        format!(
                            "child:{}",
                            siblings
                                .iter()
                                .filter(|&&s| nodes[&s].kind == n.kind)
                                .position(|&s| s == at)
                                .unwrap()
                        )
                    }
                });
                role.push((
                    &n.kind,
                    call(n).then(|| (name(n), n.code.as_str())),
                    position,
                ));
                match parent.get(&at) {
                    Some(&p) => at = p,
                    None => break,
                }
            }
            // Also pin the binding/leaf identity, without depending on the
            // optional representation of an unnamed parameter's name.
            let symbol = matches!(
                node.kind.as_str(),
                "IDENTIFIER" | "LITERAL" | "METHOD_PARAMETER_IN" | "METHOD_PARAMETER_OUT"
            )
            .then(|| (name(node), node.code.as_str()));
            (id, serde_json::to_string(&(role, symbol)).unwrap())
        })
        .collect()
}

fn assert_native_dataflow(actual: &FunctionGraph, expected: &FunctionGraph) {
    let (actual_keys, expected_keys) = (ast_keys(actual), ast_keys(expected));
    for keys in [&actual_keys, &expected_keys] {
        assert_eq!(
            keys.len(),
            keys.values().collect::<HashSet<_>>().len(),
            "duplicate AST role {}:{}",
            expected.filename,
            expected.fullname
        );
    }
    for kind in ["CFG", "REF", "REACHING_DEF"] {
        assert_eq!(
            edges_by_keys(actual, &actual_keys, kind),
            edges_by_keys(expected, &expected_keys, kind),
            "{kind} {}:{}",
            expected.filename,
            expected.fullname
        );
    }
    assert_eq!(
        definitions_by_keys(actual, &actual_keys),
        definitions_by_keys(expected, &expected_keys),
        "RD {}:{}",
        expected.filename,
        expected.fullname
    );
    let edges = |function: &FunctionGraph, keys: &HashMap<u32, String>| {
        let mut edges: Vec<_> = function
            .cpg
            .edges
            .iter()
            .filter(|e| e.kind == "REACHING_DEF")
            .map(|e| {
                (
                    keys[&e.source].clone(),
                    keys[&e.target].clone(),
                    e.label.clone().unwrap_or_default(),
                )
            })
            .collect();
        edges.sort();
        edges
    };
    assert_eq!(
        edges(actual, &actual_keys),
        edges(expected, &expected_keys),
        "raw edge multiplicity {}:{}",
        expected.filename,
        expected.fullname
    );
}

#[test]
fn native_c_frontend_matches_original_cfg_bindings_ddg_and_rd_sets() {
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow/joern-4.0.150.json"
    ))
    .unwrap();
    let mut count = 0;
    for (filename, source) in [
        (
            "flow.c",
            include_str!("../../tests/fixtures/dataflow/flow.c"),
        ),
        (
            "semantics.c",
            include_str!("../../tests/fixtures/dataflow/semantics.c"),
        ),
    ] {
        let analysis = crate::analyze(
            source,
            filename,
            &crate::graph::Options {
                data_flow: true,
                reaching_definitions: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(
            !analysis.diagnostics.iter().any(|d| d.severity == "error"),
            "{:?}",
            analysis.diagnostics
        );
        assert_eq!(
            analysis.functions.len(),
            oracle
                .methods
                .iter()
                .filter(|f| f.filename == filename)
                .count(),
            "method count {filename}"
        );
        for expected in oracle.methods.iter().filter(|f| f.filename == filename) {
            let actual = analysis
                .functions
                .iter()
                .find(|f| f.fullname == expected.fullname)
                .unwrap_or_else(|| panic!("missing {}:{}", filename, expected.fullname));
            assert_native_dataflow(actual, expected);
            count += 1;
        }
    }
    assert_eq!(count, 33);
}

#[test]
fn native_builtin_macro_callers_and_stub_methods_match_original_dataflow() {
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow/joern-4.0.150-builtins.json"
    ))
    .unwrap();
    let analysis = crate::analyze(
        include_str!("../../tests/fixtures/dataflow/builtins.c"),
        "builtins.c",
        &crate::graph::Options {
            data_flow: true,
            reaching_definitions: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!analysis.diagnostics.iter().any(|d| d.severity == "error"));
    assert_eq!(analysis.functions.len(), oracle.methods.len());
    assert_eq!(oracle.methods.len(), 8);
    for expected in oracle.methods {
        let actual = analysis
            .functions
            .iter()
            .find(|f| f.fullname == expected.fullname)
            .unwrap_or_else(|| panic!("missing {}", expected.fullname));
        assert_native_dataflow(actual, &expected);
    }
}

#[test]
fn native_compound_assignment_operators_match_original_dataflow() {
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow-audit/joern-4.0.150-assignments.json"
    ))
    .unwrap();
    let analysis = crate::analyze(
        include_str!("../../tests/fixtures/dataflow-audit/assignment-operators.c"),
        "assignment-operators.c",
        &crate::graph::Options {
            reaching_definitions: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!analysis.diagnostics.iter().any(|d| d.severity == "error"));
    assert_eq!(analysis.functions.len(), 10);
    assert_eq!(oracle.methods.len(), 10);
    for expected in oracle.methods {
        let actual = analysis
            .functions
            .iter()
            .find(|f| f.fullname == expected.fullname)
            .unwrap_or_else(|| panic!("missing {}", expected.fullname));
        assert_native_dataflow(actual, &expected);
    }
}

#[test]
fn native_return_display_types_match_original_metadata() {
    let oracle: Oracle = serde_json::from_str(include_str!(
        "../../tests/fixtures/dataflow-audit/joern-4.0.150-types.json"
    ))
    .unwrap();
    for (filename, source) in [
        (
            "type-metadata.c",
            include_str!("../../tests/fixtures/dataflow-audit/type-metadata.c"),
        ),
        (
            "type-metadata.cpp",
            include_str!("../../tests/fixtures/dataflow-audit/type-metadata.cpp"),
        ),
        (
            "type-detail.c",
            include_str!("../../tests/fixtures/dataflow-audit/type-detail.c"),
        ),
        (
            "type-detail.cpp",
            include_str!("../../tests/fixtures/dataflow-audit/type-detail.cpp"),
        ),
    ] {
        let actual = crate::analyze(source, filename, &Default::default()).unwrap();
        let expected: Vec<_> = oracle
            .methods
            .iter()
            .filter(|f| f.filename == filename)
            .collect();
        assert_eq!(
            actual.functions.len(),
            expected.len(),
            "coverage {filename}"
        );
        for expected in expected {
            let function = actual
                .functions
                .iter()
                .find(|f| f.name == expected.name)
                .unwrap_or_else(|| panic!("missing {filename}:{}", expected.name));
            assert_eq!(
                function.return_type, expected.return_type,
                "return display {filename}:{}",
                expected.name
            );
        }
    }
}

#[test]
fn original_joern_c_cpp_solver_snapshots_match_by_expression_role() {
    let mut count = 0;
    for snapshot in [
        include_str!("../../tests/fixtures/dataflow/joern-4.0.150.json"),
        include_str!("../../tests/fixtures/dataflow/joern-4.0.150-builtins.json"),
    ] {
        let oracle: Oracle = serde_json::from_str(snapshot).unwrap();
        for expected in oracle.methods {
            let mut actual = expected.clone();
            let ids: HashMap<_, _> = actual
                .cpg
                .nodes
                .iter()
                .enumerate()
                .map(|(i, n)| (n.id, 100_003 + 17 * i as u32))
                .collect();
            for node in &mut actual.cpg.nodes {
                node.id = ids[&node.id];
            }
            for edge in &mut actual.cpg.edges {
                edge.source = ids[&edge.source];
                edge.target = ids[&edge.target];
            }
            actual.ddg = None;
            actual.reaching_definitions = None;
            apply_with_methods(
                &mut actual,
                true,
                &oracle.internal_methods[&expected.filename],
            );
            assert_eq!(
                definitions(&actual),
                definitions(&expected),
                "RD {}:{}",
                expected.filename,
                expected.fullname
            );
            assert_eq!(
                dependencies(&actual),
                dependencies(&expected),
                "DDG {}:{}",
                expected.filename,
                expected.fullname
            );
            count += 1;
        }
    }
    assert_eq!(count, 60);
}

#[test]
fn broad_original_graphs_match_solver_overlay_and_labeled_dot_projection() {
    #[derive(serde::Deserialize)]
    struct Audit {
        methods: Vec<FunctionGraph>,
        internal_methods: HashMap<String, HashSet<String>>,
        all_internal_methods: HashMap<String, HashSet<String>>,
    }
    for (snapshot, count) in [
        (
            include_str!("../../tests/fixtures/dataflow-audit/joern-4.0.150.json"),
            82,
        ),
        (
            include_str!("../../tests/fixtures/dataflow-audit/joern-4.0.150-types.json"),
            108,
        ),
        (
            include_str!("../../tests/fixtures/dataflow-audit/joern-4.0.150-assignments.json"),
            10,
        ),
        (
            include_str!("../../tests/fixtures/dataflow-audit/joern-4.0.150-construction.json"),
            11,
        ),
    ] {
        let oracle: Audit = serde_json::from_str(snapshot).unwrap();
        assert_eq!(oracle.methods.len(), count);
        for expected in oracle.methods {
            let mut actual = expected.clone();
            actual.ddg = None;
            actual.ddg_view = None;
            actual.reaching_definitions = None;
            let nonstub = &oracle.internal_methods[&expected.filename];
            let internal = &oracle.all_internal_methods[&expected.filename];
            apply_with_methods(&mut actual, true, nonstub);
            assert_eq!(
                definitions(&actual),
                definitions(&expected),
                "RD {}",
                expected.fullname
            );
            assert_eq!(
                dependencies(&actual),
                dependencies(&expected),
                "overlay {}",
                expected.fullname
            );
            let edges = |f: &FunctionGraph| {
                let mut edges: Vec<_> = f
                    .cpg
                    .edges
                    .iter()
                    .filter(|e| e.kind == "REACHING_DEF")
                    .map(|e| (e.source, e.target, e.label.clone().unwrap_or_default()))
                    .collect();
                edges.sort();
                edges
            };
            assert_eq!(
                edges(&actual),
                edges(&expected),
                "raw edge multiplicity {}",
                expected.fullname
            );
            let cpp = expected.filename.ends_with(".cpp");
            let projected = crate::ddg::project(&actual, nonstub, internal, cpp);
            let original = expected.ddg_view.as_ref().unwrap();
            assert_eq!(
                projected.nodes.iter().map(|n| n.id).collect::<HashSet<_>>(),
                original.nodes.iter().map(|n| n.id).collect::<HashSet<_>>(),
                "projected vertices {}",
                expected.fullname,
            );
            assert_eq!(
                projected.edges.iter().collect::<HashSet<_>>(),
                original.edges.iter().collect::<HashSet<_>>(),
                "projected labeled edges {}",
                expected.fullname,
            );
        }
    }
}

fn cross_argument_edges(function: &FunctionGraph, callee: &str) -> usize {
    let call = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "CALL" && name(n) == callee)
        .unwrap();
    let arguments: HashSet<_> = function
        .cpg
        .edges
        .iter()
        .filter(|e| e.kind == "ARGUMENT" && e.source == call.id)
        .map(|e| e.target)
        .collect();
    function
        .cpg
        .edges
        .iter()
        .filter(|e| {
            e.kind == "REACHING_DEF"
                && e.source != e.target
                && arguments.contains(&e.source)
                && arguments.contains(&e.target)
        })
        .count()
}

#[test]
fn native_frontend_distinguishes_internal_body_from_empty_and_prototype_calls() {
    let source = include_str!("../../tests/fixtures/dataflow/semantics.c");
    let analysis = crate::analyze(
        source,
        "semantics.c",
        &crate::graph::Options {
            data_flow: true,
            reaching_definitions: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        !analysis.diagnostics.iter().any(|d| d.severity == "error"),
        "{:?}",
        analysis.diagnostics
    );
    let calls = analysis
        .functions
        .iter()
        .find(|f| f.name == "calls")
        .unwrap();
    // Both an empty real definition and a declaration are stubs according
    // to Joern's raw CFG rule; only a real executable body suppresses
    // implicit flows between its arguments.
    assert_eq!(cross_argument_edges(calls, "prototype"), 2);
    assert_eq!(cross_argument_edges(calls, "empty"), 2);
    assert_eq!(cross_argument_edges(calls, "body"), 0);
}

#[test]
fn explicit_cpp_language_controls_summaries_for_c_named_inputs() {
    for (language, expected_flow) in [("c", false), ("cpp", true)] {
        let analysis = crate::analyze(
            "extern void free(void *); void f(void *p) { free(p); }",
            "explicit.c",
            &crate::graph::Options {
                language: Some(language.into()),
                data_flow: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!analysis.diagnostics.iter().any(|d| d.severity == "error"));
        let function = analysis.functions.iter().find(|f| f.name == "f").unwrap();
        let call = function
            .cpg
            .nodes
            .iter()
            .find(|n| n.kind == "CALL" && name(n) == "free")
            .unwrap();
        let argument = function
            .cpg
            .edges
            .iter()
            .find(|e| e.source == call.id && e.kind == "ARGUMENT")
            .unwrap()
            .target;
        let flows_to_return_value = function
            .cpg
            .edges
            .iter()
            .any(|e| e.kind == "REACHING_DEF" && e.source == argument && e.target == call.id);
        assert_eq!(flows_to_return_value, expected_flow, "{language}");
    }
}

#[test]
fn native_base_parameter_outputs_and_cfg_are_independent_of_dataflow_toggle() {
    let source = "int f(int x) { return x; }";
    let disabled = crate::analyze(source, "toggle.c", &Default::default()).unwrap();
    let enabled = crate::analyze(
        source,
        "toggle.c",
        &crate::graph::Options {
            data_flow: true,
            reaching_definitions: true,
            ..Default::default()
        },
    )
    .unwrap();
    let off = &disabled.functions[0];
    let on = &enabled.functions[0];
    assert!(off.ddg.is_none() && off.reaching_definitions.is_none());
    assert_eq!(off.cpg.nodes.iter().filter(|n| output(n)).count(), 1);
    assert_eq!(
        off.cpg
            .edges
            .iter()
            .filter(|e| e.kind == "PARAMETER_LINK")
            .count(),
        1
    );
    let mut base = on.cpg.clone();
    base.edges.retain(|e| e.kind != "REACHING_DEF");
    assert_eq!(
        serde_json::to_value(&off.cpg).unwrap(),
        serde_json::to_value(base).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&off.cfg).unwrap(),
        serde_json::to_value(&on.cfg).unwrap()
    );
}

// int f(int x) { int y; if (x > 0) y = 1; else y = 2; return y; }
fn branch() -> FunctionGraph {
    let nodes = [
        ("METHOD", "f", "f"),
        ("METHOD_RETURN", "RET", ""),
        ("METHOD_PARAMETER_IN", "int x", "x"),
        ("IDENTIFIER", "x", "x"),
        ("LITERAL", "0", ""),
        ("CALL", "x > 0", "<operator>.greaterThan"),
        ("IDENTIFIER", "y", "y"),
        ("LITERAL", "1", ""),
        ("CALL", "y = 1", "<operator>.assignment"),
        ("IDENTIFIER", "y", "y"),
        ("LITERAL", "2", ""),
        ("CALL", "y = 2", "<operator>.assignment"),
        ("IDENTIFIER", "y", "y"),
        ("RETURN", "return y", ""),
        ("LOCAL", "int y", "y"),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, (kind, code, symbol))| Node {
        id: i as u32,
        kind: kind.into(),
        code: code.into(),
        name: (!symbol.is_empty()).then(|| symbol.into()),
        method_full_name: None,
        external_ref: None,
        cfg_nop: None,
        type_name: None,
        line: 1,
        column: 1,
        start_byte: 0,
        end_byte: 0,
    })
    .collect();
    let mut graph = PropertyGraph {
        nodes,
        edges: Vec::new(),
    };
    for (source, target) in [
        (0, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (5, 9),
        (6, 7),
        (7, 8),
        (8, 12),
        (9, 10),
        (10, 11),
        (11, 12),
        (12, 13),
        (13, 1),
    ] {
        graph.edges.push(Edge {
            source,
            target,
            kind: "CFG".into(),
            label: None,
        });
    }
    for (parent, args) in [
        (5, vec![3, 4]),
        (8, vec![6, 7]),
        (11, vec![9, 10]),
        (13, vec![12]),
    ] {
        for (i, target) in args.into_iter().enumerate() {
            graph.edges.push(Edge {
                source: parent,
                target,
                kind: "AST".into(),
                label: None,
            });
            graph.edges.push(Edge {
                source: parent,
                target,
                kind: "ARGUMENT".into(),
                label: Some((i + 1).to_string()),
            });
        }
    }
    for (source, target) in [(3, 2), (6, 14), (9, 14), (12, 14)] {
        graph.edges.push(Edge {
            source,
            target,
            kind: "REF".into(),
            label: None,
        });
    }
    FunctionGraph {
        name: "f".into(),
        fullname: "f".into(),
        filename: "f.c".into(),
        return_type: "int".into(),
        signature: "int f(int)".into(),
        start_line: 1,
        end_line: 1,
        cpg: graph,
        cfg: Cfg::default(),
        ddg: None,
        ddg_view: None,
        ddg_projection: None,
        reaching_definitions: None,
    }
}

#[test]
fn branch_definitions_reach_return_and_assignment_flows_to_lhs() {
    let mut function = branch();
    apply(&mut function, true);
    let definitions = function
        .reaching_definitions
        .as_ref()
        .unwrap()
        .iter()
        .find(|d| d.node == 13)
        .unwrap();
    assert!(definitions.incoming.contains(&6));
    assert!(definitions.incoming.contains(&9));
    let has = |source, target| {
        function
            .ddg
            .as_ref()
            .unwrap()
            .edges
            .iter()
            .any(|e| e.source == source && e.target == target)
    };
    assert!(has(6, 12) && has(9, 12));
    assert!(has(7, 6) && has(10, 9));
    assert!(!has(6, 7) && !has(9, 10));
    assert!(has(13, 1));
}

#[test]
fn optional_sets_and_repeated_pass_do_not_change_graph() {
    let mut function = branch();
    apply(&mut function, true);
    let count = function.cpg.nodes.len();
    let edges: HashSet<_> = function
        .ddg
        .as_ref()
        .unwrap()
        .edges
        .iter()
        .cloned()
        .collect();
    apply(&mut function, false);
    assert!(function.reaching_definitions.is_none());
    assert_eq!(function.cpg.nodes.len(), count);
    assert_eq!(
        function
            .ddg
            .as_ref()
            .unwrap()
            .edges
            .iter()
            .cloned()
            .collect::<HashSet<_>>(),
        edges
    );
}

#[test]
fn original_overlay_cutoff_keeps_explicit_solver_queries_available() {
    // The original live oracle produces edges at 3999 definitions and no
    // edges at 4001. Check the exact 4000 boundary as well: each increment
    // generates its CALL and IDENTIFIER, and an input parameter adds one.
    let increments = "x++;".repeat(2000);
    let at_limit = format!("int f() {{ int x; {increments} return x; }}");
    let over_limit = format!("int f(int x) {{ {increments} return x; }}");
    let options = crate::graph::Options {
        data_flow: true,
        ..Default::default()
    };
    let analyzed = crate::analyze(&at_limit, "threshold.c", &options).unwrap();
    assert!(!analyzed.functions[0].ddg.as_ref().unwrap().edges.is_empty());
    let skipped = crate::analyze(&over_limit, "threshold.c", &options).unwrap();
    let function = &skipped.functions[0];
    assert!(function.ddg.as_ref().unwrap().nodes.is_empty());
    assert!(function.ddg_view.as_ref().unwrap().nodes.is_empty());
    assert!(!function.cpg.edges.iter().any(|e| e.kind == "REACHING_DEF"));
    assert!(function.reaching_definitions.is_none());

    let analyzed = crate::analyze(
        &over_limit,
        "threshold.c",
        &crate::graph::Options {
            reaching_definitions: true,
            ..Default::default()
        },
    )
    .unwrap();
    let function = &analyzed.functions[0];
    assert!(function.ddg.as_ref().unwrap().edges.is_empty());
    assert!(
        function
            .reaching_definitions
            .as_ref()
            .unwrap()
            .iter()
            .any(|set| !set.incoming.is_empty())
    );
}

#[test]
fn normalized_access_paths_and_default_summaries() {
    assert!(exact_path(
        &[Access::Deref, Access::Field("f".into())],
        &[Access::Deref, Access::Field("f".into())]
    ));
    assert!(!exact_path(
        &[Access::Deref, Access::Variable],
        &[Access::Deref, Access::Field("f".into())]
    ));
    assert!(
        semantics("<operator>.assignment")
            .unwrap()
            .flows
            .contains(&(2, 1))
    );
    assert!(semantics("<operator>.sizeOf").unwrap().flows.is_empty());
}
