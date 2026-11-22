# C++ template-id temporaries

`template-id-temporaries.cpp` is original to this test. It calls functor
temporaries such as `std::less_equal<const void *>()(a, b)` in a return, an
`if` condition, a short-circuit expression and a loop body (with a `>>`
template-id), as in Snappy's `MemCopy64` assertion. `comparison_control` keeps
`a < b > (c)` with variables as comparisons.

With `#include <functional>` and `#include <utility>` restored,
`clang++ -std=c++17 -fsyntax-only -Wno-parentheses` (Clang 22) accepts it;
without `-Wno-parentheses` Clang rejects the deliberate chained comparison
in `comparison_control`. The fixture omits the headers, as DecBench's header
stripping does.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
