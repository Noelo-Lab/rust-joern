These nine source definitions isolate CDT builtin expansion signatures and the
distinction between alignment type operands and value operands. Both sources
contain the same bytes; their extensions select C and C++ lowering.

The two `joern-4.0.150-*.json` captures came from the untouched Joern executable
installed with PyJoern 4.0.150.4. The capture imports the source, runs
`ossdataflow`, and records method AST properties, CFG, raw `REACHING_DEF` edges,
and reaching-definition sets. Numeric IDs were replaced with per-method dense
IDs. Source, capture-script, and installed-module hashes are recorded in each
file. Global initializers and declarations were excluded from this focused
capture; the existing frozen DDG fixtures compare complete public method sets.

Original source authority is Joern tag v4.0.150 (commit
`958fdd3d976197a783f8ade1254b43e977648f28`), notably
`c2cpg/astcreation/AstForExpressionsCreator.scala` and `MacroHandler.scala`.
The committed `Capture.sc` is the exact original capture script. Run it from
an isolated temporary directory with the unchanged installed executable:

```sh
/path/to/unchanged/joern --script /path/to/ddg-builtins/Capture.sc \
  --param target_dir=/path/to/ddg-builtins/builtins.cpp
```

The native regression checks source CODE, operator names, AST ancestry, and
complete CFG edges for all nine definitions, plus complete raw dependencies
for the four expression-operand cases. The existing frozen comparison checks
both public DDGs and labeled DOT graphs for all builtin regression fixtures.
