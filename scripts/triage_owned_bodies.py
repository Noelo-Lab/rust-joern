#!/usr/bin/env python3
"""Rank saved native failures by DecBench's reference-selected source bodies.

This is read-only triage, not a replacement for corpus parity or re-running
source ownership. Every weight is a named binary/function occurrence from the
saved owner audit; all corpus cases remain in the primary comparison.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import gzip
import hashlib
import json
from pathlib import Path
import time


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def category(message: str) -> str:
    for prefix in ("unsupported expression statement syntax", "unsupported expression",
                   "unsupported statement", "unparsed executable expression",
                   "cannot recover declaration name", "unrecognized top-level brace",
                   "missing expression", "unsupported constructor member initializer"):
        if message.startswith(prefix):
            return prefix
    return message


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--ownership", type=Path, required=True)
    parser.add_argument("--direct", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    started = time.perf_counter()
    manifest = json.loads(args.manifest.read_text())
    ownership = json.loads(args.ownership.read_text())
    run = json.loads((args.direct / "run.json").read_text())
    cases = {case["id"]: case for case in manifest["cases"]}
    weights = defaultdict(Counter)
    for binary in ownership["binaries"]:
        for function in binary["functions"]:
            if function["reference_availability"] == "body":
                weights[function["reference"]["case"]][function["function"]] += 1
    diagnostics = defaultdict(list)
    failures = defaultdict(dict)
    completed = {}
    for path in sorted((args.direct / "completed").glob("*.json")):
        record = json.loads(path.read_text())
        if record["run_fingerprint"] != run["run_fingerprint"]:
            raise ValueError("Completed record belongs to a different native run")
        selected = {case for comparison in record["comparisons"].values()
                    for case in comparison["case_ids"] if case in weights}
        if not selected:
            continue
        for case in selected:
            if record["prepared_sha256"] != cases[case]["prepared_sha256"]:
                raise ValueError("Owner-selected input hash differs from native input")
            completed[case] = record
        with gzip.open(record["failure_record"], "rt") as stream:
            for line in stream:
                row = json.loads(line)
                matching = set(row["case_ids"]) & selected
                if row["type"] == "usage":
                    for case in matching:
                        diagnostics[case] = row["diagnostics"]
                else:
                    for case in matching:
                        if row["function"] in weights[case]:
                            failures[case][row["function"]] = row
    del ownership
    strict, fallback = Counter(), Counter()
    class_counts = defaultdict(Counter)
    class_examples = defaultdict(list)
    weighted_cases = []
    body_failures = []
    for case_id, names in sorted(weights.items()):
        case = cases[case_id]
        count = sum(names.values())
        if case_id not in completed:
            strict["pending"] += count
            fallback["pending"] += count
            continue
        record = completed[case_id]
        valid = record["status"] == "ok" and not record["diagnostics_count"]
        statuses = Counter()
        for name, weight in names.items():
            row = failures[case_id].get(name)
            status = row["status"] if row else "match"
            statuses[status] += weight
            fallback[status] += weight
            strict[status if valid else "usage_blocked"] += weight
            if row:
                body_failures.append({"case": case_id, "function": name,
                                      "owned_occurrences": weight,
                                      "strict_available": valid,
                                      "prepared_source": case["prepared_source"],
                                      "prepared_sha256": case["prepared_sha256"],
                                      "failure": row})
        classes = defaultdict(list)
        for diagnostic in diagnostics[case_id]:
            classes[category(diagnostic["message"])].append(diagnostic)
        for name, values in classes.items():
            class_counts[name].update(cases=1, owned_occurrences=count,
                                     diagnostic_occurrences=len(values))
            class_counts[name].update({"fallback_" + status: value for status, value in statuses.items()})
            class_examples[name].append({"case": case_id, "owned_occurrences": count,
                                         "prepared_source": case["prepared_source"],
                                         "prepared_sha256": case["prepared_sha256"],
                                         "diagnostics": values[:8],
                                         "fallback_statuses": dict(statuses)})
        weighted_cases.append({"case": case_id, "owned_occurrences": count,
                               "strict_available": valid,
                               "diagnostic_classes": list(classes),
                               "fallback_statuses": dict(statuses),
                               "failing_functions": [name for name in names if name in failures[case_id]]})
    for examples in class_examples.values():
        examples.sort(key=lambda row: row["owned_occurrences"], reverse=True)
        seen = set()
        unique = []
        for row in examples:
            if row["prepared_sha256"] not in seen:
                unique.append(row)
                seen.add(row["prepared_sha256"])
                if len(unique) == 8:
                    break
        examples[:] = unique
    weighted_cases.sort(key=lambda row: (not row["strict_available"],
                                        sum(v for k, v in row["fallback_statuses"].items() if k != "match"),
                                        row["owned_occurrences"]), reverse=True)
    body_failures.sort(key=lambda row: row["owned_occurrences"], reverse=True)
    report = {"schema_version": 1, "native_library": run["native_library"],
              "run_fingerprint": run["run_fingerprint"],
              "manifest_sha256": digest(args.manifest),
              "ownership_sha256": digest(args.ownership),
              "reference_selected_tus": len(weights), "completed_selected_tus": len(completed),
              "reference_body_occurrences": sum(sum(names.values()) for names in weights.values()),
              "default_strict_statuses": dict(strict), "permissive_fallback_statuses": dict(fallback),
              "diagnostic_classes": {name: dict(counts) for name, counts in sorted(
                  class_counts.items(), key=lambda item: item[1]["owned_occurrences"], reverse=True)},
              "diagnostic_examples": dict(class_examples),
              "weighted_cases": weighted_cases, "body_failures": body_failures,
              "notes": ["Weights count named binary/function occurrences, including a function used by multiple binaries.",
                        "Diagnostic classes overlap; their occurrence counts must not be summed.",
                        "These are graphs in reference-selected TUs; candidate source-owner resolution is a separate audit.",
                        "This priority view does not reduce the complete corpus parity scope or waive any divergence."],
              "seconds": time.perf_counter() - started}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, separators=(",", ":")) + "\n")
    print(json.dumps({key: report[key] for key in ("reference_selected_tus", "completed_selected_tus",
          "reference_body_occurrences", "default_strict_statuses", "permissive_fallback_statuses",
          "diagnostic_classes", "seconds")}, indent=2))


if __name__ == "__main__":
    main()
