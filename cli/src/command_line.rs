//! One call of the command line: options, help, version and one command. Every
//! change is written back to `--project` at once; the MCP server (`mcp`) keeps the
//! project in memory instead.

use crate::session::Session;
use crate::{args, mcp, parts_server, tools, vault};
use openecraft_i18n::{tr, tr_fmt, Lang};

/// Tools that only make sense in an MCP session, which keeps the project in memory.
const SESSION_ONLY: [&str; 2] = ["undo", "redo"];

/// Runs the command line `argv` (without the program name); returns what it prints.
pub fn run(argv: &[String]) -> Result<String, String> {
    let inv = args::parse(argv)?;
    let lang = inv.options.lang.as_deref();
    if let Some(code) = lang {
        let lang = Lang::from_code(code).ok_or_else(|| tr_fmt!("Unknown language: {}", code))?;
        openecraft_i18n::set_lang(lang);
    }
    if inv.options.version {
        return Ok(version());
    }
    let command = inv.command.as_deref().unwrap_or("help");
    if lang.is_none() && command != "mcp" {
        openecraft_i18n::init(None);
    }
    let mut session = Session::new(parts_server::Server::from_options(&inv.options));
    session.vault = vault::Vault::from_env();
    session.plugins.load_env();
    if let Some(dir) = &inv.options.plugins {
        session.plugins.load_dir(std::path::Path::new(dir));
    }
    for e in &session.plugins.errors {
        eprintln!("{e}");
    }
    match command {
        "help" => match inv.words.first().map(String::as_str) {
            None | Some("help") => Ok(usage(&session)),
            Some("mcp") => Ok(tr("openecraft-cli mcp\n\nRuns a Model Context Protocol server on stdin / stdout: the tools of this list for AI agents, with the project kept in memory (save_project writes it, undo / redo revert single tool calls). OPENECRAFT_MCP_TOOLS limits the tools offered.\n").to_string()),
            Some("version") => Ok(tr("openecraft-cli version\n\nShows the version of the command line (also --version or -V)\n").to_string()),
            Some(name) => Ok(tools::help(tools::lookup(&session, name).ok_or_else(|| unknown(name))?)),
        },
        "mcp" => {
            session.mcp_tools = mcp::tools_from_env();
            session.mcp_root = mcp::root_from_env();
            mcp::serve(&mut session, std::io::stdin().lock(), std::io::stdout().lock())?;
            Ok(String::new())
        }
        "version" => Ok(version()),
        name => {
            let tool = tools::lookup(&session, name).ok_or_else(|| unknown(name))?;
            if let Some(word) = inv.words.first() {
                return Err(tr_fmt!("Unexpected argument: {}", word));
            }
            if inv.options.help {
                return Ok(tools::help(tool));
            }
            if SESSION_ONLY.contains(&name) {
                return Err(tr("undo and redo work in an MCP session only: the command line opens the project anew for every command").to_string());
            }
            let json = read_json_argument(inv.options.json.as_deref())?;
            let params = tools::from_cli(tool.clone(), json.as_deref(), &inv.params)?;
            if tool.needs_project() {
                let path = inv.options.project.as_deref().ok_or_else(|| tr("This command needs a project: --project <file>").to_string())?;
                session.open(path)?;
            }
            let mut result = tools::call(&mut session, tool, &params)?;
            session.write_back()?;
            tools::tidy(&mut result);
            Ok(match result {
                serde_json::Value::String(text) => format!("{text}\n"),
                value => format!("{}\n", serde_json::to_string_pretty(&value).unwrap_or_default()),
            })
        }
    }
}

fn version() -> String {
    format!("openecraft-cli {}\n", env!("CARGO_PKG_VERSION"))
}

fn read_json_argument(value: Option<&str>) -> Result<Option<String>, String> {
    value.map(|value| match value.strip_prefix('@') {
        Some(path) => std::fs::read_to_string(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e))),
        None => Ok(value.to_owned()),
    }).transpose()
}

fn unknown(name: &str) -> String {
    tr_fmt!("Unknown command: {}. Run openecraft-cli help for a list.", name)
}

fn usage(session: &Session) -> String {
    let mut out = tr(
        "Usage: openecraft-cli [--project <file>] [--server <url>] [--token <token>] [--lang <code>] [--plugins <folder>] <command> [--<parameter> <value>]…\n\nopenecraft-cli mcp        runs a Model Context Protocol server on stdin / stdout\nopenecraft-cli <command> --help   shows the parameters of a command\n\nCommands:\n",
    )
    .to_string();
    let entries = tools::entries(session);
    // The descriptions start in one column, after the longest name.
    let width = entries.iter().map(|t| t.name().chars().count()).max().unwrap_or(0) + 2;
    for tool in entries {
        let mark = if SESSION_ONLY.contains(&tool.name()) { format!(" {}", tr("(MCP server only)")) } else { String::new() };
        out.push_str(&format!("  {:<width$}{}{mark}\n", tool.name(), tool.help_text()));
    }
    out
}

#[cfg(test)]
mod tests;
