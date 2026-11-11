//! Wire jumps and connection dots: where a horizontal and a vertical wire cross
//! without connecting, one of them jumps over the other with an arc, a bridge or a
//! gap; the dots of T-connections follow the style of the project.

use crate::geom::{on_segment_interior, p, P};
use crate::model::{JumpStyle, JumpWire, Page, TeeStyle, Wire};
use crate::netlist;
use std::collections::{HashMap, HashSet};

/// Half the width of a jump, and the height of an arc or a bridge (mm).
pub const JUMP_R: f32 = 1.0;
/// Radius of a connection dot (mm).
pub const DOT_R: f32 = 0.7;
/// Radius of a large connection dot (mm).
pub const LARGE_DOT_R: f32 = 1.1;
/// Pieces of the semicircle of an arc.
const ARC_STEPS: usize = 8;

fn level(w: &Wire) -> bool {
    (w.a.y - w.b.y).abs() < 0.005 && (w.a.x - w.b.x).abs() >= 0.005
}

fn upright(w: &Wire) -> bool {
    (w.a.x - w.b.x).abs() < 0.005 && (w.a.y - w.b.y).abs() >= 0.005
}

/// Points where a horizontal and a vertical wire cross inside both without a
/// connection: no wire ends there, no connection symbol and none of the `connected`
/// points. Returns (point, horizontal wire, vertical wire).
pub fn crossings(page: &Page, connected: &[P]) -> Vec<(P, u64, u64)> {
    let mut taken: HashSet<(i64, i64)> = page.wires.iter().flat_map(|w| [w.a.key(), w.b.key()]).collect();
    taken.extend(page.junctions.iter().map(|j| j.pos.key()));
    taken.extend(connected.iter().map(|q| q.key()));
    let uprights: Vec<&Wire> = page.wires.iter().filter(|w| upright(w)).collect();
    let mut out = Vec::new();
    for h in page.wires.iter().filter(|w| level(w)) {
        for v in &uprights {
            let q = p(v.a.x, h.a.y);
            if on_segment_interior(q, h.a, h.b) && on_segment_interior(q, v.a, v.b) && !taken.contains(&q.key()) {
                out.push((q, h.id, v.id));
            }
        }
    }
    out
}

/// The points each jumping wire jumps at (wire id → crossing points).
pub fn jumps(page: &Page, connected: &[P], style: JumpStyle, wire: JumpWire) -> HashMap<u64, Vec<P>> {
    let mut out: HashMap<u64, Vec<P>> = HashMap::new();
    if style == JumpStyle::None {
        return out;
    }
    for (q, h, v) in crossings(page, connected) {
        out.entry(if wire == JumpWire::Horizontal { h } else { v }).or_default().push(q);
    }
    out
}

/// The strokes of a wire as drawn: one line, or broken and bent where it jumps over
/// the wires at `at`. A horizontal wire jumps upwards, a vertical one to the right;
/// jumps that overlap become one.
pub fn strokes(w: &Wire, at: &[P], style: JumpStyle) -> Vec<Vec<P>> {
    let len = w.a.dist(w.b);
    if style == JumpStyle::None || at.is_empty() || len <= 0.0 {
        return vec![vec![w.a, w.b]];
    }
    let dir = (w.b - w.a) * (1.0 / len);
    let side = if dir.x.abs() >= dir.y.abs() { p(0.0, -1.0) } else { p(1.0, 0.0) };
    let mut spans: Vec<(f32, f32)> = at
        .iter()
        .map(|q| (*q - w.a).x * dir.x + (*q - w.a).y * dir.y)
        .map(|t| ((t - JUMP_R).max(0.0), (t + JUMP_R).min(len)))
        .collect();
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut merged: Vec<(f32, f32)> = Vec::new();
    for (s, e) in spans {
        match merged.last_mut() {
            Some(last) if s <= last.1 => last.1 = last.1.max(e),
            _ => merged.push((s, e)),
        }
    }
    let along = |t: f32| w.a + dir * t;
    let mut out = Vec::new();
    let mut cur = vec![w.a];
    for (s, e) in merged {
        cur.push(along(s));
        match style {
            JumpStyle::Arc => {
                let (c, r) = ((s + e) / 2.0, (e - s) / 2.0);
                for i in 1..ARC_STEPS {
                    let a = std::f32::consts::PI * i as f32 / ARC_STEPS as f32;
                    cur.push(along(c - r * a.cos()) + side * (r * a.sin()));
                }
                cur.push(along(e));
            }
            JumpStyle::Bridge => {
                cur.push(along(s) + side * JUMP_R);
                cur.push(along(e) + side * JUMP_R);
                cur.push(along(e));
            }
            JumpStyle::Gap | JumpStyle::None => {
                out.push(std::mem::take(&mut cur));
                cur.push(along(e));
            }
        }
    }
    cur.push(w.b);
    out.push(cur);
    out.retain(|s| s.windows(2).any(|x| !x[0].same(x[1])));
    out
}

/// Radius of the dot at the connection point `q`, None for no dot. A T-connection
/// (up to three wire directions) follows `style`; four or more directions always get
/// a dot, so that they differ from a crossing.
pub fn dot_radius(page: &Page, q: P, style: TeeStyle) -> Option<f32> {
    let large = style == TeeStyle::LargeDot;
    let tee = netlist::junctions::arms(page, q).is_some_and(|a| a.len() <= 3);
    match style {
        TeeStyle::None if tee => None,
        _ if large => Some(LARGE_DOT_R),
        _ => Some(DOT_R),
    }
}

#[cfg(test)]
mod tests;
