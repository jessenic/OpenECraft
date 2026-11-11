//! Placement of the connection point designations of a placed symbol: on the side
//! chosen by hand, or the first side of the connection point that is clear of the
//! symbol's graphics, wires, other symbols and the other texts of the symbol.

use super::{prop_bounds, tag_block_bounds, text_width, SMALL_SIZE};
use crate::geom::{p, P, R};
use crate::index::Index;
use crate::model::{LabelSide, Project, SymbolInst, VAnchor};
use crate::symbols::{Anchor, Prim, SymbolDef};

/// Clearance kept between a designation and lines around it.
const CLEARANCE: f32 = 0.15;

/// A connection point designation as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct PinLabelPlace {
    pub pin: usize,
    pub pos: P,
    pub anchor: Anchor,
    pub v: VAnchor,
    /// Side used, in the symbol's own orientation (before rotation and mirroring).
    pub side: LabelSide,
    /// Shift from the usual position of the side, in the symbol's own orientation
    /// (the offset placed by hand, or the one found by automatic placement).
    pub shift: P,
    /// Estimated area of the text (picking and placement).
    pub bounds: R,
}

/// Connection point `i` on the page, whether its lead runs vertically and the sign
/// of the direction into the symbol along the lead: along the line of the symbol
/// ending there, else away from the centre of the symbol.
fn lead(def: &SymbolDef, s: &SymbolInst, i: usize, center: P) -> (P, bool, f32) {
    let q = def.pin_world(i, s.pos, s.rot, s.mirror);
    let at = def.pins[i].pos;
    let line = def.prims.iter().find_map(|prim| match prim {
        Prim::Line { a, b, .. } if a.same(at) && !b.same(at) => Some(*b - *a),
        Prim::Line { a, b, .. } if b.same(at) && !a.same(at) => Some(*a - *b),
        _ => None,
    });
    let d = match line {
        Some(inward) => inward.transform(s.rot, s.mirror) * -1.0,
        None => q - center,
    };
    if d.y.abs() >= d.x.abs() { (q, true, -d.y.signum()) } else { (q, false, -d.x.signum()) }
}

/// Distance of a designation beside the lead from the connection point.
const DEPTH: f32 = 1.1;

/// Text position and alignment of a designation on side `side` (page directions) of
/// connection point `q`. Beside the lead the text sits [`DEPTH`] inside the symbol;
/// along the lead it sits beside the line, before or behind the connection point.
fn at_side(q: P, vertical: bool, inward: f32, side: LabelSide) -> (P, Anchor, VAnchor) {
    match (vertical, side) {
        (true, LabelSide::Left) => (p(q.x - 0.6, q.y + inward * 1.1), Anchor::Right, VAnchor::Middle),
        (true, LabelSide::Right) => (p(q.x + 0.6, q.y + inward * 1.1), Anchor::Left, VAnchor::Middle),
        (true, LabelSide::Above) => (p(q.x - 0.6, q.y - 0.4), Anchor::Right, VAnchor::Bottom),
        (true, LabelSide::Below) => (p(q.x - 0.6, q.y + 0.4), Anchor::Right, VAnchor::Top),
        (false, LabelSide::Above) => (p(q.x + inward * 1.1, q.y - 0.5), Anchor::Center, VAnchor::Bottom),
        (false, LabelSide::Below) => (p(q.x + inward * 1.1, q.y + 0.5), Anchor::Center, VAnchor::Top),
        (false, LabelSide::Left) => (p(q.x - 0.6, q.y - 0.5), Anchor::Right, VAnchor::Bottom),
        (false, LabelSide::Right) => (p(q.x + 0.6, q.y - 0.5), Anchor::Left, VAnchor::Bottom),
    }
}

/// Places tried in turn for automatic placement (page sides and distances inside
/// the symbol): beside the lead (left of vertical, above horizontal leads), moved
/// towards the connection point on short leads, then the other side of the lead,
/// then outside the symbol beside the connected wire.
fn candidates(vertical: bool, inward: f32) -> Vec<(LabelSide, f32)> {
    let (first, second, outside) = match (vertical, inward > 0.0) {
        (true, true) => (LabelSide::Left, LabelSide::Right, LabelSide::Above),
        (true, false) => (LabelSide::Left, LabelSide::Right, LabelSide::Below),
        (false, true) => (LabelSide::Above, LabelSide::Below, LabelSide::Left),
        (false, false) => (LabelSide::Above, LabelSide::Below, LabelSide::Right),
    };
    let depths = [DEPTH, 0.75, 0.4, 0.0];
    depths.iter().map(|d| (first, *d)).chain(depths.iter().map(|d| (second, *d))).chain([(outside, DEPTH)]).collect()
}

/// Estimated area of a text of height `size`.
pub fn text_box(pos: P, text: &str, size: f32, anchor: Anchor, v: VAnchor) -> R {
    let w = text_width(text, size);
    let x0 = match anchor {
        Anchor::Left => pos.x,
        Anchor::Center => pos.x - w / 2.0,
        Anchor::Right => pos.x - w,
    };
    let (y0, y1) = match v {
        VAnchor::Top => (pos.y, pos.y + size * 0.8),
        VAnchor::Middle => (pos.y - size * 0.4, pos.y + size * 0.4),
        VAnchor::Bottom => (pos.y - size * 0.8, pos.y),
    };
    R::from_points(p(x0, y0), p(x0 + w, y1))
}

/// What a designation should keep clear of.
#[derive(Default)]
struct Obstacles {
    segments: Vec<(P, P)>,
    circles: Vec<(P, f32, bool)>,
    boxes: Vec<R>,
}

impl Obstacles {
    /// Graphics of the symbol, wires and other symbols near it, its tag block and
    /// placed properties.
    fn around(prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> Obstacles {
        let mut o = Obstacles::default();
        let w = |q: P| s.pos + q.transform(s.rot, s.mirror);
        for prim in &def.prims {
            match prim {
                Prim::Line { a, b, .. } => o.segments.push((w(*a), w(*b))),
                Prim::Poly { pts, closed, .. } | Prim::Path { pts, closed, .. } => {
                    o.segments.extend(pts.windows(2).map(|ab| (w(ab[0]), w(ab[1]))));
                    if *closed && pts.len() > 2 {
                        o.segments.push((w(pts[pts.len() - 1]), w(pts[0])));
                    }
                }
                Prim::Circle { c, r, fill, .. } => o.circles.push((w(*c), *r, *fill)),
                Prim::Text { pos, text, size, .. } => o.boxes.push(text_box(w(*pos), text, *size, Anchor::Center, VAnchor::Middle)),
                Prim::Image { corners, .. } => {
                    let mut r = R::nothing();
                    corners.iter().for_each(|c| r.extend(w(*c)));
                    o.boxes.push(r);
                }
                Prim::Field { .. } | Prim::PropertyBlock(_) => {}
            }
        }
        let bounds = s.bounds(def);
        let near = bounds.expand(8.0);
        let Some(page) = prj.pages.get(page_i) else { return o };
        for wire in &page.wires {
            if R::from_points(wire.a, wire.b).expand(0.01).overlaps(&near) {
                o.segments.push((wire.a, wire.b));
            }
        }
        let symbols = prj.symbol_table();
        for other in page.symbols.iter().filter(|x| x.id != s.id && (x.pos.x - s.pos.x).abs() < 60.0 && (x.pos.y - s.pos.y).abs() < 60.0) {
            if let Some(od) = symbols.get(&other.sym) {
                let ob = other.bounds(&od);
                if ob.overlaps(&near) {
                    o.boxes.push(ob);
                }
            }
        }
        o.boxes.extend(tag_block_bounds(prj, idx, page_i, def, s));
        o.boxes.extend(s.props.iter().map(|pp| prop_bounds(prj, page_i, s, pp)));
        o
    }

    /// Number of obstacles in the way of a text in `r`.
    fn hits(&self, r: &R) -> usize {
        let r = r.expand(CLEARANCE);
        let segments = self.segments.iter().filter(|(a, b)| r.crosses(*a, *b)).count();
        let circles = self
            .circles
            .iter()
            .filter(|(c, rad, fill)| {
                let near = p(c.x.clamp(r.min.x, r.max.x), c.y.clamp(r.min.y, r.max.y)).dist(*c);
                let far = [r.min, r.max, p(r.min.x, r.max.y), p(r.max.x, r.min.y)].iter().map(|q| q.dist(*c)).fold(0.0, f32::max);
                near <= *rad && (*fill || far >= *rad)
            })
            .count();
        segments + circles + self.boxes.iter().filter(|b| b.overlaps(&r)).count()
    }
}

/// Where the connection point designations of a placed symbol are drawn: one entry
/// per pin with a designation, in pin order.
pub fn pin_label_places(prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> Vec<PinLabelPlace> {
    let mut out: Vec<PinLabelPlace> = Vec::new();
    if !def.show_pin_labels || s.pins.iter().take(def.pins.len()).all(|l| l.is_empty()) {
        return out;
    }
    let center = s.bounds(def).center();
    let mut obstacles: Option<Obstacles> = None;
    for (i, text) in s.pins.iter().enumerate().take(def.pins.len()) {
        if text.is_empty() {
            continue;
        }
        let (q, vertical, inward) = lead(def, s, i, center);
        let hand = s.pin_placement(i);
        // Page shift of a place `depth` inside instead of the usual one.
        let deeper = |side: LabelSide, depth: f32| {
            let beside = if vertical { side.dir().y == 0.0 } else { side.dir().x == 0.0 };
            let d = if beside { (depth - DEPTH) * inward } else { 0.0 };
            if vertical { p(0.0, d) } else { p(d, 0.0) }
        };
        let place = |side: LabelSide, shift: P| {
            let (pos, anchor, v) = at_side(q, vertical, inward, side);
            let pos = pos + shift;
            (pos, anchor, v, text_box(pos, text, SMALL_SIZE, anchor, v))
        };
        let (side, shift) = match hand.side {
            Some(side) => (side.on_page(s.rot, s.mirror), hand.offset.transform(s.rot, s.mirror)),
            None => {
                let o = obstacles.get_or_insert_with(|| Obstacles::around(prj, idx, page_i, def, s));
                let mut best: Option<(usize, LabelSide, P)> = None;
                for (side, depth) in candidates(vertical, inward) {
                    let shift = deeper(side, depth) + hand.offset.transform(s.rot, s.mirror);
                    let r = place(side, shift).3;
                    let n = o.hits(&r) + out.iter().filter(|l| l.bounds.overlaps(&r)).count();
                    if best.is_none_or(|(m, ..)| n < m) {
                        best = Some((n, side, shift));
                    }
                    if n == 0 {
                        break;
                    }
                }
                best.map_or((LabelSide::Left, P::default()), |b| (b.1, b.2))
            }
        };
        let (pos, anchor, v, bounds) = match (i, s.pin_label) {
            // Placed freely (the only connection point, see `SymbolInst::pin_label`).
            (0, Some(l)) => {
                let pos = s.pos + l.offset;
                (pos, l.anchor, VAnchor::Middle, text_box(pos, text, SMALL_SIZE, l.anchor, VAnchor::Middle))
            }
            _ => place(side, shift),
        };
        out.push(PinLabelPlace { pin: i, pos, anchor, v, side: side.in_symbol(s.rot, s.mirror), shift: shift.untransform(s.rot, s.mirror), bounds });
    }
    out
}

#[cfg(test)]
mod tests;
