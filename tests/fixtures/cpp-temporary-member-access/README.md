# C++ temporary member access

`temporary-member-access.cpp` is original to this test, and
`clang++ -std=c++17 -fsyntax-only` accepts it without added headers. It
covers expression statements `T(x).f(...)` and `T(x)->f(...)`, which must stay
calls rather than grouped declarations of `x`, as in double-conversion's
`Double(v).NormalizedBoundaries(...)` in `Grisu3`. `T (x);` remains a grouped
declaration control.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
