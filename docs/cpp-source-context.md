# Optional type-name context for isolated C++ source

`parse_code(..., cpp_type_names=(...))` can retain visible type-name bindings
when a caller parses a function slice from a larger translation unit. This is
an optional lexical context, not a class/type-layout definition or proof of
complete C++ semantics. Empty context retains the existing behavior.

The Rust `Options.cpp_type_names` field carries the same contract through the
C ABI. Names must be identifier-shaped, optionally `::`-qualified, and require
C++ input. They seed the existing parser type-name lookup; ordinary object
bindings still shadow a name in a braced-construction expression. The context
carries no aliases, widths, fields, constructors, bases or function prototypes.
No CLI option or default DecBench policy is changed.

## Caller contract

- Derive names from the exact preprocessed TU and the definition's visible
  lexical scope. Retain TU/definition hashes, declaration locations, scope and
  ambiguities. A short-name search in unrelated files is not sufficient.
- Pass only bindings the caller can prove. The parser does not independently
  verify caller assertions or establish that a supplied name has a particular
  class definition. Templates/dependent lookup, namespace aliases and full C++
  visibility/overload rules require separate qualification.
- Never populate producer parsing with source/debug oracle names. A producer
  must supply its own context, and it must be distinguished from oracle context.
- Do not replace missing declarations with invented record definitions or
  prototype stubs. A lexical type name cannot earn object/layout type credit.
- Retain missing/ambiguous context as an explicit unsupported reason. Name
  context alone does not qualify constructor/destructor effects, member
  initialization, exceptions, full-TU CFGs or recovered-program execution.

For an exact source slice whose parent TU proves a visible `Options` type:

```python
analysis = parse_code(
    source_slice,
    filename="source.cpp",
    language="c++",
    preprocessed=True,
    strict=True,
    cpp_type_names=("Options",),
)
```

The returned native analysis acknowledges nonempty context in `cpp_type_names`.
The Python wrapper rejects an older library that silently ignores the option.
Ordinary analyses omit the field so existing serialized graphs remain intact.
Context-bearing graph/cache consumers must fingerprint the supplied names and
provenance, native library, wrapper, language, preprocessing and loop policy.

## Earlier prototype bounds

The following observations were collected before this standalone split; they
are historical evidence, not a corpus result for the current commit.

The motivating real-source slices are two Snappy 1.2.2 functions containing an
empty braced argument. Their exact preprocessed definitions recur in O0, O2 and
O2-noinline. The existing parser accepts the braced expressions with the real
header record, but isolated parsing loses the type name and reports errors.
The optional context restores parsing of the two unique definitions across six
recorded TU configurations without adding source text or class-layout data.
Directed CFG/entry-exit/degeneracy comparisons use isolated unchanged original
PyJoern captures. Nonempty/effectful list initialization is not newly qualified.

The local prototype leaves the DecBench pin, profile and all default scores
unchanged. Constructor member-initializer diagnostics remain unsupported. A
production source-context extractor and its ambiguity/visibility rules need
separate review; source context is not automatically inferred by this API.

On the independent main-based candidate, the two isolated Snappy snippets were
rechecked: each rejects with empty context and emits a function with the
`CompressionOptions` name supplied. This is a diagnostic experiment, not
qualified oracle evidence: exact enclosing-TU visibility of that name has not
been established for these slices, and no name is injected into a producer.
