//! Intraprocedural port of Joern's reaching-definition pass. Definitions are
//! expression nodes (including call arguments), rather than assignment sites.
//! Operator and common C summaries follow Joern 4.0.150 DefaultSemantics,
//! the version used to produce DecBench's original PyJoern graphs.
use crate::graph::{DefinitionSet, Edge, FunctionGraph, Node, PropertyGraph};
use std::collections::{HashMap, HashSet, VecDeque};

pub const LIMITATIONS: &str = "Data flow is intraprocedural: cross-method closure/global propagation is not yet implemented; callee resolution depends on recovered C/C++ types.";

#[derive(Clone, PartialEq, Eq)]
struct Bits(Vec<u64>);
impl Bits {
    fn new(size: usize) -> Self {
        Self(vec![0; size.div_ceil(64)])
    }
    fn insert(&mut self, bit: usize) {
        self.0[bit / 64] |= 1 << (bit % 64);
    }
    fn union(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a |= b;
        }
    }
    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(i, word)| {
            let mut word = *word;
            std::iter::from_fn(move || {
                if word == 0 {
                    return None;
                }
                let bit = word.trailing_zeros() as usize;
                word &= word - 1;
                Some(i * 64 + bit)
            })
        })
    }
}

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
fn expression(node: &Node) -> bool {
    matches!(
        node.kind.as_str(),
        "CALL"
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
fn member(op: &str) -> bool {
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
                let position = edge
                    .label
                    .as_deref()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(graph.args[a].len() as i32 + 1);
                graph.args[a].push((position, b));
                if call(&nodes[a]) {
                    graph.parent_call[b] = Some((a, position));
                }
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
    let mut lone = Vec::new();
    for values in candidates.values().filter(|values| values.len() == 1) {
        let (at, arg) = values[0];
        gens[at].retain(|&i| i != arg);
        lone.push(arg);
    }
    let definitions: Vec<_> = gens
        .iter()
        .flatten()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    let mut definitions = definitions;
    definitions.sort_unstable();
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
    let mut queued = vec![false; n];
    let mut queue = VecDeque::from(order.clone());
    for &i in &order {
        queued[i] = true;
    }
    while let Some(i) = queue.pop_front() {
        queued[i] = false;
        let mut value = Bits::new(definitions.len());
        for &predecessor in &prev[i] {
            value.union(&outgoing[predecessor]);
        }
        incoming[i] = value.clone();
        for ((word, kill), generated) in value.0.iter_mut().zip(&kill_sets[i].0).zip(&gen_sets[i].0)
        {
            *word = *word & !kill | generated;
        }
        if value != outgoing[i] {
            outgoing[i] = value;
            for &successor in &next[i] {
                if !queued[successor] {
                    queued[successor] = true;
                    queue.push_back(successor);
                }
            }
        }
    }
    let mut ddg = Edges {
        graph: &graph,
        edges: Vec::new(),
        seen: HashSet::new(),
    };
    for &i in &order {
        let uses = graph.uses(i);
        if ddg_node(&nodes[i]) && uses.is_empty() {
            ddg.add(entry, i, "");
        }
        if call(&nodes[i]) || nodes[i].kind == "RETURN" || output(&nodes[i]) {
            for &used in &uses {
                let relevant: Vec<_> = incoming[i]
                    .iter()
                    .map(|d| definitions[d])
                    .filter(|&d| graph.is_using(used, d))
                    .collect();
                for &definition in &relevant {
                    if definition != used {
                        ddg.add(definition, used, graph.label(definition));
                    }
                }
                if call(&nodes[i]) {
                    for &generated in &gens[i] {
                        if used != generated && ddg_node(&nodes[used]) {
                            ddg.add(used, generated, graph.label(used));
                        }
                    }
                } else if nodes[i].kind == "RETURN" {
                    ddg.add(used, i, &nodes[used].code);
                    if relevant.is_empty() {
                        ddg.add(entry, i, "");
                    }
                }
                if nodes[used].kind == "BLOCK" {
                    graph.block_edges(used, i, &incoming, &definitions, &mut ddg);
                }
            }
        }
        if nodes[i].kind == "RETURN" {
            ddg.add(i, exit, "<RET>");
        }
    }
    for (&param, &out) in params.iter().zip(&outputs) {
        ddg.add(param, out, name(&nodes[param]));
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
    function.cpg.edges.extend(edges);
}

fn reverse_postorder(entry: usize, next: &[Vec<usize>]) -> Vec<usize> {
    let mut seen = vec![false; next.len()];
    let mut result = Vec::new();
    let mut stack = vec![(entry, 0)];
    seen[entry] = true;
    while let Some((node, child)) = stack.last_mut() {
        if let Some(&successor) = next[*node].get(*child) {
            *child += 1;
            if !seen[successor] {
                seen[successor] = true;
                stack.push((successor, 0));
            }
        } else {
            result.push(*node);
            stack.pop();
        }
    }
    result.reverse();
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
impl Graph<'_> {
    fn semantic(&self, at: usize) -> Option<Semantic> {
        let node = &self.nodes[at];
        match node.method_full_name.as_deref() {
            Some(full_name) => semantics(full_name),
            None if !self.cpp || name(node).starts_with("<operator>.") => semantics(name(node)),
            None => None,
        }
    }
    fn output_argument_of_internal_method(&self, at: usize) -> bool {
        self.parent_call[at].is_some_and(|(parent, _)| {
            let node = &self.nodes[parent];
            let full_name = node
                .method_full_name
                .as_deref()
                .or_else(|| (!self.cpp).then(|| name(node)));
            full_name.is_some_and(|n| self.internal_methods.contains(n))
                && self.semantic(parent).is_none()
        })
    }
    fn descendants(&self, at: usize) -> Vec<usize> {
        let mut stack = self.ast[at].clone();
        let mut result = Vec::new();
        while let Some(i) = stack.pop() {
            result.push(i);
            stack.extend(self.ast[i].iter().copied());
        }
        result
    }
    fn argument(&self, at: usize, position: i32) -> Option<usize> {
        self.args[at]
            .iter()
            .find(|&&(p, _)| p == position)
            .map(|&(_, n)| n)
    }
    fn uses(&self, at: usize) -> Vec<usize> {
        let node = &self.nodes[at];
        if call(node) {
            self.args[at]
                .iter()
                .map(|&(_, i)| i)
                .filter(|&i| self.nodes[i].kind != "FIELD_IDENTIFIER")
                .collect()
        } else if node.kind == "RETURN" {
            self.ast[at]
                .iter()
                .copied()
                .filter(|&i| expression(&self.nodes[i]) && self.nodes[i].kind != "FIELD_IDENTIFIER")
                .collect()
        } else if output(node) {
            vec![at]
        } else {
            Vec::new()
        }
    }
    fn text(&self, at: usize) -> Option<&str> {
        let node = &self.nodes[at];
        if node.kind == "IDENTIFIER" || input(node) || output(node) {
            Some(name(node))
        } else if expression(node) {
            Some(&node.code)
        } else {
            None
        }
    }
    fn label(&self, at: usize) -> &str {
        if input(&self.nodes[at]) {
            name(&self.nodes[at])
        } else {
            &self.nodes[at].code
        }
    }
    fn is_using(&self, used: usize, definition: usize) -> bool {
        let node = &self.nodes[definition];
        let text = self.text(used);
        let same = if input(node) || node.kind == "IDENTIFIER" {
            text == Some(name(node))
        } else if matches!(
            name(node),
            "<operator>.addressOf" | "<operator>.indirection"
        ) {
            self.argument(definition, 1)
                .is_some_and(|arg| text == Some(self.nodes[arg].code.as_str()))
        } else if call(node) {
            text == Some(node.code.as_str())
        } else {
            false
        };
        if same {
            return true;
        }
        if call(node)
            && container(name(node))
            && self
                .argument(definition, 1)
                .is_some_and(|base| text == self.text(base))
        {
            return true;
        }
        if call(&self.nodes[used])
            && container(name(&self.nodes[used]))
            && (input(node) || node.kind == "IDENTIFIER")
        {
            let base = self.argument(used, 1);
            if base.is_some_and(|b| self.text(b) == Some(name(node))) {
                return true;
            }
        }
        call(&self.nodes[used])
            && call(node)
            && self
                .path(used)
                .zip(self.path(definition))
                .is_some_and(|(a, b)| a.0 == b.0 && exact_path(&a.1, &b.1))
    }
    fn used(&self, at: usize) -> bool {
        self.parent_call[at].is_none_or(|(parent, position)| {
            self.semantic(parent).is_none_or(|s| {
                s.pass && position != 0 || s.flows.iter().any(|&(src, _)| src == position)
            })
        })
    }
    fn defined(&self, at: usize) -> bool {
        self.parent_call[at].is_none_or(|(parent, position)| {
            self.semantic(parent).is_none_or(|s| {
                s.pass && position != 0 || s.flows.iter().any(|&(_, dst)| dst == position)
            })
        })
    }
    fn flow(&self, source: usize, target: usize) -> bool {
        let Some((parent, source_position)) = self.parent_call[source] else {
            return true;
        };
        let target_position = self.parent_call[target].map(|(_, p)| p).unwrap_or(-1);
        self.semantic(parent).is_none_or(|s| {
            s.pass && source_position == target_position
                || s.flows.contains(&(source_position, target_position))
        })
    }
    fn retval_blocked(&self, at: usize) -> bool {
        call(&self.nodes[at])
            && self
                .semantic(at)
                .is_some_and(|s| !s.pass && !s.flows.iter().any(|&(_, dst)| dst == -1))
    }
    fn valid_edge(&self, source: usize, target: usize) -> bool {
        let (src, dst) = (&self.nodes[source], &self.nodes[target]);
        if src.kind == "UNKNOWN" || dst.kind == "UNKNOWN" {
            return false;
        }
        if expression(dst) {
            if self.retval_blocked(source) {
                return false;
            }
            let same_site = self.parent_call[source].map(|(p, _)| p)
                == self.parent_call[target].map(|(p, _)| p);
            if expression(src) {
                if same_site && self.output_argument_of_internal_method(source) {
                    return false;
                }
                if !(same_site && self.used(source) && self.defined(target)
                    || !same_site && self.used(target))
                {
                    return false;
                }
            } else if !self.used(target) {
                return false;
            }
            if call(dst)
                && self.retval_blocked(target)
                && self.args[target].iter().any(|&(_, arg)| arg == source)
            {
                return false;
            }
            if expression(src) && same_site && self.defined(target) && self.used(source) {
                return self.flow(source, target);
            }
            true
        } else {
            !self.retval_blocked(source)
        }
    }
    fn block_edges(
        &self,
        block: usize,
        towards: usize,
        incoming: &[Bits],
        definitions: &[usize],
        edges: &mut Edges<'_, '_>,
    ) {
        let Some(&last) = self.ast[block].last() else {
            return;
        };
        if self.nodes[last].kind == "IDENTIFIER" {
            let relevant: Vec<_> = incoming[last]
                .iter()
                .map(|d| definitions[d])
                .filter(|&d| {
                    self.is_using(last, d)
                        && (self.nodes[d].kind == "IDENTIFIER" || call(&self.nodes[d]))
                })
                .collect();
            for &source in &relevant {
                edges.add(source, block, self.label(source));
            }
            if !relevant.is_empty() {
                edges.add(block, towards, "");
            }
        } else if call(&self.nodes[last]) {
            edges.add(last, block, self.label(last));
            edges.add(block, towards, "");
        }
    }
    fn path(&self, at: usize) -> Option<(String, Vec<Access>)> {
        let mut at = at;
        let mut steps = Vec::new();
        loop {
            if self.nodes[at].kind == "BLOCK" {
                at = self.ast[at]
                    .iter()
                    .rev()
                    .copied()
                    .find(|&i| expression(&self.nodes[i]))?;
                continue;
            }
            if !call(&self.nodes[at]) || !member(name(&self.nodes[at])) {
                break;
            }
            let op = name(&self.nodes[at]);
            let field = self.argument(at, 2).and_then(|i| {
                let node = &self.nodes[i];
                match node.kind.as_str() {
                    "LITERAL" => Some(node.code.clone()),
                    "FIELD_IDENTIFIER" => Some(node.name.as_deref().unwrap_or(&node.code).into()),
                    "IDENTIFIER" if op != "<operator>.indexAccess" => Some(name(node).into()),
                    _ => None,
                }
            });
            let token = field.map(Access::Field).unwrap_or(Access::Variable);
            let shift = self
                .argument(at, 2)
                .and_then(|i| self.nodes[i].code.parse::<i32>().ok())
                .map(Access::Shift)
                .unwrap_or(Access::VariableShift);
            steps.push(match op {
                "<operator>.indirection" => vec![Access::Deref],
                "<operator>.addressOf" => vec![Access::Address],
                "<operator>.indirectFieldAccess" => vec![Access::Deref, token],
                "<operator>.indirectIndexAccess" => vec![shift, Access::Deref],
                "<operator>.pointerShift" => vec![shift],
                "<operator>.getElementPtr" => vec![Access::Deref, token, Access::Address],
                _ => vec![token],
            });
            at = self.argument(at, 1)?;
        }
        let base = if self.nodes[at].kind == "IDENTIFIER"
            || input(&self.nodes[at])
            || output(&self.nodes[at])
        {
            format!("v:{}", name(&self.nodes[at]))
        } else if call(&self.nodes[at]) {
            format!("r:{}", self.nodes[at].id)
        } else if matches!(
            self.nodes[at].kind.as_str(),
            "LITERAL" | "METHOD_REF" | "TYPE_REF"
        ) {
            format!("leaf:{}", self.nodes[at].id)
        } else if matches!(
            self.nodes[at].kind.as_str(),
            "UNKNOWN" | "CONTROL_STRUCTURE" | "FIELD_IDENTIFIER"
        ) {
            "unknown".into()
        } else {
            return None;
        };
        let mut normalized: Vec<Access> = Vec::new();
        for step in steps.into_iter().rev().flatten() {
            match (normalized.last(), &step) {
                (_, Access::Shift(0)) => {}
                (Some(Access::Deref), Access::Address) | (Some(Access::Address), Access::Deref) => {
                    normalized.pop();
                }
                (Some(Access::Shift(a)), Access::Shift(b)) => {
                    let sum = a.wrapping_add(*b);
                    normalized.pop();
                    if sum != 0 {
                        normalized.push(Access::Shift(sum));
                    }
                }
                (Some(Access::Shift(_) | Access::VariableShift), Access::VariableShift)
                | (Some(Access::VariableShift), Access::Shift(_)) => {
                    normalized.pop();
                    normalized.push(Access::VariableShift);
                }
                _ => normalized.push(step),
            }
        }
        Some((base, normalized))
    }
}

struct Edges<'g, 'n> {
    graph: &'g Graph<'n>,
    edges: Vec<Edge>,
    seen: HashSet<(usize, usize, String)>,
}
impl Edges<'_, '_> {
    fn add(&mut self, source: usize, target: usize, label: &str) {
        if self.graph.valid_edge(source, target) && self.seen.insert((source, target, label.into()))
        {
            self.edges.push(Edge {
                source: self.graph.nodes[source].id,
                target: self.graph.nodes[target].id,
                kind: "REACHING_DEF".into(),
                label: Some(label.into()),
            });
        }
    }
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
fn exact_path(a: &[Access], b: &[Access]) -> bool {
    let head = |path: &[Access]| {
        path.iter()
            .rposition(|e| {
                !matches!(
                    e,
                    Access::Address | Access::Shift(_) | Access::VariableShift
                )
            })
            .map(|i| i + 1)
            .unwrap_or(0)
    };
    let (ah, bh) = (head(a), head(b));
    if ah != bh
        || a[..ah] != b[..bh]
        || a[..ah]
            .iter()
            .any(|e| matches!(e, Access::Variable | Access::VariableShift))
    {
        return false;
    }
    for (a, b) in a.iter().zip(b).skip(ah) {
        if matches!(
            (a, b),
            (
                Access::VariableShift,
                Access::Shift(_) | Access::VariableShift
            ) | (Access::Shift(_), Access::VariableShift)
        ) {
            return false;
        }
        if a != b {
            break;
        }
    }
    !b[bh..].contains(&Access::VariableShift)
}

struct Semantic {
    flows: &'static [(i32, i32)],
    pass: bool,
}
fn semantics(name: &str) -> Option<Semantic> {
    let flows: &[(i32, i32)] = match name {
        "<operator>.addition"
        | "<operator>.cast"
        | "<operator>.elvis"
        | "calloc"
        | "difftime"
        | "difftime64"
        | "fdopen" => &[(1, -1), (2, -1)],
        "<operator>.assignment" => &[(2, 1), (2, -1)],
        "<operator>.assignmentAnd"
        | "<operator>.assignmentArithmeticShiftRight"
        | "<operator>.assignmentDivision"
        | "<operator>.assignmentExponentiation"
        | "<operator>.assignmentLogicalShiftRight"
        | "<operator>.assignmentMinus"
        | "<operator>.assignmentModulo"
        | "<operator>.assignmentMultiplication"
        | "<operator>.assignmentOr"
        | "<operator>.assignmentPlus"
        | "<operator>.assignmentShiftLeft"
        | "<operator>.assignmentXor" => &[(2, 1), (1, 1), (2, -1)],
        "<operator>.addressOf"
        | "<operator>.computedMemberAccess"
        | "<operator>.notNullAssert"
        | "<operator>.fieldAccess"
        | "<operator>.getElementPtr"
        | "<operator>.indexAccess"
        | "<operator>.indirectComputedMemberAccess"
        | "<operator>.indirectFieldAccess"
        | "<operator>.indirectMemberAccess"
        | "<operator>.indirection"
        | "<operator>.memberAccess"
        | "<operator>.pointerShift"
        | "ctime"
        | "ctime64"
        | "ctime_r"
        | "ctime64_r"
        | "exp"
        | "fabs" => &[(1, -1)],
        "<operator>.indirectIndexAccess" => &[(1, -1), (2, 1)],
        "<operator>.conditional" => &[(2, -1), (3, -1)],
        "<operator>.incBy" => &[(1, 1), (2, 1), (3, 1), (4, 1)],
        "<operator>.postDecrement"
        | "<operator>.postIncrement"
        | "<operator>.preDecrement"
        | "<operator>.preIncrement"
        | "abs"
        | "asctime"
        | "asctime_r"
        | "atof"
        | "atoi"
        | "atol"
        | "fclose"
        | "feof"
        | "ferror"
        | "fflush"
        | "fgetc"
        | "strlen" => &[(1, 1), (1, -1)],
        "<operator>.sizeOf" | "abort" | "clock" => &[],
        "ceil" | "exit" | "free" | "getc" => &[(1, 1)],
        "fwrite" => &[(1, 1), (1, -1), (2, -1), (3, -1), (4, -1)],
        "scanf" => &[(2, 2)],
        "strcmp" => &[(1, 1), (1, -1), (2, 2), (2, -1)],
        "strncpy" => &[(1, 1), (2, 2), (3, 3), (1, -1), (2, -1)],
        "strncat" => &[(1, 1), (1, -1), (2, 2), (2, -1)],
        "<operator>.modulo" | "<operator>.arrayInitializer" | "div" => {
            return Some(Semantic {
                flows: &[],
                pass: true,
            });
        }
        _ => return None,
    };
    Some(Semantic { flows, pass: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Cfg;
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(serde::Deserialize)]
    struct Oracle {
        methods: Vec<FunctionGraph>,
        internal_methods: HashMap<String, HashSet<String>>,
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

    fn definitions(
        function: &FunctionGraph,
    ) -> BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)> {
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
    }

    #[test]
    fn native_c_frontend_matches_original_cfg_bindings_ddg_and_rd_sets() {
        let oracle: Oracle = serde_json::from_str(include_str!(
            "../tests/fixtures/dataflow/joern-4.0.150.json"
        ))
        .unwrap();
        let mut count = 0;
        for (filename, source) in [
            ("flow.c", include_str!("../tests/fixtures/dataflow/flow.c")),
            (
                "semantics.c",
                include_str!("../tests/fixtures/dataflow/semantics.c"),
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
            "../tests/fixtures/dataflow/joern-4.0.150-builtins.json"
        ))
        .unwrap();
        let analysis = crate::analyze(
            include_str!("../tests/fixtures/dataflow/builtins.c"),
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
    fn original_joern_c_cpp_solver_snapshots_match_by_expression_role() {
        let mut count = 0;
        for snapshot in [
            include_str!("../tests/fixtures/dataflow/joern-4.0.150.json"),
            include_str!("../tests/fixtures/dataflow/joern-4.0.150-builtins.json"),
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
        let source = include_str!("../tests/fixtures/dataflow/semantics.c");
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
            let flows_to_return_value =
                function.cpg.edges.iter().any(|e| {
                    e.kind == "REACHING_DEF" && e.source == argument && e.target == call.id
                });
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
}
