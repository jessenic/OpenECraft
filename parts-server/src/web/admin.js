"use strict";
/// Administration of the server (overview, users, access rights, catalogs, import),
/// shown to signed-in users with the rights for it. The page only offers what the
/// caller may do; the server checks every change. Loaded before portal.js, whose
/// helpers (t, tf, esc, api, rows, box, …) it uses.
const ADMIN_TABS = [
  ["overview", "Overview", () => true],
  ["users", "Users", () => may("administer", "*")],
  ["access", "Access rights", () => may("administer", "*")],
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
/// The caller's rights on a scope (a manufacturer or `*`), as `Access::rights` resolves them.
function rightsOn(scope) {
  const covering = entries().filter(e => { const p = (e.scope || "").trim() || "*"; return scope === "*" ? p === "*" : wild(p, scope); });
  const denied = new Set(covering.filter(e => e.deny).flatMap(e => e.rights || []));
  return new Set(covering.filter(e => !e.deny).flatMap(e => e.rights || []).filter(r => !denied.has(r) && (scope === "*" || !serverWide(r))));
}
function may(right, scope) { return rightsOn(serverWide(right) ? "*" : (scope || "").trim()).has(right); }
function mayAnywhere(right) { return entries().some(e => !e.deny && (e.rights || []).includes(right)); }
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
  const views = { overview: adminOverview, users: adminUsers, access: adminAccess, catalogs: adminCatalogs, import: adminImport, backup: adminBackup };
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
  const [users, settings, registration] = admin ? await Promise.all([api("/users"), api("/access"), api("/registration")]) : [[], null, null];
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
      ["Defined roles", String((p.roles || []).length)], ["Grants", String((p.grants || []).length)],
      ["Directories", esc((settings.providers || []).map(d => d.title || d.id).join(", ") || t("Only local accounts"))]]));
  }
  // Where the caller has each right: server-wide rights apply or not, the others per manufacturer.
  const mine = (T.rights || []).map(r => {
    const where = r.server_wide ? (may(r.name, "*") ? t("Yes") : "") : rightsOn("*").has(r.name) ? t("All manufacturers") :
      [...new Set(entries().filter(e => !e.deny && (e.rights || []).includes(r.name)).map(e => e.scope))].filter(s => may(r.name, s)).join(", ");
    return [r.title, esc(where)];
  });
  html += box(tp("Signed in as {}", session.user.name), rows(mine));
  adminBody(html + "</div>");
}

// Users

async function adminUsers() {
  const [users, makers] = await Promise.all([api("/users"), manufacturers()]);
  users.sort((a, b) => (b.pending ? 1 : 0) - (a.pending ? 1 : 0) || a.name.localeCompare(b.name));
  const groups = users.flatMap(u => u.groups || []);
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
    html += field("Directory", "<span>" + esc(u.provider) + "</span>", t("The directory checks the password and sets the groups at every sign-in"));
    html += field("Groups", "<span>" + esc((u.groups || []).join(", ")) + "</span>");
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
      update.groups = el("uf-groups").value.split(",").map(g => g.trim()).filter(Boolean);
    }
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
  const [s, users, makers, registration] = await Promise.all([api("/access"), api("/users"), manufacturers(), api("/registration")]);
  policy = Object.assign({ anonymous: "", roles: [], grants: [] }, s.policy);
  providers = s.providers || [];
  adminBody('<div id="registration-box"></div><div id="access"></div>' + datalist("subjects-user", users.map(u => u.name)) + datalist("subjects-group", users.flatMap(u => u.groups || [])) + datalist("scopes", ["*"].concat(makers)));
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
  let html = '<p class="muted">' + esc(t("A role is a set of rights. Grants give a user or a group a role on the parts of a manufacturer (* and ? as wildcards, * for all); a deny grant takes the rights of the role away, whatever other grants give.")) + "</p>";
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

  html += '<h3 class="section">' + esc(t("Directories")) + "</h3>";
  html += providers.length ? '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + "</th><th>" + esc(t("Type")) + "</th></tr></thead><tbody>" +
    providers.map(d => "<tr><td>" + esc(d.title || d.id) + ' <span class="muted">' + esc(d.id) + "</span></td><td>" + esc(d.kind === "active_directory" ? "Active Directory" : d.kind === "ldap" ? "LDAP" : d.kind) + "</td></tr>").join("") + "</tbody></table></div>"
    : '<p class="muted">' + esc(t("Only local accounts")) + "</p>";
  html += '<p class="muted">' + esc(t("Directories (Active Directory, LDAP) are set up in the access settings file of the server.")) + "</p>";
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

// Catalogs

async function adminCatalogs() {
  const admin = may("administer", "*");
  const [list, bundled, makers] = await Promise.all([api("/catalogs"), admin ? api("/bundled").catch(() => []) : [], manufacturers()]);
  list.forEach(c => { catalogNames[c.id.toLowerCase()] = c.name; catalogManufacturers[c.id.toLowerCase()] = c.manufacturer; });
  let html = '<div id="catalog-form"></div><div class="toolbar"><strong>' + esc(t("Catalogs")) + "</strong>" +
    (mayAnywhere("edit_catalogs") ? '<button class="primary" id="new-catalog">' + esc(t("New catalog")) + "</button>" : "") + "</div>";
  html += '<div class="table-wrap"><table class="plain"><thead><tr><th>' + esc(t("Name")) + "</th><th>" + esc(t("Manufacturer")) + "</th><th>" + esc(t("Version")) + '</th><th class="num">' + esc(t("Parts")) +
    '</th><th class="hide-narrow">' + esc(t("Last change")) + "</th><th></th></tr></thead><tbody>";
  list.forEach((c, i) => {
    const mine = may("edit_catalogs", c.manufacturer);
    html += "<tr><td><strong>" + esc(c.name) + '</strong> <span class="muted">' + esc(c.id) + "</span></td><td>" + esc(c.manufacturer) + "</td><td>" + esc(c.version) + '</td><td class="num">' + c.parts +
      '</td><td class="hide-narrow">' + esc(date(c.modified)) + '</td><td class="actions"><button class="small" data-show="' + i + '">' + esc(t("Show parts")) + "</button>" +
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

function catalogForm(c) {
  const field = (label, input) => labelFor(label, input) + "<div>" + input + "</div>";
  const v = k => esc(c ? c[k] || "" : "");
  let html = '<form id="cf" class="box section"><h3>' + (c ? tf("Edit catalog {}", c.name) : esc(t("New catalog"))) + '</h3><div class="form">';
  html += field("Identifier", '<input id="cf-id" required autocomplete="off" value="' + v("id") + '"' + (c ? " disabled" : "") + ">");
  html += field("Name", '<input id="cf-name" value="' + v("name") + '">');
  html += field("Manufacturer", '<input id="cf-maker" list="makers" value="' + v("manufacturer") + '">');
  html += field("Version", '<input id="cf-version" value="' + v("version") + '">');
  html += field("Description", '<textarea id="cf-desc" rows="3">' + v("description") + "</textarea>");
  html += '</div><div class="buttons"><button class="primary">' + esc(t("Save")) + '</button><button type="button" id="cf-cancel">' + esc(t("Cancel")) + '</button><span id="cf-msg"></span></div></form>';
  const box = document.getElementById("catalog-form");
  box.innerHTML = html;
  const el = id => document.getElementById(id);
  el("cf-cancel").onclick = () => { box.innerHTML = ""; };
  (c ? el("cf-name") : el("cf-id")).focus();
  el("cf").onsubmit = async e => {
    e.preventDefault();
    const id = c ? c.id : el("cf-id").value.trim();
    if (!id || /\s/.test(id)) { message("cf-msg", t("A catalog id must not be empty or contain blanks")); return; }
    const catalog = { id, name: el("cf-name").value.trim(), manufacturer: el("cf-maker").value.trim(), version: el("cf-version").value.trim(), description: el("cf-desc").value.trim() };
    if (!c && catalogNames[id.toLowerCase()] && !confirm(tp("Catalog {} exists already. Change it?", id))) return;
    try {
      const saved = await api("/catalogs/" + encodeURIComponent(id), { method: "PUT", json: catalog });
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
/// CSV as the parts export writes it (semicolons, quoted where needed).
function csvFile(rows) {
  const cell = v => { const s = String(v ?? ""); return /[;"\r\n]/.test(s) ? '"' + s.replace(/"/g, '""') + '"' : s; };
  return new Blob([rows.map(r => r.map(cell).join(";")).join("\r\n") + "\r\n"], { type: "text/csv" });
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
    html += card(t("Access rights"), t("The roles, the grants and the access without sign-in. Restoring replaces them; the directories stay as set up on the server."),
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
  run("x-catalogs", async () => saveFile(jsonFile(await api("/catalogs")), "catalogs-" + today() + ".json"));
  run("x-users", async () => {
    const users = await api("/users");
    const rows = [["name", "role", "manufacturer", "directory", "groups", "sessions", "signed_in"]].concat(users.map(u =>
      [u.name, u.role, u.manufacturer, u.provider || "", (u.groups || []).join(", "), u.sessions || 0, u.signed_in ? new Date(u.signed_in * 1000).toISOString() : ""]));
    saveFile(csvFile(rows), "users-" + today() + ".csv");
  });
  run("x-access", async () => saveFile(jsonFile((await api("/access")).policy), "access-" + today() + ".json"));
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
      message("backup-msg", t("Access settings saved"), true);
      refreshSession();
    } catch (e) {
      message("backup-msg", e instanceof SyntaxError ? t("The file holds no access settings") : e.status === 409 ? t("The access settings would leave nobody who manages users") : e.message);
    }
  };
}
