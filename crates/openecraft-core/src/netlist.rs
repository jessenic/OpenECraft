//! Connectivity analysis: builds electrical nets (potentials) from wires, connection
//! points, terminals, potential connection points and interruption points. Wires
//! between the ports of fluid power and process symbols form fluid nets (hoses,
//! pipes) that are kept apart from the electrical ones.

pub mod junctions;
mod paths;
mod contacts;

use crate::geom::{on_segment_interior, P};
use crate::model::{Page, PageKind, Project};
use crate::symbols::Func;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PinRef {
    pub page: usize,
    /// Index into `page.symbols`.
    pub sym: usize,
    pub pin: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Net {
    pub name: String,
    /// True if the name comes from a potential or interruption point.
    pub named: bool,
    pub pins: Vec<PinRef>,
    /// (page index, wire id)
    pub wires: Vec<(usize, u64)>,
    /// Connects ports of fluid symbols (hose / pipe).
    pub fluid: bool,
    /// Connects fluid ports with electrical connection points (an error).
    pub mixed: bool,
}

#[derive(Default)]
pub struct Netlist {
    pub nets: Vec<Net>,
    pub wire_net: HashMap<u64, usize>,
    /// (symbol id, pin index) -> net
    pub pin_net: HashMap<(u64, usize), usize>,
    /// All connection points on a net, including junction and internal bridge pins.
    /// These are attachment points, not additional device endpoints.
    pub connection_point_net: HashMap<(u64, usize), usize>,
    /// (symbol id, pin index) of pins that touch nothing else.
    pub unconnected: Vec<(usize, u64, usize)>,
}

struct Uf {
    parent: Vec<usize>,
}

impl Uf {
    fn add(&mut self) -> usize {
        self.parent.push(self.parent.len());
        self.parent.len() - 1
    }
    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.parent[rb] = ra;
        }
    }
}

/// What one schematic page contributes to a netlist: its connection points joined by
/// wires and by the bridges inside symbols. It depends only on the page and the symbol
/// definitions, so [`Scans`] keeps it while both stay the same.
#[derive(Clone, Default)]
struct PageScan {
    /// Groups of connected points on the page (local nodes `0..nodes`).
    nodes: usize,
    /// (symbol index, symbol id, pin, node, fluid port, touches nothing else)
    pins: Vec<(usize, u64, usize, usize, bool, bool)>,
    connection_points: Vec<(u64, usize, usize)>,
    /// (wire id, node)
    wires: Vec<(u64, usize)>,
    /// (strip, terminal designation) -> node
    terminals: Vec<((String, String), usize)>,
    /// (plug, pin designation) -> node
    plug_pins: Vec<((String, String), usize)>,
    /// `pot:` / `int:` names -> node
    named: Vec<(String, usize)>,
}

impl PageScan {
    fn new(prj: &Project, page: &Page, bridge_terminals: bool) -> PageScan {
        if contacts::enabled(prj,page) {
            return contacts::scan(prj,page,bridge_terminals);
        }
        let mut uf = Uf { parent: Vec::new() };
        let mut points: HashMap<(i64, i64), usize> = HashMap::new();
        let mut point = |uf: &mut Uf, q: P| *points.entry(q.key()).or_insert_with(|| uf.add());
        // Number of things touching each point (to find unconnected pins).
        let mut degree: HashMap<usize, usize> = HashMap::new();
        let mut pins = Vec::new();
        let mut wires = Vec::new();
        let mut scan = PageScan::default();
        let mut ends = wire_ends(page);
        // Crossings and bridges connect the wires passing through their point.
        for j in page.junctions.iter().filter(|j| j.connects()) {
            ends.entry(j.pos.key()).or_insert((j.pos, 0)).1 += 1;
        }
        // An authored pin is an attachment point even when a conductor passes
        // through it without a saved wire endpoint (e.g. a terminal jumper rail).
        for s in &page.symbols {
            let Some(def) = prj.symbol(&s.sym) else { continue };
            if matches!(def.func, Func::Overview | Func::Graphic) {
                continue;
            }
            for i in 0..def.pins.len() {
                let q = def.pin_world(i, s.pos, s.rot, s.mirror);
                ends.entry(q.key()).or_insert((q, 0));
            }
        }
        for w in &page.wires {
            let na = point(&mut uf, w.a);
            let nb = point(&mut uf, w.b);
            uf.union(na, nb);
            *degree.entry(na).or_default() += 1;
            *degree.entry(nb).or_default() += 1;
            for (q, _) in ends.values() {
                if on_segment_interior(*q, w.a, w.b) {
                    let nq = point(&mut uf, *q);
                    uf.union(na, nq);
                    *degree.entry(nq).or_default() += 2;
                }
            }
            wires.push((w.id, na));
        }
        for (si, s) in page.symbols.iter().enumerate() {
            let Some(def) = prj.symbol(&s.sym) else { continue };
            if matches!(def.func, Func::Overview | Func::Graphic) {
                continue;
            }
            let mut nodes = Vec::new();
            for i in 0..def.pins.len() {
                let q = def.pin_world(i, s.pos, s.rot, s.mirror);
                let n = point(&mut uf, q);
                *degree.entry(n).or_default() += 1;
                nodes.push(n);
                scan.connection_points.push((s.id, i, n));
                if def.func != Func::Junction && !def.pins[i].internal {
                    pins.push((si, s.id, i, n, def.is_fluid()));
                }
            }
            if bridge_terminals || !def.func.is_feedthrough() {
                for &(x, y) in &def.bridges {
                    uf.union(nodes[x], nodes[y]);
                }
            }
            if def.func == Func::Terminal && !nodes.is_empty() {
                scan.terminals.push(((s.tag.clone(), s.label.clone()), nodes[0]));
            }
            if matches!(def.func, Func::Plug(_)) && !nodes.is_empty() && !s.tag.is_empty() {
                scan.plug_pins.push(((s.tag.clone(), s.label.clone()), nodes[0]));
            }
            // A user symbol may have no pin at all.
            match (def.func, nodes.first()) {
                (Func::Potential, Some(&n)) if !s.label.is_empty() => scan.named.push((format!("pot:{}", s.label), n)),
                (Func::Interruption, Some(&n)) if !s.label.is_empty() => scan.named.push((format!("int:{}", s.interruption_key()), n)),
                _ => {}
            }
        }
        // Local nodes: one per group of connected points.
        let mut local: HashMap<usize, usize> = HashMap::new();
        let mut node = |uf: &mut Uf, n: usize| {
            let r = uf.find(n);
            let next = local.len();
            *local.entry(r).or_insert(next)
        };
        scan.pins = pins.into_iter().map(|(si, id, i, n, fluid)| (si, id, i, node(&mut uf, n), fluid, degree.get(&n).copied().unwrap_or(0) <= 1)).collect();
        scan.wires = wires.into_iter().map(|(id, n)| (id, node(&mut uf, n))).collect();
        for (_, _, n) in &mut scan.connection_points {
            *n = node(&mut uf, *n);
        }
        for n in scan.terminals.iter_mut().chain(&mut scan.plug_pins).map(|(_, n)| n).chain(scan.named.iter_mut().map(|(_, n)| n)) {
            *n = node(&mut uf, *n);
        }
        scan.nodes = local.len();
        scan
    }
}

/// The page scans of a netlist, kept between builds: a page is scanned again only when
/// it or the symbol definitions changed (incremental analysis of large projects).
#[derive(Clone, Default)]
pub struct Scans {
    symbols: crate::model::symlib::SymbolList,
    /// Page id -> (fingerprint of the page, scan)
    pages: HashMap<u64, (u64, Arc<PageScan>)>,
    /// Pages scanned by the last build (not taken from earlier builds).
    pub scanned: usize,
}

impl Scans {
    fn get(&mut self, prj: &Project, page: &Page, bridge_terminals: bool) -> Arc<PageScan> {
        let fp = page_fingerprint(prj, page);
        if let Some((have, scan)) = self.pages.get(&page.id) {
            if *have == fp {
                return scan.clone();
            }
        }
        self.scanned += 1;
        let scan = Arc::new(PageScan::new(prj, page, bridge_terminals));
        self.pages.insert(page.id, (fp, scan.clone()));
        scan
    }
}

/// Fingerprint of what a page scan reads from the page.
fn page_fingerprint(prj: &Project, page: &Page) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let explicit = contacts::enabled(prj, page);
    page.values.get("page.explicit_wire_junctions").hash(&mut h);
    page.wires.len().hash(&mut h);
    for w in &page.wires {
        (w.id, w.a.key(), w.b.key(),w.explicit_junctions).hash(&mut h);
    }
    page.symbols.len().hash(&mut h);
    for s in &page.symbols {
        (s.id, &s.sym, s.pos.key(), s.rot, s.mirror, &s.tag, &s.label, s.values.get("function.connection_scope")).hash(&mut h);
    }
    for j in page.junctions.iter().filter(|j| explicit || j.connects()) {
        j.pos.key().hash(&mut h);
    }
    h.finish()
}

/// End points of all wires on a page plus the number of wires ending at each point.
fn wire_ends(page: &Page) -> HashMap<(i64, i64), (P, usize)> {
    let mut ends: HashMap<(i64, i64), (P, usize)> = HashMap::new();
    for w in &page.wires {
        for e in [w.a, w.b] {
            ends.entry(e.key()).or_insert((e, 0)).1 += 1;
        }
    }
    ends
}

/// Positions where a connection dot must be drawn (three or more connections; the
/// point of a crossing or bridge counts as one).
pub fn junctions(prj: &Project, page: &Page) -> Vec<P> {
    end_counts(prj, page).into_values().filter(|(_, n)| *n >= 3).map(|(q, _)| q).collect()
}

/// Wire ends that touch nothing: no other wire, no connection point (wire id, end).
pub fn open_wire_ends(prj: &Project, page: &Page) -> Vec<(u64, P)> {
    if contacts::enabled(prj,page) {return contacts::open_wire_ends(prj,page);}
    let counts = end_counts(prj, page);
    let mut open: Vec<(u64, P)> = Vec::new();
    for w in &page.wires {
        for e in [w.a, w.b] {
            if counts.get(&e.key()).is_some_and(|(_, n)| *n == 1) {
                open.push((w.id, e));
            }
        }
    }
    open
}

/// Wire ends with the number of connections there: wires ending or passing, pins.
fn end_counts(prj: &Project, page: &Page) -> HashMap<(i64, i64), (P, usize)> {
    if contacts::enabled(prj,page) {return contacts::counts(prj,page);}
    let mut ends = wire_ends(page);
    for j in page.junctions.iter().filter(|j| j.connects()) {
        ends.entry(j.pos.key()).or_insert((j.pos, 0)).1 += 1;
    }
    let mut count: HashMap<(i64, i64), (P, usize)> = ends.clone();
    // Ends by their x and y key: a horizontal or vertical wire (the usual case) only
    // passes through ends in its own row or column (±1 key for the 0.01 mm tolerance
    // of `on_segment_interior`), so only those are tested against it.
    let mut by_x: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    let mut by_y: HashMap<i64, Vec<(i64, i64)>> = HashMap::new();
    for k in ends.keys() {
        by_x.entry(k.0).or_default().push(*k);
        by_y.entry(k.1).or_default().push(*k);
    }
    let all: Vec<(i64, i64)> = ends.keys().copied().collect();
    let near = |by: &HashMap<i64, Vec<(i64, i64)>>, c: i64| (c - 1..=c + 1).flat_map(|c| by.get(&c).into_iter().flatten().copied()).collect::<Vec<_>>();
    for w in &page.wires {
        let (a, b) = (w.a.key(), w.b.key());
        let candidates = if a.0 == b.0 {
            near(&by_x, a.0)
        } else if a.1 == b.1 {
            near(&by_y, a.1)
        } else {
            all.clone()
        };
        for k in candidates {
            if on_segment_interior(ends[&k].0, w.a, w.b) {
                count.get_mut(&k).unwrap().1 += 2;
            }
        }
    }
    for s in &page.symbols {
        if let Some(def) = prj.symbol(&s.sym) {
            for i in 0..def.pins.len() {
                let q = def.pin_world(i, s.pos, s.rot, s.mirror);
                if let Some(c) = count.get_mut(&q.key()) {
                    c.1 += 1;
                }
            }
        }
    }
    count
}

impl Netlist {
    /// Builds the netlist. With `bridge_terminals == false` the two sides of a terminal
    /// are kept as separate nets (used for terminal diagrams and wire numbers); with
    /// `true` the jumpers of terminal strips connect their terminals as well.
    pub fn build(prj: &Project, bridge_terminals: bool) -> Netlist {
        Netlist::build_with(prj, bridge_terminals, &mut Scans::default())
    }

    /// Like [`Netlist::build`], scanning only the pages that changed since the last
    /// build with the same `scans` (and the same `bridge_terminals`).
    pub fn build_with(prj: &Project, bridge_terminals: bool, scans: &mut Scans) -> Netlist {
        if !scans.symbols.same_snapshot(&prj.symbols) {
            *scans = Scans { symbols: prj.symbols.clone(), ..Scans::default() };
        }
        scans.scanned = 0;
        let pages: Vec<(usize, Arc<PageScan>)> =
            prj.pages.iter().enumerate().filter(|(_, p)| p.kind == PageKind::Schematic).map(|(pi, p)| (pi, scans.get(prj, p, bridge_terminals))).collect();
        if scans.pages.len() > pages.len() {
            let ids: std::collections::HashSet<u64> = prj.pages.iter().map(|p| p.id).collect();
            scans.pages.retain(|id, _| ids.contains(id));
        }

        let mut uf = Uf { parent: Vec::new() };
        let mut named: HashMap<String, usize> = HashMap::new();
        // (page index, scan, first node of the page)
        let mut offsets = Vec::with_capacity(pages.len());
        // (strip, terminal designation) -> node, for jumpers
        let mut terminals: HashMap<(String, String), usize> = HashMap::new();
        // (plug, pin designation) -> nodes of its pins (male and female are mated)
        let mut plug_pins: HashMap<(String, String), Vec<usize>> = HashMap::new();
        for (pi, scan) in &pages {
            let first = uf.parent.len();
            for _ in 0..scan.nodes {
                uf.add();
            }
            for (key, n) in &scan.terminals {
                terminals.insert(key.clone(), first + n);
            }
            for (key, n) in &scan.plug_pins {
                plug_pins.entry(key.clone()).or_default().push(first + n);
            }
            for (name, n) in &scan.named {
                let node = match named.get(name) {
                    Some(&node) => node,
                    None => {
                        let node = uf.add();
                        named.insert(name.clone(), node);
                        node
                    }
                };
                uf.union(node, first + n);
            }
            offsets.push((*pi, scan, first));
        }

        if bridge_terminals {
            for nodes in plug_pins.values() {
                for w in nodes.windows(2) {
                    uf.union(w[0], w[1]);
                }
            }
            for (strip, labels) in crate::strips::jumper_groups(prj) {
                let nodes: Vec<usize> = labels.iter().filter_map(|l| terminals.get(&(strip.clone(), l.clone())).copied()).collect();
                for w in nodes.windows(2) {
                    uf.union(w[0], w[1]);
                }
            }
        }

        let mut nl = Netlist::default();
        let mut root_net: HashMap<usize, usize> = HashMap::new();
        let mut net_for = |uf: &mut Uf, nets: &mut Vec<Net>, node: usize| -> usize {
            let r = uf.find(node);
            *root_net.entry(r).or_insert_with(|| {
                nets.push(Net::default());
                nets.len() - 1
            })
        };
        let mut electric = vec![];
        for (pi, scan, first) in &offsets {
            for &(si, sym_id, pin, node, fluid, unconnected) in &scan.pins {
                let ni = net_for(&mut uf, &mut nl.nets, first + node);
                nl.nets[ni].pins.push(PinRef { page: *pi, sym: si, pin });
                electric.resize(nl.nets.len(), false);
                if fluid {
                    nl.nets[ni].fluid = true;
                } else {
                    electric[ni] = true;
                }
                nl.pin_net.insert((sym_id, pin), ni);
                if unconnected {
                    nl.unconnected.push((*pi, sym_id, pin));
                }
            }
        }
        for (pi, scan, first) in &offsets {
            for &(wid, node) in &scan.wires {
                let ni = net_for(&mut uf, &mut nl.nets, first + node);
                nl.nets[ni].wires.push((*pi, wid));
                nl.wire_net.insert(wid, ni);
            }
        }
        for (_, scan, first) in &offsets {
            for &(symbol, pin, node) in &scan.connection_points {
                if let Some(&net) = root_net.get(&uf.find(first + node)) {
                    nl.connection_point_net.insert((symbol, pin), net);
                }
            }
        }
        for (ni, net) in nl.nets.iter_mut().enumerate() {
            net.mixed = net.fluid && electric.get(ni).copied().unwrap_or(false);
        }
        // Names: potentials win over interruption points. Two different potentials in
        // one net stay visible as "A / B" (short circuit).
        let mut pots: HashMap<usize, Vec<String>> = HashMap::new();
        let mut ints: HashMap<usize, Vec<String>> = HashMap::new();
        for (name, node) in &named {
            let r = uf.find(*node);
            if let Some(&ni) = root_net.get(&r) {
                match name.split_once(':') {
                    Some(("pot", n)) => pots.entry(ni).or_default().push(n.to_string()),
                    Some((_, n)) => {
                        let label = n.strip_prefix('\0').and_then(|n| n.split_once('\0')).map_or(n, |(_, label)| label);
                        ints.entry(ni).or_default().push(label.to_string());
                    }
                    None => {}
                }
            }
        }
        for (ni, net) in nl.nets.iter_mut().enumerate() {
            if let Some(v) = pots.get_mut(&ni) {
                v.sort();
                net.name = v.join(" / ");
                net.named = true;
            } else if let Some(v) = ints.get_mut(&ni) {
                v.sort();
                net.name = v[0].clone();
                net.named = true;
            }
        }
        let (mut counter, mut fluid) = (0, 0);
        for net in &mut nl.nets {
            if net.named {
            } else if net.fluid && !net.mixed {
                fluid += 1;
                net.name = format!("F{fluid}");
            } else {
                counter += 1;
                net.name = format!("N{counter}");
            }
        }
        nl
    }

    pub fn net_of_wire(&self, id: u64) -> Option<&Net> {
        self.wire_net.get(&id).map(|&i| &self.nets[i])
    }
}

/// A point-to-point connection between two device connection points.
#[derive(Clone, Debug)]
pub struct Connection {
    /// Potential (from the netlist with bridged terminals).
    pub net: String,
    pub a: PinRef,
    pub b: PinRef,
    pub color: String,
    pub cross_section: String,
    pub wire_type: String,
    pub number: String,
    /// Cable tag and core, if the wires are a cable core.
    pub cable: String,
    pub core: String,
    /// Harness the connection belongs to.
    pub harness: String,
    /// Native wire segments on this physical route (page index, wire ID).
    pub wires: Vec<(usize,u64)>,
    pub route_traced: bool,
}

/// Point-to-point connections: the device connection points of each connection of
/// `open` (terminal sides apart) are chained in schematic order (potentials and
/// connectors are not physical wire ends). Devices placed in the same enclosure are
/// chained first, so the wiring stays inside the cabinet and one wire leaves for the
/// devices outside, those on the same busbar system one after the other after the
/// first of the others (its bars connect them, see [`crate::panel::virtual_wiring`]); connections assigned to a harness stay as they are
/// ([`crate::harness::keep_assigned`]). A connection with directed T-junctions takes
/// its wires from them instead ([`junctions::trace`]). Potential names come from `nl`.
/// Fluid nets are left out (see [`crate::fluid::hoses`]).
pub fn connections(prj: &Project, nl: &Netlist, open: &Netlist) -> Vec<Connection> {
    let mut out = Vec::new();
    let assigned = crate::harness::assigned(prj);
    let tees = junctions::Tees::new(prj);
    let paths = paths::Paths::new(prj, open);
    let pos = crate::panel::positions(&prj.layout);
    for net in open.nets.iter().filter(|n| !n.fluid) {
        let on = tees.on_net(prj, net);
        let traced = paths.trace(prj, open, net, &on);
        let pairs: Vec<[PinRef; 2]> = if let Some(traced) = &traced {
            traced.pairs.clone()
        } else if on.is_empty() {
            chained_order(prj, &assigned, net, &pos).windows(2).map(|w| [w[0], w[1]]).collect()
        } else {
            junctions::trace(prj, net, &on).pairs
        };
        if pairs.is_empty() {
            continue;
        }
        let first = pairs[0][0];
        let potential = nl
            .pin_net
            .get(&(prj.pages[first.page].symbols[first.sym].id, first.pin))
            .map(|&n| nl.nets[n].name.clone())
            .unwrap_or_else(|| net.name.clone());
        for (i,pair) in pairs.iter().enumerate() {
            let scoped = traced.as_ref().map(|t| Net {wires:t.wire_routes[i].clone(),..Default::default()});
            let physical = scoped.as_ref().unwrap_or(net);
            let wires = crate::wirenum::wires_of(prj, physical);
            let props = crate::potentials::props_of(prj, nl, physical);
            let (color,cross_section) = (props.color,props.cross_section);
            let at_pin = |p: PinRef| -> Option<String> {
                let s = &prj.pages[p.page].symbols[p.sym];
                let q = prj.symbol(&s.sym)?.pin_world(p.pin, s.pos, s.rot, s.mirror);
                prj.pages[p.page].wires.iter().find(|w| !w.cross_section.is_empty() && (w.a.same(q) || w.b.same(q)) && physical.wires.contains(&(p.page, w.id))).map(|w| w.cross_section.clone())
            };
            let core_wire=wires.iter().find(|w|!w.cable.is_empty());
            let (cable,core) = core_wire.map(|w| (w.cable.clone(),w.core.clone())).unwrap_or_default();
            let color=core_wire.filter(|w|!w.color.is_empty()).map(|w|w.color.clone()).unwrap_or(color);
            let number = core_wire.filter(|w|!w.number.is_empty()).map(|w|w.number.clone()).unwrap_or_else(||crate::wirenum::number_of(prj,physical));
            let harness = crate::harness::of(prj, &pin_designation(prj, pair[0]), &pin_designation(prj, pair[1]));
            out.push(Connection {
                net: potential.clone(),
                a: pair[0],
                b: pair[1],
                color: color.clone(),
                cross_section: [pair[0], pair[1]].into_iter().filter_map(at_pin).min_by(|a, b| crate::harness::outer_diameter(a).total_cmp(&crate::harness::outer_diameter(b))).unwrap_or_else(|| cross_section.clone()),
                wire_type: core_wire.filter(|w|!w.wire_type.is_empty()).or_else(||wires.iter().find(|w|!w.wire_type.is_empty())).map(|w|w.wire_type.clone()).unwrap_or_default(),
                number: number.clone(),
                cable: cable.clone(),
                core: core.clone(),
                harness,
                wires: physical.wires.clone(),
                route_traced: traced.is_some(),
            });
        }
    }
    out
}

/// Trace a drawn route between exact connection points, without named cross-page jumps.
/// This includes virtual endpoints such as interruption points and junction ports.
pub fn local_wire_route(prj:&Project,open:&Netlist,from:(u64,usize),to:(u64,usize))->Option<Vec<(usize,u64)>> {
    paths::local_route(prj,open,from,to)
}

/// The device connection points of a net in the order they are chained: schematic
/// order, devices of joined busbar systems across enclosures one after the other,
/// harness connections kept.
fn chained_order(prj: &Project, assigned: &std::collections::HashSet<[String; 2]>, net: &Net, pos: &[[f32; 3]]) -> Vec<PinRef> {
    let mut pins = chained_pins(prj, net);
    let place = |p: &PinRef| match crate::panel::locate_symbol(prj, &prj.pages[p.page].symbols[p.sym]) {
        Some((e, it)) => match crate::panel::busbar_system(&prj.layout, pos, e, it) {
            Some((first, bar)) => (first, Some(bar)),
            None => (e, None),
        },
        None => (usize::MAX, None),
    };
    pins.sort_by_key(|p| place(p).0);
    // In an enclosure the devices on a busbar system one after the other, after the
    // first of the others.
    let mut start = 0;
    while start < pins.len() {
        let e = place(&pins[start]).0;
        let end = (start..pins.len()).find(|&j| place(&pins[j]).0 != e).unwrap_or(pins.len());
        let (mut on_bars, others): (Vec<PinRef>, Vec<PinRef>) = pins[start..end].iter().partition(|p| place(p).1.is_some());
        on_bars.sort_by_key(|p| place(p).1);
        let order: Vec<PinRef> = others.iter().take(1).chain(&on_bars).chain(others.iter().skip(1)).copied().collect();
        pins[start..end].copy_from_slice(&order);
        start = end;
    }
    crate::harness::keep_assigned(prj, assigned, pins)
}

/// Connection points of the devices in a net in schematic order (page, then left to
/// right, top to bottom); potentials and connectors are left out.
pub fn chained_pins(prj: &Project, net: &Net) -> Vec<PinRef> {
    let mut pins: Vec<PinRef> = net
        .pins
        .iter()
        .filter(|p| {
            let s = &prj.pages[p.page].symbols[p.sym];
            prj.symbol(&s.sym).map(|d| d.func.has_tag()).unwrap_or(false)
        })
        .copied()
        .collect();
    let at = |p: &PinRef| {
        let s = &prj.pages[p.page].symbols[p.sym];
        prj.symbol(&s.sym).map(|d| d.pin_world(p.pin, s.pos, s.rot, s.mirror)).unwrap_or(s.pos)
    };
    // Several source pin records can describe one physical attachment. Keep all
    // in the electrical net, without inventing a zero-length wire inside the device.
    pins.sort_by_key(|p| (p.page,p.sym,p.pin));
    let mut attachments = std::collections::HashSet::new();
    pins.retain(|p| attachments.insert((p.page,p.sym,at(p).key())));
    pins.sort_by(|a, b| {
        let (qa, qb) = (at(a), at(b));
        a.page.cmp(&b.page).then(qa.x.total_cmp(&qb.x)).then(qa.y.total_cmp(&qb.y))
    });
    pins
}

/// Human readable designation of a pin: `-K1:A1`, `=L1`, `-X1:3`.
pub fn pin_designation(prj: &Project, pin: PinRef) -> String {
    let s = &prj.pages[pin.page].symbols[pin.sym];
    let Some(def) = prj.symbol(&s.sym) else { return "?".into() };
    match def.func {
        Func::Potential => format!("={}", s.label),
        Func::Interruption => format!("»{}", s.label),
        Func::Terminal | Func::Plug(_) => format!("{}:{}", s.tag, s.label),
        Func::Plc if def.plc_channel.is_none() && def.plc_channels.is_empty() => format!("{}:{}", s.tag, s.label),
        _ => {
            let pl = s.pins.get(pin.pin).map(String::as_str).unwrap_or("");
            if pl.is_empty() {
                s.tag.clone()
            } else {
                format!("{}:{}", s.tag, pl)
            }
        }
    }
}

#[cfg(test)]
mod tests;
