# rust-joern

A small native Rust port of the C/C++ parsing and graph-generation behavior used
by Joern and PyJoern. DecBench CFG compatibility is the first target.

The core has no JVM, external parser, graph database, or Tree-sitter dependency.
It uses a native lexer and recursive parser, Joern's expression/statement lowering
and CFG construction, and the DOT projection and basic-block normalization that
PyJoern applies. Parsing and analysis can run entirely in memory.

## Build and use

```sh
cargo build --release
./target/release/rust-joern analyze input.c --output graphs.json
./target/release/rust-joern analyze input.cpp --format dot --graph cfg --output cfg.dot
./target/release/rust-joern analyze input.c --data-flow --reaching-definitions --output all.json
```

Files, directories, and stdin (`analyze -`) are accepted. C/C++ detection uses the
filename, including preprocessed `.i` and `.ii`; `--language c|cpp` overrides it.
The default computes CFG and CPG only. `--data-flow` adds DDG/`REACHING_DEF` edges;
`--reaching-definitions` additionally exports the solver's incoming/outgoing sets.
JSON contains per-function CPG nodes/edges and normalized CFG blocks with their
statement IDs and explicit entry/exit roles. DOT supports `cfg`, `ddg`, and `cpg`.

The parser targets already-preprocessed C/C++ and sanitized decompiler output,
as supplied by DecBench. `.i`/`.ii` files are treated as preprocessed;
`--preprocessed` or Python `preprocessed=True` selects this mode for other
filenames. Retained compiler macro definitions and line directives preserve
source offsets. Active macros in raw inputs require preparation by the caller.
Unsupported executable syntax produces diagnostics; `--strict` rejects error
diagnostics. Recovery supported by the original CDT frontend follows its CFG.
This is an initial port of the relevant frontend behavior, not a complete port of
Eclipse CDT's compiler, type system, or full Joern schema. Data-flow diagnostics
report the current intraprocedural limitations. CFG parity is checked separately
from DDG/CPG parity.

## Python API

Build the library above, then install the thin wrapper (`pip install -e .`).
NetworkX supplies graph views and orjson decodes native results. Direct
`PYTHONPATH=python` usage also works with NetworkX alone, using standard JSON.

```python
from rust_joern import parse_code, parse_source

analysis = parse_code("int f(int n) { if (n) return 1; return 0; }")
function = analysis.functions[0]
cfg = function.cfg                  # NetworkX DiGraph, matching PyJoern roles
cpg = function.cpg                  # AST, CFG, argument and reference edges
diagnostics = analysis.diagnostics

functions = parse_source("input.c", no_ddg=True)
cfg = functions["f"].cfg

flow = parse_code("int f(int x) { int y=x; return y; }",
                  data_flow=True, reaching_definitions=True)
ddg = flow.functions[0].ddg
displayed_ddg = flow.functions[0].ddg_projection
sets = flow.functions[0].reaching_definitions
```

The wrapper calls the Rust shared library through ctypes, with no subprocess or
temporary graph files. Set `RUST_JOERN_LIBRARY` to use a library outside the local
`target/release` or `target/debug` directories. `parse_source` accepts PyJoern's
`no_metadata`, `no_cfg`, `no_ddg`, `no_ast`, and `is_decompilation` flags. Its generic
DDG default follows PyJoern; `parse_code` defaults to CFG/CPG only.

## Run DecBench with the port

DecBench uses `pyjoern.parse_source`, `Function.name`, and `Function.cfg`. Its GED
metric reads graph topology and block entry/exit flags; source selection also
checks whether singleton blocks contain only `Nop` statements. The compatibility
wrapper exposes those fields, including PyJoern's normalization quirks.

The opt-in `compat/pyjoern` shim lets existing DecBench use this implementation
without modifying DecBench or replacing the installed PyJoern oracle:

```sh
PYTHONPATH="$PWD/compat:$PWD/python" /home/mahaloz/.virtualenvs/decbench/bin/decbench --help
```

Use the same environment for an existing DecBench driver. The shim defaults to
`no_ddg=True` because DecBench only consumes CFGs and `strict=True` so unsupported
executable syntax fails extraction; explicit flags still work.
DecBench continues to own its original sanitization, macro expansion, system
header stripping, translation-unit selection, and metric calculations. Use an
isolated metric cache for candidate results so old backend caches cannot mask a
change in graph recovery.

## Compare with unchanged PyJoern

The comparison tool checks function coverage, directed graph isomorphism with
entry/exit roles, and source-CFG degeneracy. Equal node/edge counts alone do not
pass. Candidate diagnostics, missing/extra functions, and every divergence fail.
Reference and candidate receive the same prepared input bytes, checked by SHA-256.

```sh
PYTHONPATH=python /home/mahaloz/.virtualenvs/decbench/bin/python scripts/compare_pyjoern.py \
    tests/fixtures/control.c tests/fixtures/expressions.c tests/fixtures/functions.cpp \
    --reference-dir tests/fixtures --output workspace/fixture-comparison.json

PYTHONPATH=python /home/mahaloz/.virtualenvs/decbench/bin/python scripts/compare_pyjoern.py \
    tests/fixtures/decbench/bits.c tests/fixtures/decbench/libgzip_a-stripslash.c \
    --reference-dir tests/fixtures/decbench --output workspace/decbench-comparison.json
```

For a new DecBench input, supply `--reference-python` pointing to an interpreter
with the original PyJoern and `--decbench` pointing to that checkout. `.i`/`.ii`
inputs use DecBench's original preparation; `--sanitize-decompiled` prepares
decompiler output. The oracle runs in an isolated subprocess with the replacement
import path removed. See `scripts/compare_pyjoern.py --help`. Frozen references
can be written with `--save-reference-dir PATH` for later comparisons.

Frozen fixtures come from unchanged PyJoern 4.0.150.4 and Joern 4.0.150.
Actual DecBench input provenance and upstream fixture licenses are retained in
`tests/fixtures/decbench`. Additional original snapshots cover C/C++ declarations,
parser recovery, expressions, function references, CFG projection and selection.

## O0/O2 corpus audit

The [latest audit](reports/decbench-parity-latest/README.md) records the final
frozen build against `full_run_address_2026-09-11`: all **8,808 inputs**
(6,377 unique prepared inputs), including O0/O2 source, IDA and Kuna output.
The raw cache comparison has **8,803 exact matches**, zero strict extraction
failures, zero missing functions and zero differing CFGs. The remaining five
cases have empty original references; isolated original Joern recaptures on
these exact inputs reproduce recursive DOT exporter stack overflows.
Separately recovered original graphs match all 2,609 function occurrences in
those five cases, giving **8,808/8,808 CFG matches**.

Recovery changes only traversal of existing original CFG edges; it preserves
original parsing, graph construction, DOT labels and PyJoern normalization.
It was checked against 892 byte-identical original DOT exports. Original cached
references and raw differences remain unchanged. The certificate is separate
and applies only to the five proven exporter failures.

The direct native audit completed in **101.14 seconds** with eight workers and
two Rayon threads per worker. It measures native analysis/serialization plus
standard Python JSON decoding and comparison, excluding DecBench preparation
and graph materialization. The actual DecBench extraction audit completed in
**142.49 seconds**, including preparation, graph construction and audit checks.
Its native call median/p95 is **51.60/141.98 ms**, and its complete extraction
median/p95 is **87.26/250.35 ms**. Per-file CFG timings and source
ownership results are linked from the latest report.

The actual Python/DecBench path also matches all inputs. Real DWARF ownership
checks cover **513 binaries and 60,704 named functions**: all **57,499 available
original source bodies** match, with zero selected-TU differences. The same
3,205 names have no source body on either side. Older published per-binary JSON
has separate snapshot/selection differences, retained in the audit evidence.

The [initial audit](reports/decbench-parity/README.md) retains the earlier build
and its failures for comparison. Its numbers describe that historical snapshot.

The full audit inventories every compiled `.i`/`.ii` and IDA/Kuna `.c` input in
the canonical results tree. Source references are loaded from the original
content-addressed PyJoern cache. Missing references are captured with isolated
original PyJoern 4.0.150.4 invocations. Neither DecBench nor the installed oracle
is modified.

Install `pip install -e '.[parity]'` for the compiled exact graph matcher used
by the corpus audit. Symmetric switch CFGs can make the Python matcher very slow.
The Rust parser and runtime do not depend on this extra.

```sh
PY=/home/mahaloz/.virtualenvs/decbench/bin/python
DEC=/home/mahaloz/github/decbench
$PY scripts/inventory_decbench.py --results "$DEC/results/full_run_address_2026-09-11" --decbench "$DEC" --output workspace/decbench-parity
$PY scripts/capture_pyjoern_corpus.py --manifest workspace/decbench-parity/manifest.json
RAYON_NUM_THREADS=2 $PY scripts/check_decbench_usage.py --manifest workspace/decbench-parity/manifest.json --output workspace/decbench-parity/candidates
$PY scripts/compare_decbench.py --manifest workspace/decbench-parity/manifest.json --candidates workspace/decbench-parity/candidates --output reports/decbench-parity
$PY scripts/check_source_ownership.py --manifest workspace/decbench-parity/manifest.json --candidates workspace/decbench-parity/candidates --dataset /home/mahaloz/github/decbench-dataset --output workspace/decbench-parity/ownership.json
```

The usage checker runs DecBench's actual extractor through the opt-in shim. It
records default strict failures separately from permissive fallback graphs.
Every file retains preparation hashes, diagnostics, graph coverage and measured
generation times. Native timing includes analysis, serialization and Python JSON
decoding; graph materialization and end-to-end DecBench extraction are separate.
The historical source cache has no CFG-generation timing, so that field remains
null. Fresh reference timings include original PyJoern JVM startup and lifting.
Checks return a nonzero status when differences are recorded.

The report includes raw and recovered-reference summaries, per-file `files.csv`,
compressed function comparisons, binary ownership records, timings, build
fingerprints and artifact hashes. Degenerate graphs are counted separately.
No ordinary corpus comparison tolerates a divergence.

## Focused checks

```sh
cargo test
cargo clippy --all-targets -- -D warnings
PYTHONPATH=python:workspace/decbench-parity/tools /home/mahaloz/.virtualenvs/decbench/bin/python -m unittest discover -s tests -p 'test_*.py' -v
```

The Rust suite includes comparisons against all 60 original data-flow fixture
methods: CFG/reference edges, raw DDG, displayed DDG and reaching-definition
sets. These establish the supported intraprocedural behavior; the full DecBench
corpus audit establishes CFG parity. The corpus does not exercise or certify the
entire Joern CPG schema or interprocedural data flow.

The final check has 132 passing Rust tests and 84 passing Python tests;
formatting and Clippy pass. C/C++ JSON and CFG/DDG/CPG DOT exports are checked.
The runtime consists of nine Rust files and the thin Python wrapper. The
rewrite removes 2,209 old files; the old frontends and JVM build are unnecessary.

The original Joern checkout is retained in git history at
`82456558d52ea127e005a74391ca06e3f16b17a6`; the conversion reference for this session
is `/tmp/rust-joern-reference-82456558`. Exact DecBench behavior is also checked
against Joern v4.0.150 at `958fdd3d976197a783f8ade1254b43e977648f28`
in `/tmp/joern-v4.0.150`. None of the old implementation is required to build or
run the port.
