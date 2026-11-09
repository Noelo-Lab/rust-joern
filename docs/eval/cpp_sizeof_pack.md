# C++ pack-size expression

Strict parsing previously rejected valid `sizeof...(T)` on C++ parameter
packs. This change recognizes a single parenthesized identifier and preserves
the full spelling in a `<operator>.sizeOf` node. The operand is unevaluated;
no target ABI is available, so the expression type is kept unknown. The parser
does not verify that an identifier denotes a pack or validate all C++ types.
Pack expansion in calls and other forms remain outside this change.

## Evidence

Base `9f8cad170d6b6883fe08627fa524ecb28128380d` release executable SHA-256
`407537b4402f7a374b7295d8b1edf093340096107ab1f7da8e73acb060d6ee33`;
candidate executable SHA-256
`60162cf8dd916e3cfc4198d8dfda5e0919a9114da0d2dbdafd48f9db22d51d85`.
Both used strict C++/preprocessed mode on the same eight real source TUs.
Every TU remains rejected; the table counts diagnostics, not functions.

| TU | SHA-256 | Before | After | Added |
| --- | --- | ---: | ---: | ---: |
| Snappy Clang GNU++11 | `a8bbe3a7c73b3b4e7357feb7ef48227035be302a470232bb514ebc6ade852355` | 531 | 529 | 0 |
| Snappy Clang GNU++14 | `2e13eaaacc24cbfa4f10b4afa04a5b38f78bf5b908340d64e26d924a6ffd10b2` | 580 | 578 | 0 |
| Snappy Linux GCC GNU++11 | `d32de56d4b8e315ff7d7010db2c5eb55a5c2a88c0083d8df25238923ba194ace` | 473 | 459 | 0 |
| Snappy Windows MinGW GNU++11 | `8d5fe154aca5693eb20220f0ae857a785446fe24516df1fbfbf9e569d4557f5a` | 334 | 320 | 0 |
| Ninja Clang C++17 | `702c714a94f74856515a8c77a19ab6cef5b840b2b0e0c02f207586ba82a26a14` | 713 | 693 | 0 |
| Snappy sink Clang GNU++11 | `51960ff1da75919e951e80e840b8e7ebf7a95006d84e4026fd530f989600f1c9` | 2 | 2 | 0 |
| Snappy sink Linux GCC GNU++11 | `7fd6e23fe75250360a41a8ba935f8df9a6a6cd6d40b29b05fb31febd9317009a` | 2 | 2 | 0 |
| Snappy sink Windows MinGW GNU++11 | `bfd4c26e05a8eea005e07066fad6f3a0a089a5f0f765aaf21423e6a8540010a4` | 3 | 3 | 0 |

Clang 22 accepts the type- and value-pack test fixtures in C++11 mode. For the
type-pack fixture, unchanged original PyJoern 4.0.150.4 / Joern 4.0.150 and
this candidate both export a one-node CFG with equal entry/exit roles. The
original labels the size expression `UnsupportedStmt`. This observation is
not a claim of CPG label, DDG or execution parity. Full local replay artifacts
are not distributed with this patch. No DecBench metrics or pins changed.
