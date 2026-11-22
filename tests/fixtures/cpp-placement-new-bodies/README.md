# C++ placement new bodies

`unbraced-bodies.cpp` is original to this test. With `#include <new>`
restored, `clang++ -std=c++17 -fsyntax-only` accepts it. It covers placement
new as the unbraced body of `if`, `for`, `while` and `else`, as in the
`clone` member of Ninja's bundled `emhash8::HashMap`.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
