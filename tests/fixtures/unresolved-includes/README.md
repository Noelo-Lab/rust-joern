These inputs and normalized CFG references were captured with the unchanged
installed PyJoern 4.0.150.4 / Joern v4.0.150, using Python `-I` and an isolated
Joern workspace. No compiler or decompiler ran. AST/CFG capture enabled metadata
and disabled DDG generation. Each reference records the exact input SHA256.

The reduced C/C++ inputs include missing angle, quoted, system and body headers.
The genuine Claude input is byte-identical to the saved DecBench O0/base-passwd
update-passwd output, SHA256
`2589cd9bb5f2863e6e75ac7c716d29bd1b46b76cd976190573ac1dfd048037e1`.
All three originals match their include-blanked controls across every normalized
CFG property, including topology, entry/exit roles and degeneracy.

Full raw evidence, transparent capture script, controls, provenance and hashes:
`workspace/ged-reeval/probes/include_semantics.summary.json` and
`include_semantics.SHA256SUMS` in the main working tree. These fixture references
are copied unchanged from that immutable evidence.
