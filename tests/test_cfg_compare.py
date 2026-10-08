"""The viewer must display real attributes and distinguish them from topology."""
from copy import deepcopy
from http.server import ThreadingHTTPServer
import gzip
import json
from pathlib import Path
import shutil
import sys
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import urlopen

import networkx as nx

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from cfg_compare import SERIALIZERS, compare_graphs, differences, layout_graph, make_handler, resolve_data_path, select_cases


def attributes(text, *, entry=False, exit=False, addr=1, idx=0):
    return {"block_class": "Block", "entry": entry, "exit": exit, "addr": addr, "idx": idx,
            "block_fields": {}, "statements": [{"class": "Statement", "text": text,
                "fields": {"raw_text": text, "source_line_number": 7}}], "graph_attributes": {}}


def graph(nodes, edges, metadata=None):
    return {"representation": "DiGraph", "nodes": [{"id": i, "attributes": a} for i, a in nodes.items()],
            "edges": [{"source": u, "target": v, "attributes": a} for u, v, a in edges],
            "metadata": {"graph_attributes": metadata or {}}}


class CfgCompareTests(unittest.TestCase):
    def test_public_serializer_preserves_actual_statement_types_values_and_graph_references(self):
        namespace = {}
        exec(SERIALIZERS, namespace)
        class Assignment:
            def __init__(self):
                self.raw_text = ["assignment", "x = y"]
                self.source_line_number = 19
                self.src, self.dst = "x", "y"
            def __str__(self):
                return "x = y"
        class Block:
            def __init__(self, line):
                self.addr, self.idx = line, 4
                self.statements = [Assignment()]
                self.is_entrypoint, self.is_exitpoint = line == 19, line == 20
                self.is_merged_node = True
        first, second = Block(19), Block(20)
        g = nx.DiGraph(name="actual function")
        g.add_node(first, node=first, label="<literal &lt;>")
        g.add_node(second, node=second)
        g.add_edge(first, second, src=first, dst=second, condition="x < 4")
        result = namespace["serialize_graph"](g)
        node = result["nodes"][0]["attributes"]
        self.assertEqual(node["block_class"], "Block")
        self.assertEqual(node["addr"], 19)
        self.assertEqual(node["idx"], 4)
        self.assertTrue(node["block_fields"]["is_merged_node"])
        self.assertEqual(node["statements"][0], {"class": "Assignment", "text": "x = y",
            "fields": {"raw_text": ["assignment", "x = y"], "source_line_number": 19, "src": "x", "dst": "y"}})
        self.assertEqual(node["graph_attributes"], {"node": {"$node": 0}, "label": "<literal &lt;>"})
        self.assertEqual(result["edges"][0]["attributes"], {"src": {"$node": 0}, "dst": {"$node": 1}, "condition": "x < 4"})
        self.assertEqual(result["metadata"]["graph_attributes"], {"name": "actual function"})
        self.assertEqual(result["metadata"]["serialization_warnings"], [])

    def test_slots_and_candidate_only_fields_are_retained(self):
        namespace = {}
        exec(SERIALIZERS, namespace)
        class Statement:
            __slots__ = ("id", "kind", "raw_text", "source_line_number")
            def __init__(self):
                self.id, self.kind, self.raw_text, self.source_line_number = 42, "RETURN", "return x", 9
        self.assertEqual(namespace["public_fields"](Statement()),
                         {"id": 42, "kind": "RETURN", "raw_text": "return x", "source_line_number": 9})

    def test_allocation_difference_is_visible_but_does_not_change_semantic_badge(self):
        a = graph({0: attributes("x = 1", entry=True), 1: attributes("return x", exit=True)}, [(0, 1, {})])
        b = deepcopy(a)
        b["nodes"][0]["attributes"].update({"addr": 9876, "idx": None, "block_fields": {"id": 400}})
        b["nodes"][0]["attributes"]["statements"][0]["fields"].update({"id": 413, "kind": "CALL"})
        result = compare_graphs(a, b)
        self.assertTrue(result["topology_equal"])
        self.assertTrue(result["attributes_equal"])
        self.assertFalse(result["raw_attributes_equal"])
        fields = result["raw_attribute_differences"][0]["fields"]
        self.assertIn("statements[0].fields.kind", {r["path"] for r in fields})

    def test_statement_class_text_raw_text_line_and_boundary_roles_are_semantic(self):
        a = graph({0: attributes("x = 1", entry=True), 1: attributes("return x", exit=True)}, [(0, 1, {})])
        mutations = [lambda n: n.update(entry=False),
                     lambda n: n["statements"][0].update({"class": "Assignment"}),
                     lambda n: n["statements"][0].update({"text": "x = 2"}),
                     lambda n: n["statements"][0]["fields"].update(raw_text=["x", "1"]),
                     lambda n: n["statements"][0]["fields"].update(source_line_number=8)]
        for mutate in mutations:
            b = deepcopy(a)
            mutate(b["nodes"][0]["attributes"])
            result = compare_graphs(a, b)
            self.assertTrue(result["topology_equal"])
            self.assertFalse(result["attributes_equal"])
            self.assertTrue(result["attribute_differences"])

    def test_mapping_chooses_matching_semantics_among_structural_automorphisms(self):
        a = graph({0: attributes("start", entry=True), 1: attributes("left"), 2: attributes("right"),
                   3: attributes("end", exit=True)}, [(0, 1, {}), (0, 2, {}), (1, 3, {}), (2, 3, {})])
        b = graph({50: attributes("start", entry=True), 52: attributes("right"), 51: attributes("left"),
                   53: attributes("end", exit=True)}, [(50, 52, {}), (50, 51, {}), (52, 53, {}), (51, 53, {})])
        result = compare_graphs(a, b)
        self.assertTrue(result["attributes_equal"])
        self.assertEqual(result["mapping"], [{"reference": 0, "candidate": 50}, {"reference": 1, "candidate": 51},
                                             {"reference": 2, "candidate": 52}, {"reference": 3, "candidate": 53}])
        self.assertTrue(result["mapping_optimal"])

    def test_node_and_edge_object_references_follow_the_mapping(self):
        a0, a1 = attributes("start"), attributes("end")
        a0["graph_attributes"]["node"] = {"$node": 0}
        a1["graph_attributes"]["node"] = {"$node": 1}
        b0, b1 = deepcopy(a0), deepcopy(a1)
        b0["graph_attributes"]["node"] = {"$node": 90}
        b1["graph_attributes"]["node"] = {"$node": 91}
        a = graph({0: a0, 1: a1}, [(0, 1, {"src": {"$node": 0}, "dst": {"$node": 1}})])
        b = graph({90: b0, 91: b1}, [(90, 91, {"src": {"$node": 90}, "dst": {"$node": 91}})])
        self.assertTrue(compare_graphs(a, b)["attributes_equal"])
        b["edges"][0]["attributes"]["condition"] = "candidate only"
        result = compare_graphs(a, b)
        self.assertTrue(result["topology_equal"])
        self.assertFalse(result["edge_attributes_equal"])
        self.assertFalse(result["attributes_equal"])

    def test_graph_attributes_and_absent_values_are_not_silently_normalized(self):
        a = graph({0: attributes("one")}, [], {"name": "original"})
        b = deepcopy(a)
        b["metadata"]["graph_attributes"]["name"] = "candidate"
        result = compare_graphs(a, b)
        self.assertFalse(result["attributes_equal"])
        self.assertEqual(result["graph_attribute_differences"][0]["path"], "name")
        row = differences({}, {"line": None})[0]
        self.assertFalse(row["reference_present"])
        self.assertTrue(row["candidate_present"])

    def test_topology_and_invalid_endpoints_are_checked(self):
        nodes = {i: attributes("same") for i in range(3)}
        a = graph(nodes, [(0, 1, {}), (1, 2, {}), (2, 0, {})])
        b = graph(nodes, [(0, 1, {}), (1, 2, {})])
        self.assertFalse(compare_graphs(a, b)["topology_equal"])
        b["edges"].append({"source": 99, "target": 0, "attributes": {}})
        with self.assertRaisesRegex(ValueError, "absent"):
            compare_graphs(a, b)

    @unittest.skipUnless(shutil.which("dot"), "Graphviz required")
    def test_layout_retains_cycles_and_all_nodes_with_positive_viewbox(self):
        g = graph({0: attributes("a"), 1: attributes("b"), 2: attributes("c")},
                  [(0, 1, {}), (1, 1, {}), (1, 2, {}), (2, 0, {})])
        edges = deepcopy(g["edges"])
        layout_graph(g)
        self.assertEqual(g["edges"], edges)
        for n in g["nodes"]:
            self.assertGreater(n["position"]["x"], 0)
            self.assertGreater(n["position"]["y"], 0)
            self.assertLess(n["position"]["x"], g["layout"]["width"])
            self.assertLess(n["position"]["y"], g["layout"]["height"])
        self.assertEqual(g["layout"]["node_width"], 250)

    def test_seeded_selection_is_reproducible_and_does_not_filter_for_agreement(self):
        import random
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "cached.json"
            path.write_text(json.dumps({"functions": {"f": {"cfg": {"nodes": [0, 1, 2]}}}}))
            cases = [{"id": f"O0/p/source/{i}.i", "language": "c", "optimization": "O0", "kind": "source",
                      "prepared_bytes": 800, "prepared_sha256": str(i), "reference": str(path)} for i in range(6)]
            manifest = {"cases": cases}
            first = select_cases(manifest, random.Random(77), 1000, 3, 50, 1)[0]
            second = select_cases({"cases": list(reversed(cases))}, random.Random(77), 1000, 3, 50, 1)[0]
            self.assertEqual(first, second)
            self.assertEqual(len(first), 1)

    def test_http_server_serves_exact_saved_data_and_static_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            static = root / "static"
            static.mkdir()
            data = root / "dataset.json"
            payload = b'{"pairs": [], "literal": "<tag>"}\n'
            data.write_bytes(payload)
            (static / "index.html").write_text("<html>viewer</html>")
            (static / "app.js").write_text("window.loaded = true;")
            (root / "private.txt").write_text("private")
            server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(data, static))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            base = f"http://127.0.0.1:{server.server_port}"
            try:
                with urlopen(base + "/api/data?uncached=1") as response:
                    self.assertEqual(response.read(), payload)
                    self.assertIn("application/json", response.headers["Content-Type"])
                    self.assertEqual(response.headers["Cache-Control"], "no-store")
                with urlopen(base + "/") as response:
                    self.assertIn(b"viewer", response.read())
                with urlopen(base + "/app.js") as response:
                    self.assertIn(b"window.loaded", response.read())
                with self.assertRaises(HTTPError) as error:
                    urlopen(base + "/%2e%2e/private.txt")
                self.assertEqual(error.exception.code, 403)
                with self.assertRaises(HTTPError) as error:
                    urlopen(base + "/missing.css")
                self.assertEqual(error.exception.code, 404)
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=2)

    def test_default_falls_back_to_gzip_and_api_preserves_exact_json_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            static = root / "tools/cfg-compare"
            static.mkdir(parents=True)
            bundled = static / "samples.json.gz"
            payload = b'{"pairs": [], "source": "\\u00e9 <x>"}\n'
            bundled.write_bytes(gzip.compress(payload, mtime=0))
            self.assertEqual(resolve_data_path(root=root), bundled)
            server = ThreadingHTTPServer(("127.0.0.1", 0), make_handler(bundled, static))
            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                with urlopen(f"http://127.0.0.1:{server.server_port}/api/data") as response:
                    self.assertEqual(response.read(), payload)
                    self.assertIn("application/json", response.headers["Content-Type"])
                    self.assertEqual(int(response.headers["Content-Length"]), len(payload))
            finally:
                server.shutdown()
                server.server_close()
                thread.join(timeout=2)
            generated = root / "workspace/cfg-compare/dataset.json"
            generated.parent.mkdir(parents=True)
            generated.write_bytes(b'{"newer": true}\n')
            self.assertEqual(resolve_data_path(root=root), generated)
            self.assertEqual(resolve_data_path(bundled, root), bundled)


if __name__ == "__main__":
    unittest.main()
