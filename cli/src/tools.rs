//! The tools of the command line and the MCP server that need the session or the
//! machine (files, parts server, Autodesk Vault, undo); the operations on the project are commands
//! of the built-in plugins (`openecraft_api::builtin`). A tool has typed parameters
//! (from which the JSON schema and the command line parsing follow) and returns JSON.
//! Help texts are English (`tr_const!`); the command line shows them translated, MCP
//! clients get them as they are. The commands of plugins (`openecraft_api`) follow the
//! built-in tools.

mod config;
mod devices;
mod integration;
mod languages;
mod management;
mod output;
mod pages;
mod panel;
mod parts;
mod plc;
mod plugins;
mod reports;
mod project;
mod properties;
mod sheets;
mod structure;
mod tables;
mod vault;
#[cfg(test)]
mod workflows;

use crate::session::Session;
use openecraft_api::builtin::common;
use openecraft_api::params;
pub use openecraft_api::params::{optional, param, Args, Kind, Param};
use openecraft_api::Command;
use openecraft_i18n::tr;
use serde_json::{Map, Value};

pub type Run = fn(&mut Session, &Args) -> Result<Value, String>;

pub struct Tool {
    pub name: &'static str,
    pub help: &'static str,
    pub params: &'static [Param],
    /// Works on the open project (the command line opens `--project` first).
    pub needs_project: bool,
    pub run: Run,
}

pub fn all() -> impl Iterator<Item = &'static Tool> {
    [project::TOOLS, management::TOOLS, pages::TOOLS, sheets::TOOLS, devices::TOOLS, plc::TOOLS, tables::TOOLS, panel::TOOLS, integration::TOOLS, languages::TOOLS, structure::TOOLS, properties::TOOLS, config::TOOLS, output::TOOLS, reports::TOOLS, parts::TOOLS, vault::TOOLS, plugins::TOOLS].into_iter().flatten()
}

pub fn find(name: &str) -> Option<&'static Tool> {
    all().find(|t| t.name == name)
}

/// A built-in tool or a command of a plugin: both are commands of the command line and
/// tools of the MCP server.
#[derive(Clone)]
pub enum Entry {
    Tool(&'static Tool),
    Plugin(Command),
}

impl From<&'static Tool> for Entry {
    fn from(t: &'static Tool) -> Entry {
        Entry::Tool(t)
    }
}

impl Entry {
    pub fn name(&self) -> &str {
        match self {
            Entry::Tool(t) => t.name,
            Entry::Plugin(c) => &c.name,
        }
    }

    /// Help text in English (MCP clients).
    pub fn help(&self) -> &str {
        match self {
            Entry::Tool(t) => t.help,
            Entry::Plugin(c) => &c.help,
        }
    }

    /// Help text in the current language.
    pub fn help_text(&self) -> &str {
        match self {
            Entry::Tool(t) => tr(t.help),
            Entry::Plugin(c) => params::text(&c.help),
        }
    }

    pub fn params(&self) -> &[Param] {
        match self {
            Entry::Tool(t) => t.params,
            Entry::Plugin(c) => &c.params,
        }
    }

    pub fn needs_project(&self) -> bool {
        match self {
            Entry::Tool(t) => t.needs_project,
            Entry::Plugin(_) => true,
        }
    }
}

/// The built-in tools followed by the commands of the enabled plugins.
pub fn entries(session: &Session) -> Vec<Entry> {
    all().map(Entry::Tool).chain(session.plugins.commands().cloned().map(Entry::Plugin)).collect()
}

pub fn lookup(session: &Session, name: &str) -> Option<Entry> {
    find(name).map(Entry::Tool).or_else(|| session.plugins.command(name).cloned().map(Entry::Plugin))
}

/// Checks the parameters against the tool and runs it. Changes of the project become
/// one undo step.
pub fn call(session: &mut Session, entry: impl Into<Entry>, values: &Map<String, Value>) -> Result<Value, String> {
    let entry = entry.into();
    let values = &openecraft_api::builtin::current_names(entry.name(), values);
    params::check(entry.params(), values)?;
    let before = if entry.needs_project() { Some(session.project()?.clone()) } else { None };
    let mut result = match &entry {
        Entry::Tool(t) => (t.run)(session, &Args(values)),
        Entry::Plugin(c) => session.run_command(c, values),
    };
    match (&mut result, before) {
        // A command that fails leaves the project as it was, also halfway through.
        (Err(_), Some(prj)) => session.restore(prj),
        // Devices moved into or out of structure boxes take their structure.
        (Ok(value), Some(before)) if matches!(entry, Entry::Plugin(_)) => {
            let kept = openecraft_core::structure_boxes::follow(session.project_mut()?, &before);
            if let (false, Some(map)) = (kept.is_empty(), value.as_object_mut()) {
                map.insert("structure_kept".into(), Value::from(kept));
            }
        }
        _ => {}
    }
    session.commit();
    result
}

/// Numbers of the project are `f32`: as JSON they would show the binary noise of the
/// conversion (`41.79999923706055`). Numbers that are exactly an `f32` are written
/// as the shortest decimal of that `f32` (`41.8`); other numbers stay as they are.
pub fn tidy(value: &mut Value) {
    match value {
        Value::Number(n) if n.is_f64() => {
            if let Some(f) = n.as_f64().filter(|f| f.is_finite() && (*f as f32) as f64 == *f) {
                if let Some(short) = format!("{}", f as f32).parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
                    *n = short;
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(tidy),
        Value::Object(map) => map.values_mut().for_each(tidy),
        _ => {}
    }
}

/// JSON schema of the tool's parameters (MCP `inputSchema`).
pub fn schema(entry: impl Into<Entry>) -> Value {
    params::schema(entry.into().params())
}

/// Parameters from the command line: `--json` first, then `--name value` pairs
/// converted to the parameter's type.
pub fn from_cli(entry: impl Into<Entry>, json: Option<&str>, pairs: &[(String, Option<String>)]) -> Result<Map<String, Value>, String> {
    let entry = entry.into();
    let pairs: Vec<(String, Option<String>)> = pairs.iter().map(|(k, v)| (openecraft_api::builtin::renamed_param(entry.name(), k).map_or_else(|| k.clone(), String::from), v.clone())).collect();
    let mut values = params::from_cli(entry.params(), None, &pairs)?;
    if let Some(text) = json {
        // Parameters of --json may use the old names as well; pairs win.
        let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) else { return Err(openecraft_i18n::tr("--json needs a JSON object").to_string()) };
        for (k, v) in openecraft_api::builtin::current_names(entry.name(), &map) {
            values.entry(k).or_insert(v);
        }
    }
    Ok(values)
}

/// Command line help of a tool.
pub fn help(entry: impl Into<Entry>) -> String {
    let entry = entry.into();
    let mut out = format!("openecraft-cli {}{}\n\n{}\n", entry.name(), if entry.needs_project() { " --project <file>" } else { "" }, entry.help_text());
    if !entry.params().is_empty() {
        out.push('\n');
        for p in entry.params() {
            let arg = format!("--{} {}", p.name, p.kind.hint());
            let mark = if p.required { String::new() } else { format!("{} ", tr("(optional)")) };
            out.push_str(&format!("  {arg:<28} {mark}{}\n", params::text(&p.help)));
        }
    }
    out
}

#[cfg(test)]
mod tests;
