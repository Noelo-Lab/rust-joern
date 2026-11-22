# C++ defaulted and deleted members

`defaulted-members.cpp` is original to this test and needs no headers;
`clang++ -std=c++17 -fsyntax-only` accepts it. It covers constructors,
destructors and assignment operators declared `= default` or `= delete` in a
class body, a pure virtual member (`= 0`), an out-of-class `= default`
constructor and a plain operator declaration. Joern exports each such member
as a method; CDT parses `= default`/`= delete` as definitions, so a defaulted
`operator=` is named `=` while a plain declaration stays `operator +=`.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
