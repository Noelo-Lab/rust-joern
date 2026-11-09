"""Native ABI and the graph contract consumed by DecBench."""

import os
import importlib.util
import json
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
        self.assertIsNone(function.ddg_projection)
        self.assertIn("schema_version", analysis.to_json())

    def test_native_decoder_preserves_full_graph_data_and_errors(self):
        source = 'int f(int x) { const char *s = "雪🐍\\n\\t"; int y = x; return y; }'
        decoded = rust_joern.parse_code(source, filename="雪.c", reaching_definitions=True)
        with patch.object(rust_joern, "_loads", json.loads):
            standard = rust_joern.parse_code(source, filename="雪.c", reaching_definitions=True)
        self.assertEqual(decoded.raw, standard.raw)
        self.assertEqual(json.loads(decoded.to_json()), decoded.raw)
        for loads in (rust_joern._loads, json.loads):
            with self.subTest(decoder=loads.__module__), patch.object(rust_joern, "_loads", loads):
                with self.assertRaisesRegex(RuntimeError, "coroutine"):
                    rust_joern.parse_code("int f() { co_await work(); return 0; }", filename="f.cpp", strict=True)

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
        self.assertTrue(all(isinstance(block, rust_joern.Block) for block in function.ddg))
        self.assertTrue(function.reaching_definitions)
        self.assertIsNotNone(function.ddg_projection)
        self.assertTrue(set(function.ddg_projection).issubset(function.cpg))
        self.assertTrue(all(edge["kind"] == "REACHING_DEF" for _, _, edge in function.ddg_raw.edges(data=True)))
        self.assertTrue(all(edge["kind"] == "DDG" for _, _, edge in function.ddg_projection.edges(data=True)))
        assignment = next(node for node, attrs in function.ddg_projection.nodes(data=True)
                          if attrs.get("name") == "<operator>.assignment")
        method_return = next(node for node, attrs in function.ddg_projection.nodes(data=True)
                             if attrs["kind"] == "METHOD_RETURN")
        self.assertEqual({edge["label"] for edge in function.ddg_projection[assignment][method_return].values()},
                         {"x", "y", "y = x"})

    def test_optional_ddg_projection_preserves_raw_graph_and_export(self):
        data = {"name": "f", "fullname": "f", "filename": "input.c", "return_type": "int",
                "signature": "int f()", "start_line": 1, "end_line": 1,
                "cpg": {"nodes": [], "edges": []}, "cfg": {"nodes": [], "edges": []},
                "ddg": {"nodes": [{"id": 0, "kind": "METHOD", "code": "f"}], "edges": []}}
        self.assertIsNone(rust_joern.Function(data).ddg_projection)
        projection = {"nodes": [{"id": 0, "kind": "METHOD", "code": "f"},
                                {"id": 1, "kind": "CALL", "code": "sink(x)"}],
                      "edges": [{"source": 0, "target": 1, "kind": "DDG", "label": "x"},
                                {"source": 0, "target": 1, "kind": "DDG", "label": "y"}]}
        data["ddg_projection"] = projection
        analysis = rust_joern.Analysis({"schema_version": 1, "diagnostics": [], "functions": [data]})
        function, = analysis.functions
        self.assertIsNone(function.ddg)
        self.assertEqual(set(function.ddg_raw), {0})
        self.assertEqual(set(function.ddg_projection), {0, 1})
        self.assertEqual(function.ddg_projection.number_of_edges(), 2)
        self.assertEqual({edge["label"] for _, _, edge in function.ddg_projection.edges(data=True)}, {"x", "y"})
        import json
        self.assertEqual(json.loads(analysis.to_json())["functions"][0]["ddg_projection"], projection)
        self.assertIsNone(rust_joern.Function(data, no_ddg=True).ddg_projection)
        self.assertIsNone(rust_joern.Function(data | {"ddg_projection": None}).ddg_projection)

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

    def test_decbench_shim_lowers_constant_true_loop_heads_like_omitted_for_conditions(self):
        import networkx as nx
        shim = self._shim()
        graphs = {}
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "loop.c"
            for head in ("for (;;)", "while (1)", "while (true)"):
                path.write_text(f"int f(int n) {{ {head} {{ if (n) break; n++; }} return n; }}", encoding="utf-8")
                graphs[head] = shim.parse_source(path)["f"].cfg
                graphs[head, "joern"] = shim.parse_source(path, joern_loop_conditions=True)["f"].cfg
        for head in ("while (1)", "while (true)"):
            with self.subTest(head=head):
                self.assertTrue(nx.is_isomorphic(graphs[head], graphs["for (;;)"]))
                self.assertFalse(nx.is_isomorphic(graphs[head, "joern"], graphs["for (;;)"]))
        self.assertTrue(nx.is_isomorphic(graphs["for (;;)", "joern"], graphs["for (;;)"]))

    def test_knr_definitions_keep_every_parameter_unless_joern_parameters_are_requested(self):
        source = "int f(a, b, c) char *c; int a, b; { return a + b + *c; }\nint g(a, b) int a; { return a + b; }\n"

        def parameters(**options):
            analysis = rust_joern.parse_code(source, filename="knr.c", data_flow=True, **options)
            return {function.name: [(node["name"], node["code"]) for node in function.raw["cpg"]["nodes"]
                                    if node["kind"] == "METHOD_PARAMETER_IN"] for function in analysis.functions}

        # Every name of the identifier list, in list order; `b` of `g` is an implicit int.
        self.assertEqual(parameters(), {
            "f": [("a", "int a, b;"), ("b", "int a, b;"), ("c", "char *c;")],
            "g": [("a", "int a;"), ("b", "b")],
        })
        # Joern 4.0.150: one parameter per declaration, in declaration order.
        self.assertEqual(parameters(joern_knr_parameters=True), {
            "f": [("c", "char *c;"), ("a", "int a, b;")],
            "g": [("a", "int a;")],
        })

    def test_cdt_failures_keep_their_code_unless_joern_failures_are_requested(self):
        source = Path(__file__).resolve().parent / "fixtures/ddg-decbench/cdt-failures.c"

        def functions(**options):
            analysis = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True, **options)
            return {function.name: [(node["kind"], node["code"]) for node in function.raw["cpg"]["nodes"]
                                    if node["kind"] in ("CALL", "UNKNOWN", "CONTROL_STRUCTURE")]
                    for function in analysis.functions}

        # A parameter list CDT backtracks on still has its definition, `*` on
        # a literal still has its statement, condition and loop, and a type
        # name stays a type after a declaration CDT reads as its function.
        default = functions()
        self.assertEqual(set(default), {"typed_twice", "named", "literal_dereference", "ordinary", "pointer_local"})
        self.assertEqual(default["pointer_local"], [("CALL", "later = &a"), ("CALL", "&a")])
        self.assertIn(("CALL", "rdi = arg1"), default["typed_twice"])
        self.assertIn(("CALL", "message = 0"), default["named"])
        for node in (("CALL", "*0x40004000 = a"), ("CALL", "*0x40004004 == a"),
                     ("CONTROL_STRUCTURE", "for (a = *0x40004008;a < 4;a++)"), ("CALL", "a = a + 2")):
            self.assertIn(node, default["literal_dereference"])
        # Joern 4.0.150: no method for either definition; the statement has
        # no nodes, the condition is a problem node, and the loop is dropped.
        # `word_t` is a declared function, so `word_t* later;` multiplies.
        joern = functions(joern_cdt_failures=True)
        self.assertEqual(set(joern), {"literal_dereference", "ordinary", "pointer_local", "word_t"})
        self.assertEqual(joern["pointer_local"][0], ("CALL", "word_t* later"))
        self.assertEqual(joern["literal_dereference"], [
            ("CONTROL_STRUCTURE", "if (*0x40004004 == a)"), ("UNKNOWN", "*0x40004004 == a"), ("CALL", "a = 1"),
            ("CALL", "a < 4"), ("CALL", "a++"),
        ])
        self.assertEqual(joern["ordinary"], default["ordinary"])

    def test_bindings_from_cdt_failures_need_joern_failures(self):
        fixtures = Path(__file__).resolve().parent / "fixtures/ddg-decbench"

        def codes(name, **options):
            source = fixtures / name
            analysis = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True, **options)
            return {function.name: [node["code"] for node in function.raw["cpg"]["nodes"]
                                    if node["kind"] in ("CALL", "CONTROL_STRUCTURE")]
                    for function in analysis.functions}

        # `T* x;` is a product once `x` resolves: a file-scope object declared
        # before, or a function declared anywhere in the file.
        default = codes("leaked-bindings.c")
        for product in ("int32_t* file_object", "int32_t* declared", "int32_t* later_function"):
            self.assertIn(product, default["file_scope"])
        self.assertNotIn("int32_t* later_object", default["file_scope"])
        # Bindings that only a CDT failure makes are left out: the locals of a
        # body it closes early, and a type name it reads as a function.
        self.assertNotIn("int32_t* rax_2", default["after_leak"])
        self.assertNotIn("int64_t* rbx_1", default["after_external"])
        self.assertNotIn("callback_t* early", default["before_function"])
        joern = codes("leaked-bindings.c", joern_cdt_failures=True)
        for product in ("int32_t* rax_2", "int32_t** result", "int32_t* counts"):
            self.assertIn(product, joern["after_leak"])
        self.assertNotIn("int32_t* nested", joern["after_leak"])
        self.assertIn("int64_t* rbx_1", joern["after_external"])
        self.assertIn("callback_t* early", joern["before_function"])
        self.assertEqual(joern["file_scope"], default["file_scope"])

        # A literal after a complete operand ends the statement in both modes.
        # CDT's missing semicolon also fails the enclosing loop, and its skip
        # over an unclosed literal ends with the next brace group.
        default = codes("adjacent-literals.c")
        joern = codes("adjacent-literals.c", joern_cdt_failures=True)
        self.assertEqual(joern["messages"], default["messages"])
        self.assertIn('format = "list entry "%.100s', default["messages"])
        self.assertIn("while (arg1)", default["enclosed"])
        self.assertNotIn("while (arg1)", joern["enclosed"])
        self.assertIn('format = "retry "%d', joern["enclosed"])
        self.assertNotIn("arg1 = arg1 + 3", default["unclosed"])
        self.assertIn("arg1 = arg1 + 3", joern["unclosed"])

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
