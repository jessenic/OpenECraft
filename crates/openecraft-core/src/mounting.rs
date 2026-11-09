//! Mounting assistant (in the spirit of EPLAN Smart Mounting): the items of the panel
//! layout as a checklist for the workshop in mounting order – cutouts, then rails,
//! ducts and busbars, then the devices on the plate, the accessories, and last the
//! items on the door and the walls – with their place, size and part. Mounted and
//! problem marks are stored in the project with the item's place and part at that
//! time, so items moved or changed afterwards show up.

use crate::model::panel::{Enclosure, Face, ItemKind, PanelItem};
use crate::model::Project;
pub use crate::model::mounting::MountingMark;
pub use crate::model::wiring::Mark;
use openecraft_i18n::tr;
use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Open,
    Done,
    /// Mounted, but moved or given another part since.
    Changed,
    Problem,
}

impl Status {
    pub fn title(self) -> &'static str {
        match self {
            Status::Open => tr("Open"),
            Status::Done => tr("Mounted"),
            Status::Changed => tr("Changed since mounted"),
            Status::Problem => tr("Problem"),
        }
    }
}

/// Mounting phases in their order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Cutouts,
    Carriers,
    Devices,
    Accessories,
    DoorAndWalls,
}

impl Phase {
    pub fn title(self) -> &'static str {
        match self {
            Phase::Cutouts => tr("Cutouts"),
            Phase::Carriers => tr("Mounting plates, rails, ducts and busbars"),
            Phase::Devices => tr("Devices"),
            Phase::Accessories => tr("Accessories"),
            Phase::DoorAndWalls => tr("Door and walls"),
        }
    }

    pub fn of(it: &PanelItem) -> Phase {
        match (it.face, it.kind) {
            (f, _) if f != Face::Plate => Phase::DoorAndWalls,
            (_, ItemKind::Cutout) => Phase::Cutouts,
            (_, ItemKind::Rail | ItemKind::Duct | ItemKind::Busbar | ItemKind::MountingPlate) => Phase::Carriers,
            (_, ItemKind::Device) => Phase::Devices,
            (_, ItemKind::Accessory) => Phase::Accessories,
        }
    }
}

/// One item of the checklist.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub enclosure: usize,
    pub item: usize,
    pub id: u64,
    pub phase: Phase,
    pub status: Status,
    pub note: String,
}

pub fn mark_of(prj: &Project, id: u64) -> Option<&MountingMark> {
    prj.mounting_marks.iter().find(|m| m.item == id)
}

pub fn status(prj: &Project, it: &PanelItem) -> Status {
    match mark_of(prj, it.id) {
        None => Status::Open,
        Some(m) if m.mark == Mark::Problem => Status::Problem,
        Some(m) if (m.x - it.x).abs() > 0.5 || (m.y - it.y).abs() > 0.5 || m.part != it.part => Status::Changed,
        Some(_) => Status::Done,
    }
}

/// The items of an enclosure (all with `None`) in mounting order: by phase, then top
/// to bottom and left to right.
pub fn steps(prj: &Project, enclosure: Option<usize>) -> Vec<Step> {
    let mut v = Vec::new();
    for (ei, e) in prj.layout.enclosures.iter().enumerate().filter(|(i, _)| enclosure.is_none_or(|e| e == *i)) {
        for (ii, it) in e.items.iter().enumerate() {
            v.push(Step { enclosure: ei, item: ii, id: it.id, phase: Phase::of(it), status: status(prj, it), note: mark_of(prj, it.id).map(|m| m.note.clone()).unwrap_or_default() });
        }
    }
    let item = |s: &Step| &prj.layout.enclosures[s.enclosure].items[s.item];
    v.sort_by(|a, b| {
        (a.enclosure, a.phase, item(a).face.order())
            .cmp(&(b.enclosure, b.phase, item(b).face.order()))
            .then(item(a).y.total_cmp(&item(b).y))
            .then(item(a).x.total_cmp(&item(b).x))
    });
    v
}

/// Marks an item as mounted / problem, or open again with `None`.
pub fn set_mark(prj: &mut Project, it: &PanelItem, mark: Option<Mark>, note: &str) {
    prj.mounting_marks.retain(|m| m.item != it.id);
    if let Some(mark) = mark {
        prj.mounting_marks.push(MountingMark { item: it.id, mark, note: note.trim().to_string(), x: it.x, y: it.y, part: it.part.clone() });
    }
}

/// The next step after `after` that needs work (wrapping around).
pub fn next_open(steps: &[Step], after: Option<usize>) -> Option<usize> {
    let n = steps.len();
    let start = after.map_or(0, |i| i + 1);
    (0..n).map(|k| (start + k) % n).find(|&i| steps[i].status != Status::Done)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub total: usize,
    pub done: usize,
    pub changed: usize,
    pub problems: usize,
}

impl Progress {
    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            (self.done - self.changed) as f32 / self.total as f32
        }
    }
}

pub fn progress(prj: &Project, enclosure: Option<usize>) -> Progress {
    let mut p = Progress::default();
    for s in steps(prj, enclosure) {
        p.total += 1;
        match s.status {
            Status::Done => p.done += 1,
            Status::Changed => {
                p.done += 1;
                p.changed += 1;
            }
            Status::Problem => p.problems += 1,
            Status::Open => {}
        }
    }
    p
}

/// What an item is, for lists: the device tag, the part or the kind.
pub fn item_title(it: &PanelItem) -> String {
    let kind = match it.kind {
        ItemKind::Device => tr("Device"),
        ItemKind::Rail => tr("Mounting rail"),
        ItemKind::Duct => tr("Cable duct"),
        ItemKind::Busbar => tr("Busbar"),
        ItemKind::Cutout => tr("Cutout"),
        ItemKind::Accessory => tr("Accessory"),
        ItemKind::MountingPlate => tr("Mounting plate"),
    };
    match (it.tag.is_empty(), it.part.is_empty()) {
        (false, _) => format!("{} ({kind})", it.tag),
        (true, false) => format!("{kind} {}", it.part),
        (true, true) => kind.to_string(),
    }
}

/// Marks of items that no longer exist.
pub fn stale(prj: &Project) -> Vec<u64> {
    let ids: std::collections::HashSet<u64> = prj.layout.enclosures.iter().flat_map(|e| e.items.iter().map(|i| i.id)).collect();
    prj.mounting_marks.iter().filter(|m| !ids.contains(&m.item)).map(|m| m.item).collect()
}

pub fn remove_stale(prj: &mut Project) -> usize {
    let old = stale(prj);
    prj.mounting_marks.retain(|m| !old.contains(&m.item));
    old.len()
}

/// The checklist as CSV (`Enclosure;Phase;Item;Part;Face;X;Y;Width;Height;State;Note`).
pub fn csv(prj: &Project, enclosure: Option<usize>) -> String {
    let cell = |s: &str| if s.contains([';', '"', '\n']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() };
    let mut out = String::from("Enclosure;Phase;Item;Part;Face;X;Y;Width;Height;State;Note\n");
    for s in steps(prj, enclosure) {
        let e: &Enclosure = &prj.layout.enclosures[s.enclosure];
        let it = &e.items[s.item];
        let _ = writeln!(
            out,
            "{};{};{};{};{:?};{:.0};{:.0};{:.0};{:.0};{};{}",
            cell(&e.name),
            cell(s.phase.title()),
            cell(&item_title(it)),
            cell(&it.part),
            it.face,
            it.x,
            it.y,
            it.w,
            it.h,
            cell(s.status.title()),
            cell(&s.note)
        );
    }
    out
}

#[cfg(test)]
mod tests;
