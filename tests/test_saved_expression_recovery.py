"""Saved decompiler problem expressions, captured from unchanged CDT/Joern."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/saved-expression-recovery"
SPEC = importlib.util.spec_from_file_location("saved_comparison", ROOT / "scripts/compare_pyjoern.py")
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)
CASES = ("malformed_format_literals", "colon_grouped_expressions",
         "problem_expression_boundaries", "additional_saved_syntax",
         "saved_problem_operand_controls")
STAGE2 = ("problem_grammar_containers.c", "problem_grammar_containers.cpp",
          "literal_newline_boundaries.c", "literal_newline_boundaries.cpp",
          "literal_condition_boundaries.c", "literal_condition_recovery.cpp",
          "stage2_controls.c", "stage2_controls.cpp",
          "stage2_container_followups.c", "stage2_container_followups.cpp",
          "actual_clear_pass.c")


class SavedExpressionRecoveryTests(unittest.TestCase):
    def test_bare_brace_and_quoted_condition_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("final_bare_brace_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                source = FIXTURES / ("final_bare_brace_boundaries" + ext)
                reference = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
                for name in ("simple_if_condition", "nested_if_condition", "malformed_json_quote_condition"):
                    self.assertEqual(functions[name]["end_line"], reference["functions"][name]["end_line"])
                nodes = functions["malformed_json_quote_condition"]["cpg"]["nodes"]
                self.assertEqual(sum(n["kind"] == "CONTROL_STRUCTURE" and n.get("name") == "IF" for n in nodes), 1)
                self.assertEqual(sum(n.get("name") == "known" and n["kind"] == "CALL" for n in nodes), 2)
                self.assertFalse(any(n["kind"] == "LOCAL" and n.get("name") == "z" for n in nodes))
                nodes = functions["label_assignment"]["cpg"]["nodes"]
                self.assertEqual(any(n["kind"] == "JUMP_TARGET" for n in nodes), ext == ".cpp")
                assignments = [n for n in nodes if n.get("name") == "<operator>.assignment"]
                self.assertEqual(len(assignments), int(ext == ".cpp"))
                if assignments:
                    children = {e["target"] for e in functions["label_assignment"]["cpg"]["edges"]
                                if e["source"] == assignments[0]["id"] and e["kind"] == "AST"}
                    self.assertEqual(len(children), 1)
                returns = [n for n in functions["ordinary_return"]["cpg"]["nodes"] if n["kind"] == "RETURN"]
                self.assertEqual(len(returns), int(ext == ".cpp"))

    def check_original(self, filename):
        source = FIXTURES / filename
        prepared = source.read_bytes()
        reference = json.loads(source.with_name(source.name + ".pyjoern.json").read_text())
        self.assertIn(reference["generator"], ("pyjoern 4.0.150.4 / Joern v4.0.150",
                                              "Original public PyJoern 4.0.150.4 / Joern v4.0.150"))
        digest = hashlib.sha256(prepared).hexdigest()
        self.assertEqual(reference["source_sha256"], digest)
        self.assertEqual(reference.get("prepared_sha256", reference["source_sha256"]), digest)
        analysis = rust_joern.parse_code(prepared, filename=source, strict=True, preprocessed=True)
        self.assertFalse(analysis.diagnostics)
        actual = COMPARISON.candidate_functions(analysis.raw)
        self.assertEqual(set(actual), set(reference["functions"]))
        for row in COMPARISON.compare(reference["functions"], actual):
            self.assertEqual(row["status"], "match", row)
        return analysis

    def test_c_problem_containers_and_valid_operands(self):
        for name in CASES:
            with self.subTest(name=name):
                self.check_original(name + ".c")

    def test_cpp_problem_containers_and_valid_operands(self):
        for name in CASES:
            with self.subTest(name=name):
                self.check_original(name + ".cpp")

    def test_numeric_suffix_keeps_recovered_assignment_and_identifier(self):
        for suffix in (".c", ".cpp"):
            with self.subTest(language=suffix):
                analysis = self.check_original("saved_problem_operand_controls" + suffix)
                functions = {function["name"]: function for function in analysis.raw["functions"]}
                for name, code in (("width_assignment", "n=n/32"),
                                   ("width_bare_assignment", "n=32"),
                                   ("width_plus_assignment", "n=n+32")):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(node["kind"] == "CALL" and node["code"] == code
                                        and node.get("name") == "<operator>.assignment" for node in nodes))
                    self.assertTrue(any(node["kind"] == "IDENTIFIER" and node.get("name") == "divisor"
                                        for node in nodes))

    def test_stage2_complete_container_recovery(self):
        for filename in STAGE2:
            with self.subTest(filename=filename):
                self.check_original(filename)

    def test_missing_separators_preserve_the_original_cpg_siblings(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("stage2_container_followups" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                assignment = functions["missing_assign_separator"]["cpg"]["nodes"]
                self.assertEqual(sum(n["kind"] == "CALL" and n.get("name") == "known" for n in assignment), 2)
                calls = functions["missing_call_separator"]["cpg"]["nodes"]
                self.assertEqual(sum(n["kind"] == "CALL" and n.get("name") == "known" for n in calls), 1)
                self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "x" and n.get("type_name") == "known" for n in calls))
                recovered = functions["for_scalar_head"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "mystery" for n in recovered))
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in recovered))


    def test_member_empty_operand_and_separator_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("problem_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["separator_comparison_assignment"]["cpg"]["nodes"]
                self.assertEqual(sum(n["kind"] == "CALL" and n.get("name") == "known" for n in nodes), 2)
                nodes = functions["separator_identifier_assignment"]["cpg"]["nodes"]
                self.assertEqual(sum(n["kind"] == "CALL" and n.get("name") == "known" for n in nodes), 1)
                self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "x" for n in nodes))
                nodes = functions["dotdot_argument"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CALL" and n.get("name") == "known" for n in nodes))

    def test_malformed_operator_slots_preserve_valid_operations(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("operator_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["signed_mul_initializer"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "<operator>.multiplication" and n["code"] == "x *s" for n in nodes))
                self.assertTrue(any(n["kind"] == "IDENTIFIER" and n.get("name") == "y" for n in nodes))
                nodes = functions["unsigned_shift_for_update"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["valid_shift_for"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "<operators>.assignmentArithmeticShiftRight" for n in nodes))


    def test_scoped_empty_macro_problem_provenance(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("macro_problem_guarded" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["macro_return_only"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "UNKNOWN" and "annotation(x,1)" in n["code"] for n in nodes))
                nodes = functions["plain_return_only"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] in ("UNKNOWN", "RETURN") for n in nodes))
                nodes = functions["valid_empty_initializer"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "annotation" for n in nodes))

    def test_cast_prefix_and_grouped_postfix_follow_original_ambiguity(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("prefix_cast_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for category in ("unknown", "inferred", "explicit", "shadow", "object"):
                    for operation, operator in (("inc", "preIncrement"), ("dec", "preDecrement")):
                        for container in ("assign", "return", "init", "condition"):
                            nodes = functions[f"{category}_{operation}_{container}"]["cpg"]["nodes"]
                            self.assertTrue(any(n.get("name") == "<operator>.cast" for n in nodes))
                            self.assertTrue(any(n.get("name") == "<operator>." + operator for n in nodes))
                analysis = self.check_original("postfix_group_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for category in ("unknown", "inferred", "explicit", "shadow", "object"):
                    for operation, operator in (("inc", "postIncrement"), ("dec", "postDecrement")):
                        for dereference in ("", "deref_"):
                            nodes = functions[f"{category}_{dereference}post_{operation}"]["cpg"]["nodes"]
                            self.assertFalse(any(n.get("name") == "<operator>.cast" for n in nodes))
                            self.assertTrue(any(n.get("name") == "<operator>." + operator for n in nodes))
                nodes = functions["postfix_bang_braced_if"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["postfix_bang_scalar_if"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["ungrouped_postfix_suffix"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "<operator>.postIncrement" for n in nodes))
                self.assertTrue(any(n["kind"] == "IDENTIFIER" and n.get("name") == "suffix" for n in nodes))
                nodes = functions["ungrouped_postfix_suffix_return"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "RETURN" for n in nodes))

    def test_incomplete_do_and_newline_literals_preserve_recovered_siblings(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("loop_literal_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["missing_do_while_assignment"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" or n.get("name") == "work" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operator>.postIncrement" for n in nodes))
                for name in ("assignment_unclosed_string", "scalar_if_unclosed_string"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n.get("name") == "<operator>.assignment" for n in nodes))
                    self.assertTrue(any(n.get("name") == "work" for n in nodes))
                    self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["return_unclosed_string"]["cpg"]["nodes"]
                self.assertFalse(any(n.get("name") == "work" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operator>.postIncrement" for n in nodes))
                analysis = self.check_original("literal_statement_extension" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["assignment_followed_assignment"]["cpg"]["nodes"]
                self.assertEqual(sum(n.get("name") == "<operator>.assignment" for n in nodes), 2)
                for name in ("braced_if_string", "braced_while_char", "braced_do_string"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                    self.assertTrue(any(n.get("name") == "work" for n in nodes))

    def test_malformed_declaration_suffixes_recover_as_control_siblings(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("scalar_declaration_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for prefix in ("suffix", "initializer_suffix"):
                    for control in ("if", "else", "while", "for"):
                        nodes = functions[f"{prefix}_scalar_{control}"]["cpg"]["nodes"]
                        self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                        self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "u" for n in nodes))
                    for control in ("if", "while", "for", "do"):
                        nodes = functions[f"{prefix}_braced_{control}"]["cpg"]["nodes"]
                        self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["initializer_suffix_scalar_else"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "<operator>.assignment" for n in nodes))
                self.assertTrue(any(n["kind"] == "IDENTIFIER" and n.get("name") == "y" for n in nodes))
                self.assertFalse(any(n.get("name") == "work" for n in nodes))
                for prefix in ("ordinary", "initialized", "unknown_type_initialized"):
                    nodes = functions[f"{prefix}_scalar_if"]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))

    def test_postfix_bang_and_adjacent_group_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("round4_postfix_bang_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["bare_bang_initializer"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "y" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operator>.assignment" and n["code"].replace(" ", "") == "y=r6" for n in nodes))
                nodes = functions["binary_bang_initializer"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "LOCAL" for n in nodes))
                nodes = functions["binary_bang_scalar_if"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["binary_bang_braced_if"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                for name in ("grouped_adjacent_statement", "grouped_adjacent_initializer", "grouped_adjacent_scalar_if"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n.get("name") == "<operator>.assignment" for n in nodes))
                    self.assertTrue(any(n.get("name") == "CONCAT" for n in nodes))
                nodes = functions["positive_object_cast"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "<operator>.cast" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operator>.logicalNot" for n in nodes))

    def test_scanner_backticks_and_empty_cast_operands(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("backtick_empty_cast" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                nodes = functions["quoted_backtick_if_scalar"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                self.assertTrue(any(n.get("name") == "report_error" for n in nodes))
                self.assertTrue(any(n["kind"] == "LITERAL" and '`' in n["code"] for n in nodes))
                nodes = functions["empty_cast_initializer"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "LOCAL" for n in nodes))
                self.assertTrue(any(n.get("name") == "work" for n in nodes))
                nodes = functions["empty_cast_if_scalar"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["empty_cast_if_braced"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))

    def test_cast_grammar_preserves_true_object_binding_ambiguities(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("round4_shadow_cast_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for category in ("object", "typedef_shadow", "unknown", "inferred"):
                    for operand in ("literal", "tilde", "pointer"):
                        nodes = functions[f"{category}_{operand}"]["cpg"]["nodes"]
                        self.assertTrue(any(n.get("name") == "<operator>.cast" for n in nodes))
                    for operand, operator in (("multiply", "<operator>.multiplication"),
                                              ("addition", "<operator>.addition"),
                                              ("parenthesized_call", "<operator>.pointerCall" if ext == ".c" else "<operator>()")):
                        nodes = functions[f"{category}_{operand}"]["cpg"]["nodes"]
                        self.assertFalse(any(n.get("name") == "<operator>.cast" for n in nodes))
                        self.assertTrue(any(n.get("name") == operator for n in nodes))
                for operand in ("literal", "tilde", "pointer", "multiply", "addition", "parenthesized_call"):
                    nodes = functions[f"typedef_{operand}"]["cpg"]["nodes"]
                    self.assertTrue(any(n.get("name") == "<operator>.cast" for n in nodes))

    def test_consecutive_named_casts_keep_unambiguous_grammar_and_call_controls(self):
        cast_names = ("object_both", "object_outer", "object_inner", "shadow_outer",
                      "shadow_inner", "shadow_both", "true_typedef_both", "unbound_both",
                      "inferred_both", "mixed_signed", "object_primitive")
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("consecutive_named_cast_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in cast_names:
                    graph = functions[name]["cpg"]
                    casts = {n["id"] for n in graph["nodes"] if n.get("name") == "<operator>.cast"}
                    self.assertEqual(len(casts), 2, name)
                    self.assertTrue(any(e["kind"] == "AST" and e["source"] in casts
                                        and e["target"] in casts for e in graph["edges"]), name)
                for name in ("ordinary_call", "shadow_call", "unknown_call"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n.get("name") == "<operator>.cast" for n in nodes), name)
                    operator = "<operator>.pointerCall" if ext == ".c" else "<operator>()"
                    self.assertTrue(any(n.get("name") == operator for n in nodes), name)

    def test_nonterminal_member_and_memory_suffix_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("nonterminal_member_and_memory_shift" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("call_empty_before_member_scalar_if", "member_before_call_empty_scalar_while",
                             "memory_u_suffix_scalar_if", "memory_s_suffix_for_scalar_body"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes), name)
                for name in ("call_empty_before_deref_scalar_if", "positive_memory_shift_scalar_if"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes), name)
                graph = functions["call_empty_before_deref_compound"]["cpg"]
                assignments = {n["id"] for n in graph["nodes"] if n.get("name") == "<operator>.assignment"}
                multiplies = {n["id"] for n in graph["nodes"] if n.get("name") == "<operator>.multiplication"}
                self.assertTrue(any(e["kind"] == "AST" and e["source"] in assignments
                                    and e["target"] in multiplies for e in graph["edges"]))
                nodes = functions["memory_identifier_suffix_statement"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "<operator>.indirection" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operators>.assignmentArithmeticShiftRight" for n in nodes))
                for name in ("brace_literal_condition", "missing_operand_before_comma"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertEqual(sum(n["kind"] == "UNKNOWN" for n in nodes), 1)
                    self.assertFalse(any(n.get("name") in ("<operator>.logicalAnd", "<operator>.logicalOr") for n in nodes))

    def test_malformed_quoted_calls_follow_original_container_and_scope_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                filename = "malformed_call_terminal_boundaries" + ext
                analysis = self.check_original(filename)
                reference = json.loads((FIXTURES / (filename + ".pyjoern.json")).read_text())["functions"]
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("return_terminal", "return_inner_brace", "initializer_terminal", "initializer_inner_brace"):
                    self.assertEqual(functions[name]["start_line"], reference[name]["start_line"], name)
                    self.assertEqual(functions[name]["end_line"], reference[name]["end_line"], name)

    def test_partial_indexes_and_generic_declaration_suffixes(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("remaining_rare_expressions" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("missing_multiply_index", "missing_multiply_index_sum", "empty_index_lvalue"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n.get("name") in ("<operator>.addressOf", "<operator>.indexAccess",
                                                            "<operator>.multiplication") for n in nodes), name)
                    self.assertFalse(any(n["kind"] == "IDENTIFIER" and n.get("name") == "v23" for n in nodes), name)
                for name in ("memory_concat", "plain_concat_assignment"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n.get("name") == "<operator>.assignment" for n in nodes), name)
                    self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "v32"
                                        and n.get("type_name") == "CONCAT" for n in nodes), name)
                nodes = functions["valid_gnu_call_ge"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "<operator>.greaterEqualsThan" for n in nodes))
                self.assertTrue(any(n.get("name") == "<operator>.addition" for n in nodes))
                self.check_original("naked_backslash_formats" + ext)

    def test_terminal_statement_recovery_preserves_brace_and_eof_boundaries(self):
        analysis = self.check_original("actual_clear_pass.c")
        nodes = next(f for f in analysis.raw["functions"] if f["name"] == "clear_pass")["cpg"]["nodes"]
        self.assertTrue(any(n.get("name") == "clear_pass" and n["kind"] == "CALL" for n in nodes))
        self.assertTrue(any(n.get("name") == "secure_free" for n in nodes))
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("terminal_prefix_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for prefix in ("bound_empty", "unknown_empty", "bound_compound", "assignment", "increment", "primitive_decl", "unknown_decl"):
                    nodes = functions[f"{prefix}_missing_scalar_if"]["cpg"]["nodes"]
                    self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                    nodes = functions[f"{prefix}_missing_braced_if"]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                    nodes = functions[f"{prefix}_proper_scalar_if"]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))
                nodes = functions["bound_empty_missing_compound"]["cpg"]["nodes"]
                self.assertTrue(any(n.get("name") == "known" for n in nodes))
                nodes = functions["bound_plain_missing_compound"]["cpg"]["nodes"]
                self.assertFalse(any(n.get("name") == "known" for n in nodes))
                self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "x" for n in nodes))
                nodes = functions["primitive_decl_missing_scalar_if"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "LOCAL" and n.get("name") == "y" for n in nodes))
                nodes = functions["return_missing_compound"]["cpg"]["nodes"]
                self.assertFalse(any(n["kind"] == "RETURN" for n in nodes))
                for stem in ("eof_bound_empty", "eof_unknown_empty", "eof_bound_plain", "eof_bound_compound"):
                    analysis = self.check_original(stem + ext)
                    nodes = next(f for f in analysis.raw["functions"] if f["name"] == stem)["cpg"]["nodes"]
                    self.assertFalse(any(n["kind"] in ("CALL", "LOCAL", "IDENTIFIER") for n in nodes))

    def test_missing_group_operands_label_separators_and_quoted_ternaries(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("condition_label_ternary" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("comma_if", "comma_nested_if", "comma_while"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertEqual(sum(n["kind"] == "UNKNOWN" for n in nodes), 1, name)
                    self.assertFalse(any(n.get("name") in ("<operator>.logicalAnd", "<operator>.logicalOr") for n in nodes), name)
                for name in ("call_before_label", "empty_call_before_label", "call_before_case", "empty_call_before_case"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "JUMP_TARGET" for n in nodes), name)
                    self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "known" for n in nodes), name)
                for name in ("quoted_ternary_assignment", "quoted_ternary_initializer", "quoted_ternary_scalar_if", "quoted_ternary_braced_if"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n.get("name") == "<operator>.conditional" for n in nodes), name)
                    self.assertFalse(any(n["kind"] == "LOCAL" and n.get("name") == "next" for n in nodes), name)
                    self.assertTrue(any(n["kind"] == "CALL" and n.get("name") == "known" for n in nodes), name)

    def test_member_separators_preserve_siblings_and_scalar_control_boundaries(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("member_separator_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for prefix in ("arrow_member", "dot_member"):
                    for control in (("if", "while") if prefix == "arrow_member" else ("if",)):
                        missing = functions[f"{prefix}_{control}_scalar_missing"]["cpg"]["nodes"]
                        valid = functions[f"{prefix}_{control}_scalar_valid"]["cpg"]["nodes"]
                        self.assertFalse(any(n["kind"] == "CONTROL_STRUCTURE" for n in missing))
                        self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in valid))
                for name in ("memory_division_block_s", "memory_division_block_arbitrary_identifier"):
                    graph = functions[name]["cpg"]
                    divide = {n["id"] for n in graph["nodes"] if n.get("name") == "<operator>.assignmentDivision"}
                    indirection = {n["id"] for n in graph["nodes"] if n.get("name") == "<operator>.indirection"}
                    self.assertTrue(divide and indirection)
                    self.assertFalse(any(e["kind"] == "AST" and e["source"] in divide
                                         and e["target"] in indirection for e in graph["edges"]))

    def test_malformed_quoted_arguments_preserve_complete_original_scope_recovery(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                filename = "round6_malformed_quoted_calls" + ext
                analysis = self.check_original(filename)
                reference = json.loads((FIXTURES / (filename + ".pyjoern.json")).read_text())["functions"]
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("unclosed_format_string_return", "plain_unclosed_string_return"):
                    self.assertEqual(functions[name]["end_line"], reference[name]["end_line"], name)
                for name in ("format_char_words_statement", "escaped_assert_args_statement", "unclosed_format_string_statement", "char_words_statement"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n["kind"] == "CALL" for n in nodes), name)
                    self.assertTrue(any(n["kind"] == "RETURN" for n in nodes), name)

    def test_preclassified_problem_calls_end_before_compound_braces(self):
        for ext in (".c", ".cpp"):
            with self.subTest(language=ext):
                analysis = self.check_original("final_call_boundaries" + ext)
                functions = {f["name"]: f for f in analysis.raw["functions"]}
                for name in ("multiline_quote_before_brace", "opaque_call_before_brace", "opaque_two_before_brace"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertFalse(any(n["kind"] == "CALL" for n in nodes), name)
                for name in ("multiline_quote_braced_if", "opaque_two_braced_if"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes), name)
                    self.assertEqual(sum(n["kind"] == "CALL" for n in nodes), 1, name)
                for name in ("valid_quoted_splice", "valid_adjacent_strings"):
                    nodes = functions[name]["cpg"]["nodes"]
                    self.assertEqual(sum(n["kind"] == "CALL" for n in nodes), 2, name)
                nodes = functions["valid_nested_if"]["cpg"]["nodes"]
                self.assertTrue(any(n["kind"] == "CONTROL_STRUCTURE" for n in nodes))


if __name__ == "__main__":
    unittest.main()
