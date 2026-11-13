//! Sheet commands: plot frames (paper format, orientation, columns / rows), title
//! blocks, page templates and document pages with embedded files.

use crate::builtin::common::*;
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::forms::{self, Form};
use openecraft_core::frame::{self, Labels, Paper, PlotFrame};
use openecraft_core::model::Project;
use openecraft_core::page_templates;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Sheets;

impl Plugin for Sheets {
    fn info(&self) -> Info {
        info("sheets", tr_const!("Sheets"), tr_const!("Plot frames, title blocks and page templates of the project"))
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
            optional("artwork", Kind::Object, tr_const!("Frame drawing primitives in paper millimetres in an items array")),
            optional("title_block", Kind::Str, tr_const!("Title block of the pages with this frame that do not choose one; empty: the project default")),
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
        name: "list_title_blocks",
        help: tr_const!("Lists the title blocks (built-in and the project's own) with their elements, the project default and the placeholders"),
        params: &[],
        run: list_title_blocks,
    },
    Spec {
        name: "set_title_block",
        help: tr_const!("Creates or changes a title block of the project (built-in ones are copied under a new name)"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the title block")),
            optional("based_on", Kind::Str, tr_const!("Title block to start from when it is new (default: Standard)")),
            optional("items", Kind::Object, tr_const!("Elements (Line, Rect, Text, Image) in mm from the top left corner of the title block area in an items array")),
        ],
        run: set_title_block,
    },
    Spec {
        name: "delete_title_block",
        help: tr_const!("Deletes a title block of the project; pages using it get the project default"),
        params: &[param("name", Kind::Str, tr_const!("Name of the title block"))],
        run: delete_title_block,
    },
    Spec {
        name: "set_default_title_block",
        help: tr_const!("Sets the title block of all pages that do not choose one, or of the pages of one type"),
        params: &[
            param("name", Kind::Str, tr_const!("Name of the title block (with page_type: empty removes the choice)")),
            optional("page_type", Kind::Str, tr_const!("Only for pages of this type: schematic, cover, report, mounting, overview, graphic or document")),
        ],
        run: set_default_title_block,
    },
    Spec {
        name: "set_cover_logo",
        help: tr_const!("Places the project logo on the cover page (mm from the top left corner of the drawing area); without parameters it shows the place"),
        params: &[
            optional("x", Kind::Num, tr_const!("Left edge in mm")),
            optional("y", Kind::Num, tr_const!("Top edge in mm")),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("height", Kind::Num, tr_const!("Height in mm; the logo keeps its aspect ratio inside")),
            optional("automatic", Kind::Bool, tr_const!("Back to the place above the project name")),
        ],
        run: set_cover_logo,
    },
    Spec {
        name: "set_stamp",
        help: tr_const!("Sets the stamp drawn across all pages by the state of the revision workflow (e.g. DRAFT); without parameters it shows the stamp"),
        params: &[
            optional("draft", Kind::Str, tr_const!("Text while a revision is a draft or nothing is released yet (placeholders allowed; empty: none)")),
            optional("review", Kind::Str, tr_const!("Text while a revision is in review or was rejected")),
            optional("approved", Kind::Str, tr_const!("Text when the revision is approved but not released")),
            optional("released", Kind::Str, tr_const!("Text when everything is released, e.g. AS BUILT")),
            optional("size", Kind::Num, tr_const!("Text height in mm (0: fitted to the page)")),
            optional("angle", Kind::Num, tr_const!("Angle in degrees, counter-clockwise")),
            optional("color", Kind::Str, tr_const!("Colour as #RRGGBB")),
        ],
        run: set_stamp,
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
        "artwork": f.artwork,
        "title_block": f.title_block,
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
    if let Some(t) = a.str("title_block") {
        if !t.is_empty() && !forms::names(prj).iter().any(|n| n == t) {
            return Err(tr_fmt!("No title block named {}", t));
        }
        f.title_block = t.into();
    }
    if let Some(artwork) = a.value("artwork") {
        f.artwork = serde_json::from_value(artwork.get("items").cloned().unwrap_or(Value::Null)).map_err(|e| tr_fmt!("Invalid frame artwork: {}", e))?;
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

fn title_block_json(prj: &Project, f: &Form) -> Value {
    let used = (0..prj.pages.len()).filter(|&i| forms::title_block(prj, i).name == f.name).count();
    json!({
        "name": f.name,
        "builtin": !prj.forms.title_blocks.iter().any(|x| x.name == f.name),
        "pages": used,
        "items": f.items,
    })
}

fn list_title_blocks(prj: &mut Project, _: &Args) -> Result<Value, String> {
    let blocks: Vec<Value> = forms::names(prj).iter().map(|n| title_block_json(prj, &forms::find(prj, n))).collect();
    let default = if prj.forms.default_title_block.is_empty() { forms::STANDARD } else { prj.forms.default_title_block.as_str() };
    let placeholders: Vec<Value> = forms::PLACEHOLDERS.iter().map(|(ph, meaning)| json!({ "placeholder": ph, "meaning": tr(meaning) })).collect();
    let page_types = &prj.forms.page_type_title_blocks;
    Ok(json!({ "default": default, "page_types": page_types, "area": [forms::AREA_W, forms::AREA_H], "title_blocks": blocks, "placeholders": placeholders }))
}

fn set_title_block(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim();
    if name.is_empty() {
        return Err(tr("The name must not be empty").into());
    }
    let own = prj.forms.title_blocks.iter().position(|f| f.name == name);
    if own.is_none() && forms::names(prj).iter().any(|n| n == name) {
        return Err(tr_fmt!("{} is a built-in title block; give the title block a new name", name));
    }
    let mut f = match own {
        Some(i) => prj.forms.title_blocks[i].clone(),
        None => {
            let base = a.str("based_on").unwrap_or(forms::STANDARD);
            if !forms::names(prj).iter().any(|n| n == base) {
                return Err(tr_fmt!("No title block named {}", base));
            }
            forms::find(prj, base).into_owned()
        }
    };
    f.name = name.into();
    if let Some(items) = a.value("items") {
        f.items = serde_json::from_value(items.get("items").cloned().unwrap_or(Value::Null)).map_err(|e| tr_fmt!("Invalid title block elements: {}", e))?;
    }
    let i = match own {
        Some(i) => {
            prj.forms.title_blocks[i] = f;
            i
        }
        None => {
            prj.forms.title_blocks.push(f);
            prj.forms.title_blocks.len() - 1
        }
    };
    Ok(title_block_json(prj, &prj.forms.title_blocks[i]))
}

fn delete_title_block(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name");
    let Some(i) = prj.forms.title_blocks.iter().position(|f| f.name == name) else {
        return Err(tr_fmt!("No title block of the project named {}", name));
    };
    prj.forms.title_blocks.remove(i);
    let mut pages = 0;
    for pg in prj.pages.iter_mut().filter(|pg| pg.title_block == name) {
        pg.title_block.clear();
        pages += 1;
    }
    if prj.forms.default_title_block == name {
        prj.forms.default_title_block.clear();
    }
    Ok(json!({ "deleted": name, "pages": pages }))
}

fn set_default_title_block(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let name = a.req("name").trim();
    let page_type = a.str("page_type").map(|t| t.trim().to_ascii_lowercase());
    if let Some(t) = &page_type {
        if !forms::PAGE_TYPES.iter().any(|(k, _)| k == t) {
            return Err(tr_fmt!("Unknown page type: {}", t));
        }
    }
    if (page_type.is_none() || !name.is_empty()) && !forms::names(prj).iter().any(|n| n == name) {
        return Err(tr_fmt!("No title block named {}", name));
    }
    match page_type {
        Some(t) if name.is_empty() => {
            prj.forms.page_type_title_blocks.remove(&t);
            return Ok(json!({ "page_type": t, "removed": true }));
        }
        Some(t) => {
            prj.forms.page_type_title_blocks.insert(t.clone(), name.into());
            let mut v = title_block_json(prj, &forms::find(prj, name));
            v["page_type"] = json!(t);
            return Ok(v);
        }
        None => prj.forms.default_title_block = if name == forms::STANDARD { String::new() } else { name.into() },
    }
    Ok(title_block_json(prj, &forms::find(prj, name)))
}

fn set_cover_logo(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let page = prj.pages.iter().position(|p| p.kind == openecraft_core::model::PageKind::Cover).unwrap_or(0);
    let frame = frame::of_page(prj, page);
    if a.flag("automatic") {
        prj.info.cover_logo = None;
    } else if ["x", "y", "width", "height"].iter().any(|k| a.num(k).is_some()) {
        let mut place = openecraft_core::render::cover_logo_place(prj, &frame);
        let area = frame.area();
        place.x = num_in(a, "x", -50.0, area.width())?.unwrap_or(place.x);
        place.y = num_in(a, "y", -50.0, area.height())?.unwrap_or(place.y);
        place.width = num_in(a, "width", 1.0, area.width())?.unwrap_or(place.width);
        place.height = num_in(a, "height", 1.0, area.height())?.unwrap_or(place.height);
        prj.info.cover_logo = Some(place);
    }
    let place = openecraft_core::render::cover_logo_place(prj, &frame);
    Ok(json!({
        "automatic": prj.info.cover_logo.is_none(),
        "x": place.x, "y": place.y, "width": place.width, "height": place.height,
        "logo": prj.info.logo.as_ref().map(|l| l.file.name.clone()),
    }))
}

fn hex_color(s: &str) -> Result<[u8; 3], String> {
    let h = s.trim().trim_start_matches('#');
    let byte = |i: usize| h.get(i..i + 2).and_then(|b| u8::from_str_radix(b, 16).ok());
    match (h.len(), byte(0), byte(2), byte(4)) {
        (6, Some(r), Some(g), Some(b)) => Ok([r, g, b]),
        _ => Err(tr_fmt!("Not a colour (#RRGGBB): {}", s)),
    }
}

fn set_stamp(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let color = a.str("color").map(hex_color).transpose()?;
    let size = num_in(a, "size", 0.0, 200.0)?;
    let s = &mut prj.forms.stamp;
    for (k, field) in [("draft", &mut s.draft), ("review", &mut s.review), ("approved", &mut s.approved), ("released", &mut s.released)] {
        if let Some(v) = a.str(k) {
            *field = v.into();
        }
    }
    if let Some(v) = size {
        s.size = v;
    }
    if let Some(v) = a.num("angle") {
        s.angle = v;
    }
    if let Some(c) = color {
        s.color = c;
    }
    let s = &prj.forms.stamp;
    let color = format!("#{:02X}{:02X}{:02X}", s.color[0], s.color[1], s.color[2]);
    Ok(json!({
        "draft": s.draft, "review": s.review, "approved": s.approved, "released": s.released,
        "size": s.size, "angle": s.angle, "color": color,
        "shown": forms::stamp_text(prj),
    }))
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
