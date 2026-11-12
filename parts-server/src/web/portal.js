"use strict";
const API = "/api/v1";
const LIMIT = 25;
const FIXED = ["top_group", "group", "subgroup", "manufacturer", "catalog", "lifecycle"];
const TITLES = { top_group: "Product top group", group: "Product group", subgroup: "Product subgroup", manufacturer: "Manufacturer", catalog: "Catalog", lifecycle: "Lifecycle" };
const KEYED = { top_group: "top_group", lifecycle: "lifecycle" };
let T = { texts: {}, keys: {}, languages: [], lang: "en", rights: [], roles: [] };
let query = blank();
let view = "parts";
let favorites = [];
let catalogNames = {};
let catalogManufacturers = {};
let expanded = {};
let session = load("session") || loadTemporary("session");
/// The sign-in form: "user" (name and password), "token" (administrator token) or
/// "register" (a new account, when the server lets users register).
let loginMode = "user";
let registrationOpen = false;

function blank() { return { q: null, top_group: null, group: null, subgroup: null, manufacturer: null, catalog: null, lifecycle: null, attributes: [], numbers: [], sort: "Number", descending: false, brief: true, offset: 0, limit: LIMIT }; }
function load(k) { try { return JSON.parse(localStorage.getItem(k)); } catch (e) { return null; } }
function save(k, v) { try { v === null ? localStorage.removeItem(k) : localStorage.setItem(k, JSON.stringify(v)); } catch (e) {} }
function loadTemporary(k) { try { return JSON.parse(sessionStorage.getItem(k)); } catch (e) { return null; } }
/// Keeps the session for the next visit; one with the access token only for this tab.
function keepSession() {
  const temporary = !!session && session.kind === "token";
  try { temporary ? sessionStorage.setItem("session", JSON.stringify(session)) : sessionStorage.removeItem("session"); } catch (e) {}
  save("session", session && !temporary ? session : null);
}
function esc(s) { return String(s ?? "").replace(/[&<>"']/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])); }
function t(s) { return T.texts[s] || s; }
function tf(s, ...args) { let i = 0; return t(s).replace(/\{(\d*)\}/g, (_, n) => esc(args[n === "" ? i++ : +n])); }
function key(kind, k) { return T.keys[kind + ":" + k] || k; }
function shown(facet, value) {
  if (KEYED[facet]) return key(KEYED[facet], value);
  if (facet === "catalog") return catalogNames[value.toLowerCase()] || value;
  if (facet === "group" || facet === "subgroup") return t(value);
  return value;
}
function date(secs) { return secs ? new Date(secs * 1000).toLocaleDateString(T.lang) : ""; }
function dateTime(secs) { return secs ? new Date(secs * 1000).toLocaleString(T.lang, { dateStyle: "short", timeStyle: "short" }) : ""; }
function size(bytes) { return bytes < 1024 ? bytes + " B" : bytes < 1048576 ? Math.round(bytes / 1024) + " kB" : (bytes / 1048576).toLocaleString(T.lang, { maximumFractionDigits: 1 }) + " MB"; }
/// The CSV export of the parts shown (the filters the server can read from the address).
function exportUrl() {
  const p = new URLSearchParams();
  for (const k of ["q"].concat(FIXED)) if (query[k]) p.set(k, query[k]);
  p.set("sort", query.sort);
  if (query.descending) p.set("descending", "true");
  return API + "/export.csv?" + p.toString();
}
/// The view and the search as the address keeps them (#catalogs, #part=…, otherwise
/// the filters: q, groups, attr=Name=Value, sort, desc, offset, view=favorites), so
/// Back, Forward and bookmarks work.
function stateHash() {
  if (view === "catalogs") return "catalogs";
  if (view === "admin") return "admin/" + adminTab;
  const p = new URLSearchParams();
  if (view === "favorites") p.set("view", "favorites");
  for (const k of ["q"].concat(FIXED)) if (query[k]) p.set(k, query[k]);
  query.attributes.forEach(([k, v]) => p.append("attr", k + "=" + v));
  if (query.sort !== "Number") p.set("sort", query.sort);
  if (query.descending) p.set("desc", "1");
  if (query.offset) p.set("offset", String(query.offset));
  return p.toString();
}
function readHash() {
  const h = location.hash.replace(/^#/, "");
  if (h === "catalogs") { view = "catalogs"; return; }
  if (h === "admin" || h.startsWith("admin/")) { view = "admin"; adminTab = h.slice(6) || "overview"; return; }
  const p = new URLSearchParams(h);
  view = p.get("view") === "favorites" ? "favorites" : "parts";
  const q = blank();
  for (const k of ["q"].concat(FIXED)) q[k] = p.get(k) || null;
  q.attributes = p.getAll("attr").map(a => { const i = a.indexOf("="); return i < 0 ? ["", ""] : [a.slice(0, i), a.slice(i + 1)]; }).filter(([k]) => k);
  q.sort = ["Number", "Manufacturer", "Description", "Price", "Modified"].includes(p.get("sort")) ? p.get("sort") : "Number";
  q.descending = p.get("desc") === "1";
  q.offset = Math.max(0, parseInt(p.get("offset") || "0", 10) || 0);
  query = q;
  const box = document.getElementById("q");
  if (box) box.value = query.q || "";
}
/// Shows the current view and search as a new entry of the browser history.
function go() {
  const h = stateHash();
  if (location.hash.replace(/^#/, "") === h) route(); else location.hash = h;
}
function money(p) { return p.price ? p.price.toLocaleString(T.lang, { minimumFractionDigits: 2, maximumFractionDigits: 2 }) + " " + esc(p.currency) : ""; }
/// Only web addresses become links (no javascript: or data: addresses of the data).
function webLink(url) { return /^https?:\/\/[^\s]+$/i.test(String(url ?? "").trim()); }
function partLink(n) { return '<a href="#part=' + encodeURIComponent(n) + '">' + esc(n) + "</a>"; }

async function api(path, options = {}) {
  const headers = Object.assign({}, options.headers || {});
  if (session && session.token) headers.Authorization = "Bearer " + session.token;
  if (options.json !== undefined) { headers["Content-Type"] = "application/json"; options.body = JSON.stringify(options.json); }
  const r = await fetch(API + path, Object.assign({}, options, { headers }));
  // A session the server no longer knows: sign out and read anonymously.
  if (r.status === 401 && session && !options.retried && path !== "/login") {
    session = null; favorites = []; keepSession(); renderAccount();
    return api(path, Object.assign({}, options, { retried: true }));
  }
  if (!r.ok) {
    let msg = r.status + " " + r.statusText;
    try { msg = (await r.json()).error || msg; } catch (e) {}
    const err = new Error(msg); err.status = r.status; throw err;
  }
  return r.status === 204 ? null : options.raw ? r.blob() : r.json();
}

function translatePage() {
  document.documentElement.lang = T.lang;
  document.title = t("OpenECraft data portal");
  document.querySelectorAll("[data-t]").forEach(e => e.textContent = t(e.dataset.t));
  document.querySelectorAll("[data-tp]").forEach(e => e.placeholder = t(e.dataset.tp));
  const sel = document.getElementById("lang");
  sel.setAttribute("aria-label", t("Language"));
  sel.innerHTML = T.languages.map(([c, n]) => '<option value="' + c + '"' + (c === T.lang ? " selected" : "") + ">" + esc(n) + "</option>").join("");
}

async function setLanguage(code) {
  T = await api("/texts?lang=" + encodeURIComponent(code));
  save("lang", T.lang);
  translatePage();
  renderAccount();
}

function renderAccount() {
  const box = document.getElementById("account");
  document.getElementById("nav-admin").hidden = !adminAllowed();
  if (session) {
    box.innerHTML = '<span class="muted">' + tf("Signed in as {}", session.user.name) + " (" + esc(key("role", session.user.role)) + ')</span><button id="logout">' + esc(t("Sign out")) + "</button>";
    document.getElementById("logout").onclick = logout;
  } else {
    if (loginMode === "register" && !registrationOpen) loginMode = "user";
    const input = (id, type, auto, hint) => '<input id="' + id + '"' + (type ? ' type="' + type + '"' : "") + ' autocomplete="' + auto + '" placeholder="' + esc(hint) + '" aria-label="' + esc(hint) + '">';
    const link = (mode, label, title) => '<button type="button" class="link" data-mode="' + mode + '"' + (title ? ' title="' + esc(title) + '"' : "") + ">" + esc(label) + "</button>";
    let fields, submit, links;
    if (loginMode === "token") {
      fields = input("token", "password", "off", t("Access token"));
      submit = t("Sign in");
      links = link("user", t("User account"), t("Sign in with a user account"));
    } else if (loginMode === "register") {
      fields = input("user", "", "username", t("User name")) + input("pw", "password", "new-password", t("Password")) + input("pw2", "password", "new-password", t("Repeat password"));
      submit = t("Register");
      links = link("user", t("Cancel"));
    } else {
      fields = input("user", "", "username", t("User name")) + input("pw", "password", "current-password", t("Password"));
      submit = t("Sign in");
      links = (registrationOpen ? link("register", t("Register"), t("Create an account on this server")) : "") + link("token", t("Access token"), t("Sign in with the access token of the server"));
    }
    box.innerHTML = '<form id="login" style="display:flex;gap:6px;flex-wrap:wrap">' + fields + '<button class="' + (loginMode === "register" ? "primary" : "") + '">' + esc(submit) + "</button>" + links +
      '</form><span id="login-error" class="error"></span>';
    box.querySelectorAll("[data-mode]").forEach(b => b.onclick = () => { loginMode = b.dataset.mode; renderAccount(); document.querySelector("#login input").focus(); });
    const fail = msg => { document.getElementById("login-error").textContent = msg; };
    document.getElementById("login").onsubmit = async e => {
      e.preventDefault();
      const value = id => document.getElementById(id).value;
      try {
        if (loginMode === "token") {
          const token = value("token").trim();
          const info = await api("/info", { headers: { Authorization: "Bearer " + token } });
          if (!info.user) throw Object.assign(new Error(t("Wrong access token")), { status: 0 });
          session = { token, user: info.user, access: info.access, kind: "token" };
        } else if (loginMode === "register") {
          const name = value("user").trim();
          if (!name || /\s/.test(name)) { fail(t("A user name must not be empty or contain blanks")); return; }
          if (value("pw") !== value("pw2")) { fail(t("The passwords do not match")); return; }
          if (value("pw").trim().length < 8) { fail(tf("The password needs at least {} characters", 8).replace(/&[^;]+;/g, "")); return; }
          const created = await api("/register", { method: "POST", json: { name, password: value("pw") } });
          loginMode = "user";
          // Waiting for an administrator: no session yet.
          if (!created.token) {
            renderAccount();
            const note = document.getElementById("login-error");
            note.className = "ok";
            note.textContent = t("Your account was created. An administrator has to approve it before you can sign in.");
            return;
          }
          session = created;
        } else {
          session = await api("/login", { method: "POST", json: { name: value("user"), password: value("pw") } });
        }
        keepSession();
        await loadFavorites();
        renderAccount();
        route();
      } catch (err) {
        const register = loginMode === "register";
        if (register && err.status === 409) fail(t("This user name is taken"));
        else if (register && err.status === 403) { registrationOpen = false; renderAccount(); fail(t("The registration is closed")); }
        else if (register && err.status === 429) fail(t("Too many registrations from this address, try again later"));
        else if (register && err.status === 400) fail(t("This user name cannot be used"));
        else if (loginMode === "user" && err.status === 403) fail(t("Your account awaits the approval of an administrator"));
        else fail(err.status === 401 ? (loginMode === "token" ? t("Wrong access token") : t("Wrong user name or password")) : err.status === 429 ? t("Too many failed sign-ins, try again later") : err.message);
      }
    };
  }
}

/// The user and the rights of the session as the server sees them now (roles and
/// grants may have changed since the sign-in).
async function refreshSession() {
  if (!session) return;
  try {
    const info = await api("/info");
    if (info.user && session) { session.user = info.user; session.access = info.access; keepSession(); }
  } catch (e) {}
  renderAccount();
}

async function logout() {
  try { await api("/logout", { method: "POST" }); } catch (e) {}
  session = null; favorites = []; keepSession();
  renderAccount();
  if (view === "favorites" || view === "admin") view = "parts";
  go();
}

async function loadFavorites() {
  favorites = [];
  if (!session) return;
  try { favorites = await api("/favorites"); }
  catch (e) { if (e.status === 401) { session = null; keepSession(); } }
}

async function toggleFavorite(number) {
  if (!session) return;
  const next = favorites.includes(number) ? favorites.filter(n => n !== number) : favorites.concat([number]);
  favorites = await api("/favorites", { method: "PUT", json: next });
}

function setNav() {
  for (const v of ["parts", "favorites", "catalogs", "admin"]) document.getElementById("nav-" + v).classList.toggle("active", view === v);
}

function content(html) { document.getElementById("content").innerHTML = html; }

async function search() {
  setNav();
  const q = Object.assign({}, query);
  if (view === "favorites") {
    if (!session) { document.getElementById("facets").innerHTML = ""; content('<div class="status">' + esc(t("Sign in to keep favorites")) + "</div>"); return; }
    q.numbers = favorites.length ? favorites : ["\u0000"];
  }
  let r;
  try { r = await api("/search", { method: "POST", json: q }); }
  catch (e) { content('<div class="status error">' + esc(t("The server cannot be reached")) + ": " + esc(e.message) + "</div>"); return; }
  renderFacets(r);
  renderResults(r);
}

function renderFacets(r) {
  let html = "";
  const chips = [];
  for (const f of FIXED) if (query[f]) chips.push({ label: t(TITLES[f]) + ": " + shown(f, query[f]), clear: () => { query[f] = null; } });
  query.attributes.forEach(([k, v], i) => chips.push({ label: k + ": " + v, clear: () => { query.attributes.splice(i, 1); } }));
  if (chips.length) {
    html += '<div class="chips">' + chips.map((c, i) => '<span class="chip" data-chip="' + i + '">' + esc(c.label) + " ✕</span>").join("") + '<span class="chip" data-chip="all">' + esc(t("Clear filters")) + "</span></div>";
  }
  const block = (name, title, values, selected, attribute) => {
    if (!values.length) return "";
    const open = expanded[name] || values.length <= 8;
    const list = (open ? values : values.slice(0, 8)).map((v, i) =>
      '<li class="' + (selected === v.value ? "sel" : "") + '" data-facet="' + esc(name) + '" data-attr="' + (attribute ? 1 : 0) + '" data-i="' + i + '"><span>' + esc(attribute ? v.value : shown(name, v.value)) + '</span><span class="count">' + v.count + "</span></li>").join("");
    const more = open ? "" : '<li class="more" data-more="' + esc(name) + '">… ' + (values.length - 8) + "</li>";
    return '<div class="facet"><h3>' + esc(title) + "</h3><ul>" + list + more + "</ul></div>";
  };
  for (const f of r.facets) html += block(f.name, t(TITLES[f.name] || f.name), f.values, query[f.name], false);
  if (r.attributes.length) html += '<h3 style="margin-top:18px">' + esc(t("Technical data")) + "</h3>";
  for (const f of r.attributes) {
    const sel = query.attributes.find(([k]) => k.toLowerCase() === f.name.toLowerCase());
    html += block(f.name, t(f.name), f.values, sel ? sel[1] : null, true);
  }
  const aside = document.getElementById("facets");
  aside.innerHTML = html;
  aside.querySelectorAll("[data-chip]").forEach(e => e.onclick = () => {
    if (e.dataset.chip === "all") { const keep = { sort: query.sort, descending: query.descending }; query = Object.assign(blank(), keep); }
    else chips[+e.dataset.chip].clear();
    query.offset = 0; go();
  });
  aside.querySelectorAll("[data-more]").forEach(e => e.onclick = () => { expanded[e.dataset.more] = true; renderFacets(r); });
  aside.querySelectorAll("li[data-facet]").forEach(e => e.onclick = () => {
    const name = e.dataset.facet;
    const all = e.dataset.attr === "1" ? r.attributes : r.facets;
    const value = all.find(f => f.name === name).values[+e.dataset.i].value;
    if (e.dataset.attr === "1") {
      const i = query.attributes.findIndex(([k]) => k.toLowerCase() === name.toLowerCase());
      if (i >= 0 && query.attributes[i][1] === value) query.attributes.splice(i, 1);
      else if (i >= 0) query.attributes[i] = [name, value];
      else query.attributes.push([name, value]);
    } else query[name] = query[name] === value ? null : value;
    query.offset = 0; go();
  });
}

function renderResults(r) {
  const sorts = ["Number", "Manufacturer", "Description", "Price", "Modified"];
  let html = '<div class="toolbar"><strong>' + tf("{} parts", r.total) + '</strong><span style="display:flex;gap:10px;flex-wrap:wrap"><label>' + esc(t("Sort by")) + ' <select id="sort">' +
    sorts.map(s => '<option value="' + s + '"' + (query.sort === s ? " selected" : "") + ">" + esc(key("sort", s)) + "</option>").join("") +
    '</select></label><label><input type="checkbox" id="desc"' + (query.descending ? " checked" : "") + "> " + esc(t("Descending")) + "</label>" +
    (view === "favorites" || query.attributes.length ? "" : '<a href="' + esc(exportUrl()) + '">' + esc(t("Export (CSV)")) + "</a>") + "</span></div>";
  if (!r.parts.length) html += '<div class="status">' + esc(t("No parts found")) + "</div>";
  else {
    html += '<div class="table-wrap"><table><thead><tr><th></th><th>' + esc(t("Part number")) + "</th><th>" + esc(t("Description")) + '</th><th class="hide-narrow">' + esc(t("Manufacturer")) + '</th><th class="hide-narrow">' + esc(t("Product group")) + '</th><th class="num">' + esc(t("Price")) + "</th><th>" + esc(t("Lifecycle")) + "</th></tr></thead><tbody>";
    for (const p of r.parts) {
      const fav = favorites.includes(p.number);
      const life = p.lifecycle || "Active";
      html += '<tr data-n="' + esc(p.number) + '"><td class="star">' + (session ? (fav ? "★" : "☆") : "") + "</td><td><strong>" + esc(p.number) + "</strong></td><td>" + esc(p.description) +
        '</td><td class="hide-narrow">' + esc(p.manufacturer) + '</td><td class="hide-narrow">' + esc(t(p.group)) + '</td><td class="num">' + money(p) + '</td><td><span class="badge ' + life + '">' + esc(key("lifecycle", life)) + "</span></td></tr>";
    }
    html += "</tbody></table></div>";
    const pages = Math.max(1, Math.ceil(r.total / LIMIT));
    const page = Math.floor(r.offset / LIMIT) + 1;
    html += '<div class="pager"><button id="prev"' + (page <= 1 ? " disabled" : "") + ">" + esc(t("Previous")) + "</button><span>" + tf("Page {} of {}", page, pages) + '</span><button id="next"' + (page >= pages ? " disabled" : "") + ">" + esc(t("Next")) + "</button></div>";
  }
  content(html);
  const el = id => document.getElementById(id);
  el("sort").onchange = e => { query.sort = e.target.value; query.offset = 0; go(); };
  el("desc").onchange = e => { query.descending = e.target.checked; go(); };
  if (el("prev")) el("prev").onclick = () => { query.offset = Math.max(0, query.offset - LIMIT); go(); };
  if (el("next")) el("next").onclick = () => { query.offset += LIMIT; go(); };
  document.querySelectorAll("tbody tr").forEach(tr => tr.onclick = async e => {
    if (e.target.classList.contains("star") && session) { await toggleFavorite(tr.dataset.n); search(); return; }
    location.hash = "part=" + encodeURIComponent(tr.dataset.n);
  });
}

/// A box with a heading; nothing when it has no rows.
function box(title, body) {
  return body ? '<div class="box"><h3>' + esc(title) + "</h3>" + body + "</div>" : "";
}

function rows(pairs) {
  const items = pairs.filter(([, v]) => v !== "" && v !== null && v !== undefined && v !== 0);
  return items.length ? "<dl>" + items.map(([k, v]) => "<dt>" + esc(t(k)) + "</dt><dd>" + v + "</dd>").join("") + "</dl>" : "";
}

async function showPart(number) {
  document.getElementById("facets").innerHTML = "";
  content('<div class="status">' + esc(t("Loading…")) + "</div>");
  let p, history = [], related = { accessories: [], fits: [] };
  try {
    p = await api("/parts/" + encodeURIComponent(number));
    history = await api("/parts/" + encodeURIComponent(number) + "/history").catch(() => []);
    related = await api("/parts/" + encodeURIComponent(number) + "/accessories").catch(() => related);
  } catch (e) {
    const msg = e.status === 404 ? tf("Part {} not found", number) : esc(e.message);
    content('<div class="status error">' + msg + '</div><div class="status"><a href="#" id="back">← ' + esc(t("Back to the results")) + "</a></div>");
    document.getElementById("back").onclick = ev => { ev.preventDefault(); history_back(); };
    return;
  }
  const life = p.lifecycle || "Active";
  const fav = favorites.includes(p.number);
  const mm = v => v ? v + " mm" : "";
  const md = p.mounting_data || {};
  const cutout = c => !c ? "" : c.round ? "Ø" + c.round.d + " mm" : c.rect ? c.rect.w + " x " + c.rect.h + " mm" : "";
  const files = p.attachments || [];
  let html = '<div class="detail"><div class="head"><div><a href="#" id="back">← ' + esc(t("Back to the results")) + "</a><h2>" + esc(p.number) + "</h2><div>" + esc(p.description) + " " +
    '<span class="badge ' + life + '">' + esc(key("lifecycle", life)) + "</span>" + (p.successor ? " " + tf("Replaced by {}", "\u0001") .replace("\u0001", partLink(p.successor)) : "") + "</div></div>";
  html += session ? '<div><button id="fav">' + (fav ? "★ " + esc(t("Remove from favorites")) : "☆ " + esc(t("Add to favorites"))) + "</button></div>" : "";
  html += '</div><div class="grid">';
  html += box(t("General"), rows([
    ["Type number", esc(p.type_number)], ["Order number", esc(p.order_number)], ["Manufacturer", esc(p.manufacturer)], ["Supplier", esc(p.supplier)],
    ["Product top group", esc(key("top_group", p.top_group || "Electrical"))], ["Product group", esc(t(p.group))], ["Product subgroup", esc(t(p.subgroup))],
    ["Catalog", esc(catalogNames[(p.catalog || "").toLowerCase()] || p.catalog || "")], ["Identifier", esc(p.identifier)], ["Note", esc(p.note)],
    ["Last change", esc(date(p.modified))]]));
  html += box(t("Technical data"), rows([
    ["Voltage", esc(p.voltage)], ["Current", esc(p.current)], ["Power", esc(p.power)], ["Technical data", esc(p.tech_data)], ["Certificates", esc(p.certificates)],
    ["Datasheet", webLink(p.datasheet) ? '<a href="' + esc(p.datasheet) + '" target="_blank" rel="noopener noreferrer">' + esc(p.datasheet) + "</a>" : esc(p.datasheet)],
    ["Functions", esc((p.functions || []).map(f => key("fn", f.kind) + (f.pins ? " " + f.pins : "")).join("\n"))]]));
  html += box(t("Commercial data") + " / " + t("Mounting data"), rows([
    ["Price", money(p) + (p.price_unit > 1 ? " / " + p.price_unit + " " + esc(p.quantity_unit) : "")], ["Width", mm(p.width)], ["Height", mm(p.height)], ["Depth", mm(p.depth)],
    ["Weight", p.weight ? p.weight + " kg" : ""], ["Mounting", esc(p.mounting)],
    ["Power loss", md.power_loss ? md.power_loss + " W" : ""], ["Clearance top", mm(md.clearance_top)], ["Clearance bottom", mm(md.clearance_bottom)],
    ["Mounting holes", (md.holes || []).length ? esc((md.hole_thread || "") + ": " + md.holes.map(h => h[0] + "," + h[1]).join("; ")) : ""],
    ["Door cutout", cutout(md.door_cutout)], ["Cutout", cutout(md.cutout)]]));
  const relatedList = list => list.map(a => partLink(a.number) + (a.description ? ' <span class="muted">' + esc(a.description) + "</span>" : "")).join("<br>");
  html += box(t("Accessories"), rows([
    ["Accessory", p.accessory ? esc(t("Yes")) + " · " + esc(key("mount", p.accessory_mount || "Auto")) : ""],
    ["Accessories", relatedList(related.accessories || [])], ["Fits", relatedList(related.fits || [])]]));
  const graphics = [];
  if (p.model_2d && (p.model_2d.prims || []).length) {
    graphics.push("<figure><figcaption>" + esc(t("Front view (2D)")) + "</figcaption>" + frontView(p.model_2d) + "</figure>");
  }
  if (p.model_3d && (p.model_3d.triangles || []).length) {
    graphics.push("<figure><figcaption>" + esc(t("3D model")) + '</figcaption><canvas id="mesh" title="' + esc(t("Drag to turn the model")) + '"></canvas></figure>');
  }
  if (graphics.length) html += '<div class="box wide"><h3>' + esc(t("Graphics")) + '</h3><div class="graphics">' + graphics.join("") + "</div></div>";
  if (files.length) {
    const link = a => '<a href="' + API + "/files/" + encodeURIComponent(a.id) + '" target="_blank" rel="noopener">' + esc(a.name) + "</a>";
    html += '<div class="box"><h3>' + esc(t("Files")) + "</h3>" + rows(files.map(a => [key("kind", a.kind || "Other"), link(a) + ' <span class="muted">' + size(a.size) + "</span>"]));
    const images = files.filter(a => a.kind === "Image" || a.kind === "Drawing" && /\.svg$/i.test(a.name));
    if (images.length) html += '<div class="images">' + images.map(a => '<img loading="lazy" alt="' + esc(a.name) + '" src="' + API + "/files/" + encodeURIComponent(a.id) + '">').join("") + "</div>";
    html += "</div>";
  }
  if (history.length) {
    html += '<div class="box"><h3>' + esc(t("History")) + "</h3>" + rows(history.map(h => [tf("Revision {}", h.revision), esc(dateTime(h.time) + " · " + key("change", h.change) + (h.user ? " · " + h.user : ""))])) + "</div>";
  }
  html += "</div></div>";
  content(html);
  document.getElementById("back").onclick = e => { e.preventDefault(); history_back(); };
  const fb = document.getElementById("fav");
  if (fb) fb.onclick = async () => { await toggleFavorite(p.number); showPart(p.number); };
  const canvas = document.getElementById("mesh");
  if (canvas) meshView(canvas, p.model_3d);
}

/// SVG of a 2D front view (mm, origin top left, y down).
function frontView(m) {
  const w = m.width || 1, h = m.height || 1, pad = Math.max(w, h) * 0.03;
  const n = v => String(Math.round((+v || 0) * 100) / 100);
  const fill = p => p.fill ? ' class="fill"' : "";
  const pts = list => (list || []).map(q => n(q[0]) + "," + n(q[1])).join(" ");
  const shape = p => {
    switch (p.type) {
      case "line": return '<line x1="' + n(p.a[0]) + '" y1="' + n(p.a[1]) + '" x2="' + n(p.b[0]) + '" y2="' + n(p.b[1]) + '"/>';
      case "rect": return '<rect x="' + n(p.min[0]) + '" y="' + n(p.min[1]) + '" width="' + n(p.max[0] - p.min[0]) + '" height="' + n(p.max[1] - p.min[1]) + '"' + fill(p) + "/>";
      case "circle": return '<circle cx="' + n(p.c[0]) + '" cy="' + n(p.c[1]) + '" r="' + n(p.r) + '"' + fill(p) + "/>";
      case "polyline": return "<" + (p.closed ? "polygon" : "polyline") + ' points="' + pts(p.pts) + '"' + fill(p) + "/>";
      case "text": return '<text x="' + n(p.pos[0]) + '" y="' + n(p.pos[1]) + '" font-size="' + n(p.size || 3) + '">' + esc(p.text) + "</text>";
      default: return "";
    }
  };
  return '<svg viewBox="' + [-pad, -pad, w + 2 * pad, h + 2 * pad].map(n).join(" ") + '" preserveAspectRatio="xMidYMid meet" role="img">' + (m.prims || []).map(shape).join("") + "</svg>";
}

/// Shaded 3D model on a canvas (x right, y up, z towards the viewer) with its sharp
/// edges; dragging turns it.
function meshView(canvas, m) {
  const V = m.vertices || [], T = (m.triangles || []).filter(t => t.every(i => V[i]));
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  V.forEach(v => { for (let k = 0; k < 3; k++) { min[k] = Math.min(min[k], v[k]); max[k] = Math.max(max[k], v[k]); } });
  const mid = [0, 1, 2].map(k => (min[k] + max[k]) / 2);
  const size = Math.hypot(max[0] - min[0], max[1] - min[1], max[2] - min[2]) || 1;
  const sub = (p, q) => [p[0] - q[0], p[1] - q[1], p[2] - q[2]];
  const cross = (u, v) => [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
  const unit = v => { const l = Math.hypot(v[0], v[1], v[2]); return l ? v.map(x => x / l) : v; };
  const dot = (u, v) => u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
  const normals = T.map(([a, b, c]) => unit(cross(sub(V[b], V[a]), sub(V[c], V[a]))));
  // Sharp edges: at the border or between faces at more than 20°, found by position so
  // that meshes without shared vertices (STL) work too.
  const key = v => v.map(x => Math.round(x * 100)).join(",");
  const byKey = new Map();
  T.forEach((t, n) => [[t[0], t[1]], [t[1], t[2]], [t[2], t[0]]].forEach(([i, j]) => {
    const [ki, kj] = [key(V[i]), key(V[j])];
    const k = ki < kj ? ki + "|" + kj : kj + "|" + ki;
    if (!byKey.has(k)) byKey.set(k, { a: V[i], b: V[j], faces: [] });
    byKey.get(k).faces.push(n);
  }));
  const edges = [...byKey.values()].filter(e => e.faces.length !== 2 || dot(normals[e.faces[0]], normals[e.faces[1]]) < 0.94);
  let yaw = 0.6, pitch = 0.35;
  const limit = size * 0.06;
  // Painted back to front: small pieces keep large faces from covering what is in
  // front of them.
  const split = (a, b, c, depth, put) => {
    const d2 = (p, q) => (p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2;
    const e = [d2(a, b), d2(b, c), d2(c, a)];
    const k = e.indexOf(Math.max(...e));
    if (depth >= 6 || e[k] <= limit * limit) { put(a, b, c); return; }
    const [p, q, r] = [[a, b, c], [b, c, a], [c, a, b]][k];
    const md = [(p[0] + q[0]) / 2, (p[1] + q[1]) / 2, (p[2] + q[2]) / 2];
    split(p, md, r, depth + 1, put);
    split(md, q, r, depth + 1, put);
  };
  const draw = () => {
    if (!canvas.isConnected) return;
    const ratio = window.devicePixelRatio || 1, W = canvas.clientWidth, H = canvas.clientHeight;
    canvas.width = Math.round(W * ratio);
    canvas.height = Math.round(H * ratio);
    const g = canvas.getContext("2d");
    g.setTransform(ratio, 0, 0, ratio, 0, 0);
    g.clearRect(0, 0, W, H);
    const [cy, sy, cp, sp] = [Math.cos(yaw), Math.sin(yaw), Math.cos(pitch), Math.sin(pitch)];
    const s = 0.9 * Math.min(W, H) / size;
    // Turned so that the eye looks from the yaw / pitch direction (orthographic).
    const turn = v => {
      const x = v[0] - mid[0], y = v[1] - mid[1], z = v[2] - mid[2];
      const x1 = x * cy - z * sy, z1 = x * sy + z * cy;
      return [x1, y * cp - z1 * sp, y * sp + z1 * cp];
    };
    const P = V.map(turn);
    const facing = normals.map(n => turn([n[0] + mid[0], n[1] + mid[1], n[2] + mid[2]]));
    const light = unit([0.35, 0.55, 0.76]);
    const dark = matchMedia("(prefers-color-scheme: dark)").matches;
    const items = [];
    T.forEach(([a, b, c], n) => {
      if (facing[n][2] <= 1e-6) return;
      const lit = Math.max(0, dot(facing[n], light));
      split(P[a], P[b], P[c], 0, (p, q, r) => {
        const depth = (p[2] + q[2] + r[2]) / 3;
        // Nearer faces a little lighter, so that parallel faces differ.
        const k = (dark ? 55 + 140 * lit : 115 + 115 * lit) + 18 * (depth / size + 0.5);
        items.push([depth, "rgb(" + Math.round(k * 0.92) + "," + Math.round(k * 0.96) + "," + Math.round(Math.min(255, k)) + ")", [p, q, r]]);
      });
    });
    const ink = dark ? "rgba(10,12,16,0.85)" : "rgba(40,46,58,0.8)";
    for (const e of edges) {
      if (!e.faces.some(n => facing[n][2] > 1e-6)) continue;
      const [a, b] = [turn(e.a), turn(e.b)];
      const n = Math.max(1, Math.ceil(Math.hypot(...sub(a, b)) / limit));
      for (let i = 0; i < n; i++) {
        const [t0, t1] = [i / n, (i + 1) / n];
        const p = a.map((x, k) => x + (b[k] - x) * t0), q = a.map((x, k) => x + (b[k] - x) * t1);
        items.push([(p[2] + q[2]) / 2 + limit * 0.6, ink, [p, q]]);
      }
    }
    items.sort((f, q) => f[0] - q[0]);
    const at = q => [W / 2 + q[0] * s, H / 2 - q[1] * s];
    g.lineJoin = "round";
    for (const [, col, pts] of items) {
      g.beginPath();
      pts.forEach((q, i) => { const [x, y] = at(q); if (i) g.lineTo(x, y); else g.moveTo(x, y); });
      if (pts.length === 3) {
        g.closePath();
        g.fillStyle = g.strokeStyle = col;
        g.lineWidth = 0.6;
        g.fill();
        g.stroke();
      } else {
        g.strokeStyle = col;
        g.lineWidth = 1;
        g.stroke();
      }
    }
  };
  let drag = null;
  canvas.onpointerdown = e => { drag = [e.clientX, e.clientY]; canvas.setPointerCapture(e.pointerId); canvas.style.cursor = "grabbing"; };
  canvas.onpointermove = e => {
    if (!drag) return;
    yaw -= (e.clientX - drag[0]) * 0.01;
    pitch = Math.max(-1.5, Math.min(1.5, pitch + (e.clientY - drag[1]) * 0.01));
    drag = [e.clientX, e.clientY];
    requestAnimationFrame(draw);
  };
  canvas.onpointerup = canvas.onpointercancel = () => { drag = null; canvas.style.cursor = ""; };
  addEventListener("resize", () => requestAnimationFrame(draw));
  draw();
}

/// Back to the list the part was opened from (its filters are still in `query`).
function history_back() { location.hash = stateHash(); }

async function showCatalogs() {
  setNav();
  document.getElementById("facets").innerHTML = "";
  let list = [];
  try { list = await api("/catalogs"); } catch (e) { content('<div class="status error">' + esc(e.message) + "</div>"); return; }
  if (!list.length) { content('<div class="status">–</div>'); return; }
  content('<div class="cards">' + list.map((c, i) => '<div class="box"><h3>' + esc(c.manufacturer) + "</h3><strong>" + esc(c.name) + "</strong>" +
    rows([["Version", esc(c.version)], ["Parts", String(c.parts)], ["Last change", esc(date(c.modified))]]) + (c.description ? '<p class="muted">' + esc(c.description) + "</p>" : "") +
    '<button data-c="' + i + '">' + esc(t("Show parts")) + "</button></div>").join("") + "</div>");
  document.querySelectorAll("[data-c]").forEach(b => b.onclick = () => {
    const c = list[+b.dataset.c];
    query = blank(); query.catalog = c.id; view = "parts"; go();
  });
}

function route() {
  const m = location.hash.match(/^#part=(.+)$/);
  if (m) { showPart(decodeURIComponent(m[1])); return; }
  readHash();
  if (view === "catalogs") showCatalogs(); else if (view === "admin") showAdmin(); else search();
}

async function footer() {
  try {
    const info = await api("/info");
    if (registrationOpen !== !!info.registration) {
      registrationOpen = !!info.registration;
      if (!session) renderAccount();
    }
    document.getElementById("footer").textContent = info.name + " " + info.version + " · " + tf("{} parts in {} catalogs, revision {}", info.parts, info.catalogs, info.revision).replace(/&[^;]+;/g, "");
  } catch (e) {}
}

async function start() {
  // The language of the address (?lang=de) wins over the one chosen last.
  const asked = new URLSearchParams(location.search).get("lang");
  const stored = load("lang");
  try { await setLanguage(asked || stored || navigator.language || "en"); } catch (e) {}
  try { (await api("/catalogs")).forEach(c => { catalogNames[c.id.toLowerCase()] = c.name; catalogManufacturers[c.id.toLowerCase()] = c.manufacturer; }); } catch (e) {}
  await refreshSession();
  await loadFavorites();
  renderAccount();
  document.getElementById("lang").onchange = async e => { await setLanguage(e.target.value); route(); footer(); };
  document.getElementById("search-form").onsubmit = e => {
    e.preventDefault();
    query.q = document.getElementById("q").value.trim() || null;
    query.offset = 0;
    if (view === "catalogs" || view === "admin") view = "parts";
    go();
  };
  document.getElementById("nav-parts").onclick = () => { view = "parts"; query.numbers = []; query.offset = 0; go(); };
  document.getElementById("nav-favorites").onclick = async () => { view = "favorites"; query.offset = 0; await loadFavorites(); go(); };
  document.getElementById("nav-catalogs").onclick = () => { view = "catalogs"; go(); };
  document.getElementById("nav-admin").onclick = () => { view = "admin"; go(); };
  window.onhashchange = route;
  route();
  footer();
}
start();
