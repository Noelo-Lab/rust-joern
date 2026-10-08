"""Directory analysis shares callee resolution without sharing parser state."""

from dataclasses import fields
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import networkx as nx

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/ddg-directory"


def labeled_edges(graph, kind):
    nodes = {
        n["id"]: (n["kind"], n["code"], n["line"], n["column"])
        for n in graph["nodes"]
    }
    if len(set(nodes.values())) != len(nodes):
        raise AssertionError("fixture endpoint identities must be unique")
    return {
        (nodes[e["source"]], nodes[e["target"]], e.get("label", ""))
        for e in graph["edges"] if e["kind"] == kind
    }


def block_identity(attributes):
    block = attributes["node"]
    return (
        block.is_entrypoint,
        block.is_exitpoint,
        tuple((type(statement).__name__, tuple(
            (field.name, getattr(statement, field.name))
            for field in fields(statement) if field.name != "id"
        )) for statement in block.statements),
    )


class DirectoryDataflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.oracle = json.loads((FIXTURES / "joern-4.0.150.json").read_text())["methods"]

    def assert_matches_oracle(self, functions):
        self.assertEqual(len(self.oracle), 2)
        for expected in self.oracle:
            actual = next(f for f in functions if f.fullname == expected["fullname"]
                          and f.filename.name == expected["filename"])
            with self.subTest(method=expected["fullname"]):
                self.assertEqual(labeled_edges(actual.raw["cpg"], "REACHING_DEF"),
                                 labeled_edges(expected["cpg"], "REACHING_DEF"))
                self.assertEqual(labeled_edges(actual.raw["ddg_view"], "DDG"),
                                 labeled_edges(expected["ddg_view"], "DDG"))
                projected = rust_joern.Function(expected).ddg
                self.assertTrue(nx.is_isomorphic(
                    actual.ddg, projected,
                    node_match=lambda a, b: block_identity(a) == block_identity(b),
                ))

    def test_parse_directory_matches_original_raw_and_public_ddg(self):
        # Public analysis reads existing sources and passes them in memory.
        with patch("subprocess.run", side_effect=AssertionError("runtime subprocess")), \
                patch("tempfile.TemporaryDirectory", side_effect=AssertionError("runtime temp file")):
            functions = rust_joern.parse_source(FIXTURES, strict=True, reaching_definitions=True)
        self.assert_matches_oracle(functions.values())
        self.assertTrue(all(isinstance(key, tuple) for key in functions))
        self.assertTrue(all(function.reaching_definitions for function in functions.values()))

    def test_cli_directory_uses_global_method_context(self):
        completed = subprocess.run(
            [str(ROOT / "target/debug/rust-joern"), "analyze", str(FIXTURES),
             "--data-flow", "--reaching-definitions", "--strict"],
            check=True, text=True, capture_output=True,
        )
        self.assert_matches_oracle(rust_joern.Analysis(json.loads(completed.stdout)).functions)

    def test_batch_is_in_memory_and_context_does_not_leak(self):
        caller = (FIXTURES / "caller.c").read_bytes()
        body = (FIXTURES / "body.c").read_text()
        with patch.object(Path, "read_bytes", side_effect=AssertionError("runtime file read")), \
                patch("subprocess.run", side_effect=AssertionError("runtime subprocess")):
            joined = rust_joern.Analysis(rust_joern._analyze_many(
                [(body, "body.c"), (caller, "caller.c")], reaching_definitions=True, strict=True,
            ))
            isolated = rust_joern.Analysis(rust_joern._analyze_many(
                [(caller, "caller.c")], data_flow=True, strict=True,
            ))
            legacy = rust_joern.parse_code(caller, filename="caller.c", data_flow=True, strict=True)
        self.assert_matches_oracle(joined.functions)
        joined_caller = next(f for f in joined.functions if f.name == "caller")
        isolated_caller = next(f for f in isolated.functions if f.name == "caller")
        legacy_caller = next(f for f in legacy.functions if f.name == "caller")
        combined = labeled_edges(joined_caller.raw["cpg"], "REACHING_DEF")
        separate = labeled_edges(isolated_caller.raw["cpg"], "REACHING_DEF")
        self.assertEqual(len(separate - combined), 2)
        self.assertEqual(combined - separate, set())
        self.assertEqual(separate, labeled_edges(legacy_caller.raw["cpg"], "REACHING_DEF"))

    def test_directory_disable_flags_and_native_errors(self):
        functions = rust_joern.parse_source(FIXTURES, no_ddg=True, no_cfg=True, no_ast=True,
                                           no_metadata=True, strict=True)
        self.assertTrue(functions)
        for function in functions.values():
            self.assertIsNone(function.ddg)
            self.assertIsNone(function.ddg_raw)
            self.assertIsNone(function.cfg)
            self.assertIsNone(function.ast)
            self.assertIsNone(function.reaching_definitions)
            self.assertEqual(function.return_type, "")
        with self.assertRaisesRegex(RuntimeError, "not UTF-8"):
            rust_joern._analyze_many([(b"\xff", "invalid.c")])
        with self.assertRaisesRegex(ValueError, "null byte"):
            rust_joern._analyze_many([("", "invalid\0.c")])
        with self.assertRaisesRegex(RuntimeError, "unsupported language"):
            rust_joern._analyze_many([("", "input.c")], language="rust")
        with tempfile.TemporaryDirectory() as directory:
            with patch.object(rust_joern, "_library", side_effect=AssertionError("empty directory needs no library")):
                self.assertEqual(rust_joern.parse_source(directory), {})

    def test_preprocessed_and_strict_options_apply_to_every_file(self):
        source = "#define value 7\nint f(void) { return value; }\n"
        with self.assertRaisesRegex(RuntimeError, "macro"):
            rust_joern._analyze_many([(source, "raw.c")], strict=True)
        for options, filename in (({}, "prepared.i"), ({"preprocessed": True}, "raw.c")):
            with self.subTest(filename=filename):
                result = rust_joern._analyze_many([(source, filename)], strict=True, **options)
                self.assertEqual(result["diagnostics"], [])
                self.assertEqual(result["functions"][0]["start_line"], 2)


if __name__ == "__main__":
    unittest.main()
