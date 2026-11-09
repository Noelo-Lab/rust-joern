# Qualified special-member recovery in isolated input

Default C++ parsing can retain an explicit `Owner::Owner()` or `Owner::~Owner()`
body when its class declaration is absent. The owner and special-member leaf
must agree exactly; unresolved template owners, ordinary methods with missing
return specifiers and mismatched destructor names are not newly admitted.

This restores Joern 4.0.150 recovery behavior on isolated real-source slices.
It does not establish that a slice is valid standalone C++. For example, Clang
rejects a destructor definition for an undeclared class even when the reference
frontend recovers its explicit body. The parser and benchmark must distinguish
recovery, declaration completeness and source validity.

Recovery does not insert a class binding, fields, implicit `this`, hidden
initializations or destructor cleanup. Explicit body expressions keep the
existing parser/CFG behavior. Qualified constructors with initializer lists
still report unsupported effects. This independent change does not introduce an
opt-in constructor profile.
References, subobjects, virtual operations, exceptions,
unwinding and complete lifetime behavior are not qualified by this change.

Regression fixtures are handwritten controls, with input hashes and captures
from unchanged PyJoern in an isolated interpreter. The isolated control is
intentionally incomplete C++; the complete control passes Clang syntax
checking. Comparison covers function presence, directed CFG topology,
entry/exit roles and degeneracy; it is not full CPG/DDG equality or execution
equivalence. Real corpus excerpts are evaluated locally and not redistributed
in this contribution.
DecBench's production parser pin, metrics and default score populations are not
changed by updating this Rust-Joern research branch.

## Local fixed-input replay

Against main `9f8cad170d6b6883fe08627fa524ecb28128380d`, a local 1,225-request
prepared C++ pool was replayed with default and Joern loop-condition options.
Both profiles produced the same seven changed analyses: six formerly empty
requests now expose an explicit body, and one formerly empty constructor is
rejected because its initializer effects are unsupported. No previously parsed
request was lost. These correlated excerpts are not a complete function
population, source-validity proof or DecBench score.
