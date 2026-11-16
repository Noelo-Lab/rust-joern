import runpy
import unittest
from importlib.util import find_spec
from pathlib import Path

from rust_joern import parse_code


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "cpp_alias_evidence.py"
evidence = runpy.run_path(str(SCRIPT))
AliasIndex = evidence["AliasIndex"]
Unqualified = evidence["Unqualified"]
render = evidence["render_alias_declarations"]


@unittest.skipUnless(
    find_spec("tree_sitter") and find_spec("tree_sitter_cpp"),
    "optional C++ alias evidence dependencies are missing",
)
class CppAliasEvidenceTests(unittest.TestCase):
    def parse(self, source):
        from tree_sitter import Language, Parser
        import tree_sitter_cpp

        return Parser(Language(tree_sitter_cpp.language())).parse(source).root_node

    def return_type(self, root):
        definition = root.named_children[-1]
        return definition.child_by_field_name("type")

    def test_chain_crosses_linkage_and_reopened_namespace(self):
        source = b"""
extern "C" { typedef unsigned int Word; }
namespace n { using Size = Word; }
namespace n { Size target(Size x) { if (x < 2) x += 3; return x; } }
"""
        root = self.parse(source)
        definition = (
            root.named_children[-1].child_by_field_name("body").named_children[0]
        )
        ty = definition.child_by_field_name("type")
        used = {}
        self.assertEqual(
            AliasIndex(root).resolve(ty, (b"n",), used), (b"int", b"unsigned")
        )
        prefix = render(tuple(used.values()))
        self.assertEqual(
            prefix,
            b"typedef unsigned int Word; namespace n { using Size = Word; } ",
        )
        isolated = prefix + b"namespace n { " + definition.text + b" }"
        original, replay = [
            parse_code(data, "alias.cpp", preprocessed=True, strict=True).raw[
                "functions"
            ]
            for data in (source, isolated)
        ]
        self.assertEqual(len(original), 1)
        self.assertEqual(len(replay), 1)
        self.assertEqual(original[0]["fullname"], "n.target:unsigned int(unsigned int)")
        self.assertEqual(original[0]["fullname"], replay[0]["fullname"])
        self.assertGreater(len(original[0]["cfg"]["nodes"]), 1)
        self.assertEqual(original[0]["cfg"], replay[0]["cfg"])

    def test_shadowing_and_explicit_global_lookup(self):
        source = b"""
typedef int Word;
namespace n { typedef long Word; using Global = ::Word; Global f(Word x) { return x; } }
"""
        root = self.parse(source)
        definition = (
            root.named_children[-1].child_by_field_name("body").named_children[-1]
        )
        index = AliasIndex(root)
        param = (
            definition.child_by_field_name("declarator")
            .child_by_field_name("parameters")
            .named_children[0]
        )
        self.assertEqual(
            index.resolve(param.child_by_field_name("type"), (b"n",), {}), (b"long",)
        )
        self.assertEqual(
            index.resolve(definition.child_by_field_name("type"), (b"n",), {}),
            (b"int",),
        )

    def test_same_spelled_declarations_in_two_namespaces(self):
        source = b"""
namespace left { typedef int Word; }
namespace right { typedef long Word; }
namespace left { Word f(Word x) { return x; } }
"""
        root = self.parse(source)
        method = root.named_children[-1].child_by_field_name("body").named_children[0]
        index = AliasIndex(root)
        used = {}
        self.assertEqual(
            index.resolve(method.child_by_field_name("type"), (b"left",), used),
            (b"int",),
        )
        self.assertIn(b"typedef int Word;", render(tuple(used.values())))
        self.assertNotIn(b"typedef long Word;", render(tuple(used.values())))

    def test_standard_typedef_names_require_declarations(self):
        source = b"typedef unsigned int Word; typedef Word uint32_t; uint32_t f(uint32_t p) { return p; }"
        root = self.parse(source)
        used = {}
        self.assertEqual(
            AliasIndex(root).resolve(self.return_type(root), (), used),
            (b"int", b"unsigned"),
        )
        self.assertEqual(len(used), 2)
        root = self.parse(b"uint32_t f(uint32_t p) { return p; }")
        with self.assertRaises(Unqualified):
            AliasIndex(root).resolve(self.return_type(root), (), {})

    def test_conflicting_redeclaration_clears_partial_evidence(self):
        source = (
            b"typedef int Word; typedef unsigned int Word; Word f(Word x) { return x; }"
        )
        root = self.parse(source)
        used = {}
        with self.assertRaisesRegex(Unqualified, "ambiguous_type_alias"):
            AliasIndex(root).resolve(self.return_type(root), (), used)
        self.assertEqual(used, {})

    def test_unsupported_lookup_is_rejected(self):
        for source in (
            b"namespace n { using Word = int; } using namespace n; Word f(Word n) { return n; }",
            b"using Word = int; namespace { typedef short Word; } Word f(Word n) { return n; }",
            b"using Word = int; inline namespace n { typedef long Word; } Word f(Word n) { return n; }",
            b"using Word = int*; Word f(Word n) { return n; }",
            b"Word f(Word n) { return n; } typedef int Word;",
        ):
            root = self.parse(source)
            function = next(
                n for n in root.named_children if n.type == "function_definition"
            )
            with self.subTest(source=source), self.assertRaises(Unqualified):
                AliasIndex(root).resolve(function.child_by_field_name("type"), (), {})
