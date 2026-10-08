#!/usr/bin/env python3
"""Freeze original Joern method ASTs, CFGs and reaching-definition solutions."""
import argparse
import hashlib
import json
from pathlib import Path
import re


def normalize(method):
    ids = {node["id"]: index for index, node in enumerate(method["cpg"]["nodes"])}
    fields = {"id", "kind", "code", "name", "method_full_name", "type_name", "line", "column", "start_byte", "end_byte"}
    nodes = []
    for node in method["cpg"]["nodes"]:
        node = {key: value for key, value in node.items() if key in fields and (value != "" or key == "code")}
        node["id"] = ids[node["id"]]
        nodes.append(node)
    method["cpg"]["nodes"] = nodes
    for edge in method["cpg"]["edges"]:
        edge["source"], edge["target"] = ids[edge["source"]], ids[edge["target"]]
        if edge["kind"] not in {"ARGUMENT", "REACHING_DEF"}:
            edge.pop("label", None)
    for definition in method["reaching_definitions"]:
        definition["node"] = ids[definition["node"]]
        for key in ("incoming", "outgoing"):
            definition[key] = sorted(ids[node] for node in definition[key])
    method["reaching_definitions"].sort(key=lambda definition: definition["node"])
    for key in ("capture_nodes", "external_ddg_edges", "dot_ddg"):
        method.pop(key, None)
    return method


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--filename", required=True)
    parser.add_argument("--source", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    methods = [json.loads(text) for text in re.findall(r"FLOW_JSON_START\n(.*?)FLOW_JSON_END", args.log.read_text(), re.S)]
    methods = [normalize(method) for method in methods if method["filename"] == args.filename]
    if not methods:
        raise ValueError(f"No original methods captured for {args.filename}")
    fixture = Path(__file__).parent
    result = {
        "generator": "Original Joern 4.0.150 / dataflowOss and ReachingDefProblem",
        "source_sha256": hashlib.sha256((args.source or fixture / args.filename).read_bytes()).hexdigest(),
        "capture_sha256": hashlib.sha256((fixture / "capture.sc").read_bytes()).hexdigest(),
        "methods": methods,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
