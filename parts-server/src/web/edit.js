"use strict";
/// The form of a new or a changed part, offered to users who may change parts (by
/// manufacturer or by catalog); the server checks the rights again. Loaded after
/// admin.js (rights, helpers) and before portal.js, whose helpers it uses.

/// Field, label and kind of the inputs, by box.
const PART_FIELDS = [
  ["General", [
    ["description", "Description", "text"], ["manufacturer", "Manufacturer", "maker"], ["type_number", "Type number", "text"], ["order_number", "Order number", "text"],
    ["supplier", "Supplier", "text"], ["catalog", "Catalog", "catalog"], ["top_group", "Product top group", "top_group"], ["group", "Product group", "group"],
    ["subgroup", "Product subgroup", "subgroup"], ["identifier", "Identifier", "text"], ["lifecycle", "Lifecycle", "lifecycle"], ["successor", "Successor", "text"], ["note", "Note", "area"],
  ]],
  ["Technical data", [
    ["voltage", "Voltage", "text"], ["current", "Current", "text"], ["power", "Power", "text"], ["tech_data", "Technical data", "area"],
    ["certificates", "Certificates", "text"], ["datasheet", "Datasheet", "text"],
  ]],
  ["Commercial data", [
    ["price", "Price", "number"], ["currency", "Currency", "text"], ["price_unit", "Price unit", "int"], ["quantity_unit", "Quantity unit", "text"],
  ]],
  ["Mounting data", [
    ["width", "Width", "number"], ["height", "Height", "number"], ["depth", "Depth", "number"], ["weight", "Weight", "number"], ["mounting", "Mounting", "text"],
  ]],
];
const PART_HINTS = { tech_data: "One line per value, e.g. Coil voltage: 24 V DC", width: "mm", height: "mm", depth: "mm", weight: "kg", datasheet: "https://…" };

/// Values of the facets of all parts (manufacturers, groups), for suggestions.
async function facetValues() {
  try {
    const r = await api("/search", { method: "POST", json: Object.assign(blank(), { limit: 1 }) });
    const of = name => ((r.facets.find(f => f.name === name) || {}).values || []).map(v => v.value);
    return { manufacturer: of("manufacturer"), group: of("group"), subgroup: of("subgroup") };
  } catch (e) { return { manufacturer: [], group: [], subgroup: [] }; }
}

/// Shows the form of a new part (`p` null) or of the part `p`.
async function partForm(p) {
  setNav();
  document.getElementById("facets").innerHTML = "";
  let catalogs = [];
  try { catalogs = await api("/catalogs"); } catch (e) {}
  const values = await facetValues();
  const part = Object.assign({ top_group: "Electrical", lifecycle: "Active", currency: "EUR", price_unit: 1, quantity_unit: "pcs" }, p || {});
  // The catalogs the user may add parts to, and the part's own.
  const open = catalogs.filter(c => mayOnPart("edit_parts", { manufacturer: c.manufacturer, catalog: c.id }) || (p && (p.catalog || "").toLowerCase() === c.id.toLowerCase()));
  const keys = kind => Object.keys(T.keys).filter(k => k.startsWith(kind + ":")).map(k => k.slice(kind.length + 1));
  const input = (f, kind) => {
    const v = part[f] ?? "";
    const id = "pf-" + f;
    switch (kind) {
      case "area": return '<textarea id="' + id + '" rows="' + (f === "tech_data" ? 5 : 2) + '">' + esc(v) + "</textarea>";
      // Text, so that both decimal separators work.
      case "number": case "int": return '<input id="' + id + '" inputmode="' + (kind === "int" ? "numeric" : "decimal") + '" value="' + esc(v === 0 ? "" : v) + '">';
      case "maker": return '<input id="' + id + '" list="pf-makers" value="' + esc(v) + '">';
      case "group": case "subgroup": return '<input id="' + id + '" list="pf-' + kind + 's" value="' + esc(v) + '">';
      case "catalog": return '<select id="' + id + '"><option value="">' + esc(t("Without catalog")) + "</option>" +
        open.map(c => '<option value="' + esc(c.id) + '"' + ((v || "").toLowerCase() === c.id.toLowerCase() ? " selected" : "") + ">" + esc(c.name + " (" + c.manufacturer + ")") + "</option>").join("") + "</select>";
      case "top_group": case "lifecycle": return '<select id="' + id + '">' + keys(kind).map(k => '<option value="' + esc(k) + '"' + (k === v ? " selected" : "") + ">" + esc(key(kind, k)) + "</option>").join("") + "</select>";
      default: return '<input id="' + id + '" value="' + esc(v) + '">';
    }
  };
  let html = '<form id="pf" class="detail"><div class="head"><div><a href="#" id="pf-back">← ' + esc(t(p ? "Back to the part" : "Back to the results")) + "</a><h2>" +
    (p ? tf("Edit part {}", p.number) : esc(t("New part"))) + "</h2></div></div>";
  html += '<div class="form" style="margin-bottom:14px">' + labelFor("Part number", '<input id="pf-number">') + '<div><input id="pf-number" required autocomplete="off"' +
    (p ? ' value="' + esc(p.number) + '" disabled' : "") + "></div></div>";
  html += '<div class="grid">';
  for (const [title, fields] of PART_FIELDS) {
    html += '<div class="box"><h3>' + esc(t(title)) + '</h3><div class="form">';
    for (const [f, label, kind] of fields) {
      const hint = PART_HINTS[f];
      html += labelFor(label, input(f, kind)) + "<div>" + input(f, kind) + (hint ? '<div class="hint">' + esc(f === "tech_data" ? t(hint) : hint) + "</div>" : "") + "</div>";
    }
    html += "</div></div>";
  }
  html += '</div><div class="buttons sticky"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="pf-cancel">' + esc(t("Cancel")) + '</button><span id="pf-msg"></span></div></form>';
  html += datalist("pf-makers", values.manufacturer.concat(Object.values(catalogManufacturers))) + datalist("pf-groups", values.group) + datalist("pf-subgroups", values.subgroup);
  content(html);
  const el = id => document.getElementById(id);
  // Showing the part again also shows it when the address did not change.
  el("pf-back").onclick = e => { e.preventDefault(); p ? showPart(p.number) : history_back(); };
  el("pf-cancel").onclick = () => { p ? showPart(p.number) : history_back(); };
  (p ? el("pf-description") : el("pf-number")).focus();
  el("pf").onsubmit = async e => {
    e.preventDefault();
    const number = p ? p.number : el("pf-number").value;
    if (!number.trim() || number.trim() !== number) { message("pf-msg", t("A part number must not be empty or start or end with blanks")); return; }
    const next = Object.assign({}, part, { number });
    for (const [, fields] of PART_FIELDS) {
      for (const [f, , kind] of fields) {
        const v = el("pf-" + f).value;
        next[f] = kind === "number" ? (parseFloat(String(v).replace(",", ".")) || 0) : kind === "int" ? Math.max(1, parseInt(v, 10) || 1) : kind === "area" ? v.trim() : v.trim();
      }
    }
    if (!mayOnPart("edit_parts", next)) { message("pf-msg", tp("You may not change parts of manufacturer {}", next.manufacturer || "–")); return; }
    const button = e.target.querySelector("button.primary");
    button.disabled = true;
    try {
      await api("/parts/" + encodeURIComponent(number), { method: "PUT", json: next, headers: p ? {} : { "If-None-Match": "*" } });
      footer();
      if (p) showPart(number); else location.hash = "part=" + encodeURIComponent(number);
    } catch (err) {
      button.disabled = false;
      message("pf-msg", err.status === 409 && !p ? tp("Part {} exists already", number) : err.message);
    }
  };
}
