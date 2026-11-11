//! Wiring assistant (in the spirit of EPLAN Smart Wiring): the routed wires of the
//! panel layout as a checklist for the workshop, in a useful order, with the end
//! treatment of each wire and the progress. Wired and problem marks are stored in the
//! project together with the wire's properties at that time, so wires changed in the
//! schematic after they were wired show up.

use openecraft_i18n::tr;
use crate::model::{natural_cmp, Project};
use crate::panel::RoutedWire;
use crate::production::{cross_section_mm2, EndTreatment, Ends, Twin};
use std::fmt::Write;
pub use crate::model::wiring::{Mark, WiringMark};

impl WiringMark {
    fn matches(&self, w: &RoutedWire) -> bool {
        (self.from == w.from && self.to == w.to) || (self.from == w.to && self.to == w.from)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Open,
    Done,
    /// Wired, but its colour, cross-section or number changed since.
    Changed,
    Problem,
}

impl Status {
    pub fn title(self) -> &'static str {
        match self {
            Status::Open => tr("Open"),
            Status::Done => tr("Wired"),
            Status::Changed => tr("Changed since wired"),
            Status::Problem => tr("Problem"),
        }
    }
}

/// Order of the wiring steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Order {
    /// Grouped by the device the wire starts at.
    #[default]
    Device,
    /// Top left of the mounting plate first.
    Position,
    /// Thickest wires first (power before control).
    CrossSection,
    Number,
}

impl Order {
    pub const ALL: [Order; 4] = [Order::Device, Order::Position, Order::CrossSection, Order::Number];

    pub fn title(self) -> &'static str {
        match self {
            Order::Device => tr("By device"),
            Order::Position => tr("By position (top left first)"),
            Order::CrossSection => tr("By cross-section (thickest first)"),
            Order::Number => tr("By wire number"),
        }
    }

    pub fn code(self) -> &'static str {
        match self {
            Order::Device => "device",
            Order::Position => "position",
            Order::CrossSection => "cross_section",
            Order::Number => "number",
        }
    }

    pub fn from_code(code: &str) -> Option<Order> {
        Order::ALL.into_iter().find(|o| o.code() == code.trim())
    }
}

/// One wire of the checklist.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    /// Index into the routed wires.
    pub wire: usize,
    pub status: Status,
    pub note: String,
    /// Stripping length and ferrule or terminal of its `from` and `to` end.
    pub ends: [Option<EndTreatment>; 2],
    /// The other wire sharing the twin ferrule of its `from` and `to` end.
    pub twins: [Option<Twin>; 2],
}

pub fn mark_of<'a>(prj: &'a Project, w: &RoutedWire) -> Option<&'a WiringMark> {
    prj.wiring_marks.iter().find(|m| m.matches(w))
}

pub fn status(prj: &Project, w: &RoutedWire) -> Status {
    match mark_of(prj, w) {
        None => Status::Open,
        Some(m) if m.mark == Mark::Problem => Status::Problem,
        Some(m) if m.color != w.color || m.cross_section != w.cross_section || m.number != w.number => Status::Changed,
        Some(_) => Status::Done,
    }
}

/// Wires of an enclosure (all with `None`) as wiring steps in the given order; two
/// wires sharing a twin ferrule one after the other, as they are crimped together.
pub fn steps(prj: &Project, ends: &Ends, wiring: &[RoutedWire], enclosure: Option<usize>, order: Order) -> Vec<Step> {
    let mut v: Vec<Step> = wiring
        .iter()
        .enumerate()
        .filter(|(_, w)| enclosure.is_none_or(|e| w.in_enclosure(e)))
        .map(|(i, w)| Step {
            wire: i,
            status: status(prj, w),
            note: mark_of(prj, w).map(|m| m.note.clone()).unwrap_or_default(),
            ends: [ends.end(prj, &w.from, &w.to, &w.cross_section), ends.end(prj, &w.to, &w.from, &w.cross_section)],
            twins: [ends.twin(prj, &w.from, &w.to), ends.twin(prj, &w.to, &w.from)],
        })
        .collect();
    let start = |w: &RoutedWire| w.route.points.first().copied().unwrap_or_default();
    v.sort_by(|a, b| {
        let (wa, wb) = (&wiring[a.wire], &wiring[b.wire]);
        let by_ends = || natural_cmp(&wa.from, &wb.from).then_with(|| natural_cmp(&wa.to, &wb.to));
        let first = match order {
            Order::Device => std::cmp::Ordering::Equal,
            Order::Position => {
                let (pa, pb) = (start(wa), start(wb));
                // Rows of 50 mm, left to right within a row.
                ((pa.y / 50.0).floor()).total_cmp(&(pb.y / 50.0).floor()).then(pa.x.total_cmp(&pb.x))
            }
            Order::CrossSection => {
                let cs = |w: &RoutedWire| cross_section_mm2(&w.cross_section).unwrap_or(0.0);
                cs(wb).total_cmp(&cs(wa))
            }
            Order::Number => natural_cmp(&wa.number, &wb.number),
        };
        wa.enclosure.cmp(&wb.enclosure).then(first).then_with(by_ends)
    });
    together(v, wiring)
}

/// The steps with the wires sharing a twin ferrule right after the first of them.
fn together(steps: Vec<Step>, wiring: &[RoutedWire]) -> Vec<Step> {
    let partners = |k: usize| -> Vec<usize> {
        let w = &wiring[steps[k].wire];
        [&w.from, &w.to]
            .into_iter()
            .zip(&steps[k].twins)
            .filter_map(|(at, t)| {
                let t = t.as_ref()?;
                steps.iter().position(|s| {
                    let o = &wiring[s.wire];
                    (&o.from == at && o.to == t.with) || (&o.to == at && o.from == t.with)
                })
            })
            .collect()
    };
    let mut order = Vec::with_capacity(steps.len());
    let mut done = vec![false; steps.len()];
    for i in 0..steps.len() {
        let mut next = vec![i];
        while let Some(k) = next.pop() {
            if std::mem::replace(&mut done[k], true) {
                continue;
            }
            order.push(k);
            next.extend(partners(k).into_iter().rev());
        }
    }
    let mut steps: Vec<Option<Step>> = steps.into_iter().map(Some).collect();
    order.into_iter().filter_map(|k| steps[k].take()).collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Progress {
    pub total: usize,
    /// Wired (including changed ones).
    pub done: usize,
    pub changed: usize,
    pub problems: usize,
    /// Wire length in mm.
    pub length_total: f32,
    pub length_done: f32,
}

impl Progress {
    /// Share of the wires that are wired and unchanged, 0 … 1.
    pub fn fraction(&self) -> f32 {
        if self.total == 0 { 0.0 } else { (self.done - self.changed) as f32 / self.total as f32 }
    }
}

pub fn progress(prj: &Project, wiring: &[RoutedWire], enclosure: Option<usize>) -> Progress {
    let mut p = Progress::default();
    for w in wiring.iter().filter(|w| enclosure.is_none_or(|e| w.in_enclosure(e))) {
        p.total += 1;
        p.length_total += w.length;
        match status(prj, w) {
            Status::Open => {}
            Status::Problem => p.problems += 1,
            s => {
                p.done += 1;
                p.length_done += w.length;
                if s == Status::Changed {
                    p.changed += 1;
                }
            }
        }
    }
    p
}

/// Marks a wire as wired or as a problem, or removes its mark (`None`).
pub fn set_mark(prj: &mut Project, w: &RoutedWire, mark: Option<Mark>, note: &str) {
    prj.wiring_marks.retain(|m| !m.matches(w));
    if let Some(mark) = mark {
        prj.wiring_marks.push(WiringMark {
            from: w.from.clone(),
            to: w.to.clone(),
            mark,
            note: note.trim().to_string(),
            color: w.color.clone(),
            cross_section: w.cross_section.clone(),
            number: w.number.clone(),
        });
    }
}

/// Marks of wires that are no longer in the wiring (schematic or layout changed).
pub fn stale<'a>(prj: &'a Project, wiring: &[RoutedWire]) -> Vec<&'a WiringMark> {
    prj.wiring_marks.iter().filter(|m| !wiring.iter().any(|w| m.matches(w))).collect()
}

/// Removes the marks of wires that no longer exist. Returns how many.
pub fn remove_stale(prj: &mut Project, wiring: &[RoutedWire]) -> usize {
    let before = prj.wiring_marks.len();
    prj.wiring_marks.retain(|m| wiring.iter().any(|w| m.matches(w)));
    before - prj.wiring_marks.len()
}

/// The next step after `current` that still needs work (open, changed or problem),
/// wrapping around; None when everything is wired.
pub fn next_open(steps: &[Step], current: Option<usize>) -> Option<usize> {
    let start = current.map(|c| c + 1).unwrap_or(0);
    (0..steps.len()).map(|k| (start + k) % steps.len().max(1)).find(|&i| steps[i].status != Status::Done)
}

/// Checklist with the wiring state as CSV (machine readable, English headers).
pub fn csv(prj: &Project, ends: &Ends, wiring: &[RoutedWire], order: Order) -> String {
    let mut out = String::from("Enclosure;Wire no.;From;To;Colour;Cross-section;Length (mm);Strip (mm);Termination from;Termination to;Status;Note\n");
    for s in steps(prj, ends, wiring, None, order) {
        let w = &wiring[s.wire];
        let [a, b] = s.ends.map(|e| e.map(|e| format!("{:.0}", e.strip)).unwrap_or_default());
        let strip = if a == b { a } else { format!("{a}/{b}") };
        let [from_end, to_end] = [0, 1].map(|k| {
            let part = s.ends[k].map(|e| if e.part().is_empty() { e.kind.key().to_string() } else { e.part() }).unwrap_or_default();
            match &s.twins[k] {
                Some(t) => format!("{part} with {}", t.with),
                None => part,
            }
        });
        let status = match s.status {
            Status::Open => "open",
            Status::Done => "wired",
            Status::Changed => "changed",
            Status::Problem => "problem",
        };
        let _ = writeln!(
            out,
            "{};{};{};{};{};{};{:.0};{strip};{from_end};{to_end};{status};{}",
            w.enclosure_names(&prj.layout),
            w.number,
            w.from,
            w.to,
            w.color,
            w.cross_section,
            w.length,
            s.note.replace([';', '\n'], " ")
        );
    }
    out
}

#[cfg(test)]
mod tests;
