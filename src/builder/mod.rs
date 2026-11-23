use crate::{
    graph::{
        Diagnostic, Edge, ExternalReference, FunctionGraph, Node, NodeId, Options, PropertyGraph,
    },
    joern_calls::{CallKind, CallLayout, Operand, unresolved_full_name},
    joern_operators::binary_operator,
    lexer::integer_truth,
    normalize,
    syntax::*,
};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

mod control_flow;
mod expressions;
mod nodes;
mod statements;
use expressions::*;

#[derive(Default)]
struct Fragment {
    entry: Option<NodeId>,
    edges: Vec<[NodeId; 2]>,
    fringe: Vec<NodeId>,
    breaks: Vec<NodeId>,
    continues: Vec<NodeId>,
    labels: HashMap<String, NodeId>,
    cases: Vec<(NodeId, bool)>,
    jumps: Vec<(NodeId, String)>,
}

/// Canonical parser declarations consumed by bounded call lookup.
pub struct CallBindings<'a> {
    pub methods: &'a HashMap<String, Vec<&'a Function>>,
    pub known_types: &'a HashSet<String>,
}

struct Builder<'a> {
    source: &'a str,
    filename: &'a str,
    cpp: bool,
    graph: PropertyGraph,
    scopes: Vec<HashMap<String, (NodeId, String)>>,
    functions: &'a HashSet<String>,
    methods: &'a HashMap<String, Vec<&'a Function>>,
    known_types: &'a HashSet<String>,
    function_pointers: HashSet<NodeId>,
    closures: HashMap<NodeId, Closure>,
    inherited_bindings: HashMap<String, String>,
    inherited_closures: HashMap<String, Closure>,
    implicit_fields: HashMap<String, String>,
    member_cv_qualified: bool,
    lambda: bool,
    method_full_name: &'a str,
    method_scope: String,
    diagnostics: Vec<Diagnostic>,
    try_depth: usize,
    joern_loop_conditions: bool,
    joern_cpp_control_flow: bool,
}

pub fn build(
    function: &Function,
    source: &str,
    filename: &str,
    known_functions: &HashSet<String>,
    cpp: bool,
    bindings: &CallBindings<'_>,
    options: &Options,
) -> (FunctionGraph, Vec<Diagnostic>) {
    let mut builder = Builder {
        source,
        filename,
        cpp,
        graph: PropertyGraph::default(),
        scopes: vec![HashMap::new()],
        functions: known_functions,
        methods: bindings.methods,
        known_types: bindings.known_types,
        function_pointers: HashSet::new(),
        closures: HashMap::new(),
        inherited_bindings: function.inherited_bindings.iter().cloned().collect(),
        inherited_closures: function.inherited_closures.iter().cloned().collect(),
        implicit_fields: function.implicit_fields.iter().cloned().collect(),
        member_cv_qualified: function.member_cv_qualified,
        lambda: function.lambda,
        method_full_name: &function.full_name,
        method_scope: if function.extern_c {
            String::new()
        } else {
            function
                .full_name
                .split(':')
                .next()
                .unwrap_or("")
                .rsplit_once('.')
                .map(|(scope, _)| scope.to_string())
                .unwrap_or_default()
        },
        diagnostics: Vec::new(),
        try_depth: 0,
        joern_loop_conditions: options.joern_loop_conditions,
        joern_cpp_control_flow: options.joern_cpp_control_flow,
    };
    let entry = builder.node("METHOD", &function.span, Some(&function.name), None);
    let exit = builder.node("METHOD_RETURN", &function.span, None, Some("RET"));
    builder.graph.nodes[exit as usize].type_name = Some(function.return_type.clone());
    if let Some(owner) = &function.implicit_this {
        let id = builder.node(
            "METHOD_PARAMETER_IN",
            &function.span,
            Some("this"),
            Some("this"),
        );
        builder.graph.nodes[id as usize].type_name = Some(owner.clone());
        builder.ast(entry, id, None);
        builder.scopes[0].insert("this".into(), (id, owner.clone()));
    }
    let mut parameters: Vec<_> = function.parameters.iter().collect();
    if options.joern_knr_parameters && parameters.iter().any(|parameter| parameter.knr.is_some()) {
        // ICASTKnRFunctionDeclarator.getParameterDeclarations: `int a, b;`
        // is one parameter named `a`, and an undeclared name is not one.
        parameters.retain(|parameter| parameter.knr == Some(KnrParameter::First));
        parameters.sort_by_key(|parameter| parameter.span.start);
    }
    let mut last_parameter = None;
    for parameter in parameters {
        if parameter.type_name == "..." && parameter.name == "..." {
            // setVariadic marks the last named parameter instead of creating
            // one for the ellipsis; `f(...)` has no parameter at all.
            if let Some(id) = last_parameter {
                builder.graph.nodes[id as usize].code.push_str("...");
            }
            continue;
        }
        let id = builder.node(
            "METHOD_PARAMETER_IN",
            &parameter.span,
            (!parameter.name.is_empty()).then_some(parameter.name.as_str()),
            None,
        );
        last_parameter = Some(id);
        builder.graph.nodes[id as usize].type_name = Some(parameter.type_name.clone());
        if parameter.function_pointer {
            builder.function_pointers.insert(id);
        }
        builder.ast(entry, id, None);
        if !function.lambda {
            builder.scopes[0].insert(parameter.name.clone(), (id, parameter.type_name.clone()));
        }
    }
    let mut initializers = Fragment::default();
    for initialization in &function.constructor_initializers {
        let id = builder.node(
            "CALL",
            &initialization.span,
            Some("<operator>.fieldInitialization"),
            None,
        );
        builder.ast(entry, id, None);
        let target = Expr {
            kind: ExprKind::Member {
                base: Box::new(Expr {
                    kind: ExprKind::Identifier("this".into()),
                    span: initialization.member_span.clone(),
                }),
                name: initialization.member.clone(),
                indirect: true,
            },
            span: initialization.member_span.clone(),
        };
        let target_flow = builder.expression(&target, id, Some(1));
        let value_flow = builder.expression(&initialization.value, id, Some(2));
        initializers = initializers
            .append(target_flow)
            .append(value_flow)
            .append(Fragment::single(id));
    }
    let body = if matches!(function.body.kind, StmtKind::Empty) {
        // Function declarations still have a body container in Joern's CPG.
        let block = builder.node("BLOCK", &Span::default(), None, Some(""));
        builder.graph.nodes[block as usize].type_name = Some("ANY".into());
        builder.ast(entry, block, None);
        Fragment::default()
    } else {
        builder.statement(&function.body, entry, false)
    };
    if function.lambda || function.is_static {
        let modifier = builder.node("MODIFIER", &Span::default(), None, Some(""));
        builder.ast(entry, modifier, None);
    }
    builder.ast(entry, exit, None);
    let mut flow = Fragment::single(entry)
        .append(initializers)
        .append(body)
        .append(Fragment::single(exit));
    for (source, target) in &flow.jumps {
        if target == "*" {
            flow.edges
                .extend(flow.labels.values().map(|&id| [*source, id]));
        } else if let Some(&id) = flow.labels.get(target) {
            flow.edges.push([*source, id]);
        }
        // Joern leaves an unresolved goto without an outgoing edge. Its
        // missing-label message is logging, rather than a parse failure.
    }
    let mut seen = HashSet::new();
    for [source, target] in flow.edges {
        if seen.insert((source, target)) {
            builder.edge(source, target, "CFG", None);
        }
    }
    builder.finish_cfg_statement_kinds();
    let cfg = normalize::normalize(&builder.graph);
    let graph = FunctionGraph {
        name: function.name.clone(),
        fullname: function.full_name.clone(),
        filename: filename.into(),
        return_type: function.return_type.clone(),
        signature: function.signature.clone(),
        start_line: function.span.line as i64,
        end_line: function.span.end_line as i64,
        cpg: builder.graph,
        cfg,
        ddg: None,
        ddg_projection: None,
        ddg_view: None,
        reaching_definitions: None,
    };
    (graph, builder.diagnostics)
}

#[cfg(test)]
mod tests;
