These C and C++ sources were parsed byte-for-byte with the unchanged installed
PyJoern 4.0.150.4 / Joern v4.0.150. References preserve complete normalized
CFGs, entry/exit roles and degeneracy. Capture flags, source SHA-256, engine
identity and original timings are embedded in each reference.

The controls cover default language bindings, undefined vendor symbols, nested
inactive code, active source define/undef state, annotations before the first
conditional, empty object/function macros and generic recovery of a malformed
macro-colliding function header. The following function must survive.

Full raw AST/CFG and direct bundled CDT token evidence are archived under
`workspace/ged-reeval/probes/conditional-fallback` and
`workspace/ged-reeval/probes/conditional_empty_scoped.*`.
