//! Fluid and process commands: hoses / pipes between fluid ports, their defaults and
//! configuration, the fluid netlist, fluid device groups, the links between fluid and
//! electrical devices, and the PCT loops of the pre-planning with their engineering
//! status.

use crate::builtin::common::{int_in, num_in, NO_MAX};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::electrofluid::{self, DeviceGroup, GroupKind};
use openecraft_core::fluid::{self, LineType, Medium};
use openecraft_core::index::Index;
use openecraft_core::model::Project;
use openecraft_core::netlist::Netlist;
use openecraft_core::process::{self, PctLoop, Status};
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Process;

impl Plugin for Process {
    fn info(&self) -> Info {
        info("process", tr_const!("Process engineering"), tr_const!("Hoses, fluid nets and groups, electro-fluid links and PCT loops"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "set_fluid_port_role",
        help: tr_const!("Sets the fluid port function of a user symbol connection point"),
        params: &[
            param("symbol", Kind::Str, tr_const!("Symbol key")),
            param("pin", Kind::Int, tr_const!("Zero-based connection point index")),
            param("role", Kind::Str, tr_const!("Port function, or automatic")),
        ],
        run: set_fluid_port_role,
    },
    Spec {
        name: "list_hoses",
        help: tr_const!("Lists the hoses and pipes (connections between fluid ports) with medium, nominal size, colour and length"),
        params: &[],
        run: list_hoses,
    },
    Spec {
        name: "set_fluid_defaults",
        help: tr_const!("Sets the defaults of hoses that do not set their own values, and the hose number prefix"),
        params: &[
            optional("medium", Kind::Str, tr_const!("compressed_air, vacuum, hydraulic_oil, water, coolant, lubricant, gas, steam or product")),
            optional("size", Kind::Str, tr_const!("Nominal size, e.g. 8 mm or DN25")),
            optional("color", Kind::Str, tr_const!("Hose colour")),
            optional("prefix", Kind::Str, tr_const!("Hose number prefix, e.g. H")),
            optional("pressure", Kind::Num, tr_const!("Operating pressure in bar (0 = hose ratings are not checked)")),
            optional("allowance", Kind::Num, tr_const!("Cutting allowance per tube end in mm")),
        ],
        run: set_fluid_defaults,
    },
    Spec {
        name: "configure_hose",
        help: tr_const!("Configures a hose / tube / pipe: line type, part, end fittings, pressure rating, bend radius, medium, size, colour and length"),
        params: &[
            param("hose", Kind::Str, tr_const!("Hose number or fluid net (F1)")),
            optional("line", Kind::Str, tr_const!("hose, tube or pipe")),
            optional("part", Kind::Str, tr_const!("Part number of the hose / tube / pipe")),
            optional("fitting_a", Kind::Str, tr_const!("Fitting at the start")),
            optional("fitting_b", Kind::Str, tr_const!("Fitting at the end")),
            optional("pressure", Kind::Num, tr_const!("Maximum operating pressure in bar")),
            optional("bend_radius", Kind::Num, tr_const!("Smallest bend radius in mm")),
            optional("medium", Kind::Str, tr_const!("compressed_air, vacuum, hydraulic_oil, water, coolant, lubricant, gas, steam or product")),
            optional("size", Kind::Str, tr_const!("Nominal size, e.g. 8 mm or DN25")),
            optional("color", Kind::Str, tr_const!("Hose colour")),
            optional("length", Kind::Num, tr_const!("Hoses / pipes: length in m")),
        ],
        run: configure_hose,
    },
    Spec {
        name: "list_fluid_nets",
        help: tr_const!("Lists the fluid netlist: each hose line with its line kind (pressure, working, return, pilot …), medium, size and the ports with their function"),
        params: &[],
        run: list_fluid_nets,
    },
    Spec {
        name: "list_fluid_groups",
        help: tr_const!("Lists the fluid device groups (valve terminals, service units, manifolds) with their devices in slot order"),
        params: &[],
        run: list_fluid_groups,
    },
    Spec {
        name: "set_fluid_group",
        help: tr_const!("Creates or changes a fluid device group"),
        params: &[
            param("tag", Kind::Str, tr_const!("Device tag of the group, e.g. -KH1")),
            optional("new_name", Kind::Str, tr_const!("New name")),
            optional("kind", Kind::Str, tr_const!("valve_terminal, service_unit, manifold, power_unit or other")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("slots", Kind::Int, tr_const!("Places of the group (0 = not limited)")),
            optional("members", Kind::Str, tr_const!("Device tags of the members in slot order, separated by commas or blanks")),
            optional("part", Kind::Str, tr_const!("Part number of the base")),
        ],
        run: set_fluid_group,
    },
    Spec {
        name: "delete_fluid_group",
        help: tr_const!("Deletes a fluid device group"),
        params: &[param("tag", Kind::Str, tr_const!("Device tag of the group, e.g. -KH1"))],
        run: delete_fluid_group,
    },
    Spec {
        name: "list_electro_fluid",
        help: tr_const!("Lists the fluid devices with solenoids or links and the electrical devices linked to them"),
        params: &[],
        run: list_electro_fluid,
    },
    Spec {
        name: "link_fluid_device",
        help: tr_const!("Links a fluid device with the electrical devices that actuate or monitor it (replaces the links)"),
        params: &[
            param("tag", Kind::Str, tr_const!("Device tag of the fluid device, e.g. -QM1")),
            param("devices", Kind::Str, tr_const!("Electrical device tags separated by commas or blanks (empty = remove the links)")),
        ],
        run: link_fluid_device,
    },
    Spec {
        name: "sync_electro_fluid",
        help: tr_const!("Links solenoid valves with the coil of the same number (-QM3 › -MB3) and copies function texts between linked devices"),
        params: &[],
        run: sync_electro_fluid,
    },
    Spec {
        name: "list_pct_loops",
        help: tr_const!("Lists the PCT loops of the pre-planning and of the P&ID instruments with their devices and engineering status"),
        params: &[],
        run: list_pct_loops,
    },
    Spec {
        name: "set_pct_loop",
        help: tr_const!("Creates or changes a PCT loop of the pre-planning"),
        params: &[
            param("name", Kind::Str, tr_const!("Loop number, e.g. 101")),
            optional("new_name", Kind::Str, tr_const!("New name")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("range", Kind::Str, tr_const!("Measuring range or set point")),
            optional("devices", Kind::Str, tr_const!("Further device tags of the loop, separated by commas or blanks")),
        ],
        run: set_pct_loop,
    },
    Spec {
        name: "delete_pct_loop",
        help: tr_const!("Deletes a PCT loop of the pre-planning"),
        params: &[param("name", Kind::Str, tr_const!("Loop number"))],
        run: delete_pct_loop,
    },
];

fn status_code(s: Status) -> &'static str {
    match s {
        Status::Planned => "planned",
        Status::InProgress => "in_engineering",
        Status::Engineered => "engineered",
    }
}

fn list_hoses(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let open = Netlist::build(prj, false);
    let f = &prj.fluid;
    let hoses: Vec<Value> = fluid::hoses(prj, &open)
        .into_iter()
        .map(|h| {
            let c = &h.config;
            json!({
                "number": h.number, "net": h.net, "from": h.from, "to": h.to, "medium": h.medium.code(), "size": h.size, "color": h.color, "length_m": h.length,
                "line": c.line.code(), "part": c.part, "fitting_a": c.fitting_a, "fitting_b": c.fitting_b, "max_pressure_bar": c.pressure, "bend_radius_mm": c.bend_radius,
                "routed_m": h.routed.as_ref().map(|r| r.length), "cut_length_m": h.cut_length(f.allowance),
            })
        })
        .collect();
    let defaults = json!({ "medium": f.medium.code(), "size": f.size, "color": f.color, "prefix": f.prefix, "pressure_bar": f.pressure, "allowance_mm": f.allowance });
    Ok(json!({ "hoses": hoses, "defaults": defaults }))
}

fn set_fluid_defaults(prj: &mut Project, a: &Args) -> Result<Value, String> {
    if let Some(m) = a.str("medium") {
        prj.fluid.medium = Medium::from_code(m).ok_or_else(|| tr_fmt!("Unknown medium: {}", m))?;
    }
    for (name, field) in [("size", &mut prj.fluid.size), ("color", &mut prj.fluid.color), ("prefix", &mut prj.fluid.prefix)] {
        if let Some(v) = a.str(name) {
            *field = v.trim().into();
        }
    }
    if let Some(p) = num_in(a, "pressure", 0.0, NO_MAX)? {
        prj.fluid.pressure = p;
    }
    if let Some(v) = a.num("allowance") {
        prj.fluid.allowance = v;
    }
    list_hoses(prj, a)
}

fn configure_hose(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("hose").trim().to_string();
    let open = Netlist::build(prj, false);
    let found = fluid::fluid_nets(prj, &open).into_iter().find(|n| n.number == name || n.net == name).ok_or_else(|| tr_fmt!("No hose {}", name))?;
    let net = open.nets[found.index].clone();
    let mut config = fluid::config_of(prj, &net);
    if let Some(l) = a.str("line") {
        config.line = LineType::from_code(l).ok_or_else(|| tr_fmt!("Unknown line type: {}", l))?;
    }
    for (field, value) in [("part", &mut config.part), ("fitting_a", &mut config.fitting_a), ("fitting_b", &mut config.fitting_b)] {
        if let Some(v) = a.str(field) {
            *value = v.trim().into();
        }
    }
    for (field, value) in [("pressure", &mut config.pressure), ("bend_radius", &mut config.bend_radius)] {
        if let Some(v) = num_in(a, field, 0.0, NO_MAX)? {
            *value = v;
        }
    }
    let medium = match a.str("medium") {
        Some(m) if m.trim().is_empty() => Some(None),
        Some(m) => Some(Some(Medium::from_code(m).ok_or_else(|| tr_fmt!("Unknown medium: {}", m))?)),
        None => None,
    };
    let hose = (!config.is_default()).then_some(config);
    for (pi, wid) in &net.wires {
        let Some(w) = prj.pages[*pi].wires.iter_mut().find(|w| w.id == *wid) else { continue };
        w.hose = hose.clone();
        if let Some(m) = medium {
            w.medium = m;
        }
        if let Some(v) = a.str("size") {
            w.cross_section = v.trim().into();
        }
        if let Some(v) = a.str("color") {
            w.color = v.trim().into();
        }
        if let Some(l) = num_in(a, "length", 0.0, NO_MAX)? {
            w.length = l;
        }
    }
    let v = list_hoses(prj, a)?;
    let hoses: Vec<Value> = v["hoses"].as_array().into_iter().flatten().filter(|h| h["net"] == net.name.as_str()).cloned().collect();
    Ok(Value::Array(hoses))
}

fn set_fluid_port_role(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let key = a.req("symbol");
    let pin = int_in(a, "pin", 0, u32::MAX.into())?.unwrap_or(0) as usize;
    let code = a.req("role");
    let role = if code == "automatic" { None } else {
        Some(fluid::PortKind::ALL.into_iter().find(|kind| kind.code() == code).ok_or_else(|| tr_fmt!("Unknown port function: {}", code))?)
    };
    let def = prj.symbol(key).ok_or_else(|| tr_fmt!("Unknown symbol: {}", key))?;
    if !def.is_fluid() { return Err(tr_fmt!("Symbol {} has no fluid ports", key)); }
    if pin >= def.pins.len() { return Err(tr_fmt!("Connection point index {} is out of range", pin)); }
    let symbol = prj.symbols.iter_mut().find(|s| s.key == key).ok_or_else(|| tr_fmt!("Unknown user symbol: {}", key))?;
    if let Some(role) = role { symbol.fluid_ports.insert(pin, role); } else { symbol.fluid_ports.remove(&pin); }
    Ok(json!({ "symbol": key, "pin": pin, "role": code }))
}

fn list_fluid_nets(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let open = Netlist::build(prj, false);
    let nets: Vec<Value> = fluid::fluid_nets(prj, &open)
        .into_iter()
        .map(|n| {
            let ports: Vec<Value> = n.ports.iter().map(|p| json!({ "port": p.designation, "kind": p.kind.code() })).collect();
            json!({ "net": n.net, "number": n.number, "line": n.line.code(), "medium": n.medium.code(), "size": n.size, "branches": n.branches(), "ports": ports })
        })
        .collect();
    Ok(Value::Array(nets))
}

fn groups_json(prj: &Project) -> Result<Value, String> {
    let groups: Vec<Value> = prj
        .fluid
        .groups
        .iter()
        .map(|g| json!({ "tag": g.tag, "kind": g.kind.code(), "description": g.description, "slots": g.slots, "members": g.members, "part": g.part }))
        .collect();
    Ok(Value::Array(groups))
}

fn list_fluid_groups(prj: &mut Project, _: &Args) -> Result<Value, String> {
    groups_json(prj)
}

fn set_fluid_group(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag = a.req("tag").trim().to_string();
    if tag.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", "tag"));
    }
    if let Some(new) = a.str("new_name").map(str::trim).filter(|n| *n != tag) {
        if new.is_empty() || prj.fluid.groups.iter().any(|g| g.tag == new) {
            return Err(tr_fmt!("Device group {} already exists", new));
        }
    }
    let i = match prj.fluid.groups.iter().position(|g| g.tag == tag) {
        Some(i) => i,
        None => {
            prj.fluid.groups.push(DeviceGroup { tag: tag.clone(), ..Default::default() });
            prj.fluid.groups.len() - 1
        }
    };
    let g = &mut prj.fluid.groups[i];
    if let Some(k) = a.str("kind") {
        g.kind = GroupKind::from_code(k).ok_or_else(|| tr_fmt!("Unknown group kind: {}", k))?;
    }
    for (field, value) in [("description", &mut g.description), ("part", &mut g.part)] {
        if let Some(v) = a.str(field) {
            *value = v.trim().into();
        }
    }
    if let Some(n) = int_in(a, "slots", 0, u32::MAX.into())? {
        g.slots = n as u32;
    }
    if let Some(m) = a.str("members") {
        g.members = crate::builtin::common::words(m).into_iter().map(String::from).collect();
    }
    if let Some(new) = a.str("new_name") {
        g.tag = new.trim().into();
    }
    groups_json(prj)
}

fn delete_fluid_group(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag = a.req("tag");
    let i = prj.fluid.groups.iter().position(|g| g.tag == tag).ok_or_else(|| tr_fmt!("No device group {}", tag))?;
    prj.fluid.groups.remove(i);
    groups_json(prj)
}

fn electro_fluid_json(prj: &Project) -> Result<Value, String> {
    let idx = Index::build(prj);
    let entries: Vec<Value> = electrofluid::entries(prj, &idx)
        .into_iter()
        .map(|e| {
            let els: Vec<Value> = e.electrical.iter().map(|(tag, function, location)| json!({ "tag": tag, "function": function, "location": location })).collect();
            json!({ "tag": e.tag, "function": e.function, "location": e.location, "solenoids": e.solenoids, "electrical": els })
        })
        .collect();
    Ok(Value::Array(entries))
}

fn list_electro_fluid(prj: &mut Project, _: &Args) -> Result<Value, String> {
    electro_fluid_json(prj)
}

fn link_fluid_device(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let tag = a.req("tag").trim().to_string();
    if tag.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", "tag"));
    }
    let devices: Vec<String> = crate::builtin::common::words(a.req("devices")).into_iter().map(String::from).collect();
    if devices.is_empty() {
        prj.fluid.links.remove(&tag);
    } else {
        prj.fluid.links.insert(tag, devices);
    }
    electro_fluid_json(prj)
}

fn sync_electro_fluid(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let sync = electrofluid::synchronize(prj);
    let linked: Vec<Value> = sync.linked.iter().map(|(f, e)| json!({ "fluid": f, "electrical": e })).collect();
    Ok(json!({ "linked": linked, "texts_copied": sync.texts, "devices": electro_fluid_json(prj)? }))
}

fn loops_json(prj: &Project) -> Result<Value, String> {
    let idx = Index::build(prj);
    let loops: Vec<Value> = process::loops(prj, &idx)
        .into_iter()
        .map(|l| {
            let variable = process::variable(l.functions.first().map(String::as_str).unwrap_or(""));
            json!({
                "name": l.name,
                "description": l.description,
                "range": l.range,
                "defined": l.defined,
                "functions": l.functions,
                "measured_variable": variable,
                "devices": l.devices,
                "missing_devices": l.missing,
                "status": status_code(l.status),
            })
        })
        .collect();
    Ok(Value::Array(loops))
}

fn list_pct_loops(prj: &mut Project, _: &Args) -> Result<Value, String> {
    loops_json(prj)
}

fn set_pct_loop(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if name.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", "name"));
    }
    if let Some(new) = a.str("new_name").map(str::trim).filter(|n| *n != name) {
        if new.is_empty() || prj.pct_loops.iter().any(|l| l.name == new) {
            return Err(tr_fmt!("PCT loop {} already exists", new));
        }
    }
    let i = match prj.pct_loops.iter().position(|l| l.name == name) {
        Some(i) => i,
        None => {
            prj.pct_loops.push(PctLoop { name: name.clone(), ..Default::default() });
            prj.pct_loops.len() - 1
        }
    };
    let l = &mut prj.pct_loops[i];
    for (field, value) in [("description", &mut l.description), ("range", &mut l.range)] {
        if let Some(v) = a.str(field) {
            *value = v.trim().into();
        }
    }
    if let Some(d) = a.str("devices") {
        l.devices = crate::builtin::common::words(d).into_iter().map(String::from).collect();
    }
    if let Some(new) = a.str("new_name") {
        l.name = new.trim().into();
    }
    loops_json(prj)
}

fn delete_pct_loop(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    let i = prj.pct_loops.iter().position(|l| l.name == name).ok_or_else(|| tr_fmt!("No PCT loop {}", name))?;
    prj.pct_loops.remove(i);
    loops_json(prj)
}

#[cfg(test)]
mod tests;
