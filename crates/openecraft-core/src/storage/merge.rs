//! Three-way merge of one file of a split project, for the merge driver of a version
//! control system (git calls it with the common ancestor and both versions of a file
//! that changed on both sides).
//!
//! The JSON is merged record by record instead of line by line: fields of a record,
//! entries of a map and the records of a list that have an `id` are merged on their
//! own, so two people who changed different symbols of a page, or different fields
//! of one, do not conflict. What both sides changed differently is a conflict: the
//! file keeps the value of this side (`ours`) and the conflict is reported.
//!
//! Object ids are not coordinated between branches: two sides that each added an
//! object may have given it the same id. Within one file this is reported as a
//! conflict; across files only the project verification finds it
//! (`crate::verify`, which also repairs it).

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::ser::{SerializeMap, SerializeSeq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeSet;

/// The merged file and what could not be merged.
#[derive(Clone, Debug, PartialEq)]
pub struct Merged {
    pub content: Vec<u8>,
    /// Where both sides differ (path in the file); the value of `ours` was kept.
    pub conflicts: Vec<String>,
}

/// JSON with the order of the keys kept, so a merged file is written like a saved one.
#[derive(Clone, Debug, PartialEq)]
enum Node {
    Value(Value),
    List(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Node, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Node;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("JSON")
            }
            fn visit_bool<E>(self, v: bool) -> Result<Node, E> {
                Ok(Node::Value(v.into()))
            }
            fn visit_i64<E>(self, v: i64) -> Result<Node, E> {
                Ok(Node::Value(v.into()))
            }
            fn visit_u64<E>(self, v: u64) -> Result<Node, E> {
                Ok(Node::Value(v.into()))
            }
            fn visit_f64<E>(self, v: f64) -> Result<Node, E> {
                Ok(Node::Value(v.into()))
            }
            fn visit_str<E>(self, v: &str) -> Result<Node, E> {
                Ok(Node::Value(v.into()))
            }
            fn visit_unit<E>(self) -> Result<Node, E> {
                Ok(Node::Value(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
                let mut list = Vec::new();
                while let Some(item) = seq.next_element()? {
                    list.push(item);
                }
                Ok(Node::List(list))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Node::Map(entries))
            }
        }
        d.deserialize_any(V)
    }
}

impl Serialize for Node {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Node::Value(v) => v.serialize(s),
            Node::List(list) => {
                let mut seq = s.serialize_seq(Some(list.len()))?;
                for item in list {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Node::Map(entries) => {
                let mut map = s.serialize_map(Some(entries.len()))?;
                for (key, value) in entries {
                    map.serialize_entry(key, value)?;
                }
                map.end()
            }
        }
    }
}

/// Merges the versions `ours` and `theirs` of the file `path` of a split project
/// (path as in [`super::Files`]; only its name matters) that both come from `base`
/// (empty: the file is new on both sides).
pub fn merge_file(path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<Merged, String> {
    let parse = |side: &str, bytes: &[u8]| serde_json::from_slice::<Node>(bytes).map_err(|e| openecraft_i18n::tr_fmt!("Not a valid project file: {}", format!("{path} ({side}): {e}")));
    let base = if base.iter().all(u8::is_ascii_whitespace) { None } else { Some(parse("base", base)?) };
    let (ours, theirs) = (parse("ours", ours)?, parse("theirs", theirs)?);
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let mut m = Merge { order: name == "order.json", index: name.ends_with(&format!(".{}", super::INDEX_EXT)), at: Vec::new(), conflicts: Vec::new() };
    if m.index {
        // A side written with a layout this version does not know is not merged
        // under the rules of this one. (Only the index names the layout: the driver
        // of an older version cannot tell for the other files.)
        for side in base.iter().chain([&ours, &theirs]) {
            if let Node::Map(fields) = side {
                if let Some((_, Node::Value(Value::Number(layout)))) = fields.iter().find(|(key, _)| key == "layout") {
                    super::check_layout(layout.as_u64().unwrap_or(0) as u32)?;
                }
            }
        }
    }
    let merged = match (&ours, &theirs) {
        // The id counter: both sides counted on from the same value.
        (Node::Value(Value::Number(a)), Node::Value(Value::Number(b))) if name == "next_id.json" => Node::Value(a.as_u64().max(b.as_u64()).into()),
        // Two new files of the same name are two objects (two pages with the same
        // id), not versions of one.
        (Node::Map(_), _) | (_, Node::Map(_)) if base.is_none() && ours != theirs => {
            m.conflict();
            ours.clone()
        }
        _ => m.merge(base.as_ref(), &ours, &theirs),
    };
    Ok(Merged { content: super::line(&merged)?, conflicts: m.conflicts })
}

struct Merge {
    /// The file is the order of the records of a list.
    order: bool,
    /// The file is the index: its `pages` are the order of the pages.
    index: bool,
    /// Path to what is merged.
    at: Vec<String>,
    conflicts: Vec<String>,
}

type Entries<'a> = Vec<(String, &'a Node)>;

impl Merge {
    fn conflict(&mut self) {
        self.conflicts.push(if self.at.is_empty() { "/".into() } else { self.at.join("") });
    }

    fn merge(&mut self, base: Option<&Node>, ours: &Node, theirs: &Node) -> Node {
        if ours == theirs || base == Some(theirs) {
            return ours.clone();
        }
        if base == Some(ours) {
            return theirs.clone();
        }
        match (ours, theirs) {
            (Node::Map(o), Node::Map(t)) => {
                let b = match base {
                    Some(Node::Map(b)) => fields(b),
                    _ => Vec::new(),
                };
                let sep = if self.at.is_empty() { "" } else { "." };
                Node::Map(self.entries(b, fields(o), fields(t), |key| format!("{sep}{key}"), false))
            }
            (Node::List(o), Node::List(t)) => {
                let base_list = match base {
                    Some(Node::List(b)) => b.as_slice(),
                    _ => &[],
                };
                let names = self.order || (self.index && self.at == ["pages"]);
                match (keyed(base_list, names), keyed(o, names), keyed(t, names)) {
                    (Some(b), Some(o), Some(t)) => Node::List(self.entries(b, o, t, |key| format!("[{key}]"), true).into_iter().map(|(_, node)| node).collect()),
                    _ => {
                        self.conflict();
                        ours.clone()
                    }
                }
            }
            _ => {
                self.conflict();
                ours.clone()
            }
        }
    }

    /// Merges entries with unique keys (fields of a record, records of a list): in the
    /// order of `ours`, what `theirs` added behind the entry it follows there.
    /// `records`: two entries added under the same key are two objects, not one.
    fn entries(&mut self, base: Entries, ours: Entries, theirs: Entries, label: impl Fn(&str) -> String, records: bool) -> Vec<(String, Node)> {
        let mut out: Vec<(String, Node)> = Vec::new();
        for (key, o) in &ours {
            self.at.push(label(key));
            match (find(&base, key), find(&theirs, key)) {
                (b, Some(t)) => {
                    if records && b.is_none() && *o != t {
                        // The same id for two new objects.
                        self.conflict();
                        out.push((key.clone(), (*o).clone()));
                    } else {
                        out.push((key.clone(), self.merge(b, o, t)));
                    }
                }
                (None, None) => out.push((key.clone(), (*o).clone())),
                // Removed by them.
                (Some(b), None) => {
                    if b != *o {
                        self.conflict();
                        out.push((key.clone(), (*o).clone()));
                    }
                }
            }
            self.at.pop();
        }
        let mut before: Option<&str> = None;
        for (i, (key, t)) in theirs.iter().enumerate() {
            let ours_has = find(&ours, key).is_some();
            match find(&base, key) {
                _ if ours_has => {}
                // Removed by us, changed by them: stays removed.
                Some(b) => {
                    if b != *t {
                        self.at.push(label(key));
                        self.conflict();
                        self.at.pop();
                    }
                }
                None => {
                    // Behind the entry it follows; in front if it comes before all of ours.
                    let leads = before.is_none() && theirs[i + 1..].iter().any(|(k, _)| find(&ours, k).is_some());
                    let at = before.and_then(|b| out.iter().position(|(k, _)| k == b)).map_or(if leads { 0 } else { out.len() }, |i| i + 1);
                    out.insert(at, (key.clone(), (*t).clone()));
                }
            }
            if out.iter().any(|(k, _)| k == key) {
                before = Some(key);
            }
        }
        out
    }
}

fn fields(map: &[(String, Node)]) -> Entries<'_> {
    map.iter().map(|(k, v)| (k.clone(), v)).collect()
}

fn find<'a>(list: &Entries<'a>, key: &str) -> Option<&'a Node> {
    list.iter().find(|(k, _)| k == key).map(|(_, node)| *node)
}

fn keyed(list: &[Node], names: bool) -> Option<Entries<'_>> {
    if names {
        by_text(list)
    } else {
        by_id(list)
    }
}

/// The records of a list by their `id`; none if a record has none or ids repeat.
fn by_id(list: &[Node]) -> Option<Entries<'_>> {
    let mut seen = BTreeSet::new();
    list.iter()
        .map(|node| {
            let Node::Map(fields) = node else { return None };
            let id = match fields.iter().find(|(k, _)| k == "id")? {
                (_, Node::Value(Value::Number(n))) => n.to_string(),
                _ => return None,
            };
            seen.insert(id.clone()).then_some((id, node))
        })
        .collect()
}

/// The names of a list of names; none if it holds something else or a name repeats.
fn by_text(list: &[Node]) -> Option<Entries<'_>> {
    let mut seen = BTreeSet::new();
    list.iter()
        .map(|node| match node {
            Node::Value(Value::String(name)) if seen.insert(name.clone()) => Some((name.clone(), node)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests;
