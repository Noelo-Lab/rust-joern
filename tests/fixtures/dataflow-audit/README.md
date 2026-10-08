These fixtures freeze 82 original Joern 4.0.150 methods across six C/C++ sources.
The snapshots include original CFG/REF/REACHING_DEF edges, original solver in/out
sets, and `ddg_view` parsed from the original `method.dotDdg` output. Native
results were never used as expected graphs.

`joern-4.0.150-types.json` adds 108 original methods from
`type-metadata.{c,cpp}` and `type-detail.{c,cpp}`. These distinguish parameter
spelling, return display types, canonical C++ binding types, and unresolved
type recovery. `original-types-captures.json` and `types.provenance.json`
retain their independent capture output and hashes.

`constructors.cpp` and `joern-4.0.150-construction.json` add 11 original methods
for allocation, declared constructors, qualified type operands, and deletion.
They preserve the type operand's original IDENTIFIER kind even when its name
also names a constructor. `original-construction-captures.json` and
`construction.provenance.json` retain the raw capture and hashes.

`assignment-operators.c` and `joern-4.0.150-assignments.json` cover all ten
compound assignment operators, including the original frontend's distinct
`<operator>` and `<operators>` prefixes. Their native source lowering, raw
dependencies, solver sets, and original-CPG replay are checked independently.
`original-assignments-captures.json` and `assignments.provenance.json` retain
the original capture and its hashes.

The Rust solver tests replay original CPGs to check the analysis separately
from parsing. They require exact incoming/outgoing sets, raw REACHING_DEF
edges, projected vertices, and labeled projected edges. Source-to-native
comparisons also use the public DDG harness under `tests/fixtures/ddg-parity`.

`globals.cpp` covers global reads/writes and explicit/nested lambda captures;
`operators.c` covers pointer/field access paths, sizeof, and shadowing;
`control.c` covers unreachable code and control exits; `cpp_calls.cpp` covers
namespaces, member receivers, overloads, unresolved calls, and extern C;
`context.c` and `context.cpp` pin prototypes, empty bodies, and callee flags.

`internal_methods` contains original nonstub internal methods for reaching
definitions. `all_internal_methods` also contains original internal prototypes
and empty real definitions, required for DOT visibility. `callee-context.json`
records authoritative isExternal/isStub flags from the two context sources.

`original-captures.json` retains the original flatgraph IDs, raw DOT, and global
AST/capture diagnostics. `provenance.json` records source and capture hashes.

Recapture each source with an isolated original Joern 4.0.150 process and unique
working directory: `joern --script capturecontext.sc --param target_dir=<source>`.
Save output as `<prefix>-stdout.log`, then run `python3 normalize.py <prefix>`.
The normalizer only consumes original capture output.
