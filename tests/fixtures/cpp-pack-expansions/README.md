# C++ pack expansions

`pack-expansions.cpp` is original to this test. With its `#include <array>`
and `#include <cstddef>` lines restored, `clang++ -std=c++17 -fsyntax-only`
accepts it. The fixture omits them, as DecBench's header stripping does. It
covers pack expansions in braced lists, a braced temporary with template
arguments and a call, as in Snappy's `MakeTable` and `MakePatternMaskBytes`.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
