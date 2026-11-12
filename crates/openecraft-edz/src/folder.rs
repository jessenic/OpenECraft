//! Loose EPLAN catalogs. Paths from XML only select files indexed under the chosen directory.
//! A parts XML file of an unpacked EDZ file (`items/partxml/`) is read with its connection
//! points and drilling pattern (the files next to it) and the macros and pictures below
//! `items/`.
use crate::archive::text;
use crate::{Edz, EdzFile};
use openecraft_core::symlib::ema;
use openecraft_parts::portal::AttachmentKind;
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct Folder {
    pub macros: Vec<PathBuf>,
    xml: Vec<PathBuf>,
    pictures: Vec<PathBuf>,
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
                    "png" | "jpg" | "jpeg" => folder.pictures.push(path),
                    _ => (),
                }
            }
        }
    }
    folder.macros.sort();
    folder.xml.sort();
    folder.pictures.sort();
    Ok(folder)
}

/// The values of an attribute in an XML text (without unescaping).
fn attribute_values<'a>(text: &'a str, name: &str) -> Vec<&'a str> {
    let key = format!("{name}=\"");
    text.match_indices(&key).filter_map(|(i, _)| text[i + key.len()..].split('"').next()).filter(|v| !v.is_empty()).collect()
}

/// The connection point and drilling pattern documents the parts XML `text` refers to,
/// from the XML files next to it named after a reference or a part (as in EDZ files:
/// `SIE.LOGO.connectionpoints.xml`, `SIE.LOGO_71.5x90.construction.xml`).
fn companions(path: &Path, source: &str) -> Vec<String> {
    let mut refs: Vec<(&str, &str)> = Vec::new();
    for (reference, definition) in [("P_ARTICLE_REF_TERMINAL_NAME", "P_PART_TERMINAL_NAME"), ("P_ARTICLE_REF_CONSTRUCTION_NAME", "P_PART_CONSTRUCTION_NAME")] {
        refs.extend(attribute_values(source, reference).into_iter().map(|v| (v, definition)));
    }
    if refs.is_empty() {
        return Vec::new();
    }
    let prefixes: Vec<String> = refs.iter().map(|(r, _)| r.to_lowercase()).chain(attribute_values(source, "P_ARTICLE_PARTNR").into_iter().map(str::to_lowercase)).collect();
    let Some(entries) = path.parent().and_then(|dir| std::fs::read_dir(dir).ok()) else { return Vec::new() };
    let mut found = Vec::new();
    for candidate in entries.flatten().map(|e| e.path()) {
        let name = candidate.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
        if candidate == path || !name.ends_with(".xml") || !prefixes.iter().any(|p| name.starts_with(p.as_str())) {
            continue;
        }
        let Ok(data) = bytes(&candidate) else { continue };
        let doc = text(&data);
        // Definitions only: another part file would add its parts.
        if !doc.contains("P_ARTICLE_PARTNR") && refs.iter().any(|(r, definition)| doc.contains(&format!("{definition}=\"{r}\""))) {
            found.push(doc);
        }
    }
    found
}

/// The directory to look for the macros and pictures of a parts XML file in: the EDZ
/// folder of `items/partxml/`, otherwise the file's own directory.
fn catalog_root(path: &Path) -> Option<&Path> {
    let dir = path.parent()?;
    let named = |p: &Path, n: &str| p.file_name().and_then(|f| f.to_str()).is_some_and(|f| f.eq_ignore_ascii_case(n));
    match dir.parent() {
        Some(items) if named(dir, "partxml") && named(items, "items") => items.parent(),
        _ => Some(dir),
    }
}

/// The picture files the parts of an EPLAN parts XML text name (`P_ARTICLE_PICTUREFILE`):
/// part number and picture reference.
fn picture_refs(text: &str) -> Vec<(String, String)> {
    let Ok(root) = openecraft_parts::xml::parse(text.trim_start_matches('\u{feff}')) else { return Vec::new() };
    let mut parts = Vec::new();
    root.descendants("part", &mut parts);
    parts
        .into_iter()
        .filter_map(|p| {
            let number = openecraft_parts::language_text(p.attribute("P_ARTICLE_PARTNR")?);
            let picture = p.attribute("P_ARTICLE_PICTUREFILE").or_else(|| p.all("variant").find_map(|v| v.attribute("P_ARTICLE_PICTUREFILE")))?;
            Some((number, picture.to_string()))
        })
        .collect()
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
    let local = path.is_absolute() && path.extension().is_some_and(|s| s.eq_ignore_ascii_case("xml"));
    let source = text(data);
    let mut parts = if local && openecraft_parts::is_eplan_xml(&source) {
        let docs: Vec<String> = std::iter::once(source.clone()).chain(companions(path, &source)).collect();
        let mut parts = openecraft_parts::from_eplan_documents(&docs.iter().map(String::as_str).collect::<Vec<_>>())?;
        for p in &mut parts {
            p.migrate_tech_data();
        }
        parts
    } else {
        crate::import(data)?
    };
    let mut files = Vec::new();
    let pictures = if local { picture_refs(&source) } else { Vec::new() };
    if local && (parts.iter().any(|p| !p.eplan_macro.is_empty()) || !pictures.is_empty()) {
        let parent = path.parent().ok_or("Missing catalog directory")?;
        let root = catalog_root(path).unwrap_or(parent);
        let mut catalog = if root != parent { index(root)? } else { Folder::default() };
        if root == parent {
            for entry in std::fs::read_dir(parent).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let candidate = entry.path();
                if entry.file_type().map_err(|e| e.to_string())?.is_file() {
                    match candidate.extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
                        "ema" => catalog.macros.push(candidate),
                        "png" | "jpg" | "jpeg" => catalog.pictures.push(candidate),
                        _ => (),
                    }
                }
            }
        }
        for (number, reference) in &pictures {
            let Some(part) = parts.iter_mut().find(|p| &p.number == number) else { continue };
            let Ok(file) = Folder::pick(&catalog.pictures, reference, "$(md_img)/", path) else { continue };
            let Ok(data) = bytes(file) else { continue };
            if let Some(picture) = crate::picture::thumbnail(&data) {
                part.picture = picture;
            }
            files.push(EdzFile { part: number.clone(), name: file.file_name().unwrap_or_default().to_string_lossy().into_owned(), kind: AttachmentKind::Image, data });
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
        Self::pick(&self.macros, reference, "$(md_macros)/", article).map_err(|missing| if missing { format!("Missing EMA {reference}") } else { format!("Ambiguous EMA {reference}") })
    }

    /// The file of `files` a reference (`$(MD_MACROS)\Siemens\x.ema`) names: by its
    /// path, otherwise the only one of that name next to the article or anywhere. The
    /// error tells a missing file (true) from an ambiguous name.
    fn pick<'a>(files: &'a [PathBuf], reference: &str, prefix: &str, article: &Path) -> Result<&'a Path, bool> {
        let normalized = reference.replace('\\', "/").to_lowercase();
        let relative = normalized.strip_prefix(prefix).unwrap_or(&normalized);
        let file = relative.rsplit('/').next().unwrap_or(relative);
        let matching: Vec<_> =
            files.iter().filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(file))).collect();
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
            [] => Err(true),
            _ => Err(false),
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
                found.push((count, path, p, source.clone()));
            }
        }
        found.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
        let mut iter = found.into_iter();
        let (_, source, mut part, article) = iter.next().ok_or_else(|| format!("Unknown part {number}"))?;
        let conflicting: Vec<_> = iter.filter(|(_, _, p, _)| p != &part).map(|(_, path, _, _)| path.display().to_string()).collect();
        // The connection points and drilling pattern of the chosen article.
        let others = companions(source, &article);
        if !others.is_empty() {
            let docs: Vec<&str> = std::iter::once(article.as_str()).chain(others.iter().map(String::as_str)).collect();
            let parts = openecraft_parts::from_eplan_documents(&docs).map_err(|e| format!("{}: {e}", source.display()))?;
            if let Some(p) = parts.into_iter().find(|p| p.number == number) {
                part = p;
            }
        }
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
        if let Some((_, reference)) = picture_refs(&article).into_iter().find(|(n, _)| n == number) {
            if let Some(data) = Self::pick(&self.pictures, &reference, "$(md_img)/", source).ok().and_then(|p| bytes(p).ok()) {
                part.picture = crate::picture::thumbnail(&data).unwrap_or_default();
            }
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
