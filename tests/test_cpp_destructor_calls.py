"""Reference CFGs for explicit destructor calls; controls are not corpus samples."""

import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = ROOT / "tests/fixtures/cpp-destructor-calls"
SPEC = importlib.util.spec_from_file_location(
    "destructor_comparison", ROOT / "scripts/compare_pyjoern.py"
)
COMPARISON = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(COMPARISON)


class CppDestructorCallTests(unittest.TestCase):
    def test_frozen_original_cfgs(self):
        for case in json.loads((FIXTURES / "provenance.json").read_text())["captures"]:
            with self.subTest(source=case["source"]):
                source = FIXTURES / case["source"]
                reference_bytes = source.with_name(
                    source.name + ".pyjoern.json"
                ).read_bytes()
                reference = json.loads(reference_bytes)
                self.assertEqual(
                    hashlib.sha256(source.read_bytes()).hexdigest(),
                    case["source_sha256"],
                )
                self.assertEqual(
                    hashlib.sha256(reference_bytes).hexdigest(),
                    case["reference_sha256"],
                )
                self.assertEqual(
                    reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150"
                )
                self.assertEqual(reference["prepared_sha256"], case["source_sha256"])
                analysis = rust_joern.parse_code(
                    source.read_bytes(),
                    filename=source,
                    language="c++",
                    preprocessed=True,
                    strict=True,
                    joern_loop_conditions=True,
                )
                actual = COMPARISON.candidate_functions(analysis.raw)
                self.assertEqual(set(actual), set(reference["functions"]))
                for row in COMPARISON.compare(reference["functions"], actual):
                    self.assertEqual(row["status"], "match", row)

    def test_invalid_destructor_arguments_are_not_qualified_by_recovery(self):
        for case in json.loads((FIXTURES / "provenance.json").read_text())[
            "rejected_controls"
        ]:
            source = FIXTURES / case["source"]
            raw_reference = source.with_name(source.name + ".pyjoern.json").read_bytes()
            self.assertEqual(
                hashlib.sha256(source.read_bytes()).hexdigest(), case["source_sha256"]
            )
            self.assertEqual(
                hashlib.sha256(raw_reference).hexdigest(), case["reference_sha256"]
            )
            reference = json.loads(raw_reference)
            self.assertEqual(
                reference["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150"
            )
            self.assertEqual(reference["prepared_sha256"], case["source_sha256"])
            self.assertIn("f", reference["functions"])
            permissive = rust_joern.parse_code(
                source.read_bytes(),
                filename=source,
                language="c++",
                preprocessed=True,
                strict=False,
            )
            self.assertTrue(
                any(
                    d["severity"] == "error" and "destructor-id" in d["message"]
                    for d in permissive.raw["diagnostics"]
                )
            )
            with self.assertRaisesRegex(RuntimeError, "destructor-id"):
                rust_joern.parse_code(
                    source.read_bytes(),
                    filename=source,
                    language="c++",
                    preprocessed=True,
                    strict=True,
                )


if __name__ == "__main__":
    unittest.main()
