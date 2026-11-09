//! Field wiring: cables outside the enclosures, routed through the plant or along the
//! machine. A 2D plant layout (nodes with an elevation, so routes have 3D lengths)
//! holds the route network of cable trays, ladders, trunking, conduits and drag
//! chains; enclosures and field devices are placed at its nodes. Cables are planned
//! through the network along the shortest allowed path (segregation classes, fill,
//! forced via points and avoided segments of their route definition), which gives
//! their length, the fill of every segment and the topology problems. Equipotential
//! bonding ([`bonding`]) and the accessories at the cable ends ([`accessories`])
//! belong to the field side as well.

use crate::model::Project;
use crate::netlist::Connection;
use crate::issue::{Issue, Severity};
use crate::routing3d::{self, Class};
use openecraft_i18n::{tr, tr_fmt};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

pub mod accessories;
pub mod bonding;
pub mod draw;
pub use accessories::{Accessory, AccessoryKind};
pub use bonding::{Bond, BondKind};
pub use crate::model::field::{Field, FieldRules, Node, Outline, Place, RouteDef, Segment, SegmentKind};

// ---------------------------------------------------------------------- model

impl Field {
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty() && self.places.is_empty() && self.outlines.is_empty() && self.bonds.is_empty() && self.accessories.is_empty() && self.routes.is_empty()
    }

    pub fn node(&self, id: u64) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn segment(&self, id: u64) -> Option<&Segment> {
        self.segments.iter().find(|s| s.id == id)
    }

    /// Name of a node, `N<id>` without one.
    pub fn node_name(&self, id: u64) -> String {
        match self.node(id) {
            Some(n) if !n.name.is_empty() => n.name.clone(),
            _ => format!("N{id}"),
        }
    }

    /// Name of a segment, `S<id>` without one.
    pub fn segment_name(&self, id: u64) -> String {
        match self.segment(id) {
            Some(s) if !s.name.is_empty() => s.name.clone(),
            _ => format!("S{id}"),
        }
    }

    /// Length of a segment in metres: as set, otherwise the distance of its nodes.
    pub fn segment_length(&self, s: &Segment) -> f32 {
        if s.length > 0.0 {
            return s.length;
        }
        match (self.node(s.a), self.node(s.b)) {
            (Some(a), Some(b)) => ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt(),
            _ => 0.0,
        }
    }

    /// Removes a node with its segments and places.
    pub fn remove_node(&mut self, id: u64) -> bool {
        let n = self.nodes.len();
        self.nodes.retain(|x| x.id != id);
        let gone: Vec<u64> = self.segments.iter().filter(|s| s.a == id || s.b == id).map(|s| s.id).collect();
        self.segments.retain(|s| s.a != id && s.b != id);
        self.places.retain(|p| p.node != id);
        for r in &mut self.routes {
            r.via.retain(|v| *v != id);
            r.avoid.retain(|s| !gone.contains(s));
        }
        self.nodes.len() != n
    }

    pub fn remove_segment(&mut self, id: u64) -> bool {
        let n = self.segments.len();
        self.segments.retain(|s| s.id != id);
        for r in &mut self.routes {
            r.avoid.retain(|s| *s != id);
        }
        self.segments.len() != n
    }

    /// Route definition of a cable (created on demand).
    pub fn route_mut(&mut self, cable: &str) -> &mut RouteDef {
        if let Some(i) = self.routes.iter().position(|r| r.cable == cable) {
            return &mut self.routes[i];
        }
        self.routes.push(RouteDef { cable: cable.into(), ..Default::default() });
        self.routes.last_mut().expect("just pushed")
    }

    /// Keeps places, route definitions, bonds and accessories when a device or cable
    /// is renamed.
    pub fn rename(&mut self, old: &str, new: &str) {
        self.rename_device(old,new);
        self.rename_cable(old,new);
    }

    /// Device endpoints and places, independent of cable tags.
    pub fn rename_device(&mut self, old:&str, new:&str) {
        for p in self.places.iter_mut().filter(|p| p.tag == old) {
            p.tag = new.into();
        }
        for b in &mut self.bonds {
            for end in [&mut b.a, &mut b.b] {
                if end == old {
                    *end = new.into();
                }
            }
        }
        for a in &mut self.accessories {
            if a.at == old {
                a.at = new.into();
            }
        }
    }

    /// Cable routes and accessories, independent of device endpoint tags.
    pub fn rename_cable(&mut self,old:&str,new:&str) {
        for route in self.routes.iter_mut().filter(|route|route.cable==old) {route.cable=new.into();}
        for accessory in self.accessories.iter_mut().filter(|accessory|accessory.cable==old) {accessory.cable=new.into();}
    }
}

impl SegmentKind {
    pub const ALL: [SegmentKind; 7] = [SegmentKind::Tray, SegmentKind::Ladder, SegmentKind::Trunking, SegmentKind::Conduit, SegmentKind::DragChain, SegmentKind::Buried, SegmentKind::Free];

    pub fn title(self) -> &'static str {
        match self {
            SegmentKind::Tray => tr("Cable tray"),
            SegmentKind::Ladder => tr("Cable ladder"),
            SegmentKind::Trunking => tr("Trunking"),
            SegmentKind::Conduit => tr("Conduit"),
            SegmentKind::DragChain => tr("Drag chain"),
            SegmentKind::Buried => tr("Buried"),
            SegmentKind::Free => tr("Free run"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            SegmentKind::Tray => "tray",
            SegmentKind::Ladder => "ladder",
            SegmentKind::Trunking => "trunking",
            SegmentKind::Conduit => "conduit",
            SegmentKind::DragChain => "drag_chain",
            SegmentKind::Buried => "buried",
            SegmentKind::Free => "free",
        }
    }

    pub fn from_key(key: &str) -> Option<SegmentKind> {
        SegmentKind::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    /// Metal carriers are part of the equipotential bonding.
    pub fn metallic(self) -> bool {
        matches!(self, SegmentKind::Tray | SegmentKind::Ladder | SegmentKind::Trunking)
    }

    /// Cost of a metre against a cable carrier: free runs only where nothing else goes.
    fn factor(self) -> f32 {
        if self == SegmentKind::Free { 3.0 } else { 1.0 }
    }
}

impl Segment {
    pub fn accepts(&self, class: Class) -> bool {
        self.classes == 0 || self.classes & class.bit() != 0
    }

    /// Usable cross-section in mm² with the fill limit; `None` without dimensions.
    pub fn capacity(&self, fill: f32) -> Option<f32> {
        (self.width > 0.0 && self.height > 0.0).then(|| self.width * self.height * fill / 100.0 * (1.0 - self.reserve / 100.0))
    }
}

impl RouteDef {
    pub fn is_empty(&self) -> bool {
        self.via.is_empty() && self.avoid.is_empty()
    }
}

impl Default for FieldRules {
    fn default() -> Self {
        FieldRules { reserve: 0.5, cabinet: 1.5, cut: 0.5, fill: 40.0, bend_factor: 10.0, main_earth: "MET".into(), continuity: true }
    }
}

impl FieldRules {
    pub fn is_default(&self) -> bool {
        *self == FieldRules::default()
    }
}

// ---------------------------------------------------------------------- planning

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Routed,
    /// An end is not placed in the plant layout (its tag).
    NotPlaced(String),
    /// No allowed path through the network.
    NoPath,
}

/// A cable planned through the field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldCable {
    pub cable: String,
    pub class: Class,
    /// Device tags of both ends.
    pub ends: [String; 2],
    /// Places (indices) of both ends.
    pub places: [Option<usize>; 2],
    /// Nodes and segments along the route, in order.
    pub nodes: Vec<u64>,
    pub segments: Vec<u64>,
    /// Length in the network, metres.
    pub network: f32,
    /// Length with drops, enclosure lengths and reserves, metres.
    pub length: f32,
    /// Cut length (rounded up), metres.
    pub cut: f32,
    pub diameter: f32,
    pub shielded: bool,
    /// Ends that lie in an enclosure.
    pub in_enclosure: [bool; 2],
    pub status: Status,
}

impl FieldCable {
    pub fn routed(&self) -> bool {
        self.status == Status::Routed
    }

    pub fn status_text(&self) -> String {
        match &self.status {
            Status::Routed => tr("routed").to_string(),
            Status::NotPlaced(tag) => tr_fmt!("{} is not placed in the plant layout", tag),
            Status::NoPath => tr("no path through the route network").to_string(),
        }
    }
}

/// Use of a segment by the planned cables.
#[derive(Clone, Debug, PartialEq)]
pub struct SegmentUse {
    pub segment: u64,
    pub cables: Vec<String>,
    /// Cross-section of the cables, mm².
    pub area: f32,
    pub capacity: Option<f32>,
    /// Bits of the classes running in it.
    pub classes: u8,
}

impl SegmentUse {
    /// Fill in percent of the whole cross-section.
    pub fn fill(&self, s: &Segment) -> Option<f32> {
        (s.width > 0.0 && s.height > 0.0).then(|| self.area / (s.width * s.height) * 100.0)
    }
}

#[derive(Clone, Debug, Default)]
pub struct Plan {
    pub cables: Vec<FieldCable>,
    pub segments: Vec<SegmentUse>,
    /// Connected parts of the network.
    pub islands: usize,
    pub issues: Vec<Issue>,
}

impl Plan {
    pub fn cable(&self, tag: &str) -> Option<&FieldCable> {
        self.cables.iter().find(|c| c.cable == tag)
    }

    pub fn segment(&self, id: u64) -> Option<&SegmentUse> {
        self.segments.iter().find(|s| s.segment == id)
    }
}

fn same_location(a: &str, b: &str) -> bool {
    !a.is_empty() && a.trim_start_matches('+') == b.trim_start_matches('+')
}

/// Place where a device is reached, and whether that is an enclosure: the place of the
/// device itself, of the enclosure it is mounted in, or of its location.
pub fn place_of(prj: &Project, tag: &str) -> Option<(usize, bool)> {
    let places = &prj.field.places;
    if let Some(i) = places.iter().position(|p| p.tag == tag) {
        let enclosure = prj.layout.enclosures.iter().any(|e| same_location(&e.name, tag));
        return Some((i, enclosure));
    }
    if let Some((ei, _)) = crate::panel::locate(prj, tag) {
        let name = &prj.layout.enclosures[ei].name;
        if let Some(i) = places.iter().position(|p| same_location(&p.tag, name)) {
            return Some((i, true));
        }
    }
    let location = crate::structure::Dt::parse(tag).location;
    let i = places.iter().position(|p| p.tag.starts_with('+') && same_location(&p.tag, &location))?;
    Some((i, prj.layout.enclosures.iter().any(|e| same_location(&e.name, &location))))
}

/// Number of connected parts of the network (nodes without segments count alone).
fn islands(f: &Field) -> usize {
    let idx: HashMap<u64, usize> = f.nodes.iter().enumerate().map(|(i, n)| (n.id, i)).collect();
    let mut parent: Vec<usize> = (0..f.nodes.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for s in &f.segments {
        if let (Some(&a), Some(&b)) = (idx.get(&s.a), idx.get(&s.b)) {
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            parent[ra] = rb;
        }
    }
    (0..f.nodes.len()).filter(|&i| root(&mut parent, i) == i).count()
}

/// Shortest path from `a` to `b` over the allowed segments; `cost` of a segment or
/// `None` if it may not be used. Returns nodes and segments.
fn shortest(f: &Field, a: u64, b: u64, cost: &dyn Fn(&Segment) -> Option<f32>) -> Option<(Vec<u64>, Vec<u64>)> {
    if a == b {
        return Some((vec![a], Vec::new()));
    }
    let mut adj: HashMap<u64, Vec<(u64, u64, f32)>> = HashMap::new();
    for s in &f.segments {
        let Some(c) = cost(s) else { continue };
        adj.entry(s.a).or_default().push((s.b, s.id, c));
        adj.entry(s.b).or_default().push((s.a, s.id, c));
    }
    // Costs in micrometres keep the heap ordered on integers.
    let mut dist: HashMap<u64, u64> = HashMap::from([(a, 0)]);
    let mut prev: HashMap<u64, (u64, u64)> = HashMap::new();
    let mut heap = BinaryHeap::from([Reverse((0u64, a))]);
    while let Some(Reverse((d, n))) = heap.pop() {
        if n == b {
            break;
        }
        if dist.get(&n).is_some_and(|&x| x < d) {
            continue;
        }
        for &(m, sid, c) in adj.get(&n).map(Vec::as_slice).unwrap_or(&[]) {
            let nd = d + (c * 1e6) as u64;
            if dist.get(&m).is_none_or(|&x| nd < x) {
                dist.insert(m, nd);
                prev.insert(m, (n, sid));
                heap.push(Reverse((nd, m)));
            }
        }
    }
    let mut nodes = vec![b];
    let mut segs = Vec::new();
    let mut n = b;
    while n != a {
        let &(p, s) = prev.get(&n)?;
        nodes.push(p);
        segs.push(s);
        n = p;
    }
    nodes.reverse();
    segs.reverse();
    Some((nodes, segs))
}

/// Plans the field cables: every cable with at least one end placed in the plant
/// layout is routed through the network, power first and thick cables before thin
/// ones; segments that would be overfilled are avoided while there is another way.
pub fn plan(prj: &Project, conns: &[Connection]) -> Plan {
    let f = &prj.field;
    let rules = &f.rules;
    let mut out = Plan { islands: islands(f), ..Default::default() };
    if f.is_empty() {
        return out;
    }
    struct Job {
        cable: String,
        class: Class,
        ends: [String; 2],
        places: [Option<(usize, bool)>; 2],
        diameter: f32,
        shielded: bool,
    }
    let mut jobs: Vec<Job> = Vec::new();
    for c in &prj.cables {
        let cores: Vec<&Connection> = conns.iter().filter(|x| x.cable == c.tag).collect();
        let Some((a, b)) = routing3d::cable_end_tags(prj, c, &cores) else { continue };
        let places = [place_of(prj, &a), place_of(prj, &b)];
        if places == [None, None] {
            continue;
        }
        jobs.push(Job {
            cable: c.tag.clone(),
            class: routing3d::class_of_cable(prj, c, &cores),
            ends: [a, b],
            places,
            diameter: routing3d::cable_diameter(c),
            shielded: routing3d::is_shielded(c),
        });
    }
    jobs.sort_by(|a, b| a.class.cmp(&b.class).then(b.diameter.total_cmp(&a.diameter)).then(crate::model::natural_cmp(&a.cable, &b.cable)));

    let mut used: BTreeMap<u64, SegmentUse> = BTreeMap::new();
    for job in jobs {
        let area = std::f32::consts::PI * (job.diameter / 2.0).powi(2);
        let def = f.routes.iter().find(|r| r.cable == job.cable);
        let avoid: &[u64] = def.map(|d| d.avoid.as_slice()).unwrap_or(&[]);
        let cost = |s: &Segment| {
            if !s.accepts(job.class) || avoid.contains(&s.id) {
                return None;
            }
            let full = match (s.capacity(rules.fill), used.get(&s.id)) {
                (Some(cap), Some(u)) => u.area + area > cap,
                (Some(cap), None) => area > cap,
                _ => false,
            };
            Some(f.segment_length(s).max(0.001) * s.kind.factor() * if full { 20.0 } else { 1.0 })
        };
        let mut fc = FieldCable {
            cable: job.cable.clone(),
            class: job.class,
            ends: job.ends.clone(),
            places: job.places.map(|p| p.map(|x| x.0)),
            nodes: Vec::new(),
            segments: Vec::new(),
            network: 0.0,
            length: 0.0,
            cut: 0.0,
            diameter: job.diameter,
            shielded: job.shielded,
            in_enclosure: job.places.map(|p| p.is_some_and(|x| x.1)),
            status: Status::Routed,
        };
        let (Some((pa, _)), Some((pb, _))) = (job.places[0], job.places[1]) else {
            let missing = if job.places[0].is_none() { &job.ends[0] } else { &job.ends[1] };
            fc.status = Status::NotPlaced(missing.clone());
            out.cables.push(fc);
            continue;
        };
        let mut stops = vec![f.places[pa].node];
        stops.extend(def.map(|d| d.via.clone()).unwrap_or_default());
        stops.push(f.places[pb].node);
        let mut route: Option<(Vec<u64>, Vec<u64>)> = Some((vec![stops[0]], Vec::new()));
        for w in stops.windows(2) {
            route = route.and_then(|(mut nodes, mut segs)| {
                let (n, s) = shortest(f, w[0], w[1], &cost)?;
                nodes.extend(n.into_iter().skip(1));
                segs.extend(s);
                Some((nodes, segs))
            });
        }
        let Some((nodes, segs)) = route else {
            fc.status = Status::NoPath;
            out.cables.push(fc);
            continue;
        };
        fc.network = segs.iter().filter_map(|&s| f.segment(s)).map(|s| f.segment_length(s)).sum();
        let drops = f.places[pa].drop + f.places[pb].drop;
        let cabinets = fc.in_enclosure.iter().filter(|e| **e).count() as f32 * rules.cabinet;
        fc.length = fc.network + drops + cabinets + 2.0 * rules.reserve;
        fc.cut = if rules.cut > 0.0 { (fc.length / rules.cut - 1e-4).ceil() * rules.cut } else { fc.length };
        for &s in &segs {
            let u = used.entry(s).or_insert_with(|| SegmentUse {
                segment: s,
                cables: Vec::new(),
                area: 0.0,
                capacity: f.segment(s).and_then(|x| x.capacity(rules.fill)),
                classes: 0,
            });
            if !u.cables.contains(&job.cable) {
                u.cables.push(job.cable.clone());
                u.area += area;
                u.classes |= job.class.bit();
            }
        }
        fc.nodes = nodes;
        fc.segments = segs;
        out.cables.push(fc);
    }
    out.cables.sort_by(|a, b| crate::model::natural_cmp(&a.cable, &b.cable));
    out.segments = used.into_values().collect();
    out.issues = problems(prj, &out);
    out
}

/// Problems of the field wiring: broken references, cables without a route,
/// overfilled carriers, classes without divider, bend radii in drag chains, and the
/// problems of the bonding and the accessories.
fn problems(prj: &Project, plan: &Plan) -> Vec<Issue> {
    let f = &prj.field;
    let mut v = Vec::new();
    for s in &f.segments {
        for n in [s.a, s.b] {
            if f.node(n).is_none() {
                v.push(crate::issue!(Severity::Error, None, "Field segment {} refers to the missing node {}", f.segment_name(s.id), n));
            }
        }
    }
    for p in &f.places {
        if f.node(p.node).is_none() {
            v.push(crate::issue!(Severity::Error, None, "{} is placed at the missing node {}", p.tag, p.node));
        }
    }
    for r in &f.routes {
        if prj.cable(&r.cable).is_none() {
            v.push(crate::issue!(Severity::Warning, None, "Field route definition refers to the unknown cable {}", r.cable));
        }
        for n in r.via.iter().filter(|n| f.node(**n).is_none()) {
            v.push(crate::issue!(Severity::Warning, None, "Field route of {} runs via the missing node {}", r.cable, n));
        }
    }
    for c in &plan.cables {
        match &c.status {
            Status::Routed => {}
            Status::NotPlaced(tag) => v.push(crate::issue!(Severity::Warning, None, "Field cable {}: {} is not placed in the plant layout", c.cable, tag)),
            Status::NoPath => v.push(crate::issue!(Severity::Error, None, "Field cable {} cannot be routed from {} to {}", c.cable, c.ends[0], c.ends[1])),
        }
    }
    for u in &plan.segments {
        let Some(s) = f.segment(u.segment) else { continue };
        let name = f.segment_name(s.id);
        if let Some(cap) = u.capacity.filter(|cap| u.area > *cap) {
            let fill = u.fill(s).unwrap_or(0.0);
            v.push(crate::issue!(Severity::Warning, None, "Field segment {} is filled to {} % (allowed {} %)", name, format!("{fill:.0}"), format!("{:.0}", cap / (s.width * s.height) * 100.0)));
        }
        let apart = Class::ALL.iter().any(|c| u.classes & c.bit() != 0 && u.classes & c.apart_bits() != 0);
        if apart && !s.divider {
            v.push(crate::issue!(Severity::Warning, None, "Field segment {} carries power together with signal cables without a divider", name));
        }
        if s.kind == SegmentKind::DragChain && s.bend_radius > 0.0 {
            for tag in &u.cables {
                let Some(c) = plan.cable(tag) else { continue };
                let need = c.diameter * f.rules.bend_factor;
                if need > s.bend_radius {
                    v.push(crate::issue!(Severity::Error, None, "Cable {} needs a bend radius of {} mm, drag chain {} has {} mm", tag, format!("{need:.0}"), name, format!("{:.0}", s.bend_radius)));
                }
            }
        }
    }
    v.extend(bonding::problems(prj));
    v.extend(accessories::problems(prj, plan));
    v
}

/// Takes the cut lengths of the routed field cables over as cable lengths. Returns the
/// number of cables changed.
pub fn apply_lengths(prj: &mut Project, plan: &Plan) -> usize {
    let mut n = 0;
    for fc in plan.cables.iter().filter(|c| c.routed()) {
        if let Some(c) = prj.cables.iter_mut().find(|c| c.tag == fc.cable) {
            if (c.length - fc.cut).abs() > 1e-3 {
                c.length = fc.cut;
                n += 1;
            }
        }
    }
    n
}

/// Carriers by part number and length, for the bill of materials.
pub fn material(prj: &Project) -> Vec<(String, f32, Vec<String>)> {
    let f = &prj.field;
    let mut m: BTreeMap<String, (f32, Vec<String>)> = BTreeMap::new();
    for s in f.segments.iter().filter(|s| !s.part.is_empty()) {
        let e = m.entry(s.part.clone()).or_default();
        e.0 += f.segment_length(s);
        e.1.push(f.segment_name(s.id));
    }
    for b in f.bonds.iter().filter(|b| !b.part.is_empty()) {
        let e = m.entry(b.part.clone()).or_default();
        e.0 += b.length;
        e.1.push(format!("{} – {}", b.a, b.b));
    }
    for (part, n, used) in accessories::material(prj) {
        let e = m.entry(part).or_default();
        e.0 += n;
        e.1.extend(used);
    }
    m.into_iter().map(|(k, (q, u))| (k, (q * 100.0).round() / 100.0, u)).collect()
}

#[cfg(test)]
mod tests;
