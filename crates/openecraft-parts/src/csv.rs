//! CSV / JSON import and export of parts.

use crate::{points_from_text, points_to_text, FnKind, FunctionTemplate, Part, PartsDb, TopGroup};

const CSV_FIELDS: [&str; 33] = [
    "Part number",
    "Type number",
    "Order number",
    "Manufacturer",
    "Supplier",
    "Description",
    "Note",
    "Product top group",
    "Product group",
    "Product subgroup",
    "Identifier",
    "Price",
    "Currency",
    "Price unit",
    "Pack quantity",
    "Quantity unit",
    "Voltage",
    "Current",
    "Power",
    "Technical data",
    "Certificates",
    "Datasheet",
    "Width",
    "Height",
    "Depth",
    "Weight",
    "Mounting",
    "Functions",
    "Accessories",
    "Connection points",
    "Catalog",
    "Lifecycle",
    "Successor",
];

pub fn csv_escape(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn to_csv(parts: &[Part]) -> String {
    let mut out = CSV_FIELDS.join(";");
    for key in crate::MOUNTING_KEYS {
        out.push(';');
        out.push_str(key);
    }
    out.push_str(";Components;Accessory;Accessory mounting;Fits");
    out.push('\n');
    for p in parts {
        let functions = p
            .functions
            .iter()
            .map(|f| format!("{}:{}", f.kind.short(), f.pins))
            .collect::<Vec<_>>()
            .join("|");
        let top = match p.top_group {
            TopGroup::Electrical => "Electrical",
            TopGroup::Fluid => "Fluid",
            TopGroup::Mechanics => "Mechanics",
            TopGroup::Process => "Process",
        };
        let row = [
            p.number.clone(),
            p.type_number.clone(),
            p.order_number.clone(),
            p.manufacturer.clone(),
            p.supplier.clone(),
            p.description.clone(),
            p.note.clone(),
            top.to_string(),
            p.group.clone(),
            p.subgroup.clone(),
            p.identifier.clone(),
            format!("{:.2}", p.price),
            p.currency.clone(),
            p.price_unit.to_string(),
            p.pack_quantity.to_string(),
            p.quantity_unit.clone(),
            p.voltage.clone(),
            p.current.clone(),
            p.power.clone(),
            p.tech_data.clone(),
            p.certificates.clone(),
            p.datasheet.clone(),
            p.width.to_string(),
            p.height.to_string(),
            p.depth.to_string(),
            p.weight.to_string(),
            p.mounting.clone(),
            functions,
            p.accessories.join(","),
            points_to_text(&p.connection_points),
            p.catalog.clone(),
            format!("{:?}", p.lifecycle),
            p.successor.clone(),
        ];
        let typed = crate::MOUNTING_KEYS.iter().map(|k| p.mounting_data.get(k).unwrap_or_default());
        let components = [crate::components_to_text(&p.components), if p.accessory { "yes".into() } else { String::new() }, if p.accessory_mount.is_auto() { String::new() } else { p.accessory_mount.key().into() }, p.fits.join(",")];
        out.push_str(&row.into_iter().chain(typed).chain(components).map(|c| csv_escape(&c)).collect::<Vec<_>>().join(";"));
        out.push('\n');
    }
    out
}

/// Splits CSV text (separator `;` or `,`) into records, honouring quotes.
pub fn parse_csv(text: &str) -> Vec<Vec<String>> {
    let sep = if text.lines().next().map(|l| l.contains(';')).unwrap_or(true) { ';' } else { ',' };
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            c if c == sep && !quoted => row.push(std::mem::take(&mut cell)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                row.push(std::mem::take(&mut cell));
                if row.iter().any(|c| !c.is_empty()) {
                    rows.push(std::mem::take(&mut row));
                } else {
                    row.clear();
                }
            }
            c => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

/// Reads parts from CSV. Columns are matched by header name; unknown columns are ignored.
/// Lines starting with `#` before the header (the export information of the parts
/// server) are skipped.
pub fn from_csv(text: &str) -> Result<Vec<Part>, String> {
    let mut text = text.trim_start_matches('\u{feff}');
    while text.starts_with('#') {
        text = text.split_once('\n').map_or("", |(_, rest)| rest);
    }
    let rows = parse_csv(text);
    let Some((header, data)) = rows.split_first() else { return Err("empty file".into()) };
    let col = |name: &str| header.iter().position(|h| h.trim().eq_ignore_ascii_case(name));
    if col("Part number").is_none() {
        return Err("column 'Part number' missing".into());
    }
    let idx: Vec<Option<usize>> = CSV_FIELDS.iter().map(|f| col(f)).collect();
    let typed: Vec<(&str, usize)> = crate::MOUNTING_KEYS.iter().filter_map(|k| Some((*k, col(k)?))).collect();
    let components_col = col("Components");
    let accessory_cols = [col("Accessory"), col("Accessory mounting"), col("Fits")];
    let mut parts = Vec::new();
    for r in data {
        let get = |i: usize| idx[i].and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default();
        let num = |i: usize| get(i).replace(',', ".").parse::<f32>().unwrap_or(0.0);
        let int = |i: usize, d: u32| get(i).parse::<u32>().unwrap_or(d);
        let number = get(0);
        if number.is_empty() {
            continue;
        }
        let top_group = match get(7).to_lowercase().as_str() {
            s if s.starts_with("fluid") => TopGroup::Fluid,
            s if s.starts_with("mech") => TopGroup::Mechanics,
            s if s.starts_with("proc") => TopGroup::Process,
            _ => TopGroup::Electrical,
        };
        let functions = get(27)
            .split('|')
            .filter(|s| !s.trim().is_empty())
            .filter_map(|s| {
                let (k, pins) = s.split_once(':').unwrap_or((s, ""));
                FnKind::from_short(k).map(|k| FunctionTemplate::new(k, pins.trim()))
            })
            .collect();
        parts.push(Part {
            number,
            type_number: get(1),
            order_number: get(2),
            manufacturer: get(3),
            supplier: get(4),
            description: get(5),
            note: get(6),
            top_group,
            group: get(8),
            subgroup: get(9),
            identifier: get(10),
            price: num(11),
            currency: if get(12).is_empty() { "EUR".into() } else { get(12) },
            price_unit: int(13, 1).max(1),
            pack_quantity: int(14, 1).max(1),
            quantity_unit: if get(15).is_empty() { "pcs".into() } else { get(15) },
            voltage: get(16),
            current: get(17),
            power: get(18),
            tech_data: get(19),
            certificates: get(20),
            datasheet: get(21),
            width: num(22),
            height: num(23),
            depth: num(24),
            weight: num(25),
            mounting: get(26),
            functions,
            accessories: get(28).split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            // 2D / 3D models are only exchanged as JSON.
            model_2d: None,
            model_3d: None,
            connection_points: points_from_text(&get(29)),
            catalog: get(30),
            lifecycle: crate::portal::lifecycle_from_key(&get(31)).unwrap_or_default(),
            successor: get(32),
            ..Default::default()
        });
        if let Some(part) = parts.last_mut() {
            if let Some(c) = components_col {
                part.components = crate::components_from_text(r.get(c).map(|s| s.as_str()).unwrap_or_default());
            }
            let [flag, mount, fits] = accessory_cols.map(|c| c.and_then(|c| r.get(c)).map(|s| s.trim()).unwrap_or_default());
            part.accessory = matches!(flag.to_ascii_lowercase().as_str(), "yes" | "true" | "1" | "x");
            part.accessory_mount = crate::AccessoryMount::from_key(mount).unwrap_or_default();
            part.fits = fits.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
            for (key, c) in &typed {
                let v = r.get(*c).map(|s| s.trim()).unwrap_or_default();
                if !v.is_empty() && !part.mounting_data.set(key, v) {
                    part.tech_data = [part.tech_data.as_str(), &format!("{key}: {v}")].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n");
                }
            }
        }
    }
    Ok(parts)
}

/// Imports a parts file (JSON database / part list, BMEcat or CSV). Returns the parts.
pub fn import(text: &str) -> Result<Vec<Part>, String> {
    let mut parts = import_raw(text)?;
    for p in &mut parts {
        p.migrate_tech_data();
    }
    Ok(parts)
}

fn import_raw(text: &str) -> Result<Vec<Part>, String> {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    if crate::bmecat::is_bmecat(t) {
        crate::bmecat::from_bmecat(t)
    } else if crate::eplan::is_eplan_xml(t) {
        crate::eplan::from_eplan_xml(t)
    } else if t.starts_with('{') {
        serde_json::from_str::<PartsDb>(t).map(|d| d.parts).map_err(|e| e.to_string())
    } else if t.starts_with('[') {
        serde_json::from_str::<Vec<Part>>(t).map_err(|e| e.to_string())
    } else {
        from_csv(t)
    }
}

#[cfg(test)]
mod tests;
