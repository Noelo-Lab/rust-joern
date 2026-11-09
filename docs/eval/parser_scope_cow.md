# Parser variable-scope snapshots

The parser keeps variable names, variable types and closure bindings in private
per-parser maps. Entering a block saves these maps so a declaration can shadow
an outer binding and the outer binding is restored afterwards. The old path
deep-copied all three maps when entering each block, even if the block never
changed them. These snapshots now share ownership and copy only on mutation.
The maps are parser-local; no mutable data is shared between analysis workers.

## Bounded validation

The baseline was main `9f8cad170d6b6883fe08627fa524ecb28128380d`, with the
same Rust toolchain, release profile, one Rayon thread, command and input
bytes on both builds. The candidate's only source change was scope snapshot
ownership. A local replay compared the full analysis payloads for 1,225
prepared C++ requests (1,223 distinct input hashes): 1,132 parsed, 59 rejected
and 34 returned no methods in both versions. There were no changed analysis
payloads or status changes. These are mostly decompiler output fragments,
not independent programs or a C++ language coverage measure.

Four whole C++ TUs in permissive mode, four C11 TUs and four selected C++
output fragments in strict mode produced identical full CLI stdout, stderr,
and exit codes. Existing frozen original-Joern fixtures `control.c`,
`expressions.c`, and `functions.cpp` matched 48/48 exported functions in CFG
topology, entry/exit roles and degeneracy. This does not establish CPG or
execution equivalence. A focused test checks nested callable shadowing.

Seven alternating, warm-cache CLI runs per C++ input gave median wall-time
reductions of 16–19% on three large TUs, all rejected by strict mode with
identical diagnostics. For four selected short C++ output fragments that
returned successfully, three medians improved 14–16%, while one increased
2.5%. Fifteen alternating runs on four successful zlib C11 TUs had medians
between 1% and 5% lower. Samples include CLI startup and JSON serialization;
millisecond-scale measurements are sensitive to host noise. No full-dataset,
multi-platform or end-to-end DecBench performance claim is made.

The 1,225 request texts and exact benchmark captures remain in local research
storage; source licensing and the historical compilation settings are not
established for every fragment. The project does not redistribute those inputs
with this change. The frozen Joern fixtures and test sources are checked in.
