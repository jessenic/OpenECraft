//! Project database: an index of project files (properties, size, check result) to
//! find projects by text or property value, and batch operations that change
//! project properties in many project files at once, in memory ([`Entry::read`],
//! [`Batch::apply_to_file`]) or on disk ([`Entry::load`], [`Batch::apply_on_disk`]).

use openecraft_i18n::{tr, tr_fmt};
use crate::archive;
use crate::model::Project;
use crate::properties::{self, Object};
use crate::storage::folder;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// File extensions of project files, archives and the index files of split projects.
pub const EXTENSIONS: [&str; 3] = ["oec", archive::ARCHIVE_EXT, crate::storage::INDEX_EXT];

/// Fields of the project information that batch operations and queries know by name.
pub const FIELDS: [&str; 6] = ["name", "description", "company", "creator", "date", "revision"];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub path: String,
    pub name: String,
    pub description: String,
    pub company: String,
    pub creator: String,
    pub date: String,
    pub revision: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub properties: BTreeMap<String, String>,
    pub pages: usize,
    pub devices: usize,
    pub format: u32,
    pub archive: bool,
    /// Subproject: the name of the main project.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub parent: String,
    /// Subprojects that are out.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subprojects_out: Vec<String>,
    /// Verified: errors and warnings of the verification and the checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified: Option<(usize, usize)>,
    /// Why the file could not be read.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub error: String,
}

impl Entry {
    /// Index entry of a project file (`verify`: also runs the verification).
    pub fn read(path: &str, bytes: &[u8], verify: bool) -> Entry {
        match archive::read_project(bytes) {
            Ok((prj, conv)) => {
                let mut e = Entry::of(path, &prj, verify);
                e.format = conv.from;
                e.archive = archive::is_archive(bytes);
                e
            }
            Err(error) => Entry { path: path.into(), error, ..Default::default() },
        }
    }

    /// Index entry of a file found on disk: a project file or archive, or the index
    /// of a split project (read with its folder).
    pub fn load(path: &str, verify: bool) -> Entry {
        let file = std::path::Path::new(path);
        let failed = |error: String| Entry { path: path.into(), error, ..Default::default() };
        if folder::is_index(file) {
            return match folder::open(file) {
                Ok((prj, conv)) => Entry { format: conv.from, ..Entry::of(path, &prj, verify) },
                Err(error) => failed(error),
            };
        }
        match std::fs::read(file) {
            Ok(bytes) => Entry::read(path, &bytes, verify),
            Err(e) => failed(tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e))),
        }
    }

    pub fn of(path: &str, prj: &Project, verify: bool) -> Entry {
        let i = &prj.info;
        let verified = verify.then(|| {
            let v = crate::verify::verify(prj);
            let sev = crate::issue::Severity::Error;
            (v.count(sev) + v.check_errors, v.count(crate::issue::Severity::Warning) + v.check_warnings)
        });
        Entry {
            path: path.into(),
            name: i.name.clone(),
            description: i.description.clone(),
            company: i.company.clone(),
            creator: i.creator.clone(),
            date: i.date.clone(),
            revision: i.revision.clone(),
            properties: i.properties.clone(),
            pages: prj.pages.len(),
            devices: crate::index::Index::build(prj).devices.len(),
            format: prj.format,
            archive: false,
            parent: prj.subprojects.parent.as_ref().map(|p| p.project.clone()).unwrap_or_default(),
            subprojects_out: prj.subprojects.children.iter().filter(|c| c.out).map(|c| c.name.clone()).collect(),
            verified,
            error: String::new(),
        }
    }

    /// Value of a field or a project property.
    pub fn field(&self, key: &str) -> String {
        match key {
            "name" => self.name.clone(),
            "description" => self.description.clone(),
            "company" => self.company.clone(),
            "creator" => self.creator.clone(),
            "date" => self.date.clone(),
            "revision" => self.revision.clone(),
            "path" => self.path.clone(),
            _ => self.properties.get(key).cloned().unwrap_or_default(),
        }
    }

    /// Matches a query: terms separated by `;`, each `key=value` (equal, case does not
    /// matter), `key~text` (contains) or plain text found in any field or property.
    pub fn matches(&self, query: &str) -> bool {
        query.split(';').map(str::trim).filter(|t| !t.is_empty()).all(|term| {
            let lower = |s: &str| s.to_lowercase();
            if let Some((k, v)) = term.split_once('~') {
                lower(&self.field(k.trim())).contains(&lower(v.trim()))
            } else if let Some((k, v)) = term.split_once('=') {
                lower(&self.field(k.trim())) == lower(v.trim())
            } else {
                let t = lower(term);
                FIELDS.iter().chain(["path"].iter()).any(|f| lower(&self.field(f)).contains(&t)) || self.properties.values().any(|v| lower(v).contains(&t))
            }
        })
    }
}

/// The project database (stored by the apps or as a JSON file).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Database {
    /// Folders searched for project files.
    pub folders: Vec<String>,
    pub entries: Vec<Entry>,
}

impl Database {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Database, String> {
        serde_json::from_str(text).map_err(|e| tr_fmt!("Not a project database: {}", e))
    }

    /// Adds or replaces the entry of its file.
    pub fn upsert(&mut self, e: Entry) {
        match self.entries.iter_mut().find(|x| x.path == e.path) {
            Some(x) => *x = e,
            None => self.entries.push(e),
        }
        self.entries.sort_by(|a, b| crate::model::natural_cmp(&a.name, &b.name).then(a.path.cmp(&b.path)));
    }

    /// Replaces the entries of the files of a folder (files that are gone are dropped).
    pub fn update_folder(&mut self, folder: &str, found: Vec<Entry>) {
        self.entries.retain(|e| !e.path.starts_with(folder));
        for e in found {
            self.upsert(e);
        }
        if !self.folders.iter().any(|f| f == folder) {
            self.folders.push(folder.into());
        }
    }

    pub fn remove_folder(&mut self, folder: &str) {
        self.folders.retain(|f| f != folder);
        self.entries.retain(|e| !e.path.starts_with(folder));
    }

    pub fn search(&self, query: &str) -> Vec<&Entry> {
        self.entries.iter().filter(|e| e.matches(query)).collect()
    }

    /// Names of the project properties used by the entries.
    pub fn property_keys(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.entries.iter().flat_map(|e| e.properties.keys().cloned()).collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

/// A file is a project file or archive by its extension.
pub fn is_project_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    EXTENSIONS.iter().any(|ext| lower.ends_with(&format!(".{ext}")))
}

// ------------------------------------------------------------------ batch

/// Changes of project properties for many projects.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    /// Field or property → value (an empty value removes a property).
    pub set: BTreeMap<String, String>,
    pub remove: Vec<String>,
    /// Field / property (`*`: all), text and replacement.
    pub replace: Vec<(String, String, String)>,
    /// Property renames (old → new name).
    pub rename: Vec<(String, String)>,
}

/// A changed value.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Change {
    pub key: String,
    pub old: String,
    pub new: String,
}

impl Batch {
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.remove.is_empty() && self.replace.is_empty() && self.rename.is_empty()
    }

    /// Applies the changes to a project. Returns the changed values.
    pub fn apply(&self, prj: &mut Project) -> Result<Vec<Change>, String> {
        let mut changes = Vec::new();
        for (from, to) in &self.rename {
            if FIELDS.contains(&from.as_str()) || FIELDS.contains(&to.as_str()) {
                return Err(tr_fmt!("{} cannot be renamed", if FIELDS.contains(&from.as_str()) { from } else { to }));
            }
            if let Some(v) = prj.info.properties.get(from).cloned() {
                set(prj, from, "", &mut changes)?;
                set(prj, to, &v, &mut changes)?;
            }
        }
        for (k, v) in &self.set {
            set(prj, k, v, &mut changes)?;
        }
        for k in &self.remove {
            if FIELDS.contains(&k.as_str()) {
                return Err(tr_fmt!("{} cannot be removed", k));
            }
            set(prj, k, "", &mut changes)?;
        }
        for (k, find, with) in &self.replace {
            if find.is_empty() {
                return Err(tr("Replace needs the text to find").into());
            }
            let keys: Vec<String> = if k == "*" {
                FIELDS.iter().map(|f| f.to_string()).chain(prj.info.properties.keys().cloned()).collect()
            } else {
                vec![k.clone()]
            };
            for key in keys {
                let old = get(prj, &key);
                if old.contains(find.as_str()) {
                    set(prj, &key, &old.replace(find.as_str(), with), &mut changes)?;
                }
            }
        }
        Ok(changes)
    }

    /// Applies the changes to a project file. Returns the changes and the new file
    /// content (none if nothing changed). Archives are written as archives again.
    /// Applies the batch to a file on disk (a project file, an archive or the index
    /// of a split project) and writes it back if something changed (`write`).
    /// Returns the changes and the new index entry of a file that was written.
    pub fn apply_on_disk(&self, path: &str, date: &str, write: bool) -> Result<(Vec<Change>, Option<Entry>), String> {
        let file = std::path::Path::new(path);
        if folder::is_index(file) {
            let (mut prj, _) = folder::open(file)?;
            let changes = self.apply(&mut prj)?;
            if changes.is_empty() || !write {
                return Ok((changes, None));
            }
            folder::save(file, &prj)?;
            return Ok((changes, Some(Entry::of(path, &prj, false))));
        }
        let bytes = std::fs::read(file).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
        match self.apply_to_file(&bytes, date)? {
            (changes, Some(new)) if write => {
                std::fs::write(file, &new).map_err(|e| tr_fmt!("Cannot write {}: {}", path, openecraft_i18n::io_error(&e)))?;
                Ok((changes, Some(Entry::read(path, &new, false))))
            }
            (changes, _) => Ok((changes, None)),
        }
    }

    pub fn apply_to_file(&self, bytes: &[u8], date: &str) -> Result<(Vec<Change>, Option<Vec<u8>>), String> {
        let (mut prj, _) = archive::read_project(bytes)?;
        let changes = self.apply(&mut prj)?;
        if changes.is_empty() {
            return Ok((changes, None));
        }
        let out = if archive::is_archive(bytes) {
            let m = archive::manifest(bytes).unwrap_or_default();
            let pdf = m.files.iter().any(|f| f == "project.pdf");
            archive::archive(&prj, &archive::ArchiveOptions { comment: m.comment, date: date.into(), pdf, compact: false })
        } else {
            prj.to_json().into_bytes()
        };
        Ok((changes, Some(out)))
    }
}

/// Value of a field or a project property.
pub fn get(prj: &Project, key: &str) -> String {
    let i = &prj.info;
    match key {
        "name" => i.name.clone(),
        "description" => i.description.clone(),
        "company" => i.company.clone(),
        "creator" => i.creator.clone(),
        "date" => i.date.clone(),
        "revision" => i.revision.clone(),
        _ if properties::valid_key(prj, key).is_some() => properties::get(prj, &Object::Project, key),
        _ => i.properties.get(key).cloned().unwrap_or_default(),
    }
}

/// Sets a field or a project property: defined properties are checked, other names
/// are free project properties.
fn set(prj: &mut Project, key: &str, value: &str, changes: &mut Vec<Change>) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        return Err(tr("The property needs a name").into());
    }
    let old = get(prj, key);
    if properties::valid_key(prj, key).is_some() {
        properties::set(prj, &Object::Project, key, value)?;
    } else {
        let i = &mut prj.info;
        match key {
            "name" => i.name = value.into(),
            "description" => i.description = value.into(),
            "company" => i.company = value.into(),
            "creator" => i.creator = value.into(),
            "date" => i.date = value.into(),
            "revision" => i.revision = value.into(),
            _ if value.is_empty() => {
                i.properties.remove(key);
            }
            _ => {
                i.properties.insert(key.into(), value.into());
            }
        }
    }
    let new = get(prj, key);
    if new != old {
        changes.push(Change { key: key.into(), old, new });
    }
    Ok(())
}

#[cfg(test)]
mod tests;
