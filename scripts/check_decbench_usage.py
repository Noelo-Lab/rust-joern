#!/usr/bin/env python3
"""Record exact DecBench extraction through the opt-in native PyJoern shim.

Consumes the parity runner's manifest without launching original Joern or
modifying DecBench. Native timing includes the C ABI and JSON decoding; CFG
materialization is measured separately. Strict failures remain failures even
when a secondary permissive parse supplies graphs for comparison.
"""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
import hashlib
import json
import logging
import multiprocessing
import os
from pathlib import Path
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
BLACKLIST = ("<", "+", "*", "(", ">", "JUMPOUT", "__builtin_unreachable")


def initialize(decbench_root: str, cache_root: str) -> None:
    sys.path[:0] = [str(ROOT / "compat"), str(ROOT / "python"), decbench_root]
    os.environ["DECBENCH_CACHE_DIR"] = cache_root
    os.environ["DECBENCH_NO_CACHE"] = "1"
    os.environ.setdefault("RAYON_NUM_THREADS", "2")
    import pyjoern

    if Path(pyjoern.__file__).resolve() != ROOT / "compat/pyjoern/__init__.py":
        raise RuntimeError(f"Expected the opt-in shim, imported {pyjoern.__file__}")
    logging.getLogger("decbench.utils.cfg").setLevel(logging.CRITICAL)


def serialize(graphs: dict) -> dict:
    from decbench.publish.cfg_export import relabel_cfg

    result = {}
    for name, graph in graphs.items():
        nodes, edges, _, entry, exit_, degenerate = relabel_cfg(graph)
        result[name] = {"name": name, "cfg": {
            "nodes": nodes, "edges": edges, "entry": entry,
            "exit": exit_, "degenerate": degenerate,
        }}
    return result


def check_case(case: dict, output_root: str) -> dict:
    import pyjoern
    import rust_joern
    from decbench.utils.cfg import extract_cfgs_from_source

    original_parse = pyjoern.parse_source
    original_analyze = rust_joern._analyze
    original_cfg = rust_joern.Function._cfg
    phase = "primary"
    clocks = {name: {"native": 0.0, "graph": 0.0, "parse": 0.0} for name in ("primary", "fallback")}
    captured = {}
    diagnostics = []

    def timed_analyze(*args, **kwargs):
        if phase == "primary":
            captured["strict"] = kwargs.get("strict", False)
            captured["data_flow"] = kwargs.get("data_flow", False)
            captured["preprocessed"] = kwargs.get("preprocessed", False)
        started = time.perf_counter()
        try:
            data = original_analyze(*args, **kwargs)
            diagnostics[:] = data["diagnostics"]
            return data
        finally:
            clocks[phase]["native"] += time.perf_counter() - started

    def timed_cfg(*args, **kwargs):
        started = time.perf_counter()
        try:
            return original_cfg(*args, **kwargs)
        finally:
            clocks[phase]["graph"] += time.perf_counter() - started

    def capture_parse(path, *args, **kwargs):
        source = Path(path).read_bytes()
        captured.update(source=source, filename=str(path), prepared_sha256=hashlib.sha256(source).hexdigest())
        started = time.perf_counter()
        try:
            parsed = original_parse(path, *args, **kwargs)
            captured["lines"] = {
                function.name: {"start_line": function.start_line, "end_line": function.end_line}
                for function in parsed.values()
            }
            return parsed
        finally:
            clocks[phase]["parse"] += time.perf_counter() - started

    pyjoern.parse_source = capture_parse
    rust_joern._analyze = timed_analyze
    rust_joern.Function._cfg = staticmethod(timed_cfg)
    record = {key: case.get(key) for key in ("id", "optimization", "project", "kind", "source", "metric_excluded", "native_library_sha256")}
    record.update(status="ok", error=None, diagnostics=[], functions={})
    graphs = None
    started = time.perf_counter()
    try:
        graphs = extract_cfgs_from_source(
            Path(case["source"]), sanitize_decompiled=case["kind"] != "source", raise_on_error=True,
        )
        if captured.get("prepared_sha256") != case["prepared_sha256"]:
            raise ValueError("Exact DecBench preparation differs from the manifest")
        if (captured.get("strict") is not True or captured.get("data_flow") is not False
                or captured.get("preprocessed") is not True):
            raise ValueError("DecBench shim must request strict, preprocessed CFG-only analysis")
    except Exception as error:
        record.update(status="parse_error", error=str(error))
    finally:
        elapsed = time.perf_counter() - started

    primary = clocks["primary"]
    record["timings"] = {
        "preparation_seconds": max(0.0, elapsed - primary["parse"]),
        "native_seconds": primary["native"],
        "python_seconds": max(0.0, primary["parse"] - primary["native"]),
        "graph_materialization_seconds": primary["graph"],
        "end_to_end_seconds": elapsed,
    }
    try:
        if graphs is None and "source" in captured:
            phase = "fallback"
            started = time.perf_counter()
            try:
                analysis = rust_joern.parse_code(
                    captured["source"], captured["filename"], strict=False, preprocessed=True,
                )
                graphs = {}
                for function in analysis.functions:
                    if not function.name or function.name.startswith(BLACKLIST) or not function.cfg:
                        continue
                    previous = graphs.get(function.name)
                    if previous is None or len(function.cfg) >= len(previous):
                        graphs[function.name] = function.cfg
                        captured.setdefault("lines", {})[function.name] = {
                            "start_line": function.start_line, "end_line": function.end_line,
                        }
                record["diagnostics_source"] = "permissive_fallback"
            except Exception as error:
                record["fallback_error"] = str(error)
            finally:
                fallback_elapsed = time.perf_counter() - started
                fallback = clocks["fallback"]
                record["fallback_timings"] = {
                    "native_seconds": fallback["native"],
                    "python_seconds": max(0.0, fallback_elapsed - fallback["native"]),
                    "graph_materialization_seconds": fallback["graph"],
                    "end_to_end_seconds": fallback_elapsed,
                }
    finally:
        pyjoern.parse_source = original_parse
        rust_joern._analyze = original_analyze
        rust_joern.Function._cfg = staticmethod(original_cfg)

    record["prepared_sha256"] = captured.get("prepared_sha256")
    record["prepared_matches_manifest"] = record["prepared_sha256"] == case["prepared_sha256"]
    record["parse_suffix"] = Path(captured["filename"]).suffix if "filename" in captured else None
    record["requested_options"] = {key: captured.get(key) for key in ("strict", "data_flow", "preprocessed")}
    record["rayon_threads"] = int(os.environ["RAYON_NUM_THREADS"])
    record["pyjoern_module"] = pyjoern.__file__
    record["diagnostics"] = diagnostics
    started = time.perf_counter()
    record["functions"] = serialize(graphs or {})
    for name, function in record["functions"].items():
        function.update(captured.get("lines", {}).get(name, {}))
    record["timings"]["serialization_seconds"] = time.perf_counter() - started
    output = Path(output_root) / f"{case['id']}.json"
    if not output.resolve().is_relative_to(Path(output_root).resolve()):
        raise ValueError(f"Case id escapes the output directory: {case['id']}")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".tmp")
    temporary.write_text(json.dumps(record, ensure_ascii=False) + "\n", encoding="utf-8")
    temporary.replace(output)
    return {"id": case["id"], "status": record["status"], "error": (record["error"] or "")[:200],
            "metric_excluded": case.get("metric_excluded"), "functions": len(record["functions"]),
            "timings": record["timings"], "output": str(output)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--case", action="append", help="Check only this exact case id (repeatable)")
    parser.add_argument("--limit", type=int, help="Explicit smoke limit; omitted means every case")
    args = parser.parse_args()
    if args.workers < 1 or (args.limit is not None and args.limit < 1):
        parser.error("workers and limit must be positive")
    manifest = json.loads(args.manifest.read_text())
    cases = manifest["cases"]
    available = len(cases)
    if len({case["id"] for case in cases}) != available:
        parser.error("manifest contains duplicate case ids")
    if args.case:
        unknown = set(args.case) - {case["id"] for case in cases}
        if unknown:
            parser.error(f"unknown case ids: {sorted(unknown)}")
        cases = [case for case in cases if case["id"] in set(args.case)]
    if args.limit:
        cases = cases[:args.limit]
    output_root = str(args.output.resolve())
    args.output.mkdir(parents=True, exist_ok=True)
    library = Path(os.environ.get("RUST_JOERN_LIBRARY", ROOT / "target/release/librust_joern.so")).resolve()
    library_digest = hashlib.sha256(library.read_bytes()).hexdigest() if library.is_file() else None
    source_digest = hashlib.sha256()
    for path in sorted([ROOT / "Cargo.toml", ROOT / "Cargo.lock", *(ROOT / "src").glob("*.rs")]):
        if path.is_file():
            source_digest.update(str(path.relative_to(ROOT)).encode())
            source_digest.update(b"\0")
            source_digest.update(path.read_bytes())
    cases = [{**case, "native_library_sha256": library_digest} for case in cases]
    started = time.perf_counter()
    with tempfile.TemporaryDirectory(prefix="rust-joern-usage-cache-") as cache:
        with ProcessPoolExecutor(
            max_workers=args.workers, mp_context=multiprocessing.get_context("spawn"),
            initializer=initialize, initargs=(manifest["decbench_root"], cache),
        ) as executor:
            results = list(executor.map(check_case, cases, [output_root] * len(cases), chunksize=1))
    status_counts = dict(Counter(result["status"] for result in results))
    summary = {
        "schema_version": 1, "manifest": str(args.manifest.resolve()),
        "available_cases": available, "checked_cases": len(results), "complete": len(results) == available,
        "statuses": status_counts, "metric_excluded_cases": sum(bool(case.get("metric_excluded")) for case in cases),
        "coverage": dict(Counter(f"{case['optimization']}/{case['kind']}" for case in cases)),
        "seconds": time.perf_counter() - started,
        "native_library": {"path": str(library), "sha256": library_digest},
        "source_build_fingerprint": source_digest.hexdigest(),
        "required_candidate_options": {"strict": True, "data_flow": False, "preprocessed": True},
        "worker_config": {"workers": args.workers, "start_method": "spawn", "rayon_threads": int(os.environ.get("RAYON_NUM_THREADS", "2")), "cache_disabled": True},
        "timing_notes": "native_seconds includes C ABI/JSON decoding; preparation includes temp-file I/O and cleanup; fallback timings are secondary",
        "case_records": "Per-case JSON files at <case-id>.json under this directory",
    }
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"DecBench usage: {len(results)}/{available} cases, {status_counts}, {summary['seconds']:.3f}s")
    return int(status_counts.get("parse_error", 0) > 0)


if __name__ == "__main__":
    raise SystemExit(main())
