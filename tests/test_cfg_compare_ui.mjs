import assert from "node:assert/strict";
import {readFileSync} from "node:fs";
import {gunzipSync} from "node:zlib";
import test from "node:test";
import {comparePair, discoverFields, getField, remapNodeReferences, validateDataset} from "../tools/cfg-compare/app.js";

const data = JSON.parse(gunzipSync(readFileSync(new URL("../tools/cfg-compare/samples.json.gz", import.meta.url))));

test("all bundled fresh captures have complete directed correspondences", () => {
  assert.equal(validateDataset(data), data);
  assert.equal(data.pairs.length, 24);
  assert.equal(new Set(data.pairs.map(pair => pair.case_id)).size, 8);
  for (const pair of data.pairs) {
    assert.equal(pair.comparison.topology_equal, true, pair.id);
    assert.equal(comparePair(pair, ["entry", "exit"]).selectedAttributesEqual, true, pair.id);
    assert.equal(pair.reference.metadata.serialization_warnings.length, 0, pair.id);
    assert.equal(pair.candidate.metadata.serialization_warnings.length, 0, pair.id);
  }
});

test("attribute toggles expose actual captured differences without losing raw fields", () => {
  const fields = discoverFields(data.pairs);
  assert.ok(fields.includes("statements.class"));
  assert.ok(fields.includes("statements.fields.raw_text"));
  assert.ok(fields.includes("statements.fields.id"));
  assert.ok(fields.includes("block_fields.is_merged_node"));
  const before = JSON.stringify(data);
  for (const pair of data.pairs) {
    assert.equal(comparePair(pair, []).differences.size, 0);
    assert.equal(comparePair(pair, fields).selectedAttributesEqual, false, pair.id);
  }
  assert.equal(JSON.stringify(data), before);
});

test("missing, null, ordered statements, and raw text types stay distinct", () => {
  const node = {id: 0, attributes: {idx: null, statements: [
    {class: "Return", fields: {raw_text: ["return", "x"]}},
    {class: "Nop", fields: {raw_text: ""}},
  ]}};
  assert.notDeepEqual(getField(node, "idx"), getField(node, "addr"));
  assert.deepEqual(getField(node, "statements.class"), {present: true, statements: [
    {present: true, value: "Return"}, {present: true, value: "Nop"},
  ]});
  assert.deepEqual(getField(node, "statements.fields.raw_text").statements[0], {present: true, value: ["return", "x"]});
  assert.deepEqual(getField(node, "statements.fields.ret").statements, [{present: false}, {present: false}]);
});

test("only explicit graph-object references follow the node correspondence", () => {
  const original = {self: {$node: 0}, code: "node 0, addr 0", addr: 0, list: [{$node: 0}]};
  assert.deepEqual(remapNodeReferences(original, new Map([["0", 90]])), {
    self: {$node: 90}, code: "node 0, addr 0", addr: 0, list: [{$node: 90}],
  });
  assert.equal(original.self.$node, 0);
});

test("import rejects an incomplete or reused correspondence", () => {
  const partial = structuredClone(data);
  partial.pairs[0].comparison.mapping.pop();
  assert.throws(() => validateDataset(partial), /complete correspondence/);
  const reused = structuredClone(data);
  reused.pairs[0].comparison.mapping[1].candidate = reused.pairs[0].comparison.mapping[0].candidate;
  assert.throws(() => validateDataset(reused), /repeated correspondence/);
});

test("import rejects changed directed edges and multiplicity despite a match claim", () => {
  const changed = structuredClone(data);
  const edge = changed.pairs[0].candidate.edges[0];
  [edge.source, edge.target] = [edge.target, edge.source];
  assert.throws(() => validateDataset(changed), /preserve directed edges/);
  const duplicate = structuredClone(data);
  duplicate.pairs[0].candidate.edges.push(structuredClone(duplicate.pairs[0].candidate.edges[0]));
  assert.throws(() => validateDataset(duplicate), /preserve directed edges/);
});
