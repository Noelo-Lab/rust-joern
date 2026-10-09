#!/usr/bin/env python3
"""Fresh, reproducible public CFG captures and a local comparison viewer.

The original runs in an isolated interpreter, without this checkout on its
import path. Cached DecBench references help select readable functions, but
never supply graph or attribute data displayed by the viewer.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
from copy import deepcopy
from datetime import datetime, timezone
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import mimetypes
import os
from pathlib import Path
import random
import secrets
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from urllib.parse import unquote, urlsplit

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_ORACLE = "/home/mahaloz/.virtualenvs/decbench/bin/python"
FLAGS = {"no_metadata": False, "no_cfg": False, "no_ddg": True, "no_ast": True}
SEMANTIC_EXCLUSIONS = ("addr", "idx", "block_fields.id", "statements.fields.id", "statements.fields.kind")

# Used verbatim in both isolated processes. Object-valued NetworkX attributes
# become explicit references to graph nodes, rather than opaque repr strings.
SERIALIZERS = r'''
import hashlib, json, pathlib, sys, time
FIELDS = ("raw_text", "source_line_number", "type", "src", "dst", "ret", "func", "args",
          "name", "arg1", "arg2", "cond", "true", "false", "total_nodes")
def public_fields(obj):
    keys = set(getattr(obj, "__dict__", {}))
    for cls in type(obj).__mro__:
        slots = getattr(cls, "__slots__", ())
        keys.update([slots] if isinstance(slots, str) else slots)
    keys.update(key for key in FIELDS if hasattr(obj, key))
    return {key: getattr(obj, key) for key in sorted(keys) if not key.startswith("_") and not callable(getattr(obj, key))}
def serialize_graph(graph):
    nodes = list(graph)
    index = {id(node): i for i, node in enumerate(nodes)}
    warnings = []
    def value(obj):
        if id(obj) in index:
            return {"$node": index[id(obj)]}
        if obj is None or isinstance(obj, (str, bool, int, float)):
            return obj
        if isinstance(obj, pathlib.Path):
            return str(obj)
        if isinstance(obj, dict):
            return {str(key): value(item) for key, item in obj.items()}
        if isinstance(obj, (list, tuple)):
            return [value(item) for item in obj]
        if isinstance(obj, (set, frozenset)):
            return {"$set": sorted((value(item) for item in obj), key=lambda item: json.dumps(item, sort_keys=True))}
        warnings.append(type(obj).__module__ + "." + type(obj).__name__)
        return {"$type": type(obj).__module__ + "." + type(obj).__name__, "$repr": repr(obj)}
    records = []
    for node in nodes:
        fields = public_fields(node)
        for name in ("statements", "addr", "idx", "is_entrypoint", "is_exitpoint"):
            fields.pop(name, None)
        if hasattr(node, "is_merged_node"):
            fields["is_merged_node"] = node.is_merged_node
        records.append({"id": index[id(node)], "attributes": {
            "block_class": type(node).__name__, "entry": bool(node.is_entrypoint),
            "exit": bool(node.is_exitpoint), "addr": value(node.addr), "idx": value(node.idx),
            "block_fields": value(fields), "statements": [{"class": type(stmt).__name__,
                "text": str(stmt), "fields": value(public_fields(stmt))} for stmt in node.statements],
            "graph_attributes": value(dict(graph.nodes[node]))}})
    return {"representation": type(graph).__name__, "nodes": records,
        "edges": [{"source": index[id(u)], "target": index[id(v)], "attributes": value(dict(attrs))}
                  for u, v, attrs in graph.edges(data=True)],
        "metadata": {"graph_attributes": value(dict(graph.graph)), "serialization_warnings": sorted(set(warnings))}}
def serialize_function(function):
    return {"name": function.name, "fullname": function.fullname, "filename": str(function.filename),
        "signature": function.signature, "return_type": function.return_type,
        "start_line": function.start_line, "end_line": function.end_line, "cfg": serialize_graph(function.cfg)}
'''

REFERENCE_WORKER = SERIALIZERS + r'''
from importlib.metadata import version
config = json.load(sys.stdin)
import pyjoern
module = pathlib.Path(pyjoern.__file__).resolve()
if version("pyjoern") != "4.0.150.4" or getattr(pyjoern, "JOERN_VERSION", None) != "v4.0.150":
    raise RuntimeError("Reference must be unchanged PyJoern 4.0.150.4 / Joern v4.0.150")
if module.is_relative_to(pathlib.Path(config["checkout"]).resolve()):
    raise RuntimeError("Reference imported the candidate checkout")
def fingerprint():
    return {name: hashlib.sha256((module.parent / name).read_bytes()).hexdigest() for name in config["oracle_file_sha256"]}
before = fingerprint()
if before != config["oracle_file_sha256"]:
    raise RuntimeError("Original parser code differs from the frozen independent oracle hashes")
source = pathlib.Path(config["source"])
if hashlib.sha256(source.read_bytes()).hexdigest() != config["prepared_sha256"]:
    raise RuntimeError("Prepared input changed")
started = time.perf_counter()
functions = pyjoern.parse_source(source, **config["flags"])
elapsed = time.perf_counter() - started
if functions is None:
    raise RuntimeError("Original returned no functions")
eligible = {name: serialize_function(function) for name, function in functions.items()
    if function.cfg is not None and config["min_nodes"] <= len(function.cfg) <= config["max_nodes"]}
if fingerprint() != before:
    raise RuntimeError("Original code changed during capture")
print(json.dumps({"functions": eligible, "function_count": len(functions), "eligible_count": len(eligible),
    "seconds": elapsed, "reference_module": str(module), "oracle_file_sha256": before,
    "python_version": sys.version, "versions": {name: version(name) for name in ("pyjoern", "networkx", "cfgutils", "pygraphviz")}}))
'''

CANDIDATE_WORKER = SERIALIZERS + r'''
import os
config = json.load(sys.stdin)
sys.path.insert(0, config["python_package"])
os.environ["RUST_JOERN_LIBRARY"] = config["library"]
import rust_joern
source = pathlib.Path(config["source"])
if hashlib.sha256(source.read_bytes()).hexdigest() != config["prepared_sha256"]:
    raise RuntimeError("Prepared input changed")
started = time.perf_counter()
functions = rust_joern.parse_source(source, **config["flags"], strict=True, preprocessed=True)
elapsed = time.perf_counter() - started
selected = {name: serialize_function(functions[name]) for name in config["names"] if name in functions}
print(json.dumps({"functions": selected, "function_count": len(functions), "seconds": elapsed,
    "module": rust_joern.__file__, "module_sha256": hashlib.sha256(pathlib.Path(rust_joern.__file__).read_bytes()).hexdigest(),
    "python_version": sys.version, "json_decoder": rust_joern._loads.__module__}))
'''


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def invoke(python: str, worker: str, config: dict, cwd: Path, timeout: float) -> dict:
    env = dict(os.environ)
    for key in ("PYTHONPATH", "RUST_JOERN_LIBRARY"):
        env.pop(key, None)
    process = subprocess.Popen([python, "-I", "-c", worker], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd, env=env, start_new_session=True)
    try:
        stdout, stderr = process.communicate(json.dumps(config), timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(process.pid, signal.SIGTERM)
        try:
            process.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
        raise
    if process.returncode:
        raise RuntimeError(f"Capture exited {process.returncode}: {stderr[-12000:]}")
    output = json.loads(stdout)
    if stderr:
        output["stderr"] = stderr
    return output


def semantic_attributes(attributes: dict) -> dict:
    attrs = deepcopy(attributes)
    attrs.pop("addr", None)
    attrs.pop("idx", None)
    attrs.get("block_fields", {}).pop("id", None)
    for stmt in attrs.get("statements", []):
        stmt.get("fields", {}).pop("id", None)
        stmt.get("fields", {}).pop("kind", None)
    return attrs


def remap_refs(value, mapping: dict):
    if isinstance(value, dict):
        if set(value) == {"$node"}:
            return {"$node": mapping[value["$node"]]}
        return {key: remap_refs(item, mapping) for key, item in value.items()}
    if isinstance(value, list):
        return [remap_refs(item, mapping) for item in value]
    return value


def differences(left, right, path="") -> list[dict]:
    result = []
    if isinstance(left, dict) and isinstance(right, dict):
        for key in sorted(set(left) | set(right)):
            child = f"{path}.{key}" if path else key
            if key not in left or key not in right:
                result.append({"path": child, "reference": left.get(key), "candidate": right.get(key),
                               "reference_present": key in left, "candidate_present": key in right})
            else:
                result.extend(differences(left[key], right[key], child))
    elif isinstance(left, list) and isinstance(right, list):
        for i in range(max(len(left), len(right))):
            child = f"{path}[{i}]"
            if i >= len(left) or i >= len(right):
                result.append({"path": child, "reference": left[i] if i < len(left) else None,
                    "candidate": right[i] if i < len(right) else None,
                    "reference_present": i < len(left), "candidate_present": i < len(right)})
            else:
                result.extend(differences(left[i], right[i], child))
    elif type(left) != type(right) or left != right:
        result.append({"path": path, "reference": left, "candidate": right,
                       "reference_present": True, "candidate_present": True})
    return result


def compare_graphs(reference: dict, candidate: dict) -> dict:
    import networkx as nx
    def build(data):
        graph = nx.DiGraph()
        graph.add_nodes_from(node["id"] for node in data["nodes"])
        if len(graph) != len(data["nodes"]):
            raise ValueError("Duplicate node IDs")
        for edge in data["edges"]:
            if edge["source"] not in graph or edge["target"] not in graph:
                raise ValueError("Edge endpoint absent")
            graph.add_edge(edge["source"], edge["target"])
        return graph
    left = {node["id"]: node["attributes"] for node in reference["nodes"]}
    right = {node["id"]: node["attributes"] for node in candidate["nodes"]}
    matcher = nx.algorithms.isomorphism.DiGraphMatcher(build(reference), build(candidate))
    best, score, examined, exhausted = None, None, 0, True
    # Small CFGs rarely have many automorphisms. An explicit limit avoids
    # claiming optimal matching when a heavily symmetric graph exceeds it.
    for mapping in matcher.isomorphisms_iter():
        examined += 1
        rows = [differences(remap_refs(semantic_attributes(attrs), mapping), semantic_attributes(right[mapping[node]]))
                for node, attrs in left.items()]
        current = (sum(bool(row) for row in rows), sum(len(row) for row in rows))
        if score is None or current < score:
            best, score = mapping, current
        if current == (0, 0):
            break  # This proves optimal semantic equality.
        if examined >= 10000:
            exhausted = False
            break
    mapping = best or {}
    rows, raw_rows = [], []
    for node, target in sorted(mapping.items()):
        sem = differences(remap_refs(semantic_attributes(left[node]), mapping), semantic_attributes(right[target]))
        raw = differences(remap_refs(left[node], mapping), right[target])
        if sem:
            rows.append({"reference": node, "candidate": target, "fields": sem})
        if raw:
            raw_rows.append({"reference": node, "candidate": target, "fields": raw})
    def edge_attributes(data, remap):
        return Counter(json.dumps([remap.get(e["source"], e["source"]), remap.get(e["target"], e["target"]),
            remap_refs(e.get("attributes", {}), remap)], sort_keys=True) for e in data["edges"])
    identity = {node: node for node in right}
    edge_equal = best is not None and edge_attributes(reference, mapping) == edge_attributes(candidate, identity)
    graph_diff = differences(remap_refs(reference.get("metadata", {}).get("graph_attributes", {}), mapping),
                             candidate.get("metadata", {}).get("graph_attributes", {})) if best is not None else []
    return {"topology_equal": best is not None, "mapping": [{"reference": n, "candidate": t} for n, t in sorted(mapping.items())],
        "attributes_equal": best is not None and not rows and edge_equal and not graph_diff,
        "raw_attributes_equal": best is not None and not raw_rows and edge_equal and not graph_diff,
        "attribute_differences": rows, "raw_attribute_differences": raw_rows,
        "edge_attributes_equal": edge_equal, "graph_attribute_differences": graph_diff,
        "allocation_fields": list(SEMANTIC_EXCLUSIONS), "mapping_candidates_examined": examined,
        "mapping_optimal": exhausted, "mapping_note": "Minimum semantic field differences among topology isomorphisms; raw identifiers are retained."}


def layout_graph(graph: dict, dot: str = "dot") -> None:
    lines = ["digraph CFG {", 'graph [rankdir=TB, nodesep=0.55, ranksep=0.8];',
             'node [shape=box, width=3.65, height=1.65, fixedsize=true, label=""];']
    lines.extend(f'"n{node["id"]}";' for node in graph["nodes"])
    lines.extend(f'"n{edge["source"]}" -> "n{edge["target"]}";' for edge in graph["edges"])
    lines.append("}")
    result = subprocess.run([dot, "-Tjson"], input="\n".join(lines), capture_output=True, text=True, check=True)
    data = json.loads(result.stdout)
    x0, y0, x1, y1 = map(float, data["bb"].split(","))
    coords = {obj["name"]: tuple(map(float, obj["pos"].split(","))) for obj in data.get("objects", [])}
    for node in graph["nodes"]:
        x, y = coords[f'n{node["id"]}']
        node["position"] = {"x": x - x0 + 50, "y": y1 - y + 50}
    graph["layout"] = {"width": x1 - x0 + 100, "height": y1 - y0 + 100,
                       "node_width": 250, "node_height": 112}


def select_cases(manifest: dict, rng: random.Random, maximum_bytes: int, min_nodes: int, max_nodes: int, each: int) -> tuple[list, list]:
    strata = sorted({(c["language"], c["optimization"], c["kind"]) for c in manifest["cases"]})
    selected, rejects, seen = [], [], set()
    for stratum in strata:
        pool = sorted((c for c in manifest["cases"] if (c["language"], c["optimization"], c["kind"]) == stratum
            and not c.get("metric_excluded") and 400 <= c["prepared_bytes"] <= maximum_bytes), key=lambda c: c["id"])
        rng.shuffle(pool)
        for case in pool:
            if case["prepared_sha256"] in seen:
                continue
            cached = json.loads(Path(case["reference"]).read_text())
            eligible = sum(min_nodes <= len(f["cfg"]["nodes"]) <= max_nodes
                           for f in cached.get("functions", {}).values() if f.get("cfg"))
            if eligible < each:
                rejects.append({"case_id": case["id"], "reason": "cached readability prefilter", "eligible_functions": eligible})
                continue
            selected.append(case)
            seen.add(case["prepared_sha256"])
            break
        else:
            raise ValueError(f"No eligible inputs for stratum {stratum}")
    return selected, rejects


def sample(args) -> None:
    if not shutil.which("dot"):
        raise RuntimeError("Graphviz dot is required to lay out sampled CFGs")
    manifest_path, library, output = args.manifest.resolve(), args.library.resolve(), args.output.resolve()
    manifest = json.loads(manifest_path.read_text())
    seed = int(args.seed) if args.seed is not None else secrets.randbits(64)
    rng = random.Random(seed)
    strata_count = len({(c["language"], c["optimization"], c["kind"]) for c in manifest["cases"]})
    if args.count < strata_count or args.count % strata_count:
        raise ValueError(f"--count must be a positive multiple of {strata_count} to preserve all strata")
    each = args.count // strata_count
    cases, rejected = select_cases(manifest, rng, args.max_bytes, args.min_nodes, args.max_nodes, each)
    output.parent.mkdir(parents=True, exist_ok=True)
    evidence = output.parent / "captures"
    evidence.mkdir(exist_ok=True)
    oracle_hashes = json.loads((ROOT / "tests/fixtures/ddg-parity/references/control.c.ddg.pyjoern.json").read_text())["oracle_file_sha256"]
    guarded = {str(path.relative_to(ROOT)): sha256(path) for path in [ROOT / "Cargo.toml", ROOT / "Cargo.lock",
        ROOT / "python/rust_joern/__init__.py", *sorted((ROOT / "src").glob("*.rs"))]}
    library_hash = sha256(library)
    started = time.perf_counter()
    def capture(case):
        prepared = Path(case["prepared_source"])
        source = Path(case["source"])
        if sha256(source) != case["source_sha256"] or sha256(prepared) != case["prepared_sha256"]:
            raise ValueError(f"Input changed: {case['id']}")
        config = {"checkout": str(ROOT), "source": str(prepared), "prepared_sha256": case["prepared_sha256"],
            "flags": FLAGS, "min_nodes": args.min_nodes, "max_nodes": args.max_nodes,
            "oracle_file_sha256": oracle_hashes}
        with tempfile.TemporaryDirectory(prefix="cfg-compare-original-") as cwd:
            reference = invoke(args.oracle_python, REFERENCE_WORKER, config, Path(cwd), args.timeout)
        return case, config, reference
    # Independent original captures can share two worker slots. Selection and
    # output order stay deterministic for the recorded seed.
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        original_captures = list(pool.map(capture, cases))
    pairs, capture_provenance = [], []
    for i, (case, config, reference) in enumerate(original_captures):
        if len(reference["functions"]) < each:
            raise RuntimeError(f"Fresh original has insufficient readable functions: {case['id']}")
        names = rng.sample(sorted(reference["functions"]), each)
        config.update({"python_package": str(ROOT / "python"), "library": str(library), "names": names})
        candidate = invoke(args.candidate_python, CANDIDATE_WORKER, config, ROOT, args.timeout)
        capture_file = evidence / f"{i:02d}-{case['prepared_sha256']}.json"
        capture_file.write_text(json.dumps({"case": case, "config": config, "reference": reference, "candidate": candidate}, ensure_ascii=False, indent=2) + "\n")
        capture_provenance.append({"case_id": case["id"], "path": str(capture_file), "sha256": sha256(capture_file),
            "reference_seconds": reference["seconds"], "candidate_seconds": candidate["seconds"],
            "reference_function_count": reference["function_count"], "eligible_function_count": reference["eligible_count"],
            "selected_functions": names, "reference_module": reference["reference_module"], "oracle_file_sha256": reference["oracle_file_sha256"],
            "versions": reference["versions"], "reference_python": reference["python_version"], "candidate_python": candidate["python_version"],
            "candidate_module": candidate["module"], "candidate_module_sha256": candidate["module_sha256"], "json_decoder": candidate["json_decoder"]})
        for name in names:
            original = deepcopy(reference["functions"][name])
            native = deepcopy(candidate["functions"].get(name))
            if native is None:
                raise RuntimeError(f"Rust is missing sampled original function {case['id']}::{name}")
            left, right = original.pop("cfg"), native.pop("cfg")
            left["metadata"].update(original)
            right["metadata"].update(native)
            comparison = compare_graphs(left, right)
            layout_graph(left)
            if comparison["topology_equal"]:
                coords = {n["id"]: n["position"] for n in left["nodes"]}
                mapping = {m["candidate"]: m["reference"] for m in comparison["mapping"]}
                for node in right["nodes"]:
                    node["position"] = deepcopy(coords[mapping[node["id"]]])
                right["layout"] = deepcopy(left["layout"])
            else:
                layout_graph(right)
            text = Path(case["prepared_source"]).read_text(errors="replace")
            lines = text.splitlines()
            start, end = original["start_line"], original["end_line"]
            pairs.append({"id": f"pair-{len(pairs):03d}", "name": name, "case_id": case["id"], "project": case["project"],
                "optimization": case["optimization"], "kind": case["kind"], "language": case["language"],
                "source": {"path": case["source"], "sha256": case["source_sha256"],
                    "prepared_path": case["prepared_source"], "prepared_sha256": case["prepared_sha256"],
                    "start_line": start, "end_line": end, "text": "\n".join(lines[max(start - 1, 0):max(end, 0)]),
                    "first_line": max(start, 1), "text_start_line": max(start, 1)},
                "reference": left, "candidate": right, "comparison": comparison})
        print(f"Captured {case['id']}: {len(names)} functions; original {reference['seconds']:.2f}s, native {candidate['seconds']:.4f}s", flush=True)
    if sha256(library) != library_hash or any(sha256(ROOT / path) != value for path, value in guarded.items()):
        raise RuntimeError("Native source or library changed during capture")
    result = {"schema_version": 1, "seed": str(seed), "generated_at": datetime.now(timezone.utc).isoformat(),
        "provenance": {"manifest": str(manifest_path), "manifest_sha256": sha256(manifest_path), "library": str(library),
            "library_sha256": library_hash, "native_source_sha256": guarded, "sampler_sha256": sha256(Path(__file__)),
            "selection": {"algorithm": "Python random.Random shuffle per sorted language/optimization/kind stratum; sample sorted fresh original function names",
                "strata": strata_count, "functions_per_translation_unit": each, "min_nodes": args.min_nodes, "max_nodes": args.max_nodes,
                "max_prepared_bytes": args.max_bytes, "metric_exclusions_removed": True, "cached_prefilter_only": True,
                "requires_attribute_or_topology_agreement": False, "rejected_inputs": rejected}, "parse_flags": FLAGS,
            "candidate_additional_flags": {"strict": True, "preprocessed": True}, "captures": capture_provenance,
            "capture_wall_seconds": time.perf_counter() - started, "original_includes_jvm_startup": True,
            "graphviz_version": subprocess.run(["dot", "-V"], capture_output=True, text=True, check=True).stderr.strip(),
            "allocation_fields": list(SEMANTIC_EXCLUSIONS)}, "pairs": pairs}
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(f"Saved {len(pairs)} pairs to {output} (seed {seed})", flush=True)


def make_handler(data_path: Path, static_root: Path):
    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            path = unquote(urlsplit(self.path).path)
            if path == "/api/data":
                file = data_path
            else:
                relative = path.lstrip("/") or "index.html"
                file = (static_root / relative).resolve()
                if not file.is_relative_to(static_root.resolve()):
                    self.send_error(403)
                    return
            if not file.is_file():
                self.send_error(404)
                return
            content = file.read_bytes()
            content_type = "application/json" if path == "/api/data" else mimetypes.guess_type(str(file))[0] or "application/octet-stream"
            self.send_response(200)
            self.send_header("Content-Type", content_type + ("; charset=utf-8" if content_type.startswith("text/") or content_type == "application/json" else ""))
            self.send_header("Content-Length", str(len(content)))
            self.send_header("Cache-Control", "no-store")
            self.end_headers()
            self.wfile.write(content)
    return Handler


def serve(args) -> None:
    data = args.data.resolve()
    if not data.is_file():
        raise FileNotFoundError(f"Capture data not found: {data}. Run the sample command first.")
    server = ThreadingHTTPServer((args.host, args.port), make_handler(data, ROOT / "tools/cfg-compare"))
    print(f"CFG comparison: http://{args.host}:{server.server_port} ({data})", flush=True)
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    sampler = commands.add_parser("sample", help="Capture random fresh original/native CFG pairs")
    sampler.add_argument("--manifest", type=Path, default=ROOT / "workspace/decbench-parity/manifest.json")
    sampler.add_argument("--output", type=Path, default=ROOT / "workspace/cfg-compare/dataset.json")
    sampler.add_argument("--library", type=Path, default=ROOT / "target/release/librust_joern.so")
    sampler.add_argument("--seed")
    sampler.add_argument("--count", type=int, default=24)
    sampler.add_argument("--min-nodes", type=int, default=3)
    sampler.add_argument("--max-nodes", type=int, default=50)
    sampler.add_argument("--max-bytes", type=int, default=250000)
    sampler.add_argument("--oracle-python", default=DEFAULT_ORACLE)
    sampler.add_argument("--candidate-python", default=sys.executable)
    sampler.add_argument("--workers", type=int, default=2)
    sampler.add_argument("--timeout", type=float, default=300)
    sampler.set_defaults(run=sample)
    server = commands.add_parser("serve", help="Serve saved pairs and viewer locally")
    server.add_argument("--data", type=Path, default=ROOT / "workspace/cfg-compare/dataset.json")
    server.add_argument("--host", default="127.0.0.1")
    server.add_argument("--port", type=int, default=8765)
    server.set_defaults(run=serve)
    args = parser.parse_args()
    args.run(args)


if __name__ == "__main__":
    main()
