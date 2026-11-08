//! Production data of the panel layout (in the spirit of EPLAN Pro Panel): drilling
//! pattern and cutouts of the mounting plate, the door and the walls with NC export
//! (DXF / CSV), wire end treatment (stripping length, ferrules). The clearance checks are in
//! [`crate::panel::validate`].

use crate::geom::{p, P, R};
use crate::model::Project;
use crate::panel::{Enclosure, Face, Fastening, ItemKind, PanelItem};
use crate::parts::Part;
use std::fmt::Write;

/// Rails: M5 holes 12.5 mm from the ends, at most every 200 mm.
pub const RAIL_HOLES: (f32, f32, &str) = (12.5, 200.0, "M5");
/// Cable ducts: M4 holes 20 mm from the ends, at most every 300 mm.
pub const DUCT_HOLES: (f32, f32, &str) = (20.0, 300.0, "M4");
/// Busbar holders: M6 holes 25 mm from the ends, at most every 250 mm.
pub const BUSBAR_HOLES: (f32, f32, &str) = (25.0, 250.0, "M6");
/// Devices screwed onto the plate without mounting data: two M4 holes, 5 mm from
/// two opposite corners.
pub const DEVICE_HOLE_INSET: f32 = 5.0;
pub const BUSBAR_HOLDER: &str = "GEN.BB-HOLDER-3P";

/// Tap drill diameter of a metric thread (M3 … M10), or a plain diameter ("Ø4.5",
/// "4.5").
pub fn drill_diameter(spec: &str) -> Option<f32> {
    let s = spec.trim();
    let tap = match s.to_uppercase().as_str() {
        "M3" => Some(2.5),
        "M4" => Some(3.3),
        "M5" => Some(4.2),
        "M6" => Some(5.0),
        "M8" => Some(6.8),
        "M10" => Some(8.5),
        _ => None,
    };
    tap.or_else(|| s.trim_start_matches(['Ø', 'D', 'd']).trim().replace(',', ".").parse().ok())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Hole {
    pub item: u64,
    /// Face coordinates (origin top left, y down).
    pub pos: P,
    /// Drill diameter (weld studs: their nominal diameter).
    pub diameter: f32,
    /// Thread to cut ("M5"), empty for through holes.
    pub thread: String,
    /// How the item is fixed at this point.
    pub fastening: Fastening,
    /// Size of the screw ("M5", also for through holes), empty for plain diameters.
    pub thread_size: String,
}

/// Positions along a length: `end` from both ends and evenly spaced in between with
/// at most `pitch` distance.
pub fn line_positions(len: f32, end: f32, pitch: f32) -> Vec<f32> {
    if len <= 2.0 * end + 1.0 {
        return vec![len / 2.0];
    }
    let span = len - 2.0 * end;
    let n = (span / pitch).ceil().max(1.0) as usize;
    (0..=n).map(|i| end + span * i as f32 / n as f32).collect()
}

/// Mounting holes from the part data: `Mounting holes: M4: 5,5; 40,85` (positions
/// relative to the top left corner of the part, front view).
pub fn part_holes(part: &Part) -> Option<(String, Vec<P>)> {
    let spec = part.data_value("Mounting holes")?;
    let (thread, list) = spec.split_once(':')?;
    let pts: Vec<P> = list
        .split(';')
        .filter_map(|xy| {
            let (x, y) = xy.split_once(',')?;
            Some(p(x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect();
    (!pts.is_empty()).then(|| (thread.trim().to_string(), pts))
}

fn hole_with(item: u64, pos: P, thread: &str, fastening: Fastening) -> Hole {
    let size = crate::parts::mechanics::thread(thread).unwrap_or("").to_string();
    let diameter = fastening.hole_diameter(thread).or_else(|| drill_diameter(thread)).unwrap_or(4.2);
    let tapped = fastening == Fastening::Tapped && !size.is_empty();
    Hole { item, pos, diameter, thread: if tapped { size.clone() } else { String::new() }, fastening, thread_size: size }
}

fn hole(item: u64, pos: P, thread: &str) -> Hole {
    hole_with(item, pos, thread, Fastening::Tapped)
}

/// Holes of one item by its fastening (devices on rails, glued or clipped items need
/// none; weld studs are fixing points without a hole).
pub fn item_holes(it: &PanelItem, parts: &[Part]) -> Vec<Hole> {
    let part = parts.iter().find(|x| x.number == it.part).or_else(|| crate::parts::catalog().iter().find(|x| x.number == it.part));
    let fastening = Fastening::resolve(it, part);
    if !fastening.has_points() {
        return Vec::new();
    }
    pattern_holes(it, parts)
        .into_iter()
        .map(|h| if h.thread_size.is_empty() { Hole { fastening, ..h } } else { hole_with(h.item, h.pos, &h.thread_size, fastening) })
        .collect()
}

/// Fixing points of an item as tapped holes (the pattern, independent of the
/// fastening).
fn pattern_holes(it: &PanelItem, parts: &[Part]) -> Vec<Hole> {
    let along = |(end, pitch, thread): (f32, f32, &str)| {
        let (a, _) = it.center_line();
        line_positions(it.length(), end, pitch)
            .into_iter()
            .map(|t| hole(it.id, if it.horizontal() { p(a.x + t, a.y) } else { p(a.x, a.y + t) }, thread))
            .collect::<Vec<_>>()
    };
    match it.kind {
        ItemKind::Rail => along(RAIL_HOLES),
        ItemKind::Duct => along(DUCT_HOLES),
        ItemKind::Busbar => {
            // Holders sit at the two outer bars; one hole each side of the system.
            let (a, _) = it.center_line();
            let half = if it.horizontal() { it.h / 2.0 - 6.0 } else { it.w / 2.0 - 6.0 };
            let off = if it.horizontal() { p(0.0, half) } else { p(half, 0.0) };
            let mut v = Vec::new();
            for t in line_positions(it.length(), BUSBAR_HOLES.0, BUSBAR_HOLES.1) {
                let c = if it.horizontal() { p(a.x + t, a.y) } else { p(a.x, a.y + t) };
                v.push(hole(it.id, c - off, BUSBAR_HOLES.2));
                v.push(hole(it.id, c + off, BUSBAR_HOLES.2));
            }
            v
        }
        ItemKind::Device | ItemKind::Accessory | ItemKind::MountingPlate => {
            let part = parts.iter().find(|x| x.number == it.part).or_else(|| crate::parts::catalog().iter().find(|x| x.number == it.part));
            match part.and_then(part_holes) {
                Some((thread, pts)) => {
                    // Turned devices (width and height swapped) turn their pattern.
                    let pd = part.unwrap();
                    let rotated = (pd.width - pd.height).abs() > 0.5 && (it.w - pd.height).abs() < 0.5 && (it.h - pd.width).abs() < 0.5;
                    pts.into_iter()
                        .map(|q| if rotated { p(it.x + pd.height - q.y, it.y + q.x) } else { p(it.x + q.x, it.y + q.y) })
                        .map(|q| hole(it.id, q, &thread))
                        .collect()
                }
                None if it.kind!=ItemKind::MountingPlate && it.w > 2.0 * DEVICE_HOLE_INSET && it.h > 2.0 * DEVICE_HOLE_INSET => {
                    let i = DEVICE_HOLE_INSET;
                    vec![hole(it.id, p(it.x + i, it.y + i), "M4"), hole(it.id, p(it.x + it.w - i, it.y + it.h - i), "M4")]
                }
                None => Vec::new(),
            }
        }
        ItemKind::Cutout => Vec::new(),
    }
}

/// Drilling pattern of a mounting plate.
pub fn holes(e: &Enclosure, parts: &[Part]) -> Vec<Hole> {
    holes_on(e, parts, Face::Plate)
}

/// Drilling pattern (and weld studs) of a face.
pub fn holes_on(e: &Enclosure, parts: &[Part], face: Face) -> Vec<Hole> {
    let mut v: Vec<Hole> = e.items.iter().filter(|it| it.face == face).flat_map(|it| item_holes(it, parts)).collect();
    v.sort_by(|a, b| a.pos.y.total_cmp(&b.pos.y).then(a.pos.x.total_cmp(&b.pos.x)));
    v
}

/// Material of busbar systems besides the bars: holders (one per hole pair).
pub fn busbar_holders(e: &Enclosure) -> usize {
    e.items
        .iter()
        .filter(|i| i.kind == ItemKind::Busbar)
        .map(|i| line_positions(i.length(), BUSBAR_HOLES.0, BUSBAR_HOLES.1).len())
        .sum()
}

/// Number of parallel bars of a busbar system (part data `Bars: 3`).
pub fn bar_count(parts: &[Part], part: &str) -> usize {
    parts
        .iter()
        .find(|x| x.number == part)
        .and_then(|x| x.tech_value("Bars"))
        .and_then(|v| v.parse().ok())
        .unwrap_or(3)
        .clamp(1, 8)
}

/// Centres of the bars of a busbar system across its width `across` (mm from its
/// edge): at the part's centre distance (`Centre distance: 185 mm`) where the bars fit,
/// otherwise evenly spaced.
pub fn bar_centres(parts: &[Part], part: &str, across: f32) -> Vec<f32> {
    let n = bar_count(parts, part);
    let distance = parts
        .iter()
        .find(|x| x.number == part)
        .and_then(|x| x.tech_value("Centre distance"))
        .and_then(|v| v.trim().trim_end_matches("mm").trim().parse::<f32>().ok())
        .filter(|c| n > 1 && *c > 0.0 && (n - 1) as f32 * c < across);
    match distance {
        Some(c) => {
            let first = (across - (n - 1) as f32 * c) / 2.0;
            (0..n).map(|k| first + k as f32 * c).collect()
        }
        None => (0..n).map(|k| across / n as f32 * (k as f32 + 0.5)).collect(),
    }
}

// ---------------------------------------------------------------------- cutouts

/// Opening in the mounting plate or the door, in the coordinates of its face.
#[derive(Clone, Debug, PartialEq)]
pub struct Opening {
    /// Cutout item, or the door device the opening is for.
    pub item: u64,
    pub center: P,
    pub w: f32,
    pub h: f32,
    pub round: bool,
}

/// A cutout size: `Ø22.5` (round) or `92 x 92` (rectangular). Returns (round, width,
/// height).
pub fn parse_cutout(spec: &str) -> Option<(bool, f32, f32)> {
    let s = spec.trim().trim_end_matches("mm").trim().replace(',', ".");
    if let Some((w, h)) = s.split_once(['x', 'X', '×']) {
        if let (Ok(w), Ok(h)) = (w.trim().parse::<f32>(), h.trim().parse::<f32>()) {
            return Some((false, w, h));
        }
    }
    drill_diameter(&s).map(|d| (true, d, d))
}

/// Door cutout of a device: part data `Door cutout: Ø22.5` (round) or `92 x 92`
/// (rectangular); without data Ø22.5 for command and signal devices, otherwise the
/// device outline less 2 mm on each side. Returns (round, width, height).
pub fn door_cutout(part: Option<&Part>, it: &PanelItem) -> (bool, f32, f32) {
    if let Some(c) = part.and_then(|p| p.data_value("Door cutout").or_else(|| p.data_value("Cutout"))).and_then(|v| parse_cutout(&v)) {
        return c;
    }
    let command = part.map(|p| p.group == "Sensors, switches, pushbuttons" || p.group == "Signal devices").unwrap_or(false);
    if command {
        (true, 22.5, 22.5)
    } else {
        (false, (it.w - 4.0).max(1.0), (it.h - 4.0).max(1.0))
    }
}

/// Openings of a face: cutout items, one per door device on the door, and the
/// cutouts of accessories and devices whose parts have one (`Cutout: 177 x 177`, fans,
/// cable glands).
pub fn openings(e: &Enclosure, parts: &[Part], face: Face) -> Vec<Opening> {
    let mut v = Vec::new();
    for it in e.items.iter().filter(|i| i.face == face) {
        let part = || parts.iter().chain(crate::parts::catalog()).find(|p| p.number == it.part);
        match it.kind {
            ItemKind::Cutout => v.push(Opening { item: it.id, center: it.center(), w: it.w, h: it.h, round: it.round }),
            ItemKind::Device if face == Face::Door => {
                let (round, w, h) = door_cutout(part(), it);
                v.push(Opening { item: it.id, center: it.center(), w, h, round });
            }
            ItemKind::Device | ItemKind::Accessory => {
                if let Some((round, w, h)) = part().and_then(|p| p.data_value("Cutout")).and_then(|v| parse_cutout(&v)) {
                    // Quarter-turned items turn their cutout.
                    let (w, h) = if (it.w - it.h).abs() > 0.5 && part().is_some_and(|p| (p.width - it.h).abs() < 0.5) { (h, w) } else { (w, h) };
                    v.push(Opening { item: it.id, center: it.center(), w, h, round });
                }
            }
            _ => {}
        }
    }
    v
}

// ---------------------------------------------------------------------- NC export

fn csv_num(v: f32) -> String {
    format!("{v:.1}")
}

/// Drilling and cutout list of a face (mounting plate, door, wall) for NC machines:
/// coordinates from the bottom left corner, y up.
pub fn nc_csv(prj: &Project, enc: usize, face: Face) -> String {
    let e = &prj.layout.enclosures[enc];
    let (_, fh) = e.face_size(face);
    let name = |id: u64| e.item(id).map(|i| if i.kind == ItemKind::Device { i.tag.clone() } else { i.kind.title().to_string() }).unwrap_or_default();
    let mut out = String::from("Enclosure;Face;Item;Operation;X;Y;Diameter;Thread;Width;Height\n");
    for h in holes_on(e, &prj.parts, face) {
        let op = h.fastening.operation();
        let thread = if h.fastening == Fastening::Tapped { &h.thread } else { &h.thread_size };
        let _ = writeln!(out, "{};{};{};{op};{};{};{};{};;", e.name, face.title(), name(h.item), csv_num(h.pos.x), csv_num(fh - h.pos.y), csv_num(h.diameter), thread);
    }
    for o in openings(e, &prj.parts, face) {
        let (op, dia) = if o.round { ("Cutout round", csv_num(o.w)) } else { ("Cutout rectangular", String::new()) };
        let _ = writeln!(out, "{};{};{};{op};{};{};{dia};;{};{}", e.name, face.title(), name(o.item), csv_num(o.center.x), csv_num(fh - o.center.y), csv_num(o.w), csv_num(o.h));
    }
    out
}

/// A face as DXF for NC machines: outline, holes on one layer per diameter
/// (`HOLE_D4.2`), thread marks, weld studs (`STUD_M5`), cutouts; millimetres, origin
/// bottom left.
pub fn nc_dxf(prj: &Project, enc: usize, face: Face) -> String {
    let e = &prj.layout.enclosures[enc];
    let (fw, fh) = e.face_size(face);
    let all = holes_on(e, &prj.parts, face);
    let (studs, hs): (Vec<Hole>, Vec<Hole>) = all.into_iter().partition(|h| h.fastening == Fastening::Stud);
    let mut diameters: Vec<f32> = Vec::new();
    for h in &hs {
        if !diameters.iter().any(|d| (d - h.diameter).abs() < 0.01) {
            diameters.push(h.diameter);
        }
    }
    let hole_layer = |d: f32| format!("HOLE_D{d:.1}");
    let stud_layer = |h: &Hole| format!("STUD_{}", h.thread_size);
    let outline = face.key().to_uppercase();
    let outline = outline.as_str();
    let mut layers: Vec<(String, u8)> = vec![(outline.into(), 7), ("CUTOUT".into(), 1), ("THREAD".into(), 3)];
    layers.extend(diameters.iter().map(|d| (hole_layer(*d), 5)));
    for h in &studs {
        if !layers.iter().any(|l| l.0 == stud_layer(h)) {
            layers.push((stud_layer(h), 6));
        }
    }
    let mut out = String::new();
    crate::dxf::header(&mut out, &layers);
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    let pt = |out: &mut String, code: u16, q: P| {
        let _ = write!(out, "{}\n{:.3}\n{}\n{:.3}\n{}\n0.0\n", code, q.x, code + 10, fh - q.y, code + 20);
    };
    let rect = |out: &mut String, layer: &str, r: R| {
        let _ = write!(out, "0\nPOLYLINE\n8\n{layer}\n66\n1\n70\n1\n10\n0.0\n20\n0.0\n30\n0.0\n");
        for q in [r.min, p(r.max.x, r.min.y), r.max, p(r.min.x, r.max.y)] {
            let _ = write!(out, "0\nVERTEX\n8\n{layer}\n");
            pt(out, 10, q);
        }
        let _ = write!(out, "0\nSEQEND\n8\n{layer}\n");
    };
    let circle = |out: &mut String, layer: &str, c: P, r: f32| {
        let _ = write!(out, "0\nCIRCLE\n8\n{layer}\n");
        pt(out, 10, c);
        let _ = write!(out, "40\n{r:.3}\n");
    };
    rect(&mut out, outline, R::from_points(p(0.0, 0.0), p(fw, fh)));
    for h in &hs {
        circle(&mut out, &hole_layer(h.diameter), h.pos, h.diameter / 2.0);
        if !h.thread.is_empty() {
            // Thread marking: the nominal diameter on its own layer.
            let nominal: f32 = h.thread.trim_start_matches('M').parse().unwrap_or(h.diameter);
            circle(&mut out, "THREAD", h.pos, nominal / 2.0);
        }
    }
    for h in &studs {
        circle(&mut out, &stud_layer(h), h.pos, h.diameter / 2.0);
    }
    for o in openings(e, &prj.parts, face) {
        if o.round {
            circle(&mut out, "CUTOUT", o.center, o.w / 2.0);
        } else {
            let half = p(o.w / 2.0, o.h / 2.0);
            rect(&mut out, "CUTOUT", R::from_points(o.center - half, o.center + half));
        }
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    out
}

// ---------------------------------------------------------------------- wire ends

/// Cross-section in mm² from texts like "1.5 mm²", "2,5" or "0.75mm2".
pub fn cross_section_mm2(s: &str) -> Option<f32> {
    let num: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    let v: f32 = num.replace(',', ".").parse().ok()?;
    (v > 0.0).then_some(v)
}

/// Wire end ferrule (DIN 46228-4 colour code).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ferrule {
    pub cross_section: f32,
    pub color: &'static str,
    /// Sleeve length = stripping length.
    pub length: f32,
    pub part: &'static str,
}

pub const FERRULES: [Ferrule; 10] = [
    Ferrule { cross_section: 0.34, color: "turquoise", length: 8.0, part: "GEN.FER-0.34" },
    Ferrule { cross_section: 0.5, color: "white", length: 8.0, part: "GEN.FER-0.5" },
    Ferrule { cross_section: 0.75, color: "grey", length: 8.0, part: "GEN.FER-0.75" },
    Ferrule { cross_section: 1.0, color: "red", length: 8.0, part: "GEN.FER-1.0" },
    Ferrule { cross_section: 1.5, color: "black", length: 8.0, part: "GEN.FER-1.5" },
    Ferrule { cross_section: 2.5, color: "blue", length: 8.0, part: "GEN.FER-2.5" },
    Ferrule { cross_section: 4.0, color: "grey", length: 10.0, part: "GEN.FER-4" },
    Ferrule { cross_section: 6.0, color: "yellow", length: 12.0, part: "GEN.FER-6" },
    Ferrule { cross_section: 10.0, color: "red", length: 12.0, part: "GEN.FER-10" },
    Ferrule { cross_section: 16.0, color: "blue", length: 12.0, part: "GEN.FER-16" },
];

/// End treatment of a wire end: stripping length and ferrule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndTreatment {
    pub strip: f32,
    pub ferrule: Option<Ferrule>,
}

/// End treatment for a wire cross-section (None if it is unknown).
pub fn end_treatment(cross_section: &str, ferrules: bool) -> Option<EndTreatment> {
    let cs = cross_section_mm2(cross_section)?;
    let f = FERRULES.iter().find(|f| (f.cross_section - cs).abs() < 0.01).copied();
    match (ferrules, f) {
        (true, Some(f)) => Some(EndTreatment { strip: f.length, ferrule: Some(f) }),
        _ => Some(EndTreatment { strip: if cs <= 2.5 { 8.0 } else if cs <= 6.0 { 10.0 } else { 12.0 }, ferrule: None }),
    }
}

/// Ferrules needed for the routed wires of the panel: (part, count).
pub fn ferrule_material(prj: &Project, wiring: &[crate::panel::RoutedWire]) -> Vec<(&'static str, usize)> {
    let mut v: Vec<(&'static str, usize)> = Vec::new();
    if !prj.layout.ferrules {
        return v;
    }
    for w in wiring {
        if let Some(f) = end_treatment(&w.cross_section, true).and_then(|t| t.ferrule) {
            match v.iter_mut().find(|x| x.0 == f.part) {
                Some(x) => x.1 += 2,
                None => v.push((f.part, 2)),
            }
        }
    }
    v
}

#[cfg(test)]
mod tests;
