"""Prepared unresolved headers preserve original CDT CFGs and boundary roles."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/unresolved-includes"
SPEC = importlib.util.spec_from_file_location("include_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class UnresolvedIncludeTests(unittest.TestCase):
    def check_reference(self, name):
        source = FIXTURES / name
        expected = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
        self.assertEqual(expected["generator"], "Original public PyJoern 4.0.150.4 / Joern v4.0.150")
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), expected["source_sha256"])
        actual = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True, preprocessed=True)
        self.assertFalse(actual.diagnostics)
        functions = COMPARISON.candidate_functions(actual.raw)
        self.assertEqual(set(functions), set(expected["functions"]))
        for row in COMPARISON.compare(expected["functions"], functions):
            self.assertEqual(row["status"], "match", row)

    def test_c_unresolved_angle_quoted_and_body_includes(self):
        self.check_reference("includes_unresolved.c")

    def test_cpp_unresolved_angle_quoted_and_body_includes(self):
        self.check_reference("includes_unresolved.cpp")

    def test_unchanged_claude_prepared_cfg(self):
        self.check_reference("claude_base_passwd_update_passwd.c")

    def test_raw_include_requires_preparation(self):
        with self.assertRaisesRegex(RuntimeError, "unexpanded preprocessor directive"):
            rust_joern.parse_code("#include <missing.h>\nint f(void){return 0;}\n", strict=True)


if __name__ == "__main__":
    unittest.main()
