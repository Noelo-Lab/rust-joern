# Constructor member-initializer controls

These two handwritten examples are original to this test and accepted by
`clang++ -std=c++17 -fsyntax-only`. Each constructor has a parenthesized
member-initializer list whose calls Joern 4.0.150 leaves out of the CFG. The
graph starts at the constructor body. `out-of-line.cpp` has a qualified
definition with a base initializer and a loop body. `in-class.cpp` has two
in-class constructors, one with an empty body.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes, with source and prepared SHA-256 hashes.
The local Python module path was replaced with a generic label; graph data and
hashes were not changed.

The Python regression parses both files in strict mode and checks function
coverage, directed topology, entry/exit roles and degeneracy against these
references. Braced member initializers are outside this fixture.
