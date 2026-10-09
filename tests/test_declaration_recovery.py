"""Declaration recovery controls captured from unchanged CDT/Joern."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/declaration-recovery"
SPEC = importlib.util.spec_from_file_location("declaration_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class DeclarationRecoveryTests(unittest.TestCase):
    def test_original_c_and_cpp_declaration_coverage_and_control_flow(self):
        for capture in json.loads((FIXTURES / "provenance.json").read_text())["captures"]:
            with self.subTest(source=capture["source"]):
                source = FIXTURES / capture["source"]
                prepared = source.read_bytes()
                reference_bytes = source.with_name(source.name + ".pyjoern.json").read_bytes()
                reference = json.loads(reference_bytes)
                self.assertEqual(hashlib.sha256(prepared).hexdigest(), capture["source_sha256"])
                self.assertEqual(hashlib.sha256(reference_bytes).hexdigest(), capture["reference_sha256"])
                self.assertEqual(reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
                self.assertEqual(reference["source_sha256"], capture["source_sha256"])
                analysis = rust_joern.parse_code(prepared, filename=source, strict=True, preprocessed=True)
                self.assertFalse(analysis.diagnostics)
                actual = COMPARISON.candidate_functions(analysis.raw)
                self.assertEqual(set(actual), set(reference["functions"]))
                for row in COMPARISON.compare(reference["functions"], actual):
                    self.assertEqual(row["status"], "match", row)


if __name__ == "__main__":
    unittest.main()
