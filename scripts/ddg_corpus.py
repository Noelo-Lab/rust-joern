#!/usr/bin/env python3
"""DecBench-scale DDG parity audit against unchanged PyJoern 4.0.150.4.

inventory  prepare every compiled and decompiled input once, by content
capture    freeze the original Function.ddg and dotDdg for each unique input
compare    compare one frozen native build with those references
replay     decide functions that match no frozen export, from one more original run
defaults   check that DecBench's default options differ only as documented
summarize  expand unique inputs back to cases and group the differences

The original parser runs once per unique prepared input, in an isolated
interpreter and working directory. Native output never creates a reference.
Original Joern's DOT projection depends on an unordered node iteration, so a
function can differ between two original parses of the same bytes; `capture
--round` stores additional independent captures for that check.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
import gzip
import hashlib
import json
import multiprocessing as mp
import os
from pathlib import Path
import random
import shutil
import signal
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare_ddg_pyjoern as ddg  # noqa: E402

SCHEMA_VERSION = 1
FLAGS = ddg.FLAGS
GENERATOR = "pyjoern 4.0.150.4 / Joern v4.0.150"
# Each original parse starts short-lived JVMs. These flags only bound their
# compiler and collector threads; `capture --jvm-options ""` uses the defaults.
JVM_OPTIONS = "-XX:ActiveProcessorCount=2 -XX:TieredStopAtLevel=1"
# DecBench prepares its inputs, so it parses them as preprocessed. Comparing
# with an original export also requests Joern's behavior wherever the default
# intentionally differs. Parsing stays permissive: every function is compared,
# and inputs with error diagnostics, which strict parsing rejects, are counted.
DECBENCH_OPTIONS = {"preprocessed": True}
JOERN_OPTIONS = ("joern_loop_conditions", "joern_knr_parameters", "joern_cdt_failures", "joern_cpp_control_flow")
REFERENCE_OPTIONS = {**DECBENCH_OPTIONS, **dict.fromkeys(JOERN_OPTIONS, True)}

# The unchanged `pyjoern.parse_source` runs once. Its FastParser output and the
# record behind each selected Function are observed, not replaced.
CAPTURE_WORKER = ddg.SERIALIZERS + r'''
import gzip, os, types
from importlib.metadata import version
config = json.load(sys.stdin)
import pyjoern
from pyjoern.parsing import fast_parser
from pyjoern.parsing.function import Function
module = pathlib.Path(pyjoern.__file__).resolve()
if version("pyjoern") != "4.0.150.4" or getattr(pyjoern, "JOERN_VERSION", None) != "v4.0.150":
    raise RuntimeError("The reference must be unchanged PyJoern 4.0.150.4 / Joern v4.0.150")
if module.is_relative_to(pathlib.Path(config["checkout"]).resolve()):
    raise RuntimeError("Reference interpreter imported the candidate checkout")
package_files = ["__init__.py", "parsing/fast_parser.py", "parsing/function.py", "cfg/__init__.py",
                 "cfg/jil/lifter.py", "cfg/jil/statement.py", "scala/FastParser.sc"]
def fingerprint():
    return {name: hashlib.sha256((module.parent / name).read_bytes()).hexdigest() for name in package_files}
before = fingerprint()
source = pathlib.Path(config["source"])
if input_digest(source) != config["prepared_sha256"]:
    raise RuntimeError("Prepared bytes changed before oracle parse")
runs, processes = [], []
original_run = fast_parser._run_fast_parser_scala_script
def recording_run(*args, **kwargs):
    raw = original_run(*args, **kwargs)
    runs.append(raw)
    return raw
fast_parser._run_fast_parser_scala_script = recording_run
subprocess_run = fast_parser.subprocess.run
def recording_process(*args, **kwargs):
    process = subprocess_run(*args, **kwargs)
    processes.append({"returncode": process.returncode, "stdout_bytes": len(process.stdout or ""),
                      "stderr_tail": (process.stderr or "")[-6000:]})
    return process
fast_parser.subprocess = types.SimpleNamespace(run=recording_process)
original_init = Function.__init__
def recording_init(self, **record):
    original_init(self, **record)
    self._captured_record = record
Function.__init__ = recording_init
flags = config["flags"]
status, error, parsed = "ok", None, {}
started = time.perf_counter()
try:
    parsed = pyjoern.parse_source(source, **flags)
    if parsed is None:
        status, error, parsed = "oracle_error", "Original PyJoern returned no parse result", {}
except Exception as exception:
    status, error = "oracle_error", f"{type(exception).__name__}: {str(exception)[-6000:]}"
seconds = time.perf_counter() - started
if status == "ok" and len(runs) != 1:
    raise RuntimeError("Expected exactly one original FastParser run")
functions = {}
for f in parsed.values():
    record = f._captured_record
    key = function_key(f, source)
    if key in functions:
        raise RuntimeError("Duplicate public function key " + key)
    functions[key] = {"name": f.name, "fullname": f.fullname, "filename": str(f.filename),
        "start_line": f.start_line, "end_line": f.end_line,
        "public": public_graph(f.ddg), "dot": dot_graph(record["ddg"]), "ddg_dot": record["ddg"]}
if before != fingerprint():
    raise RuntimeError("Oracle code changed during capture")
if status == "ok" and not functions:
    status = "oracle_empty"
value = {"schema_version": 1, "generator": "pyjoern 4.0.150.4 / Joern v4.0.150", "status": status, "error": error,
    "reference_module": str(module), "oracle_file_sha256": before,
    "package_versions": {name: version(name) for name in ("pyjoern", "networkx", "pygraphviz", "cfgutils")},
    "python_version": sys.version, "parse_flags": flags, "jvm_options": os.environ.get("JAVA_TOOL_OPTIONS", ""),
    "prepared_sha256": config["prepared_sha256"], "language": config["language"],
    "timings": {"public_parse_source_seconds": seconds},
    "timing_kind": "one_isolated_file_parse_including_jvm_startup",
    "raw_function_records": len(runs[0]) if runs else None, "oracle_processes": processes,
    "functions": functions}
temporary = config["output"] + ".tmp"
with gzip.open(temporary, "wt", compresslevel=6) as stream:
    json.dump(value, stream, separators=(",", ":"))
os.replace(temporary, config["output"])
print(json.dumps({"status": status, "functions": len(functions), "seconds": seconds, "error": error}))
'''


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> dict:
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(path, "rt") as stream:
        return json.load(stream)


def write_json(path: Path, value, *, indent=None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(path.name + f".tmp.{os.getpid()}")
    opener = gzip.open if path.suffix == ".gz" else open
    with opener(temporary, "wt") as stream:
        json.dump(value, stream, indent=indent, separators=None if indent else (",", ":"))
    temporary.replace(path)


def input_key(case: dict) -> str:
    return f"{case['language']}-{case['prepared_sha256']}"


def unique_inputs(manifest: dict) -> dict[str, dict]:
    """Map each (language, prepared bytes) pair to its path and every case using it."""
    inputs: dict[str, dict] = {}
    for case in manifest["cases"]:
        if "preparation_error" in case:
            continue
        entry = inputs.setdefault(input_key(case), {
            "key": input_key(case), "language": case["language"], "prepared_sha256": case["prepared_sha256"],
            "prepared_source": case["prepared_source"], "prepared_bytes": case["prepared_bytes"], "cases": []})
        entry["cases"].append(case["id"])
    return inputs


# ---------------------------------------------------------------- inventory

def inventory(args) -> int:
    from inventory_decbench import prepare

    root, decbench, output = args.results.resolve(), args.decbench.resolve(), args.output.resolve()
    prepared_dir = output / "prepared"
    prepared_dir.mkdir(parents=True, exist_ok=True)
    optimizations = args.optimizations or sorted(
        path.name for path in root.iterdir() if path.is_dir() and any(path.glob("*/compiled")))
    cases = []
    for opt in optimizations:
        for project in sorted((root / opt).iterdir()):
            if not project.is_dir():
                continue
            for source in sorted((project / "compiled").glob("*")):
                if source.is_file() and source.suffix in (".i", ".ii"):
                    cases.append({"id": f"{opt}/{project.name}/source/{source.name}", "optimization": opt,
                                  "project": project.name, "kind": "source", "source": str(source),
                                  "metric_excluded": None})
            for source in sorted((project / "decompiled").glob("*")):
                if source.is_file() and source.suffix in (".c", ".cpp"):
                    kind = source.name.split("_", 1)[0]
                    cases.append({"id": f"{opt}/{project.name}/{kind}/{source.name}", "optimization": opt,
                                  "project": project.name, "kind": kind, "source": str(source),
                                  "metric_excluded": None})
    print(f"Preparing {len(cases)} inputs with {args.workers} workers", flush=True)
    started = time.perf_counter()
    prepared = []
    with ProcessPoolExecutor(max_workers=args.workers, mp_context=mp.get_context("spawn")) as pool:
        futures = {pool.submit(prepare, case, str(decbench), str(prepared_dir)): case for case in cases}
        for done, future in enumerate(as_completed(futures), 1):
            try:
                prepared.append(future.result())
            except Exception as error:
                prepared.append({**futures[future], "preparation_error": f"{type(error).__name__}: {error}"})
            if done % 2000 == 0 or done == len(cases):
                print(f"Prepared {done}/{len(cases)} ({time.perf_counter() - started:.1f}s)", flush=True)
    manifest = {"schema_version": SCHEMA_VERSION, "results_root": str(root), "decbench_root": str(decbench),
                "decbench_commit": subprocess.run(["git", "-C", str(decbench), "rev-parse", "HEAD"],
                                                  capture_output=True, text=True).stdout.strip(),
                "created_at": datetime.now(timezone.utc).isoformat(),
                "scope": {"optimizations": optimizations, "input_kinds": sorted({case["kind"] for case in cases}),
                          "preparation": "strip_system_headers for .i/.ii; sanitize_decompiled_c then "
                                         "preprocess_decompiled_c for decompiled .c; no loop-head rewrite"},
                "preparation_wall_seconds": time.perf_counter() - started,
                "cases": sorted(prepared, key=lambda case: case["id"])}
    write_json(output / "manifest.json", manifest, indent=1)
    inputs = unique_inputs(manifest)
    failed = sum("preparation_error" in case for case in prepared)
    print(f"Wrote {output / 'manifest.json'}: {len(prepared)} cases, {len(inputs)} unique prepared inputs, "
          f"{failed} preparation errors", flush=True)
    print(dict(Counter(case["kind"] for case in prepared)), flush=True)
    return int(failed > 0)


# ------------------------------------------------------------------ capture

def capture_one(entry: dict, args, references: Path) -> dict:
    output = references / f"{entry['key']}.json.gz"
    row = {"key": entry["key"], "prepared_bytes": entry["prepared_bytes"]}
    if output.exists() and not args.force:
        return {**row, "status": "cached"}
    source = Path(entry["prepared_source"])
    env = dict(os.environ)
    for name in ("PYTHONPATH", "RUST_JOERN_LIBRARY", "JAVA_TOOL_OPTIONS"):
        env.pop(name, None)
    if args.jvm_options:
        env["JAVA_TOOL_OPTIONS"] = args.jvm_options
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix="ddg-corpus-") as temporary:
        cwd = Path(temporary)
        parse_path = cwd / ("input" + source.suffix)
        shutil.copyfile(source, parse_path)
        config = {"source": str(parse_path), "prepared_sha256": entry["prepared_sha256"], "flags": FLAGS,
                  "checkout": str(ROOT), "language": entry["language"], "output": str(output)}
        process = subprocess.Popen([args.reference_python, "-I", "-c", CAPTURE_WORKER], stdin=subprocess.PIPE,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd, env=env,
                                   start_new_session=True)
        try:
            stdout, stderr = process.communicate(json.dumps(config), timeout=args.timeout)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGTERM)
            try:
                process.communicate(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.communicate()
            stdout, stderr = "", f"timeout after {args.timeout}s"
            process.returncode = -1
    row["wall_seconds"] = time.perf_counter() - started
    if process.returncode:
        # A failed worker is an oracle failure for this input, not a parity pass.
        failure = {"schema_version": SCHEMA_VERSION, "generator": GENERATOR, "status": "worker_error",
                   "error": stderr[-8000:], "prepared_sha256": entry["prepared_sha256"],
                   "language": entry["language"], "parse_flags": FLAGS, "jvm_options": args.jvm_options,
                   "functions": {}}
        write_json(output, failure)
        return {**row, "status": "worker_error", "error": stderr[-300:]}
    return {**row, **json.loads(stdout.strip().splitlines()[-1])}


def capture(args) -> int:
    args.manifest = args.manifest.resolve()
    manifest = read_json(args.manifest)
    inputs = unique_inputs(manifest)
    references = args.manifest.parent / ("references" if not args.round else f"references-{args.round}")
    references.mkdir(exist_ok=True)
    selected = list(inputs.values())
    if args.keys:
        wanted = set(Path(args.keys).read_text().split())
        selected = [entry for entry in selected if entry["key"] in wanted]
    random.Random(args.seed).shuffle(selected)
    if args.limit:
        selected = selected[:args.limit]
    pending = [entry for entry in selected if args.force or not (references / f"{entry['key']}.json.gz").exists()]
    print(f"Capturing {len(pending)} of {len(selected)} selected unique inputs into {references} "
          f"with {args.workers} workers; JVM options {args.jvm_options!r}", flush=True)
    started = time.perf_counter()
    statuses = Counter()
    log = (references / "capture.log.jsonl").open("a")
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(capture_one, entry, args, references) for entry in pending]
        for done, future in enumerate(as_completed(futures), 1):
            row = future.result()
            statuses[row["status"]] += 1
            log.write(json.dumps(row) + "\n")
            log.flush()
            if done % 100 == 0 or done == len(futures):
                elapsed = time.perf_counter() - started
                print(f"{done}/{len(futures)} {dict(statuses)} {elapsed:.0f}s "
                      f"eta {elapsed / done * (len(futures) - done):.0f}s", flush=True)
    return 0


# ------------------------------------------------------------------ compare

def refine(identities: dict, edges: list) -> dict:
    """Colour refinement from statement identities and labelled edges.

    The colours are isomorphism invariants, so they may only narrow a search.
    """
    colour = {node: hashlib.blake2b(value.encode(), digest_size=16).digest() for node, value in identities.items()}
    classes = len(set(colour.values()))
    while classes < len(colour):
        outgoing, incoming = defaultdict(list), defaultdict(list)
        for source, target, label in edges:
            outgoing[source].append((label, colour[target]))
            incoming[target].append((label, colour[source]))
        refined = {}
        for node, value in colour.items():
            state = hashlib.blake2b(value, digest_size=16)
            for tag, neighbours in ((b"o", outgoing[node]), (b"i", incoming[node])):
                for label, other in sorted(neighbours):
                    state.update(tag + label.encode() + b"\0" + other)
            refined[node] = state.digest()
        count = len(set(refined.values()))
        colour = refined
        if count == classes:
            break
        classes = count
    return colour


def isomorphic(expected: dict | None, actual: dict | None, budget: float = 20.0) -> bool | None:
    """Exact labelled directed multigraph isomorphism; None when undecided in `budget` CPU seconds."""
    if expected is None or actual is None:
        return expected is actual
    left, left_edges = ddg.graph_parts(expected)
    right, right_edges = ddg.graph_parts(actual)
    if expected.get("invalid_block_nodes") or actual.get("invalid_block_nodes"):
        return False
    if expected["representation"] != actual["representation"]:
        return False
    if Counter(left.values()) != Counter(right.values()) or len(left_edges) != len(right_edges):
        return False
    if Counter(label for _, _, label in left_edges) != Counter(label for _, _, label in right_edges):
        return False
    left_colour, right_colour = refine(left, left_edges), refine(right, right_edges)
    if Counter(left_colour.values()) != Counter(right_colour.values()):
        return False
    if len(set(left_colour.values())) == len(left_colour):
        # Every colour names one node on each side: check that explicit bijection.
        target = {value: node for node, value in right_colour.items()}
        mapping = {node: target[value] for node, value in left_colour.items()}
        return (all(left[node] == right[other] for node, other in mapping.items())
                and Counter((mapping[u], mapping[v], label) for u, v, label in left_edges) == Counter(right_edges))
    import networkx as nx

    def build(identities, colours, edges):
        graph = nx.MultiDiGraph()
        graph.add_nodes_from((node, {"identity": (value, colours[node])}) for node, value in identities.items())
        graph.add_edges_from((u, v, {"label": label}) for u, v, label in edges)
        return graph
    matcher = nx.algorithms.isomorphism.MultiDiGraphMatcher(
        build(left, left_colour, left_edges), build(right, right_colour, right_edges),
        node_match=nx.algorithms.isomorphism.categorical_node_match("identity", None),
        edge_match=lambda a, b: Counter(d["label"] for d in a.values()) == Counter(d["label"] for d in b.values()))

    def expired(signum, frame):
        raise TimeoutError

    # Processor time of this process, so that machine load cannot decide a result.
    previous = signal.signal(signal.SIGVTALRM, expired)
    signal.setitimer(signal.ITIMER_VIRTUAL, budget)
    try:
        return matcher.is_isomorphic()
    except TimeoutError:
        return None
    finally:
        signal.setitimer(signal.ITIMER_VIRTUAL, 0)
        signal.signal(signal.SIGVTALRM, previous)


def difference(expected: dict | None, actual: dict | None, limit: int) -> dict:
    if expected is None or actual is None:
        return {"reference_available": expected is not None, "candidate_available": actual is not None}
    full = ddg.semantic_diff(expected, actual)
    def text(identity):
        return identity.get("dot_label", json.dumps(identity, sort_keys=True))[:400]
    result = {}
    for key in ("missing_nodes", "extra_nodes"):
        result[key] = {"count": sum(item["count"] for item in full[key]),
                       "sample": [text(item["identity"]) for item in full[key][:limit]]}
    for key in ("missing_edges", "extra_edges"):
        result[key] = {"count": sum(item["count"] for item in full[key]),
                       "sample": [[text(item["source"]), text(item["target"]), item["label"][:200]]
                                  for item in full[key][:limit]]}
    return result


_STATE: dict = {}


def compare_init(library: str, package: str) -> None:
    os.environ["RUST_JOERN_LIBRARY"] = library
    sys.path.insert(0, package)
    import rust_joern
    if Path(rust_joern.__file__).resolve().parent.parent != Path(package).resolve():
        raise RuntimeError(f"Imported an unexpected rust_joern: {rust_joern.__file__}")
    namespace: dict = {}
    exec(ddg.SERIALIZERS, namespace)
    _STATE.update(rust_joern=rust_joern, serializers=namespace)


def load_reference(path: Path, entry: dict) -> dict:
    reference = read_json(path)
    if reference.get("generator") != GENERATOR or reference.get("parse_flags") != FLAGS:
        raise ValueError("Reference has an unsupported generator or flags")
    if reference["prepared_sha256"] != entry["prepared_sha256"] or reference["language"] != entry["language"]:
        raise ValueError("Reference does not describe this prepared input")
    return reference


def compare_one(entry: dict, references: str, output: str, options: dict, rounds: list[str]) -> dict:
    rust_joern, serializers = _STATE["rust_joern"], _STATE["serializers"]
    row = {"key": entry["key"], "language": entry["language"], "prepared_bytes": entry["prepared_bytes"],
           "cases": len(entry["cases"])}
    reference_path = Path(references) / f"{entry['key']}.json.gz"
    if not reference_path.exists():
        return {**row, "status": "reference_missing"}
    try:
        reference = load_reference(reference_path, entry)
        # Independent original captures of the same bytes. Joern's projection
        # is not deterministic, so each is an equally valid original export.
        others = [load_reference(path, entry) for path in
                  (Path(directory) / f"{entry['key']}.json.gz" for directory in rounds) if path.exists()]
        others = [other for other in others if other["status"] == "ok"]
        source = Path(entry["prepared_source"])
        if digest(source.read_bytes()) != entry["prepared_sha256"]:
            raise ValueError("Prepared bytes changed")
    except Exception as error:
        return {**row, "status": "reference_error", "error": f"{type(error).__name__}: {error}"}
    row["original_captures"] = 1 + len(others)
    row.update(reference_status=reference["status"], reference_functions=len(reference["functions"]),
               reference_seconds=(reference.get("timings") or {}).get("public_parse_source_seconds"))
    started = time.perf_counter()
    try:
        analysis = rust_joern._analyze(source.read_bytes(), source, data_flow=True, **options)
        row["error_diagnostics"] = sum(d.get("severity") == "error" for d in analysis["diagnostics"])
        parsed = rust_joern.parse_source(source, **FLAGS, **options)
        candidate = {}
        for function in parsed.values():
            view = function.raw.get("ddg_view")
            dot = None
            if view is not None:
                dot = serializers["native_projection"](view, function._ddg_labels(view, function.raw["cpg"]))
            candidate[function.name] = {"public": serializers["public_graph"](function.ddg), "dot": dot}
    except Exception as error:
        return {**row, "status": "candidate_error", "error": f"{type(error).__name__}: {str(error)[:2000]}"}
    row["candidate_seconds"] = time.perf_counter() - started
    row["candidate_functions"] = len(candidate)
    expected = reference["functions"]
    counts, failures = Counter(), []
    for name in sorted(expected.keys() | candidate.keys()):
        if name not in expected or name not in candidate:
            status = "extra" if name not in expected else "missing"
            counts[status] += 1
            failures.append({"function": name, "status": status})
            continue
        public = isomorphic(expected[name]["public"], candidate[name]["public"])
        dot = isomorphic(expected[name]["dot"], candidate[name]["dot"])
        if public and dot:
            counts["match"] += 1
            continue
        captures = [other["functions"][name] for other in others if name in other["functions"]]
        if any(isomorphic(capture["public"], candidate[name]["public"])
               and isomorphic(capture["dot"], candidate[name]["dot"]) for capture in captures):
            counts["match_other_capture"] += 1
            continue
        status = "undecided" if public is None or dot is None else "divergent"
        if status == "divergent" and any(not isomorphic(expected[name]["dot"], capture["dot"]) for capture in captures):
            # The original captures disagree with each other for this function.
            status = "divergent_original_unstable"
        counts[status] += 1
        failure = {"function": name, "status": status, "public_match": public, "labeled_dot_match": dot,
                   "counts": {kind: {"reference": ddg.counts(expected[name][kind]),
                                     "candidate": ddg.counts(candidate[name][kind])} for kind in ("public", "dot")}}
        if not dot:
            failure["dot_diff"] = difference(expected[name]["dot"], candidate[name]["dot"], options_limit(options))
        elif not public:
            failure["public_diff"] = difference(expected[name]["public"], candidate[name]["public"], 3)
        failures.append(failure)
    matched = counts["match"] + counts["match_other_capture"]
    row.update(counts=dict(counts), matched=matched, failed=sum(counts.values()) - matched)
    if reference["status"] != "ok":
        row["status"] = "reference_" + reference["status"]
        row["reference_error"] = (reference.get("error") or "")[-1500:]
    else:
        row["status"] = "match" if not failures else "divergent"
    if failures:
        write_json(Path(output) / "failures" / f"{entry['key']}.json", {**row, "functions": failures})
    return row


def options_limit(options: dict) -> int:
    return 6


def compare(args) -> int:
    args.manifest = args.manifest.resolve()
    manifest = read_json(args.manifest)
    inputs = unique_inputs(manifest)
    references = args.references or args.manifest.parent / "references"
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()) and not args.force:
        raise SystemExit(f"{output} is not empty; choose a new directory or pass --force")
    shutil.rmtree(output, ignore_errors=True)
    (output / "candidate").mkdir(parents=True)
    # Freeze the candidate once so a report cannot mix two builds.
    library = output / "candidate" / args.library.name
    shutil.copyfile(args.library, library)
    package = output / "candidate" / "python"
    shutil.copytree(args.package, package, ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))
    selected = [entry for entry in inputs.values() if (Path(references) / f"{entry['key']}.json.gz").exists()
                or not args.captured_only]
    if args.keys:
        wanted = set(Path(args.keys).read_text().split())
        selected = [entry for entry in selected if entry["key"] in wanted]
    selected.sort(key=lambda entry: -entry["prepared_bytes"])
    options = REFERENCE_OPTIONS
    print(f"Comparing {len(selected)} of {len(inputs)} unique inputs with {args.workers} workers", flush=True)
    started = time.perf_counter()
    rows = []
    with ProcessPoolExecutor(max_workers=args.workers, mp_context=mp.get_context("spawn"),
                             initializer=compare_init, initargs=(str(library), str(package))) as pool:
        rounds = [str(args.manifest.parent / f"references-{name}") for name in args.rounds]
        futures = [pool.submit(compare_one, entry, str(references), str(output), options, rounds)
                   for entry in selected]
        for done, future in enumerate(as_completed(futures), 1):
            rows.append(future.result())
            if done % 1000 == 0 or done == len(futures):
                print(f"{done}/{len(futures)} {time.perf_counter() - started:.0f}s", flush=True)
    rows.sort(key=lambda row: row["key"])
    report = {"schema_version": SCHEMA_VERSION, "generator": GENERATOR, "manifest": str(args.manifest.resolve()),
              "references": str(Path(references).resolve()), "rounds": args.rounds,
              "candidate_options": {**FLAGS, **options},
              "candidate_library": str(args.library.resolve()), "candidate_library_sha256": digest(library.read_bytes()),
              "candidate_python_sha256": {str(path.relative_to(package)): digest(path.read_bytes())
                                          for path in sorted(package.rglob("*.py"))},
              "git_head": subprocess.run(["git", "-C", str(ROOT), "rev-parse", "HEAD"], capture_output=True,
                                         text=True).stdout.strip(),
              "git_dirty": bool(subprocess.run(["git", "-C", str(ROOT), "status", "--porcelain", "--", "src", "python"],
                                               capture_output=True, text=True).stdout.strip()),
              "wall_seconds": time.perf_counter() - started, "inputs": rows}
    write_json(output / "inputs.json.gz", report)
    (output / "unmatched-keys.txt").write_text("".join(
        row["key"] + "\n" for row in rows if row["status"] not in ("match", "reference_missing")))
    return summarize_report(report, manifest, output, quiet=False)


# ------------------------------------------------------------------- replay

UNMATCHED = ("divergent", "divergent_original_unstable", "undecided")


def replay_one(entry: dict, args, output: Path) -> dict:
    """Run replay_ddg_projection.py on one input: one more original run, with its stored edge order."""
    report = output / "reports" / f"{entry['key']}.json"
    if report.exists() and not args.force and not args.recompute:
        return {"key": entry["key"], "status": "cached"}
    capture_path = output / "captures" / f"{entry['key']}.jsonl"
    # A saved original capture is bound to the prepared bytes; only the
    # candidate side is recomputed from it.
    original = (["--capture", str(capture_path)] if capture_path.exists() and not args.force
                else ["--reference-python", args.reference_python, "--save-capture", str(capture_path)])
    command = [sys.executable, str(Path(__file__).with_name("replay_ddg_projection.py")),
               entry["prepared_source"], *original, "--library", str(args.library),
               "--language", entry["language"], "--output", str(report)]
    started = time.perf_counter()
    try:
        process = subprocess.run(command, capture_output=True, text=True, timeout=args.timeout + 600)
    except subprocess.TimeoutExpired:
        return {"key": entry["key"], "status": "timeout"}
    row = {"key": entry["key"], "wall_seconds": time.perf_counter() - started,
           "status": "ok" if report.exists() else "error"}
    if row["status"] == "error":
        row["error"] = (process.stderr or process.stdout)[-1500:]
    return row


def replay(args) -> int:
    """Decide the functions that match no frozen original export.

    A function is `stored_order_only` when the native reaching-definition edges
    equal the original's and the native projection reproduces the original DOT
    from the original's own graph and stored edge order.
    """
    args.manifest = args.manifest.resolve()
    args.library = args.library.resolve()
    manifest = read_json(args.manifest)
    inputs = unique_inputs(manifest)
    kinds = {}
    for case in manifest["cases"]:
        kinds.setdefault(input_key(case), case["kind"])
    output = args.output.resolve()
    (output / "reports").mkdir(parents=True, exist_ok=True)
    (output / "captures").mkdir(exist_ok=True)
    unmatched = {}
    for path in sorted((args.compare / "failures").glob("*.json")):
        failure = read_json(path)
        names = [function["function"] for function in failure["functions"] if function["status"] in UNMATCHED]
        if names:
            unmatched[failure["key"]] = names
    selected = [inputs[key] for key in unmatched]
    if args.fraction < 1:
        # A sample that depends only on the input, so it is stable across comparisons.
        selected = [entry for entry in selected
                    if int(digest(f"{args.seed}:{entry['key']}".encode())[:8], 16) / 2**32 < args.fraction]
    if args.limit:
        random.Random(args.seed).shuffle(selected)
        selected = selected[:args.limit]
    print(f"Replaying {len(selected)} inputs with {sum(len(unmatched[e['key']]) for e in selected)} "
          f"unmatched functions, {args.workers} workers", flush=True)
    started = time.perf_counter()
    statuses = Counter()
    with ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(replay_one, entry, args, output) for entry in selected]
        for done, future in enumerate(as_completed(futures), 1):
            statuses[future.result()["status"]] += 1
            if done % 100 == 0 or done == len(futures):
                print(f"{done}/{len(futures)} {dict(statuses)} {time.perf_counter() - started:.0f}s", flush=True)
    verdicts, weighted, by_kind, rows = Counter(), Counter(), defaultdict(Counter), []
    for entry in selected:
        key = entry["key"]
        report = output / "reports" / f"{key}.json"
        functions = defaultdict(list)
        if report.exists():
            for function in read_json(report)["functions"]:
                functions[function["name"]].append(function)
        for name in unmatched[key]:
            candidates = functions.get(name, [])
            if not report.exists():
                verdict = "replay_error"
            elif not candidates:
                verdict = "not_replayed"
            elif any(candidate["raw_equal"] is False for candidate in candidates):
                verdict = "raw_edges_differ"
            elif all(candidate["pass"] for candidate in candidates):
                verdict = ("matches_this_original_run" if all(c.get("native_equal") for c in candidates)
                           else "stored_order_only")
            else:
                verdict = "replay_differs"
            verdicts[verdict] += 1
            weighted[verdict] += len(entry["cases"])
            by_kind[kinds[key]][verdict] += 1
            rows.append({"key": key, "function": name, "verdict": verdict})
    summary = {"schema_version": SCHEMA_VERSION, "compare": str(args.compare.resolve()),
               "candidate_library_sha256": digest(args.library.read_bytes()),
               "inputs_with_unmatched_functions": len(unmatched), "fraction": args.fraction, "seed": args.seed,
               "inputs": len(selected), "input_statuses": dict(statuses),
               "unmatched_function_instances": sum(verdicts.values()),
               "verdicts": dict(verdicts), "case_weighted_verdicts": dict(weighted),
               "by_kind": {kind: dict(counter) for kind, counter in sorted(by_kind.items())},
               "wall_seconds": time.perf_counter() - started}
    write_json(output / "replay.json.gz", {"summary": summary, "functions": rows})
    write_json(output / "replay-summary.json", summary, indent=1)
    print(json.dumps(summary, indent=1))
    return int(any(verdict not in ("stored_order_only", "matches_this_original_run") for verdict in verdicts))


# ----------------------------------------------------------------- defaults

def defaults_one(entry: dict, options: dict) -> dict:
    """Compare the DDG of DecBench's default options with the Joern-reference options."""
    rust_joern, serializers = _STATE["rust_joern"], _STATE["serializers"]
    row = {"key": entry["key"], "functions": 0, "same": 0, "differences": []}
    source = Path(entry["prepared_source"])

    def analyze(**selected):
        analysis = rust_joern._analyze(data, source, data_flow=True, **selected)
        return [{"fullname": raw["fullname"], "start_line": raw["start_line"], "cpg": raw["cpg"],
                 "ddg_view": raw.get("ddg_view")} for raw in analysis["functions"]]

    def projection(raw):
        view = raw["ddg_view"]
        if view is None:
            return None
        labels = rust_joern.Function._ddg_labels(view, raw["cpg"])
        return serializers["native_projection"](view, labels)

    def same(a, b):
        # Node ids shift when an earlier function changes, so equal graphs may
        # still need the comparison by labels.
        return ((a["ddg_view"] == b["ddg_view"] and a["cpg"] == b["cpg"])
                or bool(isomorphic(projection(a), projection(b))))

    try:
        data = source.read_bytes()
        reference, default = analyze(**options), analyze(**DECBENCH_OPTIONS)
    except Exception as error:
        return {**row, "status": "candidate_error", "error": f"{type(error).__name__}: {str(error)[:500]}"}
    left = {(raw["fullname"], raw["start_line"], position): raw for position, raw in enumerate(reference)}
    right = {(raw["fullname"], raw["start_line"], position): raw for position, raw in enumerate(default)}
    if left.keys() != right.keys():
        # CDT's failures remove or add methods; match the rest by name and line.
        left = {key[:2]: raw for key, raw in left.items()}
        right = {key[:2]: raw for key, raw in right.items()}
        row["only_reference"] = len(left.keys() - right.keys())
        row["only_default"] = len(right.keys() - left.keys())
        left = {key: raw for key, raw in left.items() if key in right}
    row["functions"] = len(left)
    # Keep only the functions that differ: a large input's analyses do not fit several at a time.
    different = {key[:2]: raw for key, raw in left.items() if not same(raw, right[key])}
    row["same"] = len(left) - len(different)
    del reference, default, left, right
    causes = {key: {} for key in different}
    for option in JOERN_OPTIONS if different else ():
        # Attribute each difference: which option, switched off alone, changes
        # this function's Joern-reference graph.
        try:
            single = {(raw["fullname"], raw["start_line"]): raw for raw in analyze(**{**options, option: False})}
            single = {key: raw for key, raw in single.items() if key in different}
        except Exception:
            single = None
        for key, raw in different.items():
            causes[key][option] = single is not None and (key not in single or not same(raw, single[key]))
        del single
    row["differences"] = [{"function": key[0], "line": key[1], **causes[key]} for key in different]
    row["status"] = "ok"
    return row


def defaults(args) -> int:
    args.manifest = args.manifest.resolve()
    inputs = unique_inputs(read_json(args.manifest))
    output = args.output.resolve()
    (output / "candidate").mkdir(parents=True, exist_ok=True)
    library = output / "candidate" / args.library.name
    shutil.copyfile(args.library, library)
    package = output / "candidate" / "python"
    shutil.copytree(args.package, package, dirs_exist_ok=True,
                    ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))
    selected = list(inputs.values())
    if args.keys:
        wanted = set(Path(args.keys).read_text().split())
        selected = [entry for entry in selected if entry["key"] in wanted]
    # Large inputs need several gigabytes each; spread them out instead of starting with all of them.
    random.Random(0).shuffle(selected)
    options = REFERENCE_OPTIONS
    started = time.perf_counter()
    rows = []
    with ProcessPoolExecutor(max_workers=args.workers, mp_context=mp.get_context("spawn"),
                             initializer=compare_init, initargs=(str(library), str(package))) as pool:
        futures = [pool.submit(defaults_one, entry, options) for entry in selected]
        for done, future in enumerate(as_completed(futures), 1):
            rows.append(future.result())
            if done % 2000 == 0 or done == len(futures):
                print(f"{done}/{len(futures)} {time.perf_counter() - started:.0f}s", flush=True)
    differences = [difference for row in rows for difference in row.get("differences", [])]
    summary = {"schema_version": SCHEMA_VERSION, "candidate_library_sha256": digest(library.read_bytes()),
               "reference_options": options, "unique_inputs": len(rows),
               "input_statuses": dict(Counter(row["status"] for row in rows)),
               "function_instances": sum(row["functions"] for row in rows),
               "same_ddg": sum(row["same"] for row in rows), "different_ddg": len(differences),
               "different_by_option": {option: sum(d[option] for d in differences) for option in JOERN_OPTIONS},
               "different_by_several_options": sum(sum(d[option] for option in JOERN_OPTIONS) > 1
                                                   for d in differences),
               "different_unexplained": sum(not any(d[option] for option in JOERN_OPTIONS) for d in differences),
               "methods_only_with_reference_options": sum(row.get("only_reference", 0) for row in rows),
               "methods_only_with_default_options": sum(row.get("only_default", 0) for row in rows),
               "wall_seconds": time.perf_counter() - started}
    write_json(output / "defaults.json.gz", {"summary": summary, "inputs": sorted(rows, key=lambda row: row["key"])})
    write_json(output / "defaults-summary.json", summary, indent=1)
    print(json.dumps(summary, indent=1))
    return int(summary["different_unexplained"] > 0 or set(summary["input_statuses"]) != {"ok"})


# ---------------------------------------------------------------- summarize

def summarize_report(report: dict, manifest: dict, output: Path, quiet: bool) -> int:
    inputs = unique_inputs(manifest)
    by_key = {row["key"]: row for row in report["inputs"]}
    cases = {case["id"]: case for case in manifest["cases"]}
    unique = Counter()
    statuses = Counter(row["status"] for row in report["inputs"])
    case_weighted = Counter()
    for row in report["inputs"]:
        for name, amount in (row.get("counts") or {}).items():
            unique[name] += amount
            case_weighted[name] += amount * len(inputs[row["key"]]["cases"])
    groups: dict[tuple, Counter] = defaultdict(Counter)
    for key, entry in inputs.items():
        row = by_key.get(key)
        for case_id in entry["cases"]:
            case = cases[case_id]
            group = groups[(case["optimization"], case["kind"])]
            group["cases"] += 1
            if row is None:
                group["not_compared"] += 1
                continue
            group["cases_" + row["status"]] += 1
            # DecBench parses strictly: an input with error diagnostics yields no graphs there.
            group["cases_with_error_diagnostics"] += bool(row.get("error_diagnostics"))
            for name, amount in (row.get("counts") or {}).items():
                group["functions_" + name] += amount
    summary = {"schema_version": SCHEMA_VERSION, "candidate_library_sha256": report["candidate_library_sha256"],
               "git_head": report["git_head"], "git_dirty": report["git_dirty"],
               "candidate_options": report["candidate_options"],
               "unique_inputs": len(report["inputs"]), "unique_input_statuses": dict(statuses),
               "unique_input_function_instances": dict(unique),
               "case_weighted_function_instances": dict(case_weighted),
               "cases": sum(len(inputs[row["key"]]["cases"]) for row in report["inputs"]),
               "strict_error_inputs": sum(bool(row.get("error_diagnostics")) for row in report["inputs"]),
               "candidate_seconds_total": sum(row.get("candidate_seconds") or 0 for row in report["inputs"]),
               "reference_seconds_total": sum(row.get("reference_seconds") or 0 for row in report["inputs"]),
               "wall_seconds": report["wall_seconds"],
               "groups": {f"{opt}/{kind}": dict(counter) for (opt, kind), counter in sorted(groups.items())}}
    write_json(output / "summary.json", summary, indent=1)
    if not quiet:
        print(json.dumps({key: value for key, value in summary.items() if key != "groups"}, indent=1))
    return int(any(row["status"] != "match" for row in report["inputs"]))


def summarize(args) -> int:
    report = read_json(args.output / "inputs.json.gz")
    return summarize_report(report, read_json(args.manifest), args.output, quiet=False)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    commands = parser.add_subparsers(dest="command", required=True)
    command = commands.add_parser("inventory", help="prepare DecBench inputs by content")
    command.add_argument("--results", type=Path, required=True)
    command.add_argument("--decbench", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--optimizations", nargs="*")
    command.add_argument("--workers", type=int, default=16)
    command.set_defaults(run=inventory)
    command = commands.add_parser("capture", help="freeze original PyJoern DDG references")
    command.add_argument("--manifest", type=Path, required=True)
    command.add_argument("--reference-python", required=True,
                         help="Isolated Python with unchanged installed PyJoern 4.0.150.4")
    command.add_argument("--workers", type=int, default=8)
    command.add_argument("--timeout", type=float, default=3600)
    command.add_argument("--limit", type=int)
    command.add_argument("--seed", type=int, default=0)
    command.add_argument("--keys", help="file listing the unique input keys to capture")
    command.add_argument("--round", help="store an additional independent capture under references-ROUND")
    command.add_argument("--jvm-options", default=JVM_OPTIONS)
    command.add_argument("--force", action="store_true")
    command.set_defaults(run=capture)
    command = commands.add_parser("compare", help="compare a frozen native build with the references")
    command.add_argument("--manifest", type=Path, required=True)
    command.add_argument("--references", type=Path)
    command.add_argument("--library", type=Path, default=ROOT / "target/release/librust_joern.so")
    command.add_argument("--package", type=Path, default=ROOT / "python")
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--workers", type=int, default=8)
    command.add_argument("--keys", help="file listing the unique input keys to compare")
    command.add_argument("--captured-only", action="store_true", help="skip inputs without a reference")
    command.add_argument("--rounds", nargs="*", default=[],
                         help="additional independent original captures, as named by `capture --round`")
    command.add_argument("--force", action="store_true")
    command.set_defaults(run=compare)
    command = commands.add_parser("replay", help="decide unmatched functions from one more original run")
    command.add_argument("--manifest", type=Path, required=True)
    command.add_argument("--compare", type=Path, required=True, help="a `compare` output directory")
    command.add_argument("--reference-python", required=True,
                         help="Isolated Python with unchanged installed PyJoern 4.0.150.4")
    command.add_argument("--library", type=Path, default=ROOT / "target/release/librust_joern.so")
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--workers", type=int, default=8)
    command.add_argument("--timeout", type=float, default=3600)
    command.add_argument("--fraction", type=float, default=1.0,
                         help="replay this fraction of the inputs with unmatched functions")
    command.add_argument("--limit", type=int)
    command.add_argument("--seed", type=int, default=0)
    command.add_argument("--recompute", action="store_true",
                         help="recompute reports for another library from the saved original captures")
    command.add_argument("--force", action="store_true", help="also capture the original again")
    command.set_defaults(run=replay)
    command = commands.add_parser("defaults", help="check that default options differ only as documented")
    command.add_argument("--manifest", type=Path, required=True)
    command.add_argument("--library", type=Path, default=ROOT / "target/release/librust_joern.so")
    command.add_argument("--package", type=Path, default=ROOT / "python")
    command.add_argument("--output", type=Path, required=True)
    command.add_argument("--workers", type=int, default=8)
    command.add_argument("--keys", help="file listing the unique input keys to check")
    command.set_defaults(run=defaults)
    command = commands.add_parser("summarize", help="rebuild summary.json from a comparison")
    command.add_argument("--manifest", type=Path, required=True)
    command.add_argument("--output", type=Path, required=True)
    command.set_defaults(run=summarize)
    args = parser.parse_args()
    return args.run(args)


if __name__ == "__main__":
    raise SystemExit(main())
