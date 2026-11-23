# C++ declaration conditions

`declaration-conditions.cpp` is original to this test, and
`clang++ -std=c++17 -fsyntax-only` accepts it without added headers. It covers
`switch`, `while` and `if` statements whose condition declares a variable, as
in ninja's `GuessParallelism`, plus expression conditions as controls. Joern
drops the `switch` and `while` from the CFG, body included, and keeps the `if`;
the test requests `joern_cpp_control_flow` to reproduce that.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
