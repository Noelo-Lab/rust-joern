# rust-joern

Generate C/C++ control-flow graphs (CFG), code property graphs (CPG), data
dependency graphs (DDG), and reaching-definition sets. Export JSON or DOT from
the CLI, or access graphs in memory through Python.

The parser follows Joern's C/C++ frontend behavior and PyJoern's basic-block
normalization. Parsing and graph construction run in Rust; the Python API calls
the shared library in process. DecBench compatibility is the main target.

## Install

Requires Rust 1.90+. The Python API requires Python 3.10+.

From the checkout:

```sh
cargo build --release
python -m pip install -e .
```

## CLI

```sh
./target/release/rust-joern analyze input.c --output graphs.json
./target/release/rust-joern analyze input.cpp --format dot --graph cfg --output cfg.dot
./target/release/rust-joern analyze input.c --data-flow --reaching-definitions --output all.json
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
cpg = function.cpg                  # NetworkX MultiDiGraph
ddg = function.ddg                  # PyJoern-compatible block graph
definitions = function.reaching_definitions
diagnostics = analysis.diagnostics

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

Run DecBench in its own Python environment with the compatibility shim:

```sh
PYTHONPATH="$PWD/compat:$PWD/python" decbench --help
```

The shim forwards `pyjoern.parse_source` to Rust and defaults to
`no_ddg=True`, `strict=True`, and `preprocessed=True`. DecBench handles source
preparation, function selection, and GED scoring. Use a separate metric cache
when comparing backends.

For saved artifacts, [rescore_decbench_ged.py](scripts/rescore_decbench_ged.py)
generates fresh CFGs and GED results;
[summarize_decbench_ged.py](scripts/summarize_decbench_ged.py) produces score
comparisons for the main dataset and standalone Astra samples. See each
script's `--help` for arguments.

The [latest reevaluation](reports/ged-reevaluation-2026-10-09/README.md) completed
in **9m 1s** with 16 workers, including CFG extraction and GED scoring. It
contains per-function changes, all-decompiler score tables, timings, and audit
evidence. The original DecBench results remain unchanged.

## Verification

- **CFG:** all 8,808 saved cases match, covering 6,377 unique prepared inputs.
  Five original DOT-exporter failures were checked against recovered original
  graphs. See the [CFG certificate](reports/ged-reevaluation-2026-10-09/evidence/cfg-certificate/certification-status-root.json).
- **DDG:** the [recorded DDG audit](reports/ddg-parity/README.md) passes 841
  comparisons against original public and labeled DOT graphs. Its build
  predates the latest CFG recovery changes.
- **Speed:** an earlier paired eight-file benchmark measured **110.82s for
  PyJoern and 0.73s for Rust** (151×). These are complete API call intervals,
  including PyJoern's JVM startup and Python graph construction. See the
  [timing details](tools/cfg-compare/README.md).

Compare local fixtures with frozen PyJoern references:

```sh
python scripts/compare_pyjoern.py tests/fixtures/control.c \
    --reference-dir tests/fixtures --output workspace/cfg-comparison.json
```

For a visual comparison, run `python scripts/cfg_compare.py serve` and open
`http://127.0.0.1:8765`. The [viewer instructions](tools/cfg-compare/README.md)
cover capturing and exporting pairs.

## Development

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
python -m unittest discover -s tests -p 'test_*.py'
```
