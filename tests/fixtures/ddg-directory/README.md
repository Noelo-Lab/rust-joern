These two files were analyzed together by original Joern 4.0.150. The native
frontend previously analyzed them independently, adding two REACHING_DEF edges
between `external_body`'s `x` and `y` arguments in `caller.c`.

`original-capture.json` is the original live Joern capture,
`callee-context.json` records the original global method and stub identities,
and `joern-4.0.150.json` is the normalized capture used by the directory tests.
The scripts in `../dataflow-audit/` describe the capture and normalization.
Tests compare endpoint identities and labeled edge sets, so repeated identical
original REACHING_DEF edges and differing node IDs do not affect the result.

The oracle has exactly two methods. Joern removes `caller.c`'s declaration when
the matching definition is recovered from `body.c`; the directory tests check
that coverage as well as the raw overlay, DOT projection, and public graph.
Unresolved declarations and empty real definitions remain present.
