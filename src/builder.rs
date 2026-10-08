use crate::{
    graph::{Diagnostic, Edge, ExternalReference, FunctionGraph, Node, NodeId, PropertyGraph},
    normalize,
    syntax::*,
};
use std::collections::{HashMap, HashSet};

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

impl Fragment {
    fn single(node: NodeId) -> Self {
        Self {
            entry: Some(node),
            fringe: vec![node],
            ..Self::default()
        }
    }
    fn empty(&self) -> bool {
        self.entry.is_none()
            && self.edges.is_empty()
            && self.fringe.is_empty()
            && self.labels.is_empty()
            && self.jumps.is_empty()
    }
    fn append(mut self, other: Self) -> Self {
        if other.empty() {
            return self;
        }
        if self.empty() {
            return other;
        }
        self.edges.extend(other.edges);
        if let Some(target) = other.entry {
            self.edges
                .extend(self.fringe.iter().map(|&source| [source, target]));
        }
        self.fringe = other.fringe;
        self.breaks.extend(other.breaks);
        self.continues.extend(other.continues);
        self.labels.extend(other.labels);
        self.cases.extend(other.cases);
        self.jumps.extend(other.jumps);
        self
    }
    fn combine(parts: impl IntoIterator<Item = Self>) -> Self {
        let mut result = Self::default();
        for part in parts {
            result.edges.extend(part.edges);
            result.breaks.extend(part.breaks);
            result.continues.extend(part.continues);
            result.labels.extend(part.labels);
            result.cases.extend(part.cases);
            result.jumps.extend(part.jumps);
        }
        result
    }
}

struct Builder<'a> {
    source: &'a str,
    filename: &'a str,
    cpp: bool,
    graph: PropertyGraph,
    scopes: Vec<HashMap<String, (NodeId, String)>>,
    functions: &'a HashSet<String>,
    methods: &'a HashMap<String, Vec<&'a Function>>,
    function_pointers: HashSet<NodeId>,
    lambda_bindings: HashMap<NodeId, (String, String)>,
    is_lambda: bool,
    method_full_name: &'a str,
    method_scope: String,
    diagnostics: Vec<Diagnostic>,
    try_depth: usize,
}

pub fn build(
    function: &Function,
    source: &str,
    filename: &str,
    known_functions: &HashSet<String>,
    cpp: bool,
    methods: &HashMap<String, Vec<&Function>>,
) -> (FunctionGraph, Vec<Diagnostic>) {
    let mut builder = Builder {
        source,
        filename,
        cpp,
        graph: PropertyGraph::default(),
        scopes: vec![HashMap::new()],
        functions: known_functions,
        methods,
        function_pointers: HashSet::new(),
        lambda_bindings: HashMap::new(),
        is_lambda: function.lambda_parent.is_some(),
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
    for parameter in &function.parameters {
        let id = builder.node(
            "METHOD_PARAMETER_IN",
            &parameter.span,
            (!parameter.name.is_empty()).then_some(parameter.name.as_str()),
            None,
        );
        builder.graph.nodes[id as usize].type_name = Some(parameter.type_name.clone());
        if parameter.function_pointer {
            builder.function_pointers.insert(id);
        }
        builder.ast(entry, id, None);
        if !builder.is_lambda {
            builder.scopes[0].insert(parameter.name.clone(), (id, parameter.type_name.clone()));
        }
    }
    let body = if matches!(function.body.kind, StmtKind::Empty) {
        // Function declarations still have a body container in Joern's CPG.
        let block = builder.node("BLOCK", &function.body.span, None, Some(""));
        builder.graph.nodes[block as usize].type_name = Some("ANY".into());
        builder.ast(entry, block, None);
        Fragment::default()
    } else {
        builder.statement(&function.body, entry, false)
    };
    if builder.is_lambda {
        let modifier = builder.node("MODIFIER", &Span::default(), None, Some(""));
        builder.ast(entry, modifier, None);
    }
    builder.ast(entry, exit, None);
    let mut flow = Fragment::single(entry)
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
        reaching_definitions: None,
    };
    (graph, builder.diagnostics)
}

impl Builder<'_> {
    fn code(&self, span: &Span) -> &str {
        self.source.get(span.start..span.end).unwrap_or("")
    }
    fn node(&mut self, kind: &str, span: &Span, name: Option<&str>, code: Option<&str>) -> NodeId {
        let id = self.graph.nodes.len() as NodeId;
        self.graph.nodes.push(Node {
            id,
            kind: kind.into(),
            code: code.unwrap_or_else(|| self.code(span)).into(),
            name: name.map(str::to_string),
            method_full_name: (kind == "CALL")
                .then(|| name.filter(|name| !self.cpp || name.starts_with("<operator>.")))
                .flatten()
                .map(str::to_string),
            external_ref: None,
            cfg_nop: matches!(kind, "METHOD" | "METHOD_RETURN" | "METHOD_REF").then_some(true),
            type_name: match kind {
                "BLOCK" => Some("void".into()),
                "CALL" => Some("ANY".into()),
                _ => None,
            },
            line: span.line as i64,
            column: span.column,
            start_byte: span.start,
            end_byte: span.end,
        });
        id
    }
    fn edge(&mut self, source: NodeId, target: NodeId, kind: &str, label: Option<String>) {
        self.graph.edges.push(Edge {
            source,
            target,
            kind: kind.into(),
            label,
        });
    }
    fn ast(&mut self, parent: NodeId, child: NodeId, argument: Option<usize>) {
        self.edge(parent, child, "AST", None);
        if let Some(argument) = argument {
            self.edge(parent, child, "ARGUMENT", Some(argument.to_string()));
        }
    }
    fn binding(&self, name: &str) -> Option<&(NodeId, String)> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }
    fn identifier(
        &mut self,
        name: &str,
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
    ) -> NodeId {
        let binding = self.binding(name).cloned();
        let known_function = binding.is_none() && self.functions.contains(name);
        let expected_signature = (known_function && self.cpp)
            .then(|| {
                let argument = argument?;
                let fullname = self.graph.nodes[parent as usize]
                    .method_full_name
                    .as_deref()?;
                function_pointer_parameter(fullname, argument)
            })
            .flatten();
        let reference = known_function
            .then(|| {
                self.resolve_method_reference(name, expected_signature.as_deref())
                    .map(|method| (method.full_name.clone(), method.return_type.clone()))
            })
            .flatten();
        let kind = if known_function && (!self.cpp || reference.is_some()) {
            "METHOD_REF"
        } else {
            "IDENTIFIER"
        };
        let id = self.node(kind, span, Some(name), Some(name));
        self.ast(parent, id, argument);
        if let Some((target, type_name)) = binding {
            self.graph.nodes[id as usize].type_name = Some(type_name);
            self.edge(id, target, "REF", None);
        } else if kind == "METHOD_REF"
            && let Some((fullname, return_type)) = reference
        {
            let node = &mut self.graph.nodes[id as usize];
            node.method_full_name = Some(fullname.clone());
            node.type_name = Some(return_type);
            if fullname == self.method_full_name {
                self.edge(id, 0, "REF", None);
            } else {
                node.external_ref = Some(ExternalReference {
                    method_full_name: fullname,
                    node: 0,
                });
            }
        } else if kind == "IDENTIFIER" && (self.is_lambda || self.cpp && known_function) {
            self.graph.nodes[id as usize].type_name = Some("ANY".into());
        }
        id
    }
    fn resolve_method_reference(&self, name: &str, signature: Option<&str>) -> Option<&Function> {
        let qualified = name.trim_start_matches("::").replace("::", ".");
        let scoped = (self.cpp && !name.contains("::") && !self.method_scope.is_empty())
            .then(|| format!("{}.{}", self.method_scope, name));
        for key in scoped.iter().chain(std::iter::once(&qualified)) {
            let Some(methods) = self.methods.get(key) else {
                continue;
            };
            let candidates: Vec<_> = methods
                .iter()
                .copied()
                .filter(|method| {
                    signature.is_none_or(|signature| {
                        method
                            .full_name
                            .rsplit_once(':')
                            .is_some_and(|(_, bound)| bound == signature)
                    })
                })
                .collect();
            let first = candidates.first()?;
            if candidates.iter().all(|method| {
                method.full_name == first.full_name && method.return_type == first.return_type
            }) {
                return Some(first);
            }
            // Without a call argument list, an overloaded name has no unique
            // binding. Keep the reference node without inventing a target.
            return None;
        }
        None
    }
    fn finish_cfg_statement_kinds(&mut self) {
        let mut parents = vec![None; self.graph.nodes.len()];
        for edge in &self.graph.edges {
            if edge.kind == "AST" {
                parents[edge.target as usize] = Some(edge.source);
            }
        }
        for id in 0..self.graph.nodes.len() {
            if self.graph.nodes[id].kind != "METHOD_REF" {
                continue;
            }
            let Some(mut expression) = parents[id] else {
                continue;
            };
            // parentExpression skips generic member-access calls. Compute it
            // after generated macro CODE and every AST edge are finalized.
            while self.graph.nodes[expression as usize].kind == "CALL"
                && matches!(
                    self.graph.nodes[expression as usize].name.as_deref(),
                    Some(
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
                )
            {
                let Some(parent) = parents[expression as usize] else {
                    break;
                };
                expression = parent;
            }
            // Literal newlines in Joern's DOT label are split before lifting.
            // A split METHOD_REF label therefore loses its FUNC_START marker.
            let multiline = [id, expression as usize].into_iter().any(|node| {
                let code = &self.graph.nodes[node].code;
                let limit = if code.encode_utf16().count() > 50 {
                    47
                } else {
                    50
                };
                code.encode_utf16().take(limit).any(|c| c == b'\n' as u16)
            });
            self.graph.nodes[id].cfg_nop = Some(!multiline);
        }
    }
    fn unsupported(&mut self, span: &Span, message: &str) {
        self.diagnostics.push(Diagnostic {
            filename: self.filename.into(),
            line: span.line,
            column: span.column,
            severity: "error".into(),
            message: message.into(),
        });
    }
    fn control(&mut self, statement: &Stmt, parent: NodeId, name: &str) -> NodeId {
        let code = match &statement.kind {
            StmtKind::If { condition, .. } => Some(format!("if ({})", self.code(&condition.span))),
            StmtKind::While { condition, .. } => {
                Some(format!("while ({})", self.code(&condition.span)))
            }
            StmtKind::Switch { condition, .. } => {
                Some(format!("switch({})", self.code(&condition.span)))
            }
            StmtKind::For {
                initializer,
                condition,
                update,
                ..
            } => Some(format!(
                "for ({};{};{})",
                initializer
                    .as_ref()
                    .map(|x| self.code(&x.span).trim_end_matches(';'))
                    .unwrap_or(""),
                condition.as_ref().map(|x| self.code(&x.span)).unwrap_or(""),
                update.as_ref().map(|x| self.code(&x.span)).unwrap_or(""),
            )),
            StmtKind::Try { .. } if name == "TRY" => Some("try".into()),
            _ => None,
        };
        let id = self.node(
            "CONTROL_STRUCTURE",
            &statement.span,
            Some(name),
            code.as_deref(),
        );
        self.ast(parent, id, None);
        id
    }
    fn expression(
        &mut self,
        expression: &Expr,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        let span = &expression.span;
        match &expression.kind {
            ExprKind::Bracketed(inner) if is_expression_list(inner) => {
                let id = self.node("CALL", span, Some("<operator>.bracketedPrimary"), None);
                self.ast(parent, id, argument);
                self.expression(inner, id, Some(1))
                    .append(Fragment::single(id))
            }
            ExprKind::Bracketed(inner) => self.expression(inner, parent, argument),
            ExprKind::Generated { expression, code } => {
                let root = self.graph.nodes.len();
                let flow = self.expression(expression, parent, argument);
                if let Some(node) = self.graph.nodes.get_mut(root) {
                    node.code.clone_from(code);
                }
                flow
            }
            ExprKind::MacroCall { .. } => self.macro_call(expression, parent, argument),
            ExprKind::Lambda { full_name, .. } => {
                let id = self.node("METHOD_REF", span, None, None);
                self.graph.nodes[id as usize].method_full_name = Some(full_name.clone());
                self.graph.nodes[id as usize].type_name = Some(full_name.clone());
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
            ExprKind::Identifier(name) if self.cpp && name.contains("::") => {
                let names: Vec<_> = name.split("::").filter(|name| !name.is_empty()).collect();
                let Some((&member, qualifier)) = names.split_last() else {
                    return Fragment::default();
                };
                let id = self.node("CALL", span, Some("<operator>.fieldAccess"), None);
                self.ast(parent, id, argument);
                let owner = if qualifier.is_empty() {
                    let literal = self.node("LITERAL", span, None, Some("<global>"));
                    self.ast(id, literal, Some(1));
                    Fragment::single(literal)
                } else {
                    self.qualified_owner(qualifier, span, id, Some(1))
                };
                let field = self.node("FIELD_IDENTIFIER", span, Some(member), Some(member));
                self.ast(id, field, Some(2));
                owner
                    .append(Fragment::single(field))
                    .append(Fragment::single(id))
            }
            ExprKind::Identifier(name) => {
                Fragment::single(self.identifier(name, span, parent, argument))
            }
            ExprKind::TypeSpecifier { code, type_name } => {
                // A declaration specifier is not an IASTIdExpression, so
                // Joern never turns it into a function reference.
                let id = self.node("IDENTIFIER", span, Some(code), Some(code));
                self.graph.nodes[id as usize].type_name = Some(type_name.clone());
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
            ExprKind::Literal(value) => {
                let id = self.node("LITERAL", span, None, Some(value));
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
            ExprKind::Assembly(code) => {
                let id = self.node("UNKNOWN", span, None, Some(code));
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
            ExprKind::Problem(code) => {
                let id = self.node("UNKNOWN", span, None, Some(code));
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
            ExprKind::ArrayInitializer(expressions) => {
                let id = self.node("CALL", span, Some("<operator>.arrayInitializer"), None);
                self.ast(parent, id, argument);
                let mut flow = Fragment::default();
                for (position, expression) in expressions.iter().take(1000).enumerate() {
                    flow = flow.append(self.expression(expression, id, Some(position + 1)));
                }
                if expressions.len() > 1000 {
                    let placeholder =
                        self.node("LITERAL", span, None, Some("<too-many-initializers>"));
                    self.ast(id, placeholder, Some(1000));
                    flow = flow.append(Fragment::single(placeholder));
                }
                flow.append(Fragment::single(id))
            }
            ExprKind::Unary {
                op,
                argument: operand,
                postfix,
            } => {
                let name = unary_operator(op, *postfix);
                let id = self.node("CALL", span, Some(name), None);
                self.ast(parent, id, argument);
                self.expression(operand, id, Some(1))
                    .append(Fragment::single(id))
            }
            ExprKind::Binary { op, left, right } => {
                if op == "," {
                    let list = Expr {
                        kind: ExprKind::List(vec![(**left).clone(), (**right).clone()]),
                        span: span.clone(),
                    };
                    return self.expression(&list, parent, argument);
                }
                let id = self.node("CALL", span, Some(binary_operator(op)), None);
                self.ast(parent, id, argument);
                let left = self.expression(left, id, Some(1));
                let right = self.expression(right, id, Some(2));
                if op == "&&" || op == "||" {
                    let entry = left.entry;
                    let mut fringe = left.fringe.clone();
                    fringe.extend(&right.fringe);
                    let mut edges = Vec::new();
                    if let Some(target) = right.entry {
                        edges.extend(left.fringe.iter().map(|&source| [source, target]));
                    }
                    let mut result = Fragment::combine([left, right]);
                    result.entry = entry;
                    result.fringe = fringe;
                    result.edges.extend(edges);
                    result.append(Fragment::single(id))
                } else {
                    left.append(right).append(Fragment::single(id))
                }
            }
            ExprKind::Conditional {
                condition,
                consequence,
                alternative,
            } => {
                let id = self.node("CALL", span, Some("<operator>.conditional"), None);
                self.ast(parent, id, argument);
                let condition = self.expression(condition, id, Some(1));
                let yes = self.expression(consequence, id, Some(2));
                let no = self.expression(alternative, id, Some(3));
                let entry = condition.entry;
                let mut fringe = if yes.entry.is_some() {
                    yes.fringe.clone()
                } else {
                    condition.fringe.clone()
                };
                fringe.extend(if no.entry.is_some() {
                    &no.fringe
                } else {
                    &condition.fringe
                });
                let mut edges = Vec::new();
                for target in [yes.entry, no.entry].into_iter().flatten() {
                    edges.extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                let mut result = Fragment::combine([condition, yes, no]);
                result.entry = entry;
                result.fringe = fringe;
                result.edges.extend(edges);
                result.append(Fragment::single(id))
            }
            ExprKind::Call { callee, arguments } => {
                let lambda = if let ExprKind::Identifier(name) = &callee.kind {
                    self.binding(name)
                        .and_then(|(id, _)| self.lambda_bindings.get(id))
                        .cloned()
                } else {
                    None
                };
                let pointer = lambda.is_some()
                    || match &callee.kind {
                        ExprKind::Identifier(name) => self.binding(name).is_some_and(|(id, ty)| {
                            ty.contains('*') || self.function_pointers.contains(id)
                        }),
                        ExprKind::Member { .. } => !self.cpp,
                        _ => true,
                    };
                let name = if lambda.is_some() {
                    "<operator>()"
                } else if pointer {
                    "<operator>.pointerCall"
                } else {
                    match &callee.kind {
                        ExprKind::Identifier(name) if self.cpp => name.rsplit("::").next().unwrap(),
                        ExprKind::Identifier(name) => name.as_str(),
                        ExprKind::Member { name, .. } => name.as_str(),
                        _ => "<operator>.pointerCall",
                    }
                };
                let id = self.node("CALL", span, Some(name), None);
                if let Some((signature, return_type)) = lambda {
                    self.graph.nodes[id as usize].method_full_name =
                        Some(format!("<operator>():{signature}"));
                    self.graph.nodes[id as usize].type_name = Some(return_type);
                }
                if !pointer
                    && let Some((fullname, return_type)) = self
                        .resolve_call(callee, arguments)
                        .map(|method| (method.full_name.clone(), method.return_type.clone()))
                {
                    self.graph.nodes[id as usize].method_full_name = Some(fullname);
                    self.graph.nodes[id as usize].type_name = Some(return_type);
                }
                self.ast(parent, id, argument);
                let mut flow = Fragment::default();
                if pointer {
                    let receiver = self.graph.nodes.len() as NodeId;
                    flow = self.expression(callee, id, None);
                    if self.graph.nodes.len() != receiver as usize {
                        self.edge(id, receiver, "RECEIVER", None);
                    }
                } else {
                    match &callee.kind {
                        ExprKind::Identifier(_) => (),
                        ExprKind::Member { base, .. } if self.cpp => {
                            flow = self.expression(base, id, Some(0));
                        }
                        _ => {
                            flow = self.expression(callee, id, Some(0));
                        }
                    }
                }
                for (position, argument) in arguments.iter().enumerate() {
                    flow = flow.append(self.expression(argument, id, Some(position + 1)));
                }
                flow.append(Fragment::single(id))
            }
            ExprKind::Member {
                base,
                name,
                indirect,
            } => {
                let operator = if *indirect {
                    "<operator>.indirectFieldAccess"
                } else {
                    "<operator>.fieldAccess"
                };
                let id = self.node("CALL", span, Some(operator), None);
                self.ast(parent, id, argument);
                let base = self.expression(base, id, Some(1));
                let field = self.node("FIELD_IDENTIFIER", span, Some(name), Some(name));
                self.ast(id, field, Some(2));
                base.append(Fragment::single(field))
                    .append(Fragment::single(id))
            }
            ExprKind::Index { base, index } => {
                let id = self.node("CALL", span, Some("<operator>.indirectIndexAccess"), None);
                self.ast(parent, id, argument);
                self.expression(base, id, Some(1))
                    .append(self.expression(index, id, Some(2)))
                    .append(Fragment::single(id))
            }
            ExprKind::Cast {
                type_name,
                argument: operand,
            } => {
                let id = self.node("CALL", span, Some("<operator>.cast"), None);
                self.ast(parent, id, argument);
                let type_code = self.cast_type_code(span).unwrap_or(type_name).to_string();
                let ty = self.node("UNKNOWN", span, None, Some(&type_code));
                self.ast(id, ty, Some(1));
                Fragment::single(ty)
                    .append(self.expression(operand, id, Some(2)))
                    .append(Fragment::single(id))
            }
            ExprKind::List(expressions) => {
                let id = self.node("CALL", span, Some("<operator>.expressionList"), None);
                self.ast(parent, id, argument);
                let mut flow = Fragment::default();
                for (index, expression) in expressions.iter().enumerate() {
                    flow = flow.append(self.expression(expression, id, Some(index + 1)));
                }
                flow.append(Fragment::single(id))
            }
            ExprKind::Block(expressions) => {
                self.expression_block(expressions, span, parent, argument)
            }
            ExprKind::Statement(statement) => {
                let before = self.graph.nodes.len();
                let flow = self.statement(statement, parent, false);
                if self.graph.nodes.len() != before
                    && let Some(argument) = argument
                {
                    self.edge(
                        parent,
                        before as NodeId,
                        "ARGUMENT",
                        Some(argument.to_string()),
                    );
                }
                flow
            }
            ExprKind::Unknown(text) if text.is_empty() => Fragment::default(),
            ExprKind::Unknown(_) => {
                self.unsupported(span, "unsupported expression; CFG parity is unverified");
                let id = self.node("UNKNOWN", span, None, None);
                self.ast(parent, id, argument);
                Fragment::single(id)
            }
        }
    }
    fn block_is_container(&self, parent: NodeId) -> bool {
        let node = &self.graph.nodes[parent as usize];
        node.kind == "METHOD"
            || node.kind == "CONTROL_STRUCTURE"
            || matches!(
                node.name.as_deref(),
                Some("<operator>.conditional" | "<operator>.logicalAnd" | "<operator>.logicalOr")
            )
    }
    fn expression_block(
        &mut self,
        expressions: &[Expr],
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        let id = self.node("BLOCK", span, None, None);
        self.ast(parent, id, argument);
        let mut flow = Fragment::default();
        for expression in expressions {
            flow = flow.append(self.expression(expression, id, None));
        }
        if !self.block_is_container(parent) {
            flow = flow.append(Fragment::single(id));
        }
        flow
    }
    fn cast_type_code(&self, span: &Span) -> Option<&str> {
        let code = self.code(span);
        let (open, close, offset) = if code.starts_with('(') {
            ('(', ')', 0)
        } else if [
            "static_cast",
            "dynamic_cast",
            "reinterpret_cast",
            "const_cast",
        ]
        .iter()
        .any(|name| code.starts_with(name))
        {
            ('<', '>', code.find('<')?)
        } else {
            return None;
        };
        let mut depth = 0;
        for (position, ch) in code[offset..].char_indices() {
            if ch == open {
                depth += 1;
            }
            if ch == close {
                depth -= 1;
                if depth == 0 {
                    let mut type_code = code[offset + 1..offset + position].trim();
                    while let Some(rest) = type_code.strip_prefix("__extension__")
                        && rest.starts_with(char::is_whitespace)
                    {
                        type_code = rest.trim_start();
                    }
                    return Some(type_code);
                }
            }
        }
        None
    }
    fn resolve_call(&self, callee: &Expr, arguments: &[Expr]) -> Option<&Function> {
        let mut keys = Vec::new();
        match &callee.kind {
            ExprKind::Identifier(name) => {
                if self.cpp && !name.contains("::") && !self.method_scope.is_empty() {
                    keys.push(format!("{}.{}", self.method_scope, name));
                }
                keys.push(name.trim_start_matches("::").replace("::", "."));
            }
            ExprKind::Member { base, name, .. } if self.cpp => {
                let owner = self.expression_type(base)?;
                let owner = owner.trim_end_matches(['*', '&']).replace("::", ".");
                keys.push(format!("{owner}.{name}"));
            }
            _ => return None,
        }
        for key in keys {
            let Some(methods) = self.methods.get(&key) else {
                continue;
            };
            let mut identities = HashSet::new();
            let candidates: Vec<_> = methods
                .iter()
                .copied()
                .filter(|method| {
                    let variadic = method.parameters.last().is_some_and(|p| {
                        p.type_name == "..." || self.code(&p.span).trim() == "..."
                    });
                    let void_only = method.parameters.len() == 1
                        && method.parameters[0].name.is_empty()
                        && method.parameters[0].type_name == "void";
                    let arity = method.parameters.len() - usize::from(variadic || void_only);
                    (arguments.len() == arity || variadic && arguments.len() >= arity)
                        && identities.insert(method.full_name.as_str())
                })
                .collect();
            if candidates.len() == 1 {
                return Some(candidates[0]);
            }
            let mut typed = candidates.into_iter().filter(|method| {
                method
                    .parameters
                    .iter()
                    .zip(arguments)
                    .all(|(parameter, argument)| {
                        self.expression_type(argument)
                            .is_some_and(|ty| ty == parameter.type_name)
                    })
            });
            if let Some(method) = typed.next()
                && typed.next().is_none()
            {
                return Some(method);
            }
        }
        None
    }
    fn expression_type(&self, expression: &Expr) -> Option<String> {
        match &expression.kind {
            ExprKind::Identifier(name) => self.binding(name).map(|(_, ty)| ty.clone()),
            ExprKind::Literal(value) if value.starts_with('"') => Some("char*".into()),
            ExprKind::Literal(value) if value.starts_with('\'') => Some("char".into()),
            ExprKind::Literal(value) if value.chars().all(|c| c.is_ascii_digit()) => {
                Some("int".into())
            }
            ExprKind::Literal(value) if matches!(value.as_str(), "true" | "false") => {
                Some("bool".into())
            }
            ExprKind::Cast { type_name, .. } => Some(type_name.clone()),
            ExprKind::Bracketed(inner)
            | ExprKind::Generated {
                expression: inner, ..
            } => self.expression_type(inner),
            ExprKind::Unary { op, argument, .. } if op == "*" => self
                .expression_type(argument)
                .and_then(|ty| ty.strip_suffix('*').map(str::to_string)),
            ExprKind::Unary { op, argument, .. } if op == "&" => {
                self.expression_type(argument).map(|ty| format!("{ty}*"))
            }
            _ => None,
        }
    }
    fn qualified_owner(
        &mut self,
        names: &[&str],
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        let Some((&head, tail)) = names.split_first() else {
            return Fragment::default();
        };
        if tail.is_empty() {
            return Fragment::single(self.identifier(head, span, parent, argument));
        }
        let id = self.node(
            "CALL",
            span,
            Some("<operator>.fieldAccess"),
            Some(&names.join("::")),
        );
        self.ast(parent, id, argument);
        Fragment::single(self.identifier(head, span, id, Some(1)))
            .append(self.qualified_owner(tail, span, id, Some(2)))
            .append(Fragment::single(id))
    }
    fn macro_call(
        &mut self,
        expression: &Expr,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        let ExprKind::MacroCall {
            name,
            arguments,
            expansion,
            arity,
        } = &expression.kind
        else {
            unreachable!()
        };
        let span = &expression.span;
        let id = self.node("CALL", span, Some(name), None);
        self.graph.nodes[id as usize].method_full_name = Some(format!(":-1:-1:{name}:{arity}"));
        self.ast(parent, id, argument);
        let mut invocation = Fragment::default();
        for (position, argument) in arguments.iter().enumerate() {
            invocation = invocation.append(self.expression(argument, id, Some(position + 1)));
        }
        invocation = invocation.append(Fragment::single(id));
        let block = self.node("BLOCK", span, None, Some(""));
        self.graph.nodes[block as usize].type_name = Some("ANY".into());
        self.ast(id, block, None);
        let expansion = self.expression(expansion, block, Some(1));
        // INLINED calls preserve a bypass and an expanded-code path. Their
        // final AST child is an expansion block, excluded as a raw CFG vertex.
        let entry = invocation.entry;
        let mut fringe = invocation.fringe.clone();
        fringe.extend(&expansion.fringe);
        let target = expansion.entry;
        let mut flow = Fragment::combine([invocation, expansion]);
        if let Some(target) = target {
            flow.edges.push([id, target]);
        }
        flow.entry = entry;
        flow.fringe = fringe;
        flow
    }
    fn condition(&mut self, expression: &Expr, parent: NodeId, _wrap: bool) -> Fragment {
        let before = self.graph.nodes.len();
        let flow = self.condition_expression(expression, parent);
        if self.graph.nodes.len() > before {
            self.edge(parent, before as NodeId, "CONDITION", None);
        }
        flow
    }
    fn condition_expression(&mut self, expression: &Expr, parent: NodeId) -> Fragment {
        let before = self.graph.nodes.len();
        if let ExprKind::List(expressions) = &expression.kind {
            let flow = self.expression_block(expressions, &expression.span, parent, None);
            self.graph.nodes[before].code.clear();
            flow
        } else {
            self.expression(expression, parent, None)
        }
    }
    fn branch(&mut self, statement: &Stmt, parent: NodeId) -> Fragment {
        if matches!(statement.kind, StmtKind::Block(_)) {
            return self.statement(statement, parent, false);
        }
        let block = self.node("BLOCK", &statement.span, None, Some(""));
        self.ast(parent, block, None);
        self.scopes.push(HashMap::new());
        let flow = self.statement(statement, block, false);
        self.scopes.pop();
        flow
    }
    fn declarations(
        &mut self,
        declarations: &[Declaration],
        span: &Span,
        parent: NodeId,
    ) -> Fragment {
        self.declaration_parts(declarations, span, parent)
            .into_iter()
            .fold(Fragment::default(), Fragment::append)
    }
    fn declaration_identifier(&mut self, declaration: &Declaration, parent: NodeId) -> NodeId {
        if declaration.nested_declarator {
            // LOCAL uses the nested name; initializer lowering asks the outer
            // declarator for its name, which CDT leaves empty in T(x)=value.
            let id = self.node("IDENTIFIER", &Span::default(), Some(""), Some(""));
            self.graph.nodes[id as usize].type_name = Some("ANY".into());
            self.ast(parent, id, Some(1));
            return id;
        }
        if !declaration.is_typedef {
            return self.identifier(&declaration.name, &declaration.span, parent, Some(1));
        }
        let id = self.node(
            "IDENTIFIER",
            &declaration.span,
            Some(&declaration.name),
            Some(&declaration.name),
        );
        self.graph.nodes[id as usize].type_name = Some(declaration.name.clone());
        self.ast(parent, id, Some(1));
        id
    }
    fn declaration_parts(
        &mut self,
        declarations: &[Declaration],
        span: &Span,
        parent: NodeId,
    ) -> Vec<Fragment> {
        let mut parts = Vec::new();
        for declaration in declarations {
            if declaration.problem {
                let unknown = self.node("UNKNOWN", &declaration.span, None, None);
                self.ast(parent, unknown, None);
                parts.push(Fragment::single(unknown));
                continue;
            }
            let code = if declaration.is_typedef {
                self.code(span).to_string()
            } else {
                format!("{} {}", declaration.type_name, declaration.name)
            };
            let local = self.node(
                if declaration.is_typedef {
                    "TYPE_DECL"
                } else {
                    "LOCAL"
                },
                &declaration.span,
                Some(&declaration.name),
                Some(&code),
            );
            if !declaration.is_typedef {
                self.graph.nodes[local as usize].type_name = Some(declaration.type_name.clone());
                self.scopes.last_mut().unwrap().insert(
                    declaration.name.clone(),
                    (local, declaration.type_name.clone()),
                );
            }
            self.ast(parent, local, None);
            parts.push(Fragment::default());
        }
        // CDT emits every local first, then every array allocation, followed
        // by initializers. An initialized VLA still evaluates its bounds.
        for declaration in declarations {
            if !declaration.problem && !declaration.dimensions.is_empty() {
                let assignment = self.node(
                    "CALL",
                    &declaration.span,
                    Some("<operator>.assignment"),
                    None,
                );
                self.graph.nodes[assignment as usize].type_name =
                    Some(declaration.type_name.clone());
                self.ast(parent, assignment, None);
                let lhs = self.declaration_identifier(declaration, assignment);
                let allocation =
                    self.node("CALL", &declaration.span, Some("<operator>.alloc"), None);
                self.graph.nodes[allocation as usize].type_name =
                    Some(declaration.type_name.clone());
                self.ast(assignment, allocation, Some(2));
                let mut bounds = Fragment::single(lhs);
                for (index, expression) in declaration.dimensions.iter().enumerate() {
                    bounds =
                        bounds.append(self.expression(expression, allocation, Some(index + 1)));
                }
                parts.push(
                    bounds
                        .append(Fragment::single(allocation))
                        .append(Fragment::single(assignment)),
                );
            }
        }
        for declaration in declarations {
            if declaration.problem {
                continue;
            }
            if let Some(initializer) = &declaration.initializer {
                if let Some((signature, return_type)) = lambda_binding(initializer)
                    && let Some((id, _)) = self.binding(&declaration.name)
                {
                    self.lambda_bindings
                        .insert(*id, (signature.to_string(), return_type.to_string()));
                }
                let code = self.code(&declaration.span).to_string();
                let id = self.node(
                    "CALL",
                    &declaration.span,
                    Some("<operator>.assignment"),
                    Some(&code),
                );
                self.ast(parent, id, None);
                let lhs = self.declaration_identifier(declaration, id);
                let rhs = self.expression(initializer, id, Some(2));
                parts.push(
                    Fragment::single(lhs)
                        .append(rhs)
                        .append(Fragment::single(id)),
                );
            }
        }
        parts
    }
    fn statement_parts(&mut self, statement: &Stmt, parent: NodeId) -> Vec<Fragment> {
        // CDT labels and declarations expand into sibling AST roots. Control
        // flow selects those roots by order rather than by source statement.
        match &statement.kind {
            StmtKind::Sequence(statements) => statements
                .iter()
                .flat_map(|statement| self.statement_parts(statement, parent))
                .collect(),
            StmtKind::Label {
                name,
                statement: following,
            } => {
                let id = self.node("JUMP_TARGET", &statement.span, Some(name), Some(name));
                self.ast(parent, id, None);
                let mut label = Fragment::single(id);
                label.labels.insert(name.clone(), id);
                let mut parts = vec![label];
                parts.extend(self.statement_parts(following, parent));
                parts
            }
            StmtKind::Case(expression) => {
                let name = expression
                    .as_ref()
                    .map(|value| format!("case {}", self.code(&value.span)))
                    .unwrap_or_else(|| "default".into());
                let id = self.node("JUMP_TARGET", &statement.span, Some(&name), Some(&name));
                self.ast(parent, id, None);
                let mut label = Fragment::single(id);
                label.cases.push((id, expression.is_none()));
                let mut parts = vec![label];
                if let Some(expression) = expression {
                    parts.push(self.condition_expression(expression, parent));
                }
                parts
            }
            StmtKind::Declaration(declarations) => {
                self.declaration_parts(declarations, &statement.span, parent)
            }
            StmtKind::Empty | StmtKind::Problem => Vec::new(),
            _ => vec![self.statement(statement, parent, false)],
        }
    }
    fn statement(&mut self, statement: &Stmt, parent: NodeId, _try_return: bool) -> Fragment {
        match &statement.kind {
            StmtKind::Sequence(statements) => {
                let mut flow = Fragment::default();
                for statement in statements {
                    flow = flow.append(self.statement(statement, parent, false));
                }
                flow
            }
            StmtKind::Block(statements) => {
                let code = self.code(&statement.span);
                let empty = code == "{}" || code.is_empty();
                let id = self.node("BLOCK", &statement.span, None, empty.then_some(""));
                self.ast(parent, id, None);
                self.scopes.push(HashMap::new());
                let mut flow = Fragment::default();
                for statement in statements {
                    flow = flow.append(self.statement(statement, id, false));
                }
                self.scopes.pop();
                if !self.block_is_container(parent) {
                    flow = flow.append(Fragment::single(id));
                }
                flow
            }
            StmtKind::Expression(expression) => self.expression(expression, parent, None),
            StmtKind::Declaration(declarations) => {
                self.declarations(declarations, &statement.span, parent)
            }
            StmtKind::Empty | StmtKind::Problem => Fragment::default(),
            StmtKind::Return(expression) => {
                let id = self.node("RETURN", &statement.span, None, None);
                self.ast(parent, id, None);
                let flow = expression
                    .as_ref()
                    .map(|expression| self.expression(expression, id, Some(1)))
                    .unwrap_or_default();
                let fringe = if self.try_depth != 0 {
                    flow.fringe.clone()
                } else {
                    Vec::new()
                };
                flow.append(Fragment {
                    entry: Some(id),
                    edges: vec![[id, 1]],
                    fringe,
                    ..Fragment::default()
                })
            }
            StmtKind::Break | StmtKind::Continue => {
                let is_break = matches!(statement.kind, StmtKind::Break);
                let id = self.control(
                    statement,
                    parent,
                    if is_break { "BREAK" } else { "CONTINUE" },
                );
                let mut flow = Fragment {
                    entry: Some(id),
                    ..Fragment::default()
                };
                if is_break {
                    flow.breaks.push(id);
                } else {
                    flow.continues.push(id);
                }
                flow
            }
            StmtKind::Goto(target) => {
                let id = self.control(statement, parent, "GOTO");
                Fragment {
                    entry: Some(id),
                    jumps: vec![(id, target.clone())],
                    ..Fragment::default()
                }
            }
            StmtKind::Label { .. } | StmtKind::Case(_) => self
                .statement_parts(statement, parent)
                .into_iter()
                .fold(Fragment::default(), Fragment::append),
            StmtKind::If {
                condition,
                consequence,
                alternative,
            } => {
                let id = self.control(statement, parent, "IF");
                self.scopes.push(HashMap::new());
                let condition = self.condition(condition, id, true);
                let yes = self.branch(consequence, id);
                let no = alternative
                    .as_ref()
                    .map(|alternative| {
                        let else_id = self.node(
                            "CONTROL_STRUCTURE",
                            &alternative.span,
                            Some("ELSE"),
                            Some("else"),
                        );
                        self.ast(id, else_id, None);
                        self.branch(alternative, else_id)
                    })
                    .unwrap_or_default();
                let entry = condition.entry;
                let mut fringe = if yes.entry.is_some() {
                    yes.fringe.clone()
                } else {
                    condition.fringe.clone()
                };
                fringe.extend(if no.entry.is_some() {
                    &no.fringe
                } else {
                    &condition.fringe
                });
                let mut links = Vec::new();
                for target in [yes.entry, no.entry].into_iter().flatten() {
                    links.extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                let mut flow = Fragment::combine([condition, yes, no]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                self.scopes.pop();
                flow
            }
            StmtKind::While { condition, body } => {
                let id = self.control(statement, parent, "WHILE");
                self.scopes.push(HashMap::new());
                let condition = self.condition(condition, id, true);
                let mut parts = self.statement_parts(body, id).into_iter();
                let body = parts.next().unwrap_or_default();
                let alternative = parts.next().unwrap_or_default();
                let entry = condition.entry;
                let mut fringe = condition.fringe.clone();
                fringe.extend(&body.breaks);
                fringe.extend(&alternative.fringe);
                let mut links = Vec::new();
                if let Some(target) = body.entry {
                    links.extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                if let Some(target) = alternative.entry {
                    links.extend(body.fringe.iter().map(|&source| [source, target]));
                }
                if let Some(target) = entry {
                    links.extend(
                        body.fringe
                            .iter()
                            .chain(&body.continues)
                            .map(|&source| [source, target]),
                    );
                }
                let mut flow = Fragment::combine([condition, body, alternative]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                flow.breaks.clear();
                flow.continues.clear();
                self.scopes.pop();
                flow
            }
            StmtKind::DoWhile { body, condition } => {
                let id = self.control(statement, parent, "DO");
                let body = self
                    .statement_parts(body, id)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                let condition = self.condition(condition, id, true);
                let entry = body.entry.or(condition.entry);
                let mut fringe = condition.fringe.clone();
                fringe.extend(&body.breaks);
                let mut links = Vec::new();
                if let Some(target) = condition.entry {
                    links.extend(
                        body.fringe
                            .iter()
                            .chain(&body.continues)
                            .map(|&source| [source, target]),
                    );
                }
                if let Some(target) = entry {
                    links.extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                let mut flow = Fragment::combine([body, condition]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                flow.breaks.clear();
                flow.continues.clear();
                flow
            }
            StmtKind::For {
                initializer,
                condition,
                update,
                body,
            } => {
                let id = self.control(statement, parent, "FOR");
                self.scopes.push(HashMap::new());
                let initial_block = self.node("BLOCK", &statement.span, None, Some(""));
                self.ast(id, initial_block, None);
                let initial = initializer
                    .as_ref()
                    .map(|initial| self.statement(initial, initial_block, false))
                    .unwrap_or_default();
                let mut slots = vec![initial];
                if let Some(condition) = condition {
                    let before = self.graph.nodes.len();
                    let flow = self.condition(condition, id, true);
                    if self.graph.nodes.len() != before {
                        slots.push(flow);
                    }
                }
                if let Some(update) = update {
                    let before = self.graph.nodes.len();
                    let flow = self.expression(update, id, None);
                    if self.graph.nodes.len() != before {
                        slots.push(flow);
                    }
                }
                let first_body_edge = self.graph.edges.len();
                let parts = self.statement_parts(body, id);
                let locals = self.graph.edges[first_body_edge..]
                    .iter()
                    .filter(|edge| {
                        edge.source == id
                            && edge.kind == "AST"
                            && self.graph.nodes[edge.target as usize].kind == "LOCAL"
                    })
                    .count();
                // Joern 4.0.150 selects FOR children by `order`, rather than
                // CONDITION edges. Empty expression ASTs consume no order.
                // A compound body keeps explicit order 4; other bodies shift.
                if matches!(body.kind, StmtKind::Block(_)) {
                    while slots.len() < 3 {
                        slots.push(Fragment::default());
                    }
                }
                slots.extend(parts);
                let mut slots = slots.into_iter().skip(locals);
                let initial = slots.next().unwrap_or_default();
                let condition = slots.next().unwrap_or_default();
                let update = slots.next().unwrap_or_default();
                let body = slots.next().unwrap_or_default();
                let breaks = body.breaks.clone();
                let continues = body.continues.clone();
                let update_entry = update.entry;
                let inner = body.append(update);
                let loop_entry = condition.entry.or(inner.entry);
                let entry = initial.entry.or(loop_entry);
                let mut fringe = condition.fringe.clone();
                fringe.extend(breaks);
                let mut links = Vec::new();
                if let Some(target) = loop_entry {
                    links.extend(
                        initial
                            .fringe
                            .iter()
                            .chain(&inner.fringe)
                            .map(|&source| [source, target]),
                    );
                }
                if let Some(target) = inner.entry.or(condition.entry) {
                    links.extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                if let Some(target) = update_entry.or(loop_entry) {
                    links.extend(continues.iter().map(|&source| [source, target]));
                }
                let mut flow = Fragment::combine([initial, condition, inner]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                flow.breaks.clear();
                flow.continues.clear();
                self.scopes.pop();
                flow
            }
            StmtKind::RangeFor {
                declaration,
                iterable,
                body,
            } => {
                let id = self.control(statement, parent, "FOR");
                self.scopes.push(HashMap::new());
                self.expression(iterable, id, None);
                self.declarations(
                    &[Declaration {
                        initializer: None,
                        dimensions: Vec::new(),
                        ..declaration.clone()
                    }],
                    &declaration.span,
                    id,
                );
                // Joern 4.0.150 selects for children by local count and order.
                // Its range-for body occupies the condition slot, while the
                // initializer is never evaluated. Preserve the DecBench oracle.
                let mut condition = self.statement(body, id, false);
                if let Some(target) = condition.entry {
                    condition
                        .edges
                        .extend(condition.fringe.iter().map(|&source| [source, target]));
                }
                condition.breaks.clear();
                condition.continues.clear();
                self.scopes.pop();
                condition
            }
            StmtKind::Switch { condition, body } => {
                let id = self.control(statement, parent, "SWITCH");
                let condition = self.condition(condition, id, false);
                let body = self
                    .statement_parts(body, id)
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                let entry = condition.entry;
                let mut fringe = if body.cases.iter().any(|(_, default)| *default) {
                    Vec::new()
                } else {
                    condition.fringe.clone()
                };
                fringe.extend(&body.breaks);
                fringe.extend(&body.fringe);
                let links: Vec<_> = condition
                    .fringe
                    .iter()
                    .flat_map(|&source| body.cases.iter().map(move |&(target, _)| [source, target]))
                    .collect();
                let mut flow = Fragment::combine([condition, body]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                flow.cases.clear();
                flow.breaks.clear();
                flow
            }
            StmtKind::Try { body, catches } => {
                let id = self.control(statement, parent, "TRY");
                self.try_depth += 1;
                let body_root = self.graph.nodes.len() as NodeId;
                let try_flow = self.statement(body, id, false);
                let body_has_children = self
                    .graph
                    .edges
                    .iter()
                    .any(|edge| edge.kind == "AST" && edge.source == body_root);
                let mut parts = Vec::new();
                let mut links = Vec::new();
                let mut fringe = try_flow.fringe.clone();
                let entry = try_flow.entry;
                for catch in catches {
                    let catch_id = self.control(catch, id, "CATCH");
                    let catch_flow = self.statement(catch, catch_id, false);
                    if let Some(target) = catch_flow.entry {
                        links.extend(try_flow.fringe.iter().map(|&source| [source, target]));
                    }
                    fringe.extend(&catch_flow.fringe);
                    parts.push(catch_flow);
                }
                self.try_depth -= 1;
                parts.insert(0, try_flow);
                if !body_has_children {
                    return Fragment::default();
                }
                let mut flow = Fragment::combine(parts);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                flow
            }
            StmtKind::Throw(expression) => {
                let id = self.control(statement, parent, "THROW");
                let flow = expression
                    .as_ref()
                    .map(|expression| self.expression(expression, id, Some(1)))
                    .unwrap_or_default();
                flow.append(Fragment {
                    entry: Some(id),
                    edges: vec![[id, 1]],
                    ..Fragment::default()
                })
            }
            StmtKind::Unknown(expressions) => {
                self.unsupported(
                    &statement.span,
                    "unsupported statement; CFG parity is unverified",
                );
                let id = self.node("UNKNOWN", &statement.span, None, None);
                self.ast(parent, id, None);
                let mut flow = Fragment::default();
                for expression in expressions {
                    flow = flow.append(self.expression(expression, id, None));
                }
                flow.append(Fragment::single(id))
            }
        }
    }
}

fn is_expression_list(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Generated { expression, .. } => is_expression_list(expression),
        ExprKind::List(_) => true,
        _ => false,
    }
}

fn function_pointer_parameter(fullname: &str, argument: usize) -> Option<String> {
    let (_, signature) = fullname.rsplit_once(':')?;
    let signature = signature.strip_suffix(')')?;
    let parameters = &signature[signature.find('(')? + 1..];
    let mut depth = 0;
    let mut start = 0;
    let mut index = 1;
    for (end, ch) in parameters
        .char_indices()
        .chain(std::iter::once((parameters.len(), ',')))
    {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            ',' if depth == 0 => {
                if index == argument {
                    let parameter = parameters[start..end].trim();
                    for pointer in ["(*)", "(&)", "(&&)"] {
                        if let Some(position) = parameter.find(pointer) {
                            return Some(format!(
                                "{}{}",
                                &parameter[..position],
                                &parameter[position + pointer.len()..]
                            ));
                        }
                    }
                    return None;
                }
                start = end + 1;
                index += 1;
            }
            _ => (),
        }
    }
    None
}

fn lambda_binding(expression: &Expr) -> Option<(&str, &str)> {
    match &expression.kind {
        ExprKind::Lambda {
            signature,
            return_type,
            ..
        } => Some((signature, return_type)),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => lambda_binding(inner),
        _ => None,
    }
}

fn unary_operator(operator: &str, postfix: bool) -> &str {
    match operator {
        "++" if postfix => "<operator>.postIncrement",
        "--" if postfix => "<operator>.postDecrement",
        "++" => "<operator>.preIncrement",
        "--" => "<operator>.preDecrement",
        "+" => "<operator>.plus",
        "-" => "<operator>.minus",
        "!" => "<operator>.logicalNot",
        "~" => "<operator>.not",
        "*" => "<operator>.indirection",
        "&" => "<operator>.addressOf",
        "sizeof" | "alignof" | "_Alignof" | "__alignof__" | "typeof" | "__typeof__" | "typeid" => {
            "<operator>.sizeOf"
        }
        "new" => "<operator>.new",
        "delete" | "delete[]" => "<operator>.delete",
        _ => "<operator>.unknown",
    }
}

fn binary_operator(operator: &str) -> &str {
    match operator {
        "=" => "<operator>.assignment",
        "+=" => "<operator>.assignmentPlus",
        "-=" => "<operator>.assignmentMinus",
        "*=" => "<operator>.assignmentMultiplication",
        "/=" => "<operator>.assignmentDivision",
        "%=" => "<operator>.assignmentModulo",
        "&=" => "<operator>.assignmentAnd",
        "|=" => "<operator>.assignmentOr",
        "^=" => "<operator>.assignmentXor",
        "<<=" => "<operator>.assignmentShiftLeft",
        ">>=" => "<operator>.assignmentArithmeticShiftRight",
        "+" => "<operator>.addition",
        "-" => "<operator>.subtraction",
        "*" => "<operator>.multiplication",
        "/" => "<operator>.division",
        "%" => "<operator>.modulo",
        "&" => "<operator>.and",
        "|" => "<operator>.or",
        "^" => "<operator>.xor",
        "&&" => "<operator>.logicalAnd",
        "||" => "<operator>.logicalOr",
        "<<" => "<operator>.shiftLeft",
        ">>" => "<operator>.arithmeticShiftRight",
        "==" => "<operator>.equals",
        "!=" => "<operator>.notEquals",
        "<" => "<operator>.lessThan",
        ">" => "<operator>.greaterThan",
        "<=" => "<operator>.lessEqualsThan",
        ">=" => "<operator>.greaterEqualsThan",
        _ => "<operator>.unknown",
    }
}

#[cfg(test)]
mod tests {
    use crate::{analyze, graph::Options};
    use serde_json::Value;
    use std::collections::HashMap;

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
                expected[edge[0].as_u64().unwrap() as usize][edge[1].as_u64().unwrap() as usize] =
                    true;
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
                    actual[source][a] != expected[target][b]
                        || actual[a][source] != expected[b][target]
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
            include_str!("../tests/fixtures/decbench-regressions/missing_goto_target.c"),
            include_str!("../tests/fixtures/decbench-regressions/missing_goto_target.pyjoern.json"),
        );
    }

    #[test]
    fn terminal_labels_follow_cdt_recovery() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/terminal_label.c"),
            include_str!("../tests/fixtures/decbench-regressions/terminal_label.pyjoern.json"),
        );
    }

    #[test]
    fn inline_assembly_has_a_visible_unknown_node_without_parse_failure() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/gnu_inline_asm.c"),
            include_str!("../tests/fixtures/decbench-regressions/gnu_inline_asm.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/microsoft_asm_block.c"),
            include_str!("../tests/fixtures/decbench-regressions/microsoft_asm_block.pyjoern.json"),
        );
    }

    #[test]
    fn nested_designators_preserve_joerns_initializer_flow() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/nested_designator.c"),
            include_str!("../tests/fixtures/decbench-regressions/nested_designator.pyjoern.json"),
        );
    }

    #[test]
    fn control_and_expression_graphs_match_original_joern() {
        assert_oracle(
            include_str!("../tests/fixtures/control.c"),
            include_str!("../tests/fixtures/control.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/expressions.c"),
            include_str!("../tests/fixtures/expressions.pyjoern.json"),
        );
    }

    #[test]
    fn omitted_for_slots_follow_joerns_ast_child_orders() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/for_slots.c"),
            include_str!("../tests/fixtures/decbench-regressions/for_slots.pyjoern.json"),
        );
    }

    #[test]
    fn scalar_labels_follow_joerns_control_child_selection() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/scalar_labels_c.c"),
            include_str!("../tests/fixtures/decbench-regressions/scalar_labels_c.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/scalar_labels_cpp.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/scalar_labels_cpp.pyjoern.json"),
        );
    }

    #[test]
    fn scalar_for_labels_follow_joerns_sibling_slot_selection() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/scalar_for_labels_c.c"),
            include_str!("../tests/fixtures/decbench-regressions/scalar_for_labels_c.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/scalar_for_labels_cpp.cpp"),
            include_str!(
                "../tests/fixtures/decbench-regressions/scalar_for_labels_cpp.pyjoern.json"
            ),
        );
    }

    #[test]
    fn typedef_array_bounds_preserve_original_allocations() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/typedef_vla_bounds_c.c"),
            include_str!(
                "../tests/fixtures/decbench-regressions/typedef_vla_bounds_c.pyjoern.json"
            ),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/typedef_vla_bounds_cpp.cpp"),
            include_str!(
                "../tests/fixtures/decbench-regressions/typedef_vla_bounds_cpp.pyjoern.json"
            ),
        );
    }

    #[test]
    fn condition_lists_and_type_specifiers_match_original_projection() {
        assert_oracle_with_filename(
            include_str!("../tests/fixtures/decbench-regressions/control_projection_c.c"),
            include_str!(
                "../tests/fixtures/decbench-regressions/control_projection_c.pyjoern.json"
            ),
            "input.c",
        );
        assert_oracle_with_filename(
            include_str!("../tests/fixtures/decbench-regressions/control_projection_cpp.cpp"),
            include_str!(
                "../tests/fixtures/decbench-regressions/control_projection_cpp.pyjoern.json"
            ),
            "input.cpp",
        );
    }

    #[test]
    fn multiline_method_refs_follow_original_dot_lifting() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/method_ref_lines.c"),
            include_str!("../tests/fixtures/decbench-regressions/method_ref_lines.pyjoern.json"),
        );
    }

    #[test]
    fn returns_under_nested_try_ancestors_keep_expression_fringe() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/nested_try.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/nested_try.pyjoern.json"),
        );
    }

    #[test]
    fn cpp_qualified_names_follow_joerns_field_access_ast() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/qualified_names.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/qualified_names.pyjoern.json"),
        );
    }

    #[test]
    fn method_references_preserve_unique_and_contextual_bindings() {
        for (source, oracle, filename) in [
            (
                include_str!("../tests/fixtures/decbench-regressions/method_ref_provenance_c.c"),
                include_str!(
                    "../tests/fixtures/decbench-regressions/method_ref_provenance_c.properties.json"
                ),
                "input.c",
            ),
            (
                include_str!(
                    "../tests/fixtures/decbench-regressions/method_ref_provenance_cpp.cpp"
                ),
                include_str!(
                    "../tests/fixtures/decbench-regressions/method_ref_provenance_cpp.properties.json"
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
                    .find(|n| {
                        n.kind == "METHOD_REF" && Some(n.code.as_str()) == old["code"].as_str()
                    })
                    .unwrap();
                assert_eq!(
                    node.method_full_name.as_deref(),
                    old["method_full_name"].as_str()
                );
                assert_eq!(node.type_name.as_deref(), old["type_name"].as_str());
                let target = old["refs"][0]["fullname"].as_str().unwrap();
                if target == function.fullname {
                    assert!(function.cpg.edges.iter().any(|edge| edge.kind == "REF"
                        && edge.source == node.id
                        && edge.target == 0));
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
            include_str!("../tests/fixtures/decbench-regressions/grouped_declarator_control_c.c"),
            include_str!(
                "../tests/fixtures/decbench-regressions/grouped_declarator_control_c.pyjoern.json"
            ),
        );
        assert_oracle(
            include_str!(
                "../tests/fixtures/decbench-regressions/grouped_declarator_control_cpp.cpp"
            ),
            include_str!(
                "../tests/fixtures/decbench-regressions/grouped_declarator_control_cpp.pyjoern.json"
            ),
        );
    }

    #[test]
    fn array_bounds_are_allocated_before_initializers() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/local_arrays_c.c"),
            include_str!("../tests/fixtures/decbench-regressions/local_arrays_c.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/local_arrays_cpp.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/local_arrays_cpp.pyjoern.json"),
        );
    }

    #[test]
    fn empty_try_body_preserves_original_entry_roles() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/empty_try.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/empty_try.pyjoern.json"),
        );
    }

    #[test]
    fn local_function_pointer_declarations_follow_cdt_recovery() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/local_function_pointer_c.c"),
            include_str!(
                "../tests/fixtures/decbench-regressions/local_function_pointer_c.pyjoern.json"
            ),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/local_function_pointer_cpp.cpp"),
            include_str!(
                "../tests/fixtures/decbench-regressions/local_function_pointer_cpp.pyjoern.json"
            ),
        );
    }

    #[test]
    fn builtin_macros_preserve_inlined_expansion_paths() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/gnu_builtins_c.c"),
            include_str!("../tests/fixtures/decbench-regressions/gnu_builtins_c.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/gnu_builtins_cpp.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/gnu_builtins_cpp.pyjoern.json"),
        );
    }

    #[test]
    fn cdt_object_and_function_macros_keep_original_control_flow() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/cdt_builtin_controls_c.c"),
            include_str!(
                "../tests/fixtures/decbench-regressions/cdt_builtin_controls_c.pyjoern.json"
            ),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/cdt_builtin_controls_cpp.cpp"),
            include_str!(
                "../tests/fixtures/decbench-regressions/cdt_builtin_controls_cpp.pyjoern.json"
            ),
        );
    }

    #[test]
    fn comma_statement_expressions_preserve_visible_joins() {
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/gnu_assertions_c.c"),
            include_str!("../tests/fixtures/decbench-regressions/gnu_assertions_c.pyjoern.json"),
        );
        assert_oracle(
            include_str!("../tests/fixtures/decbench-regressions/gnu_assertions_cpp.cpp"),
            include_str!("../tests/fixtures/decbench-regressions/gnu_assertions_cpp.pyjoern.json"),
        );
    }
}
