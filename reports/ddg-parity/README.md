These measurements compare unchanged PyJoern 4.0.150.4 / Joern v4.0.150 with
native Rust output on identical prepared input bytes. The 57 measured files
contain 589 functions selected by the original public `parse_source` API:
221 functions from 11 focused files, 314 from all 40 existing parser/recovery
and DecBench regression sources, and 54 from six additional actual DecBench
compiled and decompiled files. This is a measured corpus, not a claim of parity
for every C/C++ program or every DecBench input.

Two additional directory parses cover cross-file callee context and duplicate
function names in `left/input.c` and `right/input.c`. Their six original public
functions are reported separately in `directory-after.json` and
`directory-paths-after.json`. Directory hashes bind sorted full relative paths
and every file's bytes; function keys preserve `(name, relative filename)`.
This prevents duplicate basenames from overwriting each other. Synthetic
filenames are retained. Absolute candidate filenames are made relative to the
same prepared input root for comparison.

Ten additional `dataflow-audit` inputs are measured separately: four type
metadata/detail sources select 108 original public functions, and six context,
control, C++ call, global and operator sources select 74. Their rich original
CPG captures contain 82 methods in the latter group; public name/CFG selection
accounts for the smaller public scope. `audit-types-after.json` and
`audit-sources-after.json` record both graph gates passing for all 182 selected
functions on one candidate snapshot. These interim audit reports do not change
the 589-function broad-corpus summary or establish a shared final revision with
the directory reports.

Each function must pass two independent graph checks:

- Actual `Function.ddg`: directed graph isomorphism preserving Block boundary
  roles, source address, statement class, displayed text, and every lifted
  statement field. Duplicate statement identities remain separate vertices.
- Original `dotDdg`: directed multigraph isomorphism preserving complete
  decoded node labels and edge labels, including parallel edge multiplicity
  and isolated vertices. Native edge labels remain source text; literal HTML
  entities in C/C++ strings are not decoded a second time.

Counts alone never establish a match. Missing/extra functions, unavailable
projections, statement changes and labeled dependency changes are reported
separately. Errors identify whether preparation, the original parser, the
candidate, or comparison failed. Warnings remain separate from graph failures.

The oracle runs actual installed `pyjoern.parse_source` in an isolated Python
interpreter and temporary working directory. A separate unchanged installed
FastParser call supplies the original DOT and a second public graph; the
comparator requires that the two original public graphs agree. Each timing is
an isolated file parse including JVM startup, not a divided batch estimate.
References record input/prepared hashes, package versions, parse flags, Python
version and hashes of the original parser/lifter files. No native graph creates
a reference. Candidate library and Python sources are copied once at the start
of each command to prevent another worker's concurrent changes from mixing
candidate revisions. Candidate timings include lazy DDG materialization.

Frozen references are under `tests/fixtures/ddg-parity/references`. Top-level
references cover focused sources; `tier2` covers existing regressions;
`decbench` covers authentic external inputs. References bind both original and
prepared bytes. DecBench `.i/.ii` inputs use its original header stripping and
decompiled inputs its original sanitation/macro expansion. Their absolute
source paths and preparation flags are recorded in each reference.
`directory` contains the two frozen multi-file references and input manifests.
`audit-types` and `audit-sources` contain independently captured original
references for the ten additional raw sources.

`summary.json` identifies each report's scope and candidate hashes. Historical
full divergence reports are losslessly compressed as `.json.gz`; read them
with `json.load(gzip.open(path, "rt"))`. Current `*-after.json` files retain
full divergent graphs and semantic edge/node differences for localization.
An intermediate summary can contain different candidate revisions and must
not be treated as a single final validation. The `all_groups_same_candidate_revision`
field records this explicitly. The initial native API matched none of the 589
original public/labeled DDGs, because it exposed raw CPG nodes instead of the
original lifted projection.

To replay the focused corpus from frozen references:

```sh
cargo build --lib
python3 scripts/compare_ddg_pyjoern.py \
  tests/fixtures/control.c tests/fixtures/expressions.c tests/fixtures/functions.cpp \
  tests/fixtures/dataflow/flow.c tests/fixtures/dataflow/extra.cpp \
  tests/fixtures/dataflow/semantics.c tests/fixtures/dataflow/builtins.c \
  tests/fixtures/decbench/bits.c tests/fixtures/decbench/libgzip_a-stripslash.c \
  tests/fixtures/ddg-parity/patterns.c tests/fixtures/ddg-parity/patterns.cpp \
  --reference-dir tests/fixtures/ddg-parity/references \
  --output reports/ddg-parity/after.json --jobs 2
```

To replay existing regression sources, pass `tests/fixtures/parser-recovery/*.c`,
`tests/fixtures/parser-recovery/*.cpp`, `tests/fixtures/decbench-regressions/*.c`,
and `tests/fixtures/decbench-regressions/*.cpp` with the `references/tier2`
directory. To capture new original references, replace `--reference-dir` with
`--save-reference-dir` and specify `--reference-python` for an isolated
environment containing the original package. External DecBench inputs also
need `--decbench`; decompiled inputs additionally need `--sanitize-decompiled`.

Replay the four type sources using `tests/fixtures/dataflow-audit/type-*.c` and
`tests/fixtures/dataflow-audit/type-*.cpp` with `references/audit-types`. Replay
the other six audit sources with `references/audit-sources`. Frozen-reference
replay requires the native library and Python graph dependencies; it does not
start Joern.

```sh
python3 -m unittest discover -s tests -p test_ddg_comparison.py -v
```

These tests check identity-sensitive topology, directed/parallel labeled
edges, allocation-independent relabeling, duplicate statements, source and
oracle provenance, and agreement of independently captured original graphs.
Ten tests also parse the additional audit sources through the native API and
require exact public and labeled DDG parity, independent of original CPG replay.
They freeze candidate bytes once per class and skip only when the debug library
has not been built.
