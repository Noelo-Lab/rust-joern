#!/usr/bin/env python3
"""Compare a frozen native library with every selected, frozen DecBench oracle.

This is direct-ABI triage, not an end-to-end DecBench measurement. Each unique
prepared input/language is parsed once in strict mode; a failed strict request
gets a separately timed permissive fallback. All original case IDs, diagnostics
and mismatches remain in the report. A new binary requires a new output folder.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
from concurrent.futures import ProcessPoolExecutor, as_completed
import ctypes
import gzip
import hashlib
import json
import multiprocessing
import os
from pathlib import Path
import sys
import time

from compare_pyjoern import candidate_functions, compare


ROOT = Path(__file__).resolve().parents[1]
STATE = {}


class FingerprintMismatch(RuntimeError):
    """A frozen input, oracle, binary or implementation changed during a run."""


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def signature(value: object) -> str:
    return digest(json.dumps(value, sort_keys=True, separators=(",", ":")).encode())


def stat_key(path: str | Path) -> tuple:
    stat = Path(path).stat()
    return stat.st_dev, stat.st_ino, stat.st_size, stat.st_mtime_ns, stat.st_ctime_ns


def freeze(path: str | Path, expected: str | None = None) -> dict:
    path = str(Path(path).resolve())
    before = stat_key(path)
    sha = digest(Path(path).read_bytes())
    if stat_key(path) != before or (expected is not None and sha != expected):
        raise FingerprintMismatch(f"Frozen artifact digest differs: {path}")
    return {"path": path, "sha256": sha, "stat": before}


def unchanged(artifact: dict) -> None:
    if stat_key(artifact["path"]) != tuple(artifact["stat"]):
        raise FingerprintMismatch(f"Frozen artifact changed during the batch: {artifact['path']}")


def read_frozen(artifact: dict) -> bytes:
    unchanged(artifact)
    data = Path(artifact["path"]).read_bytes()
    unchanged(artifact)
    if digest(data) != artifact["sha256"]:
        raise FingerprintMismatch(f"Frozen artifact digest differs: {artifact['path']}")
    return data


def source_fingerprints(snapshot: Path) -> tuple[str, str, list[Path]]:
    sources = sorted((snapshot / "src").rglob("*.rs"))
    native = digest(b"".join(path.name.encode() + path.read_bytes() for path in sources))
    files = sorted([snapshot / "Cargo.toml", snapshot / "Cargo.lock", *sources])
    fingerprint = hashlib.sha256()
    for path in files:
        fingerprint.update(path.relative_to(snapshot).as_posix().encode())
        fingerprint.update(b"\0")
        fingerprint.update(path.read_bytes())
    return native, fingerprint.hexdigest(), files


def atomic_json(path: Path, value: dict, *, compressed: bool = False) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(str(path) + ".tmp")
    opener = gzip.open if compressed else open
    with opener(temporary, "wt", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, separators=(",", ":"))
        stream.write("\n")
    temporary.replace(path)


def initialize(config: dict, guards: list[dict]) -> None:
    os.environ["RAYON_NUM_THREADS"] = str(config["worker_config"]["rayon_threads"])
    for artifact in guards:
        read_frozen(artifact)
    library = ctypes.CDLL(config["native_library"]["path"])
    library.rust_joern_analyze.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_char_p]
    library.rust_joern_analyze.restype = ctypes.c_void_p
    library.rust_joern_free.argtypes = [ctypes.c_void_p]
    library.rust_joern_free.restype = None
    STATE.update(config=config, guards=guards, library=library)


def native(source: bytes, filename: str, options: dict) -> tuple[dict | None, str | None, float]:
    started = time.perf_counter()
    buffer = ctypes.create_string_buffer(source)
    library = STATE["library"]
    pointer = library.rust_joern_analyze(buffer, len(source), filename.encode(), json.dumps(options).encode())
    if not pointer:
        return None, "Native analysis returned a null pointer", time.perf_counter() - started
    try:
        data = json.loads(ctypes.string_at(pointer))
    finally:
        library.rust_joern_free(pointer)
    elapsed = time.perf_counter() - started
    return (None, str(data["error"]), elapsed) if "error" in data else (data, None, elapsed)


def topology(cfg: dict) -> dict:
    return {key: cfg[key] for key in ("nodes", "edges", "entry", "exit", "degenerate")}


def process_input(job: dict) -> dict:
    config = STATE["config"]
    for guard in STATE["guards"]:
        unchanged(guard)
    started = time.perf_counter()
    source = read_frozen(job["input"])
    options = config["requested_options"] | {"language": job["language"]}
    analysis, error, primary_seconds = native(source, job["input"]["path"], options)
    record = {"schema_version": 1, "run_fingerprint": config["run_fingerprint"],
              "input_id": job["input_id"], "prepared_sha256": job["input"]["sha256"],
              "language": job["language"], "case_ids": job["case_ids"],
              "requested_options": options, "native_library_sha256": config["native_library"]["sha256"],
              "status": "parse_error" if error else "ok", "error": error,
              "diagnostics": [], "timings": {"native_seconds": primary_seconds},
              "native_invocations": 1, "functions": {}}
    if error:
        analysis, fallback_error, fallback_seconds = native(
            source, job["input"]["path"], options | {"strict": False},
        )
        record.update(fallback_error=fallback_error, fallback_timings={"native_seconds": fallback_seconds},
                      diagnostics_source="permissive_fallback", native_invocations=2)
    else:
        record["diagnostics_source"] = "strict_primary"
    normalized_at = time.perf_counter()
    if analysis is not None:
        record["functions"] = candidate_functions(analysis)
        record["diagnostics"] = analysis.get("diagnostics", [])
    normalization_seconds = time.perf_counter() - normalized_at
    record["timings" if not error else "fallback_timings"]["python_normalization_seconds"] = normalization_seconds
    record["timings"]["analysis_and_fallback_seconds"] = time.perf_counter() - started
    output = Path(config["output"])
    failure_path = output / "failures" / f"{job['input_id']}.jsonl.gz"
    failure_path.parent.mkdir(parents=True, exist_ok=True)
    temporary = Path(str(failure_path) + ".tmp")
    comparisons = {}
    compare_started = time.perf_counter()
    with gzip.open(temporary, "wt", encoding="utf-8") as failure_stream:
        def failure(row: dict) -> None:
            failure_stream.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n")

        if record["status"] != "ok" or record["diagnostics"]:
            failure({"type": "usage", "input_id": job["input_id"], "case_ids": job["case_ids"],
                     "status": record["status"], "error": error, "fallback_error": record.get("fallback_error"),
                     "diagnostics": record["diagnostics"]})
        for reference_job in job["references"]:
            reference = json.loads(read_frozen(reference_job["artifact"]))
            if reference.get("prepared_sha256", reference.get("source_sha256")) != job["input"]["sha256"]:
                raise FingerprintMismatch(f"Oracle prepared digest differs: {reference_job['artifact']['path']}")
            rows = compare(reference["functions"], record["functions"])
            counts = Counter(row["status"] for row in rows)
            bodies = Counter(row["status"] for row in rows if row.get("reference", {}).get("degenerate") is False)
            declarations = Counter(row["status"] for row in rows if row.get("reference", {}).get("degenerate") is True)
            topology_status = "divergent" if any(row["status"] != "match" for row in rows) else "match"
            comparisons[reference_job["artifact"]["path"]] = {
                "case_ids": reference_job["case_ids"], "reference_sha256": reference_job["artifact"]["sha256"],
                "status": "usage_error" if error else "diagnostics" if record["diagnostics"] else topology_status,
                "topology_status": topology_status, "function_statuses": dict(counts),
                "non_degenerate_function_statuses": dict(bodies), "degenerate_function_statuses": dict(declarations),
                "reference_functions": len(reference["functions"]), "candidate_functions": len(record["functions"]),
                "empty_reference": not bool(reference["functions"]),
            }
            for row in rows:
                if row["status"] != "match":
                    for key in ("reference_graph", "candidate_graph"):
                        if key in row:
                            row[key] = topology(row[key])
                    failure({"type": "function", "input_id": job["input_id"],
                             "case_ids": reference_job["case_ids"], "reference": reference_job["artifact"]["path"], **row})
    temporary.replace(failure_path)
    record["timings"]["comparison_seconds"] = time.perf_counter() - compare_started
    record["comparisons"] = comparisons
    record_path = output / "inputs" / f"{job['input_id']}.json.gz"
    atomic_json(record_path, record, compressed=True)
    result = {key: record[key] for key in ("input_id", "prepared_sha256", "status", "timings", "native_invocations", "comparisons")}
    result.update(run_fingerprint=config["run_fingerprint"], diagnostics_count=len(record["diagnostics"]),
                  error_preview=(error or "")[:200], fallback_timings=record.get("fallback_timings"),
                  input_record=str(record_path), input_record_sha256=digest(record_path.read_bytes()),
                  failure_record=str(failure_path), failure_record_sha256=digest(failure_path.read_bytes()))
    atomic_json(output / "completed" / f"{job['input_id']}.json", result)
    return result


def resume_result(job: dict, config: dict) -> dict | None:
    path = Path(config["output"]) / "completed" / f"{job['input_id']}.json"
    if not path.exists():
        return None
    result = json.loads(path.read_bytes())
    if result.get("run_fingerprint") != config["run_fingerprint"]:
        raise FingerprintMismatch(f"Resume checkpoint belongs to another frozen run: {path}")
    for key in ("input_record", "failure_record"):
        artifact_path = Path(result[key]).resolve()
        if not artifact_path.is_relative_to(Path(config["output"]).resolve()):
            raise FingerprintMismatch(f"Resume checkpoint escapes output: {artifact_path}")
        freeze(artifact_path, result[key + "_sha256"])
    return result


def make_summary(config: dict, cases: list[dict], results: dict[str, dict], available: int, seconds: float,
                 resumed: int, abort: str | None = None) -> dict:
    output = Path(config["output"])
    file_statuses, function_statuses, usage_statuses = Counter(), Counter(), Counter()
    groups = defaultdict(lambda: {"files": 0, "file_statuses": Counter(), "function_statuses": Counter()})
    records = {}
    for result in results.values():
        for comparison in result["comparisons"].values():
            for case_id in comparison["case_ids"]:
                records[case_id] = (result, comparison)
    with gzip.open(output / "cases.jsonl.gz", "wt", encoding="utf-8") as stream:
        for case in cases:
            if case["id"] not in records:
                row = {"id": case["id"], "status": "unprocessed", "error": abort}
            else:
                result, comparison = records[case["id"]]
                row = {key: case.get(key) for key in ("id", "optimization", "project", "kind", "source", "metric_excluded", "prepared_sha256")}
                row.update({key: value for key, value in comparison.items() if key != "case_ids"})
                row.update(usage_status=result["status"], diagnostics_count=result["diagnostics_count"],
                           error_preview=result["error_preview"], shared_measurement_id=result["input_id"],
                           timings=result["timings"], fallback_timings=result["fallback_timings"],
                           input_record=result["input_record"], failure_record=result["failure_record"],
                           timing_scope="shared_unique_prepared_input")
                usage_statuses.update([result["status"]])
                function_statuses.update(comparison["function_statuses"])
                for key in (f"{case['optimization']}/{case['kind']}", f"project/{case['project']}"):
                    group = groups[key]
                    group["files"] += 1
                    group["file_statuses"].update([row["status"]])
                    group["function_statuses"].update(comparison["function_statuses"])
            file_statuses.update([row["status"]])
            stream.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n")
    with gzip.open(output / "failures.jsonl.gz", "wt", encoding="utf-8") as stream:
        for input_id in sorted(results):
            with gzip.open(results[input_id]["failure_record"], "rt", encoding="utf-8") as source:
                for line in source:
                    stream.write(line)
    measured = defaultdict(float)
    secondary = defaultdict(float)
    for result in results.values():
        for key, value in result["timings"].items():
            measured[key] += value
        for key, value in (result.get("fallback_timings") or {}).items():
            secondary[key] += value
    summary = {"schema_version": 1, "mode": "direct_native_abi_triage", "run_fingerprint": config["run_fingerprint"],
               "manifest": config["manifest"], "available_cases": available, "selected_cases": len(cases),
               "recorded_cases": len(records), "complete_selected_scope": len(records) == len(cases) and not abort,
               "complete_corpus": len(records) == len(cases) == available and not abort,
               "selected_unique_inputs": config["selected_unique_inputs"], "recorded_unique_inputs": len(results),
               "resumed_unique_inputs": resumed, "abort": abort,
               "file_statuses": dict(file_statuses), "usage_statuses": dict(usage_statuses),
               "function_statuses": dict(function_statuses), "groups": dict(groups),
               "metric_excluded_cases": sum(bool(case.get("metric_excluded")) for case in cases),
               "seconds": seconds, "native_library": config["native_library"],
               "source_build_fingerprint": config["source_build_fingerprint"],
               "build_provenance": config["build_provenance"],
               "preprocessed_option_supported": config["preprocessed_option_supported"],
               "requested_options": config["requested_options"], "worker_config": config["worker_config"],
               "native_invocations": sum(result["native_invocations"] for result in results.values()),
               "timings_unique_inputs_total": dict(measured), "fallback_timings_unique_inputs_total": dict(secondary),
               "timing_notes": "C ABI analysis/encoding plus Python JSON decoding. Case timings share one measured prepared input; sums are over unique inputs. Fallback is secondary. DecBench preparation and CFG object materialization are not measured here.",
               "case_records": "cases.jsonl.gz", "failure_records": "failures.jsonl.gz", "input_records": "inputs/*.json.gz"}
    atomic_json(output / "summary.json", summary)
    return summary


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--build-provenance", type=Path, required=True, help="Immutable snapshot/library fingerprints")
    parser.add_argument("--library", type=Path, help="Defaults to the provenance library path")
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--rayon-threads", type=int, default=2)
    parser.add_argument("--case", action="append")
    parser.add_argument("--case-file", type=Path, help="Newline case IDs, or a JSON array of IDs")
    parser.add_argument("--project", action="append")
    parser.add_argument("--kind", choices=("source", "ida", "kuna"), action="append")
    parser.add_argument("--optimization", choices=("O0", "O2"), action="append")
    parser.add_argument("--limit", type=int)
    parser.add_argument("--resume", action="store_true", help="Reuse only verified checkpoints from the same frozen run")
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
        parser.error("manifest contains duplicate case IDs")
    selected_ids = set(args.case or ())
    if args.case_file:
        content = args.case_file.read_text()
        selected_ids.update(json.loads(content) if content.lstrip().startswith("[") else content.splitlines())
    unknown = selected_ids - {case["id"] for case in cases}
    if unknown:
        parser.error(f"Unknown case IDs: {sorted(unknown)}")
    for field, selection in (("id", selected_ids), ("project", args.project), ("kind", args.kind), ("optimization", args.optimization)):
        if selection:
            cases = [case for case in cases if case[field] in selection]
    if args.limit:
        cases = cases[:args.limit]
    if not cases:
        parser.error("No cases selected")
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()) and not args.resume:
        parser.error("Output is not empty; choose a new folder or --resume")
    output.mkdir(parents=True, exist_ok=True)
    provenance_guard = freeze(args.build_provenance)
    provenance = json.loads(read_frozen(provenance_guard))
    library = args.library or Path(provenance["native_library"]["path"])
    library_guard = freeze(library, provenance.get("library_sha256", provenance["native_library"]["sha256"]))
    if library_guard["sha256"] != provenance["native_library"]["sha256"]:
        raise FingerprintMismatch("Build provenance records conflicting library digests")
    snapshot = Path(provenance["snapshot_root"]).resolve()
    native_digest, source_digest, snapshot_files = source_fingerprints(snapshot)
    if native_digest != provenance["native_source_sha256"] or source_digest != provenance["source_build_fingerprint"]:
        raise FingerprintMismatch("Immutable build snapshot differs from the recorded source fingerprint")
    guards = [manifest_guard, provenance_guard, library_guard, freeze(__file__),
              freeze(Path(__file__).with_name("compare_pyjoern.py")), *(freeze(path) for path in snapshot_files)]
    if source_fingerprints(snapshot)[:2] != (native_digest, source_digest):
        raise FingerprintMismatch("Immutable build snapshot changed while its fingerprints were checked")
    artifacts = {}
    grouped = {}
    for case in cases:
        key = (case["language"], case["prepared_sha256"])
        input_id = "-".join(key)
        for path, expected in ((case["prepared_source"], case["prepared_sha256"]), (case["reference"], None)):
            path = str(Path(path).resolve())
            if path not in artifacts:
                artifacts[path] = freeze(path, expected)
            elif expected and artifacts[path]["sha256"] != expected:
                raise FingerprintMismatch(f"Manifest records conflicting input hashes: {path}")
        job = grouped.setdefault(key, {"input_id": input_id, "language": key[0],
                                       "input": artifacts[str(Path(case["prepared_source"]).resolve())],
                                       "case_ids": [], "references": {}})
        job["case_ids"].append(case["id"])
        reference_path = str(Path(case["reference"]).resolve())
        reference = job["references"].setdefault(reference_path, {"artifact": artifacts[reference_path], "case_ids": []})
        reference["case_ids"].append(case["id"])
    jobs = [{**job, "references": list(job["references"].values())} for job in grouped.values()]
    config = {"schema_version": 1, "output": str(output), "manifest": manifest_guard["path"],
              "manifest_sha256": manifest_guard["sha256"], "selected_case_ids_sha256": signature([case["id"] for case in cases]),
              "selected_unique_inputs": len(jobs), "native_library": {key: library_guard[key] for key in ("path", "sha256")},
              "source_build_fingerprint": source_digest,
              "build_provenance": {key: provenance_guard[key] for key in ("path", "sha256")},
              "preprocessed_option_supported": provenance.get("preprocessed_option_supported"),
              "requested_options": {"strict": True, "data_flow": False, "reaching_definitions": False, "preprocessed": True},
              "worker_config": {"workers": args.workers, "start_method": "spawn", "rayon_threads": args.rayon_threads},
              "frozen_artifacts": [{key: artifact[key] for key in ("path", "sha256")} for artifact in [*guards, *artifacts.values()]]}
    config["run_fingerprint"] = signature(config)
    run_path = output / "run.json"
    if run_path.exists():
        if not args.resume or json.loads(run_path.read_bytes()).get("run_fingerprint") != config["run_fingerprint"]:
            raise FingerprintMismatch("Output belongs to another library/options/scope/input/oracle/tool version; choose a new folder")
    else:
        atomic_json(run_path, config)
    results = {}
    pending = []
    for job in jobs:
        previous = resume_result(job, config) if args.resume else None
        if previous:
            results[job["input_id"]] = previous
        else:
            pending.append(job)
    resumed = len(results)
    print(f"Frozen batch: {len(cases)}/{available} cases, {len(jobs)} unique inputs, {resumed} resumed; library {library_guard['sha256'][:12]}", flush=True)
    abort = None
    executor = None
    try:
        if pending:
            executor = ProcessPoolExecutor(max_workers=args.workers, mp_context=multiprocessing.get_context("spawn"),
                                           initializer=initialize, initargs=(config, guards))
            futures = {executor.submit(process_input, job): job["input_id"] for job in pending}
            for future in as_completed(futures):
                result = future.result()
                results[result["input_id"]] = result
                if len(results) % 100 == 0 or len(results) == len(jobs):
                    print(f"{len(results)}/{len(jobs)} unique inputs completed in {time.perf_counter() - started:.1f}s", flush=True)
            executor.shutdown(wait=True)
            executor = None
        for artifact in [*guards, *artifacts.values()]:
            read_frozen(artifact)
    except (Exception, KeyboardInterrupt) as error:
        abort = f"{type(error).__name__}: {error}"
        print(abort, file=sys.stderr, flush=True)
        if executor is not None:
            if hasattr(executor, "terminate_workers"):
                executor.terminate_workers()
            else:
                executor.shutdown(wait=False, cancel_futures=True)
    summary = make_summary(config, cases, results, available, time.perf_counter() - started, resumed, abort)
    print(f"Recorded {summary['recorded_cases']}/{len(cases)} cases: {summary['file_statuses']}; {summary['seconds']:.3f}s", flush=True)
    return 2 if abort else int(any(key != "match" for key in summary["file_statuses"]))


if __name__ == "__main__":
    raise SystemExit(main())
