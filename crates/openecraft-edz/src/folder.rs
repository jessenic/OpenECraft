//! Loose EPLAN catalogs. Paths from XML only select files indexed under the chosen directory.
use crate::archive::text;
use crate::{Edz, EdzFile};
use openecraft_core::symlib::ema;
use openecraft_parts::portal::AttachmentKind;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Folder {
    pub macros: Vec<PathBuf>,
    xml: Vec<PathBuf>,
}

pub fn index(root: &Path) -> Result<Folder, String> {
    let mut folder = Folder::default();
    let mut dirs = vec![std::path::absolute(root).map_err(|e| e.to_string())?];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))? {
            let entry = entry.map_err(|e| e.to_string())?;
            let t = entry.file_type().map_err(|e| e.to_string())?;
            let path = entry.path();
            if t.is_dir() {
                dirs.push(path);
            } else if t.is_file() {
                match path.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
                    "ema" => folder.macros.push(path),
                    "xml" => folder.xml.push(path),
                    _ => (),
                }
            }
        }
    }
    folder.macros.sort();
    folder.xml.sort();
    Ok(folder)
}

fn bytes(path: &Path) -> Result<Vec<u8>, String> {
    if path.metadata().map_err(|e| e.to_string())?.len() > 64 << 20 {
        return Err(format!("{} exceeds 64 MiB", path.display()));
    }
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

pub fn decode_file(path: &Path) -> Result<Vec<ema::Variant>, String> {
    ema::decode(&text(&bytes(path)?))
}

/// Read the selected file. Loose catalogs resolve companion EMAs in their own directory.
/// Browser file selections have no local absolute path and use the catalog data alone.
pub fn read_file(path: &Path, data: &[u8]) -> Result<Edz, String> {
    read_with_library(path, data, &[])
}

pub fn read_with_library(path: &Path, data: &[u8], mappings: &[openecraft_core::symlib::UserSymbol]) -> Result<Edz, String> {
    if crate::is_edz(data) {
        return crate::archive::read_with_library(data, mappings);
    }
    let mut parts = crate::import(data)?;
    let mut files = Vec::new();
    if path.is_absolute() && path.extension().is_some_and(|s| s.eq_ignore_ascii_case("xml")) && parts.iter().any(|p| !p.eplan_macro.is_empty()) {
        let mut catalog = Folder::default();
        let parent = path.parent().ok_or("Missing catalog directory")?;
        for entry in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let candidate = entry.path();
            if entry.file_type().map_err(|e| e.to_string())?.is_file() && candidate.extension().is_some_and(|s| s.eq_ignore_ascii_case("ema")) {
                catalog.macros.push(candidate);
            }
        }
        let mut cache = std::collections::BTreeMap::new();
        for part in parts.iter_mut().filter(|p| !p.eplan_macro.is_empty()) {
            let source = match catalog.resolve(&part.eplan_macro, path) {
                Ok(source) => source,
                Err(error) => {
                    part.eplan_variants.push(serde_json::json!({"source":part.eplan_macro,"error":error}).into());
                    continue;
                }
            };
            if !cache.contains_key(source) {
                let data = bytes(source)?;
                let converted = match ema::decode_source(&text(&data), Some(&part.eplan_macro), mappings) {
                    Ok(v) => v.iter().map(|v| openecraft_parts::Shared::new(serde_json::json!(v))).collect(),
                    Err(e) => vec![serde_json::json!({"source":source,"error":e}).into()],
                };
                cache.insert(source, (data, converted));
            }
            let (data, variants) = &cache[source];
            part.eplan_variants.clone_from(variants);
            part.mounting_data.rail = part.effective_rail_mount();
            files.push(EdzFile {
                part: part.number.clone(),
                name: source.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                kind: AttachmentKind::Other,
                data: data.clone(),
            });
        }
    }
    Ok(Edz { parts, files })
}

impl Folder {
    fn resolve(&self, reference: &str, article: &Path) -> Result<&Path, String> {
        let normalized = reference.replace('\\', "/").to_lowercase();
        let relative = normalized.strip_prefix("$(md_macros)/").unwrap_or(&normalized);
        let file = relative.rsplit('/').next().unwrap_or(relative);
        let matching: Vec<_> =
            self.macros.iter().filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(file))).collect();
        let exact: Vec<_> =
            matching.iter().filter(|p| p.to_string_lossy().replace('\\', "/").to_lowercase().ends_with(&format!("/{relative}"))).collect();
        if let [p] = exact.as_slice() {
            return Ok(p.as_path());
        }
        let sibling: Vec<_> = matching.iter().filter(|p| p.parent() == article.parent()).collect();
        if let [p] = sibling.as_slice() {
            return Ok(p.as_path());
        }
        match matching.as_slice() {
            [p] => Ok(p.as_path()),
            [] => Err(format!("Missing EMA {reference}")),
            _ => Err(format!("Ambiguous EMA {reference}")),
        }
    }

    /// Load only a selected article. Per-product XML takes priority over aggregate catalogs.
    pub fn part(&self, number: &str) -> Result<Edz, String> {
        self.part_with_library(number, &[])
    }

    pub fn part_with_library(&self, number: &str, mappings: &[openecraft_core::symlib::UserSymbol]) -> Result<Edz, String> {
        let mut found = Vec::new();
        for path in &self.xml {
            let source = text(&bytes(path)?);
            if !source.contains(number) || !openecraft_parts::is_eplan_xml(&source) {
                continue;
            }
            let parts = openecraft_parts::from_eplan_xml(&source).map_err(|e| format!("{}: {e}", path.display()))?;
            let count = parts.len();
            for p in parts.into_iter().filter(|p| p.number == number) {
                found.push((count, path, p));
            }
        }
        found.sort_by_key(|(count, path, _)| (*count, *path));
        let mut iter = found.into_iter();
        let (_, source, mut part) = iter.next().ok_or_else(|| format!("Unknown part {number}"))?;
        let conflicting: Vec<_> = iter.filter(|(_, _, p)| p != &part).map(|(_, path, _)| path.display().to_string()).collect();
        let mut files = Vec::new();
        if !part.eplan_macro.is_empty() {
            let path = self.resolve(&part.eplan_macro, source)?;
            let data = bytes(path)?;
            let variants = ema::decode_source(&text(&data), Some(&part.eplan_macro), mappings)?;
            part.eplan_variants = variants.iter().map(|v| serde_json::json!(v).into()).collect();
            part.mounting_data.rail = part.effective_rail_mount();
            files.push(EdzFile {
                part: number.into(),
                name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                kind: AttachmentKind::Other,
                data,
            });
        }
        if !conflicting.is_empty() {
            part.eplan_variants.push(serde_json::json!({"article":source,"conflicting_catalogs":conflicting}).into());
        }
        part.migrate_tech_data();
        Ok(Edz { parts: vec![part], files })
    }
}

#[cfg(test)]
mod tests;
