#!/usr/bin/env python3
"""Certify the three proven Joern export failures against immutable native output.

This produces a separate supplemental report. Original references, candidate
records, full-corpus summaries and ordinary comparator rules are never changed.
"""
from __future__ import annotations

import argparse
from collections import Counter
import copy
import gzip
import hashlib
import json
from pathlib import Path

from compare_pyjoern import compare

RECOVERED_DIGESTS = {
    "94bcb6707b30b8f36ba472a51f0dabdf3d7139363021abdfb009ae58adf21caa",
    "060a6f32b4ef8dd24af75de5a710ec761a881e28c1c831b2780f1c04b78e428b",
    "4a399fd5d8249bab0c5c7fbfe768598c26fc87a536c02d48db18bae072da720b",
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--recovery-manifest", type=Path, required=True)
    parser.add_argument("--direct", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.output.exists():
        parser.error("refusing to replace an established supplemental certification")
    recovery = json.loads(args.recovery_manifest.read_text())
    assert {item["prepared_sha256"] for item in recovery["cases"]} == RECOVERED_DIGESTS
    manifest_path = Path(recovery["manifest"]["path"])
    assert sha(manifest_path) == recovery["manifest"]["sha256"]
    manifest = json.loads(manifest_path.read_text())
    inventory = {case["id"]: case for case in manifest["cases"]}
    run = json.loads((args.direct / "run.json").read_text())
    summary_path = args.direct / "summary.json"
    summary = json.loads(summary_path.read_text())
    assert run["run_fingerprint"] == summary["run_fingerprint"]
    assert sha(Path(run["native_library"]["path"])) == run["native_library"]["sha256"]
    corrected = {key: copy.deepcopy(summary[key]) for key in
                 ("file_statuses", "function_statuses", "usage_statuses", "groups")}
    files = []
    failures = 0
    affected_cases = []
    unique_statuses = Counter()
    case_statuses = Counter()
    for item in recovery["cases"]:
        reference_path = Path(item["recovered_reference"]["path"])
        assert sha(reference_path) == item["recovered_reference"]["sha256"]
        reference = json.loads(reference_path.read_text())
        digest = item["prepared_sha256"]
        assert reference["prepared_sha256"] == digest
        assert sha(Path(item["prepared"]["path"])) == digest
        ids = item["case_ids"]
        assert set(ids) == {case["id"] for case in manifest["cases"]
                            if case.get("prepared_sha256") == digest}
        for case_id in ids:
            frozen_path = Path(inventory[case_id]["reference"])
            frozen = json.loads(frozen_path.read_text())
            assert frozen["functions"] == {} and frozen["prepared_sha256"] == digest
            expected = next(artifact for artifact in item["cached_references"]
                            if artifact["path"] == str(frozen_path.resolve()))
            assert sha(frozen_path) == expected["sha256"]
            assert sha(Path(frozen["reference_pickle"])) == frozen["reference_pickle_sha256"]
        candidate_path = args.direct / "inputs" / ("c-" + digest + ".json.gz")
        candidate = json.loads(gzip.decompress(candidate_path.read_bytes()))
        assert candidate["prepared_sha256"] == digest and set(candidate["case_ids"]) == set(ids)
        assert candidate["run_fingerprint"] == run["run_fingerprint"]
        assert candidate["native_library_sha256"] == run["native_library"]["sha256"]
        options = candidate["requested_options"]
        assert set(options) == {"strict", "data_flow", "reaching_definitions", "preprocessed", "language"}
        assert options["strict"] is True and options["preprocessed"] is True
        assert options["data_flow"] is False and options["reaching_definitions"] is False
        assert options["language"] == "c"
        rows = compare(reference["functions"], candidate["functions"])
        statuses = Counter(row["status"] for row in rows)
        strict_available = candidate["status"] == "ok" and not candidate["diagnostics"]
        qualified = strict_available and all(row["status"] == "match" for row in rows)
        failures += not qualified
        unique_statuses.update(statuses)
        case_statuses.update({key: value * len(ids) for key, value in statuses.items()})
        for case_id in ids:
            raw_file_status = "divergent" if strict_available else "usage_error"
            corrected_file_status = "match" if qualified else raw_file_status
            categories = [corrected]
            case = inventory[case_id]
            categories.extend(corrected["groups"][name] for name in
                              (case["optimization"] + "/" + case["kind"], "project/" + case["project"]))
            for category in categories:
                category["file_statuses"][raw_file_status] -= 1
                category["file_statuses"][corrected_file_status] = category["file_statuses"].get(corrected_file_status, 0) + 1
                # Direct triage function statistics retain fallback graphs separately
                # from file usage status; qualification above never accepts fallback.
                category["function_statuses"]["extra"] -= len(candidate["functions"])
                for status, count in statuses.items():
                    category["function_statuses"][status] = category["function_statuses"].get(status, 0) + count
                assert all(count >= 0 for count in category["file_statuses"].values())
                assert all(count >= 0 for count in category["function_statuses"].values())
            affected_cases.append(case_id)
        files.append({"case_ids": ids, "prepared_sha256": digest,
                      "supplemental_reference": item["recovered_reference"],
                      "candidate": {"path": str(candidate_path.resolve()), "sha256": sha(candidate_path)},
                      "native_library_sha256": candidate["native_library_sha256"],
                      "reference_functions": len(reference["functions"]),
                      "candidate_functions": len(candidate["functions"]),
                      "requested_options": options, "strict_available": strict_available,
                      "diagnostics": candidate["diagnostics"],
                      "qualified_status": "match" if qualified else "divergent" if strict_available else "usage_error",
                      "function_statuses": dict(statuses),
                      "divergences": [row for row in rows if row["status"] != "match"],
                      "candidate_timings": candidate["timings"],
                      "raw_cached_status": "divergent", "raw_cached_functions": 0,
                      "raw_cached_extra_functions": len(candidate["functions"])})
    assert len(files) == 3 and len(affected_cases) == 5
    assert set(affected_cases) == set(recovery["affected_case_ids"])
    report = {"schema_version": 1, "kind": "supplemental_original_exporter_recovery_certification",
              "recovery_manifest": {"path": str(args.recovery_manifest.resolve()), "sha256": sha(args.recovery_manifest)},
              "raw_corpus_summary": {"path": str(summary_path.resolve()), "sha256": sha(summary_path)},
              "run_fingerprint": run["run_fingerprint"], "native_library": run["native_library"],
              "source_build_fingerprint": summary["source_build_fingerprint"],
              "complete_corpus": summary["complete_corpus"], "corpus_cases": summary["recorded_cases"],
              "affected_case_ids": affected_cases, "qualified_failures": failures,
              "unique_input_function_statuses": dict(unique_statuses),
              "case_weighted_function_statuses": dict(case_statuses), "files": files,
              "raw_statuses": {key: summary[key] for key in ("file_statuses", "function_statuses", "usage_statuses", "groups")},
              "supplemental_corrected_statuses": corrected,
              "notes": ["This separately corrects exactly five proven original-exporter failures using unchanged original CPG CFG edges.",
                        "Raw frozen cache comparisons and ordinary matching rules remain unchanged.",
                        "All function names, directed topology, entry/exit roles and degeneracy must match; any diagnostic blocks qualification.",
                        "No other case receives a reference substitution or divergence tolerance."]}
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"output": str(args.output), "sha256": sha(args.output), "failures": failures,
                      "unique_input_function_statuses": dict(unique_statuses),
                      "case_weighted_function_statuses": dict(case_statuses),
                      "raw_file_statuses": summary["file_statuses"],
                      "supplemental_file_statuses": corrected["file_statuses"],
                      "raw_function_statuses": summary["function_statuses"],
                      "supplemental_function_statuses": corrected["function_statuses"]}, indent=2))
    return int(bool(failures))


if __name__ == "__main__":
    raise SystemExit(main())
