# DecBench integration and GED reevaluation

Rust Joern replaces the CFG parser used by DecBench. DecBench still prepares
source code, sanitizes decompiler output, selects functions, resolves source
ownership and computes graph edit distance (GED). CFG and DDG reference
evaluations are documented in [CFG parity](eval/cfg_parity.md) and
[DDG parity](eval/ddg_parity.md).

## Use the compatibility shim

Build Rust Joern and install its Python package in DecBench's own environment.
Run these commands from the Rust Joern checkout with that environment active:

```sh
cargo build --release
python -m pip install -e .
PYTHONPATH="$PWD/compat:$PWD/python" \
  DECBENCH_CACHE_DIR="$PWD/workspace/decbench-rust-cache" decbench --help
```

The opt-in `compat/pyjoern` shim forwards `pyjoern.parse_source` to Rust. Its
defaults are `no_ddg=True`, `strict=True` and `preprocessed=True`; explicit
arguments are preserved. Strict mode rejects error diagnostics. Preprocessed
mode assumes the caller has already handled active macros and headers.

The shim is excluded from the installed package so original PyJoern remains
available for reference comparisons. The generic `rust_joern.parse_source`
API retains its DDG-enabled default and permissive parsing with diagnostics.
`RUST_JOERN_LIBRARY` selects a shared library outside `target/release` or
`target/debug`. Use a separate `DECBENCH_CACHE_DIR` for each backend, or disable
metric caching with `DECBENCH_NO_CACHE=1` when measuring fresh work.

## Rescore saved artifacts

[rescore_decbench_ged.py](../scripts/rescore_decbench_ged.py) computes fresh CFGs
and GED from existing artifacts. It requires an external DecBench checkout
with `scripts/reeval_ged.py`, its Python dependencies including `cfgutils` and
SciPy, and both the main and standalone Astra results trees. These inputs are
not bundled with Rust Joern.

Each results tree must contain `function_results.json` and optimization/project
directories with `compiled/` preprocessed `.i`/`.ii` sources and `decompiled/`
saved `.c` artifacts. Candidate filenames identify the decompiler and binary;
`// Function: NAME @ 0xADDRESS` markers select candidate functions. Compiled
binaries supply DWARF ownership when available. Preserve the original binary
and source layout, and retain `sample_set_manifest.json` when supplied.

For example, place the saved trees in `workspace/decbench-inputs/main` and
`workspace/decbench-inputs/astra`, with the DecBench checkout at `../decbench`:

```sh
PYTHONHASHSEED=0 python scripts/rescore_decbench_ged.py \
  --decbench-root ../decbench \
  --main-tree workspace/decbench-inputs/main \
  --sample-tree workspace/decbench-inputs/astra \
  --library target/release/librust_joern.so \
  --output workspace/ged-reevaluation/run \
  --workers 16 --rayon-threads 2 --ged-max-nodes 200
```

The output directory must be new. The script copies and hashes the native
library, freezes baseline copies, disables historical metric caches and guards
code and input hashes. It generates source graphs once per prepared language
and content pair, then generates candidate graphs and GED for every artifact.
Within the run, fresh source graphs and DWARF ownership are cached to avoid
repeated work. DWARF abstract-origin following is disabled to preserve the
recorded ownership contract. No compilation or decompilation is performed.

[summarize_decbench_ged.py](../scripts/summarize_decbench_ged.py) projects that
GED overlay onto the frozen baselines and rebuilds scoreboards using DecBench's
own merge and aggregate code:

```sh
python scripts/summarize_decbench_ged.py \
  --decbench-root ../decbench \
  --baseline workspace/ged-reevaluation/run/main/baseline.function_results.json \
  --overlay workspace/ged-reevaluation/run/main/ged_new.json \
  --slices workspace/ged-reevaluation/run/main/ged_new.slices.json \
  --sample-baseline workspace/ged-reevaluation/run/sample/baseline.function_results.json \
  --sample-overlay workspace/ged-reevaluation/run/sample/ged_new.json \
  --sample-slices workspace/ged-reevaluation/run/sample/ged_new.slices.json \
  --out workspace/ged-reevaluation/report
```

The report directory must be outside the DecBench checkout. The summarizer
preserves non-GED metrics, code extras, rows and frozen dataset memberships;
refreshes sample GED values; and emits per-function differences, all-identity
and site scoreboards, aggregates, summaries and an Astra synchronization check.
Inspect `completed.json`, `timings.json` and the report's `summary.json`, including
its `provisional` flags and native-run audit, before treating a run as complete.
Canonical results and published site data are not overwritten. Both scripts
provide additional options through `--help`.

## Recorded evaluation: 2026-10-09

The completed evaluation used DecBench 1.1, GED cache version 4, Python 3.14.6,
orjson 3.13.0, NetworkX 3.6.1, SciPy 1.18.0, cfgutils 1.16.0 and pyelftools 0.33.
It used 16 spawned workers, two Rayon threads per worker, single-threaded BLAS,
`PYTHONHASHSEED=0` and a 200-node GED threshold. The release library SHA-256 was
`e96d285dfec074ad5549f8b5a12b6ec0d8ce600b4de74ae2a1e607742fe27055`.
The build used release optimization level 3, thin LTO and one codegen unit.
The original full run records remain in git history at `1cad2265b`.

| Scope | Recorded coverage |
| --- | ---: |
| Unique prepared source inputs | 5,401 |
| Main saved candidate artifacts | 7,293 |
| Main function rows / binary groups / identities | 96,103 / 770 / 16 |
| Main applied measured GED entries | 543,118 |
| Standalone Astra artifacts / frozen targets | 223 / 250 |
| Parser or metric errors | 0 |
| Missing or extra evaluated slices | 0 / 0 |

All source and candidate tasks completed. Main GED values changed on 795
function/identity pairs, were added on 494 and cleared on 383. The table records
raw value changes; a changed distance need not change whether a result is
perfect. Cleared entries remove the old GED state for an evaluated slice that
is now unmeasured instead of retaining a stale result.

| Identity | Changed | Added | Cleared |
| --- | ---: | ---: | ---: |
| angr | 76 | 0 | 0 |
| binja | 530 | 0 | 383 |
| claude-code | 1 | 1 | 0 |
| codex | 2 | 0 | 0 |
| codex@gpt-6-astra | 26 | 139 | 0 |
| dewolf | 0 | 12 | 0 |
| ghidra | 2 | 0 | 0 |
| manifold | 1 | 0 | 0 |
| r2dec | 155 | 341 | 0 |
| reko | 0 | 1 | 0 |
| ventris | 2 | 0 | 0 |

`fission`, `glaurung`, `ida`, `kuna` and `retdec` had no raw GED value changes.
Standalone Astra changed on two targets, with 245 measured values before and
after. Every one of the 250 target GED states agreed with main `codex` in both
versions. This mapping uses exact `(opt, project, binary, function)` keys; the
main optimized `codex@gpt-6-astra` column is independent.

| Selected score, normalization off | Baseline perfect / denominator | Rust perfect / denominator |
| --- | ---: | ---: |
| Main Astra, O2-noinline | 21,835 / 33,563 (65.057%) | 21,934 / 33,563 (65.352%) |
| Main binja, O2-noinline | 9,009 / 33,563 (26.842%) | 9,067 / 33,563 (27.015%) |
| Main binja, O2 | 6,589 / 24,737 (26.636%) | 6,612 / 24,737 (26.729%) |
| Main codex, sample-set | 133 / 246 (54.065%) | 132 / 246 (53.659%) |
| Standalone Astra, sample-set | 133 / 245 (54.286%) | 132 / 245 (53.878%) |

The normalized main `codex` sample-set score stayed at 37/90 (41.111%). Main
versioned Astra's sample-set score stayed at 31/246 (12.602%). The recorded
independent audits passed execution/input checks, score reconstruction and
sample synchronization, with non-GED facts and memberships unchanged.

## Interpret scores and timings

GED compares directed CFG topology and boundary roles. Zero means structural
isomorphism; it does not certify source-level semantic equivalence. Nonmatching
graphs above the 200-node threshold use a size-based approximation; smaller
ones use DecBench's VJ GED procedure. Degenerate source CFGs and missing graphs
are excluded from measurements, and parse/metric failures remain explicit.

Percentages sum perfect function counts over a shared measurable-function
denominator; they do not average per-binary percentages. A producer's missing
measurement remains a non-perfect miss when another producer measured that
function. Standalone Astra's 245 measured targets therefore cannot replace the
main sample-set denominator of 246. Normalized aggregates restrict the
comparison population using DecBench's normalization gate. All-identity tables
include site-hidden identities such as `retdec`; actual site tables apply
visibility and preset restrictions before normalization.

| Pipeline stage | Elapsed wall seconds |
| --- | ---: |
| External CLI, including setup | 541.430 (9m 1s) |
| Instrumented pipeline | 540.841 |
| Inventory | 76.451 |
| Source generation | 34.801 |
| Source cache assembly | 55.278 |
| Ownership precompute | 15.693 |
| Main scoring | 324.165 |
| Standalone scoring | 32.211 |

Summed worker CFG-generation time was 502.792s for source inputs, 1,765.630s
for main artifacts and 1.886s for standalone artifacts. These intervals include
source preparation, native parsing/CPG/CFG export, JSON decoding and Python
graph construction. Native, `parse_source` and graph-materialization intervals
are nested and must not be added again. Parallel worker sums differ from wall
time. The instrumented pipeline excludes initial copying/imports/code hashing
and final report writes; scoring also includes source loading, ownership and GED.

A separate earlier eight-file paired CFG API benchmark measured PyJoern at
110.816s and Rust at 0.732s, about 151× faster for that sample. It included
PyJoern JVM startup and Python graph construction, and used an earlier Rust
build. It does not establish a full-corpus pipeline speedup or pure native CFG
generation time. The saved-artifact reevaluation compared fresh Rust GED
against frozen historical results without rerunning the original JVM backend.
