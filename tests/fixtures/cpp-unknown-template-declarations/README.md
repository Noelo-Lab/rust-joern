# C++ unknown template-id declarations

`unknown-template-declarations.cpp` is original to this test. With
`#include <map>`, `#include <string>` and `#include <vector>` restored,
`clang++ -std=c++17 -fsyntax-only` accepts it. The fixture omits them, as
DecBench's header stripping does, so `vector` and `string` are unknown names.

CDT then reads `std::vector<std::string> lines;` as the comparison
`(std::vector < std::string) > lines`, and with `= value` as an assignment to
that comparison. Placed first in a branch before a loop, the comparison opens a
basic block, as in ninja's `StatusPrinter::PrintStatus`. Rust Joern reproduces
this only with `joern_cdt_failures`. The controls stay declarations in Joern: a
known class template, a type-keyword argument, a pointer argument and two
arguments.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
