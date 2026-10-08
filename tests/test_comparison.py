"""The corpus comparator must not confuse matching counts with CFG parity."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from compare_pyjoern import compare, isomorphic
from compare_decbench import cfg_option_contract, summarize


def cfg(nodes, edges, entry=(), exit_=(), degenerate=False):
    return {"nodes": nodes, "edges": edges, "entry": list(entry),
            "exit": list(exit_), "degenerate": degenerate}


class ComparisonTests(unittest.TestCase):
    def test_candidate_option_contracts_are_exact_and_distinct(self):
        self.assertEqual(cfg_option_contract({"strict": True, "data_flow": False}), "historical_cfg_only")
        self.assertEqual(cfg_option_contract({"strict": True, "data_flow": False, "preprocessed": True}), "preprocessed_cfg_only")
        for invalid in (None, {}, {"strict": False, "data_flow": False},
                        {"strict": True, "data_flow": True},
                        {"strict": 1, "data_flow": False},
                        {"strict": True, "data_flow": 0},
                        {"strict": True, "data_flow": False, "preprocessed": False},
                        {"strict": True, "data_flow": False, "preprocessed": 1},
                        {"strict": True, "data_flow": False, "extra": True}):
            with self.subTest(options=invalid), self.assertRaises(ValueError):
                cfg_option_contract(invalid)

    def test_same_counts_different_topology_fails(self):
        left = cfg(list(range(4)), [[0, 1], [0, 2], [1, 3], [2, 3]], [0], [3])
        right = cfg(list(range(4)), [[0, 1], [1, 2], [1, 3], [2, 3]], [0], [3])
        self.assertFalse(isomorphic(left, right))

    def test_relabeling_preserves_roles_and_cycles(self):
        left = cfg([0, 1, 2], [[0, 1], [1, 1], [1, 2]], [0], [2])
        right = cfg([8, 5, 9], [[9, 5], [5, 5], [5, 8]], [9], [8])
        self.assertTrue(isomorphic(left, right))
        right["entry"] = [5]
        self.assertFalse(isomorphic(left, right))

    def test_single_block_roles_loop_and_degeneracy_are_checked(self):
        left = cfg([0], [], [0], [0], True)
        right = cfg([99], [], [99], [99], True)
        self.assertTrue(isomorphic(left, right))
        right["edges"] = [[99, 99]]
        self.assertFalse(isomorphic(left, right))
        right["edges"] = []
        right["degenerate"] = False
        rows = compare({"f": {"cfg": left}}, {"f": {"cfg": right}})
        self.assertEqual(rows[0]["status"], "divergent")

    def test_empty_and_missing_references_are_distinct(self):
        self.assertEqual(compare({}, {}), [])
        reference = {"f": {"cfg": cfg([0], [], [0], [0], True)}}
        missing = compare(reference, {})[0]
        self.assertEqual(missing["status"], "missing")
        self.assertEqual(missing["reference_graph"], reference["f"]["cfg"])
        extra = compare({}, reference)[0]
        self.assertEqual(extra["status"], "extra")
        self.assertEqual(extra["candidate_graph"], reference["f"]["cfg"])
        with self.assertRaises(ValueError):
            isomorphic(cfg([0], [[0, 1]]), cfg([0], []))

    def test_complete_inventory_does_not_hide_uncompared_inputs(self):
        manifest = {"results_root": "/corpus", "scope": {},
                    "cases": [{"id": "a"}, {"id": "b"}]}
        rows = [{"id": "a", "optimization": "O0", "kind": "source", "status": "match"},
                {"id": "b", "optimization": "O2", "kind": "ida", "status": "reference_error"}]
        rows[0]["requested_option_contract"] = "preprocessed_cfg_only"
        summary = summarize(rows, manifest, 0)
        self.assertEqual(summary["requested_option_contracts"], {"preprocessed_cfg_only": 1, "unavailable": 1})
        self.assertEqual(summary["groups"]["O0/source"]["requested_option_contracts"], {"preprocessed_cfg_only": 1})
        self.assertTrue(summary["all_inputs_accounted_for"])
        self.assertFalse(summary["all_graph_comparisons_completed"])
        self.assertEqual(summary["unresolved_input_errors"], {"reference_error": 1})
        rows[1]["status"] = "usage_error"
        self.assertTrue(summarize(rows, manifest, 0)["all_graph_comparisons_completed"])
        rows[1]["id"] = "a"
        self.assertFalse(summarize(rows, manifest, 0)["all_inputs_accounted_for"])


if __name__ == "__main__":
    unittest.main()
