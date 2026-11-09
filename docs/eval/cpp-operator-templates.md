# C++ operator-template follow-up — 2026-10-22

This is a frontend correction on PR #3's `52a8899`, not a DecBench dependency
upgrade, metric-policy decision or decompiler ranking result.

## Defect and owner

`operator!=<>(a,b)` was not consumed as an operator-function-id followed by
template arguments. Expression recovery could replace the enclosing condition
with a problem expression or discard a return, without reporting an error.
This reproduces in complete, declared C++ accepted by two Clang compilers.

The expression-name scanner and recovery checks now agree on the name boundary.
Runtime arguments still undergo expression checking. The builder also extracts
the callee's leaf outside template arguments: `demo::operator+<types::Tag>`
must not become a call named `Tag>`.

No dependency, diagnostic suppression, default loop option, compiler-based
repair or benchmark-specific production condition is added. Strict parsing
still does not prove that arbitrary pseudocode is valid C++.

## Focused controls and regression checks

- Seven header-free witnesses pass Apple Clang 17 and Homebrew Clang 22 with
  `-std=c++17 -pedantic-errors -fsyntax-only`; nothing is linked or executed.
- Unchanged PyJoern 4.0.150.4 / Joern v4.0.150 captures use separate Python
  interpreters and temporary directories, with `PYTHONPATH` removed.
- All seven selected `f` definitions match directed topology, entry/exit roles
  and degeneracy after the correction; five did not match before. This is not
  all-export or full-CPG parity: some auxiliary operator declaration exports
  have different names. Those original captures are retained as limitations.
- Two new complete controls and their five original exports are checked into
  `tests/fixtures/cpp-qualified-expressions/`, with source/reference hashes.
  The full retained fixture suite is eight inputs / 22 selected functions.
- Rust: 221 passed across all targets, including seven added tests.
- Python with the DecBench checkout available: 166 tests, 161 passed / 5 skipped.
  Four skips require a hardcoded sibling DecBench path; one requires native
  DDG platform library discovery. No test body is waived.
- DecBench's experimental C++ focused workflow against the candidate library:
  316 passed / 5 platform-specific skips. The full DecBench suite is not rerun.
- Release build, Clippy with warnings denied, formatting and diff checks pass.

For the retained source/reference fixtures and executable-operand checks:

```sh
cargo test --all-targets
cargo build --release
PYTHONPATH=python RUST_JOERN_LIBRARY="$PWD/target/release/librust_joern.so" \
  python -m unittest discover -s tests -p 'test_*.py'
```

Use `.dylib` on macOS. The optional DecBench integration tests need its checkout
and Python dependencies; report skips when those prerequisites are absent.

## Fixed-input audit and remaining integration gates

The prior frozen C++ qualification pilot supplies 1,026 unique prepared
source/producer inputs from four tools / 250 requested addresses per tool.
Both native builds receive identical bytes and explicit
`joern_loop_conditions=True`, keeping the independent loop-policy change out
of this comparison.

- 993 complete CPG outputs unchanged; 26 inputs rejected by both builds.
- Seven CPG outputs changed, all on the producer side; six retain identical
  directed CFG topology, entry/exit roles and degeneracy. One changes from
  194 nodes / 289 edges to 210 / 313 as calls and conditions are recovered.
- No parser-success loss or definition-set change. Two fresh isolated replays
  produce identical complete comparison reports.
- Of 87 existing original-reference inputs, 82 match, four lack a unique
  original export and one large Ghidra pseudocode input remains non-isomorphic
  (original: 199 nodes / 297 edges). This was already non-isomorphic before
  the patch; no identity, error or topology difference is waived.

The remaining large pseudocode input is not qualified as complete valid C++.
Preserving its calls does not establish its full correctness or original
recovery parity. The historical 8,808-case audit and full 250-function metric
evaluation are not rerun in this follow-up. No fresh score, eligibility increase,
execution parity or official C++ support is claimed.

DecBench retains its existing pin and profiles. Adopting a new parser still
requires an explicit loop policy, resolution of the remaining comparison
limits, metric/cache review and fresh native-address/source/producer evaluation.
