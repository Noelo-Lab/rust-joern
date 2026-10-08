const DEFAULT_FIELDS = ["entry", "exit", "statements.class", "statements.text", "statements.fields.raw_text", "statements.fields.source_line_number"];
const FIELD_LABELS = {
  id: "Graph node ID", entry: "Entry boundary", exit: "Exit boundary", addr: "Block address", idx: "Block index", block_class: "Block class",
  "statements.count": "Statement count", "statements.class": "Statement classes", "statements.text": "Statement text",
  "statements.fields.raw_text": "Raw statement text", "statements.fields.source_line_number": "Source line numbers",
  "statements.fields.kind": "Native CPG kinds", "statements.fields.id": "Native statement IDs", "statements.fields.type": "Statement types",
  "statements.fields.src": "Assignment source", "statements.fields.dst": "Assignment destination", "statements.fields.ret": "Return value",
  "statements.fields.func": "Called function", "statements.fields.args": "Call arguments", "statements.fields.name": "Statement names",
  "statements.fields.arg1": "First operand", "statements.fields.arg2": "Second operand", "statements.fields.cond": "Conditions",
  "statements.fields.true": "True branch value", "statements.fields.false": "False branch value", "block_fields.is_merged_node": "Merged block flag",
};
const ALLOCATION_FIELDS = new Set(["id", "addr", "idx", "block_fields.id", "statements.fields.id", "statements.fields.kind"]);
const SVG_NS = "http://www.w3.org/2000/svg";
const state = {dataset: null, pair: null, fields: new Set(DEFAULT_FIELDS), allFields: [], selection: null, transforms: {}, noticeTimer: null};

export function canonical(value) {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`;
  if (value !== null && typeof value === "object") return `{${Object.keys(value).sort().map(key => `${JSON.stringify(key)}:${canonical(value[key])}`).join(",")}}`;
  return JSON.stringify(value);
}

function lookup(value, parts) {
  for (const part of parts) {
    if (value === null || typeof value !== "object" || !Object.hasOwn(value, part)) return {present: false};
    value = value[part];
  }
  return {present: true, value};
}

export function getField(node, path) {
  if (!node) return {present: false};
  if (path === "id") return {present: true, value: node.id};
  const attrs = node.attributes || {};
  if (path === "statements.count") return {present: Object.hasOwn(attrs, "statements"), value: (attrs.statements || []).length};
  if (path.startsWith("statements.")) {
    if (!Object.hasOwn(attrs, "statements")) return {present: false};
    return {present: true, statements: attrs.statements.map(stmt => lookup(stmt, path.split(".").slice(1)))};
  }
  return lookup(attrs, path.split("."));
}

function collectLeaves(value, prefix, fields) {
  if (value !== null && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length && !Object.hasOwn(value, "$node")) {
    for (const [key, child] of Object.entries(value)) collectLeaves(child, `${prefix}.${key}`, fields);
  } else fields.add(prefix);
}

export function discoverFields(pairs) {
  const fields = new Set(["id"]);
  for (const pair of pairs) for (const side of ["reference", "candidate"]) for (const node of pair[side].nodes) {
    for (const [key, value] of Object.entries(node.attributes || {})) {
      if (key === "statements") {
        fields.add("statements.count");
        for (const statement of value) for (const [name, child] of Object.entries(statement)) collectLeaves(child, `statements.${name}`, fields);
      } else collectLeaves(value, key, fields);
    }
  }
  const priority = [...DEFAULT_FIELDS, "statements.count", "block_class", "addr", "idx", "id"];
  return [...fields].sort((a, b) => {
    const ai = priority.indexOf(a), bi = priority.indexOf(b);
    if (ai >= 0 || bi >= 0) return (ai < 0 ? Infinity : ai) - (bi < 0 ? Infinity : bi);
    return a.localeCompare(b);
  });
}

export function remapNodeReferences(value, mapping) {
  if (Array.isArray(value)) return value.map(item => remapNodeReferences(item, mapping));
  if (value !== null && typeof value === "object") {
    if (Object.keys(value).length === 1 && Object.hasOwn(value, "$node")) return {$node: mapping.get(String(value.$node)) ?? value.$node};
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, remapNodeReferences(item, mapping)]));
  }
  return value;
}

export function comparePair(pair, fields) {
  const reference = new Map(pair.reference.nodes.map(node => [String(node.id), node]));
  const candidate = new Map(pair.candidate.nodes.map(node => [String(node.id), node]));
  const mapping = new Map((pair.comparison.mapping || []).map(row => [String(row.reference), row.candidate]));
  const inverse = new Map([...mapping].map(([referenceID, candidateID]) => [String(candidateID), referenceID]));
  const differences = new Map();
  for (const [id, node] of reference) {
    const counterpart = candidate.get(String(mapping.get(id)));
    if (!counterpart) continue;
    const different = [...fields].filter(path => canonical(remapNodeReferences(getField(node, path), mapping)) !== canonical(getField(counterpart, path)));
    if (different.length) differences.set(id, different);
  }
  const unmappedReference = [...reference.keys()].filter(id => !mapping.has(id));
  const unmappedCandidate = [...candidate.keys()].filter(id => !inverse.has(id));
  return {mapping, inverse, reference, candidate, differences, unmappedReference, unmappedCandidate,
    selectedAttributesEqual: !!pair.comparison.topology_equal && !differences.size && !unmappedReference.length && !unmappedCandidate.length};
}

export function validateDataset(data) {
  if (!data || data.schema_version !== 1 || !Array.isArray(data.pairs) || !data.pairs.length) throw new Error("Expected a CFG Compare dataset containing at least one paired capture.");
  const pairIDs = new Set();
  for (const pair of data.pairs) {
    if (!pair.id || pairIDs.has(pair.id)) throw new Error("Captured pairs must have unique IDs.");
    pairIDs.add(pair.id);
    for (const side of ["reference", "candidate"]) {
      const graph = pair[side];
      if (!graph || !Array.isArray(graph.nodes) || !Array.isArray(graph.edges)) throw new Error(`Missing ${side} graph for ${pair.name || pair.id}.`);
      const ids = new Set(graph.nodes.map(node => String(node.id)));
      if (ids.size !== graph.nodes.length) throw new Error("A graph contains duplicate node IDs.");
      if (graph.edges.some(edge => !ids.has(String(edge.source)) || !ids.has(String(edge.target)))) throw new Error("A graph edge points to an absent node.");
      if (graph.nodes.some(node => !node.attributes || typeof node.attributes !== "object")) throw new Error("Node attributes are missing from this capture.");
    }
    if (!pair.comparison || !Array.isArray(pair.comparison.mapping)) throw new Error("A pair is missing its verified node correspondence.");
    const refIDs = new Set(pair.reference.nodes.map(node => String(node.id)));
    const candIDs = new Set(pair.candidate.nodes.map(node => String(node.id)));
    const mappedRef = new Set(), mappedCand = new Set();
    for (const row of pair.comparison.mapping) {
      if (!refIDs.has(String(row.reference)) || !candIDs.has(String(row.candidate)) || mappedRef.has(String(row.reference)) || mappedCand.has(String(row.candidate))) throw new Error("Invalid or repeated correspondence in the imported capture.");
      mappedRef.add(String(row.reference)); mappedCand.add(String(row.candidate));
    }
    if (pair.comparison.topology_equal) {
      if (mappedRef.size !== refIDs.size || mappedCand.size !== candIDs.size) throw new Error("A topology match requires a complete correspondence.");
      const map = new Map(pair.comparison.mapping.map(row => [String(row.reference), String(row.candidate)]));
      const refEdges = pair.reference.edges.map(edge => canonical([map.get(String(edge.source)), map.get(String(edge.target))])).sort();
      const candEdges = pair.candidate.edges.map(edge => canonical([String(edge.source), String(edge.target)])).sort();
      if (canonical(refEdges) !== canonical(candEdges)) throw new Error("The supplied node correspondence does not preserve directed edges.");
    }
  }
  return data;
}

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function svgElement(tag, attrs = {}, text) {
  const node = document.createElementNS(SVG_NS, tag);
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value);
  if (text !== undefined) node.textContent = text;
  return node;
}

function fieldLabel(field) {return FIELD_LABELS[field] || field.replace(/^statements\.fields\./, "Statement: ").replace(/^graph_attributes\./, "Graph: ").replace(/^block_fields\./, "Block: ");}

function formatProjection(projection) {
  if (!projection.present) return "⟨not present⟩";
  if (projection.statements) return projection.statements.map((entry, index) => `${index + 1}. ${entry.present ? formatValue(entry.value) : "⟨not present⟩"}`).join("\n");
  return formatValue(projection.value);
}

function formatValue(value) {
  if (typeof value === "string") return value;
  return JSON.stringify(value, null, 2) ?? "undefined";
}

function notice(message) {
  const box = document.getElementById("notice"); box.textContent = message; box.hidden = false;
  clearTimeout(state.noticeTimer); state.noticeTimer = setTimeout(() => box.hidden = true, 6500);
}

function pairStats(pair) {return comparePair(pair, state.fields);}

function filteredPairs() {
  const search = document.getElementById("function-search").value.toLowerCase();
  const kind = document.getElementById("kind-filter").value, status = document.getElementById("status-filter").value;
  return state.dataset.pairs.filter(pair => {
    if (kind && pair.kind !== kind) return false;
    if (search && ![pair.name, pair.case_id, pair.project, pair.language].join(" ").toLowerCase().includes(search)) return false;
    if (!status) return true;
    const stats = pairStats(pair);
    return status === "different" ? !!stats.differences.size : status === "same" ? stats.selectedAttributesEqual : !pair.comparison.topology_equal;
  });
}

function renderFunctionList() {
  const box = document.getElementById("function-list"); box.replaceChildren();
  const pairs = filteredPairs();
  for (const pair of pairs) {
    const stats = pairStats(pair), button = element("button", `function-item${pair.id === state.pair?.id ? " active" : ""}`);
    button.dataset.pairId = pair.id; button.setAttribute("role", "listitem"); button.title = `${pair.name}\n${pair.case_id}`;
    button.setAttribute("aria-current", pair.id === state.pair?.id ? "true" : "false");
    const row = element("div", "function-item-top"); row.append(element("span", "function-item-name", pair.name), element("span", `sample-status${!pair.comparison.topology_equal ? " topology" : stats.differences.size ? " different" : ""}`));
    button.append(row, element("div", "function-item-meta", `${pair.project} · ${pair.optimization} · ${pair.kind} · ${pair.language === "cpp" ? "C++" : "C"}`));
    button.addEventListener("click", () => selectPair(pair)); box.append(button);
  }
  if (!pairs.length) box.append(element("p", "empty-list", "No sampled functions match these filters."));
}

function renderAttributes() {
  const box = document.getElementById("attribute-list"); box.replaceChildren();
  for (const field of state.allFields) {
    const label = element("label", "attribute-option"), input = element("input"); input.type = "checkbox"; input.checked = state.fields.has(field); input.dataset.field = field;
    input.addEventListener("change", () => {input.checked ? state.fields.add(field) : state.fields.delete(field); attributesChanged();});
    const description = element("span", "", fieldLabel(field)); description.append(element("small", "", `${field}${ALLOCATION_FIELDS.has(field) ? " · allocation detail" : ""}`));
    label.append(input, description); box.append(label);
  }
  document.getElementById("attribute-count").textContent = `${state.fields.size} selected`;
}

function attributesChanged() {
  document.getElementById("attribute-count").textContent = `${state.fields.size} selected`;
  renderFunctionList(); renderBadges(); renderGraphs(false); renderInspector(); renderCapture();
}

function renderBadges() {
  if (!state.pair) return;
  const pair = state.pair, stats = pairStats(pair), box = document.getElementById("comparison-badges"); box.replaceChildren();
  const topology = element("span", `badge${pair.comparison.topology_equal ? "" : " error"}`, `${pair.comparison.topology_equal ? "✓ Same" : "≠ Different"} directed topology`);
  topology.title = "The verified correspondence preserves directed edges. Boundary roles are also available as node attributes.";
  box.append(topology);
  if (!state.fields.size) box.append(element("span", "badge neutral", "No attributes selected"));
  else if (!pair.comparison.topology_equal) box.append(element("span", "badge neutral", "No complete node correspondence"));
  else box.append(element("span", `badge${stats.selectedAttributesEqual ? "" : " different"}`, stats.selectedAttributesEqual ? "✓ Selected node attributes match" : `${stats.differences.size} / ${pair.reference.nodes.length} nodes differ`));
  const graphAttrs = pair.comparison.graph_attribute_differences || [];
  if (!pair.comparison.edge_attributes_equal || graphAttrs.length) {
    const extra = element("span", "badge neutral", "Graph / edge metadata differs"); extra.title = "Captured graph and edge attributes can be inspected in Capture provenance or the exported JSON. This status is independent of node attribute toggles."; box.append(extra);
  }
}

function graphDimensions(graph) {
  return {width: graph.layout?.width || 700, height: graph.layout?.height || Math.max(350, graph.nodes.length * 170), nodeWidth: graph.layout?.node_width || 250, nodeHeight: graph.layout?.node_height || 112};
}

function nodePosition(node, graph, index) {return node.position || {x: graphDimensions(graph).width / 2, y: 90 + index * 170};}

function labelLines(node) {
  const rows = [];
  const priorities = ["statements.class", "statements.text", "statements.fields.raw_text", "statements.fields.source_line_number"];
  const fields = [...state.fields].filter(field => !["entry", "exit", "id"].includes(field)).sort((a, b) => {
    const ai = priorities.indexOf(a), bi = priorities.indexOf(b);
    return (ai < 0 ? 100 : ai) - (bi < 0 ? 100 : bi);
  });
  for (const field of fields) {
    const projection = getField(node, field);
    const label = fieldLabel(field);
    if (field === "statements.class" && projection.statements) rows.push(`${projection.statements.map(row => row.present ? row.value : "∅").join(" · ")}`);
    else if (field === "statements.fields.source_line_number" && projection.statements) rows.push(`lines ${projection.statements.map(row => row.present ? row.value : "∅").join(", ")}`);
    else {
      const text = formatProjection(projection);
      if (field.startsWith("statements.") && projection.statements) {
        for (const entry of projection.statements) rows.push(`${field === "statements.text" ? "" : `${label}: `}${entry.present ? formatValue(entry.value).replace(/\s*\n\s*/g, " ⏎ ") : "⟨not present⟩"}`);
      } else rows.push(`${label}: ${text.replace(/\s*\n\s*/g, " ⏎ ")}`);
    }
  }
  return rows;
}

function edgePath(source, target, width, height) {
  if (source.x === target.x && source.y === target.y) return `M ${source.x + width / 2 - 10} ${source.y - 10} C ${source.x + width / 2 + 60} ${source.y - 110},${source.x + width / 2 + 60} ${source.y + 100},${source.x + width / 2 - 2} ${source.y + 20}`;
  const down = target.y > source.y;
  const start = {x: source.x, y: source.y + (down ? height / 2 : -height / 2)};
  const end = {x: target.x, y: target.y + (down ? -height / 2 - 5 : height / 2 + 5)};
  const delta = Math.max(32, Math.abs(end.y - start.y) / 2);
  if (down) return `M ${start.x} ${start.y} C ${start.x} ${start.y + delta}, ${end.x} ${end.y - delta}, ${end.x} ${end.y}`;
  const bend = Math.max(source.x, target.x) + width / 2 + 30;
  return `M ${source.x + width / 2} ${source.y} C ${bend + 30} ${source.y}, ${bend + 30} ${target.y}, ${target.x + width / 2 + 5} ${target.y}`;
}

function renderGraph(side, stats) {
  const graph = state.pair[side], svg = document.getElementById(`${side}-svg`); svg.replaceChildren();
  const dimensions = graphDimensions(graph); svg.dataset.layoutWidth = dimensions.width; svg.dataset.layoutHeight = dimensions.height;
  const defs = svgElement("defs"), marker = svgElement("marker", {id: `${side}-arrow`, viewBox: "0 0 10 10", refX: "9", refY: "5", markerWidth: "6", markerHeight: "6", orient: "auto-start-reverse"});
  marker.append(svgElement("path", {d: "M 0 0 L 10 5 L 0 10 z", fill: "#9eafc1"})); defs.append(marker); svg.append(defs);
  const content = svgElement("g", {class: "graph-content"}), positions = new Map(graph.nodes.map((node, index) => [String(node.id), nodePosition(node, graph, index)]));
  const selectedID = state.selection?.[side];
  for (const edge of graph.edges) {
    const selected = String(edge.source) === String(selectedID) || String(edge.target) === String(selectedID);
    const path = svgElement("path", {d: edgePath(positions.get(String(edge.source)), positions.get(String(edge.target)), dimensions.nodeWidth, dimensions.nodeHeight), class: `graph-edge${selected && selectedID !== undefined ? " selected-edge" : ""}`, "marker-end": `url(#${side}-arrow)`});
    path.append(svgElement("title", {}, JSON.stringify(edge.attributes || {}))); content.append(path);
  }
  graph.nodes.forEach((node, index) => {
    const id = String(node.id), referenceID = side === "reference" ? id : stats.inverse.get(id);
    const counterpartExists = side === "reference" ? stats.mapping.has(id) : stats.inverse.has(id);
    const different = stats.differences.has(String(referenceID)), selected = selectedID !== undefined && id === String(selectedID);
    const position = positions.get(id), width = dimensions.nodeWidth, height = dimensions.nodeHeight;
    const dimmed = document.getElementById("dim-matches").checked && !different && !selected && counterpartExists;
    const group = svgElement("g", {class: `graph-node${different ? " different" : ""}${!counterpartExists ? " unmapped" : ""}${selected ? " selected" : ""}${dimmed ? " dimmed" : ""}`, transform: `translate(${position.x - width / 2},${position.y - height / 2})`, "data-node-id": id, role: "button", tabindex: "0", "aria-label": `${side} node ${id}${different ? ", selected attributes differ" : ""}`});
    group.append(svgElement("rect", {class: "node-rect", x: "0", y: "0", width, height, rx: "7"}), svgElement("rect", {class: "node-accent", x: "0", y: "17", width: "3", height: "16", rx: "1"}));
    group.append(svgElement("text", {class: "node-id", x: "13", y: "20"}, `Block ${node.id}`));
    const roles = [state.fields.has("entry") && node.attributes.entry ? "ENTRY" : "", state.fields.has("exit") && node.attributes.exit ? "EXIT" : ""].filter(Boolean).join(" · ");
    group.append(svgElement("text", {class: "node-role", x: width - 12, y: "20", "text-anchor": "end"}, roles));
    const rows = labelLines(node), max = Math.max(1, Math.floor((height - 35) / 14));
    rows.slice(0, max).forEach((row, i) => group.append(svgElement("text", {class: "node-line", x: "13", y: 40 + i * 14}, row.length > 36 ? row.slice(0, 35) + "…" : row)));
    if (rows.length > max) group.append(svgElement("text", {class: "node-more", x: width - 10, y: height - 6, "text-anchor": "end"}, `+${rows.length - max} more · inspect`));
    group.append(svgElement("title", {}, `${fieldLabel("statements.text")}\n${formatProjection(getField(node, "statements.text"))}\n${different ? "Selected attributes differ. " : ""}Click to inspect every captured attribute.`));
    group.addEventListener("click", event => {if (!svg._dragMoved) {event.stopPropagation(); selectNode(side, node.id);}});
    group.addEventListener("keydown", event => {if (event.key === "Enter" || event.key === " ") {event.preventDefault(); selectNode(side, node.id);}});
    content.append(group);
  });
  svg.append(content); applyTransform(side);
}

function renderGraphs(fit = false) {
  if (!state.pair) return;
  const stats = pairStats(state.pair);
  renderGraph("reference", stats); renderGraph("candidate", stats);
  if (fit) fitGraphs();
}

function applyTransform(side) {
  const svg = document.getElementById(`${side}-svg`), transform = state.transforms[side] || {scale: 1, x: 0, y: 0};
  svg.querySelector(".graph-content")?.setAttribute("transform", `translate(${transform.x},${transform.y}) scale(${transform.scale})`);
}

function setTransform(side, transform, sync = true) {
  state.transforms[side] = transform; applyTransform(side);
  if (sync && document.getElementById("sync-views").checked) {
    const other = side === "reference" ? "candidate" : "reference";
    state.transforms[other] = {...transform}; applyTransform(other);
  }
}

function fitGraphs() {
  if (!state.pair) return;
  const sync = document.getElementById("sync-views").checked;
  let commonScale = Infinity;
  for (const side of ["reference", "candidate"]) {
    const svg = document.getElementById(`${side}-svg`), dimensions = graphDimensions(state.pair[side]);
    const scale = Math.min((svg.clientWidth - 24) / dimensions.width, (svg.clientHeight - 28) / dimensions.height, 1.6);
    commonScale = Math.min(commonScale, scale);
  }
  for (const side of ["reference", "candidate"]) {
    const svg = document.getElementById(`${side}-svg`), dimensions = graphDimensions(state.pair[side]);
    const scale = Math.max(.03, sync ? commonScale : Math.min((svg.clientWidth - 24) / dimensions.width, (svg.clientHeight - 28) / dimensions.height, 1.6));
    setTransform(side, {scale, x: (svg.clientWidth - dimensions.width * scale) / 2, y: (svg.clientHeight - dimensions.height * scale) / 2}, false);
  }
}

function zoom(side, factor, point) {
  const svg = document.getElementById(`${side}-svg`), old = state.transforms[side] || {scale: 1, x: 0, y: 0};
  point ||= {x: svg.clientWidth / 2, y: svg.clientHeight / 2};
  const scale = Math.min(6, Math.max(.025, old.scale * factor)), ratio = scale / old.scale;
  setTransform(side, {scale, x: point.x - (point.x - old.x) * ratio, y: point.y - (point.y - old.y) * ratio});
}

function bindCanvas(side) {
  const svg = document.getElementById(`${side}-svg`);
  let drag = null;
  svg.addEventListener("wheel", event => {event.preventDefault(); const rect = svg.getBoundingClientRect(); zoom(side, Math.exp(-event.deltaY * .0018), {x: event.clientX - rect.left, y: event.clientY - rect.top});}, {passive: false});
  svg.addEventListener("pointerdown", event => {
    if (event.button !== 0) return;
    drag = {x: event.clientX, y: event.clientY, transform: {...state.transforms[side]}}; svg._dragMoved = false;
  });
  svg.addEventListener("pointermove", event => {
    if (!drag) return;
    const dx = event.clientX - drag.x, dy = event.clientY - drag.y;
    if (Math.abs(dx) + Math.abs(dy) > 4) {svg._dragMoved = true; if (!svg.hasPointerCapture(event.pointerId)) svg.setPointerCapture(event.pointerId);}
    if (svg._dragMoved) setTransform(side, {...drag.transform, x: drag.transform.x + dx, y: drag.transform.y + dy});
  });
  svg.addEventListener("pointerup", event => {drag = null; if (svg.hasPointerCapture(event.pointerId)) svg.releasePointerCapture(event.pointerId);});
  svg.addEventListener("pointercancel", () => drag = null);
}

function selectNode(side, id) {
  const stats = pairStats(state.pair);
  state.selection = side === "reference" ? {reference: id, candidate: stats.mapping.get(String(id))} : {reference: stats.inverse.get(String(id)), candidate: id};
  renderGraphs(false); renderInspector(); renderSource();
  setTab("node");
}

function renderInspector() {
  const box = document.getElementById("node-details"), title = document.getElementById("inspector-selection");
  if (!state.selection || !state.pair) {
    box.replaceChildren(); const empty = element("div", "empty-inspector"); empty.append(element("span", "", "◎"), element("p", "", "Click a node to inspect every captured attribute beside its counterpart."), element("small", "", "Missing attributes stay missing. Statement text and classes are never normalized to make a match.")); box.append(empty); title.textContent = "Select any node in either graph"; return;
  }
  const stats = pairStats(state.pair), left = stats.reference.get(String(state.selection.reference)), right = stats.candidate.get(String(state.selection.candidate));
  const fields = discoverFields([{reference: {nodes: left ? [left] : []}, candidate: {nodes: right ? [right] : []}}]);
  const table = element("table", "attribute-table"), head = element("thead"), headRow = element("tr");
  headRow.append(element("th", "", "Captured attribute"), element("th", "", left ? `PyJoern · node ${left.id}` : "PyJoern · no counterpart"), element("th", "", right ? `Rust · node ${right.id}` : "Rust · no counterpart")); head.append(headRow); table.append(head);
  const body = element("tbody");
  for (const field of fields) {
    const l = getField(left, field), r = getField(right, field);
    const changed = canonical(remapNodeReferences(l, stats.mapping)) !== canonical(r), selected = state.fields.has(field);
    const row = element("tr", `${changed ? "changed" : ""}${!selected ? " unselected-row" : ""}`);
    row.dataset.attribute = field;
    const name = element("td", "", field);
    const lcell = element("td", `value${!l.present ? " missing-value" : ""}`, formatProjection(l));
    const rcell = element("td", `value${!r.present ? " missing-value" : ""}`, formatProjection(r));
    row.append(name, lcell, rcell); body.append(row);
  }
  table.append(body); box.replaceChildren(table);
  title.textContent = `${left ? `PyJoern ${left.id}` : "unmapped"} ↔ ${right ? `Rust ${right.id}` : "unmapped"} · all captured fields`;
}

function renderSource() {
  const box = document.getElementById("source-details"); box.replaceChildren(); if (!state.pair) return;
  const source = state.pair.source || {}, text = source.text;
  if (typeof text !== "string") {box.append(element("pre", "capture-pre", "This capture has no embedded source snippet.\n" + (source.prepared_path || source.path || ""))); return;}
  const selectedLines = new Set();
  if (state.selection) for (const side of ["reference", "candidate"]) {
    const node = state.pair[side].nodes.find(node => String(node.id) === String(state.selection[side]));
    for (const statement of node?.attributes.statements || []) if (typeof statement.fields?.source_line_number === "number") selectedLines.add(statement.fields.source_line_number);
  }
  const pre = element("pre", "source-pre"), first = source.text_start_line || source.first_line || 1;
  text.split("\n").forEach((line, index) => {
    const number = first + index, row = element("span", `source-line${selectedLines.has(number) ? " highlight-line" : ""}`);
    row.append(element("span", "source-line-number", String(number)), document.createTextNode(line)); pre.append(row);
  }); box.append(pre);
}

function renderCapture() {
  if (!state.pair) return;
  const pair = state.pair, source = {...pair.source}; delete source.text;
  const data = {seed: state.dataset.seed, generated_at: state.dataset.generated_at, function: pair.name, case: pair.case_id, source,
    reference: pair.reference.metadata, candidate: pair.candidate.metadata,
    comparison: {...pair.comparison, attribute_differences: undefined, raw_attribute_differences: undefined},
    edge_attributes: {reference: pair.reference.edges, candidate: pair.candidate.edges},
    selected_attribute_paths: [...state.fields], sample_provenance: state.dataset.provenance};
  document.getElementById("capture-details").replaceChildren(element("pre", "capture-pre", JSON.stringify(data, null, 2)));
}

function setTab(name) {
  for (const button of document.querySelectorAll("[data-tab]")) {const active = button.dataset.tab === name; button.classList.toggle("active", active); button.setAttribute("aria-selected", String(active));}
  for (const tab of ["node", "source", "capture"]) document.getElementById(`${tab}-details`).hidden = tab !== name;
  if (name === "capture") renderCapture();
}

function selectPair(pair) {
  state.pair = pair; state.selection = null;
  document.getElementById("function-name").textContent = pair.name;
  document.getElementById("case-description").textContent = `${pair.project} / ${pair.optimization} / ${pair.kind} · ${pair.language === "cpp" ? "C++" : "C"}`;
  document.getElementById("graph-summary").textContent = pair.case_id;
  for (const side of ["reference", "candidate"]) document.getElementById(`${side}-counts`).textContent = `${pair[side].nodes.length} nodes · ${pair[side].edges.length} edges`;
  const capture = state.dataset.provenance?.captures?.find(capture => capture.case_id === pair.case_id);
  const originalVersion = capture?.versions?.pyjoern || "captured oracle";
  document.getElementById("reference-version").textContent = originalVersion;
  document.getElementById("mapping-note").textContent = pair.comparison.mapping_optimal === false ? "Mapping search was bounded; another correspondence may differ." : "Verified directed correspondence; allocation IDs kept intact.";
  document.getElementById("export-button").disabled = false;
  renderFunctionList(); renderBadges(); renderGraphs(true); renderInspector(); renderSource(); renderCapture();
  const url = new URL(window.location); url.hash = encodeURIComponent(pair.id); history.replaceState(null, "", url);
}

function loadDataset(data) {
  state.dataset = validateDataset(data); state.allFields = discoverFields(data.pairs);
  const selected = Array.isArray(data.viewer_selected_attributes) ? data.viewer_selected_attributes : DEFAULT_FIELDS;
  state.fields = new Set(selected.filter(field => state.allFields.includes(field)));
  const files = new Set(data.pairs.map(pair => pair.case_id));
  document.getElementById("sample-description").textContent = `${data.pairs.length} functions · ${files.size} inputs · seed ${data.seed ?? "imported"}`;
  document.getElementById("function-search").value = ""; document.getElementById("kind-filter").value = ""; document.getElementById("status-filter").value = "";
  renderAttributes();
  let wanted; try {wanted = decodeURIComponent(window.location.hash.slice(1));} catch {wanted = "";}
  selectPair(data.pairs.find(pair => pair.id === wanted) || data.pairs[0]);
}

function randomPair() {
  const pairs = filteredPairs().filter(pair => pair.id !== state.pair?.id);
  if (!pairs.length) {notice("No other captured function matches the current filters."); return;}
  const random = new Uint32Array(1); crypto.getRandomValues(random);
  selectPair(pairs[random[0] % pairs.length]);
}

function exportPair() {
  if (!state.pair) return;
  const data = {schema_version: 1, seed: state.dataset.seed, generated_at: state.dataset.generated_at, provenance: state.dataset.provenance, pairs: [state.pair], viewer_selected_attributes: [...state.fields]};
  const url = URL.createObjectURL(new Blob([JSON.stringify(data, null, 2)], {type: "application/json"})), link = element("a");
  link.href = url; link.download = `cfg-${state.pair.name.replace(/[^a-zA-Z0-9_-]/g, "_")}.json`; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1000);
}

async function initialize() {
  document.getElementById("function-search").addEventListener("input", () => state.dataset && renderFunctionList());
  for (const id of ["kind-filter", "status-filter"]) document.getElementById(id).addEventListener("change", () => state.dataset && renderFunctionList());
  document.getElementById("random-button").addEventListener("click", () => state.dataset && randomPair());
  document.getElementById("fit-button").addEventListener("click", fitGraphs);
  document.getElementById("dim-matches").addEventListener("change", () => renderGraphs(false));
  document.getElementById("sync-views").addEventListener("change", fitGraphs);
  for (const [id, mode] of [["semantic-fields", "semantic"], ["all-fields", "all"], ["no-fields", "none"]]) document.getElementById(id).addEventListener("click", () => {
    state.fields = new Set(mode === "none" ? [] : state.allFields.filter(field => mode === "all" || !ALLOCATION_FIELDS.has(field)));
    renderAttributes(); attributesChanged();
  });
  for (const button of document.querySelectorAll("[data-tab]")) button.addEventListener("click", () => setTab(button.dataset.tab));
  for (const button of document.querySelectorAll("[data-zoom]")) button.addEventListener("click", () => {const [side, factor] = button.dataset.zoom.split(":"); zoom(side, Number(factor));});
  document.getElementById("export-button").addEventListener("click", exportPair);
  document.getElementById("import-button").addEventListener("click", () => document.getElementById("import-file").click());
  document.getElementById("import-file").addEventListener("change", async event => {
    const file = event.target.files[0]; if (!file) return;
    try {const data = JSON.parse(await file.text()); loadDataset(data); notice(`Opened ${file.name}`);} catch (error) {notice(`Could not open capture: ${error.message}`);} finally {event.target.value = "";}
  });
  document.addEventListener("keydown", event => {
    if (event.target.matches("input,textarea,select") || event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.key.toLowerCase() === "f") {event.preventDefault(); fitGraphs();}
    if (event.key === "Escape") {state.selection = null; renderGraphs(false); renderInspector();}
  });
  for (const side of ["reference", "candidate"]) bindCanvas(side);
  let resizeTimer; window.addEventListener("resize", () => {clearTimeout(resizeTimer); resizeTimer = setTimeout(fitGraphs, 100);});
  try {
    const response = await fetch("/api/data", {cache: "no-store"});
    if (!response.ok) throw new Error(`Capture request returned HTTP ${response.status}`);
    loadDataset(await response.json());
  } catch (error) {
    document.getElementById("sample-description").textContent = "Open a paired capture JSON, or start the local server.";
    notice(`Could not load captures: ${error.message}. Run python3 scripts/cfg_compare.py serve.`);
  } finally {document.getElementById("loading").hidden = true;}
}

if (typeof document !== "undefined") initialize();
