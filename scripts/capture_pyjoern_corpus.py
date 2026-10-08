#!/usr/bin/env python3
"""Capture unchanged PyJoern CFGs from a prepared DecBench corpus manifest.

Each parse runs in its own working directory and JVM. Equal prepared bytes and
languages share one reference. Existing references resume a stopped capture.
The reported ``seconds`` measures one original CFG-only ``parse_source`` call,
including its JVM startup; it is never a divided batch measurement.
"""
from __future__ import annotations

import argparse
from concurrent.futures import ThreadPoolExecutor, as_completed
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time


WORKER = r'''
import hashlib, json, pathlib, sys, time
from importlib.metadata import version
config = json.load(sys.stdin)
import pyjoern
package_version = version("pyjoern")
if package_version != "4.0.150.4" or not hasattr(pyjoern, "JOERN_VERSION"):
    raise RuntimeError("The reference interpreter must provide original PyJoern 4.0.150.4")
source = pathlib.Path(config["source"])
if hashlib.sha256(source.read_bytes()).hexdigest() != config["prepared_sha256"]:
    raise RuntimeError("Prepared input changed before the oracle parse")
flags = {"no_metadata": True, "no_ddg": True, "no_ast": True}
started = time.perf_counter()
parsed = pyjoern.parse_source(source, **flags)
elapsed = time.perf_counter() - started
if parsed is None:
    raise RuntimeError("Original PyJoern returned no parse result")
functions = {}
for func in parsed.values():
    graph = func.cfg
    if graph is None:
        raise RuntimeError("Original PyJoern returned no CFG for " + func.name)
    nodes = list(graph)
    index = {node: i for i, node in enumerate(nodes)}
    functions[func.name] = {"name": func.name, "start_line": func.start_line,
        "end_line": func.end_line, "cfg": {
            "nodes": list(range(len(nodes))),
            "edges": [[index[u], index[v]] for u, v in graph.edges],
            "entry": [index[n] for n in nodes if n.is_entrypoint],
            "exit": [index[n] for n in nodes if n.is_exitpoint],
            "degenerate": not nodes or len(nodes) == 1 and all(type(s).__name__ == "Nop" for n in nodes for s in n.statements),
            "labels": {str(index[n]): str(n) for n in nodes},
        }}
print(json.dumps({"schema_version": 1,
    "generator": "pyjoern " + package_version + " / Joern " + pyjoern.JOERN_VERSION,
    "reference_module": str(pathlib.Path(pyjoern.__file__).resolve()),
    "seconds": elapsed, "timing_kind": "isolated_file_parse_source_cfg_only",
    "source_sha256": config["prepared_sha256"],
    "prepared_sha256": config["prepared_sha256"],
    "parse_flags": flags, "empty_reference": not bool(functions), "functions": functions}))
'''


def write_json(path: Path, value: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix="." + path.name + ".", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as stream:
            json.dump(value, stream, separators=(",", ":"))
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        Path(temporary).unlink(missing_ok=True)


def existing_reference(path: Path, digest: str) -> dict | None:
    try:
        value = json.loads(path.read_text())
        if value.get("prepared_sha256", value.get("source_sha256")) == digest and isinstance(value.get("functions"), dict):
            return value
    except (OSError, ValueError):
        pass
    return None


def invoke(python: str, config: dict, cwd: Path, timeout: float) -> tuple[str, str]:
    environment = dict(os.environ)
    environment.pop("PYTHONPATH", None)
    # A private process group lets a timeout stop the Python worker, Joern JVM,
    # and nested C/C++ frontend JVM together, without orphaning expensive work.
    process = subprocess.Popen([python, "-c", WORKER], stdin=subprocess.PIPE,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=cwd,
        env=environment, start_new_session=True)
    try:
        stdout, stderr = process.communicate(json.dumps(config), timeout=timeout)
    except subprocess.TimeoutExpired:
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            os.killpg(process.pid, signal.SIGKILL)
            process.communicate()
        raise
    if process.returncode:
        raise RuntimeError(f"Reference worker exited {process.returncode}: {stderr[-20000:]}")
    return stdout, stderr


def capture(group: dict, args: argparse.Namespace) -> dict:
    case = group["cases"][0]
    digest = case["prepared_sha256"]
    destinations = sorted(group["references"])
    row = {"prepared_sha256": digest, "language": case["language"],
        "cases": [c["id"] for c in group["cases"]], "references": destinations}
    started = time.perf_counter()
    attempts = []
    try:
        prepared = Path(case["prepared_source"]).read_bytes()
        if hashlib.sha256(prepared).hexdigest() != digest:
            raise ValueError("Manifest prepared input hash differs: " + case["prepared_source"])
        checked = {case["prepared_source"]}
        for alias in group["cases"][1:]:
            if alias["prepared_source"] not in checked:
                checked.add(alias["prepared_source"])
                if hashlib.sha256(Path(alias["prepared_source"]).read_bytes()).hexdigest() != digest:
                    raise ValueError("Manifest prepared input hash differs: " + alias["prepared_source"])
        baseline = next((value for destination in destinations
            if (value := existing_reference(Path(destination), digest)) is not None), None)
        if baseline is not None:
            for destination in destinations:
                path = Path(destination)
                if existing_reference(path, digest) is None:
                    write_json(path, baseline)
            return row | {"status": "reused", "functions": len(baseline["functions"])}

        for timeout in (args.timeout, args.retry_timeout):
            attempt_started = time.perf_counter()
            with tempfile.TemporaryDirectory(prefix="rust-joern-oracle-") as temporary:
                cwd = Path(temporary)
                source = cwd / ("input.cpp" if case["language"] == "cpp" else "input.c")
                source.write_bytes(prepared)
                try:
                    stdout, stderr = invoke(args.reference_python,
                        {"source": str(source), "prepared_sha256": digest}, cwd, timeout)
                except subprocess.TimeoutExpired:
                    attempts.append({"timeout_seconds": timeout,
                        "wall_seconds": time.perf_counter() - attempt_started, "status": "timeout"})
                    print(f"timeout: {case['id']} after {timeout:g}s", file=sys.stderr, flush=True)
                    continue
                attempts.append({"timeout_seconds": timeout,
                    "wall_seconds": time.perf_counter() - attempt_started, "status": "success"})
                baseline = json.loads(stdout)
                if baseline["prepared_sha256"] != digest:
                    raise ValueError("Oracle response prepared hash differs")
                baseline["capture_attempts"] = attempts
                baseline["capture_wall_seconds"] = time.perf_counter() - started
                for destination in destinations:
                    path = Path(destination)
                    write_json(path, baseline)
                    Path(str(path) + ".error.json").unlink(missing_ok=True)
                return row | {"status": "captured", "seconds": baseline["seconds"],
                    "wall_seconds": baseline["capture_wall_seconds"], "functions": len(baseline["functions"]),
                    "attempts": attempts}
        raise TimeoutError("Original PyJoern timed out on both capture attempts")
    except Exception as error:
        row.update(status="error", error=f"{type(error).__name__}: {error}",
            attempts=attempts, wall_seconds=time.perf_counter() - started)
        for destination in destinations:
            write_json(Path(destination + ".error.json"), row)
        return row


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=8)
    parser.add_argument("--reference-python", default="/home/mahaloz/.virtualenvs/decbench/bin/python")
    parser.add_argument("--output", type=Path, help="Override the manifest reference directory")
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--retry-timeout", type=float, default=600)
    parser.add_argument("--limit", type=int, help="Capture at most this many distinct inputs")
    args = parser.parse_args()
    if args.workers < 1 or min(args.timeout, args.retry_timeout) <= 0:
        parser.error("workers and timeouts must be positive")
    manifest = json.loads(args.manifest.read_text())
    if manifest.get("schema_version") != 1:
        parser.error("unsupported manifest schema")
    groups = {}
    for case in manifest["cases"]:
        key = (case["prepared_sha256"], case["language"])
        group = groups.setdefault(key, {"cases": [], "references": set()})
        group["cases"].append(case)
        destination = Path(case["reference"])
        if args.output:
            destination = args.output / destination.name
        group["references"].add(str(destination.resolve()))
    jobs = list(groups.values())
    if args.limit is not None:
        jobs = jobs[:args.limit]
    started = time.perf_counter()
    rows = []
    with ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = [executor.submit(capture, group, args) for group in jobs]
        for future in as_completed(futures):
            row = future.result()
            rows.append(row)
            message = row.get("error") or f"{row.get('functions', 0)} functions, {row.get('seconds', 0):.2f}s"
            if row["status"] != "reused" or len(rows) % 100 == 0 or len(rows) == len(jobs):
                print(f"[{len(rows)}/{len(jobs)}] {row['status']}: {row['cases'][0]}: {message}", flush=True)
    summary = {"schema_version": 1, "manifest": str(args.manifest.resolve()),
        "mode": "isolated_file", "workers": args.workers,
        "reference_python": args.reference_python,
        "wall_seconds": time.perf_counter() - started,
        "counts": {status: sum(row["status"] == status for row in rows)
            for status in ("captured", "reused", "error")}, "inputs": rows}
    output = args.output or args.manifest.resolve().parent / "references"
    write_json(output / "capture-summary.json", summary)
    print(json.dumps(summary["counts"]))
    return int(summary["counts"]["error"] != 0)


if __name__ == "__main__":
    raise SystemExit(main())
