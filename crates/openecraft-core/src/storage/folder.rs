//! The split format on disk: a project folder with the index file (`<name>.oecp`)
//! and the folders `pages`, `data` and `files` next to it. Saving writes only the
//! files that changed (a temporary file first, the index last) and removes the files
//! of deleted pages and records; other files of the folder are left alone. A folder
//! without a `.gitattributes` gets one, so that git leaves the files as they are.

use super::{join, split, Files, INDEX, INDEX_EXT};
use crate::archive::Conversion;
use crate::model::Project;
use openecraft_i18n::{io_error, tr_fmt};
use std::path::{Path, PathBuf};

/// Folders of the format next to the index.
const FOLDERS: [&str; 3] = ["pages", "data", "files"];
const TMP: &str = ".tmp";
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

/// Reads the project of an index file.
pub fn open(index: &Path) -> Result<(Project, Conversion), String> {
    join(&read(index)?)
}

/// Writes the project into the folder of the index file. Returns the number of
/// files written or removed (0: the project on disk was the same).
pub fn save(index: &Path, prj: &Project) -> Result<usize, String> {
    write(index, &split(prj))
}

/// The files of the project of an index file.
pub fn read(index: &Path) -> Result<Files, String> {
    let mut files = Files::new();
    files.insert(INDEX.into(), read_file(index)?);
    for (name, path) in stored(folder(index))? {
        if owned(&name) {
            files.insert(name, read_file(&path)?);
        }
    }
    Ok(files)
}

/// Makes the folder of the index file hold exactly `files`.
pub fn write(index: &Path, files: &Files) -> Result<usize, String> {
    let dir = folder(index);
    std::fs::create_dir_all(dir).map_err(|e| cannot_write(dir, &e))?;
    for entry in std::fs::read_dir(dir).map_err(|e| cannot_read(dir, &e))?.flatten() {
        let other = entry.path();
        if is_index(&other) && other.file_name() != index.file_name() {
            return Err(tr_fmt!("The folder {} already holds the project {}: a split project needs a folder of its own", dir.display(), entry.file_name().to_string_lossy()));
        }
    }
    let before = stored(dir)?;
    let mut changed = 0;
    for (name, content) in files.iter().filter(|(name, _)| *name != INDEX) {
        // Embedded files are named by their content.
        let same_size = name.starts_with("files/");
        changed += usize::from(put(&dir.join(name), content, same_size)?);
    }
    for (name, path) in before {
        let stale = match name.strip_suffix(TMP) {
            Some(file) => owned(file),
            None => owned(&name) && !files.contains_key(&name),
        };
        if stale {
            std::fs::remove_file(&path).map_err(|e| cannot_write(&path, &e))?;
            changed += usize::from(!name.ends_with(TMP));
        }
    }
    // Folders without records left (not empty ones are kept by the file system).
    if let Ok(entries) = std::fs::read_dir(dir.join("data")) {
        for entry in entries.flatten() {
            let _ = std::fs::remove_dir(entry.path());
        }
    }
    for name in FOLDERS {
        let _ = std::fs::remove_dir(dir.join(name));
    }
    if let Some(content) = files.get(INDEX) {
        changed += usize::from(put(index, content, false)?);
    }
    let attributes = dir.join(ATTRIBUTES_FILE);
    if !attributes.exists() {
        std::fs::write(&attributes, ATTRIBUTES).map_err(|e| cannot_write(&attributes, &e))?;
    }
    Ok(changed)
}

fn folder(index: &Path) -> &Path {
    match index.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        _ => Path::new("."),
    }
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

/// A file the format writes (and may remove): JSON files of pages and data,
/// embedded files named by their hash.
fn owned(name: &str) -> bool {
    match name.strip_prefix("files/") {
        Some(file) => file.split_once('.').is_some_and(|(hash, ext)| hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()) && !ext.is_empty() && ext.bytes().all(|b| b.is_ascii_alphanumeric())),
        None => name.ends_with(".json"),
    }
}

/// The files in the folders of the format: name in [`Files`] and path.
fn stored(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut out = Vec::new();
    let mut dirs: Vec<(String, PathBuf)> = FOLDERS.iter().map(|f| (f.to_string(), dir.join(f))).filter(|(_, path)| path.is_dir()).collect();
    while let Some((name, path)) = dirs.pop() {
        for entry in std::fs::read_dir(&path).map_err(|e| cannot_read(&path, &e))? {
            let entry = entry.map_err(|e| cannot_read(&path, &e))?;
            let Some(file) = entry.file_name().to_str().map(String::from) else { continue };
            let name = format!("{name}/{file}");
            if entry.path().is_dir() {
                dirs.push((name, entry.path()));
            } else {
                out.push((name, entry.path()));
            }
        }
    }
    Ok(out)
}

/// Writes the file unless it has this content (`same_size`: this size) already. A
/// temporary file is written first so a crash never leaves a half written file.
fn put(path: &Path, content: &[u8], same_size: bool) -> Result<bool, String> {
    let same = if same_size {
        std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.len() == content.len() as u64)
    } else {
        std::fs::read(path).is_ok_and(|old| old == content)
    };
    if same {
        return Ok(false);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| cannot_write(dir, &e))?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(TMP);
    std::fs::write(&tmp, content).and_then(|()| std::fs::rename(&tmp, path)).map_err(|e| cannot_write(path, &e))?;
    Ok(true)
}

#[cfg(test)]
mod tests;
