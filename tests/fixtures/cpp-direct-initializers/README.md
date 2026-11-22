# C++ direct initializers

`direct-initializers.cpp` is original to this test, and
`clang++ -std=c++17 -fsyntax-only` accepts it without added headers. It
covers local objects whose parenthesized argument cannot be a parameter
declaration: a conditional, a comparison, a call, a literal, a unary or binary
operator, a member access, `nullptr` and `this`, plus a variable argument.
CDT reads each as an object with a direct initializer, and Joern lowers it to
a call `box(args)`. `prototype` declares a real local function prototype as a
control.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
