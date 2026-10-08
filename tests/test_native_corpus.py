"""Corpus triage keeps strict failures, exact roles and immutable checkpoints."""

import gzip
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import compare_native_corpus as corpus


class NativeCorpusTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "input.c"
        self.source.write_text("int body(void) { return 0; }\n")
        self.reference = self.root / "reference.json"
        self.reference.write_text(json.dumps({
            "prepared_sha256": corpus.digest(self.source.read_bytes()),
            "functions": {"body": {"cfg": {
                "nodes": [0, 1], "edges": [[0, 1]], "entry": [0], "exit": [1], "degenerate": False,
            }}},
        }))
        self.config = {"output": str(self.root / "output"), "run_fingerprint": "frozen-run",
                       "manifest": "manifest.json", "selected_unique_inputs": 1,
                       "native_library": {"path": "immutable.so", "sha256": "binary-sha"},
                       "source_build_fingerprint": "snapshot-sha", "build_provenance": {},
                       "preprocessed_option_supported": True,
                       "requested_options": {"strict": True, "data_flow": False, "reaching_definitions": False, "preprocessed": True},
                       "worker_config": {"workers": 1, "rayon_threads": 2}}
        self.job = {"input_id": "c-" + corpus.digest(self.source.read_bytes()), "language": "c",
                    "input": corpus.freeze(self.source), "case_ids": ["first", "second"],
                    "references": [{"artifact": corpus.freeze(self.reference), "case_ids": ["first", "second"]}]}
        self.analysis = {"diagnostics": [{"severity": "error", "message": "unsupported executable syntax"}],
                         "functions": [{"name": "body", "fullname": "body", "filename": "input.c",
                             "start_line": 1, "end_line": 1,
                             "cpg": {"nodes": [{"id": 7, "kind": "METHOD_REF"}, {"id": 9, "kind": "CALL"}]},
                             "cfg": {"nodes": [
                                 {"id": 7, "statements": [7], "is_entrypoint": True, "is_exitpoint": False},
                                 {"id": 9, "statements": [9], "is_entrypoint": False, "is_exitpoint": True}],
                                 "edges": [[7, 9]]}}]}
        self.previous_state = corpus.STATE.copy()
        corpus.STATE.update(config=self.config, guards=[])
        self.addCleanup(lambda: (corpus.STATE.clear(), corpus.STATE.update(self.previous_state)))

    def run_input(self):
        # This tests the harness decision, not parser recovery: the strict ABI
        # result is an error, and a distinct permissive request returns graphs.
        with patch.object(corpus, "native", side_effect=[
            (None, "unsupported executable syntax", .1), (self.analysis, None, .2),
        ]) as native:
            result = corpus.process_input(self.job)
        self.assertEqual([call.args[2]["strict"] for call in native.call_args_list], [True, False])
        self.assertTrue(all(call.args[2]["preprocessed"] for call in native.call_args_list))
        return result

    def test_topology_match_does_not_erase_strict_error_or_duplicate_measurement(self):
        result = self.run_input()
        self.assertEqual(result["status"], "parse_error")
        self.assertEqual(result["native_invocations"], 2)
        self.assertEqual(result["timings"]["native_seconds"], .1)
        self.assertEqual(result["fallback_timings"]["native_seconds"], .2)
        comparison = next(iter(result["comparisons"].values()))
        self.assertEqual(comparison["topology_status"], "match")
        self.assertEqual(comparison["status"], "usage_error")
        with gzip.open(result["failure_record"], "rt") as stream:
            failures = [json.loads(line) for line in stream]
        self.assertEqual(len(failures), 1)
        self.assertEqual(failures[0]["case_ids"], ["first", "second"])
        cases = [{"id": name, "optimization": "O0", "project": "test", "kind": "ida"}
                 for name in ("first", "second")]
        summary = corpus.make_summary(self.config, cases, {result["input_id"]: result}, 2, 1.0, 0)
        self.assertTrue(summary["complete_corpus"])
        self.assertEqual(summary["file_statuses"], {"usage_error": 2})
        self.assertEqual(summary["function_statuses"], {"match": 2})
        self.assertEqual(summary["timings_unique_inputs_total"]["native_seconds"], .1)
        self.assertEqual(summary["fallback_timings_unique_inputs_total"]["native_seconds"], .2)

    def test_role_change_is_recorded_even_when_nodes_and_edges_match(self):
        self.analysis["diagnostics"] = []
        self.analysis["functions"][0]["cfg"]["nodes"][1]["is_entrypoint"] = True
        result = self.run_input()
        comparison = next(iter(result["comparisons"].values()))
        self.assertEqual(comparison["function_statuses"], {"divergent": 1})
        with gzip.open(result["failure_record"], "rt") as stream:
            rows = [json.loads(line) for line in stream]
        self.assertEqual(rows[-1]["type"], "function")
        self.assertEqual(rows[-1]["candidate_graph"]["entry"], [7, 9])

    def test_input_or_reference_mutation_aborts_without_native_analysis(self):
        for path in (self.source, self.reference):
            with self.subTest(path=path):
                path.write_bytes(path.read_bytes() + b" ")
                if path == self.source:
                    with patch.object(corpus, "native") as native:
                        with self.assertRaises(corpus.FingerprintMismatch):
                            corpus.process_input(self.job)
                        native.assert_not_called()
                    self.job["input"] = corpus.freeze(self.source)
                else:
                    with patch.object(corpus, "native", return_value=(self.analysis, None, .1)):
                        with self.assertRaises(corpus.FingerprintMismatch):
                            corpus.process_input(self.job)

    def test_resume_rejects_different_run_or_changed_candidate(self):
        result = self.run_input()
        self.assertEqual(corpus.resume_result(self.job, self.config), result)
        with self.assertRaises(corpus.FingerprintMismatch):
            corpus.resume_result(self.job, self.config | {"run_fingerprint": "different-binary"})
        Path(result["input_record"]).write_bytes(b"corrupted candidate")
        with self.assertRaises(corpus.FingerprintMismatch):
            corpus.resume_result(self.job, self.config)


if __name__ == "__main__":
    unittest.main()
