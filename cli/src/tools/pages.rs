//! Page tools: list, show, add, change and delete pages.

use super::common::*;
use super::{optional, param, Args, Kind, Tool};
use crate::session::Session;
use openecraft_core::model::*;
use openecraft_core::{dwg, dxf_import, frame, ops, page_templates, pdf_comments, sections};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "add_page",
        help: tr_const!("Adds a page"),
        params: &[
            optional("kind", Kind::Str, tr_const!("schematic, overview, graphic, cover or a report: table_of_contents, device_list, bill_of_materials, terminal_diagram, terminal_lineup, terminal_strip_overview, terminal_connections, plug_diagram, pin_connections, connection_list, device_connections, plc_assignment, potential_list, panel_wiring, cable_overview, cable_diagram, cable_connections, cable_sizing, revision_overview, harness_data, duct_fill, panel_legend, climate, marking, hose_list, pct_loops, fluid_netlist, hose_configuration, tubing_list, fluid_device_groups, electro_fluid")),
            optional("template", Kind::Str, tr_const!("Page template (list_page_templates) instead of kind")),
            optional("frame", Kind::Str, tr_const!("Plot frame (list_frames); default: the project's")),
            optional("after", Kind::Str, tr_const!("Insert after this page (default: at the end)")),
            optional("name", Kind::Str, tr_const!("Page name (default: next number)")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("section", Kind::Str, tr_const!("Section of the page (list_sections; default: that of the page before)")),
        ],
        needs_project: true,
        run: add_page,
    },
    Tool {
        name: "import_dxf",
        help: tr_const!("Imports a DXF drawing as graphics (lines, arcs, texts, blocks) on a new schematic page"),
        params: &[
            param("path", Kind::Str, tr_const!("DXF file")),
            optional("after", Kind::Str, tr_const!("Insert after this page (default: at the end)")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("fit", Kind::Bool, tr_const!("Scale drawings down that do not fit on the page (default: true)")),
        ],
        needs_project: true,
        run: import_drawing,
    },
    Tool {
        name: "import_dwg",
        help: tr_const!("Imports a DWG drawing (AutoCAD R12 … 2018) as graphics (lines, arcs, texts, blocks) on a new schematic page"),
        params: &[
            param("path", Kind::Str, tr_const!("DWG file")),
            optional("after", Kind::Str, tr_const!("Insert after this page (default: at the end)")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("fit", Kind::Bool, tr_const!("Scale drawings down that do not fit on the page (default: true)")),
        ],
        needs_project: true,
        run: import_drawing,
    },
    Tool {
        name: "import_pdf_comments",
        help: tr_const!("Imports the comments a reviewer made in a PDF export of the project (notes, text boxes, highlights, shapes, replies) into the review layer; comments imported before are skipped"),
        params: &[
            param("path", Kind::Str, tr_const!("PDF file with comments")),
            optional("author", Kind::Str, tr_const!("Author of comments without one (default: the user name)")),
        ],
        needs_project: true,
        run: import_pdf_comments,
    },
];

fn import_pdf_comments(s: &mut Session, a: &Args) -> Result<Value, String> {
    let path = a.req("path");
    let bytes = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    let user = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
    let author = a.str("author").map(String::from).unwrap_or(user);
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let r = pdf_comments::import(s.project_mut()?, &bytes, &author, &openecraft_parts::date_from_unix(secs))?;
    Ok(json!({ "comments": r.comments, "replies": r.replies, "markups": r.markups, "skipped": r.skipped, "outside": r.outside }))
}

/// `import_dxf` and `import_dwg`: the format is told apart by the content.
fn import_drawing(s: &mut Session, a: &Args) -> Result<Value, String> {
    let path = a.req("path");
    let bytes = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    let prj = s.project_mut()?;
    let after = match a.str("after") {
        Some(n) => page_index(prj, n)?,
        None => prj.pages.len() - 1,
    };
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let description = a.str("description").map(String::from).unwrap_or_else(|| file.rsplit_once('.').map(|x| x.0).unwrap_or(file).to_string());
    let fit = a.0.get("fit").and_then(Value::as_bool).unwrap_or(true);
    let r = dwg::import_drawing(prj, &bytes, &description, after, dxf_import::DxfOptions { fit })?;
    let mut v = page_json(&prj.pages[r.page]);
    v["lines"] = json!(r.lines);
    v["texts"] = json!(r.texts);
    v["scale"] = json!(r.scale);
    v["source_bounds"] = json!({ "min": r.source_min, "max": r.source_max });
    v["page_origin"] = json!(r.page_origin);
    v["not_imported"] = json!(r.skipped);
    Ok(v)
}

fn add_page(s: &mut Session, a: &Args) -> Result<Value, String> {
    let report = match a.str("kind").map(|k| k.trim().to_ascii_lowercase()) {
        Some(k) if !matches!(k.as_str(), "schematic" | "overview" | "graphic" | "graphics" | "cover") => {
            Some(report_id(&s.plugins, &k).map_err(|_| tr_fmt!("Unknown page type: {} (schematic, overview, graphic, cover or a report of list_reports)", k))?)
        }
        _ => None,
    };
    let prj = s.project_mut()?;
    let template = match a.str("template") {
        Some(name) => Some(page_templates::find(prj, name).ok_or_else(|| tr_fmt!("No page template named {}", name))?),
        None => None,
    };
    let kind = match (a.str("kind").map(|k| k.trim().to_ascii_lowercase()), &template) {
        (Some(k), _) => match k.as_str() {
            "schematic" => PageKind::Schematic,
            "overview" => PageKind::Overview,
            "graphic" | "graphics" => PageKind::Graphic,
            "cover" => PageKind::Cover,
            _ => PageKind::Report(report.unwrap_or(ReportKind::DeviceList.into())),
        },
        (None, Some(t)) => t.kind,
        (None, None) => PageKind::Schematic,
    };
    if let Some(f) = a.str("frame") {
        if !frame::names(prj).iter().any(|n| n == f) {
            return Err(tr_fmt!("No plot frame named {}", f));
        }
    }
    let at = match a.str("after") {
        Some(after) => page_index(prj, after)? + 1,
        None => prj.pages.len(),
    };
    let section = a.str("section").map(|s| section_id(prj, s)).transpose()?;
    if let Some(name) = a.str("name").map(str::trim) {
        if name.is_empty() {
            return Err(tr("A page needs a name").into());
        }
        if prj.pages.iter().any(|pg| pg.name == name) {
            return Err(tr_fmt!("A page named {} already exists", name));
        }
    }
    let mut i = match template {
        Some(mut t) => {
            t.kind = kind;
            page_templates::insert(prj, &t, at)
        }
        None => ops::add_page(prj, kind, at),
    };
    if let Some(section) = section {
        i = sections::place_new_page(prj, i, section);
    }
    let pg = &mut prj.pages[i];
    if let Some(f) = a.str("frame") {
        pg.frame = f.into();
    }
    if let Some(name) = a.str("name") {
        pg.name = name.into();
    }
    if let Some(d) = a.str("description") {
        pg.description = d.into();
    }
    Ok(page_json(pg))
}

#[cfg(test)]
mod tests;
