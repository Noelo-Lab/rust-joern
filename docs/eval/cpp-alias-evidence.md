# Scalar C++ alias evidence for CFG replay

`scripts/cpp_alias_evidence.py` collects source declarations for scalar
`typedef` and `using` chains in preprocessed C++. It keeps the original
declarations and namespace paths so an isolated function can be replayed
without guessing a built-in spelling. The evaluator supplies and validates
the local and class scope. The script can abstain on conflicting declarations
and unsupported lookup. Diagnostic overlap and strict CFG replay are separate
checks performed by the evaluator.

## Reduced replay

The test uses a typedef in a C linkage block, a `using` alias in a reopened
namespace and a function with a branch. It compares the full and
isolated Rust-Joern normalized CFGs. The optional test packages are separate
from the package's runtime dependencies:

```sh
python3 -m pip install -e . 'tree-sitter==0.25.2' 'tree-sitter-cpp==0.23.4'
cargo build --locked --lib
PYTHONPATH=python python3 -m unittest discover -s tests -p test_cpp_alias_evidence.py
```

## Corpus observation

The local source qualifier using this helper and the diagnostic spans from
#34 qualified 13 of 4,580 definitions across 83 prepared TUs. The prior
qualifier accepted eight. Five additional definitions from LevelDB, libzmq,
and Ninja matched the original Joern normalized CFG topology. GCC 13.3
independently confirmed the alias location and integer type on observed
fields or parameters. All five real gains involved typedefs. The using
declaration is covered by the reduced replay.

The tracked tests reproduce the reduced case. Full cohort inputs and
qualification artifacts were generated locally. The Rust parser, Python API,
strict-mode behavior, and benchmark scoring require no changes to run the
reduced check.
