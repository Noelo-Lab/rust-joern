#!/usr/bin/env python3
"""Recheck the checked-in snapshot against the original Joern source and JVM.

Java is a reference-capture dependency only. Normal builds/regeneration use
the reviewed snapshot and have no JVM/CDT dependency.
"""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess

from generate_operators import REFERENCE


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument(
        "--joern", type=Path, required=True, help="Original joern-cli directory"
    )
    args = parser.parse_args()
    manifest = json.loads((REFERENCE / "manifest.json").read_text())
    revision = subprocess.check_output(
        ["git", "-C", str(args.source), "rev-parse", "HEAD"], text=True
    ).strip()
    if revision != manifest["commit"]:
        raise ValueError("wrong upstream commit")
    base = (
        args.source
        / "joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation"
    )
    for name, digest in manifest["files"].items():
        original = args.source / name if name == "LICENSE" else base / name
        if hashlib.sha256(original.read_bytes()).hexdigest() != digest:
            raise ValueError(f"upstream source mismatch: {name}")
    jars = args.joern.resolve() / "frontends/c2cpg/lib"
    for name, digest in manifest["constant_jars"].items():
        if hashlib.sha256((jars / name).read_bytes()).hexdigest() != digest:
            raise ValueError(f"reference JVM jar mismatch: {name}")
    exported = subprocess.check_output(
        [
            "java",
            "-cp",
            str(jars / "*"),
            str(Path(__file__).with_name("ExportConstants.java")),
            *manifest["constants"],
        ],
        text=True,
    )
    constants = {}
    for line in exported.splitlines():
        owner, name, value = line.split("\t")
        fields = constants.setdefault(owner, {})
        if name in fields:
            raise ValueError("duplicate reference constant")
        fields[name] = value
    if constants != manifest["constants"]:
        raise ValueError("original JVM constants differ from snapshot")
    print(
        json.dumps(
            {
                "upstream_commit": revision,
                "source_files": len(manifest["files"]),
                "jvm_constants": sum(map(len, constants.values())),
                "status": "verified",
            }
        )
    )


if __name__ == "__main__":
    main()
