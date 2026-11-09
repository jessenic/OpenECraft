//! Device box commands: freely drawn device boxes and the device connection points
//! placed on them (`openecraft_core::device_boxes`).

use crate::builtin::common::*;
use crate::params::{optional, param, Args, Kind, Param};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::device_boxes as boxes;
use openecraft_core::geom::{p, P, R, GRID};
use openecraft_core::model::{DeviceBoxRole, PinLabel, Project, SymbolInst};
use openecraft_core::symbols::Anchor;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct DeviceBoxes;

impl Plugin for DeviceBoxes {
    fn info(&self) -> Info {
        info("device_boxes", tr_const!("Device boxes"), tr_const!("Freely drawn device boxes with individually placed device connection points"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

const ID: Param = param("id", Kind::Int, tr_const!("Item id"));
const ROLE: Param = optional("role", Kind::Str, tr_const!("What the box is: device (default), extension (more connection points of a device placed elsewhere) or view (shows connection points a device placed elsewhere already has)"));
const DIRECTION: Param = optional("direction", Kind::Str, tr_const!("Direction the connection leads away: up, right, down or left (default: away from the edge of the box it is on)"));

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "add_device_box",
        help: tr_const!("Draws a device box of any size as a new device; device connection points placed on it belong to its device"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position of the top left corner in mm")),
            param("y", Kind::Num, tr_const!("Y position of the top left corner in mm (downwards)")),
            param("width", Kind::Num, tr_const!("Width in mm")),
            param("height", Kind::Num, tr_const!("Height in mm")),
            optional("tag", Kind::Str, tr_const!("Device tag, e.g. -A1 (default: the next free -A tag)")),
            optional("function_text", Kind::Str, tr_const!("Function text")),
            ROLE,
        ],
        run: add_device_box,
    },
    Spec {
        name: "update_device_box",
        help: tr_const!("Changes a device box: its outline (points on a moved edge and their wires go with it), its device tag (its connection points go with it) or its function text"),
        params: &[
            PAGE,
            ID,
            optional("x", Kind::Num, tr_const!("X position of the top left corner in mm")),
            optional("y", Kind::Num, tr_const!("Y position of the top left corner in mm (downwards)")),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("height", Kind::Num, tr_const!("Height in mm")),
            optional("tag", Kind::Str, tr_const!("New device tag of the box and its connection points")),
            optional("function_text", Kind::Str, tr_const!("Function text")),
            ROLE,
        ],
        run: update_device_box,
    },
    Spec {
        name: "add_device_connections",
        help: tr_const!("Places device connection points, one after the other at a distance: on a device box they belong to its device; designations not given are numbered on"),
        params: &[
            PAGE,
            param("x", Kind::Num, tr_const!("X position of the first point in mm (on the outline of the box)")),
            param("y", Kind::Num, tr_const!("Y position of the first point in mm (downwards)")),
            DIRECTION,
            optional("designations", Kind::Str, tr_const!("Connection point designations, comma separated, e.g. L,N,PE (default: the next numbers)")),
            optional("count", Kind::Int, tr_const!("Number of points (default: one per designation, else 1)")),
            optional("dx", Kind::Num, tr_const!("Distance to the next point in x, mm (default: 8 along a horizontal edge)")),
            optional("dy", Kind::Num, tr_const!("Distance to the next point in y, mm (default: 8 along a vertical edge)")),
            optional("tag", Kind::Str, tr_const!("Device tag (default: the device of the box at the point)")),
        ],
        run: add_device_connections,
    },
    Spec {
        name: "update_device_connection",
        help: tr_const!("Changes a device connection point: its designation, direction, device box, or the position and alignment of its designation"),
        params: &[
            PAGE,
            ID,
            optional("designation", Kind::Str, tr_const!("Connection point designation, e.g. +24V")),
            DIRECTION,
            optional("box", Kind::Int, tr_const!("Id of the device box whose device the point belongs to")),
            optional("label_dx", Kind::Num, tr_const!("Position of the designation relative to the point in x, mm")),
            optional("label_dy", Kind::Num, tr_const!("Position of the designation relative to the point in y, mm")),
            optional("label_align", Kind::Str, tr_const!("Alignment of the designation: left, center or right")),
            optional("label_auto", Kind::Bool, tr_const!("Put the designation back beside the point")),
        ],
        run: update_device_connection,
    },
    Spec {
        name: "show_device_pins",
        help: tr_const!("Makes a pin view show exactly the connection points named: new ones are placed on its outline, the others are removed"),
        params: &[PAGE, ID, param("designations", Kind::Str, tr_const!("Connection point designations to show, comma separated"))],
        run: show_device_pins,
    },
    Spec {
        name: "list_device_boxes",
        help: tr_const!("Lists the device boxes of a page with their outline and their device connection points"),
        params: &[PAGE],
        run: list_device_boxes,
    },
];

fn direction(a: &Args) -> Result<Option<u8>, String> {
    match a.str("direction").map(|d| d.trim().to_ascii_lowercase()) {
        None => Ok(None),
        Some(d) => match d.as_str() {
            "up" => Ok(Some(0)),
            "right" => Ok(Some(1)),
            "down" => Ok(Some(2)),
            "left" => Ok(Some(3)),
            _ => Err(tr_fmt!("Unknown direction: {}", d)),
        },
    }
}

fn box_role(a: &Args) -> Result<Option<DeviceBoxRole>, String> {
    match a.str("role").map(|r| r.trim().to_ascii_lowercase()) {
        None => Ok(None),
        Some(r) => match r.as_str() {
            "device" => Ok(Some(DeviceBoxRole::Device)),
            "extension" => Ok(Some(DeviceBoxRole::Extension)),
            "view" => Ok(Some(DeviceBoxRole::View)),
            _ => Err(tr_fmt!("Unknown device box role: {}", r)),
        },
    }
}

fn role_name(role: DeviceBoxRole) -> &'static str {
    match role {
        DeviceBoxRole::Device => "device",
        DeviceBoxRole::Extension => "extension",
        DeviceBoxRole::View => "view",
    }
}

fn direction_name(rot: u8) -> &'static str {
    ["up", "right", "down", "left"][(rot % 4) as usize]
}

fn anchor(a: &Args) -> Result<Option<Anchor>, String> {
    match a.str("label_align").map(|d| d.trim().to_ascii_lowercase()) {
        None => Ok(None),
        Some(d) => match d.as_str() {
            "left" => Ok(Some(Anchor::Left)),
            "center" => Ok(Some(Anchor::Center)),
            "right" => Ok(Some(Anchor::Right)),
            _ => Err(tr_fmt!("Unknown alignment: {}", d)),
        },
    }
}

fn find(prj: &Project, page: usize, id: u64) -> Option<&SymbolInst> {
    prj.pages[page].symbols.iter().find(|s| s.id == id)
}

fn point_json(prj: &Project, page: usize, s: &SymbolInst) -> Value {
    let symbols = prj.symbol_table();
    let mut v = symbol_json(&symbols, s);
    v["designation"] = json!(s.pins.first().cloned().unwrap_or_default());
    v["direction"] = json!(direction_name(s.rot));
    v["box"] = json!(boxes::box_of(prj, &prj.pages[page], s));
    if let Some(l) = s.pin_label {
        v["designation_at"] = json!({ "dx": l.offset.x, "dy": l.offset.y, "align": format!("{:?}", l.anchor).to_lowercase() });
    }
    v
}

fn box_json(prj: &Project, page: usize, id: u64) -> Value {
    let Some(b) = find(prj, page, id) else { return Value::Null };
    let r = boxes::rect(prj, b).unwrap_or(R::nothing());
    let points: Vec<Value> = boxes::points_of(prj, &prj.pages[page], id).into_iter().filter_map(|i| find(prj, page, i)).map(|s| point_json(prj, page, s)).collect();
    json!({
        "id": b.id,
        "tag": b.tag,
        "function_text": b.function_text,
        "role": role_name(b.box_role),
        "device_pins": boxes::device_pins(prj, &b.tag),
        "x": r.min.x,
        "y": r.min.y,
        "width": r.width(),
        "height": r.height(),
        "points": points,
    })
}

fn the_box(prj: &Project, page: usize, id: u64) -> Result<&SymbolInst, String> {
    find(prj, page, id).filter(|s| boxes::is_box(s)).ok_or_else(|| tr_fmt!("No device box with id {}", id))
}

fn add_device_box(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let tag = a.str("tag").unwrap_or_default();
    if !tag.trim().is_empty() {
        openecraft_core::ops::check_tag(tag)?;
    }
    let role = box_role(a)?;
    if role.is_some_and(|r| !r.is_device()) && tag.trim().is_empty() {
        return Err(tr("A device box that extends or shows a device needs the tag of that device").to_string());
    }
    let at = p(a.num("x").unwrap_or_default(), a.num("y").unwrap_or_default());
    let size = p(a.num("width").unwrap_or_default(), a.num("height").unwrap_or_default());
    let id = boxes::add_box(prj, i, at, at + size, tag, a.str("function_text").unwrap_or_default())?;
    if let Some(role) = role {
        boxes::set_role(prj, i, id, role)?;
    }
    Ok(box_json(prj, i, id))
}

fn update_device_box(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let r = boxes::rect(prj, the_box(prj, i, id)?).unwrap_or(R::nothing());
    let min = p(a.num("x").unwrap_or(r.min.x), a.num("y").unwrap_or(r.min.y));
    let size = p(a.num("width").unwrap_or(r.width()), a.num("height").unwrap_or(r.height()));
    let to = R::from_points(min, min + size);
    if to != r {
        // A box that only moves takes its points and their wires along.
        if (size.x - r.width()).abs() < 0.01 && (size.y - r.height()).abs() < 0.01 {
            let sel = boxes::with_points(prj, i, &std::collections::BTreeSet::from([id]));
            openecraft_core::routing::move_selection(prj, i, &sel, (min - r.min).snap(GRID));
        } else {
            boxes::resize(prj, i, id, to)?;
        }
    }
    if let Some(tag) = a.str("tag") {
        openecraft_core::ops::check_tag(tag)?;
        let pos = find(prj, i, id).map_or(p(0.0, 0.0), |s| s.pos);
        let tag = openecraft_core::structure::scope(&prj.pages[i], pos).complete(tag.trim());
        boxes::set_tag(prj, i, id, &tag);
    }
    if let Some(t) = a.str("function_text") {
        if let Some(s) = prj.pages[i].symbols.iter_mut().find(|s| s.id == id) {
            s.function_text = t.into();
        }
    }
    if let Some(role) = box_role(a)? {
        boxes::set_role(prj, i, id, role)?;
    }
    Ok(box_json(prj, i, id))
}

fn show_device_pins(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let names: Vec<String> = a.str("designations").unwrap_or_default().split(',').map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect();
    boxes::show_pins(prj, i, id, &names)?;
    Ok(box_json(prj, i, id))
}

fn add_device_connections(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let start = p(a.num("x").unwrap_or_default(), a.num("y").unwrap_or_default()).snap(GRID);
    let names: Vec<String> = a.str("designations").map(|d| d.split(',').map(|s| s.trim().to_string()).collect()).unwrap_or_default();
    let count = a.int("count").unwrap_or(names.len().max(1) as i64);
    if !(1..=200).contains(&count) {
        return Err(tr("The number of points must be between 1 and 200").to_string());
    }
    let rot = match direction(a)? {
        Some(r) => r,
        None => boxes::direction_at(prj, &prj.pages[i], start).unwrap_or(0),
    };
    // Along the edge the points are on: across their direction.
    let along = if rot % 2 == 0 { p(8.0, 0.0) } else { p(0.0, 8.0) };
    let step = p(a.num("dx").unwrap_or(along.x), a.num("dy").unwrap_or(along.y));
    let tag = a.str("tag").unwrap_or_default();
    if !tag.trim().is_empty() {
        openecraft_core::ops::check_tag(tag)?;
    }
    let before = prj.clone();
    let mut out = Vec::new();
    let mut last = String::new();
    for n in 0..count as usize {
        let at: P = start + step * n as f32;
        // Designations given go first, then the numbers go on from the last one.
        let designation = names.get(n).cloned().filter(|d| !d.is_empty()).or_else(|| boxes::increment(&last)).unwrap_or_default();
        match boxes::place_point(prj, i, at, rot, tag, &designation) {
            Ok(id) => {
                let s = find(prj, i, id).cloned().unwrap_or_default();
                last = s.pins.first().cloned().unwrap_or_default();
                out.push(point_json(prj, i, &s));
            }
            Err(e) => {
                *prj = before;
                return Err(e);
            }
        }
    }
    Ok(Value::Array(out))
}

fn update_device_connection(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    if !find(prj, i, id).is_some_and(|s| boxes::is_point(prj, s)) {
        return Err(tr_fmt!("No device connection point with id {}", id));
    }
    if let Some(b) = a.int("box") {
        boxes::assign(prj, i, id, b as u64)?;
    }
    let rot = direction(a)?;
    let align = anchor(a)?;
    let s = prj.pages[i].symbols.iter_mut().find(|s| s.id == id).ok_or_else(|| tr_fmt!("No device connection point with id {}", id))?;
    if let Some(d) = a.str("designation") {
        s.pins = vec![d.trim().to_string()];
    }
    if let Some(r) = rot {
        s.rot = r;
    }
    if a.flag("label_auto") {
        s.pin_label = None;
    }
    if a.value("label_dx").is_some() || a.value("label_dy").is_some() || align.is_some() {
        let now = s.pin_label.unwrap_or(PinLabel { offset: p(-1.0, -5.0), anchor: Anchor::Right });
        s.pin_label = Some(PinLabel {
            offset: p(a.num("label_dx").unwrap_or(now.offset.x), a.num("label_dy").unwrap_or(now.offset.y)),
            anchor: align.unwrap_or(now.anchor),
        });
    }
    let s = s.clone();
    Ok(point_json(prj, i, &s))
}

fn list_device_boxes(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = page_index(prj, a.req("page"))?;
    Ok(Value::Array(boxes::boxes(&prj.pages[i]).into_iter().map(|(id, _, _)| box_json(prj, i, id)).collect()))
}

#[cfg(test)]
mod tests;
