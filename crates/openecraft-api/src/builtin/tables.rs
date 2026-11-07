//! Table commands: devices, connections, terminals, cables, plug pins, PLC points, pages and parts as rows to read and change in bulk,
//! and the CSV round trip.

use crate::params::{optional, param, Args, Kind, Param};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::model::Project;
use openecraft_core::netlist::Netlist;
use openecraft_core::search::{self, Query};
use openecraft_core::tables::{self, TableKind};
use openecraft_i18n::{tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Tables;

impl Plugin for Tables {
    fn info(&self) -> Info {
        info("tables", tr_const!("Tables"), tr_const!("Changes the device, connection and terminal tables"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_connections",
        help: tr_const!("Lists native point-to-point wiring connections by symbol ID and pin index"),
        params: &[],
        run: list_connections,
    },
    Spec {
        name: "list_nets",
        help: tr_const!("Lists native net membership by symbol ID and pin index"),
        params: &[optional("bridge_terminals", Kind::Bool, tr_const!("Include internal terminal bridges"))],
        run: list_nets,
    },
    Spec {
        name: "find",
        help: tr_const!("Finds text in the whole project: pages, device tags, function texts, free texts, wire numbers, terminals, cables, parts, descriptions"),
        params: &[
            param("text", Kind::Str, tr_const!("Text to find")),
            optional("case_sensitive", Kind::Bool, tr_const!("Match case")),
            optional("whole_word", Kind::Bool, tr_const!("Whole words only")),
            optional("kinds", Kind::Str, tr_const!("Only these kinds, separated by commas: page, device, function_text, text, wire_number, terminal, cable, part, description")),
        ],
        run: find,
    },
    Spec {
        name: "replace_text",
        help: tr_const!("Replaces text in free texts, function texts, page, device and cable descriptions of the whole project; released pages stay"),
        params: &[
            param("find", Kind::Str, tr_const!("Text to find")),
            param("replace", Kind::Str, tr_const!("Replacement")),
            optional("case_sensitive", Kind::Bool, tr_const!("Match case")),
            optional("whole_word", Kind::Bool, tr_const!("Whole words only")),
            optional("kinds", Kind::Str, tr_const!("Only these kinds, separated by commas: page, function_text, text, description")),
        ],
        run: replace_text,
    },
    Spec {
        name: "table_set",
        help: tr_const!("Changes a cell of a table: device tag, function text, description, part; wire number, colour, cross-section, cable, core; strip, terminal"),
        params: &[KIND, param("key", Kind::Str, tr_const!("Row key (from table_get)")), param("column", Kind::Str, tr_const!("Column name, e.g. Description")), param("value", Kind::Str, tr_const!("New value"))],
        run: table_set,
    },
    Spec {
        name: "table_replace",
        help: tr_const!("Finds and replaces text in a column of all (filtered) rows; without find the column is filled with the replacement"),
        params: &[
            KIND,
            param("column", Kind::Str, tr_const!("Column name, e.g. Description")),
            optional("find", Kind::Str, tr_const!("Text to find; empty: fill")),
            param("replace", Kind::Str, tr_const!("Replacement")),
            optional("filter", Kind::Str, tr_const!("Only rows containing this text")),
        ],
        run: table_replace,
    },
];

pub const KIND: Param = param("table", Kind::Str, tr_const!("devices, connections, terminals, cables, plugs, plc, pages or parts"));

fn list_connections(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let potentials = Netlist::build(prj, true);
    let open = Netlist::build(prj, false);
    let endpoint = |pin: openecraft_core::netlist::PinRef| {
        json!({"symbol":prj.pages[pin.page].symbols[pin.sym].id,"pin":pin.pin})
    };
    Ok(Value::Array(openecraft_core::netlist::connections(prj, &potentials, &open).iter().map(|c| {
        json!({"from":endpoint(c.a),"to":endpoint(c.b),"potential":c.net,"number":c.number,
               "color":c.color,"cross_section":c.cross_section,"cable":c.cable,"core":c.core})
    }).collect()))
}

fn list_nets(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let netlist = Netlist::build(prj, a.value("bridge_terminals").and_then(Value::as_bool).unwrap_or(true));
    let mut pins: Vec<_> = netlist.pin_net.iter().map(|(&(symbol, pin), &net)| (symbol, pin, net)).collect();
    pins.sort_unstable();
    let mut points: Vec<_> = netlist.connection_point_net.iter().map(|(&(symbol, pin), &net)| (symbol, pin, net)).collect();
    points.sort_unstable();
    Ok(json!({
        "pins": pins.into_iter().map(|(symbol,pin,net)| json!({"symbol":symbol,"pin":pin,"net":net})).collect::<Vec<_>>(),
        "connection_points": points.into_iter().map(|(symbol,pin,net)| json!({"symbol":symbol,"pin":pin,"net":net})).collect::<Vec<_>>()
    }))
}

pub fn kind(a: &Args) -> Result<TableKind, String> {
    TableKind::from_key(a.req("table")).ok_or_else(|| tr_fmt!("Unknown table: {}", a.req("table")))
}

fn column(kind: TableKind, name: &str) -> Result<usize, String> {
    kind.column(name).ok_or_else(|| tr_fmt!("Unknown column: {}", name))
}

fn table_set(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let kind = kind(a)?;
    let col = column(kind, a.req("column"))?;
    let open = Netlist::build(prj, false);
    let changed = tables::set_cell(prj, &open, kind, a.req("key"), col, a.req("value"))?;
    Ok(json!({ "changed": changed }))
}

fn table_replace(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let kind = kind(a)?;
    let col = column(kind, a.req("column"))?;
    let (nl, open) = (Netlist::build(prj, true), Netlist::build(prj, false));
    let rows: Vec<tables::Row> = tables::rows(prj, &nl, &open, kind).into_iter().filter(|r| tables::matches(r, a.str("filter").unwrap_or(""))).collect();
    let (n, problems) = tables::replace(prj, &open, kind, &rows, col, a.str("find").unwrap_or(""), a.req("replace"));
    Ok(json!({ "changed": n, "problems": problems }))
}

fn query(a: &Args) -> Result<Query, String> {
    let mut kinds = Vec::new();
    for code in crate::builtin::common::words(a.str("kinds").unwrap_or_default()) {
        kinds.push(search::Kind::from_code(code).ok_or_else(|| tr_fmt!("Unknown kind: {}", code))?);
    }
    // `find` searches `text`, `replace_text` replaces `find`.
    let text = a.str("find").or(a.str("text")).unwrap_or_default();
    if text.is_empty() {
        return Err(tr_fmt!("Missing parameter: {}", if a.value("find").is_some() { "find" } else { "text" }));
    }
    Ok(Query { text: text.to_string(), case_sensitive: a.flag("case_sensitive"), whole_word: a.flag("whole_word"), kinds })
}

fn find(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let hits = search::find(prj, &query(a)?);
    Ok(Value::Array(
        hits.into_iter()
            .map(|h| json!({ "kind": h.kind.code(), "page": h.page.map(|p| prj.pages[p].name.clone()), "id": Some(h.item).filter(|i| *i != 0), "text": h.text }))
            .collect(),
    ))
}

fn replace_text(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let q = query(a)?;
    let n = search::replace(prj, &q, a.req("replace"));
    Ok(json!({ "changed": n }))
}

