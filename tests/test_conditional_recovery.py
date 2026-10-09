"""Scoped conditional and empty annotation recovery against unchanged Joern."""
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/conditional-recovery"
SPEC = importlib.util.spec_from_file_location("conditional_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class ConditionalRecoveryTests(unittest.TestCase):
    def check_reference(self, language, prefix="conditional_empty_scoped", count=11):
        source = FIXTURES / (prefix + "." + language)
        expected = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
        self.assertEqual(expected["generator"], "Original public PyJoern 4.0.150.4 / Joern v4.0.150")
        self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), expected["source_sha256"])
        actual = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True, preprocessed=True)
        self.assertFalse(actual.diagnostics)
        functions = COMPARISON.candidate_functions(actual.raw)
        self.assertEqual(set(functions), set(expected["functions"]))
        self.assertEqual(len(functions), count)
        for row in COMPARISON.compare(expected["functions"], functions):
            self.assertEqual(row["status"], "match", row)

    def test_c_default_bindings_and_empty_annotation_recovery(self):
        self.check_reference("c")

    def test_cpp_default_bindings_and_empty_annotation_recovery(self):
        self.check_reference("cpp")

    def test_c_erased_macro_declarator_keeps_following_definitions(self):
        self.check_reference("c", "conditional_header_erasure", 3)

    def test_cpp_erased_macro_declarator_keeps_following_definitions(self):
        self.check_reference("cpp", "conditional_header_erasure", 3)

    def test_raw_conditionals_require_preparation(self):
        with self.assertRaisesRegex(RuntimeError, "unexpanded preprocessor directive"):
            rust_joern.parse_code("#if 0\nint f(){return 0;}\n#endif\n", strict=True)

    def test_nonempty_source_expansion_is_not_silently_parsed(self):
        with self.assertRaisesRegex(RuntimeError, "unsupported macro expansion"):
            rust_joern.parse_code("#if 1\n#define VALUE 3\nint f(){return VALUE;}\n#endif\n", strict=True, preprocessed=True)


if __name__ == "__main__":
    unittest.main()
