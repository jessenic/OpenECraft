//! Subprojects: pages of a project (chosen one by one or by structure) are written to
//! a project file of their own with the device, cable and part data they use, edited
//! there (by a colleague or a subcontractor) and inserted again. While a subproject
//! is out its pages are locked in the main project; inserting replaces them with the
//! edited pages. Ids created in the subproject come from a range reserved for it, so
//! they cannot collide with ids created in the main project meanwhile.

use openecraft_i18n::{tr, tr_fmt};
use crate::model::*;
use std::collections::BTreeSet;
pub use crate::model::subproject::{Child, Parent, Subprojects};

/// Ids reserved for the new objects of one subproject.
const ID_RANGE: u64 = 1 << 20;

impl Subprojects {
    pub fn is_empty(&self) -> bool {
        self.parent.is_none() && self.children.is_empty()
    }

    pub fn child(&self, name: &str) -> Option<&Child> {
        self.children.iter().find(|c| c.name == name)
    }
}

/// Pages chosen by name and / or by structure (`=A1`, `=A1+CAB1`: pages whose
/// structure starts with it).
pub fn select_pages(prj: &Project, names: &[String], structure: &str) -> Result<Vec<u64>, String> {
    let mut ids = Vec::new();
    for n in names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()) {
        let pg = prj.pages.iter().find(|p| p.name == n).ok_or_else(|| tr_fmt!("No page {}", n))?;
        ids.push(pg.id);
    }
    let structure = structure.trim();
    if !structure.is_empty() {
        ids.extend(prj.pages.iter().filter(|p| crate::structure::page_structure(p).starts_with(structure)).map(|p| p.id));
    }
    let mut seen = BTreeSet::new();
    ids.retain(|id| seen.insert(*id));
    // In page order.
    ids.sort_by_key(|id| prj.pages.iter().position(|p| p.id == *id));
    Ok(ids)
}

fn tags_of(pages: &[Page]) -> BTreeSet<String> {
    let mut tags: BTreeSet<String> = pages.iter().flat_map(|p| p.symbols.iter()).filter(|s| !s.tag.is_empty()).map(|s| s.tag.clone()).collect();
    tags.extend(pages.iter().flat_map(|p| p.wires.iter()).filter(|w| !w.cable.is_empty()).map(|w| w.cable.clone()));
    tags.extend(pages.iter().flat_map(|p| p.cable_defs.iter()).map(|c| c.cable.clone()));
    tags
}

/// Takes pages out as a subproject: returns the subproject and records it (its pages
/// locked) in `prj`.
pub fn extract(prj: &mut Project, name: &str, description: &str, pages: &[u64], date: &str) -> Result<Project, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr("The subproject needs a name").into());
    }
    if prj.subprojects.child(name).is_some_and(|c| c.out) {
        return Err(tr_fmt!("Subproject {} is already out", name));
    }
    if pages.is_empty() {
        return Err(tr("Choose the pages of the subproject").into());
    }
    let chosen: BTreeSet<u64> = pages.iter().copied().collect();
    for c in prj.subprojects.children.iter().filter(|c| c.out) {
        if c.pages.iter().any(|p| chosen.contains(p)) {
            return Err(tr_fmt!("Pages of subproject {} cannot be taken out again", c.name));
        }
    }
    for &id in pages {
        let pg = prj.pages.iter().find(|p| p.id == id).ok_or_else(|| tr_fmt!("No page with id {}", id))?;
        if matches!(pg.kind, PageKind::Mounting(_)) {
            return Err(tr_fmt!("Page {}: mounting layout pages stay in the main project", pg.name));
        }
    }
    if let Some(o) = prj.model_options.parked_on(&chosen).first() {
        return Err(tr_fmt!("Activate option {} first: it hides content of these pages", o));
    }

    let mut sub = prj.clone();
    sub.pages.retain(|p| chosen.contains(&p.id));
    let tags = tags_of(&sub.pages);
    sub.devices.retain(|t, _| tags.contains(t));
    sub.cables.retain(|c| tags.contains(&c.tag));
    sub.strips.retain(|t, _| tags.contains(t));
    sub.plugs.retain(|t, _| tags.contains(t));
    sub.plc_cards.retain(|t, _| tags.contains(t));
    let used: BTreeSet<&str> = sub.devices.values().flat_map(|d| d.parts.iter().map(|p| p.number.as_str())).collect();
    sub.parts.retain(|p| used.contains(p.number.as_str()));
    // Panel layout, project history and plant data stay in the main project.
    sub.layout = Default::default();
    sub.revisions.clear();
    sub.baselines.clear();
    sub.review = Default::default();
    sub.wiring_marks.clear();
    sub.report_blocks.clear();
    sub.harnesses.clear();
    sub.field = Default::default();
    let ids: BTreeSet<u64> = sub.pages.iter().flat_map(|p| std::iter::once(p.id).chain(crate::options::item_ids(p))).collect();
    for o in &mut sub.model_options.options {
        o.members.retain(|id| ids.contains(id));
    }
    sub.model_options.options.retain(|o| !o.members.is_empty());
    sub.subprojects = Subprojects {
        parent: Some(Parent { project: prj.info.name.clone(), name: name.into(), pages: pages.to_vec(), exported: date.into() }),
        children: Vec::new(),
    };
    sub.info.name = name.into();
    if !description.is_empty() {
        sub.info.description = description.into();
    }
    // Reserve the ids the subproject creates.
    prj.fix_next_id();
    sub.next_id = prj.next_id;
    prj.next_id += ID_RANGE;

    let locked_before: Vec<u64> = prj.pages.iter().filter(|p| chosen.contains(&p.id) && p.locked).map(|p| p.id).collect();
    for pg in prj.pages.iter_mut().filter(|p| chosen.contains(&p.id)) {
        pg.locked = true;
    }
    let child = Child { name: name.into(), description: description.into(), pages: pages.to_vec(), exported: date.into(), inserted: String::new(), out: true, locked_before, first_id: sub.next_id };
    prj.subprojects.children.retain(|c| c.name != name);
    prj.subprojects.children.push(child);
    Ok(sub)
}

/// Result of inserting a subproject.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Inserted {
    pub name: String,
    pub pages_removed: usize,
    pub pages_inserted: usize,
    pub devices: usize,
    pub parts: usize,
    /// Objects that got new ids because theirs were taken meanwhile.
    pub renumbered: usize,
}

/// Inserts an edited subproject: its pages replace the pages taken out.
pub fn insert(prj: &mut Project, sub: &Project, date: &str) -> Result<Inserted, String> {
    let parent = sub.subprojects.parent.as_ref().ok_or_else(|| tr("This project is not a subproject").to_string())?;
    let child = prj.subprojects.child(&parent.name).cloned().ok_or_else(|| tr_fmt!("Subproject {} was not taken out of this project", parent.name))?;
    // Inserting again (or a file of an earlier extraction) would undo what was changed
    // here since.
    if !child.out {
        return Err(tr_fmt!("Subproject {} was inserted already", child.name));
    }
    if sub.next_id < child.first_id {
        return Err(tr_fmt!("The file is from an earlier extraction of subproject {}", child.name));
    }
    if let Some(o) = sub.model_options.options.iter().find(|o| !o.parked.is_empty()) {
        return Err(tr_fmt!("Activate option {} in the subproject first", o.name));
    }
    let old: BTreeSet<u64> = child.pages.iter().copied().collect();
    let removed: Vec<Page> = prj.pages.iter().filter(|p| old.contains(&p.id)).cloned().collect();
    let at = prj.pages.iter().position(|p| old.contains(&p.id)).unwrap_or(prj.pages.len());
    prj.pages.retain(|p| !old.contains(&p.id));

    // Ids used outside the replaced pages.
    let mut taken: BTreeSet<u64> = prj.pages.iter().flat_map(|p| std::iter::once(p.id).chain(crate::options::item_ids(p))).collect();
    prj.next_id = prj.next_id.max(sub.next_id);
    prj.fix_next_id();
    let mut pages = sub.pages.clone();
    let mut renumbered = 0;
    for pg in &mut pages {
        let mut fresh = |id: &mut u64, prj: &mut Project| {
            if !taken.insert(*id) {
                *id = prj.new_id();
                taken.insert(*id);
                renumbered += 1;
            }
        };
        fresh(&mut pg.id, prj);
        macro_rules! renumber {
            ($($list:ident),*) => { $(for x in &mut pg.$list { fresh(&mut x.id, prj); })* };
        }
        renumber!(symbols, wires, texts, shapes, dimensions, images, conn_defs, cable_defs);
    }
    let pages_inserted = pages.len();
    for (i, pg) in pages.into_iter().enumerate() {
        prj.pages.insert(at + i, pg);
    }

    // Data of the devices, cables and parts the pages use.
    let old_tags = tags_of(&removed);
    let now: BTreeSet<String> = tags_of(&prj.pages);
    for t in old_tags.iter().filter(|t| !now.contains(*t) && !sub.devices.contains_key(*t)) {
        prj.devices.remove(t);
        prj.strips.remove(t);
        prj.plugs.remove(t);
        prj.plc_cards.remove(t);
    }
    prj.cables.retain(|c| !old_tags.contains(&c.tag) || now.contains(&c.tag) || sub.cable(&c.tag).is_some());
    for (t, d) in &sub.devices {
        prj.devices.insert(t.clone(), d.clone());
    }
    for (t, d) in &sub.strips {
        prj.strips.insert(t.clone(), d.clone());
    }
    for (t, d) in &sub.plugs {
        prj.plugs.insert(t.clone(), d.clone());
    }
    for (t, d) in &sub.plc_cards {
        prj.plc_cards.insert(t.clone(), d.clone());
    }
    for c in &sub.cables {
        match prj.cables.iter_mut().find(|x| x.tag == c.tag) {
            Some(x) => *x = Cable { id: x.id, ..c.clone() },
            None => {
                let id = prj.new_id();
                prj.cables.push(Cable { id, ..c.clone() });
            }
        }
    }
    for p in &sub.parts {
        match prj.parts.iter_mut().find(|x| x.number == p.number) {
            Some(x) => *x = p.clone(),
            None => prj.parts.push(p.clone()),
        }
    }
    prj.property_db.merge(sub.property_db.clone());
    crate::symlib::merge(&mut prj.symbols, crate::symlib::SymbolLibrary { symbols: sub.symbols.to_vec() });

    // Option assignments of the inserted pages.
    let ids: BTreeSet<u64> = prj.pages.iter().skip(at).take(pages_inserted).flat_map(|p| std::iter::once(p.id).chain(crate::options::item_ids(p))).collect();
    let removed_ids: BTreeSet<u64> = removed.iter().flat_map(|p| std::iter::once(p.id).chain(crate::options::item_ids(p))).collect();
    for o in &mut prj.model_options.options {
        o.members.retain(|id| !removed_ids.contains(id));
    }
    for so in &sub.model_options.options {
        let members: Vec<u64> = so.members.iter().copied().filter(|id| ids.contains(id)).collect();
        if members.is_empty() {
            continue;
        }
        if prj.model_options.get(&so.name).is_none() {
            prj.model_options.options.push(crate::options::ModelOption { name: so.name.clone(), description: so.description.clone(), condition: so.condition.clone(), ..Default::default() });
        }
        crate::options::assign(prj, &so.name, &members)?;
    }

    let c = prj.subprojects.children.iter_mut().find(|c| c.name == child.name).expect("child exists");
    c.pages = prj.pages.iter().skip(at).take(pages_inserted).map(|p| p.id).collect();
    c.out = false;
    c.inserted = date.into();
    c.locked_before.clear();
    prj.normalize();
    Ok(Inserted { name: child.name, pages_removed: removed.len(), pages_inserted, devices: sub.devices.len(), parts: sub.parts.len(), renumbered })
}

/// Gives up a subproject that is out: its pages are unlocked again and the record is
/// removed.
pub fn discard(prj: &mut Project, name: &str) -> Result<(), String> {
    let child = prj.subprojects.child(name).cloned().ok_or_else(|| tr_fmt!("No subproject {}", name))?;
    if child.out {
        for pg in prj.pages.iter_mut().filter(|p| child.pages.contains(&p.id)) {
            pg.locked = child.locked_before.contains(&pg.id);
        }
    }
    prj.subprojects.children.retain(|c| c.name != name);
    Ok(())
}

#[cfg(test)]
mod tests;
