//! Page templates: a new page with its type, plot frame, title block, structure,
//! description and content (frame graphics, fixed texts, symbols). Built-in ones
//! cover the common page types; companies save their own and share them as files.

use openecraft_i18n::{tr, tr_const};
use crate::model::{PageKind, Project};
use crate::ops::{self, Clip};
use crate::structure::Dt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
pub use crate::model::page_templates::PageTemplate;

impl Default for PageTemplate {
    fn default() -> Self {
        PageTemplate {
            name: String::new(),
            kind: PageKind::Schematic,
            frame: String::new(),
            title_block: String::new(),
            description: String::new(),
            structure: Dt::default(),
            values: Default::default(),
            content: Clip::default(),
        }
    }
}

const BUILTIN: [(&str, PageKind, &str, &str); 6] = [
    (tr_const!("Schematic"), PageKind::Schematic, "", tr_const!("Schematic")),
    (tr_const!("Schematic A4 portrait"), PageKind::Schematic, "A4 portrait", tr_const!("Schematic")),
    (tr_const!("Overview"), PageKind::Overview, "", tr_const!("Overview")),
    (tr_const!("Graphics A4 portrait"), PageKind::Graphic, "A4 portrait", tr_const!("Graphics")),
    (tr_const!("Cover sheet"), PageKind::Cover, "", tr_const!("Cover sheet")),
    (tr_const!("Document A4 portrait"), PageKind::Document, "A4 portrait", tr_const!("Document")),
];

/// The built-in templates (descriptions in the current language).
pub fn builtin() -> Vec<PageTemplate> {
    BUILTIN
        .iter()
        .map(|(name, kind, frame, description)| PageTemplate {
            name: name.to_string(),
            kind: *kind,
            frame: frame.to_string(),
            description: tr(description).to_string(),
            ..Default::default()
        })
        .collect()
}

pub fn is_builtin(name: &str) -> bool {
    BUILTIN.iter().any(|b| b.0 == name)
}

/// Shown name: built-in templates are translated.
pub fn display_name(name: &str) -> String {
    match BUILTIN.iter().find(|b| b.0 == name) {
        Some(b) => tr(b.0).to_string(),
        None => name.to_string(),
    }
}

/// All templates of a project: its own first, then the built-in ones it does not replace.
pub fn all(prj: &Project) -> Vec<PageTemplate> {
    let mut v = prj.page_templates.clone();
    for t in builtin() {
        if !v.iter().any(|x| x.name == t.name) {
            v.push(t);
        }
    }
    v
}

pub fn find(prj: &Project, name: &str) -> Option<PageTemplate> {
    all(prj).into_iter().find(|t| t.name == name || display_name(&t.name) == name)
}

/// Inserts a page made from a template at position `at`. Returns its index.
pub fn insert(prj: &mut Project, t: &PageTemplate, at: usize) -> usize {
    let at = ops::add_page(prj, t.kind, at);
    let page = &mut prj.pages[at];
    page.frame = t.frame.clone();
    page.title_block = t.title_block.clone();
    if !t.description.is_empty() {
        page.description = t.description.clone();
    }
    t.structure.apply_to(page);
    page.values.extend(t.values.clone());
    if !t.content.is_empty() {
        ops::paste(prj, at, &t.content, t.content.bounds().min);
    }
    at
}

/// A template of a page: its settings and its whole content.
pub fn from_page(prj: &Project, page_i: usize, name: &str) -> PageTemplate {
    let page = &prj.pages[page_i];
    let all: BTreeSet<u64> = ops::all_ids(page).into_iter().collect();
    PageTemplate {
        name: name.trim().to_string(),
        kind: page.kind,
        frame: page.frame.clone(),
        title_block: page.title_block.clone(),
        description: page.description.clone(),
        structure: Dt::of_page(page),
        values: page.values.clone(),
        content: ops::copy(page, &all),
    }
}

/// Adds a template to the project or replaces the one of the same name.
pub fn save(prj: &mut Project, t: PageTemplate) {
    match prj.page_templates.iter_mut().find(|x| x.name == t.name) {
        Some(x) => *x = t,
        None => prj.page_templates.push(t),
    }
}

pub fn remove(prj: &mut Project, name: &str) -> bool {
    let n = prj.page_templates.len();
    prj.page_templates.retain(|t| t.name != name);
    prj.page_templates.len() != n
}

/// File format for exchanging page templates.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Library {
    pub page_templates: Vec<PageTemplate>,
}

impl Library {
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Result<Library, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }
}

/// Adds the templates of a library (same names are replaced). Returns their number.
pub fn merge(prj: &mut Project, lib: Library) -> usize {
    let n = lib.page_templates.len();
    for t in lib.page_templates {
        save(prj, t);
    }
    n
}

#[cfg(test)]
mod tests;
