These methods were captured from unchanged PyJoern 4.0.150.4 and its bundled
Joern v4.0.150, tag commit `958fdd3d976197a783f8ade1254b43e977648f28`.
`constructors.cpp` reduces the constructor selection in compiled NuttX
`libxx_typeinfo.ii`, and adds declaration-only constructors.
`joern-4.0.150.json` retains ordered raw methods,
original CPG properties and edges, original DOT, and the actual result of
`Function.from_many` with normalized CFG and DDG counts.
`using-namespace.cpp` contains namespace-owned `A` and `B` methods defined
outside the namespace after `using namespace N`, including const methods,
non-const methods, and destructors. Its corresponding ordering snapshot is
`joern-4.0.150-using-namespace.json`.

Run the installed original `joern` from a fresh temporary working directory:

```sh
joern --script /path/to/repo/tests/fixtures/ddg-classes/capture.sc \
  --param target_dir=/path/to/repo/tests/fixtures/ddg-classes/constructors.cpp \
  > capture.log
python /path/to/repo/tests/fixtures/ddg-classes/report.py capture.log \
  --source /path/to/repo/tests/fixtures/ddg-classes/constructors.cpp \
  --output /path/to/repo/tests/fixtures/ddg-classes/joern-4.0.150.json
```

Use an interpreter containing original PyJoern for `report.py`. The capture
does not sanitize the C++ source or select duplicate methods itself.
For standard method ASTs, CFG edges, and reaching-definition solver states,
capture either source with the sibling `ddg-lambdas/capture.sc` instead. That
script emits the `FLOW_JSON_START` schema used by graph regressions.

The source rules come from Joern v4.0.150:

- [FullNameProvider.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/FullNameProvider.scala):
  AST return type and signature come from declaration specifiers; bound full
  names come separately from CDT's function type. Empty constructor/destructor
  specifiers produce `ANY`. A virtual destructor declaration instead has AST
  return type `virtual`, although its bound full name ends in `:ANY()`.
- [AstForFunctionsCreator.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/AstForFunctionsCreator.scala):
  only definitions get an implicit `this` parameter, at index/order zero,
  `BY_VALUE`, with the owner's type and no pointer suffix. The bare constructor
  definitions in this fixture have no `CONSTRUCTOR` modifier.
- [AstCreationPass.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/passes/AstCreationPass.scala)
  and [FunctionDeclNodePass.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/passes/FunctionDeclNodePass.scala):
  unmatched declarations are emitted after definitions, as non-external methods
  with an `ANY` block. Definitions suppress declarations by bound full name.
Declaration iteration follows a Scala map and can differ from source order.

Every constructor in this fixture has one normalized CFG node. PyJoern's
`Function.from_many` only discards a later duplicate when its CFG has fewer
nodes, so a later declaration replaces a definition on ties. The selected
`type_info` is the default declaration at line 6, with an empty DDG. Likewise,
the declaration-only `Plain` overloads appear copy then default in Joern's
method iteration, selecting the default at line 18 despite reverse source order.

Captures of the exact prepared compiled inputs confirm `libxx_dynamic_cast`
selects its copy declaration at line 364, with full name
`std.type_info.type_info:ANY(std.type_info&)`, one `rhs` parameter, and three
public DDG nodes. `libxx_typeinfo` selects its default declaration at line 362,
`std.type_info.type_info:ANY()`, with no parameters and an empty DDG. Its copy
definition at line 563 also has one normalized CFG node and loses the tie.

In `using-namespace.cpp`, all methods have bound full names under `N.A` or
`N.B`, including `N.A.~A:ANY()` and `N.B.~B:ANY()`. Every implicit `this`
parameter has the qualified owner type `N.A` or `N.B`. Const `read_a` and
`read_b` rewrite `value` as `this->value`; their generated `this` identifiers
have pointer types `N.A*` and `N.B*`. Non-const `plain_a` and `plain_b` retain
bare `value` identifiers with unqualified owner-pointer types `A*` and `B*`,
even though the declared field type is `int`.

This follows
[AstForPrimitivesCreator.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/AstForPrimitivesCreator.scala)
`syntheticThisAccess`: owner scope matching compares the rendered owner type
to the current type/method name, and failed matching retains the expression
type. In
[AstCreatorHelper.scala](https://github.com/joernio/joern/blob/v4.0.150/joern-cli/frontends/c2cpg/src/main/scala/io/joern/c2cpg/astcreation/AstCreatorHelper.scala),
`typeForCPPASTIdExpression` renders `EvalMemberAccess` using its owner type
plus a pointer-dereference suffix.

`method-order-oracle.json` records the exact source literals from the parser
tests for block-scope prototypes and multiple top-level function declarators,
captured independently as C and C++. Each case includes its language, source
hash, and complete ordered method metadata. The original block-scope orders
are `f, start, first` in C and `f, first, start` in C++. The top-level orders
are `third, fourth, first, second` in C and `third, second, first, fourth` in
C++. These small declaration maps follow Java bucket iteration; the larger
constructor declaration map follows Scala's improved-hash trie order.

The `*-flow.json` snapshots use `../ddg-lambdas/capture.sc` and retain the full
per-method CPG, CFG edges, reaching-definition edges, and solver sets. Normalize
their FLOW logs with `../ddg-lambdas/normalize.py --filename constructors.cpp
--source constructors.cpp --output joern-4.0.150-constructors-flow.json`, supplying
the log as its first argument. Public snapshots under `references` retain both
the actual `Function.ddg` graph and the original labeled DOT projection.
