use super::cpp_call_leaf;
use crate::{analyze, graph::Options};
use serde_json::Value;
use std::collections::HashMap;

#[test]
fn cpp_call_leaves_ignore_template_argument_qualification() {
    for (name, expected) in [
        ("::demo::get<types::Box<int>>", "get<types::Box<int>>"),
        ("demo::operator+<types::Tag>", "operator+<types::Tag>"),
        ("demo::operator< <types::Tag>", "operator< <types::Tag>"),
        ("demo::operator>><types::Tag>", "operator>><types::Tag>"),
        ("demo::myoperator<types::Tag>", "myoperator<types::Tag>"),
        ("demo::operator_names", "operator_names"),
    ] {
        assert_eq!(cpp_call_leaf(name), expected);
    }
}

#[test]
fn gnu_case_range_keeps_joern_ellipsis_operator() {
    let analysis = analyze(
        "int f(int x) { switch (x) { case 1 ... 3: return 1; } return 0; }",
        "input.c",
        &Options::default(),
    )
    .unwrap();
    let cpg = serde_json::to_value(&analysis.functions[0].cpg).unwrap();
    assert!(
        cpg["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["name"] == "<operator>.op_ellipses"),
        "{cpg}"
    );
}

#[test]
fn nested_function_method_is_an_ast_child_without_parent_cfg_execution() {
    // This definition shape is captured from unchanged Joern in
    // nested_function_declarations.c: the work call belongs to nested.
    let nested_code = "int nested(int x) { return work(x); }";
    let source =
        format!("int work(int);\nint valid_outer(int n) {{ {nested_code} return nested(n); }}");
    let analysis = analyze(
        &source,
        "input.c",
        &Options {
            strict: true,
            preprocessed: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert!(analysis.diagnostics.is_empty());
    let outer = analysis
        .functions
        .iter()
        .find(|function| function.name == "valid_outer")
        .unwrap();
    let nested = analysis
        .functions
        .iter()
        .find(|function| function.name == "nested")
        .unwrap();
    let marker = outer
        .cpg
        .nodes
        .iter()
        .find(|node| node.kind == "METHOD" && node.name.as_deref() == Some("nested"))
        .unwrap();
    assert_eq!(marker.code, nested_code);
    assert_eq!(marker.line, 2);
    assert_eq!(
        marker.method_full_name.as_deref(),
        Some(nested.fullname.as_str())
    );
    let target = marker.external_ref.as_ref().unwrap();
    assert_eq!(target.method_full_name, nested.fullname);
    assert_eq!(target.node, nested.cpg.nodes[0].id);
    assert!(outer.cpg.edges.iter().any(|edge| {
        edge.kind == "AST"
            && edge.target == marker.id
            && outer.cpg.nodes[edge.source as usize].kind == "BLOCK"
    }));
    assert!(!outer.cpg.edges.iter().any(|edge| {
        edge.kind == "CFG" && (edge.source == marker.id || edge.target == marker.id)
    }));
    assert!(
        !outer
            .cpg
            .nodes
            .iter()
            .any(|node| node.kind == "CALL" && node.name.as_deref() == Some("work"))
    );
    assert!(
        nested
            .cpg
            .nodes
            .iter()
            .any(|node| node.kind == "CALL" && node.name.as_deref() == Some("work"))
    );
    assert_eq!(outer.cfg.nodes.len(), 1);
    assert!(outer.cfg.edges.is_empty());
    assert!(!outer.cfg.nodes[0].statements.contains(&marker.id));
}

#[test]
fn deep_binary_expressions_fit_the_default_worker_stack() {
    // Construct and drop the recursive input on a larger caller stack;
    // only lowering runs on the ordinary two-megabyte worker stack.
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            use crate::syntax::{Expr, ExprKind, Span, Stmt, StmtKind};
            const DEPTH: usize = 4096;
            let source = "int deep(int x) { return x; }";
            for right_associated in [false, true] {
                let mut unit = crate::parser::parse_preprocessed(source, false, true);
                let identifier = || Expr {
                    kind: ExprKind::Identifier("x".into()),
                    span: Span::default(),
                };
                let mut expression = identifier();
                for index in 0..DEPTH {
                    let (left, right) = if right_associated {
                        (identifier(), expression)
                    } else {
                        (expression, identifier())
                    };
                    expression = Expr {
                        kind: ExprKind::Binary {
                            op: ["+", ",", "&&", "||"][index % 4].into(),
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        span: Span::default(),
                    };
                }
                let StmtKind::Block(body) = &mut unit.functions[0].body.kind else {
                    panic!("missing function body");
                };
                body[0] = Stmt {
                    kind: StmtKind::Return(Some(expression)),
                    span: Span::default(),
                };
                let function = &unit.functions[0];
                let known_functions = [function.name.clone()].into_iter().collect();
                let methods: HashMap<_, _> = [(function.full_name.clone(), vec![function])]
                    .into_iter()
                    .collect();
                let bindings = super::CallBindings {
                    methods: &methods,
                    known_types: &unit.known_type_names,
                };
                let (graph, diagnostics) = std::thread::scope(|scope| {
                    std::thread::Builder::new()
                        .stack_size(2 * 1024 * 1024)
                        .spawn_scoped(scope, || {
                            super::build(
                                function,
                                source,
                                "input.c",
                                &known_functions,
                                false,
                                &bindings,
                                &Options::default(),
                            )
                        })
                        .unwrap()
                        .join()
                        .unwrap()
                });
                assert!(diagnostics.is_empty());
                assert_eq!(
                    graph
                        .cpg
                        .nodes
                        .iter()
                        .filter(|node| node.kind == "CALL")
                        .count(),
                    DEPTH
                );
                assert_eq!(
                    graph
                        .cpg
                        .edges
                        .iter()
                        .filter(|edge| edge.kind == "ARGUMENT")
                        .count(),
                    2 * DEPTH + 1
                );
                assert_eq!(
                    graph
                        .cfg
                        .nodes
                        .iter()
                        .map(|node| node.statements.len())
                        .sum::<usize>(),
                    DEPTH + 3
                );
                assert_eq!(
                    graph
                        .cfg
                        .nodes
                        .iter()
                        .filter(|node| node.is_entrypoint)
                        .count(),
                    1
                );
                assert_eq!(
                    graph
                        .cfg
                        .nodes
                        .iter()
                        .filter(|node| node.is_exitpoint)
                        .count(),
                    1
                );
                // A left-associated chain's identifier-only right
                // operands project away, leaving a single CFG block.
                if !right_associated {
                    assert_eq!(graph.cfg.nodes.len(), 1);
                    assert!(graph.cfg.edges.is_empty());
                }
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

fn oracle_node_keys(graph: &crate::graph::FunctionGraph) -> HashMap<u32, String> {
    graph
        .cpg
        .nodes
        .iter()
        .map(|node| {
            let symbol = matches!(
                node.kind.as_str(),
                "CALL"
                    | "IDENTIFIER"
                    | "LOCAL"
                    | "METHOD"
                    | "METHOD_PARAMETER_IN"
                    | "METHOD_PARAMETER_OUT"
            )
            .then(|| node.name.as_deref().unwrap_or(""));
            // CONTROL_STRUCTURE formatting is independent of its expression
            // children and does not participate in CFG or dependence edges.
            let code = if node.kind == "CONTROL_STRUCTURE" {
                ""
            } else {
                &node.code
            };
            (
                node.id,
                serde_json::to_string(&(&node.kind, code, symbol, node.line, node.column)).unwrap(),
            )
        })
        .collect()
}

fn assert_frontend_dependence_oracle(source: &str, filename: &str, oracle: &str) {
    use crate::graph::FunctionGraph;
    use std::collections::{BTreeMap, BTreeSet};
    let reference: Value = serde_json::from_str(oracle).unwrap();
    assert_eq!(
        reference["generator"],
        "Original Joern 4.0.150 / dataflowOss and ReachingDefProblem"
    );
    let expected: Vec<FunctionGraph> =
        serde_json::from_value(reference["methods"].clone()).unwrap();
    let actual = analyze(
        source,
        filename,
        &Options {
            strict: true,
            data_flow: true,
            reaching_definitions: true,
            ..Options::default()
        },
    )
    .unwrap();
    assert_eq!(actual.functions.len(), expected.len(), "method coverage");
    for original in expected {
        let native = actual
            .functions
            .iter()
            .find(|function| function.fullname == original.fullname)
            .unwrap_or_else(|| panic!("missing {}", original.fullname));
        assert_eq!(
            native.signature, original.signature,
            "{} signature",
            original.fullname
        );
        assert_eq!(
            native.return_type, original.return_type,
            "{} return type",
            original.fullname
        );
        let old_keys = oracle_node_keys(&original);
        let new_keys = oracle_node_keys(native);
        for keys in [&old_keys, &new_keys] {
            assert_eq!(
                keys.len(),
                keys.values().collect::<BTreeSet<_>>().len(),
                "ambiguous source identities"
            );
        }
        assert_eq!(
            new_keys.values().collect::<BTreeSet<_>>(),
            old_keys.values().collect::<BTreeSet<_>>(),
            "{} nodes",
            original.fullname
        );
        let edge_keys = |function: &FunctionGraph, keys: &HashMap<u32, String>, kind| {
            function
                .cpg
                .edges
                .iter()
                .filter(|edge| edge.kind == kind)
                .map(|edge| {
                    let label = if kind == "ARGUMENT"
                        && function.cpg.nodes[edge.source as usize].kind == "RETURN"
                    {
                        ""
                    } else {
                        edge.label.as_deref().unwrap_or("")
                    };
                    (
                        keys[&edge.source].clone(),
                        keys[&edge.target].clone(),
                        label.to_string(),
                    )
                })
                .collect::<BTreeSet<_>>()
        };
        for kind in [
            "AST",
            "ARGUMENT",
            "RECEIVER",
            "CFG",
            "REF",
            "PARAMETER_LINK",
            "REACHING_DEF",
        ] {
            assert_eq!(
                edge_keys(native, &new_keys, kind),
                edge_keys(&original, &old_keys, kind),
                "{} {kind}",
                original.fullname
            );
        }
        let sets = |function: &FunctionGraph, keys: &HashMap<u32, String>| {
            function
                .reaching_definitions
                .as_ref()
                .unwrap()
                .iter()
                .map(|definition| {
                    (
                        keys[&definition.node].clone(),
                        (
                            definition
                                .incoming
                                .iter()
                                .map(|node| keys[node].clone())
                                .collect::<BTreeSet<_>>(),
                            definition
                                .outgoing
                                .iter()
                                .map(|node| keys[node].clone())
                                .collect::<BTreeSet<_>>(),
                        ),
                    )
                })
                .collect::<BTreeMap<_, _>>()
        };
        assert_eq!(
            sets(native, &new_keys),
            sets(&original, &old_keys),
            "{} reaching-definition sets",
            original.fullname
        );
        for old in original.cpg.nodes.iter().filter(|node| {
            node.kind == "METHOD_REF"
                || node.name.as_deref() == Some("<operator>()")
                || original.name.starts_with("<lambda>") && node.kind == "IDENTIFIER"
        }) {
            let new = native
                .cpg
                .nodes
                .iter()
                .find(|node| new_keys[&node.id] == old_keys[&old.id])
                .unwrap();
            assert_eq!(
                new.method_full_name, old.method_full_name,
                "{} method identity",
                original.fullname
            );
            assert_eq!(new.type_name, old.type_name, "{} type", original.fullname);
        }
    }
}

#[test]
fn cpp_lambda_cfg_receivers_bindings_and_dependence_match_original_joern() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-lambdas/lambdas.cpp"),
        "lambdas.cpp",
        include_str!("../../tests/fixtures/ddg-lambdas/joern-4.0.150.json"),
    );
}

#[test]
fn cpp_implicit_fields_and_this_capture_match_original_joern() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-lambdas/fields.cpp"),
        "fields.cpp",
        include_str!("../../tests/fixtures/ddg-lambdas/joern-4.0.150-fields.json"),
    );
}

#[test]
fn cpp_member_pointer_receivers_match_original_joern() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-lambdas/method-pointers.cpp"),
        "method-pointers.cpp",
        include_str!("../../tests/fixtures/ddg-lambdas/joern-4.0.150-method-pointers.json"),
    );
}

#[test]
fn cpp_namespaced_member_owner_scopes_match_original_joern() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-lambdas/namespaced-fields.cpp"),
        "namespaced-fields.cpp",
        include_str!("../../tests/fixtures/ddg-lambdas/joern-4.0.150-namespaced-fields.json"),
    );
}

#[test]
fn cpp_grouped_calls_preserve_typed_pointer_and_unresolved_receivers() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/parser-recovery/ambiguous_casts.cpp"),
        "ambiguous_casts.cpp",
        include_str!("../../tests/fixtures/ddg-lambdas/joern-4.0.150-grouped-calls.json"),
    );
}

#[test]
fn cpp_bare_constructors_and_qualified_destructors_match_original_joern() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-classes/constructors.cpp"),
        "constructors.cpp",
        include_str!("../../tests/fixtures/ddg-classes/joern-4.0.150-constructors-flow.json"),
    );
}

#[test]
fn cpp_namespace_using_resolves_definition_owners() {
    assert_frontend_dependence_oracle(
        include_str!("../../tests/fixtures/ddg-classes/using-namespace.cpp"),
        "using-namespace.cpp",
        include_str!("../../tests/fixtures/ddg-classes/joern-4.0.150-namespace-flow.json"),
    );
}

fn assert_oracle(source: &str, oracle: &str) {
    let reference: Value = serde_json::from_str(oracle).unwrap();
    let filename = if reference["source"]
        .as_str()
        .is_some_and(|name| name.ends_with(".cpp"))
    {
        "input.cpp"
    } else {
        "input.c"
    };
    assert_oracle_with_filename(source, oracle, filename);
}

fn assert_oracle_with_filename(source: &str, oracle: &str, filename: &str) {
    let reference: Value = serde_json::from_str(oracle).unwrap();
    let expected = reference["functions"].as_object().unwrap();
    let parsed = analyze(
        source,
        filename,
        &Options {
            strict: true,
            joern_loop_conditions: true,
            ..Options::default()
        },
    )
    .unwrap();
    let visible: Vec<_> = parsed
        .functions
        .iter()
        .filter(|function| !function.cfg.nodes.is_empty())
        .collect();
    assert_eq!(visible.len(), expected.len(), "function coverage");
    for function in visible {
        let cfg = &function.cfg;
        let old = &expected
            .get(&function.name)
            .unwrap_or_else(|| panic!("unexpected {}", function.name))["cfg"];
        let nodes = old["nodes"].as_array().unwrap();
        assert_eq!(cfg.nodes.len(), nodes.len(), "{} nodes", function.name);
        assert_eq!(
            cfg.edges.len(),
            old["edges"].as_array().unwrap().len(),
            "{} edges",
            function.name
        );
        let count = nodes.len();
        let positions: HashMap<_, _> = cfg
            .nodes
            .iter()
            .enumerate()
            .map(|(i, b)| (b.id, i))
            .collect();
        let mut actual = vec![vec![false; count]; count];
        let mut expected = actual.clone();
        for [src, dst] in &cfg.edges {
            actual[positions[src]][positions[dst]] = true;
        }
        for edge in old["edges"].as_array().unwrap() {
            expected[edge[0].as_u64().unwrap() as usize][edge[1].as_u64().unwrap() as usize] = true;
        }
        let actual_roles: Vec<_> = cfg
            .nodes
            .iter()
            .map(|n| (n.is_entrypoint, n.is_exitpoint))
            .collect();
        let expected_roles: Vec<_> = nodes
            .iter()
            .map(|n| {
                (
                    old["entry"].as_array().unwrap().contains(n),
                    old["exit"].as_array().unwrap().contains(n),
                )
            })
            .collect();
        assert!(
            isomorphic(
                &actual,
                &expected,
                &actual_roles,
                &expected_roles,
                &mut Vec::new(),
                &mut vec![false; count]
            ),
            "{} topology and roles",
            function.name
        );
        let degenerate = count == 0
            || count == 1
                && cfg.nodes[0].statements.iter().all(|&id| {
                    function.cpg.nodes[id as usize].cfg_nop.unwrap_or(matches!(
                        function.cpg.nodes[id as usize].kind.as_str(),
                        "METHOD" | "METHOD_RETURN" | "METHOD_REF"
                    ))
                });
        assert_eq!(
            degenerate,
            old["degenerate"].as_bool().unwrap(),
            "{} degeneracy",
            function.name
        );
    }
}

fn isomorphic(
    actual: &[Vec<bool>],
    expected: &[Vec<bool>],
    roles: &[(bool, bool)],
    expected_roles: &[(bool, bool)],
    map: &mut Vec<usize>,
    used: &mut [bool],
) -> bool {
    let source = map.len();
    if source == actual.len() {
        return true;
    }
    for target in 0..expected.len() {
        if used[target]
            || roles[source] != expected_roles[target]
            || actual[source][source] != expected[target][target]
            || actual[source].iter().filter(|&&v| v).count()
                != expected[target].iter().filter(|&&v| v).count()
            || (0..actual.len()).filter(|&i| actual[i][source]).count()
                != (0..expected.len()).filter(|&i| expected[i][target]).count()
            || map.iter().enumerate().any(|(a, &b)| {
                actual[source][a] != expected[target][b] || actual[a][source] != expected[b][target]
            })
        {
            continue;
        }
        map.push(target);
        used[target] = true;
        if isomorphic(actual, expected, roles, expected_roles, map, used) {
            return true;
        }
        used[target] = false;
        map.pop();
    }
    false
}

#[test]
fn missing_goto_targets_follow_joerns_unwired_recovery() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/missing_goto_target.c"),
        include_str!("../../tests/fixtures/decbench-regressions/missing_goto_target.pyjoern.json"),
    );
}

#[test]
fn terminal_labels_follow_cdt_recovery() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/terminal_label.c"),
        include_str!("../../tests/fixtures/decbench-regressions/terminal_label.pyjoern.json"),
    );
}

#[test]
fn inline_assembly_has_a_visible_unknown_node_without_parse_failure() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/gnu_inline_asm.c"),
        include_str!("../../tests/fixtures/decbench-regressions/gnu_inline_asm.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/microsoft_asm_block.c"),
        include_str!("../../tests/fixtures/decbench-regressions/microsoft_asm_block.pyjoern.json"),
    );
}

#[test]
fn nested_designators_preserve_joerns_initializer_flow() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/nested_designator.c"),
        include_str!("../../tests/fixtures/decbench-regressions/nested_designator.pyjoern.json"),
    );
}

#[test]
fn control_and_expression_graphs_match_original_joern() {
    assert_oracle(
        include_str!("../../tests/fixtures/control.c"),
        include_str!("../../tests/fixtures/control.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/expressions.c"),
        include_str!("../../tests/fixtures/expressions.pyjoern.json"),
    );
}

#[test]
fn omitted_for_slots_follow_joerns_ast_child_orders() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/for_slots.c"),
        include_str!("../../tests/fixtures/decbench-regressions/for_slots.pyjoern.json"),
    );
}

#[test]
fn constant_true_loop_heads_lower_like_omitted_for_conditions() {
    fn shapes(source: &str, filename: &str, joern_loop_conditions: bool) -> Vec<Vec<String>> {
        let options = Options {
            strict: true,
            joern_loop_conditions,
            ..Options::default()
        };
        let analysis = analyze(source, filename, &options).unwrap();
        assert!(analysis.diagnostics.is_empty(), "{source}");
        let function = &analysis.functions[0];
        let code = |id: u32| {
            let node = &function.cpg.nodes[id as usize];
            if node.kind == "METHOD" {
                "METHOD".to_string()
            } else {
                node.code.clone()
            }
        };
        let mut raw: Vec<_> = function
            .cpg
            .edges
            .iter()
            .filter(|edge| edge.kind == "CFG")
            .map(|edge| format!("{} -> {}", code(edge.source), code(edge.target)))
            .collect();
        let blocks: HashMap<_, _> = function
            .cfg
            .nodes
            .iter()
            .map(|block| {
                let statements: Vec<_> = block.statements.iter().map(|&id| code(id)).collect();
                let roles = (block.is_entrypoint, block.is_exitpoint);
                (block.id, format!("{statements:?}{roles:?}"))
            })
            .collect();
        let mut projected: Vec<_> = function
            .cfg
            .edges
            .iter()
            .map(|[source, target]| format!("{} -> {}", blocks[source], blocks[target]))
            .collect();
        raw.sort();
        projected.sort();
        vec![raw, projected]
    }
    let bodies = [
        "{ if (n) break; n++; }",
        "{ if (n) continue; n--; }",
        "n++;",
        "again: n--;",
        ";",
    ];
    for (filename, heads) in [
        (
            "input.c",
            &["while (1)", "while (true)", "while ( 2 )", "for (;0x1u;)"][..],
        ),
        (
            "input.cpp",
            &["while (1)", "while (true)", "for (;true;)"][..],
        ),
    ] {
        for body in bodies {
            let function = |head: &str| format!("int f(int n) {{ {head} {body} return n; }}");
            let expected = shapes(&function("for (;;)"), filename, false);
            for head in heads {
                let actual = shapes(&function(head), filename, false);
                assert_eq!(actual, expected, "{filename}: {head} {body}");
            }
            assert_ne!(shapes(&function("while (0)"), filename, false), expected);
        }
    }
    // Joern 4.0.150 keeps the condition literal in the CFG with an exit
    // edge; the default keeps only its AST and CONDITION edge.
    let source = "int f(int n) { while (1) { if (n) break; n++; } return n; }";
    let omitted = "int f(int n) { for (;;) { if (n) break; n++; } return n; }";
    assert_ne!(
        shapes(source, "input.c", true),
        shapes(omitted, "input.c", true)
    );
    let analysis = analyze(source, "input.c", &Options::default()).unwrap();
    let cpg = &analysis.functions[0].cpg;
    let literal = cpg
        .nodes
        .iter()
        .find(|node| node.kind == "LITERAL" && node.code == "1")
        .unwrap();
    assert!(cpg.edges.iter().any(|edge| {
        edge.kind == "CONDITION"
            && edge.target == literal.id
            && cpg.nodes[edge.source as usize].code == "while (1)"
    }));
    assert!(
        !cpg.edges
            .iter()
            .any(|edge| edge.kind == "CFG"
                && (edge.source == literal.id || edge.target == literal.id))
    );
}

#[test]
fn scalar_labels_follow_joerns_control_child_selection() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/scalar_labels_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/scalar_labels_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/scalar_labels_cpp.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/scalar_labels_cpp.pyjoern.json"),
    );
}

#[test]
fn scalar_for_labels_follow_joerns_sibling_slot_selection() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/scalar_for_labels_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/scalar_for_labels_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/scalar_for_labels_cpp.cpp"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/scalar_for_labels_cpp.pyjoern.json"
        ),
    );
}

#[test]
fn typedef_array_bounds_preserve_original_allocations() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/typedef_vla_bounds_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/typedef_vla_bounds_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/typedef_vla_bounds_cpp.cpp"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/typedef_vla_bounds_cpp.pyjoern.json"
        ),
    );
}

#[test]
fn condition_lists_and_type_specifiers_match_original_projection() {
    assert_oracle_with_filename(
        include_str!("../../tests/fixtures/decbench-regressions/control_projection_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/control_projection_c.pyjoern.json"),
        "input.c",
    );
    assert_oracle_with_filename(
        include_str!("../../tests/fixtures/decbench-regressions/control_projection_cpp.cpp"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/control_projection_cpp.pyjoern.json"
        ),
        "input.cpp",
    );
}

#[test]
fn multiline_method_refs_follow_original_dot_lifting() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/method_ref_lines.c"),
        include_str!("../../tests/fixtures/decbench-regressions/method_ref_lines.pyjoern.json"),
    );
}

#[test]
fn returns_under_nested_try_ancestors_keep_expression_fringe() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/nested_try.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/nested_try.pyjoern.json"),
    );
}

#[test]
fn cpp_qualified_names_follow_joerns_field_access_ast() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/qualified_names.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/qualified_names.pyjoern.json"),
    );
}

#[test]
fn method_references_preserve_unique_and_contextual_bindings() {
    for (source, oracle, filename) in [
        (
            include_str!("../../tests/fixtures/decbench-regressions/method_ref_provenance_c.c"),
            include_str!(
                "../../tests/fixtures/decbench-regressions/method_ref_provenance_c.properties.json"
            ),
            "input.c",
        ),
        (
            include_str!("../../tests/fixtures/decbench-regressions/method_ref_provenance_cpp.cpp"),
            include_str!(
                "../../tests/fixtures/decbench-regressions/method_ref_provenance_cpp.properties.json"
            ),
            "input.cpp",
        ),
    ] {
        let reference: Value = serde_json::from_str(oracle).unwrap();
        let expected = reference["data"][0]["method_refs"].as_array().unwrap();
        let analysis = analyze(
            source,
            filename,
            &Options {
                strict: true,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(
            analysis
                .functions
                .iter()
                .flat_map(|f| &f.cpg.nodes)
                .filter(|n| n.kind == "METHOD_REF")
                .count(),
            expected.len()
        );
        for old in expected {
            let function = analysis
                .functions
                .iter()
                .find(|f| Some(f.fullname.as_str()) == old["owner"].as_str())
                .unwrap();
            let node = function
                .cpg
                .nodes
                .iter()
                .find(|n| n.kind == "METHOD_REF" && Some(n.code.as_str()) == old["code"].as_str())
                .unwrap();
            assert_eq!(
                node.method_full_name.as_deref(),
                old["method_full_name"].as_str()
            );
            assert_eq!(node.type_name.as_deref(), old["type_name"].as_str());
            let target = old["refs"][0]["fullname"].as_str().unwrap();
            if target == function.fullname {
                assert!(
                    function.cpg.edges.iter().any(|edge| edge.kind == "REF"
                        && edge.source == node.id
                        && edge.target == 0)
                );
                assert!(node.external_ref.is_none());
            } else {
                let external = node.external_ref.as_ref().unwrap();
                assert_eq!(external.method_full_name, target);
                assert_eq!(external.node, 0);
            }
        }
    }
}

#[test]
fn grouped_declarators_keep_cdt_scalar_control_children() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/grouped_declarator_control_c.c"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/grouped_declarator_control_c.pyjoern.json"
        ),
    );
    assert_oracle(
        include_str!(
            "../../tests/fixtures/decbench-regressions/grouped_declarator_control_cpp.cpp"
        ),
        include_str!(
            "../../tests/fixtures/decbench-regressions/grouped_declarator_control_cpp.pyjoern.json"
        ),
    );
}

#[test]
fn array_bounds_are_allocated_before_initializers() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/local_arrays_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/local_arrays_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/local_arrays_cpp.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/local_arrays_cpp.pyjoern.json"),
    );
}

#[test]
fn empty_try_body_preserves_original_entry_roles() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/empty_try.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/empty_try.pyjoern.json"),
    );
}

#[test]
fn local_function_pointer_declarations_follow_cdt_recovery() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/local_function_pointer_c.c"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/local_function_pointer_c.pyjoern.json"
        ),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/local_function_pointer_cpp.cpp"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/local_function_pointer_cpp.pyjoern.json"
        ),
    );
}

#[test]
fn builtin_macros_preserve_inlined_expansion_paths() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/gnu_builtins_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/gnu_builtins_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/gnu_builtins_cpp.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/gnu_builtins_cpp.pyjoern.json"),
    );
}

#[test]
fn cdt_object_and_function_macros_keep_original_control_flow() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/cdt_builtin_controls_c.c"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/cdt_builtin_controls_c.pyjoern.json"
        ),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/cdt_builtin_controls_cpp.cpp"),
        include_str!(
            "../../tests/fixtures/decbench-regressions/cdt_builtin_controls_cpp.pyjoern.json"
        ),
    );
}

#[test]
fn comma_statement_expressions_preserve_visible_joins() {
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/gnu_assertions_c.c"),
        include_str!("../../tests/fixtures/decbench-regressions/gnu_assertions_c.pyjoern.json"),
    );
    assert_oracle(
        include_str!("../../tests/fixtures/decbench-regressions/gnu_assertions_cpp.cpp"),
        include_str!("../../tests/fixtures/decbench-regressions/gnu_assertions_cpp.pyjoern.json"),
    );
}
