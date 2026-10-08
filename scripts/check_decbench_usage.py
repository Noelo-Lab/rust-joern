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
from concurrent.futures import ProcessPoolExecutor, as_completed
import hashlib
import json
import logging
import multiprocessing
import os
from pathlib import Path
import sys
import tempfile
import time

from compare_native_corpus import (FingerprintMismatch, atomic_json, digest, freeze,
                                   read_frozen, signature, source_fingerprints, unchanged)


ROOT = Path(__file__).resolve().parents[1]
BLACKLIST = ("<", "+", "*", "(", ">", "JUMPOUT", "__builtin_unreachable")
REQUIRED_OPTIONS = {"strict": True, "data_flow": False, "preprocessed": True}
SELECTION_CONTRACT = "last_fullname_filename_then_largest_name_filename_later_tie"
RUN = {}


def decoder_provenance() -> tuple[dict, list[dict]]:
    sys.path.insert(0, str(ROOT / "python"))
    import rust_joern

    name = rust_joern._loads.__module__
    module = sys.modules[name]
    modules = [module, sys.modules.get("orjson.orjson")] if name == "orjson" else [
        module, json.decoder, json.scanner, sys.modules.get("_json")]
    guards = [freeze(item.__file__) for item in modules if item is not None]
    return {"module": name, "version": getattr(module, "__version__", None),
            "python_version": sys.version,
            "artifacts": [{key: guard[key] for key in ("path", "sha256")} for guard in guards]}, guards


def initialize(decbench_root: str, cache_root: str, config: dict, guards: list[dict]) -> None:
    sys.path[:0] = [str(ROOT / "compat"), str(ROOT / "python"), decbench_root]
    os.environ["DECBENCH_CACHE_DIR"] = cache_root
    os.environ["DECBENCH_NO_CACHE"] = "1"
    os.environ["RAYON_NUM_THREADS"] = str(config["worker_config"]["rayon_threads"])
    os.environ["RUST_JOERN_LIBRARY"] = config["native_library"]["path"]
    for guard in guards:
        read_frozen(guard)
    RUN.update(config=config, guards=guards)
    import pyjoern

    if Path(pyjoern.__file__).resolve() != ROOT / "compat/pyjoern/__init__.py":
        raise RuntimeError(f"Expected the opt-in shim, imported {pyjoern.__file__}")
    if decoder_provenance()[0] != config["json_decoder"]:
        raise FingerprintMismatch("Worker JSON decoder differs from the frozen run provenance")
    logging.getLogger("decbench.utils.cfg").setLevel(logging.CRITICAL)


def verify_graph(data: dict, nodes: list[dict], graph, counters: dict) -> None:
    from decbench.utils.cfg import is_degenerate_source_cfg

    raw_nodes = {node["id"]: node for node in nodes}
    blocks = {block.id: block for block in graph}
    if len(blocks) != len(data["nodes"]) or set(blocks) != {block["id"] for block in data["nodes"]}:
        raise ValueError("Python graph changed native CFG node IDs")
    for block in data["nodes"]:
        actual = blocks[block["id"]]
        if (actual.is_entrypoint, actual.is_exitpoint) != (block["is_entrypoint"], block["is_exitpoint"]):
            raise ValueError("Python graph changed native entry/exit flags")
        if [statement.id for statement in actual.statements] != block["statements"]:
            raise ValueError("Python graph changed native statement membership")
        for statement in actual.statements:
            node = raw_nodes[statement.id]
            nop = node.get("cfg_nop", node["kind"] in ("METHOD", "METHOD_RETURN", "METHOD_REF"))
            expected = "Nop" if nop else "Statement"
            if type(statement).__name__ != expected or statement.kind != node["kind"]:
                raise ValueError("Python statement class disagrees with native cfg_nop/CPG kind")
            counters[expected] += 1
            counters["method_ref_overrides"] += node["kind"] == "METHOD_REF" and node.get("cfg_nop") is False
    if {(u.id, v.id) for u, v in graph.edges} != {tuple(edge) for edge in data["edges"]}:
        raise ValueError("Python graph changed native directed edges")
    degenerate = len(data["nodes"]) == 0 or len(data["nodes"]) == 1 and all(
        raw_nodes[stmt].get("cfg_nop", raw_nodes[stmt]["kind"] in ("METHOD", "METHOD_RETURN", "METHOD_REF"))
        for block in data["nodes"] for stmt in block["statements"])
    if is_degenerate_source_cfg(graph) != degenerate:
        raise ValueError("Actual Python statement classes changed native CFG degeneracy")
    counters["functions_checked"] += 1
    counters["blocks_checked"] += len(blocks)


def verify_selection(data: dict, functions: dict, counters: dict) -> None:
    exported = {(raw["fullname"], raw["filename"]): raw for raw in data["functions"]}
    counters["fullname_file_groups"] += len(exported)
    counters["fullname_file_shadowed"] += len(data["functions"]) - len(exported)
    selected = {}
    for original in data["functions"]:
        raw = exported[(original["fullname"], original["filename"])]
        name, size = raw["name"], len(raw["cfg"]["nodes"])
        filename = Path(raw["filename"])
        if not name or not filename or name.startswith(BLACKLIST):
            counters["blacklisted"] += 1
            continue
        if not size:
            counters["empty_cfg"] += 1
            continue
        key = name, str(filename)
        previous = selected.get(key)
        if previous is not None:
            previous_size = len(previous["cfg"]["nodes"])
            if previous_size > size:
                counters["smaller_duplicate_skipped"] += 1
                continue
            counters["later_tie_winner" if previous_size == size else "larger_winner"] += 1
        selected[key] = raw
    selected = {name: raw for (name, _), raw in selected.items()}
    if set(functions) != set(selected) or any(functions[name].raw is not raw for name, raw in selected.items()):
        raise ValueError("Actual Python selection differs from FastParser FULL_NAME/FILE export and NAME/FILE size/tie contract")
    counters.update(raw_functions=len(data["functions"]), selected_functions=len(selected), selection_verified=True)


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

    for guard in [*RUN["guards"], *case["_guards"]]:
        unchanged(guard)

    original_parse = pyjoern.parse_source
    original_analyze = rust_joern._analyze
    original_cfg = rust_joern.Function._cfg
    phase = "primary"
    clocks = {name: {"native": 0.0, "graph": 0.0, "parse": 0.0, "proof": 0.0} for name in ("primary", "fallback")}
    proof = {name: dict(functions_checked=0, blocks_checked=0, Nop=0, Statement=0,
                        method_ref_overrides=0, blacklisted=0, empty_cfg=0,
                        fullname_file_groups=0, fullname_file_shadowed=0,
                        smaller_duplicate_skipped=0, later_tie_winner=0, larger_winner=0,
                        selection_verified=False) for name in ("primary", "fallback")}
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
            captured[phase + "_analysis"] = data
            return data
        finally:
            clocks[phase]["native"] += time.perf_counter() - started

    def timed_cfg(*args, **kwargs):
        started = time.perf_counter()
        try:
            graph = original_cfg(*args, **kwargs)
        finally:
            clocks[phase]["graph"] += time.perf_counter() - started
        started = time.perf_counter()
        try:
            verify_graph(args[0], args[1], graph, proof[phase])
        finally:
            clocks[phase]["proof"] += time.perf_counter() - started
        return graph

    def capture_parse(path, *args, **kwargs):
        source = Path(path).read_bytes()
        captured.update(source=source, filename=str(path), prepared_sha256=hashlib.sha256(source).hexdigest())
        started = time.perf_counter()
        try:
            parsed = original_parse(path, *args, **kwargs)
            started_proof = time.perf_counter()
            verify_selection(captured["primary_analysis"], parsed, proof["primary"])
            clocks[phase]["proof"] += time.perf_counter() - started_proof
            captured["selected_graphs"] = {function.name: function.cfg for function in parsed.values()}
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
    record.update(status="ok", error=None, diagnostics=[], functions={},
                  run_fingerprint=RUN["config"]["run_fingerprint"], python_view_proof=proof,
                  source_sha256=case.get("source_sha256"), reference=case.get("reference"),
                  reference_sha256=case.get("reference_sha256"))
    graphs = None
    started = time.perf_counter()
    try:
        graphs = extract_cfgs_from_source(
            Path(case["source"]), sanitize_decompiled=case["kind"] != "source", raise_on_error=True,
        )
        if (set(graphs) != set(captured["selected_graphs"])
                or any(graphs[name] is not graph for name, graph in captured["selected_graphs"].items())):
            raise ValueError("DecBench extractor changed selected Python CFG views")
        proof["primary"]["extractor_verified"] = True
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
        "python_view_proof_seconds": primary["proof"],
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
                exported = {(function.raw["fullname"], function.raw["filename"]): function
                            for function in analysis.functions}
                selected_by_file = {}
                for original in analysis.functions:
                    function = exported[(original.raw["fullname"], original.raw["filename"])]
                    if (not function.name or not function.filename
                            or function.name.startswith(BLACKLIST) or not function.cfg):
                        continue
                    key = function.name, str(function.filename)
                    previous = selected_by_file.get(key)
                    if previous is None or len(function.cfg) >= len(previous.cfg):
                        selected_by_file[key] = function
                selected = {name: function for (name, _), function in selected_by_file.items()}
                graphs = {name: function.cfg for name, function in selected.items()}
                captured["lines"] = {name: {"start_line": function.start_line, "end_line": function.end_line}
                                     for name, function in selected.items()}
                started_proof = time.perf_counter()
                verify_selection(captured["fallback_analysis"], selected, proof["fallback"])
                clocks[phase]["proof"] += time.perf_counter() - started_proof
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
                    "python_view_proof_seconds": fallback["proof"],
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
    for guard in [*RUN["guards"], *case["_guards"]]:
        unchanged(guard)
    output = Path(output_root) / f"{case['id']}.json"
    if not output.resolve().is_relative_to(Path(output_root).resolve()):
        raise ValueError(f"Case id escapes the output directory: {case['id']}")
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".tmp")
    temporary.write_text(json.dumps(record, ensure_ascii=False) + "\n", encoding="utf-8")
    temporary.replace(output)
    result = {"id": case["id"], "status": record["status"], "error": (record["error"] or "")[:200],
            "metric_excluded": case.get("metric_excluded"), "functions": len(record["functions"]),
            "timings": record["timings"], "fallback_timings": record.get("fallback_timings"),
            "python_view_proof": proof, "output": str(output),
            "run_fingerprint": record["run_fingerprint"], "output_sha256": digest(output.read_bytes())}
    atomic_json(Path(output_root) / "completed" / f"{case['id']}.json", result)
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--rayon-threads", type=int, default=2)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--build-provenance", type=Path, help="Immutable native library/source snapshot")
    parser.add_argument("--resume", action="store_true")
    parser.add_argument("--case", action="append", help="Check only this exact case id (repeatable)")
    parser.add_argument("--limit", type=int, help="Explicit smoke limit; omitted means every case")
    args = parser.parse_args()
    if not 1 <= args.workers <= min(32, os.cpu_count() or 1) or not 1 <= args.rayon_threads <= 32:
        parser.error("workers must be 1..min(32, CPU count); rayon-threads must be 1..32")
    if args.limit is not None and args.limit < 1:
        parser.error("limit must be positive")
    started = time.perf_counter()
    manifest_guard = freeze(args.manifest)
    manifest = json.loads(read_frozen(manifest_guard))
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
    if args.output.exists() and any(args.output.iterdir()) and not args.resume:
        parser.error("Output is not empty; choose a new folder or --resume")
    args.output.mkdir(parents=True, exist_ok=True)
    provenance = json.loads(args.build_provenance.read_bytes()) if args.build_provenance else None
    library = args.library or (Path(provenance["native_library"]["path"]) if provenance else
                              Path(os.environ.get("RUST_JOERN_LIBRARY", ROOT / "target/release/librust_joern.so")))
    library_guard = freeze(library, provenance["native_library"]["sha256"] if provenance else None)
    guards = [manifest_guard, library_guard, freeze(__file__),
              freeze(ROOT / "scripts/compare_native_corpus.py"), freeze(ROOT / "python/rust_joern/__init__.py"),
              freeze(ROOT / "compat/pyjoern/__init__.py")]
    decoder, decoder_guards = decoder_provenance()
    guards += decoder_guards
    decbench = Path(manifest["decbench_root"])
    guards += [freeze(decbench / relative) for relative in
               ("decbench/utils/cfg.py", "decbench/utils/langs.py", "decbench/publish/cfg_export.py")]
    snapshot = Path(provenance["snapshot_root"]) if provenance else ROOT
    native_digest, source_digest, snapshot_files = source_fingerprints(snapshot)
    if provenance:
        if (provenance.get("library_sha256", library_guard["sha256"]) != library_guard["sha256"]
                or provenance["native_source_sha256"] != native_digest
                or provenance["source_build_fingerprint"] != source_digest
                or provenance.get("preprocessed_option_supported") is not True):
            raise FingerprintMismatch("Build provenance differs from immutable library/snapshot/preprocessed contract")
        guards += [freeze(args.build_provenance), *(freeze(path) for path in snapshot_files)]
    artifacts = {}
    for case in cases:
        case_guards = []
        for field, sha_field in (("source", "source_sha256"), ("prepared_source", "prepared_sha256"), ("reference", None)):
            if not case.get(field):
                continue
            path = str(Path(case[field]).resolve())
            expected = case.get(sha_field) if sha_field else None
            if path not in artifacts:
                artifacts[path] = freeze(path, expected)
            elif expected and artifacts[path]["sha256"] != expected:
                raise FingerprintMismatch(f"Manifest records conflicting artifact digests: {path}")
            case_guards.append(artifacts[path])
            if sha_field == "source_sha256":
                case["source_sha256"] = artifacts[path]["sha256"]
            if field == "reference":
                case["reference_sha256"] = artifacts[path]["sha256"]
        case.update(_guards=case_guards, native_library_sha256=library_guard["sha256"])
    config = {"schema_version": 1, "manifest": manifest_guard["path"], "manifest_sha256": manifest_guard["sha256"],
              "output": output_root, "selected_case_ids_sha256": signature([case["id"] for case in cases]),
              "native_library": {key: library_guard[key] for key in ("path", "sha256")},
              "json_decoder": decoder,
              "native_source_sha256": native_digest, "source_build_fingerprint": source_digest,
              "build_provenance": str(args.build_provenance.resolve()) if args.build_provenance else None,
              "snapshot_verified": bool(provenance), "required_candidate_options": REQUIRED_OPTIONS,
              "required_python_view_proof": True, "function_selection_contract": SELECTION_CONTRACT,
              "worker_config": {"workers": args.workers, "start_method": "spawn", "rayon_threads": args.rayon_threads, "cache_disabled": True},
              "frozen_artifacts": [{key: guard[key] for key in ("path", "sha256")} for guard in [*guards, *artifacts.values()]]}
    config["run_fingerprint"] = signature(config)
    run_path = args.output / "run.json"
    if run_path.exists():
        if not args.resume or json.loads(run_path.read_bytes()).get("run_fingerprint") != config["run_fingerprint"]:
            raise FingerprintMismatch("Resume library/snapshot/options/scope/input/reference/wrapper/tool fingerprint differs")
    else:
        atomic_json(run_path, config)
    results, pending = {}, []
    for case in cases:
        checkpoint = args.output / "completed" / f"{case['id']}.json"
        if args.resume and checkpoint.exists():
            result = json.loads(checkpoint.read_bytes())
            if result.get("run_fingerprint") != config["run_fingerprint"]:
                raise FingerprintMismatch(f"Resume checkpoint belongs to another generation: {checkpoint}")
            if Path(result["output"]).resolve() != (args.output / f"{case['id']}.json").resolve():
                raise FingerprintMismatch(f"Resume checkpoint targets another case: {checkpoint}")
            freeze(result["output"], result["output_sha256"])
            results[case["id"]] = result
        else:
            pending.append(case)
    resumed, abort, executor = len(results), None, None
    print(f"Exact DecBench usage: {len(cases)}/{available} cases, {resumed} resumed; library {library_guard['sha256'][:12]}", flush=True)
    with tempfile.TemporaryDirectory(prefix="rust-joern-usage-cache-") as cache:
        try:
            if pending:
                executor = ProcessPoolExecutor(max_workers=args.workers, mp_context=multiprocessing.get_context("spawn"),
                                               initializer=initialize, initargs=(str(decbench), cache, config, guards))
                futures = {executor.submit(check_case, case, output_root): case["id"] for case in pending}
                for future in as_completed(futures):
                    result = future.result()
                    results[result["id"]] = result
                    if len(results) % 100 == 0 or len(results) == len(cases):
                        print(f"{len(results)}/{len(cases)} actual extractor calls recorded in {time.perf_counter() - started:.1f}s", flush=True)
                executor.shutdown(wait=True)
                executor = None
            for guard in [*guards, *artifacts.values()]:
                read_frozen(guard)
        except (Exception, KeyboardInterrupt) as error:
            abort = f"{type(error).__name__}: {error}"
            print(abort, file=sys.stderr, flush=True)
            if executor is not None:
                if hasattr(executor, "terminate_workers"):
                    executor.terminate_workers()
                else:
                    executor.shutdown(wait=False, cancel_futures=True)
    results = list(results.values())
    status_counts = dict(Counter(result["status"] for result in results))
    summary = {
        "schema_version": 1, "manifest": str(args.manifest.resolve()),
        "available_cases": available, "selected_cases": len(cases), "checked_cases": len(results),
        "complete": len(results) == len(cases) == available and not abort,
        "complete_selected_scope": len(results) == len(cases) and not abort,
        "resumed_cases": resumed, "abort": abort, "run_fingerprint": config["run_fingerprint"],
        "statuses": status_counts, "metric_excluded_cases": sum(bool(case.get("metric_excluded")) for case in cases),
        "coverage": dict(Counter(f"{case['optimization']}/{case['kind']}" for case in cases)),
        "seconds": time.perf_counter() - started,
        "native_library": config["native_library"], "native_source_sha256": native_digest,
        "json_decoder": config["json_decoder"],
        "source_build_fingerprint": source_digest, "snapshot_verified": bool(provenance),
        "build_provenance": config["build_provenance"], "required_candidate_options": REQUIRED_OPTIONS,
        "required_python_view_proof": True, "function_selection_contract": SELECTION_CONTRACT,
        "worker_config": config["worker_config"],
        "python_view_proof": {phase: {key: sum(result["python_view_proof"][phase].get(key, 0) for result in results)
                                      for key in ("functions_checked", "blocks_checked", "Nop", "Statement", "method_ref_overrides", "selection_verified", "extractor_verified", "fullname_file_groups", "fullname_file_shadowed", "blacklisted", "empty_cfg", "smaller_duplicate_skipped", "later_tie_winner", "larger_winner")}
                              for phase in ("primary", "fallback")},
        "timing_notes": "native_seconds includes C ABI/JSON decoding; preparation includes temp-file I/O/cleanup; graph_materialization excludes separate proof overhead; fallback timings are secondary; resumed case measurements are retained from their original actual extractor calls",
        "case_records": "Per-case JSON files at <case-id>.json under this directory",
    }
    (args.output / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"DecBench usage: {len(results)}/{available} cases, {status_counts}, {summary['seconds']:.3f}s")
    return 2 if abort else int(status_counts.get("parse_error", 0) > 0)


if __name__ == "__main__":
    raise SystemExit(main())
