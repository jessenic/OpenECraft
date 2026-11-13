//! Data of the portal: parts, catalogs, history, accounts, favorites, files and
//! libraries (JSON documents shared by all projects, e.g. the company's forms).
//! [`Store`] holds the rules (revisions, history, attachments); a [`Backend`] only
//! persists them: SQLite (default) or JSON files.

mod json;
pub mod sqlite;

use crate::auth::{sha256_hex, Account};
use crate::bundled::{self, Bundle};
use openecraft_parts::api::{ExportInfo, Installed, PartQuery};
use openecraft_parts::portal::{brief, content, Attachment, AttachmentKind, Catalog, ChangeKind, HistoryEntry, SearchResult};
use openecraft_parts::{default_catalog, Part};
use std::collections::HashMap;
use std::io;
use std::path::Path;

pub use json::JsonBackend;
pub use sqlite::SqliteBackend;

/// Changes kept per part in the history.
pub const HISTORY_LIMIT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StorageKind {
    Sqlite,
    Json,
}

impl StorageKind {
    /// The explicit choice, otherwise JSON for `.json` files and SQLite for all others.
    pub fn resolve(choice: Option<StorageKind>, path: &Path) -> StorageKind {
        choice.unwrap_or(if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) { StorageKind::Json } else { StorageKind::Sqlite })
    }
}

/// One history entry with the part as of that change.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Snapshot {
    pub number: String,
    pub entry: HistoryEntry,
    pub part: Option<Part>,
}

/// A file of an attachment.
#[derive(Clone, Debug, PartialEq)]
pub struct StoredFile {
    pub id: String,
    pub name: String,
    pub data: Vec<u8>,
}

/// Everything a request changes, saved at once (all or nothing).
#[derive(Clone, Debug, Default)]
pub struct Change {
    pub revision: u64,
    pub parts: Vec<Part>,
    pub deleted: Vec<String>,
    pub history: Vec<Snapshot>,
    pub catalogs: Vec<Catalog>,
    pub deleted_catalogs: Vec<String>,
    pub accounts: Vec<Account>,
    pub deleted_accounts: Vec<String>,
    pub favorites: Vec<(String, Vec<String>)>,
    /// Libraries by name (JSON text).
    pub libraries: Vec<(String, String)>,
    pub files: Vec<StoredFile>,
    /// Ids of files no part or history entry refers to any more.
    pub deleted_files: Vec<String>,
}

pub trait Backend: Send {
    fn kind(&self) -> &'static str;
    /// Revision of the last change (0: new database).
    fn revision(&self) -> u64;
    fn count(&self) -> io::Result<usize>;
    /// Parts that may contain all `words` (lower case) and have one of the `numbers`
    /// (any number if empty). The models are left out if `brief`.
    fn candidates(&self, words: &[String], numbers: &[String], brief: bool) -> io::Result<Vec<Part>>;
    fn part(&self, number: &str) -> io::Result<Option<Part>>;
    /// Number of parts per catalog id.
    fn catalog_counts(&self) -> io::Result<HashMap<String, usize>>;
    fn catalogs(&self) -> io::Result<Vec<Catalog>>;
    fn accounts(&self) -> io::Result<Vec<Account>>;
    /// History of a part, newest first.
    fn history(&self, number: &str) -> io::Result<Vec<Snapshot>>;
    fn favorites(&self, user: &str) -> io::Result<Vec<String>>;
    /// A library (JSON text) by name.
    fn library(&self, name: &str) -> io::Result<Option<String>>;
    fn file(&self, id: &str) -> io::Result<Option<StoredFile>>;
    /// Ids of all stored files.
    fn file_ids(&self) -> io::Result<Vec<String>>;
    /// Those of `ids` that no part and no history entry refers to.
    fn unused_files(&self, ids: &[String]) -> io::Result<Vec<String>>;
    fn commit(&mut self, change: Change) -> io::Result<()>;
}

pub fn open_backend(kind: StorageKind, path: &Path) -> io::Result<Box<dyn Backend>> {
    Ok(match kind {
        StorageKind::Sqlite => Box::new(SqliteBackend::open(path)?),
        StorageKind::Json => Box::new(JsonBackend::open(path)?),
    })
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)
}

pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub struct Store {
    backend: Box<dyn Backend>,
    pub revision: u64,
    /// Kept in memory: needed for every authorized request.
    accounts: Vec<Account>,
}

impl Store {
    /// Opens the database; a new one is filled with the example catalog and the
    /// bundled manufacturer catalogs.
    pub fn open(kind: StorageKind, path: &Path) -> io::Result<Store> {
        let backend = open_backend(kind, path)?;
        let new = backend.revision() == 0;
        let mut store = Store::with_backend(backend, default_catalog())?;
        if new {
            for bundle in bundled::all().map_err(io::Error::other)? {
                store.install(bundle)?;
            }
        }
        Ok(store)
    }

    /// Opens a backend; seeds it with `seed` if it is new.
    pub fn with_backend(backend: Box<dyn Backend>, seed: Vec<Part>) -> io::Result<Store> {
        let mut store = Store { revision: backend.revision(), accounts: backend.accounts()?, backend };
        if store.revision == 0 {
            let time = now();
            let parts = seed.into_iter().map(|p| Part { revision: 1, modified: time, ..p }).collect();
            store.backend.commit(Change { revision: 1, parts, ..Default::default() })?;
            store.revision = 1;
        }
        Ok(store)
    }

    /// A store that is not persisted (tests).
    #[cfg(test)]
    pub fn in_memory(parts: Vec<Part>) -> Store {
        Store::with_backend(Box::new(JsonBackend::in_memory()), parts).unwrap()
    }

    #[cfg(test)]
    pub fn into_backend(self) -> Box<dyn Backend> {
        self.backend
    }

    pub fn kind(&self) -> &'static str {
        self.backend.kind()
    }

    pub fn count(&self) -> io::Result<usize> {
        self.backend.count()
    }

    pub fn get(&self, number: &str) -> io::Result<Option<Part>> {
        self.backend.part(number)
    }

    /// Number of a part that differs from `number` only in case.
    pub fn other_case(&self, number: &str) -> io::Result<Option<String>> {
        let words: Vec<String> = number.split_whitespace().map(str::to_lowercase).collect();
        let key = number.to_lowercase();
        Ok(self.backend.candidates(&words, &[], true)?.into_iter().map(|p| p.number).find(|n| n != number && n.to_lowercase() == key))
    }

    fn candidates(&self, q: &PartQuery) -> io::Result<Vec<Part>> {
        let words: Vec<String> = q.q.as_deref().unwrap_or("").split_whitespace().map(str::to_lowercase).collect();
        self.backend.candidates(&words, &q.numbers, q.brief)
    }

    pub fn list(&self, q: &PartQuery) -> io::Result<(usize, Vec<Part>)> {
        Ok(q.apply(&self.candidates(q)?))
    }

    pub fn search(&self, q: &PartQuery) -> io::Result<SearchResult> {
        Ok(q.search(&self.candidates(q)?, self.revision))
    }

    /// All parts with their models.
    pub fn all(&self) -> io::Result<Vec<Part>> {
        self.list(&PartQuery::default()).map(|(_, parts)| parts)
    }

    /// Saves a change as the next revision (if it changes parts or catalogs), then
    /// removes the files the changed parts no longer use (see [`Store::remove_unused`]).
    fn commit(&mut self, mut change: Change, new_revision: bool) -> io::Result<()> {
        change.revision = if new_revision { self.revision + 1 } else { self.revision };
        let mut files = Vec::new();
        for number in change.parts.iter().map(|p| &p.number).chain(&change.deleted) {
            files.extend(self.file_refs(number)?);
        }
        let accounts = change.accounts.clone();
        let deleted = change.deleted_accounts.clone();
        let revision = change.revision;
        self.backend.commit(change)?;
        self.revision = revision;
        for a in accounts {
            match self.accounts.iter_mut().find(|x| x.user.name == a.user.name) {
                Some(x) => *x = a,
                None => self.accounts.push(a),
            }
        }
        self.accounts.retain(|a| !deleted.contains(&a.user.name));
        // The change itself is saved; a file left over here is removed by the next
        // clean-up at startup.
        if let Err(e) = self.remove_unused(files) {
            log::warn!("unused files not removed: {e}");
        }
        Ok(())
    }

    /// Ids of the files a part and its history refer to.
    fn file_refs(&self, number: &str) -> io::Result<Vec<String>> {
        let current = self.backend.part(number)?.map(|p| p.attachments).unwrap_or_default();
        let history = self.backend.history(number)?.into_iter().flat_map(|s| s.part.map(|p| p.attachments).unwrap_or_default());
        Ok(current.into_iter().chain(history).map(|a| a.id).collect())
    }

    /// Deletes those of the files that no part and no history entry refers to (files
    /// stay as long as an older version of a part in the history needs them).
    /// Returns the number of deleted files.
    fn remove_unused(&mut self, mut ids: Vec<String>) -> io::Result<usize> {
        ids.sort();
        ids.dedup();
        if ids.is_empty() {
            return Ok(0);
        }
        // The logos of the catalogs are files too.
        let logos: Vec<String> = self.backend.catalogs()?.into_iter().filter_map(|c| c.logo.map(|l| l.id)).collect();
        let unused: Vec<String> = self.backend.unused_files(&ids)?.into_iter().filter(|id| !logos.contains(id)).collect();
        let n = unused.len();
        if n > 0 {
            self.backend.commit(Change { revision: self.revision, deleted_files: unused, ..Default::default() })?;
        }
        Ok(n)
    }

    /// Deletes all files nothing refers to any more (e.g. left by earlier versions).
    pub fn collect_garbage(&mut self) -> io::Result<usize> {
        let ids = self.backend.file_ids()?;
        self.remove_unused(ids)
    }

    /// Inserts or replaces parts as one revision. Parts equal to the stored version
    /// are left as they are. Returns the stored parts, with whether each was created.
    pub fn put_parts(&mut self, user: &str, parts: Vec<Part>) -> io::Result<Vec<(Part, bool)>> {
        let revision = self.revision + 1;
        let time = now();
        let mut change = Change::default();
        let mut out = Vec::new();
        let mut seen: HashMap<String, usize> = HashMap::new();
        for part in parts {
            let old = match seen.get(&part.number) {
                Some(&i) => Some(change.parts[i].clone()),
                None => self.backend.part(&part.number)?,
            };
            if let Some(old) = old.as_ref().filter(|o| content(o) == content(&part)) {
                out.push((old.clone(), false));
                continue;
            }
            let stored = Part { revision, modified: time, ..part };
            let kind = if old.is_some() { ChangeKind::Changed } else { ChangeKind::Created };
            change.history.push(Snapshot {
                number: stored.number.clone(),
                entry: HistoryEntry { revision, time, user: user.into(), change: kind },
                part: Some(stored.clone()),
            });
            match seen.get(&stored.number) {
                Some(&i) => change.parts[i] = stored.clone(),
                None => {
                    seen.insert(stored.number.clone(), change.parts.len());
                    change.parts.push(stored.clone());
                }
            }
            out.push((stored, old.is_none()));
        }
        if !change.parts.is_empty() {
            self.commit(change, true)?;
        }
        Ok(out)
    }

    /// Removes a part. Returns false if it did not exist.
    pub fn delete(&mut self, user: &str, number: &str) -> io::Result<bool> {
        let Some(old) = self.backend.part(number)? else { return Ok(false) };
        let entry = HistoryEntry { revision: self.revision + 1, time: now(), user: user.into(), change: ChangeKind::Deleted };
        let change = Change { deleted: vec![number.into()], history: vec![Snapshot { number: number.into(), entry, part: Some(old) }], ..Default::default() };
        self.commit(change, true)?;
        Ok(true)
    }

    pub fn history(&self, number: &str) -> io::Result<Vec<HistoryEntry>> {
        Ok(self.backend.history(number)?.into_iter().map(|s| s.entry).collect())
    }

    /// The part as of a change.
    pub fn history_part(&self, number: &str, revision: u64) -> io::Result<Option<Part>> {
        Ok(self.backend.history(number)?.into_iter().find(|s| s.entry.revision == revision).and_then(|s| s.part))
    }

    /// Catalogs with their number of parts.
    pub fn catalogs(&self) -> io::Result<Vec<Catalog>> {
        let counts = self.backend.catalog_counts()?;
        let mut list = self.backend.catalogs()?;
        for c in &mut list {
            c.parts = counts.get(&c.id.to_lowercase()).copied().unwrap_or(0);
        }
        list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.id.cmp(&b.id)));
        Ok(list)
    }

    pub fn catalog(&self, id: &str) -> io::Result<Option<Catalog>> {
        Ok(self.catalogs()?.into_iter().find(|c| c.id.eq_ignore_ascii_case(id)))
    }

    /// Stores a catalog; its logo stays as it is ([`Store::set_logo`] changes it).
    pub fn put_catalog(&mut self, catalog: Catalog) -> io::Result<Catalog> {
        let logo = self.catalog(&catalog.id)?.and_then(|c| c.logo);
        let stored = Catalog { modified: now(), parts: 0, logo, ..catalog };
        self.commit(Change { catalogs: vec![stored.clone()], ..Default::default() }, true)?;
        Ok(self.catalog(&stored.id)?.unwrap_or(stored))
    }

    pub fn delete_catalog(&mut self, id: &str) -> io::Result<()> {
        let logo = self.catalog(id)?.and_then(|c| c.logo);
        self.commit(Change { deleted_catalogs: vec![id.into()], ..Default::default() }, true)?;
        self.remove_unused(logo.into_iter().map(|l| l.id).collect())?;
        Ok(())
    }

    /// Sets (`Some((file name, image))`) or removes the logo of a catalog. `None` if
    /// the catalog does not exist.
    pub fn set_logo(&mut self, id: &str, logo: Option<(&str, Vec<u8>)>) -> io::Result<Option<Catalog>> {
        let Some(mut catalog) = self.catalog(id)? else { return Ok(None) };
        let old = catalog.logo.take();
        let mut files = Vec::new();
        if let Some((name, data)) = logo {
            let file_id = sha256_hex(&data);
            catalog.logo = Some(Attachment { id: file_id.clone(), name: name.into(), kind: AttachmentKind::Image, size: data.len() as u64 });
            files.push(StoredFile { id: file_id, name: name.into(), data });
        }
        let stored = Catalog { modified: now(), parts: 0, ..catalog };
        self.commit(Change { catalogs: vec![stored.clone()], files, ..Default::default() }, true)?;
        self.remove_unused(old.into_iter().map(|l| l.id).collect())?;
        self.catalog(&stored.id)
    }

    /// Installs a bundled catalog: stores the catalog if it is missing or has another
    /// version, adds its new parts and updates the parts nobody but the installer has
    /// changed. Parts users changed or deleted, and parts of another catalog with the
    /// same number, are kept.
    pub fn install(&mut self, bundle: Bundle) -> io::Result<Installed> {
        let mut result = Installed::default();
        let Bundle { catalog, parts, logo } = bundle;
        if self.catalog(&catalog.id)?.is_none_or(|c| c.version != catalog.version) {
            self.put_catalog(catalog.clone())?;
        }
        // The bundled logo while the catalog has none (also after it was removed).
        if let Some(svg) = logo.filter(|_| self.catalog(&catalog.id).ok().flatten().is_some_and(|c| c.logo.is_none())) {
            self.set_logo(&catalog.id, Some((&format!("{}.svg", catalog.id), svg.as_bytes().to_vec())))?;
        }
        let mut changed = Vec::new();
        for part in parts {
            let old = self.backend.part(&part.number)?;
            if old.as_ref().is_some_and(|o| content(o) == content(&part)) {
                continue;
            }
            let by_user = self.backend.history(&part.number)?.first().is_some_and(|s| s.entry.user != bundled::INSTALLER);
            let other_catalog = old.as_ref().is_some_and(|o| !o.catalog.eq_ignore_ascii_case(&catalog.id));
            if by_user || other_catalog {
                result.kept.push(part.number);
                continue;
            }
            match old {
                Some(_) => result.updated += 1,
                None => result.added += 1,
            }
            changed.push(part);
        }
        if !changed.is_empty() {
            self.put_parts(bundled::INSTALLER, changed)?;
        }
        Ok(result)
    }

    pub fn accounts(&self) -> &[Account] {
        &self.accounts
    }

    /// A copy of the whole database (parts with history and files, catalogs, accounts,
    /// favorites) as one SQLite file, which the server opens with `--data` or copies
    /// with `--migrate-from`. Its `meta` table keeps the fields of `info` (the version
    /// of the server, `export.` before their names).
    pub fn backup(&self, info: &ExportInfo) -> io::Result<Vec<u8>> {
        let dir = std::env::temp_dir().join(format!("openecraft-backup-{}-{}", std::process::id(), now_nanos()));
        // The copy holds the password hashes: only this user may read it.
        #[cfg(unix)]
        std::os::unix::fs::DirBuilderExt::mode(&mut std::fs::DirBuilder::new(), 0o700).create(&dir)?;
        #[cfg(not(unix))]
        std::fs::create_dir(&dir)?;
        let path = dir.join("backup.sqlite");
        let copy = || {
            let mut target = SqliteBackend::open(&path)?;
            migrate(self.backend.as_ref(), &mut target)?;
            for (key, value) in info.fields() {
                target.set_meta(&format!("export.{key}"), &value)?;
            }
            drop(target);
            std::fs::read(&path)
        };
        let result = copy();
        std::fs::remove_dir_all(&dir).ok();
        result
    }

    pub fn account(&self, name: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.user.name.eq_ignore_ascii_case(name))
    }

    /// Account of a session token.
    pub fn session(&self, token: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.has_session(token))
    }

    pub fn put_account(&mut self, account: Account) -> io::Result<()> {
        self.commit(Change { accounts: vec![account], ..Default::default() }, false)
    }

    pub fn delete_account(&mut self, name: &str) -> io::Result<()> {
        self.commit(Change { deleted_accounts: vec![name.into()], ..Default::default() }, false)
    }

    pub fn favorites(&self, user: &str) -> io::Result<Vec<String>> {
        self.backend.favorites(user)
    }

    pub fn set_favorites(&mut self, user: &str, numbers: Vec<String>) -> io::Result<()> {
        self.commit(Change { favorites: vec![(user.into(), numbers)], ..Default::default() }, false)
    }

    pub fn library(&self, name: &str) -> io::Result<Option<String>> {
        self.backend.library(name)
    }

    /// Stores a library (JSON text), replacing the one of that name.
    pub fn put_library(&mut self, name: &str, json: String) -> io::Result<()> {
        self.commit(Change { libraries: vec![(name.into(), json)], ..Default::default() }, false)
    }

    /// Stores a file and attaches it to a part (a file of the same name is replaced).
    /// `None` if the part does not exist.
    pub fn attach(&mut self, user: &str, number: &str, name: &str, kind: Option<AttachmentKind>, data: Vec<u8>) -> io::Result<Option<Part>> {
        let Some(mut part) = self.backend.part(number)? else { return Ok(None) };
        let id = sha256_hex(&data);
        let attachment = Attachment { id: id.clone(), name: name.into(), kind: kind.unwrap_or_else(|| AttachmentKind::guess(name)), size: data.len() as u64 };
        part.attachments.retain(|a| a.name != name);
        part.attachments.push(attachment);
        let file = StoredFile { id, name: name.into(), data };
        self.put_with_files(user, part, vec![file])
    }

    /// Removes an attachment from a part (the file stays for older versions).
    pub fn detach(&mut self, user: &str, number: &str, id: &str) -> io::Result<Option<Part>> {
        let Some(mut part) = self.backend.part(number)? else { return Ok(None) };
        let before = part.attachments.len();
        part.attachments.retain(|a| a.id != id);
        if part.attachments.len() == before {
            return Ok(None);
        }
        self.put_with_files(user, part, Vec::new())
    }

    fn put_with_files(&mut self, user: &str, part: Part, files: Vec<StoredFile>) -> io::Result<Option<Part>> {
        let revision = self.revision + 1;
        let time = now();
        let stored = Part { revision, modified: time, ..part };
        let entry = HistoryEntry { revision, time, user: user.into(), change: ChangeKind::Changed };
        let change = Change {
            parts: vec![stored.clone()],
            history: vec![Snapshot { number: stored.number.clone(), entry, part: Some(stored.clone()) }],
            files,
            ..Default::default()
        };
        self.commit(change, true)?;
        Ok(Some(stored))
    }

    pub fn file(&self, id: &str) -> io::Result<Option<StoredFile>> {
        self.backend.file(id)
    }
}

/// Part data without the models, and the models (stored apart, so lists stay small).
pub(crate) fn split_models(part: &Part) -> (Part, Option<serde_json::Value>) {
    let models = (part.model_2d.is_some() || part.model_3d.is_some() || !part.connection_points.is_empty() || !part.eplan_variants.is_empty()).then(|| {
        serde_json::json!({ "model_2d": part.model_2d, "model_3d": part.model_3d, "connection_points": part.connection_points, "eplan_variants": part.eplan_variants })
    });
    (brief(part), models)
}

pub(crate) fn join_models(mut part: Part, models: Option<&str>) -> Part {
    if let Some(m) = models.and_then(|m| serde_json::from_str::<serde_json::Value>(m).ok()) {
        part.model_2d = serde_json::from_value(m["model_2d"].clone()).unwrap_or(None);
        part.model_3d = serde_json::from_value(m["model_3d"].clone()).unwrap_or(None);
        part.connection_points = serde_json::from_value(m["connection_points"].clone()).unwrap_or_default();
        part.eplan_variants = serde_json::from_value(m["eplan_variants"].clone()).unwrap_or_default();
    }
    part
}

/// Lower case text the search words are looked up in.
pub(crate) fn search_text(p: &Part) -> String {
    [&p.number, &p.type_number, &p.order_number, &p.manufacturer, &p.description, &p.group, &p.subgroup, &p.catalog]
        .iter()
        .map(|s| s.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Copies all data of one backend into an empty one (change of the storage).
pub fn migrate(from: &dyn Backend, to: &mut dyn Backend) -> io::Result<usize> {
    let parts = from.candidates(&[], &[], false)?;
    let mut change = Change { revision: from.revision().max(1), catalogs: from.catalogs()?, accounts: from.accounts()?, ..Default::default() };
    for id in change.catalogs.iter().filter_map(|c| c.logo.as_ref().map(|l| l.id.clone())).collect::<Vec<_>>() {
        if !change.files.iter().any(|f| f.id == id) {
            change.files.extend(from.file(&id)?);
        }
    }
    for p in &parts {
        let history = from.history(&p.number)?;
        let attachments = p.attachments.iter().chain(history.iter().flat_map(|s| s.part.iter().flat_map(|p| &p.attachments)));
        for a in attachments {
            if !change.files.iter().any(|f| f.id == a.id) {
                change.files.extend(from.file(&a.id)?);
            }
        }
        change.history.extend(history.into_iter().rev());
    }
    for a in &change.accounts {
        change.favorites.push((a.user.name.clone(), from.favorites(&a.user.name)?));
    }
    let n = parts.len();
    change.parts = parts;
    to.commit(change)?;
    Ok(n)
}

#[cfg(test)]
mod tests;
