# C++ expression names and caller validation

Rust Joern accepts preprocessed source and decompiler pseudocode. Its strict
mode rejects **reported parser errors**; it is not a compiler's complete C++
syntax, binding, type or ABI check. In particular, duplicate local declarations
and some malformed expressions can be recovered without an error. A returned
CFG does not certify that the input is a valid C++ translation unit.

## Supported expression-name subset

The expression parser reads qualified names component by component. A
component can have template arguments before the next `::`, so a call such as
`::demo::Box<demo::Box<int>>::get(x)` retains its complete callee and runtime
argument. Operator-function-ids such as `operator[]`, `operator()`,
`operator+` and `operator new` are names, rather than index, arithmetic or
allocation expressions. Explicit member qualification such as
`receiver().demo::Box::operator[](tick())` retains the receiver and argument
as separate call operands. Explicitly qualified member targets remain
unresolved: complete C++ qualified/base lookup is not implemented, and the
parser must not guess a same-named method on the receiver or a global owner.

An operator-function-id may itself have explicit template arguments, including
the empty list in `operator!=<>(a, b)` and a member call such as
`box.template operator()<int>()`. Its operator token and template arguments
remain part of the callee name, not executable comparisons. Scope separators
inside those arguments must not rename the call to a template argument's leaf
type. Expression recovery checks recognize the same name boundary before
checking runtime operands; they must not discard a valid enclosing return or
replace its loop condition with a problem expression.

Template parameter `class` keywords and default arguments do not introduce a
record body or object initializer. Class-template declarations are traversed
to retain their member declarations and definitions. This does **not** add
template instantiation, substitution, specialization matching, dependent-name
type checking or complete C++ name lookup.

The parser's existing CFG lowering determines evaluation edges. These changes
retain each receiver and argument occurrence; they do not add or duplicate
runtime evaluation for template type arguments. A deterministic CFG traversal
of function arguments is not a claim that C++ specifies one universal
left-to-right argument evaluation order.

`tests/cpp_qualified_expressions.rs` checks call count, operand positions,
receiver-before-call edges, nested-call edges, explicit qualifier preservation,
comparison disambiguation and existing invalid-name diagnostics. Original
Joern graph references, when available, are a separate compatibility check;
self-produced snapshots cannot establish that compatibility.

## Requirements for callers evaluating decompiler output

Before treating a parse as measurable output:

1. Preserve the exact producer text and its language/dialect, preparation
   steps, tool version and native function identity. Keep missing output,
   missing definitions, ambiguity, unsupported syntax and infrastructure
   failure separate. Never join by a convenient same-named function.
2. Distinguish source C/C++ from an ABI pseudocode dialect. Explicit `this`
   parameters, address-valued pseudo-identifiers and explicit ABI arguments
   to member/operator calls do not automatically form legal source C++.
3. Account for producer-owned declaration context: typedefs, namespace/class
   declarations and external function signatures. Missing declarations can
   cause cascading compiler errors or prevent checking a body. Do not repair
   recovered output using ground-truth source types.
4. Keep known malformed output visible as a producer/transport failure. Better
   parser coverage can uncover a duplicate binding that was previously hidden
   by an unrelated unsupported template call; this is not new valid coverage.
5. Preserve unknown classifications. An isolated, declared C++ witness proves
   support for that syntax feature; it does not prove validity or correct CFG
   construction for a larger decompiled function containing similar text.
6. Do not delete calls, effects or branches, suppress diagnostics, or invent
   declarations merely to increase the eligible population. Any future
   pseudocode-name transport must preserve identity and expression structure.

These are integration requirements, not a newly implemented validation API or
an official C++ metric policy. No new adapter field, normalization step or
benchmark population rule is introduced by the expression-name change.

## Explicit destructor calls

After `.` or `->`, an explicit destructor-id such as `~Object` is one member
name. `pointer->~Object()`, `reference.~Object()` and
`receiver()->Box<int>::~Box()` retain a destructor call, with the original
receiver as argument zero. Receiver/index computations are evaluated once
through the existing expression and short-circuit CFG lowering. The tilde must
not become a field name followed by a separate constructor/function call.
Whitespace and the alternative `compl` token preserve their original source
code while the member name uses the canonical destructor marker `~`.

This uses the same typed expression-name recognizer as operator/template names.
Unary bitwise complement and `operator~()` remain separate syntax. Recognized
destructor names require an empty call; argument lists or a missing call produce
an unsupported diagnostic. Other malformed C++ still follows the existing
recovery/validation contract; this does not make strict mode a compiler.

This is explicit call preservation, not implicit scope-exit cleanup, virtual
lookup, object lifetime/unwinding or template-instantiation support. Source
slices lacking a class owner may still be unmeasurable. No class/field layout,
missing receiver declaration or hidden destruction is invented.
