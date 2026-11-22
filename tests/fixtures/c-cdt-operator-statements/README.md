# C `operator` names and failed statements

`operator-statements.c` and `operator-name.c` are original to this test. They
cover decompiled C++ read as C, where CDT's C parser has no operator-function-ids
and `operator` is an ordinary name:

- `operator()(...) { ... }` becomes the declaration-only method `operator`;
- `p = operator new(n);` and `p = (T *)operator new(n);` are a completed
  assignment followed by `new(...)`, and the missing semicolon also drops an
  enclosing label;
- a conditional whose `:` follows the point where its expression stops fails
  the whole statement; a closed conditional is a control.

`operator-name.c` is a control with a plain C function named `operator`, which
parses in both modes. The C++ statements are not valid C, so no compiler check
applies to `operator-statements.c`; `cc -std=c11 -fsyntax-only` accepts
`operator-name.c`.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
