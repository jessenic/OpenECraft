//! Macros: reusable circuit parts. A window macro is a selection of a page, a page
//! macro a complete page. Placeholders `{NAME}` in tags, texts, function texts,
//! designations and connection points are filled from value sets when inserting.
//! A macro can have variants (A, B, …): the same function in other versions, e.g. a
//! motor feeder direct on line or star-delta, chosen when inserting.

use openecraft_i18n::tr_fmt;
use crate::model::*;
use crate::ops::{self, Clip};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MacroKind {
    #[default]
    Window,
    Page,
}

/// Named values for the placeholders of a macro (e.g. "Pump 1.5 kW").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ValueSet {
    pub name: String,
    pub values: BTreeMap<String, String>,
}

/// A further variant of a macro.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MacroVariant {
    pub name: String,
    pub description: String,
    pub clip: Clip,
    pub page_description: String,
}

/// Name of the variant a macro holds in its own items when none is given.
pub const FIRST_VARIANT: &str = "A";

/// A PXF window with several identified devices becomes a native multi-object macro.
/// Its individual definitions are installed with the variant's symbol library.
pub fn eplan_window(v: &crate::symlib::ema::Variant) -> Option<Macro> {
    if v.devices.is_empty() {
        return None;
    }
    let mut clip = Clip { anchor: Some(Default::default()), ..Clip::new() };
    for (symbol, tag, pos) in
        std::iter::once((&v.symbol, "", Default::default())).chain(v.devices.iter().map(|d| (&d.symbol, d.tag.as_str(), d.position)))
    {
        if symbol.prims.is_empty() && symbol.pins.is_empty() {
            continue;
        }
        clip.symbol_defs.push(symbol.clone());
        clip.symbols.push(SymbolInst {
            id: clip.symbols.len() as u64 + 1,
            sym: symbol.key.clone(),
            tag: tag.into(),
            pos,
            pins: symbol.pins.iter().map(|p| p.label.clone()).collect(),
            ..Default::default()
        });
    }
    Some(Macro { name: v.symbol.name.clone(), category: "EPLAN".into(), description: v.description.clone(), clip, ..Default::default() })
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Macro {
    pub name: String,
    pub category: String,
    pub description: String,
    pub kind: MacroKind,
    pub clip: Clip,
    /// Page macros: description of the inserted page.
    pub page_description: String,
    pub value_sets: Vec<ValueSet>,
    /// Name of the variant held in `clip` (empty: [`FIRST_VARIANT`]).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub variant: String,
    /// The other variants.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<MacroVariant>,
}

/// The macro library (stored with the application, like the parts database).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MacroLibrary {
    pub macros: Vec<Macro>,
}

impl MacroLibrary {
    /// A macro library file, or a file with a single macro. (Every object would parse
    /// as an empty library because of the defaults, so the key decides.)
    pub fn from_json(text: &str) -> Result<MacroLibrary, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value.get("macros").is_some() {
            serde_json::from_value(value).map_err(|e| e.to_string())
        } else {
            serde_json::from_value::<Macro>(value).map(|m| MacroLibrary { macros: vec![m] }).map_err(|e| e.to_string())
        }
    }

    /// Adds a macro or replaces the one with the same name.
    pub fn upsert(&mut self, m: Macro) {
        match self.macros.iter_mut().find(|x| x.name == m.name) {
            Some(x) => *x = m,
            None => self.macros.push(m),
        }
        self.macros.sort_by(|a, b| natural_cmp(&a.category, &b.category).then_with(|| natural_cmp(&a.name, &b.name)));
    }

    /// Saves `m` as variant `variant` of the macro with its name: a new macro, or a
    /// new / replaced variant of an existing one. Returns whether the variant existed.
    pub fn upsert_variant(&mut self, mut m: Macro, variant: &str) -> bool {
        let variant = if variant.trim().is_empty() { FIRST_VARIANT } else { variant.trim() };
        match self.macros.iter_mut().find(|x| x.name == m.name) {
            Some(x) => x.set_variant(variant, m.description, m.clip, m.page_description),
            None => {
                m.variant = if variant == FIRST_VARIANT { String::new() } else { variant.to_string() };
                self.upsert(m);
                false
            }
        }
    }

    pub fn get(&self, name: &str) -> Option<&Macro> {
        self.macros.iter().find(|m| m.name == name)
    }

    /// Adds the macros of another library (same names are replaced). Returns the count.
    pub fn merge(&mut self, other: MacroLibrary) -> usize {
        let n = other.macros.len();
        for m in other.macros {
            self.upsert(m);
        }
        n
    }
}

fn scan(s: &str, out: &mut BTreeSet<String>) {
    let mut rest = s;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        match after.find('}') {
            Some(end) if end > 0 && after[..end].chars().all(|c| c.is_alphanumeric() || c == '_' || c == ' ' || c == '.') => {
                out.insert(after[..end].to_string());
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
}

fn fill(s: &str, values: &BTreeMap<String, String>) -> String {
    let mut out = s.to_string();
    for (k, v) in values {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// Every text of a clip that may hold placeholders.
fn texts_mut(clip: &mut Clip) -> Vec<&mut String> {
    let mut v: Vec<&mut String> = Vec::new();
    for s in &mut clip.symbols {
        v.push(&mut s.tag);
        v.push(&mut s.label);
        v.push(&mut s.function_text);
        v.extend(s.values.values_mut());
        v.extend(s.pins.iter_mut());
    }
    for w in &mut clip.wires {
        v.push(&mut w.color);
        v.push(&mut w.cross_section);
    }
    v.extend(clip.texts.iter_mut().map(|t| &mut t.text));
    for device in clip.devices.values_mut() {
        v.push(&mut device.description);
        v.extend(device.values.values_mut());
        v.extend(device.parts.iter_mut().map(|p| &mut p.number));
    }
    for part in &mut clip.parts {
        v.push(&mut part.number);
        v.push(&mut part.description);
    }
    v
}

impl Macro {
    /// Placeholders used in the macro, sorted.
    pub fn placeholders(&self) -> Vec<String> {
        let mut clip = self.clip.clone();
        let mut set = BTreeSet::new();
        for t in texts_mut(&mut clip) {
            scan(t, &mut set);
        }
        for tag in clip.devices.keys() {
            scan(tag, &mut set);
        }
        scan(&self.page_description, &mut set);
        set.into_iter().collect()
    }

    /// The macro's items with the placeholders filled in (unknown ones stay).
    pub fn resolve(&self, values: &BTreeMap<String, String>) -> Clip {
        let mut clip = self.clip.clone();
        for t in texts_mut(&mut clip) {
            *t = fill(t, values);
        }
        clip.devices = clip.devices.into_iter().map(|(tag, data)| (fill(&tag, values), data)).collect();
        clip
    }

    /// Name of the variant held in the macro's own items.
    pub fn main_variant(&self) -> &str {
        if self.variant.is_empty() { FIRST_VARIANT } else { &self.variant }
    }

    /// All variant names, the macro's own first.
    pub fn variant_names(&self) -> Vec<String> {
        std::iter::once(self.main_variant().to_string()).chain(self.variants.iter().map(|v| v.name.clone())).collect()
    }

    /// The macro as variant `name` (its items, page and variant description); `None`
    /// if there is no such variant. An empty name is the macro's own variant.
    pub fn with_variant(&self, name: &str) -> Option<Macro> {
        if name.is_empty() || name == self.main_variant() {
            return Some(Macro { variants: Vec::new(), ..self.clone() });
        }
        let v = self.variants.iter().find(|v| v.name == name)?;
        Some(Macro {
            clip: v.clip.clone(),
            page_description: v.page_description.clone(),
            description: if v.description.is_empty() { self.description.clone() } else { v.description.clone() },
            variant: v.name.clone(),
            variants: Vec::new(),
            ..self.clone()
        })
    }

    /// Sets the items of a variant (adds it if new). Returns whether it existed.
    pub fn set_variant(&mut self, name: &str, description: String, clip: Clip, page_description: String) -> bool {
        if name == self.main_variant() {
            self.clip = clip;
            self.page_description = page_description;
            return true;
        }
        match self.variants.iter_mut().find(|v| v.name == name) {
            Some(v) => {
                v.clip = clip;
                v.page_description = page_description;
                v.description = description;
                true
            }
            None => {
                self.variants.push(MacroVariant { name: name.to_string(), description, clip, page_description });
                self.variants.sort_by(|a, b| natural_cmp(&a.name, &b.name));
                false
            }
        }
    }

    /// Removes a further variant (the macro's own variant stays).
    pub fn remove_variant(&mut self, name: &str) -> bool {
        let n = self.variants.len();
        self.variants.retain(|v| v.name != name);
        n != self.variants.len()
    }

    /// Number of symbols / wires / texts and graphics.
    pub fn summary(&self) -> String {
        let c = &self.clip;
        tr_fmt!("{} symbols, {} wires, {} texts / graphics", c.symbols.len(), c.wires.len(), c.texts.len() + c.shapes.len())
    }
}

/// Window macro from the selected items of a page.
pub fn from_selection(page: &Page, sel: &BTreeSet<u64>, name: &str) -> Macro {
    Macro { name: name.into(), kind: MacroKind::Window, clip: ops::copy(page, sel), ..Default::default() }
}

/// Page macro from a complete page.
pub fn from_page(page: &Page, name: &str) -> Macro {
    let all = ops::all_ids(page).into_iter().collect();
    Macro {
        name: name.into(),
        kind: MacroKind::Page,
        clip: ops::copy(page, &all),
        page_description: page.description.clone(),
        description: page.description.clone(),
        ..Default::default()
    }
}

/// Inserts a page macro as a new schematic page after page `after`, in the structure
/// of the macro's page. Returns the index of the new page.
pub fn insert_page(prj: &mut Project, after: usize, m: &Macro, values: &BTreeMap<String, String>) -> usize {
    let clip = m.resolve(values);
    let at = (after + 1).min(prj.pages.len());
    let id = prj.new_id();
    let mut page = Page::new(id, &ops::next_page_name(prj, at), &fill(&m.page_description, values), PageKind::Schematic);
    clip.structure().apply_to(&mut page);
    ops::insert_page(prj, at, page);
    // Items keep their position on the page.
    let origin = clip.bounds().min;
    ops::paste(prj, at, &clip, origin);
    at
}

#[cfg(test)]
mod tests;
