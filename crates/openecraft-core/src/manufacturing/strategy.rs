//! Machining strategies: how the holes and openings of a face are made on a machine
//! (drilling, tapping, circular and contour milling, punching with a matching tool,
//! nibbling with a smaller one), the order of the operations (by tool, shortest path)
//! and the time estimate.

use super::machines::{Machine, MachineKind, Tool, ToolKind};
use crate::geom::{p, P};
use crate::model::Project;
use crate::panel::{Face, Fastening, ItemKind, PLATE_THICKNESS};
use crate::production::{holes_on, openings};
use openecraft_i18n::{tr, tr_fmt};
pub use crate::model::manufacturing::{Method, OpKind, Operation, Strategy};

impl Method {
    pub fn key(self) -> &'static str {
        match self {
            Method::Auto => "auto",
            Method::Contour => "contour",
        }
    }

    pub fn from_key(s: &str) -> Option<Method> {
        [Method::Auto, Method::Contour].into_iter().find(|m| m.key() == s.trim())
    }
}

impl Default for Strategy {
    fn default() -> Self {
        Strategy { cutouts: Method::Auto, tap: true, drill_max: 13.0, tolerance: 0.1, nibble_overlap: 0.2, optimize: true }
    }
}

impl OpKind {
    /// Machine readable name (CSV lists, feedback).
    pub fn key(&self) -> &'static str {
        match self {
            OpKind::Drill { .. } => "drill",
            OpKind::Tap { .. } => "tap",
            OpKind::Punch { .. } => "punch",
            OpKind::Nibble { .. } => "nibble",
            OpKind::MillCircle { .. } => "mill_circle",
            OpKind::MillRect { .. } => "mill_rect",
            OpKind::Stud { .. } => "stud",
            OpKind::Manual { .. } => "manual",
        }
    }

    /// Hits of drills, taps and punches (time estimate).
    fn hits(&self) -> usize {
        match self {
            OpKind::Drill { .. } | OpKind::Tap { .. } | OpKind::Punch { .. } => 1,
            OpKind::Nibble { hits } => hits.len(),
            _ => 0,
        }
    }
}

/// Machining of one face on one machine.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub enclosure: String,
    pub face: Face,
    pub machine: String,
    pub width: f32,
    pub height: f32,
    pub thickness: f32,
    pub ops: Vec<Operation>,
    /// Rapid travel between the operations, mm.
    pub travel: f32,
    /// Milling path, mm.
    pub milled: f32,
    pub tool_changes: usize,
    /// Estimated machine time, s.
    pub time: f32,
    pub problems: Vec<String>,
}

impl Plan {
    /// Number of operations of a kind.
    pub fn count(&self, key: &str) -> usize {
        self.ops.iter().filter(|o| o.kind.key() == key).count()
    }

    /// Operations made on the machine (no studs, nothing manual).
    pub fn machined(&self) -> usize {
        self.ops.iter().filter(|o| o.tool.is_some()).count()
    }
}

/// Sheet thickness of a face: the mounting plate or the enclosure's walls.
pub fn thickness(e: &crate::panel::Enclosure, face: Face) -> f32 {
    if face == Face::Plate { PLATE_THICKNESS } else { e.wall() }
}

/// Hits of a tool of size `a` along the contour of a w × h rectangle (inside).
fn rect_hits(c: P, w: f32, h: f32, a: f32, pitch: f32) -> Vec<P> {
    let (x0, y0, x1, y1) = (c.x - w / 2.0 + a / 2.0, c.y - h / 2.0 + a / 2.0, c.x + w / 2.0 - a / 2.0, c.y + h / 2.0 - a / 2.0);
    let corners = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    let mut v = Vec::new();
    for i in 0..4 {
        let (s, t) = (corners[i], corners[(i + 1) % 4]);
        let len = s.dist(t);
        let n = (len / pitch).ceil().max(if len > 0.01 { 1.0 } else { 0.0 }) as usize;
        for k in 0..n {
            let f = k as f32 / n as f32;
            v.push(p(s.x + (t.x - s.x) * f, s.y + (t.y - s.y) * f));
        }
        if n == 0 && i == 0 {
            v.push(s);
        }
    }
    v
}

/// Hits of a round tool of size `a` along the inside of a circle of diameter `d`.
fn circle_hits(c: P, d: f32, a: f32, pitch: f32) -> Vec<P> {
    let r = (d - a) / 2.0;
    let n = ((std::f32::consts::TAU * r / pitch).ceil() as usize).max(3);
    (0..n).map(|k| {
        let t = std::f32::consts::TAU * k as f32 / n as f32;
        p(c.x + r * t.cos(), c.y + r * t.sin())
    }).collect()
}

struct Builder<'a> {
    m: &'a Machine,
    s: &'a Strategy,
    ops: Vec<Operation>,
    problems: Vec<String>,
}

impl Builder<'_> {
    fn push(&mut self, item: u64, tool: Option<&Tool>, pos: P, kind: OpKind) {
        self.ops.push(Operation { seq: 0, item, tool: tool.map(|t| t.number), pos, kind });
    }

    fn manual(&mut self, item: u64, pos: P, what: String, name: &str) {
        self.problems.push(tr_fmt!("{}: {} cannot be made on {}", name, what, self.m.name));
        self.push(item, None, pos, OpKind::Manual { what });
    }

    fn pitch(&self, a: f32) -> f32 {
        (a * (1.0 - self.s.nibble_overlap.clamp(0.0, 0.9))).max(0.5)
    }

    /// A round hole or opening of diameter `d`.
    fn round(&mut self, item: u64, c: P, d: f32, name: &str, opening: bool) {
        let tol = self.s.tolerance;
        let contour = opening && self.s.cutouts == Method::Contour;
        match self.m.kind {
            MachineKind::Punching => {
                if let Some(t) = self.m.tool(ToolKind::PunchRound, d, tol).filter(|_| !contour).cloned() {
                    self.push(item, Some(&t), c, OpKind::Punch { turned: false });
                } else if let Some(t) = self.m.largest(ToolKind::PunchRound, d * 0.5).cloned() {
                    let hits = circle_hits(c, d, t.diameter, self.pitch(t.diameter));
                    self.push(item, Some(&t), c, OpKind::Nibble { hits });
                } else {
                    self.manual(item, c, format!("Ø{}", super::num(d)), name);
                }
            }
            _ => {
                let drill = self.m.tool(ToolKind::Drill, d, tol).filter(|_| d <= self.s.drill_max + 1e-4 && !contour).cloned();
                if let Some(t) = drill {
                    self.push(item, Some(&t), c, OpKind::Drill { diameter: t.diameter });
                } else if let Some(t) = self.m.largest(ToolKind::Mill, d - 0.5).cloned() {
                    self.push(item, Some(&t), c, OpKind::MillCircle { diameter: d });
                } else {
                    self.manual(item, c, format!("Ø{}", super::num(d)), name);
                }
            }
        }
    }

    fn rect(&mut self, item: u64, c: P, w: f32, h: f32, name: &str) {
        let what = || format!("{} x {}", super::num(w), super::num(h));
        match self.m.kind {
            MachineKind::Punching => {
                let exact = self.m.tools.iter().find(|t| t.fits_rect(w, h)).filter(|_| self.s.cutouts == Method::Auto).cloned();
                if let Some(t) = exact {
                    let turned = (t.diameter - w).abs() >= 0.05;
                    self.push(item, Some(&t), c, OpKind::Punch { turned });
                    return;
                }
                // The largest square punch that fits, else a round one.
                let square = self.m.tools.iter().filter(|t| t.kind == ToolKind::PunchRect && (t.diameter - t.height).abs() < 0.05 && t.diameter <= w.min(h) * 0.5 + 1e-4).max_by(|a, b| a.diameter.total_cmp(&b.diameter));
                let tool = square.or_else(|| self.m.largest(ToolKind::PunchRound, w.min(h) * 0.5)).cloned();
                match tool {
                    Some(t) => {
                        let hits = rect_hits(c, w, h, t.diameter, self.pitch(t.diameter));
                        self.push(item, Some(&t), c, OpKind::Nibble { hits });
                    }
                    None => self.manual(item, c, what(), name),
                }
            }
            _ => match self.m.largest(ToolKind::Mill, w.min(h) - 0.5).cloned() {
                Some(t) => self.push(item, Some(&t), c, OpKind::MillRect { w, h }),
                None => self.manual(item, c, what(), name),
            },
        }
    }
}

/// Plans the machining of a face of an enclosure on a machine.
pub fn plan(prj: &Project, enc: usize, face: Face, m: &Machine, s: &Strategy) -> Plan {
    let e = &prj.layout.enclosures[enc];
    let (fw, fh) = e.face_size(face);
    let t = thickness(e, face);
    let name = |id: u64| e.item(id).map(|i| if i.kind == ItemKind::Device { i.tag.clone() } else { i.kind.title().to_string() }).unwrap_or_default();
    let mut b = Builder { m, s, ops: Vec::new(), problems: Vec::new() };
    if m.kind == MachineKind::Busbar {
        b.problems.push(tr_fmt!("{} is a busbar machine and makes no sheets", m.name));
    }
    let fits = |a: f32, b: f32| (fw <= a + 0.01 && fh <= b + 0.01) || (fw <= b + 0.01 && fh <= a + 0.01);
    if !fits(m.area.0, m.area.1) {
        b.problems.push(tr_fmt!("{}: the {} ({} x {} mm) is larger than the working area of {}", e.name, face.title(), super::num(fw), super::num(fh), m.name));
    }
    if t > m.max_thickness + 1e-4 {
        b.problems.push(tr_fmt!("{}: the {} is {} mm thick, {} takes at most {} mm", e.name, face.title(), super::num(t), m.name, super::num(m.max_thickness)));
    }
    let holes = holes_on(e, &prj.parts, face);
    let mut taps = Vec::new();
    for h in &holes {
        let n = name(h.item);
        if h.fastening == Fastening::Stud {
            b.push(h.item, None, h.pos, OpKind::Stud { thread: h.thread_size.clone() });
            continue;
        }
        b.round(h.item, h.pos, h.diameter, &n, false);
        if !h.thread.is_empty() {
            taps.push((h.item, h.pos, h.thread.clone(), n));
        }
    }
    let mut untapped = 0;
    for (item, pos, thread, _) in taps {
        let size: f32 = thread.trim_start_matches('M').parse().unwrap_or(0.0);
        match m.tool(ToolKind::Tap, size, 0.01).filter(|_| s.tap).cloned() {
            Some(tool) => b.push(item, Some(&tool), pos, OpKind::Tap { thread }),
            None => untapped += 1,
        }
    }
    if untapped > 0 {
        b.problems.push(tr_fmt!("{}: {} threads on the {} are cut by hand", e.name, untapped, face.title()));
    }
    for o in openings(e, &prj.parts, face) {
        let n = name(o.item);
        if o.round {
            b.round(o.item, o.center, o.w, &n, true);
        } else if o.angle != 0.0 {
            // Tools and contours run along the axes of the sheet.
            b.manual(o.item, o.center, format!("{} x {} {}°", super::num(o.w), super::num(o.h), super::num(o.angle)), &n);
        } else {
            b.rect(o.item, o.center, o.w, o.h, &n);
        }
    }
    let Builder { ops, problems, .. } = b;
    let mut out = Plan { enclosure: e.name.clone(), face, machine: m.id.clone(), width: fw, height: fh, thickness: t, ops: order(ops, m, s.optimize), travel: 0.0, milled: 0.0, tool_changes: 0, time: 0.0, problems };
    estimate(&mut out, m);
    out
}

/// Rank of an operation in the program: drilling and punching before tapping, milling
/// last; manual work and studs after the machine.
fn rank(o: &Operation, m: &Machine) -> (u8, u32) {
    let kind = |n| m.tool_by_number(n).map(|t| t.kind);
    match (&o.kind, o.tool) {
        (OpKind::Drill { .. } | OpKind::Punch { .. } | OpKind::Nibble { .. }, Some(n)) => (if kind(n) == Some(ToolKind::PunchRect) { 1 } else { 0 }, n),
        (OpKind::Tap { .. }, Some(n)) => (2, n),
        (OpKind::MillCircle { .. } | OpKind::MillRect { .. }, Some(n)) => (3, n),
        (OpKind::Stud { .. }, _) => (4, 0),
        _ => (5, 0),
    }
}

/// Groups the operations by tool (drills and punches by size) and orders each group
/// by the shortest path from the end of the previous group.
pub fn order(mut ops: Vec<Operation>, m: &Machine, optimize: bool) -> Vec<Operation> {
    let size = |o: &Operation| o.tool.and_then(|n| m.tool_by_number(n)).map(|t| t.diameter).unwrap_or(0.0);
    ops.sort_by(|a, b| rank(a, m).0.cmp(&rank(b, m).0).then(size(a).total_cmp(&size(b))).then(rank(a, m).1.cmp(&rank(b, m).1)));
    let mut out: Vec<Operation> = Vec::with_capacity(ops.len());
    let mut at = p(0.0, 0.0);
    let mut i = 0;
    while i < ops.len() {
        let key = (rank(&ops[i], m), ops[i].tool);
        let j = ops[i..].iter().position(|o| (rank(o, m), o.tool) != key).map(|k| i + k).unwrap_or(ops.len());
        let mut group: Vec<Operation> = ops[i..j].to_vec();
        let mut path = Vec::with_capacity(group.len());
        while !group.is_empty() {
            let k = (0..group.len()).min_by(|&a, &b| group[a].pos.dist(at).total_cmp(&group[b].pos.dist(at))).unwrap();
            let o = group.swap_remove(k);
            at = o.pos;
            path.push(o);
        }
        if optimize {
            two_opt(&mut path, out.last().map(|o| o.pos).unwrap_or(p(0.0, 0.0)));
            at = path.last().map(|o| o.pos).unwrap_or(at);
        }
        out.extend(path);
        i = j;
    }
    for (k, o) in out.iter_mut().enumerate() {
        o.seq = k + 1;
    }
    out
}

/// 2-opt improvement of an open path starting at `start`.
fn two_opt(path: &mut [Operation], start: P) {
    let n = path.len();
    if !(3..=400).contains(&n) {
        return;
    }
    let pos = |path: &[Operation], i: usize| if i == 0 { start } else { path[i - 1].pos };
    let mut improved = true;
    let mut rounds = 0;
    while improved && rounds < 50 {
        improved = false;
        rounds += 1;
        for i in 0..n - 1 {
            for j in i + 1..n {
                let (a, b) = (pos(path, i), path[i].pos);
                let (c, next) = (path[j].pos, path.get(j + 1).map(|o| o.pos));
                let before = a.dist(b) + next.map(|d| c.dist(d)).unwrap_or(0.0);
                let after = a.dist(c) + next.map(|d| b.dist(d)).unwrap_or(0.0);
                if after + 1e-3 < before {
                    path[i..=j].reverse();
                    improved = true;
                }
            }
        }
    }
}

/// Rapid travel, milling path, tool changes and time of a plan.
pub fn estimate(plan: &mut Plan, m: &Machine) {
    let (mut travel, mut milled, mut changes, mut hits) = (0.0, 0.0, 0, 0);
    let mut at = p(0.0, 0.0);
    let mut tool = None;
    for o in plan.ops.iter().filter(|o| o.tool.is_some()) {
        if o.tool != tool {
            changes += 1;
            tool = o.tool;
        }
        let first = match &o.kind {
            OpKind::Nibble { hits } => hits.first().copied().unwrap_or(o.pos),
            _ => o.pos,
        };
        travel += at.dist(first);
        at = first;
        hits += o.kind.hits();
        match &o.kind {
            OpKind::Nibble { hits } => {
                for w in hits.windows(2) {
                    travel += w[0].dist(w[1]);
                }
                at = hits.last().copied().unwrap_or(at);
            }
            OpKind::MillCircle { diameter } => {
                let r = m.tool_by_number(o.tool.unwrap()).map(|t| (diameter - t.diameter) / 2.0).unwrap_or(0.0);
                milled += std::f32::consts::TAU * r.max(0.0);
            }
            OpKind::MillRect { w, h } => {
                let d = m.tool_by_number(o.tool.unwrap()).map(|t| t.diameter).unwrap_or(0.0);
                milled += 2.0 * ((w - d).max(0.0) + (h - d).max(0.0));
            }
            _ => {}
        }
    }
    travel += at.dist(p(0.0, 0.0));
    plan.travel = travel;
    plan.milled = milled;
    plan.tool_changes = changes;
    let secs = travel / m.rapid.max(1.0) * 60.0 + milled / m.feed.max(1.0) * 60.0 + hits as f32 * m.hit_time + changes as f32 * m.tool_change;
    plan.time = secs * m.time_factor.max(0.01);
}

/// Machining time as text (`12 min 5 s`).
pub fn duration(secs: f32) -> String {
    let s = secs.round().max(0.0) as u64;
    if s >= 60 { tr_fmt!("{} min {} s", s / 60, s % 60) } else { tr_fmt!("{} s", s) }
}

/// Title of an operation kind (reports).
pub fn op_title(kind: &OpKind) -> &'static str {
    match kind {
        OpKind::Drill { .. } => tr("Drilling"),
        OpKind::Tap { .. } => tr("Tapping"),
        OpKind::Punch { .. } => tr("Punching"),
        OpKind::Nibble { .. } => tr("Nibbling"),
        OpKind::MillCircle { .. } => tr("Circular milling"),
        OpKind::MillRect { .. } => tr("Contour milling"),
        OpKind::Stud { .. } => tr("Weld stud"),
        OpKind::Manual { .. } => tr("Manual work"),
    }
}

#[cfg(test)]
mod tests;
