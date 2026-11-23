# C++ dependent bindings

`dependent-bindings.cpp` is original to this test, and
`clang++ -std=c++17 -fsyntax-only` accepts it as is; it needs no headers. Inside
a class template, CDT types the injected class name and template-ids with a
dependent argument as dependent, and Joern prints them as `ANY`. The copy,
move and `Span<T>` constructors therefore share one full name, as do both
assignment operators, and Joern keeps the last graph for each. The constructor
taking `Span<int>` and `total`'s `Buffer<int>` keep their spelling.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
