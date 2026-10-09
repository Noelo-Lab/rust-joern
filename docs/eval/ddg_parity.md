# DDG parity evaluation

The DDG audit recorded on **2026-10-08** matched unchanged **PyJoern
4.0.150.4 / Joern v4.0.150** for all **841 public function instances** across
77 file parses and two directory parses. Every function passed both the public
DDG and labeled DOT checks described below; none failed. The original native
baseline matched zero functions.

This is a historical result for production checkpoint
`58f24d6dcb6276d2b4c2c3cfbbd15b0536c266c5`. The final measurements were committed
as `45c438668` on 2026-10-08. They predate subsequent CFG and parser changes,
including the [CFG reevaluation](cfg_parity.md); replaying the current build is
necessary to establish its DDG result.

## What was compared

The evaluation used [compare_ddg_pyjoern.py](../../scripts/compare_ddg_pyjoern.py)
to compare two independently checked graph views for each selected public
function:

1. **Public `Function.ddg`.** Directed graph isomorphism preserves each Block's
   source address, entry and exit roles, ordered statements, statement classes,
   displayed text, and observable lifted fields. Those fields include source
   lines, raw text, operands, arguments, types, and return values. Statement IDs
   and duplicate-line Block indices are allocation details and are ignored;
   separate statements with identical attributes remain separate vertices.
2. **Original `dotDdg` projection.** Directed multigraph isomorphism preserves
   complete node labels, edge labels, edge direction, parallel edge
   multiplicity, and isolated vertices. The native side builds this projection
   from its DDG view and Joern-style labels. DOT HTML entities are decoded once;
   native edge labels remain source text, so a literal `"&lt;"` in C/C++ is not
   decoded again.

Graph counts and counters of statement-to-statement edges are diagnostics,
not the acceptance test. Isomorphism must also preserve topology when multiple
nodes have the same identity. Missing and extra functions, unavailable
projections, changed statement semantics, changed labeled dependencies, and
public API differences are reported separately. Preparation, oracle parsing,
native parsing, and comparison errors have distinct statuses; warnings are
separate from graph failures. An original parse selecting no public functions
does not count as a pass.

The public DDG is a NetworkX `DiGraph` of lifted Blocks. The native
`Function.ddg_raw` is a richer `MultiDiGraph` of CPG nodes and `REACHING_DEF`
edges. This audit establishes the public and DOT contracts above, rather than
equality of every raw CPG property or every reaching-definition solver result.

## Oracle, preparation, and provenance

The oracle ran the installed original `pyjoern.parse_source` in an isolated
Python interpreter and temporary working directory, with these flags:

```python
no_metadata=True, no_cfg=False, no_ddg=False, no_ast=True
```

A second unchanged FastParser parse supplied the original DOT and a second
public DDG. The comparator first required agreement between the two original
public graphs. Native graphs were never used to construct oracle references.

Both implementations received identical prepared bytes. Repository fixtures
were parsed unchanged. External DecBench `.i` and `.ii` inputs used DecBench's
system-header stripping; decompiled inputs used its sanitation and macro
expansion. The external examples came from DecBench's
`full_run_address_2026-09-11` results:

- Compiled: O0/bash `mbschr.i`, O0/nuttx `libxx_dynamic_cast.ii` and
  `libxx_typeinfo.ii`.
- Decompiled: O0/coreutils `ida_libstdbuf.c`, O0/shadow `kuna_groups.c`, and
  O2/sysvinit `ida_fstab-decode.c`.

The retained [frozen references](../../tests/fixtures/ddg-parity/references)
record source and prepared SHA-256 hashes, source paths, preparation flags,
language, parse flags, package versions, Python version, and hashes of the
original parser and lifter files. The recorded environment used Python 3.14.6,
NetworkX 3.6.1, PyGraphviz 2.0, and cfgutils 1.16.0. The comparator validates
the reference's provenance and both input hashes before comparison.

For directories, hashes bind the sorted full relative paths and each file's
bytes. Functions are keyed by `(name, relative filename)` instead of basename;
this keeps `left/input.c` and `right/input.c` distinct. Absolute native paths
are made relative to the prepared root, and synthetic filenames are retained.

The library and Python package were frozen once for the final run, and every
group used the same candidate bytes:

| Artifact | SHA-256 |
| --- | --- |
| Native library | `bbcc9029052314789b11ced7f495c0d63841112f59ae2ca46a569b739a1c76a6` |
| `rust_joern/__init__.py` | `7f547fb91adef18ea392c7ec0d81c214ae2c88a8fa1b609a253cd1ceb512a0ce` |

The historical summary and input manifest were consolidated into this document
for release. Their original records remain in git history at `45c438668`;
oracle captures needed by the retained comparison tests remain under
`tests/fixtures`.

## Recorded results

| Group | Parses | Public functions | Matched | Failed |
| --- | ---: | ---: | ---: | ---: |
| Focused C/C++ sources | 11 files | 221 | 221 | 0 |
| Existing parser/recovery and DecBench regressions | 40 files | 314 | 314 | 0 |
| External DecBench compiled inputs | 3 files | 48 | 48 | 0 |
| External DecBench decompiled inputs | 3 files | 6 | 6 | 0 |
| Type metadata and type detail | 4 files | 108 | 108 | 0 |
| Context, control, calls, globals, and operators | 6 files | 74 | 74 | 0 |
| Cross-file callee context | 1 directory | 2 | 2 | 0 |
| Duplicate basenames and function names | 1 directory | 4 | 4 | 0 |
| Comma conditions | 1 file | 6 | 6 | 0 |
| Global macro discovery | 1 file | 3 | 3 | 0 |
| C++ classes and namespaces | 2 files | 10 | 10 | 0 |
| C++ lambdas, fields, and member pointers | 4 files | 26 | 26 | 0 |
| Assignment operators and construction | 2 files | 19 | 19 | 0 |
| **Total** | **77 files + 2 directories** | **841** | **841** | **0** |

All requested parses and graph comparisons completed without preparation,
parser, candidate, or comparison errors. The original broad corpus contributed
589 instances: 221 focused functions, 314 regression functions, and 54 external
DecBench functions. The directory and additional audit groups expanded it to
841.

Counts are per-input public function instances. Two regression fixture pairs
have identical prepared bytes and language: `cdt_builtin_controls_c.c` and
`cdt_builtin_controls.c`, plus their C++ counterparts. Each pair repeats 21
functions, giving 42 repeated instances. After accounting for these pairs,
there are **77 unique prepared input contexts containing 799 functions**.

Public selection also limits the audit scope. The six context/control/call/
global/operator captures contain 82 original CPG methods but select 74 public
functions. The assignment/construction captures contain 21 methods but select
19; two declarations are excluded. The results describe those selected
functions and inputs, not all C/C++ programs or the full DecBench corpus.

Recorded validation at that checkpoint passed 115 Rust tests, 96 Python
discovery tests, 26 DDG comparator/source tests, and
`cargo clippy --all-targets -- -D warnings`. Oracle timings were isolated parses
including JVM startup, with the supplemental parse measured separately.
Native timings included lazy public DDG materialization. They were not divided
batch estimates or a DDG-only performance benchmark.

## Replaying the checks

Run these commands from the repository root. The comparator exits successfully
only when every requested input matches. Frozen-reference replay needs a built
native library and Python graph dependencies; it does not start Joern.
Build the library and install the wrapper if needed:

```sh
cargo build --lib
python3 -m pip install -e .
```

Replay the 11 focused files:

```sh
python3 scripts/compare_ddg_pyjoern.py \
  tests/fixtures/control.c tests/fixtures/expressions.c tests/fixtures/functions.cpp \
  tests/fixtures/dataflow/flow.c tests/fixtures/dataflow/extra.cpp \
  tests/fixtures/dataflow/semantics.c tests/fixtures/dataflow/builtins.c \
  tests/fixtures/decbench/bits.c tests/fixtures/decbench/libgzip_a-stripslash.c \
  tests/fixtures/ddg-parity/patterns.c tests/fixtures/ddg-parity/patterns.cpp \
  --reference-dir tests/fixtures/ddg-parity/references \
  --output workspace/ddg-focused.json --jobs 2
```

Replay the directory cases and the four type audit files:

```sh
python3 scripts/compare_ddg_pyjoern.py \
  tests/fixtures/ddg-directory tests/fixtures/ddg-parity/directory_paths \
  --reference-dir tests/fixtures/ddg-parity/references/directory \
  --output workspace/ddg-directory.json

python3 scripts/compare_ddg_pyjoern.py \
  tests/fixtures/dataflow-audit/type-*.c tests/fixtures/dataflow-audit/type-*.cpp \
  --reference-dir tests/fixtures/ddg-parity/references/audit-types \
  --output workspace/ddg-types.json
```

Use the same command with the following source and reference groups for the
other repository fixtures:

| Sources | Reference directory under `tests/fixtures/` |
| --- | --- |
| Captured files from `parser-recovery/` and `decbench-regressions/` | `ddg-parity/references/tier2` |
| `dataflow-audit/{context.c,context.cpp,control.c,cpp_calls.cpp,globals.cpp,operators.c}` | `ddg-parity/references/audit-sources` |
| `dataflow-audit/{assignment-operators.c,constructors.cpp}` | `ddg-parity/references/audit-extensions` |
| `ddg-parity/{comma-conditions.c,global-macros.c}` | `ddg-parity/references` |
| `ddg-classes/{constructors.cpp,using-namespace.cpp}` | `ddg-classes/references` |
| `ddg-lambdas/{lambdas.cpp,fields.cpp,method-pointers.cpp,namespaced-fields.cpp}` | `ddg-lambdas/references` |

The regression directories have grown since the 40-file audit. Select files
whose basenames have a matching `*.ddg.pyjoern.json` reference in `tier2`;
passing every current source glob would include uncaptured files. The
comparator requires unique source basenames within a frozen-reference command.
Each invocation snapshots candidate bytes; when comparing several groups,
keep the build fixed and check that their recorded candidate hashes agree.

For external DecBench replay, use the source paths recorded in
`references/decbench/*.ddg.pyjoern.json`, point `--decbench` at that checkout,
and set `--reference-python` to an interpreter with its preparation
dependencies. Add `--sanitize-decompiled` for the decompiled group. These
external source files are not bundled in this repository, and replay still
requires their original bytes.

To capture new oracle references, replace `--reference-dir` with
`--save-reference-dir` and supply `--reference-python` for an isolated
environment containing unchanged PyJoern 4.0.150.4 / Joern v4.0.150. This mode
starts the original parser and requires its Joern/JVM dependencies.

The retained [DDG comparison tests](../../tests/test_ddg_comparison.py) check
identity-sensitive topology, direction, parallel labels, isolated vertices,
allocation-independent relabeling, provenance, and agreement of the two
original captures. Twelve tests also parse the audit sources through the
native API and require both graph checks:

```sh
python3 -m unittest discover -s tests -p test_ddg_comparison.py -v
```

The native source tests snapshot the candidate once per class and skip if the
debug library has not been built. Their current result should be reported
separately from the historical 841-function audit.
