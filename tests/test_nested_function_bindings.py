"""Lexical bindings captured independently from the original CDT/Joern CPG."""

from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import unittest

import rust_joern


FIXTURES = Path(__file__).parent / "fixtures/nested-function-bindings"


def target_key(node, original=False):
    return (node["kind"], node.get("name"), node.get("line"),
            node.get("type" if original else "type_name") or "ANY")


class NestedFunctionBindingTests(unittest.TestCase):
    def test_original_parameter_shadowing_and_declaration_order(self):
        for entry in json.loads((FIXTURES / "provenance.json").read_bytes()):
            with self.subTest(language=entry["language"]):
                source = FIXTURES / entry["source"]
                reference_bytes = gzip.decompress((FIXTURES / entry["reference"]).read_bytes())
                self.assertEqual(hashlib.sha256(source.read_bytes()).hexdigest(), entry["source_sha256"])
                self.assertEqual(hashlib.sha256(reference_bytes).hexdigest(), entry["reference_sha256"])
                reference = json.loads(reference_bytes)
                self.assertTrue(reference["engine_and_source_unchanged"])
                self.assertEqual(reference["source_sha256"], entry["source_sha256"])
                analysis = rust_joern.parse_code(source.read_bytes(), filename=source,
                                               language=entry["language"], strict=True,
                                               preprocessed=True)
                self.assertFalse(analysis.diagnostics)
                methods = {method["fullname"]: method for method in analysis.raw["functions"]}
                actual = Counter()
                for method in methods.values():
                    nodes = {node["id"]: node for node in method["cpg"]["nodes"]}
                    for node in nodes.values():
                        if node["kind"] != "IDENTIFIER":
                            continue
                        targets = [nodes[edge["target"]] for edge in method["cpg"]["edges"]
                                   if edge["kind"] == "REF" and edge["source"] == node["id"]]
                        if external := node.get("external_ref"):
                            target_method = methods[external["method_full_name"]]
                            targets.append(next(target for target in target_method["cpg"]["nodes"]
                                                if target["id"] == external["node"]))
                        actual[(method["fullname"], node.get("name"), node["code"],
                                node.get("line"), node.get("type_name") or "ANY",
                                tuple(sorted(target_key(target) for target in targets)))] += 1
                expected = Counter(
                    (node["method"], node.get("name"), node["code"], node.get("line"),
                     node.get("type") or "ANY",
                     tuple(sorted(target_key(target, original=True) for target in node["refs"])))
                    for node in reference["data"][0]["identifiers"]
                )
                self.assertEqual(actual, expected)


if __name__ == "__main__":
    unittest.main()
