"""Original-Joern CFG coverage, roles, degeneracy and directed topology controls."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/cpp-qualified-expressions"
SPEC = importlib.util.spec_from_file_location("qualified_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class CppQualifiedExpressionTests(unittest.TestCase):
    def test_unchanged_original_joern_graphs(self):
        for capture in json.loads((FIXTURES / "provenance.json").read_text())["captures"]:
            with self.subTest(source=capture["source"]):
                source = FIXTURES / capture["source"]
                text = source.read_bytes()
                raw_reference = source.with_name(source.name + ".pyjoern.json").read_bytes()
                reference = json.loads(raw_reference)
                self.assertEqual(hashlib.sha256(text).hexdigest(), capture["source_sha256"])
                self.assertEqual(hashlib.sha256(raw_reference).hexdigest(), capture["reference_sha256"])
                self.assertEqual(reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
                self.assertEqual(reference["source_sha256"], capture["source_sha256"])
                self.assertEqual(reference["prepared_sha256"], capture["source_sha256"])
                analysis = rust_joern.parse_code(text, filename=source, strict=True, preprocessed=True,
                                                joern_loop_conditions=True)
                self.assertFalse(analysis.diagnostics)
                actual = COMPARISON.candidate_functions(analysis.raw)
                self.assertEqual(set(actual), set(reference["functions"]))
                for row in COMPARISON.compare(reference["functions"], actual):
                    self.assertEqual(row["status"], "match", row)


if __name__ == "__main__":
    unittest.main()
