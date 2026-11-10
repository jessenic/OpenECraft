use super::*;
use std::path::Path;

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(rust_files(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    out
}

fn all_rust_files() -> Vec<PathBuf> {
    source_dirs().iter().flat_map(|d| rust_files(d)).collect()
}

fn display(path: &Path) -> String {
    path.strip_prefix(root()).unwrap_or(path).display().to_string().replace('\\', "/")
}

fn is_test_file(path: &Path) -> bool {
    path.file_name().and_then(|n| n.to_str()) == Some("tests.rs")
}

/// Unit tests live in `<crate>/src/<module>/tests.rs`, never inline in the module file.
#[test]
fn unit_tests_are_in_separate_files() {
    let mut violations = Vec::new();
    for path in all_rust_files().into_iter().filter(|p| !is_test_file(p)) {
        let text = std::fs::read_to_string(&path).unwrap();
        let name = display(&path);
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim();
            if t.starts_with("//") {
                continue;
            }
            if t.starts_with("#[test]") || t.starts_with("mod tests {") || t.starts_with("mod tests{") {
                violations.push(format!("{name}:{}: inline test code – move it to {}", i + 1, tests_path(&name)));
            }
            if t == "mod tests;" && (i == 0 || lines[i - 1].trim() != "#[cfg(test)]") {
                violations.push(format!("{name}:{}: `mod tests;` must be preceded by `#[cfg(test)]`", i + 1));
            }
        }
    }
    assert!(violations.is_empty(), "test layout violations (see AGENTS.md):\n{}", violations.join("\n"));
}

/// Every `tests.rs` belongs to a module file next to its directory (`foo.rs` + `foo/tests.rs`).
#[test]
fn test_files_belong_to_a_module() {
    let mut orphans = Vec::new();
    for path in all_rust_files().into_iter().filter(|p| is_test_file(p)) {
        let dir = path.parent().unwrap();
        let module_file = dir.with_extension("rs");
        let declared = std::fs::read_to_string(&module_file)
            .map(|t| t.lines().any(|l| l.trim() == "mod tests;"))
            .unwrap_or(false);
        if !declared {
            orphans.push(format!(
                "{} is not declared via `#[cfg(test)] mod tests;` in {}",
                display(&path),
                display(&module_file)
            ));
        }
    }
    assert!(orphans.is_empty(), "{}", orphans.join("\n"));
}

fn tests_path(module_file: &str) -> String {
    format!("{}/tests.rs", module_file.trim_end_matches(".rs"))
}

#[test]
fn all_workspace_crates_are_checked() {
    let dirs = source_dirs();
    for member in ["openecraft/src", "crates/openecraft-parts/src", "crates/openecraft-ui/src", "parts-server/src", "parts-manager/src"] {
        assert!(dirs.iter().any(|d| d.ends_with(member)), "{member} missing in {dirs:?}");
    }
}

/// UI crates (the others have no user-visible texts of their own).
fn ui_source_files() -> Vec<PathBuf> {
    ["openecraft/src", "crates/openecraft-ui/src", "parts-manager/src"]
        .iter()
        .flat_map(|d| rust_files(&root().join(d)))
        .filter(|p| !is_test_file(p))
        .collect()
}

/// Texts shown to the user go through `tr` / `tr_fmt!` (see AGENTS.md, languages): a
/// widget call with a plain string literal is an untranslated text.
#[test]
fn ui_texts_are_translated() {
    let calls = [
        ".button(\"", ".small_button(\"", ".label(\"", ".heading(\"", ".weak(\"", ".strong(\"", ".link(\"", ".menu_button(\"",
        ".collapsing(\"", ".on_hover_text(\"", ".hint_text(\"", ".selectable_label(", ".selected_text(\"", ".hyperlink_to(\"",
        "Button::new(\"", "RichText::new(\"", "Window::new(\"",
    ];
    let mut violations = Vec::new();
    for path in ui_source_files() {
        let text = std::fs::read_to_string(&path).unwrap();
        for (i, line) in text.lines().enumerate() {
            let t = line.trim();
            if t.starts_with("//") {
                continue;
            }
            for call in calls {
                let mut rest = t;
                while let Some(at) = rest.find(call) {
                    let after = &rest[at + call.len()..];
                    rest = after;
                    if !call.ends_with('"') {
                        continue;
                    }
                    let literal: String = after.chars().take_while(|c| *c != '"').collect();
                    let letters = literal.chars().filter(|c| c.is_alphabetic()).count();
                    if letters >= 3 {
                        violations.push(format!("{}:{}: {t}", display(&path), i + 1));
                    }
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "texts shown in the UI must be translated: wrap them in `tr(\"…\")` / `tr_fmt!(…)` and add them to crates/openecraft-i18n/src/catalog/<area>/required.tsv (English, German, French, Spanish; `cargo run -p openecraft-i18n --example tidy` places and sorts them):\n{}",
        violations.join("\n")
    );
}

/// Dependency rules inside `openecraft-core` (see `docs/architecture.md`): a module may
/// refer only to the modules listed for it, which pins the current graph so that no new
/// edge appears unnoticed. Remove an entry from a list when you take a dependency away;
/// add one only with a reason. The layers are the target order; entries that point
/// upwards are the known exceptions.
const CORE_RULES: &[(&str, &[&str])] = &[
    // 1 foundations
    ("geom", &[]),
    ("xml", &[]),
    ("zip", &[]),
    ("xlsx", &["xml", "zip"]),
    ("params", &[]),
    ("table", &["params"]),
    ("issue", &[]),
    ("ttf", &[]),
    ("parallel", &[]),
    // 2 symbols
    ("symbols", &["geom"]),
    ("symlib", &["geom", "model", "symbols"]),
    // 3 the project file format (data only)
    ("model", &["geom", "symbols", "symlib"]),
    // 4 evaluation of the model
    ("index", &["frame", "geom", "model", "structure", "symbols"]),
    ("netlist", &["geom", "harness", "issue", "model", "panel", "potentials", "strips", "symbols", "symlib", "wirenum"]),
    ("structure", &["geom", "model", "ops"]),
    ("sections", &["model"]),
    ("frame", &["geom", "model"]),
    ("text", &["frame", "geom", "model", "symbols"]),
    ("numbering", &["frame", "index", "model", "ops", "sections", "structure"]),
    ("arrange", &["geom", "model", "ops", "routing", "symlib"]),
    ("search", &["model", "symbols"]),
    ("unplaced", &["model", "parts", "plc", "plc_hw", "symbols"]),
    ("mounting", &["model"]),
    ("wirenum", &["frame", "model", "netlist"]),
    ("history", &["model"]),
    ("translation", &["model"]),
    ("options", &["model"]),
    ("potentials", &["model", "netlist", "wirenum"]),
    ("functions", &["model", "parts", "properties", "props", "symbols"]),
    ("properties", &["frame", "functions", "model", "parts", "props", "references", "sections", "structure", "symbols"]),
    ("props", &["geom", "model", "properties", "symbols", "symlib"]),
    // 5 feature logic and page editing
    ("ops", &["device_boxes", "electrofluid", "geom", "graphics", "harness", "model", "plc_hw", "sections", "structure", "symbols", "symlib", "text"]),
    ("panel", &["climate", "electrofluid", "geom", "harness", "issue", "model", "netlist", "parts", "production", "strips", "symbols"]),
    ("climate", &["model", "panel", "parts"]),
    ("manufacturing", &["geom", "model", "panel", "parts", "production"]),
    ("routing", &["cabledef", "conndef", "frame", "geom", "model", "netlist", "ops", "symbols", "symlib"]),
    ("routing3d", &["fluid", "geom", "harness", "model", "netlist", "panel", "parallel", "parts", "plc_hw", "potentials"]),
    ("field", &["geom", "issue", "model", "netlist", "panel", "parts", "production", "render", "routing3d", "structure", "svg", "symbols"]),
    ("strips", &["frame", "model", "netlist", "parts", "symbols", "text"]),
    ("plugs", &["frame", "model", "netlist", "parts", "symbols", "text"]),
    ("pinout", &["geom", "issue", "model", "netlist", "parts", "plugs", "potentials", "render", "structure", "svg", "symbols"]),
    ("harness", &["analysis", "geom", "model", "netlist", "plugs", "production", "symbols"]),
    ("nailboard", &["analysis", "geom", "harness", "model", "render", "svg", "symbols"]),
    ("marking", &["index", "model", "netlist", "panel", "strips", "structure", "xlsx"]),
    ("fluid", &["issue", "model", "netlist", "panel", "symbols", "wirenum"]),
    ("references", &["index", "model", "structure", "symbols"]),
    ("electrofluid", &["fluid", "index", "issue", "model", "references", "structure", "symbols"]),
    ("process", &["index", "model", "symbols"]),
    ("plc", &["frame", "model", "netlist", "plc_hw", "symbols", "symlib", "text"]),
    ("plc_hw", &["devdesc", "issue", "model", "ops", "parts", "plc"]),
    ("devdesc", &["model", "parts", "xml"]),
    ("electrical", &["model", "netlist", "parts", "potentials", "protection", "sizing"]),
    ("protection", &["electrical", "model", "parts", "sizing"]),
    ("sizing", &["issue", "model", "netlist", "parts", "potentials", "production", "rules", "symbols", "wirenum"]),
    ("shortcircuit", &["electrical", "issue", "loads", "model", "netlist", "parts", "production", "protection", "rules", "sizing"]),
    ("loads", &["electrical", "issue", "model", "netlist", "parts", "potentials", "rules", "sizing", "symbols"]),
    ("emc", &["electrical", "issue", "model", "netlist", "parts", "potentials", "routing3d", "rules"]),
    ("safety", &["issue", "model", "sizing"]),
    ("rules", &["electrical", "issue", "model", "netlist", "potentials", "production", "routing3d", "sizing", "symbols"]),
    ("singleline", &["fluid", "geom", "index", "issue", "model", "netlist", "ops", "structure", "symbols"]),
    ("snap_lines", &["geom", "model"]),
    ("cabledef", &["geom", "issue", "model", "ops", "structure"]),
    ("structure_boxes", &["geom", "issue", "model", "ops", "structure", "symbols"]),
    ("conndef", &["geom", "issue", "model", "netlist", "wirenum"]),
    ("wiring", &["model", "panel", "production"]),
    ("production", &["dxf", "geom", "model", "panel", "parts"]),
    ("parts", &["model", "symbols"]),
    ("tables", &["index", "model", "netlist", "ops", "parts", "plc", "plugs", "strips", "symbols", "wirenum"]),
    ("revision", &["compare", "geom", "model", "review", "structure"]),
    ("review", &["geom", "model", "render", "structure", "symbols"]),
    ("compare", &["model", "panel", "revision", "structure"]),
    ("config", &["macros", "model"]),
    ("macros", &["model", "ops", "symbols", "symlib"]),
    ("page_templates", &["model", "ops", "structure"]),
    ("subproject", &["model", "options", "structure", "symlib"]),
    ("graphics", &["document", "geom", "model"]),
    ("document", &["frame", "geom", "model", "ops", "render", "symbols"]),
    ("forms", &["frame", "geom", "model", "params", "properties", "render", "revision", "structure", "symbols", "table"]),
    ("report_blocks", &["analysis", "ext", "forms", "model", "ops", "reports", "strips"]),
    ("testkit", &["analysis", "geom", "model", "parts", "symbols"]),
    // 6 derived data, checks, reports, registry
    ("analysis", &["ext", "field", "index", "issue", "model", "netlist", "panel", "parallel", "revision", "routing3d", "table"]),
    ("checks", &["analysis", "cabledef", "climate", "conndef", "electrofluid", "emc", "ext", "fluid", "frame", "harness", "index", "integration", "issue", "loads", "model", "mounting", "netlist", "panel", "parts", "pinout", "plc", "plc_hw", "plugs", "potentials", "preplanning", "process", "production", "reports", "routing3d", "rules", "safety", "shortcircuit", "singleline", "sizing", "strips", "structure", "structure_boxes", "symbols", "translation", "wirenum", "wiring"]),
    ("reports", &["analysis", "checks", "climate", "electrofluid", "ext", "field", "fluid", "frame", "geom", "harness", "index", "issue", "loads", "marking", "model", "netlist", "panel", "params", "parts", "plc", "plc_hw", "plugs", "process", "production", "revision", "routing3d", "rules", "safety", "shortcircuit", "sizing", "strips", "symbols", "table", "text"]),
    ("ext", &["analysis", "issue", "model", "params", "table"]),
    ("exporters", &["aml", "analysis", "dwg", "dxf", "export3d", "ext", "harness", "harness_export", "kbl", "marking", "model", "nailboard", "page_select", "params", "pdf", "pinout", "plc", "plugs", "reports", "svg", "translation", "xlsx"]),
    // 7 output and exchange formats
    ("render", &["analysis", "cabledef", "conndef", "device_boxes", "document", "electrofluid", "forms", "frame", "geom", "graphics", "index", "macro_boxes", "model", "netlist", "panel", "parts", "potentials", "production", "props", "references", "report_blocks", "review", "revision", "structure", "structure_boxes", "symbols", "text", "wirenum"]),
    ("svg", &["analysis", "frame", "geom", "model", "render", "symbols", "text"]),
    ("pdf", &["analysis", "document", "frame", "geom", "model", "render", "structure", "symbols", "text", "ttf"]),
    ("pdf_comments", &["geom", "model", "review"]),
    ("page_select", &["model", "structure"]),
    ("settings_scheme", &["model"]),
    ("macro_boxes", &["geom", "macros", "model", "ops"]),
    ("device_boxes", &["geom", "model", "ops", "structure", "symbols"]),
    ("preplanning", &["index", "issue", "macros", "model", "structure"]),
    ("dxf", &["analysis", "frame", "geom", "graphics", "model", "render", "symbols", "text"]),
    ("dxf_import", &["frame", "geom", "model", "ops", "symbols"]),
    ("dwg", &["analysis", "dxf", "dxf_import", "model", "panel", "production"]),
    ("export3d", &["document", "model", "panel", "parts"]),
    ("kbl", &["analysis", "harness", "model", "netlist", "parts", "plugs", "production", "routing3d", "xml", "zip"]),
    ("harness_export", &["analysis", "export3d", "harness", "model", "plugs", "production"]),
    ("aml", &["devdesc", "model", "netlist", "parts", "plc", "plc_hw", "xml"]),
    ("integration", &["analysis", "issue", "model", "netlist", "panel", "parts", "wiring"]),
    // 8 project files, sharing, verification
    ("collab", &["model", "review", "report_blocks"]),
    ("archive", &["document", "model", "pdf", "zip"]),
    ("projects", &["archive", "index", "issue", "model", "properties", "verify"]),
    ("project_templates", &["model", "structure"]),
    ("verify", &["analysis", "issue", "model", "options", "properties", "structure", "symbols"]),
    ("sample", &["electrofluid", "geom", "harness", "model", "netlist", "ops", "panel", "parts", "pinout", "potentials", "review", "revision", "sections", "sizing", "structure", "symbols", "wirenum"]),
];

#[test]
fn core_modules_respect_the_layer_rules() {
    let deps = core_module_dependencies();
    let mut violations = Vec::new();
    for (module, allowed) in CORE_RULES {
        let Some(found) = deps.get(*module) else {
            violations.push(format!("{module}: no such module in openecraft-core (update CORE_RULES)"));
            continue;
        };
        for dep in found {
            if !allowed.contains(&dep.as_str()) {
                violations.push(format!("{module} must not refer to crate::{dep}"));
            }
        }
    }
    assert!(violations.is_empty(), "layer rule violations in openecraft-core (see docs/architecture.md):
{}", violations.join("
"));
}

#[test]
fn crate_references_are_parsed_from_paths_and_lists() {
    let refs = crate_references("use crate::geom::P;
use crate::{model, netlist::Netlist, self};
let x = crate::panel::snap(1.0);");
    let v: Vec<&str> = refs.iter().map(String::as_str).collect();
    assert_eq!(v, ["geom", "model", "netlist", "panel"]);
}

/// The editor names core modules as `openecraft_core::…` instead of re-exporting
/// them as `crate::…`, so that UI code and model code stay distinguishable.
#[test]
fn the_editor_names_core_modules_explicitly() {
    let main = std::fs::read_to_string(root().join("openecraft/src/main.rs")).unwrap();
    let offending: Vec<&str> = main.lines().filter(|l| l.trim_start().starts_with("use openecraft_core::") || l.trim_start().starts_with("pub use openecraft_core::")).collect();
    assert!(offending.is_empty(), "main.rs must not re-export core modules:
{}", offending.join("
"));
}

/// Text files of the repository have no UTF-8 byte order mark: tools reading the
/// manifests or sources (TOML parsers, scripts) need not cope with one.
#[test]
fn text_files_have_no_byte_order_mark() {
    let mut files: Vec<PathBuf> = vec![root().join("Cargo.toml")];
    for dir in source_dirs() {
        let krate = dir.parent().unwrap().to_path_buf();
        files.push(krate.join("Cargo.toml"));
        files.extend(rust_files(&dir));
    }
    for f in ["README.md", "AGENTS.md", ".claude/launch.json"] {
        files.push(root().join(f));
    }
    let marked: Vec<String> = files.iter().filter(|f| std::fs::read(f).is_ok_and(|b| b.starts_with(b"\xef\xbb\xbf"))).map(|f| display(f)).collect();
    assert!(marked.is_empty(), "remove the byte order mark of {marked:?}");
}

/// `mkdir` fails on a directory of the checkout, and a shared one would add its files to
/// the release.
#[test]
fn the_release_workflow_collects_files_in_a_new_directory() {
    let workflow = std::fs::read_to_string(root().join(".github/workflows/release.yml")).unwrap();
    let dirs: Vec<&str> = workflow.lines().filter_map(|l| l.trim().strip_prefix("mkdir ")).collect();
    assert!(!dirs.is_empty(), "no mkdir found in the release workflow");
    for dir in dirs {
        assert!(!root().join(dir).exists(), "{dir} is a directory of the checkout");
    }
}

/// `snap pack` refuses licenses missing in the list of snapd, which fails the release.
#[test]
fn the_snap_names_only_licenses_snapd_knows() {
    let manifest = std::fs::read_to_string(root().join("installer/linux/snap/snapcraft.yaml")).unwrap();
    let license = manifest.lines().find(|l| l.starts_with("license:")).expect("license of the snap");
    for unknown in ["Ubuntu-font-1.0", "Bitstream-Vera"] {
        assert!(!license.contains(unknown), "snapd does not know {unknown}");
    }
}
