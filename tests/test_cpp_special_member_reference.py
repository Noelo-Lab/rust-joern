"""Compare handwritten special-member controls with original Joern CFGs."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/special-member-recovery"
SPEC = importlib.util.spec_from_file_location(
    "comparison", ROOT / "scripts/compare_pyjoern.py"
)
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class SpecialMemberReferenceTests(unittest.TestCase):
    def test_handwritten_inputs_match_original_cfgs(self):
        for source in sorted(FIXTURES.glob("*.cpp")):
            with self.subTest(source=source.name):
                reference = json.loads(source.with_suffix(".pyjoern.json").read_bytes())
                digest = hashlib.sha256(source.read_bytes()).hexdigest()
                self.assertEqual(reference["source_sha256"], digest)
                self.assertEqual(reference["prepared_sha256"], digest)
                self.assertEqual(
                    reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150"
                )
                result = rust_joern.parse_code(
                    source.read_bytes(),
                    filename=source,
                    strict=True,
                    preprocessed=True,
                    joern_loop_conditions=True,
                )
                actual = COMPARISON.candidate_functions(result.raw)
                self.assertEqual(set(actual), set(reference["functions"]))
                for row in COMPARISON.compare(reference["functions"], actual):
                    self.assertEqual(row["status"], "match", row)
