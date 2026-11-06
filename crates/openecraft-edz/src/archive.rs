//! Reading the archive and assigning its files to the parts.

use openecraft_parts::portal::AttachmentKind;
use openecraft_parts::xml::{self, Element};
use openecraft_parts::Part;
use sevenz_rust2::{ArchiveReader, Password};
use std::io::{Cursor, Read};

/// A file of the archive that belongs to a part.
#[derive(Clone, Debug, PartialEq)]
pub struct EdzFile {
    /// Part number.
    pub part: String,
    /// File name without folders.
    pub name: String,
    pub kind: AttachmentKind,
    pub data: Vec<u8>,
}

/// The content of an EDZ file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Edz {
    pub parts: Vec<Part>,
    pub files: Vec<EdzFile>,
}

/// The bytes are a 7-Zip archive (as EDZ files are).
pub fn is_edz(bytes: &[u8]) -> bool {
    bytes.starts_with(&[b'7', b'z', 0xBC, 0xAF, 0x27, 0x1C])
}

/// Most an archive may unpack to (all files together): more is refused rather than
/// running out of memory on a crafted archive.
pub const MAX_UNPACKED: u64 = 1 << 30;

/// The files of the archive: path (with `/`) and content.
fn unpack(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    unpack_with_limit(bytes, MAX_UNPACKED)
}

fn unpack_with_limit(bytes: &[u8], limit: u64) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut reader = ArchiveReader::new(Cursor::new(bytes), Password::empty()).map_err(|e| format!("not a readable EDZ archive: {e}"))?;
    let mut files = Vec::new();
    let mut left = limit;
    let mut too_large = false;
    reader
        .for_each_entries(|entry, data| {
            let mut buf = Vec::new();
            Read::take(&mut *data, left + 1).read_to_end(&mut buf)?;
            if buf.len() as u64 > left {
                too_large = true;
                return Ok(false);
            }
            left -= buf.len() as u64;
            if !entry.is_directory() {
                files.push((entry.name().replace('\\', "/"), buf));
            }
            Ok(true)
        })
        .map_err(|e| format!("not a readable EDZ archive: {e}"))?;
    if too_large {
        return Err(format!("the EDZ archive unpacks to more than {} MB", limit >> 20));
    }
    Ok(files)
}

/// Text of an XML file (UTF-8 or UTF-16 with byte order mark).
pub(crate) fn text(data: &[u8]) -> String {
    match data {
        [0xFF, 0xFE, rest @ ..] => String::from_utf16_lossy(&rest.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect::<Vec<_>>()),
        [0xFE, 0xFF, rest @ ..] => String::from_utf16_lossy(&rest.as_chunks::<2>().0.iter().map(|c| u16::from_be_bytes(*c)).collect::<Vec<_>>()),
        _ => String::from_utf8_lossy(data).into_owned(),
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

/// Kind of a file by its manifest item type.
fn kind(item_type: &str, name: &str) -> AttachmentKind {
    match item_type {
        "picture" => AttachmentKind::Image,
        // EPLAN macros, not macro libraries of OpenECraft.
        "macro" => AttachmentKind::Other,
        _ => AttachmentKind::guess(name),
    }
}

/// Packages of the manifest: part number and its items (type, path in the archive).
fn manifest(root: &Element) -> Vec<(String, Vec<(String, String)>)> {
    let mut packages = Vec::new();
    root.descendants("package", &mut packages);
    packages
        .into_iter()
        .filter(|p| p.attribute("type") == Some("part"))
        .filter_map(|p| {
            let key = p.attribute("key")?.to_string();
            let mut items = Vec::new();
            p.descendants("item", &mut items);
            let items = items
                .into_iter()
                .filter_map(|i| {
                    let t = i.attribute("type")?;
                    Some((t.to_string(), format!("items/{t}/{}", i.attribute("locator")?.replace('\\', "/"))))
                })
                .collect();
            Some((key, items))
        })
        .collect()
}

fn add(out: &mut Vec<EdzFile>, part: &str, item_type: &str, path: &str, data: &[u8]) {
    let name = file_name(path).to_string();
    if !out.iter().any(|f| f.part == part && f.name == name) {
        out.push(EdzFile { part: part.to_string(), kind: kind(item_type, &name), name, data: data.to_vec() });
    }
}

/// Reads an EDZ file: its parts with their files.
pub fn read(bytes: &[u8]) -> Result<Edz, String> {
    read_with_library(bytes,&[])
}

pub(crate) fn read_with_library(bytes: &[u8], mappings: &[openecraft_core::symlib::UserSymbol]) -> Result<Edz,String> {
    let files = unpack(bytes)?;
    let is_xml = |path: &str| path.to_lowercase().ends_with(".xml") && !path.eq_ignore_ascii_case("manifest.xml");
    let texts: Vec<String> = files.iter().filter(|(p, _)| is_xml(p)).map(|(_, d)| text(d)).collect();
    if texts.is_empty() {
        return Err("the EDZ file has no parts".into());
    }
    let mut parts = openecraft_parts::from_eplan_documents(&texts.iter().map(String::as_str).collect::<Vec<_>>())?;
    for p in &mut parts {
        p.migrate_tech_data();
    }
    let packages = files.iter().find(|(p, _)| p.eq_ignore_ascii_case("manifest.xml")).and_then(|(_, d)| xml::parse(&text(d)).ok()).map(|m| manifest(&m)).unwrap_or_default();
    let mut out = Vec::new();
    for (key, items) in &packages {
        if !parts.iter().any(|p| &p.number == key) {
            continue;
        }
        for (item_type, path) in items.iter().filter(|(t, _)| t != "partxml") {
            if let Some((_, data)) = files.iter().find(|(p, _)| p.eq_ignore_ascii_case(path)) {
                add(&mut out, key, item_type, path, data);
            }
        }
    }
    // Without a usable manifest the pictures and macros belong to the only part.
    if out.is_empty() && parts.len() == 1 {
        for (path, data) in &files {
            if let Some(item_type) = path.strip_prefix("items/").and_then(|p| p.split_once('/')).map(|(t, _)| t).filter(|t| *t != "partxml") {
                add(&mut out, &parts[0].number, item_type, path, data);
            }
        }
    }
    for file in &out {
        if !file.name.to_ascii_lowercase().ends_with(".ema") { continue; }
        let source = text(&file.data);
        if let Some(part) = parts.iter_mut().find(|p| p.number == file.part) {
            let reference = (file_name(&part.eplan_macro).eq_ignore_ascii_case(&file.name)).then_some(part.eplan_macro.as_str());
            match openecraft_core::symlib::ema::decode_source(&source,reference,mappings) {
                Ok(variants) => part.eplan_variants.extend(variants.iter().map(|v| openecraft_parts::Shared::new(serde_json::json!(v)))),
                Err(error) => part.eplan_variants.push(serde_json::json!({"source": file.name, "error": error}).into()),
            }
        }
    }
    for part in &mut parts {
        part.mounting_data.rail = part.effective_rail_mount();
    }
    Ok(Edz { parts, files: out })
}

/// Imports a parts file: an EDZ archive (its parts without the files), otherwise
/// CSV, JSON, BMEcat or EPLAN parts XML ([`openecraft_parts::import`]).
pub fn import(bytes: &[u8]) -> Result<Vec<Part>, String> {
    if is_edz(bytes) {
        read(bytes).map(|e| e.parts)
    } else {
        openecraft_parts::import(&text(bytes))
    }
}

#[cfg(test)]
mod tests;
