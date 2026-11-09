"""The DOT DDG projection depends on the stored order of incoming REACHING_DEF edges.

`definition-order.c` holds loops whose projection changes with that order but
not with the parts of it that vary between original Joern runs. Its frozen
original reference checks source -> native API parity; its frozen original
capture checks the projection replayed on Joern's own raw graph.
"""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_ddg_pyjoern import CANDIDATE_WORKER, FLAGS, compare, invoke, validate_reference
import replay_ddg_projection as replay

FIXTURES = ROOT / "tests/fixtures/ddg-decbench"
SOURCE = FIXTURES / "definition-order.c"
CAPTURE = FIXTURES / "replay/definition-order.c.capture.jsonl"
LIBRARY = ROOT / "target/debug/librust_joern.so"


def function(nodes, edges):
    """A minimal function graph: nodes as (kind, code), edges as (kind, source, target, label)."""
    return {"cpg": {"nodes": [{"id": index, "kind": kind, "code": code, "line": 1, "column": 1}
                              for index, (kind, code) in enumerate(nodes)],
                    "edges": [{"source": source, "target": target, "kind": kind, **({"label": label} if label else {})}
                              for kind, source, target, label in edges]}}


class StoredOrderClassificationTests(unittest.TestCase):
    # f(g(a), b): node 2 is the call g(a), an argument of node 1.
    NODES = [("METHOD", "m"), ("CALL", "f(g(a), b)"), ("CALL", "g(a)"), ("IDENTIFIER", "a"), ("IDENTIFIER", "b"),
             ("CALL", "x = 1"), ("CALL", "y = 2")]
    TREE = [("AST", 0, 1, None), ("AST", 1, 2, None), ("AST", 2, 3, None), ("AST", 1, 4, None), ("AST", 0, 5, None),
            ("AST", 0, 6, None), ("ARGUMENT", 1, 2, "1"), ("ARGUMENT", 2, 3, "1"), ("ARGUMENT", 1, 4, "2")]

    def classify(self, expected, actual):
        def incoming(sources):
            return [("REACHING_DEF", source, 2, label) for source, label in sources]
        return dict(replay.in_order(function(self.NODES, self.TREE + incoming(expected)),
                                    function(self.NODES, self.TREE + incoming(actual))))

    def test_equal_order_is_same(self):
        edges = [(5, "x"), (6, "y"), (3, "a")]
        self.assertEqual(self.classify(edges, edges), {"same": 1})

    def test_call_site_groups_may_interleave(self):
        # The argument's own edge and its call site's edges come from two
        # nodes of `solution.in`, whose hash order is not reproducible.
        self.assertEqual(self.classify([(5, "x"), (6, "y"), (3, "a")], [(3, "a"), (5, "x"), (6, "y")]),
                         {"interleaved": 1})

    def test_order_within_a_call_site_group_is_checked(self):
        self.assertEqual(self.classify([(5, "x"), (6, "y"), (3, "a")], [(6, "y"), (5, "x"), (3, "a")]),
                         {"unexpected": 1})

    def test_more_than_four_uses_are_hash_ordered(self):
        # `call.argument.toSet` orders the argument-to-argument edges of f,
        # but the used incoming definitions stay in definition number order.
        nodes = self.NODES + [("IDENTIFIER", name) for name in "cde"]
        tree = self.TREE + [edge for index in (7, 8, 9)
                            for edge in (("AST", 1, index, None), ("ARGUMENT", 1, index, str(index - 4)))]

        def graph(sources):
            return function(nodes, tree + [("REACHING_DEF", source, 2, "v") for source in sources])
        self.assertEqual(dict(replay.in_order(graph([5, 4, 7]), graph([5, 7, 4]))), {"hashed": 1})
        self.assertEqual(dict(replay.in_order(graph([5, 6, 4]), graph([6, 5, 4]))), {"unexpected": 1})

    def test_exit_order_is_not_read_by_the_projection(self):
        nodes = self.NODES + [("METHOD_RETURN", "RET")]
        tree = self.TREE + [("AST", 0, 7, None)]

        def graph(sources):
            return function(nodes, tree + [("REACHING_DEF", source, 7, "v") for source in sources])
        self.assertEqual(dict(replay.in_order(graph([5, 6]), graph([6, 5]))), {"exit": 1})


class DotParsingTests(unittest.TestCase):
    def test_serialized_dot_keeps_ids_parallel_labels_and_entities(self):
        dot = ('digraph "f" {  \n"7" [label = <(METHOD,f)<SUB>1</SUB>> ]\n"9" [label = <(a,a &lt; b)<SUB>2</SUB>> ]\n'
               '"9" [label = <(a,a &lt; b)<SUB>2</SUB>> ]\n  "7" -> "9" \n  "7" -> "9"  [ label = "a &lt; b"] \n'
               '  "7" -> "9"  [ label = "x\ny"] \n}\n')
        for graphviz in (True, False):
            with self.subTest(graphviz=graphviz):
                nodes, edges = replay.dot_parts(dot, graphviz)
                self.assertEqual(nodes, {7, 9})
                self.assertEqual(dict(edges), {(7, 9, ""): 1, (7, 9, "a < b"): 1, (7, 9, "x\ny"): 1})

    def test_both_parsers_agree_on_the_frozen_original_capture(self):
        methods = [json.loads(line) for line in CAPTURE.read_text().splitlines()]
        dots = [method["dot_ddg"][0] for method in methods if method.get("dot_ddg")]
        self.assertEqual(len(dots), 4)
        for dot in dots:
            self.assertEqual(replay.dot_parts(dot, True), replay.dot_parts(dot, False))


@unittest.skipUnless(LIBRARY.exists(), "Native library unavailable; run cargo build --lib")
class DefinitionOrderParityTests(unittest.TestCase):
    def test_native_api_matches_frozen_original_reference(self):
        baseline = json.loads((FIXTURES / "references" / (SOURCE.name + ".ddg.pyjoern.json")).read_text())
        sha = hashlib.sha256(SOURCE.read_bytes()).hexdigest()
        validate_reference(baseline, sha, sha, "c")
        with tempfile.TemporaryDirectory(prefix="ddg-order-") as temporary:
            snapshot = Path(temporary)
            shutil.copyfile(LIBRARY, snapshot / "librust_joern.so")
            shutil.copytree(ROOT / "python", snapshot / "python",
                            ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))
            prepared = snapshot / SOURCE.name
            prepared.write_bytes(SOURCE.read_bytes())
            candidate = invoke(sys.executable, CANDIDATE_WORKER,
                               {"source": str(prepared), "prepared_sha256": sha, "flags": FLAGS,
                                "python_package": str(snapshot / "python"),
                                "library": str(snapshot / "librust_joern.so")}, snapshot, 120, isolate=False)
        rows = compare(baseline["functions"], candidate["functions"])
        self.assertEqual([(row["function"], row["status"]) for row in rows if row["status"] != "match"], [])
        self.assertEqual(len(rows), 3)

    def replay(self, *arguments):
        with tempfile.TemporaryDirectory(prefix="ddg-replay-test-") as temporary:
            report = Path(temporary) / "report.json"
            process = subprocess.run(
                [sys.executable, str(ROOT / "scripts/replay_ddg_projection.py"), *arguments,
                 "--library", str(LIBRARY), "--output", str(report)], capture_output=True, text=True)
            return process, json.loads(report.read_text()) if report.exists() else None

    def test_projection_replays_frozen_original_capture(self):
        process, report = self.replay(str(SOURCE), "--capture", str(CAPTURE))
        self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
        self.assertEqual(report["summary"]["generator"], "pyjoern 4.0.150.4 / Joern v4.0.150")
        rows = {row["name"]: row for row in report["functions"]}
        self.assertEqual(set(rows), {"<global>", "walk_suffix", "shift_window", "fold_pairs"})
        for name, row in rows.items():
            with self.subTest(function=name):
                self.assertTrue(row["replay_equal"] and row["order_equal"] and row["pass"])
                if name != "<global>":
                    # The fixture has no hash-ordered group, so the native
                    # stored order and DOT equal this capture's exactly.
                    self.assertTrue(row["raw_equal"] and row["cfg_order_equal"] and row["native_equal"])
                    self.assertEqual(set(row["in_order"]), {"same"})

    def test_capture_of_other_bytes_is_rejected(self):
        with tempfile.TemporaryDirectory(prefix="ddg-replay-test-") as temporary:
            other = Path(temporary) / "other.c"
            other.write_text(SOURCE.read_text() + "\n")
            process, report = self.replay(str(other), "--capture", str(CAPTURE))
        self.assertNotEqual(process.returncode, 0)
        self.assertIsNone(report)
        self.assertIn("not an original capture of these source bytes", process.stderr)


if __name__ == "__main__":
    unittest.main()
