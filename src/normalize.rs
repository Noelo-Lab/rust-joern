//! Joern's DOT CFG projection followed by PyJoern/cfgutils block normalization.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, VecDeque};

use crate::graph::{BasicBlock, Cfg, Node, PropertyGraph};

struct Block {
    value: Option<BasicBlock>,
    incoming: Vec<usize>,
    outgoing: Vec<usize>,
    starts_method: bool,
}

/// Produce the CFG consumed by DecBench, including PyJoern's entry/exit flags.
pub fn normalize(cpg: &PropertyGraph) -> Cfg {
    let indices: HashMap<_, _> = cpg
        .nodes
        .iter()
        .enumerate()
        .map(|(i, n)| (n.id, i))
        .collect();
    let mut control_condition = vec![false; cpg.nodes.len()];
    let mut successors = vec![Vec::new(); cpg.nodes.len()];
    let mut ast_children = vec![Vec::new(); cpg.nodes.len()];
    for edge in &cpg.edges {
        let (Some(&src), Some(&dst)) = (indices.get(&edge.source), indices.get(&edge.target))
        else {
            continue;
        };
        match edge.kind.as_str() {
            "CFG" => successors[src].push(dst),
            "AST" => {
                ast_children[src].push(dst);
                if cpg.nodes[src].kind == "CONTROL_STRUCTURE" {
                    control_condition[dst] = true;
                }
            }
            _ => {}
        }
    }
    let visible: Vec<_> = cpg
        .nodes
        .iter()
        .enumerate()
        .map(|(i, node)| {
            (node.kind == "IDENTIFIER" && control_condition[i])
                || !matches!(
                    node.kind.as_str(),
                    "LITERAL"
                        | "IDENTIFIER"
                        | "BLOCK"
                        | "CONTROL_STRUCTURE"
                        | "JUMP_TARGET"
                        | "METHOD_PARAMETER_IN"
                )
        })
        .collect();

    // Invisible paths stop at the first visible successor. Generation marks
    // avoid allocating visited sets for each source and also deduplicate edges.
    let mut visited = vec![0; cpg.nodes.len()];
    let mut reached = vec![0; cpg.nodes.len()];
    let mut projected = Vec::new();
    let mut pending = Vec::new();
    // ContainsEdgePass walks AST children breadth first. CfgGenerator displays
    // that order, then METHOD and METHOD_RETURN. PyJoern's flag-dropping block
    // copies make this order observable when a METHOD_REF is merged.
    let mut vertices = Vec::new();
    if let Some(method) = cpg.nodes.iter().position(|node| node.kind == "METHOD")
        && !ast_children[method].is_empty()
    {
        let mut queue = VecDeque::from([method]);
        let mut seen = vec![false; cpg.nodes.len()];
        seen[method] = true;
        while let Some(parent) = queue.pop_front() {
            for &child in &ast_children[parent] {
                if seen[child] {
                    continue;
                }
                seen[child] = true;
                if !matches!(
                    cpg.nodes[child].kind.as_str(),
                    "METHOD_RETURN" | "METHOD_PARAMETER_IN" | "LOCAL" | "MODIFIER" | "MEMBER"
                ) {
                    vertices.push(child);
                }
                if !matches!(
                    cpg.nodes[child].kind.as_str(),
                    "METHOD" | "TYPE_DECL" | "FILE"
                ) {
                    queue.push_back(child);
                }
            }
        }
        vertices.push(method);
        vertices.extend(cpg.nodes.iter().enumerate().filter_map(|(i, node)| {
            (node.kind == "METHOD_RETURN" || node.kind == "METHOD_PARAMETER_IN").then_some(i)
        }));
    }
    if vertices.is_empty() {
        vertices.extend(0..cpg.nodes.len());
    }
    for src in vertices {
        if !visible[src] {
            continue;
        }
        let generation = src + 1;
        pending.push(src);
        while let Some(current) = pending.pop() {
            if visited[current] == generation {
                continue;
            }
            visited[current] = generation;
            for &dst in &successors[current] {
                if visible[dst] && reached[dst] != generation {
                    reached[dst] = generation;
                    projected.push((src, dst));
                }
            }
            // CfgGenerator emits visible children before recursively expanding
            // invisible children, in their original successor order.
            pending.extend(
                successors[current]
                    .iter()
                    .rev()
                    .copied()
                    .filter(|&dst| !visible[dst]),
            );
        }
    }

    // PyJoern's lift_graph inserts endpoints when walking projected edges;
    // visible vertices referenced by no edges are therefore absent here.
    let mut blocks: Vec<Block> = Vec::new();
    let mut slots = vec![usize::MAX; cpg.nodes.len()];
    for (source, target) in projected {
        for raw in [source, target] {
            if slots[raw] == usize::MAX {
                slots[raw] = blocks.len();
                let node = &cpg.nodes[raw];
                blocks.push(Block {
                    value: Some(BasicBlock {
                        id: node.id,
                        statements: vec![node.id],
                        is_entrypoint: is_start(node),
                        is_exitpoint: node.kind == "METHOD_RETURN" && node.cfg_nop != Some(false),
                    }),
                    incoming: Vec::new(),
                    outgoing: Vec::new(),
                    starts_method: is_start(node),
                });
            }
        }
        let (src, dst) = (slots[source], slots[target]);
        blocks[src].outgoing.push(dst);
        blocks[dst].incoming.push(src);
    }

    // NetworkX scans sources in insertion order. A heap preserves the same
    // first eligible edge without rescanning the whole graph after each merge.
    let mut candidates: BinaryHeap<_> = (0..blocks.len())
        .filter(|&src| merge_target(&blocks, src).is_some())
        .map(Reverse)
        .collect();
    while let Some(Reverse(src)) = candidates.pop() {
        let Some(dst) = merge_target(&blocks, src) else {
            continue;
        };
        let mut value = blocks[src].value.take().unwrap();
        let target = blocks[dst].value.take().unwrap();
        value.statements.extend(target.statements);
        // GenericBlock.copy drops explicit source flags. Block's getters infer
        // an entry from its first statement and an exit from its last statement.
        value.is_entrypoint = blocks[src].starts_method || target.is_entrypoint;
        value.is_exitpoint = target.is_exitpoint;
        let incoming = std::mem::take(&mut blocks[src].incoming);
        let outgoing = std::mem::take(&mut blocks[dst].outgoing);
        blocks[src].outgoing.clear();
        blocks[dst].incoming.clear();
        let merged = blocks.len();
        for &pred in &incoming {
            if pred != dst {
                *blocks[pred]
                    .outgoing
                    .iter_mut()
                    .find(|next| **next == src)
                    .unwrap() = merged;
            }
        }
        for &succ in &outgoing {
            if succ != src {
                *blocks[succ]
                    .incoming
                    .iter_mut()
                    .find(|prev| **prev == dst)
                    .unwrap() = merged;
            }
        }
        blocks.push(Block {
            value: Some(value),
            incoming: incoming
                .into_iter()
                .map(|n| if n == dst { merged } else { n })
                .collect(),
            outgoing: outgoing
                .into_iter()
                .map(|n| if n == src { merged } else { n })
                .collect(),
            starts_method: blocks[src].starts_method,
        });
        // All external neighbors retain their degrees. Only the new block can
        // become newly eligible; existing candidates are checked when popped.
        if merge_target(&blocks, merged).is_some() {
            candidates.push(Reverse(merged));
        }
    }

    let sinks: Vec<_> = blocks
        .iter()
        .enumerate()
        .filter_map(|(i, block)| {
            block
                .value
                .as_ref()
                .filter(|value| {
                    block.outgoing.is_empty() && value.statements.len() == 1 && value.is_exitpoint
                })
                .map(|_| i)
        })
        .collect();
    // PyJoern removes a sole synthetic return sink after all merges. It neither
    // removes merged function ends nor transfers exit flags to predecessors.
    if let [sink] = sinks.as_slice() {
        blocks[*sink].value = None;
        for pred in std::mem::take(&mut blocks[*sink].incoming) {
            blocks[pred].outgoing.retain(|dst| dst != sink);
        }
    }

    let mut cfg = Cfg::default();
    for block in &blocks {
        if let Some(value) = &block.value {
            cfg.nodes.push(value.clone());
            for &dst in &block.outgoing {
                cfg.edges
                    .push([value.id, blocks[dst].value.as_ref().unwrap().id]);
            }
        }
    }
    cfg
}

fn merge_target(blocks: &[Block], src: usize) -> Option<usize> {
    if blocks[src].value.is_none() || blocks[src].outgoing.len() != 1 {
        return None;
    }
    let dst = blocks[src].outgoing[0];
    (src != dst && blocks[dst].value.is_some() && blocks[dst].incoming.len() == 1).then_some(dst)
}

// PyJoern tests the DOT label with startswith("METHOD") after METHOD_RETURN;
// this also marks function-pointer METHOD_REF nodes as function starts.
fn is_start(node: &Node) -> bool {
    node.cfg_nop != Some(false) && node.kind != "METHOD_RETURN" && node.kind.starts_with("METHOD")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Node, NodeId};

    fn graph(kinds: &[&str], edges: &[(NodeId, NodeId, &str)]) -> PropertyGraph {
        PropertyGraph {
            nodes: kinds
                .iter()
                .enumerate()
                .map(|(i, kind)| Node {
                    id: i as NodeId,
                    kind: (*kind).into(),
                    code: String::new(),
                    name: None,
                    method_full_name: None,
                    cfg_nop: None,
                    type_name: None,
                    line: 1,
                    column: 1,
                    start_byte: 0,
                    end_byte: 0,
                })
                .collect(),
            edges: edges
                .iter()
                .map(|&(source, target, kind)| Edge {
                    source,
                    target,
                    kind: kind.into(),
                    label: None,
                })
                .collect(),
        }
    }

    #[test]
    fn linear_chain_retains_order_and_inferred_flags() {
        let cfg = normalize(&graph(
            &["METHOD", "CALL", "RETURN", "METHOD_RETURN"],
            &[(0, 1, "CFG"), (1, 2, "CFG"), (2, 3, "CFG")],
        ));
        assert_eq!(cfg.nodes.len(), 1);
        assert_eq!(cfg.nodes[0].statements, [0, 1, 2, 3]);
        assert!(cfg.nodes[0].is_entrypoint && cfg.nodes[0].is_exitpoint);
        assert!(cfg.edges.is_empty());
    }

    #[test]
    fn branch_removes_only_the_unmerged_method_return_sink() {
        let cfg = normalize(&graph(
            &[
                "METHOD",
                "IDENTIFIER",
                "CONTROL_STRUCTURE",
                "RETURN",
                "RETURN",
                "METHOD_RETURN",
            ],
            &[
                (2, 1, "AST"),
                (0, 1, "CFG"),
                (1, 3, "CFG"),
                (1, 4, "CFG"),
                (3, 5, "CFG"),
                (4, 5, "CFG"),
            ],
        ));
        assert_eq!(cfg.nodes.len(), 3);
        assert_eq!(cfg.edges.len(), 2);
        assert!(cfg.nodes.iter().all(|n| !n.is_exitpoint));
        assert_eq!(
            cfg.nodes
                .iter()
                .find(|n| n.is_entrypoint)
                .unwrap()
                .statements,
            [0, 1]
        );
    }

    #[test]
    fn multiple_method_return_sinks_are_retained() {
        let cfg = normalize(&graph(
            &["METHOD", "METHOD_RETURN", "METHOD_RETURN", "CALL", "CALL"],
            &[(0, 1, "CFG"), (0, 2, "CFG"), (3, 1, "CFG"), (4, 2, "CFG")],
        ));
        assert_eq!(cfg.nodes.iter().filter(|n| n.is_exitpoint).count(), 2);
    }

    #[test]
    fn literal_conditions_remain_invisible_and_edges_are_deduplicated() {
        let cfg = normalize(&graph(
            &[
                "METHOD",
                "LITERAL",
                "CONTROL_STRUCTURE",
                "RETURN",
                "METHOD_RETURN",
            ],
            &[
                (2, 1, "AST"),
                (0, 1, "CFG"),
                (1, 3, "CFG"),
                (1, 3, "CFG"),
                (3, 4, "CFG"),
            ],
        ));
        assert_eq!(cfg.nodes.len(), 1);
        assert_eq!(cfg.nodes[0].statements, [0, 3, 4]);
    }

    #[test]
    fn two_node_cycles_merge_into_self_loops() {
        let cfg = normalize(&graph(&["CALL", "CALL"], &[(0, 1, "CFG"), (1, 0, "CFG")]));
        assert_eq!(cfg.nodes.len(), 1);
        assert_eq!(cfg.nodes[0].statements, [0, 1]);
        assert_eq!(cfg.edges, [[0, 0]]);
        assert!(!cfg.nodes[0].is_entrypoint && !cfg.nodes[0].is_exitpoint);
    }

    #[test]
    fn invisible_cycles_terminate_and_unreachable_edges_survive() {
        let cfg = normalize(&graph(
            &[
                "METHOD",
                "IDENTIFIER",
                "IDENTIFIER",
                "RETURN",
                "METHOD_RETURN",
                "CALL",
                "CALL",
                "CALL",
            ],
            &[
                (0, 1, "CFG"),
                (1, 2, "CFG"),
                (2, 1, "CFG"),
                (2, 3, "CFG"),
                (3, 4, "CFG"),
                (5, 6, "CFG"),
                (6, 5, "CFG"),
            ],
        ));
        assert_eq!(cfg.nodes.len(), 2);
        assert!(cfg.nodes.iter().any(|n| n.statements == [0, 3, 4]));
        assert!(cfg.nodes.iter().any(|n| n.statements == [5, 6]));
        assert!(cfg.nodes.iter().all(|n| !n.statements.contains(&7)));
        assert_eq!(cfg.edges, [[5, 5]]);
    }

    #[test]
    fn field_identifiers_are_visible() {
        let cfg = normalize(&graph(
            &[
                "METHOD",
                "FIELD_IDENTIFIER",
                "CALL",
                "RETURN",
                "METHOD_RETURN",
            ],
            &[
                (0, 1, "CFG"),
                (1, 2, "CFG"),
                (1, 3, "CFG"),
                (2, 3, "CFG"),
                (3, 4, "CFG"),
            ],
        ));
        assert_eq!(cfg.nodes.len(), 3);
        assert_eq!(
            cfg.nodes
                .iter()
                .find(|n| n.is_entrypoint)
                .unwrap()
                .statements,
            [0, 1]
        );
    }

    #[test]
    fn copying_a_merged_source_drops_its_explicit_flag() {
        let cfg = normalize(&graph(
            &["CALL", "METHOD", "CALL"],
            &[(0, 1, "CFG"), (1, 2, "CFG")],
        ));
        assert_eq!(cfg.nodes[0].statements, [0, 1, 2]);
        assert!(!cfg.nodes[0].is_entrypoint);
    }

    #[test]
    fn method_refs_follow_pyjoerns_method_prefix_flag() {
        let cfg = normalize(&graph(
            &["METHOD_REF", "CALL"],
            &[(0, 1, "CFG"), (1, 1, "CFG")],
        ));
        assert!(
            cfg.nodes
                .iter()
                .any(|n| n.is_entrypoint && n.statements.contains(&0))
        );
    }
}
