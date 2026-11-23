# For-initializer references

`class-members.cpp` and `free-functions.c` are original to this test.
`clang++ -std=c++17 -fsyntax-only` accepts the C++ file and
`clang -std=c11 -fsyntax-only` accepts the C file; neither needs headers.

Each loop declares a local in its `for` initializer that shares its name with
a function or method, and uses it in the condition, update or body. CDT
resolves those uses to the local, so Joern keeps them as identifiers, not
method references. `free-functions.c` also returns the function name after the
loop, where it is a method reference again.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
