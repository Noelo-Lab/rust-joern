#!/usr/bin/env python3
"""Audit DecBench binary source selection using saved graphs and real DWARF.

This does not parse source, run Joern, execute binaries, or modify DecBench.
Strict extraction and the recorded permissive fallback remain separate profiles.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import hashlib
import json
from pathlib import Path
import sys
import time

from compare_pyjoern import isomorphic
from compare_decbench import cfg_option_contract


ROOT = Path(__file__).resolve().parents[1]
PROFILES = ("default_strict", "permissive_fallback")


def file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def core_sha256() -> str:
    return hashlib.sha256(b"".join(
        path.name.encode() + path.read_bytes() for path in sorted((ROOT / "src").glob("*.rs"))
    )).hexdigest()


class GraphPool:
    """Intern equal serialized graphs, especially repeated prototype CFGs."""

    def __init__(self) -> None:
        self.graphs: dict[tuple, object] = {}
        self.serial: dict[int, dict] = {}
        self.pairs: dict[tuple[int, int], str] = {}

    def graph(self, cfg: dict):
        from decbench.publish.cfg_export import rebuild_cfg
        from decbench.utils.cfg import is_degenerate_source_cfg

        nodes = tuple(sorted(cfg["nodes"]))
        edges = tuple(sorted({tuple(edge) for edge in cfg["edges"]}))
        entry, exit_ = tuple(sorted(set(cfg["entry"]))), tuple(sorted(set(cfg["exit"])))
        if len(nodes) != len(set(nodes)) or not (set(entry) | set(exit_)) <= set(nodes):
            raise ValueError("Malformed CFG node IDs or role IDs")
        if any(len(edge) != 2 or not set(edge) <= set(nodes) for edge in edges):
            raise ValueError("CFG edge references an absent node")
        key = (nodes, edges, entry, exit_, bool(cfg["degenerate"]))
        if key not in self.graphs:
            serial = dict(nodes=list(nodes), edges=[list(edge) for edge in edges],
                          entry=list(entry), exit=list(exit_), degenerate=key[-1])
            graph = rebuild_cfg(serial)
            if is_degenerate_source_cfg(graph) != key[-1]:
                raise ValueError("Serialized degeneracy does not survive rebuild_cfg")
            self.graphs[key] = graph
            self.serial[id(graph)] = serial
        return self.graphs[key]

    def functions(self, functions: dict) -> dict:
        return {sys.intern(name): self.graph(function["cfg"])
                for name, function in functions.items()}

    def describe(self, graph) -> dict | None:
        if graph is None:
            return None
        serial = self.serial[id(graph)]
        return {"nodes": len(serial["nodes"]), "edges": len(serial["edges"]),
                "degenerate": serial["degenerate"],
                "entries": len(serial["entry"]), "exits": len(serial["exit"])}

    def compare(self, expected, actual) -> str:
        if expected is None:
            return "both_absent" if actual is None else "extra"
        if actual is None:
            return "missing"
        if expected is actual:
            return "match"
        key = (id(expected), id(actual))
        if key not in self.pairs:
            left, right = self.serial[key[0]], self.serial[key[1]]
            self.pairs[key] = (
                "degeneracy_mismatch" if left["degenerate"] != right["degenerate"]
                else "match" if isomorphic(left, right) else "topology_mismatch"
            )
        return self.pairs[key]


def selection_origins(binary: str, by_tu: dict, best: dict, owners: dict,
                      resolved: dict) -> tuple[dict, dict[str, set[str]]]:
    """Trace chosen TUs, verifying every trace against the real helper result."""
    from decbench.utils.cfg import is_degenerate_source_cfg

    origins = {}
    for stem, functions in by_tu.items():
        for name, graph in functions.items():
            if name not in origins and best.get(name) is graph:
                origins[name] = (stem, "cross_tu_best")
    for name, graph in by_tu.get(binary, {}).items():
        if not is_degenerate_source_cfg(graph):
            origins[name] = (binary, "binary_stem")
    owner_tus: dict[str, set[str]] = defaultdict(set)
    for name, stem in owners.values():
        owner_tus[name].add(stem)
    for name, stems in owner_tus.items():
        if len(stems) != 1:
            continue
        stem = next(iter(stems))
        graph = by_tu.get(stem, {}).get(name)
        if graph is None or is_degenerate_source_cfg(graph):
            origins.pop(name, None)
        else:
            origins[name] = (stem, "dwarf_decl_file")
    if set(origins) != set(resolved):
        raise AssertionError("Selection provenance differs from DecBench's resolved names")
    for name, (stem, _) in origins.items():
        if by_tu[stem][name] is not resolved[name]:
            raise AssertionError("Selection provenance differs from DecBench's chosen graph")
    return origins, owner_tus


def load_project(cases: list[dict], candidates: Path, pool: GraphPool,
                 expected_library: str | None) -> tuple[dict, dict, dict, list]:
    from decbench.utils.langs import preprocessed_by_stem

    maps = {name: {} for name in ("reference", *PROFILES)}
    meta, errors, references = {}, [], {}
    paths = {Path(case["source"]).resolve(): case for case in cases}
    selected = preprocessed_by_stem(Path(cases[0]["source"]).parent)
    # DecBench's reeval cache inserts TUs in complete filename order; sorting
    # stems instead changes ties for pairs such as chown-core.i / chown.i.
    for stem, path in sorted(selected.items(), key=lambda item: item[1]):
        case = paths.get(path.resolve())
        if case is None or case.get("metric_excluded"):
            continue
        meta[stem] = {"case": case["id"], "prepared_sha256": case["prepared_sha256"],
                      "strict_available": False, "fallback_available": False,
                      "requested_option_contract": "unavailable",
                      "reference_origin": case.get("reference_origin")}
        try:
            reference_path = Path(case["reference"])
            reference_key = str(reference_path)
            if reference_key not in references:
                document = json.loads(reference_path.read_text())
                if document.get("prepared_sha256", document.get("source_sha256")) != case["prepared_sha256"]:
                    raise ValueError("Reference prepared SHA-256 differs from the case manifest")
                references[reference_key] = pool.functions(document["functions"])
            maps["reference"][stem] = references[reference_key]
        except (OSError, KeyError, ValueError) as error:
            maps["reference"][stem] = {}
            errors.append({"case": case["id"], "side": "reference", "error": str(error)})
        maps["default_strict"][stem] = {}
        maps["permissive_fallback"][stem] = {}
        try:
            document = json.loads((candidates / (case["id"] + ".json")).read_text())
            if document.get("prepared_sha256") != case["prepared_sha256"] or not document.get("prepared_matches_manifest"):
                raise ValueError("Candidate prepared SHA-256 differs from the case manifest")
            if expected_library and document.get("native_library_sha256") != expected_library:
                raise ValueError("Candidate native library differs from generation summary")
            meta[stem]["requested_option_contract"] = "invalid"
            meta[stem]["requested_option_contract"] = cfg_option_contract(document.get("requested_options"))
            functions = pool.functions(document.get("functions", {}))
            strict = document.get("status") == "ok" and not document.get("diagnostics")
            fallback = strict or document.get("diagnostics_source") == "permissive_fallback"
            meta[stem].update(strict_available=strict, fallback_available=fallback,
                              candidate_status=document.get("status"),
                              diagnostic_count=len(document.get("diagnostics", [])))
            if strict:
                maps["default_strict"][stem] = functions
            if fallback:
                maps["permissive_fallback"][stem] = functions
        except (OSError, KeyError, ValueError) as error:
            errors.append({"case": case["id"], "side": "candidate", "error": str(error)})
    return maps, meta, selected, errors


def audit_binary(entry: dict, root: Path, dataset: Path, maps: dict, best: dict,
                 meta: dict, pool: GraphPool, follow_origin: bool) -> dict:
    from decbench.utils import binfmt
    from decbench.utils.cfg import resolved_source_for_binary
    from decbench.utils.results_tree import compiled_dir, resolve_binary

    opt, project, binary = entry["opt"], entry["project"], entry["binary"]
    path = resolve_binary(compiled_dir(root, opt, project), binary)
    record = {"optimization": opt, "project": project, "binary": binary,
              "binary_path": str(path) if path else None,
              "named_functions": len(entry["functions"])}
    if path is None:
        return record | {"status": "binary_missing", "functions": []}
    started = time.perf_counter()
    owners = binfmt.source_function_owners(path, set(maps["reference"]),
                                         follow_abstract_origin=follow_origin)
    record.update(dwarf_read_seconds=time.perf_counter() - started,
                  dwarf_owners=len(owners), status="ok" if owners else "no_dwarf_owners")
    binary_sha = file_sha256(path)
    record.update(binary_sha256=binary_sha,
                  published_binary_sha256=entry.get("sha256"),
                  published_binary_matches=binary_sha == entry.get("sha256"))
    resolved, origins, times = {}, {}, {}
    owner_tus = {}
    for profile in ("reference", *PROFILES):
        started = time.perf_counter()
        resolved[profile] = resolved_source_for_binary(
            binary, maps[profile], best[profile], function_owners=owners,
        )
        times[profile] = time.perf_counter() - started
        origins[profile], owner_tus = selection_origins(
            binary, maps[profile], best[profile], owners, resolved[profile],
        )
    record["resolution_wall_seconds"] = times
    published, published_error = {}, None
    published_path = dataset / entry["source_cfg_path"]
    try:
        document = json.loads(published_path.read_text())
        if (document["opt"], document["project"], document["binary"]) != (opt, project, binary):
            raise ValueError("Published source CFG identity differs from the manifest")
        published = {name: pool.graph(cfg) for name, cfg in document["functions"].items()}
    except (OSError, KeyError, ValueError) as error:
        published_error = str(error)
    record.update(published_source_cfg=str(published_path), published_error=published_error)
    started = time.perf_counter()
    functions, counters = [], {profile: Counter() for profile in PROFILES}
    reference_availability, pub_counts, source_changes = Counter(), Counter(), Counter()
    for name in sorted(entry["functions"]):
        expected = resolved["reference"].get(name)
        ref_description = pool.describe(expected)
        availability = "absent" if expected is None else "degenerate" if ref_description["degenerate"] else "body"
        reference_availability[availability] += 1
        row = {"function": name, "dwarf_owner_tus": sorted(owner_tus.get(name, set())),
               "reference_availability": availability}
        for profile in ("reference", *PROFILES):
            graph = resolved[profile].get(name)
            origin = origins[profile].get(name)
            selected = None
            if graph is not None:
                stem, mode = origin
                selected = {"tu": stem, "selection": mode, "case": meta[stem]["case"],
                            "strict_available": meta[stem]["strict_available"],
                            "cfg": pool.describe(graph)}
            row[profile] = selected
            if profile != "reference":
                status = pool.compare(expected, graph)
                counters[profile][status] += 1
                same_tu = (origin[0] if origin else None) == (
                    origins["reference"].get(name, (None, None))[0])
                row[profile + "_status"] = status
                row[profile + "_same_selected_tu"] = same_tu
                source_changes[profile] += int(not same_tu)
        pub_status = "unavailable" if published_error else pool.compare(expected, published.get(name))
        pub_counts[pub_status] += 1
        row["published_status"] = pub_status
        row["published_proven_consistent"] = pub_status == "match" and record["published_binary_matches"]
        functions.append(row)
    record.update(functions=functions, reference_availability=dict(reference_availability),
                  profile_statuses={name: dict(counts) for name, counts in counters.items()},
                  selected_tu_differences=dict(source_changes),
                  published_statuses=dict(pub_counts),
                  comparison_wall_seconds=time.perf_counter() - started)
    return record


def summarize(records: list[dict], projects: list[dict]) -> dict:
    result = {"binaries": len(records), "named_functions": sum(r["named_functions"] for r in records),
              "binary_statuses": dict(Counter(r["status"] for r in records)),
              "published_binary_hash_matches": sum(r.get("published_binary_matches", False) for r in records),
              "project_input_errors": sum(len(p["input_errors"]) for p in projects),
              "requested_option_contracts": dict(sum((Counter(p.get("requested_option_contracts", {}))
                                                       for p in projects), Counter()))}
    availability, published = Counter(), Counter()
    profiles = {profile: {"statuses": Counter(), "reference_body_statuses": Counter(),
                          "selected_tu_differences": 0} for profile in PROFILES}
    for record in records:
        availability.update(record.get("reference_availability", {}))
        published.update(record.get("published_statuses", {}))
        for profile in PROFILES:
            profiles[profile]["statuses"].update(record.get("profile_statuses", {}).get(profile, {}))
            profiles[profile]["selected_tu_differences"] += record.get("selected_tu_differences", {}).get(profile, 0)
            profiles[profile]["reference_body_statuses"].update(
                row[profile + "_status"] for row in record["functions"]
                if row["reference_availability"] == "body"
            )
    result.update(reference_availability=dict(availability), published_statuses=dict(published),
                  profiles={profile: {key: dict(value) if isinstance(value, Counter) else value
                                      for key, value in data.items()} for profile, data in profiles.items()})
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--candidates", type=Path, required=True)
    parser.add_argument("--dataset", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--project", action="append", help="Bounded smoke scope; omit for all 513 binaries")
    parser.add_argument("--follow-abstract-origin", action=argparse.BooleanOptionalAction, default=True)
    parser.add_argument("--expected-core-sha256")
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    dataset = args.dataset.resolve()
    published_path = dataset / "configs/full/manifest.json"
    published = json.loads(published_path.read_text())
    entries = [entry for entry in published["binaries"] if entry["opt"] in ("O0", "O2")
               and (not args.project or entry["project"] in args.project)]
    groups = defaultdict(list)
    for entry in entries:
        groups[entry["opt"], entry["project"]].append(entry)
    cases = defaultdict(list)
    for case in manifest["cases"]:
        if case["kind"] == "source" and not case.get("metric_excluded"):
            cases[case["optimization"], case["project"]].append(case)
    sys.path.insert(0, manifest["decbench_root"])
    from decbench.utils.cfg import best_source_by_name

    initial_core = core_sha256()
    if args.expected_core_sha256 and initial_core != args.expected_core_sha256:
        raise ValueError("Native source fingerprint differs from the requested frozen scope")
    generation_path = args.candidates / "summary.json"
    generation = json.loads(generation_path.read_text())
    expected_library = generation.get("native_library", {}).get("sha256")
    records, projects = [], []
    started = time.perf_counter()
    for (opt, project), binaries in sorted(groups.items()):
        group_cases = cases[opt, project]
        if not group_cases:
            raise ValueError(f"Published {opt}/{project} has no current source cases")
        project_started = time.perf_counter()
        pool = GraphPool()
        maps, meta, selected, errors = load_project(group_cases, args.candidates, pool, expected_library)
        best = {profile: best_source_by_name(by_tu) for profile, by_tu in maps.items()}
        project_records = [audit_binary(entry, Path(manifest["results_root"]), dataset,
                                       maps, best, meta, pool, args.follow_abstract_origin)
                           for entry in sorted(binaries, key=lambda b: b["binary"])]
        records.extend(project_records)
        projects.append({"optimization": opt, "project": project, "source_tus": len(meta),
                         "strict_available_tus": sum(m["strict_available"] for m in meta.values()),
                         "fallback_available_tus": sum(m["fallback_available"] for m in meta.values()),
                         "requested_option_contracts": dict(Counter(m["requested_option_contract"] for m in meta.values())),
                         "interned_graphs": len(pool.graphs), "input_errors": errors,
                         "seconds": time.perf_counter() - project_started,
                         "summary": summarize(project_records, [])})
        print(f"Ownership {opt}/{project}: {len(binaries)} binaries, "
              f"{sum(r['named_functions'] for r in project_records)} functions; "
              f"{time.perf_counter() - project_started:.1f}s", flush=True)
        del maps, best, meta, selected, pool
    final_core = core_sha256()
    report = {"schema_version": 1, "manifest": str(args.manifest.resolve()),
              "manifest_sha256": file_sha256(args.manifest),
              "published_manifest": str(published_path),
              "published_manifest_sha256": file_sha256(published_path),
              "results_root": manifest["results_root"], "candidates": str(args.candidates.resolve()),
              "scope": {"optimizations": ["O0", "O2"], "projects": args.project,
                        "expected_binaries": len(entries), "follow_abstract_origin": args.follow_abstract_origin,
                        "metric_excluded_tus_in_owner_maps": False},
              "native_source_sha256": initial_core, "native_source_unchanged": initial_core == final_core,
              "candidate_native_library_sha256": expected_library,
              "reference_cfg_generation_seconds": None,
              "wall_seconds": time.perf_counter() - started,
              "notes": [
                  "Real DecBench rebuild_cfg, best_source_by_name, resolved_source_for_binary and source_function_owners are used.",
                  "default_strict excludes every failed or diagnostic-bearing TU; permissive_fallback includes explicitly recorded fallback graphs.",
                  "Graph identity is a complete exact topology-and-role proof; other pairs use directed role-aware isomorphism, never size-only acceptance.",
                  "both_absent is baseline unavailability, not a recovered source-body success.",
                  "Published JSON is a secondary consistency check; TU hash-verified cached references remain authoritative.",
                  "Full source filenames determine TU insertion order and the winner of equal-rank cross-TU fallback ties.",
                  "DWARF read and owner resolution intervals are not CFG generation times.",
                  "Historical two-field candidate options and explicit preprocessed=True three-field options are counted separately; old parser support is not inferred.",
              ], "summary": summarize(records, projects), "projects": projects, "binaries": records}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(json.dumps(report, separators=(",", ":")) + "\n")
    temporary.replace(args.output)
    print(json.dumps(report["summary"], indent=2), flush=True)
    return int(not report["native_source_unchanged"] or report["summary"]["project_input_errors"]
               or any(record["status"] != "ok" for record in records)
               or report["summary"]["profiles"]["default_strict"]["selected_tu_differences"] > 0
               or any(status not in ("match", "both_absent") for status in
                      report["summary"]["profiles"]["default_strict"]["statuses"]))


if __name__ == "__main__":
    raise SystemExit(main())
