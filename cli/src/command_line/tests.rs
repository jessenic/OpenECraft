use super::*;
use crate::testing::temp_dir;

#[test]
fn json_parameters_can_be_read_from_a_file() {
    let dir = temp_dir("json-argument");
    let path = dir.join("parameters.json");
    std::fs::write(&path, r#"{"macro":"large portable drawing"}"#).unwrap();
    assert_eq!(read_json_argument(Some(&format!("@{}", path.display()))).unwrap(), Some(r#"{"macro":"large portable drawing"}"#.into()));
    assert_eq!(read_json_argument(Some("{} ")).unwrap(), Some("{} ".into()));
    assert!(read_json_argument(Some("@does-not-exist.json")).is_err());
}

/// Runs the command line in English (the tests do not follow the system language).
fn cli(words: &[&str]) -> Result<String, String> {
    let argv: Vec<String> = ["--lang", "en"].iter().chain(words).map(|s| s.to_string()).collect();
    run(&argv)
}

#[test]
fn version_and_help_of_the_session_words() {
    let v = format!("openecraft-cli {}\n", env!("CARGO_PKG_VERSION"));
    assert_eq!(cli(&["--version"]).unwrap(), v);
    assert_eq!(cli(&["-V"]).unwrap(), v);
    assert_eq!(cli(&["version"]).unwrap(), v);
    assert!(cli(&["help", "mcp"]).unwrap().starts_with("openecraft-cli mcp"));
    assert!(cli(&["help", "version"]).unwrap().contains("--version"));
    assert!(cli(&["help", "help"]).unwrap().starts_with("Usage:"));
}

#[test]
fn help_lists_the_commands_in_one_column() {
    let help = cli(&["help"]).unwrap();
    let starts: std::collections::BTreeSet<usize> = help
        .lines()
        .filter(|l| l.starts_with("  ") && !l.starts_with("   "))
        .map(|l| {
            let name_end = 2 + l[2..].find(' ').unwrap();
            name_end + l[name_end..].find(|c: char| c != ' ').unwrap()
        })
        .collect();
    assert_eq!(starts.len(), 1, "descriptions start in different columns: {starts:?}");
    assert!(help.lines().any(|l| l.trim_start().starts_with("undo") && l.ends_with("(MCP server only)")));
}

#[test]
fn undo_is_refused_on_the_command_line() {
    let dir = temp_dir("cli-undo");
    let file = dir.join("p.oec").display().to_string();
    cli(&["new_project", "--path", &file]).unwrap();
    let err = cli(&["-p", &file, "undo"]).unwrap_err();
    assert!(err.contains("MCP session only"), "{err}");
    // Its help is still shown.
    assert!(cli(&["undo", "--help"]).unwrap().contains("undo"));
}

#[test]
fn changes_of_an_archive_are_refused_not_lost() {
    let dir = temp_dir("cli-archive");
    let file = dir.join("p.oec").display().to_string();
    let archive = dir.join("p.oecz").display().to_string();
    cli(&["new_project", "--name", "Original", "--path", &file]).unwrap();
    cli(&["-p", &file, "archive_project", "--path", &archive]).unwrap();
    // Reading and exporting work.
    assert!(cli(&["-p", &archive, "list_pages"]).is_ok());
    // A change would not reach the archive: it is an error.
    let err = cli(&["-p", &archive, "set_project_info", "--name", "Changed"]).unwrap_err();
    assert!(err.contains("is an archive"), "{err}");
    let info = cli(&["archive_info", "--path", &archive]).unwrap();
    assert!(info.contains("Original"), "{info}");
    // Saving it as a project file is the way.
    let copy = dir.join("copy.oec").display().to_string();
    cli(&["-p", &archive, "save_project", "--path", &copy]).unwrap();
    assert!(std::path::Path::new(&copy).exists());
}

#[test]
fn numbers_are_printed_without_float_noise() {
    let dir = temp_dir("cli-numbers");
    let file = dir.join("p.oec").display().to_string();
    cli(&["new_project", "--example", "cabinet_lineup", "--path", &file]).unwrap();
    let strips = cli(&["-p", &file, "list_strips"]).unwrap();
    assert!(!strips.contains("99999"), "{strips}");
}
