//! Exchange with enclosure-system configurators: the configuration of an enclosure
//! (system, size, components, accessories and the machining of plate, door and walls)
//! as JSON for the configurator of the enclosure maker, the import of a configured
//! enclosure, and the smallest standard size of a system the layout fits into.
//!
//! Positions in the files are millimetres from the bottom left corner of the face,
//! y up (like the NC lists), centres of holes, cutouts and accessories.

use crate::model::Project;
use crate::panel::{self, Enclosure, Face, ItemKind, PanelItem, PLATE_MARGIN, PLATE_THICKNESS, PLATE_Z, SYSTEMS};
use crate::production::{holes_on, openings};
use openecraft_i18n::{tr, tr_fmt};
use serde_json::{json, Map, Value};

pub const FORMAT: &str = "openecraft-enclosure/1";

/// The configuration of an enclosure for the configurator.
pub fn export(prj: &Project, enc: usize) -> Value {
    let e = &prj.layout.enclosures[enc];
    let r = |v: f32| (v * 10.0).round() / 10.0;
    let components: Vec<Value> = panel::components(e).into_iter().filter(|c| !c.part.is_empty()).map(|c| json!({ "role": c.role.title(), "part": c.part, "count": c.count })).collect();
    let mut accessories = Vec::new();
    let mut machining = Vec::new();
    for face in Face::ALL {
        let (_, fh) = e.face_size(face);
        for it in e.items.iter().filter(|i| i.face == face && i.kind == ItemKind::Accessory) {
            let c = it.center();
            accessories.push(json!({ "part": it.part, "face": face.key(), "x": r(c.x), "y": r(fh - c.y), "w": r(it.w), "h": r(it.h) }));
        }
        for h in holes_on(e, &prj.parts, face) {
            machining.push(json!({ "face": face.key(), "kind": "hole", "x": r(h.pos.x), "y": r(fh - h.pos.y), "diameter": r(h.diameter), "thread": h.thread, "fastening": h.fastening.key() }));
        }
        for o in openings(e, &prj.parts, face) {
            let (x, y) = (r(o.center.x), r(fh - o.center.y));
            machining.push(if o.round {
                json!({ "face": face.key(), "kind": "cutout", "x": x, "y": y, "diameter": r(o.w) })
            } else {
                let mut c = json!({ "face": face.key(), "kind": "cutout", "x": x, "y": y, "w": r(o.w), "h": r(o.h) });
                if o.angle != 0.0 {
                    c["angle"] = json!(r(o.angle));
                }
                if o.radius > 0.0 {
                    c["radius"] = json!(r(o.radius));
                }
                c
            });
        }
    }
    json!({
        "format": FORMAT,
        "project": prj.info.name,
        "enclosure": e.name,
        "system": e.build.system,
        "part": e.part,
        "size": [e.width, e.height, e.depth],
        "plinth": e.build.plinth,
        "left_open": e.build.left_open,
        "right_open": e.build.right_open,
        "gland_plate": e.build.gland_plate,
        "components": components,
        "accessories": accessories,
        "machining": machining,
    })
}

/// What an import did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Imported {
    pub enclosure: String,
    pub created: bool,
    pub accessories: usize,
    pub cutouts: usize,
    /// Entries that were not understood.
    pub skipped: Vec<String>,
}

fn get<'a>(o: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| o.get(*k))
}

fn number(o: &Map<String, Value>, keys: &[&str]) -> Option<f32> {
    get(o, keys).and_then(|v| v.as_f64().map(|f| f as f32).or_else(|| v.as_str().and_then(|s| s.trim().replace(',', ".").parse().ok())))
}

fn text<'a>(o: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    get(o, keys).and_then(Value::as_str)
}

/// Imports a configured enclosure: system, size, components, accessories and cutouts.
/// An enclosure of the same name is updated (its accessories and cutouts are kept,
/// those of the configuration are added unless one of the same part sits at the same
/// place), otherwise a new one is added. Keys are the ones of [`export`]; also
/// `name`, `article` / `article_number` and `width` / `height` / `depth`.
pub fn import(prj: &mut Project, text_in: &str) -> Result<Imported, String> {
    let v: Value = serde_json::from_str(text_in).map_err(|e| tr_fmt!("Invalid configuration: {}", e))?;
    let o = v.as_object().ok_or_else(|| tr("The configuration is not a JSON object").to_string())?;
    let name = text(o, &["enclosure", "name", "location"]).map(str::trim).filter(|s| !s.is_empty()).unwrap_or("+CAB1").to_string();
    let mut out = Imported { enclosure: name.clone(), ..Default::default() };
    let ei = match super::enclosure_index(prj, &name) {
        Some(i) => i,
        None => {
            let part = text(o, &["part", "article", "article_number"]).unwrap_or(panel::ENCLOSURE_PART).to_string();
            let e = panel::new_enclosure(prj, &part, &name);
            prj.layout.enclosures.push(e);
            out.created = true;
            prj.layout.enclosures.len() - 1
        }
    };
    let size = match get(o, &["size"]).and_then(Value::as_array) {
        Some(a) if a.len() == 3 => {
            let n: Vec<f32> = a.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
            (n.len() == 3).then(|| (n[0], n[1], n[2]))
        }
        _ => match (number(o, &["width"]), number(o, &["height"]), number(o, &["depth"])) {
            (Some(w), Some(h), Some(d)) => Some((w, h, d)),
            _ => None,
        },
    };
    {
        let e = &mut prj.layout.enclosures[ei];
        if let Some(system) = text(o, &["system"]).map(str::trim).filter(|s| !s.is_empty()) {
            if SYSTEMS.iter().all(|s| s.id() != system) {
                return Err(tr_fmt!("Unknown enclosure system: {}", system));
            }
            panel::apply_system(e, system, size);
        } else if let Some((w, h, d)) = size {
            (e.width, e.height, e.depth) = (w, h, d);
        }
        for v in [&mut e.width, &mut e.height, &mut e.depth] {
            *v = v.max(50.0);
        }
        if let Some(p) = number(o, &["plinth"]) {
            e.build.plinth = p.max(0.0);
        }
        let bayable = e.system().is_some_and(|s| s.data.bayable);
        for (keys, field) in [(&["left_open"][..], &mut e.build.left_open), (&["right_open"][..], &mut e.build.right_open), (&["gland_plate"][..], &mut e.build.gland_plate)] {
            if let Some(b) = get(o, keys).and_then(Value::as_bool) {
                *field = b;
            }
        }
        if !bayable {
            e.build.left_open = false;
            e.build.right_open = false;
        }
        panel::sync_part(e);
        panel::apply_anchors(e);
    }
    let list = |key: &str| get(o, &[key]).and_then(Value::as_array).cloned().unwrap_or_default();
    for a in list("accessories") {
        let Some(a) = a.as_object() else { continue };
        let part = text(a, &["part", "article", "article_number"]).unwrap_or_default().to_string();
        let face = text(a, &["face"]).and_then(Face::from_key).unwrap_or(Face::Plate);
        let (Some(x), Some(y)) = (number(a, &["x"]), number(a, &["y"])) else {
            out.skipped.push(part);
            continue;
        };
        let part_data = prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == part);
        let w = number(a, &["w", "width"]).or(part_data.map(|p| p.width)).filter(|v| *v > 0.0).unwrap_or(40.0);
        let h = number(a, &["h", "height"]).or(part_data.map(|p| p.height)).filter(|v| *v > 0.0).unwrap_or(40.0);
        let d = part_data.map(|p| p.depth).filter(|v| *v > 0.0).unwrap_or(20.0);
        if add_item(prj, ei, PanelItem { kind: ItemKind::Accessory, part, face, w, h, d, ..Default::default() }, x, y) {
            out.accessories += 1;
        }
    }
    let cut_list = [list("cutouts"), list("machining")].concat();
    for c in cut_list {
        let Some(c) = c.as_object() else { continue };
        if text(c, &["kind"]).is_some_and(|k| k != "cutout") {
            continue;
        }
        let face = text(c, &["face"]).and_then(Face::from_key).unwrap_or(Face::Plate);
        let (Some(x), Some(y)) = (number(c, &["x"]), number(c, &["y"])) else {
            out.skipped.push(format!("{c:?}"));
            continue;
        };
        let item = match number(c, &["diameter", "d"]) {
            Some(dia) => PanelItem { kind: ItemKind::Cutout, face, w: dia, h: dia, round: true, ..Default::default() },
            None => match (number(c, &["w", "width"]), number(c, &["h", "height"])) {
                (Some(w), Some(h)) => PanelItem { kind: ItemKind::Cutout, face, w, h, ..Default::default() },
                _ => {
                    out.skipped.push(format!("{c:?}"));
                    continue;
                }
            },
        };
        if add_item(prj, ei, item, x, y) {
            out.cutouts += 1;
        }
    }
    Ok(out)
}

/// Adds an item centred at (x, y) from the bottom left of its face, unless one of the
/// same kind and part sits there already.
fn add_item(prj: &mut Project, ei: usize, mut it: PanelItem, x: f32, y: f32) -> bool {
    let (_, fh) = prj.layout.enclosures[ei].face_size(it.face);
    it.x = x - it.w / 2.0;
    it.y = fh - y - it.h / 2.0;
    let e = &prj.layout.enclosures[ei];
    if e.items.iter().any(|i| i.face == it.face && i.kind == it.kind && i.part == it.part && (i.center().x - x).abs() < 0.5 && (fh - i.center().y - y).abs() < 0.5 && (i.w - it.w).abs() < 0.5) {
        return false;
    }
    it.id = prj.new_id();
    prj.layout.enclosures[ei].items.push(it);
    true
}

/// Space the layout of an enclosure needs: (width, height, depth).
pub fn needed(e: &Enclosure) -> (f32, f32, f32) {
    let (mut pw, mut ph, mut pz) = (0.0f32, 0.0f32, 0.0f32);
    let (mut dw, mut dh, mut dz) = (0.0f32, 0.0f32, 0.0f32);
    for it in &e.items {
        match it.face {
            Face::Plate => {
                pw = pw.max(it.x + it.w);
                ph = ph.max(it.y + it.h);
                pz = pz.max(it.z + it.d);
            }
            Face::Door => {
                dw = dw.max(it.x + it.w);
                dh = dh.max(it.y + it.h);
                dz = dz.max(it.d);
            }
            _ => {}
        }
    }
    let flange = e.flange();
    let width = (pw + 2.0 * PLATE_MARGIN).max(if dw > 0.0 { dw + flange } else { 0.0 });
    let height = (ph + 2.0 * PLATE_MARGIN).max(if dh > 0.0 { dh + flange } else { 0.0 });
    let depth = (PLATE_Z + PLATE_THICKNESS + pz + e.door_clearance).max(PLATE_Z + dz);
    (width, height, depth)
}

/// The smallest standard size of a system (front area, then depth) the layout of an
/// enclosure fits into.
pub fn suggest(e: &Enclosure, system: &str) -> Option<(f32, f32, f32)> {
    let s = SYSTEMS.iter().find(|s| s.id() == system)?;
    let probe = Enclosure { build: panel::Build { system: system.into(), ..e.build.clone() }, items: e.items.clone(), door_clearance: e.door_clearance, ..Default::default() };
    let (w, h, d) = needed(&probe);
    s.data
        .sizes
        .iter()
        .copied()
        .filter(|(sw, sh, sd)| *sw + 1e-3 >= w && *sh + 1e-3 >= h && *sd + 1e-3 >= d)
        .min_by(|a, b| (a.0 * a.1).total_cmp(&(b.0 * b.1)).then(a.2.total_cmp(&b.2)))
}

#[cfg(test)]
mod tests;
