These focused CFG references were captured from unmodified PyJoern 4.0.150.4
(Joern v4.0.150), cfgutils 1.16.0, NetworkX 3.6.1, and PyGraphviz 2.0.
The source SHA-256 is recorded in each JSON. They cover 48 functions across
C control flow, expressions, declarations, and a small C++ sample.

The JSON retains graph topology, entry/exit roles, degeneracy, and human-readable
provenance labels. DecBench GED ignores those labels. Baselines include original
Joern behavior even where it appears incorrect, such as an empty `while` body
without a back edge and evaluation nodes under `sizeof`. The comparator reports
every divergence; there is no automatic exception for a suspected Joern bug.

Run after `cargo build --release` using a Python with NetworkX:

```sh
python scripts/compare_pyjoern.py tests/fixtures/control.c tests/fixtures/expressions.c tests/fixtures/functions.cpp --reference-dir tests/fixtures --output workspace/fixture-comparison.json
```

For a bounded live DecBench comparison, pass a specific `.i`/`.ii` input and
`--reference-python /path/to/original-pyjoern/bin/python --decbench /path/to/decbench`.
The harness applies DecBench's own preparation and feeds identical bytes to both
parsers. Add `--sanitize-decompiled` for a decompiler's C output.
