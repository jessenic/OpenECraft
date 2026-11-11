//! Routing of connections: orthogonal paths on the grid around symbols and other
//! wires, and automatic connections between connection points facing each other.

use crate::cabledef;
use crate::conndef;
use crate::geom::{dist_to_segment, on_segment_interior, p, P, R, GRID};
use crate::ops;
use crate::symlib::Symbols;
use crate::symbols::Func;
use crate::model::*;
use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

/// Points closer than this touch.
const TOUCH: f32 = 0.01;

/// Connection points of the symbols on a page with their outward direction, and the
/// free arms of its connection symbols (T-junctions, corners) as points facing their
/// direction.
fn pins(symbols: &Symbols, page: &Page) -> Vec<(u64, P, P)> {
    let mut v: Vec<(u64, P, P)> = crate::netlist::junctions::free_arms(page).into_iter().map(|(id, q, d)| (id, q, d.vector())).collect();
    for s in &page.symbols {
        let Some(def) = symbols.get(&s.sym) else { continue };
        if def.func == Func::Graphic {
            continue;
        }
        let c = s.bounds(&def).center();
        for i in 0..def.pins.len() {
            if def.pins[i].internal {continue;}
            let q = def.pin_world(i, s.pos, s.rot, s.mirror);
            if def.pin_directions.contains_key(&i) {
                let mask=def.pin_connection_directions(i,s.rot,s.mirror);
                if mask==0 {v.push((s.id,q,p(0.,0.)));}
                for (bit,dir) in [(1,p(0.,-1.)),(2,p(1.,0.)),(4,p(0.,1.)),(8,p(-1.,0.))] {
                    if mask&bit!=0 {v.push((s.id,q,dir));}
                }
                continue;
            }
            let d = q - c;
            let dir = if d.x.abs() < 1e-3 && d.y.abs() < 1e-3 {
                p(0.0, 0.0)
            } else if d.y.abs() >= d.x.abs() {
                p(0.0, d.y.signum())
            } else {
                p(d.x.signum(), 0.0)
            };
            v.push((s.id, q, dir));
        }
    }
    v
}

/// A connection point is in use when a wire ends there or another pin touches it. The
/// arm of a connection symbol is in use when a wire leaves it in its direction.
fn pin_used(symbols:&Symbols,page: &Page, all: &[(u64, P, P)], sym: u64, q: P, dir: P) -> bool {
    if page.junctions.iter().any(|j| j.id == sym) {
        return page.wires.iter().any(|w| {
            let other = if w.a.same(q) { Some(w.b) } else if w.b.same(q) { Some(w.a) } else { None };
            let parallel = |d: P| (d.x * dir.y - d.y * dir.x).abs() < 0.01;
            other.is_some_and(|o| parallel(o - q) && (o.x - q.x) * dir.x + (o.y - q.y) * dir.y > 0.0) || (on_segment_interior(q, w.a, w.b) && parallel(w.b - w.a))
        }) || all.iter().any(|(id, o, _)| *id != sym && o.same(q));
    }
    let side_contact=page.symbols.iter().find(|s|s.id==sym).and_then(|s|symbols.get(&s.sym)).is_some_and(|d|!d.func.has_tag());
    page.wires.iter().any(|w| w.a.same(q) || w.b.same(q) || (side_contact || !w.requires_explicit_junctions(page)) && on_segment_interior(q, w.a, w.b))
        || all.iter().any(|(id, o, _)| *id != sym && o.same(q))
}

/// Can a wire run from `a` to `b` without touching anything but its end points?
fn clear(symbols: &Symbols, page: &Page, all: &[(u64, P, P)], a: P, b: P, ends: [u64; 2]) -> bool {
    let blocked=page.wires.iter().any(|w| {
        let touching=on_segment_interior(a,w.a,w.b)||on_segment_interior(b,w.a,w.b);
        let d=w.b-w.a;let route=b-a;
        touching && (!w.requires_explicit_junctions(page) || (d.x*route.y-d.y*route.x).abs()<0.01)
    });
    !blocked && unobstructed(symbols, page, all, a, b, &ends)
}

/// Is the way from `a` to `b` free of connection points, wire ends and symbols other
/// than `ends`? Its end points may lie on wires.
fn unobstructed(symbols: &Symbols, page: &Page, all: &[(u64, P, P)], a: P, b: P, ends: &[u64]) -> bool {
    let inner = |q: P| on_segment_interior(q, a, b);
    if all.iter().any(|(_, q, _)| inner(*q)) || page.wires.iter().any(|w| inner(w.a) || inner(w.b)) {
        return false;
    }
    for s in page.symbols.iter().filter(|s| !ends.contains(&s.id)) {
        let Some(def) = symbols.get(&s.sym) else { continue };
        if def.func == Func::Graphic {
            continue;
        }
        let r = s.bounds(&def);
        let mid = p((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        // Axis aligned segment against the box.
        let seg = R::from_points(a, b);
        let overlaps = seg.min.x <= r.max.x && seg.max.x >= r.min.x && seg.min.y <= r.max.y && seg.max.y >= r.min.y;
        if overlaps || r.contains(mid) {
            return false;
        }
    }
    true
}

/// Wires that connect free connection points of the selected symbols (and free arms
/// of the selected connection symbols) with free connection points or arms lying
/// straight ahead and facing them.
pub fn autoconnect(symbols: &Symbols, page: &Page, sel: &BTreeSet<u64>, max_len: f32) -> Vec<(P, P)> {
    let all = pins(symbols, page);
    let mut out: Vec<(P, P)> = Vec::new();
    // (position, direction) of the points wired already.
    let mut taken: Vec<(P, P)> = Vec::new();
    let is_taken = |taken: &[(P, P)], q: P, dir: P| taken.iter().any(|(t, td)| t.same(q) && (*td == dir || td.len() == 0.0));
    for (sym, q, dir) in all.iter().filter(|(id, _, _)| sel.contains(id)) {
        if dir.len() == 0.0 || is_taken(&taken, *q, *dir) || pin_used(symbols,page, &all, *sym, *q, *dir) {
            continue;
        }
        let mut best: Option<(f32, u64, P, P)> = None;
        for (osym, o, odir) in &all {
            if osym == sym || !(odir.x == -dir.x && odir.y == -dir.y) || is_taken(&taken, *o, *odir) {
                continue;
            }
            let d = *o - *q;
            let along = d.x * dir.x + d.y * dir.y;
            let across = (d.x * dir.y - d.y * dir.x).abs();
            if across > 0.01 || along <= 0.01 || along > max_len || best.is_some_and(|b| b.0 <= along) {
                continue;
            }
            if pin_used(symbols,page, &all, *osym, *o, *odir) {
                continue;
            }
            best = Some((along, *osym, *o, *odir));
        }
        let Some((_, osym, o, odir)) = best else { continue };
        let is_arm = |id: u64| page.junctions.iter().any(|j| j.id == id);
        // A pin gets one wire; the arms of a connection symbol share its point.
        if clear(symbols, page, &all, *q, o, [*sym, osym]) && (is_arm(*sym) || !out.iter().any(|(a, b)| dist_to_segment(*q, *a, *b) < 0.01)) {
            out.push((*q, o));
            // A symbol's pin is taken as a whole, an arm only in its direction.
            let whole = |id: u64, d: P| if is_arm(id) { d } else { p(0.0, 0.0) };
            taken.push((*q, whole(*sym, *dir)));
            taken.push((o, whole(osym, odir)));
        }
    }
    out
}

/// Adds the wires found by [`autoconnect`]. Returns their number.
pub fn apply_autoconnect(prj: &mut Project, page: usize, sel: &BTreeSet<u64>) -> usize {
    let segs = autoconnect(&prj.symbol_table(), &prj.pages[page], sel, 400.0);
    for (a, b) in &segs {
        let id = prj.new_id();
        prj.pages[page].wires.push(Wire { id, a: *a, b: *b, ..Default::default() });
    }
    segs.len()
}

/// A connection definition point on a wire: where it is and how far along the wire,
/// as a share of its length.
struct HeldPoint {
    id: u64,
    pos: P,
    wire: u64,
    along: f32,
}

/// A cable definition line: its ends, the wires it crosses and how far it reaches
/// past the first and the last of them.
struct HeldLine {
    id: u64,
    a: P,
    b: P,
    wires: Vec<u64>,
    past: (f32, f32),
}

/// A page before items move: its wires and the points and lines on them that do not
/// move by themselves, which stay on their wires.
pub struct Before {
    wires: Vec<Wire>,
    points: Vec<HeldPoint>,
    lines: Vec<HeldLine>,
}

impl Before {
    pub fn of(page: &Page, sel: &BTreeSet<u64>) -> Before {
        let free = page.conn_defs.iter().filter(|c| !sel.contains(&c.id));
        let points = free.filter_map(|c| {
            let w = conndef::wire_at(page, c.pos)?;
            Some(HeldPoint { id: c.id, pos: c.pos, wire: w.id, along: w.a.dist(c.pos) / w.a.dist(w.b).max(TOUCH) })
        });
        let lines = page.cable_defs.iter().filter(|c| !sel.contains(&c.id)).filter_map(|c| {
            let wires = cabledef::crossed_wires(page, c.a, c.b);
            let dir = (c.b - c.a) * (1.0 / c.a.dist(c.b).max(TOUCH));
            let (first, last) = reach(page.wires.iter().filter(|w| wires.contains(&w.id)), c.a, dir)?;
            Some(HeldLine { id: c.id, a: c.a, b: c.b, wires, past: (first, c.a.dist(c.b) - last) })
        });
        Before { lines: lines.collect(), points: points.collect(), wires: page.wires.clone() }
    }
}

/// A selection moving step by step (a drag). Each step squares the wires again from
/// their stretched state, so the result depends on where the selection is, not on
/// the way it took.
pub struct Moving {
    attached: Vec<(u64, bool)>,
    before: Before,
    stretched: Vec<Wire>,
    next_id: u64,
}

impl Moving {
    pub fn new(prj: &Project, page: usize, sel: &BTreeSet<u64>) -> Moving {
        let pg = &prj.pages[page];
        let attached = ops::attached_wire_ends(&prj.symbol_table(), pg, sel);
        Moving { attached, before: Before::of(pg, sel), stretched: pg.wires.clone(), next_id: prj.next_id }
    }

    /// Moves the selection `d` further; attached wires stretch and stay square.
    pub fn step(&mut self, prj: &mut Project, page: usize, sel: &BTreeSet<u64>, d: P) {
        let pg = &mut prj.pages[page];
        std::mem::swap(&mut pg.wires, &mut self.stretched);
        ops::translate(pg, sel, &self.attached, d);
        self.stretched.clone_from(&pg.wires);
        prj.next_id = self.next_id;
        square_wires(prj, page, &self.before);
    }
}

/// Moves a selection by `d`; attached wires stretch and stay square.
pub fn move_selection(prj: &mut Project, page: usize, sel: &BTreeSet<u64>, d: P) {
    Moving::new(prj, page, sel).step(prj, page, sel, d);
}

/// Rotates a selection by 90 degrees; attached wires stay square.
pub fn rotate(prj: &mut Project, page: usize, sel: &BTreeSet<u64>) {
    let before = Before::of(&prj.pages[page], sel);
    ops::rotate(&prj.symbol_table(), &mut prj.pages[page], sel);
    square_wires(prj, page, &before);
}

fn upright(w: &Wire) -> bool {
    (w.a.x - w.b.x).abs() < TOUCH
}

fn level(w: &Wire) -> bool {
    (w.a.y - w.b.y).abs() < TOUCH
}

/// Runs `f` on the page without the wires `skip` selects.
fn without<T>(page: &mut Page, skip: impl Fn(&Wire) -> bool, f: impl FnOnce(&Page) -> T) -> T {
    let all = std::mem::take(&mut page.wires);
    page.wires = all.iter().filter(|w| !skip(w)).cloned().collect();
    let out = f(page);
    page.wires = all;
    out
}

/// Nothing to connect to at `q`: neither a connection point nor a wire.
fn free(page: &Page, all: &[(u64, P, P)], q: P) -> bool {
    !all.iter().any(|(_, o, _)| o.same(q)) && !page.wires.iter().any(|w| dist_to_segment(q, w.a, w.b) < TOUCH)
}

/// Can a wire ending at `from` go to `to` along the straight run of wires it is on?
fn on_run(page: &Page, from: P, to: P) -> bool {
    let len = from.dist(to);
    let dir = (to - from) * (1.0 / len);
    let along = |q: P| (q - from).x * dir.x + (q - from).y * dir.y;
    let on_line = |q: P| ((q - from).x * dir.y - (q - from).y * dir.x).abs() < TOUCH;
    let mut reach = 0.0;
    loop {
        let wires = page.wires.iter().filter(|w| on_line(w.a) && on_line(w.b));
        let next = wires.filter(|w| along(w.a).min(along(w.b)) <= reach + TOUCH).map(|w| along(w.a).max(along(w.b))).fold(reach, f32::max);
        if next >= len - TOUCH {
            return true;
        }
        if next <= reach + TOUCH {
            return false;
        }
        reach = next;
    }
}

/// Is the path clear for the connection between symbols `ends`? Its corners must not
/// touch anything, or they would connect it.
fn path_clear(symbols: &Symbols, page: &Page, all: &[(u64, P, P)], path: &[P], ends: &[u64]) -> bool {
    path.windows(2).all(|s| unobstructed(symbols, page, all, s[0], s[1], ends)) && path[1..path.len() - 1].iter().all(|q| free(page, all, *q))
}

/// Connection from `a` to `b` that leaves both in the direction of the wire it
/// replaces, with a jog between them.
fn jog(a: P, b: P, vertical: bool, clear: impl Fn(&[P]) -> bool) -> Option<Vec<P>> {
    let (from, to) = if vertical { (a.y, b.y) } else { (a.x, b.x) };
    [1.0 / 2.0, 1.0 / 3.0, 2.0 / 3.0, 1.0 / 4.0, 3.0 / 4.0].into_iter().find_map(|t: f32| {
        let v = ((from + (to - from) * t) / GRID).round() * GRID;
        let path = if vertical { [a, p(a.x, v), p(b.x, v), b] } else { [a, p(v, a.y), p(v, b.y), b] };
        (!path[1].same(a) && !path[2].same(b) && clear(&path)).then(|| path.to_vec())
    })
}

/// Point at distance `at` along the line through `pts`.
fn point_along(pts: &[P], mut at: f32) -> P {
    for s in pts.windows(2) {
        let len = s[0].dist(s[1]);
        if at <= len {
            return s[0] + (s[1] - s[0]) * (at / len.max(TOUCH));
        }
        at -= len;
    }
    pts[pts.len() - 1]
}

/// Keeps the points of `before` on their wires. `bent` holds the wires that were
/// split or routed anew, with their course and the wires that now run it; a point
/// that left its wire takes the same share of its length, or a place near it where no
/// other wire takes it.
fn place_points(page: &mut Page, before: &Before, bent: &HashMap<u64, (Vec<P>, Vec<u64>)>) {
    for h in &before.points {
        let (line, owners) = match (bent.get(&h.wire), page.wires.iter().find(|w| w.id == h.wire)) {
            (Some((l, o)), _) => (l.clone(), o.clone()),
            (None, Some(w)) => (vec![w.a, w.b], vec![w.id]),
            (None, None) => continue,
        };
        let own = |q: P| conndef::wire_at(page, q).is_some_and(|x| owners.contains(&x.id));
        let len: f32 = line.windows(2).map(|s| s[0].dist(s[1])).sum();
        let at = |steps: f32| point_along(&line, (h.along * len + steps * GRID).clamp(0.0, len));
        let pos = if own(h.pos) { h.pos } else { [0.0, 1.0, -1.0, 2.0, -2.0].map(at).into_iter().find(|q| own(*q)).unwrap_or_else(|| at(0.0)) };
        if let Some(c) = page.conn_defs.iter_mut().find(|c| c.id == h.id) {
            c.pos = pos;
        }
    }
}

/// Redistributes placed labels once when a stretched wire becomes several legs.
fn place_wire_labels(page: &mut Page, old: &Wire, course: &[P], added: &[u64]) {
    let owns = |id| id == old.id || added.contains(&id);
    for wire in page.wires.iter_mut().filter(|w| owns(w.id)) {wire.labels.clear();}
    let length: f32 = course.windows(2).map(|s| s[0].dist(s[1])).sum();
    for label in &old.labels {
        let previous = label.attachment(old);
        let at = if course.windows(2).any(|s| dist_to_segment(previous, s[0], s[1]) < TOUCH) {previous}
            else {point_along(course, label.along * length)};
        if let Some(wire) = page.wires.iter_mut().find(|w| owns(w.id) && dist_to_segment(at, w.a, w.b) < TOUCH) {
            let mut placed = label.clone();
            placed.along = wire.a.dist(at) / wire.a.dist(wire.b).max(TOUCH);
            wire.labels.push(placed);
        }
    }
}

/// Distance along the line through `pts` to its point nearest to `q`.
fn distance_along(pts: &[P], q: P) -> f32 {
    let (mut best, mut at, mut walked) = (f32::MAX, 0.0, 0.0);
    for s in pts.windows(2) {
        let (d, len) = (s[1] - s[0], s[0].dist(s[1]));
        let t = if len > 0.0 { (((q - s[0]).x * d.x + (q - s[0]).y * d.y) / (len * len)).clamp(0.0, 1.0) } else { 0.0 };
        let gap = (s[0] + d * t).dist(q);
        if gap < best {
            (best, at) = (gap, walked + len * t);
        }
        walked += len;
    }
    at
}

/// Puts the labels of the wires `old` of a connection that ran along `was` at the
/// same share of the length along `course`, on the wires `owners` that run it now.
fn move_labels(page: &mut Page, old: &[Wire], was: &[P], course: &[P], owners: &[u64]) {
    let length = |pts: &[P]| pts.windows(2).map(|s| s[0].dist(s[1])).sum::<f32>();
    let (from, to) = (length(was).max(TOUCH), length(course));
    let labels: Vec<(f32, crate::model::wiring::WireLabel)> = old.iter().flat_map(|w| w.labels.iter().map(move |l| (distance_along(was, l.attachment(w)) / from, l.clone()))).collect();
    for x in page.wires.iter_mut().filter(|x| owners.contains(&x.id)) {
        x.labels.clear();
    }
    for (share, mut label) in labels {
        let at = point_along(course, share * to);
        if let Some(x) = page.wires.iter_mut().find(|x| owners.contains(&x.id) && dist_to_segment(at, x.a, x.b) < TOUCH) {
            label.along = x.a.dist(at) / x.a.dist(x.b).max(TOUCH);
            x.labels.push(label);
        }
    }
}

/// The wires that go on from `from`, where the wire `skip` ends: past corners where
/// one unchanged wire goes on and nothing else connects, up to a connection point, a
/// connection symbol, a branch or a wire running through. Returns their ids and the
/// course from `from` to where the connection ends.
fn connection(page: &Page, all: &[(u64, P, P)], before: &Before, skip: u64, from: P) -> (Vec<u64>, Vec<P>) {
    let unchanged = |x: &Wire| before.wires.iter().any(|o| o.id == x.id && o.a.same(x.a) && o.b.same(x.b));
    let (mut ids, mut course, mut last) = (Vec::new(), vec![from], skip);
    loop {
        let at = course[course.len() - 1];
        if all.iter().any(|(_, q, _)| q.same(at)) || page.junctions.iter().any(|j| j.pos.same(at)) {
            break;
        }
        let mut touching = page.wires.iter().filter(|x| x.id != last && x.id != skip && dist_to_segment(at, x.a, x.b) < TOUCH);
        let (Some(next), None) = (touching.next(), touching.next()) else { break };
        let far = if next.a.same(at) {
            next.b
        } else if next.b.same(at) {
            next.a
        } else {
            break;
        };
        if !unchanged(next) || ids.contains(&next.id) {
            break;
        }
        ids.push(next.id);
        course.push(far);
        last = next.id;
    }
    (ids, course)
}

/// What a page needs to square or route its wires again.
struct Ctx<'a> {
    symbols: &'a Symbols,
    area: R,
    all: &'a [(u64, P, P)],
    before: &'a Before,
}

/// Wires split or routed anew: their course with the wires that run it (connection
/// definition points), and the wires added to each (cable definition lines).
#[derive(Default)]
struct Splits {
    points: HashMap<u64, (Vec<P>, Vec<u64>)>,
    lines: HashMap<u64, Vec<u64>>,
}

/// Routes the connection of the wire `w`, diagonal after a move, anew: from its moved
/// end `m` past the corners after its far end `f` to where the connection ends, the
/// way of the least weight ([`REROUTE`]). The wires past `f` are replaced. Returns
/// false when there is no such way.
fn reroute(prj: &mut Project, page_i: usize, cx: &Ctx, w: &Wire, (m, f): (P, P), pending: &BTreeSet<u64>, splits: &mut Splits) -> bool {
    let page = &prj.pages[page_i];
    let (chain, mut was) = connection(page, cx.all, cx.before, w.id, f);
    let goal = was[was.len() - 1];
    if goal.same(m) {
        return false;
    }
    let mut old = vec![w.clone()];
    old.extend(chain.iter().filter_map(|id| page.wires.iter().find(|x| x.id == *id)).cloned());
    let gone = |x: &Wire| x.id == w.id || chain.contains(&x.id) || pending.contains(&x.id);
    let Some(path) = without(&mut prj.pages[page_i], gone, |page| search(cx.symbols, page, cx.area, m, goal, REROUTE)) else { return false };
    was.insert(0, m);
    let page = &mut prj.pages[page_i];
    page.wires.retain(|x| !chain.contains(&x.id));
    if let Some(x) = page.wires.iter_mut().find(|x| x.id == w.id) {
        (x.a, x.b) = (path[0], path[1]);
    }
    let mut owners = vec![w.id];
    for seg in path[1..].windows(2) {
        let nid = prj.new_id();
        prj.pages[page_i].wires.push(Wire { id: nid, a: seg[0], b: seg[1], labels: Vec::new(), ..w.clone() });
        owners.push(nid);
    }
    move_labels(&mut prj.pages[page_i], &old, &was, &path, &owners);
    for o in &old {
        splits.points.insert(o.id, (path.clone(), owners.clone()));
    }
    splits.lines.insert(w.id, owners[1..].to_vec());
    true
}

/// First and last place, as distances from `at` in direction `dir`, where `wires`
/// cross the endless line through `at`.
fn reach<'a>(wires: impl Iterator<Item = &'a Wire>, at: P, dir: P) -> Option<(f32, f32)> {
    let across = |q: P| (q - at).y * dir.x - (q - at).x * dir.y;
    let hits = wires.filter_map(|w| {
        let (from_a, from_b) = (across(w.a), across(w.b));
        let q = w.a + (w.b - w.a) * (from_a / (from_a - from_b));
        (from_a * from_b <= 0.0 && from_a != from_b).then(|| (q - at).x * dir.x + (q - at).y * dir.y)
    });
    hits.fold(None, |r, t| r.map_or(Some((t, t)), |(lo, hi): (f32, f32)| Some((lo.min(t), hi.max(t)))))
}

/// Ends of a line that crosses `wires` and nothing else with its direction kept: as
/// near to where it is as possible, at its old reach past the wires or else closer.
fn across_wires(page: &Page, h: &HeldLine, wires: &BTreeSet<u64>, split: &HashMap<u64, Vec<u64>>) -> Option<(P, P)> {
    const MAX_STEPS: i32 = 64;
    let owner = |id: u64| split.iter().find(|(_, added)| added.contains(&id)).map_or(id, |(wire, _)| *wire);
    let fits = |a: P, b: P| cabledef::crossed_wires(page, a, b).into_iter().map(owner).collect::<BTreeSet<_>>() == *wires;
    if fits(h.a, h.b) {
        return Some((h.a, h.b));
    }
    let dir = (h.b - h.a) * (1.0 / h.a.dist(h.b).max(TOUCH));
    let side = p(-dir.y, dir.x);
    let own: Vec<&Wire> = page.wires.iter().filter(|w| wires.contains(&owner(w.id))).collect();
    let steps = (0..=MAX_STEPS).flat_map(|n| [n, -n]);
    steps.map(|n| h.a + side * (n as f32 * GRID)).find_map(|at| {
        let met = wires.iter().all(|id| reach(own.iter().copied().filter(|w| owner(w.id) == *id), at, dir).is_some());
        let (first, last) = reach(own.iter().copied(), at, dir).filter(|_| met)?;
        let ends = |past: (f32, f32)| (at + dir * (first - past.0), at + dir * (last + past.1));
        let close = (h.past.0.min(GRID / 4.0), h.past.1.min(GRID / 4.0));
        [ends(h.past), ends(close)].into_iter().find(|(a, b)| fits(*a, *b))
    })
}

/// Keeps the cable definition lines of `before` across their wires; `split` holds the
/// wires added to a wire. A line that finds no such place stays where it was.
fn place_lines(page: &mut Page, before: &Before, split: &HashMap<u64, Vec<u64>>) {
    for h in &before.lines {
        let wires = h.wires.iter().copied().filter(|id| page.wires.iter().any(|w| w.id == *id)).collect();
        let (a, b) = across_wires(page, h, &wires, split).unwrap_or((h.a, h.b));
        if let Some(c) = page.cable_defs.iter_mut().find(|c| c.id == h.id) {
            (c.a, c.b) = (a, b);
        }
    }
}

/// Squares the wires that a move left diagonal; `before` holds the page as it was.
/// A wire keeps its direction: its far end slides along the wire at that end when
/// this connects nothing new (an open end, a corner, or a junction on a run of wires
/// such as a rail), otherwise the wire gets a jog or a route. One that cannot be
/// squared without touching other connections stays as it is. With rerouting on
/// ([`WireSettings::reroute`]) a wire that does not slide along a run, or meets a
/// corner, is routed anew with the corners after it ([`reroute`]). Connection
/// definition points and cable definition lines stay on their wires.
pub fn square_wires(prj: &mut Project, page_i: usize, before: &Before) {
    let symbols = prj.symbol_table();
    let area = crate::frame::of_page(prj, page_i).area();
    let rerouting = prj.wire_settings.reroute;
    let was = |id: u64| before.wires.iter().find(|o| o.id == id);
    let mut bent = Vec::new();
    for w in &prj.pages[page_i].wires {
        if let Some(o) = was(w.id).filter(|o| upright(o) != level(o) && !upright(w) && !level(w)) {
            bent.push((w.id, !w.a.same(o.a), upright(o)));
        }
    }
    let mut pending: BTreeSet<u64> = bent.iter().map(|b| b.0).collect();
    let all = pins(&symbols, &prj.pages[page_i]);
    let cx = Ctx { symbols: &symbols, area, all: &all, before };
    let mut splits = Splits::default();
    for (id, a_moved, vertical) in bent {
        pending.remove(&id);
        let page = &mut prj.pages[page_i];
        let Some(w) = page.wires.iter().find(|w| w.id == id).filter(|w| !upright(w) && !level(w)).cloned() else { continue };
        let (m, f) = if a_moved { (w.a, w.b) } else { (w.b, w.a) };
        let to = if vertical { p(m.x, f.y) } else { p(f.x, m.y) };
        let pin_at_f = all.iter().any(|(_, q, _)| q.same(f));
        let ends: Vec<u64> = all.iter().filter(|(_, q, _)| q.same(m) || q.same(f)).map(|x| x.0).collect();
        // The far end is a corner with the only other wire there, running across.
        let mut at_f = page.wires.iter().filter(|x| x.id != id && dist_to_segment(f, x.a, x.b) < TOUCH);
        let (first, second) = (at_f.next(), at_f.next());
        let open = !pin_at_f && first.is_none();
        let corner = match (first, second) {
            (Some(n), None) if !pin_at_f && (n.a.same(f) || n.b.same(f)) => {
                let o = was(n.id).unwrap_or(n);
                (if vertical { level(o) } else { upright(o) }).then_some(n.id)
            }
            _ => None,
        };
        let settled = |x: &Wire| x.id == id || pending.contains(&x.id);
        let leg = |page: &Page, a: P, b: P| unobstructed(&symbols, page, &all, a, b, &ends);
        let slides = match corner {
            Some(n) => without(page, |x| settled(x) || x.id == n, |page| leg(page, m, to) && leg(page, f, to) && free(page, &all, to)),
            None if open => without(page, settled, |page| leg(page, m, to) && free(page, &all, to)),
            None => !pin_at_f && without(page, settled, |page| leg(page, m, to) && on_run(page, f, to)),
        };
        if rerouting && (corner.is_some() || !slides) && reroute(prj, page_i, &cx, &w, (m, f), &pending, &mut splits) {
            continue;
        }
        let page = &mut prj.pages[page_i];
        if slides {
            for x in page.wires.iter_mut().filter(|x| x.id == id || Some(x.id) == corner) {
                if x.a.same(f) {
                    x.a = to;
                } else if x.b.same(f) {
                    x.b = to;
                }
            }
            continue;
        }
        let path = without(page, settled, |page| {
            jog(m, f, vertical, |path| path_clear(&symbols, page, &all, path, &ends)).or_else(|| search(&symbols, page, area, m, f, SQUARE))
        });
        let Some(path) = path else { continue };
        if let Some(x) = page.wires.iter_mut().find(|x| x.id == id) {
            (x.a, x.b) = (path[0], path[1]);
        }
        let mut added = Vec::new();
        for seg in path[1..].windows(2) {
            let nid = prj.new_id();
            prj.pages[page_i].wires.push(Wire { id: nid, a: seg[0], b: seg[1], labels:Vec::new(), ..w.clone() });
            added.push(nid);
        }
        let mut course = path;
        if !a_moved {
            course.reverse();
        }
        place_wire_labels(&mut prj.pages[page_i], &w, &course, &added);
        splits.points.insert(id, (course, std::iter::once(id).chain(added.iter().copied()).collect()));
        splits.lines.insert(id, added);
    }
    place_points(&mut prj.pages[page_i], before, &splits.points);
    ops::cleanup_wires(&mut prj.pages[page_i]);
    place_lines(&mut prj.pages[page_i], before, &splits.lines);
}

const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// Costs of a path on the grid: a step, a bend and crossing a wire.
#[derive(Clone, Copy)]
struct Costs {
    step: u32,
    bend: u32,
    jump: u32,
}

/// Routes of the wire tool and wires squared after a move: as few bends as possible.
const SQUARE: Costs = Costs { step: 1, bend: 4, jump: 0 };
/// Connections routed anew after a move, in tenths: the length in grid steps, 1.2 per
/// bend and 1.5 per wire crossed (a jump).
const REROUTE: Costs = Costs { step: 10, bend: 12, jump: 15 };

/// Orthogonal path on the grid from `a` to `b` inside `area` that avoids symbols,
/// other connection points, wire ends and running along other wires (crossing them
/// is allowed). Falls back to a single corner when there is no such path.
pub fn route(symbols: &Symbols, page: &Page, area: R, a: P, b: P) -> Vec<P> {
    search(symbols, page, area, a, b, SQUARE).unwrap_or_else(|| {
        if a.x == b.x || a.y == b.y {
            vec![a, b]
        } else {
            vec![a, p(b.x, a.y), b]
        }
    })
}

fn search(symbols: &Symbols, page: &Page, area: R, a: P, b: P, costs: Costs) -> Option<Vec<P>> {
    let cell = |q: P| ((q.x / GRID).round() as i32, (q.y / GRID).round() as i32);
    let at = |c: (i32, i32)| p(c.0 as f32 * GRID, c.1 as f32 * GRID);
    let (x0, y0) = cell(area.min);
    let (x1, y1) = cell(area.max);
    let (start, goal) = (cell(a), cell(b));
    if !at(start).same(a) || !at(goal).same(b) {
        return None;
    }
    let all = pins(symbols, page);
    let boxes: Vec<R> = page
        .symbols
        .iter()
        .filter_map(|s| symbols.get(&s.sym).filter(|d| d.func != Func::Graphic).map(|d| s.bounds(&d)))
        .collect();
    let blocked = |c: (i32, i32)| {
        if c == start || c == goal {
            return false;
        }
        let q = at(c);
        boxes.iter().any(|r| r.contains(q))
            || all.iter().any(|(_, o, _)| o.same(q))
            || page.wires.iter().any(|w| w.a.same(q) || w.b.same(q))
    };
    // Lies inside a horizontal / vertical wire.
    let on_wire = |c: (i32, i32), horizontal: bool| {
        let q = at(c);
        page.wires.iter().any(|w| (w.a.y == w.b.y) == horizontal && on_segment_interior(q, w.a, w.b))
    };
    let h = |c: (i32, i32)| ((c.0 - goal.0).unsigned_abs() + (c.1 - goal.1).unsigned_abs()) * costs.step;
    // State: cell and direction of arrival (4 = start).
    type State = ((i32, i32), usize);
    let mut dist: HashMap<State, u32> = HashMap::new();
    let mut prev: HashMap<State, State> = HashMap::new();
    let mut heap = BinaryHeap::new();
    dist.insert((start, 4), 0);
    heap.push(Reverse((h(start), 0u32, start, 4usize)));
    let mut found = None;
    while let Some(Reverse((_, g, c, dir))) = heap.pop() {
        if dist.get(&(c, dir)).is_some_and(|&d| d < g) {
            continue;
        }
        if c == goal {
            found = Some((c, dir));
            break;
        }
        let crossing_here = c != start && (on_wire(c, true) || on_wire(c, false));
        for (k, (dx, dy)) in DIRS.iter().enumerate() {
            let bend = dir != 4 && dir != k;
            if bend && crossing_here {
                continue;
            }
            if dir != 4 && DIRS[dir] == (-dx, -dy) {
                continue;
            }
            let n = (c.0 + dx, c.1 + dy);
            if n.0 < x0 || n.0 > x1 || n.1 < y0 || n.1 > y1 || blocked(n) {
                continue;
            }
            if on_wire(n, *dy == 0) {
                continue;
            }
            let jump = costs.jump > 0 && on_wire(n, *dy != 0);
            let ng = g + costs.step + if bend { costs.bend } else { 0 } + if jump { costs.jump } else { 0 };
            if dist.get(&(n, k)).is_none_or(|&d| ng < d) {
                dist.insert((n, k), ng);
                prev.insert((n, k), (c, dir));
                heap.push(Reverse((ng + h(n), ng, n, k)));
            }
        }
    }
    let mut s = found?;
    let mut cells = vec![s.0];
    while let Some(&q) = prev.get(&s) {
        cells.push(q.0);
        s = q;
    }
    cells.reverse();
    // Keep the corners only.
    let mut pts: Vec<P> = Vec::new();
    for (i, c) in cells.iter().enumerate() {
        let corner = i == 0 || i + 1 == cells.len() || {
            let (a, b) = (cells[i - 1], cells[i + 1]);
            (c.0 - a.0, c.1 - a.1) != (b.0 - c.0, b.1 - c.1)
        };
        if corner {
            pts.push(at(*c));
        }
    }
    Some(pts)
}

/// Joins collinear wires that meet end to end where nothing else connects. Returns
/// the number of wires removed.
pub fn merge_collinear(symbols: &Symbols, page: &mut Page) -> usize {
    let mut removed = 0;
    loop {
        let all = pins(symbols, page);
        let mut merged = None;
        'outer: for i in 0..page.wires.len() {
            for j in i + 1..page.wires.len() {
                let (w, v) = (&page.wires[i], &page.wires[j]);
                let shared = [(w.a, w.b, v.a, v.b), (w.a, w.b, v.b, v.a), (w.b, w.a, v.a, v.b), (w.b, w.a, v.b, v.a)]
                    .into_iter()
                    .find(|(m, _, n, _)| m.same(*n));
                let Some((m, far_w, _, far_v)) = shared else { continue };
                let collinear = (far_w.x == m.x && far_v.x == m.x) || (far_w.y == m.y && far_v.y == m.y);
                let opposite = (far_w - m).x * (far_v - m).x + (far_w - m).y * (far_v - m).y < 0.0;
                let alone = page.wires.iter().filter(|x| x.a.same(m) || x.b.same(m) || on_segment_interior(m, x.a, x.b)).count() == 2
                    && !all.iter().any(|(_, q, _)| q.same(m));
                let same_props = w.same_properties(v);
                if collinear && opposite && alone && same_props {
                    merged = Some((i, j, far_w, far_v));
                    break 'outer;
                }
            }
        }
        let Some((i, j, a, b)) = merged else { break };
        let mut labels = Vec::new();
        for wire in [&page.wires[i], &page.wires[j]] {
            for mut label in wire.labels.clone() {
                label.rebase(wire, a, b);
                if !labels.contains(&label) {labels.push(label);}
            }
        }
        page.wires[i].a = a;
        page.wires[i].b = b;
        page.wires[i].labels = labels;
        page.wires.remove(j);
        removed += 1;
    }
    removed
}

#[cfg(test)]
mod tests;
