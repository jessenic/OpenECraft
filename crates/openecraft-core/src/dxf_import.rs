//! DXF import (ASCII DXF, R12 … 2018): an old drawing as graphics on a new schematic
//! page – lines, polylines, circles, arcs, ellipses, splines (through their control
//! points), texts and block references (also those of dimensions), scaled to fit the
//! drawing area of the page.

use openecraft_i18n::tr;
use crate::geom::{p, P};
use crate::model::*;
use crate::symbols::Anchor;
use std::collections::{BTreeSet, HashMap};

/// Drawing elements in DXF coordinates (y up).
#[derive(Clone, Debug, PartialEq)]
enum Geo {
    Line(P, P, bool),
    Text { pos: P, text: String, height: f32, angle: f32, align: Anchor },
}

type Entity = Vec<(i32, String)>;

#[derive(Default)]
struct Drawing {
    entities: Vec<Entity>,
    blocks: HashMap<String, (P, Vec<Entity>)>,
    /// Drawing units in mm ($INSUNITS), 1 if unknown.
    unit: f32,
}

fn pairs(text: &str) -> Result<Vec<(i32, String)>, String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 2 {
        return Err(tr("Not a DXF file").into());
    }
    let mut out = Vec::with_capacity(lines.len() / 2);
    for c in lines.chunks(2) {
        let Some(v) = c.get(1) else { break };
        let code = c[0].trim().parse::<i32>().map_err(|_| tr("Not a DXF file (binary DXF is not supported)").to_string())?;
        out.push((code, v.trim_end_matches('\r').to_string()));
    }
    Ok(out)
}

fn parse(text: &str) -> Result<Drawing, String> {
    let all = pairs(text.trim_start_matches('\u{feff}'))?;
    if !all.iter().any(|(c, v)| *c == 0 && v.trim() == "SECTION") {
        return Err(tr("Not a DXF file").into());
    }
    let mut d = Drawing { unit: 1.0, ..Default::default() };
    let mut section = String::new();
    let mut i = 0;
    // Entities of the current block (name, base point) or the ENTITIES section.
    let mut block: Option<(String, P, Vec<Entity>)> = None;
    while i < all.len() {
        let (code, value) = (&all[i].0, all[i].1.trim());
        if *code != 0 {
            i += 1;
            continue;
        }
        // An entity: up to the next code 0.
        let start = i;
        i += 1;
        while i < all.len() && all[i].0 != 0 {
            i += 1;
        }
        let ent: Entity = all[start..i].iter().map(|(c, v)| (*c, v.trim().to_string())).collect();
        match value {
            "SECTION" => {
                section = get(&ent, 2).unwrap_or_default().to_string();
                // The header variables follow the section name before the next code 0.
                if let Some(k) = ent.iter().position(|(c, v)| *c == 9 && v == "$INSUNITS") {
                    d.unit = match ent.get(k + 1).map(|x| x.1.as_str()) {
                        Some("1") => 25.4,
                        Some("2") => 304.8,
                        Some("5") => 10.0,
                        Some("6") => 1000.0,
                        _ => 1.0,
                    };
                }
            }
            "ENDSEC" => section.clear(),
            "BLOCK" if section == "BLOCKS" => block = Some((get(&ent, 2).unwrap_or_default().to_string(), point(&ent, 10), Vec::new())),
            "ENDBLK" => {
                if let Some((name, base, ents)) = block.take() {
                    d.blocks.insert(name, (base, ents));
                }
            }
            _ if section == "ENTITIES" => d.entities.push(ent),
            _ if section == "BLOCKS" => {
                if let Some(b) = block.as_mut() {
                    b.2.push(ent);
                }
            }
            _ => {}
        }
    }
    Ok(d)
}

fn get(e: &Entity, code: i32) -> Option<&str> {
    e.iter().find(|(c, _)| *c == code).map(|(_, v)| v.as_str())
}

fn num(e: &Entity, code: i32, default: f32) -> f32 {
    get(e, code).and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn point(e: &Entity, code: i32) -> P {
    p(num(e, code, 0.0), num(e, code + 10, 0.0))
}

/// All values of a repeated coordinate pair (polyline vertices, spline points).
fn points(e: &Entity, code: i32) -> Vec<P> {
    let mut v = Vec::new();
    for (k, (c, x)) in e.iter().enumerate() {
        if *c == code {
            let y = e[k + 1..].iter().find(|(c2, _)| *c2 == code + 10).map(|(_, y)| y.parse().unwrap_or(0.0)).unwrap_or(0.0);
            v.push(p(x.parse().unwrap_or(0.0), y));
        }
    }
    v
}

fn dashed(e: &Entity) -> bool {
    get(e, 6).is_some_and(|l| {
        let l = l.to_ascii_uppercase();
        l.contains("DASH") || l.contains("HIDDEN") || l.contains("CENTER") || l.contains("DOT")
    })
}

/// DXF text codes: `\P` line breaks, `%%c` diameter, `%%d` degree, `%%p` plus/minus,
/// `\U+00B0` characters; MTEXT formatting (`{\fArial;…}`, `\H2.5;`) is dropped.
fn clean_text(s: &str) -> String {
    let s = s.replace("%%c", "Ø").replace("%%C", "Ø").replace("%%d", "°").replace("%%D", "°").replace("%%p", "±").replace("%%P", "±");
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' | '}' => i += 1,
            '\\' if i + 1 < chars.len() => {
                let c = chars[i + 1];
                match c {
                    'P' | 'p' => {
                        out.push('\n');
                        i += 2;
                    }
                    'U' | 'u' if chars.get(i + 2) == Some(&'+') => {
                        let hex: String = chars[i + 3..(i + 7).min(chars.len())].iter().collect();
                        if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                            out.push(ch);
                        }
                        i += 7;
                    }
                    '\\' | '{' | '}' => {
                        out.push(c);
                        i += 2;
                    }
                    // Formatting codes up to the next ';' (font, height, colour, …),
                    // single-letter switches (\L \l \O \o \K \k) without one.
                    'L' | 'l' | 'O' | 'o' | 'K' | 'k' => i += 2,
                    _ => {
                        while i < chars.len() && chars[i] != ';' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out.trim().to_string()
}

/// Polyline through points as lines.
fn polyline(out: &mut Vec<Geo>, pts: &[P], closed: bool, dash: bool) {
    for w in pts.windows(2) {
        out.push(Geo::Line(w[0], w[1], dash));
    }
    if closed && pts.len() > 2 {
        out.push(Geo::Line(pts[pts.len() - 1], pts[0], dash));
    }
}

fn arc_points(c: P, r: f32, a0: f32, a1: f32) -> Vec<P> {
    let mut sweep = a1 - a0;
    while sweep <= 0.0 {
        sweep += 360.0;
    }
    let n = ((sweep / 10.0).ceil() as usize).max(2);
    (0..=n).map(|k| (a0 + sweep * k as f32 / n as f32).to_radians()).map(|a| p(c.x + r * a.cos(), c.y + r * a.sin())).collect()
}

/// Transformation of a block reference: a linear map (scale, mirror, rotate), then a
/// move. Kept as a matrix: a mirrored block turns the blocks inside it the other way.
#[derive(Clone, Copy)]
struct Tf {
    /// Columns: images of the x and y axes.
    x: P,
    y: P,
    at: P,
}

impl Tf {
    const ID: Tf = Tf { x: P { x: 1.0, y: 0.0 }, y: P { x: 0.0, y: 1.0 }, at: P { x: 0.0, y: 0.0 } };

    /// Scale by `sx`, `sy`, turn by `deg` counter-clockwise, move to `at`.
    fn new(sx: f32, sy: f32, deg: f32, at: P) -> Tf {
        let (sin, cos) = deg.to_radians().sin_cos();
        Tf { x: p(cos * sx, sin * sx), y: p(-sin * sy, cos * sy), at }
    }

    /// The linear part only.
    fn map(&self, v: P) -> P {
        p(self.x.x * v.x + self.y.x * v.y, self.x.y * v.x + self.y.y * v.y)
    }

    fn apply(&self, q: P) -> P {
        self.at + self.map(q)
    }

    fn then(&self, outer: &Tf) -> Tf {
        // Points go through self first, then outer.
        Tf { x: outer.map(self.x), y: outer.map(self.y), at: outer.apply(self.at) }
    }

    /// A text with the base line at `deg` and height `h`: its angle and height after
    /// the transformation.
    fn text(&self, deg: f32, h: f32) -> (f32, f32) {
        let (sin, cos) = deg.to_radians().sin_cos();
        let (along, up) = (self.map(p(cos, sin)), self.map(p(-sin, cos)));
        // The height counts across the new base line.
        let len = along.x.hypot(along.y).max(1e-9);
        let across = (along.x * up.y - along.y * up.x).abs() / len;
        (along.y.atan2(along.x).to_degrees(), h * across)
    }
}

fn entities(d: &Drawing, ents: &[Entity], tf: Tf, depth: usize, out: &mut Vec<Geo>, skipped: &mut BTreeSet<String>) {
    let mut k = 0;
    while k < ents.len() {
        let e = &ents[k];
        let kind = e[0].1.as_str();
        let dash = dashed(e);
        let mut geo = Vec::new();
        match kind {
            "LINE" => geo.push(Geo::Line(point(e, 10), point(e, 11), dash)),
            "LWPOLYLINE" => polyline(&mut geo, &points(e, 10), num(e, 70, 0.0) as i32 & 1 == 1, dash),
            "POLYLINE" => {
                let closed = num(e, 70, 0.0) as i32 & 1 == 1;
                let mut pts = Vec::new();
                while k + 1 < ents.len() && ents[k + 1][0].1 == "VERTEX" {
                    k += 1;
                    pts.push(point(&ents[k], 10));
                }
                if k + 1 < ents.len() && ents[k + 1][0].1 == "SEQEND" {
                    k += 1;
                }
                polyline(&mut geo, &pts, closed, dash);
            }
            "CIRCLE" => polyline(&mut geo, &arc_points(point(e, 10), num(e, 40, 0.0), 0.0, 360.0), false, dash),
            "ARC" => polyline(&mut geo, &arc_points(point(e, 10), num(e, 40, 0.0), num(e, 50, 0.0), num(e, 51, 360.0)), false, dash),
            "ELLIPSE" => {
                let (c, m) = (point(e, 10), point(e, 11));
                let ratio = num(e, 40, 1.0);
                let (t0, t1) = (num(e, 41, 0.0), num(e, 42, std::f32::consts::TAU));
                let mut sweep = t1 - t0;
                if sweep <= 0.0 {
                    sweep += std::f32::consts::TAU;
                }
                let minor = p(-m.y * ratio, m.x * ratio);
                let n = 36;
                let pts: Vec<P> = (0..=n)
                    .map(|i| t0 + sweep * i as f32 / n as f32)
                    .map(|t| p(c.x + m.x * t.cos() + minor.x * t.sin(), c.y + m.y * t.cos() + minor.y * t.sin()))
                    .collect();
                polyline(&mut geo, &pts, false, dash);
            }
            "SPLINE" => {
                let fit = points(e, 11);
                let pts = if fit.len() >= 2 { fit } else { points(e, 10) };
                polyline(&mut geo, &pts, num(e, 70, 0.0) as i32 & 1 == 1, dash);
            }
            "SOLID" | "TRACE" | "3DFACE" => {
                let pts = [point(e, 10), point(e, 11), point(e, 13), point(e, 12)];
                polyline(&mut geo, &pts, true, dash);
            }
            "TEXT" | "ATTRIB" => {
                let h = num(e, 40, 2.5);
                let aligned = num(e, 72, 0.0) != 0.0 || num(e, 73, 0.0) != 0.0;
                let base = if aligned && get(e, 11).is_some() { point(e, 11) } else { point(e, 10) };
                let text = clean_text(get(e, 1).unwrap_or(""));
                if !text.is_empty() {
                    let align = match num(e, 72, 0.0) as i32 {
                        1 | 4 => Anchor::Center,
                        2 => Anchor::Right,
                        _ => Anchor::Left,
                    };
                    geo.push(Geo::Text { pos: p(base.x, base.y + h / 2.0), text, height: h, angle: num(e, 50, 0.0), align });
                }
            }
            "MTEXT" => {
                let h = num(e, 40, 2.5);
                let raw: String = e.iter().filter(|(c, _)| *c == 3).map(|(_, v)| v.as_str()).chain(get(e, 1)).collect();
                let text = clean_text(&raw);
                if !text.is_empty() {
                    // Attachment 1-3: top, 4-6: middle, 7-9: bottom (of the first line here).
                    let at = point(e, 10);
                    let y = match num(e, 71, 1.0) as i32 {
                        4..=6 => at.y,
                        7..=9 => at.y + h / 2.0 + (text.lines().count().max(1) - 1) as f32 * h * 1.4,
                        _ => at.y - h / 2.0,
                    };
                    geo.push(Geo::Text { pos: p(at.x, y), text, height: h, angle: num(e, 50, 0.0), align: Anchor::Left });
                }
            }
            "INSERT" | "DIMENSION" => {
                let name = get(e, 2).unwrap_or_default();
                if let (Some((base, block)), true) = (d.blocks.get(name), depth < 8) {
                    let (sx, sy, rot) = if kind == "INSERT" { (num(e, 41, 1.0), num(e, 42, 1.0), num(e, 50, 0.0)) } else { (1.0, 1.0, 0.0) };
                    let at = if kind == "INSERT" { point(e, 10) } else { p(0.0, 0.0) };
                    let local = Tf::new(sx, sy, rot, at);
                    // The block's base point lies at the insertion point.
                    let shift = Tf { at: p(-base.x, -base.y), ..Tf::ID };
                    let inner = shift.then(&local).then(&tf);
                    entities(d, block, inner, depth + 1, out, skipped);
                }
            }
            "VERTEX" | "SEQEND" | "ATTDEF" | "POINT" | "VIEWPORT" => {}
            other => {
                skipped.insert(other.to_string());
            }
        }
        for g in geo {
            out.push(match g {
                Geo::Line(a, b, dash) => Geo::Line(tf.apply(a), tf.apply(b), dash),
                Geo::Text { pos, text, height, angle, align } => {
                    let (angle, height) = tf.text(angle, height);
                    Geo::Text { pos: tf.apply(pos), text, height, angle, align }
                }
            });
        }
        k += 1;
    }
}

/// How the drawing is put on the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DxfOptions {
    /// Scale down drawings that do not fit into the drawing area (keeps 1:1 otherwise).
    pub fit: bool,
}

impl Default for DxfOptions {
    fn default() -> Self {
        DxfOptions { fit: true }
    }
}

/// What an import brought in.
#[derive(Clone, Debug, PartialEq)]
pub struct DxfImport {
    pub page: usize,
    pub lines: usize,
    pub texts: usize,
    /// Scale from drawing millimetres to the page.
    pub scale: f32,
    /// Bounds of the decoded geometry in drawing millimetres (y up).
    pub source_min: P,
    pub source_max: P,
    /// Page position corresponding to the drawing's top-left bound.
    pub page_origin: P,
    /// Entity types that are not imported (hatches, images, …).
    pub skipped: Vec<String>,
}


/// Inserts a DXF drawing as a new schematic page after `after`.
pub fn import_page(prj: &mut Project, text: &str, description: &str, after: usize, opts: DxfOptions) -> Result<DxfImport, String> {
    let d = parse(text)?;
    let mut geo = Vec::new();
    let mut skipped = BTreeSet::new();
    let unit = Tf::new(d.unit, d.unit, 0.0, p(0.0, 0.0));
    entities(&d, &d.entities, unit, 0, &mut geo, &mut skipped);
    if geo.is_empty() {
        return Err(tr("The DXF file contains nothing that can be imported").into());
    }
    let (mut min, mut max) = (p(f32::MAX, f32::MAX), p(f32::MIN, f32::MIN));
    let mut extend = |q: P| {
        min = p(min.x.min(q.x), min.y.min(q.y));
        max = p(max.x.max(q.x), max.y.max(q.y));
    };
    for g in &geo {
        match g {
            Geo::Line(a, b, _) => {
                extend(*a);
                extend(*b);
            }
            Geo::Text { pos, .. } => extend(*pos),
        }
    }
    let (w, h) = ((max.x - min.x).max(1e-3), (max.y - min.y).max(1e-3));
    // Drawing area of the new page above the title block.
    let area = crate::frame::project_default(prj).area();
    let area = (area.min.x, area.min.y, area.max.x - 5.0, area.max.y - 5.0);
    let (aw, ah) = (area.2 - area.0, area.3 - area.1);
    let scale = if opts.fit { (aw / w).min(ah / h).min(1.0) } else { 1.0 };
    // DXF y up → page y down; the drawing's top left corner at the area's.
    let to_page = |q: P| p(area.0 + (q.x - min.x) * scale, area.1 + (max.y - q.y) * scale);
    let at = crate::ops::add_page(prj, PageKind::Schematic, after + 1);
    prj.pages[at].description = description.to_string();
    let (mut lines, mut texts) = (0, 0);
    for g in geo {
        let id = prj.new_id();
        match g {
            Geo::Line(a, b, dashed) => {
                let (a, b) = (to_page(a), to_page(b));
                if (a.x - b.x).abs() + (a.y - b.y).abs() < 1e-4 {
                    continue;
                }
                prj.pages[at].shapes.push(Shape { id, kind: ShapeKind::Line, a, b, dashed, ..Default::default() });
                lines += 1;
            }
            Geo::Text { pos, text, height, angle, align } => {
                prj.pages[at].texts.push(TextItem { id, pos: to_page(pos), text, size: (height * scale).max(1.0), angle: angle.rem_euclid(360.0), align, ..Default::default() });
                texts += 1;
            }
        }
    }
    Ok(DxfImport { page: at, lines, texts, scale, source_min: min, source_max: max,
        page_origin: p(area.0, area.1), skipped: skipped.into_iter().collect() })
}

#[cfg(test)]
mod tests;
