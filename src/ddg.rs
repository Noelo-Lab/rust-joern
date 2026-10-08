//! Joern 4.0.150's semantic DOT DDG projection. The reaching-definition graph
//! remains available separately: DOT hides some sources, bypasses them while
//! retaining their incoming labels, and maps arguments to their enclosing call.
use crate::dataflow::{expression, member, semantics, Semantic};
use crate::graph::{Edge, FunctionGraph, Node, PropertyGraph};
use std::collections::{HashMap, HashSet, VecDeque};

pub fn project(
    function: &FunctionGraph,
    nonstub_methods: &HashSet<String>,
    internal_methods: &HashSet<String>,
    cpp: bool,
) -> PropertyGraph {
    let mut context = Context::new(function, nonstub_methods, internal_methods, cpp);
    let visible = context.candidates();
    let mut displayed = Vec::new();
    for &node in &visible {
        displayed.extend(context.incoming_to_display(node));
    }
    let referenced: HashSet<_> = displayed
        .iter()
        .flat_map(|edge| [edge.source, edge.target])
        .collect();
    let mut seen_nodes = HashSet::new();
    let nodes = visible
        .into_iter()
        .filter(|node| referenced.contains(node))
        .map(|node| context.surrounding_call(node))
        .filter(|&node| !context.member_call(node))
        .filter(|&node| seen_nodes.insert(node))
        .map(|node| context.nodes[node].clone())
        .collect();
    let mut seen_edges = HashSet::new();
    let edges = displayed
        .into_iter()
        .filter_map(|edge| {
            let source = context.surrounding_call(edge.source);
            let target = context.surrounding_call(edge.target);
            if source == target || context.member_call(source) || context.member_call(target) {
                return None;
            }
            let projected = Edge {
                source: context.nodes[source].id,
                target: context.nodes[target].id,
                kind: "DDG".into(),
                label: edge.label,
            };
            seen_edges.insert(projected.clone()).then_some(projected)
        })
        .collect();
    PropertyGraph { nodes, edges }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Incoming {
    source: usize,
    target: usize,
    label: Option<String>,
}

struct Frame {
    node: usize,
    result: Vec<Incoming>,
    hidden: Vec<Incoming>,
    next: usize,
}

struct Context<'a> {
    nodes: &'a [Node],
    nonstub_methods: &'a HashSet<String>,
    internal_methods: &'a HashSet<String>,
    cpp: bool,
    ast: Vec<Vec<usize>>,
    ast_parent: Vec<Option<usize>>,
    in_call: Vec<Option<(usize, i32)>>,
    arguments: Vec<Vec<usize>>,
    incoming: Vec<Vec<Incoming>>,
    cache: HashMap<usize, Vec<Incoming>>,
}

impl<'a> Context<'a> {
    fn new(
        function: &'a FunctionGraph,
        nonstub_methods: &'a HashSet<String>,
        internal_methods: &'a HashSet<String>,
        cpp: bool,
    ) -> Self {
        let nodes = &function.cpg.nodes;
        let index: HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(position, node)| (node.id, position))
            .collect();
        let mut context = Self {
            nodes,
            nonstub_methods,
            internal_methods,
            cpp,
            ast: vec![Vec::new(); nodes.len()],
            ast_parent: vec![None; nodes.len()],
            in_call: vec![None; nodes.len()],
            arguments: vec![Vec::new(); nodes.len()],
            incoming: vec![Vec::new(); nodes.len()],
            cache: HashMap::new(),
        };
        for edge in &function.cpg.edges {
            let (Some(&source), Some(&target)) = (index.get(&edge.source), index.get(&edge.target))
            else {
                continue;
            };
            match edge.kind.as_str() {
                "AST" => {
                    context.ast[source].push(target);
                    context.ast_parent[target] = Some(source);
                }
                "ARGUMENT" if nodes[source].kind == "CALL" => {
                    let position = edge
                        .label
                        .as_deref()
                        .and_then(|label| label.parse::<i32>().ok())
                        .unwrap_or(context.arguments[source].len() as i32 + 1);
                    context.arguments[source].push(target);
                    // Expression.inCall takes its first incoming ARGUMENT edge.
                    context.in_call[target].get_or_insert((source, position));
                }
                "REACHING_DEF" => context.incoming[target].push(Incoming {
                    source,
                    target,
                    label: edge.label.clone(),
                }),
                _ => {}
            }
        }
        context
    }

    fn name(&self, node: usize) -> &str {
        self.nodes[node].name.as_deref().unwrap_or("")
    }

    fn semantic(&self, node: usize) -> Option<Semantic> {
        let node = &self.nodes[node];
        match node.method_full_name.as_deref() {
            Some(full_name) => semantics(full_name),
            None if !self.cpp
                || node
                    .name
                    .as_deref()
                    .unwrap_or("")
                    .starts_with("<operator>.") =>
            {
                semantics(node.name.as_deref().unwrap_or(""))
            }
            None => None,
        }
    }

    fn method_name(&self, node: usize) -> Option<&str> {
        self.nodes[node]
            .method_full_name
            .as_deref()
            .or_else(|| (!self.cpp).then(|| self.name(node)))
    }

    fn internal_call(&self, node: usize) -> bool {
        self.method_name(node)
            .is_some_and(|name| self.internal_methods.contains(name))
    }

    fn nonstub_call(&self, node: usize) -> bool {
        self.method_name(node)
            .is_some_and(|name| self.nonstub_methods.contains(name))
    }

    fn used(&self, node: usize) -> bool {
        self.in_call[node].is_none_or(|(call, argument)| {
            self.semantic(call).is_none_or(|semantic| {
                semantic.pass && argument != 0
                    || semantic.flows.iter().any(|&(source, _)| source == argument)
            })
        })
    }

    fn defined(&self, node: usize) -> bool {
        self.in_call[node].is_none_or(|(call, argument)| {
            self.semantic(call).is_none_or(|semantic| {
                semantic.pass && argument != 0
                    || semantic.flows.iter().any(|&(_, target)| target == argument)
            })
        })
    }

    fn retval_blocked(&self, node: usize) -> bool {
        self.nodes[node].kind == "CALL"
            && self.semantic(node).is_some_and(|semantic| {
                !semantic.pass && !semantic.flows.iter().any(|&(_, target)| target == -1)
            })
    }

    fn valid(&self, source: usize, target: usize) -> bool {
        if !expression(&self.nodes[target]) {
            return !self.retval_blocked(source);
        }
        if self.retval_blocked(source) {
            return false;
        }
        if expression(&self.nodes[source]) {
            let same_call = self.in_call[source].map(|(call, _)| call)
                == self.in_call[target].map(|(call, _)| call);
            if same_call
                && self.in_call[source].is_some_and(|(call, _)| {
                    self.nonstub_call(call) && self.semantic(call).is_none()
                })
            {
                return false;
            }
            if !(same_call && self.used(source) && self.defined(target)
                || !same_call && self.used(target))
            {
                return false;
            }
        } else if !self.used(target) {
            return false;
        }
        if self.nodes[target].kind == "CALL"
            && self.retval_blocked(target)
            && self.arguments[target].contains(&source)
        {
            return false;
        }
        let ast_call =
            |node: usize| self.ast_parent[node].filter(|&parent| self.nodes[parent].kind == "CALL");
        if expression(&self.nodes[source])
            && ast_call(source) == ast_call(target)
            && self.defined(target)
            && self.used(source)
        {
            let Some((call, source_argument)) = self.in_call[source] else {
                return true;
            };
            let target_argument = self.in_call[target]
                .map(|(_, argument)| argument)
                .unwrap_or(-1);
            return self.semantic(call).is_none_or(|semantic| {
                semantic.pass && source_argument == target_argument
                    || semantic.flows.contains(&(source_argument, target_argument))
            });
        }
        true
    }

    fn source_visible(&self, source: usize, target: usize) -> bool {
        if !expression(&self.nodes[source]) || !expression(&self.nodes[target]) {
            return true;
        }
        let same_call = self.in_call[source].map(|(call, _)| call)
            == self.in_call[target].map(|(call, _)| call);
        if same_call {
            let semantic_exists =
                self.in_call[source].is_some_and(|(call, _)| self.semantic(call).is_some());
            let internal = self.in_call[source].is_some_and(|(call, _)| self.internal_call(call));
            semantic_exists && self.defined(source) || !internal
        } else {
            self.defined(source)
        }
    }

    fn displayed(&self, node: usize) -> bool {
        !matches!(
            self.nodes[node].kind.as_str(),
            "CONTROL_STRUCTURE" | "JUMP_TARGET"
        )
    }

    fn member_call(&self, node: usize) -> bool {
        self.nodes[node].kind == "CALL" && member(self.name(node))
    }

    fn surrounding_call(&self, node: usize) -> usize {
        if expression(&self.nodes[node]) {
            self.in_call[node].map(|(call, _)| call).unwrap_or(node)
        } else {
            node
        }
    }

    fn candidates(&self) -> Vec<usize> {
        let Some(entry) = self.nodes.iter().position(|node| node.kind == "METHOD") else {
            return Vec::new();
        };
        let mut result = vec![entry];
        result.extend(
            self.nodes
                .iter()
                .enumerate()
                .filter_map(|(position, node)| (node.kind == "METHOD_RETURN").then_some(position)),
        );
        result.extend(
            self.nodes
                .iter()
                .enumerate()
                .filter_map(|(position, node)| {
                    (node.kind == "METHOD_PARAMETER_IN").then_some(position)
                }),
        );
        // ContainsEdgePass traverses breadth first, stopping at nested method
        // and type declarations. Output parameters and locals are not CONTAINS
        // targets even though output parameters themselves are CfgNodes.
        let mut queue = VecDeque::from([entry]);
        let mut seen = HashSet::from([entry]);
        while let Some(parent) = queue.pop_front() {
            for &node in &self.ast[parent] {
                if !seen.insert(node) {
                    continue;
                }
                if expression(&self.nodes[node]) || self.nodes[node].kind == "JUMP_TARGET" {
                    result.push(node);
                }
                if !matches!(self.nodes[node].kind.as_str(), "METHOD" | "TYPE_DECL") {
                    queue.push_back(node);
                }
            }
        }
        result.retain(|&node| self.displayed(node));
        result
    }

    fn frame(&self, node: usize) -> Frame {
        let mut frame = Frame {
            node,
            result: Vec::new(),
            hidden: Vec::new(),
            next: 0,
        };
        for incoming in &self.incoming[node] {
            // Engine.ddgInPathElem excludes METHOD sources; DdgGenerator adds
            // those raw entry edges separately, bypassing EdgeValidator.
            if self.nodes[incoming.source].kind == "METHOD" {
                frame.result.push(incoming.clone());
            } else if incoming.source != node && self.valid(incoming.source, node) {
                if self.displayed(incoming.source) && self.source_visible(incoming.source, node) {
                    frame.result.push(incoming.clone());
                } else {
                    frame.hidden.push(incoming.clone());
                }
            }
        }
        frame
    }

    fn incoming_to_display(&mut self, node: usize) -> Vec<Incoming> {
        if let Some(cached) = self.cache.get(&node) {
            return cached.clone();
        }
        // An explicit stack reproduces the source's recursive visited-path
        // behavior without overflowing on a large native method.
        let mut frames = vec![self.frame(node)];
        let mut visited = HashSet::from([node]);
        while let Some(frame) = frames.last_mut() {
            if let Some(hidden) = frame.hidden.get(frame.next) {
                let source = hidden.source;
                if let Some(cached) = self.cache.get(&source) {
                    frame.result.extend(cached.iter().map(|edge| Incoming {
                        source: edge.source,
                        target: frame.node,
                        label: edge.label.clone(),
                    }));
                    frame.next += 1;
                } else if visited.insert(source) {
                    frames.push(self.frame(source));
                } else {
                    frame.next += 1;
                }
            } else {
                let frame = frames.pop().unwrap();
                visited.remove(&frame.node);
                let mut seen = HashSet::new();
                let result = frame
                    .result
                    .into_iter()
                    .filter(|edge| seen.insert(edge.clone()))
                    .collect();
                self.cache.insert(frame.node, result);
            }
        }
        self.cache[&node].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Options;

    #[test]
    fn original_pyjoern_copy_projection_retains_parallel_labels_and_return_identifier() {
        // Unchanged PyJoern 4.0.150.4 FastParser's flow.c::copy DDG has these
        // ten DOT edges. Its public DiGraph reduces them to eight edges.
        let analysis = crate::analyze(
            "int copy(int x) { int y = x; return y; }",
            "copy.c",
            &Options {
                data_flow: true,
                ..Default::default()
            },
        )
        .unwrap();
        let function = &analysis.functions[0];
        let projected = project(function, &HashSet::new(), &HashSet::new(), false);
        let nodes: HashMap<_, _> = projected.nodes.iter().map(|node| (node.id, node)).collect();
        let key = |id| {
            let node = nodes[&id];
            if node.kind == "METHOD" {
                format!("METHOD:{}", node.name.as_deref().unwrap())
            } else {
                format!("{}:{}", node.kind, node.code)
            }
        };
        let edges: HashSet<_> = projected
            .edges
            .iter()
            .map(|edge| {
                (
                    key(edge.source),
                    key(edge.target),
                    edge.label.as_deref().unwrap_or(""),
                )
            })
            .collect();
        let expected: HashSet<_> = [
            ("RETURN:return y;", "METHOD_RETURN:RET", "<RET>"),
            ("CALL:y = x", "METHOD_RETURN:RET", "y"),
            ("CALL:y = x", "METHOD_RETURN:RET", "x"),
            ("CALL:y = x", "METHOD_RETURN:RET", "y = x"),
            ("METHOD:copy", "METHOD_PARAMETER_IN:int x", ""),
            ("METHOD_PARAMETER_IN:int x", "CALL:y = x", "x"),
            ("METHOD:copy", "CALL:y = x", ""),
            ("IDENTIFIER:y", "RETURN:return y;", "y"),
            ("CALL:y = x", "IDENTIFIER:y", "y"),
            ("METHOD:copy", "IDENTIFIER:y", ""),
        ]
        .into_iter()
        .map(|(source, target, label)| (source.into(), target.into(), label))
        .collect();
        assert_eq!(projected.nodes.len(), 6);
        assert_eq!(edges, expected);
        assert!(function
            .ddg
            .as_ref()
            .unwrap()
            .nodes
            .iter()
            .any(|node| node.kind == "METHOD_PARAMETER_OUT"));
        assert!(projected
            .nodes
            .iter()
            .all(|node| node.kind != "METHOD_PARAMETER_OUT"));
    }
}
