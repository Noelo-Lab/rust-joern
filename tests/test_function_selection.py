"""Function export controls captured from unchanged public PyJoern."""

import hashlib
import json
from pathlib import Path
import sys
import unittest

import rust_joern


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_pyjoern import candidate_functions, compare

FIXTURES = ROOT / "tests/fixtures/function-selection"


def normalized(function):
    graph = function.cfg
    nodes = list(graph)
    return {"cfg": {"nodes": [node.id for node in nodes],
                    "edges": [[source.id, target.id] for source, target in graph.edges],
                    "entry": [node.id for node in nodes if node.is_entrypoint],
                    "exit": [node.id for node in nodes if node.is_exitpoint],
                    "degenerate": len(nodes) == 1 and all(
                        type(statement).__name__ == "Nop" for node in nodes for statement in node.statements)}}


class FunctionSelectionTests(unittest.TestCase):
    def test_unchanged_original_probe_integrity(self):
        provenance = json.loads((FIXTURES / "provenance.json").read_text())
        self.assertEqual(len(provenance["cases"]), 4)
        for case in provenance["cases"].values():
            source = (FIXTURES / case["source"]).read_bytes()
            reference = (FIXTURES / case["reference"]).read_bytes()
            self.assertEqual(hashlib.sha256(source).hexdigest(), case["source_sha256"])
            self.assertEqual(hashlib.sha256(reference).hexdigest(), case["reference_sha256"])
            data = json.loads(reference)
            self.assertEqual(data["generator"], "Original public pyjoern4.0.150.4 parse_source")
            self.assertEqual(data["source_sha256"], case["source_sha256"])

    def check_probe(self, name):
        source = FIXTURES / name
        reference = json.loads((FIXTURES / (name + ".pyjoern.json")).read_text())["functions"]
        analysis = rust_joern.parse_code(source.read_bytes(), filename=source, strict=True)
        self.assertFalse(analysis.diagnostics)
        functions = rust_joern.parse_source(source, no_ddg=True, strict=True)
        for actual in (candidate_functions(analysis.raw),
                       {name: normalized(function) for name, function in functions.items()}):
            self.assertEqual(set(actual), set(reference))
            for row in compare(reference, actual):
                self.assertEqual(row["status"], "match", row)
        for name, expected in reference.items():
            self.assertEqual(functions[name].start_line, expected["start_line"])
            self.assertEqual(functions[name].fullname, expected["fullname"])
        raw = analysis.raw["functions"]
        if source.stem == "duplicate_selection":
            for name in ("body_then_proto", "proto_then_body"):
                self.assertEqual(sum(function["name"] == name for function in raw), 1)
        else:
            # Export selection must not remove native definitions or their CPGs.
            for name in ("triplet", "trio_large_first"):
                self.assertEqual(sum(function["name"] == name for function in raw), 3)
            self.assertEqual(sum(function["name"] == "mixed_proto" for function in raw), 2)

    def test_c_duplicates_and_prototype_suppression(self):
        self.check_probe("duplicate_selection.c")

    def test_cpp_overloads_namespaces_and_prototype_suppression(self):
        self.check_probe("duplicate_selection.cpp")

    def test_c_triplets_export_last_method(self):
        self.check_probe("duplicate_triplets.c")

    def test_cpp_triplets_keep_distinct_resolved_group(self):
        self.check_probe("duplicate_triplets.cpp")


if __name__ == "__main__":
    unittest.main()
