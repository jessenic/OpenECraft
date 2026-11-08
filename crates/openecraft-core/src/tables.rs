//! Editing in tables: devices, connections, terminals, cables, plug pins, PLC points,
//! pages and the project parts as rows with editable columns, for changing many at once (filter, fill, find and replace) and for the
//! round trip through a spreadsheet (CSV with the row key in the first column).

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::index::Index;
use crate::model::*;
use crate::netlist::{pin_designation, Net, Netlist};
use crate::parts::PartRef;
use crate::symbols::Func;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TableKind {
    Devices,
    Connections,
    Terminals,
    Cables,
    Plugs,
    Plc,
    Pages,
    Parts,
}

/// A column: English name (CSV header, translated where shown) and whether it can be changed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Column {
    pub name: &'static str,
    pub editable: bool,
}

const fn col(name: &'static str, editable: bool) -> Column {
    Column { name, editable }
}

const DEVICES: [Column; 5] =
    [col(tr_const!("Device tag"), true), col(tr_const!("Function text"), true), col(tr_const!("Description"), true), col(tr_const!("Part"), true), col(tr_const!("Pages"), false)];
const CONNECTIONS: [Column; 10] = [
    col(tr_const!("From"), false),
    col(tr_const!("To"), false),
    col(tr_const!("Potential"), false),
    col(tr_const!("Wire number"), true),
    col(tr_const!("Colour"), true),
    col(tr_const!("Cross-section"), true),
    col(tr_const!("Cable"), true),
    col(tr_const!("Core"), true),
    col(tr_const!("Pages"), false),
    col(tr_const!("Wire type"), true),
];
const TERMINALS: [Column; 4] = [col(tr_const!("Strip"), true), col(tr_const!("Terminal"), true), col(tr_const!("Function text"), true), col(tr_const!("Pages"), false)];
const CABLES: [Column; 8] = [
    col(tr_const!("Cable"), true),
    col(tr_const!("Type"), true),
    col(tr_const!("Cores"), true),
    col(tr_const!("Cross-section"), true),
    col(tr_const!("Length"), true),
    col(tr_const!("Part"), true),
    col(tr_const!("Description"), true),
    col(tr_const!("Wires"), false),
];
const PLUGS: [Column; 6] = [
    col(tr_const!("Plug"), false),
    col(tr_const!("Pin"), true),
    col(tr_const!("Function text"), true),
    col(tr_const!("Socket side"), false),
    col(tr_const!("Pin side"), false),
    col(tr_const!("Potential"), false),
];
const PLC: [Column; 6] = [
    col(tr_const!("PLC card"), false),
    col(tr_const!("Address"), true),
    col(tr_const!("Direction"), false),
    col(tr_const!("Function text"), true),
    col(tr_const!("Connected"), false),
    col(tr_const!("Page"), false),
];
const PAGES: [Column; 4] = [col(tr_const!("Page"), true), col(tr_const!("Description"), true), col(tr_const!("Page type"), false), col(tr_const!("Released"), false)];
const PARTS: [Column; 7] = [
    col(tr_const!("Part number"), false),
    col(tr_const!("Description"), true),
    col(tr_const!("Manufacturer"), true),
    col(tr_const!("Supplier"), true),
    col(tr_const!("Order number"), true),
    col(tr_const!("Price"), true),
    col(tr_const!("Devices"), false),
];

impl TableKind {
    pub const ALL: [TableKind; 8] =
        [TableKind::Devices, TableKind::Connections, TableKind::Terminals, TableKind::Cables, TableKind::Plugs, TableKind::Plc, TableKind::Pages, TableKind::Parts];

    pub fn title(self) -> &'static str {
        match self {
            TableKind::Devices => tr("Devices"),
            TableKind::Connections => tr("Connections"),
            TableKind::Terminals => tr("Terminals"),
            TableKind::Cables => tr("Cables"),
            TableKind::Plugs => tr("Plugs"),
            TableKind::Plc => tr("PLC"),
            TableKind::Pages => tr("Pages"),
            TableKind::Parts => tr("Parts"),
        }
    }

    /// Name on the command line.
    pub fn key(self) -> &'static str {
        match self {
            TableKind::Devices => "devices",
            TableKind::Connections => "connections",
            TableKind::Terminals => "terminals",
            TableKind::Cables => "cables",
            TableKind::Plugs => "plugs",
            TableKind::Plc => "plc",
            TableKind::Pages => "pages",
            TableKind::Parts => "parts",
        }
    }

    pub fn from_key(s: &str) -> Option<TableKind> {
        TableKind::ALL.into_iter().find(|k| k.key() == s.trim().to_ascii_lowercase())
    }

    pub fn columns(self) -> &'static [Column] {
        match self {
            TableKind::Devices => &DEVICES,
            TableKind::Connections => &CONNECTIONS,
            TableKind::Terminals => &TERMINALS,
            TableKind::Cables => &CABLES,
            TableKind::Plugs => &PLUGS,
            TableKind::Plc => &PLC,
            TableKind::Pages => &PAGES,
            TableKind::Parts => &PARTS,
        }
    }

    /// Index of a column by its English name (case-insensitive).
    pub fn column(self, name: &str) -> Option<usize> {
        self.columns().iter().position(|c| c.name.eq_ignore_ascii_case(name.trim()))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Identifies the row: device tag, first wire id of a connection, terminal symbol id.
    pub key: String,
    pub cells: Vec<String>,
    /// Page and item to jump to.
    pub target: Option<(usize, u64)>,
    /// Lies (partly) on a released page: cannot be changed.
    pub locked: bool,
}

fn pages_text(prj: &Project, pages: impl IntoIterator<Item = usize>) -> String {
    let mut v: Vec<usize> = pages.into_iter().collect();
    v.sort();
    v.dedup();
    v.iter().map(|&p| prj.pages[p].name.clone()).collect::<Vec<_>>().join(", ")
}

/// Symbol whose function text stands for the device: the main function, else the first.
fn device_symbol(idx: &Index, tag: &str) -> Option<(usize, u64)> {
    let dev = idx.devices.get(tag)?;
    dev.mains.first().or(dev.others.first()).or(dev.auxs.first()).or(dev.overviews.first()).map(|l| (l.page, l.sym_id))
}

fn symbol(prj: &Project, at: (usize, u64)) -> Option<&SymbolInst> {
    prj.pages.get(at.0)?.symbols.iter().find(|s| s.id == at.1)
}

/// Connections that are in a table: nets of `open` with wires.
fn connection_nets(open: &Netlist) -> impl Iterator<Item = &Net> {
    open.nets.iter().filter(|n| !n.wires.is_empty())
}

fn net_key(net: &Net) -> String {
    net.wires.iter().map(|w| w.1).min().unwrap_or(0).to_string()
}

/// The rows of a table. `nl`: potentials, `open`: connections (terminal sides apart).
pub fn rows(prj: &Project, nl: &Netlist, open: &Netlist, kind: TableKind) -> Vec<Row> {
    let mut out = Vec::new();
    match kind {
        TableKind::Devices => {
            let idx = Index::build(prj);
            for tag in idx.sorted_tags() {
                let dev = &idx.devices[tag];
                let locs: Vec<_> = dev.mains.iter().chain(&dev.auxs).chain(&dev.others).chain(&dev.overviews).collect();
                let target = device_symbol(&idx, tag);
                let data = prj.devices.get(tag);
                out.push(Row {
                    key: tag.clone(),
                    cells: vec![
                        tag.clone(),
                        target.and_then(|t| symbol(prj, t)).map(|s| s.function_text.clone()).unwrap_or_default(),
                        data.map(|d| d.description.clone()).unwrap_or_default(),
                        data.and_then(|d| d.parts.first()).map(|r| r.number.clone()).unwrap_or_default(),
                        pages_text(prj, locs.iter().map(|l| l.page)),
                    ],
                    target,
                    locked: locs.iter().any(|l| prj.pages[l.page].locked),
                });
            }
        }
        TableKind::Connections => {
            for net in connection_nets(open) {
                let wires = crate::wirenum::wires_of(prj, net);
                let mut ends: Vec<String> = net
                    .pins
                    .iter()
                    .filter(|p| prj.symbol(&prj.pages[p.page].symbols[p.sym].sym).is_some_and(|d| d.func.has_tag()))
                    .map(|p| pin_designation(prj, *p))
                    .collect();
                ends.sort_by(|a, b| natural_cmp(a, b));
                ends.dedup();
                let first = |f: fn(&Wire) -> &String| wires.iter().map(|w| f(w)).find(|v| !v.is_empty()).cloned().unwrap_or_default();
                let potential = net.wires.first().and_then(|w| nl.net_of_wire(w.1)).filter(|n| n.named).map(|n| n.name.clone()).unwrap_or_default();
                let page = net.wires.iter().map(|w| w.0).min().unwrap_or(0);
                out.push(Row {
                    key: net_key(net),
                    cells: vec![
                        ends.first().cloned().unwrap_or_default(),
                        ends.get(1..).map(|r| r.join(", ")).unwrap_or_default(),
                        potential,
                        crate::wirenum::number_of(prj, net),
                        first(|w| &w.color),
                        first(|w| &w.cross_section),
                        first(|w| &w.cable),
                        first(|w| &w.core),
                        pages_text(prj, net.wires.iter().map(|w| w.0)),
                        first(|w| &w.wire_type),
                    ],
                    target: net.wires.iter().map(|w| w.1).min().map(|id| (page, id)),
                    locked: net.wires.iter().any(|w| prj.pages[w.0].locked),
                });
            }
            out.sort_by(|a, b| natural_cmp(&a.cells[0], &b.cells[0]).then_with(|| natural_cmp(&a.cells[1], &b.cells[1])));
        }
        TableKind::Terminals => {
            for strip in crate::strips::strip_tags(prj) {
                for t in crate::strips::terminals(prj, &strip) {
                    let s = &prj.pages[t.page].symbols[t.sym];
                    out.push(Row {
                        key: t.id.to_string(),
                        cells: vec![strip.clone(), t.label.clone(), s.function_text.clone(), prj.pages[t.page].name.clone()],
                        target: Some((t.page, t.id)),
                        locked: prj.pages[t.page].locked,
                    });
                }
            }
        }
        TableKind::Cables => {
            for c in &prj.cables {
                let wires: Vec<(usize, u64)> =
                    prj.pages.iter().enumerate().flat_map(|(pi, pg)| pg.wires.iter().filter(|w| w.cable == c.tag).map(move |w| (pi, w.id))).collect();
                out.push(Row {
                    key: c.id.to_string(),
                    cells: vec![
                        c.tag.clone(),
                        c.cable_type.clone(),
                        c.cores.clone(),
                        c.cross_section.clone(),
                        if c.length > 0.0 { format!("{}", c.length) } else { String::new() },
                        c.part.clone(),
                        c.description.clone(),
                        wires.len().to_string(),
                    ],
                    target: wires.first().copied(),
                    locked: false,
                });
            }
        }
        TableKind::Plugs => {
            for plug in crate::plugs::plug_tags(prj) {
                for r in crate::plugs::rows(prj, &plug, nl, open) {
                    let Some(first) = r.pins.first() else { continue };
                    out.push(Row {
                        key: first.id.to_string(),
                        cells: vec![plug.clone(), r.label.clone(), r.function_text.clone(), r.socket.clone(), r.pin.clone(), r.potential.clone()],
                        target: Some((first.page, first.id)),
                        locked: r.pins.iter().any(|p| prj.pages[p.page].locked),
                    });
                }
            }
        }
        TableKind::Plc => {
            for p in crate::plc::points(prj, nl) {
                let dir = if p.input { tr("Input") } else { tr("Output") };
                out.push(Row {
                    key: p.id.to_string(),
                    cells: vec![p.card.clone(), p.address.clone(), dir.to_string(), p.function_text.clone(), p.connected.clone(), prj.pages[p.page].name.clone()],
                    target: Some((p.page, p.id)),
                    locked: prj.pages[p.page].locked,
                });
            }
        }
        TableKind::Pages => {
            for (i, pg) in prj.pages.iter().enumerate() {
                out.push(Row {
                    key: pg.id.to_string(),
                    cells: vec![pg.name.clone(), pg.description.clone(), pg.kind.label(), if pg.locked { tr("yes").to_string() } else { String::new() }],
                    target: Some((i, 0)),
                    locked: pg.locked,
                });
            }
        }
        TableKind::Parts => {
            for part in &prj.parts {
                let used = prj.devices.values().filter(|d| d.parts.iter().any(|r| r.number == part.number)).count();
                out.push(Row {
                    key: part.number.clone(),
                    cells: vec![
                        part.number.clone(),
                        part.description.clone(),
                        part.manufacturer.clone(),
                        part.supplier.clone(),
                        part.order_number.clone(),
                        if part.price > 0.0 { format!("{}", part.price) } else { String::new() },
                        used.to_string(),
                    ],
                    target: None,
                    locked: false,
                });
            }
        }
    }
    out
}

/// A number typed into a cell (`1,5` or `1.5`; empty is 0).
fn number(value: &str) -> Result<f32, String> {
    if value.is_empty() {
        return Ok(0.0);
    }
    value.replace(',', ".").parse::<f32>().map_err(|_| tr_fmt!("Not a number: {}", value))
}

/// Sets a text field; whether it changed.
fn set_text(field: &mut String, value: &str) -> bool {
    if field == value {
        return false;
    }
    *field = value.to_string();
    true
}

/// A placed symbol by its id: page and index.
fn symbol_at(prj: &Project, id: u64) -> Option<(usize, usize)> {
    prj.pages.iter().enumerate().find_map(|(pi, pg)| pg.symbols.iter().position(|s| s.id == id).map(|si| (pi, si)))
}

fn locked_error() -> String {
    tr("The row lies on a released page").to_string()
}

/// Changes one cell. Returns whether something changed.
#[allow(clippy::too_many_lines)]
fn set(prj: &mut Project, open: &Netlist, kind: TableKind, key: &str, column: usize, value: &str) -> Result<bool, String> {
    let Some(c) = kind.columns().get(column) else { return Err(tr_fmt!("Unknown column: {}", column)) };
    if !c.editable {
        return Err(tr_fmt!("Column {} cannot be changed", tr(c.name)));
    }
    let value = value.trim();
    match kind {
        TableKind::Devices => {
            let idx = Index::build(prj);
            let Some(dev) = idx.devices.get(key) else { return Err(tr_fmt!("No device {}", key)) };
            if dev.mains.iter().chain(&dev.auxs).chain(&dev.others).chain(&dev.overviews).any(|l| prj.pages[l.page].locked) {
                return Err(locked_error());
            }
            match column {
                0 => {
                    if value == key {
                        return Ok(false);
                    }
                    if value.is_empty() || idx.devices.contains_key(value) {
                        return Err(tr_fmt!("Device {} already exists", value));
                    }
                    crate::ops::rename_device(prj, key, value);
                }
                1 => {
                    let Some((page, id)) = device_symbol(&idx, key) else { return Ok(false) };
                    let s = prj.pages[page].symbols.iter_mut().find(|s| s.id == id).unwrap();
                    let text = value.replace("\\n", "\n");
                    if s.function_text == text {
                        return Ok(false);
                    }
                    s.function_text = text;
                }
                2 => {
                    let d = prj.devices.entry(key.to_string()).or_default();
                    if d.description == value {
                        return Ok(false);
                    }
                    d.description = value.to_string();
                }
                _ => {
                    let d = prj.devices.entry(key.to_string()).or_default();
                    match (d.parts.first().map(|r| r.number.as_str()), value.is_empty()) {
                        (Some(n), _) if n == value => return Ok(false),
                        (None, true) => return Ok(false),
                        (Some(_), true) => {
                            d.parts.remove(0);
                        }
                        (Some(_), false) => d.parts[0].number = value.to_string(),
                        (None, false) => d.parts.push(PartRef { number: value.to_string(), count: 1 }),
                    }
                }
            }
        }
        TableKind::Connections => {
            let id: u64 = key.parse().map_err(|_| tr_fmt!("No connection {}", key))?;
            let Some(&ni) = open.wire_net.get(&id) else { return Err(tr_fmt!("No connection {}", key)) };
            let net = &open.nets[ni];
            if net.wires.iter().any(|w| prj.pages[w.0].locked) {
                return Err(locked_error());
            }
            if column == 3 {
                if crate::wirenum::number_of(prj, net) == value {
                    return Ok(false);
                }
                crate::wirenum::set(prj, net, value, true);
                return Ok(true);
            }
            if column == 6 && !value.is_empty() && prj.cable(value).is_none() {
                return Err(tr_fmt!("Unknown cable {}", value));
            }
            let mut changed = false;
            for (pi, wid) in &net.wires {
                let Some(w) = prj.pages[*pi].wires.iter_mut().find(|w| w.id == *wid) else { continue };
                let field = match column {
                    4 => &mut w.color,
                    5 => &mut w.cross_section,
                    6 => &mut w.cable,
                    9 => &mut w.wire_type,
                    _ => &mut w.core,
                };
                if field != value {
                    *field = value.to_string();
                    changed = true;
                }
                if column == 6 && value.is_empty() {
                    w.core.clear();
                }
            }
            return Ok(changed);
        }
        TableKind::Terminals => {
            let id: u64 = key.parse().map_err(|_| tr_fmt!("No terminal {}", key))?;
            let symbols = prj.symbol_table();
            let Some(s) = prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()).find(|s| s.id == id) else {
                return Err(tr_fmt!("No terminal {}", key));
            };
            if symbols.get(&s.sym).map(|d| d.func) != Some(Func::Terminal) {
                return Err(tr_fmt!("No terminal {}", key));
            }
            let field = match column {
                0 => &mut s.tag,
                1 => &mut s.label,
                _ => &mut s.function_text,
            };
            let text = if column == 2 { value.replace("\\n", "\n") } else { value.to_string() };
            if *field == text {
                return Ok(false);
            }
            *field = text;
        }
        TableKind::Cables => {
            let id: u64 = key.parse().map_err(|_| tr_fmt!("No cable {}", key))?;
            let Some(i) = prj.cables.iter().position(|c| c.id == id) else { return Err(tr_fmt!("No cable {}", key)) };
            if column == 0 {
                let old = prj.cables[i].tag.clone();
                if value == old {
                    return Ok(false);
                }
                if value.is_empty() || prj.cable(value).is_some() {
                    return Err(tr_fmt!("Cable {} already exists", value));
                }
                prj.cables[i].tag = value.to_string();
                crate::ops::rename_cable(prj, &old, value);
                return Ok(true);
            }
            let c = &mut prj.cables[i];
            return Ok(match column {
                1 => set_text(&mut c.cable_type, value),
                2 => set_text(&mut c.cores, value),
                3 => set_text(&mut c.cross_section, value),
                4 => {
                    let v = number(value)?;
                    let changed = (c.length - v).abs() > f32::EPSILON;
                    c.length = v;
                    changed
                }
                5 => set_text(&mut c.part, value),
                _ => set_text(&mut c.description, value),
            });
        }
        TableKind::Plugs | TableKind::Plc => {
            let id: u64 = key.parse().map_err(|_| tr_fmt!("No connection point {}", key))?;
            let Some((pi, si)) = symbol_at(prj, id) else { return Err(tr_fmt!("No connection point {}", key)) };
            if prj.pages[pi].locked {
                return Err(locked_error());
            }
            let s = &mut prj.pages[pi].symbols[si];
            return Ok(if column == 1 { set_text(&mut s.label, value) } else { set_text(&mut s.function_text, &value.replace("\\n", "\n")) });
        }
        TableKind::Pages => {
            let id: u64 = key.parse().map_err(|_| tr_fmt!("No page {}", key))?;
            let Some(pg) = prj.pages.iter_mut().find(|p| p.id == id) else { return Err(tr_fmt!("No page {}", key)) };
            if pg.locked {
                return Err(locked_error());
            }
            return Ok(if column == 0 { set_text(&mut pg.name, value) } else { set_text(&mut pg.description, value) });
        }
        TableKind::Parts => {
            let Some(part) = prj.parts.iter_mut().find(|p| p.number == key) else { return Err(tr_fmt!("No part {}", key)) };
            return Ok(match column {
                1 => set_text(&mut part.description, value),
                2 => set_text(&mut part.manufacturer, value),
                3 => set_text(&mut part.supplier, value),
                4 => set_text(&mut part.order_number, value),
                _ => {
                    let v = number(value)?;
                    let changed = (part.price - v).abs() > f32::EPSILON;
                    part.price = v;
                    changed
                }
            });
        }
    }
    Ok(true)
}

/// The locked check of terminals needs the page; done here so `set` stays one match.
fn terminal_locked(prj: &Project, key: &str) -> bool {
    let Ok(id) = key.parse::<u64>() else { return false };
    prj.pages.iter().any(|pg| pg.locked && pg.symbols.iter().any(|s| s.id == id))
}

/// Changes one cell (refusing rows on released pages). Returns whether something
/// changed. `open` must be the connections of the project before the change.
pub fn set_cell(prj: &mut Project, open: &Netlist, kind: TableKind, key: &str, column: usize, value: &str) -> Result<bool, String> {
    if kind == TableKind::Terminals && terminal_locked(prj, key) {
        return Err(locked_error());
    }
    set(prj, open, kind, key, column, value)
}

/// The table as CSV: `Key` and the English column names, `;` separated.
pub fn to_csv(kind: TableKind, rows: &[Row]) -> String {
    let mut out = String::from("Key");
    for c in kind.columns() {
        out.push(';');
        out.push_str(&crate::parts::csv_escape(c.name));
    }
    out.push('\n');
    for r in rows {
        out.push_str(&r.key);
        for cell in &r.cells {
            out.push(';');
            out.push_str(&crate::parts::csv_escape(&cell.replace('\n', "\\n")));
        }
        out.push('\n');
    }
    out
}

/// Applies a CSV written by [`to_csv`] (and changed in a spreadsheet): every editable
/// cell that differs is set. Rows are found by their key. Returns the number of
/// changed cells and the problems of single rows.
pub fn apply_csv(prj: &mut Project, nl: &Netlist, open: &Netlist, kind: TableKind, text: &str) -> Result<(usize, Vec<String>), String> {
    let records = crate::parts::parse_csv(text.trim_start_matches('\u{feff}'));
    let Some((header, data)) = records.split_first() else { return Err(tr("The file is empty").to_string()) };
    if !header.first().is_some_and(|h| h.trim().eq_ignore_ascii_case("Key")) {
        return Err(tr("The first column must be Key").to_string());
    }
    let columns: Vec<Option<usize>> = header.iter().skip(1).map(|h| kind.column(h)).collect();
    let current = rows(prj, nl, open, kind);
    let (mut changed, mut problems) = (0, Vec::new());
    // Device renames last, so that the other cells still find the device by its key.
    let mut renames = Vec::new();
    for r in data.iter().filter(|r| r.first().is_some_and(|k| !k.trim().is_empty())) {
        let key = r[0].trim();
        let Some(row) = current.iter().find(|x| x.key == key) else {
            problems.push(tr_fmt!("Row {} not found", key));
            continue;
        };
        for (i, c) in columns.iter().enumerate() {
            let (Some(c), Some(value)) = (c, r.get(i + 1)) else { continue };
            let value = value.trim();
            if !kind.columns()[*c].editable || row.cells[*c] == value.replace("\\n", "\n") {
                continue;
            }
            if kind == TableKind::Devices && *c == 0 {
                renames.push((key.to_string(), value.to_string()));
                continue;
            }
            match set_cell(prj, open, kind, key, *c, value) {
                Ok(true) => changed += 1,
                Ok(false) => {}
                Err(e) => problems.push(format!("{key}: {e}")),
            }
        }
    }
    for (key, value) in renames {
        match set_cell(prj, open, kind, &key, 0, &value) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => problems.push(format!("{key}: {e}")),
        }
    }
    Ok((changed, problems))
}

/// Rows whose cells contain `filter` (case-insensitive; empty: all rows).
pub fn matches(row: &Row, filter: &str) -> bool {
    let f = filter.trim().to_lowercase();
    f.is_empty() || row.key.to_lowercase() == f || row.cells.iter().any(|c| c.to_lowercase().contains(&f))
}

/// Replaces `find` by `replace` in column `column` of the given rows. Returns the
/// number of changed cells and the problems.
/// An empty `find` sets the whole cell (fill).
pub fn replace(prj: &mut Project, open: &Netlist, kind: TableKind, rows: &[Row], column: usize, find: &str, replace: &str) -> (usize, Vec<String>) {
    let (mut changed, mut problems) = (0, Vec::new());
    for r in rows {
        let Some(cell) = r.cells.get(column) else { continue };
        let new = if find.is_empty() { replace.to_string() } else if cell.contains(find) { cell.replace(find, replace) } else { continue };
        match set_cell(prj, open, kind, &r.key, column, &new) {
            Ok(true) => changed += 1,
            Ok(false) => {}
            Err(e) => problems.push(format!("{}: {e}", r.cells.first().cloned().unwrap_or_default())),
        }
    }
    (changed, problems)
}

#[cfg(test)]
mod tests;
