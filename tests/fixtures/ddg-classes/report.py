#!/usr/bin/env python3
"""Record ordered original methods and PyJoern's actual constructor selection."""
import argparse
import hashlib
from importlib.metadata import version
import inspect
import json
from pathlib import Path
import re

from pyjoern.parsing.function import Function


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    methods = [json.loads(s) for s in re.findall(
        r"CLASS_JSON_START\n(.*?)\nCLASS_JSON_END", args.log.read_text(), re.S)]
    if not methods:
        raise ValueError("No original methods in capture")
    fields = ("name", "return_type", "fullname", "filename", "start_line", "end_line", "signature", "cfg", "ddg")
    raw = [{key: method[key] for key in fields} for method in methods]
    functions = [Function(**method) for method in raw]
    selected = Function.from_many(raw)
    for method, function in zip(methods, functions):
        winner = selected.get((function.name, str(function.filename)))
        method["normalized_cfg_nodes"] = len(function.cfg) if function.cfg is not None else 0
        method["normalized_ddg_nodes"] = len(function.ddg) if function.ddg is not None else 0
        method["selected_by_from_many"] = bool(winner and winner.fullname == function.fullname
                                               and winner.start_line == function.start_line)
    result = {
        "generator": "Unchanged Joern 4.0.150 / PyJoern Function.from_many",
        "joern_tag_commit": "958fdd3d976197a783f8ade1254b43e977648f28",
        "package_versions": {name: version(name) for name in ("pyjoern", "networkx")},
        "source_filename": args.source.name,
        "source_sha256": sha256(args.source),
        "capture_sha256": sha256(Path(__file__).with_name("capture.sc")),
        "selector_sha256": sha256(Path(inspect.getfile(Function))),
        "methods": methods,
    }
    args.output.write_text(json.dumps(result, indent=2) + "\n")
    for method in methods:
        print(f"{'selected' if method['selected_by_from_many'] else '        '} "
              f"{method['method_index']:2} {method['name']:24} line={method['start_line']:3} "
              f"CFG={method['normalized_cfg_nodes']} DDG={method['normalized_ddg_nodes']} "
              f"{method['fullname']}")


if __name__ == "__main__":
    main()
