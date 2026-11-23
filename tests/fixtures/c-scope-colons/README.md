# C scope colons

`scope_colons.c` is an original, handwritten C source in the shape of
decompiled C that spells C++ names, such as `r = (int)ns::step(n);`. CDT's C
scanner reads `::` as two colons, so these lines are not valid C, and
`clang -std=c11 -fsyntax-only` rejects them as expected. The file covers:

- an expression that ends at the first `::` after a complete operand (name,
  literal, subscript or call, but not a cast group), including degenerate
  single-statement functions;
- that missing semicolon failing an enclosing `if`;
- a compound item `ns::f(x);`, the label `ns` with an unparseable statement:
  Joern emits no node, but a `while` around it still loops on its condition;
- controls: an empty loop body, a plain label body, a problem body, an
  unbraced loop body and a conditional `?:` colon.

`scope_colons.pyjoern.json` was captured from the same bytes with unchanged
PyJoern 4.0.150.4 / Joern v4.0.150 through `scripts/compare_pyjoern.py
--save-reference-dir`. Only the local `reference_module` path was replaced;
the source and prepared hashes are kept. `tests/test_c_scope_colons.py`
compares function coverage, directed CFG topology, entry/exit roles and
degeneracy in strict mode with the `joern_*` options.
