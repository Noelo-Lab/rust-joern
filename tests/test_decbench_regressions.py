"""Reduced failures from the full DecBench audit, with unchanged Joern baselines.

Each fixture must match the original graph in strict mode, including function
coverage, entry/exit roles and degeneracy. These tests run only the native parser;
snapshot capture is explicit.
"""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern


ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/decbench-regressions"
SPEC = importlib.util.spec_from_file_location("cfg_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class DecBenchReducedRegressionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.provenance = json.loads((FIXTURES / "provenance.json").read_text())

    def check_fixture(self, name, *, function=None):
        case = self.provenance["cases"][name]
        source = FIXTURES / case["source"]
        reference = json.loads((FIXTURES / case["reference"]).read_text())
        analysis = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True)
        actual = COMPARISON.candidate_functions(analysis.raw)
        expected = reference["functions"]
        if function is not None:
            self.assertIn(function, expected)
            self.assertIn(function, actual)
            expected = {function: expected[function]}
            actual = {function: actual[function]}
        self.assertEqual(set(expected), set(actual), "function coverage differs")
        for row in COMPARISON.compare(expected, actual):
            self.assertEqual(row["status"], "match", row)
        if function is None:
            self.assertFalse(analysis.diagnostics, analysis.diagnostics)

    def test_reference_integrity_and_original_generator(self):
        for name, case in self.provenance["cases"].items():
            with self.subTest(case=name):
                source = (FIXTURES / case["source"]).read_bytes()
                baseline = (FIXTURES / case["reference"]).read_bytes()
                self.assertEqual(hashlib.sha256(source).hexdigest(), case["source_sha256"])
                self.assertEqual(hashlib.sha256(baseline).hexdigest(), case["reference_sha256"])
                reference = json.loads(baseline)
                self.assertEqual(reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
                self.assertEqual(reference["source_sha256"], case["source_sha256"])
                self.assertEqual(reference["prepared_sha256"], case["source_sha256"])
                self.assertEqual(set(reference["functions"]), set(case["expected_functions"]))
                self.assertTrue(case["origins"])
                for origin in case["origins"]:
                    self.assertIn(origin["input_kind"], ("source", "ida", "kuna", "binja", "r2dec", "angr"))
                    self.assertGreater(origin["prepared_line"], 0)
                    self.assertEqual(len(origin["prepared_sha256"]), 64)
                    self.assertEqual(len(origin["reference_sha256"]), 64)

    def test_integer_first_knr_control_matches_reference(self):
        self.check_fixture("knr_const", function="knr_integer")

    def test_const_first_knr_parameters_retain_function_body(self):
        """Bash wcsnwidth: const-first parameter declarations become prototypes."""
        self.check_fixture("knr_const")

    def test_global_struct_initializer_preserves_later_functions(self):
        """Bash findcmd: an object initializer is confused with a type body."""
        self.check_fixture("global_struct_initializer")

    def test_gnu_inline_assembly_has_verified_cfg_without_diagnostics(self):
        """Betaflight source: valid inline asm rejects an otherwise matching CFG."""
        self.check_fixture("gnu_inline_asm")

    def test_microsoft_assembly_block_preserves_translation_unit_coverage(self):
        """Betaflight IDA: a braced asm block stops subsequent function recovery."""
        self.check_fixture("microsoft_asm_block")

    def test_terminal_label_preserves_loop_edges(self):
        """Betaflight Kuna: a label before a closing brace has no nested statement."""
        self.check_fixture("terminal_label")

    def test_casted_indirect_callee_remains_an_expression(self):
        """Betaflight Kuna: a dereference with nested cast is mistaken for a type."""
        self.check_fixture("casted_indirect_call")

    def test_decompiled_c_global_qualification_matches_original_acceptance(self):
        """Bzip2 IDA: the original frontend accepts ::stream in a .c input."""
        self.check_fixture("global_qualification")

    def test_unresolved_goto_matches_original_recovery_policy(self):
        """Coreutils Kuna: the actual input lacks its target; Joern still emits CFG."""
        self.check_fixture("missing_goto_target")

    def test_nested_member_designators_preserve_initializer_cfg(self):
        """Betaflight source: .u.buffers initializer chains are not parsed."""
        self.check_fixture("nested_designator")

    def test_cpp_operator_declarations_preserve_original_function_names(self):
        """NuttX O0/O2: operator declaration names retain a space after operator."""
        self.check_fixture("cpp_operator_declarations")

    def test_cpp_operator_definitions_preserve_original_function_names(self):
        """NuttX O0/O2: operator definitions use their operator symbol as name."""
        self.check_fixture("cpp_operator_definitions")


    def test_noreturn_suffix_with_unclosed_literal_keeps_later_functions(self):
        """Coreutils Binary Ninja: an unclosed group in a suffix body drops later functions."""
        self.check_fixture("noreturn_suffix_unclosed_literal")

    def test_unresolved_grouped_pointer_calls_are_expressions(self):
        """Betaflight r2dec: uint32_t (*r3)() (); without a typedef is a call."""
        self.check_fixture("grouped_pointer_call")

    def test_typedef_grouped_pointer_calls_follow_declarator_validity(self):
        """A typedef keeps the declaration unless it declares a function returning one."""
        self.check_fixture("grouped_pointer_call_typedef")

    def test_calls_without_terminators_keep_following_statements(self):
        """Betaflight and ChibiOS angr: unterminated calls before a loop at a block end."""
        self.check_fixture("call_missing_semicolon_before_loop")

    def test_numeric_literal_callees_are_calls(self):
        """U-Boot angr: a labelled 1619153864(); call is not a problem statement."""
        self.check_fixture("literal_callee")

    def test_brace_operand_problem_resumes_after_its_brace_group(self):
        """Zlib Binary Ninja: a != {0} condition closes its block; later statements remain."""
        self.check_fixture("brace_operand_condition")

if __name__ == "__main__":
    unittest.main()
