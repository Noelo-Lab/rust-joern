//! Statement and declaration lowering.

use super::*;

impl Builder<'_> {
    pub(super) fn declarations(
        &mut self,
        declarations: &[Declaration],
        span: &Span,
        parent: NodeId,
    ) -> Fragment {
        self.declaration_parts(declarations, span, parent)
            .into_iter()
            .fold(Fragment::default(), Fragment::append)
    }

    pub(super) fn declaration_parts(
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
                let lhs = self.declaration_identifier(declaration, assignment, 1);
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
                let closure = self.closure_for_expression(initializer).cloned();
                let code = self.code(&declaration.span).to_string();
                let id = self.node(
                    "CALL",
                    &declaration.span,
                    Some("<operator>.assignment"),
                    Some(&code),
                );
                self.ast(parent, id, None);
                let lhs = self.declaration_identifier(declaration, id, 1);
                let rhs = self.expression(initializer, id, Some(2));
                parts.push(
                    Fragment::single(lhs)
                        .append(rhs)
                        .append(Fragment::single(id)),
                );
                if let Some(closure) = closure
                    && let Some(&(local, _)) = self.binding(&declaration.name)
                {
                    self.closures.insert(local, closure);
                }
            }
        }
        parts
    }

    pub(super) fn statement_parts(&mut self, statement: &Stmt, parent: NodeId) -> Vec<Fragment> {
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

    pub(super) fn statement(
        &mut self,
        statement: &Stmt,
        parent: NodeId,
        _try_return: bool,
    ) -> Fragment {
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
            StmtKind::Expression(expression) => {
                let flow = self.expression(expression, parent, None);
                // The parser represents block-scope C++ static_assert as a
                // call. Its operands belong in the AST, but none run at runtime.
                if self.cpp
                    && !self.joern_cpp_control_flow
                    && matches!(&expression.kind, ExprKind::Call { callee, .. }
                        if matches!(&callee.kind, ExprKind::Identifier(name) if name == "static_assert"))
                {
                    Fragment::default()
                } else {
                    flow
                }
            }
            StmtKind::Declaration(declarations) => {
                self.declarations(declarations, &statement.span, parent)
            }
            StmtKind::FunctionDefinition { name, full_name } => {
                let id = self.node("METHOD", &statement.span, Some(name), None);
                let method = &mut self.graph.nodes[id as usize];
                method.method_full_name = Some(full_name.clone());
                method.external_ref = Some(ExternalReference {
                    method_full_name: full_name.clone(),
                    node: 0,
                });
                self.ast(parent, id, None);
                // Joern retains the nested method in the enclosing AST, but
                // converts its body only in the separate method CFG.
                Fragment::default()
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
                let unconditional = self.unconditional(condition);
                let condition = self.condition(condition, id, true);
                if unconditional {
                    // Lower the body through the same order slots as `for (;;)`.
                    let flow = self.for_loop(id, vec![Fragment::default()], body);
                    self.scopes.pop();
                    return flow;
                }
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
                // cfgForWhileStatement: own edges precede the nested ones.
                flow.edges.splice(0..0, links);
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
                // cfgForDoStatement: own edges precede the nested ones.
                flow.edges.splice(0..0, links);
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
                // c2cpg converts the initializer in its own block scope and
                // pops it before visiting the condition, update and body.
                self.scopes.pop();
                self.scopes.push(HashMap::new());
                let mut slots = vec![initial];
                if let Some(condition) = condition {
                    let before = self.graph.nodes.len();
                    let flow = self.condition(condition, id, true);
                    if self.graph.nodes.len() != before && !self.unconditional(condition) {
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
                let flow = self.for_loop(id, slots, body);
                self.scopes.pop();
                flow
            }
            // CDT cannot parse a C++20 init-statement, so Joern has no loop.
            StmtKind::RangeFor {
                initializer: Some(_),
                ..
            } if self.joern_cpp_control_flow => Fragment::default(),
            StmtKind::RangeFor {
                initializer,
                declaration,
                iterable,
                body,
            } => {
                let id = self.control(statement, parent, "FOR");
                self.scopes.push(HashMap::new());
                let initial = initializer
                    .as_ref()
                    .map(|statement| self.statement(statement, id, false))
                    .unwrap_or_default();
                let initial = initial.append(self.expression(iterable, id, None));
                self.declarations(
                    &[Declaration {
                        initializer: None,
                        dimensions: Vec::new(),
                        ..declaration.clone()
                    }],
                    &declaration.span,
                    id,
                );
                if self.joern_cpp_control_flow {
                    // Joern 4.0.150 selects for children by local count and order.
                    // Its range-for body occupies the condition slot, while the
                    // initializer is never evaluated.
                    let mut condition = self.statement(body, id, false);
                    if let Some(target) = condition.entry {
                        condition
                            .edges
                            .extend(condition.fringe.iter().map(|&source| [source, target]));
                    }
                    condition.breaks.clear();
                    condition.continues.clear();
                    self.scopes.pop();
                    return condition;
                }
                // The iterator protocol is implicit. Represent its test and
                // advance without inventing calls to particular overloads.
                let test = self.node("UNKNOWN", &iterable.span, None, Some("<range-for test>"));
                self.ast(id, test, None);
                let mut body = self.statement(body, id, false);
                let mut fringe = vec![test];
                fringe.append(&mut body.breaks);
                let mut links = Vec::new();
                if let Some(body_entry) = body.entry {
                    links.push([test, body_entry]);
                }
                if body.entry.is_none() || !body.fringe.is_empty() || !body.continues.is_empty() {
                    let advance =
                        self.node("UNKNOWN", &iterable.span, None, Some("<range-for advance>"));
                    self.ast(id, advance, None);
                    links.push([advance, test]);
                    if body.entry.is_none() {
                        links.push([test, advance]);
                    }
                    links.extend(
                        body.fringe
                            .iter()
                            .chain(&body.continues)
                            .map(|&node| [node, advance]),
                    );
                }
                body.continues.clear();
                let initial = initial.append(Fragment::single(test));
                let entry = initial.entry;
                let mut flow = Fragment::combine([initial, body]);
                flow.entry = entry;
                flow.fringe = fringe;
                flow.edges.extend(links);
                self.scopes.pop();
                flow
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
