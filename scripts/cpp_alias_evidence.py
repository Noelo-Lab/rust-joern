"""Experimental scalar alias evidence for preprocessed C++ CFG replay."""

from __future__ import annotations

import re
from collections import defaultdict
from dataclasses import dataclass
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from tree_sitter import Node


PRIMITIVES = frozenset(
    b"void bool char signed unsigned short int long float double wchar_t char8_t char16_t char32_t".split()
)


SCALARS = {"primitive_type", "sized_type_specifier"}


RECORDS = {"class_specifier", "struct_specifier"}


NAME = re.compile(rb"[A-Za-z_][A-Za-z_0-9]*\Z")


class Unqualified(ValueError):
    """The evidence cannot establish the advertised bounded source policy."""


def bounds(block: Node) -> tuple[Node, Node]:
    if block.child_count >= 2:
        opening, closing = block.children[0], block.children[-1]
        if (
            opening.type == "{"
            and closing.type == "}"
            and not (opening.is_missing or closing.is_missing)
        ):
            return opening, closing
    raise Unqualified("unproved_declaration_scope")


def namespace_parts(node: Node) -> tuple[bytes, ...]:
    name = node.child_by_field_name("name")
    if name is None:
        return (b"",)
    text = re.sub(rb"\s*::\s*", b"::", name.text)
    parts = tuple(text.split(b"::"))
    if not all(NAME.fullmatch(part) for part in parts):
        raise Unqualified("unproved_declaration_scope")
    return parts


@dataclass(frozen=True)
class Alias:
    declaration: Node
    target: Node
    scope: tuple[bytes, ...]
    boundaries: tuple[Node, ...]


class AliasIndex:
    """Resolve earlier namespace aliases to primitive types.

    The caller supplies the namespace path at the type occurrence and proves
    that intervening class and block scopes do not change lookup. The input
    must be the caller's preprocessed bytes. Results are candidate declaration
    evidence and must still be checked against diagnostics and strict replay.
    """

    def __init__(self, root: Node):
        self.by_name = defaultdict(list)
        self.blockers = defaultdict(list)
        self.visit(root, (), ())

    def visit(self, block: Node, path: tuple[bytes, ...], boundaries: tuple[Node, ...]):
        for node in block.named_children:
            body = node.child_by_field_name("body")
            if node.type == "namespace_definition" and body is not None:
                name = node.child_by_field_name("name")
                scoped = None
                try:
                    scoped = namespace_parts(node), bounds(body)
                except Unqualified:
                    pass
                if (
                    scoped
                    and scoped[0] != (b"",)
                    and not any(p.type == "inline" for p in node.children)
                    and all(
                        c in (name, body) or c.type == "comment"
                        for c in node.named_children
                    )
                ):
                    names, brace = scoped
                    header = tuple(
                        child for child in node.named_children if child != body
                    )
                    self.visit(body, path + names, (*boundaries, *header, *brace))
                elif scoped and scoped[0] == (b"",):
                    # Unnamed namespaces can inject names into the parent.
                    # Record only their named declarations and lookup directives.
                    self.visit(body, path, (*boundaries, node))
                elif (
                    scoped
                    and len(scoped[0]) == 1
                    and any(p.type == "inline" for p in node.children)
                ):
                    # An inline namespace injects its declarations into the
                    # enclosing lookup. Track specific competing names.
                    self.visit(body, path, (*boundaries, node))
                    self.visit(body, path + scoped[0], (*boundaries, node))
                else:
                    self.blockers[path, None].append(node)
                continue
            if node.type == "linkage_specification" and body is not None:
                value = node.child_by_field_name("value")
                if value is not None and value.text in {b'"C"', b'"C++"'}:
                    if body.type == "declaration_list":
                        try:
                            brace = bounds(body)
                        except Unqualified:
                            self.blockers[path, None].append(node)
                        else:
                            self.visit(body, path, (*boundaries, value, *brace))
                    else:
                        self.add(body, path, (*boundaries, value))
                continue
            self.add(node, path, boundaries)

    def add(self, node: Node, path: tuple[bytes, ...], boundaries: tuple[Node, ...]):
        name = node.child_by_field_name("name")
        ty = node.child_by_field_name("type")
        declarators = node.children_by_field_name("declarator")
        if node.type == "alias_declaration":
            target = (
                ty.child_by_field_name("type")
                if ty and ty.type == "type_descriptor"
                else None
            )
            if (
                name
                and target
                and not node.has_error
                and ty.named_children == [target]
                and all(
                    child in (name, ty) or child.type == "comment"
                    for child in node.named_children
                )
            ):
                self.by_name[path, name.text].append(
                    Alias(node, target, path, boundaries)
                )
            elif name:
                self.blockers[path, name.text].append(node)
            return
        if node.type == "type_definition":
            if (
                ty
                and not node.has_error
                and len(declarators) == 1
                and declarators[0].type in {"type_identifier", "primitive_type"}
                and NAME.fullmatch(declarators[0].text)
                and all(
                    child in (ty, declarators[0]) or child.type == "comment"
                    for child in node.named_children
                )
            ):
                self.by_name[path, declarators[0].text].append(
                    Alias(node, ty, path, boundaries)
                )
                return
        if node.type == "using_declaration":
            if any(c.type == "namespace" for c in node.children):
                self.blockers[path, None].append(node)
                return
            imported = node.named_children[-1] if node.named_children else None
            while imported and imported.type == "qualified_identifier":
                imported = imported.child_by_field_name("name")
            if imported and NAME.fullmatch(imported.text):
                self.blockers[path, imported.text].append(node)
            return
        if node.type in {"template_declaration", "ERROR"}:
            if node.type == "template_declaration":
                for child in node.named_children:
                    if child.type != "template_parameter_list":
                        target = child.child_by_field_name("name")
                        if target and NAME.fullmatch(target.text):
                            self.blockers[path, target.text].append(node)
            return
        for declared in self.declared_names(node):
            self.blockers[path, declared].append(node)

    @staticmethod
    def declared_names(node: Node):
        name = node.child_by_field_name("name")
        ty = node.child_by_field_name("type")
        if name and NAME.fullmatch(name.text):
            yield name.text
        if ty and ty.type in RECORDS | {"union_specifier", "enum_specifier"}:
            tag = ty.child_by_field_name("name")
            if tag and NAME.fullmatch(tag.text):
                yield tag.text
        for decl in node.children_by_field_name("declarator"):
            while decl.type not in {
                "type_identifier",
                "identifier",
                "field_identifier",
            }:
                inner = decl.child_by_field_name("declarator")
                if inner is None:
                    break
                decl = inner
            if NAME.fullmatch(decl.text):
                yield decl.text

    def resolve(
        self, ty: Node, scope: tuple[bytes, ...], used: dict[Node, Alias]
    ) -> tuple[bytes, ...]:
        dependencies = {}
        primitive = self._resolve(ty, scope, dependencies, 0)
        used.update(dependencies)
        return primitive

    def _resolve(
        self, ty: Node, scope: tuple[bytes, ...], used: dict[Node, Alias], depth: int
    ) -> tuple[bytes, ...]:
        if (
            ty.type in SCALARS
            and not ty.has_error
            and set(ty.text.split()) <= PRIMITIVES
        ):
            return tuple(sorted(ty.text.split()))
        if depth >= 32 or ty.has_error:
            raise Unqualified("unproved_type_declaration")
        if ty.type in {"type_identifier", "primitive_type"} and NAME.fullmatch(ty.text):
            name = ty.text
            scopes = [scope[:n] for n in range(len(scope), -1, -1)]
        elif (
            ty.type == "qualified_identifier"
            and ty.child_by_field_name("scope") is None
        ):
            name = ty.text.removeprefix(b"::")
            if not ty.text.startswith(b"::") or not NAME.fullmatch(name):
                raise Unqualified("unproved_type_declaration")
            scopes = [()]
        else:
            raise Unqualified("unproved_type_declaration")
        for path in scopes:
            if any(
                competitor.start_byte < ty.start_byte
                for key in ((path, name), (path, None))
                for competitor in self.blockers[key]
            ):
                raise Unqualified("unproved_alias_lookup")
            choices = [
                a
                for a in self.by_name[path, name]
                if a.declaration.end_byte <= ty.start_byte
            ]
            if not choices:
                continue
            resolved = set()
            for alias in choices:
                if any(b.type == "namespace_definition" for b in alias.boundaries):
                    raise Unqualified("unproved_alias_lookup")
                resolved.add(self._resolve(alias.target, alias.scope, used, depth + 1))
                used[alias.declaration] = alias
            if len(resolved) == 1:
                return resolved.pop()
            raise Unqualified("ambiguous_type_alias")
        raise Unqualified("unproved_type_declaration")


def render_alias_declarations(aliases: list[Alias] | tuple[Alias, ...]) -> bytes:
    """Replay the proved source declarations in their namespace and TU order."""
    chunks = []
    seen = set()
    for alias in sorted(aliases, key=lambda row: row.declaration.start_byte):
        if alias.declaration in seen:
            continue
        seen.add(alias.declaration)
        opening = b"".join(b"namespace " + part + b" { " for part in alias.scope)
        chunks.append(
            opening + alias.declaration.text + b" }" * len(alias.scope) + b" "
        )
    return b"".join(chunks)
