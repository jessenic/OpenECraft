//! The project as a set of files (path → content): the split format for version
//! control. A folder with these files, an archive of them and the single project
//! file hold the same project; [`split`] and [`join`] convert between them.
//!
//! ```text
//! project.oecp                  index: layout version, page order, small settings
//! pages/<page id>.json          one file per page
//! data/<field>/<x>/<key>.json   one file per record (devices, parts, cables, …),
//!                               in 16 folders (`x`) so that none grows too large
//! data/<field>/order.json       order of the records of a list
//! data/<field>.json             project-wide data kept in one file
//! files/<sha256>.<ext>          embedded documents and images as plain files
//! ```
//!
//! Every JSON file is one line. The files of a project depend only on its content:
//! splitting an unchanged project gives the same bytes. A list whose records have no
//! unique key is kept in one file, and embedded files that cannot be decoded stay in
//! the JSON, so every project can be stored.
//!
//! Saving a large project again and again must not cost what the whole project
//! costs: [`split_since`] writes only the pages and data that differ from the
//! project saved before.

pub mod folder;
pub mod merge;

use crate::archive::{self, Conversion};
use crate::document::{base64_encode, Document};
use crate::model::document::{with_reader, with_writer};
use crate::model::{Page, Project, FORMAT};
use openecraft_i18n::tr_fmt;
use openecraft_parts::Shared;
use serde::ser::{Impossible, SerializeStruct};
use serde::{Deserialize, Serialize, Serializer};
use serde_json::value::RawValue;
use sha2::{Digest, Sha256};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// Files of a project by their path (`/` separated, relative to the project folder).
pub type Files = BTreeMap<String, Vec<u8>>;

pub const INDEX_EXT: &str = "oecp";
/// Path of the index in [`Files`]; a project folder names it after the project.
pub const INDEX: &str = "project.oecp";
/// Version of the file layout.
pub const LAYOUT: u32 = 1;

const PAGES: &str = "pages";
const ORDER: &str = "order";
const EMBEDDED: &str = "files/";
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

/// The fields with a rule that are the same in both projects. Their names are those
/// of [`RULES`] (a test compares them).
fn same_fields(a: &Project, b: &Project) -> Vec<&'static str> {
    macro_rules! same {
        ($($field:ident),*) => {{
            let mut same = Vec::new();
            $(if a.$field == b.$field {
                same.push(stringify!($field));
            })*
            same
        }};
    }
    same!(
        baselines, cables, configuration, devices, electrical, field, forms, harnesses, import_sources, languages, layout, mounting_marks, next_id, page_templates, parts, pct_loops, plc, plc_cards, plugs,
        preplanning, property_db, report_blocks, review, revisions, strips, structures, symbols, wiring_marks
    )
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct Index {
    layout: u32,
    /// Page files in the order of the pages.
    pages: Vec<String>,
    project: Fields,
}

/// The files [`split_since`] left out: they are as the project before has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kept {
    pages: BTreeSet<String>,
    fields: Vec<&'static str>,
    /// Embedded files: those of the pages and data left out are not known.
    embedded: bool,
}

impl Kept {
    /// The file was left out: it is unchanged, or an embedded file that may still
    /// be in use.
    pub fn covers(&self, name: &str) -> bool {
        (self.embedded && name.starts_with(EMBEDDED))
            || self.pages.contains(name)
            || name.strip_prefix("data/").is_some_and(|rest| self.fields.iter().any(|f| rest.strip_prefix(f).is_some_and(|end| end == ".json" || end.starts_with('/'))))
    }
}

/// The files of the project.
pub fn split(prj: &Project) -> Files {
    split_since(prj, None).0
}

/// The files of the project that differ from those of `last` (the project as it
/// was saved before), and which were left out. The pages and each field stored on
/// its own are compared, so the cost follows what changed. Without `last`: all files.
pub fn split_since(prj: &Project, last: Option<&Project>) -> (Files, Kept) {
    let mut kept = Kept { embedded: last.is_some(), fields: last.map(|last| same_fields(prj, last)).unwrap_or_default(), ..Default::default() };
    let embedded = Rc::new(RefCell::new(Files::new()));
    let sink = embedded.clone();
    let mut files = Files::new();
    let mut index = Index { layout: LAYOUT, ..Default::default() };
    // Pages with unique ids are files of their own; otherwise they stay in the index.
    let unique = |pages: &[Page]| pages.iter().map(|pg| pg.id).collect::<BTreeSet<_>>().len() == pages.len();
    let own_pages = unique(&prj.pages);
    let before: BTreeMap<u64, &Page> = last.filter(|last| unique(&last.pages)).map(|last| last.pages.iter().map(|pg| (pg.id, pg)).collect()).unwrap_or_default();
    let mut project = with_writer(Box::new(move |doc| take_out(doc, &mut sink.borrow_mut())), || {
        if own_pages {
            for pg in &prj.pages {
                let name = format!("{PAGES}/{}.json", pg.id);
                if before.get(&pg.id) == Some(&pg) {
                    kept.pages.insert(name);
                } else {
                    files.insert(name, line(pg));
                }
                index.pages.push(pg.id.to_string());
            }
        }
        let skip = |field: &str| (own_pages && field == PAGES) || kept.fields.contains(&field);
        prj.serialize(Capture { skip: &skip }).unwrap_or_default()
    });
    project.insert("format".into(), raw(&FORMAT));
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
    files.append(&mut embedded.borrow_mut());
    (files, kept)
}

/// Takes the fields of the project as they are serialized, without serializing
/// those to skip (pages and data that did not change).
struct Capture<'a> {
    skip: &'a dyn Fn(&str) -> bool,
}

struct Captured<'a> {
    skip: &'a dyn Fn(&str) -> bool,
    fields: Fields,
}

impl SerializeStruct for Captured<'_> {
    type Ok = Fields;
    type Error = serde_json::Error;

    fn serialize_field<T: ?Sized + Serialize>(&mut self, key: &'static str, value: &T) -> Result<(), Self::Error> {
        if !(self.skip)(key) {
            self.fields.insert(key.into(), serde_json::value::to_raw_value(value)?);
        }
        Ok(())
    }

    fn end(self) -> Result<Fields, Self::Error> {
        Ok(self.fields)
    }
}

/// Only a struct (the project) is taken.
fn not_a_project<T>() -> Result<T, serde_json::Error> {
    Err(serde::ser::Error::custom("not a project"))
}

macro_rules! no_value {
    ($($method:ident($($arg:ty),*))*) => {
        $(fn $method(self $(, _: $arg)*) -> Result<Fields, Self::Error> {
            not_a_project()
        })*
    };
}

impl<'a> Serializer for Capture<'a> {
    type Ok = Fields;
    type Error = serde_json::Error;
    type SerializeSeq = Impossible<Fields, Self::Error>;
    type SerializeTuple = Impossible<Fields, Self::Error>;
    type SerializeTupleStruct = Impossible<Fields, Self::Error>;
    type SerializeTupleVariant = Impossible<Fields, Self::Error>;
    type SerializeMap = Impossible<Fields, Self::Error>;
    type SerializeStruct = Captured<'a>;
    type SerializeStructVariant = Impossible<Fields, Self::Error>;

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Captured<'a>, Self::Error> {
        Ok(Captured { skip: self.skip, fields: Fields::new() })
    }

    no_value! {
        serialize_bool(bool) serialize_i8(i8) serialize_i16(i16) serialize_i32(i32) serialize_i64(i64)
        serialize_u8(u8) serialize_u16(u16) serialize_u32(u32) serialize_u64(u64) serialize_f32(f32) serialize_f64(f64)
        serialize_char(char) serialize_str(&str) serialize_bytes(&[u8]) serialize_none() serialize_unit()
        serialize_unit_struct(&'static str) serialize_unit_variant(&'static str, u32, &'static str)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, _: &T) -> Result<Fields, Self::Error> {
        not_a_project()
    }
    fn serialize_newtype_struct<T: ?Sized + Serialize>(self, _: &'static str, _: &T) -> Result<Fields, Self::Error> {
        not_a_project()
    }
    fn serialize_newtype_variant<T: ?Sized + Serialize>(self, _: &'static str, _: u32, _: &'static str, _: &T) -> Result<Fields, Self::Error> {
        not_a_project()
    }
    fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        not_a_project()
    }
    fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
        not_a_project()
    }
    fn serialize_tuple_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeTupleStruct, Self::Error> {
        not_a_project()
    }
    fn serialize_tuple_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeTupleVariant, Self::Error> {
        not_a_project()
    }
    fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        not_a_project()
    }
    fn serialize_struct_variant(self, _: &'static str, _: u32, _: &'static str, _: usize) -> Result<Self::SerializeStructVariant, Self::Error> {
        not_a_project()
    }
}

/// The project of the files written by [`split`].
pub fn join(files: &Files) -> Result<(Project, Conversion), String> {
    let index: Index = parse(files, INDEX)?;
    if index.layout > LAYOUT {
        return Err(tr_fmt!("The project was written by a newer version (file layout {})", index.layout));
    }
    let mut project = index.project;
    if !project.contains_key(PAGES) {
        project.insert(PAGES.into(), raw(&join_list(files, PAGES, &index.pages, false)?));
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
                    let list = join_list(files, &folder, &order, true)?;
                    (!list.is_empty()).then(|| raw(&list))
                }
                Rule::Whole => None,
            }
        };
        if let Some(value) = value {
            project.insert((*field).into(), value);
        }
    }
    // Embedded files are read wherever the project holds one; a file used several
    // times is decoded once and shared.
    let embedded: Rc<RefCell<BTreeMap<String, Shared<String>>>> = Rc::default();
    let missing = Rc::new(RefCell::new(None::<String>));
    // The reader must own what it reads from: the files the project names are
    // encoded before, found by their names in the JSON.
    let text = serde_json::to_string(&project).unwrap_or_default();
    let named = format!("\"{FILE_REF}{EMBEDDED}");
    for (at, _) in text.match_indices(&named) {
        let path = text[at + 1 + FILE_REF.len()..].split('"').next().unwrap_or_default();
        if let Some(bytes) = files.get(path).filter(|_| !embedded.borrow().contains_key(path)) {
            embedded.borrow_mut().insert(path.to_string(), base64_encode(bytes).into());
        }
    }
    let (content, noted) = (embedded.clone(), missing.clone());
    let reader = move |stored: &str| {
        let path = stored.strip_prefix(FILE_REF)?;
        let data = content.borrow().get(path).cloned();
        if data.is_none() {
            noted.borrow_mut().get_or_insert_with(|| path.to_string());
        }
        data
    };
    let (prj, conv) = with_reader(Box::new(reader), || archive::convert(&text))?;
    let missing = missing.borrow_mut().take();
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
    serde_json::from_slice(bytes).map_err(|e| {
        if conflict_markers(bytes) {
            tr_fmt!("{} holds an unresolved merge conflict of git: the file was merged as text. Take one version of the file (git checkout --ours or --theirs), then set up the merge driver for split projects (merge_project_file) before merging again", path)
        } else {
            tr_fmt!("Not a valid project file: {}", format!("{path}: {e}"))
        }
    })
}

/// Git wrote both versions of a conflict into the file.
fn conflict_markers(bytes: &[u8]) -> bool {
    let mut lines = bytes.split(|b| *b == b'\n');
    lines.any(|l| l.starts_with(b"<<<<<<< ")) && bytes.split(|b| *b == b'\n').any(|l| l.starts_with(b">>>>>>> "))
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

/// Record files of `folder` (sorted): directly in it or in one of its subfolders,
/// without its order file.
fn records<'a>(files: &'a Files, folder: &str) -> impl Iterator<Item = &'a String> {
    let prefix = format!("{folder}/");
    let order = format!("{prefix}{ORDER}.json");
    files.range(prefix.clone()..).map_while(move |(path, _)| Some((path, path.strip_prefix(&prefix)?))).filter_map(move |(path, name)| {
        (name.ends_with(".json") && name.matches('/').count() <= 1 && *path != order).then_some(path)
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

/// Path of the record file `stem` of `folder`. A project has thousands of devices
/// and parts: `spread` puts their files into 16 folders, by the first digit of the
/// hash in the name or the last hex digit of an id.
fn record_path(folder: &str, stem: &str, spread: bool) -> String {
    if !spread {
        return format!("{folder}/{stem}.json");
    }
    let digit = match stem.rsplit_once('~') {
        Some((_, hash)) => hash.chars().next().unwrap_or('0'),
        None => char::from_digit((stem.parse::<u64>().unwrap_or(0) & 15) as u32, 16).unwrap_or('0'),
    };
    format!("{folder}/{digit}/{stem}.json")
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
        let path = record_path(folder, &file_stem(&key), true);
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
        if files.insert(record_path(folder, &stem, true), line(&record)).is_some() {
            return None;
        }
        order.push(stem);
    }
    Some((files, order))
}

/// The records of a list: those of `order`, then the other files of the folder
/// (records added by hand or by a merge that left the order alone).
fn join_list(files: &Files, folder: &str, order: &[String], spread: bool) -> Result<Vec<Raw>, String> {
    let ordered: Vec<String> = order.iter().map(|stem| record_path(folder, stem, spread)).collect();
    let listed: BTreeSet<&String> = ordered.iter().collect();
    let other: Vec<&String> = records(files, folder).filter(|path| !listed.contains(path)).collect();
    ordered.iter().chain(other).map(|path| parse(files, path)).collect()
}

/// Moves the content of an embedded file into `files`; returns what the JSON holds
/// in its place (none: the content, if it is empty or cannot be decoded).
fn take_out(doc: &Document, files: &mut Files) -> Option<String> {
    if doc.data.is_empty() {
        return None;
    }
    let bytes = doc.bytes().ok()?;
    let ext: String = doc.extension().chars().filter(char::is_ascii_alphanumeric).take(8).collect();
    let path = format!("{EMBEDDED}{}.{}", sha256(&bytes), if ext.is_empty() { "bin" } else { &ext });
    files.insert(path.clone(), bytes);
    Some(format!("{FILE_REF}{path}"))
}

#[cfg(test)]
mod tests;
