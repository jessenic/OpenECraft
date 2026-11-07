//! Connection symbols: directed T-junctions and corners.
//!
//! Directed T-junctions (target tracing): a plain net says which connection points
//! share a potential, a T-junction placed where three wires meet says which devices
//! the two physical wires run between. Its three ports are the stem (the direction of
//! the junction) and the two arms of the bar across it; the variant names the source
//! port and the order of its two targets, as the connection symbols of EPLAN do. The
//! wires of a connection are then source – first target and source – second target,
//! the device of a port being the first connection point along its wire, or what the
//! next T-junction on that wire hands on (its targets when the wire reaches its
//! source, its source otherwise). Connections without a T-junction keep the chaining
//! of [`super::connections`].
//!
//! A corner joins two wires at a right angle and changes no routing; like the free
//! arms of a T-junction, its arms are targets of the automatic connections
//! ([`crate::routing::autoconnect`]), so a corner placed first is wired in.
//!
//! Crossings and bridges have four arms and connect the wires passing through their
//! point (a crossing of wires without one stays unconnected, see
//! [`super::PageScan`]). A crossing is two T-junctions joined: the straight line
//! through it is the first target of both its ends, the arms across it their second
//! targets (which end gets which is the variant). A bridge is a star from the side
//! the connection comes from to the three others, the opposite side first. Every
//! symbol is a list of links between its arms ([`Junction::links`]); the device of
//! an arm is the first connection point along its wire, or what the next symbol hands
//! on: the devices of the arms linked with the one the wire comes in at.

use crate::geom::{on_segment_interior, P};
use crate::issue::{Issue, Severity};
use crate::model::{Junction, JunctionKind, Page, Project, TeeDir, Wire};
use crate::netlist::{chained_pins, Connection, Net, Netlist, PinRef, Uf};
use crate::symbols::Func;
use openecraft_i18n::{tr, tr_fmt};
use std::collections::{HashMap, HashSet};

/// Source and its first and second target of a T-junction (directions).
pub fn routing(j: &Junction) -> [TeeDir; 3] {
    let links = j.links();
    [links[0].0, links[0].1, links[1].1]
}

pub const VARIANTS: [u8; 4] = [1, 2, 3, 4];

/// Name of a kind of connection symbol.
pub fn kind_name(kind: JunctionKind) -> &'static str {
    match kind {
        JunctionKind::Tee => tr("T-junction"),
        JunctionKind::Corner => tr("Corner"),
        JunctionKind::Crossing => tr("Crossing"),
        JunctionKind::Bridge => tr("Bridge"),
    }
}

/// Side of a junction a direction points to, for texts.
pub fn side(d: TeeDir) -> &'static str {
    match d {
        TeeDir::Left => tr("on the left"),
        TeeDir::Right => tr("on the right"),
        TeeDir::Up => tr("above"),
        TeeDir::Down => tr("below"),
    }
}

/// Where the connection comes from and where it goes first and second; the arms of a
/// corner; the line and the cross wires of a crossing; the source and the targets of
/// a bridge.
pub fn describe(j: &Junction) -> String {
    let links = j.links();
    match j.kind {
        JunctionKind::Corner => tr_fmt!("Arms {} and {}", side(links[0].0), side(links[0].1)),
        JunctionKind::Tee => tr_fmt!("Source {}, first target {}, second target {}", side(links[0].0), side(links[0].1), side(links[1].1)),
        JunctionKind::Crossing => tr_fmt!(
            "Straight through from {} to {}, across from {} to {} and from {} to {}",
            side(links[0].0),
            side(links[0].1),
            side(links[1].0),
            side(links[1].1),
            side(links[2].0),
            side(links[2].1)
        ),
        JunctionKind::Bridge => tr_fmt!("Source {}, targets {}, {} and {}", side(links[0].0), side(links[0].1), side(links[1].1), side(links[2].1)),
    }
}

/// Axis direction from `from` to `to`; None for a diagonal or no distance.
fn dir_of(from: P, to: P) -> Option<TeeDir> {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    if from.same(to) {
        None
    } else if dy.abs() < 0.005 {
        Some(if dx > 0.0 { TeeDir::Right } else { TeeDir::Left })
    } else if dx.abs() < 0.005 {
        Some(if dy > 0.0 { TeeDir::Down } else { TeeDir::Up })
    } else {
        None
    }
}

fn touches(w: &Wire, q: P) -> bool {
    w.a.same(q) || w.b.same(q) || on_segment_interior(q, w.a, w.b)
}

/// Directions in which wires of the page leave `q` (a wire passing through counts
/// in both); None when a diagonal wire touches the point.
pub fn arms(page: &Page, q: P) -> Option<Vec<TeeDir>> {
    let mut v = Vec::new();
    for w in page.wires.iter().filter(|w| touches(w, q)) {
        if w.a.same(q) {
            v.push(dir_of(q, w.b)?);
        } else if w.b.same(q) {
            v.push(dir_of(q, w.a)?);
        } else {
            v.push(dir_of(q, w.a)?);
            v.push(dir_of(q, w.b)?);
        }
    }
    v.sort();
    v.dedup();
    Some(v)
}

/// The stem of a T formed by three arms: the one with no arm opposite.
pub fn stem_of(arms: &[TeeDir]) -> Option<TeeDir> {
    if arms.len() != 3 {
        return None;
    }
    arms.iter().copied().find(|d| !arms.contains(&d.opposite()))
}

/// The (vertical, horizontal) arms of a corner formed by two arms at a right angle.
pub fn corner_of(arms: &[TeeDir]) -> Option<(TeeDir, TeeDir)> {
    if arms.len() != 2 {
        return None;
    }
    let vertical = arms.iter().copied().find(|d| matches!(d, TeeDir::Up | TeeDir::Down))?;
    let horizontal = arms.iter().copied().find(|d| matches!(d, TeeDir::Left | TeeDir::Right))?;
    Some((vertical, horizontal))
}

/// Free arms of the junctions of a page (position, direction): arms no wire leaves in
/// yet, for the automatic connections.
pub fn free_arms(page: &Page) -> Vec<(u64, P, TeeDir)> {
    let mut v = Vec::new();
    for j in &page.junctions {
        let have = arms(page, j.pos).unwrap_or_default();
        v.extend(j.arms().into_iter().filter(|d| !have.contains(d)).map(|d| (j.id, j.pos, d)));
    }
    v
}

/// A connection point of a symbol lies at `q`.
fn pin_at(prj: &Project, page: &Page, q: P) -> bool {
    page.symbols.iter().any(|s| prj.symbol(&s.sym).is_some_and(|def| (0..def.pins.len()).any(|i| def.pin_world(i, s.pos, s.rot, s.mirror).same(q))))
}

/// The wires at the junction form it: a T-junction's stem and two arms across it, a
/// corner's two arms, the four arms of a crossing or a bridge, nothing else and no
/// connection point there.
pub fn fits(prj: &Project, page: &Page, j: &Junction) -> bool {
    let Some(arms) = arms(page, j.pos) else { return false };
    let mut want = j.arms();
    want.sort();
    arms == want && !pin_at(prj, page, j.pos)
}

/// The connection symbols of the project that route a connection (T-junctions,
/// crossings, bridges) and fit their wires, by page.
pub struct Tees<'a> {
    pages: Vec<Vec<&'a Junction>>,
}

impl<'a> Tees<'a> {
    pub fn new(prj: &'a Project) -> Tees<'a> {
        Tees { pages: prj.pages.iter().map(|pg| pg.junctions.iter().filter(|j| !j.is_corner() && fits(prj, pg, j)).collect()).collect() }
    }

    pub fn is_empty(&self) -> bool {
        self.pages.iter().all(Vec::is_empty)
    }

    /// The fitting junctions on the wires of `net` (page, junction).
    pub fn on_net(&self, prj: &Project, net: &Net) -> Vec<(usize, &'a Junction)> {
        let mut out: Vec<(usize, &Junction)> = Vec::new();
        for &(pi, wid) in &net.wires {
            if self.pages[pi].is_empty() {
                continue;
            }
            let Some(w) = prj.pages[pi].wires.iter().find(|w| w.id == wid) else { continue };
            for &j in &self.pages[pi] {
                if touches(w, j.pos) && !out.iter().any(|(p, o)| *p == pi && o.id == j.id) {
                    out.push((pi, j));
                }
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.pos.x.total_cmp(&b.1.pos.x)).then(a.1.pos.y.total_cmp(&b.1.pos.y)));
        out
    }
}

/// The physical wires of a connection with connection symbols.
#[derive(Default, Debug)]
pub struct Traced {
    /// Source first, then the target.
    pub pairs: Vec<[PinRef; 2]>,
    /// The connection is wired in a loop: its symbols give it more wires than its
    /// devices need (e.g. the sources of two T-junctions facing each other).
    pub loop_: bool,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum Key {
    Point(usize, (i64, i64)),
    Port(usize, TeeDir),
    Named(String),
}

/// Traces the connection points of `net` through its connection symbols `tees` (from
/// [`Tees::on_net`]): the pins reached from each arm are paired after the links of
/// the symbols.
pub fn trace(prj: &Project, net: &Net, tees: &[(usize, &Junction)]) -> Traced {
    let mut uf = Uf { parent: Vec::new() };
    let mut nodes: HashMap<Key, usize> = HashMap::new();
    let mut node = |uf: &mut Uf, k: Key| *nodes.entry(k).or_insert_with(|| uf.add());
    let tee_at = |pi: usize, q: P| tees.iter().position(|(p, j)| *p == pi && j.pos.same(q));
    // Wires, cut at the symbols and at the ends of other wires lying on them.
    for &(pi, wid) in &net.wires {
        let page = &prj.pages[pi];
        let Some(w) = page.wires.iter().find(|w| w.id == wid) else { continue };
        let mut cuts: Vec<P> = vec![w.a, w.b];
        cuts.extend(tees.iter().filter(|(p, j)| *p == pi && on_segment_interior(j.pos, w.a, w.b)).map(|(_, j)| j.pos));
        cuts.extend(page.wires.iter().flat_map(|o| [o.a, o.b]).filter(|&e| on_segment_interior(e, w.a, w.b)));
        let along = |q: P| (q.x - w.a.x) * (w.b.x - w.a.x) + (q.y - w.a.y) * (w.b.y - w.a.y);
        cuts.sort_by(|x, y| along(*x).total_cmp(&along(*y)));
        cuts.dedup_by(|x, y| x.same(*y));
        for seg in cuts.windows(2) {
            let (s, e) = (seg[0], seg[1]);
            let key = |q: P, other: P| match tee_at(pi, q) {
                Some(k) => match dir_of(q, other).filter(|d| tees[k].1.arms().contains(d)) {
                    Some(d) => Key::Port(k, d),
                    None => Key::Point(pi, q.key()),
                },
                None => Key::Point(pi, q.key()),
            };
            let (ns, ne) = (node(&mut uf, key(s, e)), node(&mut uf, key(e, s)));
            uf.union(ns, ne);
        }
    }
    // Connection points: bridges inside symbols and the names of potential and
    // interruption points join them as the netlist does.
    for pin in &net.pins {
        let s = &prj.pages[pin.page].symbols[pin.sym];
        let Some(def) = prj.symbol(&s.sym) else { continue };
        let q = def.pin_world(pin.pin, s.pos, s.rot, s.mirror);
        let n = node(&mut uf, Key::Point(pin.page, q.key()));
        let named = match def.func {
            Func::Potential if !s.label.is_empty() => Some(format!("pot:{}", s.label)),
            Func::Interruption if !s.label.is_empty() => Some(format!("int:{}", s.interruption_key())),
            _ => None,
        };
        if let Some(name) = named {
            let m = node(&mut uf, Key::Named(name));
            uf.union(n, m);
        }
        if !def.func.is_feedthrough() {
            for &(_, y) in def.bridges.iter().filter(|(x, _)| *x == pin.pin) {
                let other = def.pin_world(y, s.pos, s.rot, s.mirror);
                let m = node(&mut uf, Key::Point(pin.page, other.key()));
                uf.union(n, m);
            }
        }
    }
    // Components: their device pins in schematic order and the arms leading into them.
    let mut comp_of: HashMap<usize, usize> = HashMap::new();
    let mut comp = |uf: &mut Uf, n: usize| {
        let r = uf.find(n);
        let next = comp_of.len();
        *comp_of.entry(r).or_insert(next)
    };
    let mut pins_of: HashMap<usize, Vec<PinRef>> = HashMap::new();
    for pin in chained_pins(prj, net) {
        let s = &prj.pages[pin.page].symbols[pin.sym];
        let Some(def) = prj.symbol(&s.sym) else { continue };
        let q = def.pin_world(pin.pin, s.pos, s.rot, s.mirror);
        if let Some(&n) = nodes.get(&Key::Point(pin.page, q.key())) {
            let c = comp(&mut uf, n);
            pins_of.entry(c).or_default().push(pin);
        }
    }
    let mut port_comp: HashMap<(usize, TeeDir), usize> = HashMap::new();
    let mut ports_of: HashMap<usize, Vec<(usize, TeeDir)>> = HashMap::new();
    for (k, (_, j)) in tees.iter().enumerate() {
        for arm in j.arms() {
            if let Some(&n) = nodes.get(&Key::Port(k, arm)) {
                let c = comp(&mut uf, n);
                port_comp.insert((k, arm), c);
                ports_of.entry(c).or_default().push((k, arm));
            }
        }
    }
    let ctx = Ctx { tees, port_comp, ports_of, pins_of };
    let mut out = Traced::default();
    let mut seen: HashSet<(PinRef, PinRef)> = HashSet::new();
    let mut push = |out: &mut Traced, a: PinRef, b: PinRef| {
        if a != b && seen.insert(if (a.page, a.sym, a.pin) <= (b.page, b.sym, b.pin) { (a, b) } else { (b, a) }) {
            out.pairs.push([a, b]);
        }
    };
    for (k, (_, j)) in tees.iter().enumerate() {
        for (from, to) in j.links() {
            let sources = ctx.beyond(k, from, &mut vec![k]);
            for &b in &ctx.beyond(k, to, &mut vec![k]) {
                for &a in &sources {
                    push(&mut out, a, b);
                }
            }
        }
    }
    // Several devices on one wire piece are chained in schematic order.
    let mut comps: Vec<&Vec<PinRef>> = ctx.pins_of.values().collect();
    comps.sort_by_key(|pins| (pins[0].page, pins[0].sym, pins[0].pin));
    for pins in comps {
        for pair in pins.windows(2) {
            push(&mut out, pair[0], pair[1]);
        }
    }
    let devices: HashSet<PinRef> = out.pairs.iter().flatten().copied().collect();
    out.loop_ = out.pairs.len() + 1 > devices.len();
    out
}

struct Ctx<'a> {
    tees: &'a [(usize, &'a Junction)],
    port_comp: HashMap<(usize, TeeDir), usize>,
    ports_of: HashMap<usize, Vec<(usize, TeeDir)>>,
    pins_of: HashMap<usize, Vec<PinRef>>,
}

impl Ctx<'_> {
    /// The devices the wire of arm `arm` of symbol `k` leads to: the first connection
    /// point on it, or what the symbols it reaches hand on: the devices of the arms
    /// linked with the arm the wire comes in at.
    fn beyond(&self, k: usize, arm: TeeDir, path: &mut Vec<usize>) -> Vec<PinRef> {
        let Some(&c) = self.port_comp.get(&(k, arm)) else { return Vec::new() };
        if let Some(pins) = self.pins_of.get(&c).filter(|p| !p.is_empty()) {
            return vec![pins[0]];
        }
        let mut out = Vec::new();
        for &(k2, arm2) in self.ports_of.get(&c).into_iter().flatten() {
            if k2 == k || path.contains(&k2) {
                continue;
            }
            path.push(k2);
            for (a, b) in self.tees[k2].1.links() {
                if a == arm2 {
                    out.extend(self.beyond(k2, b, path));
                } else if b == arm2 {
                    out.extend(self.beyond(k2, a, path));
                }
            }
            path.pop();
        }
        out
    }
}

/// The connections of the net a junction lies on (indices into `conns`).
pub fn connections_of(prj: &Project, open: &Netlist, conns: &[Connection], page: usize, j: &Junction) -> Vec<usize> {
    let pg = &prj.pages[page];
    let Some(w) = pg.wires.iter().find(|w| touches(w, j.pos)) else { return Vec::new() };
    let Some(net) = open.net_of_wire(w.id) else { return Vec::new() };
    (0..conns.len()).filter(|&i| net.pins.contains(&conns[i].a) && net.pins.contains(&conns[i].b)).collect()
}

/// Junctions that do not fit their wires, two on one point, and sources facing
/// each other.
pub fn problems(prj: &Project, open: &Netlist) -> Vec<Issue> {
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        for (i, j) in pg.junctions.iter().enumerate() {
            let target = Some((pi, j.id));
            if !fits(prj, pg, j) {
                v.push(match j.kind {
                    JunctionKind::Tee => crate::issue!(Severity::Warning, target, "T-junction on page {} does not fit its wires: it needs one wire in its direction, two across and no connection point there", pg.name),
                    JunctionKind::Corner => crate::issue!(Severity::Warning, target, "Corner on page {} does not fit its wires: it needs one wire in each of its two directions and no connection point there", pg.name),
                    JunctionKind::Crossing => crate::issue!(Severity::Warning, target, "Crossing on page {} does not fit its wires: it needs a wire in all four directions and no connection point there", pg.name),
                    JunctionKind::Bridge => crate::issue!(Severity::Warning, target, "Bridge on page {} does not fit its wires: it needs a wire in all four directions and no connection point there", pg.name),
                });
            }
            if pg.junctions[..i].iter().any(|o| o.pos.same(j.pos)) {
                v.push(crate::issue!(Severity::Warning, target, "Two connection symbols on page {} lie on the same point", pg.name));
            }
        }
    }
    let tees = Tees::new(prj);
    if tees.is_empty() {
        return v;
    }
    for net in open.nets.iter().filter(|n| !n.fluid) {
        let on = tees.on_net(prj, net);
        if on.is_empty() {
            continue;
        }
        if trace(prj, net, &on).loop_ {
            for (pi, j) in on {
                v.push(crate::issue!(Severity::Warning, Some((pi, j.id)), "Connection symbol on page {} wires its connection in a loop: the symbols of the connection give it more wires than its devices need", prj.pages[pi].name));
            }
        }
    }
    v
}

#[cfg(test)]
mod tests;
