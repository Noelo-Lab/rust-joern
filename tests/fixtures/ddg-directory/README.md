These two files were analyzed together by original Joern 4.0.150. The native
frontend previously analyzed them independently, adding two REACHING_DEF edges
between `external_body`'s `x` and `y` arguments in `caller.c`.

`original-capture.json` is the original live Joern capture,
`callee-context.json` records the original global method and stub identities,
and `joern-4.0.150.json` is the normalized capture used by the directory tests.
The scripts in `../dataflow-audit/` describe the capture and normalization.
Tests compare endpoint identities and labeled edge sets, so repeated identical
original REACHING_DEF edges and differing node IDs do not affect the result.

The oracle has the two executable methods. The native frontend additionally
retains `caller.c`'s prototype; these tests verify DDG overlay and projection
parity for executable methods rather than the directory's full CPG schema.
