//! Project tools: files, project properties, undo / redo.

use super::{optional, param, Args, Kind, Param, Tool};
use crate::session::Session;
use openecraft_core::index::Index;
use openecraft_core::model::*;
use openecraft_core::analysis::Analysis;
use openecraft_core::issue::Severity;
use openecraft_core::sample;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

const DISCARD: Param = optional("discard", Kind::Bool, tr_const!("Discard unsaved changes of the open project"));

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "new_project",
        help: tr_const!("Creates a new project (optionally the demo project) and saves it if a path is given"),
        params: &[
            optional("path", Kind::Str, tr_const!("File to save the project to (.oec; .oecp: a split project in the folder of the file)")),
            optional("name", Kind::Str, tr_const!("Project name")),
            optional("demo", Kind::Bool, tr_const!("Start from the demo project")),
            optional("example", Kind::Str, tr_const!("Start from an example project (names: list_examples)")),
            optional("template", Kind::Str, tr_const!("Start from a project template: a built-in one (Machine, Distribution board (IEC 61439), Empty (IEC 60204-1)) or a project file")),
            optional("overwrite", Kind::Bool, tr_const!("Replace the file at path if it exists")),
            DISCARD,
        ],
        needs_project: false,
        run: new_project,
    },
    Tool {
        name: "list_examples",
        help: tr_const!("Lists the example projects, from a first circuit to a whole plant, with the features each one shows"),
        params: &[],
        needs_project: false,
        run: list_examples,
    },
    Tool {
        name: "open_project",
        help: tr_const!("Opens a project file or archive (files of older versions are converted)"),
        params: &[param("path", Kind::Str, tr_const!("Project file (.oec), index of a split project (.oecp) or archive (.oecz)")), DISCARD],
        needs_project: false,
        run: open_project,
    },
    Tool {
        name: "save_project",
        help: tr_const!("Saves the project to its file or to a new path"),
        params: &[optional("path", Kind::Str, tr_const!("New file name (save as)"))],
        needs_project: true,
        run: save_project,
    },
    Tool {
        name: "project_info",
        help: tr_const!("Shows the project properties, file, number of pages and devices and the check result"),
        params: &[],
        needs_project: true,
        run: project_info,
    },
    Tool {
        name: "set_project_info",
        help: tr_const!("Changes project properties"),
        params: &[
            optional("name", Kind::Str, tr_const!("Project name")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("company", Kind::Str, tr_const!("Company")),
            optional("creator", Kind::Str, tr_const!("Creator")),
            optional("date", Kind::Str, tr_const!("Date")),
            optional("revision", Kind::Str, tr_const!("Revision")),
            optional("properties", Kind::Object, tr_const!("User defined properties (name: value; an empty value removes it)")),
            optional("logo", Kind::Str, tr_const!("PNG or JPEG file of the project logo (cover page, title blocks); empty removes it")),
        ],
        needs_project: true,
        run: set_project_info,
    },
    Tool { name: "undo", help: tr_const!("Undoes the last change"), params: &[], needs_project: true, run: undo },
    Tool { name: "redo", help: tr_const!("Redoes the last undone change"), params: &[], needs_project: true, run: redo },
];

fn ensure_unsaved_ok(s: &Session, a: &Args) -> Result<(), String> {
    if s.unsaved() && !a.flag("discard") {
        return Err(tr("The open project has unsaved changes: save it first or pass discard").to_string());
    }
    Ok(())
}

fn summary(s: &Session) -> Result<Value, String> {
    let prj = s.project()?;
    let mut an = Analysis::build(prj);
    an.issues = s.plugins.check(prj, &an);
    an.issues.sort_by_key(|i| i.severity);
    let count = |sev| an.issues.iter().filter(|i| i.severity == sev).count();
    let i = &prj.info;
    Ok(json!({
        "name": i.name,
        "description": i.description,
        "company": i.company,
        "creator": i.creator,
        "date": i.date,
        "revision": i.revision,
        "properties": i.properties,
        "logo": i.logo.as_ref().map(|l| json!({ "name": l.file.name, "px": l.px })),
        "file": s.path.as_ref().map(|p| p.display().to_string()),
        "unsaved": s.unsaved(),
        "pages": prj.pages.len(),
        "devices": Index::build(prj).devices.len(),
        "errors": count(Severity::Error),
        "warnings": count(Severity::Warning),
        "can_undo": s.can_undo(),
        "can_redo": s.can_redo(),
    }))
}

fn new_project(s: &mut Session, a: &Args) -> Result<Value, String> {
    ensure_unsaved_ok(s, a)?;
    if let Some(path) = a.str("path").filter(|p| std::path::Path::new(p).exists() && !a.flag("overwrite")) {
        return Err(tr_fmt!("{} exists already: pass overwrite to replace it", path));
    }
    let mut prj = if a.flag("demo") {
        sample::demo_project()
    } else if let Some(key) = a.str("example") {
        sample::example(key).ok_or_else(|| tr_fmt!("Unknown example {}: see list_examples", key))?
    } else if let Some(t) = a.str("template") {
        let template = match openecraft_core::project_templates::builtin(t) {
            Some(p) => p,
            None if !std::path::Path::new(t).exists() => {
                let names: Vec<&str> = openecraft_core::project_templates::BUILTIN.iter().map(|(n, _)| tr(n)).collect();
                return Err(tr_fmt!("Unknown template {}: give a built-in one ({}) or a project file", t, names.join(", ")));
            }
            None => crate::session::read_project(std::path::Path::new(t))?.0,
        };
        openecraft_core::project_templates::from_template(&template, "", "")
    } else {
        Project::new_default()
    };
    if let Some(name) = a.str("name") {
        prj.info.name = name.into();
    }
    s.replace(prj, None);
    if let Some(path) = a.str("path") {
        s.save(Some(path))?;
    }
    summary(s)
}

fn list_examples(_: &mut Session, _: &Args) -> Result<Value, String> {
    let list: Vec<Value> = sample::EXAMPLES
        .iter()
        .map(|e| json!({ "name": e.key, "level": e.level, "title": tr(e.title), "description": tr(e.description) }))
        .collect();
    Ok(json!(list))
}

fn open_project(s: &mut Session, a: &Args) -> Result<Value, String> {
    ensure_unsaved_ok(s, a)?;
    let conv = s.open(a.req("path"))?;
    let mut out = summary(s)?;
    out["format"] = json!(conv.from);
    if let Some(note) = conv.message() {
        out["conversion"] = json!(note);
    }
    Ok(out)
}

fn save_project(s: &mut Session, a: &Args) -> Result<Value, String> {
    let path = s.save(a.str("path"))?;
    Ok(json!({ "path": path.display().to_string() }))
}

fn project_info(s: &mut Session, _: &Args) -> Result<Value, String> {
    summary(s)
}

fn set_project_info(s: &mut Session, a: &Args) -> Result<Value, String> {
    let info = &mut s.project_mut()?.info;
    for (name, field) in [
        ("name", &mut info.name),
        ("description", &mut info.description),
        ("company", &mut info.company),
        ("creator", &mut info.creator),
        ("date", &mut info.date),
        ("revision", &mut info.revision),
    ] {
        if let Some(v) = a.str(name) {
            *field = v.into();
        }
    }
    if let Some(Value::Object(props)) = a.value("properties") {
        let mut values = Vec::new();
        for (k, v) in props {
            let text = match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => String::new(),
                _ => return Err(tr_fmt!("Property {} needs a text or a number", k)),
            };
            values.push((k.clone(), text));
        }
        for (k, v) in values {
            match v.trim() {
                "" => info.properties.remove(&k),
                _ => info.properties.insert(k, v),
            };
        }
    }
    match a.str("logo") {
        Some("") => s.project_mut()?.info.logo = None,
        Some(path) => s.project_mut()?.info.logo = Some(super::sheets::read_picture(path)?),
        None => {}
    }
    summary(s)
}

fn undo(s: &mut Session, _: &Args) -> Result<Value, String> {
    let done = s.undo()?;
    Ok(json!({ "undone": done, "can_undo": s.can_undo(), "can_redo": s.can_redo() }))
}

fn redo(s: &mut Session, _: &Args) -> Result<Value, String> {
    let done = s.redo()?;
    Ok(json!({ "redone": done, "can_undo": s.can_undo(), "can_redo": s.can_redo() }))
}

#[cfg(test)]
mod tests;
