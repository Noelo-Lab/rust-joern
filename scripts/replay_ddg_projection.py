#!/usr/bin/env python3
"""Verify the DDG against one original Joern run, independent of its run-to-run variation.

Joern 4.0.150's `dotDdg` depends on the stored order of each node's incoming
REACHING_DEF edges. ReachingDefPass creates those edges while iterating hash
collections keyed by node objects, whose hash includes the identity hash of the
Graph, so parts of that order differ between JVM runs of identical bytes. A
frozen DOT reference therefore cannot decide such a function. This tool
captures, in ONE unchanged original run (scripts/ddg_replay.sc), each method's
raw graph with its in-edge order and that run's `dotDdg`, then reports

  raw_equal     native REACHING_DEF edges of the same file equal Joern's as a
                labeled multiset under an AST-position node correspondence;
  cfg_order_equal  every node's CFG successors are in Joern's stored order,
                which fixes `reversePostOrder` and so the definition numbers;
  order_equal   the native projection visits destination nodes in the order of
                Joern's `visibleNodes`, on Joern's graph and on the native one;
  replay_equal  the native projection (src/ddg.rs), run on Joern's raw graph
                with Joern's in-edge order, reproduces that run's DOT: the same
                node ids and the same labeled directed edge multiset;
  in_order      how the native stored in-edge order relates to this run's, per
                target node: `same`; `interleaved` (the groups of edges made
                for different call sites are each in Joern's order, but Joern
                visited the call sites in another hash order); `hashed` (the
                differing edges come from a hash-ordered collection: the uses
                of a call with more than four arguments, or the definitions
                reaching a return's operand); `exit` (the exit or an output
                parameter, whose sources are all displayed, so that no
                projection reads their order); `unexpected` (anything else);
  native_equal  the native DOT projection of the native graph equals this
                run's DOT under the same correspondence. Informational: with
                raw_equal and replay_equal, a difference is due to the stored
                order alone.

A function passes when raw_equal, order_equal and replay_equal hold and no
target is `unexpected`. Native output never creates the capture, and `compat`
is not involved.
"""
from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import ctypes
from datetime import datetime, timezone
import hashlib
import html
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CXX_SUFFIXES = {".cpp", ".cc", ".cp", ".cxx", ".c++", ".C", ".hh", ".hpp", ".hxx", ".ii"}
JVM_OPTIONS = "-XX:ActiveProcessorCount=2 -XX:TieredStopAtLevel=1"
HIDDEN = ("CONTROL_STRUCTURE", "JUMP_TARGET")
GENERATOR = "pyjoern 4.0.150.4 / Joern v4.0.150"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def capture(source: Path, python: str, jvm_options: str, timeout: float) -> list[dict]:
    """Run scripts/ddg_replay.sc in unchanged original Joern; a provenance record, then one per method."""
    env = {k: v for k, v in os.environ.items() if k not in ("PYTHONPATH", "RUST_JOERN_LIBRARY", "JAVA_TOOL_OPTIONS")}
    if jvm_options:
        env["JAVA_TOOL_OPTIONS"] = jvm_options
    with tempfile.TemporaryDirectory(prefix="ddg-replay-") as temporary:
        cwd = Path(temporary)
        probe = ("import pyjoern; from importlib.metadata import version; "
                 "assert version('pyjoern') == '4.0.150.4' and pyjoern.JOERN_VERSION == 'v4.0.150'; "
                 "print(pyjoern.JOERN_SERVER_PATH)")
        joern = subprocess.run([python, "-I", "-c", probe], capture_output=True, text=True, cwd=cwd, env=env,
                               check=True).stdout.strip().splitlines()[-1]
        parse_path = cwd / ("input" + source.suffix)
        shutil.copyfile(source, parse_path)
        output = cwd / "capture.jsonl"
        process = subprocess.run([joern, "--script", str(ROOT / "scripts" / "ddg_replay.sc"),
                                  "--param", f"target_dir={parse_path}", "--param", f"out={output}"],
                                 capture_output=True, text=True, cwd=cwd, env=env, timeout=timeout)
        if process.returncode or not output.exists():
            raise RuntimeError(f"original Joern capture failed: {process.stdout[-2000:]} {process.stderr[-2000:]}")
        records = [json.loads(line) for line in output.read_text().splitlines() if line]
    script = (ROOT / "scripts" / "ddg_replay.sc").read_bytes()
    return [{"provenance": True, "generator": GENERATOR, "source_sha256": digest(source.read_bytes()),
             "script_sha256": digest(script), "jvm_options": jvm_options,
             "captured_at": datetime.now(timezone.utc).isoformat()}] + records


def load_library(path: str | None) -> ctypes.CDLL:
    candidates = [Path(path)] if path else [ROOT / "target" / profile / "librust_joern.so"
                                            for profile in ("release", "debug")]
    found = next((p for p in candidates if p.is_file()), None)
    if found is None:
        raise RuntimeError(f"native library not found: {', '.join(map(str, candidates))}")
    native = ctypes.CDLL(str(found.resolve()))
    native.rust_joern_analyze.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_char_p]
    native.rust_joern_analyze.restype = ctypes.c_void_p
    native.rust_joern_project_ddg.argtypes = [ctypes.c_char_p]
    native.rust_joern_project_ddg.restype = ctypes.c_void_p
    native.rust_joern_free.argtypes = [ctypes.c_void_p]
    native.rust_joern_free.restype = None
    return native


def result(native: ctypes.CDLL, pointer) -> dict:
    try:
        data = json.loads(ctypes.string_at(pointer))
    finally:
        native.rust_joern_free(pointer)
    if "error" in data:
        raise RuntimeError(data["error"])
    return data


def analyze(native: ctypes.CDLL, source: Path, language: str) -> dict:
    data = source.read_bytes()
    # DecBench's prepared-input mode, with Joern's behavior where the default differs.
    options = {"language": language, "data_flow": True, "preprocessed": True,
               "joern_loop_conditions": True, "joern_knr_parameters": True, "joern_cdt_failures": True}
    return result(native, native.rust_joern_analyze(ctypes.create_string_buffer(data), len(data),
                                                    os.fspath(source).encode(), json.dumps(options).encode()))


def function_graph(method: dict) -> tuple[dict, dict]:
    """A Joern capture as the native FunctionGraph, with dense ids; returns it and Joern id -> dense id."""
    kinds = {node["id"]: node["kind"] for node in method["nodes"]}
    children = defaultdict(list)
    for kind, source, target, _ in method["edges"]:
        if kind == "AST":
            children[source].append(target)
    # A native function graph holds one method: nested METHOD and TYPE_DECL
    # nodes stay as leaves, as in ContainsEdgePass. Nodes outside that tree
    # are the recorded outside sources of REACHING_DEF edges and their parents.
    root = method["nodes"][0]["id"]
    own = {node["id"] for node in method["nodes"][:method["own_nodes"]]}
    keep, stack = {root}, [root]
    while stack:
        for child in children[stack.pop()]:
            if child not in keep:
                keep.add(child)
                if kinds[child] not in ("METHOD", "TYPE_DECL"):
                    stack.append(child)
    keep |= {node["id"] for node in method["nodes"] if node["id"] not in own}
    reached, stack = set(keep), list(keep)
    incoming = defaultdict(list)
    for kind, source, target, _ in method["edges"]:
        if kind == "REACHING_DEF":
            incoming[target].append(source)
    while stack:
        for source in incoming[stack.pop()]:
            if source not in reached:
                reached.add(source)
                stack.append(source)
    ids = {node["id"]: index for index, node in enumerate(n for n in method["nodes"] if n["id"] in reached)}
    nodes = []
    for node in method["nodes"]:
        if node["id"] not in ids:
            continue
        record = {"id": ids[node["id"]], "kind": node["kind"], "code": node["code"] or "",
                  "line": node["line"], "column": node["column"], "start_byte": 0, "end_byte": 0}
        name = node["canonical_name"] if node["kind"] == "FIELD_IDENTIFIER" else node["name"]
        for key, value in (("name", name), ("method_full_name", node["method_full_name"]),
                           ("type_name", node["type_name"])):
            if value:
                record[key] = value
        nodes.append(record)
    edges = []
    for kind, source, target, label in method["edges"]:
        if source in ids and target in ids:
            edge = {"source": ids[source], "target": ids[target], "kind": kind}
            if kind in ("ARGUMENT", "REACHING_DEF") and label is not None:
                edge["label"] = label
            edges.append(edge)
    function = {"name": method["name"], "fullname": method["fullname"], "filename": method["filename"],
                "return_type": "", "signature": method["signature"], "start_line": method["start_line"],
                "end_line": method["end_line"], "cpg": {"nodes": nodes, "edges": edges},
                "cfg": {"nodes": [], "edges": []}}
    return function, ids


def positions(function: dict) -> dict:
    """Node id -> AST-position key shared by an original and a native graph of the same method.

    A node is located by its argument index under a CALL, otherwise by its rank
    among same-kind siblings, at every level up to the method. Only the node's
    own code joins the key, and not for METHOD and BLOCK nodes, whose code no
    REACHING_DEF label of a CFG node carries apart from a BLOCK argument's.
    """
    nodes = {node["id"]: node for node in function["cpg"]["nodes"]}
    children, parent, argument = defaultdict(list), {}, {}
    for edge in function["cpg"]["edges"]:
        if edge["kind"] == "AST":
            children[edge["source"]].append(edge["target"])
            parent.setdefault(edge["target"], edge["source"])
        elif edge["kind"] == "ARGUMENT":
            argument.setdefault((edge["source"], edge["target"]), edge.get("label"))
    paths = {}

    def path(at):
        if at in paths:
            return paths[at]
        node = nodes[at]
        above = parent.get(at)
        if above is None:
            paths[at] = ("root",) if node["kind"] == "METHOD" else ("outside", node["line"], node["column"])
        elif nodes[above]["kind"] == "CALL" and (above, at) in argument:
            paths[at] = (path(above), node["kind"], "argument", argument[above, at])
        else:
            same = [child for child in children[above] if nodes[child]["kind"] == node["kind"]]
            paths[at] = (path(above), node["kind"], "child", same.index(at))
        return paths[at]

    sys.setrecursionlimit(max(sys.getrecursionlimit(), 20000))
    return {at: (path(at), "" if node["kind"] in ("METHOD", "BLOCK") else node["code"]) for at, node in nodes.items()}


def raw_edges(function: dict) -> Counter:
    keys = positions(function)
    return Counter((keys[e["source"]], keys[e["target"]], e.get("label") or "")
                   for e in function["cpg"]["edges"] if e["kind"] == "REACHING_DEF")


def successors(function: dict) -> dict:
    """Per source position, its distinct CFG successors in stored out-edge order.

    `Method.reversePostOrder` expands successors in this order, and its result
    numbers the definitions, so it must agree for the stored order to agree.
    Joern stores a goto's edge to its label twice; the repeat changes nothing.
    """
    keys = positions(function)
    result = defaultdict(list)
    for edge in function["cpg"]["edges"]:
        if edge["kind"] == "CFG" and keys[edge["target"]][0] not in result[keys[edge["source"]][0]]:
            result[keys[edge["source"]][0]].append(keys[edge["target"]][0])
    return result


def targets(function: dict) -> set:
    return {edge["target"] for edge in function["cpg"]["edges"] if edge["kind"] == "REACHING_DEF"}


def stored_order(function: dict) -> tuple[dict, dict]:
    """Per target key, its incoming REACHING_DEF edges in stored order as (group, source key, label, sibling).

    `own` edges come from the target's own arguments, `entry` edges from the
    method, and `site` edges from the call, return or parameter using the
    target; `sibling` marks a source that is another argument of that call.
    Also returns what decides whether a group is hash-ordered in Joern.
    """
    keys = positions(function)
    nodes = {node["id"]: node for node in function["cpg"]["nodes"]}
    parent, arguments = {}, Counter()
    for edge in function["cpg"]["edges"]:
        if edge["kind"] == "ARGUMENT":
            parent.setdefault(edge["target"], edge["source"])
            arguments[edge["source"]] += 1
    order, facts = defaultdict(list), {}
    for edge in function["cpg"]["edges"]:
        if edge["kind"] != "REACHING_DEF":
            continue
        source, target = edge["source"], edge["target"]
        above = parent.get(target)
        group = "entry" if nodes[source]["kind"] == "METHOD" else "own" if parent.get(source) == target else "site"
        order[keys[target]].append((group, keys[source], edge.get("label") or "",
                                    above is not None and parent.get(source) == above))
        facts[keys[target]] = {
            "exit": nodes[target]["kind"] in ("METHOD_RETURN", "METHOD_PARAMETER_OUT"),
            "own": arguments[target] > 4,
            "return": above is not None and nodes[above]["kind"] == "RETURN",
            "site": above is not None and arguments[above] > 4}
    return order, facts


def in_order(expected: dict, actual: dict) -> Counter:
    """Classify each target by how the native stored in-edge order relates to the original's."""
    (left, facts), (right, _) = stored_order(expected), stored_order(actual)
    result = Counter()
    for target, edges in left.items():
        other = right.get(target, [])
        if edges == other:
            result["same"] += 1
            continue

        def group(name, siblings=True):
            return [[e for e in side if e[0] == name and (siblings or not e[3])] for side in (edges, other)]
        differing = [name for name in ("entry", "own", "site") if group(name)[0] != group(name)[1]]
        fact = facts[target]
        # `call.argument.toSet` is a HashSet beyond four elements, which orders
        # a call's argument-to-argument and argument-to-call edges; a return
        # collects the definitions of its operand into a hash set. The used
        # incoming definitions of a call's argument stay in number order.
        hashed = {"entry": False, "own": fact["own"],
                  "site": fact["return"] or fact["site"] and group("site", False)[0] == group("site", False)[1]}
        if fact["exit"]:
            result["exit"] += 1
        elif not differing:
            result["interleaved"] += 1
        elif all(hashed[name] for name in differing):
            result["hashed"] += 1
        else:
            result["unexpected"] += 1
    return result


def dot_parts(dot: str, graphviz: bool = True) -> tuple[set, Counter]:
    """Node ids and labeled edge multiset of a `dotDdg` string.

    Parsed with pygraphviz, as the reference pipeline does; without it, by the
    line grammar of Joern's DotSerializer, whose labels are HTML-escaped.
    """
    if graphviz:
        try:
            import networkx as nx
            import pygraphviz
        except ImportError:
            graphviz = False
    if graphviz:
        graph = nx.nx_agraph.from_agraph(pygraphviz.AGraph(dot))
        return ({int(node) for node in graph},
                Counter((int(u), int(v), html.unescape(d.get("label", ""))) for u, v, d in graph.edges(data=True)))
    nodes = {int(node) for node in re.findall(r'^"([0-9]+)" \[label = <', dot, re.M)}
    edges = Counter((int(u), int(v), html.unescape(label or "")) for u, v, label in re.findall(
        r'^  "([0-9]+)" -> "([0-9]+)" (?: \[ label = "([^"]*)"\] )?$', dot, re.M))
    return nodes, edges


def describe(nodes: dict, node) -> str:
    record = nodes.get(node)
    return f"{record['kind']}:{record['code'][:60]!r}@{record['line']}" if record else f"?{node}"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("source", type=Path, help="prepared C/C++ input file")
    parser.add_argument("--reference-python", help="interpreter with unchanged PyJoern 4.0.150.4")
    parser.add_argument("--capture", type=Path, help="reuse a saved original capture instead of running Joern")
    parser.add_argument("--save-capture", type=Path, help="save this run's original capture (JSON lines)")
    parser.add_argument("--library", help="native library (default: RUST_JOERN_LIBRARY, then target/)")
    parser.add_argument("--language", choices=("c", "cpp"), help="default: by file suffix")
    parser.add_argument("--function", action="append", default=[], help="restrict the report to this method name")
    parser.add_argument("--output", type=Path, help="write the full JSON report here")
    parser.add_argument("--jvm-options", default=JVM_OPTIONS)
    parser.add_argument("--timeout", type=float, default=3600)
    parser.add_argument("--limit", type=int, default=4, help="edge differences printed per failing function")
    parser.add_argument("--verbose", action="store_true", help="print every function, not only failures")
    args = parser.parse_args()

    source = args.source.resolve()
    language = args.language or ("cpp" if source.suffix in CXX_SUFFIXES else "c")
    if args.capture:
        records = [json.loads(line) for line in args.capture.read_text().splitlines() if line]
    elif args.reference_python:
        records = capture(source, args.reference_python, args.jvm_options, args.timeout)
        if args.save_capture:
            args.save_capture.write_text("".join(json.dumps(record) + "\n" for record in records))
    else:
        parser.error("one of --reference-python or --capture is required")
    provenance = next((record for record in records if record.get("provenance")), None)
    if (provenance is None or provenance["generator"] != GENERATOR
            or provenance["source_sha256"] != digest(source.read_bytes())):
        raise SystemExit("The capture is not an original capture of these source bytes")
    context = next(record for record in records if record.get("context"))
    methods = [record for record in records if "nodes" in record]
    native = load_library(args.library or os.environ.get("RUST_JOERN_LIBRARY"))
    analysis = analyze(native, source, language)
    candidates = defaultdict(list)
    for function in analysis["functions"]:
        candidates[function["name"]].append(function)

    def project(function: dict) -> dict:
        request = {"function": function, "nonstub_methods": context["nonstub_methods"],
                   "internal_methods": context["internal_methods"], "cpp": language == "cpp"}
        return result(native, native.rust_joern_project_ddg(json.dumps(request).encode()))

    rows, totals, matched = [], Counter(), set()
    for method in methods:
        if args.function and method["name"] not in args.function:
            continue
        row = {"name": method["name"], "fullname": method["fullname"], "start_line": method["start_line"]}
        function, ids = function_graph(method)
        back = {dense: original for original, dense in ids.items()}
        records_by_id = {node["id"]: node for node in method["nodes"]}
        keys = positions(function)

        projected = project(function)
        expected_order = [node for node in method["order"] if records_by_id[node]["kind"] not in HIDDEN]
        row["order_equal"] = [back[node] for node in projected["order"]] == expected_order
        dot = method["dot_ddg"][0] if method["dot_ddg"] else None
        expected_nodes, expected_edges = dot_parts(dot) if dot else (set(), Counter())
        actual_nodes = {back[node["id"]] for node in projected["nodes"]}
        actual_edges = Counter((back[e["source"]], back[e["target"]], e.get("label") or "")
                               for e in projected["edges"])
        row["replay_equal"] = expected_nodes == actual_nodes and expected_edges == actual_edges
        row["dot"] = {"nodes": len(expected_nodes), "edges": sum(expected_edges.values())}
        if not row["replay_equal"]:
            def text(edge):
                return [describe(records_by_id, edge[0]), describe(records_by_id, edge[1]), edge[2][:80]]
            row["replay_diff"] = {
                "missing_nodes": [describe(records_by_id, n) for n in sorted(expected_nodes - actual_nodes)],
                "extra_nodes": [describe(records_by_id, n) for n in sorted(actual_nodes - expected_nodes)],
                "missing_edges": [text(e) for e in sorted((expected_edges - actual_edges).elements())],
                "extra_edges": [text(e) for e in sorted((actual_edges - expected_edges).elements())]}

        # The native function of this method: same name and line, by full
        # name among overloads declared on one line, else the only namesake.
        same_name = [f for f in candidates.get(method["name"], []) if id(f) not in matched]
        same_line = [f for f in same_name if f["start_line"] == method["start_line"]]
        actual = next((f for f in same_line if f["fullname"] == method["fullname"]),
                      same_line[0] if same_line else same_name[0] if len(same_name) == 1 else None)
        matched.add(id(actual))
        if actual is None:
            # PyJoern's public API drops synthetic methods such as <global>,
            # and the native analysis follows it: only the replay applies.
            row["raw_equal"] = None if method["name"].startswith("<") else False
            row["native"] = "absent"
        else:
            expected_raw, actual_raw = raw_edges(function), raw_edges(actual)
            row["raw_equal"] = expected_raw == actual_raw
            if not row["raw_equal"]:
                row["raw_diff"] = {"only_original": sum((expected_raw - actual_raw).values()),
                                   "only_native": sum((actual_raw - expected_raw).values())}
            else:
                row["in_order"] = dict(in_order(function, actual))
            row["cfg_order_equal"] = successors(function) == successors(actual)
            # Only a destination with incoming edges affects the edge cache,
            # so the native graph's order is compared on those nodes.
            actual_keys = positions(actual)
            row["order_equal"] = row["order_equal"] and (
                [actual_keys[node][0] for node in project(actual)["order"] if node in targets(actual)]
                == [keys[ids[node]][0] for node in expected_order if ids[node] in targets(function)])
            view = actual.get("ddg_view") or {"nodes": [], "edges": []}
            row["native_equal"] = (
                {keys[ids[node]] for node in expected_nodes} == {actual_keys[node["id"]] for node in view["nodes"]}
                and Counter((keys[ids[u]], keys[ids[v]], label) for u, v, label in expected_edges.elements())
                == Counter((actual_keys[e["source"]], actual_keys[e["target"]], e.get("label") or "")
                           for e in view["edges"]))
        row["pass"] = (row["raw_equal"] is not False and row["order_equal"] and row["replay_equal"]
                       and not row.get("in_order", {}).get("unexpected"))
        rows.append(row)
        for key in ("raw_equal", "cfg_order_equal", "order_equal", "replay_equal", "native_equal", "pass"):
            if key in row:
                totals[f"{key}={row[key]}"] += 1
        for key, count in row.get("in_order", {}).items():
            totals[f"in_order.{key}"] += count
        if not row["pass"] or args.function or args.verbose:
            print(f"{row['name']}@{row['start_line']}: raw_equal={row['raw_equal']} "
                  f"cfg_order_equal={row.get('cfg_order_equal')} order_equal={row['order_equal']} "
                  f"replay_equal={row['replay_equal']} native_equal={row.get('native_equal')} "
                  f"in_order={row.get('in_order')}")
            for key in ("missing_edges", "extra_edges"):
                for edge in row.get("replay_diff", {}).get(key, [])[:args.limit]:
                    print(f"    {key[:-6]}: {edge[0]} -> {edge[1]} [{edge[2]}]")
    summary = {"source": str(source), "source_sha256": provenance["source_sha256"], "language": language,
               "generator": provenance["generator"], "graph_hash": context.get("graph_hash"),
               "methods": len(rows), **dict(sorted(totals.items()))}
    print(json.dumps(summary))
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps({"summary": summary, "functions": rows}, indent=1))
    return 0 if all(row["pass"] for row in rows) else 1


if __name__ == "__main__":
    raise SystemExit(main())
