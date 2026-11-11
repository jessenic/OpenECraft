//! File formats of the connectors. All of them are built from the same rows of
//! neutral fields, so CSV columns and JSON keys are the field codes.

use super::{bom, fmt_number, Field};
use crate::model::{PageKind, Part, Project};
use crate::netlist::pin_designation;
use crate::analysis::Analysis;
use crate::wiring::{self, Order, Status};
use openecraft_parts::csv_escape;
use serde_json::{json, Map, Value};

type Row = Vec<(Field, String)>;

const NUMERIC: &[Field] = &[
    Field::Quantity,
    Field::Price,
    Field::Stock,
    Field::LeadTime,
    Field::ToOrder,
    Field::Total,
    Field::Ordered,
    Field::X,
    Field::Y,
    Field::Z,
    Field::Width,
    Field::Height,
    Field::Depth,
    Field::Length,
    Field::StripFrom,
    Field::StripTo,
    Field::Job,
];

fn csv(rows: &[Row]) -> String {
    let Some(first) = rows.first() else { return String::new() };
    let mut out = first.iter().map(|(f, _)| f.code()).collect::<Vec<_>>().join(";");
    out.push('\n');
    for r in rows {
        out.push_str(&r.iter().map(|(_, v)| csv_escape(v)).collect::<Vec<_>>().join(";"));
        out.push('\n');
    }
    out
}

fn object(row: &Row) -> Value {
    let mut m = Map::new();
    for (f, v) in row {
        let value = match v.parse::<f64>() {
            Ok(n) if NUMERIC.contains(f) => json!(n),
            _ => Value::String(v.clone()),
        };
        m.insert(f.code().to_string(), value);
    }
    Value::Object(m)
}

fn objects(rows: &[Row]) -> Value {
    Value::Array(rows.iter().map(object).collect())
}

fn part<'a>(prj: &'a Project, number: &str) -> Option<&'a Part> {
    prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == number)
}

fn page_kind(k: PageKind) -> &'static str {
    match k {
        PageKind::Schematic => "schematic",
        PageKind::Cover => "cover",
        PageKind::Report(_) => "report",
        PageKind::Mounting(_) => "mounting",
        PageKind::Overview => "overview",
        PageKind::Graphic => "graphic",
        PageKind::Document => "document",
    }
}

fn enclosure_of(prj: &Project, tag: &str) -> String {
    crate::panel::locate(prj, tag).and_then(|(e, _)| prj.layout.enclosures.get(e)).map(|e| e.name.clone()).unwrap_or_default()
}

fn items(prj: &Project, an: &Analysis) -> Vec<Row> {
    bom(prj, an)
        .into_iter()
        .map(|l| {
            let p = part(prj, &l.part).cloned().unwrap_or_default();
            let link = prj.integration.items.get(&l.part).cloned().unwrap_or_default();
            vec![
                (Field::PartNumber, l.part.clone()),
                (Field::ErpNumber, link.erp_number),
                (Field::ItemId, link.item_id),
                (Field::Revision, link.revision),
                (Field::Lifecycle, link.lifecycle),
                (Field::Description, p.description),
                (Field::Manufacturer, p.manufacturer),
                (Field::TypeNumber, p.type_number),
                (Field::OrderNumber, p.order_number),
                (Field::Supplier, p.supplier),
                (Field::Quantity, fmt_number(l.quantity)),
                (Field::Unit, l.unit),
                (Field::Price, fmt_number(p.price / p.price_unit.max(1) as f32)),
                (Field::Currency, p.currency),
                (Field::Stock, link.stock.map(fmt_number).unwrap_or_default()),
                (Field::LeadTime, link.lead_time.map(|d| d.to_string()).unwrap_or_default()),
                (Field::Devices, l.devices.join(", ")),
            ]
        })
        .collect()
}

fn devices(prj: &Project) -> Vec<Row> {
    prj.devices
        .iter()
        .map(|(tag, d)| {
            let number = d.parts.first().map(|r| r.number.clone()).unwrap_or_default();
            let p = part(prj, &number).cloned().unwrap_or_default();
            vec![
                (Field::Tag, tag.clone()),
                (Field::PartNumber, number),
                (Field::Description, d.description.clone()),
                (Field::Manufacturer, p.manufacturer),
                (Field::TypeNumber, p.type_number),
                (Field::Enclosure, enclosure_of(prj, tag)),
            ]
        })
        .collect()
}

fn placements(prj: &Project) -> Vec<Row> {
    let mut out = Vec::new();
    for e in &prj.layout.enclosures {
        for it in e.items.iter().filter(|i| i.kind == crate::panel::ItemKind::Device && !i.tag.is_empty()) {
            out.push(vec![
                (Field::Enclosure, e.name.clone()),
                (Field::Tag, it.tag.clone()),
                (Field::PartNumber, it.part.clone()),
                (Field::Face, it.face.key().to_string()),
                (Field::X, fmt_number(it.x)),
                (Field::Y, fmt_number(it.y)),
                (Field::Z, fmt_number(it.z)),
                (Field::Width, fmt_number(it.w)),
                (Field::Height, fmt_number(it.h)),
                (Field::Depth, fmt_number(it.d)),
            ]);
        }
    }
    out
}

fn connections(prj: &Project, an: &Analysis) -> Vec<Row> {
    an.connections
        .iter()
        .map(|c| {
            vec![
                (Field::Net, c.net.clone()),
                (Field::WireNumber, c.number.clone()),
                (Field::From, pin_designation(prj, c.a)),
                (Field::To, pin_designation(prj, c.b)),
                (Field::Color, c.color.clone()),
                (Field::CrossSection, c.cross_section.clone()),
                (Field::Cable, c.cable.clone()),
                (Field::Core, c.core.clone()),
            ]
        })
        .collect()
}

fn wires(prj: &Project, an: &Analysis) -> Vec<Row> {
    wiring::steps(prj, &an.ends, &an.wiring, None, Order::CrossSection)
        .into_iter()
        .enumerate()
        .map(|(i, s)| {
            let w = &an.wiring[s.wire];
            // The ferrule fields carry the part of any terminal of the end; a twin ferrule
            // the end of the wire it is crimped with first.
            let [strip_from, strip_to] = s.ends.map(|e| e.map(|e| fmt_number(e.strip)).unwrap_or_default());
            let [ferrule_from, ferrule_to] = s.ends.map(|e| e.filter(|e| !e.shared).map(|e| e.part()).unwrap_or_default());
            let [twin_from, twin_to] = s.twins.clone().map(|t| t.map(|t| t.with).unwrap_or_default());
            let status = match s.status {
                Status::Open => "open",
                Status::Done => "wired",
                Status::Changed => "changed",
                Status::Problem => "problem",
            };
            vec![
                (Field::Job, (i + 1).to_string()),
                (Field::Enclosure, w.enclosure_names(&prj.layout)),
                (Field::WireNumber, w.number.clone()),
                (Field::From, w.from.clone()),
                (Field::To, w.to.clone()),
                (Field::Net, w.net.clone()),
                (Field::Color, w.color.clone()),
                (Field::CrossSection, w.cross_section.clone()),
                (Field::Length, format!("{:.0}", w.length)),
                (Field::StripFrom, strip_from),
                (Field::StripTo, strip_to),
                (Field::FerruleFrom, ferrule_from),
                (Field::FerruleTo, ferrule_to),
                (Field::TwinFrom, twin_from),
                (Field::TwinTo, twin_to),
                (Field::Quantity, "1".into()),
                (Field::Status, status.into()),
                (Field::Note, s.note.replace(['\n', '\r'], " ")),
            ]
        })
        .collect()
}

fn documents(prj: &Project) -> Vec<Row> {
    prj.pages
        .iter()
        .map(|p| {
            let link = prj.integration.documents.get(&p.name).cloned().unwrap_or_default();
            vec![
                (Field::Document, p.name.clone()),
                (Field::Description, p.description.clone()),
                (Field::State, link.state),
                (Field::DocumentNumber, link.number),
                (Field::Revision, link.revision),
                (Field::Status, page_kind(p.kind).into()),
            ]
        })
        .collect()
}

fn project(prj: &Project) -> Value {
    let i = &prj.info;
    let d = &prj.integration.project;
    json!({
        "name": i.name,
        "description": i.description,
        "company": i.company,
        "creator": i.creator,
        "date": i.date,
        "project_revision": i.revision,
        Field::DocumentNumber.code(): d.number,
        Field::Revision.code(): d.revision,
        Field::State.code(): d.state,
        "properties": i.properties,
    })
}

pub fn erp(prj: &Project, an: &Analysis) -> String {
    let rows: Vec<Row> = items(prj, an)
        .into_iter()
        .map(|r| r.into_iter().filter(|(f, _)| !matches!(f, Field::ItemId | Field::Revision | Field::Lifecycle)).collect())
        .collect();
    csv(&rows)
}

pub fn pdm(prj: &Project, _: &Analysis) -> String {
    let revisions: Vec<Value> = prj.revisions.iter().map(|r| json!({ "name": r.name, "date": r.date, "author": r.author, "description": r.description })).collect();
    let v = json!({
        "format": "openecraft.pdm/1",
        "project": project(prj),
        "documents": objects(&documents(prj)),
        "revisions": revisions,
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

pub fn plm(prj: &Project, an: &Analysis) -> String {
    // Engineering BOM: location → device → parts.
    let mut locations: std::collections::BTreeMap<String, Vec<Value>> = Default::default();
    for (tag, d) in prj.devices.iter().filter(|(_, d)| !d.parts.is_empty()) {
        let parts: Vec<Value> = d.parts.iter().map(|r| json!({ Field::PartNumber.code(): r.number, Field::Quantity.code(): r.quantity() })).collect();
        locations.entry(enclosure_of(prj, tag)).or_default().push(json!({ Field::Tag.code(): tag, Field::Description.code(): d.description, "parts": parts }));
    }
    let structure: Vec<Value> = locations.into_iter().map(|(loc, devices)| json!({ Field::Enclosure.code(): loc, "devices": devices })).collect();
    let v = json!({
        "format": "openecraft.plm/1",
        "project": project(prj),
        "structure": structure,
        "items": objects(&items(prj, an)),
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

pub fn ecad(prj: &Project, an: &Analysis) -> String {
    let v = json!({
        "format": "openecraft.ecad/1",
        "project": prj.info.name,
        "components": objects(&devices(prj)),
        "nets": objects(&connections(prj, an)),
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}

pub fn mcad(prj: &Project, _: &Analysis) -> String {
    csv(&placements(prj))
}

pub fn procurement(prj: &Project, an: &Analysis) -> String {
    let mut rows: Vec<(String, String, Row)> = bom(prj, an)
        .into_iter()
        .map(|l| {
            let p = part(prj, &l.part).cloned().unwrap_or_default();
            let link = prj.integration.items.get(&l.part).cloned().unwrap_or_default();
            let need = (l.quantity - link.stock.unwrap_or(0.0)).max(0.0);
            let pack = p.pack_quantity.max(1) as f32;
            let to_order = (need / pack).ceil() * pack;
            let row = vec![
                (Field::Supplier, p.supplier.clone()),
                (Field::PartNumber, l.part.clone()),
                (Field::ErpNumber, link.erp_number),
                (Field::OrderNumber, p.order_number.clone()),
                (Field::Manufacturer, p.manufacturer.clone()),
                (Field::Description, p.description.clone()),
                (Field::Quantity, fmt_number(l.quantity)),
                (Field::Stock, link.stock.map(fmt_number).unwrap_or_default()),
                (Field::ToOrder, fmt_number(to_order)),
                (Field::Unit, l.unit),
                (Field::Price, fmt_number(p.price / p.price_unit.max(1) as f32)),
                (Field::Currency, p.currency.clone()),
                (Field::Total, format!("{:.2}", p.total_price(to_order))),
                (Field::OrderStatus, link.order_status),
                (Field::Ordered, link.ordered.map(fmt_number).unwrap_or_default()),
                (Field::Delivery, link.delivery),
            ];
            (p.supplier, l.part, row)
        })
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| crate::model::natural_cmp(&a.1, &b.1)));
    csv(&rows.into_iter().map(|r| r.2).collect::<Vec<_>>())
}

pub fn wiring_machine(prj: &Project, an: &Analysis) -> String {
    csv(&wires(prj, an))
}

/// The neutral exchange file: everything the connectors exchange in one JSON
/// document, for systems without a connector of their own.
pub fn exchange(prj: &Project, an: &Analysis) -> String {
    let v = json!({
        "format": "openecraft.exchange/1",
        "project": project(prj),
        "items": objects(&items(prj, an)),
        "devices": objects(&devices(prj)),
        "placements": objects(&placements(prj)),
        "connections": objects(&connections(prj, an)),
        "wires": objects(&wires(prj, an)),
        "documents": objects(&documents(prj)),
    });
    serde_json::to_string_pretty(&v).unwrap_or_default()
}
