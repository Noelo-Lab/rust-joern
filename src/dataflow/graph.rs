//! The reaching-definitions graph, its edges and solver order.

use super::*;

impl Bits {
    pub(super) fn new(size: usize) -> Self {
        Self(vec![0; size.div_ceil(64)])
    }

    pub(super) fn insert(&mut self, bit: usize) {
        self.0[bit / 64] |= 1 << (bit % 64);
    }

    pub(super) fn union(&mut self, other: &Self) {
        for (a, b) in self.0.iter_mut().zip(&other.0) {
            *a |= b;
        }
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = usize> + '_ {
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

/// Iteration order of a Scala 2.13 `immutable.HashMap` with these string keys:
/// a trie node yields its own entries by hash chunk, then its child nodes.
pub(super) fn scala_hash_map_order<'a>(keys: Vec<&'a str>, shift: u32, output: &mut Vec<&'a str>) {
    fn improved_hash(key: &str) -> u32 {
        let hash = key.encode_utf16().fold(0u32, |hash, unit| {
            hash.wrapping_mul(31).wrapping_add(unit as u32)
        });
        let hash = hash.wrapping_add(!(hash << 9));
        let hash = hash ^ (hash >> 14);
        let hash = hash.wrapping_add(hash << 4);
        hash ^ (hash >> 10)
    }
    if shift >= 32 {
        output.extend(keys);
        return;
    }
    let mut slots: [Vec<&str>; 32] = std::array::from_fn(|_| Vec::new());
    for key in keys {
        slots[((improved_hash(key) >> shift) & 31) as usize].push(key);
    }
    output.extend(
        slots
            .iter()
            .filter(|slot| slot.len() == 1)
            .map(|slot| slot[0]),
    );
    for slot in slots.into_iter().filter(|slot| slot.len() > 1) {
        scala_hash_map_order(slot, shift + 5, output);
    }
}

pub(super) fn reverse_postorder(entry: usize, next: &[Vec<usize>]) -> Vec<usize> {
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

impl Graph<'_> {
    pub(super) fn add_argument(&mut self, source: usize, target: usize, edge: &Edge) {
        let position = edge
            .label
            .as_deref()
            .and_then(|s| s.parse().ok())
            .unwrap_or(self.args[source].len() as i32 + 1);
        self.args[source].push((position, target));
        if call(&self.nodes[source]) {
            self.parent_call[target] = Some((source, position));
        }
    }

    pub(super) fn semantic(&self, at: usize) -> Option<Semantic> {
        let node = &self.nodes[at];
        match node.method_full_name.as_deref() {
            Some(full_name) => semantics(full_name),
            None if !self.cpp
                || name(node).starts_with("<operator>.")
                || name(node).starts_with("<operators>.") =>
            {
                semantics(name(node))
            }
            None => None,
        }
    }

    pub(super) fn output_argument_of_internal_method(&self, at: usize) -> bool {
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

    pub(super) fn descendants(&self, at: usize) -> Vec<usize> {
        let mut stack = self.ast[at].clone();
        let mut result = Vec::new();
        while let Some(i) = stack.pop() {
            result.push(i);
            stack.extend(self.ast[i].iter().copied());
        }
        result
    }

    pub(super) fn argument(&self, at: usize, position: i32) -> Option<usize> {
        self.args[at]
            .iter()
            .find(|&&(p, _)| p == position)
            .map(|&(_, n)| n)
    }

    pub(super) fn uses(&self, at: usize) -> Vec<usize> {
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

    pub(super) fn text(&self, at: usize) -> Option<&str> {
        let node = &self.nodes[at];
        if node.kind == "IDENTIFIER" || input(node) || output(node) {
            Some(name(node))
        } else if expression(node) {
            Some(&node.code)
        } else {
            None
        }
    }

    pub(super) fn label(&self, at: usize) -> &str {
        if input(&self.nodes[at]) {
            name(&self.nodes[at])
        } else {
            &self.nodes[at].code
        }
    }

    pub(super) fn is_using(&self, used: usize, definition: usize) -> bool {
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

    pub(super) fn used(&self, at: usize) -> bool {
        self.parent_call[at].is_none_or(|(parent, position)| {
            self.semantic(parent).is_none_or(|s| {
                s.pass && position != 0 || s.flows.iter().any(|&(src, _)| src == position)
            })
        })
    }

    pub(super) fn defined(&self, at: usize) -> bool {
        self.parent_call[at].is_none_or(|(parent, position)| {
            self.semantic(parent).is_none_or(|s| {
                s.pass && position != 0 || s.flows.iter().any(|&(_, dst)| dst == position)
            })
        })
    }

    pub(super) fn flow(&self, source: usize, target: usize) -> bool {
        let Some((parent, source_position)) = self.parent_call[source] else {
            return true;
        };
        let target_position = self.parent_call[target].map(|(_, p)| p).unwrap_or(-1);
        self.semantic(parent).is_none_or(|s| {
            s.pass && source_position == target_position
                || s.flows.contains(&(source_position, target_position))
        })
    }

    pub(super) fn retval_blocked(&self, at: usize) -> bool {
        call(&self.nodes[at])
            && self
                .semantic(at)
                .is_some_and(|s| !s.pass && !s.flows.iter().any(|&(_, dst)| dst == -1))
    }

    pub(super) fn valid_edge(&self, source: usize, target: usize) -> bool {
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

    pub(super) fn block_edges(
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

    pub(super) fn path(&self, at: usize) -> Option<(String, Vec<Access>)> {
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
            // extractAccessIntToken reads a constant only from a literal or
            // field identifier: `p[-1]` indexes by a call and stays variable.
            let shift = self
                .argument(at, 2)
                .and_then(|i| {
                    let node = &self.nodes[i];
                    match node.kind.as_str() {
                        "LITERAL" => node.code.parse::<i32>().ok(),
                        "FIELD_IDENTIFIER" => node
                            .name
                            .as_deref()
                            .unwrap_or(&node.code)
                            .parse::<i32>()
                            .ok(),
                        _ => None,
                    }
                })
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

impl Edges<'_, '_> {
    pub(super) fn add(&mut self, source: usize, target: usize, label: &str) {
        if self.graph.valid_edge(source, target) {
            self.edges.push(Edge {
                source: self.graph.nodes[source].id,
                target: self.graph.nodes[target].id,
                kind: "REACHING_DEF".into(),
                label: Some(label.into()),
            });
        }
    }
}
