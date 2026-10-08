"""In-process C/C++ graphs from the Rust Joern port.

Build the native library with ``cargo build --release`` before using this package.
``parse_code`` keeps all graph data in memory; ``parse_source`` implements the
PyJoern entry point used by DecBench.
"""

from __future__ import annotations

import ctypes
import json
import os
import re
import sys
from collections import defaultdict
from copy import deepcopy
from dataclasses import dataclass
from functools import cached_property, lru_cache
from html.entities import codepoint2name
from pathlib import Path

import networkx as nx

__version__ = "0.1.0"
__all__ = [
    "Analysis", "Assignment", "BinOp", "Block", "Call", "Compare", "Function", "Nop",
    "Parameter", "Return", "Statement", "Ternary", "UnsupportedStmt", "parse_code", "parse_source",
]


class _NativeSource(ctypes.Structure):
    _fields_ = [
        ("source", ctypes.c_void_p),
        ("source_len", ctypes.c_size_t),
        ("filename", ctypes.c_char_p),
    ]


@lru_cache(maxsize=None)
def _library(override: str | None) -> ctypes.CDLL:
    library_name = {"darwin": "librust_joern.dylib", "win32": "rust_joern.dll"}.get(
        sys.platform, "librust_joern.so"
    )
    if override:
        candidates = [Path(override)]
    else:
        package = Path(__file__).resolve().parent
        candidates = [package / library_name]
        for root in (package.parent.parent, Path.cwd()):
            candidates.extend(root / "target" / profile / library_name for profile in ("release", "debug"))
    library_path = next((path for path in candidates if path.is_file()), None)
    if library_path is None:
        raise RuntimeError(
            "Rust Joern native library not found. Run `cargo build --release` in "
            "the rust-joern checkout, or set RUST_JOERN_LIBRARY to the built library. "
            f"Searched: {', '.join(map(str, candidates))}"
        )
    native = ctypes.CDLL(str(library_path.resolve()))
    native.rust_joern_analyze.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_char_p, ctypes.c_char_p]
    native.rust_joern_analyze.restype = ctypes.c_void_p
    analyze_many = getattr(native, "rust_joern_analyze_many", None)
    if analyze_many is not None:
        analyze_many.argtypes = [ctypes.POINTER(_NativeSource), ctypes.c_size_t, ctypes.c_char_p]
        analyze_many.restype = ctypes.c_void_p
    native.rust_joern_free.argtypes = [ctypes.c_void_p]
    native.rust_joern_free.restype = None
    return native


@dataclass(frozen=True, slots=True)
class Statement:
    id: int
    kind: str
    raw_text: str | list[str]
    source_line_number: int | None

    def __str__(self) -> str:
        return self.raw_text

    def copy(self):
        return deepcopy(self)


class Nop(Statement):
    """Method boundary marker; DecBench checks this class name for degeneracy."""

    __slots__ = ()
    FUNC_START = 0
    FUNC_END = 1
    NOP = 2

    @property
    def type(self) -> int:
        if self.kind == "NOP":
            return self.NOP
        return self.FUNC_END if self.kind == "METHOD_RETURN" else self.FUNC_START

    def __str__(self) -> str:
        return {self.FUNC_START: "FUNCTION_START", self.FUNC_END: "FUNCTION_END", self.NOP: "NOP"}[self.type]


@dataclass(frozen=True, slots=True)
class Assignment(Statement):
    src: str
    dst: str

    def __str__(self) -> str:
        return f"{self.src} = {self.dst}"


@dataclass(frozen=True, slots=True)
class Return(Statement):
    ret: str

    def __str__(self) -> str:
        return f"return {self.ret}"


@dataclass(frozen=True, slots=True)
class Call(Statement):
    func: str
    args: list[str]

    def __str__(self) -> str:
        return f"{self.func}({''.join(self.args)})"


@dataclass(frozen=True, slots=True)
class Compare(Statement):
    EQ, GTE, GT, LTE, LT, NE = range(6)
    type: int
    arg1: str
    arg2: str

    def __str__(self) -> str:
        separator = {self.EQ: "==", self.GTE: ">=", self.GT: ">", self.LTE: "<=", self.LT: "<", self.NE: "!="}[self.type]
        return f"{self.arg1} {separator} {self.arg2}"


@dataclass(frozen=True, slots=True)
class BinOp(Statement):
    SUB, ADD, AND, OR = range(4)
    type: int
    arg1: str
    arg2: str

    def __str__(self) -> str:
        separator = {self.SUB: "-", self.ADD: "+", self.AND: "&&", self.OR: "||"}[self.type]
        return f"{self.arg1} {separator} {self.arg2}"


@dataclass(frozen=True, slots=True)
class Ternary(Statement):
    cond: str
    true: str
    false: str

    def __str__(self) -> str:
        return f"{self.cond} ? {self.true} : {self.false}"


@dataclass(frozen=True, slots=True)
class Parameter(Statement):
    name: str
    type: str

    def __str__(self) -> str:
        return f"{self.type} {self.name}"


class UnsupportedStmt(Statement):
    __slots__ = ()

    def __str__(self) -> str:
        return f"<UnsupportedStmt: {self.raw_text}>"

    def __repr__(self) -> str:
        return str(self)


@dataclass(eq=False, slots=True)
class Block:
    id: int | None
    statements: tuple[Statement, ...]
    is_entrypoint: bool
    is_exitpoint: bool
    idx: int | None = None

    @property
    def addr(self) -> int | None:
        return self.id

    def __str__(self) -> str:
        if self.idx is not None:
            return f"{self.addr}.{self.idx}:\n" + "".join(f"{statement}\n" for statement in self.statements)
        return f"{self.id}:\n" + "\n".join(map(str, self.statements))

    def __repr__(self) -> str:
        if self.idx is not None:
            return f"<Block: {self.addr}.{self.idx}, {len(self.statements)} statements>"
        return f"<Block {self.id}: {len(self.statements)} statements>"

    def contains_addr(self, addr: int) -> bool:
        return self.addr == addr or any(statement.source_line_number == addr for statement in self.statements)


_HTML4_ESCAPE = {code: f"&{name};" for code, name in codepoint2name.items()}
_MEMBER_ACCESS_NAMES = frozenset(
    f"<operator>.{name}" for name in (
        "memberAccess", "indirectComputedMemberAccess", "indirectMemberAccess", "computedMemberAccess",
        "indirection", "addressOf", "fieldAccess", "indirectFieldAccess", "indexAccess",
        "indirectIndexAccess", "pointerShift", "getElementPtr",
    )
)
_EXPRESSION_KINDS = frozenset((
    "CALL", "RETURN", "BLOCK", "CONTROL_STRUCTURE", "IDENTIFIER", "LITERAL", "METHOD_REF", "FIELD_IDENTIFIER", "UNKNOWN",
    "TYPE_REF", "ANNOTATION", "ARRAY_INITIALIZER",
))


def _ddg_limit(code: str) -> str:
    # Joern 4.0.150 truncates at 50 Java UTF-16 code units before HTML escaping.
    encoded = code.encode("utf-16-le", errors="surrogatepass")
    return code if len(encoded) <= 100 else encoded[:94].decode("utf-16-le", errors="surrogatepass") + "..."


def _ddg_code(node: dict) -> str:
    # Joern's generated CODE accessor defaults to <empty>, and propertiesMap
    # omits that default. Native empty blocks represent this omitted property.
    # Other expressions can explicitly store "": a nested pointer declarator
    # has an empty IASTName, which AstForIdentifier copies into its CODE field.
    code = node["code"]
    return "<empty>" if not code and node["kind"] == "BLOCK" else code


def _ddg_cfg_codes(cpg: dict) -> dict[int, str]:
    nodes = {node["id"]: node for node in cpg["nodes"]}
    parents = {edge["target"]: edge["source"] for edge in cpg["edges"] if edge["kind"] == "AST"}
    codes = {}
    for node in cpg["nodes"]:
        if node["kind"] not in ("IDENTIFIER", "LITERAL", "METHOD_REF"):
            continue
        current = node["id"]
        seen = set()
        while current in parents and current not in seen:
            seen.add(current)
            current = parents[current]
            parent = nodes[current]
            if (parent["kind"] == "CALL" and parent.get("name") in _MEMBER_ACCESS_NAMES) or parent["kind"] == "ANNOTATION_PARAMETER_ASSIGN":
                continue
            if parent["kind"] in _EXPRESSION_KINDS:
                codes[node["id"]] = _ddg_code(parent)
            break
    return codes


def _ddg_label(node: dict, cpg: dict | None = None, *, cfg_code: str | None = None) -> str:
    """Joern 4.0.150 DOT label, before PyJoern's statement lifting."""
    kind, code = node["kind"], _ddg_code(node)
    if kind == "CALL":
        fields = (node.get("name", ""), _ddg_limit(code))
    elif kind == "CONTROL_STRUCTURE":
        fields = (kind, node.get("name", ""), code)
    elif kind == "METHOD":
        fields = (kind, node.get("name", ""))
    elif kind == "METHOD_RETURN":
        fields = (kind, node.get("type_name", ""))
    elif kind == "METHOD_PARAMETER_IN":
        fields = ("PARAM", code)
    elif kind == "LOCAL":
        fields = (kind, f"{code}: {node.get('type_name', '')}")
    elif kind == "JUMP_TARGET":
        fields = (kind, node.get("name", ""))
    elif kind in _EXPRESSION_KINDS:
        if cfg_code is None and cpg is not None:
            cfg_code = _ddg_cfg_codes(cpg).get(node["id"])
        fields = (kind, _ddg_limit(code), _ddg_limit(code if cfg_code is None else cfg_code))
    else:
        fields = ()
    label = ("(" + ",".join(fields) + ")").translate(_HTML4_ESCAPE) if fields else ""
    line = node.get("line")
    # The native graph uses zero for an absent Joern LINE_NUMBER property.
    return label + (f"<SUB>{line}</SUB>" if line is not None and line != 0 else "")


_DDG_OPERATORS = {
    "assignment": (Assignment, None, "="),
    "minus": (BinOp, BinOp.SUB, "minus"),
    "plus": (BinOp, BinOp.ADD, "plus"),
    "logicalAnd": (BinOp, BinOp.AND, "logicalAnd"),
    "logicalOr": (BinOp, BinOp.OR, "logicalOr"),
    "equals": (Compare, Compare.EQ, "=="),
    "greaterEqualsThan": (Compare, Compare.GTE, "&gt;="),
    "greaterThan": (Compare, Compare.GT, "&gt;"),
    "lessEqualsThan": (Compare, Compare.LTE, "&lt;="),
    "lessThan": (Compare, Compare.LT, "&lt;"),
    "notEquals": (Compare, Compare.NE, "!="),
    "return": (Return, None, "return"),
    "call": (Call, None, "return"),
    "conditional": (Ternary, None, ""),
    "PARAM": (Parameter, None, None),
}


def _lift_ddg_statement(raw_data: str, node: dict) -> Statement:
    """Port PyJoern's JIL parser, including its observable fallback behavior."""
    matches = re.findall("<SUB>([0-9]+)</SUB>", raw_data)
    line = int(matches[0]) if matches else None
    text = "(".join(raw_data.split("(")[1:])
    raw_stmt = "".join(text.split(")<SUB>")[:-1]).replace(" ", " " if text.startswith("PARAM") else "")
    arguments = (node["id"], node["kind"])
    if raw_stmt.startswith("&lt;operator&gt;."):
        parts = raw_stmt.split("&lt;operator&gt;.")[1].split(",")
        statement_type = parts[0]
    elif raw_stmt.startswith("RETURN"):
        parts = raw_stmt.split("RETURN")[1].split(",")
        statement_type = "return"
    elif raw_stmt.startswith("METHOD_RETURN"):
        return Nop(*arguments, raw_stmt, line)
    elif raw_stmt.startswith("METHOD"):
        # METHOD_REF is also a function-start marker in the original parser.
        return Nop(*arguments, raw_stmt, line)
    else:
        if raw_stmt.startswith("PARAM"):
            typ, name = raw_stmt.split("PARAM")[1].split(" ")
            return Parameter(*arguments, raw_stmt, line, name, typ.replace(",", ""))
        parts = raw_stmt.replace(" ", "").split(",")
        if len(parts) > 1 and parts[0] == parts[1].split("(")[0]:
            raw_call = parts[1]
            func = raw_call.split("(")[0]
            args = ")".join("(".join(raw_call.split("(")[1:]).split(")")[:-1]).split(",")
            return Call(*arguments, raw_stmt, line, func, args)
        return UnsupportedStmt(*arguments, raw_stmt, line)

    cls, subtype, separator = _DDG_OPERATORS.get(statement_type, (UnsupportedStmt, None, None))
    raw_ops = ",".join(parts[1:])
    operands = []
    if separator:
        split_ops = raw_ops.split(separator)
        operands = [separator.join(split_ops[1:])] if not split_ops[0] else split_ops[:1] + [separator.join(split_ops[1:])]
    if cls in (Compare, BinOp):
        return cls(*arguments, parts, line, subtype, *operands)
    if cls is Ternary:
        cond = raw_ops.split("?")[0]
        # The installed lifter takes characters here; parity preserves that data.
        return Ternary(*arguments, raw_stmt, line, cond, raw_ops[0], raw_ops[1])
    return cls(*arguments, parts, line, *operands)


def _property_graph(data: dict, edge_kind: str | None = None) -> nx.MultiDiGraph:
    graph = nx.MultiDiGraph()
    graph.add_nodes_from((node["id"], node.copy()) for node in data["nodes"])
    for edge in data["edges"]:
        if edge_kind is None or edge["kind"] == edge_kind:
            graph.add_edge(edge["source"], edge["target"], **edge)
    return graph


class Function:
    def __init__(self, data: dict, *, no_cfg: bool = False, no_ddg: bool = False, no_ast: bool = False, no_metadata: bool = False):
        self.raw = data
        self.name = data["name"]
        self.fullname = data["fullname"]
        self.filename = Path(data["filename"])
        self.return_type = "" if no_metadata else data["return_type"]
        self.signature = data["signature"]
        self.start_line = data["start_line"]
        self.end_line = data["end_line"]
        self.reaching_definitions = data.get("reaching_definitions")
        self._no_ddg = no_ddg
        self._no_ast = no_ast
        nodes = data["cpg"]["nodes"]
        self.calls = [] if no_metadata else [node["name"] for node in nodes if node["kind"] == "CALL" and node.get("name")]
        self.gotos = [] if no_metadata else [node["code"] for node in nodes if node["kind"] == "CONTROL_STRUCTURE" and node["code"].startswith("goto ")]
        self.control_structures = [] if no_metadata else [node["code"] for node in nodes if node["kind"] == "CONTROL_STRUCTURE"]
        self.macro_count = 0
        self.callees = list(dict.fromkeys(node["name"] for node in nodes if node["kind"] == "CALL" and node.get("name") and not node["name"].startswith("<")))
        self.cfg = None if no_cfg else self._cfg(data["cfg"], nodes)

    @staticmethod
    def _cfg(data: dict, nodes: list[dict]) -> nx.DiGraph:
        used_ids = {node for block in data["nodes"] for node in block["statements"]}
        statements = {
            node["id"]: (Nop if node.get("cfg_nop", node["kind"] in ("METHOD", "METHOD_RETURN", "METHOD_REF")) else Statement)(
                node["id"], node["kind"], node["code"], node["line"]
            )
            for node in nodes if node["id"] in used_ids
        }
        blocks = {
            block["id"]: Block(
                block["id"], tuple(statements[node] for node in block["statements"]),
                block["is_entrypoint"], block["is_exitpoint"],
            )
            for block in data["nodes"]
        }
        graph = nx.DiGraph()
        graph.add_nodes_from((block, {"node": block}) for block in blocks.values())
        for source, target in data["edges"]:
            src, dst = blocks[source], blocks[target]
            graph.add_edge(src, dst, src=src, dst=dst)
        return graph

    @staticmethod
    def _ddg_labels(data: dict, cpg: dict) -> dict[int, str]:
        cfg_codes = _ddg_cfg_codes(cpg)
        return {node["id"]: _ddg_label(node, cfg_code=cfg_codes.get(node["id"])) for node in data["nodes"]}

    @staticmethod
    def _ddg(data: dict, cpg: dict, name: str = "") -> nx.DiGraph:
        labels = Function._ddg_labels(data, cpg)
        addr_uses = defaultdict(int)
        blocks = {}
        for node in {node["id"]: node for node in data["nodes"]}.values():
            label = labels[node["id"]]
            statements = []
            if label:
                for raw_statement in label.split("\n"):
                    try:
                        statement = _lift_ddg_statement(raw_statement, node)
                    except Exception:
                        # PyJoern's exception fallback keeps the unparsed label
                        # and drops its line number, even when SUB is present.
                        statement = UnsupportedStmt(node["id"], node["kind"], raw_statement, None)
                    statements.append(statement)
                addr = statements[0].source_line_number
            else:
                addr = -node["id"]
                statements.append(Nop(node["id"], "NOP", "", None))
            idx = addr_uses[addr]
            addr_uses[addr] += 1
            blocks[node["id"]] = Block(
                addr, tuple(statements),
                isinstance(statements[0], Nop) and statements[0].type == Nop.FUNC_START,
                isinstance(statements[-1], Nop) and statements[-1].type == Nop.FUNC_END,
                idx=idx,
            )

        edges_by_source = defaultdict(list)
        for edge in data["edges"]:
            edges_by_source[edge["source"]].append(edge["target"])
        graph = nx.DiGraph(name=name)
        # The original lifter inserts only edge endpoints: isolated DOT nodes
        # disappear, and distinct labels between the same endpoints coalesce.
        for source, block in blocks.items():
            for target in edges_by_source[source]:
                destination = blocks[target]
                graph.add_node(block, node=block)
                graph.add_node(destination, node=destination)
                graph.add_edge(block, destination, src=block, dst=destination)
        return graph

    @cached_property
    def cpg(self) -> nx.MultiDiGraph:
        return _property_graph(self.raw["cpg"])

    @cached_property
    def ddg_raw(self) -> nx.MultiDiGraph | None:
        """Native REACHING_DEF graph, with CPG IDs and parallel edge labels."""
        data = self.raw.get("ddg")
        return _property_graph(data) if data is not None and not self._no_ddg else None

    @cached_property
    def ddg(self) -> nx.DiGraph | None:
        """PyJoern-compatible DDG of lifted blocks from Joern's DOT projection."""
        data = self.raw.get("ddg_view")
        return self._ddg(data, self.raw["cpg"], self.name) if data is not None and not self._no_ddg else None

    @cached_property
    def ast(self) -> nx.MultiDiGraph | None:
        return None if self._no_ast else _property_graph(self.raw["cpg"], "AST")

    def __repr__(self) -> str:
        return f"<Function {self.name} {self.signature} {self.start_line}:{self.end_line}>"


class Analysis:
    def __init__(self, data: dict, **function_options):
        self.raw = data
        self.schema_version = data["schema_version"]
        self.diagnostics = data["diagnostics"]
        self.functions = [Function(function, **function_options) for function in data["functions"]]

    def to_json(self, *, indent: int | None = None) -> str:
        return json.dumps(self.raw, indent=indent, ensure_ascii=False)


def parse_code(
    source: str | bytes,
    filename: str | Path = "input.c",
    *,
    language: str | None = None,
    preprocessed: bool = False,
    data_flow: bool = False,
    reaching_definitions: bool = False,
    strict: bool = False,
) -> Analysis:
    """Analyze source bytes in memory; data-flow analysis is optional.

    Reaching definitions imply the data-flow pass. CPG, CFG, DDG, diagnostics,
    and optional reaching definitions are retained in ``Analysis.raw`` and can
    be exported with ``Analysis.to_json()``.
    """
    return Analysis(_analyze(source, filename, language, data_flow, reaching_definitions, strict,
                             preprocessed=preprocessed))


def _analyze(source, filename, language=None, data_flow=False, reaching_definitions=False, strict=False,
             preprocessed=False) -> dict:
    source_bytes = source.encode("utf-8") if isinstance(source, str) else source
    filename_bytes = os.fspath(filename).encode("utf-8")
    if b"\0" in filename_bytes:
        raise ValueError("filename must not contain a null byte")
    options = json.dumps({
        "language": language, "preprocessed": preprocessed, "data_flow": data_flow or reaching_definitions,
        "reaching_definitions": reaching_definitions, "strict": strict,
    }).encode("utf-8")
    native = _library(os.environ.get("RUST_JOERN_LIBRARY"))
    buffer = ctypes.create_string_buffer(source_bytes)
    pointer = native.rust_joern_analyze(buffer, len(source_bytes), filename_bytes, options)
    return _analysis_result(native, pointer)


def _analyze_many(sources, language=None, data_flow=False, reaching_definitions=False, strict=False,
                  preprocessed=False) -> dict:
    """Analyze `(source, filename)` pairs with shared internal callee context."""
    prepared = []
    for source, filename in sources:
        source_bytes = source.encode("utf-8") if isinstance(source, str) else source
        filename_bytes = os.fspath(filename).encode("utf-8")
        if b"\0" in filename_bytes:
            raise ValueError("filename must not contain a null byte")
        prepared.append((ctypes.create_string_buffer(source_bytes), len(source_bytes), filename_bytes))
    if not prepared:
        return {"schema_version": 1, "functions": [], "diagnostics": []}
    options = json.dumps({
        "language": language, "preprocessed": preprocessed, "data_flow": data_flow or reaching_definitions,
        "reaching_definitions": reaching_definitions, "strict": strict,
    }).encode("utf-8")
    native = _library(os.environ.get("RUST_JOERN_LIBRARY"))
    analyze_many = getattr(native, "rust_joern_analyze_many", None)
    if analyze_many is None:
        raise RuntimeError(
            "Rust Joern native library lacks directory analysis support. "
            "Run `cargo build --release` in the rust-joern checkout."
        )
    inputs = (_NativeSource * len(prepared))(*(
        _NativeSource(ctypes.cast(buffer, ctypes.c_void_p), length, filename)
        for buffer, length, filename in prepared
    ))
    pointer = analyze_many(inputs, len(inputs), options)
    return _analysis_result(native, pointer)


def _analysis_result(native, pointer) -> dict:
    if not pointer:
        raise RuntimeError("Rust Joern returned a null result")
    try:
        data = json.loads(ctypes.string_at(pointer))
    finally:
        native.rust_joern_free(pointer)
    if "error" in data:
        raise RuntimeError(data["error"])
    return data


def _preprocess_decompilation(source: str) -> str:
    """Port PyJoern's explicit decompilation option, without modifying its file."""
    source = re.sub(r"undefined\s+\[\d{1,4}\]\s+__rustcall", "undefined", source)
    for old, new in {
        "__int8": "char", "__int16": "short", "__int32": "int", "__int64": "long",
        "__fastcall": "", "__noreturn": "", "__cdecl": "", "__rustcall": "",
    }.items():
        source = source.replace(old, new)
    lines = source.split("\n")
    for index, line in enumerate(lines[:3]):
        for old, new in {"new": "new_joern_token", "delete": "delete_joern_token"}.items():
            if old in line:
                lines[index] = line.replace(old, new)
    return "\n".join(lines)


def parse_source(
    source_path: str | Path,
    no_metadata: bool = False,
    no_cfg: bool = False,
    no_ddg: bool = False,
    no_ast: bool = False,
    is_decompilation: bool = False,
    *,
    reaching_definitions: bool = False,
    preprocessed: bool = False,
    strict: bool = False,
) -> dict[str, Function] | dict[tuple[str, str], Function]:
    """PyJoern-compatible file entry point used by DecBench.

    A file returns functions keyed by name; a directory returns functions keyed
    by ``(name, filename)``. Use ``no_ddg=True`` to skip the data-flow pass.
    """
    path = Path(source_path).absolute()
    if not path.exists():
        raise FileNotFoundError(f"Source file {path} does not exist")
    paths = sorted(path.rglob("*")) if path.is_dir() else [path]
    extensions = {".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".C", ".h", ".hh", ".hpp", ".hxx", ".i", ".ii"}
    blacklist = ("<", "+", "*", "(", ">", "JUMPOUT", "__builtin_unreachable")
    sources = []
    for filename in paths:
        if not filename.is_file() or (path.is_dir() and filename.suffix not in extensions):
            continue
        source = filename.read_bytes()
        if is_decompilation:
            source = _preprocess_decompilation(source.decode("utf-8", errors="replace")).encode("utf-8")
        sources.append((source, filename))
    if path.is_dir():
        analysis = _analyze_many(sources, data_flow=not no_ddg, reaching_definitions=reaching_definitions,
                                 strict=strict, preprocessed=preprocessed)
    else:
        source, filename = sources[0]
        analysis = _analyze(source, filename, data_flow=not no_ddg, reaching_definitions=reaching_definitions,
                            strict=strict, preprocessed=preprocessed)
    result = {}
    for raw in analysis["functions"]:
        function = Function(raw, no_metadata=no_metadata, no_cfg=no_cfg, no_ddg=no_ddg, no_ast=no_ast)
        if function.name.startswith(blacklist) or not function.name or (not no_cfg and not function.cfg):
            continue
        key = (function.name, str(function.filename)) if path.is_dir() else function.name
        previous = result.get(key)
        if previous is not None and previous.cfg is not None and function.cfg is not None and len(function.cfg) < len(previous.cfg):
            continue
        result[key] = function
    return result
