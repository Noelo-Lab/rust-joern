# DDG parity evaluation

The DecBench corpus audit recorded on **2026-11-09** compared every function of
every saved DecBench input with unchanged **PyJoern 4.0.150.4 / Joern
v4.0.150**: **19,316 cases** covering **12,722 unique prepared inputs**, from
preprocessed sources and seventeen decompilers.

- **3,451,219 of 3,455,910** compared function instances (99.86%) equal the
  frozen original export exactly, in both the public `Function.ddg` and the
  labeled DOT projection.
- The other 4,691 match no single frozen export. Joern's export varies between
  runs of the same bytes, so a frozen export cannot decide them. A replay of
  225 of their 1,493 inputs against one more original run decided 709 of them:
  610 (86%) differ from Joern only in that run-dependent order or equal the
  new run outright, and 99 (14%) are real differences, described under
  [Remaining differences](#remaining-differences).
- Original PyJoern fails on two unique inputs (six cases), where Rust Joern
  returns 884 function graphs.

This describes release library SHA-256
`27c2261e6ee3b61a95b8b757f309b4b3a5674da70569cd1e94dc3f32d90bb60d`. A changed
build needs its own run.

## Corpus and comparison method

The corpus was DecBench's `full_run_address_2026-09-11`: every compiled `.i`
and `.ii` file and every decompiled `.c` file under O0, O2 and O2-noinline, for
40 projects. Source preparation used DecBench's unchanged header stripping;
decompiled inputs used its sanitation and local preprocessing, without the
loop-head rewrite. Inputs are identified by language and prepared SHA-256, which
reduces 11,801 source and 7,515 decompiled cases to 5,403 and 7,319 unique
inputs.

[ddg_corpus.py](../../scripts/ddg_corpus.py) ran the original parser once for
each unique input, in an isolated interpreter and working directory, as
`pyjoern.parse_source` with:

```python
no_metadata=True, no_cfg=False, no_ddg=False, no_ast=True
```

Each reference keeps the raw DOT, the parsed labeled graph, the public graph,
package versions and hashes of the original parser and lifter files. To bound
JVM startup cost the captures ran with
`-XX:ActiveProcessorCount=2 -XX:TieredStopAtLevel=1`. Those flags do not change
graphs, but deep recursion overflows the stack sooner without the optimizing
compiler: nine inputs failed under them. All 130 inputs without functions were
recaptured with default JVM options. Seven of the nine then succeeded and those
captures became their references; the 121 empty results did not change.

The candidate was one frozen release library, parsing the same prepared bytes
as DecBench does, with `preprocessed=True`. Parsing was permissive so that
every function is compared; no input produced an error diagnostic, which is
what DecBench's `strict=True` rejects. The comparison requests Joern's behavior
for the three [intentional divergences](../divergences.md):
`joern_loop_conditions`, `joern_knr_parameters` and `joern_cdt_failures`.

A function matches when both of these hold:

1. **Public `Function.ddg`.** Directed graph isomorphism over lifted Blocks,
   with the identity described under
   [the focused audit](#what-was-compared).
2. **Original `dotDdg` projection.** Directed multigraph isomorphism over
   complete node labels, edge labels, direction and parallel edge multiplicity.

Isomorphism is exact: colour refinement decides most functions and a VF2
search decides the rest. The search is limited to 20 seconds of processor time,
after which a function is reported as undecided, not as a match; none reached
the limit. A function present on one side only is missing or extra.

## Joern's export varies between runs

`dotDdg` follows hidden definitions through a cache with a visited-path cut, so
its edges depend on the stored order of each node's incoming `REACHING_DEF`
edges. `ReachingDefPass` creates those edges while iterating hash collections
keyed by node objects, and a node's hash includes the JVM identity hash of its
graph. Part of the order therefore differs between two parses of the same
bytes: the order in which call sites are visited, the arguments of a call with
more than four, and the definitions reaching a returned expression. The
reaching-definition edges themselves do not vary.

Four independent original captures of seven decompiled inputs disagreed with
each other on 2 to 5 percent of their functions. A frozen export is one sample
of such a function, and Rust Joern
[fixes the order](../divergences.md#run-dependent-ddg-export-order-is-fixed)
instead of sampling it.

[replay_ddg_projection.py](../../scripts/replay_ddg_projection.py) decides
these functions from one more original run. It captures each method's raw graph
with its stored edge order and that run's DOT, then checks that

- the native reaching-definition edges equal the original's, as a labeled
  multiset under an AST-position node correspondence, and
- the native projection code, given the original's own graph and stored order,
  reproduces the original DOT.

A function that passes both differs from Joern only in the stored order.

## Recorded results

Counts are per case; a function of an input shared by several cases is counted
once for each.

| Input | Cases | Matched | Unmatched | Extra | Missing |
| --- | ---: | ---: | ---: | ---: | ---: |
| Preprocessed source | 11,801 | 6,040,227 | 392 | 2,652 | 0 |
| angr | 770 | 92,292 | 1,110 | 0 | 1 |
| Kuna | 770 | 92,214 | 1,115 | 0 | 0 |
| IDA | 770 | 91,959 | 509 | 0 | 0 |
| Ghidra | 770 | 86,690 | 272 | 7 | 0 |
| Binary Ninja | 770 | 82,308 | 1,214 | 2 | 2 |
| r2dec | 770 | 70,069 | 137 | 0 | 0 |
| codex@gpt-6-astra | 257 | 61,332 | 153 | 3 | 7 |
| dewolf | 770 | 16,127 | 15 | 12 | 2 |
| glaurung | 223 | 1,936 | 35 | 0 | 0 |
| claude-code | 223 | 1,319 | 1 | 0 | 0 |
| codex | 223 | 1,118 | 2 | 1 | 0 |
| psalms | 222 | 259 | 0 | 0 | 0 |
| fission | 223 | 247 | 2 | 0 | 0 |
| retdec | 223 | 229 | 5 | 0 | 0 |
| ventris | 166 | 163 | 3 | 0 | 2 |
| reko | 223 | 140 | 17 | 0 | 0 |
| manifold | 142 | 134 | 8 | 0 | 0 |
| **Total** | **19,316** | **6,638,763** | **4,990** | **2,677** | **14** |

Over unique inputs the same run has 3,451,219 matched, 4,691 unmatched, 909
extra and 14 missing function instances. 11,098 inputs match in every function,
1,493 have at least one unmatched function and eight more differ only in an
extra or missing function.

2,652 of the extra case-weighted functions are the two inputs on which the
original fails: Cleanflight and Crazyflie `arm_common_tables.i`, whose
recursive export overflows the stack at every optimization level. The original
returns no functions for 121 further inputs, mostly failed dewolf and reko
output; Rust Joern returns none for them either.

### Replay of unmatched functions

`ddg_corpus.py replay` selected 225 of the 1,493 inputs with unmatched
functions by a hash of the input key and checked their 709 unmatched functions
against one more original run of each input.

| Input | Functions | Equal to the new run | Stored order only | Edges differ |
| --- | ---: | ---: | ---: | ---: |
| Binary Ninja | 221 | 71 | 121 | 29 |
| angr | 149 | 61 | 76 | 12 |
| Kuna | 139 | 53 | 86 | 0 |
| IDA | 76 | 30 | 45 | 1 |
| Ghidra | 36 | 10 | 22 | 4 |
| codex@gpt-6-astra | 32 | 1 | 0 | 31 |
| r2dec | 24 | 6 | 9 | 9 |
| Preprocessed source | 16 | 5 | 3 | 8 |
| glaurung | 8 | 2 | 6 | 0 |
| manifold | 3 | 2 | 1 | 0 |
| reko | 3 | 0 | 0 | 3 |
| dewolf | 2 | 0 | 0 | 2 |
| **Total** | **709** | **241** | **369** | **99** |

- **Equal to the new run.** The new original export equals Rust Joern's, so
  the two original exports of that function disagree with each other.
- **Stored order only.** The reaching-definition edges are equal, and the
  projection code reproduces the new original DOT from the original's own
  graph and stored order.
- **Edges differ.** The reaching-definition edges differ. These are real
  differences.

The other 1,268 inputs with unmatched functions were not replayed. If the
sample's rate holds for them, about 650 of the 4,691 unmatched functions are
real differences, 0.02% of the corpus. That is an estimate, not a count.

### Default options

DecBench uses the defaults, not the three Joern-reference options. The same
library's default DDG was compared with its Joern-reference DDG for every
function of the corpus, over unique inputs. Each difference was attributed by
switching one option off at a time.

| Functions present with both option sets | 3,457,863 |
| --- | ---: |
| Same DDG | 3,408,788 |
| Different DDG | 49,075 |
| Changed by `joern_loop_conditions` | 45,429 |
| Changed by `joern_knr_parameters` | 611 |
| Changed by `joern_cdt_failures` | 3,780 |
| Changed by more than one option | 745 |
| Changed by no single option | 0 |

98.58% of the default DDGs are the Joern-reference DDGs, and every other one
differs through a [documented divergence](../divergences.md). The option sets
also differ in which methods exist: 610 exist only by default, in r2dec and
codex output, and 52 only with the Joern options, such as a method named
`int64_t` that a CDT failure creates. A method that moves to another line
counts in both.

The default CFG was also compared with the build before this evaluation, by
directed topology with entry and exit roles, in DecBench's strict mode. Of
3,457,377 functions, 81 changed and six one-node prototype stubs disappeared,
all in r2dec, Binary Ninja and codex output. 72 of the 81 gained statements
that the earlier build dropped and nine kept their size; none lost a node. No
input fails strict parsing. DecBench's GED cache key omits the Rust Joern
build, so its `cache_version` needs a bump for those functions.

## Remaining differences

These are recorded, not waived. The 99 replayed functions whose edges differ,
and the label differences visible in the full comparison, have these causes.
Counts marked *sample* cover the 225 replayed inputs only; the others are
function instances over all unique inputs.

- **Calls through function pointers** (15, sample). Codex output calls
  `gnutls_free(pem)` and also declares `extern void (*gnutls_free)(void *ptr);`.
  Joern makes a static call and Rust Joern a pointer call, in 13 functions of
  one input. For Binary Ninja's local `int64_t (* const rax_6)();` it is the
  other way round, in two functions.
- **Function-pointer and array-pointer declarations with initializers** (about
  15, sample). For Binary Ninja's `char* (* rax)(char* arg1) = sub_4201b0;`
  Rust Joern adds an assignment that Joern lacks. For
  `char const (** const i)[0x9] = &data_807dac0;` Joern has an
  `<operator>.alloc` node that Rust Joern lacks. The same forms occur in
  preprocessed sources: `explicit_bzero`'s `memset_v` and diff3's `*using[2]`.
- **r2dec's unterminated string literals** (9, sample). After
  `rax = "LS_COLORS=';` CDT continues the expression on the next line, so
  Joern has the single assignment `rax = "LS_COLORS='; *(0x0000d138) = rdx`.
  Rust Joern keeps two statements.
- **Address operands** (10, sample). Joern keeps nodes for angr's `&&v0` and
  Ghidra's `&switchD_001027fc` that Rust Joern drops.
- **Codex firmware output** (about 12, sample). Local declarations that CDT
  reads as products, such as `List_t *pxTemp;`, and casts of integer
  addresses.
- **Preprocessed sources** (8, sample). Besides the declarations above:
  function-pointer arguments such as
  `print_script (script, find_change, print_sdiff_script)` and parameter
  edges in NuttX `strncmp`.
- **Return types** (20). Joern displays `char (*f(int))[16]` as `char(*)[]`,
  a block-scope `extern union node *f(int);` as `unionnode*` and
  `struct Elf32_Rel* const f()` as `Elf32_Rel*const`.
- **Extra and missing functions** (25 and 14, outside the two inputs where
  the original fails). They surround declarations that CDT recovers from in
  its own way: Joern has a method named `int64_t` in four diffutils inputs,
  and six prototype stubs that codex first declares as
  `extern VL53L1_preset_initializer_t sub_8020458;`. Rust Joern has
  `load_user`, `makejob` and `key_init` in Ghidra output where Joern does
  not.
- **Single statements** (the rest of the sample). Binary Ninja's
  `rax_1 = cond:0;` and `x = a <= b ? {0} : c;`, dewolf's `__noreturn`
  declarations and reko output.

## Reproducing the corpus run

Set `DECBENCH` to a DecBench checkout, `RESULTS` to its saved
`full_run_address_2026-09-11` directory and `ORACLE_PYTHON` to an interpreter
with unchanged PyJoern 4.0.150.4. These inputs are not shipped here. The
capture starts one Joern process per unique input and took 73 hours of summed
parse time. `compare` starts with the largest inputs and peaked near 130 GB of
memory with 30 workers, so run the stages one at a time.

```sh
cargo build --release
python scripts/ddg_corpus.py inventory --results "$RESULTS" --decbench "$DECBENCH" \
  --output workspace/ddg-decbench --workers 32
python scripts/ddg_corpus.py capture --manifest workspace/ddg-decbench/manifest.json \
  --reference-python "$ORACLE_PYTHON" --workers 32
python scripts/ddg_corpus.py compare --manifest workspace/ddg-decbench/manifest.json \
  --output workspace/ddg-decbench/compare --workers 32
python scripts/ddg_corpus.py replay --manifest workspace/ddg-decbench/manifest.json \
  --compare workspace/ddg-decbench/compare --reference-python "$ORACLE_PYTHON" \
  --output workspace/ddg-decbench/replay --workers 12 --fraction 0.17 --seed 2
python scripts/ddg_corpus.py defaults --manifest workspace/ddg-decbench/manifest.json \
  --output workspace/ddg-decbench/defaults --workers 12
```

`capture --jvm-options ""` uses default JVM options, and `--round NAME` stores
an additional independent capture that `compare --rounds NAME` accepts as an
equally valid original export. Reductions of each difference fixed during this
evaluation are frozen with their original references under
[ddg-decbench](../../tests/fixtures/ddg-decbench); the DDG comparison tests
replay them without Joern.

## Focused audit: 2026-10-08

An earlier audit matched all **841 public function instances** of 77 file
parses and two directory parses, at production checkpoint
`58f24d6dcb6276d2b4c2c3cfbbd15b0536c266c5`. Its 787 in-repository functions
still match with the library above. It defines the comparison identity used by
the corpus run.

### What was compared

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

### Oracle, preparation, and provenance

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

### Recorded results

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

### Replaying the checks

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
