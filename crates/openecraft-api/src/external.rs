//! External plugins: programs in any language, one folder each with a `plugin.json`
//!
//! ```json
//! { "id": "my_plugin", "name": "My plugin", "version": "1.0", "description": "…",
//!   "api": 1, "run": ["python3", "main.py"] }
//! ```
//!
//! For every request the program is started in its folder, gets one JSON object on
//! stdin and answers with one JSON object on stdout (protocol: `docs/plugins.md`).
//! `{"error": "…"}` is an error. Not available in the browser (no processes).

use crate::params::{Param, Text};
use crate::{Export, Registry, Table};
use crate::{Info, Plugin, API_VERSION};
use openecraft_core::model::Project;
use openecraft_core::issue::{Issue, Severity};
use openecraft_i18n::tr_fmt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const MANIFEST: &str = "plugin.json";

/// Longest a plugin program may take for one request; then it is stopped.
pub const TIMEOUT: Duration = Duration::from_secs(300);

/// Sends a request to the plugin and returns its answer.
pub type Transport = Arc<dyn Fn(&Value) -> Result<Value, String> + Send + Sync>;

#[derive(Deserialize)]
struct Manifest {
    #[serde(flatten)]
    info: Info,
    #[serde(default = "api_version")]
    api: u32,
    run: Vec<String>,
}

fn api_version() -> u32 {
    API_VERSION
}

/// Answer to `describe`: what the plugin offers.
#[derive(Default, Deserialize)]
#[serde(default)]
struct Description {
    commands: Vec<CommandDesc>,
    checks: Vec<Named>,
    reports: Vec<Named>,
    exporters: Vec<ExporterDesc>,
}

#[derive(Deserialize)]
struct CommandDesc {
    name: String,
    #[serde(default)]
    help: String,
    #[serde(default)]
    params: Vec<Param>,
}

#[derive(Deserialize)]
struct Named {
    name: String,
    #[serde(default)]
    title: String,
}

#[derive(Deserialize)]
struct ExporterDesc {
    name: String,
    #[serde(default)]
    title: String,
    extension: String,
    #[serde(default)]
    params: Vec<Param>,
}

pub struct External {
    info: Info,
    transport: Transport,
}

impl External {
    pub fn new(info: Info, transport: Transport) -> External {
        External { info, transport }
    }

    /// The plugin in folder `dir`.
    pub fn open(dir: &Path) -> Result<External, String> {
        let path = dir.join(MANIFEST);
        let text = std::fs::read_to_string(&path).map_err(|e| tr_fmt!("Cannot read {}: {}", path.display(), openecraft_i18n::io_error(&e)))?;
        let m: Manifest = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if m.api > API_VERSION {
            return Err(tr_fmt!("Plugin {} needs a newer OpenECraft (API {})", m.info.id, m.api));
        }
        if m.run.is_empty() {
            return Err(tr_fmt!("{}: the program to run is missing", path.display()));
        }
        Ok(External::new(m.info, process(dir.to_path_buf(), m.run, TIMEOUT)))
    }
}

/// The process of a plugin program: started in the plugin folder, JSON in UTF-8 on
/// stdin and stdout (Python is told so: on Windows it would use the ANSI code page and
/// garble every non-ASCII character of the project).
fn command(dir: &Path, program: &Path, args: &[String]) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    c.args(args)
        .current_dir(dir)
        .env("PYTHONUTF8", "1")
        .env("PYTHONIOENCODING", "utf-8")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    c
}

/// Program to try when `program` is not installed: `python3` is called `python` on
/// Windows.
fn alternative(program: &Path) -> Option<PathBuf> {
    (program.as_os_str() == "python3").then(|| PathBuf::from("python"))
}

/// Runs the program once per request; stops it after `timeout`.
fn process(dir: PathBuf, run: Vec<String>, timeout: Duration) -> Transport {
    Arc::new(move |request: &Value| {
        // A program given by path is found in the plugin folder.
        let program = if run[0].contains(['/', '\\']) && dir.join(&run[0]).exists() { dir.join(&run[0]) } else { PathBuf::from(&run[0]) };
        let started = match command(&dir, &program, &run[1..]).spawn() {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => match alternative(&program) {
                Some(other) => command(&dir, &other, &run[1..]).spawn(),
                None => Err(e),
            },
            other => other,
        };
        let mut child = started.map_err(|e| tr_fmt!("Cannot start {}: {}", program.display(), openecraft_i18n::io_error(&e)))?;
        // Request and answers flow at the same time, so a program that answers before it
        // has read everything does not block.
        let (stdin, request) = (child.stdin.take(), request.to_string());
        let writer = std::thread::spawn(move || stdin.map(|mut s| s.write_all(request.as_bytes())));
        let read = |pipe: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
                let mut buf = Vec::new();
                if let Some(mut p) = pipe {
                    let _ = p.read_to_end(&mut buf);
                }
                buf
            })
        };
        let stdout = read(child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let stderr = read(child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait().map_err(|e| e.to_string())? {
                Some(status) => break status,
                None if Instant::now() < deadline => std::thread::sleep(Duration::from_millis(10)),
                None => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(tr_fmt!("The plugin program did not answer within {} s and was stopped", timeout.as_secs()));
                }
            }
        };
        let _ = writer.join();
        let (stdout, stderr) = (stdout.join().unwrap_or_default(), stderr.join().unwrap_or_default());
        if !status.success() {
            return Err(tr_fmt!("The plugin program failed: {}", String::from_utf8_lossy(&stderr).trim()));
        }
        serde_json::from_slice(&stdout).map_err(|e| tr_fmt!("Invalid answer of the plugin: {}", e))
    })
}

/// Sends a request; `{"error": …}` becomes an error.
fn ask(transport: &Transport, mut request: Value) -> Result<Value, String> {
    request["api"] = API_VERSION.into();
    request["project_features"] = json!(["directional_pin_contacts","explicit_wire_junctions","directed_junction_paths","fluid_port_roles","wire_hide_labels","scoped_connections","product_designation_fields","function_text_fields","function_part_selection","property_blocks","wire_type","physical_wire_routes","multi_channel_plc","linked_plc_channels","plc_channel_indices","explicit_panel_mounts","individual_panel_terminals","explicit_panel_geometry","literal_part_quantities","material_ownership","portable_macro_materials","portable_cables","per_wire_junctions","macro_definition_parameters","parallel_cable_cores","page_property_blocks","page_template_values","page_frame_fields","function_cable_references","cable_property_blocks","placed_wire_labels","explicit_panel_rotations","function_cross_references"]);
    let answer = transport(&request)?;
    match answer.get("error") {
        Some(e) => Err(e.as_str().map(String::from).unwrap_or_else(|| e.to_string())),
        None => Ok(answer),
    }
}

fn project_value(prj: &Project) -> Value {
    serde_json::to_value(prj).unwrap_or_default()
}

fn decode<T: for<'a> Deserialize<'a>>(v: Value) -> Result<T, String> {
    serde_json::from_value(v).map_err(|e| tr_fmt!("Invalid answer of the plugin: {}", e))
}

fn text(s: String, name: &str) -> Text {
    Text::Owned(if s.is_empty() { name.to_string() } else { s })
}

#[derive(Deserialize)]
struct IssueDesc {
    #[serde(default)]
    severity: String,
    message: String,
    page: Option<usize>,
    item: Option<u64>,
}

/// An issue of the plugin; `page` alone points at the page (item 0), a page the
/// project does not have at nothing.
fn issue(d: IssueDesc, pages: usize) -> Issue {
    let severity = match d.severity.as_str() {
        "error" => Severity::Error,
        "info" => Severity::Info,
        _ => Severity::Warning,
    };
    let target = d.page.filter(|p| *p < pages).map(|p| (p, d.item.unwrap_or(0)));
    Issue::plain(severity, d.message, target)
}

impl Plugin for External {
    fn info(&self) -> Info {
        self.info.clone()
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        let desc: Description = decode(ask(&self.transport, json!({ "method": "describe" }))?)?;
        for c in desc.commands {
            let (t, name) = (self.transport.clone(), c.name.clone());
            reg.command(c.name.clone(), text(c.help, &c.name), c.params, move |prj, args| {
                let mut answer = ask(&t, json!({ "method": "command", "name": name, "params": args.0, "project": project_value(prj) }))?;
                if let Some(changed) = answer.get_mut("project").map(Value::take) {
                    *prj = Project::from_json(&changed.to_string())?;
                }
                Ok(answer.get_mut("result").map(Value::take).unwrap_or(Value::Null))
            });
        }
        for c in desc.checks {
            let (t, name) = (self.transport.clone(), c.name.clone());
            reg.check(c.name.clone(), text(c.title, &c.name), move |prj, _| {
                let mut answer = ask(&t, json!({ "method": "check", "name": name, "project": project_value(prj) }))?;
                let found: Vec<IssueDesc> = decode(answer.get_mut("issues").map(Value::take).unwrap_or(json!([])))?;
                Ok(found.into_iter().map(|d| issue(d, prj.pages.len())).collect())
            });
        }
        for r in desc.reports {
            let (t, name) = (self.transport.clone(), r.name.clone());
            reg.report(r.name.clone(), text(r.title, &r.name), move |prj, _| {
                #[derive(Deserialize)]
                struct Answer {
                    headers: Vec<String>,
                    #[serde(default)]
                    rows: Vec<Vec<String>>,
                }
                let a: Answer = decode(ask(&t, json!({ "method": "report", "name": name, "project": project_value(prj) }))?)?;
                let mut table = Table::new(a.headers);
                for row in a.rows {
                    table.push(row, None);
                }
                Ok(table)
            });
        }
        for e in desc.exporters {
            let (t, name) = (self.transport.clone(), e.name.clone());
            reg.exporter_with(e.name.clone(), text(e.title, &e.name), e.extension, false, e.params, move |prj, _, args| {
                let answer = ask(&t, json!({ "method": "export", "name": name, "params": args.0, "project": project_value(prj) }))?;
                answer.get("content").and_then(Value::as_str).map(|s| Export::Text(s.to_string())).ok_or_else(|| tr_fmt!("Invalid answer of the plugin: {}", "content"))
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
