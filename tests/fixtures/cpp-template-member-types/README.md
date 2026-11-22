# C++ member types of template-ids

`member-types.cpp` is original to this test, and
`clang++ -std=c++17 -fsyntax-only` accepts it as is (it needs no headers). It
covers types spelled as a member of a template-id, `Box<int>::cursor`, as
function return types (definitions and a declaration) and as parameter types.
The parameter's declarator must stay its name, so `touch(at);` in a nested
block remains a call, as in ninja's `Plan::EdgeMaybeReady`.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes with `scripts/compare_pyjoern.py
--save-reference-dir`. The local Python module path was replaced with a
generic label; graph data and hashes were not changed.
