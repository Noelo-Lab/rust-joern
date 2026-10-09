# CFG parity evaluation

The complete saved DecBench CFG corpus passed on **2026-10-09**: **8,808
cases**, covering **6,377 unique prepared C/C++ inputs**, have matching function
coverage, directed topology, entry/exit roles and source degeneracy. The raw
comparison retains five empty original caches as differences; separate recovery
of original Joern exports accounts for those cases. No other divergence was
accepted.

## Corpus and comparison method

The corpus was DecBench's `full_run_address_2026-09-11`, restricted to O0/O2
preprocessed source and IDA/Kuna decompilation. Source preparation used
DecBench's unchanged header stripping; decompiled inputs used its sanitation
and local preprocessing. SHA-256 checks ensured that both engines received
identical prepared bytes. Ten compiler probes excluded from GED source
selection were still included in the CFG audit and excluded from ownership
maps.

Original references comprised 5,363 content-addressed source caches and 1,014
isolated fresh captures. Historical caches do not record parser version or
generation time. Fresh captures used unchanged **PyJoern 4.0.150.4 / Joern
v4.0.150**, with `no_metadata=True`, `no_ddg=True` and `no_ast=True`, in separate
working directories/interpreters with `PYTHONPATH` removed.

Native analysis requested `strict=True`, `preprocessed=True`, `data_flow=False`
and `reaching_definitions=False`. Each unique prepared input/language was
generated once, then compared for every original case. Successful extraction
and an empty diagnostic list were required. The runner preserves separately
timed permissive evidence after a strict failure; such evidence cannot turn
failed strict usage into a passing case. The final run needed no retries.

Comparison requires complete function-name coverage and exact directed graph
isomorphism, coloring nodes by entry/exit roles, plus DecBench's degeneracy
classification. A graph is degenerate when empty, or when its sole block
contains only `Nop` statements. Counts alone cannot pass. The corpus matcher
used igraph 1.0.0's BLISS implementation, with exact graph identity as a fast
proof for repeated graphs. The retained comparator also has a NetworkX exact
isomorphism fallback.

Function normalization reproduces original export and selection order: the
last graph for each raw `(FULL_NAME, FILE)` is replayed at every original
occurrence; selection keeps the largest `(NAME, Path(FILE))` graph, with later
ties winning. Original blacklist, empty-CFG and method-reference behavior are
preserved.

## Recorded results

| Input | Cases | Raw cache matches | Matches with certified recovery |
| --- | ---: | ---: | ---: |
| O0/source | 3,763 | 3,760 | 3,763 |
| O0/IDA | 256 | 256 | 256 |
| O0/Kuna | 256 | 256 | 256 |
| O2/source | 4,019 | 4,017 | 4,019 |
| O2/IDA | 257 | 257 | 257 |
| O2/Kuna | 257 | 257 | 257 |
| **Total** | **8,808** | **8,803** | **8,808** |

The fresh 2026-10-09 run made 6,377 native calls, resumed none, recorded
successful strict usage for all 8,808 cases and retained 27,013 unchanged
input/reference/helper guards. Raw results contain **4,079,344 matching function
occurrences** and **2,609 extras** from the five empty caches. Recovery matches all 2,609, giving
**4,081,953 matching occurrences**, with zero missing, extra, topology/role or
degeneracy differences. These counts include repeated declarations and shared
translation units; they are not unique source-body counts.

An earlier **2026-10-08** audit ran DecBench's unchanged extractor through the
native compatibility shim, with caching disabled. All 8,808 strict calls
passed. It checked every raw function's statement classes, block membership,
edges and roles, the selected object and the returned graph: 4,082,028 raw
functions, 6,826,425 blocks and 24,159,865 statements.

That earlier build also passed real DWARF/source-selection checks for **513
published binaries and 60,704 named functions**. All binary hashes matched the
published manifest; all **57,499 available original source bodies** matched,
including selected translation units. The same **3,205 names** lacked a source
body on both sides. Ownership checks read binaries without executing them.
Older published per-binary JSON was a secondary snapshot: its 2,017 extra,
527 missing and 247 different graphs were recorded separately and granted no
CFG waiver.

The two audits used different frozen builds. The 2026-10-09 certificate covers
fresh direct CFG generation; the detailed actual-extraction and ownership
results above belong to the 2026-10-08 build.

```text
2026-10-09 library SHA-256:
e96d285dfec074ad5549f8b5a12b6ec0d8ce600b4de74ae2a1e607742fe27055
2026-10-09 source/build fingerprint:
c262cf0b657cd0fc0ed60312095c92f24a095f05d66345ce47d8c7909ecd1f73
2026-10-08 library SHA-256:
ae7bdb3584b8b36fdfd2ccb2b6e92dcfacd3acb4fc192df044effeebe71ff7b2
```

The original full reports and certificates remain in git history at
`1cad2265b`; this document preserves their methodology and results for release.

## Original exporter failures

The five cases are NuttX `lib_wcwidth.i` at O0, and Cleanflight/Crazyflie
`arm_common_tables.i` at O0/O2: three unique prepared inputs. Their historical
caches are untouched empty dictionaries without exception/version metadata.
Isolated recaptures of the exact bytes reproduced `StackOverflowError` in
recursive CFG DOT projection, after original graph construction.

Recovery read the original CPG's existing CFG edges and replaced recursive
invisible-node contraction with an explicit DFS stack, preserving path cycle
checks, traversal and output order. It reused the original DOT serializer and
unchanged PyJoern lifting/block normalization. Frontend, AST, CFG passes and
historical references were unchanged. Increasing only the Java stack separately
recovered NuttX and Cleanflight; Crazyflie required iterative projection.

The iterative exporter reproduced **892 byte-identical genuine original DOT
exports**, including the NuttX global graph and bounded C/C++ control/assembly
probes. All **1,725 unique recovered graphs** matched the same fresh native
records, yielding 2,609 case-weighted matches. The earlier audit also checked
actual DecBench-returned graphs for all five cases.

The failing global owner is inferred from export enumeration, rather than an
explicit exception field. Cleanflight/Crazyflie global exports could not be
compared with successful default recursive exports; unchanged original edges
and the byte-identical controls substantiate the recovery. Raw and supplemental
results stayed separate: the ordinary comparator reports those five empty-cache
differences and exits nonzero.

## Timing and scope

The 2026-10-09 direct audit took **103.09 s** with eight processes and two Rayon
threads each, including analysis, comparison and writing. Its summed native
call intervals were **544.19 s**, counted once per unique input. Calls include
the C ABI, CPG/CFG construction, JSON encoding and Python decoding; they exclude
DecBench preparation and Python graph materialization.

The earlier actual DecBench audit took **142.49 s** with the same concurrency.
Native call median/p95 were **51.60/141.98 ms**; audited extraction median/p95
were **87.26/250.35 ms**, including preparation, Python graphs and view-proof
overhead. These stages overlap and must not be added. Calls used orjson 3.13.0;
the direct runner uses standard JSON decoding. Original cache timings are
unknown, so these runs do not provide a full-corpus original/native speed ratio.

A separate earlier paired sample selected **24 functions from eight complete
prepared files**, across available C/C++, optimization and source/IDA/Kuna
strata, using seed `6196664079766316072`. Selection favored readable graphs
(3–50 nodes, files at most 250 KB), without requiring graph agreement. Original
`parse_source` calls totaled **110.816 s** versus **0.732 s** for Rust:
**151.4×**. Both intervals include Python CFG materialization; original calls
include JVM startup. This measures full-file API calls for that sample, using
an earlier Rust build. It is not a full-corpus or per-function speedup.

All 24 pairs matched directed topology/roles but exposed public attribute
differences. Corpus CFG parity therefore does not establish complete node/edge
attribute parity, full CDT type behavior, the entire Joern CPG schema or
interprocedural data flow. See [DDG parity](ddg_parity.md) for its separate scope
and [DecBench](../decbench.md) for GED evaluation.

## Reproducing checks

Frozen local fixtures need no original Joern installation:

```sh
cargo build --release
python -m pip install -e '.[parity]'
python scripts/compare_pyjoern.py tests/fixtures/control.c \
  --reference-dir tests/fixtures --output workspace/cfg-comparison.json
PYTHONPATH=python python -m unittest discover -s tests \
  -p test_decbench_regressions.py -v
```

For a corpus rerun, set `DECBENCH` to a DecBench checkout, `RESULTS` to the saved
`full_run_address_2026-09-11` directory, `ORACLE_PYTHON` to a separate original
PyJoern 4.0.150.4 interpreter, and `DATASET` to the published binary dataset.
The native evaluation environment also needs DecBench's Python dependencies.
Use fresh output directories. These external inputs are not shipped here.

```sh
python scripts/inventory_decbench.py --results "$RESULTS" --decbench "$DECBENCH" \
  --output workspace/cfg-eval --workers 8
python scripts/capture_pyjoern_corpus.py --manifest workspace/cfg-eval/manifest.json \
  --reference-python "$ORACLE_PYTHON" --workers 8
python scripts/check_decbench_usage.py --manifest workspace/cfg-eval/manifest.json \
  --library target/release/librust_joern.so --output workspace/cfg-usage \
  --workers 8 --rayon-threads 2
python scripts/compare_decbench.py --manifest workspace/cfg-eval/manifest.json \
  --candidates workspace/cfg-usage --output workspace/cfg-results --workers 8
python scripts/check_source_ownership.py --manifest workspace/cfg-eval/manifest.json \
  --candidates workspace/cfg-usage --dataset "$DATASET" \
  --output workspace/cfg-ownership.json
```

`compare_native_corpus.py` provides the distinct direct-ABI audit; its required
`--build-provenance` binds a frozen source snapshot and library digests. Full
replay of the five supplemental certificates also needs the original recovered
graphs, export helpers and recovery provenance from the evaluation workspace.
Those external artifacts are not part of this release. The commands above keep
empty historical caches and report their differences; they do not recreate or
silently apply recovery. Frozen [regression fixtures](../../tests/fixtures/decbench-regressions/README.md)
preserve independently captured examples of eleven corpus issues now fixed.
