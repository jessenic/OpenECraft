//! Busbar systems and the devices mounted on them. Every bar of a system is one
//! conductor: the devices on a system (terminals for the supply cables, fuse switch
//! disconnectors and circuit breakers with their feed side on the bars) may join no
//! more conductors than it has bars, otherwise the bars short-circuit them.

use super::{ItemKind, PanelItem};
use crate::issue::{Issue, Severity};
use crate::model::*;
use crate::netlist::Netlist;
use crate::parts::vec3::Tri;
use crate::parts::FnKind;
use crate::symbols::Func;
use std::collections::{BTreeMap, BTreeSet};

/// The conductors a busbar system joins: the nets on its bars (bridged netlist).
#[derive(Clone, Debug, PartialEq)]
pub struct BusbarUse {
    pub enclosure: usize,
    pub busbar: u64,
    pub bars: usize,
    pub nets: BTreeSet<usize>,
    /// The devices on the system.
    pub devices: Vec<String>,
}

/// Devices mounted on the busbar item `bar`: on its face, overlapping it, with a part
/// mounted on busbars.
fn mounted_on<'a>(prj: &Project, items: &'a [PanelItem], bar: &PanelItem) -> Vec<&'a PanelItem> {
    let on_busbar = |it: &PanelItem| super::part_by_number(prj, &it.part).is_some_and(|p| p.mounting.to_lowercase().contains("busbar"));
    items
        .iter()
        .filter(|it| it.kind == ItemKind::Device && it.face == bar.face && !it.tag.is_empty() && on_busbar(it))
        .filter(|it| it.x < bar.x + bar.w && bar.x < it.x + it.w && it.y < bar.y + bar.h && bar.y < it.y + it.h)
        .collect()
}

/// Nets that are one conductor: the primary of a current transformer passes it
/// through (a transformer clipped in behind a strip, a set in the busbar). Each net to
/// the first net of its conductor.
fn conductors(prj: &Project, nl: &Netlist) -> BTreeMap<usize, usize> {
    let mut parent: BTreeMap<usize, usize> = BTreeMap::new();
    fn root(parent: &BTreeMap<usize, usize>, mut n: usize) -> usize {
        while let Some(&p) = parent.get(&n).filter(|&&p| p != n) {
            n = p;
        }
        n
    }
    let symbols = prj.pages.iter().filter(|p| p.kind == PageKind::Schematic).flat_map(|p| &p.symbols);
    for s in symbols.filter(|s| crate::parts::kind_of_symbol(&s.sym) == Some(FnKind::CurrentTransformer)) {
        if let (Some(&a), Some(&b)) = (nl.pin_net.get(&(s.id, 0)), nl.pin_net.get(&(s.id, 1))) {
            let (ra, rb) = (root(&parent, a), root(&parent, b));
            if ra != rb {
                parent.insert(ra.max(rb), ra.min(rb));
            }
        }
    }
    let keys: Vec<usize> = parent.keys().copied().collect();
    keys.into_iter().map(|n| (n, root(&parent, n))).collect()
}

/// The two sides a device can have on the bars: the conductors of its feed side (odd
/// designations, N) and of its outgoing side (even designations, N'). Terminals have
/// one side.
fn sides(prj: &Project, nl: &Netlist, alias: &BTreeMap<usize, usize>, tag: &str) -> Vec<BTreeSet<usize>> {
    let symbols: Vec<&SymbolInst> = prj.pages.iter().filter(|p| p.kind == PageKind::Schematic).flat_map(|p| &p.symbols).filter(|s| s.tag == tag).collect();
    let kind = |s: &SymbolInst| crate::parts::kind_of_symbol(&s.sym);
    let canon = |n: usize| *alias.get(&n).unwrap_or(&n);
    let (mut feed, mut out, mut terminals) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for s in &symbols {
        let Some(def) = prj.symbol(&s.sym) else { continue };
        if def.func == Func::Terminal {
            terminals.extend((0..def.pins.len()).filter_map(|i| nl.pin_net.get(&(s.id, i)).map(|&n| canon(n))));
            continue;
        }
        let power = matches!(
            kind(s),
            Some(
                FnKind::Fuse
                    | FnKind::Fuse3
                    | FnKind::Breaker1
                    | FnKind::Breaker2
                    | FnKind::Breaker3
                    | FnKind::Breaker4
                    | FnKind::Disconnector3
                    | FnKind::MotorProtection
                    | FnKind::ResidualCurrent
                    | FnKind::MainContacts3
                    | FnKind::MainContacts4
            )
        );
        if !power {
            continue;
        }
        for i in 0..def.pins.len() {
            let Some(&n) = nl.pin_net.get(&(s.id, i)) else { continue };
            let label = s.pins.get(i).map(|p| p.trim()).unwrap_or_default();
            let outgoing = match label.parse::<u32>().ok() {
                Some(d) => d % 2 == 0,
                None => label.ends_with('\''),
            };
            if outgoing { &mut out } else { &mut feed }.insert(canon(n));
        }
    }
    if !terminals.is_empty() {
        return vec![terminals];
    }
    [feed, out].into_iter().filter(|s| !s.is_empty()).collect()
}

/// The busbar systems of the panel layout and the conductors on their bars: of each
/// device the side that shares its conductors with the other devices on the system
/// (the feed side when none does: a feeder strip of an incoming cable is turned).
pub fn uses(prj: &Project, nl: &Netlist) -> Vec<BusbarUse> {
    let mut out = Vec::new();
    let alias = conductors(prj, nl);
    for (ei, e) in prj.layout.enclosures.iter().enumerate() {
        for bar in e.items.iter().filter(|i| i.kind == ItemKind::Busbar) {
            let devices: Vec<&PanelItem> = mounted_on(prj, &e.items, bar);
            let mut tags: Vec<String> = devices.iter().map(|d| d.tag.clone()).collect();
            tags.dedup();
            let all: Vec<Vec<BTreeSet<usize>>> = tags.iter().map(|t| sides(prj, nl, &alias, t)).collect();
            let mut nets = BTreeSet::new();
            for (k, own) in all.iter().enumerate() {
                let others: BTreeSet<usize> = all.iter().enumerate().filter(|(j, _)| *j != k).flat_map(|(_, s)| s.iter().flatten().copied()).collect();
                let pick = own.iter().max_by_key(|s| (s.intersection(&others).count(), std::cmp::Reverse(own.iter().position(|o| o == *s))));
                if let Some(side) = pick {
                    nets.extend(side.iter().copied());
                }
            }
            out.push(BusbarUse { enclosure: ei, busbar: bar.id, bars: crate::production::bar_count(&prj.parts, &bar.part), nets, devices: tags });
        }
    }
    out
}

/// Colour of the copper bars and of their insulating holders.
pub const COPPER: [u8; 3] = [200, 120, 60];
pub const HOLDER: [u8; 3] = [70, 72, 78];

/// The bars of a busbar system for the 3D view: each bar on edge at the front of the
/// system, at their centres like in the 2D view (part data `Bars`, `Centre distance`,
/// `Bar: Cu 30 x 10 mm`), on insulating holders behind them that reach just across the
/// outer bars ([`holder_places`]).
pub fn mesh(prj: &Project, it: &PanelItem) -> Vec<(Tri, Option<[u8; 3]>)> {
    let section = super::part_by_number(prj, &it.part).and_then(|p| p.tech_value("Bar")).map(numbers).unwrap_or_default();
    let (big, small) = match section.as_slice() {
        [a, b, ..] => (a.max(*b), a.min(*b)),
        _ => (30.0, 10.0),
    };
    let horizontal = super::bars_along_x(&prj.parts, it);
    let (along, across) = if horizontal { (it.w, it.h) } else { (it.h, it.w) };
    let centres = crate::production::bar_centres(&prj.parts, &it.part, across);
    let pitch = centres.windows(2).map(|w| w[1] - w[0]).fold(across, f32::min);
    let thick = small.min(pitch * 0.5);
    let depth = big.min(it.d * 0.6);
    // Along / across the system and depth to plate coordinates.
    let at = |a: f32, c: f32, z: f32| if horizontal { [it.x + a, it.y + c, it.z + z] } else { [it.x + c, it.y + a, it.z + z] };
    let block = |a0: f32, a1: f32, c0: f32, c1: f32, z0: f32, z1: f32| {
        let (p, q) = (at(a0, c0, z0), at(a1, c1, z1));
        super::plate_box([p[0].min(q[0]), p[1].min(q[1]), p[2]], [p[0].max(q[0]), p[1].max(q[1]), q[2]])
    };
    let mut out = Vec::new();
    for c in &centres {
        out.extend(block(0.0, along, c - thick / 2.0, c + thick / 2.0, it.d - depth, it.d).into_iter().map(|t| (t, Some(COPPER))));
    }
    let (first, last) = (centres.first().copied().unwrap_or(0.0), centres.last().copied().unwrap_or(across));
    let (c0, c1) = ((first - 2.0 * thick).max(0.0), (last + 2.0 * thick).min(across));
    for a in holder_places(prj, it) {
        out.extend(block(a - HOLDER_WIDTH / 2.0, a + HOLDER_WIDTH / 2.0, c0, c1, 0.0, it.d - depth).into_iter().map(|t| (t, Some(HOLDER))));
    }
    out
}

/// How wide a holder of a busbar system is drawn, along the bars (mm).
pub const HOLDER_WIDTH: f32 = 30.0;

/// Where the holders of a busbar system stand along it (their middles, mm from its
/// start): at the drilling positions of the plate ([`crate::production::BUSBAR_HOLES`]),
/// each moved off the devices that sit in the bars (the transformers the bars pass
/// through: devices that do not reach across the whole system, unlike the strips and
/// terminals mounted on it) into the free stretch nearest to it; none where no stretch
/// is free.
pub fn holder_places(prj: &Project, it: &PanelItem) -> Vec<f32> {
    let horizontal = super::bars_along_x(&prj.parts, it);
    let along = if horizontal { it.w } else { it.h };
    let e = prj.layout.enclosures.iter().find(|e| e.items.iter().any(|i| i.id == it.id));
    let in_bars = |d: &&PanelItem| {
        d.kind == ItemKind::Device && d.face == it.face && d.x < it.x + it.w && it.x < d.x + d.w && d.y < it.y + it.h && it.y < d.y + d.h && if horizontal { d.h < it.h - 1.0 } else { d.w < it.w - 1.0 }
    };
    let mut busy: Vec<(f32, f32)> = e.map(|e| e.items.iter().filter(in_bars).map(|d| if horizontal { (d.x - it.x, d.x + d.w - it.x) } else { (d.y - it.y, d.y + d.h - it.y) }).collect()).unwrap_or_default();
    busy.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut free: Vec<(f32, f32)> = Vec::new();
    let mut at = 0.0;
    for (b0, b1) in busy {
        if b0 - at >= HOLDER_WIDTH {
            free.push((at, b0));
        }
        at = at.max(b1);
    }
    if along - at >= HOLDER_WIDTH {
        free.push((at, along));
    }
    let half = HOLDER_WIDTH / 2.0;
    let mut places: Vec<f32> = Vec::new();
    for t in crate::production::line_positions(along, crate::production::BUSBAR_HOLES.0, crate::production::BUSBAR_HOLES.1) {
        let Some(a) = free.iter().map(|(f0, f1)| t.clamp(f0 + half, f1 - half)).min_by(|a, b| (a - t).abs().total_cmp(&(b - t).abs())) else { continue };
        if places.iter().all(|p| (p - a).abs() >= HOLDER_WIDTH) {
            places.push(a);
        }
    }
    places
}

/// A terminal strip placed across the bars of a busbar system (the busbar terminals of
/// a supply cable): each terminal upright on its own bar, the cable coming in across
/// the bars, in strip order from the first bar the strip covers. Per terminal its box
/// (as high as the part, at most as far as the bars are apart), part and designation;
/// `None` for a strip that is not across bars or has more terminals than it covers bars.
pub fn terminals_on_bars(prj: &Project, it: &PanelItem) -> Option<Vec<(PanelItem, String, String)>> {
    if it.kind != ItemKind::Device || it.single_part || !it.terminal.is_empty() {
        return None;
    }
    super::part_by_number(prj, &it.part).filter(|p| p.mounting.to_lowercase().contains("busbar") && crate::strips::is_terminal_part(p))?;
    let e = prj.layout.enclosures.iter().find(|e| e.items.iter().any(|i| i.id == it.id))?;
    let bar = e.items.iter().find(|b| {
        b.kind == ItemKind::Busbar && b.face == it.face && it.x < b.x + b.w && b.x < it.x + it.w && it.y < b.y + b.h && b.y < it.y + it.h
    })?;
    let horizontal = super::bars_along_x(&prj.parts, bar);
    let ((start, len), (b0, across), across_bars) =
        if horizontal { ((it.y, it.h), (bar.y, bar.h), it.h > it.w) } else { ((it.x, it.w), (bar.x, bar.w), it.w > it.h) };
    if !across_bars {
        return None;
    }
    let centres: Vec<f32> =
        crate::production::bar_centres(&prj.parts, &bar.part, across).into_iter().map(|c| b0 + c).filter(|c| *c >= start && *c <= start + len).collect();
    let layout = crate::strips::strip_layout(prj, &it.tag);
    let terminals: Vec<(&String, &String)> = layout
        .iter()
        .filter_map(|el| match &el.kind {
            crate::strips::ElementKind::Terminal(label) | crate::strips::ElementKind::Spare(label) => Some((&el.part, label)),
            _ => None,
        })
        .collect();
    if terminals.is_empty() || terminals.len() > centres.len() {
        return None;
    }
    let gap = centres.windows(2).map(|w| w[1] - w[0]).fold(f32::MAX, f32::min);
    Some(
        terminals
            .into_iter()
            .zip(centres)
            .map(|((part, label), c)| {
                let high = super::part_by_number(prj, part).map_or(gap, |p| p.height).min(gap * 0.9);
                let mut sub = it.clone();
                if horizontal {
                    (sub.y, sub.h) = (c - high / 2.0, high);
                } else {
                    (sub.x, sub.w) = (c - high / 2.0, high);
                }
                (sub, part.clone(), label.clone())
            })
            .collect(),
    )
}

/// Places the terminal strip `tag` across the bars of the busbar item `bar`, `at` mm
/// along it: as wide as one terminal, as long as the system is wide (a terminal on
/// each bar).
pub fn place_on_bars(prj: &mut Project, enc: usize, tag: &str, bar: u64, at: f32) -> Option<u64> {
    let b = prj.layout.enclosures.get(enc)?.items.iter().find(|i| i.id == bar)?.clone();
    let size = super::device_size(prj, tag)?;
    let wide = super::part_by_number(prj, &size.part)?.width;
    let id = super::place_device(prj, enc, tag, crate::geom::p(b.x + at, b.y))?;
    let along_x = super::bars_along_x(&prj.parts, &b);
    let it = prj.layout.enclosures[enc].items.iter_mut().find(|i| i.id == id)?;
    if along_x {
        (it.x, it.y, it.w, it.h) = (b.x + at, b.y, wide, b.h);
    } else {
        (it.x, it.y, it.w, it.h) = (b.x, b.y + at, b.w, wide);
    }
    Some(id)
}

/// The numbers in a text (`Cu 30 x 10 mm` → 30, 10).
pub(crate) fn numbers(text: &str) -> Vec<f32> {
    text.split(|c: char| !(c.is_ascii_digit() || c == '.')).filter_map(|s| s.parse().ok()).collect()
}

/// Busbar systems joining more conductors than they have bars.
pub fn problems(prj: &Project, nl: &Netlist) -> Vec<Issue> {
    let mut v = Vec::new();
    for u in uses(prj, nl).into_iter().filter(|u| u.nets.len() > u.bars) {
        let e = &prj.layout.enclosures[u.enclosure];
        let names: Vec<String> = u.nets.iter().map(|&n| nl.nets[n].name.clone()).collect();
        let target = prj.pages.iter().enumerate().find_map(|(pi, pg)| pg.symbols.iter().find(|s| u.devices.contains(&s.tag)).map(|s| (pi, s.id)));
        v.push(crate::issue!(
            Severity::Error,
            target,
            "{}: the busbar system with {} bars joins {} conductors ({}): short circuit",
            e.name,
            u.bars,
            u.nets.len(),
            names.join(", ")
        ));
    }
    v
}

#[cfg(test)]
mod tests;
