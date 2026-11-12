//! Helpers shared by the built-in commands and the tools of the command line: pages by
//! name, JSON of pages and symbols, reports by name.

use crate::params::{param, Args, Kind, Param};
use crate::Plugins;
use openecraft_core::geom::P;
use openecraft_core::model::*;
use openecraft_core::structure;
use openecraft_core::symlib::Symbols;
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub const PAGE: Param = param("page", Kind::Str, tr_const!("Page name as shown by list_pages"));
pub const SECTION: Param = param("section", Kind::Str, tr_const!("Section number as shown by list_sections, e.g. 2.3"));

/// `DeviceList` → `device_list`.
pub fn snake(debug: impl std::fmt::Debug) -> String {
    let mut out = String::new();
    for (i, c) in format!("{debug:?}").chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// A report of the registry (built-in or of a plugin) by name.
pub fn report_id(plugins: &Plugins, name: &str) -> Result<ReportId, String> {
    let name = name.trim().to_ascii_lowercase().replace('-', "_");
    plugins.report(&name).map(|r| ReportId::new(&r.name)).ok_or_else(|| tr_fmt!("Unknown report: {}", name))
}

pub fn kind_name(kind: PageKind) -> String {
    match kind {
        PageKind::Schematic => "schematic".into(),
        PageKind::Cover => "cover".into(),
        PageKind::Report(r) => r.as_str().into(),
        PageKind::Mounting(_) => "mounting".into(),
        PageKind::Overview => "overview".into(),
        PageKind::Graphic => "graphic".into(),
        PageKind::Document => "document".into(),
    }
}

pub fn page_index(prj: &Project, name: &str) -> Result<usize, String> {
    prj.pages.iter().position(|pg| pg.name == name).ok_or_else(|| tr_fmt!("No page named {}", name))
}

/// Section by its full number (`2.3`); empty = the top level (0).
pub fn section_id(prj: &Project, number: &str) -> Result<u64, String> {
    let number = number.trim();
    if number.is_empty() {
        return Ok(0);
    }
    prj.sections.iter().map(|s| s.id).find(|&id| openecraft_core::sections::full_number(prj, id) == number).ok_or_else(|| tr_fmt!("No section numbered {}", number))
}

/// The subproject page `i` is taken out in (it is locked here meanwhile).
pub fn out_in_subproject(prj: &Project, i: usize) -> Option<&str> {
    let id = prj.pages[i].id;
    prj.subprojects.children.iter().find(|c| c.out && c.pages.contains(&id)).map(|c| c.name.as_str())
}

/// Why page `i` is locked: taken out in a subproject or released.
pub fn locked_reason(prj: &Project, i: usize) -> String {
    let name = &prj.pages[i].name;
    match out_in_subproject(prj, i) {
        Some(sub) => tr_fmt!("Page {} is out in subproject {}: insert or discard the subproject first", name, sub),
        None => tr_fmt!("Page {} is locked", name),
    }
}

/// Page whose properties may be changed (not released).
pub fn unlocked(prj: &Project, name: &str) -> Result<usize, String> {
    let i = page_index(prj, name)?;
    if prj.pages[i].locked {
        return Err(locked_reason(prj, i));
    }
    Ok(i)
}

/// The words of a list: separated by commas, semicolons or blanks (codes, tags,
/// kinds and classes contain none of them).
pub fn words(text: &str) -> Vec<&str> {
    text.split(|c: char| c == ',' || c == ';' || c.is_whitespace()).filter(|w| !w.is_empty()).collect()
}

/// A wire colour of IEC 60757 (`BK`, also two colours such as `GNYE` or `WH/BU`);
/// empty stays empty (the default).
pub fn wire_color(v: &str) -> Result<String, String> {
    let v = v.trim();
    if v.is_empty() || !openecraft_core::potentials::color_parts(v).is_empty() {
        Ok(v.to_string())
    } else {
        let codes: Vec<&str> = openecraft_core::potentials::COLORS.iter().map(|c| c.0).collect();
        Err(tr_fmt!("Unknown wire colour {} (IEC 60757: {})", v, codes.join(", ")))
    }
}

/// A cross-section such as `1.5 mm²` or `2,5`; empty stays empty (the default).
pub fn cross_section(v: &str) -> Result<String, String> {
    let v = v.trim();
    if v.is_empty() || openecraft_core::production::cross_section_mm2(v).is_some() {
        Ok(v.to_string())
    } else {
        Err(tr_fmt!("Not a cross-section: {} (e.g. 1.5 mm²)", v))
    }
}

/// The `{…}` placeholders of a format must be among `known`.
pub fn check_placeholders(format: &str, known: &[&str]) -> Result<(), String> {
    let mut rest = format;
    while let Some(start) = rest.find('{') {
        let Some(len) = rest[start..].find('}') else { break };
        let name = &rest[start + 1..start + len];
        if !known.contains(&name) {
            let list: Vec<String> = known.iter().map(|k| format!("{{{k}}}")).collect();
            return Err(tr_fmt!("Unknown placeholder {}: possible are {}", format!("{{{name}}}"), list.join(", ")));
        }
        rest = &rest[start + len + 1..];
    }
    Ok(())
}

/// Page whose content may be changed: like in the editor only unreleased schematic
/// pages, the others are generated.
pub fn editable(prj: &Project, name: &str) -> Result<usize, String> {
    let i = unlocked(prj, name)?;
    if !prj.pages[i].is_editable() {
        return Err(tr_fmt!("Page {} is generated and cannot be edited", name));
    }
    Ok(i)
}

pub fn xy(q: P) -> Value {
    json!([q.x, q.y])
}

/// Connection point designations and page positions of a placed symbol. Points
/// without designation (terminals) are numbered 1, 2, …
pub fn pins_of(symbols: &Symbols, s: &SymbolInst) -> Vec<(String, P)> {
    let Some(def) = symbols.get(&s.sym) else { return Vec::new() };
    def.pins
        .iter()
        .enumerate()
        .map(|(i, pd)| {
            let label = s.pins.get(i).map(String::as_str).unwrap_or(pd.label);
            let label = if label.is_empty() { (i + 1).to_string() } else { label.to_string() };
            (label, def.pin_world(i, s.pos, s.rot, s.mirror))
        })
        .collect()
}

pub fn symbol_json(symbols: &Symbols, s: &SymbolInst) -> Value {
    let pins: Vec<Value> = pins_of(symbols, s).into_iter().map(|(label, q)| json!({ "pin": label, "x": q.x, "y": q.y })).collect();
    json!({
        "id": s.id,
        "symbol": s.sym,
        "tag": s.tag,
        "label": s.label,
        "function_text": s.function_text,
        "x": s.pos.x,
        "y": s.pos.y,
        "rotation": s.rot as u32 * 90,
        "mirror": s.mirror,
        "scale": openecraft_core::symlib::scale_of(&s.sym).1,
        "pins": pins,
    })
}

pub fn page_json(pg: &Page) -> Value {
    json!({
        "name": pg.name,
        "description": pg.description,
        "kind": kind_name(pg.kind),
        "structure": structure::page_structure(pg),
        "function": pg.function,
        "installation": pg.installation,
        "location": pg.location,
        "product": pg.product,
        "locked": pg.locked,
        "frame": pg.frame,
        "document": pg.document.as_ref().map(|d| d.name.clone()),
    })
}

/// No upper bound for [`num_in`].
pub const NO_MAX: f32 = f32::MAX;
/// No upper bound for [`int_in`].
pub const NO_LIMIT: i64 = i64::MAX;

fn range_error(name: &str, min: f64, max: f64, unbounded: bool) -> String {
    match (unbounded, min == 0.0) {
        (true, true) => tr_fmt!("{} must not be negative", name),
        (true, false) => tr_fmt!("{} must be at least {}", name, min),
        (false, _) => tr_fmt!("{} must be between {} and {}", name, min, max),
    }
}

/// A number parameter from `min` to `max` ([`NO_MAX`]: no upper bound); a value out
/// of range is an error instead of a silent correction.
pub fn num_in(a: &Args, name: &str, min: f32, max: f32) -> Result<Option<f32>, String> {
    match a.num(name) {
        Some(v) if !(v.is_finite() && v >= min && v <= max) => Err(range_error(name, min.into(), max.into(), max == NO_MAX)),
        v => Ok(v),
    }
}

/// An integer parameter from `min` to `max` ([`NO_LIMIT`] or the largest `u32`: no
/// upper bound worth naming).
pub fn int_in(a: &Args, name: &str, min: i64, max: i64) -> Result<Option<i64>, String> {
    match a.int(name) {
        Some(v) if !(min..=max).contains(&v) => Err(range_error(name, min as f64, max as f64, max >= i64::from(u32::MAX))),
        v => Ok(v),
    }
}

pub fn enclosure_index(prj: &Project, name: &str) -> Result<usize, String> {
    prj.layout.enclosures.iter().position(|e| e.name == name).ok_or_else(|| tr_fmt!("No enclosure named {}", name))
}
