# Divergences from Joern

Rust Joern targets Joern 4.0.150 / PyJoern 4.0.150.4. This log records each
intentional difference in observable output: when it was made, why, which
graphs it affects and how to restore the reference behavior. Add an entry, newest
first, in the change that introduces a divergence. Fixes that restore parity
belong in regression fixtures, not here.

## C++20 range-for init-statements execute before the range

**Introduced:** 2026-11-09, [PR #18](https://github.com/Noelo-Lab/rust-joern/pull/18).

Joern 4.0.150 omits a range-for with an init-statement on the reduced
`for (int i = start(); const auto& x : values)` control. Rust Joern retains
the initializer, evaluates it once before the range expression, and keeps
its variable bindings in the loop scope. The zero-iteration path starts
after both evaluations and the backedge returns to the iterator test.
This changes the AST, CFG and derived data-flow views. The iterator protocol
is represented by the same abstract test and advance nodes as ordinary
range-for. No option restores the original omission.

## C++ range-for retains the range evaluation and loop exits

**Introduced:** 2026-11-09, [PR #15](https://github.com/Noelo-Lab/rust-joern/pull/15).

Joern 4.0.150 skips the range initializer in the CFG and uses the body as
the loop condition. Rust Joern evaluates the range once before the loop, then
uses abstract test and advance nodes. The test can exit before the first
iteration, `continue` goes to advance, and `break` goes to the successor.
The nodes represent control flow without resolving iterator overloads or
modeling implicit calls. This affects the CPG and raw and normalized CFGs,
and may affect the data-flow views derived from them. No option restores
the original range-for CFG.

## Constant-true loop heads lower like `for (;;)`

**Introduced:** 2026-10-14, [PR #2](https://github.com/Noelo-Lab/rust-joern/pull/2).

**Joern:** `while (1)` and `while (true)` lower with a CFG condition node and an
exit edge to the loop's successor. `for (;;)` has neither.

**Rust Joern:** a `while` or `for` condition that is `true` or a nonzero integer
literal keeps its AST node and `CONDITION` edge, but has no CFG node or exit
edge. The loop takes the `for (;;)` lowering, including Joern's order-based
slots: a braceless body that fills the condition slot keeps its exit edge, as
in `for (;;) n++;`.

**Why:** decompilers emit `while (1)` (angr, IDA, r2dec), `while (true)` (Binary
Ninja, Ghidra, dewolf, Kuna) and `while ( 2 )` (IDA) where source mostly has `for (;;)`,
so identical control flow scored differently in DecBench GED. In every tenth
artifact of `full_run_address_2026-09-11`, 3,949 of 50,856 functions with such
loops changed; all matched a textual rewrite of their loop heads to `for (;;)`.

**Affects:** raw and normalized CFGs, and the reaching definitions and DDGs
derived from them. ASTs are unchanged. `do { } while (1)` is not affected.

**Reference behavior:** `joern_loop_conditions` in `Options` and Python, or
`--joern-loop-conditions` on the CLI. Original-reference parity tests,
`compare_pyjoern.py`, `compare_native_corpus.py` and `compare_ddg_pyjoern.py`
request it. DecBench and its shim use the default, so `compare_decbench.py`
reports these loops as differences, and DecBench's GED cache needs a new
`cache_version`.

## Original exporter stack overflows are not reproduced

**Recorded:** 2026-10-09 CFG parity evaluation.

**Joern:** original PyJoern returns no functions for three prepared inputs,
NuttX `lib_wcwidth.i` and Cleanflight/Crazyflie `arm_common_tables.i`. Its
recursive CFG DOT projection raises `StackOverflowError` after graph
construction.

**Rust Joern:** returns the graphs, which match an iterative recovery of the
original exporter. See [Original exporter failures](eval/cfg_parity.md#original-exporter-failures).

**Affects:** the five corpus cases that use those inputs.

**Reference behavior:** none; comparisons report the empty original exports as
differences.
