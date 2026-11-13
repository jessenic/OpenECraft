//! JSON storage: the whole database in one file (written atomically on every change),
//! attachment files in the folder `<name>.files` next to it. Reads the files of
//! earlier server versions (revision and parts only).

use super::{Backend, Change, Snapshot, StoredFile, HISTORY_LIMIT};
use crate::auth::Account;
use openecraft_parts::portal::{brief, Catalog};
use openecraft_parts::Part;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct StoredDb {
    revision: u64,
    parts: Vec<Part>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    catalogs: Vec<Catalog>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    accounts: Vec<Account>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    favorites: BTreeMap<String, Vec<String>>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    libraries: BTreeMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    history: Vec<Snapshot>,
    /// Names of the stored files by id.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    files: BTreeMap<String, String>,
}

pub struct JsonBackend {
    path: Option<PathBuf>,
    db: StoredDb,
    /// File contents when not persisted (tests).
    memory_files: HashMap<String, Vec<u8>>,
}

impl JsonBackend {
    pub fn open(path: &Path) -> io::Result<JsonBackend> {
        let db = if path.exists() {
            let text = std::fs::read_to_string(path)?;
            let mut db: StoredDb = serde_json::from_str(text.trim_start_matches('\u{feff}'))
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display())))?;
            // Databases of earlier versions start at revision 0 or 1 with parts in them.
            if db.revision == 0 && !db.parts.is_empty() {
                db.revision = 1;
            }
            db
        } else {
            StoredDb::default()
        };
        Ok(JsonBackend { path: Some(path.to_path_buf()), db, memory_files: HashMap::new() })
    }

    #[cfg(test)]
    pub fn in_memory() -> JsonBackend {
        JsonBackend { path: None, db: StoredDb::default(), memory_files: HashMap::new() }
    }

    fn files_dir(&self) -> Option<PathBuf> {
        let path = self.path.as_ref()?;
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "parts-db".into());
        Some(path.with_file_name(format!("{stem}.files")))
    }

    fn save(&self) -> io::Result<()> {
        let Some(path) = &self.path else { return Ok(()) };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_string_pretty(&self.db).map_err(io::Error::other)?;
        // Holds the password hashes and sessions of the accounts.
        openecraft_auth::write_private(path, json.as_bytes())
    }

    fn write_file(&mut self, f: &StoredFile) -> io::Result<()> {
        match self.files_dir() {
            Some(dir) => {
                std::fs::create_dir_all(&dir)?;
                let target = dir.join(&f.id);
                if !target.exists() {
                    let tmp = dir.join(format!("{}.tmp", f.id));
                    std::fs::write(&tmp, &f.data)?;
                    std::fs::rename(&tmp, target)?;
                }
            }
            None => {
                self.memory_files.insert(f.id.clone(), f.data.clone());
            }
        }
        Ok(())
    }
}

fn apply(db: &mut StoredDb, change: Change) {
    db.revision = change.revision;
    let mut index: HashMap<String, usize> = db.parts.iter().enumerate().map(|(i, p)| (p.number.clone(), i)).collect();
    for p in change.parts {
        match index.get(&p.number) {
            Some(&i) => db.parts[i] = p,
            None => {
                index.insert(p.number.clone(), db.parts.len());
                db.parts.push(p);
            }
        }
    }
    db.parts.retain(|p| !change.deleted.contains(&p.number));
    for s in change.history {
        let number = s.number.clone();
        db.history.push(s);
        let mut n = db.history.iter().filter(|h| h.number == number).count();
        db.history.retain(|h| {
            if h.number == number && n > HISTORY_LIMIT {
                n -= 1;
                false
            } else {
                true
            }
        });
    }
    for c in change.catalogs {
        db.catalogs.retain(|x| !x.id.eq_ignore_ascii_case(&c.id));
        db.catalogs.push(c);
    }
    db.catalogs.retain(|c| !change.deleted_catalogs.iter().any(|d| d.eq_ignore_ascii_case(&c.id)));
    for a in change.accounts {
        db.accounts.retain(|x| x.user.name != a.user.name);
        db.accounts.push(a);
    }
    db.accounts.retain(|a| !change.deleted_accounts.contains(&a.user.name));
    for name in &change.deleted_accounts {
        db.favorites.remove(name);
    }
    for (user, numbers) in change.favorites {
        db.favorites.insert(user, numbers);
    }
    for (name, json) in change.libraries {
        db.libraries.insert(name, serde_json::from_str(&json).unwrap_or(serde_json::Value::Null));
    }
    for f in change.files {
        db.files.insert(f.id, f.name);
    }
    for id in &change.deleted_files {
        db.files.remove(id);
    }
}

impl Backend for JsonBackend {
    fn kind(&self) -> &'static str {
        "json"
    }

    fn revision(&self) -> u64 {
        self.db.revision
    }

    fn count(&self) -> io::Result<usize> {
        Ok(self.db.parts.len())
    }

    fn candidates(&self, words: &[String], numbers: &[String], brief_parts: bool) -> io::Result<Vec<Part>> {
        Ok(self
            .db
            .parts
            .iter()
            .filter(|p| numbers.is_empty() || numbers.contains(&p.number))
            .filter(|p| {
                let text = super::search_text(p);
                words.iter().all(|w| text.contains(w.as_str()))
            })
            .map(|p| if brief_parts { brief(p) } else { p.clone() })
            .collect())
    }

    fn part(&self, number: &str) -> io::Result<Option<Part>> {
        Ok(self.db.parts.iter().find(|p| p.number == number).cloned())
    }

    fn catalog_counts(&self) -> io::Result<HashMap<String, usize>> {
        let mut counts = HashMap::new();
        for p in self.db.parts.iter().filter(|p| !p.catalog.is_empty()) {
            *counts.entry(p.catalog.to_lowercase()).or_insert(0) += 1;
        }
        Ok(counts)
    }

    fn catalogs(&self) -> io::Result<Vec<Catalog>> {
        Ok(self.db.catalogs.clone())
    }

    fn accounts(&self) -> io::Result<Vec<Account>> {
        Ok(self.db.accounts.clone())
    }

    fn history(&self, number: &str) -> io::Result<Vec<Snapshot>> {
        Ok(self.db.history.iter().rev().filter(|h| h.number == number).cloned().collect())
    }

    fn favorites(&self, user: &str) -> io::Result<Vec<String>> {
        Ok(self.db.favorites.get(user).cloned().unwrap_or_default())
    }

    fn library(&self, name: &str) -> io::Result<Option<String>> {
        Ok(self.db.libraries.get(name).map(|v| v.to_string()))
    }

    fn file(&self, id: &str) -> io::Result<Option<StoredFile>> {
        let Some(name) = self.db.files.get(id) else { return Ok(None) };
        let data = match self.files_dir() {
            Some(dir) => match std::fs::read(dir.join(id)) {
                Ok(d) => d,
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(e),
            },
            None => match self.memory_files.get(id) {
                Some(d) => d.clone(),
                None => return Ok(None),
            },
        };
        Ok(Some(StoredFile { id: id.into(), name: name.clone(), data }))
    }

    fn file_ids(&self) -> io::Result<Vec<String>> {
        Ok(self.db.files.keys().cloned().collect())
    }

    fn unused_files(&self, ids: &[String]) -> io::Result<Vec<String>> {
        let snapshots = self.db.history.iter().filter_map(|s| s.part.as_ref());
        let used: std::collections::HashSet<&str> =
            self.db.parts.iter().chain(snapshots).flat_map(|p| p.attachments.iter().map(|a| a.id.as_str())).collect();
        Ok(ids.iter().filter(|id| !used.contains(id.as_str())).cloned().collect())
    }

    /// New files first (content addressed, harmless if the rest fails), then the
    /// database, then the deleted files. If saving fails, memory keeps the state of
    /// the file.
    fn commit(&mut self, change: Change) -> io::Result<()> {
        for f in &change.files {
            self.write_file(f)?;
        }
        let deleted = change.deleted_files.clone();
        let before = self.db.clone();
        apply(&mut self.db, change);
        if let Err(e) = self.save() {
            self.db = before;
            return Err(e);
        }
        for id in deleted {
            self.memory_files.remove(&id);
            if let Some(dir) = self.files_dir() {
                match std::fs::remove_file(dir.join(&id)) {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => log::warn!("cannot delete file {id}: {e}"),
                    _ => {}
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
