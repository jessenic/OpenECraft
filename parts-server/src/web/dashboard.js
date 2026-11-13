"use strict";
/// Dashboard: figures and graphs of the parts (`GET /api/v1/dashboard`), shown unless
/// administrators switched it off. Bars of manufacturers, groups, catalogs and
/// lifecycles open the parts they count. Loaded before portal.js, whose helpers it uses.

/// Value of the rest of a shortened distribution (`openecraft_parts::dashboard::OTHERS`).
const OTHERS = "\u0000others";
/// Colours of the segments: the top groups in a fixed order, lifecycles by state.
const TOP_GROUP_COLORS = { Electrical: "var(--viz-1)", Fluid: "var(--viz-2)", Mechanics: "var(--viz-3)", Process: "var(--viz-4)" };
const LIFECYCLE_COLORS = { Active: "var(--ok)", PhaseOut: "var(--warn)", Discontinued: "var(--bad)" };

function number(n) { return Number(n || 0).toLocaleString(T.lang); }
function percent(n, of) { return of ? (100 * n / of).toLocaleString(T.lang, { maximumFractionDigits: n && n < of / 100 ? 1 : 0 }) + " %" : "–"; }

/// The name of a value of a distribution as the page shows it.
function vizName(kind, value) {
  if (value === OTHERS) return t("Others");
  switch (kind) {
    case "catalog": return value ? catalogNames[value.toLowerCase()] || value : t("Without catalog");
    case "group": return t(value);
    case "top_group": case "lifecycle": case "role": return key(kind, value);
    default: return value;
  }
}

/// Horizontal bars, longest first; `filter` makes the names open the parts with the
/// value. With `share` the bars are parts of `total` (100 %: all parts).
function hbars(list, kind, filter, total, share) {
  if (!list.length || !list.some(v => v.count)) return '<p class="muted">' + esc(t("No parts found")) + "</p>";
  const max = share ? Math.max(total, 1) : Math.max(...list.map(v => v.count), 1);
  return '<div class="hbars">' + list.map((v, i) => {
    const name = vizName(kind, v.value);
    const tip = name + ": " + number(v.count) + (total ? " (" + percent(v.count, total) + ")" : "");
    const label = filter && v.value !== OTHERS ? '<a class="name" href="#" data-filter="' + esc(filter) + '" data-i="' + i + '" title="' + esc(t("Show parts")) + '">' + esc(name) + "</a>" : '<span class="name">' + esc(name) + "</span>";
    return '<div class="row" data-tip="' + esc(tip) + '">' + label + '<div class="track"><div class="bar" style="width:' + (100 * v.count / max).toFixed(2) + '%"></div></div><span class="num">' + (share ? percent(v.count, total) : number(v.count)) + "</span></div>";
  }).join("") + "</div>";
}

/// One bar split into the shares of the values, with a legend that names and counts them.
function segments(list, kind, colors, filter) {
  const total = list.reduce((s, v) => s + v.count, 0);
  if (!total) return '<p class="muted">' + esc(t("No parts found")) + "</p>";
  const shown = list.filter(v => v.count);
  const tip = v => vizName(kind, v.value) + ": " + number(v.count) + " (" + percent(v.count, total) + ")";
  return '<div class="segments">' + shown.map(v => '<div data-tip="' + esc(tip(v)) + '" style="flex:' + v.count + ";background:" + (colors[v.value] || "var(--muted)") + '"></div>').join("") + "</div>" +
    '<div class="legend">' + list.map(v => {
      const i = list.indexOf(v);
      const name = filter && v.count ? '<a href="#" data-filter="' + esc(filter) + '" data-i="' + i + '">' + esc(vizName(kind, v.value)) + "</a>" : esc(vizName(kind, v.value));
      return '<span data-tip="' + esc(tip(v)) + '"><i style="background:' + (colors[v.value] || "var(--muted)") + '"></i>' + name + ' <span class="muted">' + number(v.count) + " · " + percent(v.count, total) + "</span></span>";
    }).join("") + "</div>";
}

/// Columns of the parts by the month of their last change; the highest and the last
/// month are labelled, every month shows its count on hover.
function columns(list) {
  const max = Math.max(...list.map(v => v.count), 1);
  const top = list.reduce((b, v, i) => v.count > list[b].count ? i : b, 0);
  const month = m => new Date(m + "-15T00:00:00Z").toLocaleDateString(T.lang, { month: "short", timeZone: "UTC" });
  const long = m => new Date(m + "-15T00:00:00Z").toLocaleDateString(T.lang, { month: "long", year: "numeric", timeZone: "UTC" });
  return '<div class="columns">' + list.map((v, i) => '<div class="col" data-tip="' + esc(long(v.value) + ": " + number(v.count)) + '">' +
    (v.count && (i === top || i === list.length - 1) ? '<span class="top">' + number(v.count) + "</span>" : "") +
    '<div class="bar" style="height:' + (v.count ? Math.max(1.5, 100 * v.count / max).toFixed(2) : 0) + '%"></div></div>').join("") + "</div>" +
    '<div class="months">' + list.map((v, i) => "<span>" + (i % 2 === list.length % 2 ? "" : esc(month(v.value))) + "</span>").join("") + "</div>";
}

function chart(title, body, wide) { return '<div class="chart' + (wide ? " wide" : "") + '"><h3>' + esc(title) + "</h3>" + body + "</div>"; }

async function showDashboard() {
  setNav();
  document.getElementById("facets").innerHTML = "";
  content('<div class="status">' + esc(t("Loading…")) + "</div>");
  let d;
  try { d = await api("/dashboard"); }
  catch (e) { content('<div class="status error">' + esc(e.status === 404 ? t("The dashboard is hidden") : e.message) + "</div>"); return; }
  const tile = (value, label) => '<div class="tile"><div class="value">' + value + '</div><div class="label">' + esc(t(label)) + "</div></div>";
  let html = '<div class="tiles">' + tile(number(d.parts), "Parts") + tile(number(d.catalogs), "Catalogs") + tile(number(d.manufacturers), "Manufacturers") +
    tile(number(d.files), "Files") + tile(esc(size(d.file_size || 0)), "Size of the files") + tile(number(d.revision), "Revision") + "</div>";
  const c = d.coverage || {};
  const coverage = [["Description", c.description], ["Price", c.price], ["Dimensions", c.dimensions], ["Front view (2D)", c.model_2d], ["3D model", c.model_3d], ["Files", c.files], ["Datasheet", c.datasheet]]
    .map(([label, n]) => ({ value: t(label), count: n || 0 }));
  html += '<div class="charts">';
  html += chart(t("Parts per manufacturer"), hbars(d.by_manufacturer, "manufacturer", "manufacturer", d.parts));
  html += chart(t("Parts per product group"), hbars(d.by_group, "group", "group", d.parts));
  html += chart(t("Parts per catalog"), hbars(d.by_catalog, "catalog", "catalog", d.parts));
  html += chart(t("Product top group") + " / " + t("Lifecycle"), segments(d.by_top_group, "top_group", TOP_GROUP_COLORS, "top_group") + '<div style="height:16px"></div>' +
    segments(d.by_lifecycle, "lifecycle", LIFECYCLE_COLORS, "lifecycle"));
  html += chart(t("Last changes per month"), columns(d.changes));
  html += chart(t("Parts with …"), hbars(coverage, "", null, d.parts, true));
  if ((d.users || []).some(v => v.count)) html += chart(t("Users per role"), hbars(d.users, "role", null));
  if ((d.recent || []).length) {
    html += chart(t("Changed last"), '<div class="table-wrap"><table><thead><tr><th>' + esc(t("Part number")) + "</th><th>" + esc(t("Description")) + '</th><th class="hide-narrow">' + esc(t("Manufacturer")) +
      "</th><th>" + esc(t("Last change")) + "</th></tr></thead><tbody>" + d.recent.map(p => '<tr data-n="' + esc(p.number) + '"><td><strong>' + esc(p.number) + "</strong></td><td>" + esc(p.description) +
      '</td><td class="hide-narrow">' + esc(p.manufacturer) + "</td><td>" + esc(dateTime(p.modified)) + "</td></tr>").join("") + "</tbody></table></div>", true);
  }
  content(html + "</div>");
  const lists = { manufacturer: d.by_manufacturer, group: d.by_group, catalog: d.by_catalog, top_group: d.by_top_group, lifecycle: d.by_lifecycle };
  document.querySelectorAll("#content [data-filter]").forEach(a => a.onclick = e => {
    e.preventDefault();
    const f = a.dataset.filter;
    query = blank();
    query[f] = lists[f][+a.dataset.i].value;
    view = "parts";
    go();
  });
  document.querySelectorAll("#content tr[data-n]").forEach(tr => tr.onclick = () => { location.hash = "part=" + encodeURIComponent(tr.dataset.n); });
}

/// The tooltip of the marks (`data-tip`), following the pointer.
document.addEventListener("mousemove", e => {
  const tip = document.getElementById("viz-tip");
  if (!tip) return;
  const at = e.target.closest && e.target.closest("[data-tip]");
  if (!at || view !== "dashboard") { tip.hidden = true; return; }
  tip.textContent = at.dataset.tip;
  tip.hidden = false;
  const x = Math.min(e.clientX + 14, window.innerWidth - tip.offsetWidth - 8);
  const y = e.clientY + 18 + tip.offsetHeight > window.innerHeight ? e.clientY - tip.offsetHeight - 10 : e.clientY + 18;
  tip.style.left = Math.max(8, x) + "px";
  tip.style.top = y + "px";
});
