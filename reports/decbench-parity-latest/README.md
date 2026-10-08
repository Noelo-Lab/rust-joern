# DecBench CFG parity — 2026-10-08

The native Rust C/C++ port matches all **8,808 DecBench input cases**, including
**6,377 unique prepared inputs**. All available original CFGs match directly.
Five cases with empty historical references also match separately recovered
original graphs. There are no unexplained missing functions, extra functions,
topology differences, role differences, degeneracy differences or strict errors.

The corpus is `full_run_address_2026-09-11`. Every comparison uses identical
prepared bytes, verified by SHA-256. Function-name coverage, exact directed
isomorphism, entry/exit roles and DecBench source degeneracy must all agree.
Node/edge counts alone cannot pass. The native build was frozen throughout.

| Input | Cases | Raw cache matches | Matches with certified recovery |
| --- | ---: | ---: | ---: |
| O0/source | 3,763 | 3,760 | 3,763 |
| O0/IDA | 256 | 256 | 256 |
| O0/Kuna | 256 | 256 | 256 |
| O2/source | 4,019 | 4,017 | 4,019 |
| O2/IDA | 257 | 257 | 257 |
| O2/Kuna | 257 | 257 | 257 |
| **Total** | **8,808** | **8,803** | **8,808** |

Raw comparisons contain 4,079,344 matching function occurrences and 2,609 extras
from the five empty-reference cases. The separate certificates match those
2,609, giving 4,081,953 matching occurrences. These include repeated header
declarations and translation units; they are not unique source-body counts.
Ten compiler probes excluded from GED source selection remain recorded in the
corpus audit and are excluded from binary ownership maps.

## Actual DecBench extraction and source ownership

The unchanged DecBench extractor was run through the opt-in native `pyjoern`
shim for every input, with caching disabled. All **8,808 strict extractions
pass**, without permissive retries or diagnostics. Verification checks every raw
function's statement classes, block membership, edges and roles, including
discarded duplicate functions. It also verifies the actual selected object and
the graph returned by DecBench. The proof covers 4,082,028 raw functions,
6,826,425 blocks and 24,159,865 statements.

The original selection contract is preserved: the last graph per raw
`(FULL_NAME, FILE)` is replayed for each original occurrence; selection then
keeps the largest `(NAME, Path(FILE))` graph, with later ties winning. Method
reference markers and normalization quirks remain compatible with PyJoern.

Real DWARF and DecBench's unchanged ownership/selection helpers were checked
for all **513 published binaries and 60,704 named functions**. All binary hashes
match the published manifest. All **57,499 original source bodies match**, with
zero missing/extra bodies, topology/degeneracy differences or selected-TU
differences. The same **3,205 names are absent on both sides**; those are baseline
unavailability. Ownership auditing does not execute binaries.

The older published per-binary JSON is a secondary snapshot. It differs from
the current hash-verified original TU caches and current ownership rules for
2,017 extra, 527 missing and 247 differing graphs. The unchanged historical
[snapshot audit](../decbench-parity/published-reference-audit.json) explains
1,963 discrepancies with the earlier publisher's selection without DWARF;
527 current bodies are absent from published maps, and 301 shapes are absent
from the current optimization-specific caches. The latter groups remain
historical snapshot differences. They do not grant a Rust CFG waiver.

## Five original exporter failures

The affected cases are NuttX `lib_wcwidth.i` at O0 and Cleanflight/Crazyflie
`arm_common_tables.i` at O0/O2: three unique prepared inputs, five cases.
Their untouched historical caches contain empty dictionaries and do not record
the parser version, exception or generation timing. Isolated recaptures of the
exact prepared bytes with original PyJoern 4.0.150.4 / Joern v4.0.150 reproduce
`StackOverflowError` in recursive CFG DOT projection after graph construction.

Recovery reads the original CPG's unchanged CFG edges and uses an explicit DFS
stack for the same invisible-node contraction, cycle checks and output order.
It reuses the original DOT serializer and unchanged PyJoern lifting and block
normalization. No frontend, AST, CFG pass or original reference is modified.
Raising only Java's stack limit independently recovered the NuttX/Cleanflight
references. Iterative projection was verified against **892 byte-identical
genuine original DOT exports**, including the NuttX global graph and bounded
C/C++ assembly/control probes. All **1,725 unique recovered graphs** match;
the actual DecBench-returned graphs are independently checked for all five cases.

This is an exporter failure, not evidence that original CFG creation was wrong.
The crashing global owner is inferred from export enumeration rather than an
explicit exception field. Crazyflie/Cleanflight global exports cannot be checked
against successful default-stack recursive exports; the control comparisons and
unchanged original edge exports substantiate the recovery.

The [raw summary](actual-summary.json), [direct certificate](recovery-certification.json)
and [actual-extraction certificate](actual-recovery-certification.json) remain
separate. The ordinary comparator still reports the five empty-cache differences
and returns a nonzero status. No other reference is substituted or difference
tolerated. The [recovery manifest](recovery-manifest.json) records every hash,
trace, original engine and helper. The complete referenced recovery artifacts
are preserved byte for byte in [the evidence archive](recovery-evidence.tar.gz).

## CFG generation times

| Measurement | Batch wall time | Native call median / p95 | Audited extraction median / p95 |
| --- | ---: | ---: | ---: |
| Direct native audit, 6,377 unique inputs | 101.14 s | 65.77 / 185.03 ms | — |
| Actual DecBench extraction, 8,808 calls | 142.49 s | 51.60 / 141.98 ms | 87.26 / 250.35 ms |

Both batches use eight processes and two Rayon threads per process. Direct
native timing includes C ABI analysis, CPG/CFG construction, Rust JSON encoding
and standard Python JSON decoding; it excludes DecBench preparation and Python
graph objects. The direct batch wall time also includes comparison and writing.
Repeated direct cases share one measurement; totals count unique inputs once.

Actual extraction uses orjson 3.13.0 and independently prepares/parses every
case. Native calls have the same scope, with that faster decoder. End-to-end
includes preparation, Python graph construction and the view-proof overhead.
Graph materialization and proof are nested in Python time and must not be added
again. CSV rows record all stages. Measurements are per translation unit under
concurrent load, not per-function or single-thread microbenchmarks. The different
input weighting and workloads do not establish a direct-batch speed ratio.

Final comparison took 50.24 seconds; DWARF ownership auditing took 214.68 seconds.
These are validation intervals, not CFG generation times. Original cached CFG
generation times remain null, so no aggregate old/new speed ratio is claimed.
The separately recorded decoder experiment reduced warm, complete Python
extraction by 18.7–20.1% on three representative round-07 inputs with identical
objects and graphs; it is not a full-corpus paired benchmark.

## Build and focused validation

The core uses only serde, serde_json and Rayon. There is no JVM, external parser,
Tree-sitter or graph database at runtime. Nine Rust files and a thin ctypes
wrapper implement CFG, per-function CPG, optional DDG and reaching-definition
sets, with in-memory Python access and JSON/DOT files. The rewrite removes
2,209 old files; only the license, ignore file and rewritten README remain from
the old tracked checkout.

The final release library and CLI are byte-identical to the audited snapshot.
**132 Rust tests and 84 Python tests pass**, along with Clippy and formatting.
C/C++ JSON and CFG/DDG/CPG DOT exports pass smoke checks. Standard JSON fallback
also passes native graph, data-flow and strict-error checks. The Rust data-flow
tests compare all **60 original fixture methods**, without skipped methods,
including raw CFG/reference/DDG edges, displayed DDG and reaching-definition sets.

The initial port targets preprocessed C/C++ and sanitized decompiler output used
by DecBench. Full CDT compiler/type behavior, the full Joern CPG schema and
interprocedural data flow are outside this certification. Corpus-wide parity
here concerns CFGs; supported intraprocedural DDG/RD has separate fixture parity.

## Evidence and reproduction

- [Summary](summary.json), [all per-file timings](files.csv), [per-file comparisons](files.jsonl.gz)
  and [timing definitions/distributions](timings.json).
- [Actual usage proof](usage-summary.json), [raw actual comparison](actual-summary.json)
  and [raw direct comparison](direct-summary.json).
- [Ownership summary](ownership-summary.json) and [all 60,704 comparisons](ownership.json.gz).
- [Original input manifest](manifest.json.gz), [oracle captures](oracle-captures.json.gz),
  [direct run configuration](direct-run.json.gz), [actual run configuration](usage-run.json.gz)
  and [candidate output hashes](candidate-artifacts.jsonl.gz).
- [Build provenance](build-provenance.json), [runtime source snapshot](runtime-source.tar.gz),
  [root artifact checks](root-artifact-verification.json), [Rust log](rust-tests.log),
  [Python validation](python-validation.json), [Python log](python-tests.log),
  [decoder fallback](decoder-fallback.json) and [paired decoder experiment](decoder-benchmark.json).
- [Artifact index](artifact-index.json), [verification](verification.json) and [SHA256SUMS](SHA256SUMS).

Large full-corpus candidate/reference graphs and the complete function comparison
matrix remain under ignored `workspace/parity-rounds/round-08`; the index records
their locations and hashes. The bounded bundle preserves every case result,
every ownership result, the final source, and all artifacts required to inspect
the five exceptions. It does not duplicate the full corpus graph store.

Commands for inventory/capture are in the [project README](../../README.md).
To reproduce the final run in this workspace, install the wrapper and parity
extra in an isolated environment, with orjson 3.13.0 for identical decoding.
The original oracle interpreter and installed PyJoern are left unchanged.

```sh
PY=/home/mahaloz/.virtualenvs/decbench/bin/python
R=workspace/parity-rounds/round-08
export PYTHONPATH="$PWD/workspace/profiling/decoder/site:$PWD/workspace/decbench-parity/tools:$PWD/python:$PWD/scripts"
$PY scripts/check_decbench_usage.py --manifest workspace/decbench-parity/manifest.json --build-provenance "$R/build-provenance.json" --library "$R/librust_joern.so" --output workspace/reproduction-usage --workers 8 --rayon-threads 2
$PY scripts/compare_decbench.py --manifest workspace/decbench-parity/manifest.json --candidates workspace/reproduction-usage --output workspace/reproduction-comparison --workers 8
$PY scripts/check_source_ownership.py --manifest workspace/decbench-parity/manifest.json --candidates workspace/reproduction-usage --dataset /home/mahaloz/github/decbench-dataset --build-provenance "$R/build-provenance.json" --library "$R/librust_joern.so" --output workspace/reproduction-ownership.json
$PY scripts/compare_recovered_references.py --recovery-manifest workspace/reference-recaptures/raw-recovery-manifest.json --direct "$R/direct" --output workspace/reproduction-recovery.json
```

Exact original Joern source: v4.0.150,
`958fdd3d976197a783f8ade1254b43e977648f28`. Core source SHA-256:
`e492f0dd2b4c8dba279c523a1f7c3ba289090d54e91d0e9e5a140d1bf060be53`.
Native library SHA-256:
`ae7bdb3584b8b36fdfd2ccb2b6e92dcfacd3acb4fc192df044effeebe71ff7b2`.
