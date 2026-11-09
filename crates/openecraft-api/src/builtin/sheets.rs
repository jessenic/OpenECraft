//! Sheet commands: plot frames (paper format, orientation, columns / rows), page
//! templates and document pages with embedded files.

use crate::builtin::common::*;
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::frame::{self, Labels, Paper, PlotFrame};
use openecraft_core::model::Project;
use openecraft_core::page_templates;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Sheets;

impl Plugin for Sheets {
    fn info(&self) -> Info {
        info("sheets", tr_const!("Sheets"), tr_const!("Plot frames and page templates of the project"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_frames",
        help: tr_const!("Lists the plot frames (paper format, orientation, columns and rows) and the project default"),
        params: &[],
        run: list_frames,
    },
    Spec {
        name: "set_frame",
        help: tr_const!("Creates or changes a plot frame of the project (built-in frames are copied under a new name)"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the frame")),
            optional("based_on", Kind::Str, tr_const!("Frame to start from when the frame is new (default: A3 landscape)")),
            optional("paper", Kind::Str, tr_const!("A0, A1, A2, A3, A4, Letter, Legal, Tabloid or custom")),
            optional("landscape", Kind::Bool, tr_const!("Landscape (true) or portrait (false)")),
            optional("width", Kind::Num, tr_const!("Paper width in mm (custom paper)")),
            optional("height", Kind::Num, tr_const!("Paper height in mm (custom paper)")),
            optional("margin", Kind::Num, tr_const!("Distance of the border from the paper edge in mm")),
            optional("columns", Kind::Int, tr_const!("Number of columns")),
            optional("rows", Kind::Int, tr_const!("Number of rows")),
            optional("column_labels", Kind::Str, tr_const!("numbers or letters")),
            optional("row_labels", Kind::Str, tr_const!("numbers or letters")),
            optional("strips", Kind::Bool, tr_const!("Column and row strips")),
        ],
        run: set_frame,
    },
    Spec {
        name: "delete_frame",
        help: tr_const!("Deletes a plot frame of the project that no page uses"),
        params: &[param("name", Kind::Str, tr_const!("Name of the frame"))],
        run: delete_frame,
    },
    Spec {
        name: "set_default_frame",
        help: tr_const!("Sets the plot frame of all pages that do not choose one"),
        params: &[param("name", Kind::Str, tr_const!("Name of the frame"))],
        run: set_default_frame,
    },
    Spec {
        name: "list_page_templates",
        help: tr_const!("Lists the page templates (built-in and the project's own)"),
        params: &[],
        run: list_page_templates,
    },
    Spec {
        name: "save_page_template",
        help: tr_const!("Saves a page (type, plot frame, title block, structure, description, content) as a page template"),
        params: &[PAGE, param("name", Kind::Str, tr_const!("Name of the template"))],
        run: save_page_template,
    },
    Spec {
        name: "delete_page_template",
        help: tr_const!("Deletes a page template of the project"),
        params: &[param("name", Kind::Str, tr_const!("Name of the template"))],
        run: delete_page_template,
    },
];

fn frame_json(f: &PlotFrame) -> Value {
    let (w, h) = f.size();
    json!({
        "name": f.name,
        "builtin": frame::is_builtin(&f.name),
        "paper": f.paper.name(),
        "landscape": w >= h,
        "width": w,
        "height": h,
        "margin": f.margin,
        "columns": f.columns,
        "rows": f.rows,
        "column_labels": snake(f.column_labels),
        "row_labels": snake(f.row_labels),
        "strips": f.strips,
    })
}

fn labels(s: &str) -> Result<Labels, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "numbers" => Ok(Labels::Numbers),
        "letters" => Ok(Labels::Letters),
        _ => Err(tr_fmt!("Unknown labels: {}", s)),
    }
}

fn list_frames(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let frames: Vec<Value> = frame::names(prj).iter().map(|n| frame_json(&frame::find(prj, n))).collect();
    Ok(json!({ "default": frame::project_default(prj).name, "frames": frames }))
}

fn set_frame(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim();
    if name.is_empty() {
        return Err(tr("The name must not be empty").into());
    }
    if frame::is_builtin(name) {
        return Err(tr_fmt!("{} is a built-in frame; give the frame a new name", name));
    }
    let mut f = match prj.forms.frames.iter().find(|f| f.name == name) {
        Some(f) => f.clone(),
        None => frame::find(prj, a.str("based_on").unwrap_or(frame::DEFAULT)),
    };
    f.name = name.into();
    if let Some(paper) = a.str("paper") {
        f.paper = Paper::from_name(paper).ok_or_else(|| tr_fmt!("Unknown paper format: {}", paper))?;
    }
    if let Some(l) = a.value("landscape").and_then(Value::as_bool) {
        f.landscape = l;
    }
    num_in(a, "width", 50.0, 5000.0)?;
    num_in(a, "height", 50.0, 5000.0)?;
    if a.num("width").is_some() || a.num("height").is_some() {
        let (w, h) = f.size();
        f.custom = [a.num("width").unwrap_or(w), a.num("height").unwrap_or(h)];
        f.paper = Paper::Custom;
    }
    if let Some(m) = num_in(a, "margin", 0.0, 50.0)? {
        f.margin = m;
    }
    if let Some(c) = int_in(a, "columns", 1, 60)? {
        f.columns = c as usize;
    }
    if let Some(r) = int_in(a, "rows", 1, 40)? {
        f.rows = r as usize;
    }
    if let Some(l) = a.str("column_labels") {
        f.column_labels = labels(l)?;
    }
    if let Some(l) = a.str("row_labels") {
        f.row_labels = labels(l)?;
    }
    if let Some(st) = a.value("strips").and_then(Value::as_bool) {
        f.strips = st;
    }
    let v = frame_json(&f);
    frame::set(prj, f);
    Ok(v)
}

fn delete_frame(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    let Some(i) = prj.forms.frames.iter().position(|f| f.name == name) else {
        return Err(tr_fmt!("No plot frame named {}", name));
    };
    if prj.pages.iter().any(|p| p.frame == name) || prj.forms.default_frame == name {
        return Err(tr_fmt!("The plot frame {} is in use", name));
    }
    prj.forms.frames.remove(i);
    Ok(json!({ "deleted": name }))
}

fn set_default_frame(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    if !frame::names(prj).iter().any(|n| n == name) {
        return Err(tr_fmt!("No plot frame named {}", name));
    }
    prj.forms.default_frame = name.into();
    Ok(frame_json(&frame::project_default(prj)))
}

fn list_page_templates(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let list = page_templates::all(prj).into_iter().map(|t| {
        json!({
            "name": t.name,
            "builtin": !prj.page_templates.iter().any(|x| x.name == t.name),
            "kind": kind_name(t.kind),
            "frame": t.frame,
            "title_block": t.title_block,
            "description": t.description,
            "structure": t.structure.structure(),
            "values": t.values,
            "items": t.content.symbols.len() + t.content.wires.len() + t.content.texts.len() + t.content.shapes.len(),
        })
    });
    Ok(Value::Array(list.collect()))
}

fn save_page_template(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let i = page_index(prj, a.req("page"))?;
    let name = a.req("name").trim();
    if name.is_empty() {
        return Err(tr("The name must not be empty").into());
    }
    let t = page_templates::from_page(prj, i, name);
    let items = t.content.symbols.len() + t.content.wires.len() + t.content.texts.len() + t.content.shapes.len();
    page_templates::save(prj, t);
    Ok(json!({ "name": name, "items": items }))
}

fn delete_page_template(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    if !page_templates::remove(prj, name) {
        return Err(tr_fmt!("No page template named {}", name));
    }
    Ok(json!({ "deleted": name }))
}
