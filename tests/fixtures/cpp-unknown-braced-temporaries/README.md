# Braced temporaries of undeclared types

`braced-temporaries.cpp` is original to this test. With the declarations of
`chrono`, `sample`, `measure`, `ns::box` and `holder` restored, `clang++
-std=c++17 -fsyntax-only` accepts it. The fixture omits them, as DecBench's
header stripping removes such declarations. Inside an expression, or when
qualified, `T{...}` is a braced temporary even when `T` is not declared.

The CFGs were captured from PyJoern 4.0.150.4 / Joern 4.0.150 in an isolated
interpreter on the exact file bytes. The local Python module path was replaced
with a generic label; graph data and hashes were not changed.
