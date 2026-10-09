The first ten fixtures contain 172 C/C++ methods. Their references were captured
with the unchanged installed PyJoern 4.0.150.4 and Joern v4.0.150, using the
same source bytes for both parsers. Reference files and source files retain
their original hashes; the tests verify both `source_sha256` and
`prepared_sha256` before parsing in strict mode.

The capture script, original raw AST/CFG exports, source provenance, hashes,
and whole-file comparisons are in `workspace/ged-reeval/probes` of the main
checkout. `capture_original.py` checks the installed versions and module path,
uses an isolated working directory, and refuses to overwrite an oracle. The
original extractor flags are recorded in each reference.

| Independent capture | Recovery or valid syntax established by the original |
| --- | --- |
| `malformed_format_literals` | Invalid literal/identifier adjacency removes its statement, return, or declaration. Invalid conditions remain `UNKNOWN`; invalid scalar bodies remove the enclosing control. Valid adjacent strings, escaped quotes, addition, and modulo remain executable. |
| `colon_grouped_expressions` | An unpaired colon in an expression group removes its statement container. Ternary colons, including nested and GNU omitted-middle forms, remain valid. |
| `problem_expression_boundaries` | Literal adjacency distinguishes actual casts and `sizeof`, C++ string suffixes, bit-fields inside type braces, and GNU assembly operand colons. |
| `additional_saved_syntax` | Ellipsis operands, `#` names, adjacent argument identifiers, fabricated names, and malformed array-before-name declarations follow CDT problem recovery. A detached identifier after a numeric expression survives as a separate AST sibling. |
| `saved_problem_operand_controls` | The same operand rules apply across condition, return, initializer, scalar-body and block contexts. Positive controls preserve GNU range designators, variadic function-pointer casts, C++ allocation/deletion and alternative operators. Numeric-width assignments retain both their assignment and detached identifier, while returns and call arguments using that syntax are problems. |

`test_saved_expression_recovery.py` compares complete function coverage,
directed CFG topology, entry/exit roles, and degeneracy. It also asserts that
the recovered numeric-width assignment and identifier remain present in the
CPG; topology alone cannot detect losing those straight-line statements.

The final brace controls are copied byte-for-byte from the unchanged original
captures in `probes/final-bare-brace-boundaries`. Its audit hash is
`c95d2424eb8394b11e5ea21ea1b34c53afe7f54ee4f37de58d0e54849ecc1294`.
They distinguish omitted C brace-valued assignments and returns from C++ outer
assignment/return nodes without a value expression. Invalid comparison operands
have different IF and WHILE recovery boundaries; the tests check method spans,
labels, and preserved nested controls as well as complete CFGs. Valid array
initializers, typed compound literals, GNU compound expressions and escaped JSON
strings remain executable. The whole unchanged SSH/seq evidence is separate in
`probes/final-actual-ssh-seq`; its bundled-CDT traces record exact token positions.

The full prepared Binja and r2dec `O0/base-passwd/update-passwd` artifacts are
separate evidence in the probes directory. They have 48 and 45 exported
methods respectively. `expression_recovery.final_comparison.json` records exact
strict parity for all 93 methods, source hashes, immutable original reference
times, native library/parser hashes, and native analysis elapsed times. The
prepared provenance files link these bytes to the saved DecBench results;
there is no compilation, decompilation, or source repair in this comparison.
Elapsed times cover CFG/CPG parsing and Python result decoding; the original
time additionally includes the original Joern process invocation.

The second-stage controls add missing operands, empty conditions, unpaired
top-level colons, pseudo-control call/block syntax, numeric pointer tags,
missing separators, and malformed literal boundaries. Complete statements,
initializers and returns have different recovery rules from conditions and
scalar control bodies; the tests compare their complete function sets and
control-flow roles, including labels whose targets disappear. Positive
controls retain GNU omitted-middle ternaries, C/C++ comparisons, casts,
aggregate initialization, templates, and escaped or C++ raw strings.

`stage2_controls` and `stage2_container_followups` were independently captured
with `workspace/ged-reeval/probes/original-stage2/capture.py` in Python isolated
mode. Its `audit.json` records unchanged engine, helper, source and output
hashes. Four unchanged prepared saved inputs are also captured there: Claude
`clear_pass` and `test_compress`, and Reko pointer-tag inputs for NuttX and
Shadow. The separate grammar and literal-condition probes retain their raw
AST/CFG exports and capture script under `workspace/ged-reeval/probes`.
The literal-newline controls use the independent scanner-owner capture; their
source hashes are checked without changing that capture's metadata format.

The C condition fixture contains C-valid literal controls. A separate archived
probe puts a C++ raw string in C input and records the different original
scanner recovery; its oracle remains unchanged and is not used as a C-valid
positive control. The C++ condition fixture includes the original raw-string
positive control.

`expression_recovery.stage2_comparison.json` records strict exact parity for
all 461 methods across the first-stage controls, the complete Binja/r2dec
prepared artifacts, the second-stage controls, and four new complete saved
artifacts. It includes per-file elapsed time and parser/library/reference
hashes. CPG assertions additionally verify that missing assignment separators
retain both calls, while the ambiguous bare-call form retains its recovered
local and only the second executable call. This catches straight-line AST
loss that a normalized CFG alone cannot reveal.

The third-stage fixtures use the independently captured original references
in `workspace/ged-reeval/probes/round2-problem-boundaries` and
`round3-operator-boundaries`. Their audits preserve unchanged source, engine,
helper, raw AST/CFG and normalized-reference hashes. Malformed member names
and empty operand groups remove their statement container; conditions remain
opaque problem expressions. Missing call separators preserve the first call
or its recovered grouped declaration, followed by the next statement. Reko
operator suffixes retain a declaration initializer's complete prefix, while
malformed `for` slots recover different following siblings according to the
last problem clause. Valid member accesses, pointer calls, casts, shifts,
C++ alternative operators and ordinary `for` loops remain executable.

`macro_problem_guarded` is a fresh unchanged-original capture with a real
`#if 1` guard. Its audit in `probes/macro-problem-guarded/audit.json` proves
all 66 C/C++ functions have the same graphs as the earlier unguarded original
probes; their source files and references remain separate. This tests the
conditional-source lexer without expanding already prepared compiler macro
metadata. Failed empty-macro expansions retain their surviving extra argument
tokens and provenance. Plain malformed returns disappear; the corresponding
macro-origin problem returns remain `UNKNOWN`. An erased initializer is
reparsed from its exact original statement bytes, preserving the ordinary
annotation call that Joern recovers. CPG assertions verify those differences
in addition to normalized CFGs.

The prefix and postfix matrices retain independent original captures under
`probes/prefix-cast-boundaries` and `probes/postfix-group-boundaries`. Their
audit hashes are respectively
`15a500d093e9e203848bc5d0ad547e384c0fc2162d7130fd8c1ba0fb3179567f` and
`1622a93c9ce1f0e5070fdbffe0171d8130b49da924ce0fb6793b2567329df700`.
The source hashes and engine guards are recorded there and in
`stage4-provenance.json`. Original CDT chooses `(T)++x` as a cast of a prefix
increment even when T has an object binding. Without the following operand,
`(T)++` and `*(T)++` remain postfix expressions, including typedef spellings.
Missing operators inside parentheses remove their statement container, while
bad conditions remain opaque. CPG assertions verify these operators and the
different scalar and braced control boundaries.

The loop and literal matrices were captured independently by the scanner
owner under `probes/round3-loop-literal-boundaries`. An incomplete do statement
crosses the following complete statement before recovery resumes. Missing for
clause separators remove the malformed control and its body. An unclosed
ordinary literal stops at the scanner's unspliced newline: an assignment
survives along with the following statement, while a malformed return crosses
that following statement. Scalar controls lose their parent and retain the
recovered assignment; braced controls preserve their bodies. Positive controls
retain valid loops, object-shadowed typedef comparisons, escaped quotes and
normal unary operators. Both source/reference pairs are byte-identical copies
of the unchanged original captures; no saved input is repaired.

The scalar declaration matrix is copied from the unchanged original captures
in `probes/scalar-declaration-boundaries`. A malformed initializer suffix
retains both its parsed assignment and a detached identifier as AST siblings;
scalar controls disappear while braced controls remain. The separate
`initializer-ast-erratum.json` guards the earlier raw references and corrects
an earlier CPG test assumption: the identifier was already present in those
original ASTs, although CFG normalization omits it. No reference graph was
changed.

The operand matrices in `probes/round4-postfix-bang-boundaries` distinguish
`r6!` from `(r6!+4)`: a bare suffix retains an assignment prefix, while the
malformed grouped operand removes the complete statement or initializer.
A completed top-level expression followed by an ordinary call recovers both
as siblings; the same missing operator in a condition remains one opaque
problem expression. The original C and C++ conditional-call positive has the
same graph shape but different call names, which the CPG checks retain.

The independently captured `probes/round4-shadow-cast-boundaries` matrix
proves that object bindings do not prevent syntactically unambiguous casts to
a single identifier followed by another identifier, a literal, or a prefix
operator, nor an abstract pointer type-id. Ambiguous multiplication, addition,
and parenthesized calls retain their ordinary interpretation for shadowed,
unknown and inferred type names. A real unshadowed typedef retains its cast
interpretation. Declaration recovery keeps the original object binding;
the parser does not remove it to force a cast.

`probes/backtick-empty-cast-boundaries` contains the unchanged scanner and
empty-operand controls. The bundled CDT scanner skips an outside-literal
backtick while preserving quoted/commented contents and original spans;
adjacent literals retain the intervening byte in the combined literal CODE.
An empty cast operand instead removes its statement container. Valid casts,
adjacent strings, unary operators, pointer calls and control bodies remain
executable. The stage-five provenance files record each copied source and
reference hash alongside the independent audit paths.

`probes/terminal-prefix-boundaries` is a fresh capture of the bounded terminal
statement matrix, with complete coverage unaffected by the unrelated missing
goto recovery in the earlier matrix. Before a closing brace, a call,
assignment, increment or declaration without its semicolon survives as a
recovered sibling; its scalar control wrapper disappears. Braced controls
remain, and a missing-semicolon return disappears. The plain grouped call
form recovers a local declaration. The eight independent EOF fixtures are
reused byte-for-byte from `probes/terminal-separator-boundaries`: a completed
call at literal EOF with no method closing brace is omitted, while the
preceding complete method remains. Tests verify the entire method set and
the differing CPG control, local and expression nodes.

The consecutive-name matrix in `probes/consecutive-named-cast-boundaries`
adds ordinary-object and typedef-shadow controls for `(T)(U)x`. Original CDT
produces two nested casts; `(T)(x)` retains an ordinary call outside an
unshadowed typedef. The exact sources, references and independent audit hashes
are recorded in `stage6-cast-provenance.json`.

The next independent matrix in `probes/nonterminal-member-and-memory-shift`
proves nonterminal separator recovery. A call before a member assignment and
a member before a call become compound siblings, removing scalar control
wrappers. A call followed by `*q=x` instead remains one assignment with a
multiplication on its left. A complete memory operand followed by an ordinary
identifier shift assignment becomes two statements; no signedness operator
is invented. Conditions and invalid loop slots retain their independently
captured problem boundaries.

`probes/remaining-rare-expressions` adds partial and empty array subscripts,
assignment followed by a two-name declaration, and valid GNU expression
blocks. Malformed indexes remove their entire statement; preexisting array
allocations remain. A completed assignment followed by two names retains the
assignment and a separate LOCAL declaration. The reference includes the raw
AST, since that LOCAL is invisible in the normalized CFG.

The `probes/malformed-call-terminal-boundaries` sources are preserved whole:
a newline-terminated literal that swallows the call terminator makes its
statement a problem at a compound brace. C and C++ RETURN/initializer recovery
differ: C can consume later headers and retain later definitions as nested
methods. Those C-only coverage and graph differences are explicit diagnostic
exceptions while that rule is being completed; the fixture has no skip or
changed reference. `probes/round5-lexical-format-boundaries` independently
records invalid outside-literal backslashes, UCN/splice positives, and source
spans. Stage-seven provenance files link every copied source/reference byte
to the guarded unchanged-original captures.

Stage eight resolves those four C-only scope exceptions. The preserved
`probes/malformed-call-terminal-boundaries/cdt-c-recovery-evidence.json`
records an unchanged bundled-CDT trace: a failed call argument consumes the
closing compound brace before RETURN or declaration-initializer recovery
skips the following statement. Expression-statement recovery retries from
its mark and leaves that brace in place. The complete C and C++ fixtures now
match their original method sets, CFGs, roles, degeneracy, and affected
method start/end lines. The stage-eight combined proof covers 82 source and
reference pairs and 2,024 methods without skips or rewritten expectations.

Stage nine adds unchanged original captures from
`probes/round6-condition-label-ternary`, `probes/round6-malformed-quoted-calls`,
and `probes/round6-member-separator-boundaries`. Missing member separators
preserve compound siblings and remove invalid scalar control wrappers;
complete calls before labels and cases preserve those targets. Missing
grouped operands produce one whole UNKNOWN condition. An unclosed ternary
literal can swallow its required colon and the next statement's terminator.
Malformed call arguments can fail before the compound brace, so the C
consumed-brace rule applies only when the argument parser reaches that brace.
The quote fixture retains its complete scope fallout and language differences
(52 C methods, 54 C++ methods), rather than assuming every nominal method
survives. `stage9-provenance.json` guards all copied source/reference bytes.
