# Bounded C/C++ call AST port

This dependent change ports call-operand placement against Joern 4.0.150, not
CDT binding resolution. Named, pointer/closure, resolved member and unresolved
member/expression calls have separate layouts. Explicit `virtual` declarations
supply bounded receiver evidence. Incomplete lookup on a known owner keeps the
existing path. Generated intrinsic identities are retained. Receiver effects
are evaluated once before explicit arguments.

The handwritten fixture contracts in `tests/fixtures/joern-calls` compare call
identity, source location, argument indices, receiver identity and direct AST
ownership against frozen original Joern captures. They do not establish full
CPG, type, DDG or execution equivalence. Source and capture provenance and the
original port fragments are documented beside the fixtures.

## Dependent-branch replay, 2026-10-29

The baseline includes the changes in upstream #3, #6 and the separate explicit
destructor candidate. This replay uses 1,225 overlapping prepared C++ inputs,
not 1,225 independent programs, and is not a benchmark score. Both versions
parsed 1,165 inputs, rejected 26 and found no methods in 34. No existing parse
was lost. In both default and Joern-loop modes, full analyses changed for 859 inputs: the bounded
delta audit found 6,267 previously absent call identities and 190 receiver
edges sharing an existing argument-zero AST root, with zero unexplained changes.
The audit compares normalized CFGs and the remaining full serialized analysis;
it does not prove semantic correctness or original Joern equivalence on the
whole corpus. Raw source captures and detailed results are kept locally.

Current local checks: all-target Rust tests, formatting and Clippy passed.
Python unittest ran 170 tests with 16 skips due to missing local DecBench or
the platform-specific native test library. The frozen original C/C++ call
contract tests were among the tests executed successfully.
