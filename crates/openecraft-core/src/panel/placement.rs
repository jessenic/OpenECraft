//! Parametric component placement: anchors that keep an item at a distance from the
//! edges of its face (centred, or stretched between two edges) when the enclosure
//! changes size, rectangular patterns of copies, aligning and distributing items and
//! lining devices up on a mounting rail.

use super::{snap_to_rail, Enclosure, ItemKind, PanelItem, SNAP};
use crate::geom::R;
use crate::model::Project;
use openecraft_i18n::tr;
use std::collections::HashMap;
pub use crate::model::panel::{Align, Anchor};

impl Align {
    pub const KINDS: [Align; 5] = [Align::Free, Align::Start(0.0), Align::Center(0.0), Align::End(0.0), Align::Stretch(0.0, 0.0)];

    pub fn title(self) -> &'static str {
        match self {
            Align::Free => tr("Free"),
            Align::Start(_) => tr("From start"),
            Align::Center(_) => tr("Centred"),
            Align::End(_) => tr("From end"),
            Align::Stretch(..) => tr("Stretched"),
        }
    }

    pub fn same_kind(self, other: Align) -> bool {
        std::mem::discriminant(&self) == std::mem::discriminant(&other)
    }

    /// Position and length along an axis of length `len` for an item at `pos` of length
    /// `size`.
    pub fn place(self, len: f32, pos: f32, size: f32) -> (f32, f32) {
        match self {
            Align::Free => (pos, size),
            Align::Start(m) => (m, size),
            Align::Center(o) => ((len - size) / 2.0 + o, size),
            Align::End(m) => (len - m - size, size),
            Align::Stretch(a, b) => (a, (len - a - b).max(SNAP)),
        }
    }

    /// The same kind of placement for an item where it is now.
    pub fn capture(self, len: f32, pos: f32, size: f32) -> Align {
        match self {
            Align::Free => Align::Free,
            Align::Start(_) => Align::Start(pos),
            Align::Center(_) => Align::Center(pos + size / 2.0 - len / 2.0),
            Align::End(_) => Align::End(len - pos - size),
            Align::Stretch(..) => Align::Stretch(pos, len - pos - size),
        }
    }

    /// Command line form: `free`, `start:20`, `center:0`, `end:20`, `stretch:10,10`.
    pub fn parse(s: &str) -> Option<Align> {
        let (kind, rest) = s.trim().split_once(':').unwrap_or((s.trim(), "0"));
        let nums: Vec<f32> = rest.split(',').filter_map(|v| v.trim().parse().ok()).collect();
        let a = nums.first().copied().unwrap_or(0.0);
        Some(match kind.to_lowercase().as_str() {
            "free" => Align::Free,
            "start" => Align::Start(a),
            "center" | "centre" => Align::Center(a),
            "end" => Align::End(a),
            "stretch" => Align::Stretch(a, nums.get(1).copied().unwrap_or(a)),
            _ => return None,
        })
    }

    pub fn text(self) -> String {
        match self {
            Align::Free => "free".into(),
            Align::Start(v) => format!("start:{v}"),
            Align::Center(v) => format!("center:{v}"),
            Align::End(v) => format!("end:{v}"),
            Align::Stretch(a, b) => format!("stretch:{a},{b}"),
        }
    }
}

impl Anchor {
    pub fn is_free(&self) -> bool {
        self.x == Align::Free && self.y == Align::Free
    }

    /// Moves (and stretches) an item on a face of size `face`.
    pub fn apply(&self, it: &mut PanelItem, face: (f32, f32)) {
        (it.x, it.w) = self.x.place(face.0, it.x, it.w);
        (it.y, it.h) = self.y.place(face.1, it.y, it.h);
    }

    /// The anchor of the same kinds for an item where it is now (after moving it).
    pub fn capture(&self, it: &PanelItem, face: (f32, f32)) -> Anchor {
        Anchor { x: self.x.capture(face.0, it.x, it.w), y: self.y.capture(face.1, it.y, it.h) }
    }
}

/// Places one item by its anchor on its face.
pub fn apply_one_anchor(e: &Enclosure, it: &mut PanelItem) {
    if !it.anchor.is_free() {
        let a = it.anchor;
        a.apply(it, e.face_size(it.face));
    }
}

/// Places the anchored items of an enclosure (after it changed its size); devices then
/// snap onto their rails again.
pub fn apply_anchors(e: &mut Enclosure) {
    for i in 0..e.items.len() {
        if e.items[i].anchor.is_free() {
            continue;
        }
        let mut it = e.items[i].clone();
        apply_one_anchor(e, &mut it);
        snap_to_rail(e, &mut it);
        e.items[i] = it;
    }
}

/// Keeps the anchors of moved items: the same kinds of placement, new distances.
pub fn capture_anchors(e: &mut Enclosure, ids: &[u64]) {
    for i in 0..e.items.len() {
        if ids.contains(&e.items[i].id) && !e.items[i].anchor.is_free() {
            let it = &e.items[i];
            let a = it.anchor.capture(it, e.face_size(it.face));
            e.items[i].anchor = a;
        }
    }
}

/// Copies items in a rectangular pattern: `nx` × `ny` places `px` / `py` apart, the
/// originals in the first. Devices are left out (a device is placed once); groups are
/// copied as new groups. Returns the ids of the copies.
pub fn pattern_copy(prj: &mut Project, enc: usize, ids: &[u64], nx: usize, ny: usize, px: f32, py: f32) -> Vec<u64> {
    let Some(e) = prj.layout.enclosures.get(enc) else { return Vec::new() };
    let originals: Vec<PanelItem> = e.items.iter().filter(|i| ids.contains(&i.id) && i.kind != ItemKind::Device).cloned().collect();
    let groups = e.groups.clone();
    let mut out = Vec::new();
    for row in 0..ny.max(1) {
        for col in 0..nx.max(1) {
            if row == 0 && col == 0 {
                continue;
            }
            let mut new_groups: HashMap<u64, u64> = HashMap::new();
            for o in &originals {
                let group = match o.group {
                    0 => 0,
                    g => *new_groups.entry(g).or_insert_with(|| {
                        let id = prj.new_id();
                        let name = groups.iter().find(|x| x.id == g).map(|x| x.name.clone()).unwrap_or_default();
                        prj.layout.enclosures[enc].groups.push(super::ItemGroup { id, name: format!("{name} ({})", row * nx.max(1) + col + 1), source: String::new() });
                        id
                    }),
                };
                let id = prj.new_id();
                let copy = PanelItem { id, x: o.x + px * col as f32, y: o.y + py * row as f32, anchor: Default::default(), group, ..o.clone() };
                prj.layout.enclosures[enc].items.push(copy);
                out.push(id);
            }
        }
    }
    out
}

/// Edge or centre line items are aligned to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignTo {
    Left,
    CenterX,
    Right,
    Top,
    CenterY,
    Bottom,
}

impl AlignTo {
    pub const ALL: [AlignTo; 6] = [AlignTo::Left, AlignTo::CenterX, AlignTo::Right, AlignTo::Top, AlignTo::CenterY, AlignTo::Bottom];

    pub fn title(self) -> &'static str {
        match self {
            AlignTo::Left => tr("Align left"),
            AlignTo::CenterX => tr("Centre horizontally"),
            AlignTo::Right => tr("Align right"),
            AlignTo::Top => tr("Align top"),
            AlignTo::CenterY => tr("Centre vertically"),
            AlignTo::Bottom => tr("Align bottom"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            AlignTo::Left => "left",
            AlignTo::CenterX => "center_x",
            AlignTo::Right => "right",
            AlignTo::Top => "top",
            AlignTo::CenterY => "center_y",
            AlignTo::Bottom => "bottom",
        }
    }
}

fn bounds<'a>(items: impl Iterator<Item = &'a PanelItem>) -> Option<R> {
    items.map(|i| i.rect()).reduce(|mut a, b| {
        a.union(b);
        a
    })
}

/// Aligns items to an edge or the centre line of their common bounds.
pub fn align(e: &mut Enclosure, ids: &[u64], to: AlignTo) {
    let Some(b) = bounds(e.items.iter().filter(|i| ids.contains(&i.id))) else { return };
    for it in e.items.iter_mut().filter(|i| ids.contains(&i.id)) {
        match to {
            AlignTo::Left => it.x = b.min.x,
            AlignTo::CenterX => it.x = (b.min.x + b.max.x - it.w) / 2.0,
            AlignTo::Right => it.x = b.max.x - it.w,
            AlignTo::Top => it.y = b.min.y,
            AlignTo::CenterY => it.y = (b.min.y + b.max.y - it.h) / 2.0,
            AlignTo::Bottom => it.y = b.max.y - it.h,
        }
    }
    capture_anchors(e, ids);
}

/// Spaces items evenly (equal gaps) between the first and the last one, along x or y.
pub fn distribute(e: &mut Enclosure, ids: &[u64], horizontal: bool) {
    let mut sel: Vec<(u64, f32, f32)> =
        e.items.iter().filter(|i| ids.contains(&i.id)).map(|i| if horizontal { (i.id, i.x, i.w) } else { (i.id, i.y, i.h) }).collect();
    if sel.len() < 3 {
        return;
    }
    sel.sort_by(|a, b| a.1.total_cmp(&b.1));
    let start = sel[0].1;
    let end = sel.last().map(|s| s.1 + s.2).unwrap_or(start);
    let used: f32 = sel.iter().map(|s| s.2).sum();
    let gap = (end - start - used) / (sel.len() - 1) as f32;
    let mut at = start;
    for (id, _, size) in sel {
        if let Some(it) = e.items.iter_mut().find(|i| i.id == id) {
            if horizontal {
                it.x = at;
            } else {
                it.y = at;
            }
        }
        at += size + gap;
    }
    capture_anchors(e, ids);
}

/// Lines devices up on a horizontal rail from its left end, `gap` apart, in their
/// order from left to right, mounted on the rail. Returns false if they do not fit.
pub fn arrange_on_rail(e: &mut Enclosure, rail: u64, ids: &[u64], gap: f32) -> bool {
    let Some(r) = e.item(rail).filter(|r| r.kind == ItemKind::Rail && r.horizontal()).cloned() else { return false };
    let mut sel: Vec<(u64, f32)> =
        e.items.iter().filter(|i| ids.contains(&i.id) && i.kind == ItemKind::Device && i.face == r.face).map(|i| (i.id, i.x)).collect();
    sel.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut at = r.x + gap;
    for (id, _) in sel {
        if let Some(it) = e.items.iter_mut().find(|i| i.id == id) {
            it.x = at;
            it.seat_on(&r);
            at += it.w + gap;
        }
    }
    capture_anchors(e, ids);
    at <= r.x + r.w + 0.01
}

#[cfg(test)]
mod tests;
