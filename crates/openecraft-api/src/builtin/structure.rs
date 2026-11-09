//! Structure commands (IEC 81346): structure management with the defined identifiers,
//! project-wide renaming, templates, work areas and structure boxes.

use crate::builtin::common::{editable, page_index, xy, PAGE};
use crate::params::{optional, param, Args, Kind, Param};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::geom::{p, P};
use openecraft_core::model::Project;
use openecraft_core::structure::{self, Aspect, BoxCorner, BoxLine, Dt, StructureBox, WorkArea};
use openecraft_core::structure_boxes;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Structure;

impl Plugin for Structure {
    fn info(&self) -> Info {
        info("structure", tr_const!("Structure"), tr_const!("Structure identifiers, aspects and work areas"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_structure",
        help: tr_const!("Lists the structure management: aspects of the pages, defined and used identifiers, work areas"),
        params: &[],
        run: list_structure,
    },
    Spec {
        name: "define_structure",
        help: tr_const!("Defines a structure identifier or changes its description"),
        params: &[ASPECT, param("id", Kind::Str, tr_const!("Identifier, e.g. A1 or CAB1")), optional("description", Kind::Str, tr_const!("Description"))],
        run: define_structure,
    },
    Spec {
        name: "remove_structure",
        help: tr_const!("Removes the definition of a structure identifier (pages and devices keep it)"),
        params: &[ASPECT, param("id", Kind::Str, tr_const!("Identifier"))],
        run: remove_structure,
    },
    Spec {
        name: "rename_structure",
        help: tr_const!("Renames a structure identifier in the whole project: pages, devices, cables, definition and work areas"),
        params: &[
            ASPECT,
            param("id", Kind::Str, tr_const!("Identifier")),
            param("new_id", Kind::Str, tr_const!("New identifier")),
            optional("merge", Kind::Bool, tr_const!("Allow renaming onto an identifier in use: both become one")),
        ],
        run: rename_structure,
    },
    Spec {
        name: "tidy_structure",
        help: tr_const!("Defines all identifiers in use and / or removes the definitions nobody uses"),
        params: &[
            optional("define_used", Kind::Bool, tr_const!("Define the identifiers in use (default: true)")),
            optional("remove_unused", Kind::Bool, tr_const!("Remove unused definitions (default: false)")),
        ],
        run: tidy_structure,
    },
    Spec {
        name: "set_structure_aspects",
        help: tr_const!("Sets the aspects the pages use"),
        params: &[param("aspects", Kind::Str, tr_const!("Aspects separated by commas, e.g. function,location"))],
        run: set_aspects,
    },
    Spec {
        name: "set_work_area",
        help: tr_const!("Creates or changes a work area: only the pages and devices of its structure are shown"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the work area")),
            optional("function", Kind::Str, tr_const!("Function filter (= A1, A*; empty = any)")),
            optional("installation", Kind::Str, tr_const!("Installation site filter (++)")),
            optional("location", Kind::Str, tr_const!("Location filter (+)")),
            optional("product", Kind::Str, tr_const!("Product filter (-)")),
        ],
        run: set_work_area,
    },
    Spec {
        name: "delete_work_area",
        help: tr_const!("Deletes a work area"),
        params: &[param("name", Kind::Str, tr_const!("Name of the work area"))],
        run: delete_work_area,
    },
    Spec {
        name: "list_structure_boxes",
        help: tr_const!("Lists the structure boxes of a page (or of all pages) with the devices whose main function lies in them"),
        params: &[optional("page", Kind::Str, tr_const!("Page name; empty = all pages"))],
        run: list_structure_boxes,
    },
    Spec {
        name: "add_structure_box",
        help: tr_const!("Draws a structure box: the devices whose main function lies in it take its identifiers instead of the page's"),
        params: &[
            PAGE,
            param("x1", Kind::Num, tr_const!("X of one corner in mm")),
            param("y1", Kind::Num, tr_const!("Y of one corner in mm (downwards)")),
            param("x2", Kind::Num, tr_const!("X of the opposite corner in mm")),
            param("y2", Kind::Num, tr_const!("Y of the opposite corner in mm")),
            BOX_FUNCTION,
            BOX_INSTALLATION,
            BOX_LOCATION,
            BOX_PRODUCT,
            optional("description", Kind::Str, tr_const!("Description")),
            BOX_LABEL,
            BOX_LINE,
        ],
        run: add_structure_box,
    },
    Spec {
        name: "update_structure_box",
        help: tr_const!("Changes a structure box (only the given properties); the devices in it follow"),
        params: &[
            PAGE,
            param("id", Kind::Int, tr_const!("Id of the structure box")),
            optional("x1", Kind::Num, tr_const!("X of one corner in mm")),
            optional("y1", Kind::Num, tr_const!("Y of one corner in mm (downwards)")),
            optional("x2", Kind::Num, tr_const!("X of the opposite corner in mm")),
            optional("y2", Kind::Num, tr_const!("Y of the opposite corner in mm")),
            BOX_FUNCTION,
            BOX_INSTALLATION,
            BOX_LOCATION,
            BOX_PRODUCT,
            optional("description", Kind::Str, tr_const!("Description")),
            BOX_LABEL,
            BOX_LINE,
        ],
        run: update_structure_box,
    },
];

const BOX_FUNCTION: Param = optional("function", Kind::Str, tr_const!("Function (=) the box sets; empty = inherited"));
const BOX_INSTALLATION: Param = optional("installation", Kind::Str, tr_const!("Installation site (++) the box sets; empty = inherited"));
const BOX_LOCATION: Param = optional("location", Kind::Str, tr_const!("Location (+) the box sets; empty = inherited"));
const BOX_PRODUCT: Param = optional("product", Kind::Str, tr_const!("Product (-) the box sets; empty = inherited"));
const BOX_LABEL: Param = optional("label", Kind::Str, tr_const!("Corner of the identifiers: top_left, top_right, bottom_left or bottom_right"));
const BOX_LINE: Param = optional("line", Kind::Str, tr_const!("Outline: dash_dot, dashed or solid"));

const ASPECT: super::Param = param("aspect", Kind::Str, tr_const!("function (=), installation (++), location (+) or product (-)"));

fn aspect(a: &Args) -> Result<Aspect, String> {
    let s = a.req("aspect");
    Aspect::from_key(s).ok_or_else(|| tr_fmt!("Unknown aspect: {}", s))
}

fn work_area_json(w: &WorkArea) -> Value {
    json!({ "name": w.name, "filter": w.filter.structure() })
}

fn list_structure(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let rows: Vec<Value> = structure::overview(prj)
        .into_iter()
        .map(|u| {
            json!({
                "aspect": u.aspect.key(),
                "id": u.id,
                "identifier": u.aspect.format(&u.id),
                "description": u.description,
                "defined": u.defined,
                "pages": u.pages,
                "devices": u.devices,
            })
        })
        .collect();
    let aspects: Vec<&str> = prj.structures.page_aspects().into_iter().map(Aspect::key).collect();
    Ok(json!({
        "aspects": aspects,
        "identifiers": rows,
        "work_areas": prj.structures.work_areas.iter().map(work_area_json).collect::<Vec<_>>(),
    }))
}

fn define_structure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let aspect = aspect(a)?;
    let id = structure::check_identifier(aspect, a.req("id"))?;
    if id.is_empty() {
        return Err(tr("The identifier must not be empty").into());
    }
    let description = a.str("description").map(String::from).or_else(|| prj.structures.entry(aspect, &id).map(|e| e.description.clone())).unwrap_or_default();
    prj.structures.define(aspect, &id, &description);
    Ok(json!({ "identifier": aspect.format(&id), "description": description }))
}

fn remove_structure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let aspect = aspect(a)?;
    let id = structure::clean_aspect(a.req("id"));
    if !prj.structures.remove(aspect, &id) {
        return Err(tr_fmt!("{} is not defined", aspect.format(&id)));
    }
    Ok(json!({ "removed": aspect.format(&id) }))
}

fn rename_structure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let aspect = aspect(a)?;
    let (old, new) = (structure::check_identifier(aspect, a.req("id"))?, structure::check_identifier(aspect, a.req("new_id"))?);
    let known = structure::overview(prj);
    let exists = |id: &str| known.iter().any(|u| u.aspect == aspect && u.id == id);
    if !old.is_empty() && !exists(&old) {
        return Err(tr_fmt!("No identifier {} in the project", aspect.format(&old)));
    }
    if new != old && exists(&new) && !a.flag("merge") {
        return Err(tr_fmt!("{} exists already: pass merge to make both one identifier", aspect.format(&new)));
    }
    let n = structure::rename(prj, aspect, &old, &new)?;
    Ok(json!({ "renamed": n, "identifier": aspect.format(&new) }))
}

fn tidy_structure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let defined = if a.value("define_used").and_then(Value::as_bool).unwrap_or(true) { structure::define_used(prj) } else { 0 };
    let removed = if a.flag("remove_unused") { structure::remove_unused(prj) } else { 0 };
    Ok(json!({ "defined": defined, "removed": removed }))
}

fn set_aspects(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let mut aspects = Vec::new();
    for key in crate::builtin::common::words(a.req("aspects")) {
        let aspect = Aspect::from_key(key).ok_or_else(|| tr_fmt!("Unknown aspect: {}", key))?;
        if !aspects.contains(&aspect) {
            aspects.push(aspect);
        }
    }
    prj.structures.aspects = aspects;
    let aspects: Vec<&str> = prj.structures.page_aspects().into_iter().map(Aspect::key).collect();
    Ok(json!({ "aspects": aspects }))
}

fn set_work_area(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim();
    if name.is_empty() {
        return Err(tr("The name must not be empty").into());
    }
    let mut area = prj.structures.work_area(name).cloned().unwrap_or(WorkArea { name: name.into(), filter: Dt::default() });
    for aspect in Aspect::ALL {
        if let Some(v) = a.str(aspect.key()) {
            *area.filter.get_mut(aspect) = structure::clean_aspect(v);
        }
    }
    let v = work_area_json(&area);
    prj.structures.set_work_area(area);
    Ok(v)
}

fn corner_key(c: BoxCorner) -> &'static str {
    match c {
        BoxCorner::TopLeft => "top_left",
        BoxCorner::TopRight => "top_right",
        BoxCorner::BottomLeft => "bottom_left",
        BoxCorner::BottomRight => "bottom_right",
    }
}

fn line_key(l: BoxLine) -> &'static str {
    match l {
        BoxLine::DashDot => "dash_dot",
        BoxLine::Dashed => "dashed",
        BoxLine::Solid => "solid",
    }
}

fn box_json(prj: &Project, page: usize, b: &StructureBox) -> Value {
    let symbols = prj.symbol_table();
    let pg = &prj.pages[page];
    // Devices whose main function lies in the box (the innermost box of its place).
    let devices: Vec<&str> = pg
        .symbols
        .iter()
        .filter(|s| !s.tag.is_empty() && structure::boxes_at(pg, s.pos).last().is_some_and(|i| i.id == b.id))
        .filter(|s| matches!(symbols.get(&s.sym).map(|d| d.func), Some(openecraft_core::symbols::Func::Main { .. })))
        .map(|s| s.tag.as_str())
        .collect();
    json!({
        "id": b.id,
        "page": pg.name,
        "from": xy(b.a),
        "to": xy(b.b),
        "identifiers": b.identifiers(),
        "structure": structure::box_scope(pg, b).structure(),
        "description": b.description,
        "label": corner_key(b.label),
        "line": line_key(b.line),
        "devices": devices,
    })
}

fn list_structure_boxes(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let pages: Vec<usize> = match a.str("page").filter(|s| !s.is_empty()) {
        Some(name) => vec![page_index(prj, name)?],
        None => (0..prj.pages.len()).collect(),
    };
    let prj = &*prj;
    let list: Vec<Value> = pages.into_iter().flat_map(|i| prj.pages[i].structure_boxes.iter().map(move |b| box_json(prj, i, b))).collect();
    Ok(Value::Array(list))
}

/// A corner given as `x<n>` / `y<n>`; `old` where it is not given.
fn corner(a: &Args, n: u8, old: P) -> P {
    let num = |name: String, old: f32| a.value(&name).and_then(Value::as_f64).map_or(old, |v| v as f32);
    p(num(format!("x{n}"), old.x), num(format!("y{n}"), old.y))
}

/// Applies the optional properties of a box.
fn apply_box(b: &mut StructureBox, a: &Args) -> Result<(), String> {
    for aspect in Aspect::ALL {
        if let Some(v) = a.str(aspect.key()) {
            *b.structure.get_mut(aspect) = structure::check_identifier(aspect, v)?;
        }
    }
    if let Some(d) = a.str("description") {
        b.description = d.to_string();
    }
    if let Some(l) = a.str("label") {
        b.label = [BoxCorner::TopLeft, BoxCorner::TopRight, BoxCorner::BottomLeft, BoxCorner::BottomRight]
            .into_iter()
            .find(|c| corner_key(*c) == l.trim())
            .ok_or_else(|| tr_fmt!("Unknown corner: {}", l))?;
    }
    if let Some(l) = a.str("line") {
        b.line = [BoxLine::DashDot, BoxLine::Dashed, BoxLine::Solid].into_iter().find(|x| line_key(*x) == l.trim()).ok_or_else(|| tr_fmt!("Unknown line style: {}", l))?;
    }
    let r = b.rect();
    if r.width() <= 0.0 || r.height() <= 0.0 {
        return Err(tr("A structure box needs a width and a height").into());
    }
    b.a = r.min;
    b.b = r.max;
    Ok(())
}

/// Lets the devices follow the changed box; the answer lists those that could not.
fn after_box_change(prj: &mut Project, before: &Project, page: usize, id: u64) -> Value {
    let kept = structure_boxes::follow(prj, before);
    let b = prj.pages[page].structure_boxes.iter().find(|b| b.id == id).cloned().unwrap_or_default();
    let mut v = box_json(prj, page, &b);
    if !kept.is_empty() {
        v["kept"] = json!(kept);
    }
    v
}

fn add_structure_box(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let before = prj.clone();
    let mut b = structure_boxes::new_box(prj, corner(a, 1, P::default()), corner(a, 2, P::default()), Dt::default(), "");
    apply_box(&mut b, a)?;
    let id = b.id;
    prj.pages[i].structure_boxes.push(b);
    Ok(after_box_change(prj, &before, i, id))
}

fn update_structure_box(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = editable(prj, a.req("page"))?;
    let id = a.int("id").unwrap_or_default() as u64;
    let before = prj.clone();
    let b = prj.pages[i].structure_boxes.iter_mut().find(|b| b.id == id).ok_or_else(|| tr_fmt!("No structure box with id {}", id))?;
    let (a1, b1) = (b.a, b.b);
    b.a = corner(a, 1, a1);
    b.b = corner(a, 2, b1);
    apply_box(b, a)?;
    Ok(after_box_change(prj, &before, i, id))
}

fn delete_work_area(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    let areas = &mut prj.structures.work_areas;
    let n = areas.len();
    areas.retain(|w| w.name != name);
    if areas.len() == n {
        return Err(tr_fmt!("No work area named {}", name));
    }
    Ok(json!({ "deleted": name }))
}
