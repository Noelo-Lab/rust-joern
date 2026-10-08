"""DDG parity requires statement identities and directed edge labels, not counts."""
from pathlib import Path
import copy
import hashlib
import json
import shutil
import sys
import tempfile
from types import SimpleNamespace
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from compare_ddg_pyjoern import CANDIDATE_WORKER, FLAGS, SERIALIZERS, SOURCE_SUFFIXES, compare, directory_digest, directory_manifest, invoke, isomorphic, semantic_diff, validate_reference


def graph(identities, edges, representation="directed_labeled_multigraph"):
    return {"representation": representation,
            "nodes": [{"id": node, "identity": identity} for node, identity in identities.items()],
            "edges": [{"source": u, "target": v, "label": label} for u, v, label in edges]}


def fixture(ddg):
    return {"public": ddg, "dot": ddg}


class DdgComparisonTests(unittest.TestCase):
    def test_same_counts_different_statement_identity_fails(self):
        left = graph({0: "x = a", 1: "return x"}, [(0, 1, "x")])
        right = graph({0: "x = b", 1: "return x"}, [(0, 1, "x")])
        self.assertFalse(isomorphic(left, right))

    def test_edge_direction_and_variable_label_are_checked(self):
        expected = graph({0: "x = a", 1: "return x"}, [(0, 1, "x")])
        self.assertFalse(isomorphic(expected, graph({0: "x = a", 1: "return x"}, [(1, 0, "x")])))
        self.assertFalse(isomorphic(expected, graph({0: "x = a", 1: "return x"}, [(0, 1, "a")])))

    def test_parallel_label_multiplicity_is_preserved(self):
        expected = graph({0: "assignment", 1: "METHOD_RETURN"}, [(0, 1, "a"), (0, 1, "a"), (0, 1, "b")])
        candidate = graph({8: "assignment", 9: "METHOD_RETURN"}, [(8, 9, "a"), (8, 9, "b"), (8, 9, "b")])
        self.assertFalse(isomorphic(expected, candidate))
        candidate["edges"][2]["label"] = "a"
        self.assertTrue(isomorphic(expected, candidate))

    def test_duplicate_statement_identities_do_not_collapse_topology(self):
        # Identical statements at one source line are distinct vertices. These
        # graphs share even the counters of identity-to-identity edges.
        identities = dict.fromkeys(range(4), "x = 1 at line 7")
        expected = graph(identities, [(0, 1, "x"), (1, 2, "x"), (2, 3, "x"), (3, 0, "x")])
        candidate = graph(identities, [(0, 1, "x"), (1, 0, "x"), (2, 3, "x"), (3, 2, "x")])
        self.assertEqual(semantic_diff(expected, candidate)["missing_edges"], [])
        self.assertFalse(isomorphic(expected, candidate))

    def test_node_id_relabeling_preserves_statement_attributes(self):
        identity = {"statements": [{"class": "Assignment", "fields": {"raw_text": ["assignment", "x=a"]}}], "entry": False}
        expected = graph({0: identity, 1: {"exit": True}}, [(0, 1, "x")])
        candidate = graph({99: identity, 7: {"exit": True}}, [(99, 7, "x")])
        self.assertTrue(isomorphic(expected, candidate))
        candidate["nodes"][1]["identity"]["exit"] = False
        self.assertFalse(isomorphic(expected, candidate))

    def test_isolated_nodes_and_none_are_not_discarded(self):
        self.assertFalse(isomorphic(graph({0: "isolated"}, []), graph({}, [])))
        self.assertFalse(isomorphic(None, graph({}, [])))
        self.assertTrue(isomorphic(None, None))

    def test_invalid_graphs_are_rejected(self):
        with self.assertRaisesRegex(ValueError, "absent node"):
            isomorphic(graph({0: "x"}, [(0, 1, "x")]), graph({0: "x"}, []))
        duplicate = graph({0: "x"}, [])
        duplicate["nodes"].append({"id": 0, "identity": "y"})
        with self.assertRaisesRegex(ValueError, "duplicate node"):
            isomorphic(duplicate, duplicate)

    def test_public_and_labeled_projection_both_must_match(self):
        expected = graph({0: "a", 1: "b"}, [(0, 1, "x")])
        candidate = graph({0: "a", 1: "b"}, [(0, 1, "y")])
        row = compare({"f": fixture(expected)}, {"f": {"public": expected, "dot": candidate}})[0]
        self.assertEqual(row["status"], "divergent")
        self.assertTrue(row["public_match"])
        self.assertFalse(row["labeled_dot_match"])
        self.assertEqual(compare({"f": fixture(expected)}, {})[0]["status"], "missing")
        self.assertEqual(compare({}, {"f": fixture(expected)})[0]["status"], "extra")

    def test_original_dot_semantics_keep_code_and_decode_entities(self):
        namespace = {}
        exec(SERIALIZERS, namespace)
        identity = namespace["label_identity"]("(&lt;operator&gt;.lessThan,x &lt; 3)<SUB>7</SUB>")
        self.assertEqual(identity, {"dot_label": "(<operator>.lessThan,x < 3)<SUB>7</SUB>"})

    def test_native_edge_labels_preserve_literal_html_entity_text(self):
        namespace = {}
        exec(SERIALIZERS, namespace)
        view = {"nodes": [{"id": 0}, {"id": 1}],
                "edges": [{"source": 0, "target": 1, "label": '"&lt;"'}]}
        actual = namespace["native_projection"](view, {0: "(CALL,sink)", 1: "(METHOD_RETURN,int)"})
        self.assertEqual(actual["edges"][0]["label"], '"&lt;"')
        escaped_source = graph({0: {"dot_label": "(CALL,sink)"}, 1: {"dot_label": "(METHOD_RETURN,int)"}}, [(0, 1, '"<"')])
        self.assertFalse(isomorphic(actual, escaped_source))

    def test_directory_hash_and_function_key_preserve_full_relative_paths(self):
        # Same basenames and function names in separate directories must be
        # separate compared functions; renaming an input changes the binding.
        files = {"left/input.c": b"int f(void) { return 1; }", "right/input.c": b"int f(void) { return 2; }"}
        self.assertEqual(directory_digest(files), directory_digest(dict(reversed(list(files.items())))))
        renamed = {"renamed/input.c": files["left/input.c"], "right/input.c": files["right/input.c"]}
        self.assertNotEqual(directory_digest(files), directory_digest(renamed))
        namespace = {}
        exec(SERIALIZERS, namespace)
        source = Path("/temporary/input")
        absolute = SimpleNamespace(name="f", filename=source / "left/input.c")
        relative = SimpleNamespace(name="f", filename=Path("left/input.c"))
        other = SimpleNamespace(name="f", filename=Path("right/input.c"))
        self.assertEqual(namespace["function_key"](absolute, source), namespace["function_key"](relative, source))
        self.assertNotEqual(namespace["function_key"](relative, source), namespace["function_key"](other, source))

    def test_frozen_reference_provenance_and_both_input_hashes_are_required(self):
        g = graph({0: "x"}, [])
        value = {"schema_version": 1, "generator": "pyjoern 4.0.150.4 / Joern v4.0.150",
                 "source_sha256": "source", "prepared_sha256": "prepared", "language": "c", "parse_flags": FLAGS,
                 "reference_module": "/original/pyjoern/__init__.py", "oracle_file_sha256": {"a": "b"},
                 "functions": {"f": {"public": g, "supplemental_public": g}}}
        validate_reference(value, "source", "prepared", "c")
        for field in ("source_sha256", "prepared_sha256", "language", "parse_flags", "reference_module", "oracle_file_sha256"):
            invalid = copy.deepcopy(value)
            del invalid[field]
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_reference(invalid, "source", "prepared", "c")
        invalid = copy.deepcopy(value)
        invalid["functions"]["f"]["supplemental_public"] = graph({0: "different"}, [])
        with self.assertRaisesRegex(ValueError, "captures disagree"):
            validate_reference(invalid, "source", "prepared", "c")

    def test_directory_reference_manifest_is_bound_to_names_and_hashes(self):
        files = {"left/input.c": b"return 1;", "right/input.c": b"return 2;"}
        sha = directory_digest(files)
        g = graph({0: "x"}, [])
        value = {"schema_version": 1, "generator": "pyjoern 4.0.150.4 / Joern v4.0.150",
                 "source_sha256": sha, "prepared_sha256": sha, "language": "mixed_directory", "parse_flags": FLAGS,
                 "input_kind": "directory", "source_inputs": directory_manifest(files), "prepared_inputs": directory_manifest(files),
                 "reference_module": "/original/pyjoern/__init__.py", "oracle_file_sha256": {"a": "b"},
                 "functions": {"f": {"public": g, "supplemental_public": g}}}
        validate_reference(value, sha, sha, "mixed_directory", "directory")
        for field in ("source_inputs", "prepared_inputs"):
            invalid = copy.deepcopy(value)
            invalid[field][0]["filename"] = "renamed/input.c"
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "manifest hash differs"):
                validate_reference(invalid, sha, sha, "mixed_directory", "directory")

    def test_frozen_sources_and_independent_public_captures(self):
        references = ROOT / "tests/fixtures/ddg-parity/references"
        primary = {"control.c", "expressions.c", "functions.cpp", "flow.c", "extra.cpp", "semantics.c", "builtins.c",
                   "bits.c", "libgzip_a-stripslash.c", "patterns.c", "patterns.cpp"}
        captured = {p.name.removesuffix(".ddg.pyjoern.json") for p in references.glob("*.ddg.pyjoern.json")}
        self.assertTrue(primary <= captured, "A focused original DDG reference is missing")
        for path in references.rglob("*.ddg.pyjoern.json"):
            value = json.loads(path.read_text())
            # Capture paths document the source worktree but references can be
            # checked after the work is moved to another checkout.
            prefix, marker, suffix = value["source"].partition("/tests/")
            source = ROOT / "tests" / suffix if marker else Path(value["source"])
            if source.is_dir():
                source_sha = directory_digest({p.relative_to(source).as_posix(): p.read_bytes()
                    for p in source.rglob("*") if p.is_file() and p.suffix in SOURCE_SUFFIXES})
            else:
                source_sha = hashlib.sha256(source.read_bytes()).hexdigest() if source.exists() else value["source_sha256"]
            with self.subTest(reference=path.name):
                validate_reference(value, source_sha, value["prepared_sha256"], value["language"], value.get("input_kind", "file"))
                self.assertEqual(value["package_versions"]["pyjoern"], "4.0.150.4")
                self.assertIn("public_parse_source_seconds", value["timings"])


class NativeAuditDdgParityTests(unittest.TestCase):
    """Exercise source -> native API, independently of original CPG replay.

    Build the library with ``cargo build --lib`` before running these gates.
    Candidate library/wrapper bytes are frozen once for all twelve test cases.
    """

    @classmethod
    def setUpClass(cls):
        library = ROOT / "target/debug/librust_joern.so"
        if not library.exists():
            raise unittest.SkipTest("Native library unavailable; run cargo build --lib")
        cls.temporary = tempfile.TemporaryDirectory(prefix="ddg-audit-native-tests-")
        cls.snapshot = Path(cls.temporary.name)
        cls.library = cls.snapshot / "librust_joern.so"
        shutil.copyfile(library, cls.library)
        shutil.copytree(ROOT / "python", cls.snapshot / "python", ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def assert_source_matches(self, filename, group):
        source = ROOT / "tests/fixtures/dataflow-audit" / filename
        reference = ROOT / "tests/fixtures/ddg-parity/references" / group / (filename + ".ddg.pyjoern.json")
        baseline = json.loads(reference.read_text())
        sha = hashlib.sha256(source.read_bytes()).hexdigest()
        validate_reference(baseline, sha, sha, "cpp" if source.suffix == ".cpp" else "c")
        with tempfile.TemporaryDirectory(prefix="ddg-audit-input-") as temporary:
            cwd = Path(temporary)
            prepared = cwd / source.name
            prepared.write_bytes(source.read_bytes())
            candidate = invoke(sys.executable, CANDIDATE_WORKER,
                {"source": str(prepared), "prepared_sha256": sha, "flags": FLAGS,
                 "python_package": str(self.snapshot / "python"), "library": str(self.library)}, cwd, 120, isolate=False)
        rows = compare(baseline["functions"], candidate["functions"])
        failed = [(row["function"], row["status"], row.get("mismatch_kind")) for row in rows if row["status"] != "match"]
        self.assertEqual(failed, [], f"Exact source/public/labeled DDG parity failed for {filename}: {failed}")


def _native_audit_case(filename, group):
    def run(self):
        self.assert_source_matches(filename, group)
    return run


for _filename in ("type-metadata.c", "type-metadata.cpp", "type-detail.c", "type-detail.cpp"):
    setattr(NativeAuditDdgParityTests, "test_" + _filename.replace("-", "_").replace(".", "_"),
            _native_audit_case(_filename, "audit-types"))
for _filename in ("context.c", "context.cpp", "control.c", "cpp_calls.cpp", "globals.cpp", "operators.c"):
    setattr(NativeAuditDdgParityTests, "test_" + _filename.replace(".", "_"), _native_audit_case(_filename, "audit-sources"))
for _filename in ("assignment-operators.c", "constructors.cpp"):
    setattr(NativeAuditDdgParityTests, "test_" + _filename.replace("-", "_").replace(".", "_"),
            _native_audit_case(_filename, "audit-extensions"))


if __name__ == "__main__":
    unittest.main()
