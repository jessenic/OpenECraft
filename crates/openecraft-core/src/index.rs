//! Device / cross-reference index of a project.

use crate::geom::P;
use crate::model::{natural_cmp, InterruptionMode, PageKind, Project, XrefStructure};
use crate::structure;
use crate::symbols::Func;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Copy, Debug)]
pub struct Loc {
    pub page: usize,
    pub sym_id: u64,
    pub pos: P,
}

impl Loc {
    /// Complete cross reference, e.g. `/3.4` or `=A1+CAB1/3.4`.
    pub fn reference(&self, prj: &Project) -> String {
        let page = &prj.pages[self.page];
        format!("{}{}", structure::page_structure(page), crate::frame::reference(prj, self.page, self.pos))
    }

    /// Cross reference as shown on page `from`: the structure is left out if both
    /// pages have the same.
    pub fn reference_from(&self, prj: &Project, from: usize) -> String {
        let (page, here) = (&prj.pages[self.page], &prj.pages[from]);
        let full = match prj.xref.structure {
            XrefStructure::Always => true,
            XrefStructure::Never => false,
            XrefStructure::Different => structure::page_structure(page) != structure::page_structure(here),
        };
        if full {
            self.reference(prj)
        } else {
            crate::frame::reference(prj, self.page, self.pos)
        }
    }

    /// Schematic order: page, column, top to bottom, left to right.
    fn order(&self, prj: &Project) -> (usize, usize, i64, i64) {
        let col = crate::frame::of_page(prj, self.page).column_index(self.pos);
        (self.page, col, (self.pos.y * 10.0) as i64, (self.pos.x * 10.0) as i64)
    }
}

#[derive(Clone, Debug, Default)]
pub struct DeviceEntry {
    pub mains: Vec<Loc>,
    pub auxs: Vec<Loc>,
    /// Terminals, PLC points (no main / aux relation).
    pub others: Vec<Loc>,
    /// Single-line / overview representations.
    pub overviews: Vec<Loc>,
}

#[derive(Default)]
pub struct Index {
    pub devices: BTreeMap<String, DeviceEntry>,
    pub interruptions: HashMap<String, Vec<Loc>>,
}

impl Index {
    pub fn build(prj: &Project) -> Index {
        let mut idx = Index::default();
        for (pi, page) in prj.pages.iter().enumerate() {
            // Overview pages are not evaluated, only their single-line representations
            // belong to the devices.
            if !matches!(page.kind, PageKind::Schematic | PageKind::Overview) {
                continue;
            }
            for s in &page.symbols {
                let Some(def) = prj.symbol(&s.sym) else { continue };
                let loc = Loc { page: pi, sym_id: s.id, pos: s.pos };
                if page.kind == PageKind::Overview && def.func != Func::Overview {
                    continue;
                }
                match def.func {
                    Func::Interruption => idx.interruptions.entry(s.label.clone()).or_default().push(loc),
                    Func::Potential => {}
                    _ if s.tag.is_empty() => {}
                    Func::Main { .. } => idx.devices.entry(s.tag.clone()).or_default().mains.push(loc),
                    Func::Aux(_) => idx.devices.entry(s.tag.clone()).or_default().auxs.push(loc),
                    Func::Terminal | Func::Plc | Func::Plug(_) | Func::Connection => idx.devices.entry(s.tag.clone()).or_default().others.push(loc),
                    Func::Overview => idx.devices.entry(s.tag.clone()).or_default().overviews.push(loc),
                }
            }
        }
        // Sort aux functions by page and position for a stable contact image.
        for dev in idx.devices.values_mut() {
            dev.auxs.sort_by(|a, b| {
                a.page.cmp(&b.page).then(a.pos.x.total_cmp(&b.pos.x)).then(a.pos.y.total_cmp(&b.pos.y))
            });
        }
        idx
    }

    /// The counterparts an interruption point refers to (see [`InterruptionMode`]), in
    /// schematic order.
    pub fn interruption_targets(&self, prj: &Project, name: &str, me: u64) -> Vec<Loc> {
        let Some(locs) = self.interruptions.get(name) else { return Vec::new() };
        let mut all: Vec<Loc> = locs.clone();
        all.sort_by_key(|l| l.order(prj));
        let Some(at) = all.iter().position(|l| l.sym_id == me) else { return Vec::new() };
        match prj.xref.interruptions {
            InterruptionMode::All => all.into_iter().filter(|l| l.sym_id != me).collect(),
            InterruptionMode::Star if at == 0 => all.into_iter().skip(1).collect(),
            InterruptionMode::Star => vec![all[0]],
            InterruptionMode::Pairs => {
                let partner = if at % 2 == 0 { at + 1 } else { at - 1 };
                all.get(partner).copied().into_iter().collect()
            }
        }
    }

    /// Device tags in natural order.
    pub fn sorted_tags(&self) -> Vec<&String> {
        let mut v: Vec<&String> = self.devices.keys().collect();
        v.sort_by(|a, b| natural_cmp(a, b));
        v
    }
}

#[cfg(test)]
mod tests;
