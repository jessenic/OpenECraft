//! Device commands: list, rename, assign parts, accessories of their parts.

use crate::builtin::common::{check_placeholders, int_in, page_index};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::index::Index;
use openecraft_core::model::Project;
use openecraft_core::numbering::{self, DeviceNumbering};
use openecraft_core::ops;
use openecraft_core::parts::{self, PartsDb};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Devices;

impl Plugin for Devices {
    fn info(&self) -> Info {
        info("devices", tr_const!("Devices"), tr_const!("Lists and renames devices, adds accessories to their parts"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name:"add_device_part",
        help:tr_const!("Assigns a part with a per-terminal or total quantity to a device"),
        params:&[param("tag",Kind::Str,tr_const!("Device tag")),param("part",Kind::Str,tr_const!("Part number")),
            optional("count",Kind::Int,tr_const!("Quantity (default 1)")),optional("total",Kind::Bool,tr_const!("Count once for the whole device or terminal strip"))],
        run:add_device_part,
    },
    Spec {
        name:"set_device_part_quantity",
        help:tr_const!("Changes the quantity and counting mode of an assigned part"),
        params:&[param("tag",Kind::Str,tr_const!("Device tag")),param("index",Kind::Int,tr_const!("Assigned part index (1-based)")),
            optional("count",Kind::Int,tr_const!("Quantity (default 1)")),optional("total",Kind::Bool,tr_const!("Count once for the whole device or terminal strip"))],
        run:set_device_part_quantity,
    },
    Spec {
        name: "list_devices",
        help: tr_const!("Lists the devices with cross references and assigned parts"),
        params: &[],
        run: list_devices,
    },
    Spec {
        name: "rename_device",
        help: tr_const!("Renames a device with all its functions"),
        params: &[
            param("tag", Kind::Str, tr_const!("Current device tag")),
            param("new_tag", Kind::Str, tr_const!("New device tag")),
        ],
        run: rename_device,
    },
    Spec {
        name: "rename_symbol_device",
        help: tr_const!("Renames the device or cable referenced by a placed function"),
        params: &[
            param("id", Kind::Int, tr_const!("Symbol id")),
            param("new_tag", Kind::Str, tr_const!("New device tag")),
        ],
        run: rename_symbol_device,
    },
    Spec {
        name: "list_accessories",
        help: tr_const!("Lists the accessories that fit a part or the parts of a device, and the parts an accessory fits (project parts, built-in and bundled catalogs)"),
        params: &[
            optional("tag", Kind::Str, tr_const!("Device tag: the accessories that can still be added to its parts")),
            optional("part", Kind::Str, tr_const!("Part number")),
        ],
        run: list_accessories,
    },
    Spec {
        name: "add_accessory",
        help: tr_const!("Adds an accessory to a device; it must fit one of the device's parts"),
        params: &[
            param("tag", Kind::Str, tr_const!("Device tag")),
            param("part", Kind::Str, tr_const!("Part number of the accessory")),
            optional("count", Kind::Int, tr_const!("Quantity (default 1)")),
        ],
        run: add_accessory,
    },
    Spec {
        name: "list_unplaced",
        help: tr_const!("Lists PLC channels and functions of the assigned parts that are not placed yet, with the symbol, tag, label and pins for place_symbol"),
        params: &[],
        run: list_unplaced,
    },
    Spec {
        name: "renumber_devices",
        help: tr_const!("Numbers the device tags per structure and identifying letter in schematic order (page, column, top to bottom)"),
        params: &[
            optional("format", Kind::Str, tr_const!("Format of the device name: {letter}, {n} (counter), {page}, {column}, {row}; default {letter}{n}")),
            optional("start", Kind::Int, tr_const!("First counter value (default 1)")),
            optional("step", Kind::Int, tr_const!("Counting step (default 1)")),
            optional("pages", Kind::Str, tr_const!("Only devices on these pages (names separated by commas)")),
            optional("letters", Kind::Str, tr_const!("Only these identifying letters (separated by commas, e.g. K,Q)")),
            optional("terminals", Kind::Bool, tr_const!("Also renumber terminal strips, plugs and PLC cards")),
            optional("dry_run", Kind::Bool, tr_const!("Only list the new tags, change nothing")),
        ],
        run: renumber_devices,
    },
];

/// The parts accessories are looked up in: the generic catalog and the manufacturer
/// catalogs shipped with OpenECraft.
fn accessory_db() -> PartsDb {
    PartsDb { parts: parts::bundled::default_parts() }
}

fn add_device_part(prj:&mut Project,a:&Args)->Result<Value,String> {
    let tag=a.req("tag").trim();let number=a.req("part").trim();
    if tag.is_empty() {return Err(tr("Device tag must not be empty").into());}
    let part=prj.parts.iter().chain(parts::catalog()).find(|p|p.number==number).cloned().ok_or_else(||tr_fmt!("Unknown part: {}",number))?;
    let count=int_in(a,"count",0,u32::MAX.into())?.unwrap_or(1) as u32;let total=a.flag("total");
    if count==0 && !total {return Err(tr("Zero quantity requires total counting mode").into());}
    if !prj.parts.iter().any(|p|p.number==number) {prj.parts.push(part);}
    let data=prj.devices.entry(tag.into()).or_default();
    let index=if let Some(index)=data.parts.iter().position(|p|p.number==number && p.total==total) {
        data.parts[index].count=data.parts[index].count.checked_add(count).ok_or_else(||tr("Part quantity exceeds the supported range").to_string())?;index
    } else {data.parts.push(parts::PartRef {number:number.into(),count,total});data.parts.len()-1};
    Ok(json!({"tag":tag,"index":index+1,"part":data.parts[index]}))
}

fn set_device_part_quantity(prj:&mut Project,a:&Args)->Result<Value,String> {
    let tag=a.req("tag");let index=int_in(a,"index",1,u32::MAX.into())?.unwrap_or(1) as usize-1;
    let count=int_in(a,"count",0,u32::MAX.into())?.map(|v|v as u32);
    let total=a.value("total").and_then(Value::as_bool);
    let data=prj.devices.get_mut(tag).ok_or_else(||tr_fmt!("No device {}",tag))?;
    let part=data.parts.get_mut(index).ok_or_else(||tr_fmt!("No part assignment {} on device {}",index+1,tag))?;
    if count.unwrap_or(part.count)==0 && !total.unwrap_or(part.total) {return Err(tr("Zero quantity requires total counting mode").into());}
    if let Some(count)=count {part.count=count;}
    if let Some(total)=total {part.total=total;}
    Ok(json!({"tag":tag,"index":index+1,"part":part}))
}

fn brief(p: &parts::Part) -> Value {
    json!({ "number": p.number, "description": p.description, "mount": p.accessory_mount.key() })
}

fn list_accessories(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let db = accessory_db();
    if let Some(tag) = a.str("tag").filter(|t| !t.trim().is_empty()) {
        if !prj.devices.contains_key(tag) {
            return Err(tr_fmt!("No device {}", tag));
        }
        let choices: Vec<Value> = parts::accessory_choices(prj, Some(&db), tag).into_iter().map(|(p, main)| json!({ "fits": main, "accessory": brief(&p) })).collect();
        return Ok(json!({ "tag": tag, "accessories": choices }));
    }
    let number = a.str("part").filter(|n| !n.trim().is_empty()).ok_or_else(|| tr("Give a device tag or a part number").to_string())?;
    let pool: Vec<parts::Part> = prj.parts.iter().cloned().chain(db.parts.into_iter().filter(|p| !prj.parts.iter().any(|q| q.number == p.number))).collect();
    let part = pool.iter().find(|p| p.number == number).ok_or_else(|| tr_fmt!("Unknown part: {}", number))?;
    let accessories: Vec<Value> = parts::accessory::accessories_of(&pool, part).into_iter().map(brief).collect();
    let fits: Vec<Value> = parts::accessory::mains_of(&pool, part).into_iter().map(|p| json!({ "number": p.number, "description": p.description })).collect();
    Ok(json!({ "part": number, "accessory": part.accessory, "mount": part.accessory_mount.key(), "accessories": accessories, "fits": fits }))
}

fn add_accessory(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (tag, number) = (a.req("tag").trim(), a.req("part").trim());
    let count = int_in(a, "count", 1, 999)?.unwrap_or(1) as u32;
    let main = parts::add_accessory(prj, &accessory_db(), tag, number, count)?;
    let assigned: Vec<Value> = prj.devices[tag].parts.iter().map(|r| json!({ "number": r.number, "count": r.count })).collect();
    Ok(json!({ "tag": tag, "accessory": number, "fits": main, "parts": assigned }))
}

fn list_unplaced(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let item = |kind: &str, u: openecraft_core::unplaced::Unplaced| json!({ "kind": kind, "tag": u.tag, "symbol": u.symbol, "label": u.label, "pins": u.pins, "title": u.title });
    let mut v: Vec<Value> = openecraft_core::unplaced::plc_channels(prj).into_iter().map(|u| item("plc_channel", u)).collect();
    v.extend(openecraft_core::unplaced::device_functions(prj, None).into_iter().map(|u| item("function", u)));
    v.extend(openecraft_core::unplaced::potentials(prj).into_iter().map(|u| item("potential", u)));
    Ok(Value::Array(v))
}

/// Page indices of a comma separated list of page names.
pub(crate) fn page_list(prj: &Project, list: Option<&str>) -> Result<Vec<usize>, String> {
    list.unwrap_or_default().split(',').map(str::trim).filter(|s| !s.is_empty()).map(|name| page_index(prj, name)).collect()
}

fn renumber_devices(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let defaults = DeviceNumbering::default();
    if let Some(f) = a.str("format") {
        check_placeholders(f, &["letter", "n", "page", "column", "row"])?;
    }
    let opts = DeviceNumbering {
        format: a.str("format").filter(|f| !f.trim().is_empty()).map(str::to_string).unwrap_or(defaults.format),
        start: int_in(a, "start", 0, u32::MAX.into())?.map_or(defaults.start, |v| v as u32),
        step: int_in(a, "step", 1, u32::MAX.into())?.map_or(defaults.step, |v| v as u32),
        pages: page_list(prj, a.str("pages"))?,
        letters: crate::builtin::common::words(a.str("letters").unwrap_or_default()).into_iter().map(String::from).collect(),
        skip_terminals: !a.flag("terminals"),
    };
    let renamed = if a.flag("dry_run") { numbering::plan_devices(prj, &opts) } else { numbering::renumber_devices(prj, &opts) };
    Ok(json!({ "renamed": renamed.iter().map(|(old, new)| json!({ "old": old, "new": new })).collect::<Vec<_>>() }))
}

fn list_devices(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let idx = Index::build(prj);
    let refs = |locs: &[openecraft_core::index::Loc]| locs.iter().map(|l| l.reference(prj)).collect::<Vec<_>>();
    let mut tags=idx.sorted_tags();tags.extend(prj.devices.iter().filter(|(tag,d)|!idx.devices.contains_key(*tag) && parts::is_material_device(d)).map(|(tag,_)|tag));
    tags.extend(openecraft_core::panel::material_tags(prj));tags.sort();tags.dedup();
    tags.sort_by(|a,b|openecraft_core::model::natural_cmp(a,b));
    let empty=openecraft_core::index::DeviceEntry::default();
    let list = tags.into_iter().map(|tag| {
        let e = idx.devices.get(tag).unwrap_or(&empty);
        let data = prj.devices.get(tag);
        let on = |n: &str| prj.parts.iter().any(|p| p.number == n && p.accessory).then(|| parts::attached_to(prj, None, tag, n).map(|m| m.number.clone()));
        let parts: Vec<Value> = data
            .map(|d| {
                d.parts
                    .iter()
                    .map(|r| match on(&r.number) {
                        Some(main) => json!({ "number": r.number, "count": r.count, "total":r.total, "accessory_of": main }),
                        None => json!({ "number": r.number, "count": r.count, "total":r.total }),
                    })
                    .collect()
            })
            .unwrap_or_default();
        json!({
            "tag": tag,
            "description": data.map(|d| d.description.as_str()).unwrap_or_default(),
            "owner_device":data.and_then(|d|d.values.get("device.material_owner")).map(String::as_str).unwrap_or(""),
            "bom_excluded":parts::excluded_from_bom(prj,tag),
            "parts": parts,
            "main": refs(&e.mains),
            "auxiliary": refs(&e.auxs),
            "other": refs(&e.others),
            "overview": refs(&e.overviews),
        })
    });
    Ok(Value::Array(list.collect()))
}

fn rename_device(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (old, new) = (a.req("tag").trim(), a.req("new_tag").trim());
    if !prj.all_tags().iter().any(|t| t == old) && !prj.unplaced_tags().any(|t|t==old) {
        return Err(tr_fmt!("No device {}", old));
    }
    if let Some(problem) = ops::rename_problem(prj, old, new) {
        return Err(problem);
    }
    ops::rename_device(prj, old, new);
    Ok(json!({ "old": old, "new": new }))
}

fn rename_symbol_device(prj:&mut Project,a:&Args)->Result<Value,String> {
    let id=a.int("id").unwrap_or_default() as u64;let new=a.req("new_tag").trim();
    let changed=ops::rename_symbol_device(prj,id,new)?;
    Ok(json!({"id":id,"new_tag":new,"changed":changed}))
}

#[cfg(test)]
mod tests;
