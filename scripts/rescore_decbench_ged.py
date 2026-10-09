#!/usr/bin/env python3
"""Rescore saved DecBench C artifacts with a frozen native parser and unchanged GED.

All generated caches, scores, timing records, and baseline copies live in a new
output tree. Compilation, decompilation, and historical Joern replay are absent.
"""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor, as_completed
from datetime import datetime, timezone
import hashlib
from importlib.metadata import version
import json
import logging
import math
import multiprocessing
import os
from pathlib import Path
import pickle
import re
import shutil
import statistics
import sys
import time
import traceback

ROOT = Path(__file__).resolve().parents[1]
RUN = {}
PHASE = None


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for part in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(part)
    return digest.hexdigest()


def atomic_json(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_text(json.dumps(value, ensure_ascii=False, allow_nan=False) + "\n")
    temp.replace(path)


def write_pickle(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    temp = path.with_suffix(path.suffix + ".tmp")
    temp.write_bytes(pickle.dumps(value, protocol=pickle.HIGHEST_PROTOCOL))
    temp.replace(path)


def initialize(config):
    global RUN
    RUN = config
    paths = [str(ROOT / "compat"), str(ROOT / "python"), config["decbench_root"],
             str(Path(config["decbench_root"]) / "scripts")]
    if config.get("decoder_path"):
        paths.insert(0, config["decoder_path"])
    sys.path[:0] = paths
    os.environ.update(RUST_JOERN_LIBRARY=config["library"], DECBENCH_NO_CACHE="1",
                      DECBENCH_CACHE_DIR=config["metric_cache"],
                      RAYON_NUM_THREADS=str(config["rayon_threads"]),
                      DECBENCH_GED_MAX_NODES=str(config["ged_max_nodes"]),
                      OPENBLAS_NUM_THREADS="1", OMP_NUM_THREADS="1", MKL_NUM_THREADS="1")
    os.environ.pop("DECBENCH_DWARF_ABSTRACT_ORIGIN", None)
    import pyjoern
    import rust_joern
    if Path(pyjoern.__file__).resolve() != ROOT / "compat/pyjoern/__init__.py":
        raise RuntimeError("Original PyJoern was imported instead of the native shim")
    if sha256(config["library"]) != config["library_sha256"]:
        raise RuntimeError("Frozen native library changed")
    for path, digest in config.get("decoder_code_sha256", {}).items():
        if sha256(path) != digest:
            raise RuntimeError("Frozen JSON decoder changed")
    original_analyze = rust_joern._analyze
    original_parse = pyjoern.parse_source
    original_cfg = rust_joern.Function._cfg

    def analyze(*args, **kwargs):
        started = time.perf_counter()
        try:
            value = original_analyze(*args, **kwargs)
            if PHASE is not None:
                PHASE["diagnostics"].extend(value.get("diagnostics", []))
            return value
        finally:
            if PHASE is not None:
                PHASE["native_seconds"] += time.perf_counter() - started

    def parse(path, *args, **kwargs):
        started = time.perf_counter()
        try:
            if PHASE is not None:
                PHASE["prepared_sha256"] = sha256(path)
                PHASE["prepared_bytes"] = Path(path).stat().st_size
                PHASE["parse_calls"] += 1
            return original_parse(path, *args, **kwargs)
        finally:
            if PHASE is not None:
                PHASE["parse_seconds"] += time.perf_counter() - started

    def cfg(*args, **kwargs):
        started = time.perf_counter()
        try:
            return original_cfg(*args, **kwargs)
        finally:
            if PHASE is not None:
                PHASE["materialization_seconds"] += time.perf_counter() - started

    rust_joern._analyze = analyze
    pyjoern.parse_source = parse
    rust_joern.Function._cfg = staticmethod(cfg)
    logging.getLogger("decbench.utils.cfg").setLevel(logging.ERROR)


def phase():
    return {"native_seconds": 0.0, "parse_seconds": 0.0,
            "materialization_seconds": 0.0, "parse_calls": 0, "diagnostics": []}


def source_one(task):
    global PHASE
    PHASE = phase()
    started = time.perf_counter()
    from decbench.utils.cfg import extract_cfgs_from_source, is_degenerate_source_cfg
    try:
        path = Path(task["path"])
        if sha256(path) != task["sha256"]:
            raise RuntimeError("Source input changed")
        graphs = extract_cfgs_from_source(path, raise_on_error=True)
        if PHASE.get("prepared_sha256") != task["stripped_sha256"]:
            raise RuntimeError("Source preparation differs from the content-addressed input")
        generated = time.perf_counter() - started
        # GED reads topology, boundary roles, and singleton Nop degeneracy only.
        for graph in graphs.values():
            degenerate = is_degenerate_source_cfg(graph)
            for node in graph:
                node.statements = tuple(s for s in node.statements if type(s).__name__ != "Nop")[:1]
            if is_degenerate_source_cfg(graph) != degenerate:
                raise RuntimeError("Cache compaction changed source degeneracy")
        write_pickle(task["cache"], graphs)
        record = {"status": "ok", "functions": len(graphs), "cfg_seconds": generated}
    except Exception as error:
        record = {"status": "error", "error": str(error), "traceback": traceback.format_exc(),
                  "functions": 0, "cfg_seconds": time.perf_counter() - started}
        write_pickle(task["cache"], {})
    record.update(task, **PHASE, total_seconds=time.perf_counter() - started)
    PHASE = None
    atomic_json(task["record"], record)
    return record


def assemble_one(task):
    started = time.perf_counter()
    units = task["units"]
    per_stem = {stem: pickle.loads(Path(t["cache"]).read_bytes()) for stem, t in units.items()}
    loaded = time.perf_counter()
    write_pickle(task["cache"], {"per_stem": per_stem,
                 "_meta": {"native_library_sha256": RUN["library_sha256"],
                           "inputs": {stem: t["key"] for stem, t in units.items()}}})
    record = {"status": "ok", "key": task["key"], "source_load_seconds": loaded - started,
              "pickle_seconds": time.perf_counter() - loaded,
              "total_seconds": time.perf_counter() - started}
    atomic_json(task["record"], record)
    return record


def ownership_one(task):
    from decbench.utils import binfmt

    started = time.perf_counter()
    if sha256(task["binary"]) != task["binary_sha256"]:
        raise RuntimeError("Ownership binary changed")
    walk_started = time.perf_counter()
    owners = binfmt.source_function_owners(Path(task["binary"]), set(task["source_stems"]),
                                          follow_abstract_origin=False)
    walk_seconds = time.perf_counter() - walk_started
    if sha256(task["binary"]) != task["binary_sha256"]:
        raise RuntimeError("Ownership binary changed during DWARF walk")
    record = {**task, "status": "ok", "follow_abstract_origin": False,
              "owners": [[address, name, stem] for address, (name, stem) in owners.items()],
              "dwarf_seconds": walk_seconds, "total_seconds": time.perf_counter() - started}
    atomic_json(task["record"], record)
    return {k: v for k, v in record.items() if k != "owners"}


def score_one(task):
    global PHASE
    PHASE = phase()
    started = time.perf_counter()
    from decbench.metrics.ged import GEDMetric
    from decbench.utils.cfg import extract_cfgs_from_source, is_degenerate_source_cfg, resolved_source_for_binary
    from decbench.utils.results_tree import resolve_binary
    from reeval_ged import _load_src, _SRC_CACHE
    record = {"key": task["key"], "path": task["path"], "sha256": task["sha256"],
              "status": "ok", "scores": {}, "excluded": {}, "methods": {},
              "ownership_seconds": 0.0, "source_load_seconds": 0.0,
              "cfg_seconds": 0.0, "ged_seconds": 0.0}
    try:
        path = Path(task["path"])
        if sha256(path) != task["sha256"]:
            raise RuntimeError("Decompiled input changed")
        load_started = time.perf_counter()
        per_stem, best_by_name = _load_src(task["source_cache"])
        while len(_SRC_CACHE) > 2:
            _SRC_CACHE.pop(next(iter(_SRC_CACHE)))
        record["source_load_seconds"] = time.perf_counter() - load_started
        owners_started = time.perf_counter()
        binary = resolve_binary(path.parent.parent / "compiled", task["binary"])
        owners = None
        if binary is not None:
            evidence = json.loads(Path(task["ownership_cache"]).read_bytes())
            if (str(binary.resolve()) != evidence["binary"]
                    or evidence["binary_sha256"] != task["binary_sha256"]
                    or evidence["source_stems"] != sorted(per_stem)
                    or evidence["follow_abstract_origin"] is not False):
                raise RuntimeError("Ownership cache provenance differs from candidate inputs")
            stat = binary.stat()
            if [stat.st_size, stat.st_mtime_ns] != evidence["binary_stat"]:
                raise RuntimeError("Ownership binary changed after precomputation")
            owners = {address: (name, stem) for address, name, stem in evidence["owners"]}
        sources = resolved_source_for_binary(task["binary"], per_stem, best_by_name, function_owners=owners)
        record["ownership_seconds"] = time.perf_counter() - owners_started
        record["binary_path"] = str(binary) if binary else None
        cfg_started = time.perf_counter()
        try:
            graphs = extract_cfgs_from_source(path, sanitize_decompiled=True, raise_on_error=True)
        finally:
            record["cfg_seconds"] = time.perf_counter() - cfg_started
        markers = set(re.findall(r"^// Function: (\S+) @ 0x[0-9a-fA-F]+\s*$", path.read_text(errors="replace"), re.M))
        record["marker_functions"] = len(markers)
        record["parsed_functions"] = len(graphs)
        metric = GEDMetric()
        methods = Counter()
        ged_started = time.perf_counter()
        for name in sorted(markers):
            source, candidate = sources.get(name), graphs.get(name)
            if candidate is None:
                record["excluded"][name] = "missing_decompiled_cfg"
            elif source is None:
                record["excluded"][name] = "missing_source_cfg"
            elif is_degenerate_source_cfg(source):
                record["excluded"][name] = "degenerate_source_cfg"
            else:
                try:
                    value = metric.compute_for_function(None, source_cfg=source, decompiled_cfg=candidate)
                except Exception as error:
                    record["excluded"][name] = "metric_error: " + str(error)
                    continue
                if math.isfinite(value.value):
                    record["scores"][name] = {"value": float(value.value), "perfect": value.value == 0.0}
                    methods[value.metadata.get("method", "unknown")] += 1
                else:
                    record["excluded"][name] = "metric_error: " + str(value.metadata.get("error"))
        record["ged_seconds"] = time.perf_counter() - ged_started
        record["methods"] = dict(methods)
    except Exception as error:
        record.update(status="error", error=str(error), traceback=traceback.format_exc())
    record.update(PHASE, total_seconds=time.perf_counter() - started)
    PHASE = None
    record["preparation_selection_seconds"] = max(0.0, record["cfg_seconds"] - record["parse_seconds"])
    atomic_json(task["record"], record)
    return {k: v for k, v in record.items() if k not in ("scores", "excluded", "diagnostics", "traceback")}


def distribution(values):
    values = sorted(values)
    return {"observations": len(values), "sum_seconds": sum(values),
            "median_seconds": statistics.median(values) if values else None,
            "p95_seconds": values[min(len(values) - 1, math.ceil(len(values) * .95) - 1)] if values else None,
            "max_seconds": max(values) if values else None}


def pool_run(function, tasks, config, workers, label):
    results = []
    try:
        with ProcessPoolExecutor(max_workers=workers, mp_context=multiprocessing.get_context("spawn"),
                                 initializer=initialize, initargs=(config,)) as executor:
            futures = [executor.submit(function, task) for task in tasks]
            for future in as_completed(futures):
                results.append(future.result())
                if len(results) % 50 == 0 or len(results) == len(tasks):
                    errors = sum(r["status"] != "ok" for r in results)
                    print(f"[{label}] {len(results)}/{len(tasks)} completed; {errors} errors", flush=True)
    except BaseException as error:
        completed = {task["key"]: task["record"] for task in tasks if Path(task["record"]).is_file()}
        missing = [task["key"] for task in tasks if task["key"] not in completed]
        atomic_json(Path(config["output"]) / "interrupted.json",
                    {"stage": label, "complete": False, "error": type(error).__name__ + ": " + str(error),
                     "completed_record_files": completed, "unfinished": missing,
                     "native_library_sha256": config["library_sha256"]})
        raise
    return results


def catalog_tree(root, baseline, output):
    data = json.loads(Path(baseline).read_bytes())
    identities = sorted(data["decompilers"])
    artifacts, unknown = [], []
    for opt_dir in sorted(root.glob("O*")):
        if not opt_dir.is_dir():
            continue
        for project in sorted(p for p in opt_dir.iterdir() if p.is_dir()):
            for path in sorted((project / "decompiled").glob("*.c")):
                candidates = [d for d in identities if path.name.startswith(d + "_")]
                if not candidates:
                    unknown.append(str(path))
                    continue
                dec = max(candidates, key=len)
                binary = path.name[len(dec) + 1:-2]
                key = "::".join((opt_dir.name, project.name, binary, dec))
                artifacts.append({"key": key, "optimization": opt_dir.name, "project": project.name,
                                  "binary": binary, "decompiler": dec, "path": str(path.resolve()),
                                  "sha256": sha256(path), "record": str(output / "reeval_ged" / (key.replace("::", "__") + ".json"))})
    if unknown:
        raise RuntimeError(f"Unknown decompiler artifacts: {unknown[:10]}")
    if len({a["key"] for a in artifacts}) != len(artifacts):
        raise RuntimeError("Duplicate artifact keys")
    return {"root": str(root), "baseline": str(baseline), "baseline_sha256": sha256(baseline),
            "identities": identities, "artifacts": artifacts,
            "counts": dict(Counter(a["decompiler"] for a in artifacts))}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--decbench-root", type=Path, required=True)
    parser.add_argument("--main-tree", type=Path, required=True)
    parser.add_argument("--sample-tree", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--library", type=Path, default=ROOT / "target/release/librust_joern.so")
    parser.add_argument("--decoder-path", type=Path)
    parser.add_argument("--workers", type=int, default=16)
    parser.add_argument("--rayon-threads", type=int, default=2)
    parser.add_argument("--ged-max-nodes", type=int, default=200)
    args = parser.parse_args()
    if args.workers < 1 or args.rayon_threads < 1 or args.ged_max_nodes < 1:
        parser.error("Worker, Rayon-thread and GED-node counts must be positive")
    if args.output.exists():
        raise RuntimeError("Use a new output directory; old caches are not accepted")
    output = args.output.resolve()
    output.mkdir(parents=True)
    library = output / "native/librust_joern.so"
    library.parent.mkdir()
    shutil.copy2(args.library, library)
    config = {"output": str(output), "decbench_root": str(args.decbench_root.resolve()), "library": str(library),
              "library_sha256": sha256(library), "metric_cache": str(output / "unused-metric-cache"),
              "decoder_path": str(args.decoder_path.resolve()) if args.decoder_path else None,
              "rayon_threads": args.rayon_threads, "ged_max_nodes": args.ged_max_nodes}
    initialize(config)
    from reeval_ged import _source_inputs, _stripped_source_sha
    from decbench.metrics.ged import GEDMetric
    import rust_joern
    decoder = sys.modules[rust_joern._loads.__module__]
    decoder_modules = [decoder, sys.modules.get("orjson.orjson")] if decoder.__name__ == "orjson" else [decoder, json.decoder, json.scanner, sys.modules.get("_json")]
    config["decoder_code_sha256"] = {str(Path(module.__file__).resolve()): sha256(module.__file__)
                                    for module in decoder_modules if module is not None}
    guarded_paths = [ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "python/rust_joern/__init__.py",
                     ROOT / "compat/pyjoern/__init__.py", Path(__file__), *sorted((ROOT / "src").glob("*.rs")),
                     *sorted((args.decbench_root / "decbench").rglob("*.py")), args.decbench_root / "scripts/reeval_ged.py"]
    guards = {str(path.resolve()): sha256(path) for path in guarded_paths}
    guards.update(config["decoder_code_sha256"])
    guards[str(library)] = config["library_sha256"]
    started = time.perf_counter()
    provenance = {"started_at": datetime.now(timezone.utc).isoformat(), "config": config,
                  "workers": args.workers, "start_method": "spawn", "code_sha256": guards,
                  "python": sys.version, "decoder": rust_joern._loads.__module__,
                  "decoder_version": getattr(decoder, "__version__", None),
                  "dependency_versions": {name: version(name) for name in ("networkx", "scipy", "cfgutils", "pyelftools")},
                  "python_hash_seed": os.environ.get("PYTHONHASHSEED"),
                  "parser_options": {"strict": True, "preprocessed": True, "data_flow": False, "reaching_definitions": False},
                  "metric_cache_version": GEDMetric.cache_version, "metric_cache_disabled": True,
                  "historical_graph_cache_reused": False, "historical_score_cache_reused": False,
                  "dwarf_follow_abstract_origin": False}
    trees = {}
    for name, root in (("main", args.main_tree.resolve()), ("sample", args.sample_tree.resolve())):
        dest = output / name
        dest.mkdir()
        baseline = dest / "baseline.function_results.json"
        shutil.copy2(root / "function_results.json", baseline)
        for filename in ("scoreboard.toml", "sample_set_manifest.json", "ged_new.slices.json"):
            if (root / filename).exists():
                shutil.copy2(root / filename, dest / ("baseline." + filename))
        trees[name] = catalog_tree(root, baseline, dest)
    inputs = {}
    representatives = {}
    for name, tree in trees.items():
        required = sorted({(a["optimization"], a["project"]) for a in tree["artifacts"]})
        for opt, project in required:
            paths = _source_inputs(Path(tree["root"]), opt, project)
            units = {}
            for path in paths:
                digest = _stripped_source_sha(path)
                language = "cpp" if path.suffix == ".ii" else "c"
                cache_key = language + "-" + digest
                task = {"key": cache_key, "path": str(path.resolve()), "sha256": sha256(path),
                        "language": language, "stripped_sha256": digest,
                        "cache": str(output / "source/by-content" / (cache_key + ".pkl")),
                        "record": str(output / "source/records" / (cache_key + ".json"))}
                representatives.setdefault(cache_key, task)
                units[path.stem] = task
            inputs[(name, opt, project)] = units
    assembly_tasks, project_caches = {}, {}
    for project_key, units in inputs.items():
        ordered = [(stem, t["language"], t["stripped_sha256"]) for stem, t in units.items()]
        key = hashlib.sha256(json.dumps([config["library_sha256"], ordered]).encode()).hexdigest()
        cache = str(output / "source/project-maps" / (key + ".pkl"))
        project_caches[project_key] = cache
        assembly_tasks.setdefault(key, {"key": key, "units": units, "cache": cache,
                                       "record": str(output / "source/assembly-records" / (key + ".json"))})
    from decbench.utils.results_tree import resolve_binary
    ownership_tasks, binary_hashes = {}, {}
    for name, tree in trees.items():
        for task in tree["artifacts"]:
            binary = resolve_binary(Path(task["path"]).parent.parent / "compiled", task["binary"])
            if binary is None:
                continue
            binary = binary.resolve()
            binary_path = str(binary)
            if binary_path not in binary_hashes:
                binary_hashes[binary_path] = sha256(binary)
            stems = sorted(inputs[(name, task["optimization"], task["project"])])
            key = hashlib.sha256(json.dumps([binary_path, binary_hashes[binary_path], stems, False]).encode()).hexdigest()
            cache = str(output / "ownership" / (key + ".json"))
            stat = binary.stat()
            ownership_tasks.setdefault(key, {"key": key, "binary": binary_path,
                                       "binary_sha256": binary_hashes[binary_path],
                                       "binary_stat": [stat.st_size, stat.st_mtime_ns],
                                       "source_stems": stems, "record": cache})
            task.update(ownership_cache=cache, binary_sha256=binary_hashes[binary_path])
    for name, tree in trees.items():
        for task in tree["artifacts"]:
            task["source_cache"] = project_caches[(name, task["optimization"], task["project"])]
    atomic_json(output / "manifest.json", {**provenance, "trees": trees,
                                           "assembly_tasks": list(assembly_tasks.values()),
                                           "ownership_tasks": list(ownership_tasks.values()),
                                           "source_inputs": [{"tree": k[0], "optimization": k[1], "project": k[2], "units": v} for k,v in inputs.items()]})
    print(f"[inventory] {len(representatives)} unique source inputs; " + "; ".join(f"{name}: {len(tree['artifacts'])} artifacts / {len(tree['identities'])} identities" for name,tree in trees.items()), flush=True)
    inventory_seconds = time.perf_counter() - started
    source_started = time.perf_counter()
    source_results = pool_run(source_one, list(representatives.values()), config, args.workers, "source")
    source_parse_seconds = time.perf_counter() - source_started
    assembly_started = time.perf_counter()
    assembly_results = pool_run(assemble_one, list(assembly_tasks.values()), config, min(args.workers, 4), "assembly")
    assembly_seconds = time.perf_counter() - assembly_started
    ownership_started = time.perf_counter()
    ownership_results = pool_run(ownership_one, list(ownership_tasks.values()), config, args.workers, "ownership")
    ownership_seconds = time.perf_counter() - ownership_started
    batches = {}
    for name, tree in trees.items():
        tasks = tree["artifacts"]
        batch_started = time.perf_counter()
        results = pool_run(score_one, tasks, config, args.workers, name)
        batch_seconds = time.perf_counter() - batch_started
        merged = {}
        statuses = Counter()
        for task in tasks:
            record = json.loads(Path(task["record"]).read_bytes())
            statuses[record["status"]] += 1
            for function, score in record["scores"].items():
                merged[task["key"] + "::" + function] = score
        atomic_json(output / name / "ged_new.json", merged)
        atomic_json(output / name / "ged_new.slices.json", sorted(t["key"] for t in tasks))
        batches[name] = {"wall_seconds": batch_seconds, "artifacts": len(tasks), "scores": len(merged),
                         "statuses": dict(statuses), "timings": {field: distribution([r.get(field, 0.0) for r in results]) for field in
                         ("native_seconds", "parse_seconds", "materialization_seconds", "cfg_seconds", "preparation_selection_seconds", "ownership_seconds", "source_load_seconds", "ged_seconds", "total_seconds")}}
    changed = [path for path,value in guards.items() if sha256(path) != value]
    if changed:
        raise RuntimeError(f"Code changed during the frozen run: {changed}")
    if any(sha256(path) != value for path, value in binary_hashes.items()):
        raise RuntimeError("Compiled binary changed during frozen run")
    timings = {"inventory_seconds": inventory_seconds, "source_parse_wall_seconds": source_parse_seconds,
               "source_assembly_wall_seconds": assembly_seconds, "batches": batches,
               "source_assembly_maps_before": len(inputs), "source_assembly_maps_after": len(assembly_tasks),
               "source_assembly_workers": min(args.workers, 4),
               "source_assembly_timings": {field: distribution([r[field] for r in assembly_results])
                                           for field in ("source_load_seconds", "pickle_seconds", "total_seconds")},
               "ownership_precompute_wall_seconds": ownership_seconds, "ownership_precompute_queries": len(ownership_tasks),
               "ownership_precompute_timings": {field: distribution([r[field] for r in ownership_results])
                                                for field in ("dwarf_seconds", "total_seconds")},
               "total_wall_seconds": time.perf_counter() - started,
               "source_statuses": dict(Counter(r["status"] for r in source_results)),
               "source_timings": {field: distribution([r.get(field, 0.0) for r in source_results]) for field in
                                  ("native_seconds", "parse_seconds", "materialization_seconds", "cfg_seconds", "total_seconds")},
               "notes": ["Batch wall time differs from summed parallel worker intervals.",
                         "Source graphs were freshly generated once per prepared-language/content pair.",
                         "Candidate graphs and GED were freshly computed for every saved artifact, including standalone Astra samples.",
                         "Generation includes native parsing/CPG/CFG, JSON decoding and Python graph materialization; ownership and GED are separate.",
                         "Source cache compaction preserves directed topology, roles, and singleton Nop degeneracy; GED never reads statement labels.",
                         "Parsing failures are explicit evaluated slices, with no guessed graphs or stale score fallback."]}
    atomic_json(output / "timings.json", timings)
    atomic_json(output / "completed.json", {"finished_at": datetime.now(timezone.utc).isoformat(),
                                            "native_library_sha256": config["library_sha256"], "code_unchanged": True,
                                            "tree_artifact_counts": {n:len(t["artifacts"]) for n,t in trees.items()}})
    print(json.dumps(timings, indent=2), flush=True)


if __name__ == "__main__":
    main()
