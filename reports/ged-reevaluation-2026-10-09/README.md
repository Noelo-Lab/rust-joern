# Rust CFG GED reevaluation

**Status: completed audited execution.**

**Independent audit: core execution, canonical inputs, aggregate score tables and sample explorer data passed.**

The frozen release `e96d285dfec074ad5549f8b5a12b6ec0d8ce600b4de74ae2a1e607742fe27055` generated all 5,401 unique prepared source inputs, 7,293 main artifacts and 223 standalone sample artifacts. All stages completed with zero parser/metric errors. Main GED values changed on 795 function/identity pairs, were added on 494 and cleared on 383; standalone GED values changed on two targets. All 250 Astra target states agree between main `codex` and standalone `codex@gpt-6-astra`; non-GED facts and frozen memberships are preserved.

All percentages below are sums of perfect function counts divided by their authoritative shared measurable-function denominators. Per-binary percentages are never averaged.

A measured-value count differs from the shared denominator: a producer's failure on a function measurable by another producer remains a non-perfect miss. Tables use unchanged frozen preset membership.

All-identity tables retain every original column, including site-hidden `retdec`; their normalization gate includes those columns. Actual site tables are separate and apply the site's hidden/preset restrictions before normalization.

## Main

96,103 function rows in 770 groups; 16 identities; 250 frozen sample targets.

Non-GED facts and memberships preserved: `acdffd2b3adb8984cffc8ece02c0e6756b84ffe58eb2809982f5ce431e72732d`.

Covered GED slices: 7,293; applied measured entries: 543,118.

| Native stage | Planned inputs/artifacts | Status counts |
| --- | ---: | --- |
| source | 5,401 | {"ok": 5401} |
| candidate | 7,293 | {"ok": 7293} |

Metric errors: 0; missing/extra evaluated slices: 0/0.

| Identity | Baseline measured | Rust measured | Changed values | Added values | Dropped values |
| --- | ---: | ---: | ---: | ---: | ---: |
| angr | 88,559 | 88,559 | 76 | 0 | 0 |
| binja | 78,650 | 78,267 | 530 | 0 | 383 |
| claude-code | 245 | 246 | 1 | 1 | 0 |
| codex | 245 | 245 | 2 | 0 | 0 |
| codex@gpt-6-astra | 33,391 | 33,530 | 26 | 139 | 0 |
| dewolf | 15,179 | 15,191 | 0 | 12 | 0 |
| fission | 245 | 245 | 0 | 0 | 0 |
| ghidra | 82,666 | 82,666 | 2 | 0 | 0 |
| glaurung | 246 | 246 | 0 | 0 | 0 |
| ida | 87,308 | 87,308 | 0 | 0 | 0 |
| kuna | 88,577 | 88,577 | 0 | 0 | 0 |
| manifold | 141 | 141 | 1 | 0 | 0 |
| r2dec | 67,006 | 67,347 | 155 | 341 | 0 |
| reko | 153 | 154 | 0 | 1 | 0 |
| retdec | 231 | 231 | 0 | 0 | 0 |
| ventris | 165 | 165 | 2 | 0 | 0 |

### All identities, normalization off

**O0 (`unoptimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 13,900/32,762 (42.427%) | 13,898/32,762 (42.421%) | -0.006 | 32,657 | shown |
| binja | 10,623/32,762 (32.425%) | 10,606/32,762 (32.373%) | -0.052 | 28,029 | shown |
| claude-code | 69/32,762 (0.211%) | 69/32,762 (0.211%) | +0.000 | 100 | not shown |
| codex | 74/32,762 (0.226%) | 73/32,762 (0.223%) | -0.003 | 100 | not shown |
| codex@gpt-6-astra | 0/32,762 (0.000%) | 0/32,762 (0.000%) | +0.000 | 0 | not shown |
| dewolf | 1,587/32,762 (4.844%) | 1,588/32,762 (4.847%) | +0.003 | 7,153 | shown |
| fission | 37/32,762 (0.113%) | 37/32,762 (0.113%) | +0.000 | 99 | not shown |
| ghidra | 10,615/32,762 (32.400%) | 10,615/32,762 (32.400%) | +0.000 | 30,667 | shown |
| glaurung | 40/32,762 (0.122%) | 40/32,762 (0.122%) | +0.000 | 100 | not shown |
| ida | 15,062/32,762 (45.974%) | 15,062/32,762 (45.974%) | +0.000 | 32,475 | shown |
| kuna | 15,871/32,762 (48.443%) | 15,871/32,762 (48.443%) | +0.000 | 32,492 | shown |
| manifold | 7/32,762 (0.021%) | 7/32,762 (0.021%) | +0.000 | 34 | not shown |
| r2dec | 7,916/32,762 (24.162%) | 7,918/32,762 (24.168%) | +0.006 | 25,732 | shown |
| reko | 26/32,762 (0.079%) | 27/32,762 (0.082%) | +0.003 | 66 | not shown |
| retdec | 28/32,762 (0.085%) | 28/32,762 (0.085%) | +0.000 | 90 | shown |
| ventris | 32/32,762 (0.098%) | 32/32,762 (0.098%) | +0.000 | 42 | not shown |

**O2-noinline (`optimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 11,188/33,563 (33.334%) | 11,188/33,563 (33.334%) | +0.000 | 32,020 | shown |
| binja | 9,009/33,563 (26.842%) | 9,067/33,563 (27.015%) | +0.173 | 28,554 | shown |
| claude-code | 36/33,563 (0.107%) | 36/33,563 (0.107%) | +0.000 | 97 | not shown |
| codex | 32/33,563 (0.095%) | 32/33,563 (0.095%) | +0.000 | 97 | not shown |
| codex@gpt-6-astra | 21,835/33,563 (65.057%) | 21,934/33,563 (65.352%) | +0.295 | 33,530 | shown |
| dewolf | 990/33,563 (2.950%) | 991/33,563 (2.953%) | +0.003 | 4,918 | shown |
| fission | 12/33,563 (0.036%) | 12/33,563 (0.036%) | +0.000 | 97 | not shown |
| ghidra | 9,289/33,563 (27.676%) | 9,289/33,563 (27.676%) | +0.000 | 30,113 | shown |
| glaurung | 16/33,563 (0.048%) | 16/33,563 (0.048%) | +0.000 | 97 | not shown |
| ida | 11,438/33,563 (34.079%) | 11,438/33,563 (34.079%) | +0.000 | 31,449 | shown |
| kuna | 11,882/33,563 (35.402%) | 11,882/33,563 (35.402%) | +0.000 | 32,384 | shown |
| manifold | 5/33,563 (0.015%) | 5/33,563 (0.015%) | +0.000 | 74 | not shown |
| r2dec | 6,365/33,563 (18.964%) | 6,358/33,563 (18.943%) | -0.021 | 24,627 | shown |
| reko | 7/33,563 (0.021%) | 7/33,563 (0.021%) | +0.000 | 62 | not shown |
| retdec | 13/33,563 (0.039%) | 13/33,563 (0.039%) | +0.000 | 95 | shown |
| ventris | 25/33,563 (0.074%) | 26/33,563 (0.077%) | +0.003 | 84 | not shown |

**O2 (`inlined`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 7,700/24,737 (31.127%) | 7,699/24,737 (31.123%) | -0.004 | 23,882 | shown |
| binja | 6,589/24,737 (26.636%) | 6,612/24,737 (26.729%) | +0.093 | 21,684 | shown |
| claude-code | 30/24,737 (0.121%) | 30/24,737 (0.121%) | +0.000 | 49 | not shown |
| codex | 27/24,737 (0.109%) | 27/24,737 (0.109%) | +0.000 | 48 | not shown |
| codex@gpt-6-astra | 0/24,737 (0.000%) | 0/24,737 (0.000%) | +0.000 | 0 | not shown |
| dewolf | 743/24,737 (3.004%) | 744/24,737 (3.008%) | +0.004 | 3,120 | shown |
| fission | 16/24,737 (0.065%) | 16/24,737 (0.065%) | +0.000 | 49 | not shown |
| ghidra | 6,003/24,737 (24.267%) | 6,003/24,737 (24.267%) | +0.000 | 21,886 | shown |
| glaurung | 13/24,737 (0.053%) | 13/24,737 (0.053%) | +0.000 | 49 | not shown |
| ida | 7,808/24,737 (31.564%) | 7,808/24,737 (31.564%) | +0.000 | 23,384 | shown |
| kuna | 7,809/24,737 (31.568%) | 7,809/24,737 (31.568%) | +0.000 | 23,701 | shown |
| manifold | 9/24,737 (0.036%) | 9/24,737 (0.036%) | +0.000 | 33 | not shown |
| r2dec | 4,182/24,737 (16.906%) | 4,172/24,737 (16.865%) | -0.040 | 16,988 | shown |
| reko | 7/24,737 (0.028%) | 7/24,737 (0.028%) | +0.000 | 26 | not shown |
| retdec | 9/24,737 (0.036%) | 9/24,737 (0.036%) | +0.000 | 46 | shown |
| ventris | 20/24,737 (0.081%) | 20/24,737 (0.081%) | +0.000 | 39 | not shown |

**Large functions across optimization levels (`large`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 8/1,927 (0.415%) | 8/1,927 (0.415%) | +0.000 | 1,770 | shown |
| binja | 8/1,927 (0.415%) | 8/1,927 (0.415%) | +0.000 | 1,837 | shown |
| claude-code | 10/1,927 (0.519%) | 10/1,927 (0.519%) | +0.000 | 46 | not shown |
| codex | 6/1,927 (0.311%) | 6/1,927 (0.311%) | +0.000 | 46 | not shown |
| codex@gpt-6-astra | 195/1,927 (10.119%) | 195/1,927 (10.119%) | +0.000 | 1,921 | not shown |
| dewolf | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 161 | shown |
| fission | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 46 | not shown |
| ghidra | 7/1,927 (0.363%) | 7/1,927 (0.363%) | +0.000 | 1,862 | shown |
| glaurung | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 46 | not shown |
| ida | 13/1,927 (0.675%) | 13/1,927 (0.675%) | +0.000 | 1,879 | shown |
| kuna | 20/1,927 (1.038%) | 20/1,927 (1.038%) | +0.000 | 1,887 | shown |
| manifold | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 43 | not shown |
| r2dec | 4/1,927 (0.208%) | 4/1,927 (0.208%) | +0.000 | 1,508 | shown |
| reko | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 31 | not shown |
| retdec | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 46 | shown |
| ventris | 5/1,927 (0.259%) | 6/1,927 (0.311%) | +0.052 | 44 | not shown |

**Frozen 250 targets across optimization levels (`sample-set`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 69/246 (28.049%) | 69/246 (28.049%) | +0.000 | 234 | shown |
| binja | 62/246 (25.203%) | 62/246 (25.203%) | +0.000 | 234 | shown |
| claude-code | 135/246 (54.878%) | 135/246 (54.878%) | +0.000 | 246 | shown |
| codex | 133/246 (54.065%) | 132/246 (53.659%) | -0.407 | 245 | shown |
| codex@gpt-6-astra | 31/246 (12.602%) | 31/246 (12.602%) | +0.000 | 97 | not shown |
| dewolf | 13/246 (5.285%) | 13/246 (5.285%) | +0.000 | 38 | shown |
| fission | 65/246 (26.423%) | 65/246 (26.423%) | +0.000 | 245 | shown |
| ghidra | 55/246 (22.358%) | 55/246 (22.358%) | +0.000 | 239 | shown |
| glaurung | 69/246 (28.049%) | 69/246 (28.049%) | +0.000 | 246 | shown |
| ida | 68/246 (27.642%) | 68/246 (27.642%) | +0.000 | 239 | shown |
| kuna | 80/246 (32.520%) | 80/246 (32.520%) | +0.000 | 243 | shown |
| manifold | 21/246 (8.537%) | 21/246 (8.537%) | +0.000 | 141 | shown |
| r2dec | 45/246 (18.293%) | 45/246 (18.293%) | +0.000 | 202 | shown |
| reko | 40/246 (16.260%) | 41/246 (16.667%) | +0.407 | 154 | shown |
| retdec | 50/246 (20.325%) | 50/246 (20.325%) | +0.000 | 231 | shown |
| ventris | 77/246 (31.301%) | 78/246 (31.707%) | +0.407 | 165 | shown |

### All identities, normalization on

**O0 (`unoptimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 31/72 (43.056%) | 31/72 (43.056%) | +0.000 | 72 | shown |
| binja | 34/72 (47.222%) | 34/72 (47.222%) | +0.000 | 71 | shown |
| claude-code | 50/72 (69.444%) | 50/72 (69.444%) | +0.000 | 72 | not shown |
| codex | 53/72 (73.611%) | 52/72 (72.222%) | -1.389 | 72 | not shown |
| codex@gpt-6-astra | 0/72 (0.000%) | 0/72 (0.000%) | +0.000 | 0 | not shown |
| dewolf | 6/72 (8.333%) | 6/72 (8.333%) | +0.000 | 16 | shown |
| fission | 28/72 (38.889%) | 28/72 (38.889%) | +0.000 | 71 | not shown |
| ghidra | 23/72 (31.944%) | 23/72 (31.944%) | +0.000 | 72 | shown |
| glaurung | 27/72 (37.500%) | 27/72 (37.500%) | +0.000 | 72 | not shown |
| ida | 33/72 (45.833%) | 33/72 (45.833%) | +0.000 | 72 | shown |
| kuna | 39/72 (54.167%) | 39/72 (54.167%) | +0.000 | 72 | shown |
| manifold | 6/72 (8.333%) | 6/72 (8.333%) | +0.000 | 30 | not shown |
| r2dec | 22/72 (30.556%) | 22/72 (30.556%) | +0.000 | 72 | shown |
| reko | 23/72 (31.944%) | 24/72 (33.333%) | +1.389 | 51 | not shown |
| retdec | 21/72 (29.167%) | 21/72 (29.167%) | +0.000 | 72 | shown |
| ventris | 30/72 (41.667%) | 30/72 (41.667%) | +0.000 | 37 | not shown |

**O2-noinline (`optimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 9/58 (15.517%) | 9/58 (15.517%) | +0.000 | 58 | shown |
| binja | 7/58 (12.069%) | 7/58 (12.069%) | +0.000 | 58 | shown |
| claude-code | 24/58 (41.379%) | 24/58 (41.379%) | +0.000 | 58 | not shown |
| codex | 19/58 (32.759%) | 19/58 (32.759%) | +0.000 | 58 | not shown |
| codex@gpt-6-astra | 18/58 (31.034%) | 18/58 (31.034%) | +0.000 | 58 | shown |
| dewolf | 2/58 (3.448%) | 2/58 (3.448%) | +0.000 | 8 | shown |
| fission | 8/58 (13.793%) | 8/58 (13.793%) | +0.000 | 58 | not shown |
| ghidra | 6/58 (10.345%) | 6/58 (10.345%) | +0.000 | 58 | shown |
| glaurung | 9/58 (15.517%) | 9/58 (15.517%) | +0.000 | 58 | not shown |
| ida | 7/58 (12.069%) | 7/58 (12.069%) | +0.000 | 58 | shown |
| kuna | 6/58 (10.345%) | 6/58 (10.345%) | +0.000 | 58 | shown |
| manifold | 3/58 (5.172%) | 3/58 (5.172%) | +0.000 | 48 | not shown |
| r2dec | 6/58 (10.345%) | 6/58 (10.345%) | +0.000 | 58 | shown |
| reko | 4/58 (6.897%) | 4/58 (6.897%) | +0.000 | 46 | not shown |
| retdec | 7/58 (12.069%) | 7/58 (12.069%) | +0.000 | 58 | shown |
| ventris | 16/58 (27.586%) | 17/58 (29.310%) | +1.724 | 54 | not shown |

**O2 (`inlined`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 6/30 (20.000%) | 6/30 (20.000%) | +0.000 | 30 | shown |
| binja | 6/30 (20.000%) | 6/30 (20.000%) | +0.000 | 30 | shown |
| claude-code | 17/30 (56.667%) | 17/30 (56.667%) | +0.000 | 30 | not shown |
| codex | 16/30 (53.333%) | 16/30 (53.333%) | +0.000 | 30 | not shown |
| codex@gpt-6-astra | 0/30 (0.000%) | 0/30 (0.000%) | +0.000 | 0 | not shown |
| dewolf | 2/30 (6.667%) | 2/30 (6.667%) | +0.000 | 6 | shown |
| fission | 9/30 (30.000%) | 9/30 (30.000%) | +0.000 | 30 | not shown |
| ghidra | 4/30 (13.333%) | 4/30 (13.333%) | +0.000 | 30 | shown |
| glaurung | 6/30 (20.000%) | 6/30 (20.000%) | +0.000 | 30 | not shown |
| ida | 6/30 (20.000%) | 6/30 (20.000%) | +0.000 | 30 | shown |
| kuna | 6/30 (20.000%) | 6/30 (20.000%) | +0.000 | 30 | shown |
| manifold | 5/30 (16.667%) | 5/30 (16.667%) | +0.000 | 25 | not shown |
| r2dec | 5/30 (16.667%) | 5/30 (16.667%) | +0.000 | 30 | shown |
| reko | 2/30 (6.667%) | 2/30 (6.667%) | +0.000 | 18 | not shown |
| retdec | 5/30 (16.667%) | 5/30 (16.667%) | +0.000 | 30 | shown |
| ventris | 11/30 (36.667%) | 11/30 (36.667%) | +0.000 | 27 | not shown |

**Large functions across optimization levels (`large`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| binja | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| claude-code | 8/27 (29.630%) | 8/27 (29.630%) | +0.000 | 27 | not shown |
| codex | 4/27 (14.815%) | 4/27 (14.815%) | +0.000 | 27 | not shown |
| codex@gpt-6-astra | 4/27 (14.815%) | 4/27 (14.815%) | +0.000 | 27 | not shown |
| dewolf | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 0 | shown |
| fission | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | not shown |
| ghidra | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| glaurung | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | not shown |
| ida | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| kuna | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| manifold | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 26 | not shown |
| r2dec | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| reko | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 22 | not shown |
| retdec | 0/27 (0.000%) | 0/27 (0.000%) | +0.000 | 27 | shown |
| ventris | 3/27 (11.111%) | 4/27 (14.815%) | +3.704 | 26 | not shown |

**Frozen 250 targets across optimization levels (`sample-set`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 17/90 (18.889%) | 17/90 (18.889%) | +0.000 | 90 | shown |
| binja | 14/90 (15.556%) | 14/90 (15.556%) | +0.000 | 90 | shown |
| claude-code | 40/90 (44.444%) | 40/90 (44.444%) | +0.000 | 90 | shown |
| codex | 37/90 (41.111%) | 37/90 (41.111%) | +0.000 | 90 | shown |
| codex@gpt-6-astra | 11/90 (12.222%) | 11/90 (12.222%) | +0.000 | 44 | not shown |
| dewolf | 2/90 (2.222%) | 2/90 (2.222%) | +0.000 | 10 | shown |
| fission | 18/90 (20.000%) | 18/90 (20.000%) | +0.000 | 90 | shown |
| ghidra | 10/90 (11.111%) | 10/90 (11.111%) | +0.000 | 90 | shown |
| glaurung | 17/90 (18.889%) | 17/90 (18.889%) | +0.000 | 90 | shown |
| ida | 18/90 (20.000%) | 18/90 (20.000%) | +0.000 | 90 | shown |
| kuna | 16/90 (17.778%) | 16/90 (17.778%) | +0.000 | 90 | shown |
| manifold | 11/90 (12.222%) | 11/90 (12.222%) | +0.000 | 90 | shown |
| r2dec | 10/90 (11.111%) | 10/90 (11.111%) | +0.000 | 90 | shown |
| reko | 12/90 (13.333%) | 12/90 (13.333%) | +0.000 | 80 | shown |
| retdec | 11/90 (12.222%) | 11/90 (12.222%) | +0.000 | 90 | shown |
| ventris | 38/90 (42.222%) | 39/90 (43.333%) | +1.111 | 90 | shown |

<details>
<summary>Actual site tables and normalization gate</summary>

These tables remove site-hidden identities before computing shared denominators. Only identities shown by each preset appear.

**O0, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 13,900/32,762 (42.427%) | 13,898/32,762 (42.421%) | -0.006 | 32,657 | shown |
| binja | 10,623/32,762 (32.425%) | 10,606/32,762 (32.373%) | -0.052 | 28,029 | shown |
| dewolf | 1,587/32,762 (4.844%) | 1,588/32,762 (4.847%) | +0.003 | 7,153 | shown |
| ghidra | 10,615/32,762 (32.400%) | 10,615/32,762 (32.400%) | +0.000 | 30,667 | shown |
| ida | 15,062/32,762 (45.974%) | 15,062/32,762 (45.974%) | +0.000 | 32,475 | shown |
| kuna | 15,871/32,762 (48.443%) | 15,871/32,762 (48.443%) | +0.000 | 32,492 | shown |
| r2dec | 7,916/32,762 (24.162%) | 7,918/32,762 (24.168%) | +0.006 | 25,732 | shown |

**O2-noinline, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 11,188/33,563 (33.334%) | 11,188/33,563 (33.334%) | +0.000 | 32,020 | shown |
| binja | 9,009/33,563 (26.842%) | 9,067/33,563 (27.015%) | +0.173 | 28,554 | shown |
| codex@gpt-6-astra | 21,835/33,563 (65.057%) | 21,934/33,563 (65.352%) | +0.295 | 33,530 | shown |
| dewolf | 990/33,563 (2.950%) | 991/33,563 (2.953%) | +0.003 | 4,918 | shown |
| ghidra | 9,289/33,563 (27.676%) | 9,289/33,563 (27.676%) | +0.000 | 30,113 | shown |
| ida | 11,438/33,563 (34.079%) | 11,438/33,563 (34.079%) | +0.000 | 31,449 | shown |
| kuna | 11,882/33,563 (35.402%) | 11,882/33,563 (35.402%) | +0.000 | 32,384 | shown |
| r2dec | 6,365/33,563 (18.964%) | 6,358/33,563 (18.943%) | -0.021 | 24,627 | shown |

**O2, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 7,700/24,737 (31.127%) | 7,699/24,737 (31.123%) | -0.004 | 23,882 | shown |
| binja | 6,589/24,737 (26.636%) | 6,612/24,737 (26.729%) | +0.093 | 21,684 | shown |
| dewolf | 743/24,737 (3.004%) | 744/24,737 (3.008%) | +0.004 | 3,120 | shown |
| ghidra | 6,003/24,737 (24.267%) | 6,003/24,737 (24.267%) | +0.000 | 21,886 | shown |
| ida | 7,808/24,737 (31.564%) | 7,808/24,737 (31.564%) | +0.000 | 23,384 | shown |
| kuna | 7,809/24,737 (31.568%) | 7,809/24,737 (31.568%) | +0.000 | 23,701 | shown |
| r2dec | 4,182/24,737 (16.906%) | 4,172/24,737 (16.865%) | -0.040 | 16,988 | shown |

**Large functions across optimization levels, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 8/1,927 (0.415%) | 8/1,927 (0.415%) | +0.000 | 1,770 | shown |
| binja | 8/1,927 (0.415%) | 8/1,927 (0.415%) | +0.000 | 1,837 | shown |
| dewolf | 0/1,927 (0.000%) | 0/1,927 (0.000%) | +0.000 | 161 | shown |
| ghidra | 7/1,927 (0.363%) | 7/1,927 (0.363%) | +0.000 | 1,862 | shown |
| ida | 13/1,927 (0.675%) | 13/1,927 (0.675%) | +0.000 | 1,879 | shown |
| kuna | 20/1,927 (1.038%) | 20/1,927 (1.038%) | +0.000 | 1,887 | shown |
| r2dec | 4/1,927 (0.208%) | 4/1,927 (0.208%) | +0.000 | 1,508 | shown |

**Frozen 250 targets across optimization levels, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 69/246 (28.049%) | 69/246 (28.049%) | +0.000 | 234 | shown |
| binja | 62/246 (25.203%) | 62/246 (25.203%) | +0.000 | 234 | shown |
| claude-code | 135/246 (54.878%) | 135/246 (54.878%) | +0.000 | 246 | shown |
| codex | 133/246 (54.065%) | 132/246 (53.659%) | -0.407 | 245 | shown |
| dewolf | 13/246 (5.285%) | 13/246 (5.285%) | +0.000 | 38 | shown |
| fission | 65/246 (26.423%) | 65/246 (26.423%) | +0.000 | 245 | shown |
| ghidra | 55/246 (22.358%) | 55/246 (22.358%) | +0.000 | 239 | shown |
| glaurung | 69/246 (28.049%) | 69/246 (28.049%) | +0.000 | 246 | shown |
| ida | 68/246 (27.642%) | 68/246 (27.642%) | +0.000 | 239 | shown |
| kuna | 80/246 (32.520%) | 80/246 (32.520%) | +0.000 | 243 | shown |
| manifold | 21/246 (8.537%) | 21/246 (8.537%) | +0.000 | 141 | shown |
| r2dec | 45/246 (18.293%) | 45/246 (18.293%) | +0.000 | 202 | shown |
| reko | 40/246 (16.260%) | 41/246 (16.667%) | +0.407 | 154 | shown |
| ventris | 77/246 (31.301%) | 78/246 (31.707%) | +0.407 | 165 | shown |

**O0, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 5,755/15,591 (36.912%) | 5,754/15,591 (36.906%) | -0.006 | 15,591 | shown |
| binja | 5,213/15,591 (33.436%) | 5,199/15,591 (33.346%) | -0.090 | 15,250 | shown |
| dewolf | 1,089/15,591 (6.985%) | 1,090/15,591 (6.991%) | +0.006 | 5,336 | shown |
| ghidra | 4,451/15,591 (28.549%) | 4,451/15,591 (28.549%) | +0.000 | 15,591 | shown |
| ida | 6,435/15,591 (41.274%) | 6,435/15,591 (41.274%) | +0.000 | 15,558 | shown |
| kuna | 6,909/15,591 (44.314%) | 6,909/15,591 (44.314%) | +0.000 | 15,591 | shown |
| r2dec | 3,960/15,591 (25.399%) | 3,964/15,591 (25.425%) | +0.026 | 15,435 | shown |

**O2-noinline, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 3,365/11,377 (29.577%) | 3,365/11,377 (29.577%) | +0.000 | 11,376 | shown |
| binja | 3,315/11,377 (29.138%) | 3,344/11,377 (29.393%) | +0.255 | 11,353 | shown |
| codex@gpt-6-astra | 7,176/11,377 (63.075%) | 7,193/11,377 (63.224%) | +0.149 | 11,369 | shown |
| dewolf | 647/11,377 (5.687%) | 648/11,377 (5.696%) | +0.009 | 3,493 | shown |
| ghidra | 3,051/11,377 (26.817%) | 3,051/11,377 (26.817%) | +0.000 | 11,373 | shown |
| ida | 3,609/11,377 (31.722%) | 3,609/11,377 (31.722%) | +0.000 | 11,352 | shown |
| kuna | 3,613/11,377 (31.757%) | 3,613/11,377 (31.757%) | +0.000 | 11,377 | shown |
| r2dec | 2,466/11,377 (21.675%) | 2,463/11,377 (21.649%) | -0.026 | 11,167 | shown |

**O2, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 2,019/6,970 (28.967%) | 2,019/6,970 (28.967%) | +0.000 | 6,970 | shown |
| binja | 2,063/6,970 (29.598%) | 2,069/6,970 (29.684%) | +0.086 | 6,964 | shown |
| dewolf | 464/6,970 (6.657%) | 465/6,970 (6.671%) | +0.014 | 2,050 | shown |
| ghidra | 1,836/6,970 (26.341%) | 1,836/6,970 (26.341%) | +0.000 | 6,968 | shown |
| ida | 2,230/6,970 (31.994%) | 2,230/6,970 (31.994%) | +0.000 | 6,956 | shown |
| kuna | 2,100/6,970 (30.129%) | 2,100/6,970 (30.129%) | +0.000 | 6,970 | shown |
| r2dec | 1,566/6,970 (22.468%) | 1,559/6,970 (22.367%) | -0.100 | 6,785 | shown |

**Large functions across optimization levels, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 0/730 (0.000%) | 0/730 (0.000%) | +0.000 | 730 | shown |
| binja | 2/730 (0.274%) | 2/730 (0.274%) | +0.000 | 727 | shown |
| dewolf | 0/730 (0.000%) | 0/730 (0.000%) | +0.000 | 116 | shown |
| ghidra | 1/730 (0.137%) | 1/730 (0.137%) | +0.000 | 728 | shown |
| ida | 3/730 (0.411%) | 3/730 (0.411%) | +0.000 | 725 | shown |
| kuna | 11/730 (1.507%) | 11/730 (1.507%) | +0.000 | 730 | shown |
| r2dec | 0/730 (0.000%) | 0/730 (0.000%) | +0.000 | 718 | shown |

**Frozen 250 targets across optimization levels, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| angr | 17/90 (18.889%) | 17/90 (18.889%) | +0.000 | 90 | shown |
| binja | 14/90 (15.556%) | 14/90 (15.556%) | +0.000 | 90 | shown |
| claude-code | 40/90 (44.444%) | 40/90 (44.444%) | +0.000 | 90 | shown |
| codex | 37/90 (41.111%) | 37/90 (41.111%) | +0.000 | 90 | shown |
| dewolf | 2/90 (2.222%) | 2/90 (2.222%) | +0.000 | 10 | shown |
| fission | 18/90 (20.000%) | 18/90 (20.000%) | +0.000 | 90 | shown |
| ghidra | 10/90 (11.111%) | 10/90 (11.111%) | +0.000 | 90 | shown |
| glaurung | 17/90 (18.889%) | 17/90 (18.889%) | +0.000 | 90 | shown |
| ida | 18/90 (20.000%) | 18/90 (20.000%) | +0.000 | 90 | shown |
| kuna | 16/90 (17.778%) | 16/90 (17.778%) | +0.000 | 90 | shown |
| manifold | 11/90 (12.222%) | 11/90 (12.222%) | +0.000 | 90 | shown |
| r2dec | 10/90 (11.111%) | 10/90 (11.111%) | +0.000 | 90 | shown |
| reko | 12/90 (13.333%) | 12/90 (13.333%) | +0.000 | 80 | shown |
| ventris | 38/90 (42.222%) | 39/90 (43.333%) | +1.111 | 90 | shown |

</details>

## Sample

250 function rows in 223 groups; 1 identities; 250 frozen sample targets.

Non-GED facts and memberships preserved: `4a76ed6e771a56d09aae0c3c9353fc4f6f18f73a60a6c4ab858c8a075491823a`.

Covered GED slices: 223; applied measured entries: 245.

| Native stage | Planned inputs/artifacts | Status counts |
| --- | ---: | --- |
| source | 5,401 | {"ok": 5401} |
| candidate | 223 | {"ok": 223} |

Metric errors: 0; missing/extra evaluated slices: 0/0.

| Identity | Baseline measured | Rust measured | Changed values | Added values | Dropped values |
| --- | ---: | ---: | ---: | ---: | ---: |
| codex@gpt-6-astra | 245 | 245 | 2 | 0 | 0 |

### All identities, normalization off

**O0 (`unoptimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 74/100 (74.000%) | 73/100 (73.000%) | -1.000 | 100 | not shown |

**O2-noinline (`optimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 32/97 (32.990%) | 32/97 (32.990%) | +0.000 | 97 | shown |

**O2 (`inlined`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 27/48 (56.250%) | 27/48 (56.250%) | +0.000 | 48 | not shown |

**Large functions across optimization levels (`large`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 1/25 (4.000%) | 1/25 (4.000%) | +0.000 | 25 | not shown |

**Frozen 250 targets across optimization levels (`sample-set`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 133/245 (54.286%) | 132/245 (53.878%) | -0.408 | 245 | not shown |

### All identities, normalization on

**O0 (`unoptimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 74/100 (74.000%) | 73/100 (73.000%) | -1.000 | 100 | not shown |

**O2-noinline (`optimized`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 32/97 (32.990%) | 32/97 (32.990%) | +0.000 | 97 | shown |

**O2 (`inlined`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 27/48 (56.250%) | 27/48 (56.250%) | +0.000 | 48 | not shown |

**Large functions across optimization levels (`large`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 1/25 (4.000%) | 1/25 (4.000%) | +0.000 | 25 | not shown |

**Frozen 250 targets across optimization levels (`sample-set`)**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 133/245 (54.286%) | 132/245 (53.878%) | -0.408 | 245 | not shown |

<details>
<summary>Actual site tables and normalization gate</summary>

These tables remove site-hidden identities before computing shared denominators. Only identities shown by each preset appear.

**O0, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**O2-noinline, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 32/97 (32.990%) | 32/97 (32.990%) | +0.000 | 97 | shown |

**O2, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**Large functions across optimization levels, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**Frozen 250 targets across optimization levels, normalization off**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**O0, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**O2-noinline, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |
| codex@gpt-6-astra | 32/97 (32.990%) | 32/97 (32.990%) | +0.000 | 97 | shown |

**O2, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**Large functions across optimization levels, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

**Frozen 250 targets across optimization levels, normalization on**

| Identity | Baseline pass/denominator (%) | Rust pass/denominator (%) | Δ percentage points | Rust measured values | Site row |
| --- | ---: | ---: | ---: | ---: | --- |

</details>

## Astra sample synchronization

Only frozen `(opt, project, binary, function)` keys map main `codex` to standalone `codex@gpt-6-astra`. The main optimized `codex@gpt-6-astra` column remains independent.

| Version | Targets | Equal GED states | Different GED states | Main measured | Standalone measured |
| --- | ---: | ---: | ---: | ---: | ---: |
| baseline | 250 | 250 | 0 | 245 | 245 |
| new | 250 | 250 | 0 | 245 | 245 |

The published sample score uses the main sample-set shared denominator; the standalone measured count never substitutes for it.

| Scope | Normalize | Baseline | Rust |
| --- | --- | ---: | ---: |
| main `codex` | off | 133/246 (54.065%) | 132/246 (53.659%) |
| main `codex` | on | 37/90 (41.111%) | 37/90 (41.111%) |
| sample `codex@gpt-6-astra` | off | 133/245 (54.286%) | 132/245 (53.878%) |
| sample `codex@gpt-6-astra` | on | 133/245 (54.286%) | 132/245 (53.878%) |

## Measured CFG generation and pipeline time

| Interval | Elapsed wall seconds | Summed worker seconds | Observations | Median milliseconds | P95 milliseconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| External CLI wall, including initial setup | 541.430 | n/a | n/a | n/a | n/a |
| Instrumented inventory→source→main→standalone pipeline | 540.841 | n/a | n/a | n/a | n/a |
| Input inventory | 76.451 | n/a | n/a | n/a | n/a |
| Unique source phase, including worker startup/cache writes | 34.801 | n/a | n/a | n/a | n/a |
| Unique source CFG generation | n/a | 502.792 | 5401 | 93.372 | 217.045 |
| Unique source native call, including JSON decoding | n/a | 356.770 | 5401 | 66.430 | 155.960 |
| Unique source complete Python parse_source call (nested) | n/a | 467.231 | 5401 | 87.074 | 200.231 |
| Unique source Python graph materialization (nested) | n/a | 70.589 | 5401 | 10.822 | 37.650 |
| Source cache assembly | 55.278 | n/a | n/a | n/a | n/a |
| Ownership precompute (770 unique queries) | 15.693 | 188.307 | 770 | 89.755 | 849.685 |
| DWARF traversal (nested within ownership precompute) | n/a | 186.524 | 770 | 89.226 | 844.395 |
| main: complete scoring phase | 324.165 | n/a | n/a | n/a | n/a |
| main: CFG generation | n/a | 1,765.630 | 7293 | 20.855 | 1042.494 |
| main: native call, including JSON decoding (nested) | n/a | 745.813 | 7293 | 10.630 | 436.434 |
| main: complete Python parse_source call (nested) | n/a | 1,290.181 | 7293 | 12.477 | 745.344 |
| main: Python graph materialization (nested) | n/a | 492.854 | 7293 | 1.178 | 200.072 |
| main: owner replay/source selection | n/a | 18.358 | 7293 | 0.477 | 5.520 |
| main: source cache loading | n/a | 2,393.138 | 7293 | 0.002 | 713.693 |
| main: GED computation | n/a | 718.506 | 7293 | 8.942 | 522.278 |
| sample: complete scoring phase | 32.211 | n/a | n/a | n/a | n/a |
| sample: CFG generation | n/a | 1.886 | 223 | 3.476 | 28.890 |
| sample: native call, including JSON decoding (nested) | n/a | 0.729 | 223 | 1.718 | 9.528 |
| sample: complete Python parse_source call (nested) | n/a | 0.919 | 223 | 2.188 | 11.421 |
| sample: Python graph materialization (nested) | n/a | 0.107 | 223 | 0.229 | 1.495 |
| sample: owner replay/source selection | n/a | 0.916 | 223 | 0.929 | 26.709 |
| sample: source cache loading | n/a | 214.989 | 223 | 88.180 | 6476.024 |
| sample: GED computation | n/a | 5.819 | 223 | 1.197 | 314.875 |

Wall intervals are elapsed time; worker intervals are summed across parallel processes. Native, parse_source and graph materialization intervals are nested inside CFG generation, so they must not be added again. Actual DWARF traversal occurs once during ownership precompute; per-artifact ownership intervals replay cached owners and select source graphs.

The instrumented total starts after initial library copying, imports and code hashing; it excludes those startup steps and final report-file writes. External CLI elapsed time is listed separately only when independently supplied. Candidate scoring phases also include owner replay/source selection, cache loading and GED.

### Separate paired original/native CFG sample

8 previously captured full-file `parse_source` API intervals with CFG and metadata enabled and AST/DDG disabled: original PyJoern summed intervals 110.816s; Rust 0.732s; ratio 151.4257×.

This compares summed end-to-end API intervals for that sample, not the full dataset. Original intervals include JVM startup and Python CFG lifting; native intervals include ABI/serialization/copying, JSON decoding and Python CFG materialization. The Rust measurement uses an earlier build. These intervals do not establish pure Rust CFG generation time or a full-corpus pipeline speedup.

## Evidence

The JSON summaries, compressed per-function differences, canonical baseline/new scoreboards and aggregates, and Astra synchronization audit accompany this report. Canonical results and published site data were not overwritten.

## CFG certification and independent evidence

[The complete saved CFG certificate](evidence/cfg-certificate/certification-status-root.json) covers 8,808 file cases and 6,377 unique prepared inputs, with zero qualified failures and 27,013 unchanged input/reference/helper guards. The corrected total of 4,081,953 function comparisons is **case-weighted**, including repeated/shared file cases; it is not a count of unique functions. The raw comparator reported five discrepancies caused by known original DOT-exporter omissions. The unchanged original Joern CPG/CFG recovery evidence certifies the missing 2,609 case-weighted functions; raw records and corrected status records are both preserved.

[The independent core/canonical audit](evidence/core-audit/summary.json) passed 3,029,437 checks with no failures. It verified 5,401 source tasks, 7,516 candidate tasks, 770 ownership tasks and 72 assembly maps, exact 223-alias/250-target synchronization, and 20,533 unique end-hashed paths across overlapping input scopes. [The root runtime/code proof](evidence/root-final-runtime-code-guards.json) records 119 unchanged guards. [The final aggregate and sample explorer audit](evidence/aggregate-audit/summary.json) passed 1,544,702 checks with zero failures. It independently reconstructed all 1,537,898 per-function difference rows, all baseline/new scoreboards and GED/Union preset counts, all 400 sample explorer entries, and the 250 Astra target states. [The CSV audit](evidence/root-report-csv-audit.json) verified all 9,614 checked fields against the authoritative summary.

[Root validation](evidence/root-validation.json) passed 195 Rust unit tests, eight Rust integration tests and 186 Python tests, plus formatting, Clippy with warnings denied and a release build. Frozen build, integration and release provenance accompany these results.

## Durable artifact index

[summary.json.gz](summary.json.gz) preserves the full summarizer output. Each tree contains baseline/new all-identity and site scoreboards/aggregates, full compressed per-function differences and refreshed sample-extra scores. `scores.all-identities.csv` contains every identity across all five presets and both normalization states. `scores.site.csv` preserves the site aggregate projection with explicit preset visibility flags; `scores.site-visible.csv` contains the displayed rows. `raw-value-changes.csv` reports per-identity changed/added/cleared value counts.

The main published sample-set GED score uses 246 shared measurable targets (133→132 perfect); the standalone result uses 245 measured targets (133→132 perfect). These denominators are intentionally distinct. Main versioned `codex@gpt-6-astra` is an independent column, with its sample-set score unchanged at 31/246.

The existing [original recovery archive](../decbench-parity-latest/recovery-evidence.tar.gz) and [original oracle capture metadata](../decbench-parity-latest/oracle-captures.json.gz) remain in their durable location. Their exact hashes are recorded in `artifact-index.json`; full graph archives, native caches and baseline function-data JSON are not duplicated here. Full corpus manifests, timing evidence and guard records are included as compressed metadata.

[artifact-index.json](artifact-index.json) records source/stored hashes and completed audit status. [SHA256SUMS](SHA256SUMS) covers every assembled artifact except the checksum file itself. The full scoring run used saved artifacts only; compilation and decompilation were not regenerated.
