"""Exact DecBench preparation, strict failure reporting, and TU ownership."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
DECBENCH = ROOT.parent / "decbench"
HAS_DECBENCH = importlib.util.find_spec("decbench") is not None


@unittest.skipUnless(HAS_DECBENCH and (DECBENCH / "decbench/utils/cfg.py").is_file(), "local DecBench required")
class DecBenchUsageTests(unittest.TestCase):
    def check(self, source: str, prepared: str, *, kind: str = "source", suffix: str = ".i"):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        path = root / f"input{suffix}"
        path.write_text(source, encoding="utf-8")
        case = {"id": f"O0/test/{kind}/{path.name}", "optimization": "O0", "project": "test",
                "kind": kind, "source": str(path), "prepared_sha256": hashlib.sha256(prepared.encode()).hexdigest(),
                "metric_excluded": None}
        manifest = root / "manifest.json"
        manifest.write_text(json.dumps({"schema_version": 1, "decbench_root": str(DECBENCH), "cases": [case]}))
        result = subprocess.run(
            [sys.executable, str(ROOT / "scripts/check_decbench_usage.py"), "--manifest", str(manifest),
             "--output", str(root / "output"), "--workers", "1"],
            capture_output=True, text=True, check=False,
        )
        record = json.loads((root / "output" / f"{case['id']}.json").read_text())
        summary = json.loads((root / "output/summary.json").read_text())
        return result, record, summary

    def test_exact_preparation_and_timed_in_memory_graphs(self):
        prepared = "int body(int x) { return x + 1; }\n"
        source = '# 1 "/usr/include/stdlib.h"\nint header_only(void);\n# 1 "project.c"\n' + prepared
        result, record, summary = self.check(source, prepared)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(record["status"], "ok")
        self.assertEqual(set(record["functions"]), {"body"})
        self.assertTrue(record["prepared_matches_manifest"])
        options = {"strict": True, "data_flow": False, "preprocessed": True}
        self.assertEqual(record["requested_options"], options)
        self.assertEqual(summary["required_candidate_options"], options)
        self.assertGreater(record["timings"]["graph_materialization_seconds"], 0)
        self.assertGreater(record["timings"]["end_to_end_seconds"], record["timings"]["native_seconds"])
        self.assertFalse(record["functions"]["body"]["cfg"]["degenerate"])
        self.assertTrue(summary["complete"])
        self.assertEqual(summary["native_library"]["sha256"], record["native_library_sha256"])
        self.assertEqual(summary["worker_config"]["start_method"], "spawn")

    def test_strict_failure_keeps_secondary_graphs_and_diagnostics(self):
        prepared = 'int f(void) { co_await work(); return 0; }\n'
        source = '# 1 "/usr/include/system.hpp"\nint header_only(void);\n# 1 "project.cpp"\n' + prepared
        result, record, _ = self.check(source, prepared, suffix=".ii")
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(record["status"], "parse_error")
        self.assertIn("coroutine", record["error"])
        self.assertTrue(record["prepared_matches_manifest"])
        self.assertEqual(record["parse_suffix"], ".cpp")
        self.assertEqual(record["requested_options"], {"strict": True, "data_flow": False, "preprocessed": True})
        self.assertIn("f", record["functions"])
        self.assertNotIn("header_only", record["functions"])
        self.assertTrue(record["diagnostics"])
        self.assertTrue(any("coroutine" in diagnostic["message"] for diagnostic in record["diagnostics"]))
        self.assertEqual(record["diagnostics_source"], "permissive_fallback")
        self.assertGreater(record["fallback_timings"]["end_to_end_seconds"], 0)

    def test_native_graphs_preserve_decbench_source_owner_selection(self):
        import rust_joern
        from decbench.metrics.ged import _is_isomorphic
        from decbench.publish.cfg_export import rebuild_cfg
        from decbench.utils.cfg import best_source_by_name, resolved_source_for_binary

        frozen = json.loads((ROOT / "tests/fixtures/control.pyjoern.json").read_text())["functions"]
        reference = {
            "one": {"shared": rebuild_cfg(frozen["empty_function"]["cfg"]), "own": rebuild_cfg(frozen["straight"]["cfg"])},
            "two": {"shared": rebuild_cfg(frozen["straight"]["cfg"]), "own": rebuild_cfg(frozen["branch"]["cfg"])},
        }
        sources = {
            "one": "int shared(int x); int own(int x) { x += 2; return x; }",
            "two": "int shared(int x) { x += 2; return x; } int own(int x) { if(x) return 1; return 0; }",
        }
        native = {stem: {function.name: function.cfg for function in rust_joern.parse_code(source).functions}
                  for stem, source in sources.items()}
        owners = {1: ("own", "two"), 2: ("shared", "one")}
        for source_maps in (reference, native):
            best = best_source_by_name(source_maps)
            self.assertEqual(len(best["own"]), 3)
            self.assertEqual(len(resolved_source_for_binary("one", source_maps, best)["own"]), 1)
        left = resolved_source_for_binary("one", reference, best_source_by_name(reference), owners)
        right = resolved_source_for_binary("one", native, best_source_by_name(native), owners)
        self.assertEqual(set(left), set(right))
        self.assertNotIn("shared", right)
        self.assertEqual(len(right["own"]), 3)
        self.assertTrue(all(_is_isomorphic(left[name], right[name]) for name in left))


if __name__ == "__main__":
    unittest.main()
