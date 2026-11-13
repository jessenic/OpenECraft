//! Project management tools: project database, batch changes of project properties,
//! subprojects, model options, archives, version conversion and verification.

use super::{optional, param, Args, Kind, Tool};
use crate::session::Session;
use openecraft_core::archive::{self, ArchiveOptions, ARCHIVE_EXT};
use openecraft_core::projects::{self, Batch, Database, Entry};
use openecraft_core::storage::{folder, merge, INDEX_EXT};
use openecraft_core::subproject;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use openecraft_api::builtin::management::{page_list};

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "index_projects",
        help: tr_const!("Reads the project files and archives of a folder (and its subfolders) into the project database"),
        params: &[
            param("folder", Kind::Str, tr_const!("Folder with project files")),
            optional("database", Kind::Str, tr_const!("Project database file (JSON) to update")),
            optional("verify", Kind::Bool, tr_const!("Also verify each project (slower)")),
        ],
        needs_project: false,
        run: index_projects,
    },
    Tool {
        name: "search_projects",
        help: tr_const!("Finds projects in the project database or a folder: terms separated by ';', each key=value, key~text or plain text"),
        params: &[
            optional("query", Kind::Str, tr_const!("Search terms, e.g. company=ACME; customer~plant; conveyor")),
            optional("database", Kind::Str, tr_const!("Project database file (JSON)")),
            optional("folder", Kind::Str, tr_const!("Folder with project files (instead of a database; default: the current folder)")),
        ],
        needs_project: false,
        run: search_projects,
    },
    Tool {
        name: "batch_project_properties",
        help: tr_const!("Changes project properties in many project files at once (fields name, description, company, creator, date, revision or project properties)"),
        params: &[
            optional("files", Kind::Str, tr_const!("Project files, separated by ';'")),
            optional("folder", Kind::Str, tr_const!("All project files of a folder")),
            optional("query", Kind::Str, tr_const!("Only the projects that match these search terms")),
            optional("set", Kind::Object, tr_const!("Values to set (name: value; an empty value removes a property)")),
            optional("remove", Kind::Str, tr_const!("Properties to remove, separated by ','")),
            optional("replace", Kind::Str, tr_const!("Field or property whose value is searched ('*' for all)")),
            optional("find", Kind::Str, tr_const!("Text to replace")),
            optional("with", Kind::Str, tr_const!("Replacement text")),
            optional("rename", Kind::Object, tr_const!("Properties to rename (old name: new name)")),
            optional("dry_run", Kind::Bool, tr_const!("Only show the changes, write nothing")),
        ],
        needs_project: false,
        run: batch_project_properties,
    },
    Tool {
        name: "archive_project",
        help: tr_const!("Writes the project into an archive (.oecz: project, manifest, embedded files, optionally a PDF)"),
        params: &[
            param("path", Kind::Str, tr_const!("Archive file (.oecz)")),
            optional("comment", Kind::Str, tr_const!("Comment stored in the archive")),
            optional("pdf", Kind::Bool, tr_const!("Add a PDF of all pages")),
            optional("compact", Kind::Bool, tr_const!("Leave out stored baselines and wiring marks")),
        ],
        needs_project: true,
        run: archive_project,
    },
    Tool {
        name: "archive_info",
        help: tr_const!("Shows the manifest of a project archive"),
        params: &[param("path", Kind::Str, tr_const!("Archive file (.oecz)"))],
        needs_project: false,
        run: archive_info,
    },
    Tool {
        name: "convert_project",
        help: tr_const!("Converts a project to the current file format, or to another way of storing it: project file, split project (a folder for version control) or archive"),
        params: &[
            param("path", Kind::Str, tr_const!("Project file (.oec), index of a split project (.oecp) or archive (.oecz)")),
            optional("output", Kind::Str, tr_const!("Project file to write (default: the file itself; archives: next to it as .oec)")),
            optional("format", Kind::Str, tr_const!("What to write: oec (project file), oecp (split project in a folder of its own) or oecz (archive); default: by the extension of the output")),
        ],
        needs_project: false,
        run: convert_project,
    },
    Tool {
        name: "merge_project_file",
        help: tr_const!("Merges two versions of a file of a split project record by record (the merge driver of git): the result replaces the file of this side; where both sides differ its values stay and the command fails"),
        params: &[
            param("base", Kind::Str, tr_const!("File of the common ancestor (%O of git)")),
            param("ours", Kind::Str, tr_const!("File of this side, replaced by the result (%A of git)")),
            param("theirs", Kind::Str, tr_const!("File of the other side (%B of git)")),
            param("name", Kind::Str, tr_const!("Path of the file in the project (%P of git): its name says how it is merged")),
        ],
        needs_project: false,
        run: merge_project_file,
    },
    Tool {
        name: "extract_subproject",
        help: tr_const!("Takes pages out as a subproject file; they stay locked here until the subproject is inserted again"),
        params: &[
            param("name", Kind::Str, tr_const!("Subproject name")),
            param("path", Kind::Str, tr_const!("Subproject file to write (.oec)")),
            optional("pages", Kind::Str, tr_const!("Page names, separated by ','")),
            optional("structure", Kind::Str, tr_const!("Pages whose structure starts with this, e.g. =A1+CAB1")),
            optional("description", Kind::Str, tr_const!("Description")),
        ],
        needs_project: true,
        run: extract_subproject,
    },
    Tool {
        name: "insert_subproject",
        help: tr_const!("Inserts an edited subproject: its pages replace the pages taken out"),
        params: &[param("path", Kind::Str, tr_const!("Subproject file (.oec)"))],
        needs_project: true,
        run: insert_subproject,
    },
];

/// Ways to store a project (`convert_project`).
const FORMATS: [&str; 3] = ["oec", INDEX_EXT, ARCHIVE_EXT];

fn today() -> String {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    openecraft_parts::date_from_unix(secs)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path.display(), openecraft_i18n::io_error(&e)))
}

fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| tr_fmt!("Cannot write {}: {}", path.display(), openecraft_i18n::io_error(&e)))
}

/// Project files of a folder and its subfolders.
fn project_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    let mut dirs = vec![folder.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        let entries = std::fs::read_dir(&dir).map_err(|e| tr_fmt!("Cannot read {}: {}", dir.display(), openecraft_i18n::io_error(&e)))?;
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                dirs.push(path);
            } else if projects::is_project_file(&path.to_string_lossy()) {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn scan(folder: &str, verify: bool) -> Result<Vec<Entry>, String> {
    Ok(project_files(Path::new(folder))?.iter().map(|p| Entry::load(&p.display().to_string(), verify)).collect())
}

fn index_projects(_: &mut Session, a: &Args) -> Result<Value, String> {
    let folder = a.req("folder");
    let found = scan(folder, a.flag("verify"))?;
    let mut db = match a.str("database") {
        Some(path) if Path::new(path).exists() => Database::from_json(&String::from_utf8_lossy(&read(Path::new(path))?))?,
        _ => Database::default(),
    };
    db.update_folder(&Path::new(folder).display().to_string(), found.clone());
    if let Some(path) = a.str("database") {
        write(Path::new(path), db.to_json().as_bytes())?;
    }
    Ok(json!({ "projects": found, "total": db.entries.len() }))
}

fn search_projects(_: &mut Session, a: &Args) -> Result<Value, String> {
    let db = match (a.str("database"), a.str("folder")) {
        (Some(path), _) => Database::from_json(&String::from_utf8_lossy(&read(Path::new(path))?))?,
        (None, folder) => {
            let folder = folder.unwrap_or(".");
            Database { folders: vec![folder.into()], entries: scan(folder, false)? }
        }
    };
    Ok(json!(db.search(a.str("query").unwrap_or_default())))
}

fn batch_project_properties(_: &mut Session, a: &Args) -> Result<Value, String> {
    let mut b = Batch::default();
    if let Some(Value::Object(set)) = a.value("set") {
        for (k, v) in set {
            let v = match v {
                Value::String(s) => s.clone(),
                Value::Null => String::new(),
                other => other.to_string(),
            };
            b.set.insert(k.clone(), v);
        }
    }
    b.remove = a.str("remove").unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if let Some(key) = a.str("replace") {
        b.replace.push((key.into(), a.str("find").unwrap_or_default().into(), a.str("with").unwrap_or_default().into()));
    }
    if let Some(Value::Object(rename)) = a.value("rename") {
        b.rename = rename.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string())).collect();
    }
    if b.is_empty() {
        return Err(tr("Nothing to change: pass set, remove, replace or rename").into());
    }
    let mut files: Vec<PathBuf> = a.str("files").unwrap_or_default().split(';').map(str::trim).filter(|s| !s.is_empty()).map(PathBuf::from).collect();
    if let Some(folder) = a.str("folder") {
        files.extend(project_files(Path::new(folder))?);
    }
    if files.is_empty() {
        return Err(tr("Pass files or a folder").into());
    }
    let (query, dry, date) = (a.str("query").unwrap_or_default(), a.flag("dry_run"), today());
    let mut out = Vec::new();
    for path in files {
        let name = path.display().to_string();
        if !query.is_empty() && !Entry::load(&name, false).matches(query) {
            continue;
        }
        match b.apply_on_disk(&name, &date, !dry) {
            Ok((changes, written)) => out.push(json!({ "file": name, "changes": changes, "written": written.is_some() })),
            Err(e) => out.push(json!({ "file": name, "error": e })),
        }
    }
    Ok(json!(out))
}

fn archive_project(s: &mut Session, a: &Args) -> Result<Value, String> {
    let opts = ArchiveOptions { comment: a.str("comment").unwrap_or_default().into(), date: today(), pdf: a.flag("pdf"), compact: a.flag("compact") };
    let bytes = archive::archive(s.project()?, &opts);
    let path = Path::new(a.req("path"));
    write(path, &bytes)?;
    let m = archive::manifest(&bytes)?;
    Ok(json!({ "path": path.display().to_string(), "bytes": bytes.len(), "files": m.files }))
}

fn archive_info(_: &mut Session, a: &Args) -> Result<Value, String> {
    let m = archive::manifest(&read(Path::new(a.req("path")))?)?;
    Ok(serde_json::to_value(m).unwrap_or_default())
}

fn convert_project(_: &mut Session, a: &Args) -> Result<Value, String> {
    let path = Path::new(a.req("path"));
    let (prj, conv, is_archive) = crate::session::read_project(path)?;
    let format = a.str("format");
    if let Some(f) = format.filter(|f| !FORMATS.contains(f)) {
        return Err(tr_fmt!("Unknown format {}: give oec, oecp or oecz", f));
    }
    let output = match (a.str("output"), format) {
        (Some(o), _) => PathBuf::from(o),
        // A split project gets a folder of its own, named like the file.
        (None, Some(INDEX_EXT)) if !folder::is_index(path) => {
            let stem = path.file_stem().unwrap_or_default();
            let mut file = stem.to_owned();
            file.push(format!(".{INDEX_EXT}"));
            path.with_file_name(stem).join(file)
        }
        (None, Some(f)) => path.with_extension(f),
        (None, None) if is_archive => path.with_extension("oec"),
        (None, None) => path.to_path_buf(),
    };
    let by_extension = || output.extension().and_then(|e| e.to_str()).map(str::to_lowercase).unwrap_or_default();
    match format.map(String::from).unwrap_or_else(by_extension).as_str() {
        INDEX_EXT => {
            folder::save(&output, &prj)?;
        }
        ARCHIVE_EXT => write(&output, &archive::archive(&prj, &ArchiveOptions { date: today(), ..Default::default() }))?,
        _ => write(&output, prj.to_json().as_bytes())?,
    }
    Ok(json!({ "from": conv.from, "to": prj.format, "steps": conv.steps, "newer": conv.newer(), "output": output.display().to_string() }))
}

fn merge_project_file(_: &mut Session, a: &Args) -> Result<Value, String> {
    let (ours, name) = (Path::new(a.req("ours")), a.req("name"));
    let merged = merge::merge_file(name, &read(Path::new(a.req("base")))?, &read(ours)?, &read(Path::new(a.req("theirs")))?)?;
    write(ours, &merged.content)?;
    if merged.conflicts.is_empty() {
        Ok(json!({ "file": name, "conflicts": [] }))
    } else {
        Err(tr_fmt!("{}: {} conflicts, the values of this side were kept: {}", name, merged.conflicts.len(), merged.conflicts.join(", ")))
    }
}

fn extract_subproject(s: &mut Session, a: &Args) -> Result<Value, String> {
    let prj = s.project()?;
    let pages = subproject::select_pages(prj, &page_list(a), a.str("structure").unwrap_or_default())?;
    let mut edited = prj.clone();
    let sub = subproject::extract(&mut edited, a.req("name"), a.str("description").unwrap_or_default(), &pages, &today())?;
    let path = Path::new(a.req("path"));
    write(path, sub.to_json().as_bytes())?;
    *s.project_mut()? = edited;
    s.commit();
    Ok(json!({ "path": path.display().to_string(), "pages": sub.pages.iter().map(|p| p.name.clone()).collect::<Vec<_>>(), "devices": sub.devices.len() }))
}

fn insert_subproject(s: &mut Session, a: &Args) -> Result<Value, String> {
    let (sub, _, _) = crate::session::read_project(Path::new(a.req("path")))?;
    let r = subproject::insert(s.project_mut()?, &sub, &today())?;
    s.commit();
    Ok(json!({ "name": r.name, "pages_removed": r.pages_removed, "pages_inserted": r.pages_inserted, "devices": r.devices, "parts": r.parts, "renumbered": r.renumbered }))
}

#[cfg(test)]
mod tests;
