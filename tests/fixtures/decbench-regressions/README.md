These small C/C++ reconstructions preserve distinct failures confirmed in the
full DecBench O0/O2 source, IDA, and Kuna audit, and in a 2026-10-09 census of
the other decompilers' saved output. They contain only the syntax needed to
reproduce each issue, rather than copies of the original programs.

Every `.pyjoern.json` was captured independently from the unchanged installed
PyJoern 4.0.150.4 with Joern v4.0.150, in a temporary working directory with
`PYTHONPATH` removed. Capture used `parse_source` with `no_metadata=True`,
`no_ddg=True`, and `no_ast=True`. No native shim or mixed translation unit run
was used to produce the expected graphs.

| Fixture | Confirmed origin | Original native defect |
| --- | --- | --- |
| `knr_const.c` | Bash `wcsnwidth`, source O0/O2 | A const-first K&R definition becomes a degenerate prototype. |
| `global_struct_initializer.c` | Bash `findcmd`, source O0/O2 | A struct object initializer prevents recovery of later function bodies. |
| `gnu_inline_asm.c` | Betaflight `__enable_irq`, source | Its permissive graph matches, but strict parsing rejects inline assembly. |
| `microsoft_asm_block.c` | Betaflight `bbSwitchToOutput`, IDA | A braced assembly statement truncates recovery and loses later functions. |
| `terminal_label.c` | Betaflight `applyStatusProfile`, Kuna | A label before a closing brace changes loop topology: original 4 nodes/4 edges, native 6/7. |
| `casted_indirect_call.c` | Betaflight `EXTI_IRQHandler`, Kuna | A nested cast in an indirect callee causes a missing-expression diagnostic. |
| `global_qualification.c` | Bzip2 `compress`, IDA | The original accepts `::stream` in a `.c` input; the native prefix parser rejects it. |
| `missing_goto_target.c` | Coreutils `process_file` in Kuna `chmod` | The actual input lacks the target label; the original still emits a CFG, while strict native usage rejects it. |
| `nested_designator.c` | Betaflight `lsm6dsv16xAccReadSPI`, source | Chained member designators cause strict diagnostics. |
| `cpp_operator_declarations.cpp` | NuttX `libxx_dynamic_cast`, source O0/O2 | The original prototype names include `operator ==`; native names omit the space. |
| `cpp_operator_definitions.cpp` | NuttX `libxx_typeinfo`, source O0/O2 | The original definition names are operator symbols such as `==`; native names include `operator`. |
| `noreturn_suffix_unclosed_literal.c` | Coreutils `usage` in Binary Ninja `sort`, O0 | A `) __noreturn` suffix body with a newline-broken literal leaves a group unclosed; the native skip lost every later function. |
| `grouped_pointer_call.c` | Betaflight `serialBeginWrite`, r2dec O0 | `uint32_t (*r3)() ();` without a typedef is a call in the original; native parsing made it a declaration and emptied the branch. |
| `grouped_pointer_call_typedef.c` | Companion to `grouped_pointer_call.c` | With a typedef, only a function returning a function becomes a call. |
| `call_missing_semicolon_before_loop.c` | Betaflight `microsISR` and ChibiOS `rt_test_004_001_execute`, angr O0/O2 | Unterminated calls before a loop at a block end collapsed the whole method. |
| `literal_callee.c` | U-Boot `dm_gpio_clrset_flags`, angr O2-noinline | `1619153864();` was a problem statement, dropping its label and branch. |
| `brace_operand_condition.c` | Zlib `gzfread` in Binary Ninja `minigzip64`, O2 | After `!= {0}` closes the block, the stray `)` problem consumed the next statement. |

[provenance.json](provenance.json) records actual case IDs, functions and prepared
line numbers, original/prepared source hashes, full reference hashes, reduced
source and baseline hashes, and the initial native differences. Entries for both
optimizations have independently recorded reference and candidate evidence.

[test_decbench_regressions.py](../../test_decbench_regressions.py) checks function
coverage, directed topology, entry/exit roles, degeneracy, and diagnostics. All
reduced cases now pass in strict mode against the unchanged snapshots.
Their original failure evidence remains in the provenance file. Full corpus
parity is measured separately; these cases do not waive any corpus divergence.

Run against the built native library:

```sh
PYTHONPATH=python python -m unittest discover -s tests -p test_decbench_regressions.py -v
```
