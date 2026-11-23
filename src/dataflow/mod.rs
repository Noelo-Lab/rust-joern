//! Intraprocedural port of Joern's reaching-definition pass. Definitions are
//! expression nodes (including call arguments), rather than assignment sites.
//! Operator and common C summaries follow Joern 4.0.150 DefaultSemantics,
//! the version used to produce DecBench's original PyJoern graphs.
use crate::graph::{DefinitionSet, Edge, FunctionGraph, Node, PropertyGraph};
use std::collections::{HashMap, HashSet, VecDeque};

mod graph;
mod summaries;
use graph::*;
pub(crate) use summaries::*;

pub const LIMITATIONS: &str = "Data flow follows Joern's intraprocedural overlay; callee resolution depends on recovered C/C++ types.";

// PyJoern runs the original overlay with its default ReachingDefPass limit.
const MAX_DEFINITIONS: usize = 4000;

#[derive(Clone, PartialEq, Eq)]
struct Bits(Vec<u64>);

fn name(node: &Node) -> &str {
    node.name.as_deref().unwrap_or("")
}
fn input(node: &Node) -> bool {
    node.kind == "METHOD_PARAMETER_IN"
}
fn output(node: &Node) -> bool {
    node.kind == "METHOD_PARAMETER_OUT"
}
fn call(node: &Node) -> bool {
    node.kind == "CALL"
}
pub(crate) fn expression(node: &Node) -> bool {
    matches!(
        node.kind.as_str(),
        "CALL"
            | "RETURN"
            | "CONTROL_STRUCTURE"
            | "IDENTIFIER"
            | "LITERAL"
            | "BLOCK"
            | "UNKNOWN"
            | "FIELD_IDENTIFIER"
            | "METHOD_REF"
            | "TYPE_REF"
    )
}
fn ddg_node(node: &Node) -> bool {
    !matches!(
        node.kind.as_str(),
        "METHOD"
            | "METHOD_RETURN"
            | "CONTROL_STRUCTURE"
            | "FIELD_IDENTIFIER"
            | "JUMP_TARGET"
            | "LOCAL"
    )
}
pub(crate) fn member(op: &str) -> bool {
    matches!(
        op,
        "<operator>.memberAccess"
            | "<operator>.indirectComputedMemberAccess"
            | "<operator>.indirectMemberAccess"
            | "<operator>.computedMemberAccess"
            | "<operator>.indirection"
            | "<operator>.addressOf"
            | "<operator>.fieldAccess"
            | "<operator>.indirectFieldAccess"
            | "<operator>.indexAccess"
            | "<operator>.indirectIndexAccess"
            | "<operator>.pointerShift"
            | "<operator>.getElementPtr"
    )
}
fn field(op: &str) -> bool {
    member(op) && op != "<operator>.addressOf" && op != "<operator>.pointerShift"
}
fn container(op: &str) -> bool {
    matches!(
        op,
        "<operator>.fieldAccess"
            | "<operator>.indexAccess"
            | "<operator>.indirectIndexAccess"
            | "<operator>.indirectFieldAccess"
    )
}

/// Joern's base MethodDecoratorPass creates output parameters even when the
/// reaching-definition overlay is disabled.
pub fn decorate_parameters(function: &mut FunctionGraph) {
    let Some(entry) = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "METHOD")
        .map(|n| n.id)
    else {
        return;
    };
    let params: Vec<_> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| input(n))
        .cloned()
        .collect();
    parameter_outputs(function, entry, &params);
}

fn parameter_outputs(function: &mut FunctionGraph, entry: u32, params: &[Node]) -> Vec<u32> {
    let mut next_id = function.cpg.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;
    params
        .iter()
        .map(|param| {
            function
                .cpg
                .edges
                .iter()
                .find(|e| e.kind == "PARAMETER_LINK" && e.source == param.id)
                .map(|e| e.target)
                .unwrap_or_else(|| {
                    let mut node = param.clone();
                    node.id = next_id;
                    node.kind = "METHOD_PARAMETER_OUT".into();
                    next_id += 1;
                    let id = node.id;
                    function.cpg.nodes.push(node);
                    for (source, target, kind) in
                        [(entry, id, "AST"), (param.id, id, "PARAMETER_LINK")]
                    {
                        function.cpg.edges.push(Edge {
                            source,
                            target,
                            kind: kind.into(),
                            label: None,
                        });
                    }
                    id
                })
        })
        .collect()
}

/// Calculate RD and REACHING_DEF edges. Set exposure is optional; DDG always
/// requires the same solver. Reapplying the pass replaces its previous edges.
#[cfg(test)]
pub fn apply(function: &mut FunctionGraph, expose_sets: bool) {
    apply_with_methods(function, expose_sets, &HashSet::new());
}

/// Callee identities must name resolved, non-stub methods in this analysis.
/// This reproduces Joern's suppression of implicit argument-to-argument flow
/// inside an internal call whose body supplies the data-flow semantics.
#[cfg(test)]
pub fn apply_with_methods(
    function: &mut FunctionGraph,
    expose_sets: bool,
    internal_methods: &HashSet<String>,
) {
    let cpp = matches!(
        std::path::Path::new(&function.filename)
            .extension()
            .and_then(|s| s.to_str()),
        Some("cpp" | "cc" | "cxx" | "c++" | "C" | "hpp" | "hh" | "hxx" | "ii")
    );
    apply_with_context(function, expose_sets, internal_methods, cpp);
}

/// The frontend's resolved language takes precedence over a filename suffix.
pub fn apply_with_context(
    function: &mut FunctionGraph,
    expose_sets: bool,
    internal_methods: &HashSet<String>,
    cpp: bool,
) {
    function.cpg.edges.retain(|e| e.kind != "REACHING_DEF");
    function.ddg_view = None;
    function.ddg_projection = None;
    let Some(entry_id) = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "METHOD")
        .map(|n| n.id)
    else {
        return;
    };
    let Some(exit_id) = function
        .cpg
        .nodes
        .iter()
        .find(|n| n.kind == "METHOD_RETURN")
        .map(|n| n.id)
    else {
        return;
    };
    let params: Vec<Node> = function
        .cpg
        .nodes
        .iter()
        .filter(|n| input(n))
        .cloned()
        .collect();
    let outputs = parameter_outputs(function, entry_id, &params);
    let nodes = &function.cpg.nodes;
    let n = nodes.len();
    let index: HashMap<_, _> = nodes
        .iter()
        .enumerate()
        .map(|(i, node)| (node.id, i))
        .collect();
    let entry = index[&entry_id];
    let exit = index[&exit_id];
    let params: Vec<_> = params.iter().map(|p| index[&p.id]).collect();
    let outputs: Vec<_> = outputs.iter().map(|p| index[p]).collect();
    let mut graph = Graph {
        nodes,
        internal_methods,
        cpp,
        ast: vec![Vec::new(); n],
        args: vec![Vec::new(); n],
        parent_call: vec![None; n],
    };
    let mut cfg_next = vec![Vec::new(); n];
    let mut cfg_prev = vec![Vec::new(); n];
    for edge in &function.cpg.edges {
        let (Some(&a), Some(&b)) = (index.get(&edge.source), index.get(&edge.target)) else {
            continue;
        };
        match edge.kind.as_str() {
            "AST" => graph.ast[a].push(b),
            "ARGUMENT" => {
                graph.add_argument(a, b, edge);
            }
            "CFG" => {
                cfg_next[a].push(b);
                cfg_prev[b].push(a);
            }
            _ => {}
        }
    }
    for args in &mut graph.args {
        args.sort_by_key(|(position, _)| *position);
    }
    let mut order = vec![entry];
    order.extend(params.iter().copied());
    order.extend(
        reverse_postorder(entry, &cfg_next)
            .into_iter()
            .filter(|&i| i != entry && i != exit),
    );
    order.extend(outputs.iter().copied());
    order.push(exit);
    order.dedup();
    // Reproduce ReachingDefFlowGraph's separate predecessor/successor maps,
    // including cfgFirst and the first predecessor of METHOD_RETURN.
    let mut next = cfg_next.clone();
    let mut prev = cfg_prev.clone();
    if let Some(&first) = params.first() {
        next[entry] = vec![first];
    }
    for (i, &param) in params.iter().enumerate() {
        prev[param] = vec![if i == 0 { entry } else { params[i - 1] }];
        next[param] = params
            .get(i + 1)
            .map(|&p| vec![p])
            .unwrap_or_else(|| cfg_next[entry].clone());
    }
    for (i, &param) in outputs.iter().enumerate() {
        prev[param] = if i == 0 {
            cfg_prev[exit].first().copied().into_iter().collect()
        } else {
            vec![outputs[i - 1]]
        };
        next[param] = vec![outputs.get(i + 1).copied().unwrap_or(exit)];
    }
    prev[exit] = outputs
        .last()
        .copied()
        .or_else(|| cfg_prev[exit].first().copied())
        .into_iter()
        .collect();
    // cfgFirst takes precedence over the METHOD_RETURN case in Joern. This
    // matters for prototypes/empty bodies, where cfgFirst is the exit itself.
    if let Some(&first) = cfg_next[entry].first() {
        prev[first] = vec![params.last().copied().unwrap_or(entry)];
    }
    for &i in &order {
        if nodes[i].kind == "RETURN"
            || next[i] == [exit]
                && !outputs.is_empty()
                && !output(&nodes[i])
                && !input(&nodes[i])
                && nodes[i].kind != "METHOD"
        {
            next[i] = vec![outputs.first().copied().unwrap_or(exit)];
        }
    }

    let mut gens: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, node) in nodes.iter().enumerate() {
        if input(node) {
            gens[i].push(i);
        }
        if call(node) && !field(name(node)) {
            gens[i].push(i);
            gens[i].extend(
                graph.args[i]
                    .iter()
                    .map(|&(_, arg)| arg)
                    .filter(|&arg| call(&nodes[arg]) || nodes[arg].kind == "IDENTIFIER"),
            );
        }
    }
    let declared: HashSet<_> = nodes
        .iter()
        .filter(|n| input(n) || n.kind == "LOCAL")
        .map(name)
        .collect();
    let return_names: HashSet<_> = nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.kind == "RETURN")
        .flat_map(|(i, _)| graph.descendants(i))
        .filter(|&i| nodes[i].kind == "IDENTIFIER")
        .map(|i| name(&nodes[i]))
        .collect();
    let mut candidates: HashMap<&str, Vec<(usize, usize)>> = HashMap::new();
    for (i, node) in nodes.iter().enumerate().filter(|(_, n)| call(n)) {
        let _ = node;
        for &(_, arg) in &graph.args[i] {
            if nodes[arg].kind == "IDENTIFIER"
                && !declared.contains(name(&nodes[arg]))
                && !return_names.contains(name(&nodes[arg]))
            {
                candidates
                    .entry(name(&nodes[arg]))
                    .or_default()
                    .push((i, arg));
            }
        }
    }
    // loneIdentifiers collects `(call, identifier)` pairs from a map grouped
    // by name into a map keyed by call, so a call keeps only the last of its
    // lone identifiers in that map's iteration order. The others stay
    // ordinary generated definitions.
    let mut ordered = Vec::new();
    scala_hash_map_order(candidates.keys().copied().collect(), 0, &mut ordered);
    let mut lone_of_call: HashMap<usize, usize> = HashMap::new();
    for name in ordered {
        if let [(at, arg)] = candidates[name][..] {
            lone_of_call.insert(at, arg);
        }
    }
    let mut lone = Vec::new();
    for (at, arg) in lone_of_call {
        gens[at].retain(|&i| i != arg);
        lone.push(arg);
    }
    lone.sort_unstable();
    // Joern counts definitions at each generating CFG node, after removing
    // lone identifiers, rather than counting unique definition nodes.
    let skip_overlay = gens.iter().map(Vec::len).sum::<usize>() > MAX_DEFINITIONS;
    if skip_overlay && !expose_sets {
        function.ddg = Some(PropertyGraph::default());
        function.reaching_definitions = None;
        return;
    }
    // Definition numbers are positions in ReachingDefFlowGraph's
    // allNodesEvenUnreachable: the solver order, then the remaining CFG nodes
    // in `method.cfgNode` order. A BitSet of definitions iterates by number,
    // which fixes the stored order of the REACHING_DEF edges created from it.
    let mut number = vec![usize::MAX; n];
    let unreachable = contained(entry, &graph.ast, nodes);
    for (position, &i) in order.iter().chain(&unreachable).enumerate() {
        if number[i] == usize::MAX {
            number[i] = position;
        }
    }
    let mut definitions: Vec<_> = gens
        .iter()
        .flatten()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    definitions.sort_unstable_by_key(|&i| (number[i], i));
    let bits: HashMap<_, _> = definitions
        .iter()
        .enumerate()
        .map(|(bit, &i)| (i, bit))
        .collect();
    let mut gen_sets = vec![Bits::new(definitions.len()); n];
    let mut kill_sets = gen_sets.clone();
    let mut ident_groups: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut call_groups: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut fields: HashMap<&str, Vec<usize>> = HashMap::new();
    for &i in &definitions {
        if input(&nodes[i]) || nodes[i].kind == "IDENTIFIER" {
            ident_groups.entry(name(&nodes[i])).or_default().push(i);
        }
        if call(&nodes[i]) {
            call_groups.entry(&nodes[i].code).or_default().push(i);
        }
        if name(&nodes[i]) == "<operator>.fieldAccess" {
            for child in graph
                .descendants(i)
                .into_iter()
                .filter(|&c| nodes[c].kind == "IDENTIFIER")
            {
                fields.entry(name(&nodes[child])).or_default().push(i);
            }
        }
    }
    for i in 0..n {
        for &definition in &gens[i] {
            gen_sets[i].insert(bits[&definition]);
            if !call(&nodes[i]) || member(name(&nodes[i])) {
                continue;
            }
            let node = &nodes[definition];
            let group = if input(node) || node.kind == "IDENTIFIER" {
                ident_groups.get(name(node))
            } else if call(node) {
                call_groups.get(node.code.as_str())
            } else {
                None
            };
            for &other in group
                .into_iter()
                .flatten()
                .filter(|&&other| other != definition)
            {
                kill_sets[i].insert(bits[&other]);
            }
            if node.kind == "IDENTIFIER" {
                for &other in fields.get(name(node)).into_iter().flatten() {
                    kill_sets[i].insert(bits[&other]);
                }
            }
        }
    }
    let mut incoming = vec![Bits::new(definitions.len()); n];
    let mut outgoing = gen_sets.clone();
    // DataFlowSolver works in rounds: every node of a round sees the sets
    // updated earlier in it, and the successors of its changed nodes form the
    // next round, each at its first position. The flow graph's successor and
    // predecessor maps disagree around output parameters, so this is not an
    // order-independent fixed point: a node on both a round and the next is
    // evaluated twice, and the sets reaching the exit depend on that.
    let mut round = order.clone();
    let mut listed = vec![usize::MAX; n];
    let mut rounds = 0;
    while !round.is_empty() {
        let mut following = Vec::new();
        for &i in &round {
            let mut value = Bits::new(definitions.len());
            for &predecessor in &prev[i] {
                value.union(&outgoing[predecessor]);
            }
            incoming[i] = value.clone();
            for ((word, kill), generated) in
                value.0.iter_mut().zip(&kill_sets[i].0).zip(&gen_sets[i].0)
            {
                *word = *word & !kill | generated;
            }
            if value != outgoing[i] {
                outgoing[i] = value;
                for &successor in &next[i] {
                    if listed[successor] != rounds {
                        listed[successor] = rounds;
                        following.push(successor);
                    }
                }
            }
        }
        round = following;
        rounds += 1;
    }
    function.reaching_definitions = expose_sets.then(|| {
        order
            .iter()
            .map(|&i| DefinitionSet {
                node: nodes[i].id,
                incoming: incoming[i]
                    .iter()
                    .map(|d| nodes[definitions[d]].id)
                    .collect(),
                outgoing: outgoing[i]
                    .iter()
                    .map(|d| nodes[definitions[d]].id)
                    .collect(),
            })
            .collect()
    });
    // Explicit solver-set requests remain available independently of the
    // overlay, matching an explicit ReachingDefProblem/DataFlowSolver query.
    if skip_overlay {
        function.ddg = Some(PropertyGraph::default());
        return;
    }
    let mut ddg = Edges {
        graph: &graph,
        edges: Vec::new(),
    };
    // DdgGenerator.addReachingDefEdges: entry edges first, then one group of
    // edges per call, return and output parameter, then the exit's. Within a
    // group the order is Joern's: it decides each node's stored in-edge
    // order, which the DOT projection's edge cache depends on. Joern orders
    // the rest by node hashes that include the Graph's identity hash, so it
    // varies between runs: the groups themselves (keys of `solution.in`), the
    // uses of a call with more than four arguments, the definitions reaching
    // a return's operand and the lone identifiers. The solver order, argument
    // order and definition numbers stand in for those here.
    for &i in &order {
        if ddg_node(&nodes[i]) && graph.uses(i).is_empty() {
            ddg.add(entry, i, "");
        }
    }
    for &i in &order {
        let uses = graph.uses(i);
        let reaching = |used: usize| -> Vec<usize> {
            incoming[i]
                .iter()
                .map(|d| definitions[d])
                .filter(|&d| graph.is_using(used, d))
                .collect()
        };
        if call(&nodes[i]) {
            // addEdgesToCallSite: used incoming definitions of every
            // argument, then argument to generated definition, then blocks.
            for &used in &uses {
                for definition in reaching(used) {
                    if definition != used {
                        ddg.add(definition, used, graph.label(definition));
                    }
                }
            }
            for &used in &uses {
                for &generated in &gens[i] {
                    if used != generated && ddg_node(&nodes[used]) {
                        ddg.add(used, generated, graph.label(used));
                    }
                }
            }
            for &used in &uses {
                if nodes[used].kind == "BLOCK" {
                    graph.block_edges(used, i, &incoming, &definitions, &mut ddg);
                }
            }
        } else if nodes[i].kind == "RETURN" {
            // addEdgesToReturn: blocks, then per returned expression its edge
            // to the return followed by its used incoming definitions.
            for &used in &uses {
                if nodes[used].kind == "BLOCK" {
                    graph.block_edges(used, i, &incoming, &definitions, &mut ddg);
                }
            }
            for &used in &uses {
                ddg.add(used, i, &nodes[used].code);
                let relevant = reaching(used);
                for &definition in &relevant {
                    if definition != used {
                        ddg.add(definition, used, graph.label(definition));
                    }
                }
                if relevant.is_empty() {
                    ddg.add(entry, i, "");
                }
            }
            ddg.add(i, exit, "<RET>");
        } else if let Some(position) = outputs.iter().position(|&out| out == i) {
            // addEdgesToMethodParameterOut: the input parameter's edge first.
            ddg.add(params[position], i, name(&nodes[params[position]]));
            for definition in reaching(i) {
                ddg.add(definition, i, graph.label(definition));
            }
        }
    }
    for definition in incoming[exit].iter().map(|d| definitions[d]).chain(lone) {
        ddg.add(definition, exit, graph.label(definition));
    }
    let edges = ddg.edges;
    let ids: HashSet<_> = edges.iter().flat_map(|e| [e.source, e.target]).collect();
    function.ddg = Some(PropertyGraph {
        nodes: nodes
            .iter()
            .filter(|n| ids.contains(&n.id))
            .cloned()
            .collect(),
        edges: edges.clone(),
    });
    function.cpg.edges.extend(edges);
}

/// Joern's displayed DDG is a projection of REACHING_DEF: hidden arguments
/// are bypassed, expression arguments are grouped under their surrounding
/// call, and member-access calls are omitted. Keep the raw DDG separately.
/// `all_internal_methods` includes declared prototypes and empty definitions;
/// EdgeValidator instead uses only the non-stub subset.
pub fn project_ddg(
    function: &FunctionGraph,
    internal_nonstub_methods: &HashSet<String>,
    all_internal_methods: &HashSet<String>,
    cpp: bool,
) -> PropertyGraph {
    crate::ddg::project(
        function,
        internal_nonstub_methods,
        all_internal_methods,
        cpp,
    )
}

/// `method.cfgNode`: the CFG-node targets of the method's CONTAINS edges.
/// ContainsEdgePass traverses the AST breadth first without descending into
/// nested methods and type declarations. Locals and parameters are not
/// CONTAINS targets, though output parameters themselves are CfgNodes.
pub(crate) fn contained(entry: usize, ast: &[Vec<usize>], nodes: &[Node]) -> Vec<usize> {
    let mut result = Vec::new();
    let mut queue = VecDeque::from([entry]);
    let mut seen = HashSet::from([entry]);
    while let Some(parent) = queue.pop_front() {
        for &node in &ast[parent] {
            if !seen.insert(node) {
                continue;
            }
            let kind = nodes[node].kind.as_str();
            if expression(&nodes[node]) || matches!(kind, "JUMP_TARGET" | "METHOD") {
                result.push(node);
            }
            if !matches!(kind, "METHOD" | "TYPE_DECL") {
                queue.push_back(node);
            }
        }
    }
    result
}

struct Graph<'a> {
    nodes: &'a [Node],
    internal_methods: &'a HashSet<String>,
    cpp: bool,
    ast: Vec<Vec<usize>>,
    args: Vec<Vec<(i32, usize)>>,
    parent_call: Vec<Option<(usize, i32)>>,
}

struct Edges<'g, 'n> {
    graph: &'g Graph<'n>,
    edges: Vec<Edge>,
}

#[derive(Clone, PartialEq, Eq)]
enum Access {
    Field(String),
    Variable,
    VariableShift,
    Deref,
    Address,
    Shift(i32),
}

pub(crate) struct Semantic {
    pub(crate) flows: &'static [(i32, i32)],
    pub(crate) pass: bool,
}

#[cfg(test)]
mod tests;
