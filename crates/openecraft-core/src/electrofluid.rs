//! Fluid device groups (valve terminals, service units, manifolds, power units) and
//! the links between fluid devices and the electrical devices that actuate or monitor
//! them (solenoid coils, position and pressure switches): cross references in both
//! schematics, checks, and the synchronization that links solenoid valves with their
//! coils and copies function texts.

use openecraft_i18n::{tr, tr_fmt};
use crate::index::{Index, Loc};
use crate::model::*;
use crate::issue::Severity;
use crate::symbols::{Category, Func, SymbolDef, SymbolRef};
use std::collections::BTreeMap;
pub use crate::model::electrofluid::{DeviceGroup, GroupKind};

impl GroupKind {
    pub const ALL: [GroupKind; 5] = [GroupKind::ValveTerminal, GroupKind::ServiceUnit, GroupKind::Manifold, GroupKind::PowerUnit, GroupKind::Other];

    pub fn title(self) -> &'static str {
        match self {
            GroupKind::ValveTerminal => tr("Valve terminal"),
            GroupKind::ServiceUnit => tr("Service unit"),
            GroupKind::Manifold => tr("Manifold"),
            GroupKind::PowerUnit => tr("Hydraulic power unit"),
            GroupKind::Other => tr("Other"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            GroupKind::ValveTerminal => "valve_terminal",
            GroupKind::ServiceUnit => "service_unit",
            GroupKind::Manifold => "manifold",
            GroupKind::PowerUnit => "power_unit",
            GroupKind::Other => "other",
        }
    }

    pub fn from_code(code: &str) -> Option<GroupKind> {
        let c = code.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        GroupKind::ALL.into_iter().find(|k| k.code() == c)
    }
}

/// Solenoids of a fluid symbol.
pub fn solenoids(def: &SymbolDef) -> usize {
    match def.key {
        "fl_valve42" | "fl_valve52" => 1,
        _ => 0,
    }
}

pub fn is_valve(def: &SymbolDef) -> bool {
    def.category == Category::Fluid && def.letter == "QM"
}

/// Main function of a device: its symbol definition, instance and location.
pub fn main_of<'a>(prj: &'a Project, idx: &Index, tag: &str) -> Option<(SymbolRef, &'a SymbolInst, Loc)> {
    let loc = *idx.devices.get(tag)?.mains.first()?;
    let s = prj.pages[loc.page].symbols.iter().find(|s| s.id == loc.sym_id)?;
    Some((prj.symbol(&s.sym)?, s, loc))
}

/// Device whose main function is a fluid power or process symbol.
pub fn is_fluid_device(prj: &Project, idx: &Index, tag: &str) -> bool {
    main_of(prj, idx, tag).is_some_and(|(d, _, _)| d.is_fluid())
}

/// Identifying letters of a tag: `=A1-MB3` → `MB`.
pub fn letters(tag: &str) -> &str {
    let tail = tag.rsplit('-').next().unwrap_or("");
    let n = tail.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(tail.len());
    &tail[..n]
}

/// The tag with other identifying letters: `=A1-QM3`, `MB` → `=A1-MB3`.
pub fn with_letters(tag: &str, new: &str) -> Option<String> {
    let at = tag.rfind('-')?;
    let tail = &tag[at + 1..];
    Some(format!("{}-{new}{}", &tag[..at], &tail[letters(tag).len()..]))
}

/// Electrical devices linked to a fluid device.
pub fn linked<'a>(prj: &'a Project, tag: &str) -> &'a [String] {
    prj.fluid.links.get(tag).map(Vec::as_slice).unwrap_or(&[])
}

/// Fluid devices an electrical device is linked to.
pub fn fluid_devices_of<'a>(prj: &'a Project, tag: &str) -> Vec<&'a String> {
    prj.fluid.links.iter().filter(|(_, v)| v.iter().any(|t| t == tag)).map(|(k, _)| k).collect()
}

/// Cross references shown at the main function of `tag` on page `page_i`: the linked
/// devices of the other discipline with their location, e.g. `-MB1 /3.4`.
pub fn cross_refs(prj: &Project, idx: &Index, page_i: usize, tag: &str) -> Vec<(String, Option<usize>)> {
    crate::references::fluid(prj, idx, page_i, tag)
}

/// Group a fluid device belongs to.
pub fn group_of<'a>(prj: &'a Project, tag: &str) -> Option<&'a DeviceGroup> {
    prj.fluid.groups.iter().find(|g| g.members.iter().any(|m| m == tag))
}

/// One fluid device with its electrical counterparts (electro-fluid cross reference).
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub tag: String,
    pub function: String,
    pub location: String,
    pub solenoids: usize,
    /// (tag, function, location; empty when not in the schematic)
    pub electrical: Vec<(String, String, String)>,
    pub target: Option<(usize, u64)>,
}

/// Fluid power devices that have solenoids or links, in tag order.
pub fn entries(prj: &Project, idx: &Index) -> Vec<Entry> {
    let mut v = Vec::new();
    for tag in idx.sorted_tags() {
        let Some((def, _, loc)) = main_of(prj, idx, tag).filter(|(d, _, _)| d.category == Category::Fluid) else { continue };
        let links = linked(prj, tag);
        if solenoids(&def) == 0 && links.is_empty() {
            continue;
        }
        let electrical = links
            .iter()
            .map(|t| match main_of(prj, idx, t) {
                Some((d, _, l)) => (t.clone(), tr(d.name).to_string(), l.reference(prj)),
                None => (t.clone(), String::new(), String::new()),
            })
            .collect();
        v.push(Entry { tag: tag.clone(), function: tr(def.name).into(), location: loc.reference(prj), solenoids: solenoids(&def), electrical, target: Some((loc.page, loc.sym_id)) });
    }
    v
}

/// Device groups and electro-fluid links: members missing or in several groups,
/// full valve terminals, valves without coils, links to missing or wrong devices,
/// solenoid coils without valve.
pub fn problems(prj: &Project, idx: &Index) -> Vec<crate::fluid::Problem> {
    let mut v = Vec::new();
    let target = |tag: &str| main_of(prj, idx, tag).map(|(_, _, l)| (l.page, l.sym_id));
    let mut membership: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (i, g) in prj.fluid.groups.iter().enumerate() {
        if prj.fluid.groups[..i].iter().any(|o| o.tag == g.tag) {
            v.push((Severity::Error, tr_fmt!("Device group {} defined twice", g.tag), None));
        }
        if g.slots > 0 && g.members.len() > g.slots as usize {
            v.push((Severity::Error, tr_fmt!("Device group {}: {} devices on {} places", g.tag, g.members.len(), g.slots), None));
        }
        for m in &g.members {
            membership.entry(m).or_default().push(&g.tag);
            match main_of(prj, idx, m) {
                None => v.push((Severity::Warning, tr_fmt!("Device group {}: {} is not in the schematic", g.tag, m), None)),
                Some((d, _, _)) if !d.is_fluid() => v.push((Severity::Warning, tr_fmt!("Device group {}: {} is not a fluid device", g.tag, m), target(m))),
                Some((d, _, _)) if g.kind == GroupKind::ValveTerminal && !is_valve(&d) => {
                    v.push((Severity::Warning, tr_fmt!("Device group {}: {} is not a valve", g.tag, m), target(m)))
                }
                _ => {}
            }
        }
    }
    for (m, groups) in membership {
        if groups.len() > 1 {
            v.push((Severity::Error, tr_fmt!("{}: member of several device groups ({})", m, groups.join(", ")), target(m)));
        }
    }
    for (tag, els) in &prj.fluid.links {
        if main_of(prj, idx, tag).is_none() {
            v.push((Severity::Warning, tr_fmt!("{}: linked fluid device is not in the schematic", tag), None));
        }
        for e in els {
            match main_of(prj, idx, e) {
                None => v.push((Severity::Warning, tr_fmt!("{}: linked electrical device {} is not in the schematic", tag, e), target(tag))),
                Some((d, _, _)) if d.is_fluid() => v.push((Severity::Error, tr_fmt!("{}: {} is a fluid device, not an electrical one", tag, e), target(tag))),
                _ => {}
            }
        }
    }
    for tag in idx.sorted_tags() {
        let Some((def, _, _)) = main_of(prj, idx, tag) else { continue };
        let coils = linked(prj, tag).iter().filter(|t| main_of(prj, idx, t).is_some_and(|(d, _, _)| d.category == Category::Coils)).count();
        let n = solenoids(&def);
        if n > 0 && coils == 0 {
            v.push((Severity::Warning, tr_fmt!("{}: solenoid valve without linked solenoid coil (Fluid and process › Synchronize)", tag), target(tag)));
        } else if n > 0 && coils > n {
            v.push((Severity::Warning, tr_fmt!("{}: {} solenoid coils linked, the valve has {}", tag, coils, n), target(tag)));
        }
        if def.is_fluid() {
            continue;
        }
        let fluid = fluid_devices_of(prj, tag);
        if fluid.len() > 1 {
            let names: Vec<&str> = fluid.iter().map(|s| s.as_str()).collect();
            v.push((Severity::Warning, tr_fmt!("{}: linked to several fluid devices ({})", tag, names.join(", ")), target(tag)));
        } else if fluid.is_empty() && letters(tag) == "MB" && def.category == Category::Coils && matches!(def.func, Func::Main { .. }) {
            v.push((Severity::Info, tr_fmt!("{}: solenoid coil not linked to a valve", tag), target(tag)));
        }
    }
    v
}

/// Result of [`synchronize`].
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sync {
    /// New links (fluid device, electrical device).
    pub linked: Vec<(String, String)>,
    /// Function texts copied.
    pub texts: usize,
}

/// Links solenoid valves without coil with the coil of the same structure and
/// counting number (`=A1-QM3` → `=A1-MB3`), and copies the function text of linked
/// devices to the main function of the other discipline where it is empty.
pub fn synchronize(prj: &mut Project) -> Sync {
    let idx = Index::build(prj);
    let mut sync = Sync::default();
    for tag in idx.sorted_tags() {
        let Some((def, _, _)) = main_of(prj, &idx, tag) else { continue };
        if solenoids(&def) == 0 || linked(prj, tag).iter().any(|t| main_of(prj, &idx, t).is_some_and(|(d, _, _)| d.category == Category::Coils)) {
            continue;
        }
        let Some(coil) = with_letters(tag, "MB") else { continue };
        let is_coil = main_of(prj, &idx, &coil).is_some_and(|(d, _, _)| d.category == Category::Coils);
        if is_coil && fluid_devices_of(prj, &coil).is_empty() {
            prj.fluid.links.entry(tag.clone()).or_default().push(coil.clone());
            sync.linked.push((tag.clone(), coil));
        }
    }
    let pairs: Vec<(String, String)> = prj.fluid.links.iter().flat_map(|(f, els)| els.iter().map(move |e| (f.clone(), e.clone()))).collect();
    for (f, e) in pairs {
        let text = |prj: &Project, tag: &str| main_of(prj, &idx, tag).map(|(_, s, l)| (s.function_text.clone(), l));
        let (Some((tf, lf)), Some((te, le))) = (text(prj, &f), text(prj, &e)) else { continue };
        let (from, to) = match (tf.is_empty(), te.is_empty()) {
            (false, true) => (tf, le),
            (true, false) => (te, lf),
            _ => continue,
        };
        if let Some(s) = prj.pages[to.page].symbols.iter_mut().find(|s| s.id == to.sym_id) {
            s.function_text = from;
            sync.texts += 1;
        }
    }
    sync
}

/// Follows a renamed device in the groups and links.
pub fn rename_device(prj: &mut Project, old: &str, new: &str) {
    let f = &mut prj.fluid;
    for g in &mut f.groups {
        for t in std::iter::once(&mut g.tag).chain(g.members.iter_mut()) {
            if t == old {
                *t = new.to_string();
            }
        }
    }
    if let Some(els) = f.links.remove(old) {
        f.links.entry(new.to_string()).or_default().extend(els);
    }
    for t in f.links.values_mut().flat_map(|v| v.iter_mut()) {
        if t == old {
            *t = new.to_string();
        }
    }
}

#[cfg(test)]
mod tests;
