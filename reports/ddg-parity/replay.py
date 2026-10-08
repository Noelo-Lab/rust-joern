#!/usr/bin/env python3
"""Replay every frozen original corpus group against one candidate snapshot."""
from __future__ import annotations

import argparse
from collections import Counter
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
REPORTS = ROOT / "reports/ddg-parity"


def read_json(path):
    if path.suffix == ".gz":
        with gzip.open(path, "rt") as stream:
            return json.load(stream)
    return json.loads(path.read_text())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-library", type=Path, default=ROOT / "target/debug/librust_joern.so")
    parser.add_argument("--candidate-package", type=Path, default=ROOT / "python")
    parser.add_argument("--candidate-python", default=sys.executable)
    parser.add_argument("--reference-python", default=sys.executable,
                        help="Python with original DecBench dependencies for external input preparation")
    parser.add_argument("--jobs", type=int, default=4)
    args = parser.parse_args()
    manifest_path = REPORTS / "corpus-manifest.json"
    manifest = read_json(manifest_path)
    for group in manifest["groups"]:
        for entry in group["inputs"]:
            reference = ROOT / entry["reference"]
            if hashlib.sha256(reference.read_bytes()).hexdigest() != entry["reference_sha256"]:
                raise ValueError(f"Frozen original reference changed: {reference}")

    checkpoint = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    groups = []
    before_revisions = set()
    with tempfile.TemporaryDirectory(prefix="ddg-final-replay-") as temporary:
        snapshot = Path(temporary)
        library = snapshot / args.candidate_library.name
        package = snapshot / "python"
        shutil.copyfile(args.candidate_library, library)
        shutil.copytree(args.candidate_package, package, ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.so"))
        library_sha = hashlib.sha256(library.read_bytes()).hexdigest()
        python_sha = {str(p.relative_to(package)): hashlib.sha256(p.read_bytes()).hexdigest()
                      for p in sorted(package.rglob("*.py"))}
        for spec in manifest["groups"]:
            command = [args.candidate_python, str(ROOT / "scripts/compare_ddg_pyjoern.py"),
                       *[str(ROOT / entry["source"]) for entry in spec["inputs"]],
                       "--reference-dir", str(ROOT / spec["reference_dir"]),
                       "--candidate-library", str(library), "--candidate-package", str(package),
                       "--candidate-python", args.candidate_python, "--reference-python", args.reference_python,
                       "--output", str(REPORTS / spec["report"]), "--jobs", str(args.jobs)]
            if spec.get("decbench"):
                command += ["--decbench", spec["decbench"]]
            if spec.get("sanitize_decompiled"):
                command += ["--sanitize-decompiled"]
            completed = subprocess.run(command, cwd=ROOT)
            if completed.returncode not in (0, 1):
                raise RuntimeError(f"Comparison command failed for {spec['name']}")
            report = read_json(REPORTS / spec["report"])
            if report["candidate_library_sha256"] != library_sha or report["candidate_python_sha256"] != python_sha:
                raise ValueError(f"Mixed candidate revisions in {spec['name']}")
            before = read_json(REPORTS / spec["before_report"]) if spec.get("before_report") else None
            if before:
                before_revisions.add((before["candidate_library_sha256"],
                                      json.dumps(before["candidate_python_sha256"], sort_keys=True)))
            errors = [{"source": row["source"], "status": row["status"], "error": row.get("error", "")}
                      for row in report["files"] if row["status"] not in ("match", "divergent")]
            mismatches = Counter(row.get("mismatch_kind", row["status"])
                                 for file in report["files"] for row in file.get("functions", [])
                                 if row["status"] != "match")
            expected = {str(ROOT / entry["source"]): entry for entry in spec["inputs"]}
            if len(report["files"]) != len(expected):
                raise ValueError(f"Incomplete report for {spec['name']}")
            for file in report["files"]:
                entry = expected[file["source"]]
                if file["status"] not in ("preparation_error",) and (file.get("source_sha256") != entry["source_sha256"] or file.get("prepared_sha256") != entry["prepared_sha256"]):
                    raise ValueError(f"Input hash changed for {entry['source']}")
            groups.append({"name": spec["name"], "before_report": spec.get("before_report"), "after_report": spec["report"],
                           "requested_inputs": len(spec["inputs"]), "completed_inputs": report["completed_files"],
                           "input_kind": spec["inputs"][0]["input_kind"],
                           "original_public_functions": spec["original_public_functions"],
                           "before_matched": sum(f.get("matched", 0) for f in before["files"]) if before else None,
                           "before_errors": [{"source": f["source"], "status": f["status"], "error": f.get("error", "")}
                                             for f in before["files"] if f["status"] not in ("match", "divergent")] if before else [],
                           "after_matched": sum(f.get("matched", 0) for f in report["files"]),
                           "after_failed": sum(f.get("failed", 0) for f in report["files"]),
                           "errors": errors, "mismatch_kinds": dict(mismatches),
                           "candidate_library_sha256": library_sha, "candidate_python_sha256": python_sha,
                           "oracle_public_parse_seconds": sum(f.get("reference_timings", {}).get("public_parse_source_seconds", 0) for f in report["files"]),
                           "oracle_supplemental_parse_seconds": sum(f.get("reference_timings", {}).get("supplemental_raw_parse_seconds", 0) for f in report["files"]),
                           "native_public_ddg_seconds": sum(f.get("candidate_seconds", 0) for f in report["files"]),
                           "candidate_timing_kind": report["candidate_timing_kind"]})
    before_library_sha, before_python_sha = next(iter(before_revisions)) if len(before_revisions) == 1 else (None, None)
    summary = {"schema_version": 1, "scope": manifest["scope"],
               "graph_gates": ["attributed directed original public Function.ddg isomorphism",
                               "directed labeled original dotDdg multigraph isomorphism"],
               "manifest": "corpus-manifest.json", "manifest_sha256": hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
               "repository_checkpoint": checkpoint, "candidate_library_source": str(args.candidate_library.resolve()),
               "candidate_package_source": str(args.candidate_package.resolve()),
               "candidate_library_sha256": library_sha, "candidate_python_sha256": python_sha,
               "before_all_groups_same_candidate_revision": len(before_revisions) == 1,
               "before_candidate_library_sha256": before_library_sha,
               "before_candidate_python_sha256": json.loads(before_python_sha) if before_python_sha else None,
               "source_hash_binding": "Every report input must equal the manifest source and prepared SHA-256 hashes; original reference bytes are also hash-bound.",
               "file_inputs": manifest["file_inputs"], "directory_inputs": manifest["directory_inputs"],
               "original_functions": manifest["original_public_function_instances"],
               "unique_prepared_input_contexts": manifest["unique_prepared_input_contexts"],
               "original_functions_in_unique_prepared_contexts": manifest["original_functions_in_unique_prepared_contexts"],
               "duplicate_prepared_input_contexts": manifest["duplicate_prepared_input_contexts"],
               "groups": groups, "all_groups_same_candidate_revision": True,
               "before_matched": sum(g["before_matched"] or 0 for g in groups),
               "matched": sum(g["after_matched"] for g in groups), "failed": sum(g["after_failed"] for g in groups),
               "all_graph_comparisons_completed": all(not g["errors"] and g["requested_inputs"] == g["completed_inputs"]
                                                       and g["after_matched"] + g["after_failed"] >= g["original_public_functions"] for g in groups)}
    (REPORTS / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(f"Final corpus: {summary['matched']}/{summary['original_functions']} exact, {summary['failed']} divergent", flush=True)
    return int(bool(summary["failed"]) or not summary["all_graph_comparisons_completed"])


if __name__ == "__main__":
    raise SystemExit(main())
