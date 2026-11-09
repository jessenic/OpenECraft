//! Project verification: checks the project data itself – unique ids, references
//! between its parts, stored values, options and subprojects – beside the
//! engineering checks of [`crate::analysis::Analysis`], and repairs what can be
//! repaired without a decision.

use openecraft_i18n::{tr, tr_fmt};
use crate::model::*;
use crate::options::Item;
use crate::properties::{self, Object};
use crate::analysis::Analysis;
use crate::issue::Severity;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
    /// Page index and object id to show.
    pub target: Option<(usize, u64)>,
    /// [`repair`] fixes it.
    pub repairable: bool,
}

/// Result of a verification.
#[derive(Clone, Debug, Default)]
pub struct Verification {
    pub findings: Vec<Finding>,
    /// Errors and warnings of the engineering checks.
    pub check_errors: usize,
    pub check_warnings: usize,
}

impl Verification {
    pub fn count(&self, severity: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == severity).count()
    }

    pub fn repairable(&self) -> usize {
        self.findings.iter().filter(|f| f.repairable).count()
    }

    /// No errors in the data and the checks.
    pub fn passed(&self) -> bool {
        self.count(Severity::Error) == 0 && self.check_errors == 0
    }
}

/// All ids of pages and page items with the page index they are on.
fn ids(prj: &Project) -> Vec<(u64, usize)> {
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        v.push((pg.id, pi));
        v.extend(crate::options::item_ids(pg).into_iter().map(|id| (id, pi)));
    }
    v
}

/// Device tags something in the project refers to.
fn used_tags(prj: &Project) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = prj.pages.iter().flat_map(|p| p.symbols.iter()).map(|s| s.tag.clone()).collect();
    for p in prj.model_options.options.iter().flat_map(|o| &o.parked) {
        match &p.item {
            Item::Symbol(s) => {
                tags.insert(s.tag.clone());
            }
            Item::Page(pg) => tags.extend(pg.symbols.iter().map(|s| s.tag.clone())),
            _ => {}
        }
    }
    tags.extend(prj.layout.enclosures.iter().flat_map(|e| e.items.iter()).map(|i| i.tag.clone()));
    tags.remove("");
    tags
}

fn unused<'a, V>(map: &'a BTreeMap<String, V>, tags: &BTreeSet<String>, field: &str) -> Vec<&'a String> {
    map.keys().filter(|t| !tags.contains(*t) && !field.contains(&format!("\"{t}\""))).collect()
}

pub fn verify(prj: &Project) -> Verification {
    let mut v = Vec::new();
    let mut push = |severity, message: String, target, repairable| v.push(Finding { severity, message, target, repairable });

    // Ids.
    let mut seen = BTreeMap::new();
    let mut max = 0;
    for (id, pi) in ids(prj) {
        max = max.max(id);
        if seen.insert(id, pi).is_some() {
            push(Severity::Error, tr_fmt!("Id {} is used more than once", id), Some((pi, id)), true);
        }
    }
    if prj.next_id <= max.max(prj.model_options.max_id()) {
        push(Severity::Error, tr("The next free id is already in use").into(), None, true);
    }

    // Pages.
    let mut names = BTreeMap::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        let full = crate::structure::page_full_name(pg);
        if names.insert(full.clone(), pi).is_some() {
            push(Severity::Warning, tr_fmt!("Page name {} is used more than once: cross references are ambiguous", full), Some((pi, 0)), false);
        }
        if let PageKind::Mounting(e) = pg.kind {
            if !prj.layout.enclosures.iter().any(|x| x.id == e) {
                push(Severity::Error, tr_fmt!("Page {} shows an enclosure that no longer exists", pg.name), Some((pi, 0)), false);
            }
        }
        for s in &pg.symbols {
            if prj.symbol(&s.sym).is_none() {
                push(Severity::Error, tr_fmt!("Page {}: unknown symbol {}", pg.name, s.sym), Some((pi, s.id)), false);
            }
        }
    }

    // Device data.
    let tags = used_tags(prj);
    let field = serde_json::to_string(&prj.field).unwrap_or_default();
    for t in unused(&prj.devices, &tags, &field) {
        push(Severity::Warning, tr_fmt!("Device data of {} without symbol or panel item", t), None, true);
    }
    for t in unused(&prj.strips, &tags, &field) {
        push(Severity::Warning, tr_fmt!("Terminal strip data of {} without terminals", t), None, true);
    }
    for t in unused(&prj.plugs, &tags, &field) {
        push(Severity::Warning, tr_fmt!("Plug data of {} without pins", t), None, true);
    }
    for t in unused(&prj.plc_cards, &tags, &field) {
        push(Severity::Warning, tr_fmt!("PLC card data of {} without PLC points", t), None, true);
    }
    for (tag, d) in &prj.devices {
        for p in d.parts.iter().filter(|p| !prj.parts.iter().any(|x| x.number == p.number)) {
            push(Severity::Error, tr_fmt!("{}: part {} is not in the project parts", tag, p.number), None, false);
        }
        let undefined = properties::undefined_values(prj, &Object::Device(tag.clone()));
        if !undefined.is_empty() {
            push(Severity::Info, tr_fmt!("{}: {} values of undefined properties", tag, undefined.len()), None, false);
        }
    }

    // Options.
    for o in &prj.model_options.options {
        if !o.condition.is_empty() {
            if let Err(e) = prj.configuration.holds(&o.condition) {
                push(Severity::Error, tr_fmt!("Option {}: {}", o.name, e), None, false);
            }
        }
        let parked: BTreeSet<u64> = o.parked.iter().map(|p| p.item.id()).collect();
        let missing = o.members.iter().filter(|id| !parked.contains(id) && !crate::options::in_pages(prj, **id) && !in_parked_page(o, **id)).count();
        if missing > 0 {
            push(Severity::Warning, tr_fmt!("Option {}: {} assigned objects no longer exist", o.name, missing), None, true);
        }
        if !o.active {
            push(Severity::Info, tr_fmt!("Option {} is inactive: its content is hidden", o.name), None, false);
        }
    }

    // Subprojects.
    for c in &prj.subprojects.children {
        let missing = c.pages.iter().filter(|id| !prj.pages.iter().any(|p| p.id == **id)).count();
        if missing > 0 {
            push(Severity::Warning, tr_fmt!("Subproject {}: {} of its pages no longer exist", c.name, missing), None, false);
        }
        if c.out {
            push(Severity::Info, tr_fmt!("Subproject {} is being edited outside (since {})", c.name, c.exported), None, false);
        }
    }
    if let Some(p) = &prj.subprojects.parent {
        push(Severity::Info, tr_fmt!("This is subproject {} of {}", p.name, p.project), None, false);
    }

    let an = Analysis::checked(prj);
    let count = |sev| an.issues.iter().filter(|i| i.severity == sev).count();
    Verification { findings: v, check_errors: count(Severity::Error), check_warnings: count(Severity::Warning) }
}

fn in_parked_page(o: &crate::options::ModelOption, id: u64) -> bool {
    o.parked.iter().any(|p| matches!(&p.item, Item::Page(pg) if crate::options::item_ids(pg).contains(&id)))
}

/// Repairs the repairable findings. Returns what was done.
pub fn repair(prj: &mut Project) -> Vec<String> {
    let mut done = Vec::new();
    // Duplicate ids get new ones.
    prj.fix_next_id();
    let mut seen = BTreeSet::new();
    let mut renumbered = 0;
    for pi in 0..prj.pages.len() {
        let mut fresh = |id: &mut u64, next: &mut u64| {
            if !seen.insert(*id) {
                *id = *next;
                *next += 1;
                seen.insert(*id);
                renumbered += 1;
            }
        };
        let mut next = prj.next_id;
        let pg = &mut prj.pages[pi];
        fresh(&mut pg.id, &mut next);
        macro_rules! renumber {
            ($($list:ident),*) => { $(for x in &mut pg.$list { fresh(&mut x.id, &mut next); })* };
        }
        renumber!(symbols, wires, texts, shapes, dimensions, images, conn_defs, cable_defs, junctions, structure_boxes);
        prj.next_id = next;
    }
    if renumbered > 0 {
        done.push(tr_fmt!("{} duplicate ids renumbered", renumbered));
    }

    let tags = used_tags(prj);
    let field = serde_json::to_string(&prj.field).unwrap_or_default();
    let mut removed = 0;
    macro_rules! drop_unused {
        ($($map:ident),*) => {$(
            let gone: Vec<String> = unused(&prj.$map, &tags, &field).into_iter().cloned().collect();
            removed += gone.len();
            for t in gone {
                prj.$map.remove(&t);
            }
        )*};
    }
    drop_unused!(devices, strips, plugs, plc_cards);
    if removed > 0 {
        done.push(tr_fmt!("{} unused device data records removed", removed));
    }

    let mut members = 0;
    let exists: Vec<BTreeSet<u64>> = prj
        .model_options
        .options
        .iter()
        .map(|o| o.members.iter().copied().filter(|id| o.parked.iter().any(|p| p.item.id() == *id) || crate::options::in_pages(prj, *id) || in_parked_page(o, *id)).collect())
        .collect();
    for (o, keep) in prj.model_options.options.iter_mut().zip(exists) {
        members += o.members.len() - keep.len();
        o.members = keep;
    }
    if members > 0 {
        done.push(tr_fmt!("{} option assignments of deleted objects removed", members));
    }
    done
}

#[cfg(test)]
mod tests;
