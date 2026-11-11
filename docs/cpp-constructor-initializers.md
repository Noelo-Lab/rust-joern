# Experimental scalar constructor initialization

`parse_code(..., language="cpp", cpp_constructor_initializers=True)` enables a
CFG-only normal-completion profile. It is off by default and acknowledged in
the returned analysis; the Python wrapper rejects an older library that ignores
it. It is not enabled by the DecBench shim. Data-flow/reaching-definition
requests with this profile fail rather than applying unqualified operator rules.

The complete class declaration must occur in the parsed source. The parser
proves an ordered list of scalar or plain-pointer fields and requires exactly
one explicit parenthesized initializer expression for every field. It rejects
bases, unions, virtual members, references, arrays, bitfields, default member
initializers and non-scalar subobjects. A type name supplied through
A list of type names does not provide this proof. Unknown, duplicated or omitted
members and constructor definitions without class context remain errors.

The RHS subset includes known scalar identifiers, decimal, boolean and character
literals, bracketed forms, and previously declared global zero-argument
functions returning scalar values. A pointer initializer can use `nullptr` or
copy a named parameter in an inline constructor. The parameter and member
must have the same simple pointee spelling including cv qualifiers and one
plain pointer declarator. This evidence comes from the original declaration
tokens. Pointer conversions, pointer-member reads and out-of-class pointer
copies require additional evidence. Unknown calls, object conversions,
user-defined literals and general operator/argument sequencing remain
unsupported.

The syntax layer builds a typed `MemberInitialization` plan in member declaration
order, independently of initializer-list spelling. The builder emits distinct
`<operator>.fieldInitialization` nodes with explicit `this` member targets and
RHS arguments. Each RHS is evaluated once, each initialization precedes the
next, and the final initialization precedes the body. Source spans continue to
refer to the originally spelled initializer. Normal completion does not model
throwing edges, unwinding, hidden vptr operations or complete C++ lifetime/ABI
semantics. Parsing success does not certify valid producer C++ output.

## Corpus qualification

The constructor profile is experimental and must be measured separately from
default analysis. No broad corpus qualification is implied.
