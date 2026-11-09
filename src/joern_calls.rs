// SPDX-License-Identifier: Apache-2.0
//! Direct port of c2cpg 4.0.150 call-operand placement (Joern contributors).
//!
//! See AstForExpressionsCreator.scala: astForCppCallExpressionUntyped,
//! astForCppCallExpression, createPointerCallAst; and AstCreatorHelper.scala:
//! createCallAst (NOT x2cpg's callAst, which promotes base to receiver).
//! Binding evidence belongs to the parser/builder; this module never infers
//! a resolved type or a virtual method from spelling.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operand {
    None,
    Callee,
    MemberBase,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallKind {
    Named,
    Pointer,
    Closure,
    ResolvedMember {
        is_virtual: bool,
    },
    /// Known owner/qualified lookup exceeds the current binding contract.
    /// Missing Rust binding evidence is not proof of CDT's IProblemType.
    OpaqueMember,
    UnresolvedMember,
    UnresolvedExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CallLayout {
    pub operand: Operand,
    pub base: bool,
    pub receiver: bool,
}

impl CallLayout {
    pub fn for_kind(kind: CallKind) -> Self {
        let (operand, base, receiver) = match kind {
            CallKind::Named => (Operand::None, false, false),
            CallKind::Pointer | CallKind::Closure => (Operand::Callee, false, true),
            CallKind::ResolvedMember { is_virtual } => (Operand::MemberBase, true, is_virtual),
            CallKind::OpaqueMember => (Operand::MemberBase, true, false),
            CallKind::UnresolvedMember => (Operand::MemberBase, true, true),
            CallKind::UnresolvedExpression => (Operand::Callee, true, true),
        };
        Self {
            operand,
            base,
            receiver,
        }
    }

    pub fn base_index(self) -> Option<usize> {
        self.base.then_some(0)
    }
}

/// astForCppCallExpressionUntyped: the argument count is part of unresolved
/// identity, but never evidence for a resolved overload.
pub(crate) fn unresolved_full_name(name: &str, argument_count: usize) -> String {
    format!("<unresolvedNamespace>.{name}:<unresolvedSignature>({argument_count})")
}
