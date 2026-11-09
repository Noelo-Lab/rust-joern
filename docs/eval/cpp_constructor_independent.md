# Historical validation of the combined context and constructor candidate

Base: `9f8cad170d6b6883fe08627fa524ecb28128380d` (release SHA-256
`407537b4402f7a374b7295d8b1edf093340096107ab1f7da8e73acb060d6ee33`).
Initial candidate release SHA-256 (before the literal-conversion follow-up):
`c305264cea566af51b577ce2552e7a6df86c1c5a3fbb56343927dbb53a375d02`.

In the earlier combined candidate, with both options absent, strict analysis of eight prepared Snappy/Ninja
TUs returned the same failures and exactly the same diagnostic multisets as
base. This is not evidence of successfully parsing those inputs. Separately,
full CLI responses were identical for twelve inputs: six produced output and
six were rejected by both. The inputs include eight C++ regression fixtures
and four Clang C11 preprocessed Zlib TUs. These are bounded checks; they do not
prove global default compatibility. Local full records are retained in
`workspace/constructor-default-replay.json` and
`workspace/default-graph-comparison.json`.

These results predate the split and do not validate the current head.

The opt-in initialization-order fixture compiles under Clang 22 C++11. Its
runtime control printed `123 7 8`: field initializers execute in declaration
order and then the body, despite reverse source-list order. The candidate's
synthetic CFG includes the same initializer-call order in tests; this is a
focused control, not execution parity for arbitrary programs. Unchanged
PyJoern 4.0.150.4 / Joern 4.0.150 exports the fixture constructor as a one-node
body-only CFG with `body()` and omits both initializer calls. The opt-in CFG
intentionally differs from that reference; see `docs/divergences.md`.

No DecBench metric, backend pin or upstream benchmark population changed.

## Literal-conversion follow-up

The opt-in profile now rejects string literals and incompatible scalar/pointer
conversions, and accepts `nullptr` for pointer members. It also abstains on
pointer values from identifiers or calls, including valid pointer copies:
the available binding type loses pointee cv qualifiers, so it cannot prove that
such a conversion preserves them. This is a bounded rule, not a complete C++
conversion checker. A compiler control and focused regression cover these
cases; the earlier corpus and release-hash evidence above applies to the
initial candidate, not this follow-up.
