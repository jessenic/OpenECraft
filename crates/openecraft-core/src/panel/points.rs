//! Connection points of the devices in the panel layout: the connection points of a
//! device's parts (their own or generated ones) placed into the device box the same
//! way as its 3D model, and matched to the connection points of the schematic.

use super::{Face, ItemKind, PanelItem};
use crate::model::{Project, SymbolInst};
use crate::parts::vec3::{add, norm, sub, V3};
use crate::parts::{Exit, Part};
use crate::symbols::Func;

/// A connection point of a placed device.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedPoint {
    /// Connection point designation (`A1`, `13`); terminals: their side (`1`, `2`).
    pub pin: String,
    /// Terminal number on terminal strips, otherwise empty.
    pub terminal: String,
    /// Position in the coordinates of the item's face (plate or door), z above it.
    pub pos: V3,
    pub exit: Exit,
    /// From the part's own data; false if generated from its function templates.
    pub exact: bool,
}

/// One part of a device and where it sits in depth (front mounted accessories are
/// stacked in front of the main part, like in [`super::device_size`]).
struct Layer<'a> {
    part: &'a Part,
    size: [f32; 3],
    z: f32,
}

fn same(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

/// Direction after the quarter turn of a rotated device (the part's top points to the
/// right, see [`super::place_mesh`]).
fn turned(exit: Exit) -> Exit {
    match exit {
        Exit::Up => Exit::Right,
        Exit::Right => Exit::Down,
        Exit::Down => Exit::Left,
        Exit::Left => Exit::Up,
        Exit::Front => Exit::Front,
    }
}

/// The direction a wire leaves a connection point in, in the coordinates of its face
/// (y down, z away from it).
pub fn exit_vector(exit: Exit) -> V3 {
    match exit {
        Exit::Up => [0.0, -1.0, 0.0],
        Exit::Down => [0.0, 1.0, 0.0],
        Exit::Left => [-1.0, 0.0, 0.0],
        Exit::Right => [1.0, 0.0, 0.0],
        Exit::Front => [0.0, 0.0, 1.0],
    }
}

/// The nozzles of the model of a fluid device: the connection point each is for, its
/// end and the direction a pipe leaves it in, in the coordinates of the item's face.
pub fn nozzles(prj: &Project, it: &PanelItem) -> Vec<(String, V3, V3)> {
    let Some(part) = super::part_by_number(prj, &it.part) else { return Vec::new() };
    let Some(mesh) = part.model_3d.as_deref() else { return Vec::new() };
    crate::parts::nozzles(part)
        .into_iter()
        .map(|n| {
            let (a, b) = (super::place_point(mesh, it, n.at), super::place_point(mesh, it, add(n.at, n.dir)));
            (n.pin, a, norm(sub(b, a)))
        })
        .collect()
}

/// Busbar terminals closer to each other than this (mm) along the bars are entered
/// from the front: a wire has no way out between them.
pub const TIGHT_TERMINALS: f32 = 30.0;

/// The connection points of a device item, in the coordinates of its face: those of
/// each part of its [`super::assembly`] in that part's box. Terminal strips get the
/// points of their terminal part once per terminal, in strip order.
pub fn device_points(prj: &Project, it: &PanelItem) -> Vec<PlacedPoint> {
    if it.kind != ItemKind::Device {
        return Vec::new();
    }
    if let Some(a) = super::assembly(prj, &it.tag) {
        let rotated = super::item_rotated(it, a.h);
        let mut out = Vec::new();
        for piece in a.pieces() {
            piece_points(&mut out, piece.part, piece.size, &a.piece_item(it, piece), rotated);
        }
        // Accessories built into the main part: their terminals on its box (generated
        // ones laid out over it as if the accessory were as large).
        let main = a.piece_item(it, &a.main);
        for part in &a.inside {
            match part.size() {
                Some(size) if !part.connection_points.is_empty() => piece_points(&mut out, part, size, &main, rotated),
                _ => {
                    let [w, h, d] = a.main.size;
                    let sized = Part { width: w, height: h, depth: d, ..(*part).clone() };
                    piece_points(&mut out, &sized, a.main.size, &main, rotated);
                }
            }
        }
        // A device standing on the floor without connection points of its own (a
        // motor) is connected on top, where its terminal box is.
        if it.face == Face::Bottom {
            for p in out.iter_mut().filter(|p| !p.exact) {
                (p.pos, p.exit) = ([p.pos[0], it.y + it.h / 2.0, it.z + it.d], Exit::Front);
            }
        }
        return out;
    }
    let Some(data) = prj.devices.get(&it.tag) else { return Vec::new() };
    let parts: Vec<(&Part, u32)> =
        data.parts.iter().filter_map(|r| prj.parts.iter().find(|p| p.number == r.number).map(|p| (p, r.count.max(1)))).collect();
    let Some((main, [mw, mh, md])) = parts.iter().find_map(|(p, _)| p.size().map(|s| (*p, s))) else { return Vec::new() };
    let mut layers = vec![Layer { part: main, size: [mw, mh, md], z: 0.0 }];
    let mut depth = md;
    for (p, count) in &parts {
        if let (true, Some(size)) = (p.number != main.number, p.size()) {
            if size[0] <= mw + 1.0 {
                layers.push(Layer { part: p, size, z: depth });
                depth += size[2] * *count as f32;
            }
        }
    }
    // Busbar terminals across the bars: each upright on its bar. Where the next
    // terminal is closer than a wire's way out, the points facing it leave to the
    // front instead (the wire comes to the clamp from before it).
    if let Some(places) = super::busbars::terminals_on_bars(prj, it) {
        let mut out = Vec::new();
        let along_y = places.windows(2).all(|w| (w[0].0.x - w[1].0.x).abs() < 0.01);
        for (k, (sub, number, label)) in places.iter().enumerate() {
            let part = super::part_by_number(prj, number).unwrap_or(main);
            let size = part.size().unwrap_or([mw, mh, md]);
            let from = out.len();
            piece_points(&mut out, part, size, sub, super::item_rotated(sub, size[1]));
            let gap_to = |n: &PanelItem| if along_y { (sub.y - n.y - n.h).max(n.y - sub.y - sub.h) } else { (sub.x - n.x - n.w).max(n.x - sub.x - sub.w) };
            let (before, after) = (k.checked_sub(1).map(|j| &places[j].0), places.get(k + 1).map(|p| &p.0));
            for p in &mut out[from..] {
                p.terminal = label.clone();
                let towards = match (p.exit, along_y) {
                    (Exit::Up, true) | (Exit::Left, false) => before,
                    (Exit::Down, true) | (Exit::Right, false) => after,
                    _ => None,
                };
                if towards.is_some_and(|n| gap_to(n) < TIGHT_TERMINALS) {
                    p.exit = Exit::Front;
                }
            }
        }
        return out;
    }
    let strip = super::is_strip_part(main);
    if strip {
        depth = super::strip_depth(&parts);
    }
    // Rotated by a quarter turn: the part's height lies along x.
    let rotated = !same(it.h, mh) && same(it.w, mh);
    let along = if rotated { it.h } else { it.w };
    let sz = if depth > 0.0 { it.d / depth } else { 0.0 };
    // Places along the device: (start, width, terminal number, layers). Terminal
    // strips have one per terminal with its own part, in the widths of the strip
    // layout; other devices one for the whole device.
    let mut cells: Vec<(f32, f32, String, Vec<Layer>)> = Vec::new();
    let layout = if strip { crate::strips::strip_layout(prj, &it.tag) } else { Vec::new() };
    let total: f32 = layout.iter().map(|e| e.width).sum();
    if layout.is_empty() || total <= 0.0 {
        cells.push((0.0, along, String::new(), layers));
    } else {
        let mut at = 0.0;
        for e in &layout {
            let width = e.width * along / total;
            if let crate::strips::ElementKind::Terminal(label) = &e.kind {
                let part = prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == e.part).unwrap_or(main);
                let size = part.size().unwrap_or([mw, mh, md]);
                cells.push((at, width, label.clone(), vec![Layer { part, size, z: 0.0 }]));
            }
            at += width;
        }
    }
    let mut out = Vec::new();
    for (start, cell, terminal, layers) in cells {
        let [cw, ch, _] = layers[0].size;
        for layer in &layers {
            let (points, exact) = layer.part.connection_points_or_default();
            for c in points {
                // Accessories are centred on the main part's front.
                let fx = c.pos[0] + (cw - layer.size[0]) / 2.0;
                let fy = c.pos[1] + (ch - layer.size[1]) / 2.0;
                let z = it.z + (layer.z + c.pos[2]) * sz;
                let (pos, exit) = if rotated {
                    ([it.x + (ch - fy) * (it.w / ch), it.y + start + fx * (cell / cw), z], turned(c.exit))
                } else {
                    ([it.x + start + fx * (cell / cw), it.y + fy * (it.h / ch), z], c.exit)
                };
                // A device standing on the floor without connection points of its own
                // (a motor) is connected on top, where its terminal box is.
                let (pos, exit) = if it.face == Face::Bottom && !exact { ([pos[0], it.y + it.h / 2.0, it.z + it.d], Exit::Front) } else { (pos, exit) };
                out.push(PlacedPoint { pin: c.pin, terminal: terminal.clone(), pos, exit, exact });
            }
        }
    }
    out
}

/// The connection points of a part placed into the box `b` of its piece of a device.
fn piece_points(out: &mut Vec<PlacedPoint>, part: &Part, [pw, ph, pd]: [f32; 3], b: &PanelItem, rotated: bool) {
    let (points, exact) = part.connection_points_or_default();
    for c in points {
        let z = b.z + c.pos[2] * (b.d / pd.max(1e-3));
        let (pos, exit) = if rotated {
            ([b.x + (ph - c.pos[1]) * (b.w / ph), b.y + c.pos[0] * (b.h / pw), z], turned(c.exit))
        } else {
            ([b.x + c.pos[0] * (b.w / pw), b.y + c.pos[1] * (b.h / ph), z], c.exit)
        };
        out.push(PlacedPoint { pin: c.pin, terminal: String::new(), pos, exit, exact });
    }
}

/// The placed point of connection point `pin` (index) of schematic symbol `s`:
/// terminals by terminal number and side (the studs of a busbar, such as the negative
/// busbar of a DC distributor, by their number), PLC points by their address, other
/// functions by designation (as in `netlist::pin_designation`). A designation with a
/// leading number that no point has (`2S1`) is the point without it (`S1`) of the part
/// of that number: of the second of the current transformers behind the poles of a
/// fuse switch disconnector strip.
pub fn point_of<'a>(prj: &Project, points: &'a [PlacedPoint], s: &SymbolInst, pin: usize) -> Option<&'a PlacedPoint> {
    let def = prj.symbol(&s.sym)?;
    if def.func == Func::Terminal {
        let side = (pin + 1).to_string();
        let stud = || points.iter().find(|p| p.terminal.is_empty() && p.pin == s.label);
        return points.iter().find(|p| p.terminal == s.label && p.pin == side).or_else(stud);
    }
    let name = match def.func {
        Func::Plc if def.plc_channel.is_none() && def.plc_channels.is_empty() => s.label.as_str(),
        Func::Plug(_) => s.label.as_str(),
        _ => s.pins.get(pin).map(String::as_str).or_else(|| def.pins.get(pin).map(|d| d.label))?,
    };
    let named = |name: &str, n: usize| points.iter().filter(|p| p.terminal.is_empty() && p.pin == name).nth(n);
    named(name, 0).or_else(|| {
        let digits = name.chars().take_while(char::is_ascii_digit).count();
        let n: usize = name[..digits].parse().ok().filter(|n| *n > 0)?;
        named(&name[digits..], n - 1)
    })
}

#[cfg(test)]
mod tests;
