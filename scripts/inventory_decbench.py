#!/usr/bin/env python3
"""Inventory O0/O2 inputs and export content-matched, read-only CFG references."""

from __future__ import annotations

import argparse
from concurrent.futures import ProcessPoolExecutor, as_completed
import hashlib
import json
import multiprocessing as mp
from pathlib import Path
import pickle
import sys
import time


def prepare(case: dict, decbench: str, prepared_dir: str) -> dict:
    sys.path.insert(0, decbench)
    from decbench.utils.cfg import (
        preprocess_decompiled_c, sanitize_decompiled_c, strip_system_headers,
        temp_parse_suffix,
    )

    source = Path(case["source"])
    raw = source.read_bytes()
    case["source_sha256"] = hashlib.sha256(raw).hexdigest()
    case["source_bytes"] = len(raw)
    started = time.perf_counter()
    text = source.read_text(errors="replace")
    if case["kind"] == "source":
        text = strip_system_headers(text)
    else:
        text = preprocess_decompiled_c(sanitize_decompiled_c(text))
    case["preparation_seconds"] = time.perf_counter() - started
    data = text.encode()
    digest = hashlib.sha256(data).hexdigest()
    suffix = temp_parse_suffix(source)
    language = "cpp" if suffix == ".cpp" else "c"
    path = Path(prepared_dir) / f"{language}-{digest}{suffix}"
    if not path.exists():
        temporary = path.with_suffix(path.suffix + f".tmp.{mp.current_process().pid}")
        temporary.write_bytes(data)
        temporary.replace(path)
    case.update(prepared_source=str(path), prepared_sha256=digest,
                prepared_bytes=len(data), language=language)
    return case


def serialize_cached(path: Path, destination: Path, digest: str) -> int:
    from decbench.publish.cfg_export import relabel_cfg

    with path.open("rb") as handle:
        graphs = pickle.load(handle)
    if not isinstance(graphs, dict):
        raise ValueError(f"Source cache is not a function map: {path}")
    functions = {}
    for name, graph in graphs.items():
        nodes, edges, _, entry, exit_, degenerate = relabel_cfg(graph)
        functions[name] = {"name": name, "cfg": {
            "nodes": nodes, "edges": edges, "entry": entry, "exit": exit_,
            "degenerate": degenerate,
        }}
    payload = {"schema_version": 1,
               "generator": "DecBench cached PyJoern source CFGs (version unrecorded)",
               "reference_pickle": str(path),
               "reference_pickle_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
               "source_sha256": digest, "prepared_sha256": digest,
               "seconds": None, "functions": functions}
    destination.write_text(json.dumps(payload, separators=(",", ":")) + "\n")
    return len(functions)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--results", type=Path, required=True)
    parser.add_argument("--decbench", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()
    root, decbench = args.results.resolve(), args.decbench.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    prepared_dir, references_dir = output / "prepared", output / "references"
    prepared_dir.mkdir(exist_ok=True)
    references_dir.mkdir(exist_ok=True)
    sys.path.insert(0, str(decbench))
    cases = []
    for opt in ("O0", "O2"):
        for project in sorted((root / opt).iterdir()):
            if not project.is_dir():
                continue
            compiled = project / "compiled"
            for source in sorted(compiled.glob("*")):
                if not source.is_file() or source.suffix not in (".i", ".ii"):
                    continue
                excluded = None
                if "conftest" in source.name or source.name.startswith("a-"):
                    excluded = "DecBench reeval source-input filter: conftest/a-"
                cases.append({"id": f"{opt}/{project.name}/source/{source.name}",
                              "optimization": opt, "project": project.name,
                              "kind": "source", "source": str(source),
                              "metric_excluded": excluded})
            for source in sorted((project / "decompiled").glob("*")):
                if not source.is_file() or source.suffix not in (".c", ".cpp"):
                    continue
                kind = source.name.split("_", 1)[0]
                if kind not in ("ida", "kuna"):
                    continue
                cases.append({"id": f"{opt}/{project.name}/{kind}/{source.name}",
                              "optimization": opt, "project": project.name,
                              "kind": kind, "source": str(source),
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
                prepared.append({**futures[future], "preparation_error": str(error)})
            if done % 500 == 0 or done == len(cases):
                print(f"Prepared {done}/{len(cases)} ({time.perf_counter() - started:.1f}s)", flush=True)
    exported = {}
    cache_root = root / "ged_src/v1/by_content"
    for case in prepared:
        if "preparation_error" in case:
            continue
        digest = case["prepared_sha256"]
        reference = references_dir / f"{case['language']}-{digest}.pyjoern.json"
        cache = cache_root / f"{digest}.pkl"
        if case["kind"] == "source" and cache.is_file():
            if str(reference) not in exported:
                try:
                    exported[str(reference)] = serialize_cached(cache, reference, digest)
                except Exception as error:
                    case["reference_export_error"] = str(error)
            case["reference_origin"] = "source_cache" if str(reference) in exported else "needs_capture"
            case["reference_pickle"] = str(cache)
        else:
            case["reference_origin"] = "needs_capture"
        case["reference"] = str(reference)
    manifest = {"schema_version": 1, "results_root": str(root),
                "decbench_root": str(decbench),
                "scope": {"optimizations": ["O0", "O2"],
                          "input_kinds": ["source", "ida", "kuna"],
                          "source_selection": "All compiled .i/.ii, including recorded metric exclusions"},
                "preparation_wall_seconds": time.perf_counter() - started,
                "source_reference_unique_contents": len(exported),
                "source_reference_cached_function_count": sum(exported.values()),
                "cases": sorted(prepared, key=lambda case: case["id"])}
    path = output / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Wrote {path}", flush=True)
    print(f"Exported {len(exported)} cached source references; "
          f"{sum(c.get('reference_origin') == 'needs_capture' for c in prepared)} input cases need capture", flush=True)
    return int(any("preparation_error" in case for case in prepared))


if __name__ == "__main__":
    raise SystemExit(main())
