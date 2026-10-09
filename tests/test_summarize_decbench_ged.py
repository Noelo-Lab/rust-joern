"""GED report projection must preserve rows and DecBench denominator semantics."""
import gzip
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from summarize_decbench_ged import (astra_sync, audit_native_run, build_reports, certify_astra_sync, function_index, membership,
    preservation_digest, process_tree, read_overlay, refresh_sample_ged, validate_manifest)


@unittest.skipUnless(importlib.util.find_spec("decbench"), "DecBench checkout required")
class GedSummaryTests(unittest.TestCase):
    def fixture(self):
        from decbench.models.function_data import FunctionData
        decs = ["angr", "codex", "codex@gpt-6-astra", "retdec"]
        functions = []
        for i, name in enumerate(("first", "second")):
            functions.append({"function": name, "values": {dec: {"ged": float(i * 3), "type_match": 0.25, "byte_match": 1.0} for dec in decs},
                "perfects": {dec: {"ged": not i, "type_match": False, "byte_match": True} for dec in decs},
                "distances": {dec: {"ged": float(i * 3), "type_match": 3.0, "byte_match": 0.0} for dec in decs},
                "decompiled": {dec: not (i and dec == "retdec") for dec in decs},
                "compiles": {dec: True for dec in decs}, "labels": ["frozen"], "datasets": ["unoptimized", "sample-set"]})
        return FunctionData.model_validate({"decompilers": decs, "metrics": ["ged", "type_match", "byte_match"],
            "groups": [{"opt_level": "O0", "project": "project", "binary": "binary", "labels": ["original"], "functions": functions}],
            "dataset_presets": [{"name": "unoptimized"}, {"name": "optimized"}, {"name": "sample-set"}],
            "samples": [{"project": "project", "opt_level": "O0", "binary": "binary", "function": "first",
                "difficulty": "easy", "source_code": "preserve source", "decompiled": {"angr": "preserve C"},
                "values": {"angr": {"ged": 99.0, "type_match": 0.75}}, "perfects": {"angr": {"ged": False, "type_match": False}}}],
            "cost_info": {"do_not_rebuild": 12.4}, "dataset_info": {"retain": "original"},
            "compile_rates": {"angr": 1.0}})

    def test_evaluated_empty_slice_clears_only_ged_and_preserves_baseline(self):
        from decbench.results_store import update_ged
        before = self.fixture()
        after = before.model_copy(deep=True)
        digest = preservation_digest(before)
        update_ged(after, {}, covered={("O0", "project", "binary", "angr")})
        self.assertIn("ged", before.groups[0].functions[0].values["angr"])
        for function in after.groups[0].functions:
            self.assertNotIn("ged", function.values["angr"])
            self.assertNotIn("ged", function.perfects["angr"])
            self.assertNotIn("ged", function.distances["angr"])
            self.assertIn("ged", function.values["retdec"])
            self.assertEqual(function.values["angr"]["type_match"], 0.25)
        refresh_sample_ged(after)
        self.assertNotIn("ged", after.samples[0].values["angr"])
        self.assertEqual(after.samples[0].values["angr"]["type_match"], 0.75)
        self.assertEqual(after.samples[0].source_code, "preserve source")
        self.assertEqual(preservation_digest(after), digest)
        self.assertEqual(membership(before), membership(after))

    def test_sample_extras_ged_is_refreshed_from_authoritative_groups(self):
        fd = self.fixture()
        original = preservation_digest(fd)
        counts = refresh_sample_ged(fd)
        self.assertGreater(counts["changed"], 0)
        sample = fd.samples[0]
        self.assertEqual(sample.values["angr"]["ged"], 0.0)
        self.assertTrue(sample.perfects["angr"]["ged"])
        self.assertEqual(sample.values["codex"]["ged"], 0.0)
        self.assertEqual(sample.values["angr"]["type_match"], 0.75)
        self.assertEqual(preservation_digest(fd), original)

    def test_real_site_hiding_and_version_overrides_control_normalization(self):
        reports = build_reports(self.fixture(), "test")
        raw = reports["all_identity_aggregates"]["combos"]["unoptimized|1"]
        site = reports["site_aggregates"]["combos"]["unoptimized|1"]
        self.assertEqual(raw["functions"], 1)  # Hidden retdec failed one function.
        self.assertEqual(site["functions"], 2)
        self.assertNotIn("retdec", reports["site_aggregates"]["decompilers"])
        rows = {(r["preset"], r["normalize"], r["identity"]): r for r in reports["site_ged"]}
        self.assertFalse(rows["unoptimized", False, "codex"]["shown_on_preset"])
        self.assertFalse(rows["sample-set", False, "codex@gpt-6-astra"]["shown_on_preset"])
        self.assertTrue(rows["optimized", False, "codex@gpt-6-astra"]["shown_on_preset"])
        self.assertEqual(rows["sample-set", False, "codex"]["denominator"], 2)

    def test_manifest_and_exact_astra_identity_rekeying_are_verified(self):
        from decbench.models.function_data import FunctionData
        main = self.fixture()
        standalone = FunctionData.model_validate({"decompilers": ["codex@gpt-6-astra"], "metrics": main.metrics,
            "groups": [{"opt_level": "O0", "project": "project", "binary": "binary", "functions": [
                {"function": f.function, "datasets": f.datasets,
                 "values": {"codex@gpt-6-astra": f.values["codex"]},
                 "perfects": {"codex@gpt-6-astra": f.perfects["codex"]},
                 "distances": {"codex@gpt-6-astra": f.distances["codex"]}} for f in main.groups[0].functions]}]})
        with tempfile.TemporaryDirectory() as directory:
            manifest = Path(directory) / "sample_set_manifest.json"
            raw = {"functions": [{"project": "project", "opt": "O0", "binary": "binary", "function": n} for n in ("first", "second")]}
            manifest.write_text(json.dumps(raw))
            self.assertEqual(validate_manifest(main, manifest)["sample_set_count"], 2)
            raw["functions"].pop()
            manifest.write_text(json.dumps(raw))
            with self.assertRaisesRegex(ValueError, "differs"):
                validate_manifest(main, manifest)
        report = astra_sync(main, main, standalone, standalone)
        self.assertTrue(report["new"]["all_target_states_equal"])
        changed = standalone.model_copy(deep=True)
        changed.groups[0].functions[1].values["codex@gpt-6-astra"]["ged"] = 7.0
        report = astra_sync(main, main, standalone, changed)
        self.assertFalse(report["new"]["all_target_states_equal"])
        self.assertEqual(report["new"]["counts"]["different"], 1)

    def test_pipeline_emits_all_rows_changes_and_both_canonical_aggregate_views(self):
        fd = self.fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            baseline, overlay, slices = (root / name for name in ("baseline.json", "ged_new.json", "ged_new.slices.json"))
            baseline.write_text(fd.model_dump_json())
            original_bytes = baseline.read_bytes()
            overlay.write_text(json.dumps({"O0::project::binary::angr::first": {"value": 2.0, "perfect": False}}))
            slices.write_text(json.dumps(["O0::project::binary::angr"]))
            before, after, report = process_tree("fixture", baseline, overlay, slices, root / "report")
            self.assertEqual(baseline.read_bytes(), original_bytes)
            self.assertTrue(report["preservation"]["all_non_ged_facts_equal"])
            self.assertEqual(report["overlay"]["applied_entries"], 1)
            self.assertEqual(report["raw_changes"]["totals"]["changed"], 1)
            self.assertEqual(report["raw_changes"]["totals"]["dropped"], 1)
            rows = [json.loads(line) for line in gzip.decompress((root / "report/per-function-diff.jsonl.gz").read_bytes()).splitlines()]
            self.assertEqual(len(rows), 8)
            self.assertEqual({r["key"] for r in rows}, {f"O0::project::binary::{d}::{f}" for d in fd.decompilers for f in ("first", "second")})
            self.assertEqual(function_index(after)["O0", "project", "binary", "first"].values["angr"]["ged"], 2.0)
            self.assertTrue((root / "report/new.site_aggregates.json.gz").is_file())
            self.assertTrue((root / "report/baseline.all_identity_aggregates.json.gz").is_file())

    def test_disagreeing_astra_aliases_cannot_certify_completed_reports(self):
        differing = ["O0", "project", "binary", "second"]
        sync = {"baseline": {"all_target_states_equal": True},
                "new": {"all_target_states_equal": False, "rows": [{"key": differing, "equal": False}]}}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            summaries = {name: {"provisional": False} for name in ("main", "sample")}
            for name in summaries:
                (output / name).mkdir()
                for artifact in ("scoreboard.json", "site_scoreboard.json", "all_identity_aggregates.json.gz", "site_aggregates.json.gz"):
                    data = b'{"provisional": false}'
                    (output / name / ("new." + artifact)).write_bytes(gzip.compress(data) if artifact.endswith(".gz") else data)
            self.assertFalse(certify_astra_sync(sync, summaries, output))
            for name, summary in summaries.items():
                self.assertTrue(summary["provisional"])
                self.assertEqual(summary["certification_errors"][0]["keys"], [differing])
                for artifact in ("scoreboard.json", "site_scoreboard.json", "all_identity_aggregates.json.gz", "site_aggregates.json.gz"):
                    data = (output / name / ("new." + artifact)).read_bytes()
                    self.assertTrue(json.loads(gzip.decompress(data) if artifact.endswith(".gz") else data)["provisional"])
            sync["baseline"]["all_target_states_equal"] = False
            with self.assertRaisesRegex(ValueError, "baseline scores disagree"):
                certify_astra_sync(sync, summaries, output)

    def test_invalid_overlay_values_do_not_enter_the_report(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            overlay, slices = root / "ged_new.json", root / "slices.json"
            slices.write_text('["O0::project::binary::angr"]')
            for rec in ({"value": -1}, {"value": float("inf")}, {"value": 1, "perfect": True}):
                overlay.write_text(json.dumps({"O0::project::binary::angr::first": rec}))
                with self.assertRaises(ValueError):
                    read_overlay(overlay, slices, self.fixture())

    def test_parse_errors_and_missing_slices_keep_aggregate_results_provisional(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            tree = root / "main"
            tree.mkdir()
            source_record, candidate_record = root / "source.json", tree / "candidate.json"
            source_record.write_text(json.dumps({"status": "ok"}))
            candidate_record.write_text(json.dumps({"status": "error", "error": "strict recovery failed", "excluded": {}}))
            key = "O0::project::binary::angr"
            manifest = {"trees": {"main": {"artifacts": [{"key": key, "record": str(candidate_record)}]}},
                        "source_inputs": [{"units": {"input": {"key": "content", "record": str(source_record)}}}]}
            (root / "manifest.json").write_text(json.dumps(manifest))
            (root / "completed.json").write_text('{"code_unchanged": true}')
            overlay = tree / "ged_new.json"
            overlay.write_text("{}")
            (tree / "ged_new.slices.json").write_text(json.dumps([key]))
            report = audit_native_run(overlay)
            self.assertTrue(report["execution_completed"])
            self.assertTrue(report["provisional"])
            self.assertEqual(report["parse_errors"][0]["error"], "strict recovery failed")
            candidate_record.write_text(json.dumps({"status": "ok", "excluded": {}}))
            self.assertFalse(audit_native_run(overlay)["provisional"])
            (tree / "ged_new.slices.json").write_text("[]")
            self.assertEqual(audit_native_run(overlay)["missing_evaluated_slices"], [key])
            self.assertTrue(audit_native_run(overlay)["provisional"])


if __name__ == "__main__":
    unittest.main()
