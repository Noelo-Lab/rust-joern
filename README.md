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
statement IDs and explicit entry/exit roles. `ddg` retains raw dependencies;
`ddg_view` supplies Joern's labeled DOT projection. DOT supports `cfg`, `ddg`,
and `cpg`.

The parser targets already-preprocessed C/C++ and sanitized decompiler output,
as supplied by DecBench. `.i`/`.ii` files are treated as preprocessed;
`--preprocessed` or Python `preprocessed=True` selects this mode for other
filenames. Retained compiler macro definitions and line directives preserve
source offsets. Active macros in raw inputs require preparation by the caller.
Unsupported executable syntax produces diagnostics; `--strict` rejects error
diagnostics. Recovery supported by the original CDT frontend follows its CFG.
This is an initial port of the relevant frontend behavior, not a complete port of
Eclipse CDT's compiler, type system, or full Joern schema. Data flow follows
Joern's intraprocedural overlay. CFG parity is checked separately
from DDG/CPG parity.

## Python API

Build the library above, then install the thin wrapper (`pip install -e .`) or use
`PYTHONPATH=python`. It requires NetworkX for the graph view; the Rust core does not.

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
ddg = flow.functions[0].ddg          # PyJoern-compatible DiGraph of Blocks
raw_dependencies = flow.functions[0].ddg_raw  # CPG-node MultiDiGraph
sets = flow.functions[0].reaching_definitions
```

The wrapper calls the Rust shared library through ctypes, with no subprocess or
temporary graph files. Set `RUST_JOERN_LIBRARY` to use a library outside the local
`target/release` or `target/debug` directories. `parse_source` accepts PyJoern's
`no_metadata`, `no_cfg`, `no_ddg`, `no_ast`, and `is_decompilation` flags. Its generic
DDG default follows PyJoern; `parse_code` defaults to CFG/CPG only.

`Function.ddg` preserves PyJoern's statement lifting and Block entry/exit roles.
The raw dependencies remain available through `Function.ddg_raw` and CPG
`REACHING_DEF` edges. Directory parsing resolves internal callee context across
files and removes declarations when a matching definition is present.

The reaching-definition overlay uses Joern 4.0.150's default limit of 4,000
generated definitions per method. Methods above that limit have empty DDGs;
explicit `reaching_definitions=True` still returns their solver sets. The
overlay follows Joern's intraprocedural behavior, including its call semantics
and access-path matching.

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

The reference fixtures were generated with PyJoern 4.0.150.4 and Joern 4.0.150.
The focused comparison matches all 48 reference graphs; the two original DecBench
source inputs match all 65 graphs (six function bodies and 59 declarations).
The IDA `bzip2recover` decompilation from the same DecBench run also matches all
13 function graphs after DecBench's original preparation. The local combined
report is `workspace/first-version-comparison.json`: 126 matches, no divergences,
and no candidate diagnostics. These are bounded smoke checks, not a claim of
parity on the entire dataset. DDG measurements are recorded separately in
[the DDG comparison report](reports/ddg-parity/README.md).
Actual DecBench input provenance and upstream fixture licenses are retained in
`tests/fixtures/decbench`. These initial fixtures do not establish corpus parity.

The DDG comparator checks the actual original `Function.ddg` and original
`dotDdg` independently. It preserves statement identity, all lifted statement
fields, directed edges, complete node labels, labeled parallel edges, and
function coverage. Frozen references come from unchanged PyJoern 4.0.150.4;
input bytes and original/candidate versions are bound to hashes. Solver tests
also replay original CPGs to distinguish analysis defects from frontend
lowering differences.

```sh
cargo build --lib
python3 scripts/compare_ddg_pyjoern.py \
  tests/fixtures/control.c tests/fixtures/expressions.c tests/fixtures/functions.cpp \
  --reference-dir tests/fixtures/ddg-parity/references \
  --output workspace/ddg-comparison.json
```

See the DDG report for broader source, decompiler, type, and directory checks.
Its final frozen build passes all 841 DDG comparisons, covering 799 unique
function contexts across 77 file parses and two directory parses. Both the
original public statement graph and labeled DOT graph must match exactly.
The source manifest, reference provenance, and final candidate hashes are
recorded in [the summary](reports/ddg-parity/summary.json).

## O0/O2 corpus audit

The [initial audit](reports/decbench-parity/README.md) records the original native
snapshot before the conversion fixes, covering all 8,808 files:
1,447 fully match, 1,405 extract successfully but diverge, and 5,956 fail strict
extraction. All references are available and every graph comparison completed.
The port does not yet have corpus parity. Eleven reduced failures retain original
snapshots and now pass strict tests; no divergence is waived.

A subsequent frozen build compared all 8,808 cases (6,377 unique prepared inputs)
in 95.79 seconds: 6,616 complete matches, 1,375 divergent files, and 817 strict
failures. Its function differences are 4,585 missing, 2,649 extra, and 976
divergent, down from 115,636 differences in the initial audit. Generation timings
and complete mismatch graphs are retained in
`workspace/parity-rounds/round-03/direct`. This direct C ABI round measures native
analysis/serialization plus Python JSON decoding; it excludes DecBench preparation
and graph object materialization. Eight workers each use two Rayon threads.
Successful unique-input native calls have median 51.27 ms and p95 144.00 ms.

Generation took 296.17 seconds with eight workers, including strict attempts,
permissive retries, preparation and result writing. Every file has measured CFG
analysis and end-to-end times in [the CSV](reports/decbench-parity/files.csv).
The [summary](reports/decbench-parity/summary.json) separates failed attempts,
successful extractions, matching files and permissive retries. The source-owner
audit also covers all 513 published binaries and 60,704 named functions.

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

The report includes `summary.json`, per-file `files.csv`, and compressed JSONL
records for every function and divergence. Degenerate graphs are counted
separately from nondegenerate functions. Reduced known failures and their exact
original snapshots are under `tests/fixtures/decbench-regressions`; all eleven
reduced cases now pass strict comparison. No divergence is waived by the corpus
comparator.

## Focused checks

```sh
cargo test
cargo clippy --all-targets -- -D warnings
PYTHONPATH=python /home/mahaloz/.virtualenvs/decbench/bin/python tests/test_python.py -v
```

The original Joern checkout is retained in git history at
`82456558d52ea127e005a74391ca06e3f16b17a6`; the conversion reference for this session
is `/tmp/rust-joern-reference-82456558`. Exact DecBench behavior is also checked
against Joern v4.0.150 at `958fdd3d976197a783f8ade1254b43e977648f28`
in `/tmp/joern-v4.0.150`. None of the old implementation is required to build or
run the port.
