# Agent guide

This is the shared repository guide. Top-level `AGENTS.md` and `CLAUDE.md`
symlink to this file; update it here when the repository's workflow changes.
All commands below run from the repository root.

## Project and layout

Rust Joern implements the C/C++ parsing and graph behavior needed by DecBench
in native Rust, with a CLI and an in-process Python API. Compatibility with
Joern 4.0.150 / PyJoern 4.0.150.4 is the reference target. Inputs include
preprocessed source and sanitized decompiler output that may not compile.

| Location | Responsibility |
| --- | --- |
| `src/lexer.rs`, `src/parser.rs`, `src/syntax.rs` | Tokens, C/C++ syntax, bindings, and CDT-compatible recovery |
| `src/builder.rs`, `src/graph.rs` | CPG construction, CFG lowering, and serialized graph types |
| `src/normalize.rs` | Joern CFG projection and PyJoern basic-block normalization |
| `src/dataflow.rs`, `src/ddg.rs` | Reaching definitions, semantic summaries, and labeled DDG projection |
| `src/lib.rs`, `src/main.rs` | Analysis pipeline, multi-file context, C ABI, and JSON/DOT CLI |
| `python/rust_joern/__init__.py` | Shared-library loading, Python API, and NetworkX graph views |
| `compat/pyjoern/__init__.py` | Opt-in DecBench shim, excluded from the installed package |
| `tests/`, `tests/fixtures/` | Rust/Python regressions and frozen original graph references |
| `scripts/` | Reference capture, parity comparison, DecBench validation, and GED rescoring |
| `docs/` | Integration documentation, historical evaluation methods/results, and this guide |
| `workspace/` | Ignored generated captures, evaluation outputs, and scratch work |

The [README](../README.md) covers installation and public usage.
[DecBench integration](decbench.md), [CFG parity](eval/cfg_parity.md), and
[DDG parity](eval/ddg_parity.md) describe the evaluation contracts and recorded
results. Full historical reports remain in git history; current runs write
their outputs under `workspace/`.

## Build and verification

Use Rust 1.90+ and a Python 3.10+ environment with the wrapper's dependencies.
The `parity` extra adds igraph for corpus comparison:

```sh
python3 -m pip install -e '.[parity]'
cargo build --lib
cargo fmt --all -- --check
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
PYTHONPATH=python RUST_JOERN_LIBRARY="$PWD/target/debug/librust_joern.so" \
  python3 -m unittest discover -s tests -p 'test_*.py'
```

Build the debug library before Python checks: native DDG audit tests require
it. The wrapper otherwise searches for a packaged library, then release and
debug builds; an explicit `RUST_JOERN_LIBRARY` selects the build being tested.
Use `cargo build --release` for the release CLI and shared library. Run checks
appropriate to the change, and add focused regression fixtures when fixing
parser, normalization, or data-flow behavior.

DecBench-specific tests need its checkout and Python dependencies. Use that
environment to run the complete integration suite and report any skipped
tests when those prerequisites are absent.

For a quick CFG comparison against a frozen original fixture:

```sh
python3 scripts/compare_pyjoern.py tests/fixtures/control.c \
  --reference-dir tests/fixtures --output workspace/cfg-comparison.json
```

The evaluation documents contain broader replay commands. A historical audit
describes its recorded build; establish a modified build's result by running
the relevant checks and stating the scope actually measured.

## Compatibility contracts

- `parse_code` defaults to CFG/CPG generation. `parse_source` retains PyJoern's
  DDG-enabled default. The DecBench shim defaults to `no_ddg=True`,
  `strict=True`, and `preprocessed=True`, while preserving explicit options.
- Strict parsing rejects error diagnostics. Preserve diagnostics for
  unsupported executable syntax and the caller's responsibility for active
  macro expansion, headers, and decompiler preparation.
- Function selection, duplicate export order, full method identities, source
  ownership, and cross-file callee context affect observable graphs. Preserve
  those rules when changing parsing or graph construction.
- CFG parity requires directed topology, entry/exit roles, function coverage,
  and degeneracy checks. Node/edge attribute equality needs a separate check;
  matching counts alone do not establish parity.
- Keep raw reaching-definition edges, labeled DDG projection, and public
  lifted DDG views distinct. Joern's 4,000-generated-definition cutoff applies
  to DDG generation; explicit reaching-definition requests still return solver
  sets above that limit.

## Oracle and DecBench work

Capture references with unchanged original PyJoern in a separate interpreter
and working directory. Enable `compat` through `PYTHONPATH` only for DecBench
commands using the replacement backend; keep it out of original-reference
captures and ordinary test setup. References must come from the original
backend and retain source/prepared hashes, versions, flags, and provenance.
Compare identical prepared bytes, freeze the candidate for multi-group runs,
and retain failures separately from warnings and supplemental recovery.

Use `inventory_decbench.py` and `capture_pyjoern_corpus.py` to create fresh
corpus inputs and references. `compare_pyjoern.py`, `compare_ddg_pyjoern.py`,
and `compare_native_corpus.py` implement the graph comparisons.
`check_decbench_usage.py`, `check_cfg_views.py`, `compare_decbench.py`, and
`check_source_ownership.py` validate DecBench extraction and ownership.
`rescore_decbench_ged.py` and `summarize_decbench_ged.py` generate GED overlays
and score comparisons; use their `--help` for required external inputs.

Keep a separate metric cache per backend or set `DECBENCH_NO_CACHE=1` for
fresh measurements. Generate evaluation artifacts in `workspace/` and leave
canonical DecBench result trees and published site inputs intact. Preserve
source sanitization, function selection, ownership, frozen dataset membership,
and non-GED metrics when modifying the integration.
