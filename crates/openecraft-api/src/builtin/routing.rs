//! 3D routing commands: routing rules, routing paths and reservations, the routed wires,
//! cables and tubes with their bundles, branches and shields, and freezing routes
//! for production.

use crate::builtin::common::{enclosure_index, num_in, NO_MAX};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::analysis::Analysis;
use openecraft_core::model::Project;
use openecraft_core::panel::{PLATE_MARGIN, PLATE_Z};
use openecraft_core::routing3d::{self, Class, PathKind, ReserveMode, Reservation, RoutingPath, ShieldEnds};
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Routing;

impl Plugin for Routing {
    fn info(&self) -> Info {
        info("routing", tr_const!("Routing"), tr_const!("Routing rules, paths, reservations, frozen routes and cable shields"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_cable_parts",
        help: tr_const!("Lists cable parts available from the project and the built-in catalog"),
        params: &[],
        run: list_cable_parts,
    },
    Spec {
        name: "apply_cable_part",
        help: tr_const!("Applies a catalog part to a cable, including cores, cross-section and declared outer diameter"),
        params: &[
            param("cable", Kind::Str, tr_const!("Cable tag")),
            param("part", Kind::Str, tr_const!("Part number")),
        ],
        run: apply_cable_part,
    },
    Spec {
        name: "routing_rules",
        help: tr_const!("Shows the rules of the 3D routing and changes those given: grid, clearance, free air factor, segregation, bend radii, cut lengths, reserves"),
        params: &[
            optional("wires_3d", Kind::Bool, tr_const!("Route the wires in 3D as well (otherwise through the cable ducts)")),
            optional("grid", Kind::Num, tr_const!("Edge length of the routing grid in mm")),
            optional("clearance", Kind::Num, tr_const!("Distance from devices, rails and busbars in mm")),
            optional("free_factor", Kind::Num, tr_const!("Cost of free air against ducts and routing paths")),
            optional("segregation", Kind::Num, tr_const!("Distance between power and signal or fluid in mm")),
            optional("segregation_factor", Kind::Num, tr_const!("Cost of running closer than the segregation distance")),
            optional("bend_wire", Kind::Num, tr_const!("Bend radius of wires as a multiple of the diameter")),
            optional("bend_cable", Kind::Num, tr_const!("Bend radius of cables as a multiple of the diameter")),
            optional("bend_tube", Kind::Num, tr_const!("Bend radius of tubes as a multiple of the diameter")),
            optional("cut_wire", Kind::Num, tr_const!("Cut lengths of wires rounded up to this step in mm")),
            optional("cut_cable", Kind::Num, tr_const!("Cut lengths of cables rounded up to this step in mm")),
            optional("cut_tube", Kind::Num, tr_const!("Cut lengths of tubes rounded up to this step in mm")),
            optional("cable_reserve", Kind::Num, tr_const!("Length added to each cable end in mm")),
            optional("pigtail", Kind::Num, tr_const!("Length of a shield pigtail in mm")),
        ],
        run: routing_rules,
    },
    Spec {
        name: "add_routing_path",
        help: tr_const!("Adds a routing path (cable tray, tube guide) over the mounting plate that routes prefer"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("points", Kind::Points, tr_const!("Corner points on the mounting plate in mm")),
            optional("height", Kind::Num, tr_const!("Height above the mounting plate in mm (default 50)")),
            optional("width", Kind::Num, tr_const!("Width of the path in mm (default 50)")),
            optional("kind", Kind::Str, tr_const!("any, wires, cables or tubes")),
            optional("classes", Kind::Str, tr_const!("Allowed classes, comma separated: power, control, signal, fluid (default all)")),
            optional("reserve", Kind::Num, tr_const!("Share of the capacity kept free in percent")),
            optional("name", Kind::Str, tr_const!("Name")),
        ],
        run: add_routing_path,
    },
    Spec {
        name: "add_routing_reservation",
        help: tr_const!("Reserves space over the mounting plate: kept free of routes, or for one class only"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("x", Kind::Num, tr_const!("Left edge on the mounting plate in mm")),
            param("y", Kind::Num, tr_const!("Top edge on the mounting plate in mm")),
            param("w", Kind::Num, tr_const!("Width in mm")),
            param("h", Kind::Num, tr_const!("Height in mm")),
            optional("mode", Kind::Str, tr_const!("keepout (default), power, control, signal or fluid")),
            optional("name", Kind::Str, tr_const!("Name")),
        ],
        run: add_routing_reservation,
    },
    Spec {
        name: "remove_routing_object",
        help: tr_const!("Removes a routing path or reservation"),
        params: &[param("enclosure", Kind::Str, ENCLOSURE), param("id", Kind::Int, tr_const!("Id of the path or reservation"))],
        run: remove_routing_object,
    },
    Spec {
        name: "list_routing",
        help: tr_const!("Lists the 3D routing: routed wires, cables and tubes with lengths and status, bundles, branches, shields, routing paths, changes and problems"),
        params: &[optional("kind", Kind::Str, tr_const!("Only wire, cable or tube"))],
        run: list_routing,
    },
    Spec {
        name: "freeze_routes",
        help: tr_const!("Freezes routes for production (kept by the recalculation while possible) or releases them"),
        params: &[
            optional("keys", Kind::Str, tr_const!("Keys of the routes, comma separated (all if missing)")),
            optional("release", Kind::Bool, tr_const!("Release instead of freeze")),
        ],
        run: freeze_routes,
    },
    Spec {
        name: "set_cable_shield",
        help: tr_const!("Sets the shield and the outer diameter of a cable"),
        params: &[
            param("cable", Kind::Str, tr_const!("Cable tag")),
            optional("shielded", Kind::Bool, tr_const!("The cable has an overall shield")),
            optional("ends", Kind::Str, tr_const!("Shield connected at: both, first, second or none")),
            optional("diameter", Kind::Num, tr_const!("Outer diameter in mm, 0 = estimated")),
        ],
        run: set_cable_shield,
    },
];

const ENCLOSURE: &str = tr_const!("Enclosure (location, e.g. +CAB1)");

fn rules_json(r: &routing3d::RoutingRules) -> Value {
    serde_json::to_value(r).unwrap_or(Value::Null)
}

fn routing_rules(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let r = &mut prj.layout.routing;
    if let Some(v) = a.value("wires_3d").and_then(Value::as_bool) {
        r.wires_3d = v;
    }
    for (name, field) in [
        ("grid", &mut r.grid),
        ("clearance", &mut r.clearance),
        ("free_factor", &mut r.free_factor),
        ("segregation", &mut r.segregation),
        ("segregation_factor", &mut r.segregation_factor),
        ("bend_wire", &mut r.bend_wire),
        ("bend_cable", &mut r.bend_cable),
        ("bend_tube", &mut r.bend_tube),
        ("cut_wire", &mut r.cut_wire),
        ("cut_cable", &mut r.cut_cable),
        ("cut_tube", &mut r.cut_tube),
        ("cable_reserve", &mut r.cable_reserve),
        ("pigtail", &mut r.pigtail),
    ] {
        if let Some(v) = num_in(a, name, 0.0, NO_MAX)? {
            *field = v;
        }
    }
    r.grid = r.grid.max(5.0);
    Ok(rules_json(r))
}

fn classes_arg(a: &Args) -> Result<Vec<Class>, String> {
    let Some(list) = a.str("classes") else { return Ok(Vec::new()) };
    crate::builtin::common::words(list).into_iter().map(|c| Class::from_key(c).ok_or_else(|| tr_fmt!("Unknown class: {}", c))).collect()
}

fn add_routing_path(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let kind = match a.str("kind") {
        Some(k) => PathKind::from_key(k).ok_or_else(|| tr_fmt!("Unknown path kind: {}", k))?,
        None => PathKind::Any,
    };
    let classes = classes_arg(a)?;
    let z = PLATE_Z + a.num("height").unwrap_or(50.0);
    let points: Vec<[f32; 3]> = a.points("points").into_iter().map(|[x, y]| [PLATE_MARGIN + x, PLATE_MARGIN + y, z]).collect();
    if points.len() < 2 {
        return Err(tr_fmt!("A routing path needs {} points at least", 2));
    }
    let id = prj.new_id();
    let name = a.str("name").map(String::from).unwrap_or_else(|| format!("P{}", prj.layout.enclosures[ei].paths.len() + 1));
    let path = RoutingPath { id, name, kind, points, width: num_in(a, "width", 1.0, NO_MAX)?.unwrap_or(50.0), classes, reserve: num_in(a, "reserve", 0.0, 90.0)?.unwrap_or(0.0), face: None };
    let out = serde_json::to_value(&path).unwrap_or(Value::Null);
    prj.layout.enclosures[ei].paths.push(path);
    Ok(out)
}

fn add_routing_reservation(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let mode = match a.str("mode").map(|m| m.trim().to_ascii_lowercase()) {
        None => ReserveMode::Keepout,
        Some(m) if m == "keepout" => ReserveMode::Keepout,
        Some(m) => ReserveMode::For(Class::from_key(&m).ok_or_else(|| tr_fmt!("Unknown class: {}", m))?),
    };
    let (x, y, w, h) = (a.num("x").unwrap_or(0.0), a.num("y").unwrap_or(0.0), a.num("w").unwrap_or(0.0).abs(), a.num("h").unwrap_or(0.0).abs());
    let depth = prj.layout.enclosures[ei].depth;
    let id = prj.new_id();
    let name = a.str("name").map(String::from).unwrap_or_else(|| format!("R{}", prj.layout.enclosures[ei].reservations.len() + 1));
    let r = Reservation { id, name, min: [PLATE_MARGIN + x, PLATE_MARGIN + y, PLATE_Z], max: [PLATE_MARGIN + x + w, PLATE_MARGIN + y + h, depth], mode };
    let out = serde_json::to_value(&r).unwrap_or(Value::Null);
    prj.layout.enclosures[ei].reservations.push(r);
    Ok(out)
}

fn remove_routing_object(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let id = a.int("id").unwrap_or(0) as u64;
    let e = &mut prj.layout.enclosures[ei];
    let before = e.paths.len() + e.reservations.len();
    e.paths.retain(|p| p.id != id);
    e.reservations.retain(|r| r.id != id);
    if e.paths.len() + e.reservations.len() == before {
        return Err(tr_fmt!("No routing path or reservation {}", id));
    }
    Ok(json!({ "removed": id }))
}

fn list_routing(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let an = Analysis::build(prj);
    let r = &an.routing;
    let name = |k: usize| prj.layout.enclosures.get(k).map(|e| e.name.clone()).unwrap_or_default();
    let kind = a.str("kind").map(|k| k.trim().to_ascii_lowercase());
    let items: Vec<Value> = r
        .items
        .iter()
        .enumerate()
        .filter(|(_, i)| kind.as_deref().is_none_or(|k| i.kind.key() == k))
        .map(|(idx, i)| {
            json!({
                "key": i.key,
                "kind": i.kind.key(),
                "label": i.label,
                "class": i.class.key(),
                "from": i.ends[0].tag,
                "to": i.ends[1].tag,
                "enclosures": i.enclosures.iter().map(|&k| name(k)).collect::<Vec<_>>(),
                "length_mm": i.length.round(),
                "cut_mm": i.cut,
                "free_air_mm": i.free.round(),
                "bends": i.bends,
                "diameter_mm": (i.diameter * 10.0).round() / 10.0,
                "shielded": i.shielded,
                "status": i.status_text(),
                "bundles": r.bundles_of(idx),
            })
        })
        .collect();
    let bundles: Vec<Value> = r
        .bundles
        .iter()
        .map(|b| json!({ "items": b.items.iter().map(|&i| r.items[i].key.clone()).collect::<Vec<_>>(), "length_mm": b.length, "diameter_mm": (b.diameter * 10.0).round() / 10.0, "classes": b.classes.iter().map(|c| c.key()).collect::<Vec<_>>() }))
        .collect();
    let branches: Vec<Value> = r.branches.iter().map(|b| json!({ "at": b.at, "bundles": b.bundles, "items": b.items })).collect();
    let shields: Vec<Value> = r
        .shields
        .iter()
        .map(|s| json!({ "cable": s.cable, "bus": s.bus, "ends": s.ends.iter().map(|e| json!({ "at": e.tag, "termination": e.termination.text() })).collect::<Vec<_>>() }))
        .collect();
    let paths: Vec<Value> = r.paths.iter().map(|p| json!({ "enclosure": name(p.enclosure), "id": p.path, "items": p.items, "area_mm2": p.area.round(), "capacity_mm2": p.capacity.round() })).collect();
    let changes: Vec<Value> = r.changes.iter().map(|c| json!({ "key": c.key, "label": c.label, "frozen_mm": c.before.round(), "now_mm": c.after.map(f32::round) })).collect();
    let problems: Vec<&str> = r.problems.iter().map(|p| p.message.as_str()).collect();
    Ok(json!({ "items": items, "bundles": bundles, "branches": branches, "shields": shields, "paths": paths, "frozen": prj.layout.frozen.len(), "changes": changes, "problems": problems }))
}

fn freeze_routes(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let keys: Vec<String> = a.str("keys").map(|k| k.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    if a.flag("release") {
        let n = routing3d::release(prj, &keys);
        return Ok(json!({ "released": n, "frozen": prj.layout.frozen.len() }));
    }
    let an = Analysis::build(prj);
    if let Some(k) = keys.iter().find(|k| an.routing.item(k).is_none_or(|i| !i.routed())) {
        return Err(tr_fmt!("No routed item {}", k));
    }
    let n = routing3d::freeze(prj, &an.routing, &keys);
    Ok(json!({ "frozen": n, "total": prj.layout.frozen.len() }))
}

fn set_cable_shield(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag = a.req("cable");
    let c = prj.cables.iter_mut().find(|c| c.tag == tag).ok_or_else(|| tr_fmt!("No cable {}", tag))?;
    if let Some(v) = a.value("shielded").and_then(Value::as_bool) {
        c.shielded = Some(v);
    }
    if let Some(e) = a.str("ends") {
        c.shield_ends = ShieldEnds::from_key(e).ok_or_else(|| tr_fmt!("Unknown shield ends: {}", e))?;
    }
    if let Some(d) = num_in(a, "diameter", 0.0, NO_MAX)? {
        c.diameter = d;
    }
    Ok(json!({
        "cable": c.tag,
        "shielded": routing3d::is_shielded(c),
        "ends": c.shield_ends.key(),
        "diameter_mm": (routing3d::cable_diameter(c) * 10.0).round() / 10.0,
    }))
}

fn apply_cable_part(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag=a.req("cable");
    let index=prj.cables.iter().position(|c|c.tag==tag).ok_or_else(||tr_fmt!("No cable {}",tag))?;
    let number=a.req("part");
    let catalog=openecraft_core::parts::catalog();
    let part=prj.parts.iter().chain(catalog.iter()).find(|p|p.number==number).cloned().ok_or_else(||tr_fmt!("No part {}",number))?;
    if part.group!="Cables" {return Err(tr_fmt!("Part {} is not a cable",number));}
    if !prj.parts.iter().any(|p|p.number==number) {prj.parts.push(part.clone());}
    let cable=&mut prj.cables[index];
    openecraft_core::ops::apply_cable_part(cable,&part);
    Ok(json!({"cable":cable.tag,"part":cable.part,"cable_type":cable.cable_type,"cores":cable.cores,
              "cross_section":cable.cross_section,"diameter_mm":cable.diameter}))
}

fn list_cable_parts(prj: &mut Project, _a: &Args) -> Result<Value, String> {
    let choices:Vec<Value>=openecraft_core::parts::group_choices(&prj.parts,None,"Cables").into_iter().map(|part|json!({
        "number":part.number,"type_number":part.type_number,"description":part.description,"manufacturer":part.manufacturer,
        "cores":part.tech_value("Cores"),"cross_section":part.tech_value("Cross-section"),
        "diameter_mm":part.dimension_mm("Outer diameter").filter(|d|*d>0.0),
    })).collect();
    Ok(json!(choices))
}

#[cfg(test)]
mod tests;
