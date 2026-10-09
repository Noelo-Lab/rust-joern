# Rust Joern

An AI fork of [Joern](https://joern.io) in Rust, minimizing supported features
and maximizing speed. Rust Joern contains a small subset of features motivated
by use in [DecBench](https://decbench.com). It is largely used to generate CFGs
from C/C++ decompilation that may not compile. Other features include DDG, CPG,
and reaching definitions generation.

In testing, Rust Joern was around **151×** faster for CFG
generation. See the [evaluation and timing details](docs/eval/cfg_parity.md).

## Install

Requires Rust 1.90+. The Python API requires Python 3.10+.

```sh
cargo build --release
python -m pip install -e .
```

## CLI

The binary is built at `./target/release/rust-joern`. Invoke that path or add
`target/release` to your `PATH` to use the commands below.

```sh
rust-joern analyze input.c --output graphs.json
rust-joern analyze input.cpp --format dot --graph cfg --output cfg.dot
```

Accepts files, directories, or `-` for stdin. Language detection uses the file
extension; `--language c|cpp` overrides it.

CFG and CPG generation are enabled by default. `--data-flow` adds DDG edges;
`--reaching-definitions` exports incoming and outgoing definition sets. DOT
output supports `--graph cfg|ddg|cpg`. Run `./target/release/rust-joern --help`
for all options.

## Python

```python
from rust_joern import parse_code, parse_source

analysis = parse_code(
    "int f(int x) { int y = x; return y; }",
    data_flow=True,
    reaching_definitions=True,
)
function = analysis.functions[0]
cfg = function.cfg                  # NetworkX DiGraph of basic blocks

# Parse a file without running data-flow analysis.
functions = parse_source("input.c", no_ddg=True, no_ast=True)
cfg = functions["f"].cfg
```

`parse_code` defaults to CFG/CPG generation. `parse_source` follows PyJoern's
DDG-enabled default; use `no_ddg=True` for CFG-only work. It also accepts
`no_metadata`, `no_cfg`, `no_ast`, and `is_decompilation`.

Raw dependencies are available through `function.ddg_raw`; the labeled Joern
projection is `function.ddg_projection`. Use `analysis.to_json()` to serialize
an in-memory result. Set `RUST_JOERN_LIBRARY` to select a shared library outside
the checkout's `target/release` or `target/debug` directories.

## Input and analysis scope

Inputs should be preprocessed C/C++ or sanitized decompiler output. `.i` and
`.ii` files select preprocessed mode automatically; use `--preprocessed` or
Python `preprocessed=True` for other filenames. The caller handles active macro
expansion and headers.

Parsing returns diagnostics. `--strict` or Python `strict=True` rejects error
diagnostics. The frontend implements the Joern/CDT behavior needed by DecBench,
with partial coverage of the full CDT type system and Joern CPG schema.

Data flow is intraprocedural. Joern's 4,000-generated-definition limit applies
to DDG generation; requesting reaching-definition sets still returns the solver
sets for methods above that limit.

## DecBench

See [DecBench integration](docs/decbench.md) for setup, saved-artifact GED
rescoring, and the recorded reevaluation results.

## Development

Repository layout, development checks, and instructions for working on parity
are in the [agent guide](docs/agents.md).
