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

    def test_c_assembly_in_do_recovers_sibling_statements(self):
        analysis, reference = self.check_reference("asm_do_recovery.c")
        for function in analysis.raw["functions"]:
            self.assertEqual(function["end_line"], reference["functions"][function["name"]]["end_line"])

    def test_cpp_assembly_in_do_recovers_sibling_statements(self):
        analysis, reference = self.check_reference("asm_do_recovery.cpp")
        for function in analysis.raw["functions"]:
            self.assertEqual(function["end_line"], reference["functions"][function["name"]]["end_line"])

    def test_c_keywords_and_generic_selection_follow_original_cdt(self):
        self.check_reference("c_keyword_generic.c")

    def test_c_inferred_type_names_do_not_bind_ambiguous_casts(self):
        self.check_reference("type_inferred.c")

    def test_c_typedef_casts_and_parameter_shadowing(self):
        self.check_reference("type_explicit.c")

    def test_c_ordinary_type_arguments_recover_problem_containers(self):
        self.check_reference("ordinary_va_arg_recovery.c")

    def test_cpp_ordinary_type_arguments_recover_problem_containers(self):
        self.check_reference("ordinary_va_arg_recovery.cpp")

    def test_c_type_argument_cast_sizeof_and_alias_ambiguities(self):
        self.check_reference("ordinary_va_arg_scope_controls.c")

    def test_cpp_type_argument_cast_sizeof_and_alias_ambiguities(self):
        self.check_reference("ordinary_va_arg_scope_controls.cpp")

    def test_c_decorated_expression_problem_containers(self):
        self.check_reference("at_expression.c")

    def test_cpp_decorated_expression_problem_containers(self):
        self.check_reference("at_expression.cpp")

    def test_missing_if_condition_recovers_across_following_switch(self):
        self.check_reference("kuna_bare_if_before_switch.c")

    def test_c_gnu_attribute_statements_recover_control_containers(self):
        self.check_reference("gnu_attribute_statements.c")

    def test_cpp_gnu_attribute_statements_keep_empty_control_bodies(self):
        self.check_reference("gnu_attribute_statements.cpp")

    def test_c_invalid_abstract_pointer_cast_recovers_problem_container(self):
        self.check_reference("ms_pointer_qualifiers.c")

    def test_cpp_invalid_abstract_pointer_cast_recovers_problem_container(self):
        self.check_reference("ms_pointer_qualifiers.cpp")

    def test_c_grouped_extension_statement_expression_is_not_a_type_id(self):
        self.check_reference("grouped_gnu_statement_expr.c")

    def test_cpp_grouped_extension_statement_expression_is_not_a_type_id(self):
        self.check_reference("grouped_gnu_statement_expr.cpp")

    def test_c_extension_marker_keeps_genuine_casts(self):
        self.check_reference("extension_type_casts.c")

    def test_cpp_extension_marker_keeps_genuine_casts(self):
        self.check_reference("extension_type_casts.cpp")

    def test_c_problem_for_label_drops_without_losing_recovered_header_clauses(self):
        self.check_reference("labeled_problem_for_init.c")

    def test_cpp_qualified_for_initializer_keeps_its_label_and_loop(self):
        self.check_reference("labeled_problem_for_init.cpp")

    def test_c_problem_for_label_recovery_preserves_casted_pointer_update(self):
        self.check_reference("labeled_problem_for_cast_update.c")

    def test_cpp_casted_pointer_for_update_keeps_loop_edges(self):
        self.check_reference("labeled_problem_for_cast_update.cpp")


if __name__ == "__main__":
    unittest.main()
