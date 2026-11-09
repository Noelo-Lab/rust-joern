# Pinned mechanical-port pilot — 2026-10-25

This is the original experiment on its stated historical branch and binary;
the independent main-based submission is measured separately in
[independent validation](joern_operator_independent.md).

Status: **behavior-preserving first batch**, not a complete C++ frontend port
or a score improvement. See [the repeatable port workflow](../joern-mechanical-port.md).

## Identity and scope

- Rust baseline: `d6465e94744d187729716df897e66de9b87e6f94`.
- Upstream source: Joern v4.0.150,
  `958fdd3d976197a783f8ade1254b43e977648f28`.
- Original interpreter: PyJoern 4.0.150.4 / Joern v4.0.150, isolated from the
  replacement wrapper. Source and original JVM constants were verified live.
- Candidate release library SHA256:
  `0c2d0738bf9e59a1569ae8d3ad7058e4e6587ca6e35c41d0ec273b89e7fddba9`.
- Apache source/license, 128 JVM constants and the supplying jar hashes are
  retained under `tests/reference/joern-4.0.150/`.

Two complete Scala operator-selection matches are translated mechanically.
The production builder now consumes the generated Rust module. Lexical
adapters and unported CDT parsing/binding/type requirements are explicitly
separate. No new semantic behavior or intentional divergence is introduced.

## Measurements

| Evidence | Result | Interpretation |
| --- | --- | --- |
| Frozen real-input pool, default | 1,225/1,225 full analysis JSONs unchanged | No observed regression or gain |
| Same pool, Joern loop profile | 1,225/1,225 full analysis JSONs unchanged | Separate profile; same inputs, not another corpus |
| Each real-input profile | 1,171 parsed; 27 rejected; 27 no methods | Existing failures retained |
| Upstream `code(...)` inventory | 421 calls: 381 literal source controls; 40 require review | Not a translation of Scala assertions/configuration |
| Imported controls | 297 parsed; 6 rejected; 78 no methods | Source-only parser audit; not 297 upstream tests passing |
| First six original probes | 2 function CFGs match; 4 files have no original function CFG | Four unavailable comparisons remain recorded as failures; no denominator repair |
| Six additional body-bearing C++ inputs | 10/10 function CFGs match; no diagnostics | Directed topology, entry/exit, degeneracy and method population compared |
| Repeated upstream import | Byte-identical `INPUTS.json` and `INVENTORY.json` | Deterministic source/line/hash transport |

The six body-bearing inputs come from `AstCreationPassTests.scala` definition
lines 1154, 1201, 1486 and 1808, plus `CallTests.scala` lines 73 and 138.
They exercise inheritance, catch, array allocation, constructor allocation,
member calls and static calls. The source snippets, exact original captures,
origins and hashes are checked in as six frozen controls and run by
`tests/test_joern_port.py`. CFG equality does **not** establish complete CPG
property equality, C++ semantic correctness or exception/object-lifetime
effect preservation.

The six rejected imported controls remain a work queue: `decltype(...)::value`,
structured-binding range-for, namespace-scope brace initialization, pack
expansion expressions, a malformed declarator, and template specialization
cases require owner-specific investigation. The original Scala configurations/assertions must be recovered
before treating any of these as an upstream compatibility defect. They were
not fixed by changing expected graphs or suppressing diagnostics.

## Checks

- Rust: `cargo test --all-targets`: 238 passed.
- `cargo build --lib`, release CLI/shared library build, Clippy with warnings
  denied and workspace formatting passed.
- Python: complete local integration suite, including the seven new port
  tests: 176 run, 171 passed, 5 pre-existing skips.
- Scoped Black/Ruff, deterministic generated-file check, and original
  source/JVM snapshot verification passed.

Full local captures, including the four unavailable original comparisons,
are in `workspace/joern-port-4.0.150/`. `SUMMARY.json` records frozen contracts
and before/after counts; the `REAL-*-COMPARISON.json/` directories retain
full comparison/queue artifacts. The import's source manifest SHA256 is
`f22ff84c787acf056bf4c39a41398ace7c15d8608719e9380abbb15029456881`.

DecBench source, dependency pin, scoring, eligible population and published
rankings remain unchanged. No new upstream PR is part of this batch.
