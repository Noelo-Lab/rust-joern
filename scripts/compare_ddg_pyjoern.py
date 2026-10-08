#!/usr/bin/env python3
"""Compare actual PyJoern Function.ddg and its directed labeled DOT projection.

The oracle is unchanged PyJoern 4.0.150.4 in an isolated interpreter/cwd. Two
separate original parses capture the public API and its richer DOT source;
their timings are reported separately. Native output never creates a reference.
Frozen references bind source/prepared hashes, language, flags and oracle code.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
from collections import Counter
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import gzip
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_VERSION = 1
FLAGS = {"no_metadata": True, "no_cfg": False, "no_ddg": False, "no_ast": True}
CXX_SUFFIXES = {".cpp", ".cc", ".cp", ".cxx", ".c++", ".C", ".hh", ".hpp", ".hxx", ".ii"}
SOURCE_SUFFIXES = {".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".C", ".h", ".hh", ".hpp", ".hxx", ".i", ".ii"}

# Execute these functions in each worker, without importing this checkout into
# the original interpreter. Statement IDs and duplicate-line Block indices are
# allocation details; every observable statement field and boundary role is
# part of the identity. Duplicate identities remain separate graph vertices.
SERIALIZERS = r'''
import hashlib, html, json, pathlib, re, sys, time
SOURCE_SUFFIXES = {".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".C", ".h", ".hh", ".hpp", ".hxx", ".i", ".ii"}
FIELDS = ("raw_text", "source_line_number", "type", "src", "dst", "ret", "func", "args",
          "name", "arg1", "arg2", "cond", "true", "false", "total_nodes")
def public_graph(graph):
    if graph is None:
        return None
    nodes = list(graph)
    index = {n: i for i, n in enumerate(nodes)}
    records = []
    invalid = False
    for n in nodes:
        if not hasattr(n, "statements"):
            invalid = True
            identity = {"invalid_node_type": type(n).__name__, "attributes": dict(graph.nodes[n])}
        else:
            identity = {"addr": n.addr, "entry": bool(n.is_entrypoint), "exit": bool(n.is_exitpoint),
                "statements": [{"class": type(s).__name__, "text": str(s),
                    "fields": {key: getattr(s, key) for key in FIELDS if hasattr(s, key)}} for s in n.statements]}
        records.append({"id": index[n], "identity": identity})
    return {"representation": type(graph).__name__, "invalid_block_nodes": invalid,
        "nodes": records, "edges": [{"source": index[u], "target": index[v], "label": ""} for u, v in graph.edges()]}
def label_identity(label):
    # Decode HTML entities without discarding statement kind/code/line. Keeping
    # the complete decoded label avoids ambiguous comma splitting in C code.
    return {"dot_label": html.unescape(str(label))}
def dot_graph(dot):
    import networkx as nx, pygraphviz
    if isinstance(dot, list):
        dot = dot[0] if dot else None
    if not dot:
        return None
    g = nx.nx_agraph.from_agraph(pygraphviz.AGraph(dot))
    nodes = list(g)
    index = {n: i for i, n in enumerate(nodes)}
    return {"representation": "directed_labeled_multigraph", "nodes": [
        {"id": index[n], "identity": label_identity(g.nodes[n].get("label", ""))} for n in nodes],
        "edges": [{"source": index[u], "target": index[v], "label": html.unescape(d.get("label", ""))}
                  for u, v, d in g.edges(data=True)]}
def native_projection(view, labels):
    # Native labels are source bytes, while DOT labels arrived HTML-escaped.
    # Decoding native source again corrupts literal entity text such as "&lt;".
    return {"representation": "directed_labeled_multigraph", "nodes": [
        {"id": n["id"], "identity": label_identity(labels[n["id"]])} for n in view["nodes"]],
        "edges": [{"source": e["source"], "target": e["target"], "label": e.get("label", "") or ""}
                  for e in view["edges"]]}
def input_digest(source):
    if source.is_file():
        return hashlib.sha256(source.read_bytes()).hexdigest()
    records = [(p.relative_to(source).as_posix(), hashlib.sha256(p.read_bytes()).hexdigest())
               for p in sorted(source.rglob("*")) if p.is_file() and p.suffix in SOURCE_SUFFIXES]
    return hashlib.sha256(json.dumps(records, separators=(",", ":")).encode()).hexdigest()
def function_key(function, source):
    if source.is_file():
        return function.name
    filename = pathlib.Path(function.filename)
    if filename.is_absolute():
        filename = filename.relative_to(source)
    # Preserve the entire relative filename, including subdirectories and
    # duplicate basenames. Synthetic original filename "" becomes ".".
    return json.dumps([function.name, filename.as_posix()], separators=(",", ":"))
'''

REFERENCE_WORKER = SERIALIZERS + r'''
from importlib.metadata import version
config = json.load(sys.stdin)
import pyjoern
from pyjoern.parsing.fast_parser import _run_fast_parser_scala_script
from pyjoern.parsing.function import Function
module = pathlib.Path(pyjoern.__file__).resolve()
if version("pyjoern") != "4.0.150.4" or getattr(pyjoern, "JOERN_VERSION", None) != "v4.0.150":
    raise RuntimeError("The reference must be unchanged PyJoern 4.0.150.4 / Joern v4.0.150")
if module.is_relative_to(pathlib.Path(config["checkout"]).resolve()):
    raise RuntimeError("Reference interpreter imported the candidate checkout")
package_files = ["__init__.py", "parsing/fast_parser.py", "parsing/function.py", "cfg/__init__.py",
                 "cfg/jil/lifter.py", "cfg/jil/statement.py", "scala/FastParser.sc"]
fingerprint = {name: hashlib.sha256((module.parent / name).read_bytes()).hexdigest() for name in package_files}
source = pathlib.Path(config["source"])
if input_digest(source) != config["prepared_sha256"]:
    raise RuntimeError("Prepared bytes changed before oracle parse")
flags = config["flags"]
started = time.perf_counter()
parsed = pyjoern.parse_source(source, **flags)
public_seconds = time.perf_counter() - started
if parsed is None:
    raise RuntimeError("Original PyJoern returned no parse result")
started = time.perf_counter()
raw = _run_fast_parser_scala_script(source, **flags)
raw_functions = Function.from_many(raw, ignore_cfg=flags["no_cfg"])
raw_seconds = time.perf_counter() - started
raw_by_key = {(f["name"], str(pathlib.Path(f["filename"])), f["fullname"]): f for f in raw}
functions = {}
for f in parsed.values():
    key = (f.name, str(f.filename), f.fullname)
    original = raw_by_key[key]
    supplemental = raw_functions[f.name, str(f.filename)]
    functions[function_key(f, source)] = {"name": f.name, "fullname": f.fullname, "filename": str(f.filename),
        "start_line": f.start_line, "end_line": f.end_line,
        "public": public_graph(f.ddg), "supplemental_public": public_graph(supplemental.ddg),
        "dot": dot_graph(original["ddg"]), "ddg_dot": original["ddg"], "cfg_dot": original["cfg"]}
if fingerprint != {name: hashlib.sha256((module.parent / name).read_bytes()).hexdigest() for name in package_files}:
    raise RuntimeError("Oracle code changed during capture")
print(json.dumps({"schema_version": 1, "generator": "pyjoern 4.0.150.4 / Joern v4.0.150",
    "reference_module": str(module), "oracle_file_sha256": fingerprint,
    "package_versions": {name: version(name) for name in ("pyjoern", "networkx", "pygraphviz", "cfgutils")},
    "python_version": sys.version, "parse_flags": flags,
    "timings": {"public_parse_source_seconds": public_seconds, "supplemental_raw_parse_seconds": raw_seconds},
    "timing_kind": "two_separate_isolated_file_parses_including_jvm_startup", "functions": functions}))
'''

CANDIDATE_WORKER = SERIALIZERS + r'''
import os
config = json.load(sys.stdin)
sys.path.insert(0, config["python_package"])
os.environ["RUST_JOERN_LIBRARY"] = config["library"]
import rust_joern
source = pathlib.Path(config["source"])
if input_digest(source) != config["prepared_sha256"]:
    raise RuntimeError("Prepared bytes changed before native parse")
started = time.perf_counter()
parsed = rust_joern.parse_source(source, **config["flags"])
parse_seconds = time.perf_counter() - started
started = time.perf_counter()
for f in parsed.values():
    _ = f.ddg
materialization_seconds = time.perf_counter() - started
elapsed = parse_seconds + materialization_seconds
functions = {}
for f in parsed.values():
    view = f.raw.get("ddg_view")
    dot = None
    if view is not None:
        labels = f._ddg_labels(view, f.raw["cpg"])
        dot = native_projection(view, labels)
    functions[function_key(f, source)] = {"name": f.name, "fullname": f.fullname, "filename": str(f.filename),
        "start_line": f.start_line, "end_line": f.end_line,
        "public": public_graph(f.ddg), "dot": dot, "raw_ddg": f.raw.get("ddg")}
print(json.dumps({"module": rust_joern.__file__, "module_sha256": hashlib.sha256(pathlib.Path(rust_joern.__file__).read_bytes()).hexdigest(),
    "seconds": elapsed, "public_parse_source_seconds": parse_seconds,
    "ddg_materialization_seconds": materialization_seconds, "python_version": sys.version, "functions": functions}))
'''


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def directory_manifest(files: dict[str, bytes]) -> list[dict]:
    return [{"filename": name, "sha256": digest(data), "bytes": len(data)} for name, data in sorted(files.items())]


def directory_digest(files: dict[str, bytes]) -> str:
    records = [(name, digest(data)) for name, data in sorted(files.items())]
    return digest(json.dumps(records, separators=(",", ":")).encode())


def invoke(python: str, worker: str, config: dict, cwd: Path, timeout: float, *, isolate=True) -> dict:
    env = dict(os.environ)
    for name in ("PYTHONPATH", "RUST_JOERN_LIBRARY"):
        env.pop(name, None)
    process = subprocess.Popen([python, *(["-I"] if isolate else []), "-c", worker], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd,
        env=env, start_new_session=True)
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
        raise RuntimeError(f"Worker exited {process.returncode}: {stderr[-15000:]}")
    try:
        value = json.loads(stdout)
        if stderr:
            value["worker_stderr"] = stderr
        return value
    except ValueError as error:
        raise ValueError(f"Worker returned invalid JSON: {stdout[-3000:]} {stderr[-3000:]}") from error


def graph_parts(graph: dict) -> tuple[dict, list]:
    identities = {node["id"]: json.dumps(node["identity"], sort_keys=True, separators=(",", ":"))
                  for node in graph["nodes"]}
    if len(identities) != len(graph["nodes"]):
        raise ValueError("DDG contains duplicate node IDs")
    edges = []
    for edge in graph["edges"]:
        source, target = edge["source"], edge["target"]
        if source not in identities or target not in identities:
            raise ValueError("DDG contains an edge to an absent node")
        edges.append((source, target, edge["label"]))
    return identities, edges


def isomorphic(expected: dict | None, actual: dict | None) -> bool:
    """Prove directed multigraph equality with statement identity and labels."""
    if expected is None or actual is None:
        return expected is actual
    left, le = graph_parts(expected)
    right, re_ = graph_parts(actual)
    if expected.get("invalid_block_nodes") or actual.get("invalid_block_nodes"):
        return False
    if expected["representation"] != actual["representation"]:
        return False
    if Counter(left.values()) != Counter(right.values()) or len(le) != len(re_):
        return False
    if Counter(label for _, _, label in le) != Counter(label for _, _, label in re_):
        return False
    if left == right and Counter(le) == Counter(re_):
        return True
    import networkx as nx
    def build(identities, edges):
        g = nx.MultiDiGraph()
        g.add_nodes_from((n, {"identity": value}) for n, value in identities.items())
        g.add_edges_from((u, v, {"label": label}) for u, v, label in edges)
        return g
    return nx.is_isomorphic(build(left, le), build(right, re_),
        node_match=nx.algorithms.isomorphism.categorical_node_match("identity", None),
        edge_match=lambda a, b: Counter(d["label"] for d in a.values()) == Counter(d["label"] for d in b.values()))


def counts(graph: dict | None) -> dict | None:
    if graph is None:
        return None
    return {"nodes": len(graph["nodes"]), "edges": len(graph["edges"]),
            "representation": graph["representation"]}


def semantic_diff(expected: dict | None, actual: dict | None) -> dict:
    if expected is None or actual is None:
        return {"reference_available": expected is not None, "candidate_available": actual is not None}
    left, le = graph_parts(expected)
    right, re_ = graph_parts(actual)
    missing_nodes = Counter(left.values()) - Counter(right.values())
    extra_nodes = Counter(right.values()) - Counter(left.values())
    # Identity edge counters localize differences; equality of these counters
    # is only a diagnostic because repeated identical statements are ambiguous.
    expected_edges = Counter((left[u], left[v], label) for u, v, label in le)
    actual_edges = Counter((right[u], right[v], label) for u, v, label in re_)
    def nodes(counter):
        return [{"identity": json.loads(identity), "count": amount} for identity, amount in sorted(counter.items())]
    def edges(counter):
        return [{"source": json.loads(u), "target": json.loads(v), "label": label, "count": amount}
                for (u, v, label), amount in sorted(counter.items())]
    return {"missing_nodes": nodes(missing_nodes), "extra_nodes": nodes(extra_nodes),
        "missing_edges": edges(expected_edges - actual_edges), "extra_edges": edges(actual_edges - expected_edges)}


def compare(reference: dict, candidate: dict) -> list[dict]:
    rows = []
    for name in sorted(reference.keys() | candidate.keys()):
        row = {"function": name}
        selected = reference.get(name, candidate.get(name))
        if selected and "name" in selected:
            row.update(name=selected["name"], filename=selected.get("filename"))
        if name not in reference or name not in candidate:
            row["status"] = "extra" if name not in reference else "missing"
        else:
            expected, actual = reference[name], candidate[name]
            public = isomorphic(expected["public"], actual["public"])
            dot = isomorphic(expected["dot"], actual["dot"])
            row.update(status="match" if public and dot else "divergent", public_match=public, labeled_dot_match=dot)
            for kind in ("public", "dot"):
                row[kind] = {"reference": counts(expected[kind]), "candidate": counts(actual[kind])}
                if not row["public_match" if kind == "public" else "labeled_dot_match"]:
                    row[kind]["diff"] = semantic_diff(expected[kind], actual[kind])
            if row["status"] != "match":
                if actual["dot"] is None:
                    row["mismatch_kind"] = "projection_unavailable"
                elif row["dot"].get("diff", {}).get("missing_nodes") or row["dot"].get("diff", {}).get("extra_nodes"):
                    row["mismatch_kind"] = "statement_node_semantics"
                elif not dot:
                    row["mismatch_kind"] = "labeled_dependency_edges"
                else:
                    row["mismatch_kind"] = "public_api_semantics"
        if row["status"] != "match":
            row.update(reference=reference.get(name), candidate=candidate.get(name))
        rows.append(row)
    return rows


def validate_reference(value: dict, source_sha256: str, prepared_sha256: str, language: str, input_kind="file") -> None:
    if value.get("schema_version") != SCHEMA_VERSION or value.get("generator") != "pyjoern 4.0.150.4 / Joern v4.0.150":
        raise ValueError("Frozen reference has an unsupported generator/schema")
    for field, expected in (("source_sha256", source_sha256), ("prepared_sha256", prepared_sha256),
                            ("language", language), ("parse_flags", FLAGS)):
        if value.get(field) != expected:
            raise ValueError(f"Frozen reference {field} differs; recapture explicitly")
    if value.get("input_kind", "file") != input_kind:
        raise ValueError("Frozen reference input_kind differs; recapture explicitly")
    if input_kind == "directory":
        for field, expected in (("source_inputs", source_sha256), ("prepared_inputs", prepared_sha256)):
            inputs = value.get(field)
            if not isinstance(inputs, list) or not inputs:
                raise ValueError("Frozen directory reference lacks " + field)
            names = [item["filename"] for item in inputs]
            if len(set(names)) != len(names):
                raise ValueError("Frozen directory reference contains duplicate filenames")
            records = sorted((item["filename"], item["sha256"]) for item in inputs)
            if digest(json.dumps(records, separators=(",", ":")).encode()) != expected:
                raise ValueError("Frozen directory reference manifest hash differs")
    if not value.get("oracle_file_sha256") or not value.get("reference_module"):
        raise ValueError("Frozen reference lacks original oracle provenance")
    for name, function in value["functions"].items():
        if not isomorphic(function["public"], function["supplemental_public"]):
            raise ValueError("Original public/raw captures disagree for " + name)


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(value, indent=2) + "\n"
    if path.suffix == ".gz":
        with gzip.open(path, "wt") as stream:
            stream.write(text)
    else:
        path.write_text(text)


def prepare(source: Path, args) -> bytes:
    if source.suffix not in (".i", ".ii") and not args.sanitize_decompiled:
        return source.read_bytes()
    if not args.decbench:
        raise ValueError("--decbench is required for DecBench preprocessed/decompiled preparation")
    worker = r'''
import json, pathlib, sys
config = json.load(sys.stdin)
sys.path.insert(0, config["decbench"])
from decbench.utils.cfg import strip_system_headers, sanitize_decompiled_c, preprocess_decompiled_c
source = pathlib.Path(config["source"])
text = source.read_text(errors="replace")
if source.suffix in (".i", ".ii"):
    text = strip_system_headers(text)
else:
    text = preprocess_decompiled_c(sanitize_decompiled_c(text))
print(json.dumps({"text": text}))
'''
    with tempfile.TemporaryDirectory(prefix="ddg-prepare-") as temp:
        return invoke(args.reference_python, worker, {"source": str(source), "decbench": str(args.decbench.resolve())},
                      Path(temp), args.timeout)["text"].encode()


def run_source(source: Path, args) -> dict:
    source = source.resolve()
    row = {"source": str(source)}
    phase = "preparation"
    try:
        input_kind = "directory" if source.is_dir() else "file"
        if input_kind == "directory":
            source_files = {p.relative_to(source).as_posix(): p.read_bytes()
                for p in sorted(source.rglob("*")) if p.is_file() and p.suffix in SOURCE_SUFFIXES}
            if not source_files:
                raise ValueError("Directory contains no eligible C/C++ source files")
            prepared_files = {name: prepare(source / name, args) for name in source_files}
            source_sha, prepared_sha = directory_digest(source_files), directory_digest(prepared_files)
            language = "mixed_directory"
            row.update(source_inputs=directory_manifest(source_files), prepared_inputs=directory_manifest(prepared_files))
        else:
            original = source.read_bytes()
            prepared = prepare(source, args)
            source_sha, prepared_sha = digest(original), digest(prepared)
            language = "cpp" if source.suffix in CXX_SUFFIXES else "c"
        row.update(source_sha256=source_sha, prepared_sha256=prepared_sha, language=language, input_kind=input_kind)
        reference_path = args.reference_dir / (source.name + ".ddg.pyjoern.json") if args.reference_dir else None
        with tempfile.TemporaryDirectory(prefix="ddg-parity-") as temporary:
            cwd = Path(temporary)
            if input_kind == "directory":
                parse_path = cwd / source.name
                for name, data in prepared_files.items():
                    target = parse_path / name
                    target.parent.mkdir(parents=True, exist_ok=True)
                    target.write_bytes(data)
            else:
                parse_path = cwd / (source.stem + (".cpp" if language == "cpp" else ".c"))
                parse_path.write_bytes(prepared)
            config = {"source": str(parse_path), "prepared_sha256": prepared_sha,
                      "flags": FLAGS, "checkout": str(ROOT)}
            phase = "reference"
            if reference_path:
                baseline = json.loads(reference_path.read_text())
            else:
                baseline = invoke(args.reference_python, REFERENCE_WORKER, config, cwd, args.timeout)
                baseline.update(source_sha256=source_sha, prepared_sha256=prepared_sha,
                                language=language, input_kind=input_kind, source=str(source), capture_worker_sha256=digest(REFERENCE_WORKER.encode()),
                                captured_at=datetime.now(timezone.utc).isoformat(),
                                preparation={"strip_system_headers": source.suffix in (".i", ".ii"),
                                    "sanitize_decompiled": args.sanitize_decompiled and source.suffix not in (".i", ".ii"),
                                    "decbench": str(args.decbench.resolve()) if args.decbench else None})
                if input_kind == "directory":
                    baseline.update(source_inputs=directory_manifest(source_files), prepared_inputs=directory_manifest(prepared_files))
                    baseline["preparation"]["per_input"] = {name: {
                        "strip_system_headers": Path(name).suffix in (".i", ".ii"),
                        "sanitize_decompiled": args.sanitize_decompiled and Path(name).suffix not in (".i", ".ii")}
                        for name in source_files}
                validate_reference(baseline, source_sha, prepared_sha, language, input_kind)
                if args.save_reference_dir:
                    write_json(args.save_reference_dir / (source.name + ".ddg.pyjoern.json"), baseline)
            validate_reference(baseline, source_sha, prepared_sha, language, input_kind)
            if not baseline["functions"]:
                raise ValueError("Original parser returned no public functions; this is not a parity pass")
            phase = "candidate"
            config.update(python_package=str(args.candidate_package.resolve()), library=str(args.candidate_library.resolve()))
            candidate = invoke(args.candidate_python, CANDIDATE_WORKER, config, cwd, args.timeout, isolate=False)
            phase = "comparison"
            rows = compare(baseline["functions"], candidate["functions"])
            row.update(status="match" if all(r["status"] == "match" for r in rows) else "divergent",
                reference_timings=baseline["timings"], candidate_seconds=candidate["seconds"],
                candidate_timings={"public_parse_source_seconds": candidate["public_parse_source_seconds"],
                    "ddg_materialization_seconds": candidate["ddg_materialization_seconds"]},
                candidate_module=candidate["module"], candidate_module_sha256=candidate["module_sha256"],
                candidate_python_version=candidate["python_version"],
                reference_warnings=baseline.get("worker_stderr", ""), candidate_warnings=candidate.get("worker_stderr", ""),
                reference_functions=len(baseline["functions"]), candidate_functions=len(candidate["functions"]),
                matched=sum(r["status"] == "match" for r in rows), failed=sum(r["status"] != "match" for r in rows), functions=rows)
    except Exception as error:
        row.update(status=phase + "_error", error=str(error))
    return row


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", type=Path, nargs="+")
    parser.add_argument("--reference-python", default=sys.executable,
                        help="Isolated Python with unchanged installed PyJoern 4.0.150.4")
    parser.add_argument("--candidate-python", default=sys.executable)
    parser.add_argument("--candidate-package", type=Path, default=ROOT / "python")
    parser.add_argument("--candidate-library", type=Path, default=ROOT / "target/debug/librust_joern.so")
    parser.add_argument("--reference-dir", type=Path)
    parser.add_argument("--save-reference-dir", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--decbench", type=Path)
    parser.add_argument("--sanitize-decompiled", action="store_true")
    parser.add_argument("--timeout", type=float, default=180)
    parser.add_argument("--jobs", type=int, default=1)
    args = parser.parse_args()
    if args.jobs < 1:
        parser.error("--jobs must be positive")
    names = [p.name for p in args.sources]
    if (args.reference_dir or args.save_reference_dir) and len(set(names)) != len(names):
        parser.error("Frozen references require unique source basenames")
    started = time.perf_counter()
    results = []
    origin_library, origin_package = args.candidate_library.resolve(), args.candidate_package.resolve()
    # Another worker may rebuild or edit the wrapper during a long oracle run.
    # Freeze candidate bytes once so a report cannot mix candidate revisions.
    with tempfile.TemporaryDirectory(prefix="ddg-candidate-snapshot-") as temporary:
        snapshot = Path(temporary)
        args.candidate_library = snapshot / origin_library.name
        args.candidate_package = snapshot / "python"
        shutil.copyfile(origin_library, args.candidate_library)
        shutil.copytree(origin_package, args.candidate_package, ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))
        library_sha = digest(args.candidate_library.read_bytes())
        package_sha = {str(p.relative_to(args.candidate_package)): digest(p.read_bytes()) for p in sorted(args.candidate_package.rglob("*.py"))}
        with ThreadPoolExecutor(max_workers=args.jobs) as pool:
            futures = {pool.submit(run_source, source, args): source for source in args.sources}
            for future in as_completed(futures):
                row = future.result()
                results.append(row)
                print(f"{row['status']}: {Path(row['source']).name}: {row.get('matched', 0)} matched, {row.get('failed', 0)} failed"
                      + (": " + row["error"] if row.get("error") else ""), flush=True)
                report = {"schema_version": SCHEMA_VERSION, "scope": "per-file original Function.ddg plus directed labeled dotDdg; statement identities and multiplicity preserved",
                    "candidate_library": str(origin_library), "candidate_library_sha256": library_sha,
                    "candidate_package": str(origin_package), "candidate_python_sha256": package_sha,
                    "candidate_timing_kind": "parse_source_plus_lazy_ddg_materialization",
                    "seconds": time.perf_counter() - started, "requested_files": len(args.sources), "completed_files": len(results),
                    "files": sorted(results, key=lambda r: r["source"])}
                write_json(args.output, report)
    return int(any(row["status"] != "match" for row in results))


if __name__ == "__main__":
    raise SystemExit(main())
