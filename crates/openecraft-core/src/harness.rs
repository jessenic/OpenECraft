//! Wire harnesses: connections (point-to-point, identified by their two connection
//! point designations) assigned to a harness are made as one assembly. The harness data lists its wires with lengths and end treatment, its
//! connectors (device ends with their pin assignment) and the bundle diameter; the 3D
//! topology of the bundle is in [`topology`]. Harnesses are exported as KBL
//! (VDA 4964, [`crate::kbl`]) and through the harness interface
//! ([`crate::harness_export`]).

use crate::model::{natural_cmp, Project};
use crate::netlist::{pin_designation, PinRef};
use crate::analysis::Analysis;
use std::collections::{BTreeSet, HashSet};
pub use crate::model::harness::{End, ExtraPart, Fixing, FixingKind, Harness, PartKind, Protection, ProtectionKind};

pub mod topology;

impl ProtectionKind {
    pub const ALL: [ProtectionKind; 5] = [ProtectionKind::Tape, ProtectionKind::Tube, ProtectionKind::CorrugatedTube, ProtectionKind::Sleeve, ProtectionKind::HeatShrink];

    pub fn key(self) -> &'static str {
        match self {
            ProtectionKind::Tape => "tape",
            ProtectionKind::Tube => "tube",
            ProtectionKind::CorrugatedTube => "corrugated_tube",
            ProtectionKind::Sleeve => "sleeve",
            ProtectionKind::HeatShrink => "heat_shrink",
        }
    }

    pub fn from_key(key: &str) -> Option<ProtectionKind> {
        ProtectionKind::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    pub fn title(self) -> &'static str {
        openecraft_i18n::tr(match self {
            ProtectionKind::Tape => "Tape",
            ProtectionKind::Tube => "Tube",
            ProtectionKind::CorrugatedTube => "Corrugated tube",
            ProtectionKind::Sleeve => "Braided sleeve",
            ProtectionKind::HeatShrink => "Heat shrink tube",
        })
    }
}

impl FixingKind {
    pub const ALL: [FixingKind; 4] = [FixingKind::Clip, FixingKind::CableTie, FixingKind::Grommet, FixingKind::Bracket];

    pub fn key(self) -> &'static str {
        match self {
            FixingKind::Clip => "clip",
            FixingKind::CableTie => "cable_tie",
            FixingKind::Grommet => "grommet",
            FixingKind::Bracket => "bracket",
        }
    }

    pub fn from_key(key: &str) -> Option<FixingKind> {
        FixingKind::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    pub fn title(self) -> &'static str {
        openecraft_i18n::tr(match self {
            FixingKind::Clip => "Clip",
            FixingKind::CableTie => "Cable tie",
            FixingKind::Grommet => "Grommet",
            FixingKind::Bracket => "Bracket",
        })
    }
}

impl PartKind {
    pub const ALL: [PartKind; 4] = [PartKind::Accessory, PartKind::CoPack, PartKind::Assembly, PartKind::Component];

    pub fn key(self) -> &'static str {
        match self {
            PartKind::Accessory => "accessory",
            PartKind::CoPack => "co_pack",
            PartKind::Assembly => "assembly",
            PartKind::Component => "component",
        }
    }

    pub fn from_key(key: &str) -> Option<PartKind> {
        PartKind::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }
}

/// KBL connector usages ([`End::usage`]).
pub const USAGES: [&str; 4] = ["no end", "ring terminal", "splice", "dangler"];

/// End treatment of a wire end at `device:pin`: the entry of the pin, otherwise that
/// of the device.
pub fn end_of<'a>(h: &'a Harness, device: &str, pin: &str) -> Option<&'a End> {
    h.ends.iter().find(|e| e.device == device && !e.pin.is_empty() && e.pin == pin).or_else(|| h.ends.iter().find(|e| e.device == device && e.pin.is_empty()))
}

/// Terminal part, seal part and stripping length of a wire end: from the end
/// treatment of the harness, otherwise the termination chosen for the connection point
/// or device ([`crate::production::termination_of`]), ferrules (panel layout setting)
/// at device ends.
pub fn end_parts(prj: &Project, h: &Harness, device: &str, pin: &str, cross_section: &str) -> (String, String, f32) {
    let e = end_of(h, device, pin);
    let at = if pin.is_empty() { device.to_string() } else { format!("{device}:{pin}") };
    let default = match crate::production::termination_of(prj, &at, "") {
        Some(kind) => crate::production::treatment(cross_section, kind),
        None => crate::production::end_treatment(cross_section, prj.layout.ferrules && !crate::plugs::plug_tags(prj).iter().any(|p| p == device)),
    };
    let terminal = e.map(|e| e.terminal.clone()).filter(|t| !t.is_empty()).or_else(|| default.map(|d| d.part()).filter(|p| !p.is_empty())).unwrap_or_default();
    let strip = e.map(|e| e.strip).filter(|s| *s > 0.0).or_else(|| default.map(|d| d.strip)).unwrap_or(0.0);
    (terminal, e.map(|e| e.seal.clone()).unwrap_or_default(), strip)
}

/// Order independent key of a connection.
pub fn key(a: &str, b: &str) -> [String; 2] {
    if natural_cmp(a, b) == std::cmp::Ordering::Greater { [b.into(), a.into()] } else { [a.into(), b.into()] }
}

/// Harness of the connection between two connection points (empty: none).
pub fn of(prj: &Project, a: &str, b: &str) -> String {
    let k = key(a, b);
    prj.harnesses.iter().find(|h| h.connections.contains(&k)).map(|h| h.tag.clone()).unwrap_or_default()
}

/// Assigns a connection to a harness (empty: to none).
pub fn assign(prj: &mut Project, a: &str, b: &str, tag: &str) {
    let k = key(a, b);
    for h in &mut prj.harnesses {
        h.connections.retain(|c| *c != k);
    }
    if let Some(h) = prj.harnesses.iter_mut().find(|h| h.tag == tag) {
        h.connections.push(k);
    }
}

/// Keeps the assignments when a device is renamed.
pub fn rename_device(prj: &mut Project, old: &str, new: &str) {
    for h in &mut prj.harnesses {
        for c in &mut h.connections {
            for end in c.iter_mut() {
                if end == old {
                    *end = new.to_string();
                } else if let Some(pin) = end.strip_prefix(old).and_then(|r| r.strip_prefix(':')) {
                    *end = format!("{new}:{pin}");
                }
            }
            *c = key(&c[0], &c[1]);
        }
    }
}

/// The connections assigned to any harness.
pub fn assigned(prj: &Project) -> HashSet<[String; 2]> {
    prj.harnesses.iter().flat_map(|h| h.connections.iter().cloned()).collect()
}

/// Orders the connection points of a net (`pins`, in schematic order) so that the
/// connections assigned to a harness stay connections: they are wires of the harness,
/// so moving pages or symbols does not chain them anew. A run of assigned connections
/// keeps together where its first point is; the other points keep their order.
pub fn keep_assigned(prj: &Project, assigned: &HashSet<[String; 2]>, pins: Vec<PinRef>) -> Vec<PinRef> {
    let n = pins.len();
    if assigned.is_empty() || n < 3 {
        return pins;
    }
    let names: Vec<String> = pins.iter().map(|&p| pin_designation(prj, p)).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    // Links between assigned points, nearest in schematic order first; each point has
    // at most two and they form no ring, so the links make paths.
    let mut links: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut group: Vec<usize> = (0..n).collect();
    for d in 1..n {
        for i in 0..n - d {
            let j = i + d;
            if links[i].len() < 2 && links[j].len() < 2 && assigned.contains(&key(&names[i], &names[j])) {
                let (ri, rj) = (root(&mut group, i), root(&mut group, j));
                if ri != rj {
                    group[ri] = rj;
                    links[i].push(j);
                    links[j].push(i);
                }
            }
        }
    }
    let mut done = vec![false; n];
    let mut out = Vec::with_capacity(n);
    for s in 0..n {
        if done[s] {
            continue;
        }
        let mut path = vec![s];
        let mut k = 0;
        while k < path.len() {
            for &m in &links[path[k]] {
                if !path.contains(&m) {
                    path.push(m);
                }
            }
            k += 1;
        }
        // From the end that comes first in schematic order.
        let mut cur = path.iter().copied().filter(|&m| links[m].len() < 2).min().unwrap_or(s);
        loop {
            done[cur] = true;
            out.push(pins[cur]);
            match links[cur].iter().copied().find(|&m| !done[m]) {
                Some(m) => cur = m,
                None => break,
            }
        }
    }
    out
}

/// Assigned connections that do not exist in the schematic (any more): (harness, ends).
pub fn stale(prj: &Project, an: &Analysis) -> Vec<(String, [String; 2])> {
    let existing: Vec<[String; 2]> = an.connections.iter().map(|c| key(&pin_designation(prj, c.a), &pin_designation(prj, c.b))).collect();
    prj.harnesses
        .iter()
        .flat_map(|h| h.connections.iter().filter(|c| !existing.contains(c)).map(|c| (h.tag.clone(), c.clone())))
        .collect()
}

/// Connections (indices into `an.connections`) whose two ends both lie on end
/// points of the given wires of a page: selecting the wires between two devices
/// selects their connection.
pub fn connections_at(prj: &Project, an: &Analysis, page: usize, wires: &BTreeSet<u64>) -> Vec<usize> {
    let ends: Vec<crate::geom::P> = prj.pages[page].wires.iter().filter(|w| wires.contains(&w.id)).flat_map(|w| [w.a, w.b]).collect();
    let pos = |pin: PinRef| {
        let s = &prj.pages[pin.page].symbols[pin.sym];
        prj.symbol(&s.sym).map(|d| d.pin_world(pin.pin, s.pos, s.rot, s.mirror))
    };
    let touched = |pin: PinRef| pin.page == page && pos(pin).map(|q| ends.iter().any(|e| e.same(q))).unwrap_or(false);
    (0..an.connections.len()).filter(|&i| touched(an.connections[i].a) && touched(an.connections[i].b)).collect()
}

/// Connections of the net a wire belongs to (indices into `an.connections`).
pub fn connections_of_wire(an: &Analysis, wire: u64) -> Vec<usize> {
    let Some(net) = an.open.wire_net.get(&wire).map(|&n| &an.open.nets[n]) else { return Vec::new() };
    (0..an.connections.len()).filter(|&i| net.pins.contains(&an.connections[i].a) && net.pins.contains(&an.connections[i].b)).collect()
}

/// One wire of a harness.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HarnessWire {
    pub number: String,
    /// Device tag and connection point of both ends.
    pub from: (String, String),
    pub to: (String, String),
    pub color: String,
    pub cross_section: String,
    /// Length in mm (routed in the panel layout, along the stored topology, or the
    /// harness length); 0 = unknown.
    pub length: f32,
    pub routed: bool,
    pub target: Option<(usize, u64)>,
    /// Potential of the wire.
    pub net: String,
    /// Cable tag and core, if the wire is a cable core.
    pub cable: String,
    pub core: String,
}

/// A device end of a harness with its pin assignment: (pin, wire number, other end).
#[derive(Clone, Debug, PartialEq)]
pub struct Connector {
    pub device: String,
    pub pins: Vec<(String, String, String)>,
}

fn end(prj: &Project, pin: PinRef) -> (String, String) {
    let tag = prj.pages[pin.page].symbols[pin.sym].tag.clone();
    let d = pin_designation(prj, pin);
    let pin_name = d.strip_prefix(tag.as_str()).unwrap_or(&d).trim_start_matches(':').to_string();
    (tag, pin_name)
}

/// `device:pin` of a wire end (the device alone without a pin).
pub fn joined(e: &(String, String)) -> String {
    if e.1.is_empty() { e.0.clone() } else { format!("{}:{}", e.0, e.1) }
}

/// Wires of a harness, in wire number order.
pub fn wires(prj: &Project, an: &Analysis, tag: &str) -> Vec<HarnessWire> {
    let default = prj.harnesses.iter().find(|h| h.tag == tag).map(|h| h.length * 1000.0).unwrap_or(0.0);
    let mut v: Vec<HarnessWire> = an
        .connections
        .iter()
        .filter(|c| c.harness == tag)
        .map(|c| {
            let (from, to) = (end(prj, c.a), end(prj, c.b));
            let (fa, fb) = (pin_designation(prj, c.a), pin_designation(prj, c.b));
            let routed = an.wiring.iter().find(|w| (w.from == fa && w.to == fb) || (w.from == fb && w.to == fa)).map(|w| w.length);
            HarnessWire {
                number: c.number.clone(),
                from,
                to,
                color: c.color.clone(),
                cross_section: c.cross_section.clone(),
                length: routed.unwrap_or(default),
                routed: routed.is_some(),
                target: Some((c.a.page, prj.pages[c.a.page].symbols[c.a.sym].id)),
                net: c.net.clone(),
                cable: c.cable.clone(),
                core: c.core.clone(),
            }
        })
        .collect();
    v.sort_by(|a, b| natural_cmp(&a.number, &b.number).then(natural_cmp(&joined(&a.from), &joined(&b.from))));
    // Wires outside the panel layout: along the stored topology, if it routes them.
    if let Some(h) = prj.harnesses.iter().find(|h| h.tag == tag && !h.nodes.is_empty()) {
        let t = topology::build(prj, an, h, &v);
        for (i, w) in v.iter_mut().enumerate().filter(|(_, w)| !w.routed) {
            if let Some(l) = t.wire_length(i).filter(|l| *l > 0.0) {
                w.length = l;
            }
        }
    }
    v
}

/// Device ends of the wires with their pin assignment.
pub fn connectors(wires: &[HarnessWire]) -> Vec<Connector> {
    let mut v: Vec<Connector> = Vec::new();
    let mut add = |dev: &str, pin: &str, number: &str, other: &(String, String)| {
        let entry = (pin.to_string(), number.to_string(), joined(other));
        match v.iter_mut().find(|c| c.device == dev) {
            Some(c) => c.pins.push(entry),
            None => v.push(Connector { device: dev.to_string(), pins: vec![entry] }),
        }
    };
    for w in wires {
        add(&w.from.0, &w.from.1, &w.number, &w.to);
        add(&w.to.0, &w.to.1, &w.number, &w.from);
    }
    for c in &mut v {
        c.pins.sort_by(|a, b| natural_cmp(&a.0, &b.0));
    }
    v.sort_by(|a, b| natural_cmp(&a.device, &b.device));
    v
}

/// Outer diameter of a single core wire (PVC insulated, H05V-K / H07V-K like).
pub fn outer_diameter(cross_section: &str) -> f32 {
    let cs = crate::production::cross_section_mm2(cross_section).unwrap_or(1.0);
    1.0 + 1.6 * cs.sqrt()
}

/// Diameter of the bundle (wires packed round, 15 % gaps).
pub fn bundle_diameter(wires: &[HarnessWire]) -> f32 {
    let sum: f32 = wires.iter().map(|w| outer_diameter(&w.cross_section).powi(2)).sum();
    sum.sqrt() * 1.15
}

#[cfg(test)]
mod tests;
