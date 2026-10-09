Unchanged genuine prepared saved DecBench inputs, captured through public
PyJoern 4.0.150.4 / Joern v4.0.150 under Python `-I` and isolated Joern workspaces.
No compiler or decompiler ran; references include exact source SHA256.

`literal_actual_r2dec.c` is O2/shadow/sulogin/r2dec (3675 bytes); its missing
double quote appears in the `dcgettext` call before the labels at the end of
`main`. Original CFG: 24 nodes, 37 edges.

`literal_actual_binja.c` is O2/bash/mksyntax/binja (5187 bytes); it contains
malformed embedded double quotes and quote/character fragments. All three
original methods (`main`, `addcchar`, `addcstr`) remain available, with CFG
counts 33/48, 9/12 and 13/17 respectively.

The installed bundled CDT lexer emits unterminated ordinary literal tokens up
to the first unspliced newline or EOF and reports a nonfatal scanner problem.
It retains exact source offsets. Function/statement recovery subsequently
operates on these original tokens; the native lexer never repairs source text.

Raw AST/CFG, reduced C/C++ controls, EOF controls, direct bundled-CDT token
dumps, capture script and original prepared provenance are archived under
`workspace/ged-reeval/probes` in the main working tree, with `literal_` names.
