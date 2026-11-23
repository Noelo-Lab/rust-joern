# C++ fields declared after a member body

`later_fields.cpp` is an original, header-free C++17 input written for this
test. Its inline member functions pass fields declared at the end of the class
to a member function that is also declared later: once as an unbraced `if`
body and once alone in a nested block. A member body is a complete-class
context, so CDT resolves the later fields and Joern exports `resize(limit)` as
a call with a `this->limit` argument, not as a declaration `resize limit;`.
Homebrew Clang 22 accepts it with `-std=c++17 -pedantic-errors -fsyntax-only`;
it needs no headers.

`later_fields.pyjoern.json` was captured from unchanged PyJoern 4.0.150.4 /
Joern v4.0.150 with `scripts/compare_pyjoern.py --reference-python ...
--save-reference-dir`, in a separate interpreter without the `compat` shim.
The local module path is omitted; the source and prepared hashes are kept. The
native candidate was not used to generate the expected graphs.
