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

The initial RHS subset is deliberately small: known scalar/pointer identifiers,
plain decimal/boolean/string/character literals for non-pointer fields, bracketed forms, and
previously declared global zero-argument functions returning scalar/pointer
values. Pointer RHS types must agree with the member type apart from cv spelling;
unknown derived/base adjustments are refused. Unknown calls, object conversions,
user-defined literals and general
operator/argument sequencing remain unsupported. These bounds prevent inventing
implicit conversion effects from unknown expression types.

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
