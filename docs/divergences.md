# Divergences from Joern

Rust Joern targets Joern 4.0.150 / PyJoern 4.0.150.4. This log records each
intentional difference in observable output: when it was made, why, which
graphs it affects and how to restore the reference behavior. Add an entry, newest
first, in the change that introduces a divergence. Fixes that restore parity
belong in regression fixtures, not here.

## Code that CDT fails to parse is still analyzed

**Recorded:** 2026-11-09, [PR #19](https://github.com/Noelo-Lab/rust-joern/pull/19), which added the option that restores Joern's
behavior. See the [DDG parity evaluation](eval/ddg_parity.md).

**Joern:** CDT's C parser backtracks out of a parameter list it cannot parse,
such as `f(int64_t a, signed int64_t b)` or `f(<anonymous> *** r0)`. The
declaration and its body become one problem node and there is no method. `*`
applied directly to an integer, floating or character literal fails the whole
expression: `*0x40004000 = x;` has no nodes, an `if`, `while` or `switch`
condition becomes an `UNKNOWN` node, and a `for` statement is dropped. A local
declaration whose declarator fails, and which is not an expression either, has
no nodes, and a missing semicolon also fails the enclosing label or control
statement. A call whose unclosed string literal runs into a later brace
statement is a problem up to the end of that brace group.

Some failures also change what later names mean. A file-scope
`T (*)(args) name()` declares a function named `T`, and `T (*x)[n] = v;` binds
an undeclared `T` as an external function; after either, `T *x;` is a
multiplication. `if (a != {0})` closes the function at the brace of `{0}`, so
the rest of its body is parsed at file scope and its locals become globals;
a later `T *x;` that names one of them is a multiplication too. Of
`T name = *0x3b;` only the expression `T` remains.

**Rust Joern:** the definition keeps its method and body, the expression keeps
its nodes, and the statement is converted. Labels and control statements stay,
a problem ends at its semicolon, `T` stays a type and locals stay local.

**Why:** decompilers emit these forms routinely: r2dec's `signed int64_t`
parameters and `return void (*0x3900)() ();`, and `*0x40040014 = 2;` from
dewolf, Binary Ninja and angr on firmware. The code is recoverable, and
DecBench has scored it since the earlier
[recovery work](decbench.md#decompiler-output-recovery-update). Reproducing
CDT's failures by default would remove 610 methods from the decompiled DecBench
inputs and change the DDG of 3,780 other functions.

**Affects:** which methods exist, and the CPG, CFG, reaching definitions and
DDG of functions containing such statements. Preprocessed sources are
unaffected.

**Reference behavior:** `joern_cdt_failures` in `Options` and Python, or
`--joern-cdt-failures` on the CLI. `compare_ddg_pyjoern.py`, `ddg_corpus.py`,
`replay_ddg_projection.py` and the original-reference DDG tests request it.
DecBench and its shim use the default.

## Run-dependent DDG export order is fixed

**Introduced:** 2026-11-09, [PR #19](https://github.com/Noelo-Lab/rust-joern/pull/19). See the
[DDG parity evaluation](eval/ddg_parity.md).

**Joern:** `dotDdg` follows hidden definitions through a cache with a
visited-path cut, so the edges it exports depend on the stored order of each
node's incoming `REACHING_DEF` edges. Part of that order comes from hash
collections keyed by node objects, whose hash includes the JVM identity hash of
the graph: the order in which call sites are visited, the arguments of a call
with more than four, and the definitions reaching a returned expression. Two
original parses of the same bytes can therefore export different DDGs for the
same function. The reaching-definition edges themselves do not vary.

**Rust Joern:** reproduces every deterministic part of that order and replaces
the hash-dependent parts with solver order, argument order and definition
numbers. The same bytes always produce the same DDG.

**Why:** a benchmark needs reproducible graphs, and no single original export
is the reference for such a function.

**Affects:** the labeled DDG projection and the public `Function.ddg` of
functions whose hidden definitions form cycles, mostly large loop-heavy
decompiled functions. Raw reaching-definition edges are unaffected.

**Reference behavior:** none. `replay_ddg_projection.py` and
`ddg_corpus.py replay` check such a function against one original run instead:
equal reaching-definition edges, and the original DOT reproduced from the
original's own graph and stored order.

## K&R definitions keep every parameter

**Recorded:** 2026-11-09, [PR #19](https://github.com/Noelo-Lab/rust-joern/pull/19), which added the option that restores Joern's
behavior. See the [DDG parity evaluation](eval/ddg_parity.md).

**Joern:** a K&R definition gets one parameter for each parameter
*declaration*, in declaration order, named by its first declarator. In
`int f(a, b, c) char *c; int a, b; { ... }` the parameters are `c` and `a`.
`b` is not a parameter, and neither is a name without a declaration, which C
gives the type `int`.

**Rust Joern:** every name of the identifier list is a parameter, in list
order: `a`, `b`, `c`. Each keeps the code of its declaration, so `a` and `b`
both have the code `int a, b;`; an undeclared name has its own spelling as code.

**Why:** DecBench compares a source function with its decompilation, which
declares every parameter. Bash and other K&R-style sources otherwise lose
parameter nodes and the definitions that reach their uses, so a correct
decompilation scores as different. The option changes the DDG of 611 functions
in the DecBench corpus.

**Affects:** parameter nodes in CPGs and ASTs, and the reaching definitions and
DDGs that start at them. CFGs are unchanged.

**Reference behavior:** `joern_knr_parameters` in `Options` and Python, or
`--joern-knr-parameters` on the CLI. `compare_ddg_pyjoern.py`, `ddg_corpus.py`
and the original-reference DDG tests request it. DecBench and its shim use the
default.

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
