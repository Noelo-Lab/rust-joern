//! CPG node and edge creation, bindings and identifiers.

use super::*;

impl Builder<'_> {
    pub(super) fn code(&self, span: &Span) -> &str {
        self.source.get(span.start..span.end).unwrap_or("")
    }

    pub(super) fn node(
        &mut self,
        kind: &str,
        span: &Span,
        name: Option<&str>,
        code: Option<&str>,
    ) -> NodeId {
        let id = self.graph.nodes.len() as NodeId;
        self.graph.nodes.push(Node {
            id,
            kind: kind.into(),
            code: match code {
                // A control structure's CODE joins already shortened parts.
                Some(code) if kind == "CONTROL_STRUCTURE" => code.into(),
                _ => shorten_code(code.unwrap_or_else(|| self.code(span))).into(),
            },
            name: name.map(str::to_string),
            method_full_name: (kind == "CALL")
                .then(|| {
                    name.filter(|name| {
                        !self.cpp
                            || name.starts_with("<operator>.")
                            || name.starts_with("<operators>.")
                    })
                })
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

    pub(super) fn edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        kind: &str,
        label: Option<String>,
    ) {
        self.graph.edges.push(Edge {
            source,
            target,
            kind: kind.into(),
            label,
        });
    }

    pub(super) fn ast(&mut self, parent: NodeId, child: NodeId, argument: Option<usize>) {
        self.edge(parent, child, "AST", None);
        if let Some(argument) = argument {
            self.edge(parent, child, "ARGUMENT", Some(argument.to_string()));
        }
    }

    pub(super) fn binding(&self, name: &str) -> Option<&(NodeId, String)> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    pub(super) fn identifier(
        &mut self,
        name: &str,
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
    ) -> NodeId {
        self.identifier_node(name, span, parent, argument, true)
    }

    /// A scope qualifier names a class or namespace, so it is never a function
    /// reference, even when it spells the constructor of the enclosing class.
    pub(super) fn identifier_node(
        &mut self,
        name: &str,
        span: &Span,
        parent: NodeId,
        argument: Option<usize>,
        may_reference_function: bool,
    ) -> NodeId {
        let binding = self.binding(name).cloned();
        let known_function = may_reference_function
            && binding.is_none()
            && !self.inherited_bindings.contains_key(name)
            && self.functions.contains(name);
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
        } else if let Some(type_name) = self.inherited_bindings.get(name) {
            self.graph.nodes[id as usize].type_name = Some(type_name.clone());
        } else if self.implicit_fields.contains_key(name)
            && let Some(owner) = self.this_owner()
        {
            // CDT's non-cv-qualified member owner loses namespace/class
            // qualification. syntheticThisAccess rejects that scope match.
            self.graph.nodes[id as usize].type_name =
                Some(format!("{}*", owner.rsplit('.').next().unwrap()));
        } else if kind == "IDENTIFIER" && (self.lambda || self.cpp && known_function) {
            self.graph.nodes[id as usize].type_name = Some("ANY".into());
        }
        id
    }

    pub(super) fn resolve_method_reference(
        &self,
        name: &str,
        signature: Option<&str>,
    ) -> Option<&Function> {
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

    pub(super) fn declaration_identifier(
        &mut self,
        declaration: &Declaration,
        parent: NodeId,
        argument: usize,
    ) -> NodeId {
        let declarator = self.code(&declaration.span);
        if declarator.trim_start().starts_with('(') {
            let (tokens, _) = crate::lexer::lex_preprocessed(declarator, true);
            let token = |index: usize| {
                tokens
                    .get(index)
                    .map(|token| &declarator[token.span.start..token.span.end])
            };
            if token(0) == Some("(") && matches!(token(1), Some("*" | "&" | "&&")) {
                // AstForInitializer visits the outer declarator's empty NAME,
                // while binding resolution still recovers the nested symbol.
                // Its CODE is explicitly empty and has no source location.
                let id =
                    self.identifier(&declaration.name, &Span::default(), parent, Some(argument));
                self.graph.nodes[id as usize].code.clear();
                return id;
            }
        }
        if declaration.nested_declarator {
            // `T (x) = v`, including CDT's reading of `LOBYTE(x) = v`, takes
            // the same path: nameForIdentifier resolves the outer
            // declarator's empty name to the nested variable's binding.
            let id = self.identifier(&declaration.name, &Span::default(), parent, Some(argument));
            self.graph.nodes[id as usize].code.clear();
            return id;
        }
        if declaration.is_typedef {
            let id = self.node(
                "IDENTIFIER",
                &declaration.span,
                Some(&declaration.name),
                Some(&declaration.name),
            );
            self.graph.nodes[id as usize].type_name = Some(declaration.name.clone());
            self.ast(parent, id, Some(argument));
            return id;
        }
        let mut span = declaration.span.clone();
        // A declarator starts at its pointer/reference operators, while CDT's
        // generated initializer LHS identifier starts at the declared name.
        if let Some(offset) =
            self.code(&span)
                .match_indices(&declaration.name)
                .find_map(|(offset, _)| {
                    let code = self.code(&span);
                    let word = |c: char| c == '_' || c.is_alphanumeric();
                    let before = code[..offset].chars().next_back();
                    let after = code[offset + declaration.name.len()..].chars().next();
                    (!before.is_some_and(word) && !after.is_some_and(word)).then_some(offset)
                })
        {
            let prefix = &self.code(&span)[..offset];
            let newlines = prefix.bytes().filter(|&byte| byte == b'\n').count();
            span.line += newlines;
            span.column = prefix
                .rsplit_once('\n')
                .map_or(span.column + offset, |(_, tail)| tail.len() + 1);
            span.start += offset;
            span.end = span.start + declaration.name.len();
            span.end_line = span.line;
        }
        self.identifier(&declaration.name, &span, parent, Some(argument))
    }

    pub(super) fn this_owner(&self) -> Option<&String> {
        self.binding("this")
            .map(|(_, ty)| ty)
            .or_else(|| self.inherited_bindings.get("this"))
    }

    pub(super) fn implicit_field_scope_matches(&self) -> bool {
        self.this_owner()
            .is_some_and(|owner| !owner.contains('.') || self.member_cv_qualified)
    }
}
