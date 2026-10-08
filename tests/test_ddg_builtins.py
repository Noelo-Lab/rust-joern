"""Builtin lowering parity against the unchanged original PyJoern captures."""

import hashlib
import json
from pathlib import Path
import sys
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_ddg_pyjoern import FLAGS, SERIALIZERS, isomorphic, validate_reference


class BuiltinDdgTests(unittest.TestCase):
    def test_original_cpg_capture_provenance(self):
        fixtures = ROOT / "tests/fixtures/ddg-builtins"
        script_sha = hashlib.sha256((fixtures / "Capture.sc").read_bytes()).hexdigest()
        for language in ("c", "cpp"):
            with self.subTest(language=language):
                reference = json.loads((fixtures / f"joern-4.0.150-{language}.json").read_text())
                source = fixtures / reference["source"]
                self.assertEqual(reference["source_sha256"], hashlib.sha256(source.read_bytes()).hexdigest())
                self.assertEqual(reference["capture_script_sha256"], script_sha)
                self.assertEqual(reference["generator"], "Original Joern 4.0.150 / dataflowOss and ReachingDefProblem")
                self.assertEqual(len(reference["methods"]), 9)

    def test_builtin_sources_match_original_public_ddg_and_labeled_dot(self):
        serializers = {}
        exec(SERIALIZERS, serializers)
        references = ROOT / "tests/fixtures/ddg-parity/references/tier2"
        sources = (
            "decbench-regressions/cdt_builtin_controls_c.c",
            "decbench-regressions/cdt_builtin_controls_cpp.cpp",
            "decbench-regressions/gnu_builtins_c.c",
            "decbench-regressions/gnu_builtins_cpp.cpp",
            "parser-recovery/cdt_builtin_controls.c",
            "parser-recovery/cdt_builtin_controls.cpp",
        )
        compared = 0
        for relative in sources:
            source = ROOT / "tests/fixtures" / relative
            expected = json.loads((references / (source.name + ".ddg.pyjoern.json")).read_text())
            sha = hashlib.sha256(source.read_bytes()).hexdigest()
            validate_reference(expected, sha, sha, "cpp" if source.suffix == ".cpp" else "c")
            parsed = rust_joern.parse_source(source, **FLAGS, strict=True)
            actual = {function.name: function for function in parsed.values()}
            self.assertEqual(set(actual), set(expected["functions"]), source.name)
            for name, original in expected["functions"].items():
                with self.subTest(source=source.name, function=name):
                    function = actual[name]
                    self.assertTrue(isomorphic(original["public"], serializers["public_graph"](function.ddg)))
                    view = function.raw["ddg_view"]
                    labels = function._ddg_labels(view, function.raw["cpg"])
                    self.assertTrue(isomorphic(original["dot"], serializers["native_projection"](view, labels)))
                    compared += 1
        self.assertEqual(compared, 112)


if __name__ == "__main__":
    unittest.main()
