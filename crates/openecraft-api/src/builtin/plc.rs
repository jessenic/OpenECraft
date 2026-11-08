//! PLC hardware commands: stations with racks and modules, bus systems and their port
//! connections, device descriptions, automatic configuration and AutomationML import.

use crate::builtin::common::{int_in, NO_LIMIT};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::model::Project;
use openecraft_core::plc_hw::{self, BusKind, BusSystem, DeviceDescription, PortRef, Scheme, Station, StationKind};
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Plc;

impl Plugin for Plc {
    fn info(&self) -> Info {
        info("plc", tr_const!("PLC"), tr_const!("PLC hardware: stations, cards, bus systems and port connections"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name:"get_plc_points",
        help:tr_const!("Lists every PLC channel with its module, address, symbol name and physical pins"),
        params:&[optional("representations",Kind::Bool,tr_const!("Include every graphical representation of each logical PLC channel"))],run:get_plc_points,
    },
    Spec {
        name:"set_plc_channel",
        help:tr_const!("Changes one channel of a complete PLC module symbol"),
        params:&[
            param("symbol",Kind::Int,tr_const!("Symbol ID")),
            param("channel",Kind::Int,tr_const!("Zero-based PLC channel index")),
            optional("address",Kind::Str,tr_const!("Address")),
            optional("symbolic",Kind::Str,tr_const!("Symbolic address")),
            optional("comment",Kind::Str,tr_const!("Comment")),
            optional("data_type",Kind::Str,tr_const!("Data type")),
        ],run:set_plc_channel,
    },
    Spec {
        name: "get_plc_hardware",
        help: tr_const!("Shows the PLC hardware configuration: stations with racks, slots, modules and address ranges, bus systems, port connections and the problems found"),
        params: &[],
        run: get_plc_hardware,
    },
    Spec {
        name: "configure_plc_hardware",
        help: tr_const!("Automatic hardware configuration: stations for CPUs and bus couplers, bus systems, cards to stations, slots, start addresses, point addresses, IP addresses and a line topology"),
        params: &[optional("cables", Kind::Bool, tr_const!("Also create a bus cable for every connection without cable"))],
        run: configure_plc_hardware,
    },
    Spec {
        name: "set_station",
        help: tr_const!("Creates or changes a PLC station (controller, remote I/O, drive, HMI). Changing the address scheme converts the addresses of its points"),
        params: &[
            param("name", Kind::Str, STATION),
            optional("new_name", Kind::Str, tr_const!("Renames the station")),
            optional("kind", Kind::Str, tr_const!("controller, io_device, drive, hmi or other")),
            optional("tag", Kind::Str, tr_const!("Device tag of the CPU / bus coupler, e.g. -A0")),
            optional("device", Kind::Str, tr_const!("Device description id of the head (list_device_descriptions)")),
            optional("scheme", Kind::Str, tr_const!("Address scheme: siemens, iec or rockwell")),
            optional("network", Kind::Str, tr_const!("Bus system name")),
            optional("address", Kind::Str, tr_const!("IP address or bus address")),
            optional("device_number", Kind::Int, tr_const!("Device number on the bus")),
            optional("controller", Kind::Str, tr_const!("Remote I/O: station of the controller")),
            optional("comment", Kind::Str, tr_const!("Comment")),
        ],
        run: set_station,
    },
    Spec {
        name: "remove_station",
        help: tr_const!("Removes a station; its cards stay unassigned, its bus connections are removed"),
        params: &[param("name", Kind::Str, STATION)],
        run: remove_station,
    },
    Spec {
        name: "set_plc_card",
        help: tr_const!("Sets PLC card hardware; preserves channel addresses unless address parameters or readdress are supplied"),
        params: &[
            param("card", Kind::Str, tr_const!("Card tag, e.g. -A1")),
            optional("station", Kind::Str, tr_const!("Station name (empty: unassigned)")),
            optional("rack", Kind::Int, tr_const!("Rack number")),
            optional("slot", Kind::Int, tr_const!("Slot number")),
            optional("start_byte", Kind::Int, tr_const!("First byte of the inputs (and outputs)")),
            optional("output_byte", Kind::Int, tr_const!("First byte of the outputs if it differs; -1 removes it")),
            optional("device", Kind::Str, tr_const!("Device description id (empty: from the parts)")),
            optional("readdress", Kind::Bool, tr_const!("Rebuild channel addresses from the card configuration")),
        ],
        run: set_plc_card,
    },
    Spec {
        name: "set_bus_system",
        help: tr_const!("Creates or changes a bus system (PROFINET, EtherCAT, …)"),
        params: &[
            param("name", Kind::Str, tr_const!("Bus system name, e.g. PN/IE_1")),
            optional("kind", Kind::Str, tr_const!("profinet, ethercat, ethernet_ip, profibus_dp, modbus_tcp or canopen")),
            optional("subnet", Kind::Str, tr_const!("First address of the subnet, e.g. 192.168.0.0")),
            optional("remove", Kind::Bool, tr_const!("Removes the bus system with its connections")),
        ],
        run: set_bus_system,
    },
    Spec {
        name: "connect_ports",
        help: tr_const!("Connects two ports of stations with a network cable; connections already on these ports are replaced"),
        params: &[
            param("a", Kind::Str, tr_const!("First port as station:port, e.g. PLC1:X1 P2")),
            param("b", Kind::Str, tr_const!("Second port as station:port")),
            optional("cable", Kind::Str, tr_const!("Cable tag")),
            optional("create_cable", Kind::Bool, tr_const!("Creates a bus cable for the connection (without cable tag)")),
        ],
        run: connect_ports,
    },
    Spec {
        name: "create_bus_cables",
        help: tr_const!("Creates a cable of the bus type (type, cores, cross-section) for every bus connection without cable"),
        params: &[],
        run: create_bus_cables,
    },
    Spec {
        name: "disconnect_port",
        help: tr_const!("Removes the bus connection of a port"),
        params: &[param("port", Kind::Str, tr_const!("Port as station:port"))],
        run: disconnect_port,
    },
    Spec {
        name: "list_device_descriptions",
        help: tr_const!("Lists the device descriptions of PLC hardware: imported ones, from the project parts and the built-in generic ones"),
        params: &[optional("filter", Kind::Str, tr_const!("Text in id, name or order number"))],
        run: list_device_descriptions,
    },
];

const STATION: &str = tr_const!("Station name, e.g. PLC1");

pub fn description_json(d: &DeviceDescription) -> Value {
    json!({
        "id": d.id,
        "name": d.title(),
        "manufacturer": d.manufacturer,
        "order_number": d.order_number,
        "kind": format!("{:?}", d.kind),
        "inputs": d.inputs,
        "outputs": d.outputs,
        "input_bytes": d.input_bytes,
        "output_bytes": d.output_bytes,
        "slots": d.slots,
        "ports": d.ports.iter().map(|p| json!({ "name": p.name, "bus": p.bus.key() })).collect::<Vec<_>>(),
        "source": if d.source.is_empty() { "built-in" } else { d.source.as_str() },
    })
}

pub fn hardware_json(prj: &Project) -> Value {
    let hw = &prj.plc;
    let stations: Vec<Value> = hw
        .stations
        .iter()
        .map(|st| {
            let modules: Vec<Value> = plc_hw::modules(prj, &st.name)
                .iter()
                .map(|m| {
                    json!({
                        "card": m.card,
                        "rack": m.rack,
                        "slot": m.slot,
                        "device": m.description.as_ref().map(|d| d.id.clone()),
                        "inputs": [m.inputs_used, m.description.as_ref().map(|d| d.inputs)],
                        "outputs": [m.outputs_used, m.description.as_ref().map(|d| d.outputs)],
                        "input_range": [m.input_start, m.input_bytes],
                        "output_range": [m.output_start, m.output_bytes],
                    })
                })
                .collect();
            json!({
                "name": st.name,
                "kind": st.kind.key(),
                "tag": st.tag,
                "device": plc_hw::station_description(prj, st).map(|d| d.id),
                "scheme": st.scheme.key(),
                "network": st.network,
                "address": st.address,
                "device_number": st.device_number,
                "controller": st.controller,
                "comment": st.comment,
                "ports": plc_hw::ports(prj, st).iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
                "modules": modules,
            })
        })
        .collect();
    let unassigned: Vec<String> = openecraft_core::plc::cards(prj).into_iter().filter(|c| plc_hw::station_of(prj, c).is_none()).collect();
    json!({
        "stations": stations,
        "unassigned_cards": unassigned,
        "bus_systems": hw.networks.iter().map(|n| json!({ "name": n.name, "kind": n.kind.key(), "subnet": n.subnet })).collect::<Vec<_>>(),
        "connections": hw.connections.iter().map(|c| json!({ "network": c.network, "a": c.a.to_string(), "b": c.b.to_string(), "cable": c.cable })).collect::<Vec<_>>(),
        "bus_cables": prj.cables.iter().filter(|c| c.bus.is_some() || !plc_hw::cable_links(prj, &c.tag).is_empty()).map(|c| json!({
            "cable": c.tag,
            "type": c.cable_type,
            "bus": c.bus.map(|b| b.key()),
            "cores": c.cores,
            "length_m": c.length,
            "connections": plc_hw::cable_links(prj, &c.tag).len(),
        })).collect::<Vec<_>>(),
        "problems": plc_hw::problems(prj).into_iter().map(|i| i.message()).collect::<Vec<_>>(),
    })
}

fn get_plc_hardware(prj: &mut Project, _: &Args) -> Result<Value, String> {
    Ok(hardware_json(prj))
}

fn get_plc_points(prj:&mut Project,a:&Args)->Result<Value,String> {
    let nl=openecraft_core::netlist::Netlist::build(prj,true);
    let points=if a.flag("representations") {openecraft_core::plc::representations(prj,&nl)} else {openecraft_core::plc::points(prj,&nl)};
    Ok(json!(points.iter().map(|p| {
        let channels=openecraft_core::plc::channels::definitions(prj,p.id);
        let pins=p.channel_index.and_then(|i|channels.get(i)).map(|c|c.pins.clone()).unwrap_or_default();
        json!({"symbol":p.id,"channel":p.channel_index,"card":p.card,"address":p.address,"input":p.direction,
               "kind":p.io_kind(),"designation":p.channel,"terminal":p.terminal,"pins":pins,"comment":p.function_text,
               "symbolic":p.symbolic,"data_type":p.data_type,"connected":p.connected,"page":prj.pages[p.page].name,
               "binding":p.channel_index.and_then(|index|prj.pages[p.page].symbols[p.sym].plc_bindings.get(&index)),
               "hardware_index":p.channel_index.and_then(|index|openecraft_core::model::plc_hw::bindings::index(prj,p.id,index))})
    }).collect::<Vec<_>>()))
}

fn set_plc_channel(prj:&mut Project,a:&Args)->Result<Value,String> {
    let id=int_in(a,"symbol",1,NO_LIMIT)?.unwrap_or(0) as u64;
    let index=int_in(a,"channel",0,NO_LIMIT)?.unwrap_or(0) as usize;
    if index>=openecraft_core::plc::channels::definitions(prj,id).len() {return Err(tr_fmt!("No connection point {}",format!("{id}:{index}")));}
    let mut changed=false;
    for (param,field) in [("address","address"),("symbolic","symbol"),("comment","comment"),("data_type","data_type")] {
        if let Some(value)=a.str(param) {changed|=openecraft_core::plc::channels::set(prj,id,index,field,value)?;}
    }
    Ok(json!({"changed":changed,"symbol":id,"channel":index}))
}

fn configure_plc_hardware(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let r = plc_hw::auto_configure(prj, a.flag("cables"));
    let mut v = hardware_json(prj);
    v["created"] = json!({ "stations": r.stations, "bus_systems": r.networks, "cards_assigned": r.assigned, "addresses_changed": r.addresses, "connections": r.connections, "cables": r.cables });
    Ok(v)
}

fn set_station(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if name.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", "name"));
    }
    let kind = a.str("kind").map(|k| StationKind::parse(k).ok_or_else(|| tr_fmt!("Unknown station kind: {}", k))).transpose()?;
    let scheme = a.str("scheme").map(|k| Scheme::parse(k).ok_or_else(|| tr_fmt!("Unknown address scheme: {}", k))).transpose()?;
    if let Some(d) = a.str("device").filter(|d| !d.is_empty()) {
        if plc_hw::find_description(prj, d).is_none() {
            return Err(tr_fmt!("No device description {}", d));
        }
    }
    let new_name = a.str("new_name").map(str::trim).filter(|n| !n.is_empty() && *n != name);
    if let Some(new) = new_name.filter(|n| prj.plc.station(n).is_some()) {
        return Err(tr_fmt!("Station {} exists already", new));
    }
    if prj.plc.station(&name).is_none() {
        prj.plc.stations.push(Station { name: name.clone(), ..Default::default() });
    }
    let i = prj.plc.stations.iter().position(|st| st.name == name).unwrap();
    let old_scheme = prj.plc.stations[i].scheme;
    {
        let st = &mut prj.plc.stations[i];
        st.kind = kind.unwrap_or(st.kind);
        st.scheme = scheme.unwrap_or(st.scheme);
        for (key, field) in [
            ("tag", &mut st.tag),
            ("device", &mut st.device),
            ("network", &mut st.network),
            ("address", &mut st.address),
            ("controller", &mut st.controller),
            ("comment", &mut st.comment),
        ] {
            if let Some(v) = a.str(key) {
                *field = v.to_string();
            }
        }
        if let Some(n) = int_in(a, "device_number", 0, u32::MAX.into())? {
            st.device_number = n as u32;
        }
    }
    let converted = if prj.plc.stations[i].scheme != old_scheme { plc_hw::convert_addresses(prj) } else { 0 };
    if let Some(new) = new_name {
        plc_hw::rename_station(prj, &name, new);
    }
    let mut v = hardware_json(prj);
    v["addresses_changed"] = converted.into();
    Ok(v)
}

fn remove_station(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    if prj.plc.station(name).is_none() {
        return Err(tr_fmt!("No station {}", name));
    }
    plc_hw::remove_station(prj, name);
    Ok(hardware_json(prj))
}

fn set_plc_card(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let card = a.req("card").to_string();
    if let Some(st) = a.str("station").filter(|st| !st.is_empty()) {
        if prj.plc.station(st).is_none() {
            return Err(tr_fmt!("No station {}", st));
        }
    }
    if let Some(d) = a.str("device").filter(|d| !d.is_empty()) {
        if plc_hw::find_description(prj, d).is_none() {
            return Err(tr_fmt!("No device description {}", d));
        }
    }
    let c = prj.plc_cards.entry(card.clone()).or_default();
    if let Some(v) = a.str("station") {
        c.station = v.to_string();
    }
    if let Some(v) = a.str("device") {
        c.device = v.to_string();
    }
    if let Some(v) = int_in(a, "rack", 0, NO_LIMIT)? {
        c.rack = v.to_string();
    }
    if let Some(v) = int_in(a, "slot", 0, NO_LIMIT)? {
        c.slot = v.to_string();
    }
    if let Some(v) = int_in(a, "start_byte", 0, u32::MAX.into())? {
        c.start_byte = v as u32;
    }
    if let Some(v) = a.int("output_byte") {
        c.output_byte = (v >= 0).then_some(v as u32);
    }
    let readdress=a.flag("readdress") || a.int("start_byte").is_some() || a.int("output_byte").is_some()
        || a.int("slot").is_some() && plc_hw::scheme_of(prj,&card)==Scheme::Rockwell;
    let n = if readdress {openecraft_core::plc::readdress(prj, &card)} else {0};
    let mut v = hardware_json(prj);
    v["addresses_changed"] = n.into();
    Ok(v)
}

fn set_bus_system(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if a.flag("remove") {
        if prj.plc.network(&name).is_none() {
            return Err(tr_fmt!("No bus system {}", name));
        }
        plc_hw::remove_network(prj, &name);
        return Ok(hardware_json(prj));
    }
    let kind = a.str("kind").map(|k| BusKind::parse(k).ok_or_else(|| tr_fmt!("Unknown bus system: {}", k))).transpose()?;
    if prj.plc.network(&name).is_none() {
        let kind = kind.unwrap_or_default();
        let subnet = if kind.ethernet() { "192.168.0.0".to_string() } else { String::new() };
        prj.plc.networks.push(BusSystem { name: name.clone(), kind, subnet });
    }
    let net = prj.plc.networks.iter_mut().find(|n| n.name == name).unwrap();
    if let Some(k) = kind {
        net.kind = k;
    }
    if let Some(v) = a.str("subnet") {
        net.subnet = v.to_string();
    }
    Ok(hardware_json(prj))
}

fn port(s: &str) -> Result<PortRef, String> {
    let (st, p) = s.split_once(':').ok_or_else(|| tr_fmt!("Port must be given as station:port: {}", s))?;
    Ok(PortRef::new(st.trim(), p.trim()))
}

fn connect_ports(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (pa, pb) = (port(a.req("a"))?, port(a.req("b"))?);
    plc_hw::connect(prj, pa.clone(), pb.clone(), a.str("cable").unwrap_or(""))?;
    let i = prj.plc.connections.iter().position(|c| c.a == pa && c.b == pb).unwrap();
    if a.flag("create_cable") && prj.plc.connections[i].cable.is_empty() {
        plc_hw::add_bus_cable(prj, i);
    }
    Ok(hardware_json(prj))
}

fn create_bus_cables(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let n = plc_hw::create_cables(prj);
    let mut v = hardware_json(prj);
    v["created"] = n.into();
    Ok(v)
}

fn disconnect_port(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let p = port(a.req("port"))?;
    let before = prj.plc.connections.len();
    prj.plc.connections.retain(|c| c.a != p && c.b != p);
    if prj.plc.connections.len() == before {
        return Err(tr_fmt!("Port {} is not connected", p));
    }
    Ok(hardware_json(prj))
}

fn list_device_descriptions(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let f = a.str("filter").unwrap_or("").to_lowercase();
    Ok(plc_hw::descriptions(prj)
        .iter()
        .filter(|d| f.is_empty() || [&d.id, &d.title(), &d.order_number].iter().any(|t| t.to_lowercase().contains(&f)))
        .map(description_json)
        .collect())
}

#[cfg(test)]
mod tests;
