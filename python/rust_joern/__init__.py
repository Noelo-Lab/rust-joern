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
from dataclasses import dataclass
from functools import cached_property, lru_cache
from pathlib import Path

import networkx as nx

try:
    from orjson import loads as _loads
except ImportError:
    _loads = json.loads

__version__ = "0.1.0"
__all__ = ["Analysis", "Block", "Function", "Nop", "Statement", "parse_code", "parse_source"]


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
    native.rust_joern_free.argtypes = [ctypes.c_void_p]
    native.rust_joern_free.restype = None
    return native


@dataclass(frozen=True, slots=True)
class Statement:
    id: int
    kind: str
    raw_text: str
    source_line_number: int

    def __str__(self) -> str:
        return self.raw_text


class Nop(Statement):
    """Method boundary marker; DecBench checks this class name for degeneracy."""

    __slots__ = ()
    FUNC_START = 0
    FUNC_END = 1

    @property
    def type(self) -> int:
        return self.FUNC_END if self.kind == "METHOD_RETURN" else self.FUNC_START

    def __str__(self) -> str:
        return "FUNCTION_START" if self.type == self.FUNC_START else "FUNCTION_END"


@dataclass(eq=False, slots=True)
class Block:
    id: int
    statements: tuple[Statement, ...]
    is_entrypoint: bool
    is_exitpoint: bool

    @property
    def addr(self) -> int:
        return self.id

    def __str__(self) -> str:
        return f"{self.id}:\n" + "\n".join(map(str, self.statements))

    def __repr__(self) -> str:
        return f"<Block {self.id}: {len(self.statements)} statements>"


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

    @cached_property
    def cpg(self) -> nx.MultiDiGraph:
        return _property_graph(self.raw["cpg"])

    @cached_property
    def ddg(self) -> nx.MultiDiGraph | None:
        data = self.raw.get("ddg")
        return _property_graph(data) if data is not None and not self._no_ddg else None

    @cached_property
    def ddg_projection(self) -> nx.MultiDiGraph | None:
        """Joern's displayed DDG, alongside the raw reaching-definition graph."""
        data = self.raw.get("ddg_projection")
        return _property_graph(data) if data is not None and not self._no_ddg else None

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
    if not pointer:
        raise RuntimeError("Rust Joern returned a null result")
    try:
        data = _loads(ctypes.string_at(pointer))
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
    result = {}
    extensions = {".c", ".cc", ".cp", ".cpp", ".cxx", ".c++", ".C", ".h", ".hh", ".hpp", ".hxx", ".i", ".ii"}
    blacklist = ("<", "+", "*", "(", ">", "JUMPOUT", "__builtin_unreachable")
    for filename in paths:
        if not filename.is_file() or (path.is_dir() and filename.suffix not in extensions):
            continue
        source = filename.read_bytes()
        if is_decompilation:
            source = _preprocess_decompilation(source.decode("utf-8", errors="replace")).encode("utf-8")
        analysis = _analyze(source, filename, data_flow=not no_ddg, reaching_definitions=reaching_definitions,
                            strict=strict, preprocessed=preprocessed)
        # FastParser's per-(FULL_NAME, FILE) toMap exports the last method,
        # once for each original occurrence. Keep all native methods in Analysis.
        exported = {
            (raw["fullname"], raw["filename"]): Function(
                raw, no_metadata=no_metadata, no_cfg=no_cfg, no_ddg=no_ddg, no_ast=no_ast,
            ) for raw in analysis["functions"]
        }
        for raw in analysis["functions"]:
            function = exported[(raw["fullname"], raw["filename"])]
            if (function.name.startswith(blacklist) or not function.name or not function.filename
                    or (not no_cfg and not function.cfg)):
                continue
            key = (function.name, str(function.filename))
            previous = result.get(key)
            if previous is not None and previous.cfg is not None and function.cfg is not None and len(function.cfg) < len(previous.cfg):
                continue
            result[key] = function
    return result if path.is_dir() else {name: function for (name, _), function in result.items()}
