"""Original-Joern control for out-of-line nested record definitions."""

import hashlib
import json
from pathlib import Path
import runpy
import unittest

from rust_joern import parse_code


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "tests/fixtures/cpp-qualified-records.cpp"
COMPARISON = runpy.run_path(str(ROOT / "scripts/compare_pyjoern.py"))


class QualifiedRecordReferenceTest(unittest.TestCase):
    def test_nested_method_coverage_and_directed_cfg_match_original(self):
        source = SOURCE.read_bytes()
        reference = json.loads(SOURCE.with_suffix(".pyjoern.json").read_bytes())
        digest = hashlib.sha256(source).hexdigest()
        self.assertEqual(reference["source_sha256"], digest)
        self.assertEqual(reference["prepared_sha256"], digest)
        self.assertEqual(reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
        result = parse_code(
            source, SOURCE, language="cpp", preprocessed=True, strict=True,
            joern_loop_conditions=True, joern_knr_parameters=True,
            joern_cdt_failures=True, joern_cpp_control_flow=True,
        )
        actual = COMPARISON["candidate_functions"](result.raw)
        self.assertEqual(set(actual), {"read", "after"})
        self.assertEqual(set(actual), set(reference["functions"]))
        self.assertEqual(
            (actual["read"]["start_line"], actual["read"]["end_line"]), (10, 13)
        )
        for row in COMPARISON["compare"](reference["functions"], actual):
            self.assertEqual(row["status"], "match", row)
