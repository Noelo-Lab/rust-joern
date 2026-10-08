These two small inputs come from DecBench's existing GNU gzip 1.12 O0 results.
They were prepared with the unchanged `decbench.utils.cfg.strip_system_headers`
and parsed by the original PyJoern 4.0.150.4 / Joern v4.0.150. The sidecars record
the original corpus paths, raw and prepared hashes, preparation, and upstream
copyright notices. The gzip and gnulib fixture inputs retain their upstream
GPL/LGPL terms; see COPYING and COPYING.LESSER. They are parser input fixtures.

The baselines contain 65 functions: six real bodies and 59 declaration-only
graphs. `bits.c` exercises K&R definitions and expanded control-flow macros;
`libgzip_a-stripslash.c` exercises attribute-heavy prototypes and pointer code.
The original PyJoern marks function references as function-start no-ops, which
gives `bi_init` two entry roles. These references preserve that behavior.

```sh
python scripts/compare_pyjoern.py tests/fixtures/decbench/bits.c tests/fixtures/decbench/libgzip_a-stripslash.c --reference-dir tests/fixtures/decbench --output workspace/decbench-comparison.json
```

The same references also accept the original `.i` files when `--reference-python`
and `--decbench` point to the original environment; both hashes must match.
