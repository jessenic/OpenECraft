"use strict";
/// Administration of the server (overview, users, access rights, catalogs, import),
/// shown to signed-in users with the rights for it. The page only offers what the
/// caller may do; the server checks every change. Loaded before portal.js, whose
/// helpers (t, tf, esc, api, rows, box, …) it uses.
const ADMIN_TABS = [
  ["overview", "Overview", () => true],
  ["users", "Users", () => may("administer", "*")],
  ["access", "Access rights", () => may("administer", "*")],
  ["groups", "Groups", () => may("administer", "*")],
  ["directories", "Directories", () => may("administer", "*")],
  ["catalogs", "Catalogs", () => may("administer", "*") || mayAnywhere("edit_catalogs")],
  ["import", "Import parts", () => mayAnywhere("edit_parts")],
  ["backup", "Export and backup", () => true],
];
const ROLES = ["Reader", "Manufacturer", "Editor", "Admin"];
let adminTab = "overview";
/// The access settings being edited (saved together).
let policy = null;
let providers = [];

/// Text with `{}` placeholders for dialogs (not HTML: nothing is escaped).
function tp(s, ...args) { let i = 0; return t(s).replace(/\{(\d*)\}/g, (_, n) => String(args[n === "" ? i++ : +n])); }

/// Case-insensitive match with `*` (any text) and `?` (one character).
function wild(pattern, text) {
  const re = pattern.trim().toLowerCase().replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, ".*").replace(/\?/g, ".");
  return new RegExp("^" + re + "$", "s").test(text.trim().toLowerCase());
}
function serverWide(right) { const r = (T.rights || []).find(x => x.name === right); return r ? r.server_wide : right === "read" || right === "administer"; }
function entries() { return (session && session.access && session.access.entries) || []; }
/// The caller's rights on a scope (a manufacturer, `catalog:<id>` or `*`), as `Access::rights` resolves them.
function rightsOn(scope) { return rightsOnAll([scope]); }
/// The rights on several scopes together, as `Access::rights_on` resolves them: a
/// deny on any of them takes the right away.
function rightsOnAll(scopes) {
  scopes = scopes.map(s => String(s || "").trim());
  const server = scopes.includes("*");
  const covering = entries().filter(e => { const p = (e.scope || "").trim() || "*"; return scopes.some(s => s === "*" ? p === "*" : wild(p, s)); });
  const denied = new Set(covering.filter(e => e.deny).flatMap(e => e.rights || []));
  return new Set(covering.filter(e => !e.deny).flatMap(e => e.rights || []).filter(r => !denied.has(r) && (server || !serverWide(r))));
}
function may(right, scope) { return rightsOn(serverWide(right) ? "*" : (scope || "").trim()).has(right); }
function mayAnywhere(right) { return entries().some(e => !e.deny && (e.rights || []).includes(right)); }
function catalogScope(id) { return "catalog:" + String(id || "").trim(); }
/// The right on a part or catalog: by its manufacturer or by its catalog.
function mayOnPart(right, p) {
  if (!session) return false;
  const scopes = [p.manufacturer || ""].concat((p.catalog || "").trim() ? [catalogScope(p.catalog)] : []);
  return rightsOnAll(scopes).has(right);
}
function mayCreateParts() { return !!session && mayAnywhere("edit_parts"); }
/// The panel is for those who manage the server, its catalogs or its parts.
function adminAllowed() { return !!session && (may("administer", "*") || mayAnywhere("edit_catalogs") || mayAnywhere("edit_parts")); }

function roleTitle(name) {
  const b = (T.roles || []).find(r => r.name.toLowerCase() === String(name || "").trim().toLowerCase());
  return b ? b.title : name;
}
function roleNames() { return (T.roles || []).map(r => r.name).concat(((policy && policy.roles) || []).map(r => r.name.trim()).filter(Boolean)); }
function options(values, selected, title) { return values.map(v => '<option value="' + esc(v) + '"' + (v === selected ? " selected" : "") + ">" + esc(title ? title(v) : v) + "</option>").join(""); }
function datalist(id, values) { return '<datalist id="' + id + '">' + [...new Set(values.filter(Boolean))].sort().map(v => '<option value="' + esc(v) + '">').join("") + "</datalist>"; }
function message(id, text, ok) { const e = document.getElementById(id); if (e) { e.className = ok ? "ok" : "error"; e.textContent = text; } }
/// A label for the input of `html` (the first `id` in it).
function labelFor(label, html) { const id = (html.match(/id="([^"]+)"/) || [])[1]; return "<label" + (id ? ' for="' + id + '"' : "") + ">" + esc(t(label)) + "</label>"; }
function adminBody(html) { const b = document.getElementById("admin-body"); if (b) b.innerHTML = html; return !!b; }
/// The manufacturers the server knows (catalogs and parts), for suggestions.
async function manufacturers() {
  const names = Object.values(catalogManufacturers);
  try {
    const r = await api("/search", { method: "POST", json: Object.assign(blank(), { limit: 1 }) });
    const f = r.facets.find(f => f.name === "manufacturer");
    if (f) names.push(...f.values.map(v => v.value));
  } catch (e) {}
  return names;
}

async function showAdmin() {
  setNav();
  document.getElementById("facets").innerHTML = "";
  if (!adminAllowed()) { content('<div class="status">' + esc(t("Sign in as an administrator to manage the server")) + "</div>"); return; }
  const tabs = ADMIN_TABS.filter(([, , ok]) => ok());
  if (!tabs.some(([id]) => id === adminTab)) adminTab = "overview";
  content('<div class="admin"><div class="tabs">' + tabs.map(([id, title]) => '<button data-tab="' + id + '"' + (id === adminTab ? ' class="active"' : "") + ">" + esc(t(title)) + "</button>").join("") +
    '</div><div id="admin-body"><div class="status">' + esc(t("Loading…")) + "</div></div></div>");
  document.querySelectorAll("[data-tab]").forEach(b => b.onclick = () => { adminTab = b.dataset.tab; go(); });
  const views = { overview: adminOverview, users: adminUsers, access: adminAccess, groups: adminGroups, directories: adminDirectories, catalogs: adminCatalogs, import: adminImport, backup: adminBackup };
  try { await views[adminTab](); }
  catch (e) {
    // Signed out meanwhile (e.g. the own sessions ended).
    if (!session) { route(); return; }
    adminBody('<div class="status error">' + esc(e.message) + "</div>");
  }
}

async function adminOverview() {
  const info = await api("/info");
  const admin = may("administer", "*");
  const [users, settings, registration, dashboard] = admin ? await Promise.all([api("/users"), api("/access"), api("/registration"), api("/dashboard/settings")]) : [[], null, null, null];
  let html = '<div class="cards">';
  html += box(t("Server"), rows([["Version", esc(info.name + " " + info.version)], ["Storage", esc(info.storage)],
    ["Status", info.writable ? esc(t("Changes allowed")) : '<span class="error">' + esc(t("Read only")) + "</span>"],
    ["Parts", String(info.parts)], ["Catalogs", String(info.catalogs)], ["Revision", String(info.revision)]]));
  if (admin) {
    const count = f => String(users.filter(f).length);
    html += box(t("Users"), rows(ROLES.map(r => [key("role", r), count(u => u.role === r)]).concat([
      ["Directory", count(u => u.provider)], ["Signed in", count(u => u.sessions > 0)],
      ["Awaiting approval", users.some(u => u.pending) ? '<a href="#admin/users">' + count(u => u.pending) + "</a>" : "0"]])));
    const p = settings.policy;
    html += box(t("Access rights"), rows([["Without sign-in", esc(p.anonymous ? roleTitle(p.anonymous) : t("Sign-in needed to read"))],
      ["Registration", esc(registration.enabled ? t("The registration is open") : t("The registration is closed"))],
      ["Approval of new accounts", esc(registration.approval ? t("Yes") : t("No"))],
      ["Defined roles", String((p.roles || []).length)], ["Grants", String((p.grants || []).length)], ["Groups", String((settings.groups || []).length)],
      ["Directories", esc((settings.providers || []).map(d => d.title || d.id).join(", ") || t("Only local accounts"))]]));
    html += '<div class="box"><h3>' + esc(t("Dashboard")) + '</h3><label class="check"><input type="checkbox" id="dashboard-shown"' + (dashboard.enabled ? " checked" : "") + (dashboard.fixed ? " disabled" : "") + "> " +
      esc(t("Show the dashboard with graphs of the parts")) + '</label><p class="hint muted">' +
      esc(dashboard.fixed ? t("Set by the start options of the server (--dashboard or OPENECRAFT_DASHBOARD).") : t("Everybody who may read the parts sees it.")) + '</p><span id="dashboard-msg"></span></div>';
  }
  // Where the caller has each right: server-wide rights apply or not, the others per manufacturer.
  const mine = (T.rights || []).map(r => {
    const where = r.server_wide ? (may(r.name, "*") ? t("Yes") : "") : rightsOn("*").has(r.name) ? t("All manufacturers") :
      [...new Set(entries().filter(e => !e.deny && (e.rights || []).includes(r.name)).map(e => e.scope))].filter(s => may(r.name, s)).join(", ");
    return [r.title, esc(where)];
  });
  html += box(tp("Signed in as {}", session.user.name), rows(mine));
  adminBody(html + "</div>");
  const shown = document.getElementById("dashboard-shown");
  if (shown) shown.onchange = async () => {
    shown.disabled = true;
    try {
      const s = await api("/dashboard/settings", { method: "PUT", json: { enabled: shown.checked } });
      shown.checked = s.enabled;
      message("dashboard-msg", s.enabled ? t("The dashboard is shown") : t("The dashboard is hidden"), true);
      await footer();
    } catch (e) { shown.checked = !shown.checked; message("dashboard-msg", e.message); }
    shown.disabled = false;
  };
}

// Users

async function adminUsers() {
  const [users, makers, defined] = await Promise.all([api("/users"), manufacturers(), api("/groups")]);
  users.sort((a, b) => (b.pending ? 1 : 0) - (a.pending ? 1 : 0) || a.name.localeCompare(b.name));
  const groups = defined.map(g => g.name);
  const waiting = users.filter(u => u.pending).length;
  adminBody('<div id="user-form"></div>' + (waiting ? '<p class="notice">' + tf("Awaiting approval: {}", waiting) + "</p>" : "") + '<div class="toolbar"><input id="user-filter" type="search" placeholder="' + esc(t("Search users")) + '">' +
    '<button class="primary" id="new-user">' + esc(t("New user")) + '</button></div><div id="user-list"></div>' + datalist("makers", makers) + datalist("groups", groups));
  const list = () => {
    const f = document.getElementById("user-filter").value.trim().toLowerCase();
    const shown = users.filter(u => !f || u.name.toLowerCase().includes(f) || (u.manufacturer || "").toLowerCase().includes(f) || (u.groups || []).some(g => g.toLowerCase().includes(f)));
    let html = '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("User name")) + "</th><th>" + esc(t("Role")) + "</th><th>" + esc(t("Manufacturer")) + '</th><th class="hide-narrow">' + esc(t("Groups")) +
      '</th><th class="hide-narrow">' + esc(t("Directory")) + "</th><th>" + esc(t("Last sign-in")) + "</th><th></th></tr></thead><tbody>";
    for (const u of shown) {
      const i = users.indexOf(u);
      const signedIn = u.signed_in ? esc(dateTime(u.signed_in)) + (u.sessions ? ' <span class="muted">(' + tf("Open sessions: {}", u.sessions) + ")</span>" : "") : '<span class="muted">' + esc(t("Never")) + "</span>";
      html += "<tr><td><strong>" + esc(u.name) + "</strong>" + (u.name === session.user.name ? ' <span class="muted">(' + esc(t("you")) + ")</span>" : "") +
        (u.pending ? ' <span class="badge PhaseOut">' + esc(t("Awaiting approval")) + "</span>" : "") + "</td><td>" + esc(key("role", u.role)) + "</td><td>" + esc(u.manufacturer) +
        '</td><td class="hide-narrow">' + esc((u.groups || []).join(", ")) + '</td><td class="hide-narrow">' + esc(u.provider || t("Local")) + "</td><td>" + signedIn +
        '</td><td class="actions">' + (u.pending ? '<button class="small primary" data-approve="' + i + '">' + esc(t("Approve")) + '</button><button class="small danger" data-reject="' + i + '">' + esc(t("Reject")) + "</button>" :
          '<button class="small" data-edit="' + i + '">' + esc(t("Edit")) + "</button>" +
          (u.sessions ? '<button class="small" data-out="' + i + '">' + esc(t("Sign out everywhere")) + "</button>" : "") +
          '<button class="small danger" data-del="' + i + '">' + esc(t("Delete")) + "</button>") + "</td></tr>";
    }
    if (!shown.length) html += '<tr><td colspan="7" class="muted">' + esc(t("No users")) + "</td></tr>";
    document.getElementById("user-list").innerHTML = html + '</tbody></table></div><p id="users-msg"></p>';
    document.querySelectorAll("[data-edit]").forEach(b => b.onclick = () => userForm(users[+b.dataset.edit]));
    document.querySelectorAll("[data-out]").forEach(b => b.onclick = async () => {
      const u = users[+b.dataset.out];
      if (!confirm(tp("Sign out {} on all devices?", u.name))) return;
      try { await api("/users/" + encodeURIComponent(u.name) + "/sessions", { method: "DELETE" }); await showAdmin(); }
      catch (e) { message("users-msg", e.message); }
    });
    document.querySelectorAll("[data-approve]").forEach(b => b.onclick = async () => {
      const u = users[+b.dataset.approve];
      try { await api("/users/" + encodeURIComponent(u.name) + "/approval", { method: "POST" }); await showAdmin(); message("users-msg", tp("Approved user {}", u.name), true); }
      catch (e) { message("users-msg", e.message); }
    });
    document.querySelectorAll("[data-reject]").forEach(b => b.onclick = async () => {
      const u = users[+b.dataset.reject];
      if (!confirm(tp("Reject the registration of {}? The account is deleted.", u.name))) return;
      try { await api("/users/" + encodeURIComponent(u.name), { method: "DELETE" }); await showAdmin(); }
      catch (e) { message("users-msg", e.message); }
    });
    document.querySelectorAll("[data-del]").forEach(b => b.onclick = async () => {
      const u = users[+b.dataset.del];
      if (!confirm(tp("Delete user {}? Their sessions and favorites are removed.", u.name))) return;
      try { await api("/users/" + encodeURIComponent(u.name), { method: "DELETE" }); await showAdmin(); }
      catch (e) { message("users-msg", e.message); }
    });
  };
  document.getElementById("user-filter").oninput = list;
  document.getElementById("new-user").onclick = () => userForm(null);
  list();
}

/// The form of a new (`u` null) or an existing user.
function userForm(u) {
  const local = !u || !u.provider;
  const field = (label, input, hint) => labelFor(label, input) + "<div>" + input + (hint ? '<div class="hint">' + esc(hint) + "</div>" : "") + "</div>";
  let html = '<form id="uf" class="box section"><h3>' + (u ? tf("Edit user {}", u.name) : esc(t("New user"))) + '</h3><div class="form">';
  html += field("User name", '<input id="uf-name" autocomplete="off" required' + (u ? ' value="' + esc(u.name) + '" disabled' : "") + ">");
  html += field("Role", '<select id="uf-role">' + options(ROLES, u ? u.role : "Reader", r => key("role", r)) + "</select>",
    t("Readers keep favorites, manufacturers maintain the parts and catalogs of their manufacturer, editors all parts, administrators also the users."));
  html += '<label id="uf-maker-label" for="uf-maker">' + esc(t("Manufacturer")) + '</label><div id="uf-maker-box"><input id="uf-maker" list="makers" value="' + esc(u ? u.manufacturer : "") + '"></div>';
  if (local) {
    html += field(u ? "New password" : "Password", '<input id="uf-pw" type="password" autocomplete="new-password"' + (u ? ' placeholder="' + esc(t("unchanged if empty")) + '"' : " required") + ">",
      tp("At least {} characters", 8));
    html += field("Repeat password", '<input id="uf-pw2" type="password" autocomplete="new-password">');
    html += field("Groups", '<input id="uf-groups" list="groups" value="' + esc(u ? (u.groups || []).join(", ") : "") + '" placeholder="' + esc(t("separated by commas")) + '">', t("Grants of the server's access settings can refer to them"));
  } else {
    const own = u.local_groups || [];
    html += field("Directory", "<span>" + esc(u.provider) + "</span>", t("The directory checks the password and sets the groups at every sign-in"));
    html += field("Directory groups", "<span>" + esc((u.groups || []).filter(g => !own.includes(g)).join(", ")) + "</span>");
    html += field("Groups", '<input id="uf-groups" list="groups" value="' + esc(own.join(", ")) + '" placeholder="' + esc(t("separated by commas")) + '">', t("Groups of this server, next to those of the directory"));
  }
  html += '</div><div class="buttons"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="uf-cancel">' + esc(t("Cancel")) + '</button><span id="uf-msg"></span></div></form>';
  const box = document.getElementById("user-form");
  box.innerHTML = html;
  const el = id => document.getElementById(id);
  const makerShown = () => { const on = el("uf-role").value === "Manufacturer"; el("uf-maker-label").hidden = el("uf-maker-box").hidden = !on; };
  el("uf-role").onchange = makerShown;
  makerShown();
  el("uf-cancel").onclick = () => { box.innerHTML = ""; };
  (u ? el("uf-role") : el("uf-name")).focus();
  box.scrollIntoView({ block: "nearest" });
  el("uf").onsubmit = async e => {
    e.preventDefault();
    const name = u ? u.name : el("uf-name").value.trim();
    const role = el("uf-role").value;
    const update = { role, manufacturer: role === "Manufacturer" ? el("uf-maker").value.trim() : "", password: "" };
    if (!name || /\s/.test(name)) { message("uf-msg", t("A user name must not be empty or contain blanks")); return; }
    if (role === "Manufacturer" && !update.manufacturer) { message("uf-msg", t("A manufacturer user needs a manufacturer")); return; }
    if (local) {
      const pw = el("uf-pw").value;
      if (pw !== el("uf-pw2").value) { message("uf-msg", t("The passwords do not match")); return; }
      if ((pw || !u) && pw.trim().length < 8) { message("uf-msg", tp("The password needs at least {} characters", 8)); return; }
      update.password = pw;
    }
    update.groups = el("uf-groups").value.split(",").map(g => g.trim()).filter(Boolean);
    if (!u && !confirmNew(name)) return;
    try {
      await api("/users/" + encodeURIComponent(name), { method: "PUT", json: update });
      await showAdmin();
      message("users-msg", tp("Saved user {}", name), true);
    } catch (err) { message("uf-msg", err.status === 409 ? t("The last administrator cannot lose the right to manage users") : err.message); }
  };
  // A new name that only differs in case would change the existing user.
  function confirmNew(name) {
    const same = [...document.querySelectorAll("#user-list strong")].map(s => s.textContent).find(n => n.toLowerCase() === name.toLowerCase());
    return !same || confirm(tp("User {} exists already. Change it?", same));
  }
}

// Access rights: anonymous access, defined roles, grants and directories.

async function adminAccess() {
  const [s, users, makers, registration, catalogs] = await Promise.all([api("/access"), api("/users"), manufacturers(), api("/registration"), api("/catalogs")]);
  policy = Object.assign({ anonymous: "", roles: [], grants: [] }, s.policy);
  providers = s.providers || [];
  const groups = (s.groups || []).map(g => g.name).concat(users.flatMap(u => u.groups || []));
  adminBody('<div id="registration-box"></div><div id="access"></div>' + datalist("subjects-user", users.map(u => u.name)) + datalist("subjects-group", groups) +
    datalist("scopes", ["*"].concat(makers, catalogs.map(c => catalogScope(c.id)))));
  renderRegistration(registration);
  renderAccess();
}

/// Whether users may register themselves (switched here unless the start options of
/// the server set it) and whether an administrator approves new accounts.
function renderRegistration(r, note) {
  const check = (id, on, disabled, label) => '<label class="check"><input type="checkbox" id="' + id + '"' + (on ? " checked" : "") + (disabled ? " disabled" : "") + "> " + esc(label) + "</label>";
  document.getElementById("registration-box").innerHTML = '<div class="box section"><h3>' + esc(t("Registration")) + "</h3>" +
    check("registration", r.enabled, r.fixed, t("Users may register themselves (as readers)")) + '<p class="hint muted">' +
    esc(r.fixed ? t("Set by the start options of the server (--registration or OPENECRAFT_REGISTRATION).") : t("Registered users become readers: they keep favorites and read the parts even when reading needs a sign-in. Give them more rights in Users.")) + "</p>" +
    check("registration-approval", r.approval, false, t("New accounts need the approval of an administrator")) + '<p class="hint muted">' +
    esc(t("Waiting accounts cannot sign in; approve or reject them in Users.")) + '</p><span id="registration-msg"></span></div>';
  if (note) message("registration-msg", note, true);
  const enabled = document.getElementById("registration"), approval = document.getElementById("registration-approval");
  enabled.onchange = approval.onchange = async () => {
    enabled.disabled = approval.disabled = true;
    try {
      const next = await api("/registration", { method: "PUT", json: { enabled: enabled.checked, approval: approval.checked } });
      registrationOpen = next.enabled;
      renderRegistration(next, (next.enabled ? t("The registration is open") : t("The registration is closed")) + (next.enabled && next.approval ? " · " + t("New accounts need the approval of an administrator") : ""));
    } catch (e) { renderRegistration(r); message("registration-msg", e.message); }
  };
}

function renderAccess() {
  const rights = T.rights || [];
  const check = (on, attrs) => '<input type="checkbox"' + (on ? " checked" : "") + " " + attrs + ">";
  let html = '<p class="muted">' + esc(t("A role is a set of rights. Grants give a user or a group a role on the parts of a manufacturer (* and ? as wildcards, * for all); a deny grant takes the rights of the role away, whatever other grants give.")) + " " +
    esc(t("Grants on catalog:<id> apply to one catalog and its parts (Catalogs › Permissions).")) + "</p>";
  html += '<div class="form"><label for="anon">' + esc(t("Without sign-in")) + '</label><div><select id="anon"><option value="">' + esc(t("Sign-in needed to read")) + "</option>" +
    options(roleNames(), policy.anonymous, roleTitle) + "</select></div></div>";

  html += '<h3 class="section">' + esc(t("Roles")) + '</h3><div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Role")) + '</th><th class="hide-narrow">' + esc(t("Description")) + "</th>" +
    rights.map(r => '<th class="check">' + esc(r.title) + "</th>").join("") + "<th></th></tr></thead><tbody>";
  for (const b of T.roles || []) {
    html += "<tr><td>" + esc(b.title) + '</td><td class="hide-narrow muted">' + esc(t("Built-in")) + "</td>" + rights.map(r => '<td class="check">' + check(b.rights.includes(r.name), "disabled") + "</td>").join("") + "<td></td></tr>";
  }
  policy.roles.forEach((r, i) => {
    html += '<tr><td><input data-role="' + i + '" data-f="name" value="' + esc(r.name) + '" placeholder="' + esc(t("Name")) + '"></td><td class="hide-narrow"><input data-role="' + i + '" data-f="description" value="' + esc(r.description || "") + '"></td>' +
      rights.map(x => '<td class="check">' + check((r.rights || []).includes(x.name), 'data-role="' + i + '" data-right="' + x.name + '" aria-label="' + esc(x.title) + '"') + "</td>").join("") +
      '<td><button type="button" class="small danger" data-del-role="' + i + '" title="' + esc(t("Remove")) + '">✕</button></td></tr>';
  });
  html += '</tbody></table></div><div class="buttons"><button type="button" id="add-role">' + esc(t("New role")) + "</button></div>";

  html += '<h3 class="section">' + esc(t("Grants")) + '</h3><div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Type")) + "</th><th>" + esc(t("User or group")) + "</th><th>" + esc(t("Role")) + "</th><th>" +
    esc(t("Manufacturer")) + '</th><th class="check">' + esc(t("Deny")) + "</th><th></th></tr></thead><tbody>";
  const names = roleNames();
  policy.grants.forEach((g, i) => {
    const group = g.subject.group !== undefined;
    const who = group ? g.subject.group : g.subject.user || "";
    const roles = names.includes(g.role) ? names : names.concat([g.role]);
    html += '<tr><td><select data-grant="' + i + '" data-f="kind"><option value="user">' + esc(t("User")) + '</option><option value="group"' + (group ? " selected" : "") + ">" + esc(t("Group")) + "</option></select></td>" +
      '<td><input data-grant="' + i + '" data-f="who" list="subjects-' + (group ? "group" : "user") + '" value="' + esc(who) + '"' + (group ? ' placeholder="' + esc(t("directory:group")) + '"' : "") + "></td>" +
      '<td><select data-grant="' + i + '" data-f="role">' + options(roles, g.role, roleTitle) + "</select></td>" +
      '<td><input data-grant="' + i + '" data-f="scope" list="scopes" value="' + esc(g.scope || "") + '" placeholder="* (' + esc(t("All manufacturers")) + ')"></td>' +
      '<td class="check">' + check(g.deny, 'data-grant="' + i + '" data-f="deny" aria-label="' + esc(t("Deny")) + '"') + "</td>" +
      '<td><button type="button" class="small danger" data-del-grant="' + i + '" title="' + esc(t("Remove")) + '">✕</button></td></tr>';
  });
  if (!policy.grants.length) html += '<tr><td colspan="6" class="muted">' + esc(t("No grants: users have the rights of their role")) + "</td></tr>";
  html += '</tbody></table></div><div class="buttons"><button type="button" id="add-grant">' + esc(t("Add grant")) + "</button></div>";

  html += '<div class="buttons sticky"><button class="primary" id="save-access">' + esc(t("Save")) + '</button><button type="button" id="reload-access">' + esc(t("Discard changes")) + '</button><span id="access-msg"></span></div>';
  document.getElementById("access").innerHTML = html;

  const all = sel => document.querySelectorAll(sel);
  document.getElementById("anon").onchange = e => { policy.anonymous = e.target.value; };
  all("[data-role]").forEach(e => e.oninput = e.onchange = ev => {
    const r = policy.roles[+e.dataset.role];
    if (e.dataset.right) r.rights = rights.map(x => x.name).filter(n => n === e.dataset.right ? e.checked : (r.rights || []).includes(n));
    else r[e.dataset.f] = e.value;
    // The roles of the selections follow the names.
    if (e.dataset.f === "name" && ev.type === "change") renderAccess();
  });
  all("[data-grant]").forEach(e => e.onchange = e.oninput = ev => {
    const g = policy.grants[+e.dataset.grant];
    const f = e.dataset.f;
    if (f === "kind") { const who = g.subject.user ?? g.subject.group ?? ""; g.subject = e.value === "group" ? { group: who } : { user: who }; if (ev.type === "change") renderAccess(); }
    else if (f === "who") g.subject = g.subject.group !== undefined ? { group: e.value } : { user: e.value };
    else if (f === "deny") g.deny = e.checked;
    else g[f] = e.value;
  });
  all("[data-del-role]").forEach(b => b.onclick = () => { policy.roles.splice(+b.dataset.delRole, 1); renderAccess(); });
  all("[data-del-grant]").forEach(b => b.onclick = () => { policy.grants.splice(+b.dataset.delGrant, 1); renderAccess(); });
  document.getElementById("add-role").onclick = () => { policy.roles.push({ name: "", description: "", rights: ["read"] }); renderAccess(); focusLast("[data-f=name][data-role]"); };
  document.getElementById("add-grant").onclick = () => { policy.grants.push({ subject: { user: "" }, role: "reader", scope: "", deny: false }); renderAccess(); focusLast("[data-f=who]"); };
  document.getElementById("reload-access").onclick = () => adminAccess();
  document.getElementById("save-access").onclick = async () => {
    const p = { anonymous: policy.anonymous, roles: policy.roles.map(r => ({ name: r.name.trim(), description: (r.description || "").trim(), rights: r.rights || [] })),
      grants: policy.grants.map(g => Object.assign({}, g, { scope: (g.scope || "").trim() })) };
    try {
      const s = await api("/access", { method: "PUT", json: p });
      policy = Object.assign({ anonymous: "", roles: [], grants: [] }, s.policy);
      providers = s.providers || [];
      renderAccess();
      message("access-msg", t("Access settings saved"), true);
      refreshSession();
    } catch (e) { message("access-msg", e.status === 409 ? t("The access settings would leave nobody who manages users") : e.message); }
  };
}

function focusLast(sel) { const l = document.querySelectorAll(sel); if (l.length) l[l.length - 1].focus(); }

// Groups

async function adminGroups() {
  const [groups, users] = await Promise.all([api("/groups"), api("/users")]);
  groups.sort((a, b) => a.name.localeCompare(b.name));
  let html = '<div id="group-form"></div><p class="muted">' + esc(t("Groups gather users, local ones and those of a directory. Grants to a group give its members the rights of a role, on the whole server, a manufacturer or a catalog.")) + "</p>" +
    '<div class="toolbar"><strong>' + esc(t("Groups")) + '</strong><button class="primary" id="new-group">' + esc(t("New group")) + "</button></div>";
  html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + '</th><th class="hide-narrow">' + esc(t("Description")) + "</th><th>" + esc(t("Members")) + "</th><th></th></tr></thead><tbody>";
  groups.forEach((g, i) => {
    html += "<tr><td><strong>" + esc(g.name) + '</strong></td><td class="hide-narrow">' + esc(g.description || "") + "</td><td>" + esc((g.members || []).join(", ")) +
      ' <span class="muted">(' + (g.members || []).length + ')</span></td><td class="actions"><button class="small" data-edit="' + i + '">' + esc(t("Edit")) + '</button><button class="small danger" data-del="' + i + '">' + esc(t("Delete")) + "</button></td></tr>";
  });
  if (!groups.length) html += '<tr><td colspan="4" class="muted">' + esc(t("No groups")) + "</td></tr>";
  adminBody(html + '</tbody></table></div><p id="groups-msg"></p>' + datalist("group-users", users.map(u => u.name)));
  document.getElementById("new-group").onclick = () => groupForm(null, groups);
  document.querySelectorAll("[data-edit]").forEach(b => b.onclick = () => groupForm(groups[+b.dataset.edit], groups));
  document.querySelectorAll("[data-del]").forEach(b => b.onclick = async () => {
    const g = groups[+b.dataset.del];
    if (!confirm(tp("Delete group {}? The grants to it are removed.", g.name))) return;
    try { await api("/groups/" + encodeURIComponent(g.name), { method: "DELETE" }); await adminGroups(); refreshSession(); }
    catch (e) { message("groups-msg", e.status === 409 ? t("The groups would leave nobody who manages users") : e.message); }
  });
}

/// The form of a new (`g` null) or an existing group.
function groupForm(g, groups) {
  const field = (label, input, hint) => labelFor(label, input) + "<div>" + input + (hint ? '<div class="hint">' + esc(hint) + "</div>" : "") + "</div>";
  let html = '<form id="gf" class="box section"><h3>' + (g ? tf("Edit group {}", g.name) : esc(t("New group"))) + '</h3><div class="form">';
  html += field("Name", '<input id="gf-name" required autocomplete="off"' + (g ? ' value="' + esc(g.name) + '" disabled' : "") + ">");
  html += field("Description", '<input id="gf-desc" value="' + esc(g ? g.description || "" : "") + '">');
  html += field("Members", '<textarea id="gf-members" rows="3" placeholder="' + esc(t("separated by commas")) + '">' + esc(g ? (g.members || []).join(", ") : "") + "</textarea>" +
    '<div class="pair" style="margin-top:6px"><input id="gf-add" list="group-users" placeholder="' + esc(t("User name")) + '"><button type="button" id="gf-add-button">' + esc(t("Add")) + "</button></div>",
    t("Users of a directory too, by the name they sign in with"));
  html += '</div><div class="buttons"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="gf-cancel">' + esc(t("Cancel")) + '</button><span id="gf-msg"></span></div></form>';
  const box = document.getElementById("group-form");
  box.innerHTML = html;
  const el = id => document.getElementById(id);
  const members = () => el("gf-members").value.split(/[,\n]/).map(m => m.trim()).filter(Boolean);
  el("gf-add-button").onclick = () => {
    const name = el("gf-add").value.trim();
    if (name && !members().some(m => m.toLowerCase() === name.toLowerCase())) el("gf-members").value = members().concat([name]).join(", ");
    el("gf-add").value = "";
    el("gf-add").focus();
  };
  el("gf-cancel").onclick = () => { box.innerHTML = ""; };
  (g ? el("gf-desc") : el("gf-name")).focus();
  el("gf").onsubmit = async e => {
    e.preventDefault();
    const name = g ? g.name : el("gf-name").value.trim();
    if (!name || name.includes(":")) { message("gf-msg", t("A group name must not be empty or contain ':'")); return; }
    if (!g && groups.some(x => x.name.toLowerCase() === name.toLowerCase()) && !confirm(tp("Group {} exists already. Change it?", name))) return;
    try {
      await api("/groups/" + encodeURIComponent(name), { method: "PUT", json: { name, description: el("gf-desc").value.trim(), members: members() } });
      await adminGroups();
      message("groups-msg", tp("Saved group {}", name), true);
      refreshSession();
    } catch (err) { message("gf-msg", err.status === 409 ? t("The groups would leave nobody who manages users") : err.message); }
  };
}

// Directories (Active Directory, LDAP)

const DIRECTORY_KINDS = [["active_directory", "Active Directory"], ["ldap", "LDAP"]];

async function adminDirectories() {
  const list = await api("/directories");
  let html = '<div id="directory-form"></div><p class="muted">' + esc(t("Users sign in with the account of a directory (Active Directory, LDAP) when the server has no local account of their name. Grants refer to the groups of a directory as directory:group.")) + "</p>" +
    '<div class="toolbar"><strong>' + esc(t("Directories")) + '</strong><button class="primary" id="new-directory">' + esc(t("New directory")) + "</button></div>";
  html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + "</th><th>" + esc(t("Type")) + '</th><th class="hide-narrow">' + esc(t("Address")) + '</th><th class="hide-narrow">' + esc(t("Search base")) + "</th><th></th></tr></thead><tbody>";
  list.forEach((d, i) => {
    html += "<tr><td><strong>" + esc(d.title || d.id) + '</strong> <span class="muted">' + esc(d.id) + "</span></td><td>" + esc((DIRECTORY_KINDS.find(k => k[0] === d.kind) || [d.kind, d.kind])[1]) +
      '</td><td class="hide-narrow">' + esc(d.url) + '</td><td class="hide-narrow">' + esc(d.base_dn) + '</td><td class="actions"><button class="small" data-edit="' + i + '">' + esc(t("Edit")) +
      '</button><button class="small danger" data-del="' + i + '">' + esc(t("Delete")) + "</button></td></tr>";
  });
  if (!list.length) html += '<tr><td colspan="5" class="muted">' + esc(t("Only local accounts")) + "</td></tr>";
  adminBody(html + '</tbody></table></div><p id="directories-msg"></p>');
  document.getElementById("new-directory").onclick = () => directoryForm(null, list);
  document.querySelectorAll("[data-edit]").forEach(b => b.onclick = () => directoryForm(list[+b.dataset.edit], list));
  document.querySelectorAll("[data-del]").forEach(b => b.onclick = async () => {
    const d = list[+b.dataset.del];
    if (!confirm(tp("Delete directory {}? Its users can no longer sign in.", d.title || d.id))) return;
    try { await api("/directories/" + encodeURIComponent(d.id), { method: "DELETE" }); await adminDirectories(); }
    catch (e) { message("directories-msg", e.message); }
  });
}

/// The settings of a new (`d` null) or an existing directory, with a test of the
/// connection and of a sign-in.
function directoryForm(d, list) {
  const v = k => esc(d ? d[k] ?? "" : "");
  const field = (label, input, hint) => labelFor(label, input) + "<div>" + input + (hint ? '<div class="hint">' + esc(hint) + "</div>" : "") + "</div>";
  const check = (id, on, label) => '<label class="check"><input type="checkbox" id="' + id + '"' + (on ? " checked" : "") + "> " + esc(t(label)) + "</label>";
  let html = '<form id="df" class="box section"><h3>' + (d ? tf("Edit directory {}", d.title || d.id) : esc(t("New directory"))) + '</h3><div class="form">';
  html += field("Identifier", '<input id="df-id" required autocomplete="off" value="' + v("id") + '"' + (d ? " disabled" : "") + ">", t("Short name without blanks; qualifies the groups (id:group)"));
  html += field("Name", '<input id="df-title" value="' + v("title") + '">');
  html += field("Type", '<select id="df-kind">' + DIRECTORY_KINDS.map(([k, n]) => '<option value="' + k + '"' + ((d ? d.kind : "active_directory") === k ? " selected" : "") + ">" + n + "</option>").join("") + "</select>");
  html += field("Address", '<input id="df-url" required placeholder="ldaps://dc1.corp.example" value="' + v("url") + '">', t("ldap:// or ldaps://, with the port if it is not the usual one"));
  html += labelFor("Encryption", '<span id="df-tls"></span>') + '<div id="df-tls">' + check("df-starttls", d && d.starttls, "StartTLS (for ldap://)") + "<br>" +
    check("df-insecure", d && d.insecure_tls, "Accept any certificate (test installations only)") + "</div>";
  html += field("Service account", '<input id="df-bind-dn" autocomplete="off" placeholder="CN=svc-openecraft,OU=Service,DC=corp,DC=example" value="' + v("bind_dn") + '">', t("Looks the users up; empty: users bind with their own name (below)"));
  html += field("Password", '<input id="df-bind-pw" type="password" autocomplete="new-password" placeholder="' + esc(d && d.password_set ? t("unchanged if empty") : "") + '">',
    t("Better kept in an environment variable of the server"));
  html += field("Password variable", '<input id="df-bind-env" autocomplete="off" placeholder="OPENECRAFT_LDAP_PASSWORD" value="' + v("bind_password_env") + '">', t("Environment variable with the password of the service account"));
  html += field("User bind", '<input id="df-user-bind" autocomplete="off" placeholder="{user}@corp.example" value="' + v("user_bind") + '">', t("Bind name of a user without service account; {user} is replaced"));
  html += field("Search base", '<input id="df-base" required placeholder="DC=corp,DC=example" value="' + v("base_dn") + '">');
  html += field("User filter", '<input id="df-filter" autocomplete="off" placeholder="(&(objectClass=user)(sAMAccountName={user}))" value="' + v("user_filter") + '">', t("Empty: the usual filter of the type"));
  html += field("Name attribute", '<input id="df-name-attr" autocomplete="off" placeholder="sAMAccountName / uid" value="' + v("name_attribute") + '">');
  html += labelFor("Groups", '<span id="df-nested"></span>') + '<div id="df-nested">' + check("df-nested-groups", d && d.nested_groups, "Also groups of groups (Active Directory)") + "</div>";
  html += field("Group filter", '<input id="df-group-filter" autocomplete="off" placeholder="(&(objectClass=groupOfNames)(member={dn}))" value="' + v("group_filter") + '">', t("Empty: the memberOf attribute of the user"));
  html += field("Group search base", '<input id="df-group-base" autocomplete="off" value="' + v("group_base_dn") + '">', t("Empty: the search base"));
  html += field("Allowed groups", '<input id="df-allowed" autocomplete="off" value="' + esc(d ? (d.allowed_groups || []).join(", ") : "") + '" placeholder="' + esc(t("separated by commas")) + '">', t("Only their members sign in; empty: all users"));
  html += field("Timeout (s)", '<input id="df-timeout" type="number" min="1" max="120" value="' + esc(d ? d.timeout_secs || 10 : 10) + '">');
  html += labelFor("Test", '<input id="df-test-user">') + '<div><div class="directory-test"><input id="df-test-user" autocomplete="off" placeholder="' + esc(t("User name")) + '"><input id="df-test-pw" type="password" autocomplete="off" placeholder="' + esc(t("Password")) + '">' +
    '<button type="button" id="df-test">' + esc(t("Test")) + '</button></div><div class="hint">' + esc(t("Without a user: connects and binds with the service account. With a user: signs them in and shows their groups.")) + '</div><div id="df-test-msg"></div></div>';
  html += '</div><div class="buttons"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="df-cancel">' + esc(t("Cancel")) + '</button><span id="df-msg"></span></div></form>';
  const box = document.getElementById("directory-form");
  box.innerHTML = html;
  box.scrollIntoView({ block: "nearest" });
  const el = id => document.getElementById(id);
  (d ? el("df-title") : el("df-id")).focus();
  el("df-cancel").onclick = () => { box.innerHTML = ""; };
  const settings = () => ({
    id: d ? d.id : el("df-id").value.trim(), title: el("df-title").value.trim(), kind: el("df-kind").value, url: el("df-url").value.trim(),
    starttls: el("df-starttls").checked, insecure_tls: el("df-insecure").checked, bind_dn: el("df-bind-dn").value.trim(), bind_password: el("df-bind-pw").value,
    bind_password_env: el("df-bind-env").value.trim(), user_bind: el("df-user-bind").value.trim(), base_dn: el("df-base").value.trim(), user_filter: el("df-filter").value.trim(),
    name_attribute: el("df-name-attr").value.trim(), nested_groups: el("df-nested-groups").checked, group_filter: el("df-group-filter").value.trim(),
    group_base_dn: el("df-group-base").value.trim(), allowed_groups: el("df-allowed").value.split(",").map(g => g.trim()).filter(Boolean),
    timeout_secs: Math.max(1, parseInt(el("df-timeout").value, 10) || 10),
  });
  const valid = s => {
    if (!s.id || /[\s:]/.test(s.id)) return t("A directory id must not be empty or contain blanks or ':'");
    if (!/^ldaps?:\/\//.test(s.url)) return t("The address must start with ldap:// or ldaps://");
    return "";
  };
  el("df-test").onclick = async () => {
    const s = settings();
    const problem = valid(s);
    const out = el("df-test-msg");
    if (problem) { out.className = "error"; out.textContent = problem; return; }
    el("df-test").disabled = true;
    out.className = "muted";
    out.textContent = t("Loading…");
    try {
      const r = await api("/directories/" + encodeURIComponent(s.id) + "/test", { method: "POST", json: { directory: s, name: el("df-test-user").value.trim(), password: el("df-test-pw").value } });
      out.className = r.ok ? "ok" : "error";
      out.textContent = (r.ok ? t("The test succeeded") : t("The test failed")) + ": " + r.message + (r.identity && (r.identity.groups || []).length ? " · " + r.identity.groups.join(", ") : "");
    } catch (e) { out.className = "error"; out.textContent = e.message; }
    el("df-test").disabled = false;
  };
  el("df").onsubmit = async e => {
    e.preventDefault();
    const s = settings();
    const problem = valid(s);
    if (problem) { message("df-msg", problem); return; }
    if (!d && list.some(x => x.id.toLowerCase() === s.id.toLowerCase()) && !confirm(tp("Directory {} exists already. Change it?", s.id))) return;
    try {
      await api("/directories/" + encodeURIComponent(s.id), { method: "PUT", json: s });
      await adminDirectories();
      message("directories-msg", tp("Saved directory {}", s.title || s.id), true);
    } catch (err) { message("df-msg", err.message); }
  };
}

// Catalogs

async function adminCatalogs() {
  const admin = may("administer", "*");
  const [list, bundled, makers] = await Promise.all([api("/catalogs"), admin ? api("/bundled").catch(() => []) : [], manufacturers()]);
  list.forEach(rememberCatalog);
  let html = '<div id="catalog-form"></div><div class="toolbar"><strong>' + esc(t("Catalogs")) + "</strong>" +
    (mayAnywhere("edit_catalogs") ? '<button class="primary" id="new-catalog">' + esc(t("New catalog")) + "</button>" : "") + "</div>";
  html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + "</th><th>" + esc(t("Manufacturer")) + "</th><th>" + esc(t("Version")) + '</th><th class="num">' + esc(t("Parts")) +
    '</th><th class="hide-narrow">' + esc(t("Last change")) + "</th><th></th></tr></thead><tbody>";
  list.forEach((c, i) => {
    const mine = mayOnPart("edit_catalogs", { manufacturer: c.manufacturer, catalog: c.id });
    html += '<tr><td><span class="with-logo">' + logoImg(c, "logo-small") + "<span><strong>" + esc(c.name) + '</strong> <span class="muted">' + esc(c.id) + "</span></span></span></td><td>" + esc(c.manufacturer) + "</td><td>" + esc(c.version) + '</td><td class="num">' + c.parts +
      '</td><td class="hide-narrow">' + esc(date(c.modified)) + '</td><td class="actions"><button class="small" data-show="' + i + '">' + esc(t("Show parts")) + "</button>" +
      (admin ? '<button class="small" data-grants="' + i + '">' + esc(t("Permissions")) + "</button>" : "") +
      (mine ? '<button class="small" data-edit="' + i + '">' + esc(t("Edit")) + "</button>" : "") +
      (mine ? '<button class="small danger" data-del="' + i + '"' + (c.parts ? ' disabled title="' + esc(t("Only empty catalogs can be deleted")) + '"' : "") + ">" + esc(t("Delete")) + "</button>" : "") + "</td></tr>";
  });
  if (!list.length) html += '<tr><td colspan="6" class="muted">–</td></tr>';
  html += '</tbody></table></div><p id="catalogs-msg"></p>';
  if (bundled.length) {
    html += '<h3 class="section">' + esc(t("Bundled catalogs")) + '</h3><p class="muted">' + esc(t("Catalogs shipped with the server. Installing adds their parts and updates those nobody changed on this server.")) + "</p>";
    html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + "</th><th>" + esc(t("Manufacturer")) + "</th><th>" + esc(t("Version")) + '</th><th class="num">' + esc(t("Parts")) +
      "</th><th>" + esc(t("On this server")) + "</th><th></th></tr></thead><tbody>";
    bundled.forEach((b, i) => {
      const current = b.installed === b.version;
      html += "<tr><td><strong>" + esc(b.name) + "</strong></td><td>" + esc(b.manufacturer) + "</td><td>" + esc(b.version) + '</td><td class="num">' + b.parts + "</td><td>" +
        (b.installed ? esc(b.installed) : '<span class="muted">–</span>') + '</td><td class="actions"><button class="small' + (current ? "" : " primary") + '" data-install="' + i + '">' +
        esc(t(!b.installed ? "Install" : current ? "Install again" : "Update")) + "</button></td></tr>";
    });
    html += '</tbody></table></div><p id="bundled-msg"></p>';
  }
  adminBody(html + datalist("makers", makers));
  const all = sel => document.querySelectorAll(sel);
  all("[data-show]").forEach(b => b.onclick = () => { query = blank(); query.catalog = list[+b.dataset.show].id; view = "parts"; go(); });
  all("[data-edit]").forEach(b => b.onclick = () => catalogForm(list[+b.dataset.edit]));
  all("[data-grants]").forEach(b => b.onclick = () => catalogGrants(list[+b.dataset.grants]));
  all("[data-del]").forEach(b => b.onclick = async () => {
    const c = list[+b.dataset.del];
    if (!confirm(tp("Delete catalog {}?", c.name))) return;
    try { await api("/catalogs/" + encodeURIComponent(c.id), { method: "DELETE" }); delete catalogNames[c.id.toLowerCase()]; await adminCatalogs(); footer(); }
    catch (e) { message("catalogs-msg", e.message); }
  });
  all("[data-install]").forEach(b => b.onclick = async () => {
    const c = bundled[+b.dataset.install];
    all("[data-install]").forEach(x => x.disabled = true);
    try {
      const r = await api("/bundled/" + encodeURIComponent(c.id), { method: "POST" });
      await adminCatalogs();
      message("bundled-msg", tp("{}: {} parts added, {} updated, {} kept", c.name, r.added, r.updated, r.kept.length), true);
      footer();
    } catch (e) { all("[data-install]").forEach(x => x.disabled = false); message("bundled-msg", e.message); }
  });
  const nb = document.getElementById("new-catalog");
  if (nb) nb.onclick = () => catalogForm(null);
}

/// Who may do what with one catalog and its parts: grants on `catalog:<id>`.
async function catalogGrants(c) {
  const box = document.getElementById("catalog-form");
  let grants, users, s;
  try { [grants, users, s] = await Promise.all([api("/catalogs/" + encodeURIComponent(c.id) + "/grants"), api("/users"), api("/access")]); }
  catch (e) { message("catalogs-msg", e.message); return; }
  policy = Object.assign({ anonymous: "", roles: [], grants: [] }, s.policy);
  const groups = (s.groups || []).map(g => g.name).concat(users.flatMap(u => u.groups || []));
  const render = () => {
    const names = roleNames();
    let html = '<div class="box section"><h3>' + tf("Permissions of catalog {}", c.name) + '</h3><p class="muted">' +
      esc(t("The roles given here apply to this catalog and its parts, whatever their manufacturer; deny takes the rights of the role away. Reading is a right of the whole server.")) + "</p>";
    html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Type")) + "</th><th>" + esc(t("User or group")) + "</th><th>" + esc(t("Role")) + '</th><th class="check">' + esc(t("Deny")) + "</th><th></th></tr></thead><tbody>";
    grants.forEach((g, i) => {
      const group = g.subject.group !== undefined;
      const roles = names.includes(g.role) ? names : names.concat([g.role]);
      html += '<tr><td><select data-cg="' + i + '" data-f="kind"><option value="user">' + esc(t("User")) + '</option><option value="group"' + (group ? " selected" : "") + ">" + esc(t("Group")) + "</option></select></td>" +
        '<td><input data-cg="' + i + '" data-f="who" list="cg-' + (group ? "groups" : "users") + '" value="' + esc(group ? g.subject.group : g.subject.user || "") + '"></td>' +
        '<td><select data-cg="' + i + '" data-f="role">' + options(roles, g.role, roleTitle) + "</select></td>" +
        '<td class="check"><input type="checkbox" data-cg="' + i + '" data-f="deny"' + (g.deny ? " checked" : "") + ' aria-label="' + esc(t("Deny")) + '"></td>' +
        '<td><button type="button" class="small danger" data-cg-del="' + i + '" title="' + esc(t("Remove")) + '">✕</button></td></tr>';
    });
    if (!grants.length) html += '<tr><td colspan="5" class="muted">' + esc(t("No grants: the rights by manufacturer apply")) + "</td></tr>";
    html += '</tbody></table></div><div class="buttons"><button type="button" id="cg-add">' + esc(t("Add grant")) + '</button><button type="button" class="primary" id="cg-save">' + esc(t("Save")) + '</button><button type="button" id="cg-cancel">' + esc(t("Cancel")) + '</button><span id="cg-msg"></span></div></div>' +
      datalist("cg-users", users.map(u => u.name)) + datalist("cg-groups", groups);
    box.innerHTML = html;
    box.scrollIntoView({ block: "nearest" });
    box.querySelectorAll("[data-cg]").forEach(e => e.onchange = e.oninput = ev => {
      const g = grants[+e.dataset.cg];
      const f = e.dataset.f;
      if (f === "kind") { const who = g.subject.user ?? g.subject.group ?? ""; g.subject = e.value === "group" ? { group: who } : { user: who }; if (ev.type === "change") render(); }
      else if (f === "who") g.subject = g.subject.group !== undefined ? { group: e.value } : { user: e.value };
      else if (f === "deny") g.deny = e.checked;
      else g[f] = e.value;
    });
    box.querySelectorAll("[data-cg-del]").forEach(b => b.onclick = () => { grants.splice(+b.dataset.cgDel, 1); render(); });
    document.getElementById("cg-add").onclick = () => { grants.push({ subject: { group: "" }, role: "editor", scope: catalogScope(c.id), deny: false }); render(); focusLast("[data-cg][data-f=who]"); };
    document.getElementById("cg-cancel").onclick = () => { box.innerHTML = ""; };
    document.getElementById("cg-save").onclick = async () => {
      const list = grants.map(g => Object.assign({}, g, { scope: catalogScope(c.id) }));
      if (list.some(g => !(g.subject.user ?? g.subject.group ?? "").trim())) { message("cg-msg", t("A grant needs a user or a group")); return; }
      try {
        grants = await api("/catalogs/" + encodeURIComponent(c.id) + "/grants", { method: "PUT", json: list });
        render();
        message("cg-msg", t("Access settings saved"), true);
        refreshSession();
      } catch (e) { message("cg-msg", e.message); }
    };
  };
  render();
}

function catalogForm(c) {
  const field = (label, input) => labelFor(label, input) + "<div>" + input + "</div>";
  const v = k => esc(c ? c[k] || "" : "");
  let html = '<form id="cf" class="box section"><h3>' + (c ? tf("Edit catalog {}", c.name) : esc(t("New catalog"))) + '</h3><div class="form">';
  html += field("Identifier", '<input id="cf-id" required autocomplete="off" value="' + v("id") + '"' + (c ? " disabled" : "") + ">");
  html += field("Name", '<input id="cf-name" value="' + v("name") + '">');
  html += field("Manufacturer", '<input id="cf-maker" list="makers" value="' + v("manufacturer") + '">');
  html += field("Version", '<input id="cf-version" value="' + v("version") + '">');
  html += field("Description", '<textarea id="cf-desc" rows="3">' + v("description") + "</textarea>");
  html += labelFor("Logo", '<input id="cf-logo">') + '<div><div class="with-logo"><span id="cf-logo-preview">' + (c ? logoImg(c, "logo-medium") : "") + '</span><input id="cf-logo" type="file" accept="' + LOGO_TYPES + '"></div>' +
    (c && c.logo ? '<label class="check"><input type="checkbox" id="cf-logo-remove"> ' + esc(t("Remove the logo")) + "</label>" : "") +
    '<div class="hint">' + esc(t("The logo of the company: PNG, JPEG, GIF, WebP or SVG, at most 512 kB")) + "</div></div>";
  html += '</div><div class="buttons"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="cf-cancel">' + esc(t("Cancel")) + '</button><span id="cf-msg"></span></div></form>';
  const box = document.getElementById("catalog-form");
  box.innerHTML = html;
  const el = id => document.getElementById(id);
  el("cf-cancel").onclick = () => { box.innerHTML = ""; };
  (c ? el("cf-name") : el("cf-id")).focus();
  // Shows the chosen file before it is saved.
  el("cf-logo").onchange = () => {
    const file = el("cf-logo").files[0];
    el("cf-logo-preview").innerHTML = "";
    if (!file) return;
    const img = document.createElement("img");
    img.className = "logo-medium";
    img.alt = "";
    img.src = URL.createObjectURL(file);
    el("cf-logo-preview").appendChild(img);
    if (el("cf-logo-remove")) el("cf-logo-remove").checked = false;
  };
  el("cf").onsubmit = async e => {
    e.preventDefault();
    const id = c ? c.id : el("cf-id").value.trim();
    if (!id || /\s/.test(id)) { message("cf-msg", t("A catalog id must not be empty or contain blanks")); return; }
    const catalog = { id, name: el("cf-name").value.trim(), manufacturer: el("cf-maker").value.trim(), version: el("cf-version").value.trim(), description: el("cf-desc").value.trim() };
    const logo = el("cf-logo").files[0];
    if (logo && (logo.size > 512 * 1024 || !/\.(png|jpe?g|gif|webp|svg)$/i.test(logo.name))) { message("cf-msg", t("The logo of the company: PNG, JPEG, GIF, WebP or SVG, at most 512 kB")); return; }
    if (!c && catalogNames[id.toLowerCase()] && !confirm(tp("Catalog {} exists already. Change it?", id))) return;
    try {
      const saved = await api("/catalogs/" + encodeURIComponent(id), { method: "PUT", json: catalog });
      const path = "/catalogs/" + encodeURIComponent(saved.id) + "/logo";
      if (logo) await api(path + "?name=" + encodeURIComponent(logo.name), { method: "PUT", body: logo, headers: { "Content-Type": logo.type || "application/octet-stream" } });
      else if (el("cf-logo-remove") && el("cf-logo-remove").checked) await api(path, { method: "DELETE" });
      await adminCatalogs();
      footer();
      message("catalogs-msg", tp("Saved catalog {}", saved.name), true);
    } catch (err) { message("cf-msg", err.message); }
  };
}

// Import

function adminImport() {
  let html = '<form id="import" class="box"><h3>' + esc(t("Import parts")) + '</h3><p class="muted">' + esc(t("CSV (with the column names of the export), JSON, BMEcat or EPLAN XML. Parts with the same number are replaced; their files are kept.")) + "</p>";
  html += '<div class="buttons"><input id="import-file" type="file" accept=".csv,.txt,.json,.xml,.bmecat" required><button class="primary">' + esc(t("Import")) + '</button></div><p id="import-msg"></p></form>';
  html += '<p><a href="' + API + '/export.csv">' + esc(t("Export (CSV)")) + "</a></p>";
  adminBody(html);
  document.getElementById("import").onsubmit = async e => {
    e.preventDefault();
    const file = document.getElementById("import-file").files[0];
    if (!file) return;
    const button = e.target.querySelector("button");
    button.disabled = true;
    message("import-msg", t("Loading…"), true);
    try {
      const r = await api("/import", { method: "POST", body: await file.text(), headers: { "Content-Type": "text/plain; charset=utf-8" } });
      message("import-msg", tp("Imported {} parts (revision {})", r.imported, r.revision), true);
      footer();
    } catch (err) { message("import-msg", err.message); }
    button.disabled = false;
  };
}

// Export and backup

function today() { return new Date().toISOString().slice(0, 10); }
function saveFile(blob, name) {
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(a.href), 10000);
}
function jsonFile(value) { return new Blob([JSON.stringify(value, null, 2) + "\n"], { type: "application/json" }); }
/// Who writes an export (`openecraft_parts::api::ExportInfo`): the server, its version
/// and the revision, so that later versions can convert the file.
async function exportInfo() {
  const info = await api("/info");
  return { format: "openecraft-parts", format_version: 1, server: info.name, server_version: info.version, revision: info.revision, exported: Math.floor(Date.now() / 1000) };
}
/// CSV as the parts export writes it (semicolons, quoted where needed), after the line
/// with the export information.
function csvFile(rows, info) {
  const cell = v => { const s = String(v ?? ""); return /[;"\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s; };
  const head = info ? "# " + Object.entries(info).map(([k, v]) => k + "=" + String(v).replace(/[;\r\n]/g, " ")).join(";") + "\r\n" : "";
  return new Blob([head + rows.map(r => r.map(cell).join(";")).join("\r\n") + "\r\n"], { type: "text/csv" });
}

function adminBackup() {
  const admin = may("administer", "*");
  const action = (id, label) => '<button type="button" id="' + id + '">' + esc(label) + "</button>";
  const card = (title, text, buttons, extra) => '<div class="box"><h3>' + esc(title) + '</h3><p class="muted">' + esc(text) + '</p><div class="buttons">' + buttons + "</div>" + (extra || "") + "</div>";
  let html = '<p class="muted">' + esc(t("Export the parts, catalogs and settings, or download a backup of the whole database.")) + '</p><div class="cards">';
  html += card(t("Parts"), t("All parts with their data, for spreadsheets or to import them into another server. Their files are only in the database backup."),
    action("x-parts-csv", "CSV") + action("x-parts-json", "JSON"));
  html += card(t("Catalogs"), t("The catalogs with their manufacturer, version and description."), action("x-catalogs", "JSON"));
  if (admin) {
    html += card(t("Users"), t("The users with their role, manufacturer, groups and last sign-in, without passwords."), action("x-users", "CSV"));
    html += card(t("Access rights"), t("The roles, the grants, the groups and the access without sign-in, with the version of the server. Restoring replaces the roles and grants and adds the groups; the directories stay as set up on the server."),
      action("x-access", "JSON") + action("x-access-restore", t("Restore from a file")) + '<input type="file" id="x-access-file" accept=".json,application/json" hidden>');
    html += card(t("Database backup"), t("One SQLite file with all parts, their history and files, the catalogs, the users and their favorites. It holds the password hashes of the users: keep it safe."),
      '<button type="button" class="primary" id="x-backup">' + esc(t("Download")) + "</button>",
      '<p class="hint muted">' + esc(t("To restore it, start the server with --data <file>, or copy it into a new database with --migrate-from <file>.")) + "</p>");
  }
  adminBody(html + '</div><p id="backup-msg"></p>');
  const run = (id, work) => {
    const b = document.getElementById(id);
    if (!b) return;
    b.onclick = async () => {
      b.disabled = true;
      message("backup-msg", t("Loading…"), true);
      try { await work(); message("backup-msg", ""); }
      catch (e) { message("backup-msg", e.message); }
      b.disabled = false;
    };
  };
  run("x-parts-csv", async () => saveFile(await api("/export.csv", { raw: true }), "parts-" + today() + ".csv"));
  run("x-parts-json", async () => saveFile(await api("/export.json", { raw: true }), "parts-" + today() + ".json"));
  run("x-catalogs", async () => saveFile(jsonFile({ export: await exportInfo(), catalogs: await api("/catalogs") }), "catalogs-" + today() + ".json"));
  run("x-users", async () => {
    const users = await api("/users");
    const rows = [["name", "role", "manufacturer", "directory", "groups", "sessions", "signed_in"]].concat(users.map(u =>
      [u.name, u.role, u.manufacturer, u.provider || "", (u.groups || []).join(", "), u.sessions || 0, u.signed_in ? new Date(u.signed_in * 1000).toISOString() : ""]));
    saveFile(csvFile(rows, await exportInfo()), "users-" + today() + ".csv");
  });
  run("x-access", async () => { const s = await api("/access"); saveFile(jsonFile({ export: await exportInfo(), policy: s.policy, groups: s.groups || [] }), "access-" + today() + ".json"); });
  run("x-backup", async () => saveFile(await api("/backup", { raw: true }), "parts-db-backup-" + today() + ".sqlite"));
  const restore = document.getElementById("x-access-file");
  if (restore) document.getElementById("x-access-restore").onclick = () => restore.click();
  if (restore) restore.onchange = async () => {
    const file = restore.files[0];
    restore.value = "";
    if (!file) return;
    try {
      const data = JSON.parse(await file.text());
      // The policy itself, or access settings as the server sends them.
      const p = data && (data.policy || data);
      if (!p || typeof p !== "object" || !Array.isArray(p.roles || []) || !Array.isArray(p.grants || [])) throw new Error(t("The file holds no access settings"));
      if (!confirm(t("Replace the roles and grants with those of the file?"))) return;
      await api("/access", { method: "PUT", json: { anonymous: p.anonymous || "", roles: p.roles || [], grants: p.grants || [] } });
      // Files of this version also hold the groups; they are added or changed.
      for (const g of Array.isArray(data.groups) ? data.groups : []) {
        if (g && g.name) await api("/groups/" + encodeURIComponent(g.name), { method: "PUT", json: { name: g.name, description: g.description || "", members: g.members || [] } });
      }
      message("backup-msg", t("Access settings saved"), true);
      refreshSession();
    } catch (e) {
      message("backup-msg", e instanceof SyntaxError ? t("The file holds no access settings") : e.status === 409 ? t("The access settings would leave nobody who manages users") : e.message);
    }
  };
}
