//! Model Context Protocol server: JSON-RPC 2.0, one message per line on stdin /
//! stdout. Offers every tool of [`crate::tools`]; the session (open project, undo
//! history, parts server) lives as long as the connection. Texts for the agent are
//! English like the rest of the protocol.

use crate::session::Session;
use crate::tools;
use serde_json::{json, Map, Value};
use std::io::{BufRead, Write};
use std::path::{Component, Path, PathBuf};

pub const PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED: [&str; 3] = [PROTOCOL_VERSION, "2025-03-26", "2024-11-05"];

const INSTRUCTIONS: &str = "OpenECraft is an electrical CAE (schematic) editor. Open or create a project first \
(open_project, new_project); the tools change it in memory until save_project writes it, undo / redo revert single \
tool calls. Pages are A3 (420 x 297 mm, y downwards, 10 columns, 6 rows); positions snap to a 4 mm grid. Use \
list_symbols for symbol keys and connection points, get_page for positions and item ids, connect to draw wires \
between connection points (TAG:PIN, e.g. -K1:A1), check to find errors and report / export for the results. Device \
tags follow IEC 81346 (-K1 contactor, -S1 switch, -X1 terminal strip). Parts come from the project, the built-in \
catalog or the parts server (parts_* tools).";

/// Names of the tools to offer, from `OPENECRAFT_MCP_TOOLS` (comma separated; `parts_*`
/// for all tools starting with `parts_`). Some clients take only a limited number
/// of tools.
pub fn tools_from_env() -> Vec<String> {
    std::env::var("OPENECRAFT_MCP_TOOLS").unwrap_or_default().split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
}

/// Folder the files of the MCP tools must be in, from `OPENECRAFT_MCP_ROOT` (`*`:
/// anywhere); default the working folder. An agent misled by text it reads cannot
/// read or overwrite files elsewhere (other projects, settings, plugin folders).
pub fn root_from_env() -> Option<PathBuf> {
    match std::env::var("OPENECRAFT_MCP_ROOT").unwrap_or_default().trim() {
        "*" => None,
        "" => std::env::current_dir().ok(),
        dir => Some(PathBuf::from(dir)),
    }
}

/// Parameters of the tools that name files or folders.
const PATH_PARAMS: [&str; 8] = ["path", "output", "folder", "database", "target", "template", "macros", "logo"];

/// Parameters of the tools that name several files, separated by `;`.
const PATH_LIST_PARAMS: [&str; 1] = ["files"];

/// Parameters that are ids of the vault or a format, not files (tool, parameter).
const NOT_PATHS: [(&str, &str); 3] = [("vault_browse", "folder"), ("vault_upload", "folder"), ("convert_project", "format")];

/// The absolute path without `.` and `..`, its existing part with symbolic links
/// resolved.
fn resolve(path: &Path) -> PathBuf {
    let full = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().unwrap_or_default().join(path) };
    let mut out = PathBuf::new();
    for c in full.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    let mut missing = Vec::new();
    let mut existing = out.as_path();
    while !existing.exists() {
        let (Some(name), Some(parent)) = (existing.file_name(), existing.parent()) else { break };
        missing.push(name.to_owned());
        existing = parent;
    }
    let mut resolved = existing.canonicalize().unwrap_or_else(|_| existing.to_path_buf());
    resolved.extend(missing.iter().rev());
    resolved
}

/// The file and folder parameters of a call of the tool `tool` lie in `root`.
fn confine(root: &Path, tool: &str, args: &Map<String, Value>) -> Result<(), String> {
    let root = resolve(root);
    let named = args.iter().filter(|(name, _)| !NOT_PATHS.contains(&(tool, name.as_str())));
    let paths = named.filter_map(|(name, value)| Some((name.as_str(), value.as_str()?))).flat_map(|(name, value)| {
        let list = PATH_LIST_PARAMS.contains(&name);
        let values: Vec<&str> = if list { value.split(';').collect() } else if PATH_PARAMS.contains(&name) { vec![value] } else { Vec::new() };
        values.into_iter().map(move |v| (name, v))
    });
    for (name, path) in paths {
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        // Built-in templates are names, not files.
        let file = path.contains(['/', '\\']) || [".oec", ".oecp", ".oecz"].iter().any(|e| path.to_lowercase().ends_with(e));
        if name == "template" && !file {
            continue;
        }
        if !resolve(Path::new(path)).starts_with(&root) {
            return Err(format!("{path} is outside {}, the folder this MCP server may use (OPENECRAFT_MCP_ROOT)", root.display()));
        }
    }
    Ok(())
}

/// True if the MCP server offers the tool `name`.
fn offered(session: &Session, name: &str) -> bool {
    session.mcp_tools.is_empty()
        || session.mcp_tools.iter().any(|p| match p.strip_suffix('*') {
            Some(prefix) => name.starts_with(prefix),
            None => p == name,
        })
}

pub fn serve(session: &mut Session, input: impl BufRead, mut output: impl Write) -> Result<(), String> {
    for line in input.lines() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(session, &msg),
            Err(e) => Some(error(Value::Null, -32700, &format!("parse error: {e}"))),
        };
        if let Some(reply) = reply {
            writeln!(output, "{reply}").and_then(|_| output.flush()).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

/// Answer to one message; `None` for notifications and responses.
pub fn handle(session: &mut Session, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let Some(method) = msg.get("method").and_then(Value::as_str) else {
        // The server sends no requests, so responses of the client need no answer.
        let response = msg.get("result").is_some() || msg.get("error").is_some();
        return (!response).then(|| error(id.unwrap_or(Value::Null), -32600, "invalid request"));
    };
    let id = id?;
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools::entries(session).iter().filter(|t| offered(session, t.name())).map(describe).collect::<Vec<_>>() })),
        "tools/call" => call(session, &params),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => error(id, code, &message),
    })
}

fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str).unwrap_or_default();
    let version = SUPPORTED.into_iter().find(|v| *v == asked).unwrap_or(PROTOCOL_VERSION);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": { "name": "openecraft", "title": "OpenECraft", "version": env!("CARGO_PKG_VERSION") },
        "instructions": INSTRUCTIONS,
    })
}

fn describe(tool: &tools::Entry) -> Value {
    json!({ "name": tool.name(), "description": tool.help(), "inputSchema": tools::schema(tool.clone()) })
}

fn call(session: &mut Session, params: &Value) -> Result<Value, (i64, String)> {
    let name = params.get("name").and_then(Value::as_str).ok_or((-32602, "missing tool name".to_string()))?;
    let tool = tools::lookup(session, name).filter(|_| offered(session, name)).ok_or_else(|| (-32602, format!("unknown tool: {name}")))?;
    let args = match params.get("arguments") {
        None | Some(Value::Null) => Map::new(),
        Some(Value::Object(map)) => map.clone(),
        Some(_) => return Err((-32602, "arguments must be an object".into())),
    };
    // Tool errors are results the agent can react to, not protocol errors.
    let allowed = session.mcp_root.as_deref().map_or(Ok(()), |root| confine(root, name, &args));
    let (text, is_error) = match allowed.and_then(|_| tools::call(session, tool, &args)) {
        Ok(Value::String(text)) => (text, false),
        Ok(mut value) => {
            tools::tidy(&mut value);
            (serde_json::to_string_pretty(&value).unwrap_or_default(), false)
        }
        Err(e) => (e, true),
    };
    Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }))
}

#[cfg(test)]
mod tests;
