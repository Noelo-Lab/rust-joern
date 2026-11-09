# C++ qualified expression controls

Six header-free C++17 inputs cover template-qualified statement/return/nested
calls, an explicitly qualified member `operator[]`, and conditional/short-circuit
and loop/break control around those calls. They were accepted by Clang 20 and
22 using `-fsyntax-only`. They do not execute or require linking.

The `.pyjoern.json` files were captured with unchanged installed PyJoern
4.0.150.4 / Joern v4.0.150 in separate interpreters and temporary working
directories with `PYTHONPATH` removed. Original parse flags, source hashes and
reference hashes are recorded in `provenance.json`. The native candidate was
not used to generate expected graphs.

Comparison checks full selected function coverage (17 functions, including
prototypes), directed topology, entry/exit roles and degeneracy. Statement
attribute/effect validation is separate: the Rust integration tests verify
call identities, arguments, receiver evaluation and CFG ordering. Topology
agreement does not certify full expression semantics or C++ validity of
arbitrary producer output.

The 2026-10-22 follow-up adds two header-free controls for an explicit operator
template return call and a comma/short-circuit loop condition. Apple Clang 17
and Homebrew Clang 22 accept both with
`-std=c++17 -pedantic-errors -fsyntax-only`. Their five exported functions
also have isolated, unchanged-original references, bringing the retained
suite to eight inputs / 22 selected functions. Matching degeneracy matters:
an empty function and a return-containing function may both normalize to a
one-node CFG, but only the former contains exclusively boundary Nops.
