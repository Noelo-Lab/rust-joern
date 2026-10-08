"""Global builtin uses must retain the original external macro methods."""

import hashlib
import json
from pathlib import Path
import sys
import unittest

import rust_joern

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_ddg_pyjoern import SERIALIZERS, isomorphic, validate_reference
from compare_pyjoern import candidate_functions, isomorphic as cfg_isomorphic


class GlobalMacroTests(unittest.TestCase):
    def test_global_builtin_methods_match_original_metadata_cfg_and_ddg(self):
        source = ROOT / "tests/fixtures/ddg-parity/global-macros.c"
        references = source.parent / "references"
        ddg_path = references / (source.name + ".ddg.pyjoern.json")
        expected = json.loads(ddg_path.read_text())
        cfg = json.loads((references / (source.name + ".cfg.pyjoern.json")).read_text())
        source_sha = hashlib.sha256(source.read_bytes()).hexdigest()
        validate_reference(expected, source_sha, source_sha, "c")
        self.assertEqual(cfg["source_sha256"], source_sha)
        self.assertEqual(cfg["derived_from_reference_sha256"], hashlib.sha256(ddg_path.read_bytes()).hexdigest())

        serializers = {}
        exec(SERIALIZERS, serializers)
        functions = rust_joern.parse_source(source, no_metadata=True, no_ast=True, strict=True)
        actual = {function.name: function for function in functions.values()}
        self.assertEqual(set(actual), set(expected["functions"]))
        self.assertEqual(len(actual), 3)
        actual_cfg = candidate_functions({"functions": [function.raw for function in actual.values()]})
        for name, original in expected["functions"].items():
            with self.subTest(function=name):
                function = actual[name]
                self.assertEqual({field: getattr(function, field) for field in
                                  ("name", "fullname", "start_line", "end_line")},
                                 {field: original[field] for field in
                                  ("name", "fullname", "start_line", "end_line")})
                self.assertEqual(Path(function.filename).name, Path(original["filename"]).name)
                self.assertTrue(cfg_isomorphic(cfg["functions"][name]["cfg"], actual_cfg[name]["cfg"]))
                self.assertTrue(isomorphic(original["public"], serializers["public_graph"](function.ddg)))
                view = function.raw["ddg_view"]
                labeled = serializers["native_projection"](view, function._ddg_labels(view, function.raw["cpg"]))
                self.assertTrue(isomorphic(original["dot"], labeled))


if __name__ == "__main__":
    unittest.main()
