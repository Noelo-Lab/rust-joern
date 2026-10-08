"""CDT problem recovery and ambiguous expression parsing, from original Joern."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/parser-recovery"
SPEC = importlib.util.spec_from_file_location("parser_recovery_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class ParserRecoveryTests(unittest.TestCase):
    def check_reference(self, name):
        source = FIXTURES / name
        reference = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
        prepared = source.read_bytes()
        self.assertEqual(reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
        self.assertEqual(hashlib.sha256(prepared).hexdigest(), reference["source_sha256"])
        self.assertEqual(reference["prepared_sha256"], reference["source_sha256"])
        analysis = rust_joern.parse_code(prepared, filename=source, strict=True)
        self.assertFalse(analysis.diagnostics)
        actual = COMPARISON.candidate_functions(analysis.raw)
        self.assertEqual(set(actual), set(reference["functions"]))
        for row in COMPARISON.compare(reference["functions"], actual):
            self.assertEqual(row["status"], "match", row)
        return analysis, reference

    def test_c_assembly_problem_preserves_prefix_and_later_methods(self):
        analysis, reference = self.check_reference("microsoft_asm.c")
        for function in analysis.raw["functions"]:
            self.assertEqual(function["end_line"], reference["functions"][function["name"]]["end_line"])

    def test_cpp_assembly_problem_preserves_prefix_and_later_methods(self):
        analysis, reference = self.check_reference("microsoft_asm.cpp")
        for function in analysis.raw["functions"]:
            self.assertEqual(function["end_line"], reference["functions"][function["name"]]["end_line"])

    def test_c_cast_and_call_ambiguity(self):
        self.check_reference("ambiguous_casts.c")

    def test_cpp_cast_and_call_ambiguity(self):
        self.check_reference("ambiguous_casts.cpp")

    def test_c_qualification_recovers_condition_and_problem_statements(self):
        self.check_reference("qualification.c")

    def test_c_qualification_recovers_declarations_for_slots_and_inner_blocks(self):
        self.check_reference("qualification_context.c")

    def test_c_builtin_replacement_precedence_and_macro_method_coverage(self):
        self.check_reference("cdt_builtin_controls.c")

    def test_cpp_builtin_replacement_precedence_and_macro_method_coverage(self):
        self.check_reference("cdt_builtin_controls.cpp")

    def test_c_typeof_builtin_arguments_and_static_assert_calls(self):
        self.check_reference("cdt_types_typeof.c")

    def test_cpp_typeof_builtin_arguments_and_static_assert_calls(self):
        self.check_reference("cdt_types_typeof.cpp")

    def test_invalid_scalar_if_drops_the_control_structure(self):
        self.check_reference("qualification_if_recovery.c")

    def test_invalid_scalar_loop_and_label_recovery(self):
        self.check_reference("qualification_scalar_recovery.c")

    def test_c_bracket_attributes_follow_problem_statement_recovery(self):
        self.check_reference("body_attributes.c")

    def test_cpp_bracket_attributes_preserve_executable_statements(self):
        self.check_reference("body_attributes.cpp")


if __name__ == "__main__":
    unittest.main()
