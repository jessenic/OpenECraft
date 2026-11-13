//! The project as a set of files (path → content): the split format for version
//! control. A folder with these files, an archive of them and the single project
//! file hold the same project; [`split`] and [`join`] convert between them.
//!
//! ```text
//! project.oecp              index: layout version, page order, small settings
//! pages/<page id>.json      one file per page
//! data/<field>/<key>.json   one file per record (devices, parts, cables, …)
//! data/<field>/order.json   order of the records of a list
//! data/<field>.json         project-wide data kept in one file
//! files/<sha256>.<ext>      embedded documents and images as plain files
//! ```
//!
//! Every JSON file is one line. The files of a project depend only on its content:
//! splitting an unchanged project gives the same bytes. A list whose records have no
//! unique key is kept in one file, and embedded files that cannot be decoded stay in
//! the JSON, so every project can be stored.

use crate::archive::{self, Conversion};
use crate::document::{base64_encode, Document};
use crate::model::forms::{FormItem, FormLayout};
use crate::model::{Page, Project, FORMAT};
use openecraft_i18n::tr_fmt;
use openecraft_parts::Shared;
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

/// Files of a project by their path (`/` separated, relative to the project folder).
pub type Files = BTreeMap<String, Vec<u8>>;

pub const INDEX_EXT: &str = "oecp";
/// Path of the index in [`Files`]; a project folder names it after the project.
pub const INDEX: &str = "project.oecp";
/// Version of the file layout.
pub const LAYOUT: u32 = 1;

const PAGES: &str = "pages";
const ORDER: &str = "order";
/// Prefix of [`Document::data`] that names a file instead of holding its content
/// (base64 has no colon).
const FILE_REF: &str = "file:";

type Raw = Box<RawValue>;
type Fields = BTreeMap<String, Raw>;

/// How a field of the project is stored; fields without a rule stay in the index.
enum Rule {
    /// A map: one file per entry.
    Map,
    /// A list: one file per record, named by this field of the record.
    List(&'static str),
    /// One file.
    Whole,
}

const RULES: &[(&str, Rule)] = &[
    ("baselines", Rule::List("name")),
    ("cables", Rule::List("id")),
    ("configuration", Rule::Whole),
    ("devices", Rule::Map),
    ("electrical", Rule::Whole),
    ("field", Rule::Whole),
    ("forms", Rule::Whole),
    ("harnesses", Rule::List("id")),
    ("import_sources", Rule::Map),
    ("languages", Rule::Whole),
    ("layout", Rule::Whole),
    ("mounting_marks", Rule::Whole),
    // Changes with every new object: kept out of the index.
    ("next_id", Rule::Whole),
    ("page_templates", Rule::Whole),
    ("parts", Rule::List("number")),
    ("pct_loops", Rule::Whole),
    ("plc", Rule::Whole),
    ("plc_cards", Rule::Map),
    ("plugs", Rule::Map),
    ("preplanning", Rule::Whole),
    ("property_db", Rule::Whole),
    ("report_blocks", Rule::Whole),
    ("review", Rule::Whole),
    ("revisions", Rule::Whole),
    ("strips", Rule::Map),
    ("structures", Rule::Whole),
    ("symbols", Rule::List("key")),
    ("wiring_marks", Rule::Whole),
];

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Index {
    layout: u32,
    /// Page files in the order of the pages.
    pages: Vec<String>,
    project: Fields,
}

/// The files of the project.
pub fn split(prj: &Project) -> Files {
    let mut prj = prj.clone();
    prj.format = FORMAT;
    let mut files = Files::new();
    each_document(&mut prj, &mut |doc| take_out(doc, &mut files));
    let text = serde_json::to_string(&prj).unwrap_or_default();
    let mut project: Fields = serde_json::from_str(&text).unwrap_or_default();
    let mut index = Index { layout: LAYOUT, ..Default::default() };
    if let Some((records, order)) = project.get(PAGES).and_then(|raw| split_list(raw, "id", PAGES)) {
        files.extend(records);
        index.pages = order;
        project.remove(PAGES);
    }
    for (field, rule) in RULES {
        let Some(raw) = project.remove(*field) else { continue };
        let folder = format!("data/{field}");
        let records = match rule {
            Rule::Map => split_map(&raw, &folder),
            Rule::List(key) => split_list(&raw, key, &folder).map(|(mut records, order)| {
                if !order.is_empty() {
                    records.insert(format!("{folder}/{ORDER}.json"), line(&order));
                }
                records
            }),
            Rule::Whole => None,
        };
        match records {
            Some(records) => files.extend(records),
            None => {
                files.insert(format!("{folder}.json"), line(&raw));
            }
        }
    }
    index.project = project;
    files.insert(INDEX.into(), line(&index));
    files
}

/// The project of the files written by [`split`].
pub fn join(files: &Files) -> Result<(Project, Conversion), String> {
    let index: Index = parse(files, INDEX)?;
    if index.layout > LAYOUT {
        return Err(tr_fmt!("The project was written by a newer version (file layout {})", index.layout));
    }
    let mut project = index.project;
    if !project.contains_key(PAGES) {
        project.insert(PAGES.into(), raw(&join_list(files, PAGES, &index.pages)?));
    }
    for (field, rule) in RULES {
        let folder = format!("data/{field}");
        let whole = format!("{folder}.json");
        let value = if files.contains_key(&whole) {
            Some(parse::<Raw>(files, &whole)?)
        } else {
            match rule {
                Rule::Map => {
                    let mut entries = Fields::new();
                    for path in records(files, &folder) {
                        entries.extend(parse::<Fields>(files, path)?);
                    }
                    (!entries.is_empty()).then(|| raw(&entries))
                }
                Rule::List(_) => {
                    let order_file = format!("{folder}/{ORDER}.json");
                    let order: Vec<String> = if files.contains_key(&order_file) { parse(files, &order_file)? } else { Vec::new() };
                    let list = join_list(files, &folder, &order)?;
                    (!list.is_empty()).then(|| raw(&list))
                }
                Rule::Whole => None,
            }
        };
        if let Some(value) = value {
            project.insert((*field).into(), value);
        }
    }
    let (mut prj, conv) = archive::convert(&serde_json::to_string(&project).unwrap_or_default())?;
    let mut loaded: BTreeMap<String, Shared<String>> = BTreeMap::new();
    let mut missing = None;
    each_document(&mut prj, &mut |doc| {
        let Some(path) = doc.data.strip_prefix(FILE_REF).map(String::from) else { return };
        if let Some(data) = loaded.get(&path) {
            doc.data = data.clone();
        } else if let Some(bytes) = files.get(&path) {
            doc.data = base64_encode(bytes).into();
            loaded.insert(path, doc.data.clone());
        } else {
            missing.get_or_insert(path);
        }
    });
    match missing {
        Some(path) => Err(missing_file(&path)),
        None => Ok((prj, conv)),
    }
}

fn missing_file(path: &str) -> String {
    tr_fmt!("The project file {} is missing", path)
}

fn parse<'a, T: Deserialize<'a>>(files: &'a Files, path: &str) -> Result<T, String> {
    let bytes = files.get(path).ok_or_else(|| missing_file(path))?;
    serde_json::from_slice(bytes).map_err(|e| tr_fmt!("Not a valid project file: {}", format!("{path}: {e}")))
}

fn raw<T: Serialize>(value: &T) -> Raw {
    serde_json::value::to_raw_value(value).unwrap_or_default()
}

/// Content of a JSON file: one line.
fn line<T: Serialize>(value: &T) -> Vec<u8> {
    let mut text = serde_json::to_string(value).unwrap_or_default();
    text.push('\n');
    text.into_bytes()
}

/// Record files directly in `folder` (sorted), without its order file.
fn records<'a>(files: &'a Files, folder: &str) -> impl Iterator<Item = &'a String> {
    let prefix = format!("{folder}/");
    let order = format!("{prefix}{ORDER}.json");
    files.range(prefix.clone()..).map_while(move |(path, _)| Some((path, path.strip_prefix(&prefix)?))).filter_map(move |(path, name)| {
        (name.ends_with(".json") && !name.contains('/') && *path != order).then_some(path)
    })
}

/// File name (without extension) of the record with this key. Keys are device tags,
/// part numbers and names: characters that are not safe in file names are replaced,
/// the hash keeps the names of different keys apart (also on file systems that
/// ignore case).
fn file_stem(key: &str) -> String {
    let name: String = key.chars().take(48).map(|c| if c.is_ascii_alphanumeric() || "-_+=.".contains(c) { c } else { '_' }).collect();
    format!("{name}~{}", &sha256(key.as_bytes())[..8])
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// One file per entry of a map (the entry as a map of its own); none if `raw` is
/// not a map or two keys give the same file.
fn split_map(raw: &RawValue, folder: &str) -> Option<Files> {
    let entries: BTreeMap<String, &RawValue> = serde_json::from_str(raw.get()).ok()?;
    let mut files = Files::new();
    for (key, value) in entries {
        let path = format!("{folder}/{}.json", file_stem(&key));
        if files.insert(path, line(&BTreeMap::from([(key, value)]))).is_some() {
            return None;
        }
    }
    Some(files)
}

/// One file per record of a list and the file names in the order of the list; none
/// if `raw` is not a list of records with a unique `key`.
fn split_list(raw: &RawValue, key: &str, folder: &str) -> Option<(Files, Vec<String>)> {
    let list: Vec<&RawValue> = serde_json::from_str(raw.get()).ok()?;
    let mut files = Files::new();
    let mut order = Vec::new();
    for record in list {
        let fields: BTreeMap<String, &RawValue> = serde_json::from_str(record.get()).ok()?;
        let value = fields.get(key)?.get();
        let stem = match serde_json::from_str::<String>(value) {
            Ok(text) => file_stem(&text),
            Err(_) if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => value.to_string(),
            Err(_) => return None,
        };
        if files.insert(format!("{folder}/{stem}.json"), line(&record)).is_some() {
            return None;
        }
        order.push(stem);
    }
    Some((files, order))
}

/// The records of a list: those of `order`, then the other files of the folder
/// (records added by hand or by a merge that left the order alone).
fn join_list(files: &Files, folder: &str, order: &[String]) -> Result<Vec<Raw>, String> {
    let ordered: Vec<String> = order.iter().map(|stem| format!("{folder}/{stem}.json")).collect();
    let listed: BTreeSet<&String> = ordered.iter().collect();
    let other: Vec<&String> = records(files, folder).filter(|path| !listed.contains(path)).collect();
    ordered.iter().chain(other).map(|path| parse(files, path)).collect()
}

/// Moves the content of an embedded file into `files`.
fn take_out(doc: &mut Document, files: &mut Files) {
    if doc.data.is_empty() {
        return;
    }
    let Ok(bytes) = doc.bytes() else { return };
    let ext: String = doc.extension().chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    let path = format!("files/{}.{}", sha256(&bytes), if ext.is_empty() { "bin" } else { &ext });
    doc.data = format!("{FILE_REF}{path}").into();
    files.insert(path, bytes);
}

/// Every embedded file of the project, also those of baselines and revisions.
fn each_document(prj: &mut Project, f: &mut dyn FnMut(&mut Document)) {
    if let Some(logo) = &mut prj.info.logo {
        f(&mut logo.file);
    }
    page_documents(&mut prj.pages, f);
    for form in &mut prj.forms.title_blocks {
        item_documents(&mut form.items, f);
    }
    for report in prj.forms.reports.values_mut() {
        layout_documents(&mut report.layout, f);
    }
    for revision in &mut prj.revisions {
        if !revision.pages.0.is_empty() {
            page_documents(std::sync::Arc::make_mut(&mut revision.pages.0).as_mut_slice(), f);
        }
    }
    for baseline in &mut prj.baselines {
        each_document(std::sync::Arc::make_mut(&mut baseline.project.0), f);
    }
}

fn page_documents(pages: &mut [Page], f: &mut dyn FnMut(&mut Document)) {
    for pg in pages {
        if let Some(doc) = &mut pg.document {
            f(doc);
        }
        for image in &mut pg.images {
            f(&mut image.file);
        }
        if let Some(table) = pg.report.as_mut().and_then(|report| report.frozen.as_mut()) {
            layout_documents(&mut table.layout, f);
        }
    }
}

fn layout_documents(layout: &mut Option<FormLayout>, f: &mut dyn FnMut(&mut Document)) {
    if let Some(layout) = layout {
        item_documents(&mut layout.header, f);
        item_documents(&mut layout.row, f);
    }
}

fn item_documents(items: &mut [FormItem], f: &mut dyn FnMut(&mut Document)) {
    for item in items {
        if let FormItem::Image { picture: Some(picture), .. } = item {
            f(&mut picture.file);
        }
    }
}

#[cfg(test)]
mod tests;
