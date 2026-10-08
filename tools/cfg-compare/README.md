# CFG Compare

Run from the repository root:

```sh
python3 scripts/cfg_compare.py serve
```

Open <http://127.0.0.1:8765>. Python 3.10+ serves the viewer and bundled captures;
viewing needs no Joern installation, Rust build, npm packages, or internet access.
The server uses `workspace/cfg-compare/dataset.json` if present, otherwise
`tools/cfg-compare/samples.json.gz`. To explicitly use the bundled sample:

```sh
python3 scripts/cfg_compare.py serve --data tools/cfg-compare/samples.json.gz
```

Choose a function in the sidebar or use the random button to choose another
captured function. Click a node in either graph to inspect it beside its verified
counterpart. Checked attributes control graph labels and comparison highlights;
the inspector always retains all captured fields. Scroll to zoom, drag to pan,
and press **F** to fit. Views can be synchronized. Prepared source shows selected
statement lines; capture provenance includes input/build hashes, parser versions,
flags, directed correspondence, edge attributes, and translation-unit timings.
Export pair saves its complete data and selected attributes; Open JSON restores it.

Topology checks preserve directed edges. Attribute comparisons preserve missing
versus null values, statement order, classes, text, source lines, and array/string
types. Only explicit graph-object references are remapped to the corresponding
node. Raw addresses, IDs, and native CPG kinds remain available. The Semantic
preset omits those allocation/backend details from its selected fields; All
includes them. Graph/edge metadata has an independent status. A bounded mapping
search is explicitly labeled when another correspondence might be better.

The bundled sample contains **24 functions from eight prepared inputs**, covering
C/C++, O0/O2, source, IDA, and Kuna strata available in DecBench. Its recorded
seed is `6196664079766316072`. Each graph was freshly captured from the unchanged
installed PyJoern 4.0.150.4 / Joern 4.0.150 or the native port, using identical
prepared files. Cached topology only helped select readable functions; neither
topology nor attribute agreement was a selection condition. All 24 pairs match
directed topology and entry/exit roles, but all expose public attribute
differences. This sample does not establish full node-attribute parity.

To generate new captures, build the current Rust library, have NetworkX and
Graphviz `dot` available, and supply the prepared DecBench inventory and the
interpreter containing the original PyJoern:

```sh
cargo build --release --lib
python3 scripts/cfg_compare.py sample \
  --manifest workspace/decbench-parity/manifest.json \
  --oracle-python /home/mahaloz/.virtualenvs/decbench/bin/python \
  --count 24
```

Add `--seed 6196664079766316072` to repeat the selection. Without `--seed`, a new
seed is generated and recorded. Count must be a positive multiple of the number
of language/optimization/input-kind strata (eight in this inventory). Defaults
limit selected functions to 3–50 nodes and input files to 250 KB; these are
readability limits, not a representative performance benchmark. The original
interpreter is isolated from the replacement package and checked against frozen
oracle code hashes. Capture JSON records original and native generation times
for each complete translation unit; original timings include JVM startup.

Viewer checks:

```sh
python3 -m unittest discover -s tests -p test_cfg_compare.py -v
node --test tests/test_cfg_compare_ui.mjs
```
