# DecBench O0/O2 audit — 2026-10-07

The audit is complete; the Rust port does **not** have corpus parity yet. All 8,808 source, IDA and Kuna files in `full_run_address_2026-09-11` were compared with unchanged PyJoern references using identical prepared bytes. No graph divergence was waived. The native implementation was frozen during this audit.

| Input | Files | Full match | Successful extraction with divergence | Strict extraction failure |
| --- | ---: | ---: | ---: | ---: |
| O0/source | 3,763 | 435 | 655 | 2,673 |
| O0/ida | 256 | 112 | 55 | 89 |
| O0/kuna | 256 | 177 | 0 | 79 |
| O2/source | 4,019 | 445 | 661 | 2,913 |
| O2/ida | 257 | 138 | 34 | 85 |
| O2/kuna | 257 | 140 | 0 | 117 |
| **Total** | **8,808** | **1,447** | **1,405** | **5,956** |

A full match requires successful default strict usage, complete function-name coverage, exact directed CFG isomorphism preserving entry/exit roles, matching DecBench degeneracy, and no diagnostics. Empty original function maps are retained as valid references; there are 77 such files. Ten compiler probes excluded by the GED source filter were also audited and are marked in the manifest; they are excluded from the binary ownership maps.

Permissive retries retain graph evidence for all 5,956 strict failures. These graphs contribute to the topology counts below, but never turn a failed strict extraction into a successful file.

| Original graph class | Matched | Missing | Divergent |
| --- | ---: | ---: | ---: |
| Nondegenerate | 718,338 | 26,208 | 9,712 |
| Degenerate | 3,313,182 | 11,831 | 73 |

There are also **67,812 extra function names**. These are translation-unit occurrences: shared headers and declarations recur across many files. Degenerate graphs are not all prototypes; empty bodies can also normalize to a Nop block. The ownership results below provide a more useful view of DecBench source recovery.

DecBench’s actual source-selection helpers and DWARF owners were checked for all **513 published O0/O2 binaries and 60,704 named functions**. All 513 raw binary hashes match the published manifest. There were no missing binary or input errors. Original references resolve 57,499 function bodies; the other 3,205 names lack an original source CFG.

| Native profile | Matching original source bodies | Missing | Topology differences | Degeneracy differences |
| --- | ---: | ---: | ---: | ---: |
| Default strict | 12,724 | 44,663 | 110 | 2 |
| Permissive fallback evidence | 42,476 | 14,147 | 868 | 8 |

Existing published per-binary source JSON is a secondary consistency check. It differs from the hash-verified TU references resolved with current DecBench rules for 2,017 extra, 527 missing and 247 differing graphs. Confirmed examples retain a different cross-TU fallback: published Bash `bashversion.main` and `psize.main` use `shell.i.main`, and published bzip2 `bsPutUChar`/`bsPutUInt32` use `bzip2recover.i` rather than their DWARF owner. These reference-selection differences are recorded separately from Rust defects. They do not authorize any CFG waiver.

[Snapshot evidence](published-reference-audit.json) establishes that the example CFG files last changed on July 28, before the publisher's August 26 DWARF ownership fix. Of all 2,791 secondary discrepancies, 1,963 exactly reproduce the historical selection without DWARF; 527 current reference bodies are absent from published maps; and 301 shapes are absent from the current optimization-specific caches. The latter two groups remain unexplained snapshot differences. Five example source hashes confirm identical prepared bytes.

The native extraction batch took **296.17 seconds with 8 workers and 2 Rayon threads per worker**, including strict attempts, permissive retries, preparation and result writing. Final graph comparison took 135.54 seconds. Capturing the missing original references took 2,618.49 seconds with 8 independent original PyJoern workers.

| Input | Native attempt median / p95 | DecBench extraction median / p95 |
| --- | ---: | ---: |
| O0/source | 46.44 / 184.99 ms | 59.39 / 211.49 ms |
| O0/ida | 20.72 / 382.85 ms | 40.44 / 1072.31 ms |
| O0/kuna | 22.60 / 479.93 ms | 42.98 / 1043.89 ms |
| O2/source | 48.77 / 199.74 ms | 61.43 / 218.82 ms |
| O2/ida | 19.56 / 299.18 ms | 42.02 / 809.54 ms |
| O2/kuna | 15.13 / 349.49 ms | 30.26 / 700.14 ms |

This table includes failed strict attempts. The 2,852 successful strict extractions have a native median of 10.10 ms; the 1,447 fully matching files have a native median of 6.14 ms. These subsets have different inputs and sizes, so their timing is not a corpus-wide speed or accuracy claim.

Native timing measures the Rust ABI analysis, Rust JSON serialization and Python JSON decoding. CFG-only disables data flow but still constructs CPG. Python graph materialization, preparation, end-to-end extraction and audit graph serialization have separate per-file columns. Graph materialization is part of Python overhead, not an additional additive stage. Strict failures and their permissive retries have separate timings. The nearest-rank method computes p95.

Historical source caches do not record parser version or CFG generation time; missing measurements remain null. Fresh original reference times measure isolated CFG-only `parse_source`, including JVM startup, DOT export and PyJoern lifting. Identical bytes and language share one reference capture. No aggregate old/new speed ratio is inferred from these different workloads.

Eleven reduced C/C++ regressions with independently captured PyJoern 4.0.150.4 / Joern v4.0.150 snapshots are in [the fixture catalog](../../tests/fixtures/decbench-regressions/README.md). They reproduce K&R const-first definitions, global struct initializers, GNU and Microsoft assembly, terminal labels, indirect calls, C++ global qualification, unresolved goto recovery, nested designators, and C++ operator names. Their [tests](../../tests/test_decbench_regressions.py) are explicit expected failures until the port is fixed. The Python suite reports 17 passing checks and 11 expected failures; all 126 original smoke graphs still match. No parser fix was mixed into the measured run.

The reference set contains 5,363 original content-addressed source caches and 1,014 isolated fresh captures: 6,377 unique prepared inputs, covering all 8,808 files. The graph matcher is exact directed, role-colored BLISS through igraph 1.0.0; the audit extra is independent of the Rust runtime.

Useful records:

- [Summary and timing distributions](summary.json), [per-file CSV](files.csv), and [per-file diagnostics and measurements](files.jsonl.gz).
- [All function comparisons](functions.jsonl.gz) and [every divergence](divergences.jsonl.gz), including graph adjacency for missing, extra and differing functions and strict usage errors.
- [Binary ownership summary](ownership-summary.json) and [all 60,704 ownership comparisons](ownership.json.gz).
- [Input manifest with original/prepared hashes](manifest.json.gz), [oracle capture provenance](oracle-captures.json.gz), [artifact verification](verification.json), [checks](checks.json), and [environment](environment.json).

The generation inputs and full reference/candidate JSON remain under `workspace/decbench-parity`; their absolute locations are recorded in the report. The durable compressed report preserves graph differences, hashes, timings and ownership results. Reproduction commands are in the [project README](../../README.md). Audit commands return a nonzero status when they find divergences.
