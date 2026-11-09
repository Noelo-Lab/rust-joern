# Independent operator port validation

The operator generator and fixture snapshot are split from the larger call,
receiver and type-binding work. This branch starts at upstream main
`9f8cad170d6b6883fe08627fa524ecb28128380d`, not the historical prototype
baseline in `joern_port_2026-10-25.md`. The pinned Joern source and license are
reference/test material, not a runtime dependency.

Base executable SHA-256:
`407537b4402f7a374b7295d8b1edf093340096107ab1f7da8e73acb060d6ee33`.
Candidate executable SHA-256:
`43999d83e00d1aa89a865d10e2d2d5fc8baf52335a8a22ee374a8a6e951944c6`.
On eight previously prepared Snappy/Ninja TUs, strict C++ diagnostics were
identical; all remained rejected. On a separate set of 18 C/C++ inputs, full
CLI outputs and exit statuses were identical to base: 12 outputs and six
rejections. This is bounded evidence, not a global equivalence claim.

Six pinned original Joern reference inputs matched directed CFG topology,
entry/exit roles and degeneracy for 10/10 exported functions. Frozen original
fixtures were reused; their provenance is in the fixture manifest. This does
not establish all-node-label, effect, type or execution parity. The generator's
consistency check is part of the Python test suite.

The original branch and candidate hash in the historical report remain an
independent earlier experiment. Local independent run artifacts are retained
under `workspace/operator-default-replay.json`,
`workspace/operator-full-response.json` and
`workspace/operator-fixture-compare.json`. No DecBench dependency, score or
backend pin changed.
