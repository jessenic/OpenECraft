//! Panel layout commands: enclosure systems and their components, accessories on the
//! plate, the door and the walls, fastening, parametric placement (anchors, patterns,
//! aligning, rails), assemblies and 3D macros.

use crate::builtin::common::{enclosure_index, int_in, num_in, NO_MAX};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::geom::p;
use openecraft_core::model::Project;
use openecraft_core::panel::{self, Align, AlignTo, Anchor, Enclosure, Face, Fastening, ItemGroup, PanelItem};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Panel;

impl Plugin for Panel {
    fn info(&self) -> Info {
        info("panel", tr_const!("Panel layout"), tr_const!("Enclosures, panel items, accessories and panel macros"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name:"place_panel_device",
        help:tr_const!("Places a device or an individual terminal with explicit physical dimensions"),
        params:&[
            param("enclosure",Kind::Str,ENCLOSURE),
            param("tag",Kind::Str,tr_const!("Device tag")),
            optional("terminal",Kind::Str,tr_const!("Terminal designation; empty for the whole device")),
            optional("single_part",Kind::Bool,tr_const!("This footprint represents only its own part")),
            optional("part",Kind::Str,tr_const!("Part number")),
            param("width",Kind::Num,tr_const!("Width in mm")),
            param("height",Kind::Num,tr_const!("Height in mm")),
            optional("depth",Kind::Num,tr_const!("Depth in mm")),
            optional("face",Kind::Str,FACE),
            optional("x",Kind::Num,tr_const!("Left edge on the face in mm")),
            optional("y",Kind::Num,tr_const!("Top edge on the face in mm")),
            optional("z",Kind::Num,tr_const!("Distance from the face in mm (rails: standoff height)")),
        ],
        run:place_panel_device,
    },
    Spec {
        name:"get_panel_points",
        help:tr_const!("Lists physical device connection points, including separately placed terminals"),
        params:&[optional("enclosure",Kind::Str,ENCLOSURE),optional("id",Kind::Int,tr_const!("Item id"))],
        run:get_panel_points,
    },
    Spec {
        name: "list_enclosures",
        help: tr_const!("Lists the enclosures with their system, components (assembly), items per face and assemblies"),
        params: &[optional("enclosure", Kind::Str, ENCLOSURE)],
        run: list_enclosures,
    },
    Spec {
        name: "add_enclosure",
        help: tr_const!("Adds an enclosure: of a system in one of its sizes (or a custom size), or from an enclosure part"),
        params: &[
            param("name", Kind::Str, ENCLOSURE),
            optional("system", Kind::Str, tr_const!("compact, baying, terminal_box, plastic, house_connection or rack19")),
            optional("part", Kind::Str, tr_const!("Enclosure part number")),
            optional("device_tag",Kind::Str,tr_const!("Device tag assigned to this enclosure body")),
            optional("owner_device",Kind::Str,tr_const!("Material owner device tag")),
            optional("bom_excluded",Kind::Bool,tr_const!("Exclude from bill of materials")),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("height", Kind::Num, tr_const!("Height in mm")),
            optional("depth", Kind::Num, tr_const!("Depth in mm")),
            optional("assembly", Kind::Bool, tr_const!("Open mounting assembly without enclosure walls")),
            optional("implicit_plate",Kind::Bool,tr_const!("Generate a default mounting plate when no explicit plate is placed")),
            optional("mounting_origin",Kind::Object,tr_const!("Mounting-face origin as x, y and z; empty object restores the default")),
        ],
        run: add_enclosure,
    },
    Spec {
        name: "set_enclosure",
        help: tr_const!("Changes the system, size and components of an enclosure (plinth, bayed sides, gland plate); anchored items follow"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            optional("assembly", Kind::Bool, tr_const!("Open mounting assembly without enclosure walls")),
            optional("implicit_plate",Kind::Bool,tr_const!("Generate a default mounting plate when no explicit plate is placed")),
            optional("mounting_origin",Kind::Object,tr_const!("Mounting-face origin as x, y and z; empty object restores the default")),
            optional("system", Kind::Str, tr_const!("compact, baying, terminal_box, plastic, house_connection or rack19; empty for none")),
            optional("part", Kind::Str, tr_const!("Enclosure part number")),
            optional("device_tag",Kind::Str,tr_const!("Device tag assigned to this enclosure body")),
            optional("owner_device",Kind::Str,tr_const!("Material owner device tag")),
            optional("bom_excluded",Kind::Bool,tr_const!("Exclude from bill of materials")),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("height", Kind::Num, tr_const!("Height in mm")),
            optional("depth", Kind::Num, tr_const!("Depth in mm")),
            optional("plinth", Kind::Num, tr_const!("Plinth height in mm, 0 without plinth")),
            optional("left_open", Kind::Bool, tr_const!("Left side bayed (no side panel)")),
            optional("right_open", Kind::Bool, tr_const!("Right side bayed (no side panel)")),
            optional("gland_plate", Kind::Bool, tr_const!("Bottom with removable gland plate")),
        ],
        run: set_enclosure,
    },
    Spec {
        name: "bay_enclosure",
        help: tr_const!("Bays an enclosure side by side to the right of another one (line-up); without right_of it leaves its line-up"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            optional("right_of", Kind::Str, tr_const!("Enclosure it stands right of; empty to take it out of its line-up")),
        ],
        run: bay_enclosure,
    },
    Spec {
        name: "add_cable_passage",
        help: tr_const!("Adds a cable passage to the right neighbour: round cutouts facing each other in both side walls; wires between the enclosures run through it"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            optional("y", Kind::Num, tr_const!("Centre from the top of the enclosure in mm")),
            optional("z", Kind::Num, tr_const!("Centre from the rear wall in mm")),
            optional("diameter", Kind::Num, tr_const!("Diameter in mm")),
        ],
        run: add_cable_passage,
    },
    Spec {
        name: "place_accessory",
        help: tr_const!("Places an enclosure accessory (fan, filter, cable gland, light, handle, PE bar) on a face"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("part", Kind::Str, tr_const!("Accessory part number, e.g. GEN.GLAND-M25")),
            param("face", Kind::Str, FACE),
            optional("x", Kind::Num, tr_const!("Left edge on the face in mm")),
            optional("y", Kind::Num, tr_const!("Top edge on the face in mm")),
            optional("anchor_x", Kind::Str, ANCHOR),
            optional("anchor_y", Kind::Str, ANCHOR),
            optional("fastening", Kind::Str, tr_const!("auto, tapped, through, rivet_nut, stud, clip or adhesive")),
        ],
        run: place_accessory,
    },
    Spec {
        name:"place_mounting_plate",
        help:tr_const!("Places an explicit mounting plate in a cabinet or open assembly"),
        params:&[
            param("enclosure",Kind::Str,ENCLOSURE),
            param("width",Kind::Num,tr_const!("Width in mm")),
            param("height",Kind::Num,tr_const!("Height in mm")),
            optional("thickness",Kind::Num,tr_const!("Plate thickness in mm")),
            optional("part",Kind::Str,tr_const!("Part number")),
            optional("x",Kind::Num,tr_const!("Left edge on the face in mm")),
            optional("y",Kind::Num,tr_const!("Top edge on the face in mm")),
            optional("z",Kind::Num,tr_const!("Distance from the face in mm (rails: standoff height)")),
        ],
        run:place_mounting_plate,
    },
    Spec {
        name: "set_panel_item",
        help: tr_const!("Changes position, face, fastening or anchors of a panel item"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("id", Kind::Int, tr_const!("Item id")),
            optional("part",Kind::Str,tr_const!("Part number")),
            optional("x", Kind::Num, tr_const!("Left edge on the face in mm")),
            optional("y", Kind::Num, tr_const!("Top edge on the face in mm")),
            optional("z", Kind::Num, tr_const!("Distance from the face in mm (rails: standoff height)")),
            optional("face", Kind::Str, FACE),
            optional("width",Kind::Num,tr_const!("Width in mm")),
            optional("height",Kind::Num,tr_const!("Height in mm")),
            optional("depth",Kind::Num,tr_const!("Depth in mm")),
            optional("anchor_x", Kind::Str, ANCHOR),
            optional("anchor_y", Kind::Str, ANCHOR),
            optional("fastening", Kind::Str, tr_const!("auto, tapped, through, rivet_nut, stud, clip or adhesive")),
            optional("mounted_on",Kind::Int,tr_const!("Mounting parent item id; 0 detaches without moving the item")),
            optional("rail_mount_override",Kind::Bool,tr_const!("Keep the instance mounting datum instead of refreshing it from its part")),
            optional("rail_vertical",Kind::Bool,tr_const!("Mounting rail runs vertically")),
            optional("rail_offset",Kind::Num,tr_const!("Mounting datum offset across the rail in mm")),
            optional("clip_on",Kind::Num,tr_const!("Distance from rail top to device rear in mm")),
            optional("terminal",Kind::Str,tr_const!("Terminal designation; empty for the whole device")),
            optional("single_part",Kind::Bool,tr_const!("This footprint represents only its own part")),
            optional("owner_device",Kind::Str,tr_const!("Material owner device tag")),
            optional("bom_excluded",Kind::Bool,tr_const!("Exclude from bill of materials")),
        ],
        run: set_panel_item,
    },
    Spec {
        name: "arrange_panel_items",
        help: tr_const!("Aligns items, spaces them evenly, lines devices up on a rail or copies items in a pattern"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("ids", Kind::Ids, IDS),
            param("action", Kind::Str, tr_const!("left, center_x, right, top, center_y, bottom, distribute_x, distribute_y, rail or pattern")),
            optional("rail", Kind::Int, tr_const!("Rail id (action rail)")),
            optional("gap", Kind::Num, tr_const!("Gap between the devices on the rail in mm")),
            optional("columns", Kind::Int, tr_const!("Columns of the pattern")),
            optional("rows", Kind::Int, tr_const!("Rows of the pattern")),
            optional("pitch_x", Kind::Num, tr_const!("Horizontal pitch of the pattern in mm")),
            optional("pitch_y", Kind::Num, tr_const!("Vertical pitch of the pattern in mm")),
        ],
        run: arrange_panel_items,
    },
    Spec {
        name: "group_panel_items",
        help: tr_const!("Groups items into an assembly that is selected and moved together; without a name the items leave their assemblies"),
        params: &[param("enclosure", Kind::Str, ENCLOSURE), param("ids", Kind::Ids, IDS), optional("name", Kind::Str, tr_const!("Name of the assembly"))],
        run: group_panel_items,
    },
    Spec {
        name: "list_panel_macros",
        help: tr_const!("Lists the 3D macros: the project's own and the built-in ones"),
        params: &[],
        run: list_panel_macros,
    },
    Spec {
        name: "save_panel_macro",
        help: tr_const!("Saves items of an enclosure as a 3D macro of the project"),
        params: &[param("enclosure", Kind::Str, ENCLOSURE), param("ids", Kind::Ids, IDS), param("name", Kind::Str, tr_const!("Macro name"))],
        run: save_panel_macro,
    },
    Spec {
        name: "insert_panel_macro",
        help: tr_const!("Inserts a 3D macro as an assembly: anchored items at their place, the others relative to x / y on the face"),
        params: &[
            param("enclosure", Kind::Str, ENCLOSURE),
            param("macro", Kind::Str, tr_const!("Macro name")),
            optional("face", Kind::Str, FACE),
            optional("x", Kind::Num, tr_const!("Left edge on the face in mm")),
            optional("y", Kind::Num, tr_const!("Top edge on the face in mm")),
        ],
        run: insert_panel_macro,
    },
];

const ENCLOSURE: &str = tr_const!("Enclosure (location, e.g. +CAB1)");

const IDS: &str = tr_const!("Item ids");

const FACE: &str = tr_const!("plate, door, left, right, rear, roof or bottom");

const ANCHOR: &str = tr_const!("Anchor along the axis: free, start:<mm>, center:<mm>, end:<mm> or stretch:<mm>,<mm>");

fn face_arg(a: &Args, name: &str) -> Result<Option<Face>, String> {
    a.str(name).map(|f| Face::from_key(f).ok_or_else(|| tr_fmt!("Unknown face: {}", f))).transpose()
}

fn align_arg(a: &Args, name: &str) -> Result<Option<Align>, String> {
    a.str(name).map(|v| Align::parse(v).ok_or_else(|| tr_fmt!("Unknown anchor: {}", v))).transpose()
}

fn fastening_arg(a: &Args) -> Result<Option<Fastening>, String> {
    a.str("fastening").map(|v| Fastening::from_key(v).ok_or_else(|| tr_fmt!("Unknown fastening: {}", v))).transpose()
}

fn bool_arg(a: &Args, name: &str) -> Option<bool> {
    a.value(name).and_then(Value::as_bool)
}

fn item_json(prj: &Project, it: &PanelItem) -> Value {
    let part = prj.parts.iter().chain(openecraft_core::parts::catalog()).find(|p| p.number == it.part);
    json!({
        "id": it.id,
        "kind": if it.kind==panel::ItemKind::MountingPlate {"mounting_plate".into()} else {format!("{:?}", it.kind).to_lowercase()},
        "tag": it.tag,
        "terminal":it.terminal,
        "single_part":it.single_part,
        "owner_device":it.owner_device,"bom_excluded":it.bom_excluded,
        "mounting_height":prj.layout.enclosures.iter().find(|e|e.items.iter().any(|i|i.id==it.id)).and_then(|e|panel::mounting_height(e,it)),
        "part": it.part,
        "face": it.face.key(),
        "x": it.x, "y": it.y, "w": it.w, "h": it.h, "z": it.z, "d": it.d,
        "fastening": Fastening::resolve(it, part).key(),
        "anchor": [it.anchor.x.text(), it.anchor.y.text()],
        "group": it.group,
        "mounted_on":it.mounted_on,"mounting_explicit":it.mounting_explicit,"rail_mount_override":it.rail_mount_override,"rail_mount":it.rail_mount,"rail_vertical":it.rail_vertical,
    })
}

/// Line-up of an enclosure: neighbours and the cable passages to the right one.
fn lineup_json(prj: &Project, ei: usize) -> Value {
    let (left, right) = panel::neighbours(&prj.layout, ei);
    let name = |i: Option<usize>| i.map(|i| prj.layout.enclosures[i].name.clone());
    let pos = panel::positions(&prj.layout);
    let passages: Vec<Value> = panel::passages(&prj.layout, &pos, ei)
        .into_iter()
        .map(|x| json!({ "y": x.at[1] - pos[ei][1], "z": x.at[2] - pos[ei][2], "cutouts": [x.cutouts.0, x.cutouts.1] }))
        .collect();
    json!({ "right_of": name(left), "left_of": name(right), "position": pos[ei], "passages_right": passages })
}

fn enclosure_json(prj: &Project, e: &Enclosure) -> Value {
    let components: Vec<Value> = panel::components(e).into_iter().map(|c| json!({ "role": c.role.title(), "part": c.part, "count": c.count })).collect();
    let faces: Vec<Value> = e
        .faces()
        .iter()
        .filter_map(|&f| {
            let items: Vec<Value> = e.items.iter().filter(|i| i.face == f).map(|i| item_json(prj, i)).collect();
            (!items.is_empty()).then(|| json!({ "face": e.face_key(f), "items": items }))
        })
        .collect();
    let ei = prj.layout.enclosures.iter().position(|x| std::ptr::eq(x, e)).unwrap_or(0);
    json!({
        "name": e.name,
        "kind":format!("{:?}",e.kind).to_lowercase(),
        "implicit_plate":!e.no_implicit_plate,
        "mounting_origin":e.mounting_origin,
        "mounting_frame":panel::face_frame(e,Face::Plate),
        "lineup": lineup_json(prj, ei),
        "system": e.build.system,
        "part": e.part,
        "device_tag":e.device_tag,
        "owner_device":e.owner_device,"bom_excluded":e.bom_excluded,
        "size": [e.width, e.height, e.depth],
        "plinth": e.build.plinth,
        "bayed": [e.build.left_open, e.build.right_open],
        "gland_plate": e.build.gland_plate,
        "components": components,
        "faces": faces,
        "assemblies": e.groups.iter().map(|g| json!({ "id": g.id, "name": g.name, "items": panel::group_members(e, g.id) })).collect::<Vec<_>>(),
        "problems": panel::problem_items(prj, e).into_iter().collect::<Vec<_>>(),
    })
}

fn list_enclosures(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let list = match a.str("enclosure") {
        Some(name) => vec![&prj.layout.enclosures[enclosure_index(prj, name)?]],
        None => prj.layout.enclosures.iter().collect(),
    };
    Ok(Value::Array(list.into_iter().map(|e| enclosure_json(prj, e)).collect()))
}

fn add_enclosure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim().to_string();
    if let Some(e) = prj.layout.enclosures.iter().find(|e| e.name == name) {
        return Err(if e.is_room() { tr_fmt!("{} is a room already", name) } else { tr_fmt!("Enclosure {} exists already", name) });
    }
    check_size(a)?;
    let mut e = panel::new_enclosure(prj, a.str("part").unwrap_or(""), &name);
    if let Some(system) = a.str("system") {
        if panel::SYSTEMS.iter().all(|x| x.id() != system) {
            return Err(tr_fmt!("Unknown enclosure system: {}", system));
        }
        let size = e.system().filter(|x| x.id() == system).map(|_| (e.width, e.height, e.depth));
        let size = size.or_else(|| panel::SYSTEMS.iter().find(|x| x.id() == system).and_then(|x| x.data.sizes.first().copied()));
        panel::apply_system(&mut e, system, size);
    }
    resize(&mut e, a);
    if a.value("mounting_origin").is_some() {e.mounting_origin=mounting_origin(a)?;}
    if let Some(implicit)=bool_arg(a,"implicit_plate") {e.no_implicit_plate = !implicit;}
    if a.flag("assembly") {e.kind=panel::EnclosureKind::Assembly;e.part.clear();e.build=Default::default();}
    if let Some(tag)=a.str("device_tag") {e.device_tag=tag.into();}
    if let Some(tag)=a.str("owner_device") {e.owner_device=tag.into();}
    if let Some(excluded)=bool_arg(a,"bom_excluded") {e.bom_excluded=excluded;}
    prj.layout.enclosures.push(e);
    Ok(enclosure_json(prj, prj.layout.enclosures.last().unwrap()))
}

/// Smallest width, height and depth of an enclosure (mm).
const MIN_ENCLOSURE: f32 = 50.0;

fn check_size(a: &Args) -> Result<(), String> {
    if ["width", "height", "depth"].iter().any(|n| a.num(n).is_some_and(|v| !(v >= MIN_ENCLOSURE && v.is_finite()))) {
        return Err(tr_fmt!("Width, height and depth of an enclosure are at least {} mm", MIN_ENCLOSURE));
    }
    Ok(())
}

fn resize(e: &mut Enclosure, a: &Args) {
    for (name, field) in [("width", &mut e.width), ("height", &mut e.height), ("depth", &mut e.depth)] {
        if let Some(v) = a.num(name) {
            *field = v;
        }
    }
    panel::sync_part(e);
    panel::apply_anchors(e);
}

fn set_enclosure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    check_size(a)?;
    let origin=mounting_origin(a)?;
    let old_part=prj.layout.enclosures[ei].part.clone();let old_tag=prj.layout.enclosures[ei].device_tag.clone();
    if a.num("plinth").is_some_and(|v| !(v >= 0.0 && v.is_finite())) {
        return Err(tr("The plinth height must not be negative").to_string());
    }
    let e = &mut prj.layout.enclosures[ei];
    if bool_arg(a,"assembly")==Some(true) && e.items.iter().any(|i|i.face!=Face::Plate) {return Err(tr("Open assemblies support only the mounting face").into());}
    if let Some(system) = a.str("system") {
        if !system.is_empty() && panel::SYSTEMS.iter().all(|x| x.id() != system) {
            return Err(tr_fmt!("Unknown enclosure system: {}", system));
        }
        panel::apply_system(e, system, None);
        if e.is_assembly() && !a.flag("assembly") {e.kind=panel::EnclosureKind::Cabinet;}
    }
    if let Some(value)=bool_arg(a,"assembly") {
        if value {e.kind=panel::EnclosureKind::Assembly;}
        else if e.is_assembly() {e.kind=panel::EnclosureKind::Cabinet;}
    }
    if let Some(implicit)=bool_arg(a,"implicit_plate") {e.no_implicit_plate = !implicit;}
    if a.value("mounting_origin").is_some() {e.mounting_origin=origin;}
    if let Some(v) = a.num("plinth") {
        e.build.plinth = v;
    }
    for (name, field) in [("left_open", &mut e.build.left_open), ("right_open", &mut e.build.right_open), ("gland_plate", &mut e.build.gland_plate)] {
        if let Some(v) = bool_arg(a, name) {
            *field = v;
        }
    }
    resize(e, a);
    if let Some(part)=a.str("part") {e.part=part.into();}
    if let Some(tag)=a.str("device_tag") {e.device_tag=tag.into();}
    if let Some(tag)=a.str("owner_device") {e.owner_device=tag.into();}
    if let Some(excluded)=bool_arg(a,"bom_excluded") {e.bom_excluded=excluded;}
    let new_part=e.part.clone();let new_tag=e.device_tag.clone();
    openecraft_core::parts::replace_device_part(prj,if new_tag==old_tag {&old_tag} else {&new_tag},&old_part,&new_part);
    Ok(enclosure_json(prj, &prj.layout.enclosures[ei]))
}

fn mounting_origin(a:&Args)->Result<Option<[f32;3]>,String> {
    let Some(value)=a.value("mounting_origin") else {return Ok(None);};
    if value.as_object().is_some_and(|o|o.is_empty()) {return Ok(None);}
    let coordinates=["x","y","z"].map(|key|value.get(key).and_then(Value::as_f64).map(|v|v as f32));
    match coordinates {
        [Some(x),Some(y),Some(z)] if [x,y,z].iter().all(|v|v.is_finite())=>Ok(Some([x,y,z])),
        _=>Err(tr("Mounting origin must contain finite x, y and z coordinates").into()),
    }
}

fn bay_enclosure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    match a.str("right_of").filter(|n| !n.trim().is_empty()) {
        Some(left) => {
            let li = enclosure_index(prj, left.trim())?;
            if let Some(room) = [ei, li].into_iter().find(|&i| prj.layout.enclosures[i].is_room()) {
                return Err(tr_fmt!("{} is a room and cannot be bayed", prj.layout.enclosures[room].name));
            }
            if !panel::bay_right_of(prj, ei, li) {
                return Err(tr_fmt!("{} cannot be bayed to itself", prj.layout.enclosures[ei].name));
            }
        }
        None => panel::unbay(prj, ei),
    }
    let lineup: Vec<&str> = panel::lineup_of(&prj.layout, ei).into_iter().map(|i| prj.layout.enclosures[i].name.as_str()).collect();
    Ok(json!({ "lineup": lineup }))
}

fn add_cable_passage(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let Some(right) = panel::neighbours(&prj.layout, ei).1 else {
        return Err(tr_fmt!("{} has no enclosure bayed to its right side", prj.layout.enclosures[ei].name));
    };
    let (y, z) = panel::next_passage_place(&prj.layout, ei);
    let (y, z) = (a.num("y").unwrap_or(y), a.num("z").unwrap_or(z));
    let ids = panel::add_passage(prj, ei, y, z, num_in(a, "diameter", 1.0, NO_MAX)?.unwrap_or(panel::PASSAGE_DIAMETER));
    let (left_name, right_name) = (prj.layout.enclosures[ei].name.clone(), prj.layout.enclosures[right].name.clone());
    Ok(json!({ "cutouts": ids, "from": left_name, "to": right_name, "lineup": lineup_json(prj, ei) }))
}

fn place_accessory(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let face = face_arg(a, "face")?.unwrap_or_default();
    let part = a.req("part").to_string();
    let known = prj.parts.iter().chain(openecraft_core::parts::catalog()).find(|p| p.number == part).cloned();
    let pd = known.ok_or_else(|| tr_fmt!("Unknown part: {}", part))?;
    if !panel::allowed_faces(&pd).contains(&face) {
        return Err(tr_fmt!("{} cannot be mounted on the {}", part, face.title()));
    }
    let mut it = panel::new_accessory(prj, &part, face, p(a.num("x").unwrap_or(0.0), a.num("y").unwrap_or(0.0)));
    it.fastening = fastening_arg(a)?.unwrap_or_default();
    it.anchor = Anchor { x: align_arg(a, "anchor_x")?.unwrap_or_default(), y: align_arg(a, "anchor_y")?.unwrap_or_default() };
    let e = &mut prj.layout.enclosures[ei];
    panel::apply_one_anchor(e, &mut it);
    let id = it.id;
    e.items.push(it);
    Ok(item_json(prj, prj.layout.enclosures[ei].item(id).unwrap()))
}

fn place_mounting_plate(prj:&mut Project,a:&Args)->Result<Value,String> {
    let ei=enclosure_index(prj,a.req("enclosure"))?;
    let w=num_in(a,"width",1.,NO_MAX)?.unwrap_or(100.);
    let h=num_in(a,"height",1.,NO_MAX)?.unwrap_or(100.);
    let d=num_in(a,"thickness",0.,NO_MAX)?.unwrap_or(2.);
    let item=PanelItem {id:prj.new_id(),kind:panel::ItemKind::MountingPlate,part:a.str("part").unwrap_or("").into(),
        x:a.num("x").unwrap_or(0.),y:a.num("y").unwrap_or(0.),z:a.num("z").unwrap_or(0.),w,h,d,..Default::default()};
    prj.layout.enclosures[ei].items.push(item);
    Ok(item_json(prj,prj.layout.enclosures[ei].items.last().unwrap()))
}

fn place_panel_device(prj:&mut Project,a:&Args)->Result<Value,String> {
    let ei=enclosure_index(prj,a.req("enclosure"))?;
    let face=face_arg(a,"face")?.unwrap_or_default();
    if !prj.layout.enclosures[ei].faces().contains(&face) {return Err(tr("Open assemblies support only the mounting face").into());}
    let item=PanelItem {id:prj.new_id(),tag:a.req("tag").into(),terminal:a.str("terminal").unwrap_or("").into(),part:a.str("part").unwrap_or("").into(),
        kind:panel::ItemKind::Device,face,x:a.num("x").unwrap_or(0.),y:a.num("y").unwrap_or(0.),z:a.num("z").unwrap_or(0.),
        w:num_in(a,"width",1e-3,NO_MAX)?.unwrap_or(1.),h:num_in(a,"height",1e-3,NO_MAX)?.unwrap_or(1.),d:num_in(a,"depth",0.,NO_MAX)?.unwrap_or(0.),
        mounting_explicit:true,single_part:a.flag("single_part"),..Default::default()};
    prj.layout.enclosures[ei].items.push(item);
    Ok(item_json(prj,prj.layout.enclosures[ei].items.last().unwrap()))
}

fn get_panel_points(prj:&mut Project,a:&Args)->Result<Value,String> {
    let selected=a.str("enclosure").map(|name|enclosure_index(prj,name)).transpose()?;
    let id=a.int("id").map(|id|id as u64);
    let mut out=Vec::new();
    for (_,e) in prj.layout.enclosures.iter().enumerate().filter(|(ei,_)|selected.is_none_or(|i|i==*ei)) {
        for it in e.items.iter().filter(|it|id.is_none_or(|id|it.id==id)) {
            out.push(json!({"enclosure":e.name,"id":it.id,"tag":it.tag,"terminal":it.terminal,"single_part":it.single_part,
                "points":panel::device_points(prj,it).into_iter().map(|p|json!({"pin":p.pin,"terminal":p.terminal,"pos":p.pos,"exit":p.exit,"exact":p.exact})).collect::<Vec<_>>()}));
        }
    }
    Ok(json!(out))
}

fn set_panel_item(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let id = a.int("id").unwrap_or(0) as u64;
    let (face, fastening, ax, ay) = (face_arg(a, "face")?, fastening_arg(a)?, align_arg(a, "anchor_x")?, align_arg(a, "anchor_y")?);
    let e = &mut prj.layout.enclosures[ei];
    let idx = e.items.iter().position(|i| i.id == id).ok_or_else(|| tr_fmt!("No item {}", id))?;
    let mut it = e.items[idx].clone();
    let old_part=it.part.clone();let tag=it.tag.clone();
    if let Some(part)=a.str("part") {it.part=part.into();}
    let before=e.items.clone();
    if let Some(terminal)=a.str("terminal") {it.terminal=terminal.into();}
    if let Some(single)=bool_arg(a,"single_part") {it.single_part=single;}
    if let Some(tag)=a.str("owner_device") {it.owner_device=tag.into();}
    if let Some(excluded)=bool_arg(a,"bom_excluded") {it.bom_excluded=excluded;}
    if let Some(parent)=a.int("mounted_on") {
        if parent<0 || !panel::can_mount(e,id,parent as u64) {return Err(tr("Invalid mounting parent").into());}
        it.mounted_on=parent as u64;
        it.mounting_explicit=true;
    }
    if let Some(value)=bool_arg(a,"rail_mount_override") {it.rail_mount_override=value;}
    if let Some(value)=bool_arg(a,"rail_vertical") {it.rail_vertical=value;}
    if let Some(value)=a.num("rail_offset") {it.rail_mount.center_offset=value;it.rail_mount_override=true;}
    if let Some(value)=a.num("clip_on") {it.rail_mount.clip_on=value;it.rail_mount_override=true;}
    let moved = a.num("x").is_some() || a.num("y").is_some();
    for (name, field) in [("x", &mut it.x), ("y", &mut it.y), ("z", &mut it.z)] {
        if let Some(v) = a.num(name) {
            *field = v;
        }
    }
    for (name,field) in [("width",&mut it.w),("height",&mut it.h),("depth",&mut it.d)] {
        if let Some(value)=num_in(a,name,0.,NO_MAX)? {*field=value;}
    }
    if let Some(f) = face {
        it.face = f;
    }
    if let Some(f) = fastening {
        it.fastening = f;
    }
    if moved && ax.is_none() && ay.is_none() {
        it.anchor = it.anchor.capture(&it, e.face_size(it.face));
    }
    it.anchor = Anchor { x: ax.unwrap_or(it.anchor.x), y: ay.unwrap_or(it.anchor.y) };
    panel::apply_one_anchor(e, &mut it);
    if a.int("mounted_on").is_none() && (moved || a.num("z").is_some() || face.is_some() || ax.is_some() || ay.is_some()) {panel::snap_to_rail(e, &mut it);}
    e.items[idx] = it;
    panel::carry_mounts(e,&before,&[id]);
    let part=e.items[idx].part.clone();
    openecraft_core::parts::replace_device_part(prj,&tag,&old_part,&part);
    Ok(item_json(prj, &prj.layout.enclosures[ei].items[idx]))
}

fn arrange_panel_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let ids = a.ids("ids");
    let action = a.req("action").to_string();
    let mut created = Vec::new();
    let e = &mut prj.layout.enclosures[ei];
    match action.as_str() {
        "distribute_x" => panel::distribute(e, &ids, true),
        "distribute_y" => panel::distribute(e, &ids, false),
        "rail" => {
            let rail = a.int("rail").unwrap_or(0) as u64;
            if e.item(rail).is_none() {
                return Err(tr_fmt!("No item {}", rail));
            }
            if !panel::arrange_on_rail(e, rail, &ids, a.num("gap").unwrap_or(0.0)) {
                return Err(tr_fmt!("The devices do not fit onto rail {}", rail));
            }
        }
        "pattern" => {
            let (nx, ny) = (int_in(a, "columns", 1, 100)?.unwrap_or(1) as usize, int_in(a, "rows", 1, 100)?.unwrap_or(1) as usize);
            created = panel::pattern_copy(prj, ei, &ids, nx, ny, a.num("pitch_x").unwrap_or(0.0), a.num("pitch_y").unwrap_or(0.0));
        }
        other => {
            let to = AlignTo::ALL.into_iter().find(|t| t.key() == other).ok_or_else(|| tr_fmt!("Unknown action: {}", other))?;
            panel::align(e, &ids, to);
        }
    }
    let e = &prj.layout.enclosures[ei];
    let items: Vec<Value> = e.items.iter().filter(|i| ids.contains(&i.id) || created.contains(&i.id)).map(|i| item_json(prj, i)).collect();
    Ok(json!({ "items": items, "created": created }))
}

fn group_panel_items(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let ids = a.ids("ids");
    let group = match a.str("name").filter(|n| !n.trim().is_empty()) {
        Some(name) => {
            let id = prj.new_id();
            prj.layout.enclosures[ei].groups.push(ItemGroup { id, name: name.trim().into(), source: String::new() });
            id
        }
        None => 0,
    };
    let e = &mut prj.layout.enclosures[ei];
    for it in e.items.iter_mut().filter(|i| ids.contains(&i.id)) {
        it.group = group;
    }
    let used: Vec<u64> = e.items.iter().map(|i| i.group).collect();
    e.groups.retain(|g| used.contains(&g.id));
    Ok(json!({ "group": group, "items": panel::group_members(e, group) }))
}

fn list_panel_macros(prj: &mut Project, _: &Args) -> Result<Value, String> {
    Ok(Value::Array(
        panel::all_macros(prj)
            .into_iter()
            .map(|m| {
                let faces: Vec<&str> = Face::ALL.into_iter().filter(|f| m.items.iter().any(|i| i.face == *f)).map(|f| f.key()).collect();
                json!({ "name": m.name, "title": m.title(), "built_in": m.is_builtin(), "description": m.description, "face": m.face.key(), "faces": faces, "items": m.items.len() })
            })
            .collect(),
    ))
}

fn save_panel_macro(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let name = a.req("name").trim().to_string();
    let m = panel::macro_from_items(&prj.layout.enclosures[ei], &a.ids("ids"), &name).ok_or_else(|| tr_fmt!("No items to save in {}", name))?;
    let n = m.items.len();
    prj.layout.macros.retain(|x| x.name != name);
    prj.layout.macros.push(m);
    Ok(json!({ "name": name, "items": n }))
}

fn insert_panel_macro(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ei = enclosure_index(prj, a.req("enclosure"))?;
    let name = a.req("macro");
    let m = panel::all_macros(prj).into_iter().find(|m| m.name == name || m.title() == name).ok_or_else(|| tr_fmt!("No macro named {}", name))?;
    let face = face_arg(a, "face")?.unwrap_or(m.face);
    let ids = panel::insert_macro(prj, ei, &m, face, p(a.num("x").unwrap_or(0.0), a.num("y").unwrap_or(0.0)));
    let e = &prj.layout.enclosures[ei];
    Ok(json!({ "items": ids.iter().filter_map(|id| e.item(*id)).map(|i| item_json(prj, i)).collect::<Vec<_>>() }))
}

#[cfg(test)]
mod tests;
