//! Parts server tools: the functions of the parts manager and the data portal
//! (faceted search, catalogs, files, users, roles and grants, favorites, update check)
//! without a window.

use super::{optional, param, Args, Kind, Tool};
use crate::session::Session;
use openecraft_client::{Endpoint, PortalKind};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use openecraft_parts::access::{Grant, Policy, Right, Role as DefinedRole, Subject, SERVER};
use openecraft_parts::api::{PartQuery, UserUpdate};
use openecraft_parts::portal::{self, catalog_id, AttachmentKind, Catalog, Facet, Lifecycle, PartsRight, Role, Sort};
use openecraft_parts::{Part, TopGroup};
use serde_json::{json, Value};

const NUMBER: super::Param = param("number", Kind::Str, tr_const!("Part number"));

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "import_eplan_symbols",
        help: tr_const!("Imports EPLAN symbols from a file or a selected article in a folder"),
        params: &[param("path", Kind::Str, tr_const!("EPLAN archive, macro, article XML or vendor folder")), optional("part", Kind::Str, tr_const!("Part number to import from a vendor folder"))],
        needs_project: true,
        run: import_eplan_symbols,
    },
    Tool {
        name: "parts_connect",
        help: tr_const!("Sets the parts server (URL and access token, or user and password) and shows its state"),
        params: &[
            optional("kind", Kind::Str, tr_const!("Kind of server: OpenECraft (parts server) or Eplan (EPLAN Data Portal: searching and reading only, the token is the personal access token of the EPLAN account)")),
            optional("url", Kind::Str, tr_const!("Server URL, e.g. http://127.0.0.1:8090")),
            optional("token", Kind::Str, tr_const!("Access token for changes")),
            optional("user", Kind::Str, tr_const!("User name to sign in with")),
            optional("password", Kind::Str, tr_const!("Password of the user")),
        ],
        needs_project: false,
        run: connect,
    },
    Tool {
        name: "parts_search",
        help: tr_const!("Searches the parts on the parts server (data portal) by text, groups, catalog, lifecycle and technical data"),
        params: &[
            optional("query", Kind::Str, tr_const!("Text in number, type, manufacturer, description, groups or catalog; every word must occur")),
            optional("top_group", Kind::Str, tr_const!("Product top group: Electrical, Fluid, Mechanics or Process")),
            optional("group", Kind::Str, tr_const!("Product group")),
            optional("subgroup", Kind::Str, tr_const!("Product subgroup")),
            optional("manufacturer", Kind::Str, tr_const!("Manufacturer")),
            optional("catalog", Kind::Str, tr_const!("Catalog id")),
            optional("lifecycle", Kind::Str, tr_const!("Lifecycle: Active, PhaseOut or Discontinued")),
            optional("attributes", Kind::Object, tr_const!("Technical data that must match, e.g. {\"Voltage\": \"24 V DC\", \"Poles\": \"3\"}")),
            optional("sort", Kind::Str, tr_const!("Order: Number, Manufacturer, Description, Price or Modified")),
            optional("descending", Kind::Bool, tr_const!("Descending order")),
            optional("offset", Kind::Int, tr_const!("Number of hits to skip")),
            optional("limit", Kind::Int, tr_const!("Maximum number of hits (default 50)")),
            optional("full", Kind::Bool, tr_const!("Return all part data instead of a summary")),
            optional("facets", Kind::Bool, tr_const!("Also return the facets: values of groups, manufacturers, catalogs, lifecycle and technical data with their counts")),
        ],
        needs_project: false,
        run: search,
    },
    Tool {
        name: "parts_history",
        help: tr_const!("Shows the changes of a part on the parts server, or the part as of one change"),
        params: &[NUMBER, optional("revision", Kind::Int, tr_const!("Revision of the change; returns the part as it was"))],
        needs_project: false,
        run: history,
    },
    Tool {
        name: "parts_attach",
        help: tr_const!("Stores a file (datasheet, image, model, macro …) with a part on the parts server"),
        params: &[
            NUMBER,
            param("path", Kind::Str, tr_const!("File to upload")),
            optional("kind", Kind::Str, tr_const!("Datasheet, Image, Manual, Certificate, Drawing, Model3d, Macro or Other (default: by file name)")),
        ],
        needs_project: false,
        run: attach,
    },
    Tool {
        name: "parts_detach",
        help: tr_const!("Removes a file from a part on the parts server"),
        params: &[NUMBER, param("id", Kind::Str, tr_const!("File id (see the attachments of parts_get)"))],
        needs_project: false,
        run: detach,
    },
    Tool {
        name: "parts_file",
        help: tr_const!("Downloads a file of a part from the parts server"),
        params: &[param("id", Kind::Str, tr_const!("File id (see the attachments of parts_get)")), param("path", Kind::Str, tr_const!("Output file"))],
        needs_project: false,
        run: file,
    },
    Tool {
        name: "parts_catalogs",
        help: tr_const!("Lists the catalogs of the parts server with their number of parts"),
        params: &[],
        needs_project: false,
        run: catalogs,
    },
    Tool {
        name: "parts_catalog_save",
        help: tr_const!("Creates or changes a catalog on the parts server (id, name, manufacturer, version, description)"),
        params: &[param("catalog", Kind::Object, tr_const!("Catalog data; without id it is derived from the name"))],
        needs_project: false,
        run: catalog_save,
    },
    Tool {
        name: "parts_catalog_delete",
        help: tr_const!("Deletes a catalog without parts from the parts server"),
        params: &[param("id", Kind::Str, tr_const!("Catalog id"))],
        needs_project: false,
        run: catalog_delete,
    },
    Tool {
        name: "parts_favorites",
        help: tr_const!("Shows the favorites of the signed in user, optionally adding or removing a part"),
        params: &[optional("add", Kind::Str, tr_const!("Part number to add")), optional("remove", Kind::Str, tr_const!("Part number to remove"))],
        needs_project: false,
        run: favorites,
    },
    Tool {
        name: "parts_users",
        help: tr_const!("Lists the users of the parts server (administrators only)"),
        params: &[],
        needs_project: false,
        run: users,
    },
    Tool {
        name: "parts_user_save",
        help: tr_const!("Creates or changes a user of the parts server (administrators only)"),
        params: &[
            param("name", Kind::Str, tr_const!("User name")),
            param("role", Kind::Str, tr_const!("Reader, Manufacturer, Editor or Admin")),
            optional("manufacturer", Kind::Str, tr_const!("Manufacturer whose parts a manufacturer user maintains")),
            optional("password", Kind::Str, tr_const!("Password (needed for new users; empty keeps the current one)")),
            optional("groups", Kind::Str, tr_const!("Local groups of the user, separated by commas")),
        ],
        needs_project: false,
        run: user_save,
    },
    Tool {
        name: "parts_user_delete",
        help: tr_const!("Deletes a user of the parts server (administrators only)"),
        params: &[param("name", Kind::Str, tr_const!("User name"))],
        needs_project: false,
        run: user_delete,
    },
    Tool {
        name: "parts_access",
        help: tr_const!("Shows the roles, grants and directories of the parts server, optionally setting the role of anonymous requests (administrators only)"),
        params: &[optional("anonymous", Kind::Str, tr_const!("Role of requests without sign-in; none: sign-in needed for reading"))],
        needs_project: false,
        run: access,
    },
    Tool {
        name: "parts_grant",
        help: tr_const!("Gives a user or group a role on a manufacturer (scope) of the parts server, with deny takes its rights away, with revoke removes grants (administrators only)"),
        params: &[
            param("subject", Kind::Str, tr_const!("user or group")),
            param("name", Kind::Str, tr_const!("User or group name (wildcards; directory groups as directory:group)")),
            optional("role", Kind::Str, tr_const!("Role: reader, manufacturer, editor, admin or a defined role")),
            optional("scope", Kind::Str, tr_const!("Manufacturer (wildcards); default * (everything)")),
            optional("deny", Kind::Bool, tr_const!("Take the rights of the role away instead")),
            optional("revoke", Kind::Bool, tr_const!("Remove the grants of the user or group (only of this role and scope, if given)")),
        ],
        needs_project: false,
        run: grant,
    },
    Tool {
        name: "parts_role",
        help: tr_const!("Defines, changes or deletes a role of the parts server (administrators only)"),
        params: &[
            param("name", Kind::Str, tr_const!("Role name")),
            optional("rights", Kind::Str, tr_const!("Rights separated by commas: read, edit_parts, delete_parts, edit_catalogs, administer")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("delete", Kind::Bool, tr_const!("Delete the role")),
        ],
        needs_project: false,
        run: role,
    },
    Tool {
        name: "parts_check_updates",
        help: tr_const!("Compares the project parts with the parts server: newer versions, phase-out, discontinued and removed parts"),
        params: &[optional("apply", Kind::Bool, tr_const!("Replace the project parts by their newer server versions"))],
        needs_project: true,
        run: check_updates,
    },
    Tool {
        name: "parts_get",
        help: tr_const!("Shows all data of a part"),
        params: &[NUMBER],
        needs_project: false,
        run: get,
    },
    Tool {
        name: "parts_save",
        help: tr_const!("Creates or replaces a part on the parts server (same JSON as parts_get; missing fields are empty)"),
        params: &[param("part", Kind::Object, tr_const!("Part data"))],
        needs_project: false,
        run: save,
    },
    Tool {
        name: "parts_rename",
        help: tr_const!("Changes the number of a part on the parts server"),
        params: &[NUMBER, param("new_number", Kind::Str, tr_const!("New part number"))],
        needs_project: false,
        run: rename,
    },
    Tool {
        name: "parts_delete",
        help: tr_const!("Deletes a part from the parts server"),
        params: &[NUMBER],
        needs_project: false,
        run: delete,
    },
    Tool {
        name: "parts_import",
        help: tr_const!("Imports a parts file into the parts server; the pictures and macros of an EDZ file are stored with their parts"),
        params: &[param("path", Kind::Str, tr_const!("CSV, JSON, BMEcat, EPLAN XML or EDZ file")), optional("part", Kind::Str, tr_const!("Part number to import from a vendor folder"))],
        needs_project: false,
        run: import,
    },
    Tool {
        name: "parts_export",
        help: tr_const!("Exports all parts of the parts server as CSV or EPLAN parts XML"),
        params: &[
            optional("path", Kind::Str, tr_const!("Output file; without it the content is returned")),
            optional("format", Kind::Str, tr_const!("csv (default) or eplan_xml")),
        ],
        needs_project: false,
        run: export,
    },
];

fn connect(s: &mut Session, a: &Args) -> Result<Value, String> {
    if let Some(kind) = optional_choice(a, "kind", &PortalKind::ALL)?.filter(|k| *k != s.server.kind) {
        s.server.kind = kind;
        s.server.endpoint = Endpoint { url: kind.default_url(), token: String::new() };
    }
    if let Some(url) = a.str("url") {
        s.server.endpoint.url = url.into();
    }
    if let Some(token) = a.str("token") {
        s.server.endpoint.token = token.into();
    }
    if let Some(user) = a.str("user") {
        s.server.login(user, a.str("password").unwrap_or(""))?;
    }
    let info = s.server.info()?;
    Ok(json!({ "kind": s.server.kind, "url": s.server.endpoint.url, "info": info }))
}

/// A value of an enumeration by its name (case insensitive), e.g. `PhaseOut`.
fn choice<T: Copy + std::fmt::Debug>(all: &[T], name: &str, text: &str) -> Result<T, String> {
    all.iter()
        .copied()
        .find(|v| format!("{v:?}").eq_ignore_ascii_case(text.trim()))
        .ok_or_else(|| tr_fmt!("Unknown {}: {} (possible: {})", name, text, all.iter().map(|v| format!("{v:?}")).collect::<Vec<_>>().join(", ")))
}

fn optional_choice<T: Copy + std::fmt::Debug>(a: &Args, key: &str, all: &[T]) -> Result<Option<T>, String> {
    a.str(key).map(|t| choice(all, key, t)).transpose()
}

fn search(s: &mut Session, a: &Args) -> Result<Value, String> {
    let text = |k: &str| a.str(k).map(String::from);
    let attributes = match a.value("attributes") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))).collect(),
        _ => Vec::new(),
    };
    let query = PartQuery {
        q: text("query"),
        top_group: optional_choice(a, "top_group", &TopGroup::ALL)?,
        group: text("group"),
        subgroup: text("subgroup"),
        manufacturer: text("manufacturer"),
        catalog: text("catalog"),
        lifecycle: optional_choice(a, "lifecycle", &Lifecycle::ALL)?,
        attributes,
        sort: optional_choice(a, "sort", &Sort::ALL)?.unwrap_or_default(),
        descending: a.flag("descending"),
        brief: !a.flag("full"),
        offset: a.int("offset").map(|o| o.max(0) as usize),
        limit: Some(a.int("limit").unwrap_or(50).max(0) as usize),
        ..Default::default()
    };
    let r = s.server.search(&query)?;
    let parts: Vec<Value> = if a.flag("full") {
        r.parts.iter().map(|p| serde_json::to_value(p).unwrap_or_default()).collect()
    } else {
        r.parts
            .iter()
            .map(|p| {
                let mut v = json!({
                    "number": p.number,
                    "manufacturer": p.manufacturer,
                    "type_number": p.type_number,
                    "description": p.description,
                    "group": p.group,
                });
                if !p.lifecycle.is_active() {
                    v["lifecycle"] = json!(p.lifecycle);
                    v["successor"] = json!(p.successor);
                }
                if !p.catalog.is_empty() {
                    v["catalog"] = json!(p.catalog);
                }
                v
            })
            .collect()
    };
    let mut out = json!({ "total": r.total, "offset": r.offset, "parts": parts });
    if a.flag("facets") {
        let facets = |list: &[Facet]| -> Value {
            list.iter().map(|f| (f.name.clone(), f.values.iter().map(|v| json!([v.value, v.count])).collect::<Value>())).collect::<serde_json::Map<_, _>>().into()
        };
        out["facets"] = facets(&r.facets);
        out["attributes"] = facets(&r.attributes);
    }
    Ok(out)
}

fn history(s: &mut Session, a: &Args) -> Result<Value, String> {
    let number = a.req("number");
    match a.int("revision") {
        Some(rev) => {
            let part = s.server.history_part(number, rev.max(0) as u64)?.ok_or_else(|| tr_fmt!("Part {} has no version {}", number, rev))?;
            Ok(serde_json::to_value(part).unwrap_or_default())
        }
        None => Ok(serde_json::to_value(s.server.history(number)?).unwrap_or_default()),
    }
}

fn attach(s: &mut Session, a: &Args) -> Result<Value, String> {
    let (number, path) = (a.req("number"), a.req("path"));
    let kind = optional_choice(a, "kind", &AttachmentKind::ALL)?;
    let data = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.into());
    let part = s.server.attach(number, &name, kind, data)?;
    Ok(json!({ "number": part.number, "attachments": part.attachments }))
}

fn detach(s: &mut Session, a: &Args) -> Result<Value, String> {
    let part = s.server.detach(a.req("number"), a.req("id"))?;
    Ok(json!({ "number": part.number, "attachments": part.attachments }))
}

fn file(s: &mut Session, a: &Args) -> Result<Value, String> {
    let (id, path) = (a.req("id"), a.req("path"));
    let data = s.server.file(id)?;
    std::fs::write(path, &data).map_err(|e| tr_fmt!("Cannot write {}: {}", path, openecraft_i18n::io_error(&e)))?;
    Ok(json!({ "path": path, "bytes": data.len() }))
}

fn catalogs(s: &mut Session, _: &Args) -> Result<Value, String> {
    Ok(serde_json::to_value(s.server.catalogs()?).unwrap_or_default())
}

fn catalog_save(s: &mut Session, a: &Args) -> Result<Value, String> {
    let mut c: Catalog = serde_json::from_value(a.value("catalog").cloned().unwrap_or_default()).map_err(|e| tr_fmt!("Not a valid catalog: {}", e))?;
    if c.id.trim().is_empty() {
        c.id = catalog_id(&c.name);
    }
    if c.id.is_empty() {
        return Err(tr("The catalog needs a name").to_string());
    }
    Ok(serde_json::to_value(s.server.save_catalog(&c)?).unwrap_or_default())
}

fn catalog_delete(s: &mut Session, a: &Args) -> Result<Value, String> {
    let id = a.req("id");
    s.server.delete_catalog(id)?;
    Ok(json!({ "deleted": id }))
}

fn favorites(s: &mut Session, a: &Args) -> Result<Value, String> {
    let mut list = s.server.favorites()?;
    let before = list.clone();
    if let Some(n) = a.str("add").filter(|n| !list.iter().any(|f| f == n)) {
        list.push(n.into());
    }
    if let Some(n) = a.str("remove") {
        list.retain(|f| f != n);
    }
    if list != before {
        list = s.server.set_favorites(&list)?;
    }
    Ok(json!(list))
}

fn users(s: &mut Session, _: &Args) -> Result<Value, String> {
    Ok(serde_json::to_value(s.server.users()?).unwrap_or_default())
}

fn user_save(s: &mut Session, a: &Args) -> Result<Value, String> {
    let update = UserUpdate {
        role: choice(&Role::ALL, "role", a.req("role"))?,
        manufacturer: a.str("manufacturer").unwrap_or("").into(),
        password: a.str("password").unwrap_or("").into(),
        groups: a.str("groups").map(list),
    };
    Ok(serde_json::to_value(s.server.save_user(a.req("name"), &update)?).unwrap_or_default())
}

fn user_delete(s: &mut Session, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    s.server.delete_user(name)?;
    Ok(json!({ "deleted": name }))
}

/// Comma separated values without blanks.
fn list(text: &str) -> Vec<String> {
    text.split(',').map(str::trim).filter(|v| !v.is_empty()).map(String::from).collect()
}

fn subject(a: &Args) -> Result<Subject, String> {
    let name = a.req("name").trim().to_string();
    match a.req("subject").trim().to_lowercase().as_str() {
        "user" => Ok(Subject::User(name)),
        "group" => Ok(Subject::Group(name)),
        other => Err(tr_fmt!("Unknown {}: {} (possible: {})", "subject", other, "user, group")),
    }
}

/// Changes the policy of the server and returns the new settings.
fn change_policy(s: &mut Session, change: impl FnOnce(&mut Policy<PartsRight>) -> Result<(), String>) -> Result<Value, String> {
    let mut policy = s.server.access()?.policy;
    change(&mut policy)?;
    Ok(serde_json::to_value(s.server.save_access(&policy)?).unwrap_or_default())
}

fn access(s: &mut Session, a: &Args) -> Result<Value, String> {
    match a.str("anonymous") {
        Some(role) => change_policy(s, |p| {
            p.anonymous = if role.trim().eq_ignore_ascii_case("none") { String::new() } else { role.trim().into() };
            Ok(())
        }),
        None => Ok(serde_json::to_value(s.server.access()?).unwrap_or_default()),
    }
}

fn grant(s: &mut Session, a: &Args) -> Result<Value, String> {
    let subject = subject(a)?;
    let (role, scope) = (a.str("role").map(str::trim), a.str("scope").map(str::trim));
    if a.flag("revoke") {
        return change_policy(s, |p| {
            let before = p.grants.len();
            p.grants.retain(|g| !(g.subject == subject && role.is_none_or(|r| g.role.eq_ignore_ascii_case(r)) && scope.is_none_or(|sc| g.scope().eq_ignore_ascii_case(sc))));
            if p.grants.len() == before {
                return Err(tr_fmt!("No grant of {} found", subject.name()));
            }
            Ok(())
        });
    }
    let grant = Grant { deny: a.flag("deny"), ..Grant::new(subject, role.unwrap_or(""), scope.unwrap_or(SERVER)) };
    change_policy(s, |p| {
        p.grants.retain(|g| !(g.subject == grant.subject && g.role.eq_ignore_ascii_case(&grant.role) && g.scope().eq_ignore_ascii_case(grant.scope())));
        p.grants.push(grant);
        Ok(())
    })
}

fn role(s: &mut Session, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if a.flag("delete") {
        return change_policy(s, |p| {
            let before = p.roles.len();
            p.roles.retain(|r| !r.name.eq_ignore_ascii_case(&name));
            if p.roles.len() == before {
                return Err(tr_fmt!("No role {} defined", name));
            }
            Ok(())
        });
    }
    let rights = list(a.str("rights").unwrap_or(""))
        .iter()
        .map(|r| PartsRight::from_name(r).ok_or_else(|| tr_fmt!("Unknown {}: {} (possible: {})", "right", r, PartsRight::ALL.iter().map(|x| x.name()).collect::<Vec<_>>().join(", "))))
        .collect::<Result<_, _>>()?;
    let role = DefinedRole { name, description: a.str("description").unwrap_or("").into(), rights };
    change_policy(s, |p| {
        p.roles.retain(|r| !r.name.eq_ignore_ascii_case(&role.name));
        p.roles.push(role);
        Ok(())
    })
}

fn check_updates(s: &mut Session, a: &Args) -> Result<Value, String> {
    let local = s.server.kind.update_candidates(&s.project()?.parts);
    let remote = s.server.versions(&local)?;
    let updates = portal::check_updates(&local, &remote);
    let applied = if a.flag("apply") { portal::apply_updates(&mut s.project_mut()?.parts, &remote, &updates) } else { 0 };
    Ok(json!({ "updates": updates, "applied": applied }))
}

fn existing(s: &Session, number: &str) -> Result<Part, String> {
    s.server.get(number)?.ok_or_else(|| tr_fmt!("Unknown part: {}", number))
}

fn get(s: &mut Session, a: &Args) -> Result<Value, String> {
    Ok(serde_json::to_value(existing(s, a.req("number"))?).unwrap_or_default())
}

fn save(s: &mut Session, a: &Args) -> Result<Value, String> {
    let part: Part = serde_json::from_value(a.value("part").cloned().unwrap_or_default()).map_err(|e| tr_fmt!("Not a valid part: {}", e))?;
    if part.number.trim().is_empty() {
        return Err(tr("The part number must not be empty").to_string());
    }
    s.server.put(&part)?;
    Ok(json!({ "saved": part.number }))
}

fn rename(s: &mut Session, a: &Args) -> Result<Value, String> {
    let (number, new) = (a.req("number"), a.req("new_number").trim());
    let mut part = existing(s, number)?;
    if new.is_empty() {
        return Err(tr("The part number must not be empty").to_string());
    }
    if new != number && s.server.get(new)?.is_some() {
        return Err(tr("Part number already exists").to_string());
    }
    part.number = new.into();
    s.server.put(&part)?;
    if new != number {
        s.server.delete(number)?;
    }
    Ok(json!({ "old": number, "new": new }))
}

fn delete(s: &mut Session, a: &Args) -> Result<Value, String> {
    let number = a.req("number");
    existing(s, number)?;
    s.server.delete(number)?;
    Ok(json!({ "deleted": number }))
}

fn import_eplan_symbols(s: &mut Session, a: &Args) -> Result<Value, String> {
    use openecraft_core::symlib::ema;
    let path = std::path::absolute(a.req("path")).map_err(|e| e.to_string())?;
    let project = s.project_mut()?;
    let (parts, variants) = if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ema")) && path.is_file() {
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let text = openecraft_edz::decode_text(&bytes);
        (Vec::new(), ema::decode_with_library(&text, &project.symbols)?)
    } else {
        let package = if path.is_dir() {
            let number = a.str("part").filter(|n| !n.is_empty()).ok_or_else(|| tr("Select a part number for folder import").to_string())?;
            openecraft_edz::folder::index(&path)?.part_with_library(number,&project.symbols)?
        } else {
            let data = std::fs::read(&path).map_err(|e| e.to_string())?;
            if !openecraft_edz::is_edz(&data) && !path.extension().is_some_and(|s| s.eq_ignore_ascii_case("xml")) {
                return Err(tr("Expected an EPLAN archive, window macro or article XML").to_string());
            }
            openecraft_edz::folder::read_with_library(&path, &data, &project.symbols)?
        };
        let variants =
            package.parts.iter().flat_map(|p| &p.eplan_variants).filter_map(|v| serde::Deserialize::deserialize(v.as_ref()).ok()).collect();
        (package.parts, variants)
    };
    let added = ema::import_project(project, &variants);
    let diagnostics: Vec<_> = parts
        .iter()
        .flat_map(|p| p.eplan_variants.iter().filter(|v| v["symbol"].is_null()).map(|v| json!({"part":p.number,"diagnostic":v})))
        .collect();
    let numbers: Vec<_> = parts.iter().map(|p| p.number.clone()).collect();
    for part in parts {
        if let Some(old) = project.parts.iter_mut().find(|p| p.number == part.number) {
            old.eplan_variants = part.eplan_variants;
            old.mounting_data.rail = part.mounting_data.rail;
            old.eplan_macro = part.eplan_macro;
        } else {
            project.parts.push(part);
        }
    }
    if !numbers.is_empty() {
        openecraft_core::parts::link_unassigned_plc_parts(project, &openecraft_core::parts::PartsDb { parts: Vec::new() }, None);
        project.refresh_rail_mounts();
    }
    let report: Vec<_> = variants
        .iter()
        .map(|v| {
            json!({
                "key":v.symbol.key,
                "pins":v.definitions().map(|s| s.pins.len()).sum::<usize>(),
                "graphics":v.definitions().map(|s| s.prims.len()).sum::<usize>(),
                "warnings":v.warnings,
                "unresolved":v.unresolved,
                "connection_markers":v.connection_markers,
                "transfers":v.transfers,
                "placeholders":v.placeholders,
                "existing_definition_differs":v.definitions().any(|incoming| project.symbols.iter().find(|s| s.key==incoming.key).is_some_and(|s| s!=incoming))
            })
        })
        .collect();
    Ok(json!({"added":added,"parts":numbers,"variants":report,"diagnostics":diagnostics}))
}

fn import(s: &mut Session, a: &Args) -> Result<Value, String> {
    let path = a.req("path");
    if std::path::Path::new(path).is_dir() {
        let number = a.str("part").filter(|n| !n.is_empty()).ok_or_else(|| tr("Select a part number for folder import").to_string())?;
        let edz = openecraft_edz::folder::index(std::path::Path::new(path))?.part(number)?;
        return import_package(s, edz);
    }
    let content = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    if std::path::Path::new(path).extension().is_some_and(|s| s.eq_ignore_ascii_case("xml")) && openecraft_parts::is_eplan_xml(&String::from_utf8_lossy(&content)) {
        let path = std::path::absolute(path).map_err(|e| e.to_string())?;
        return import_package(s,openecraft_edz::folder::read_file(&path,&content)?);
    }
    if !openecraft_edz::is_edz(&content) {
        let result = s.server.import(content)?;
        return Ok(json!({ "imported": result.imported, "revision": result.revision }));
    }
    import_package(s, openecraft_edz::read(&content)?)
}

fn import_package(s: &mut Session, edz: openecraft_edz::Edz) -> Result<Value, String> {
    let result = s.server.import(serde_json::to_vec(&edz.parts).map_err(|e| e.to_string())?)?;
    let mut attached = Vec::new();
    for f in edz.files {
        s.server.attach(&f.part, &f.name, Some(f.kind), f.data)?;
        attached.push(json!({ "number": f.part, "file": f.name }));
    }
    Ok(json!({ "imported": result.imported, "revision": result.revision, "attached": attached }))
}

fn export(s: &mut Session, a: &Args) -> Result<Value, String> {
    let csv = s.server.export()?;
    let csv = match a.str("format").unwrap_or("csv") {
        "csv" => csv,
        "eplan_xml" => openecraft_core::parts::to_eplan_xml(&openecraft_core::parts::from_csv(&csv)?),
        other => return Err(tr_fmt!("Unknown format: {}", other)),
    };
    match a.str("path") {
        Some(path) => {
            std::fs::write(path, &csv).map_err(|e| tr_fmt!("Cannot write {}: {}", path, openecraft_i18n::io_error(&e)))?;
            Ok(json!({ "path": path, "bytes": csv.len() }))
        }
        None => Ok(Value::String(csv)),
    }
}

#[cfg(test)]
mod tests;
