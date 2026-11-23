//! CFG fragments and control-structure lowering.

use super::*;

impl Fragment {
    pub(super) fn single(node: NodeId) -> Self {
        Self {
            entry: Some(node),
            fringe: vec![node],
            ..Self::default()
        }
    }

    pub(super) fn empty(&self) -> bool {
        self.entry.is_none()
            && self.edges.is_empty()
            && self.fringe.is_empty()
            && self.labels.is_empty()
            && self.jumps.is_empty()
    }

    pub(super) fn append(mut self, other: Self) -> Self {
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

    pub(super) fn combine(parts: impl IntoIterator<Item = Self>) -> Self {
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

impl Builder<'_> {
    pub(super) fn finish_cfg_statement_kinds(&mut self) {
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

    pub(super) fn control(&mut self, statement: &Stmt, parent: NodeId, name: &str) -> NodeId {
        let short = |span: &Span| shorten_code(self.code(span));
        let code = match &statement.kind {
            StmtKind::If { condition, .. } => Some(format!("if ({})", short(&condition.span))),
            StmtKind::While { condition, .. } => {
                Some(format!("while ({})", short(&condition.span)))
            }
            StmtKind::Switch { condition, .. } => {
                Some(format!("switch({})", short(&condition.span)))
            }
            StmtKind::For {
                initializer,
                condition,
                update,
                ..
            } => Some(format!(
                "for ({}{};{})",
                // The initializer statement is shortened with its semicolon,
                // and keeps the white space before it: `for (i = 0 ;i;i++)`.
                initializer.as_ref().map_or(";".into(), |x| {
                    let code = self.code(&x.span).trim_end_matches(';');
                    let rest = self.source.get(x.span.end..).unwrap_or("");
                    let space = &rest[..rest.len() - rest.trim_start().len()];
                    let space = if rest[space.len()..].starts_with(';') {
                        space
                    } else {
                        ""
                    };
                    shorten_code(&format!("{code}{space};")).into_owned()
                }),
                condition
                    .as_ref()
                    .map(|x| short(&x.span))
                    .unwrap_or_default(),
                update.as_ref().map(|x| short(&x.span)).unwrap_or_default(),
            )),
            StmtKind::Try { .. } if name == "TRY" => Some("try".into()),
            StmtKind::Throw(expression) => Some(
                expression
                    .as_ref()
                    // The whole throw expression is one shortened CODE.
                    .map(|expression| {
                        let code = format!("throw {}", self.code(&expression.span));
                        shorten_code(&code).into_owned()
                    })
                    .unwrap_or_else(|| "throw".into()),
            ),
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

    /// Decompilers spell an infinite loop `while (1)`, `while (true)` or, in
    /// IDA, `while ( 2 )`, whereas source mostly uses `for (;;)`. Joern 4.0.150
    /// omits the CFG condition node and exit edge only for `for (;;)`. Unless
    /// `joern_loop_conditions` is set, a constant-true condition keeps its AST
    /// but lowers like that omitted condition.
    pub(super) fn unconditional(&self, condition: &Expr) -> bool {
        if self.joern_loop_conditions {
            return false;
        }
        match &condition.kind {
            ExprKind::Identifier(value) => value == "true",
            ExprKind::Literal(value) => value == "true" || integer_truth(value) == Some(true),
            _ => false,
        }
    }

    /// Joern 4.0.150 selects FOR children by `order`, rather than CONDITION
    /// edges. `slots` holds the initializer and each nonempty condition or
    /// update; empty expression ASTs consume no order. A compound body keeps
    /// explicit order 4, while other bodies shift into the vacant slots.
    pub(super) fn for_loop(
        &mut self,
        id: NodeId,
        mut slots: Vec<Fragment>,
        body: &Stmt,
    ) -> Fragment {
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
        // cfgForForStatement lists the loop's own edges before the nested
        // ones, which orders the successors of a node ending the body.
        flow.edges.splice(0..0, links);
        flow.breaks.clear();
        flow.continues.clear();
        flow
    }

    pub(super) fn branch(&mut self, statement: &Stmt, parent: NodeId) -> Fragment {
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
}
