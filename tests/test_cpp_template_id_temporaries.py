"""Compare C++ template-id temporaries with original Joern CFGs."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/cpp-template-id-temporaries"
SPEC = importlib.util.spec_from_file_location(
    "comparison", ROOT / "scripts/compare_pyjoern.py"
)
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class TemplateIdTemporaryReferenceTests(unittest.TestCase):
    def test_strict_parsing_matches_original_cfgs(self):
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
                    joern_cdt_failures=True,
                    joern_cpp_control_flow=True,
                )
                self.assertEqual(result.diagnostics, [])
                actual = COMPARISON.candidate_functions(result.raw)
                self.assertEqual(set(actual), set(reference["functions"]))
                for row in COMPARISON.compare(reference["functions"], actual):
                    self.assertEqual(row["status"], "match", row)


if __name__ == "__main__":
    unittest.main()
