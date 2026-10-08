These fixtures were captured from unchanged PyJoern 4.0.150.4 and its bundled
Joern v4.0.150. The original sources are parsed as C++, without sanitization.

`lambdas.cpp` covers explicit and implicit captures, reference aliases, immediate
and bracketed invocation, nested closures, mutable closures, and captured locals.
`fields.cpp` covers implicit member access, shadowing, members declared later,
and captures of `this`. The raw snapshots include detached lambda methods that
PyJoern removes through its function-name filter. The public snapshots under
`references` preserve both `Function.ddg` and the labeled original DOT graph.

Joern 4.0.150 converts lambda bodies after popping their parameter scope. Body
parameters consequently have no `REF`, while captured identifiers retain the
surrounding binding's type. Capture lists create no extra executable AST children
or cross-method dependence edges. Raw snapshots omit `REF` targets outside each
method's AST, as the native property graphs use separate per-method node IDs.

Regenerate a raw snapshot from a temporary working directory with the original
installed `joern` executable:

```sh
joern --script /path/to/ddg-lambdas/capture.sc --param target_dir=/path/to/ddg-lambdas/lambdas.cpp > original.log
python /path/to/ddg-lambdas/normalize.py original.log --filename lambdas.cpp --output /path/to/ddg-lambdas/joern-4.0.150.json
```

The `using.cpp`, `namespaced.cpp`, and `method-names.json` inputs record the
original qualified C++ method names and signatures used by parser regressions.
