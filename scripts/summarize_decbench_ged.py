#!/usr/bin/env python3
"""Project a fresh GED overlay onto frozen DecBench results without publishing.

Uses DecBench's slice-scoped merge and authoritative scoreboard/site aggregate
builders. All other metrics, function rows, frozen memberships and source/code
extras are retained. The output is an isolated report, never a canonical tree.
"""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime, timezone
import gzip
import hashlib
import json
import math
from pathlib import Path
import sys
import time


def digest_file(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            hasher.update(chunk)
    return hasher.hexdigest()


def encoded(value) -> bytes:
    return json.dumps(value, sort_keys=True, ensure_ascii=True, separators=(",", ":"), allow_nan=False).encode()


def write_json(path: Path, value) -> None:
    data = json.dumps(value, ensure_ascii=True, indent=2, allow_nan=False).encode() + b"\n"
    path.write_bytes(gzip.compress(data, mtime=0) if path.suffix == ".gz" else data)


def load_json(path: Path):
    data = path.read_bytes()
    return json.loads(gzip.decompress(data) if path.suffix == ".gz" else data)


def function_index(fd) -> dict:
    result = {}
    for group in fd.groups:
        for function in group.functions:
            key = (group.opt_level, group.project, group.binary, function.function)
            if key in result:
                raise ValueError(f"Duplicate function identity: {key}")
            result[key] = function
    return result


def without_ged_columns(record: dict, columns=("values", "perfects", "distances")) -> dict:
    result = dict(record)
    for column in columns:
        if column in result:
            result[column] = {dec: rest for dec, metrics in result[column].items()
                if (rest := {metric: value for metric, value in metrics.items() if metric != "ged"})}
    return result


def preservation_digest(fd) -> str:
    """Hash every non-GED fact, including code extras and ordered membership."""
    hasher = hashlib.sha256()
    hasher.update(encoded(fd.model_dump(mode="json", exclude={"groups", "samples"})))
    for group in fd.groups:
        hasher.update(encoded(group.model_dump(mode="json", exclude={"functions"})))
        for function in group.functions:
            hasher.update(encoded(without_ged_columns(function.model_dump(mode="json"))))
    for sample in fd.samples:
        hasher.update(encoded(without_ged_columns(sample.model_dump(mode="json"), ("values", "perfects"))))
    return hasher.hexdigest()


def membership(fd) -> dict:
    records = [(group.opt_level, group.project, group.binary, function.function, function.datasets)
               for group in fd.groups for function in group.functions]
    frozen = sorted(tuple(row[:4]) for row in records if "sample-set" in row[4])
    return {"function_rows": len(records), "binary_groups": len(fd.groups),
            "ordered_membership_sha256": hashlib.sha256(encoded(records)).hexdigest(),
            "sample_set_count": len(frozen), "sample_set_sha256": hashlib.sha256(encoded(frozen)).hexdigest(),
            "sample_set_keys": frozen}


def validate_manifest(fd, path: Path | None) -> dict:
    facts = membership(fd)
    if path is None or not path.is_file():
        return {"available": False, "membership_source": "frozen baseline FunctionData datasets", **facts}
    raw = load_json(path)
    keys = sorted((row["opt"], row["project"], row["binary"], row["function"]) for row in raw["functions"])
    if len(set(keys)) != len(keys):
        raise ValueError("Frozen sample manifest contains duplicate targets")
    if keys != facts["sample_set_keys"]:
        raise ValueError("Frozen sample manifest differs from baseline sample-set membership")
    return {"available": True, "path": str(path), "sha256": digest_file(path), **facts}


def ged_state(function, dec: str) -> dict:
    result = {}
    for column, label in (("values", "value"), ("perfects", "perfect"), ("distances", "distance")):
        values = getattr(function, column).get(dec, {}) if function is not None else {}
        result[label + "_present"] = "ged" in values
        result[label] = values.get("ged")
    return result


def refresh_sample_ged(fd) -> dict:
    indexed = function_index(fd)
    counts = Counter()
    for sample in fd.samples:
        function = indexed.get((sample.opt_level, sample.project, sample.binary, sample.function))
        if function is None:
            raise ValueError(f"Sample extra references absent function: {sample.function}")
        for dec in set(fd.decompilers) | set(sample.values) | set(sample.perfects):
            before = {column: getattr(sample, column).get(dec, {}).get("ged") for column in ("values", "perfects")}
            for column in ("values", "perfects"):
                destination, source = getattr(sample, column), getattr(function, column).get(dec, {})
                if "ged" in source:
                    destination.setdefault(dec, {})["ged"] = source["ged"]
                elif dec in destination:
                    destination[dec].pop("ged", None)
            after = {column: getattr(sample, column).get(dec, {}).get("ged") for column in ("values", "perfects")}
            counts["changed" if before != after else "unchanged"] += 1
    return dict(counts)


def read_overlay(overlay: Path, slices: Path, fd) -> tuple[dict, set, dict]:
    from decbench.results_store import slices_from_overlay_keys
    values = load_json(overlay)
    raw_slices = load_json(slices)
    covered = {tuple(item.split("::", 3)) for item in raw_slices}
    if any(len(item) != 4 for item in covered):
        raise ValueError("Invalid covered-slice key")
    inferred = slices_from_overlay_keys(values)
    covered |= inferred
    known = function_index(fd)
    unmatched = []
    for key, row in values.items():
        parts = key.split("::", 4)
        if len(parts) != 5:
            raise ValueError(f"Invalid overlay key: {key}")
        opt, project, binary, dec, function = parts
        if dec not in fd.decompilers or (opt, project, binary, function) not in known:
            unmatched.append(key)
        value = float(row["value"])
        if not math.isfinite(value) or value < 0:
            raise ValueError(f"GED overlay contains an invalid distance: {key}")
        if bool(row.get("perfect", value == 0)) != (value == 0):
            raise ValueError(f"GED zero/perfect flags disagree: {key}")
    return values, covered, {"overlay_entries": len(values), "sidecar_slices": len(raw_slices),
                            "covered_slices": len(covered), "slices_inferred_beyond_sidecar": len(inferred - {tuple(k.split("::", 3)) for k in raw_slices}),
                            "overlay_keys_not_in_baseline": unmatched}


def raw_differences(before, after, output: Path) -> dict:
    counts = {dec: Counter() for dec in before.decompilers}
    old_index, new_index = function_index(before), function_index(after)
    if old_index.keys() != new_index.keys() or before.decompilers != after.decompilers:
        raise ValueError("Function/decompiler identities changed during GED-only projection")
    with output.open("wb") as file:
        with gzip.GzipFile(fileobj=file, mode="wb", mtime=0) as stream:
            for key, function in old_index.items():
                for dec in before.decompilers:
                    old, new = ged_state(function, dec), ged_state(new_index[key], dec)
                    if old["value_present"] and not new["value_present"]:
                        status = "dropped"
                    elif new["value_present"] and not old["value_present"]:
                        status = "added"
                    elif old["value_present"] and new["value_present"] and old["value"] != new["value"]:
                        status = "changed"
                    elif not old["value_present"] and not new["value_present"]:
                        status = "unmeasured_both"
                    else:
                        status = "unchanged"
                    counter = counts[dec]
                    counter[status] += 1
                    counter["baseline_measured"] += old["value_present"]
                    counter["new_measured"] += new["value_present"]
                    counter["perfect_or_distance_metadata_changed"] += old != new and status in {"unchanged", "unmeasured_both"}
                    counter["state_changed"] += old != new
                    record = {"key": "::".join([*key[:3], dec, key[3]]), "status": status,
                              "state_changed": old != new, "baseline": old, "new": new}
                    stream.write(encoded(record) + b"\n")
    return {"rows": len(old_index) * len(before.decompilers), "by_identity": {dec: dict(c) for dec, c in counts.items()},
            "totals": dict(sum(counts.values(), Counter())), "path": str(output), "sha256": digest_file(output)}


def combo_rows(aggregates: dict) -> list[dict]:
    rows = []
    for name, combo in aggregates["combos"].items():
        preset, normalize = name.rsplit("|", 1)
        restricted, sample_only = aggregates.get("decompiler_presets", {}), set(aggregates.get("sample_set_only", []))
        eligible = [dec for dec in aggregates["decompilers"]
                    if (dec not in restricted or preset in restricted[dec])
                    and (dec not in sample_only or preset == "sample-set")]
        percentages = {dec: (100 * pair[0] / pair[1] if pair[1] else None)
                       for dec in aggregates["decompilers"] if (pair := combo["per_metric"][dec].get("ged")) is not None}
        ranked = sorted((dec for dec in eligible if percentages.get(dec) is not None), key=lambda d: -percentages[d])
        ranks = {dec: i + 1 for i, dec in enumerate(ranked)}
        for dec in aggregates["decompilers"]:
            numerator, denominator = combo["per_metric"][dec].get("ged", [0, 0])
            distance = combo["distance"][dec].get("ged")
            rows.append({"preset": preset, "normalize": bool(int(normalize)), "identity": dec,
                "shown_on_preset": dec in eligible, "rank": ranks.get(dec), "perfect_count": numerator,
                "denominator": denominator, "perfect_percentage": percentages.get(dec),
                "distance": distance, "mean": distance["mean"] if distance else None,
                "median": distance["median"] if distance else None, "measured_count": distance["n"] if distance else 0,
                "functions": combo["functions"], "binaries": combo["binaries"]})
    return rows


def build_reports(fd, name: str) -> dict:
    from decbench.rendering.aggregate import build_aggregates
    from decbench.rendering.visibility import apply_hidden_decompilers
    from decbench.scoring.scoreboard import build_scoreboard_from_function_data
    scoreboard = build_scoreboard_from_function_data(fd, name=name)
    all_aggregates = build_aggregates(fd, scoreboard)
    site_scoreboard, visible = apply_hidden_decompilers(scoreboard, fd)
    # Recompute the shared denominators after hiding, as the actual site does.
    site_scoreboard = build_scoreboard_from_function_data(visible, name=name)
    site_aggregates = build_aggregates(visible, site_scoreboard)
    return {"scoreboard": scoreboard.model_dump(mode="json"),
            "site_scoreboard": site_scoreboard.model_dump(mode="json"),
            "all_identity_aggregates": all_aggregates, "site_aggregates": site_aggregates,
            "all_identity_ged": combo_rows(all_aggregates), "site_ged": combo_rows(site_aggregates)}


def report_delta(before: list, after: list) -> list:
    key = lambda row: (row["preset"], row["normalize"], row["identity"])
    old = {key(row): row for row in before}
    return [{"preset": row["preset"], "normalize": row["normalize"], "identity": row["identity"],
             "baseline": old[key(row)], "new": row,
             "percentage_point_change": (row["perfect_percentage"] - old[key(row)]["perfect_percentage"]
                if row["perfect_percentage"] is not None and old[key(row)]["perfect_percentage"] is not None else None),
             "perfect_count_change": row["perfect_count"] - old[key(row)]["perfect_count"],
             "denominator_change": row["denominator"] - old[key(row)]["denominator"]}
            for row in after]


def audit_native_run(overlay: Path) -> dict:
    root = overlay.parent.parent
    manifest_path = root / "manifest.json"
    if not manifest_path.is_file():
        return {"available": False, "provisional": True, "reason": "Native execution evidence was not supplied"}
    manifest = load_json(manifest_path)
    tree = manifest.get("trees", {}).get(overlay.parent.name)
    if tree is None:
        return {"available": False, "provisional": True, "reason": "Overlay is not a catalogued native run tree"}
    sources = {}
    for group in manifest.get("source_inputs", []):
        for task in group.get("units", {}).values():
            sources[task["key"]] = task
    records, errors, excluded = {}, [], Counter()
    evidence = {}
    for stage, tasks in (("source", list(sources.values())), ("candidate", tree["artifacts"])):
        statuses = Counter()
        for task in tasks:
            path = Path(task["record"])
            if not path.is_file():
                statuses["missing_checkpoint"] += 1
                continue
            record = load_json(path)
            evidence[str(path)] = digest_file(path)
            statuses[record.get("status", "missing_status")] += 1
            if record.get("status") != "ok":
                errors.append({"stage": stage, "key": task["key"], "record": str(path),
                               "error": record.get("error"), "diagnostics": record.get("diagnostics", [])})
            if stage == "candidate":
                excluded.update(record.get("excluded", {}).values())
        records[stage] = {"planned": len(tasks), "statuses": dict(statuses)}
    completed = root / "completed.json"
    timings = root / "timings.json"
    finished = load_json(completed) if completed.is_file() else None
    sidecar = overlay.with_name("ged_new.slices.json")
    actual_slices = set(load_json(sidecar)) if sidecar.is_file() else set()
    planned_slices = {task["key"] for task in tree["artifacts"]}
    missing_slices = sorted(planned_slices - actual_slices)
    extra_slices = sorted(actual_slices - planned_slices)
    metric_errors = sum(n for reason, n in excluded.items() if reason.startswith("metric_error"))
    successful = all(info["statuses"].get("ok", 0) == info["planned"] for info in records.values())
    verified = bool(finished and finished.get("code_unchanged") and successful and not metric_errors and not missing_slices and not extra_slices)
    return {"available": True, "provisional": not verified, "execution_completed": finished is not None,
        "execution_completed_record": finished, "execution_manifest": str(manifest_path),
        "execution_manifest_sha256": digest_file(manifest_path), "stage_statuses": records,
        "parse_errors": errors, "metric_error_count": metric_errors, "exclusions": dict(excluded),
        "missing_evaluated_slices": missing_slices, "extra_evaluated_slices": extra_slices,
        "checkpoint_evidence_sha256": evidence, "timings": load_json(timings) if timings.is_file() else None,
        "note": "Aggregate arithmetic does not certify parser recovery. Parse/metric errors or incomplete execution make these score projections provisional."}


def process_tree(name: str, baseline: Path, overlay: Path, slices: Path, output: Path, manifest: Path | None = None) -> tuple:
    from decbench.models.function_data import FunctionData
    from decbench.results_store import update_ged
    started = time.perf_counter()
    before = FunctionData.model_validate(load_json(baseline))
    frozen = validate_manifest(before, manifest)
    preserve_before = preservation_digest(before)
    values, covered, overlay_audit = read_overlay(overlay, slices, before)
    baseline_reports = build_reports(before, f"{name} baseline")
    native_audit = audit_native_run(overlay)
    after = before.model_copy(deep=True)
    applied = update_ged(after, values, covered=covered)
    if overlay_audit["overlay_keys_not_in_baseline"]:
        raise ValueError(f"Overlay has {len(overlay_audit['overlay_keys_not_in_baseline'])} keys outside the frozen baseline")
    extras = refresh_sample_ged(after)
    preserve_after = preservation_digest(after)
    if preserve_before != preserve_after or membership(before) != membership(after):
        raise ValueError("GED projection changed a non-GED fact or frozen membership")
    new_reports = build_reports(after, f"{name} Rust GED" + (" (provisional)" if native_audit["provisional"] else ""))
    for artifact in ("scoreboard", "site_scoreboard", "all_identity_aggregates", "site_aggregates"):
        new_reports[artifact]["provisional"] = native_audit["provisional"]
    output.mkdir(parents=True, exist_ok=True)
    changes = raw_differences(before, after, output / "per-function-diff.jsonl.gz")
    for label, reports in (("baseline", baseline_reports), ("new", new_reports)):
        for artifact in ("scoreboard", "site_scoreboard"):
            write_json(output / f"{label}.{artifact}.json", reports[artifact])
        for artifact in ("all_identity_aggregates", "site_aggregates"):
            write_json(output / f"{label}.{artifact}.json.gz", reports[artifact])
    write_json(output / "sample-extras-scores.json.gz", [{"opt_level": sample.opt_level, "project": sample.project,
        "binary": sample.binary, "function": sample.function, "difficulty": sample.difficulty,
        "values": sample.values, "perfects": sample.perfects} for sample in after.samples])
    summary = {"name": name, "inputs": {label: {"path": str(path), "sha256": digest_file(path)}
        for label, path in (("baseline", baseline), ("overlay", overlay), ("slices", slices))},
        "decompilers": before.decompilers, "membership": frozen,
        "preservation": {"non_ged_before_sha256": preserve_before, "non_ged_after_sha256": preserve_after,
                         "all_non_ged_facts_equal": True, "rows_and_membership_equal": True},
        "overlay": {**overlay_audit, "applied_entries": applied}, "raw_changes": changes,
        "sample_extras_ged": extras, "all_identity_combos": report_delta(baseline_reports["all_identity_ged"], new_reports["all_identity_ged"]),
        "site_combos": report_delta(baseline_reports["site_ged"], new_reports["site_ged"]),
        "site_hidden_identities": sorted(set(before.decompilers) - set(new_reports["site_aggregates"]["decompilers"])),
        "native_run_audit": native_audit, "provisional": native_audit["provisional"],
        "timing_seconds": time.perf_counter() - started,
        "notes": ["GED-only projection; no canonical FunctionData or site was written.",
                  "All-identity normalized aggregates include hidden identities; site aggregates use the actual site visibility gate.",
                  "Preset ranks follow descending GED perfect percentage with baseline identity order for ties.",
                  "Site distance median is the authoritative upper-middle convention; scoreboard median retains scoreboard semantics.",
                  "Existing sample extras GED scores were refreshed from groups; memberships, difficulty and code were retained."]}
    write_json(output / "summary.json", summary)
    print(f"{name}: {applied} GED entries; {changes['totals'].get('changed', 0)} changed, {changes['totals'].get('added', 0)} added, {changes['totals'].get('dropped', 0)} dropped", flush=True)
    return before, after, summary


def astra_sync(main_before, main_after, sample_before, sample_after) -> dict:
    keys = membership(main_before)["sample_set_keys"]
    if keys != membership(sample_before)["sample_set_keys"]:
        raise ValueError("Main and standalone frozen Astra sample memberships differ")
    report = {"main_identity": "codex", "standalone_identity": "codex@gpt-6-astra", "targets": len(keys),
              "mapping": "Exact (opt,project,binary,function) keys in the frozen sample-set; optimized main codex@gpt-6-astra is independent."}
    for label, main, sample in (("baseline", main_before, sample_before), ("new", main_after, sample_after)):
        left, right = function_index(main), function_index(sample)
        rows, counts = [], Counter()
        for key in keys:
            original, standalone = ged_state(left.get(tuple(key)), "codex"), ged_state(right.get(tuple(key)), "codex@gpt-6-astra")
            equal = original == standalone
            counts["equal" if equal else "different"] += 1
            counts["main_measured"] += original["value_present"]
            counts["standalone_measured"] += standalone["value_present"]
            counts["main_perfect"] += original["value_present"] and bool(original["perfect"])
            counts["standalone_perfect"] += standalone["value_present"] and bool(standalone["perfect"])
            rows.append({"key": list(key), "equal": equal, "main": original, "standalone": standalone})
        report[label] = {"counts": dict(counts), "all_target_states_equal": counts["different"] == 0, "rows": rows}
    report["denominator_note"] = "The published sample-set GED denominator comes from main shared measurable groups, not the standalone producer's measured count."
    return report


def certify_astra_sync(synchronization: dict, summaries: dict, output: Path) -> bool:
    if not synchronization["baseline"]["all_target_states_equal"]:
        raise ValueError("Frozen main and standalone Astra baseline scores disagree")
    if synchronization["new"]["all_target_states_equal"]:
        return True
    differing = [row["key"] for row in synchronization["new"]["rows"] if not row["equal"]]
    for name, summary in summaries.items():
        summary["provisional"] = True
        summary.setdefault("certification_errors", []).append(
            {"reason": "Main and standalone Astra GED states disagree", "keys": differing})
        write_json(output / name / "summary.json", summary)
        for artifact in ("scoreboard.json", "site_scoreboard.json", "all_identity_aggregates.json.gz", "site_aggregates.json.gz"):
            path = output / name / ("new." + artifact)
            data = load_json(path)
            data["provisional"] = True
            write_json(path, data)
    return False


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("decbench-root", "baseline", "overlay", "slices", "out"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("sample-baseline", "sample-overlay", "sample-slices", "sample-manifest"):
        parser.add_argument("--" + name, type=Path)
    parser.add_argument("--manifest", type=Path)
    args = parser.parse_args()
    if any((args.sample_baseline, args.sample_overlay, args.sample_slices)) and not all((args.sample_baseline, args.sample_overlay, args.sample_slices)):
        parser.error("Sample baseline, overlay and slices must be provided together")
    root, output = args.decbench_root.resolve(), args.out.resolve()
    if output.is_relative_to(root):
        parser.error("--out must be an isolated directory outside the canonical DecBench tree")
    sys.path.insert(0, str(root))
    import decbench
    if not Path(decbench.__file__).resolve().is_relative_to(root):
        raise RuntimeError("Imported DecBench differs from --decbench-root")
    guarded = [args.baseline, args.overlay, args.slices]
    if args.sample_baseline:
        guarded.extend([args.sample_baseline, args.sample_overlay, args.sample_slices])
    hashes = {str(path.resolve()): digest_file(path) for path in guarded}
    started = time.perf_counter()
    manifest = args.manifest or args.baseline.with_name("sample_set_manifest.json")
    if not manifest.is_file() and args.baseline.with_name("baseline.sample_set_manifest.json").is_file():
        manifest = args.baseline.with_name("baseline.sample_set_manifest.json")
    main_before, main_after, main_summary = process_tree("main", args.baseline.resolve(), args.overlay.resolve(), args.slices.resolve(), output / "main", manifest)
    summary = {"schema_version": 1, "generated_at": datetime.now(timezone.utc).isoformat(), "main": main_summary,
               "provisional": main_summary["provisional"]}
    if args.sample_baseline:
        sample_manifest = args.sample_manifest or args.sample_baseline.with_name("sample_set_manifest.json")
        if not sample_manifest.is_file() and args.sample_baseline.with_name("baseline.sample_set_manifest.json").is_file():
            sample_manifest = args.sample_baseline.with_name("baseline.sample_set_manifest.json")
        sample_before, sample_after, sample_summary = process_tree("sample", args.sample_baseline.resolve(), args.sample_overlay.resolve(),
            args.sample_slices.resolve(), output / "sample", sample_manifest)
        summary["sample"] = sample_summary
        summary["provisional"] |= sample_summary["provisional"]
        synchronization = astra_sync(main_before, main_after, sample_before, sample_after)
        write_json(output / "astra-synchronization.json", synchronization)
        synchronized = certify_astra_sync(synchronization, {"main": main_summary, "sample": sample_summary}, output)
        summary["provisional"] |= not synchronized
        summary["astra_synchronization"] = {key: value for key, value in synchronization.items() if key not in {"baseline", "new"}}
        summary["astra_synchronization"].update({label: {key: value for key, value in synchronization[label].items() if key != "rows"}
                                                for label in ("baseline", "new")})
        summary["main_sample_set_scores"] = [row for row in main_summary["site_combos"] if row["preset"] == "sample-set" and row["identity"] == "codex"]
    if any(digest_file(Path(path)) != sha for path, sha in hashes.items()):
        raise RuntimeError("A frozen input changed during summarization")
    modules = [root / "decbench" / name for name in ("results_store.py", "scoring/scoreboard.py", "rendering/aggregate.py",
               "rendering/visibility.py", "rendering/content.py", "models/function_data.py", "models/scoreboard.py")]
    modules.extend(sorted((root / "decbench/rendering/content").glob("*.toml")))
    summary["provenance"] = {"python": sys.version, "interpreter": sys.executable, "decbench_root": str(root),
        "decbench_version": getattr(decbench, "__version__", None), "script_sha256": digest_file(Path(__file__)),
        "input_sha256_unchanged": hashes, "authoritative_code_sha256": {str(path.relative_to(root)): digest_file(path) for path in modules},
        "summary_wall_seconds": time.perf_counter() - started}
    write_json(output / "summary.json", summary)
    print(f"Saved isolated GED report to {output}", flush=True)


if __name__ == "__main__":
    main()
