# Explicit destructor call controls

The checked-in source files are handwritten controls. The three supported
cases pass Clang C++11 syntax checks; `invalid-arguments.cpp` deliberately does
not. The matching reference graphs were captured using unchanged PyJoern
4.0.150.4 / Joern 4.0.150. Source and sanitized reference hashes are in
`provenance.json`; only local module path metadata was removed from the captured
JSON. The graph data remains unchanged.

These checks cover public CFG topology, entry/exit roles and degeneracy, plus
separate Rust tests for receiver operands and evaluation paths. They do not
establish full AST/DDG parity or implicit lifetime and exception semantics.
The real-source comparison discussed in local validation is not included in
these fixtures.
