//! Project storage: per project a folder with a split project (`<name>/<name>.oecp`
//! with one file per page and record, see `openecraft_core::storage`; it opens in the
//! editor like any other project) and a state file (`<name>.state.json`: revision and
//! id counter) in the data directory. A change writes only the files it concerns,
//! each atomically.

use openecraft_core::collab::{name_key, stored_name, ID_BLOCK};
use openecraft_core::model::Project;
use openecraft_core::storage::{folder, INDEX_EXT};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::path::{Path, PathBuf};

/// Project files of earlier versions of the server.
const OLD_EXT: &str = "oec";
const STATE_EXT: &str = "state.json";
/// Earlier revisions kept per project (in memory) to find what a save changed.
pub const KEPT_REVISIONS: usize = 16;

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct State {
    revision: u64,
    next_id: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub project: Project,
    /// Incremented on every change of the project.
    pub revision: u64,
    /// First object id not given to a session yet.
    pub next_id: u64,
}

pub struct Store {
    dir: Option<PathBuf>,
    pub projects: BTreeMap<String, Entry>,
    /// The last [`KEPT_REVISIONS`] revisions before the current one, oldest first.
    past: BTreeMap<String, VecDeque<(u64, Project)>>,
}

impl Store {
    /// A store that is not persisted (tests).
    #[cfg(test)]
    pub fn in_memory() -> Store {
        Store { dir: None, projects: BTreeMap::new(), past: BTreeMap::new() }
    }

    /// Opens (and creates) the data directory and loads its projects. Project files
    /// of earlier versions (`<name>.oec`) are converted to split projects first.
    pub fn open(dir: &Path) -> io::Result<Store> {
        std::fs::create_dir_all(dir)?;
        // Name → the project is still a project file of an earlier version.
        let mut found: BTreeMap<String, bool> = BTreeMap::new();
        for file in std::fs::read_dir(dir)? {
            let path = file?.path();
            let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            let (name, old) = match file_name.strip_suffix(&format!(".{OLD_EXT}")) {
                Some(name) if path.is_file() => (name, true),
                _ if path.is_dir() && index(dir, file_name).is_file() => (file_name, false),
                _ => continue,
            };
            if !stored_name(name) {
                log::warn!("skipping {}: not a valid project name", path.display());
                continue;
            }
            if let Some(other) = found.keys().find(|n| *n != name && name_key(n) == name_key(name)) {
                log::warn!("skipping {}: the name differs from project {other} only in case", path.display());
                continue;
            }
            if !old && found.contains_key(name) {
                log::warn!("{name}.{OLD_EXT} is left alone: the project has a folder already");
            }
            // The folder of a project counts, whichever is found first.
            let old = old && found.get(name).copied().unwrap_or(true);
            found.insert(name.to_string(), old);
        }
        let mut projects = BTreeMap::new();
        for (name, old) in found {
            let path = index(dir, &name);
            let invalid = |e: String| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", path.display()));
            let project = if old {
                let file = dir.join(format!("{name}.{OLD_EXT}"));
                let project = Project::from_json(&std::fs::read_to_string(&file)?).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {e}", file.display())))?;
                folder::save(&path, &project).map_err(io::Error::other)?;
                let mut kept = file.clone().into_os_string();
                kept.push(".bak");
                std::fs::rename(&file, &kept)?;
                log::info!("converted {} to a split project in {}; the file is kept as {}", file.display(), path.parent().unwrap_or(dir).display(), Path::new(&kept).display());
                project
            } else {
                folder::open(&path).map_err(invalid)?.0
            };
            let state_path = dir.join(format!("{name}.{STATE_EXT}"));
            let state: State = match std::fs::read_to_string(&state_path) {
                Ok(text) => serde_json::from_str(&text).map_err(|e| invalid(e.to_string()))?,
                Err(e) if e.kind() == io::ErrorKind::NotFound => State::default(),
                Err(e) => return Err(e),
            };
            let next_id = state.next_id.max(project.next_id);
            projects.insert(name, Entry { project, revision: state.revision.max(1), next_id });
        }
        Ok(Store { dir: Some(dir.to_path_buf()), projects, past: BTreeMap::new() })
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.projects.get(name)
    }

    /// The project as of `revision`: the current one or one of the kept revisions.
    pub fn at(&self, name: &str, revision: u64) -> Option<&Project> {
        match self.projects.get(name) {
            Some(e) if e.revision == revision => Some(&e.project),
            _ => self.past.get(name)?.iter().find(|(r, _)| *r == revision).map(|(_, p)| p),
        }
    }

    /// The project whose name differs from `name` at most in case: both would be
    /// the same file on Windows and macOS.
    pub fn taken(&self, name: &str) -> Option<&str> {
        let key = name_key(name);
        self.projects.keys().find(|n| name_key(n) == key).map(String::as_str)
    }

    /// Stores `entry` under `name`: in memory only if writing the files succeeded.
    fn put(&mut self, name: &str, mut entry: Entry) -> io::Result<()> {
        entry.project.next_id = entry.next_id;
        if let Some(dir) = &self.dir {
            folder::save(&index(dir, name), &entry.project).map_err(io::Error::other)?;
            let state = State { revision: entry.revision, next_id: entry.next_id };
            write(&dir.join(format!("{name}.{STATE_EXT}")), &serde_json::to_string_pretty(&state).map_err(io::Error::other)?)?;
        }
        self.projects.insert(name.to_string(), entry);
        Ok(())
    }

    /// Adds a new project (the caller checks that the name is free).
    pub fn create(&mut self, name: &str, mut project: Project) -> io::Result<()> {
        project.normalize();
        let next_id = project.next_id;
        self.put(name, Entry { project, revision: 1, next_id })
    }

    /// Replaces the project; a new revision if it changed. Returns the revision.
    pub fn save(&mut self, name: &str, project: Project) -> io::Result<u64> {
        let Some(old) = self.projects.get(name) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, name.to_string()));
        };
        let mut project = project;
        project.next_id = old.next_id;
        if project == old.project {
            return Ok(old.revision);
        }
        let before = (old.revision, old.project.clone());
        let entry = Entry { project, revision: old.revision + 1, next_id: old.next_id };
        let revision = entry.revision;
        self.put(name, entry)?;
        let past = self.past.entry(name.to_string()).or_default();
        past.push_back(before);
        if past.len() > KEPT_REVISIONS {
            past.pop_front();
        }
        Ok(revision)
    }

    /// Removes the project with its folder and state file.
    pub fn delete(&mut self, name: &str) -> io::Result<()> {
        if self.projects.remove(name).is_none() {
            return Err(io::Error::new(io::ErrorKind::NotFound, name.to_string()));
        }
        self.past.remove(name);
        if let Some(dir) = &self.dir {
            let removed = [std::fs::remove_dir_all(dir.join(name)), std::fs::remove_file(dir.join(format!("{name}.{STATE_EXT}")))];
            for result in removed {
                match result {
                    Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
                    _ => {}
                }
            }
        }
        Ok(())
    }

    /// Reserves [`ID_BLOCK`] object ids for a session; returns the first one.
    pub fn reserve_ids(&mut self, name: &str) -> io::Result<u64> {
        let Some(old) = self.projects.get(name) else {
            return Err(io::Error::new(io::ErrorKind::NotFound, name.to_string()));
        };
        let mut entry = old.clone();
        let first = entry.next_id;
        entry.next_id += ID_BLOCK;
        self.put(name, entry)?;
        Ok(first)
    }
}

/// Index file of the project `name` in the data directory.
fn index(dir: &Path, name: &str) -> PathBuf {
    dir.join(name).join(format!("{name}.{INDEX_EXT}"))
}

/// Writes a temporary file first so a crash never leaves a half written file.
fn write(path: &Path, content: &str) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    std::fs::write(&tmp, content)?;
    std::fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests;
