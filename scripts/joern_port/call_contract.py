#!/usr/bin/env python3
"""Freeze/compare bounded call AST contracts from unchanged Joern CPG captures.

This compares call population/identity, argument indices, receiver identity,
shared base/receiver roots and direct AST ownership. It is not full CPG or
execution parity. Original raw captures remain independently inspectable.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def project(method: dict) -> dict:
    graph = method["cpg"]
    nodes = {node["id"]: node for node in graph["nodes"]}
    if len(nodes) != len(graph["nodes"]):
        raise ValueError("duplicate CPG node identity")
    edges = graph["edges"]
    if any(e["source"] not in nodes or e["target"] not in nodes for e in edges):
        raise ValueError("unbounded CPG edge")

    def describe(node_id):
        node = nodes[node_id]
        return {key: node.get(key) or "" for key in ("kind", "code", "name")}

    calls = []
    for node in nodes.values():
        name = node.get("name", "")
        if (
            node["kind"] != "CALL"
            or name.startswith(("<operator>.", "<operators>."))
            and name != "<operator>.pointerCall"
        ):
            continue
        outgoing = [edge for edge in edges if edge["source"] == node["id"]]
        arguments = [e for e in outgoing if e["kind"] == "ARGUMENT"]
        indices = [int(e["label"]) for e in arguments]
        if len(indices) != len(set(indices)):
            raise ValueError("ambiguous call argument positions")
        receivers = [e["target"] for e in outgoing if e["kind"] == "RECEIVER"]
        if len(receivers) > 1:
            raise ValueError("ambiguous call receiver")
        children = [e["target"] for e in outgoing if e["kind"] == "AST"]
        if len(children) != len(set(children)):
            raise ValueError("duplicated direct AST child")
        if not set(receivers + [e["target"] for e in arguments]) <= set(children):
            raise ValueError("call role references a non-child")
        base = next((e["target"] for e in arguments if int(e["label"]) == 0), None)
        receiver = receivers[0] if receivers else None
        calls.append(
            {
                "code": node["code"],
                "name": name,
                "line": node["line"],
                "column": node["column"],
                "method_full_name": node.get("method_full_name") or "",
                "arguments": [
                    {"index": int(e["label"]), "node": describe(e["target"])}
                    for e in sorted(arguments, key=lambda e: int(e["label"]))
                ],
                "receiver": None if receiver is None else describe(receiver),
                "shared_base_receiver": receiver is not None and base == receiver,
                "ast_children": sorted(
                    (describe(n) for n in children),
                    key=lambda n: json.dumps(n, sort_keys=True),
                ),
            }
        )
    return {
        "fullname": method["fullname"],
        "line": method["start_line"],
        "calls": sorted(calls, key=lambda n: json.dumps(n, sort_keys=True)),
    }


def compare(reference: dict, analysis: dict) -> list[dict]:
    expected = reference["methods"]
    actual = [project(m) for m in analysis["functions"]]

    def keyed(rows):
        result = {}
        for row in rows:
            key = row["fullname"], row["line"]
            if key in result:
                raise ValueError("ambiguous method population")
            result[key] = row
        return result

    left, right = keyed(expected), keyed(actual)
    differences = []
    for key in sorted(left.keys() | right.keys()):
        if left.get(key) != right.get(key):
            differences.append(
                {
                    "method": list(key),
                    "original": left.get(key),
                    "candidate": right.get(key),
                }
            )
    if analysis.get("diagnostics"):
        differences.append({"diagnostics": analysis["diagnostics"]})
    return differences


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--log", type=Path, required=True)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    capture = (
        Path(__file__).resolve().parents[2] / "tests/fixtures/ddg-lambdas/capture.sc"
    )
    methods = [
        json.loads(text)
        for text in re.findall(
            r"FLOW_JSON_START\n(.*?)FLOW_JSON_END", args.log.read_text(), re.S
        )
    ]
    if not methods or any(
        Path(m["filename"]).name != args.source.name for m in methods
    ):
        raise ValueError("empty/wrong-source original capture")
    projected = [project(m) for m in methods]
    keys = [(m["fullname"], m["line"]) for m in projected]
    if len(keys) != len(set(keys)):
        raise ValueError("ambiguous original method population")
    result = {
        "schema": "joern-call-ast-contract-v1",
        "generator": "Original Joern v4.0.150 bundled with PyJoern 4.0.150.4",
        "source_sha256": digest(args.source.read_bytes()),
        "capture_sha256": digest(capture.read_bytes()),
        "raw_log_sha256": digest(args.log.read_bytes()),
        "projector_sha256": digest(Path(__file__).read_bytes()),
        "bounds": "Call identity, operand roles and AST ownership; excludes type inference, dispatch property, DDG and execution parity",
        "methods": sorted(projected, key=lambda m: (m["fullname"], m["line"])),
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                "methods": len(methods),
                "calls": sum(len(m["calls"]) for m in projected),
                "source_sha256": result["source_sha256"],
            }
        )
    )


if __name__ == "__main__":
    main()
