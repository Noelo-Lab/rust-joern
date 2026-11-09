# Special-member recovery controls

These two handwritten examples are original to this test. `complete.cpp` is
compiler-valid C++11; `isolated.cpp` intentionally omits its owner declaration
and tests recovery only. The accompanying CFGs were captured from PyJoern
4.0.150.4 / Joern 4.0.150 in an isolated interpreter on the exact file bytes.
Source and prepared SHA-256 hashes are retained in each capture. The original
local Python module path was replaced with a generic label; graph data and
source hashes were not changed.

To check the frozen references, build the release CLI and run
`scripts/compare_pyjoern.py` with both source paths and this reference directory.
The Python regression also checks source hashes and graph topology. The captures
do not prove full C++ validity, AST/data-flow parity or lifetime semantics.
