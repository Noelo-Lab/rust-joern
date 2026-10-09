"""Original CDT CFGs for unchanged saved inputs with malformed literals."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/literal-recovery"
SPEC = importlib.util.spec_from_file_location("literal_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class LiteralRecoveryTests(unittest.TestCase):
    def check_reference(self, name):
        source = FIXTURES / name
        expected = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
        self.assertEqual(expected["generator"], "Original public PyJoern 4.0.150.4 / Joern v4.0.150")
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), expected["source_sha256"])
        analysis = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True, preprocessed=True)
        self.assertFalse(analysis.diagnostics)
        functions = COMPARISON.candidate_functions(analysis.raw)
        self.assertEqual(set(functions), set(expected["functions"]))
        for row in COMPARISON.compare(expected["functions"], functions):
            self.assertEqual(row["status"], "match", row)

    def test_r2dec_unclosed_string_keeps_labels_and_loop_roles(self):
        self.check_reference("literal_actual_r2dec.c")

    def test_binja_malformed_quotes_preserve_surrounding_control_and_methods(self):
        self.check_reference("literal_actual_binja.c")


if __name__ == "__main__":
    unittest.main()
