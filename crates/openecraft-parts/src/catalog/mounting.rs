//! Generated 2D front views and 3D models of the mounting material of the example
//! catalog: rails, ducts, busbar systems, enclosures and their components,
//! accessories. Coordinates of the boxes as in 3D (y up from the bottom); the front
//! view is drawn with y down.

use crate::{Mesh3d, Model2d, Part, Prim2d};

/// A model being drawn: boxes in 3D, outlines in the front view.
struct Draft {
    h: f32,
    m3: Mesh3d,
    prims: Vec<Prim2d>,
}

impl Draft {
    fn new(w: f32, h: f32) -> Draft {
        Draft { h, m3: Mesh3d::default(), prims: vec![Prim2d::Rect { min: [0.0, 0.0], max: [w, h], fill: false }] }
    }

    /// A box in 3D only.
    fn solid(&mut self, min: [f32; 3], max: [f32; 3]) {
        self.m3.add_box(min, max);
    }

    /// A rectangle of the front view.
    fn face(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, fill: bool) {
        self.prims.push(Prim2d::Rect { min: [x0, self.h - y1], max: [x1, self.h - y0], fill });
    }

    /// A box with its outline in the front view.
    fn block(&mut self, min: [f32; 3], max: [f32; 3]) {
        self.solid(min, max);
        self.face(min[0], min[1], max[0], max[1], false);
    }

    fn circle(&mut self, x: f32, y: f32, r: f32, fill: bool) {
        self.prims.push(Prim2d::Circle { c: [x, self.h - y], r, fill });
    }

    fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32) {
        self.prims.push(Prim2d::Line { a: [x0, self.h - y0], b: [x1, self.h - y1] });
    }

    fn done(self, w: f32) -> (Model2d, Mesh3d) {
        (Model2d { width: w, height: self.h, prims: self.prims }, self.m3)
    }
}

/// How far behind its face the model of a cable gland goes on inside the enclosure
/// (its locknut): where the wall lies, as thick as it is where the gland is placed.
pub const GLAND_GAP: f32 = 0.1;

/// Nickel-plated brass and light grey polyamide (RAL 7035) of cable glands.
const NICKEL: [u8; 3] = [178, 186, 198];
const POLYAMIDE: [u8; 3] = [204, 207, 205];

/// Positions of `n` evenly spaced points between `a` and `b` (the middle for one).
fn spread(a: f32, b: f32, n: usize) -> Vec<f32> {
    match n {
        0 => Vec::new(),
        1 => vec![(a + b) / 2.0],
        _ => (0..n).map(|i| a + (b - a) * i as f32 / (n - 1) as f32).collect(),
    }
}

/// Mounting holes `x,y; x,y` of the technical data (`Mounting holes: M5: 10,15; 190,15`).
fn holes(p: &Part) -> Vec<(f32, f32)> {
    let Some(line) = p.tech_data.lines().find_map(|l| l.strip_prefix("Mounting holes:")) else { return Vec::new() };
    let list = line.split_once(':').map_or(line, |(_, rest)| rest);
    list.split(';')
        .filter_map(|xy| {
            let (x, y) = xy.split_once(',')?;
            Some((x.trim().parse().ok()?, y.trim().parse().ok()?))
        })
        .collect()
}

/// The front view and 3D model of a mounting part with its dimensions; `None` without
/// dimensions.
pub(super) fn model(p: &Part) -> Option<(Model2d, Mesh3d)> {
    let (w, h, d) = (p.width, p.height, p.depth);
    if w <= 0.0 || h <= 0.0 || d <= 0.0 {
        return None;
    }
    let mut m = Draft::new(w, h);
    match p.subgroup.as_str() {
        "Mounting rail" => {
            // Top-hat profile: flanges in front, web at the back.
            let t = (d * 0.15).clamp(0.8, 1.5);
            let f = (h * 0.15).max(2.0);
            m.solid([0.0, 0.0, d - t], [w, f, d]);
            m.solid([0.0, h - f, d - t], [w, h, d]);
            m.solid([0.0, f - t, 0.0], [w, f, d]);
            m.solid([0.0, h - f, 0.0], [w, h - f + t, d]);
            m.solid([0.0, f - t, 0.0], [w, h - f + t, t]);
            m.line(0.0, f, w, f);
            m.line(0.0, h - f, w, h - f);
            if p.description.contains("perforated") {
                let (sw, sh) = (15.0f32.min(w / 4.0), (h - 2.0 * f) * 0.3);
                for x in spread(12.5, w - 12.5, ((w - 25.0) / 25.0).max(0.0) as usize + 1) {
                    m.face(x - sw / 2.0, h / 2.0 - sh / 2.0, x + sw / 2.0, h / 2.0 + sh / 2.0, false);
                }
            }
        }
        "Cable duct" => {
            // Base, slotted side walls and the cover; the front view shows the cover
            // and the fingers of the walls.
            let t = 2.0f32.min(h / 4.0).min(d / 4.0);
            m.solid([0.0, 0.0, 0.0], [w, h, t]);
            m.solid([0.0, 0.0, 0.0], [w, t, d]);
            m.solid([0.0, h - t, 0.0], [w, h, d]);
            m.solid([0.0, t, d - t], [w, h - t, d]);
            let rim = (h * 0.12).max(t);
            m.line(0.0, rim, w, rim);
            m.line(0.0, h - rim, w, h - rim);
            for x in spread(0.0, w, (w / 12.0) as usize + 1).into_iter().skip(1) {
                m.line(x, 0.0, x, rim);
                m.line(x, h - rim, x, h);
            }
        }
        "Busbar system" => {
            // Holders at both ends, three bars across.
            let hw = (w * 0.04).max(20.0);
            for x0 in [0.0, w - hw] {
                m.block([x0, 0.0, 0.0], [x0 + hw, h, d * 0.8]);
            }
            let bh = h * 0.12;
            for y in [h * 0.22, h * 0.5, h * 0.78] {
                m.block([0.0, y - bh / 2.0, d * 0.7], [w, y + bh / 2.0, d]);
            }
        }
        "Busbar holder" => {
            m.block([0.0, 0.0, 0.0], [w, h, d * 0.6]);
            let cw = w * 0.16;
            for x in [w / 6.0, w / 2.0, w * 5.0 / 6.0] {
                m.block([x - cw / 2.0, 0.0, d * 0.6], [x + cw / 2.0, h, d]);
            }
        }
        "Enclosure" => {
            // Body, door with hinges and handle (two doors from 1000 mm).
            let e = (w.min(h) * 0.015).max(2.0);
            m.solid([0.0, 0.0, 0.0], [w, h, d - 3.0]);
            let leaves: Vec<(f32, f32)> = if w >= 1000.0 { vec![(e, w / 2.0 - 1.0), (w / 2.0 + 1.0, w - e)] } else { vec![(e, w - e)] };
            let (hh, hw) = ((h * 0.12).clamp(20.0, 160.0), (w * 0.03).clamp(8.0, 30.0));
            for (i, &(x0, x1)) in leaves.iter().enumerate() {
                m.block([x0, e, d - 3.0], [x1, h - e, d - 1.0]);
                // Handle at the free edge (the middle of two doors), hinges at the other.
                let handle_right = i == 0;
                let (hx, gx) = if handle_right { (x1 - hw * 2.5, x0) } else { (x0 + hw * 1.5, x1 - hw * 0.6) };
                m.block([hx, h / 2.0 - hh / 2.0, d - 1.0], [hx + hw, h / 2.0 + hh / 2.0, d]);
                for y in spread(h * 0.15, h * 0.85, if h > 1200.0 { 3 } else { 2 }) {
                    m.block([gx, y - hh * 0.2, d - 1.0], [gx + hw * 0.6, y + hh * 0.2, d]);
                }
            }
        }
        "Gland plate" | "Side panel" | "Rear wall" => {
            // A plate with its rim and the fixing holes.
            m.solid([0.0, 0.0, 0.0], [w, h, d]);
            let rim = (w.min(h) * 0.06).clamp(8.0, 25.0);
            m.face(rim, rim, w - rim, h - rim, false);
            let r = (rim * 0.2).clamp(2.0, 4.0);
            let (nx, ny) = ((w / 300.0) as usize + 2, (h / 300.0) as usize + 2);
            for x in spread(rim / 2.0, w - rim / 2.0, nx) {
                m.circle(x, rim / 2.0, r, false);
                m.circle(x, h - rim / 2.0, r, false);
            }
            for y in spread(rim / 2.0, h - rim / 2.0, ny).into_iter().skip(1).take(ny.saturating_sub(2)) {
                m.circle(rim / 2.0, y, r, false);
                m.circle(w - rim / 2.0, y, r, false);
            }
        }
        "Plinth" => {
            // Corner pieces with trims front, back and at the sides.
            let c = (w.min(d) * 0.1).clamp(40.0, 80.0);
            for (x0, z0) in [(0.0, 0.0), (w - c, 0.0), (0.0, d - c), (w - c, d - c)] {
                m.solid([x0, 0.0, z0], [x0 + c, h, z0 + c]);
            }
            let t = 3.0;
            m.solid([c, h * 0.1, d - t], [w - c, h * 0.9, d]);
            m.solid([c, h * 0.1, 0.0], [w - c, h * 0.9, t]);
            m.solid([0.0, h * 0.1, c], [t, h * 0.9, d - c]);
            m.solid([w - t, h * 0.1, c], [w, h * 0.9, d - c]);
            m.face(0.0, 0.0, c, h, false);
            m.face(w - c, 0.0, w, h, false);
            m.face(c, h * 0.1, w - c, h * 0.9, false);
        }
        "Filter fan" | "Outlet filter" => {
            // Housing inside the enclosure, frame with louvres outside.
            let outside = (d * 0.25).min(25.0);
            let inset = w.min(h) * 0.07;
            m.solid([inset, inset, 0.0], [w - inset, h - inset, d - outside]);
            m.solid([0.0, 0.0, d - outside], [w, h, d]);
            let g = w.min(h) * 0.12;
            m.face(g, g, w - g, h - g, false);
            let n = ((h - 2.0 * g) / 8.0) as usize;
            for y in spread(g, h - g, n + 1).into_iter().skip(1).take(n.saturating_sub(1)) {
                m.line(g, y, w - g, y);
            }
            if p.subgroup == "Filter fan" {
                m.circle(w / 2.0, h / 2.0, w.min(h) * 0.3, false);
            }
        }
        "Cable gland" => {
            // Seen from outside (z 0 is the outermost point): the seal insert standing a
            // little out of the mouth of the dome (no face of it in one plane with one of
            // the dome: they would flicker), the dome of the cap nut, its ring, the
            // threaded neck and the hexagon body, against the face at z `Outside`; behind
            // it, [`GLAND_GAP`] further (the wall of the enclosure there, as thick as it
            // is), the locknut and the end of the thread inside. Brass nickel-plated or
            // polyamide (light grey, or black) by the material and colour of its
            // technical data.
            use super::models::{BLACK, DARK};
            let material = p.tech_value("Material").unwrap_or_default().to_lowercase();
            let metal = ["brass", "nickel", "steel", "metal"].iter().any(|m| material.contains(m)) || p.description.to_lowercase().contains("brass");
            let black = p.tech_value("Colour").is_some_and(|c| c.to_lowercase().contains("black") || c.contains("9005")) || p.description.to_lowercase().contains("black");
            let body = if metal { NICKEL } else if black { BLACK } else { POLYAMIDE };
            let (cx, cy, r) = (w / 2.0, h / 2.0, w.min(h) / 2.0);
            let m3 = &mut m.m3;
            let o = p.tech_value("Outside").and_then(|v| v.trim().parse::<f32>().ok()).filter(|o| *o > 0.0 && *o < d - GLAND_GAP).unwrap_or(d);
            m3.add_cylinder([cx, cy, 0.0], 2, r * 0.5, o * 0.08, 16, DARK);
            m3.add_frustum([cx, cy, o * 0.04], 2, r * 0.6, r, o * 0.21, 16, body);
            m3.add_cylinder([cx, cy, o * 0.25], 2, r, o * 0.2, 16, body);
            m3.add_cylinder([cx, cy, o * 0.45], 2, r * 0.8, o * 0.2, 16, if metal { NICKEL } else { DARK });
            m3.add_frustum([cx, cy, o * 0.65], 2, r, r, o * 0.35, 6, body);
            let inner = d - o - GLAND_GAP;
            if inner > 0.0 {
                let nut = o + GLAND_GAP;
                m3.add_frustum([cx, cy, nut], 2, r * 0.9, r * 0.9, inner * 0.6, 6, body);
                m3.add_cylinder([cx, cy, nut + inner * 0.6], 2, r * 0.62, inner * 0.4, 16, if metal { NICKEL } else { DARK });
            }
            let hex: Vec<[f32; 2]> = (0..6).map(|i| {
                let a = std::f32::consts::FRAC_PI_3 * i as f32;
                [cx + r * a.cos(), cy + r * a.sin()]
            }).collect();
            m.prims = vec![Prim2d::Polyline { pts: hex, closed: true, fill: false }];
            m.circle(cx, cy, r, false);
            m.circle(cx, cy, r * 0.6, false);
            m.circle(cx, cy, r * 0.3, false);
            // The outline of the bounding box keeps the size of the front view.
            m.prims.push(Prim2d::Rect { min: [0.0, 0.0], max: [w, h], fill: false });
        }
        "Enclosure light" => {
            m.block([0.0, 0.0, 0.0], [w, h, d * 0.6]);
            m.block([w * 0.04, h * 0.15, d * 0.6], [w * 0.96, h * 0.85, d]);
            m.circle(w * 0.02, h / 2.0, h * 0.12, true);
        }
        "PE busbar" => {
            // Insulators at the ends, the bar with its terminal screws.
            let iw = (w * 0.1).max(10.0);
            for x0 in [0.0, w - iw] {
                m.block([x0, 0.0, 0.0], [x0 + iw, h, d * 0.6]);
            }
            m.block([0.0, h * 0.35, d * 0.6], [w, h * 0.65, d * 0.85]);
            let r = h * 0.1;
            for x in spread(iw * 1.5, w - iw * 1.5, 10) {
                m.solid([x - r, h / 2.0 - r, d * 0.85], [x + r, h / 2.0 + r, d]);
                m.circle(x, h / 2.0, r, false);
            }
        }
        "Cable clamp rail" => {
            // C profile: web, flanges and lips.
            let t = 2.0f32.min(d / 4.0);
            m.solid([0.0, 0.0, 0.0], [w, h, t]);
            m.solid([0.0, 0.0, 0.0], [w, t, d]);
            m.solid([0.0, h - t, 0.0], [w, h, d]);
            m.solid([0.0, 0.0, d - t], [w, h * 0.25, d]);
            m.solid([0.0, h * 0.75, d - t], [w, h, d]);
            m.line(0.0, h * 0.25, w, h * 0.25);
            m.line(0.0, h * 0.75, w, h * 0.75);
        }
        "Door handle" => {
            m.block([0.0, 0.0, 0.0], [w, h, d * 0.3]);
            m.block([w * 0.2, h * 0.08, d * 0.3], [w * 0.8, h * 0.72, d]);
            m.circle(w / 2.0, h * 0.85, w * 0.2, false);
        }
        "Document holder" => {
            // Back plate and the pocket in front of its lower part.
            let t = 2.0f32.min(d / 4.0);
            m.solid([0.0, 0.0, 0.0], [w, h, t]);
            let ph = h * 0.65;
            m.solid([0.0, 0.0, d - t], [w, ph, d]);
            m.solid([0.0, 0.0, 0.0], [t, ph, d]);
            m.solid([w - t, 0.0, 0.0], [w, ph, d]);
            m.solid([0.0, 0.0, 0.0], [w, t, d]);
            m.face(0.0, 0.0, w, ph, false);
            m.line(w * 0.35, ph, w * 0.65, ph);
        }
        "Rail standoff" => {
            m.solid([0.0, 0.0, 0.0], [w, h, d]);
            m.circle(w / 2.0, h / 2.0, w.min(h) * 0.15, false);
        }
        _ => m.solid([0.0, 0.0, 0.0], [w, h, d]),
    }
    for (x, y) in holes(p) {
        // Holes are given from the top left of the front view.
        m.circle(x, h - y, 2.5f32.min(w.min(h) * 0.1), false);
    }
    Some(m.done(w))
}
