"""Native ABI and the graph contract consumed by DecBench."""

import os
import importlib.util
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import rust_joern


class PythonApiTests(unittest.TestCase):
    @staticmethod
    def _shim():
        spec = importlib.util.spec_from_file_location(
            "decbench_shim", Path(__file__).resolve().parents[1] / "compat/pyjoern/__init__.py"
        )
        shim = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(shim)
        return shim

    def test_branch_loop_in_memory(self):
        analysis = rust_joern.parse_code(
            "int f(int x) { while (x > 0) { if (x == 2) break; x--; } return x; }"
        )
        function, = analysis.functions
        self.assertEqual(function.name, "f")
        self.assertGreater(function.cfg.number_of_nodes(), 1)
        self.assertTrue(any(node.is_entrypoint for node in function.cfg))
        self.assertTrue(any(node.is_exitpoint for node in function.cfg))
        self.assertTrue(any(len(list(function.cfg.successors(node))) > 1 for node in function.cfg))
        self.assertIsNone(function.ddg)
        self.assertIn("schema_version", analysis.to_json())

    def test_decbench_straight_line_and_prototype_contract(self):
        analysis = rust_joern.parse_code("int empty(int x); int body(int x) { return x + 1; }")
        body = next(function for function in analysis.functions if function.name == "body")
        self.assertEqual(len(body.cfg), 1)
        self.assertTrue(any(type(statement).__name__ != "Nop" for node in body.cfg for statement in node.statements))
        for function in analysis.functions:
            if function.name == "empty":
                self.assertTrue(all(type(statement).__name__ == "Nop" for node in function.cfg for statement in node.statements))

    def test_optional_data_flow(self):
        analysis = rust_joern.parse_code("int f(int x) { int y = x; return y; }", reaching_definitions=True)
        function, = analysis.functions
        self.assertIsNotNone(function.ddg)
        self.assertTrue(function.ddg.number_of_edges())
        self.assertTrue(function.reaching_definitions)

    def test_parse_source_and_disable_flags(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "example.c"
            path.write_text("int f(void) { return 1; }", encoding="utf-8")
            functions = rust_joern.parse_source(path, no_ddg=True, no_ast=True)
            self.assertIsNotNone(functions["f"].cfg)
            self.assertIsNone(functions["f"].ddg)
            self.assertIsNone(functions["f"].ast)
            functions = rust_joern.parse_source(path, no_cfg=True, no_ddg=True)
            self.assertIsNone(functions["f"].cfg)
            self.assertIn(("f", str(path)), rust_joern.parse_source(directory, no_ddg=True))

    def test_missing_library_is_actionable(self):
        with patch.dict(os.environ, {"RUST_JOERN_LIBRARY": "/nonexistent/librust_joern.so"}):
            with self.assertRaisesRegex(RuntimeError, "cargo build --release"):
                rust_joern.parse_code("int f(void) { return 0; }")

    def test_decbench_shim_skips_only_default_data_flow(self):
        shim = self._shim()
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "example.c"
            path.write_text("int f(int x) { return x; }", encoding="utf-8")
            self.assertIsNone(shim.parse_source(path)["f"].ddg)
            self.assertIsNotNone(shim.parse_source(path, no_ddg=False)["f"].ddg)

    def test_decbench_shim_rejects_unverified_executable_syntax(self):
        source = 'int f(void) { co_await work(); return 0; }'
        analysis = rust_joern.parse_code(source, filename="example.cpp")
        self.assertTrue(analysis.diagnostics)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "example.cpp"
            path.write_text(source, encoding="utf-8")
            shim = self._shim()
            with self.assertRaises(RuntimeError):
                shim.parse_source(path)
            self.assertIn("f", shim.parse_source(path, strict=False))

    def test_preprocessed_macros_are_explicit_and_keep_source_locations(self):
        source = '#define value 7\nint f(void) { return value; }\n'
        with self.assertRaisesRegex(RuntimeError, "macro"):
            rust_joern.parse_code(source, strict=True)
        analysis = rust_joern.parse_code(source, strict=True, preprocessed=True)
        function, = analysis.functions
        self.assertEqual(function.start_line, 2)
        self.assertFalse(analysis.diagnostics)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "prepared.i"
            path.write_text(source, encoding="utf-8")
            self.assertIn("f", rust_joern.parse_source(path, strict=True, no_ddg=True))

    def test_cfg_method_ref_marker_override_preserves_cpg_identity(self):
        for marker, expected in ((None, "Nop"), (False, "Statement"), (True, "Nop")):
            with self.subTest(marker=marker):
                node = {"id": 0, "kind": "METHOD_REF", "code": "f", "line": 1}
                if marker is not None:
                    node["cfg_nop"] = marker
                graph = rust_joern.Function._cfg(
                    {"nodes": [{"id": 0, "statements": [0], "is_entrypoint": True, "is_exitpoint": False}], "edges": []},
                    [node],
                )
                statement, = next(iter(graph)).statements
                self.assertEqual(type(statement).__name__, expected)
                self.assertEqual(statement.kind, "METHOD_REF")


if __name__ == "__main__":
    unittest.main()
