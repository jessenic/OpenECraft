//! The split format on disk: a project folder with the index file (`<name>.oecp`)
//! and the folders `pages`, `data` and `files` next to it. Other files of the folder
//! are left alone; a folder without a `.gitattributes` gets one, so that git leaves
//! the files as they are.
//!
//! A project has thousands of files, so what a save costs is counted in files:
//!
//! - A save writes the files that changed and removes those of deleted pages and
//!   records. A working copy that remembers the files as it read them ([`Tracked`])
//!   neither reads the folder again nor writes a page it did not change, and it
//!   tells its own changes from those of another program (git, another editor).
//! - A save is one step: the changed files are written next to the old ones, a
//!   journal names them, and only then they take their places. A crash before the
//!   journal leaves the project as it was; after it, the next access finishes the
//!   save. (A power failure can still lose what the system had not written yet.)
//! - Files are found with the times and sizes the folder listing gives, and read
//!   and written on all threads.

use super::{join, sha256, split, split_since, Files, EMBEDDED, INDEX, INDEX_EXT};
use crate::archive::Conversion;
use crate::model::Project;
use crate::parallel::par_map;
use openecraft_i18n::{io_error, tr_fmt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Folders of the format next to the index.
const FOLDERS: [&str; 3] = ["pages", "data", "files"];
const TMP: &str = ".tmp";
/// Names the files of a save that are written but not in their places yet.
const JOURNAL: &str = ".oecp-save";
/// Written into a project folder that has none: git must not convert line ends (a
/// saved project would differ from its files, embedded files would be damaged) and
/// merges the JSON files with the merge driver, once it is set up (`merge`).
const ATTRIBUTES_FILE: &str = ".gitattributes";
const ATTRIBUTES: &str = "# OpenECraft split project: no conversion of line ends, merged record by record\n# (git config merge.openecraft.driver, see docs/command-line.md).\n*.oecp -text merge=openecraft\npages/*.json -text merge=openecraft\ndata/**/*.json -text merge=openecraft\nfiles/** binary\n";

/// The path names the index of a split project.
pub fn is_index(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(INDEX_EXT))
}

/// Where a split project saved as `chosen` goes: there, if the folder is empty or
/// holds this project already, otherwise into a new folder named like the project (a
/// split project needs a folder of its own).
pub fn place(chosen: &Path) -> PathBuf {
    let mut index = chosen.to_path_buf();
    if !is_index(chosen) {
        index.as_mut_os_string().push(format!(".{INDEX_EXT}"));
    }
    let dir = folder(&index);
    let free = index.is_file() || std::fs::read_dir(dir).map_or(true, |mut entries| entries.next().is_none());
    match (free, index.file_stem(), index.file_name()) {
        (false, Some(stem), Some(name)) => dir.join(stem).join(name),
        _ => index,
    }
}

/// The files of a project folder as a working copy knows them: as they were when it
/// read the project or last saved it, and the project it saved. A save compares the
/// folder and the project with them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracked {
    index: PathBuf,
    files: BTreeMap<String, Stamp>,
    /// The project of the last save: what equals it needs no writing. None after
    /// reading, as a project that was read may differ from its files (conversions).
    last: Option<Project>,
}

impl Tracked {
    /// The files are those of the project of this index file.
    pub fn is_for(&self, index: &Path) -> bool {
        self.index == index
    }
}

/// A file as it was seen: unchanged while size and time are the same.
#[derive(Clone, Debug, PartialEq)]
struct Stamp {
    len: u64,
    modified: Option<SystemTime>,
    /// Hash of the content; none for embedded files, whose name is their hash.
    hash: String,
}

/// What [`save_tracked`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Saved {
    /// Files written or removed.
    pub changed: usize,
    /// Files another program changed since the project was read. They were left as
    /// they are: the project in memory lacks their changes until it is read again.
    pub foreign: usize,
}

/// Reads the project of an index file.
pub fn open(index: &Path) -> Result<(Project, Conversion), String> {
    open_tracked(index).map(|(prj, conv, _)| (prj, conv))
}

/// Reads the project of an index file and remembers its files for [`save_tracked`].
pub fn open_tracked(index: &Path) -> Result<(Project, Conversion, Tracked), String> {
    recover(index)?;
    let (listed, _) = list(index)?;
    let read = par_map(&listed, |file| read_file(&file.path).map(|content| (hash(&file.name, &content), content)));
    let (mut files, mut stamps) = (Files::new(), BTreeMap::new());
    for (file, read) in listed.into_iter().zip(read) {
        let (hash, content) = read?;
        stamps.insert(file.name.clone(), Stamp { len: file.len, modified: file.modified, hash });
        files.insert(file.name, content);
    }
    if !files.contains_key(INDEX) {
        // Missing or unreadable: say why.
        read_file(index)?;
    }
    let (mut prj, conv) = join(&files)?;
    // Another copy of the folder may be edited at the same time.
    prj.scatter_ids();
    Ok((prj, conv, Tracked { index: index.to_path_buf(), files: stamps, last: None }))
}

/// Writes the project into the folder of the index file. Returns the number of
/// files written or removed (0: the project on disk was the same).
pub fn save(index: &Path, prj: &Project) -> Result<usize, String> {
    write(index, &split(prj))
}

/// The files of the project of an index file.
pub fn read(index: &Path) -> Result<Files, String> {
    recover(index)?;
    let (listed, _) = list(index)?;
    let read = par_map(&listed, |file| read_file(&file.path));
    let mut files = Files::new();
    for (file, content) in listed.into_iter().zip(read) {
        files.insert(file.name, content?);
    }
    if !files.contains_key(INDEX) {
        read_file(index)?;
    }
    Ok(files)
}

/// The files of the project folder as they are now.
pub fn track(index: &Path) -> Result<Tracked, String> {
    recover(index)?;
    Ok(Tracked { index: index.to_path_buf(), files: on_disk(index, None)?, last: None })
}

/// Writes the changes of the project into the folder of the index file: the files
/// that differ from `tracked`. Files another program changed meanwhile stay as they
/// are ([`Saved::foreign`]); a file changed by both is not overwritten: nothing is
/// written and the files are named. Without `tracked` for this index (a new place)
/// the folder is made to hold the project, like [`save`].
pub fn save_tracked(index: &Path, prj: &Project, tracked: &mut Option<Tracked>) -> Result<Saved, String> {
    recover(index)?;
    let Some(base) = tracked.as_ref().filter(|t| t.index == index) else {
        let files = split(prj);
        let changed = write(index, &files)?;
        let (listed, _) = list(index)?;
        let stamps = listed.into_iter().filter_map(|file| Some((file.name.clone(), Stamp { len: file.len, modified: file.modified, hash: hash(&file.name, files.get(&file.name)?) }))).collect();
        *tracked = Some(Tracked { index: index.to_path_buf(), files: stamps, last: Some(prj.clone()) });
        return Ok(Saved { changed, foreign: 0 });
    };
    let dir = folder(index);
    // Only what differs from the project saved before is serialized.
    let (new, kept) = split_since(prj, base.last.as_ref());
    let disk = on_disk(index, Some(base))?;
    let names: BTreeSet<&String> = base.files.keys().chain(disk.keys()).chain(new.keys()).collect();
    // What to write (content) or remove (none), files of others, files of both.
    let (mut ours, mut foreign, mut both) = (Vec::new(), 0, Vec::new());
    let mut files = BTreeMap::new();
    for name in names {
        let was = base.files.get(name).map(|s| &s.hash);
        let is = disk.get(name).map(|s| &s.hash);
        let wanted = if kept.covers(name) { was.cloned() } else { new.get(name).map(|content| hash(name, content)) };
        match (wanted.as_ref() != was, is != was) {
            (true, false) => ours.push((name, new.get(name), wanted.clone())),
            (false, true) => foreign += 1,
            // Both made the same change.
            (true, true) if wanted.as_ref() == is => {}
            (true, true) => both.push(name.as_str()),
            (false, false) => {}
        }
        // Files of others stay unknown, so that the next save leaves them again.
        let stamp = if is != was && wanted.as_ref() != is { base.files.get(name) } else { disk.get(name) };
        if let Some(stamp) = stamp {
            files.insert(name.clone(), stamp.clone());
        }
    }
    if !both.is_empty() {
        let more = if both.len() > 5 { ", \u{2026}" } else { "" };
        return Err(tr_fmt!("{} files were changed in the project folder by another program and in the open project: {}{}. Save the project under another name, or open it again and repeat the changes", both.len(), both.iter().take(5).copied().collect::<Vec<_>>().join(", "), more));
    }
    check_folder(dir, index)?;
    let (mut writes, mut removes) = (Vec::new(), Vec::new());
    for (name, content, hash) in ours {
        match content {
            Some(content) => writes.push((name.clone(), content.as_slice(), hash.unwrap_or_default())),
            // An embedded file may be used by a page another program added.
            None if foreign > 0 && embedded(name) => {}
            None => removes.push(name.clone()),
        }
    }
    commit(dir, index, &writes.iter().map(|(name, content, _)| (name.as_str(), *content)).collect::<Vec<_>>(), &removes)?;
    for name in &removes {
        files.remove(name);
    }
    for (name, content, hash) in &writes {
        let path = file_path(dir, index, name);
        let modified = std::fs::metadata(&path).map_err(|e| cannot_read(&path, &e))?.modified().ok();
        files.insert(name.clone(), Stamp { len: content.len() as u64, modified, hash: hash.clone() });
    }
    if !removes.is_empty() {
        tidy(dir);
    }
    write_attributes(dir)?;
    let changed = writes.len() + removes.len();
    *tracked = Some(Tracked { index: index.to_path_buf(), files, last: Some(prj.clone()) });
    Ok(Saved { changed, foreign })
}

/// Makes the folder of the index file hold exactly `files`. Returns the number of
/// files written or removed.
pub fn write(index: &Path, files: &Files) -> Result<usize, String> {
    let dir = folder(index);
    check_folder(dir, index)?;
    recover(index)?;
    let (listed, left) = list(index)?;
    for path in left {
        let _ = std::fs::remove_file(path);
    }
    let on_disk: BTreeMap<&str, &Listed> = listed.iter().map(|file| (file.name.as_str(), file)).collect();
    let differs = |(name, content): &(&String, &Vec<u8>)| match on_disk.get(name.as_str()) {
        None => true,
        Some(file) if file.len != content.len() as u64 => true,
        // Embedded files are named by their content.
        Some(_) if embedded(name) => false,
        Some(file) => std::fs::read(&file.path).map_or(true, |old| old != **content),
    };
    let candidates: Vec<(&String, &Vec<u8>)> = files.iter().collect();
    let changed = par_map(&candidates, differs);
    let writes: Vec<(&str, &[u8])> = candidates.iter().zip(changed).filter(|(_, changed)| *changed).map(|((name, content), _)| (name.as_str(), content.as_slice())).collect();
    let removes: Vec<String> = listed.iter().filter(|file| !files.contains_key(&file.name)).map(|file| file.name.clone()).collect();
    commit(dir, index, &writes, &removes)?;
    if !removes.is_empty() {
        tidy(dir);
    }
    write_attributes(dir)?;
    Ok(writes.len() + removes.len())
}

fn folder(index: &Path) -> &Path {
    match index.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
}

/// Path of a file of the project (the index has its own name).
fn file_path(dir: &Path, index: &Path, name: &str) -> PathBuf {
    if name == INDEX {
        index.to_path_buf()
    } else {
        dir.join(name)
    }
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(TMP);
    tmp.into()
}

fn cannot_read(path: &Path, e: &std::io::Error) -> String {
    tr_fmt!("Cannot read {}: {}", path.display(), io_error(e))
}

fn cannot_write(path: &Path, e: &std::io::Error) -> String {
    tr_fmt!("Cannot write {}: {}", path.display(), io_error(e))
}

fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| cannot_read(path, &e))
}

fn embedded(name: &str) -> bool {
    name.starts_with(EMBEDDED)
}

/// What tells the content of a file: its hash; for an embedded file its name does.
fn hash(name: &str, content: &[u8]) -> String {
    if embedded(name) {
        String::new()
    } else {
        sha256(content)
    }
}

/// A file the format writes (and may remove): JSON files of pages and data,
/// embedded files named by their hash.
fn owned(name: &str) -> bool {
    match name.strip_prefix(EMBEDDED) {
        Some(file) => file.split_once('.').is_some_and(|(hash, ext)| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric())),
        None => name.ends_with(".json"),
    }
}

/// A file of the project as the folder lists it.
struct Listed {
    /// Name in [`Files`].
    name: String,
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

/// The files of the project: the index and the files of the format in its folders,
/// with the size and time of the folder listing (no access to the files), and the
/// temporary files a crash left.
fn list(index: &Path) -> Result<(Vec<Listed>, Vec<PathBuf>), String> {
    let dir = folder(index);
    let (mut files, mut left) = (Vec::new(), Vec::new());
    if let Ok(meta) = std::fs::metadata(index) {
        if meta.is_file() {
            files.push(Listed { name: INDEX.into(), path: index.to_path_buf(), len: meta.len(), modified: meta.modified().ok() });
        }
    }
    let mut dirs: Vec<(String, PathBuf)> = FOLDERS.iter().map(|f| (f.to_string(), dir.join(f))).filter(|(_, path)| path.is_dir()).collect();
    while let Some((folder, path)) = dirs.pop() {
        for entry in std::fs::read_dir(&path).map_err(|e| cannot_read(&path, &e))? {
            let entry = entry.map_err(|e| cannot_read(&path, &e))?;
            let Some(file) = entry.file_name().to_str().map(String::from) else { continue };
            let name = format!("{folder}/{file}");
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                dirs.push((name, entry.path()));
            } else if owned(&name) {
                let meta = entry.metadata().map_err(|e| cannot_read(&entry.path(), &e))?;
                files.push(Listed { name, path: entry.path(), len: meta.len(), modified: meta.modified().ok() });
            } else if name.strip_suffix(TMP).is_some_and(owned) {
                left.push(entry.path());
            }
        }
    }
    Ok((files, left))
}

/// The files of the folder as they are now; those that look as `known` has them
/// are not read.
fn on_disk(index: &Path, known: Option<&Tracked>) -> Result<BTreeMap<String, Stamp>, String> {
    let (listed, left) = list(index)?;
    for path in left {
        let _ = std::fs::remove_file(path);
    }
    let same = |file: &Listed| known.and_then(|t| t.files.get(&file.name)).filter(|s| s.len == file.len && s.modified == file.modified && s.modified.is_some()).cloned();
    let stamps = par_map(&listed, |file| match same(file) {
        Some(stamp) => Ok(stamp),
        None if embedded(&file.name) => Ok(Stamp { len: file.len, modified: file.modified, hash: String::new() }),
        None => Ok(Stamp { len: file.len, modified: file.modified, hash: sha256(&read_file(&file.path)?) }),
    });
    listed.into_iter().zip(stamps).map(|(file, stamp): (Listed, Result<Stamp, String>)| Ok((file.name, stamp?))).collect()
}

/// The files of a save that were written next to their places when it stopped.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Journal {
    /// File name of the index.
    index: String,
    write: Vec<String>,
    remove: Vec<String>,
}

/// Writes and removes files of the project as one step: all of it happens, also
/// after a crash, or nothing. The index takes its place last.
fn commit(dir: &Path, index: &Path, writes: &[(&str, &[u8])], removes: &[String]) -> Result<(), String> {
    if writes.is_empty() && removes.is_empty() {
        return Ok(());
    }
    let folders: BTreeSet<PathBuf> = writes.iter().filter_map(|(name, _)| file_path(dir, index, name).parent().map(Path::to_path_buf)).collect();
    for folder in &folders {
        std::fs::create_dir_all(folder).map_err(|e| cannot_write(folder, &e))?;
    }
    // A folder without an index holds no project yet: there is nothing to keep
    // whole, the files are written to their places and the index last.
    if !index.exists() && removes.is_empty() {
        let (last, first): (Vec<_>, Vec<_>) = writes.iter().partition(|(name, _)| *name == INDEX);
        let put = |(name, content): &&(&str, &[u8])| {
            let path = file_path(dir, index, name);
            std::fs::write(&path, content).map_err(|e| cannot_write(&path, &e))
        };
        par_map(&first, put).into_iter().collect::<Result<Vec<()>, String>>()?;
        return last.iter().try_for_each(put);
    }
    // Next to their places first: the project stays as it was until all are written.
    let staged = par_map(writes, |(name, content)| {
        let path = file_path(dir, index, name);
        std::fs::write(tmp_path(&path), content).map_err(|e| cannot_write(&path, &e))
    });
    if let Some(error) = staged.into_iter().find_map(Result::err) {
        for (name, _) in writes {
            let _ = std::fs::remove_file(tmp_path(&file_path(dir, index, name)));
        }
        return Err(error);
    }
    // One file needs no journal: it takes its place in one step.
    let journal = (writes.len() + removes.len() > 1).then(|| dir.join(JOURNAL));
    if let Some(path) = &journal {
        let entry = Journal { index: index.file_name().unwrap_or_default().to_string_lossy().into_owned(), write: writes.iter().map(|(name, _)| name.to_string()).collect(), remove: removes.to_vec() };
        let tmp = tmp_path(path);
        std::fs::write(&tmp, serde_json::to_vec(&entry).unwrap_or_default()).and_then(|()| std::fs::rename(&tmp, path)).map_err(|e| cannot_write(path, &e))?;
    }
    apply(dir, index, &writes.iter().map(|(name, _)| *name).collect::<Vec<_>>(), removes)?;
    if let Some(path) = &journal {
        std::fs::remove_file(path).map_err(|e| cannot_write(path, &e))?;
    }
    Ok(())
}

/// Moves the written files to their places and removes the others (the second part
/// of a save; it may be repeated).
fn apply(dir: &Path, index: &Path, writes: &[&str], removes: &[String]) -> Result<(), String> {
    let place = |name: &&str| {
        let path = file_path(dir, index, name);
        match std::fs::rename(tmp_path(&path), &path) {
            // Moved before the crash.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && path.exists() => Ok(()),
            result => result.map_err(|e| cannot_write(&path, &e)),
        }
    };
    let (last, first): (Vec<&str>, Vec<&str>) = writes.iter().partition(|name| **name == INDEX);
    par_map(&first, place).into_iter().collect::<Result<Vec<()>, String>>()?;
    for name in removes {
        let path = file_path(dir, index, name);
        match std::fs::remove_file(&path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(cannot_write(&path, &e)),
            _ => {}
        }
    }
    last.iter().try_for_each(place)
}

/// Finishes a save that a crash stopped after its files were written.
fn recover(index: &Path) -> Result<(), String> {
    let dir = folder(index);
    let path = dir.join(JOURNAL);
    let Ok(bytes) = std::fs::read(&path) else { return Ok(()) };
    if let Ok(journal) = serde_json::from_slice::<Journal>(&bytes) {
        let index = dir.join(&journal.index);
        apply(dir, &index, &journal.write.iter().map(String::as_str).collect::<Vec<_>>(), &journal.remove)?;
    }
    std::fs::remove_file(&path).map_err(|e| cannot_write(&path, &e))
}

/// Creates the folder; it must not hold another project.
fn check_folder(dir: &Path, index: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| cannot_write(dir, &e))?;
    for entry in std::fs::read_dir(dir).map_err(|e| cannot_read(dir, &e))?.flatten() {
        let other = entry.path();
        if is_index(&other) && other.file_name() != index.file_name() {
            return Err(tr_fmt!("The folder {} already holds the project {}: a split project needs a folder of its own", dir.display(), entry.file_name().to_string_lossy()));
        }
    }
    Ok(())
}

/// Removes folders without records left (not empty ones are kept by the file system).
fn tidy(dir: &Path) {
    let folders = |path: PathBuf| std::fs::read_dir(path).into_iter().flatten().flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir())).map(|e| e.path());
    for field in folders(dir.join("data")) {
        for part in folders(field.clone()) {
            let _ = std::fs::remove_dir(part);
        }
        let _ = std::fs::remove_dir(field);
    }
    for name in FOLDERS {
        let _ = std::fs::remove_dir(dir.join(name));
    }
}

fn write_attributes(dir: &Path) -> Result<(), String> {
    let attributes = dir.join(ATTRIBUTES_FILE);
    if !attributes.exists() {
        std::fs::write(&attributes, ATTRIBUTES).map_err(|e| cannot_write(&attributes, &e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
