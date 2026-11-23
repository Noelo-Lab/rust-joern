//! Expression and call lowering.

use super::*;

impl Builder<'_> {
    pub(super) fn unsupported(&mut self, span: &Span, message: &str) {
        self.diagnostics.push(Diagnostic {
            filename: self.filename.into(),
            line: span.line,
            column: span.column,
            severity: "error".into(),
            message: message.into(),
        });
    }

    pub(super) fn expression(
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
            ExprKind::Identifier(name)
                if self.cpp
                    && self.binding(name).is_none()
                    && !self.inherited_bindings.contains_key(name)
                    && self.implicit_fields.contains_key(name)
                    && self.implicit_field_scope_matches() =>
            {
                let id = self.node(
                    "CALL",
                    span,
                    Some("<operator>.indirectFieldAccess"),
                    Some(&format!("this->{name}")),
                );
                self.ast(parent, id, argument);
                let this = self.identifier("this", span, id, Some(1));
                // Type recovery gives the generated this expression a pointer
                // to its owner, while the implicit parameter keeps owner type.
                let owner = self.this_owner();
                self.graph.nodes[this as usize].type_name =
                    owner.map(|ty| format!("{}*", ty.trim_end_matches('*')));
                let field = self.node("FIELD_IDENTIFIER", span, Some(name), Some(name));
                self.ast(id, field, Some(2));
                Fragment::single(this)
                    .append(Fragment::single(field))
                    .append(Fragment::single(id))
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
            ExprKind::Lambda(closure) => {
                let id = self.node("METHOD_REF", span, None, None);
                self.graph.nodes[id as usize].method_full_name = Some(closure.full_name.clone());
                self.graph.nodes[id as usize].type_name = Some(closure.full_name.clone());
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
                let name = if type_id_operand(operand) {
                    "<operator>.sizeOf"
                } else {
                    unary_operator(op, *postfix)
                };
                let id = self.node("CALL", span, Some(name), None);
                self.ast(parent, id, argument);
                let operand_flow = self.expression(operand, id, Some(1));
                if self.cpp && op == "noexcept" && !self.joern_cpp_control_flow {
                    // The operand is still represented in the AST, but C++
                    // does not evaluate it and its effects cannot enter the CFG.
                    Fragment::single(id)
                } else {
                    operand_flow.append(Fragment::single(id))
                }
            }
            ExprKind::Binary { .. } => self.binary_expression(expression, parent, argument),
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
                let closure = self.closure_for_expression(callee).cloned();
                let member_pointer = self.cpp && member_pointer_expression(callee);
                let pointer = closure.is_none()
                    && !member_pointer
                    && match &callee.kind {
                        ExprKind::Identifier(name) => self.binding(name).is_some_and(|(id, ty)| {
                            ty.contains('*') || self.function_pointers.contains(id)
                        }),
                        ExprKind::Member { .. } => !self.cpp,
                        _ => !self.cpp || self.pointer_call_expression(callee),
                    };
                let unresolved_operator = self.cpp
                    && closure.is_none()
                    && !pointer
                    && !matches!(
                        callee.kind,
                        ExprKind::Identifier(_) | ExprKind::Member { .. }
                    );
                let name = if closure.is_some() || unresolved_operator {
                    "<operator>()"
                } else if pointer {
                    "<operator>.pointerCall"
                } else {
                    match &callee.kind {
                        ExprKind::Identifier(name) if self.cpp => cpp_call_leaf(name),
                        ExprKind::Identifier(name) => name.as_str(),
                        ExprKind::Member { name, .. } if self.cpp => cpp_call_leaf(name),
                        ExprKind::Member { name, .. } => name.as_str(),
                        _ => "<operator>.pointerCall",
                    }
                };
                let resolved = (!pointer && closure.is_none() && !unresolved_operator)
                    .then(|| self.resolve_call(callee, arguments))
                    .flatten()
                    .map(|method| {
                        (
                            method.full_name.clone(),
                            if self.cpp {
                                method.binding_return_type.clone()
                            } else {
                                method.return_type.clone()
                            },
                            method.is_virtual,
                        )
                    });
                let kind = if pointer {
                    CallKind::Pointer
                } else if closure.is_some() {
                    CallKind::Closure
                } else if unresolved_operator {
                    CallKind::UnresolvedExpression
                } else if self.cpp && matches!(callee.kind, ExprKind::Member { .. }) {
                    match &resolved {
                        Some((_, _, is_virtual)) => CallKind::ResolvedMember {
                            is_virtual: *is_virtual,
                        },
                        None if self.member_binding_is_opaque(callee) => CallKind::OpaqueMember,
                        None => CallKind::UnresolvedMember,
                    }
                } else {
                    CallKind::Named
                };
                let layout = CallLayout::for_kind(kind);
                let id = self.node("CALL", span, Some(name), None);
                if let Some(closure) = &closure {
                    self.graph.nodes[id as usize].method_full_name = Some(format!(
                        "<operator>():{}({})",
                        closure.return_type,
                        closure.parameter_types.join(","),
                    ));
                    self.graph.nodes[id as usize].type_name = Some(closure.return_type.clone());
                } else if let Some((fullname, return_type, _)) = resolved {
                    self.graph.nodes[id as usize].method_full_name = Some(fullname);
                    self.graph.nodes[id as usize].type_name = Some(return_type);
                } else if self.cpp
                    && !pointer
                    && kind != CallKind::OpaqueMember
                    && self.graph.nodes[id as usize].method_full_name.is_none()
                {
                    // Our parser also lowers CDT's distinct new-expression
                    // class to a synthetic Call. Its canonical intrinsic
                    // identity is already supplied by node(); it is not an
                    // unresolved source-level function call.
                    self.graph.nodes[id as usize].method_full_name =
                        Some(unresolved_full_name(name, arguments.len()));
                }
                self.ast(parent, id, argument);
                // createCallAst shares a base/receiver AST: lower the selected
                // expression exactly once, and attach both edges to its root.
                // Root identity comes from the AST edge, not allocation order.
                let operand = match layout.operand {
                    Operand::None => None,
                    Operand::Callee => Some(callee.as_ref()),
                    Operand::MemberBase => match &callee.kind {
                        ExprKind::Member { base, .. } => Some(base.as_ref()),
                        _ => unreachable!("member layout requires a member callee"),
                    },
                };
                let mut flow = Fragment::default();
                if let Some(operand) = operand {
                    let start = self.graph.edges.len();
                    flow = self.expression(operand, id, layout.base_index());
                    if layout.receiver {
                        let mut roots = self.graph.edges[start..]
                            .iter()
                            .filter(|edge| edge.kind == "AST" && edge.source == id)
                            .map(|edge| edge.target);
                        let root = roots.next();
                        if root.is_none() || roots.next().is_some() {
                            self.unsupported(span, "call receiver has no unique AST root");
                        } else if let Some(root) = root {
                            self.edge(id, root, "RECEIVER", None);
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
                let mut type_span = span.clone();
                let raw = self.code(span);
                let prefix = if raw.starts_with('(') {
                    Some(1)
                } else if [
                    "static_cast",
                    "reinterpret_cast",
                    "const_cast",
                    "dynamic_cast",
                ]
                .iter()
                .any(|cast| raw.starts_with(cast))
                {
                    raw.find('<').map(|index| index + 1)
                } else {
                    None
                };
                if let Some(prefix) = prefix {
                    let offset = prefix + raw[prefix..].len() - raw[prefix..].trim_start().len();
                    type_span.start += offset;
                    type_span.line += raw[..offset].bytes().filter(|&byte| byte == b'\n').count();
                    type_span.column = raw[..offset]
                        .rfind('\n')
                        .map_or(span.column + offset, |newline| offset - newline);
                }
                let type_code = self.cast_type_code(span).unwrap_or(type_name).to_string();
                let ty = self.node("UNKNOWN", &type_span, None, Some(&type_code));
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

    pub(super) fn binary_expression(
        &mut self,
        expression: &Expr,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        enum Task<'a> {
            Lower(&'a Expr, NodeId, Option<usize>),
            Join(NodeId, bool),
        }

        // Left-associated expressions can be thousands of nodes deep in
        // decompiler output. Keep the recursive conversion's preorder node
        // creation and postorder CFG joins without consuming the worker stack.
        let mut tasks = vec![Task::Lower(expression, parent, argument)];
        let mut fragments = Vec::new();
        while let Some(task) = tasks.pop() {
            match task {
                Task::Lower(expression, parent, argument) => {
                    if let ExprKind::Binary { op, left, right } = &expression.kind {
                        let name = if op == "," {
                            "<operator>.expressionList"
                        } else {
                            binary_operator(op)
                        };
                        let id = self.node("CALL", &expression.span, Some(name), None);
                        self.ast(parent, id, argument);
                        tasks.push(Task::Join(id, op == "&&" || op == "||"));
                        tasks.push(Task::Lower(right, id, Some(2)));
                        tasks.push(Task::Lower(left, id, Some(1)));
                    } else {
                        fragments.push(self.expression(expression, parent, argument));
                    }
                }
                Task::Join(id, short_circuit) => {
                    let right = fragments.pop().unwrap();
                    let left = fragments.pop().unwrap();
                    let flow = if short_circuit {
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
                    };
                    fragments.push(flow);
                }
            }
        }
        fragments.pop().unwrap()
    }

    pub(super) fn block_is_container(&self, parent: NodeId) -> bool {
        let node = &self.graph.nodes[parent as usize];
        node.kind == "METHOD"
            || node.kind == "CONTROL_STRUCTURE"
            || matches!(
                node.name.as_deref(),
                Some("<operator>.conditional" | "<operator>.logicalAnd" | "<operator>.logicalOr")
            )
    }

    pub(super) fn expression_block(
        &mut self,
        expressions: &[Expr],
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
    ) -> Fragment {
        let designated = self.graph.nodes[parent as usize].name.as_deref()
            == Some("<operator>.arrayInitializer")
            && expressions.iter().all(
                |expression| matches!(&expression.kind, ExprKind::Binary { op, .. } if op == "="),
            );
        // Designator conversion creates a block without CODE; its generated
        // getter exposes <empty>, while ordinary expression blocks use source.
        let id = self.node("BLOCK", span, None, designated.then_some("<empty>"));
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

    pub(super) fn cast_type_code(&self, span: &Span) -> Option<&str> {
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

    pub(super) fn closure_for_expression<'a>(
        &'a self,
        expression: &'a Expr,
    ) -> Option<&'a Closure> {
        match &expression.kind {
            ExprKind::Lambda(closure) => Some(closure),
            ExprKind::Identifier(name) => self
                .binding(name)
                .and_then(|(id, _)| self.closures.get(id))
                .or_else(|| self.inherited_closures.get(name)),
            ExprKind::Bracketed(inner)
            | ExprKind::Generated {
                expression: inner, ..
            } => self.closure_for_expression(inner),
            ExprKind::Unary { op, argument, .. } if matches!(op.as_str(), "*" | "&") => {
                self.closure_for_expression(argument)
            }
            _ => None,
        }
    }

    pub(super) fn pointer_call_expression(&self, expression: &Expr) -> bool {
        match &expression.kind {
            ExprKind::Identifier(name) => self
                .binding(name)
                .is_some_and(|(id, ty)| ty.contains('*') || self.function_pointers.contains(id)),
            ExprKind::Bracketed(inner)
            | ExprKind::Generated {
                expression: inner, ..
            } => self.pointer_call_expression(inner),
            _ => self
                .expression_type(expression)
                .is_some_and(|ty| ty.contains('*')),
        }
    }

    pub(super) fn member_binding_is_opaque(&self, callee: &Expr) -> bool {
        let ExprKind::Member { base, name, .. } = &callee.kind else {
            return false;
        };
        if name.contains("::") {
            return true;
        }
        self.receiver_has_known_type(base)
    }

    pub(super) fn receiver_has_known_type(&self, base: &Expr) -> bool {
        // A known owner can require inheritance, overload or qualified lookup
        // that we have not implemented. Retain the prior base-only AST rather
        // than treating our incomplete lookup as CDT's problem binding.
        let owner = match &base.kind {
            ExprKind::Call { callee, arguments } => self
                .resolve_call(callee, arguments)
                .map(|method| method.binding_return_type.clone()),
            _ => self.expression_type(base),
        };
        if owner.is_some_and(|ty| {
            let owner = ty
                .trim_end_matches(['*', '&'])
                .split_whitespace()
                .filter(|word| !matches!(*word, "const" | "volatile"))
                .collect::<Vec<_>>()
                .join(" ");
            self.known_types.contains(&owner)
        }) {
            return true;
        }
        // This is a refusal boundary, not result-type inference: a known
        // carrier through indexing/field access can require missing lookup.
        // Do not turn that missing evidence into a guessed problem binding.
        match &base.kind {
            ExprKind::Index { base, .. } | ExprKind::Member { base, .. } => {
                self.receiver_has_known_type(base)
            }
            ExprKind::Bracketed(inner)
            | ExprKind::Generated {
                expression: inner, ..
            }
            | ExprKind::Unary {
                argument: inner, ..
            } => self.receiver_has_known_type(inner),
            _ => false,
        }
    }

    pub(super) fn resolve_call(&self, callee: &Expr, arguments: &[Expr]) -> Option<&Function> {
        let mut keys = Vec::new();
        match &callee.kind {
            ExprKind::Identifier(name) => {
                if self.cpp && !name.contains("::") && !self.method_scope.is_empty() {
                    keys.push(format!("{}.{}", self.method_scope, name));
                }
                keys.push(name.trim_start_matches("::").replace("::", "."));
            }
            ExprKind::Member { base, name, .. } if self.cpp => {
                if name.contains("::") {
                    // A base/namespace qualifier requires C++ qualified lookup,
                    // not a same-named method on the receiver's type or a
                    // guessed global owner. Retain the callee text unresolved.
                    return None;
                } else {
                    let owner = self.expression_type(base)?;
                    let owner = owner.trim_end_matches(['*', '&']).replace("::", ".");
                    keys.push(format!("{owner}.{name}"));
                }
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

    pub(super) fn expression_type(&self, expression: &Expr) -> Option<String> {
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

    pub(super) fn qualified_owner(
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
            return Fragment::single(self.identifier_node(head, span, parent, argument, false));
        }
        let id = self.node(
            "CALL",
            span,
            Some("<operator>.fieldAccess"),
            Some(&names.join("::")),
        );
        self.ast(parent, id, argument);
        Fragment::single(self.identifier_node(head, span, id, Some(1), false))
            .append(self.qualified_owner(tail, span, id, Some(2)))
            .append(Fragment::single(id))
    }

    #[allow(clippy::too_many_arguments)] // Syntax and AST placement are independent macro inputs.
    pub(super) fn macro_call(
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

    pub(super) fn condition(&mut self, expression: &Expr, parent: NodeId, _wrap: bool) -> Fragment {
        let before = self.graph.nodes.len();
        let flow = self.condition_expression(expression, parent);
        if self.graph.nodes.len() > before {
            self.edge(parent, before as NodeId, "CONDITION", None);
        }
        flow
    }

    pub(super) fn condition_expression(&mut self, expression: &Expr, parent: NodeId) -> Fragment {
        let before = self.graph.nodes.len();
        if let ExprKind::List(expressions) = &expression.kind {
            let flow = self.expression_block(expressions, &expression.span, parent, None);
            self.graph.nodes[before].code.clear();
            flow
        } else {
            self.expression(expression, parent, None)
        }
    }
}

/// The parser has already accepted the expression name. Scope separators in
/// template arguments do not qualify the callee itself, and punctuation in an
/// operator-function-id is not the start of a template argument list.
pub(super) fn cpp_call_leaf(name: &str) -> &str {
    let mut leaf = 0;
    let mut depth = 0usize;
    for (index, ch) in name.char_indices() {
        if depth == 0 && index >= leaf {
            let rest = &name[index..];
            if name[leaf..index].trim().is_empty()
                && rest.strip_prefix("operator").is_some_and(|suffix| {
                    !suffix.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
            {
                return rest;
            }
            if rest.starts_with("::") {
                leaf = index + 2;
            }
        }
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    name[leaf..].trim_start()
}

/// AstNodeBuilder.shortenCode: commons-lang `abbreviate(code, 1000)`, which
/// keeps 997 UTF-16 units of a longer CODE and appends an ellipsis.
pub(super) fn shorten_code(code: &str) -> Cow<'_, str> {
    const MAX: usize = 1000;
    if code.len() <= MAX {
        return Cow::Borrowed(code);
    }
    let (mut units, mut cut) = (0, code.len());
    for (at, character) in code.char_indices() {
        units += character.len_utf16();
        if units > MAX - 3 && cut == code.len() {
            cut = at;
        }
    }
    if units <= MAX {
        Cow::Borrowed(code)
    } else {
        Cow::Owned(format!("{}...", &code[..cut]))
    }
}

pub(super) fn is_expression_list(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Generated { expression, .. } => is_expression_list(expression),
        ExprKind::List(_) => true,
        _ => false,
    }
}

pub(super) fn function_pointer_parameter(fullname: &str, argument: usize) -> Option<String> {
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

// Allocation expressions are Unary nodes in our syntax tree, while CDT has
// separate new/delete AST classes. Keep that lexical bridge separate from the
// mechanically translated IASTUnaryExpression operator match.
pub(super) fn unary_operator(operator: &str, postfix: bool) -> &'static str {
    match operator {
        "sizeof..." => "<operator>.sizeOf",
        "new" => "<operator>.new",
        "delete" | "delete[]" => "<operator>.delete",
        _ => crate::joern_operators::unary_operator(operator, postfix),
    }
}

pub(super) fn type_id_operand(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::TypeSpecifier { .. } => true,
        ExprKind::Generated { expression, .. } => type_id_operand(expression),
        _ => false,
    }
}

pub(super) fn member_pointer_expression(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Binary { op, .. } => matches!(op.as_str(), ".*" | "->*"),
        ExprKind::Bracketed(inner)
        | ExprKind::Generated {
            expression: inner, ..
        } => member_pointer_expression(inner),
        _ => false,
    }
}
