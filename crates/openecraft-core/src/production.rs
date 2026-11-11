//! Production data of the panel layout (in the spirit of EPLAN Pro Panel): drilling
//! pattern and cutouts of the mounting plate, the door and the walls with NC export
//! (DXF / CSV), wire end treatment (stripping length, ferrules). The clearance checks are in
//! [`crate::panel::validate`].

use crate::geom::{p, P, R};
use crate::model::{natural_cmp, Project};
use crate::panel::{Enclosure, Face, Fastening, ItemKind, PanelItem};
use crate::parts::vec3::{add, cross, dot, norm, scale, sub, Tri, V3};
use crate::parts::Part;
pub use crate::model::panel::{Termination, WireTermination};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use std::collections::{HashMap, HashSet};
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
                    let inferred=if (pd.width - pd.height).abs() > 0.5 && (it.w - pd.height).abs() < 0.5 && (it.h - pd.width).abs() < 0.5 {crate::panel::PanelRotation::Deg90} else {crate::panel::PanelRotation::Deg0};
                    let rotation=it.rotation.unwrap_or(inferred);
                    pts.into_iter()
                        .map(|q| {let [x,y]=rotation.map([q.x,q.y],[pd.width,pd.height]);p(it.x+x,it.y+y)})
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

impl Ferrule {
    /// Colour of its plastic collar.
    pub fn rgb(&self) -> [u8; 3] {
        match self.color {
            "turquoise" => [64, 200, 200],
            "white" => [235, 235, 235],
            "grey" => [140, 140, 145],
            "red" => [210, 40, 40],
            "black" => [35, 35, 38],
            "blue" => [40, 90, 200],
            "yellow" => [235, 200, 40],
            _ => [128, 128, 128],
        }
    }

    /// Length of its plastic collar over the insulation (DIN 46228-4).
    pub fn collar(&self) -> f32 {
        if self.cross_section <= 4.0 { 6.0 } else if self.cross_section <= 6.0 { 8.0 } else { 10.0 }
    }
}

impl Termination {
    pub const ALL: [Termination; 8] =
        [Termination::Bare, Termination::Ferrule, Termination::TwinFerrule, Termination::Ring, Termination::Fork, Termination::Pin, Termination::Blade, Termination::Receptacle];

    /// Key in the project file and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Termination::Bare => "bare",
            Termination::Ferrule => "ferrule",
            Termination::TwinFerrule => "twin_ferrule",
            Termination::Ring => "ring",
            Termination::Fork => "fork",
            Termination::Pin => "pin",
            Termination::Blade => "blade",
            Termination::Receptacle => "receptacle",
        }
    }

    pub fn from_key(key: &str) -> Option<Termination> {
        let key = key.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Termination::ALL.into_iter().find(|t| t.key() == key)
    }

    /// Name, translated.
    pub fn title(self) -> &'static str {
        tr(match self {
            Termination::Bare => tr_const!("Bare conductor"),
            Termination::Ferrule => tr_const!("Ferrule"),
            Termination::TwinFerrule => tr_const!("Twin ferrule"),
            Termination::Ring => tr_const!("Ring terminal"),
            Termination::Fork => tr_const!("Fork terminal"),
            Termination::Pin => tr_const!("Pin terminal"),
            Termination::Blade => tr_const!("Flat plug"),
            Termination::Receptacle => tr_const!("Flat receptacle"),
        })
    }
}

/// Insulation of a crimp terminal, coloured by the cross-sections it takes: red up to
/// 1 mm², blue 1.5 to 2.5 mm², yellow 4 to 6 mm²; uninsulated from 10 mm².
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Band {
    Red,
    Blue,
    Yellow,
    Uninsulated,
}

/// A crimped terminal (ring, fork, pin, flat plug or receptacle) for a cross-section.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crimp {
    pub kind: Termination,
    pub band: Band,
    pub cross_section: f32,
}

impl Crimp {
    /// The terminal of `kind` for `cross_section` (mm²); forks, flat plugs and
    /// receptacles only up to 6 mm².
    pub fn for_cross_section(kind: Termination, cross_section: f32) -> Option<Crimp> {
        let band = match cross_section {
            c if c <= 1.0 => Band::Red,
            c if c <= 2.5 => Band::Blue,
            c if c <= 6.0 => Band::Yellow,
            _ => Band::Uninsulated,
        };
        let fits = matches!(kind, Termination::Ring | Termination::Pin) || (matches!(kind, Termination::Fork | Termination::Blade | Termination::Receptacle) && band != Band::Uninsulated);
        fits.then_some(Crimp { kind, band, cross_section })
    }

    /// Colour of the insulation ("" uninsulated).
    pub fn color(&self) -> &'static str {
        match self.band {
            Band::Red => "red",
            Band::Blue => "blue",
            Band::Yellow => "yellow",
            Band::Uninsulated => "",
        }
    }

    pub fn rgb(&self) -> [u8; 3] {
        match self.band {
            Band::Red => [210, 40, 40],
            Band::Blue => [40, 90, 200],
            Band::Yellow => [235, 200, 40],
            Band::Uninsulated => SLEEVE_RGB,
        }
    }

    /// Generic part number: the kind and the colour, uninsulated ones the cross-section.
    pub fn part(&self) -> String {
        let kind = match (self.kind, self.band) {
            (Termination::Ring, Band::Uninsulated) => "LUG",
            (Termination::Ring, _) => "RING",
            (Termination::Fork, _) => "FORK",
            (Termination::Blade, _) => "TAB",
            (Termination::Receptacle, _) => "REC",
            _ => "PIN",
        };
        match self.band {
            Band::Red => format!("GEN.{kind}-R"),
            Band::Blue => format!("GEN.{kind}-B"),
            Band::Yellow => format!("GEN.{kind}-Y"),
            Band::Uninsulated => format!("GEN.{kind}-{}", self.cross_section),
        }
    }

    /// Length of the crimp barrel, mm.
    pub fn barrel(&self) -> f32 {
        match self.band {
            Band::Red => 5.0,
            Band::Blue => 6.0,
            Band::Yellow => 7.0,
            Band::Uninsulated => 8.0 + 2.0 * self.cross_section.sqrt(),
        }
    }

    /// Hole of a ring or fork terminal, mm: M4, M5, M6 or M8.
    pub fn hole(&self) -> f32 {
        match self.band {
            Band::Red | Band::Blue => 4.3,
            Band::Yellow => 5.3,
            Band::Uninsulated if self.cross_section <= 16.0 => 6.4,
            Band::Uninsulated => 8.4,
        }
    }
}

/// End treatment of a wire end: how it is terminated, its stripping length, the
/// ferrule or the crimped terminal.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndTreatment {
    pub kind: Termination,
    pub strip: f32,
    pub ferrule: Option<Ferrule>,
    pub crimp: Option<Crimp>,
    /// The twin ferrule is the one of the other wire ([`Twin::first`]): it is listed,
    /// counted and shown there.
    pub shared: bool,
}

/// The shape of an [`EndPiece`]: round along the wire, or flat (lying across the
/// direction it is seen from best, see [`EndPiece::mesh`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EndShape {
    Round,
    /// A plate (flat plug, receptacle).
    Flat { width: f32, thickness: f32 },
    /// The ring of a ring terminal round the connection point.
    Ring { outer: f32, hole: f32, thickness: f32 },
    /// The open ring of a fork terminal.
    Fork { width: f32, slot: f32, thickness: f32 },
}

/// A piece of a wire end as it is seen (3D view): length along the wire and diameter in
/// mm, colour and shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EndPiece {
    pub length: f32,
    pub diameter: f32,
    pub color: [u8; 3],
    pub shape: EndShape,
}

/// Tin-plated copper of a ferrule sleeve or a terminal.
pub const SLEEVE_RGB: [u8; 3] = [196, 198, 204];
/// Bare copper of a stripped conductor.
pub const COPPER_RGB: [u8; 3] = [196, 112, 52];

/// Diameter (mm) of the stranded conductor of a cross-section (mm²).
pub fn conductor_diameter(cross_section: f32) -> f32 {
    1.15 * 2.0 * (cross_section / std::f32::consts::PI).sqrt()
}

impl EndTreatment {
    /// Part number of the ferrule or terminal; empty for the bare conductor.
    pub fn part(&self) -> String {
        match (self.kind, self.ferrule, self.crimp) {
            (Termination::TwinFerrule, Some(f), _) => f.part.replace("GEN.FER-", "GEN.FER2-"),
            (_, Some(f), _) => f.part.to_string(),
            (_, _, Some(c)) => c.part(),
            _ => String::new(),
        }
    }

    /// What the end gets, translated: "Ferrule 1.5 mm² black", "Ring terminal red", …
    pub fn describe(&self) -> String {
        match (self.kind, self.ferrule, self.crimp) {
            (Termination::TwinFerrule, Some(f), _) => tr_fmt!("Twin ferrule {} mm² {}", f.cross_section, tr(f.color)),
            (_, Some(f), _) => tr_fmt!("Ferrule {} mm² {}", f.cross_section, tr(f.color)),
            (_, _, Some(c)) if c.band == Band::Uninsulated => self.kind.title().to_string(),
            (_, _, Some(c)) => format!("{} {}", self.kind.title(), tr(c.color())),
            (Termination::Bare, ..) => self.kind.title().to_string(),
            _ => tr_fmt!("{}: none for this cross-section", self.kind.title()),
        }
    }

    /// How the end of a wire of `cross_section` (with the outer diameter `outer`) looks,
    /// from the tip inwards: the metal sleeve of a ferrule and its collar over the
    /// insulation, the ring, fork, pin or plate of a terminal and its insulated sleeve
    /// (its barrel when uninsulated), or the bare copper conductor as long as it is
    /// stripped.
    pub fn pieces(&self, cross_section: &str, outer: f32) -> Vec<EndPiece> {
        let core = conductor_diameter(cross_section_mm2(cross_section).unwrap_or(1.0)).min(outer * 0.8);
        let round = |length: f32, diameter: f32, color: [u8; 3]| EndPiece { length, diameter, color, shape: EndShape::Round };
        if self.shared {
            // Only its conductor, inside the twin ferrule of the other wire.
            return vec![round(self.strip, core, COPPER_RGB)];
        }
        if let Some(f) = self.ferrule {
            return match self.kind {
                Termination::TwinFerrule => vec![round(f.length, core * 1.5 + 0.3, SLEEVE_RGB), round(f.collar() * 1.4, outer * 1.7, f.rgb())],
                _ => vec![round(f.length, core + 0.3, SLEEVE_RGB), round(f.collar(), outer + 0.8, f.rgb())],
            };
        }
        let Some(c) = self.crimp else { return vec![round(self.strip, core, COPPER_RGB)] };
        let insulated = c.band != Band::Uninsulated;
        let thickness = if insulated { 0.8 } else { 1.5 };
        // The ring and the fork lie round the connection point at the tip.
        let eye = c.hole() * 1.9;
        let neck = 2.5;
        let tongue = match c.kind {
            Termination::Ring => EndPiece { length: eye / 2.0 + neck, diameter: eye, color: SLEEVE_RGB, shape: EndShape::Ring { outer: eye, hole: c.hole(), thickness } },
            Termination::Fork => EndPiece { length: eye / 2.0 + neck, diameter: eye, color: SLEEVE_RGB, shape: EndShape::Fork { width: eye, slot: c.hole(), thickness } },
            Termination::Blade => EndPiece { length: 8.0, diameter: 6.3, color: SLEEVE_RGB, shape: EndShape::Flat { width: 6.3, thickness: 0.8 } },
            Termination::Receptacle => EndPiece { length: 9.0, diameter: 7.4, color: SLEEVE_RGB, shape: EndShape::Flat { width: 7.4, thickness: 3.0 } },
            _ => {
                let pin = match c.band {
                    Band::Red | Band::Blue => 1.9,
                    Band::Yellow => 2.8,
                    Band::Uninsulated => core * 0.8,
                };
                round(if insulated { 10.0 } else { 16.0 }, pin, SLEEVE_RGB)
            }
        };
        let sleeve = if insulated { round(c.barrel() + 3.0, outer + 1.6, c.rgb()) } else { round(c.barrel(), core + 1.6, SLEEVE_RGB) };
        vec![tongue, sleeve]
    }
}

impl EndPiece {
    /// Triangles of a flat piece (none for a round one) with its tip at `tip`, running
    /// along `dir` (a unit vector, into the wire) and lying flat across `up` (a unit
    /// vector square to `dir`): the ring and the fork round the tip, a plate from it.
    pub fn mesh(&self, tip: V3, dir: V3, up: V3) -> Vec<Tri> {
        let side = cross(dir, up);
        let at = |along: f32, across: f32| add(add(tip, scale(dir, along)), scale(side, across));
        // A plate from `from` to `to` along the wire, `width` wide, round `across`.
        let plate = |from: f32, to: f32, across: f32, width: f32, thickness: f32| slab(at(from, across), at(to, across), side, width / 2.0, up, thickness / 2.0);
        match self.shape {
            EndShape::Round => Vec::new(),
            EndShape::Flat { width, thickness } => plate(0.0, self.length, 0.0, width, thickness),
            EndShape::Ring { outer, hole, thickness } => {
                let mut v = annulus(tip, dir, side, up, outer / 2.0, hole / 2.0, thickness / 2.0, 0.0, std::f32::consts::TAU);
                v.extend(plate(outer * 0.3, self.length, 0.0, outer * 0.45, thickness));
                v
            }
            EndShape::Fork { width, slot, thickness } => {
                // The closed half towards the wire, the prongs open away from it.
                let (r, prong) = (width / 2.0, (width - slot) / 2.0);
                let half = std::f32::consts::PI;
                let mut v = annulus(tip, dir, side, up, r, slot / 2.0, thickness / 2.0, -half / 2.0, half / 2.0);
                for s in [-1.0, 1.0] {
                    v.extend(plate(-r, 0.0, s * (slot / 2.0 + prong / 2.0), prong, thickness));
                }
                v.extend(plate(r * 0.6, self.length, 0.0, width * 0.45, thickness));
                v
            }
        }
    }
}

/// Two triangles of a quad, wound to face `out`.
fn quad(q: [V3; 4], out: V3) -> [Tri; 2] {
    let n = cross(sub(q[1], q[0]), sub(q[2], q[0]));
    if dot(n, out) >= 0.0 {
        [[q[0], q[1], q[2]], [q[0], q[2], q[3]]]
    } else {
        [[q[0], q[2], q[1]], [q[0], q[3], q[2]]]
    }
}

/// A box from `a` to `b`, `half_w` to both sides along `side` and `half_t` along `up`.
fn slab(a: V3, b: V3, side: V3, half_w: f32, up: V3, half_t: f32) -> Vec<Tri> {
    let c = |p: V3, s: f32, u: f32| add(add(p, scale(side, s * half_w)), scale(up, u * half_t));
    let dir = norm(sub(b, a));
    let faces = [
        ([c(a, -1.0, 1.0), c(b, -1.0, 1.0), c(b, 1.0, 1.0), c(a, 1.0, 1.0)], up),
        ([c(a, -1.0, -1.0), c(b, -1.0, -1.0), c(b, 1.0, -1.0), c(a, 1.0, -1.0)], scale(up, -1.0)),
        ([c(a, 1.0, -1.0), c(b, 1.0, -1.0), c(b, 1.0, 1.0), c(a, 1.0, 1.0)], side),
        ([c(a, -1.0, -1.0), c(b, -1.0, -1.0), c(b, -1.0, 1.0), c(a, -1.0, 1.0)], scale(side, -1.0)),
        ([c(a, -1.0, -1.0), c(a, 1.0, -1.0), c(a, 1.0, 1.0), c(a, -1.0, 1.0)], scale(dir, -1.0)),
        ([c(b, -1.0, -1.0), c(b, 1.0, -1.0), c(b, 1.0, 1.0), c(b, -1.0, 1.0)], dir),
    ];
    faces.into_iter().flat_map(|(q, out)| quad(q, out)).collect()
}

/// A flat ring round `centre` (in the plane of `dir` and `side`, `half_t` thick along
/// `up`) from the angle `from` to `to`, measured from `dir` towards `side`.
#[allow(clippy::too_many_arguments)]
fn annulus(centre: V3, dir: V3, side: V3, up: V3, outer: f32, inner: f32, half_t: f32, from: f32, to: f32) -> Vec<Tri> {
    const STEPS: usize = 16;
    let radial = |a: f32| add(scale(dir, a.cos()), scale(side, a.sin()));
    let at = |a: f32, r: f32, u: f32| add(add(centre, scale(radial(a), r)), scale(up, u * half_t));
    let mut v = Vec::new();
    for k in 0..STEPS {
        let (a0, a1) = (from + (to - from) * k as f32 / STEPS as f32, from + (to - from) * (k + 1) as f32 / STEPS as f32);
        let mid = (a0 + a1) / 2.0;
        for u in [1.0, -1.0] {
            v.extend(quad([at(a0, inner, u), at(a0, outer, u), at(a1, outer, u), at(a1, inner, u)], scale(up, u)));
        }
        v.extend(quad([at(a0, outer, -1.0), at(a1, outer, -1.0), at(a1, outer, 1.0), at(a0, outer, 1.0)], radial(mid)));
        v.extend(quad([at(a0, inner, -1.0), at(a1, inner, -1.0), at(a1, inner, 1.0), at(a0, inner, 1.0)], scale(radial(mid), -1.0)));
    }
    v
}

/// End treatment for a wire cross-section (None if it is unknown): a ferrule where the
/// wire ends get them (`ferrules`), otherwise the bare conductor.
pub fn end_treatment(cross_section: &str, ferrules: bool) -> Option<EndTreatment> {
    treatment(cross_section, if ferrules { Termination::Ferrule } else { Termination::Bare })
}

/// End treatment of a wire of `cross_section` terminated as `kind` (None for an unknown
/// cross-section); a cross-section without such a ferrule is stripped bare.
pub fn treatment(cross_section: &str, kind: Termination) -> Option<EndTreatment> {
    let cs = cross_section_mm2(cross_section)?;
    let bare = EndTreatment { kind: Termination::Bare, strip: if cs <= 2.5 { 8.0 } else if cs <= 6.0 { 10.0 } else { 12.0 }, ferrule: None, crimp: None, shared: false };
    Some(match kind {
        Termination::Bare => bare,
        Termination::Ferrule | Termination::TwinFerrule => match FERRULES.iter().find(|f| (f.cross_section - cs).abs() < 0.01) {
            Some(f) => EndTreatment { kind, strip: f.length, ferrule: Some(*f), crimp: None, shared: false },
            None => bare,
        },
        _ => {
            let crimp = Crimp::for_cross_section(kind, cs);
            EndTreatment { kind, strip: crimp.map(|c| c.barrel() + 1.0).unwrap_or(bare.strip), ferrule: None, crimp, shared: false }
        }
    })
}

/// The device of a connection point designation (`-K1:13` → `-K1`).
fn device_of(at: &str) -> &str {
    at.rsplit_once(':').map(|(d, _)| d).unwrap_or(at)
}

/// The termination chosen for the end `at` of the wire from `at` to `to`: for that
/// wire, for every wire at `at`, or for every wire at its device.
pub fn termination_of(prj: &Project, at: &str, to: &str) -> Option<Termination> {
    let chosen = &prj.layout.terminations;
    chosen
        .iter()
        .find(|t| t.at == at && !t.to.is_empty() && t.to == to)
        .or_else(|| chosen.iter().find(|t| t.at == at && t.to.is_empty()))
        .or_else(|| chosen.iter().find(|t| t.at == device_of(at) && t.to.is_empty()))
        .map(|t| t.kind)
}

/// End treatment of the end `at` of the wire of `cross_section` from `at` to `to`: as
/// chosen ([`termination_of`]), otherwise ferrules or the bare conductor as the layout
/// sets; without the other wires at `at` (twin ferrules are not paired, see [`Ends`]).
pub fn end_at(prj: &Project, at: &str, to: &str, cross_section: &str) -> Option<EndTreatment> {
    match termination_of(prj, at, to) {
        Some(kind) => treatment(cross_section, kind),
        None => end_treatment(cross_section, prj.layout.ferrules),
    }
}

/// Chooses the termination of the wire ends at `at` (a connection point or a device),
/// only of the wire to `to` where it is given; `None` goes back to the layout setting.
pub fn set_termination(prj: &mut Project, at: &str, to: &str, kind: Option<Termination>) {
    let (at, to) = (at.trim(), to.trim());
    prj.layout.terminations.retain(|t| !(t.at == at && t.to == to));
    if let Some(kind) = kind {
        prj.layout.terminations.push(WireTermination { at: at.into(), to: to.into(), kind });
    }
}

/// A twin ferrule the wire end shares with another wire at its connection point
/// ([`Ends::twin`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Twin {
    /// The far end of the other wire.
    pub with: String,
    /// Cross-section of the other wire.
    pub cross_section: String,
    /// This end carries the ferrule (it is listed, counted and shown here); the other
    /// one shares it.
    pub first: bool,
}

/// The wires ending at each connection point, to tell which of them share a twin
/// ferrule: the termination of a wire end with all it depends on.
#[derive(Clone, Debug, Default)]
pub struct Ends {
    /// Connection point → (far end, cross-section) of each wire ending there.
    at: HashMap<String, Vec<(String, String)>>,
}

impl Ends {
    /// The ends of the wires (from, to, cross-section).
    pub fn of(wires: impl IntoIterator<Item = (String, String, String)>) -> Ends {
        let mut at: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (from, to, cs) in wires {
            if from == to {
                continue;
            }
            at.entry(from.clone()).or_default().push((to.clone(), cs.clone()));
            at.entry(to).or_default().push((from, cs));
        }
        for v in at.values_mut() {
            v.sort_by(|a, b| natural_cmp(&a.0, &b.0));
        }
        Ends { at }
    }

    /// The ends of the connections of a project, cable cores included.
    pub fn new(prj: &Project, conns: &[crate::netlist::Connection]) -> Ends {
        Ends::of(conns.iter().map(|c| (crate::netlist::pin_designation(prj, c.a), crate::netlist::pin_designation(prj, c.b), c.cross_section.clone())))
    }

    /// The far ends and cross-sections of the wires at the connection point `at`.
    pub fn wires_at(&self, at: &str) -> &[(String, String)] {
        self.at.get(at).map_or(&[], Vec::as_slice)
    }

    /// The connection points with wires, in no particular order.
    pub fn points(&self) -> impl Iterator<Item = &str> {
        self.at.keys().map(String::as_str)
    }

    /// Termination of the end `at` of the wire to `to` if it had none chosen for this
    /// wire: as chosen for the connection point or its device, otherwise a twin ferrule
    /// where the layout sets them and exactly two wires end at `at` (the other one
    /// without a choice), otherwise ferrules or the bare conductor as the layout sets.
    pub fn default_kind(&self, prj: &Project, at: &str, to: &str) -> Termination {
        if let Some(kind) = termination_of(prj, at, "") {
            return kind;
        }
        let wires = self.wires_at(at);
        if !prj.layout.ferrules {
            Termination::Bare
        } else if prj.layout.twin_ferrules && wires.len() == 2 && wires.iter().filter(|(o, _)| o != to).all(|(o, _)| termination_of(prj, at, o).is_none()) {
            Termination::TwinFerrule
        } else {
            Termination::Ferrule
        }
    }

    /// Termination of the end `at` of the wire to `to`: as chosen
    /// ([`termination_of`]), otherwise [`Ends::default_kind`].
    pub fn kind(&self, prj: &Project, at: &str, to: &str) -> Termination {
        termination_of(prj, at, to).unwrap_or_else(|| self.default_kind(prj, at, to))
    }

    /// The other wire sharing the twin ferrule of the end `at` of the wire to `to`: the
    /// wire ends at `at` with twin ferrules pair up in the order of their far ends.
    pub fn twin(&self, prj: &Project, at: &str, to: &str) -> Option<Twin> {
        let twins: Vec<&(String, String)> = self.wires_at(at).iter().filter(|(o, _)| self.kind(prj, at, o) == Termination::TwinFerrule).collect();
        let i = twins.iter().position(|(o, _)| o == to)?;
        let (with, cross_section) = twins.get(i ^ 1)?;
        Some(Twin { with: with.clone(), cross_section: cross_section.clone(), first: i % 2 == 0 })
    }

    /// End treatment of the end `at` of the wire of `cross_section` to `to`; a shared
    /// twin ferrule is the one for the larger of the two cross-sections.
    pub fn end(&self, prj: &Project, at: &str, to: &str, cross_section: &str) -> Option<EndTreatment> {
        let kind = self.kind(prj, at, to);
        let twin = if kind == Termination::TwinFerrule { self.twin(prj, at, to) } else { None };
        let Some(t) = twin else { return treatment(cross_section, kind) };
        let mm2 = |cs: &str| cross_section_mm2(cs).unwrap_or(0.0);
        let larger = if mm2(&t.cross_section) > mm2(cross_section) { t.cross_section.as_str() } else { cross_section };
        treatment(larger, kind).map(|e| EndTreatment { shared: !t.first, ..e })
    }
}

/// Chooses the termination of the end `at` of the wire to `to` (`to` empty: of all
/// wires at `at`, see [`set_termination`]). A twin ferrule chosen for one wire is
/// chosen for the other wire at `at` too, where there is exactly one; choosing
/// something else for a wire with a twin ferrule takes it from the other one as well.
pub fn choose_termination(prj: &mut Project, ends: &Ends, at: &str, to: &str, kind: Option<Termination>) {
    let (at, to) = (at.trim(), to.trim());
    if !to.is_empty() {
        let others: Vec<String> = ends.wires_at(at).iter().filter(|(o, _)| o != to).map(|(o, _)| o.clone()).collect();
        let was_twin = ends.kind(prj, at, to) == Termination::TwinFerrule;
        if let [other] = others.as_slice() {
            if kind == Some(Termination::TwinFerrule) {
                set_termination(prj, at, other, kind);
            } else if was_twin && termination_of(prj, at, other) == Some(Termination::TwinFerrule) {
                set_termination(prj, at, other, None);
            }
        }
    }
    set_termination(prj, at, to, kind);
}

/// Ferrules and terminals needed for the routed wires of the panel: (part, count); a
/// twin ferrule is counted once for its two wires.
pub fn terminal_material(prj: &Project, ends: &Ends, wiring: &[crate::panel::RoutedWire]) -> Vec<(String, usize)> {
    let mut parts: Vec<(String, usize)> = Vec::new();
    // The twin ferrules counted: (connection point, far ends of both wires).
    let mut twins: HashSet<(String, String, String)> = HashSet::new();
    for w in wiring {
        for (at, to) in [(&w.from, &w.to), (&w.to, &w.from)] {
            let Some(t) = ends.end(prj, at, to, &w.cross_section) else { continue };
            let part = t.part();
            if part.is_empty() {
                continue;
            }
            if let Some(tw) = ends.twin(prj, at, to).filter(|_| t.kind == Termination::TwinFerrule) {
                let pair = if natural_cmp(to, &tw.with).is_le() { (to.clone(), tw.with) } else { (tw.with, to.clone()) };
                if !twins.insert((at.clone(), pair.0, pair.1)) {
                    continue;
                }
            }
            match parts.iter_mut().find(|x| x.0 == part) {
                Some(x) => x.1 += 1,
                None => parts.push((part, 1)),
            }
        }
    }
    parts
}

#[cfg(test)]
mod tests;
