//! Comparing two states of a project and taking differences over: another project
//! file, a baseline (stored state of the whole project) or a revision (pages only).
//! Elements are matched by id (pages, cables, enclosures) or by name (devices,
//! parts, potentials, strips, plugs, PLC cards); everything else is compared per
//! section.

use crate::model::{natural_cmp, Page, Project};
use crate::revision::{ChangeKind, Snapshot};
use openecraft_i18n::{tr, tr_fmt};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
pub use crate::model::compare::{Baseline, Stored};

impl PartialEq for Stored {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0 == other.0
    }
}

/// The project as stored in a baseline: without revisions, baselines and review.
pub fn stored_copy(prj: &Project) -> Project {
    let mut copy = prj.clone();
    copy.revisions.clear();
    copy.baselines.clear();
    copy.review = Default::default();
    copy
}

pub fn add_baseline(prj: &mut Project, name: &str, date: &str, author: &str, description: &str) {
    let project = Stored(Arc::new(stored_copy(prj)));
    prj.baselines.push(Baseline { name: name.into(), date: date.into(), author: author.into(), description: description.into(), project });
}

/// The project with the pages of revision `i` (for comparing with a revision).
pub fn with_revision_pages(prj: &Project, pages: &Snapshot) -> Project {
    let mut copy = stored_copy(prj);
    copy.pages = pages.0.as_ref().clone();
    copy
}

/// Parts of the project compared as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Info,
    WireNumbering,
    Wires,
    Layout,
    PlcHardware,
    Symbols,
    Forms,
    ReportBlocks,
    Configuration,
    Marking,
    Fluid,
    Process,
    Languages,
    Structures,
    PageTemplates,
    Layers,
    Workflow,
}

impl Section {
    const ALL: [Section; 17] = [
        Section::Info,
        Section::WireNumbering,
        Section::Wires,
        Section::Layout,
        Section::PlcHardware,
        Section::Symbols,
        Section::Forms,
        Section::ReportBlocks,
        Section::Configuration,
        Section::Marking,
        Section::Fluid,
        Section::Process,
        Section::Languages,
        Section::Structures,
        Section::PageTemplates,
        Section::Layers,
        Section::Workflow,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Section::Info => tr("Project properties"),
            Section::WireNumbering => tr("Wire numbering"),
            Section::Wires => tr("Wire colours and cross-sections"),
            Section::Layout => tr("Panel layout settings"),
            Section::PlcHardware => tr("PLC hardware"),
            Section::Symbols => tr("Project symbols"),
            Section::Forms => tr("Title blocks and report forms"),
            Section::ReportBlocks => tr("Report blocks"),
            Section::Configuration => tr("Configurator"),
            Section::Marking => tr("Marking"),
            Section::Fluid => tr("Fluid power settings"),
            Section::Process => tr("PCT loops"),
            Section::Languages => tr("Languages and dictionary"),
            Section::Structures => tr("Structure identifiers"),
            Section::PageTemplates => tr("Page templates"),
            Section::Layers => tr("Layers"),
            Section::Workflow => tr("Approvers"),
        }
    }

    fn same(self, a: &Project, b: &Project) -> bool {
        match self {
            Section::Info => a.info == b.info,
            Section::WireNumbering => a.wire_numbering == b.wire_numbering,
            Section::Wires => (&a.wire_defaults, a.wire_colors, &a.wire_settings) == (&b.wire_defaults, b.wire_colors, &b.wire_settings),
            Section::Layout => layout_settings(a) == layout_settings(b),
            Section::PlcHardware => a.plc == b.plc,
            Section::Symbols => a.symbols == b.symbols,
            Section::Forms => a.forms == b.forms,
            Section::ReportBlocks => a.report_blocks == b.report_blocks,
            Section::Configuration => a.configuration == b.configuration,
            Section::Marking => a.marking == b.marking,
            Section::Fluid => a.fluid == b.fluid,
            Section::Process => a.pct_loops == b.pct_loops,
            Section::Languages => a.languages == b.languages,
            Section::Structures => a.structures == b.structures,
            Section::PageTemplates => a.page_templates == b.page_templates,
            Section::Layers => a.layers == b.layers,
            Section::Workflow => a.workflow == b.workflow,
        }
    }

    fn take(self, to: &mut Project, from: &Project) {
        match self {
            Section::Info => to.info = from.info.clone(),
            Section::WireNumbering => to.wire_numbering = from.wire_numbering.clone(),
            Section::Wires => {
                to.wire_defaults = from.wire_defaults.clone();
                to.wire_colors = from.wire_colors;
                to.wire_settings = from.wire_settings.clone();
            }
            Section::Layout => {
                let enclosures = std::mem::take(&mut to.layout.enclosures);
                to.layout = layout_settings(from);
                to.layout.enclosures = enclosures;
            }
            Section::PlcHardware => to.plc = from.plc.clone(),
            Section::Symbols => to.symbols = from.symbols.clone(),
            Section::Forms => to.forms = from.forms.clone(),
            Section::ReportBlocks => to.report_blocks = from.report_blocks.clone(),
            Section::Configuration => to.configuration = from.configuration.clone(),
            Section::Marking => to.marking = from.marking.clone(),
            Section::Fluid => to.fluid = from.fluid.clone(),
            Section::Process => to.pct_loops = from.pct_loops.clone(),
            Section::Languages => to.languages = from.languages.clone(),
            Section::Structures => to.structures = from.structures.clone(),
            Section::PageTemplates => to.page_templates = from.page_templates.clone(),
            Section::Layers => to.layers = from.layers.clone(),
            Section::Workflow => to.workflow = from.workflow.clone(),
        }
    }
}

fn layout_settings(prj: &Project) -> crate::panel::Layout {
    crate::panel::Layout { enclosures: Vec::new(), ..prj.layout.clone() }
}

/// An element that differs.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Item {
    Page(u64),
    PageOrder,
    Device(String),
    Part(String),
    Potential(String),
    Cable(u64),
    Harness(u64),
    Enclosure(u64),
    Strip(String),
    Plug(String),
    PlcCard(String),
    Section(Section),
}

impl Item {
    /// Kind of element, e.g. "Device".
    pub fn kind_title(&self) -> &'static str {
        match self {
            Item::Page(_) | Item::PageOrder => tr("Page"),
            Item::Device(_) => tr("Device"),
            Item::Part(_) => tr("Part"),
            Item::Potential(_) => tr("Potential"),
            Item::Cable(_) => tr("Cable"),
            Item::Harness(_) => tr("Harness"),
            Item::Enclosure(_) => tr("Enclosure"),
            Item::Strip(_) => tr("Terminal strip"),
            Item::Plug(_) => tr("Plug"),
            Item::PlcCard(_) => tr("PLC card"),
            Item::Section(_) => tr("Settings"),
        }
    }
}

/// A difference of the current project (`Added`: only in the current project,
/// `Removed`: only in the other one) with a name and details.
#[derive(Clone, Debug, PartialEq)]
pub struct Difference {
    pub item: Item,
    pub kind: ChangeKind,
    pub name: String,
    pub details: Vec<String>,
}

fn keyed<T: PartialEq>(
    current: &[T],
    other: &[T],
    key: impl Fn(&T) -> String,
    item: impl Fn(String) -> Item,
    out: &mut Vec<Difference>,
) {
    for c in current {
        let k = key(c);
        match other.iter().find(|o| key(o) == k) {
            None => out.push(Difference { item: item(k.clone()), kind: ChangeKind::Added, name: k, details: Vec::new() }),
            Some(o) if o != c => out.push(Difference { item: item(k.clone()), kind: ChangeKind::Modified, name: k, details: Vec::new() }),
            Some(_) => {}
        }
    }
    for o in other.iter().filter(|o| !current.iter().any(|c| key(c) == key(o))) {
        let k = key(o);
        out.push(Difference { item: item(k.clone()), kind: ChangeKind::Removed, name: k, details: Vec::new() });
    }
}

fn mapped<T: PartialEq>(current: &BTreeMap<String, T>, other: &BTreeMap<String, T>, item: impl Fn(String) -> Item, out: &mut Vec<Difference>) {
    let keys: BTreeSet<&String> = current.keys().chain(other.keys()).collect();
    for k in keys {
        let kind = match (current.get(k), other.get(k)) {
            (Some(_), None) => ChangeKind::Added,
            (None, Some(_)) => ChangeKind::Removed,
            (Some(a), Some(b)) if a != b => ChangeKind::Modified,
            _ => continue,
        };
        out.push(Difference { item: item(k.clone()), kind, name: k.clone(), details: Vec::new() });
    }
}

fn page_name(pg: &Page) -> String {
    let name = crate::structure::page_full_name(pg);
    if pg.description.is_empty() {
        name
    } else {
        format!("{name} {}", pg.description)
    }
}

/// Differences of `current` to `other`.
pub fn diff(current: &Project, other: &Project) -> Vec<Difference> {
    let mut out = Vec::new();
    for pc in crate::revision::compare(&other.pages, &current.pages) {
        let page = current.pages.iter().chain(other.pages.iter()).find(|p| p.id == pc.page_id);
        let whole = pc.changes.iter().find(|c| c.what == "Page" && c.kind != ChangeKind::Modified);
        let kind = whole.map(|c| c.kind).unwrap_or(ChangeKind::Modified);
        let details = if whole.is_some() {
            Vec::new()
        } else {
            pc.changes.iter().map(|c| format!("{} {} {}", tr(c.what), c.kind.title(), c.label).trim().to_string()).collect()
        };
        out.push(Difference { item: Item::Page(pc.page_id), kind, name: page.map(page_name).unwrap_or(pc.name), details });
    }
    // Pages in another order.
    let order = |p: &Project| p.pages.iter().map(|pg| pg.id).filter(|id| current.pages.iter().any(|c| c.id == *id) && other.pages.iter().any(|o| o.id == *id)).collect::<Vec<_>>();
    if order(current) != order(other) && !out.iter().any(|d| matches!(d.item, Item::Page(_))) {
        out.push(Difference { item: Item::PageOrder, kind: ChangeKind::Modified, name: tr("Page order").into(), details: Vec::new() });
    }
    mapped(&current.devices, &other.devices, Item::Device, &mut out);
    keyed(&current.parts, &other.parts, |p| p.number.clone(), Item::Part, &mut out);
    keyed(&current.potentials, &other.potentials, |p| p.name.clone(), Item::Potential, &mut out);
    let cable = |prj: &Project, id: u64| prj.cables.iter().find(|c| c.id == id).map(|c| c.tag.clone()).unwrap_or_default();
    let mut ids = Vec::new();
    keyed(&current.cables, &other.cables, |c| c.id.to_string(), |k| Item::Cable(k.parse().unwrap_or(0)), &mut ids);
    keyed(&current.harnesses, &other.harnesses, |h| h.id.to_string(), |k| Item::Harness(k.parse().unwrap_or(0)), &mut ids);
    keyed(&current.layout.enclosures, &other.layout.enclosures, |e| e.id.to_string(), |k| Item::Enclosure(k.parse().unwrap_or(0)), &mut ids);
    for mut d in ids {
        d.name = match d.item {
            Item::Cable(id) => Some(cable(current, id)).filter(|t| !t.is_empty()).unwrap_or_else(|| cable(other, id)),
            Item::Harness(id) => current.harnesses.iter().chain(other.harnesses.iter()).find(|h| h.id == id).map(|h| h.tag.clone()).unwrap_or_default(),
            Item::Enclosure(id) => current.layout.enclosures.iter().chain(other.layout.enclosures.iter()).find(|e| e.id == id).map(|e| e.name.clone()).unwrap_or_default(),
            _ => d.name,
        };
        out.push(d);
    }
    mapped(&current.strips, &other.strips, Item::Strip, &mut out);
    mapped(&current.plugs, &other.plugs, Item::Plug, &mut out);
    mapped(&current.plc_cards, &other.plc_cards, Item::PlcCard, &mut out);
    for s in Section::ALL {
        if !s.same(current, other) {
            out.push(Difference { item: Item::Section(s), kind: ChangeKind::Modified, name: s.title().into(), details: Vec::new() });
        }
    }
    out.sort_by(|a, b| {
        item_rank(&a.item).cmp(&item_rank(&b.item)).then_with(|| page_pos(current, other, &a.item).cmp(&page_pos(current, other, &b.item))).then_with(|| natural_cmp(&a.name, &b.name))
    });
    out
}

fn item_rank(item: &Item) -> u8 {
    match item {
        Item::Page(_) | Item::PageOrder => 0,
        Item::Device(_) => 1,
        Item::Part(_) => 2,
        Item::Potential(_) => 3,
        Item::Cable(_) => 4,
        Item::Harness(_) => 5,
        Item::Enclosure(_) => 6,
        Item::Strip(_) => 7,
        Item::Plug(_) => 8,
        Item::PlcCard(_) => 9,
        Item::Section(_) => 10,
    }
}

fn page_pos(current: &Project, other: &Project, item: &Item) -> usize {
    let Item::Page(id) = item else { return 0 };
    current.pages.iter().position(|p| p.id == *id).or_else(|| other.pages.iter().position(|p| p.id == *id)).unwrap_or(usize::MAX)
}

/// Every object id in use.
fn used_ids(prj: &Project) -> BTreeSet<u64> {
    let mut ids = BTreeSet::new();
    for pg in &prj.pages {
        ids.insert(pg.id);
        ids.extend(pg.symbols.iter().map(|s| s.id));
        ids.extend(pg.wires.iter().map(|s| s.id));
        ids.extend(pg.texts.iter().map(|s| s.id));
        ids.extend(pg.shapes.iter().map(|s| s.id));
    }
    for e in &prj.layout.enclosures {
        ids.insert(e.id);
        ids.extend(e.items.iter().map(|i| i.id));
    }
    ids.extend(prj.cables.iter().map(|c| c.id));
    ids.extend(prj.harnesses.iter().map(|h| h.id));
    ids
}

/// Gives the objects of a page taken from another project new ids where they
/// are not the ones of the page it replaces (`keep`).
fn renumber_page(prj: &mut Project, page: &mut Page, keep: &BTreeSet<u64>) {
    let mut fresh = |id: &mut u64| {
        if !keep.contains(id) {
            *id = prj.new_id();
        }
    };
    fresh(&mut page.id);
    page.symbols.iter_mut().for_each(|s| fresh(&mut s.id));
    page.wires.iter_mut().for_each(|s| fresh(&mut s.id));
    page.texts.iter_mut().for_each(|s| fresh(&mut s.id));
    page.shapes.iter_mut().for_each(|s| fresh(&mut s.id));
}

fn take_keyed<T: Clone>(to: &mut Vec<T>, from: &[T], key: impl Fn(&T) -> bool) {
    match (to.iter().position(&key), from.iter().find(|x| key(x))) {
        (Some(i), Some(x)) => to[i] = x.clone(),
        (Some(i), None) => {
            to.remove(i);
        }
        (None, Some(x)) => to.push(x.clone()),
        (None, None) => {}
    }
}

fn take_mapped<T: Clone>(to: &mut BTreeMap<String, T>, from: &BTreeMap<String, T>, key: &str) {
    match from.get(key) {
        Some(v) => {
            to.insert(key.to_string(), v.clone());
        }
        None => {
            to.remove(key);
        }
    }
}

/// Makes `item` of `to` equal to that of `from` (inserts, replaces or removes it).
pub fn take(to: &mut Project, from: &Project, item: &Item) {
    match item {
        Item::Page(id) => {
            let at = to.pages.iter().position(|p| p.id == *id);
            match (at, from.pages.iter().position(|p| p.id == *id)) {
                (Some(i), None) => {
                    to.pages.remove(i);
                }
                (at, Some(fi)) => {
                    let mut page = from.pages[fi].clone();
                    let keep: BTreeSet<u64> = match at {
                        Some(i) => {
                            let old = &to.pages[i];
                            let mut k: BTreeSet<u64> = [old.id].into();
                            k.extend(old.symbols.iter().map(|s| s.id));
                            k.extend(old.wires.iter().map(|s| s.id));
                            k.extend(old.texts.iter().map(|s| s.id));
                            k.extend(old.shapes.iter().map(|s| s.id));
                            k
                        }
                        // A new page keeps the ids that are free.
                        None => {
                            let used = used_ids(to);
                            let mut k: BTreeSet<u64> = [page.id].into();
                            k.extend(page.symbols.iter().map(|s| s.id));
                            k.extend(page.wires.iter().map(|s| s.id));
                            k.extend(page.texts.iter().map(|s| s.id));
                            k.extend(page.shapes.iter().map(|s| s.id));
                            k.into_iter().filter(|id| !used.contains(id) && *id < to.next_id).collect()
                        }
                    };
                    renumber_page(to, &mut page, &keep);
                    match at {
                        Some(i) => to.pages[i] = page,
                        None => {
                            // After the page that precedes it in `from`.
                            let before = from.pages[..fi].iter().rev().find_map(|p| to.pages.iter().position(|t| t.id == p.id));
                            let i = before.map(|b| b + 1).unwrap_or(0);
                            to.pages.insert(i, page);
                        }
                    }
                }
                (None, None) => {}
            }
        }
        Item::PageOrder => {
            // Pages of both in the order of `from`; others stay behind their predecessor.
            let mut key = -1.0;
            let mut keys = BTreeMap::new();
            for pg in &to.pages {
                key = match from.pages.iter().position(|f| f.id == pg.id) {
                    Some(i) => i as f64,
                    None => key + 0.001,
                };
                keys.insert(pg.id, key);
            }
            to.pages.sort_by(|a, b| keys[&a.id].total_cmp(&keys[&b.id]));
        }
        Item::Device(k) => take_mapped(&mut to.devices, &from.devices, k),
        Item::Part(k) => take_keyed(&mut to.parts, &from.parts, |p| p.number == *k),
        Item::Potential(k) => take_keyed(&mut to.potentials, &from.potentials, |p| p.name == *k),
        Item::Cable(id) => take_keyed(&mut to.cables, &from.cables, |c| c.id == *id),
        Item::Harness(id) => take_keyed(&mut to.harnesses, &from.harnesses, |h| h.id == *id),
        Item::Enclosure(id) => take_keyed(&mut to.layout.enclosures, &from.layout.enclosures, |e| e.id == *id),
        Item::Strip(k) => take_mapped(&mut to.strips, &from.strips, k),
        Item::Plug(k) => take_mapped(&mut to.plugs, &from.plugs, k),
        Item::PlcCard(k) => take_mapped(&mut to.plc_cards, &from.plc_cards, k),
        Item::Section(s) => s.take(to, from),
    }
}

/// Takes several items over; returns a message for the status line.
pub fn take_all(to: &mut Project, from: &Project, items: &[Item]) -> String {
    for item in items {
        take(to, from, item);
    }
    tr_fmt!("{} differences taken over", items.len())
}

#[cfg(test)]
mod tests;
