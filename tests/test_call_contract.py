"""Original-Joern call AST evidence and population/identity guards."""

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "port_call_contract", ROOT / "scripts/joern_port/call_contract.py"
)
contract = importlib.util.module_from_spec(spec)
spec.loader.exec_module(contract)


class CallContractTests(unittest.TestCase):
    def test_direct_port_sources_have_pinned_fragment_identity(self):
        manifest = json.loads(
            (ROOT / "tests/reference/joern-4.0.150/call-rules.json").read_text()
        )
        self.assertEqual(manifest["commit"], "958fdd3d976197a783f8ade1254b43e977648f28")
        self.assertEqual(len(manifest["functions"]), 7)
        self.assertEqual(len({f["owner"] for f in manifest["functions"]}), 3)
        for function in manifest["functions"]:
            self.assertEqual(
                hashlib.sha256(function["scala"].encode()).hexdigest(),
                function["fragment_sha256"],
            )

    def original(self, language):
        import rust_joern

        base = ROOT / "tests/fixtures/joern-calls"
        source = base / ("calls." + language)
        reference = json.loads((base / (language + "-original.json")).read_text())
        self.assertEqual(
            hashlib.sha256(source.read_bytes()).hexdigest(), reference["source_sha256"]
        )
        capture = ROOT / "tests/fixtures/ddg-lambdas/capture.sc"
        self.assertEqual(
            hashlib.sha256(capture.read_bytes()).hexdigest(),
            reference["capture_sha256"],
        )
        self.assertIn("Joern v4.0.150", reference["generator"])
        raw = rust_joern.parse_code(
            source.read_text(), filename=source.name, preprocessed=True, strict=True
        ).raw
        self.assertEqual(contract.compare(reference, raw), [])
        return reference

    def test_cpp_calls_match_original_roles_identity_and_ast_ownership(self):
        reference = self.original("cpp")
        self.assertEqual(len(reference["methods"]), 18)
        self.assertEqual(sum(len(m["calls"]) for m in reference["methods"]), 25)

    def test_c_calls_match_original_and_are_not_given_cpp_roles(self):
        reference = self.original("c")
        self.assertEqual(len(reference["methods"]), 5)
        self.assertEqual(sum(len(m["calls"]) for m in reference["methods"]), 7)

    def sample(self):
        return {
            "fullname": "f",
            "start_line": 1,
            "cpg": {
                "nodes": [
                    {
                        "id": 0,
                        "kind": "CALL",
                        "name": "missing",
                        "code": "p->missing()",
                        "line": 1,
                        "column": 1,
                    },
                    {"id": 1, "kind": "IDENTIFIER", "name": "p", "code": "p"},
                ],
                "edges": [
                    {"source": 0, "target": 1, "kind": "AST"},
                    {"source": 0, "target": 1, "kind": "ARGUMENT", "label": "0"},
                    {"source": 0, "target": 1, "kind": "RECEIVER"},
                ],
            },
        }

    def test_same_spelling_does_not_prove_shared_receiver_identity(self):
        original = self.sample()
        wrong = copy.deepcopy(original)
        wrong["cpg"]["nodes"].append(
            {"id": 2, "kind": "IDENTIFIER", "name": "p", "code": "p"}
        )
        wrong["cpg"]["edges"].append({"source": 0, "target": 2, "kind": "AST"})
        wrong["cpg"]["edges"][-2]["target"] = 2
        self.assertTrue(contract.project(original)["calls"][0]["shared_base_receiver"])
        self.assertFalse(contract.project(wrong)["calls"][0]["shared_base_receiver"])
        self.assertNotEqual(contract.project(original), contract.project(wrong))

    def test_projection_rejects_ambiguous_or_unbounded_evidence(self):
        mutations = [
            lambda graph: graph["nodes"].append(copy.deepcopy(graph["nodes"][0])),
            lambda graph: graph["edges"].append(copy.deepcopy(graph["edges"][1])),
            lambda graph: graph["edges"].append(copy.deepcopy(graph["edges"][2])),
            lambda graph: graph["edges"].append(copy.deepcopy(graph["edges"][0])),
            lambda graph: graph["edges"].append(
                {"source": 0, "target": 999, "kind": "RECEIVER"}
            ),
            lambda graph: graph["edges"].pop(0),
        ]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                method = self.sample()
                mutate(method["cpg"])
                with self.assertRaises(ValueError):
                    contract.project(method)

    def test_comparison_rejects_duplicate_method_population(self):
        method = self.sample()
        with self.assertRaises(ValueError):
            contract.compare(
                {"methods": [contract.project(method)]}, {"functions": [method, method]}
            )


if __name__ == "__main__":
    unittest.main()
