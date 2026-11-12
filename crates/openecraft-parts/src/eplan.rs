//! EPLAN parts XML (the parts management export of EPLAN Platform: `<part>` elements
//! with `P_ARTICLE_*` attributes, optionally with `<variant>` children): read into
//! parts and written for EPLAN. Multi-language texts (`de_DE@Schütz;en_US@Contactor;`)
//! take the English text, otherwise German, otherwise the first one. Product groups
//! are mapped by their exchange numbers ([`crate::productgroups`]).
//!
//! Function templates (`<functiontemplate>`), connection point definitions
//! (`<terminal>`) and drilling patterns (`<construction>`) are read as well, also from
//! separate files as in EPLAN Data Portal archives ([`from_eplan_documents`]).
//! Connection points on a plug (`terminalNr` / `devicetag` `X10`) are named
//! `X10.1`, so that the designations of a part stay unique; they keep the wires their
//! clamps take. Drilling patterns give the mounting holes (through or tapped) and the
//! cutout of the part; rail data or texts naming a rail give its mounting on a rail.

use crate::productgroups::{self, Classification};
use crate::xml::{self, Element};
use crate::{ConnectionPoint, Cutout, Exit, FnKind, FunctionTemplate, Part, PlacedCutout};
use std::collections::HashMap;
use std::fmt::Write;

/// The text looks like an EPLAN parts XML file.
pub fn is_eplan_xml(text: &str) -> bool {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    t.starts_with('<') && t.contains("P_ARTICLE_PARTNR")
}

/// `de_DE`, `en_US`, `??_??`: the code of an entry of a multi-language value.
fn is_language(code: &str) -> bool {
    let b = code.as_bytes();
    b.len() == 5 && b[2] == b'_' && [b[0], b[1], b[3], b[4]].iter().all(|c| c.is_ascii_alphabetic() || *c == b'?')
}

/// The text of a multi-language value in the preferred language. Language neutral
/// texts (`??_??@{{LOGO!}};//??_??@LOGO!;`) come after English and German. A `;`
/// ends an entry only before the next language code: texts may contain it.
pub fn language_text(value: &str) -> String {
    if !value.contains('@') {
        return value.trim().to_string();
    }
    let mut segments: Vec<&str> = value.split(';').collect();
    if segments.last().is_some_and(|s| s.trim().is_empty()) {
        segments.pop();
    }
    let mut raw: Vec<(&str, String)> = Vec::new();
    for seg in segments {
        match seg.trim_start().trim_start_matches('/').split_once('@').filter(|(l, _)| is_language(l.trim())) {
            Some((l, t)) => raw.push((l.trim(), t.to_string())),
            None => {
                if let Some((_, t)) = raw.last_mut() {
                    t.push(';');
                    t.push_str(seg);
                }
            }
        }
    }
    let entries: Vec<(&str, &str)> = raw
        .iter()
        .map(|(l, t)| (*l, t.trim()))
        .map(|(l, t)| (l, t.strip_prefix("{{").and_then(|t| t.strip_suffix("}}")).unwrap_or(t)))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    for lang in ["en_US", "en_GB", "de_DE", "??_??"] {
        if let Some((_, t)) = entries.iter().find(|(l, _)| l.eq_ignore_ascii_case(lang)) {
            return t.to_string();
        }
    }
    entries.first().map(|(_, t)| t.to_string()).unwrap_or_else(|| value.trim().to_string())
}

fn attr<'a>(el: &'a Element, name: &str) -> Option<&'a str> {
    el.attributes.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str()).filter(|v| !v.trim().is_empty())
}

/// A number written by EPLAN (`45`, `45.5`, `45,5`); 0 if there is none.
fn num(v: Option<&str>) -> f32 {
    v.map(|s| s.trim().replace(',', ".")).and_then(|s| s.parse().ok()).unwrap_or(0.0)
}

/// Designation of a connection point, with its plug if it has one (`X10.1`).
fn pin_name(plug: Option<&str>, name: &str) -> String {
    match plug.map(str::trim).filter(|p| !p.is_empty()) {
        Some(plug) => format!("{plug}.{}", name.trim()),
        None => name.trim().to_string(),
    }
}

/// Function of an EPLAN function definition (category / group / id). PLC connection
/// points (category 300, group 1): 2 digital input, 3 digital output, 4 analog input,
/// 5 analog output; the multi-function ones (6) by their name (`I1`, `Q2`, `AI1`).
fn function_kind(category: u32, group: u32, id: u32, description: &str) -> FnKind {
    match (category, group, id) {
        (300, 1, 2) => FnKind::PlcInput,
        (300, 1, 3) => FnKind::PlcOutput,
        (300, 1, 4) => FnKind::PlcAnalogInput,
        (300, 1, 5) => FnKind::PlcAnalogOutput,
        (300, 1, 6) => {
            let d = description.trim().to_uppercase();
            let numbered = |prefix: &str| d.strip_prefix(prefix).and_then(|r| r.chars().next()).is_some_and(|c| c.is_ascii_digit());
            if numbered("AI") || numbered("AE") {
                FnKind::PlcAnalogInput
            } else if numbered("AO") || numbered("AA") {
                FnKind::PlcAnalogOutput
            } else if numbered("I") || numbered("DI") || numbered("E") {
                FnKind::PlcInput
            } else if numbered("Q") || numbered("DO") || numbered("A") {
                FnKind::PlcOutput
            } else {
                FnKind::Generic
            }
        }
        _ => FnKind::Generic,
    }
}

/// A function template with a connection point; main functions without one (PLC box,
/// black box) are left out.
fn function_template(el: &Element) -> Option<FunctionTemplate> {
    let designation = attr(el, "connectionDesignation")?;
    let description = attr(el, "connectiondescription").map(language_text).unwrap_or_default();
    let n = |name: &str| num(attr(el, name)) as u32;
    let kind = function_kind(n("functiondefcategory"), n("functiondefgroup"), n("functiondefid"), &description);
    Some(FunctionTemplate { kind, pins: pin_name(attr(el, "terminalNr"), designation), description })
}

/// Cross-section in mm² of an AWG size.
fn awg_mm2(awg: f32) -> f32 {
    let d = 0.127 * 92f32.powf((36.0 - awg) / 39.0);
    (std::f32::consts::FRAC_PI_4 * d * d * 100.0).round() / 100.0
}

/// Connection points of a `<terminal>` definition in EPLAN coordinates (y up from the
/// lower edge) with the direction of their wires; the points carry their name and the
/// wires the clamp takes (cross-sections in mm², otherwise converted from AWG).
fn terminal_points(el: &Element) -> Vec<([f32; 3], [f32; 3], ConnectionPoint)> {
    el.all("terminalPosition")
        .filter_map(|t| {
            let name = attr(t, "name")?;
            let v = |a: &str| num(attr(t, a));
            let mm2 = |metric: &str, awg: &str| match (v(metric), v(awg)) {
                (m, _) if m > 0.0 => m,
                (_, a) if a > 0.0 => awg_mm2(a),
                _ => 0.0,
            };
            let point = ConnectionPoint {
                pin: pin_name(attr(t, "devicetag"), name),
                min_cross_section: mm2("mincrosssection", "mincrosssectionawg"),
                max_cross_section: mm2("maxcrosssection", "maxcrosssectionawg"),
                max_wires: v("maxwirecount").max(0.0) as u32,
                ..Default::default()
            };
            Some(([v("xpos"), v("ypos"), v("zpos")], [v("xdir"), v("ydir"), v("zdir")], point))
        })
        .collect()
}

/// The connection points in the part's front view (y down from the upper edge).
fn connection_points(points: &[([f32; 3], [f32; 3], ConnectionPoint)], height: f32) -> Vec<ConnectionPoint> {
    points
        .iter()
        .map(|(p, d, point)| {
            let exit = match d {
                [_, y, _] if *y > 0.5 => Exit::Up,
                [_, y, _] if *y < -0.5 => Exit::Down,
                [x, _, _] if *x < -0.5 => Exit::Left,
                [x, _, _] if *x > 0.5 => Exit::Right,
                [_, _, z] if z.abs() > 0.5 => Exit::Front,
                _ if p[1] < height / 2.0 => Exit::Down,
                _ => Exit::Up,
            };
            ConnectionPoint { pos: [p[0], height - p[1], p[2]], exit, ..point.clone() }
        })
        .collect()
}

/// A position of a drilling pattern (`<drillingPosition>`), EPLAN coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Drill {
    /// Through hole of this diameter (type 1).
    Hole([f32; 2], f32),
    /// Tapped hole of this nominal size (type 2: `M4`).
    Thread([f32; 2], f32),
    /// Opening for the device around its centre, turned by an angle (degrees,
    /// counter-clockwise): rectangular (type 3, height and width) or round (holes larger
    /// than a screw, as for command devices).
    Cutout([f32; 2], Cutout, f32),
}

/// Holes up to this diameter take screws (the clearance hole of M16 is 17.5 mm); larger
/// ones are openings for the device (command devices: 22.5 mm).
const MAX_SCREW_HOLE: f32 = 18.0;

/// The positions of a `<construction>` drilling pattern, and their records as they are
/// (kept with the part: what a subtype or the third dimension means is not known).
fn drill_holes(el: &Element) -> (Vec<Drill>, Vec<serde_json::Value>) {
    let positions: Vec<&Element> = el.all("drillingPosition").collect();
    let drills = positions
        .iter()
        .filter_map(|h| {
            let v = |a: &str| num(attr(h, a));
            let pos = [v("xpos"), v("ypos")];
            let (d1, d2) = (v("dimension1"), v("dimension2"));
            Some(match v("type") as u32 {
                2 => Drill::Thread(pos, d1),
                3 if d1 > 0.0 && d2 > 0.0 => Drill::Cutout(pos, Cutout::Rect { w: d2, h: d1 }, v("angle")),
                3 => return None,
                _ if d1 > MAX_SCREW_HOLE => Drill::Cutout(pos, Cutout::Round { d: d1 }, 0.0),
                _ => Drill::Hole(pos, d1),
            })
        })
        .collect();
    let records = positions.iter().map(|h| serde_json::Value::Object(h.attributes.iter().map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone()))).collect())).collect();
    (drills, records)
}

/// Texts that say a part snaps onto a mounting rail.
const RAIL_WORDS: [&str; 9] = ["din rail", "din-rail", "top hat rail", "top-hat rail", "mounting rail", "rail mount", "hutschiene", "tragschiene", "en 60715"];

/// Rails by their names (in [`rail_text`]).
const RAIL_NAMES: [&str; 4] = ["ts35", "ts15", "ts32", "g32"];

/// A text in lower case with the names of rails written together: `ts 35` → `ts35`,
/// `35 x 15` → `35x15`, `g 32` → `g32`.
fn rail_text(text: &str) -> String {
    let chars: Vec<char> = text.to_lowercase().replace('×', "x").chars().collect();
    // `w` ends `out` as a word of its own.
    let ends = |out: &str, w: &str| out.ends_with(w) && !out[..out.len() - w.len()].chars().next_back().is_some_and(char::is_alphanumeric);
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_whitespace() {
            let next = chars[i + 1..].iter().find(|c| !c.is_whitespace()).copied();
            let join = match (out.chars().next_back(), next) {
                (Some(p), Some('x')) if p.is_ascii_digit() => true,
                (Some('x'), Some(n)) if n.is_ascii_digit() => out.chars().rev().nth(1).is_some_and(|c| c.is_ascii_digit()),
                (Some(_), Some(n)) if n.is_ascii_digit() => ends(&out, "ts") || ends(&out, "g"),
                _ => false,
            };
            if join {
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// `pattern` stands on its own in `text`: no letter or digit before it, no digit after.
fn standalone(text: &str, pattern: &str) -> bool {
    text.match_indices(pattern).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + pattern.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(|c| c.is_ascii_digit())
    })
}

/// The mounting of a part on a rail by EPLAN's rail data (snap height, centre offset)
/// or by its texts (description and others): `Mounting rail 35 mm`,
/// `Mounting rail 35 x 15 mm`, `Mounting rail 15 mm` or `Mounting rail G 32` by the rail
/// they name; `None` for parts that are not snapped onto a rail, and for rails, whose
/// description starts with one.
fn rail_mounting(snap_height: f32, center_offset: f32, description: &str, others: &[&str]) -> Option<String> {
    let lead = rail_text(description.trim());
    if RAIL_WORDS.iter().chain(&RAIL_NAMES).any(|w| lead.starts_with(w)) {
        return None;
    }
    let text = rail_text(&std::iter::once(description).chain(others.iter().copied()).collect::<Vec<_>>().join("\n"));
    let named = RAIL_WORDS.iter().any(|w| text.contains(w)) || RAIL_NAMES[..3].iter().any(|w| standalone(&text, w));
    if !named && snap_height <= 0.0 && center_offset == 0.0 {
        return None;
    }
    Some(if standalone(&text, "35x15") || text.contains("ts35x15") {
        "Mounting rail 35 x 15 mm".into()
    } else if standalone(&text, "ts15") || text.contains("15 mm rail") {
        "Mounting rail 15 mm".into()
    } else if standalone(&text, "ts32") || standalone(&text, "g32") {
        "Mounting rail G 32".into()
    } else {
        "Mounting rail 35 mm".into()
    })
}

fn part_of(el: &Element, variant: Option<&Element>) -> Option<Part> {
    // Values of the variant win over those of the part.
    let get = |name: &str| variant.and_then(|v| attr(v, name)).or_else(|| attr(el, name));
    let text = |name: &str| get(name).map(language_text).unwrap_or_default();
    let number = text("P_ARTICLE_PARTNR");
    if number.is_empty() {
        return None;
    }
    let mut part = Part { number, ..Default::default() };
    part.type_number = text("P_ARTICLE_TYPENR");
    part.eplan_macro = text("P_ARTICLE_GROUPSYMBOLMACRO");
    part.order_number = text("P_ARTICLE_ORDERNR");
    part.manufacturer = text("P_ARTICLE_MANUFACTURER");
    part.supplier = text("P_ARTICLE_SUPPLIER");
    part.description = text("P_ARTICLE_DESCR1");
    let more: Vec<String> = ["P_ARTICLE_DESCR2", "P_ARTICLE_DESCR3", "P_ARTICLE_NOTE"].iter().map(|n| text(n)).filter(|t| !t.is_empty()).collect();
    part.note = more.join("\n");
    part.identifier = text("P_ARTICLE_ERPNR");
    part.price = num(get("P_ARTICLE_SALESPRICE_1").or_else(|| get("P_ARTICLE_PURCHASEPRICE_1")));
    part.price_unit = num(get("P_ARTICLE_PRICEUNIT")).max(0.0) as u32;
    part.pack_quantity = num(get("P_ARTICLE_PACKAGINGQUANTITY")).max(0.0) as u32;
    part.quantity_unit = text("P_ARTICLE_QUANTITYUNIT");
    part.width = num(get("P_ARTICLE_WIDTH"));
    part.height = num(get("P_ARTICLE_HEIGHT"));
    part.depth = num(get("P_ARTICLE_DEPTH"));
    part.weight = num(get("P_ARTICLE_WEIGHT"));
    part.voltage = text("P_ARTICLE_VOLTAGE");
    part.current = text("P_ARTICLE_ELECTRICALCURRENT");
    part.power = text("P_ARTICLE_ELECTRICALPOWER");
    part.certificates = text("P_ARTICLE_CERTIFICATE");
    // A number is one of EPLAN's mounting sites, 0 none.
    part.mounting = text("P_ARTICLE_MOUNTINGSITE");
    part.mounting_data.rail.center_offset = num(get("P_ARTICLE_MIDDLEOFFSET"));
    part.mounting_data.rail.clip_on = num(get("P_ARTICLE_SNAPHEIGHT"));
    part.accessory = matches!(text("P_ARTICLE_ISACCESSORY").to_ascii_lowercase().as_str(), "1" | "true" | "yes");
    if part.mounting == "0" {
        part.mounting.clear();
    }
    let rail = part.mounting_data.rail;
    if part.mounting.is_empty() || part.mounting.to_lowercase().contains("rail") {
        if let Some(m) = rail_mounting(rail.clip_on, rail.center_offset, &part.description, &[&part.mounting, &part.note]) {
            part.mounting = m;
        }
    }
    let mut tech: Vec<String> = Vec::new();
    for (key, name) in [("P_ARTICLE_CHARACTERISTICS", ""), ("P_ARTICLE_TECHNICALCHARACTERISTIC", ""), ("P_ARTICLE_POWERLOSS", "Power loss"), ("P_ARTICLE_POWERDISSIPATION", "Power loss"), ("P_ARTICLE_COLOR", "Colour")] {
        let v = text(key);
        if !v.is_empty() && !tech.iter().any(|t| name == "Power loss" && t.starts_with("Power loss:")) {
            tech.push(if name.is_empty() { v } else { format!("{name}: {v}") });
        }
    }
    if text("P_ARTICLE_IDENTTYPE").eq_ignore_ascii_case("GTIN") && !text("P_ARTICLE_IDENTCODE").is_empty() {
        tech.push(format!("GTIN: {}", text("P_ARTICLE_IDENTCODE")));
    }
    part.tech_data = tech.join("\n");
    // The first line of an external document is its address, the rest its name.
    part.datasheet = get("P_ARTICLE_EXTERNAL_DOCUMENT_1").and_then(|d| d.lines().next()).map(str::trim).filter(|d| d.starts_with("http")).unwrap_or_default().to_string();
    let md = &mut part.mounting_data;
    md.clearance_top = num(get("P_ARTICLE_SPACING_ABOVE"));
    md.clearance_bottom = num(get("P_ARTICLE_SPACING_BELOW"));
    md.clearance_left = num(get("P_ARTICLE_SPACING_LEFT"));
    md.clearance_right = num(get("P_ARTICLE_SPACING_RIGHT"));
    md.clearance_front = num(get("P_ARTICLE_SPACING_FRONT"));
    let templates: Vec<&Element> = variant.into_iter().flat_map(|v| v.all("functiontemplate")).collect();
    let templates = if templates.is_empty() { el.all("functiontemplate").collect() } else { templates };
    part.functions = templates.into_iter().filter_map(function_template).collect();
    let c = Classification {
        top_group: num(get("P_ARTICLE_PRODUCTTOPGROUP")) as u16,
        group: num(get("P_ARTICLE_PRODUCTGROUP")) as u16,
        subgroup: num(get("P_ARTICLE_PRODUCTSUBGROUP")) as u16,
    };
    productgroups::apply(&mut part, c);
    Some(part)
}

/// The elements named `name` (the root or below, not inside one another).
fn collect<'a>(el: &'a Element, name: &str, out: &mut Vec<&'a Element>) {
    if el.name.eq_ignore_ascii_case(name) {
        out.push(el);
        return;
    }
    for c in &el.children {
        collect(c, name, out);
    }
}

/// The parts of an EPLAN parts XML file; a part with variants gives one part per
/// variant with its own part number.
pub fn from_eplan_xml(text: &str) -> Result<Vec<Part>, String> {
    from_eplan_documents(&[text])
}

/// The parts of several EPLAN parts XML documents (parts, connection point
/// definitions, drilling patterns). Parts get the connection points and mounting holes
/// they refer to (`P_ARTICLE_REF_TERMINAL_NAME`, `P_ARTICLE_REF_CONSTRUCTION_NAME`).
pub fn from_eplan_documents(texts: &[&str]) -> Result<Vec<Part>, String> {
    let roots = texts.iter().map(|t| xml::parse(t.trim_start_matches('\u{feff}'))).collect::<Result<Vec<_>, _>>()?;
    let mut terminals = HashMap::new();
    let mut constructions = HashMap::new();
    let mut parts = Vec::new();
    for root in &roots {
        let mut found = Vec::new();
        collect(root, "terminal", &mut found);
        for t in found {
            terminals.insert(attr(t, "P_PART_TERMINAL_NAME").unwrap_or_default().to_string(), terminal_points(t));
        }
        let mut found = Vec::new();
        collect(root, "construction", &mut found);
        for c in found {
            constructions.insert(attr(c, "P_PART_CONSTRUCTION_NAME").unwrap_or_default().to_string(), drill_holes(c));
        }
    }
    for root in &roots {
        let mut elements = Vec::new();
        collect(root, "part", &mut elements);
        for el in elements {
            let variants: Vec<&Element> = el.all("variant").collect();
            let read: Vec<Part> = if variants.is_empty() { part_of(el, None).into_iter().collect() } else { variants.into_iter().filter_map(|v| part_of(el, Some(v))).collect() };
            let points = attr(el, "P_ARTICLE_REF_TERMINAL_NAME").and_then(|n| terminals.get(n));
            let holes = attr(el, "P_ARTICLE_REF_CONSTRUCTION_NAME").and_then(|n| constructions.get(n));
            // Where the drilling pattern sits on the part.
            let offset = el.child("constructionPosition").map(|c| [num(attr(c, "offsetx")), num(attr(c, "offsety"))]).unwrap_or_default();
            for mut part in read {
                if let Some(points) = points {
                    part.connection_points = connection_points(points, part.height);
                }
                if let Some((holes, records)) = holes {
                    if !records.is_empty() {
                        let mut data = part.source_data.as_ref().map(|d| d.as_ref().clone()).unwrap_or_else(|| serde_json::json!({}));
                        if let Some(map) = data.as_object_mut() {
                            map.insert("eplan_drilling".into(), serde_json::Value::Array(records.clone()));
                            part.source_data = Some(data.into());
                        }
                    }
                    let (width, height) = (part.width, part.height);
                    let md = &mut part.mounting_data;
                    let at = |p: &[f32; 2]| [p[0] + offset[0], height - (p[1] + offset[1])];
                    let found: Vec<[f32; 2]> = holes.iter().filter_map(|h| match h { Drill::Hole(p, _) | Drill::Thread(p, _) => Some(at(p)), Drill::Cutout(..) => None }).collect();
                    if !found.is_empty() {
                        md.holes = found;
                    }
                    // One size for all holes: the thread of tapped ones, otherwise the
                    // diameter of the through holes.
                    let thread = holes.iter().find_map(|h| match h { Drill::Thread(_, d) if *d > 0.0 => Some(format!("M{d}")), _ => None });
                    let hole = holes.iter().find_map(|h| match h { Drill::Hole(_, d) if *d > 0.0 => Some(format!("Ø{d}")), _ => None });
                    if let Some(size) = thread.or(hole) {
                        md.hole_thread = size;
                    }
                    let cutouts: Vec<PlacedCutout> = holes
                        .iter()
                        .filter_map(|h| match h {
                            Drill::Cutout(p, shape, angle) => {
                                let [x, y] = at(p);
                                Some(PlacedCutout { shape: *shape, offset: [x - width / 2.0, y - height / 2.0], angle: *angle, radius: 0.0 })
                            }
                            _ => None,
                        })
                        .collect();
                    if !cutouts.is_empty() {
                        md.cutouts = cutouts;
                    }
                }
                parts.push(part);
            }
        }
    }
    if parts.is_empty() {
        return Err("the file has no EPLAN parts (P_ARTICLE_PARTNR)".into());
    }
    Ok(parts)
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\n', "&#10;")
}

/// The parts as EPLAN parts XML (texts in `en_US`), for the import into EPLAN.
pub fn to_eplan_xml(parts: &[Part]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<partsmanagement>\n");
    for p in parts {
        let c = productgroups::classify(p);
        let lang = |t: &str| if t.is_empty() { String::new() } else { format!("en_US@{t};") };
        let n = |v: f32| if v == 0.0 { String::new() } else { format!("{v}") };
        let attrs = [
            ("P_ARTICLE_PARTNR", p.number.clone()),
            ("P_ARTICLE_TYPENR", p.type_number.clone()),
            ("P_ARTICLE_ORDERNR", p.order_number.clone()),
            ("P_ARTICLE_MANUFACTURER", p.manufacturer.clone()),
            ("P_ARTICLE_SUPPLIER", p.supplier.clone()),
            ("P_ARTICLE_DESCR1", lang(&p.description)),
            ("P_ARTICLE_NOTE", lang(&p.note)),
            ("P_ARTICLE_ERPNR", p.identifier.clone()),
            ("P_ARTICLE_SALESPRICE_1", n(p.price)),
            ("P_ARTICLE_PRICEUNIT", if p.price_unit > 0 { p.price_unit.to_string() } else { String::new() }),
            ("P_ARTICLE_PACKAGINGQUANTITY", if p.pack_quantity > 0 { p.pack_quantity.to_string() } else { String::new() }),
            ("P_ARTICLE_QUANTITYUNIT", p.quantity_unit.clone()),
            ("P_ARTICLE_WIDTH", n(p.width)),
            ("P_ARTICLE_HEIGHT", n(p.height)),
            ("P_ARTICLE_MIDDLEOFFSET", n(p.effective_rail_mount().center_offset)),
            ("P_ARTICLE_SNAPHEIGHT", n(p.mounting_data.rail.clip_on)),
            ("P_ARTICLE_DEPTH", n(p.depth)),
            ("P_ARTICLE_WEIGHT", n(p.weight)),
            ("P_ARTICLE_VOLTAGE", p.voltage.clone()),
            ("P_ARTICLE_ELECTRICALCURRENT", p.current.clone()),
            ("P_ARTICLE_ELECTRICALPOWER", p.power.clone()),
            ("P_ARTICLE_CERTIFICATE", p.certificates.clone()),
            ("P_ARTICLE_CHARACTERISTICS", lang(&p.tech_data)),
            ("P_ARTICLE_PRODUCTTOPGROUP", c.top_group.to_string()),
            ("P_ARTICLE_PRODUCTGROUP", c.group.to_string()),
            ("P_ARTICLE_PRODUCTSUBGROUP", c.subgroup.to_string()),
            ("P_ARTICLE_ISACCESSORY", if p.accessory { "1".into() } else { String::new() }),
        ];
        out.push_str("  <part");
        for (k, v) in attrs.iter().filter(|(_, v)| !v.is_empty()) {
            let _ = write!(out, " {k}=\"{}\"", esc(v));
        }
        out.push_str(" />\n");
    }
    out.push_str("</partsmanagement>\n");
    out
}

#[cfg(test)]
mod tests;
