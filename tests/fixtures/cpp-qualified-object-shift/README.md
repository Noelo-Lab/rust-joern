# Qualified object statements after qualified types

`stream-statements.cpp` is original to this test. With its `#include
<iostream>` and `#include <string>` lines restored, `clang++ -std=c++17
-fsyntax-only` accepts it. The fixture omits them, as DecBench's header
stripping does. A declaration with a qualified type such as `std::string`
must not make the scope `std` a type, which turned the later
`std::cout << ...;` statements into failed declarations.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
