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
//! `X10.1`, so that the designations of a part stay unique.

use crate::productgroups::{self, Classification};
use crate::xml::{self, Element};
use crate::{ConnectionPoint, Exit, FnKind, FunctionTemplate, Part};
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

/// Connection points of a `<terminal>` definition in EPLAN coordinates (y up from the
/// lower edge) with the direction of their wires.
fn terminal_points(el: &Element) -> Vec<([f32; 3], [f32; 3], String)> {
    el.all("terminalPosition")
        .filter_map(|t| {
            let name = attr(t, "name")?;
            let v = |a: &str| num(attr(t, a));
            Some(([v("xpos"), v("ypos"), v("zpos")], [v("xdir"), v("ydir"), v("zdir")], pin_name(attr(t, "devicetag"), name)))
        })
        .collect()
}

/// The connection points in the part's front view (y down from the upper edge).
fn connection_points(points: &[([f32; 3], [f32; 3], String)], height: f32) -> Vec<ConnectionPoint> {
    points
        .iter()
        .map(|(p, d, pin)| {
            let exit = match d {
                [_, y, _] if *y > 0.5 => Exit::Up,
                [_, y, _] if *y < -0.5 => Exit::Down,
                [x, _, _] if *x < -0.5 => Exit::Left,
                [x, _, _] if *x > 0.5 => Exit::Right,
                [_, _, z] if z.abs() > 0.5 => Exit::Front,
                _ if p[1] < height / 2.0 => Exit::Down,
                _ => Exit::Up,
            };
            ConnectionPoint { pin: pin.clone(), pos: [p[0], height - p[1], p[2]], exit }
        })
        .collect()
}

/// Drill holes of a `<construction>`: position (EPLAN coordinates) and diameter.
fn drill_holes(el: &Element) -> Vec<([f32; 2], f32)> {
    el.all("drillingPosition").map(|h| ([num(attr(h, "xpos")), num(attr(h, "ypos"))], num(attr(h, "dimension1")))).collect()
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
                if let Some(holes) = holes.filter(|h| !h.is_empty()) {
                    let md = &mut part.mounting_data;
                    md.holes = holes.iter().map(|(p, _)| [p[0] + offset[0], part.height - (p[1] + offset[1])]).collect();
                    if let Some(d) = holes.iter().map(|(_, d)| *d).find(|d| *d > 0.0) {
                        md.hole_thread = format!("Ø{d}");
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
