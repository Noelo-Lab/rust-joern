#!/usr/bin/env python3
"""Verify every saved CFG through DecBench's offline graph view, without parsing.

Normalized reports omit CPG statements and unselected functions. This checks
serialized nodes, edges, roles and degeneracy, and explicitly leaves Python
statement classification and parse_source selection unverified.
"""

from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ProcessPoolExecutor
from functools import lru_cache
import gzip
import json
import multiprocessing
from pathlib import Path
import sys
import time

from compare_native_corpus import atomic_json, freeze, read_frozen, unchanged
from compare_pyjoern import BLACKLIST


@lru_cache(maxsize=4096)
def roundtrip(parts: tuple) -> None:
    from decbench.publish.cfg_export import rebuild_cfg, relabel_cfg
    from decbench.utils.cfg import is_degenerate_source_cfg

    nodes, edges, entry, exit_, degenerate = parts
    if len(set(nodes)) != len(nodes):
        raise ValueError("CFG contains duplicate node IDs")
    if not set(entry + exit_) <= set(nodes):
        raise ValueError("CFG contains a role for an absent node")
    if any(len(edge) != 2 or not set(edge) <= set(nodes) for edge in edges):
        raise ValueError("CFG contains an edge to an absent node")
    cfg = {"nodes": list(nodes), "edges": list(edges), "entry": list(entry), "exit": list(exit_), "degenerate": degenerate}
    graph = rebuild_cfg(cfg)
    new_nodes, new_edges, _, new_entry, new_exit, new_degenerate = relabel_cfg(graph)
    index = {node: i for i, node in enumerate(nodes)}
    if (new_nodes != list(range(len(nodes)))
            or {tuple(edge) for edge in new_edges} != {(index[u], index[v]) for u, v in edges}
            or set(new_entry) != {index[node] for node in entry}
            or set(new_exit) != {index[node] for node in exit_}
            or new_degenerate != degenerate or is_degenerate_source_cfg(graph) != degenerate):
        raise ValueError("DecBench rebuild/relabel changed topology, roles or degeneracy")


def check_input(job: tuple) -> dict:
    checkpoint_guard, generation_fingerprint = job
    checkpoint = json.loads(read_frozen(checkpoint_guard))
    if checkpoint["run_fingerprint"] != generation_fingerprint:
        raise ValueError("Checkpoint belongs to another native generation")
    input_guard = freeze(checkpoint["input_record"], checkpoint["input_record_sha256"])
    record = json.loads(gzip.decompress(read_frozen(input_guard)))
    if record["run_fingerprint"] != generation_fingerprint:
        raise ValueError("Input record belongs to another native generation")
    before = roundtrip.cache_info()
    failures, degenerate = [], 0
    for name, function in record["functions"].items():
        try:
            if name != function["name"] or not name or name.startswith(BLACKLIST):
                raise ValueError("Selected function key/name violates the recorded API contract")
            cfg = function["cfg"]
            if not cfg["nodes"]:
                raise ValueError("Selected function has an empty CFG")
            roundtrip((tuple(cfg["nodes"]), tuple(tuple(edge) for edge in cfg["edges"]),
                       tuple(cfg["entry"]), tuple(cfg["exit"]), cfg["degenerate"]))
            degenerate += bool(cfg["degenerate"])
        except (ValueError, KeyError, TypeError) as error:
            failures.append({"function": name, "error": str(error), "cfg": function.get("cfg")})
    unchanged(input_guard)
    unchanged(checkpoint_guard)
    after = roundtrip.cache_info()
    return {"input_id": record["input_id"], "case_ids": record["case_ids"],
            "input_record": input_guard["path"], "input_record_sha256": input_guard["sha256"],
            "native_status": record["status"], "functions": len(record["functions"]),
            "degenerate_functions": degenerate, "status": "serialized_roundtrip_error" if failures else "serialized_roundtrip_match",
            "worker_cache_misses": after.misses - before.misses, "worker_cache_hits": after.hits - before.hits,
            "failures": failures}


def initialize(decbench: str) -> None:
    sys.path.insert(0, decbench)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--generation", type=Path, required=True, help="Completed direct-native report directory")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--decbench", type=Path, required=True)
    parser.add_argument("--workers", type=int, default=8)
    args = parser.parse_args()
    if not 1 <= args.workers <= 32:
        parser.error("workers must be 1..32")
    if args.output.exists() and any(args.output.iterdir()):
        parser.error("Output is not empty; choose a new folder")
    args.output.mkdir(parents=True, exist_ok=True)
    started = time.perf_counter()
    generation_guard = freeze(args.generation / "summary.json")
    generation = json.loads(read_frozen(generation_guard))
    library_guard = freeze(generation["native_library"]["path"], generation["native_library"]["sha256"])
    guards = [generation_guard, library_guard, freeze(__file__), freeze(Path(__file__).with_name("compare_pyjoern.py")),
              freeze(args.decbench / "decbench/publish/cfg_export.py"), freeze(args.decbench / "decbench/utils/cfg.py")]
    checkpoints = [freeze(path) for path in sorted((args.generation / "completed").glob("*.json"))]
    jobs = [(checkpoint, generation["run_fingerprint"]) for checkpoint in checkpoints]
    statuses, usage, cases = Counter(), Counter(), set()
    functions = degenerate = misses = hits = 0
    with gzip.open(args.output / "inputs.jsonl.gz", "wt", encoding="utf-8") as output:
        with ProcessPoolExecutor(max_workers=args.workers, mp_context=multiprocessing.get_context("spawn"),
                                 initializer=initialize, initargs=(str(args.decbench.resolve()),)) as executor:
            for row in executor.map(check_input, jobs, chunksize=8):
                output.write(json.dumps(row, ensure_ascii=False, separators=(",", ":")) + "\n")
                statuses.update([row["status"]])
                usage.update([row["native_status"]])
                cases.update(row["case_ids"])
                functions += row["functions"]
                degenerate += row["degenerate_functions"]
                misses += row["worker_cache_misses"]
                hits += row["worker_cache_hits"]
    for guard in [*guards, *checkpoints]:
        unchanged(guard)
    complete = len(checkpoints) == generation["recorded_unique_inputs"] and len(cases) == generation["recorded_cases"]
    summary = {"schema_version": 1, "mode": "serialized_decbench_graph_roundtrip", "generation_run_fingerprint": generation["run_fingerprint"],
               "native_library": generation["native_library"], "source_build_fingerprint": generation["source_build_fingerprint"],
               "input_records": len(checkpoints), "case_ids": len(cases), "functions": functions,
               "degenerate_functions": degenerate, "complete_saved_graph_coverage": complete,
               "statuses": dict(statuses), "native_usage_statuses": dict(usage),
               "worker_cache_misses": misses, "worker_cache_hits": hits, "native_invocations": 0,
               "seconds": time.perf_counter() - started,
               "verified": ["DecBench rebuild/relabel topology", "entry/exit roles", "serialized degeneracy", "selected name/key consistency"],
               "unverified": ["raw CPG cfg_nop to Python statement classes", "preselection duplicate/blacklist/empty-function records", "actual parse_source and extract_cfgs_from_source"],
               "limitation": "Normalized inputs discarded raw CPG statements and preselection functions; those checks require raw Analysis captures or a future end-to-end extraction run.",
               "artifacts": [{key: guard[key] for key in ("path", "sha256")} for guard in guards]}
    atomic_json(args.output / "summary.json", summary)
    print(f"DecBench graph roundtrip: {len(checkpoints)} inputs/{len(cases)} cases/{functions} functions; {dict(statuses)}; {summary['seconds']:.3f}s")
    return int(not complete or bool(statuses["serialized_roundtrip_error"]))


if __name__ == "__main__":
    raise SystemExit(main())
