"""Mechanical port tooling contracts; these controls are not corpus evidence."""

import importlib.util
import hashlib
import json
from pathlib import Path
import sys
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(
        "joern_port_" + name, ROOT / "scripts/joern_port" / (name + ".py")
    )
    result = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = result
    spec.loader.exec_module(result)
    return result


generate = module("generate_operators")
fixtures = module("import_fixtures")


class PortTests(unittest.TestCase):
    def test_imported_cpp_cfgs_against_frozen_original(self):
        import rust_joern

        spec = importlib.util.spec_from_file_location(
            "port_compare_pyjoern", ROOT / "scripts/compare_pyjoern.py"
        )
        comparison = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(comparison)
        base = ROOT / "tests/reference/joern-4.0.150/fixtures"
        manifest = json.loads((base / "manifest.json").read_text())
        for row in manifest["rows"]:
            with self.subTest(source=row["source"]):
                data = (base / row["source"]).read_bytes()
                reference = (base / row["reference"]).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(), row["source_sha256"])
                self.assertEqual(
                    hashlib.sha256(reference).hexdigest(), row["reference_sha256"]
                )
                expected = json.loads(reference)
                self.assertEqual(expected["prepared_sha256"], row["source_sha256"])
                self.assertIn("4.0.150.4 / Joern v4.0.150", expected["generator"])
                candidate = rust_joern.parse_code(
                    data.decode(),
                    filename="input.cpp",
                    preprocessed=True,
                    strict=True,
                    joern_loop_conditions=True,
                ).raw
                self.assertFalse(candidate["diagnostics"])
                results = comparison.compare(
                    expected["functions"], comparison.candidate_functions(candidate)
                )
                self.assertTrue(results)
                self.assertTrue(all(r["status"] == "match" for r in results), results)

    def test_checked_in_port_is_current(self):
        subprocess.run(
            [
                sys.executable,
                str(ROOT / "scripts/joern_port/generate_operators.py"),
                "--check",
            ],
            check=True,
            capture_output=True,
            text=True,
        )

    def test_complete_original_matches_translate_without_guessing_names(self):
        output = generate.generate()
        self.assertIn('"%=" => "<operators>.assignmentModulo"', output)
        self.assertIn('("typeid", _) => "<operator>.typeOf"', output)
        self.assertIn("op_max: separate/unsupported parser owner", output)
        self.assertIn('"..." => "<operator>.op_ellipses"', output)
        self.assertEqual(output.count("=>"), 46)

    def test_translation_rejects_new_scala_statements_and_missing_default(self):
        for source in (
            "match { case E.op_plus => compute()\n}",
            "match { case E.op_plus => Operators.plus\n}",
            "match { case _ => Defines.OperatorUnknown\nthrow error\n}",
            "match { case _ => Defines.OperatorUnknown\ncase _ => Defines.OperatorUnknown\n}",
        ):
            with self.assertRaises(ValueError):
                generate.extract_match(source, "match {", "E")

    def test_scala_comments_and_strings_do_not_create_fake_inputs(self):
        source = """
// code("ignored")
/* nested /* code("ignored") */ comment */
val label = "code(ignored)"
val cpg = code("int f() { return 1; }", "test.cpp")
"""
        rows = fixtures.extract(source)
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]["language"], "cpp")
        self.assertEqual(rows[0]["text"], "int f() { return 1; }")

    def test_strip_margin_preserves_empty_lines_and_source_bytes(self):
        row = fixtures.extract(
            'code("""\n\n  |int f() {\r\n |  return 1;\n |}\n""".stripMargin)'
        )[0]
        self.assertEqual(row["text"], "\n\nint f() {\r\n  return 1;\n}\n")

    def test_no_interpolation_composition_or_fixture_context_guessing(self):
        for source in (
            'code(s"""$function""")',
            'code("int f();" + body)',
            'code("int f();", filename)',
            'code("""|int f();""".stripMargin.trim)',
            'class T extends C2CpgSuite(fileSuffix = ".cpp") { code("int f();") }',
            r'code("""int \u0066();""")',
        ):
            row = fixtures.extract(source)[0]
            self.assertEqual(row["status"], "requires_review", source)
            self.assertNotIn("text", row)


if __name__ == "__main__":
    unittest.main()
