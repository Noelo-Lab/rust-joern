"""Offline CFG validation must retain roles and reject impossible metadata."""

import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
from check_cfg_views import roundtrip
from compare_pyjoern import candidate_functions, compare


@unittest.skipUnless(importlib.util.find_spec("decbench"), "local DecBench required")
class CfgViewTests(unittest.TestCase):
    def test_decbench_roundtrip_keeps_cycles_multiple_entries_and_statement_presence(self):
        with patch("pyjoern.parse_source", side_effect=AssertionError("native parsing is forbidden")):
            roundtrip(((8, 4, 9), ((8, 4), (4, 4), (4, 9)), (8, 4), (9,), False))
            roundtrip(((41,), (), (41,), (41,), True))
            roundtrip(((41,), (), (), (), False))

    def test_corrupt_roles_and_degeneracy_fail_instead_of_being_repaired(self):
        for cfg in (((1,), (), (2,), (), False),
                    ((1, 2), ((1, 2),), (1,), (2,), True),
                    ((), (), (), (), False)):
            with self.subTest(cfg=cfg), self.assertRaises(ValueError):
                roundtrip(cfg)

    def test_cached_analysis_replays_exact_selection_and_markers_through_extractor(self):
        import rust_joern
        from decbench.publish.cfg_export import relabel_cfg
        from decbench.utils.cfg import extract_cfgs_from_source

        spec = importlib.util.spec_from_file_location(
            "view_test_shim", Path(__file__).resolve().parents[1] / "compat/pyjoern/__init__.py",
        )
        shim = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(shim)

        def function(name, markers):
            nodes = [{"id": i, "kind": "METHOD_REF", "cfg_nop": marker, "code": name, "line": 1}
                     for i, marker in enumerate(markers)]
            return {"name": name, "fullname": name, "filename": "replay.c", "signature": name,
                    "return_type": "int", "start_line": 1, "end_line": 1, "cpg": {"nodes": nodes, "edges": []},
                    "cfg": {"nodes": [{"id": i, "statements": [i], "is_entrypoint": True,
                                       "is_exitpoint": i == len(nodes) - 1} for i in range(len(nodes))],
                            "edges": [[i, i + 1] for i in range(len(nodes) - 1)]}}

        analysis = {"schema_version": 1, "diagnostics": [], "functions": [
            function("largest", [True]), function("largest", [True, False]), function("largest", [False]),
            function("tie", [True]), function("tie", [False]),
            function("", [True]), function("JUMPOUT_stub", [False]), function("empty_cfg", []),
        ]}
        expected = candidate_functions(analysis)
        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "replay.c"
            source.write_text("int placeholder;\n")
            with patch.object(rust_joern, "_analyze", return_value=analysis) as replay:
                with patch("pyjoern.parse_source", shim.parse_source):
                    graphs = extract_cfgs_from_source(source, raise_on_error=True)
        self.assertEqual(replay.call_count, 1)
        self.assertTrue(replay.call_args.kwargs["strict"])
        self.assertTrue(replay.call_args.kwargs["preprocessed"])
        self.assertFalse(replay.call_args.kwargs["data_flow"])
        self.assertEqual(set(graphs), {"largest", "tie"})
        self.assertEqual(len(graphs["largest"]), 2)
        statement, = next(iter(graphs["tie"])).statements
        self.assertEqual(type(statement).__name__, "Statement")
        self.assertEqual(statement.kind, "METHOD_REF")
        actual = {}
        for name, graph in graphs.items():
            nodes, edges, _, entry, exit_, degenerate = relabel_cfg(graph)
            actual[name] = {"cfg": {"nodes": nodes, "edges": edges, "entry": entry,
                                    "exit": exit_, "degenerate": degenerate}}
        self.assertTrue(all(row["status"] == "match" for row in compare(expected, actual)))


if __name__ == "__main__":
    unittest.main()
