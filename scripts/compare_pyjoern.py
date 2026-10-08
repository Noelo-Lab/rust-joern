#!/usr/bin/env python3
"""Compare CFG coverage and directed topology against unchanged PyJoern.

Use --reference for a frozen fixture, or --reference-python for a live Joern
baseline. Both backends receive the same prepared bytes. Non-isomorphism,
missing functions, degeneracy differences, and candidate diagnostics fail.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
CXX_SUFFIXES = {".cc", ".cpp", ".cxx", ".c++", ".C", ".ii"}
BLACKLIST = ("<", "+", "*", "(", ">", "JUMPOUT", "__builtin_unreachable")

# Running the reference in a separate interpreter and temporary cwd prevents
# the replacement Python package from shadowing the installed original.
REFERENCE_WORKER = r'''
import hashlib, json, pathlib, sys, time
config = json.load(sys.stdin)
if config.get("decbench"):
    sys.path.insert(0, config["decbench"])
source = pathlib.Path(config["source"])
if config["mode"] == "prepare":
    text = source.read_text(errors="replace")
    if source.suffix in (".i", ".ii") or config["sanitize"]:
        from decbench.utils.cfg import strip_system_headers, sanitize_decompiled_c, preprocess_decompiled_c
        if source.suffix in (".i", ".ii"):
            text = strip_system_headers(text)
        else:
            text = sanitize_decompiled_c(text)
            text = preprocess_decompiled_c(text)
    print(json.dumps({"text": text}))
else:
    import pyjoern
    from importlib.metadata import version
    started = time.perf_counter()
    parsed = pyjoern.parse_source(source, no_metadata=True, no_ddg=True, no_ast=True)
    elapsed = time.perf_counter() - started
    if parsed is None:
        raise RuntimeError("PyJoern returned no parse result")
    functions = {}
    for key, func in parsed.items():
        graph = func.cfg
        nodes = list(graph)
        index = {node: i for i, node in enumerate(nodes)}
        functions[func.name] = {"name": func.name, "start_line": func.start_line, "end_line": func.end_line, "cfg": {
            "nodes": list(range(len(nodes))),
            "edges": [[index[u], index[v]] for u, v in graph.edges],
            "entry": [index[n] for n in nodes if n.is_entrypoint],
            "exit": [index[n] for n in nodes if n.is_exitpoint],
            "degenerate": len(nodes) == 0 or len(nodes) == 1 and all(type(s).__name__ == "Nop" for n in nodes for s in n.statements),
            "labels": {str(index[n]): str(n) for n in nodes},
        }}
    print(json.dumps({"schema_version": 1, "generator": "pyjoern " + version("pyjoern") + " / Joern " + pyjoern.JOERN_VERSION,
        "reference_module": str(pathlib.Path(pyjoern.__file__).resolve()), "seconds": elapsed, "functions": functions}))
'''


def reference_worker(python: str, config: dict, cwd: Path) -> dict:
    environment = dict(os.environ)
    environment.pop("PYTHONPATH", None)
    result = subprocess.run(
        [python, "-c", REFERENCE_WORKER], input=json.dumps(config),
        text=True, capture_output=True, cwd=cwd, env=environment, check=True,
    )
    return json.loads(result.stdout)


def candidate_functions(analysis: dict) -> dict:
    functions = {}
    for function in analysis["functions"]:
        name = function["name"]
        cfg = function["cfg"]
        if not name or name.startswith(BLACKLIST) or not cfg["nodes"]:
            continue
        previous = functions.get(name)
        if previous and len(previous["cfg"]["nodes"]) > len(cfg["nodes"]):
            continue
        nops = {
            node["id"]: node.get("cfg_nop", node["kind"] in ("METHOD", "METHOD_RETURN", "METHOD_REF"))
            for node in function["cpg"]["nodes"]
        }
        normalized = {
            "nodes": [node["id"] for node in cfg["nodes"]],
            "edges": cfg["edges"],
            "entry": [node["id"] for node in cfg["nodes"] if node["is_entrypoint"]],
            "exit": [node["id"] for node in cfg["nodes"] if node["is_exitpoint"]],
            "degenerate": len(cfg["nodes"]) == 1 and all(
                nops[stmt]
                for node in cfg["nodes"] for stmt in node["statements"]
            ),
        }
        functions[name] = {"name": name, "cfg": normalized}
    return functions


def role_graph(cfg: dict):
    import networkx as nx

    graph = nx.DiGraph()
    entry, exit_ = set(cfg["entry"]), set(cfg["exit"])
    if not (entry | exit_) <= set(cfg["nodes"]):
        raise ValueError("CFG contains a role for an absent node")
    graph.add_nodes_from((node, {"role": (node in entry, node in exit_)}) for node in cfg["nodes"])
    for source, target in cfg["edges"]:
        if source not in graph or target not in graph:
            raise ValueError("CFG contains an edge to an absent node")
        graph.add_edge(source, target)
    return graph


def counts(cfg: dict) -> dict:
    return {"nodes": len(cfg["nodes"]), "edges": len(cfg["edges"]),
            "entries": len(cfg["entry"]), "exits": len(cfg["exit"]),
            "degenerate": cfg["degenerate"]}


def isomorphic(expected: dict, actual: dict) -> bool:
    from collections import Counter
    import networkx as nx

    def parts(cfg):
        nodes = set(cfg["nodes"])
        if len(nodes) != len(cfg["nodes"]):
            raise ValueError("CFG contains duplicate node IDs")
        entry, exit_ = set(cfg["entry"]), set(cfg["exit"])
        if not (entry | exit_) <= nodes:
            raise ValueError("CFG contains a role for an absent node")
        edges = {tuple(edge) for edge in cfg["edges"]}
        if any(len(edge) != 2 or not set(edge) <= nodes for edge in edges):
            raise ValueError("CFG contains an edge to an absent node")
        return nodes, edges, {n: (n in entry, n in exit_) for n in nodes}

    a, b = parts(expected), parts(actual)
    # Identity is a complete isomorphism proof; this keeps millions of identical
    # prototype CFGs from paying for NetworkX construction and VF2.
    if a == b:
        return True
    if len(a[0]) != len(b[0]) or len(a[1]) != len(b[1]):
        return False
    if Counter(a[2].values()) != Counter(b[2].values()):
        return False
    if len(a[0]) <= 1:
        return True
    try:
        import igraph
    except ImportError:
        igraph = None
    if igraph is not None:
        def compiled(parts):
            nodes, edges, roles = parts
            index = {node: i for i, node in enumerate(nodes)}
            graph = igraph.Graph(n=len(nodes),
                                 edges=[(index[s], index[t]) for s, t in edges], directed=True)
            return graph, [2 * roles[node][0] + roles[node][1] for node in nodes]
        left, left_roles = compiled(a)
        right, right_roles = compiled(b)
        return left.isomorphic_bliss(right, color1=left_roles, color2=right_roles)
    left, right = role_graph(expected), role_graph(actual)
    palette = {}
    colors = []
    for graph in (left, right):
        colors.append({n: palette.setdefault((graph.nodes[n]["role"], graph.in_degree(n), graph.out_degree(n)), len(palette)) for n in graph})
    for _ in range(max(len(a[0]), len(b[0]))):
        palette = {}
        updated = []
        for graph, old in zip((left, right), colors):
            updated.append({n: palette.setdefault((old[n], tuple(sorted(old[p] for p in graph.predecessors(n))), tuple(sorted(old[s] for s in graph.successors(n)))), len(palette)) for n in graph})
        stable = all(len(set(a.values())) == len(set(b.values())) for a, b in zip(colors, updated))
        colors = updated
        if Counter(colors[0].values()) != Counter(colors[1].values()):
            return False
        if stable:
            break
    for graph, coloring in zip((left, right), colors):
        nx.set_node_attributes(graph, coloring, "color")
    def components(graph, coloring):
        return Counter((len(part), tuple(sorted(Counter(coloring[n] for n in part).items())))
                       for part in nx.strongly_connected_components(graph))
    if components(left, colors[0]) != components(right, colors[1]):
        return False
    return nx.vf2pp_is_isomorphic(left, right, node_label="color")


def compare(reference: dict, candidate: dict, progress=None) -> list[dict]:
    rows = []
    for name in sorted(reference.keys() | candidate.keys()):
        row = {"function": name}
        if name not in candidate:
            row.update(status="missing", reference=counts(reference[name]["cfg"]),
                       reference_graph=reference[name]["cfg"])
        elif name not in reference:
            row.update(status="extra", candidate=counts(candidate[name]["cfg"]),
                       candidate_graph=candidate[name]["cfg"])
        else:
            expected, actual = reference[name]["cfg"], candidate[name]["cfg"]
            if progress and max(len(expected["nodes"]), len(actual["nodes"])) >= 20:
                progress(name)
            same = isomorphic(expected, actual)
            row.update(status="match" if same and expected["degenerate"] == actual["degenerate"] else "divergent", reference=counts(expected), candidate=counts(actual))
            if row["status"] != "match":
                row.update(reference_graph=expected, candidate_graph=actual)
        rows.append(row)
    return rows


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("sources", type=Path, nargs="+", help="C/C++ sources or DecBench preprocessed .i/.ii files")
    parser.add_argument("--candidate", type=Path, default=ROOT / "target/release/rust-joern")
    parser.add_argument("--reference-python", default=sys.executable, help="Python with the original PyJoern and optional DecBench installed")
    parser.add_argument("--reference", type=Path, help="Frozen reference JSON (requires one source)")
    parser.add_argument("--reference-dir", type=Path, help="Directory of STEM.pyjoern.json fixture baselines")
    parser.add_argument("--save-reference-dir", type=Path, help="Save freshly captured baseline JSON")
    parser.add_argument("--decbench", type=Path, help="DecBench checkout, for exactly its .i/.ii stripping and decompilation sanitization")
    parser.add_argument("--sanitize-decompiled", action="store_true", help="Apply DecBench's decompiled-side sanitation and local macro expansion")
    parser.add_argument("--output", type=Path, help="Write a full JSON comparison report")
    args = parser.parse_args()
    if args.reference and len(args.sources) != 1:
        parser.error("--reference requires exactly one source")
    if args.reference and args.reference_dir:
        parser.error("choose --reference or --reference-dir")
    report = {"schema_version": 1, "files": [], "matched": 0, "failed": 0}
    for source in args.sources:
        source = source.resolve()
        source_digest = hashlib.sha256(source.read_bytes()).hexdigest()
        reference_path = args.reference or (args.reference_dir / source.with_suffix(".pyjoern.json").name if args.reference_dir else None)
        try:
            with tempfile.TemporaryDirectory(prefix="rust-joern-compare-") as temp:
                cwd = Path(temp)
                config = {"source": str(source), "mode": "prepare", "sanitize": args.sanitize_decompiled, "decbench": str(args.decbench.resolve()) if args.decbench else None}
                prepared = reference_worker(args.reference_python, config, cwd)["text"]
                prepared_digest = hashlib.sha256(prepared.encode()).hexdigest()
                parse_path = cwd / ("input.cpp" if source.suffix in CXX_SUFFIXES else "input.c")
                parse_path.write_text(prepared)
                if reference_path:
                    baseline = json.loads(reference_path.read_text())
                    if source_digest not in {baseline["source_sha256"], baseline.get("original_source_sha256")}:
                        raise ValueError("Frozen reference source hash differs; recapture it explicitly")
                    if baseline.get("prepared_sha256", baseline["source_sha256"]) != prepared_digest:
                        raise ValueError("Frozen reference preparation differs; recapture it explicitly")
                else:
                    config.update(source=str(parse_path), mode="parse")
                    baseline = reference_worker(args.reference_python, config, cwd)
                    baseline.update(source_sha256=source_digest, prepared_sha256=prepared_digest)
                    if args.save_reference_dir:
                        args.save_reference_dir.mkdir(parents=True, exist_ok=True)
                        (args.save_reference_dir / source.with_suffix(".pyjoern.json").name).write_text(json.dumps(baseline, indent=2) + "\n")
                started = time.perf_counter()
                result = subprocess.run([str(args.candidate.resolve()), "analyze", str(parse_path)], text=True, capture_output=True, check=True)
                elapsed = time.perf_counter() - started
                analysis = json.loads(result.stdout)
                rows = compare(baseline["functions"], candidate_functions(analysis))
                diagnostics = analysis.get("diagnostics", [])
                if diagnostics:
                    for row in rows:
                        if row["status"] == "match":
                            row["status"] = "incomplete"
                matched = sum(row["status"] == "match" for row in rows)
                failed = len(rows) - matched
                if not rows or not baseline["functions"]:
                    failed += 1
                report["matched"] += matched
                report["failed"] += failed
                report["files"].append({"source": str(source), "source_sha256": source_digest, "prepared_sha256": prepared_digest,
                    "reference_generator": baseline["generator"], "reference_seconds": baseline.get("seconds"),
                    "candidate_seconds": elapsed, "diagnostics": diagnostics, "functions": rows,
                    "empty_reference": not bool(baseline["functions"])})
                print(f"{source.name}: {matched}/{len(rows)} matched, {failed} failures, candidate {elapsed:.4f}s")
                for row in rows:
                    if row["status"] != "match":
                        print(f"  {row['function']}: {row['status']} (reference={row.get('reference')}, candidate={row.get('candidate')})")
        except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
            detail = error.stderr if isinstance(error, subprocess.CalledProcessError) else str(error)
            report["files"].append({"source": str(source), "error": detail})
            report["failed"] += 1
            print(f"{source.name}: error: {detail}", file=sys.stderr)
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, indent=2) + "\n")
    print(f"Total: {report['matched']} matched, {report['failed']} failures")
    return 1 if report["failed"] else 0


if __name__ == "__main__":
    raise SystemExit(main())
