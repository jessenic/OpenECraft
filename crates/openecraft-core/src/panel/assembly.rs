//! How the parts of a device are mounted together: the main part, accessories
//! snapped onto its front (auxiliary contact blocks, stacked in depth), accessories
//! beside it on the rail (auxiliary and signal contacts of breakers), bases it sits on
//! (component modules, adapters) and accessories built into it (releases, auxiliary
//! switches of circuit breakers). The footprint, the connection points, the 3D model
//! and the 2D front view of a device item all follow it.

use super::{Face, ItemKind, PanelItem};
use crate::model::Project;
use crate::parts::{AccessoryMount, Part};

/// One part of an assembly and its box in the device, unscaled (mm): `at` along the
/// rail from the left end, `across` its middle off the middle of the device across the
/// rail, `z` above the plate.
#[derive(Clone, Copy, Debug)]
pub struct Piece<'a> {
    pub part: &'a Part,
    pub size: [f32; 3],
    pub at: f32,
    pub across: f32,
    pub z: f32,
}

#[derive(Clone, Debug)]
pub struct Assembly<'a> {
    pub main: Piece<'a>,
    /// Accessories on the front of the main part, centred on it, back to front.
    pub front: Vec<Piece<'a>>,
    /// Accessories beside the main part, left to right.
    pub sides: Vec<Piece<'a>>,
    /// Bases under the main part, back to front; the main part and its other
    /// accessories stand on the front one.
    pub behind: Vec<Piece<'a>>,
    /// Accessories built into the main part: no box of their own, their connection
    /// points are on the main part.
    pub inside: Vec<&'a Part>,
    /// Length along the rail, extent across it and depth of the whole device.
    pub w: f32,
    pub h: f32,
    pub d: f32,
}

impl<'a> Assembly<'a> {
    /// Only the main part: nothing mounted on it.
    pub fn is_plain(&self) -> bool {
        self.front.is_empty() && self.sides.is_empty() && self.behind.is_empty()
    }

    /// All pieces with a box, back to front: the bases, the main part, the front and
    /// the side accessories.
    pub fn pieces(&self) -> impl Iterator<Item = &Piece<'a>> {
        self.behind.iter().chain(std::iter::once(&self.main)).chain(&self.front).chain(&self.sides)
    }

    /// The box of a piece placed into the device item `it` (scaled to the item, turned
    /// with it), as an item of its own.
    pub fn piece_item(&self, it: &PanelItem, piece: &Piece) -> PanelItem {
        let rotated = super::item_rotated(it, self.h);
        let (along, across) = if rotated { (it.h, it.w) } else { (it.w, it.h) };
        let sa = along / self.w.max(1e-3);
        let sc = across / self.h.max(1e-3);
        let sz = it.d / self.d.max(1e-3);
        // Centred across the rail, unless it lies off the middle.
        let (a0, len) = (piece.at * sa, piece.size[0] * sa);
        let cross = piece.size[1] * sc;
        let c0 = (across - cross) / 2.0 + piece.across * sc;
        let mut sub = it.clone();
        if rotated {
            (sub.x, sub.w, sub.y, sub.h) = (it.x + c0, cross, it.y + a0, len);
        } else {
            (sub.x, sub.w, sub.y, sub.h) = (it.x + a0, len, it.y + c0, cross);
        }
        (sub.z, sub.d) = (it.z + piece.z * sz, piece.size[2] * sz);
        sub.part = piece.part.number.clone();
        sub
    }
}

/// Where the parts of a device sit: the main part is the first part with dimensions
/// that is no accessory (otherwise the first part with dimensions). Accessories
/// mounted in front ([`AccessoryMount::Front`], or [`AccessoryMount::Auto`] when they
/// are not wider than the main part) are stacked in front of it as often as the device
/// uses them, accessories mounted left or right stand beside it. Bases
/// ([`AccessoryMount::Behind`]) are stacked under the main part, centred on each
/// other, several of one part side by side; accessories built into it ([`AccessoryMount::Inside`]) need no dimensions.
/// Terminal strips and devices without parts with dimensions have none.
pub fn assembly<'a>(prj: &'a Project, tag: &str) -> Option<Assembly<'a>> {
    let data = prj.devices.get(tag)?;
    let all: Vec<(&Part, u32)> = data.parts.iter().filter_map(|r| prj.parts.iter().find(|x| x.number == r.number).map(|p| (p, r.count.max(1)))).collect();
    let parts: Vec<(&Part, [f32; 3], u32)> = all.iter().filter_map(|(p, n)| p.size().map(|s| (*p, s, *n))).collect();
    let (main, size, _) = *parts.iter().find(|(p, _, _)| !p.accessory).or(parts.first())?;
    if super::is_strip_part(main) {
        return None;
    }
    let mut a = Assembly {
        main: Piece { part: main, size, at: 0.0, across: 0.0, z: 0.0 },
        front: Vec::new(),
        sides: Vec::new(),
        behind: Vec::new(),
        inside: Vec::new(),
        w: size[0],
        h: size[1],
        d: size[2],
    };
    for (p, count) in all.iter().filter(|(p, _)| p.number != main.number && p.accessory_mount == AccessoryMount::Inside) {
        a.inside.extend(std::iter::repeat_n(*p, *count as usize));
    }
    // The bases first: the main part stands on them. Several of one part lie side by
    // side along the longer side of the main part (a current transformer behind each
    // pole of a vertical fuse switch strip), different parts on each other.
    let mut base = 0.0;
    let mut layer_w = Vec::new();
    let lengthwise = size[1] > size[0];
    for (p, s, count) in parts.iter().filter(|(p, _, _)| p.number != main.number && p.accessory_mount == AccessoryMount::Behind) {
        let n = *count as f32;
        for k in 0..*count {
            let k = k as f32;
            let piece = if lengthwise {
                a.h = a.h.max(n * s[1]);
                Piece { part: p, size: *s, at: 0.0, across: (k - (n - 1.0) / 2.0) * s[1], z: base }
            } else {
                Piece { part: p, size: *s, at: k * s[0], across: 0.0, z: base }
            };
            a.behind.push(piece);
            layer_w.push(if lengthwise { s[0] } else { n * s[0] });
        }
        base += s[2];
    }
    a.main.z = base;
    a.d = base + size[2];
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (p, s, count) in parts.iter().filter(|(p, _, _)| p.number != main.number) {
        let mount = match p.accessory_mount {
            AccessoryMount::Auto if s[0] <= size[0] + 1.0 => AccessoryMount::Front,
            AccessoryMount::Auto => AccessoryMount::Loose,
            m => m,
        };
        for _ in 0..*count {
            match mount {
                AccessoryMount::Front => {
                    a.front.push(Piece { part: p, size: *s, at: 0.0, across: 0.0, z: a.d });
                    a.d += s[2];
                }
                AccessoryMount::Left => left.push((*p, *s)),
                AccessoryMount::Right => right.push((*p, *s)),
                AccessoryMount::Auto | AccessoryMount::Loose | AccessoryMount::Behind | AccessoryMount::Inside => {}
            }
        }
    }
    // Left accessories in their order outwards from the main part.
    let mut at = 0.0;
    for (p, s) in left.iter().rev() {
        a.sides.push(Piece { part: p, size: *s, at, across: 0.0, z: base });
        at += s[0];
    }
    a.main.at = at;
    for f in &mut a.front {
        f.at = at + (size[0] - f.size[0]) / 2.0;
    }
    at += size[0];
    for (p, s) in right {
        a.sides.push(Piece { part: p, size: s, at, across: 0.0, z: base });
        at += s[0];
    }
    // The row of main part and side accessories centred on a wider base, the bases
    // centred under a wider row.
    let wide = layer_w.iter().copied().fold(at, f32::max);
    let shift = (wide - at) / 2.0;
    for piece in std::iter::once(&mut a.main).chain(&mut a.front).chain(&mut a.sides) {
        piece.at += shift;
    }
    for (b, w) in a.behind.iter_mut().zip(&layer_w) {
        b.at += (wide - w) / 2.0;
    }
    a.w = wide;
    a.h = a.behind.iter().map(|b| b.size[1]).fold(a.h, f32::max);
    let deepest = a.sides.iter().map(|s| s.z + s.size[2]).fold(a.d, f32::max);
    a.d = deepest;
    Some(a)
}

/// The assembly of a placed device item, if parts are mounted on its main part (the
/// item drawn piece by piece). Door and floor devices are drawn as one.
pub fn mounted<'a>(prj: &'a Project, it: &PanelItem) -> Option<Assembly<'a>> {
    if it.kind != ItemKind::Device || it.single_part || !it.terminal.is_empty() || matches!(it.face, Face::Door | Face::Bottom) {
        return None;
    }
    assembly(prj, &it.tag).filter(|a| !a.is_plain() && a.main.part.number == it.part)
}

#[cfg(test)]
mod tests;
