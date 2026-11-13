//! Sheet tools: document pages with embedded files and pictures of title blocks.

use super::common::*;
use super::{optional, param, Args, Kind, Tool};
use crate::session::Session;
use openecraft_core::document;
use openecraft_core::forms::{self, FormItem, Picture};
use openecraft_core::geom::p;
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub static TOOLS: &[Tool] = &[
    Tool {
        name: "add_document_page",
        help: tr_const!("Embeds a file (PDF, Office document, image …) as a document page; the PDF export attaches it to the page"),
        params: &[
            param("path", Kind::Str, tr_const!("File to embed")),
            optional("after", Kind::Str, tr_const!("Insert after this page (default: at the end)")),
            optional("description", Kind::Str, tr_const!("Description")),
        ],
        needs_project: true,
        run: add_document_page,
    },
    Tool {
        name: "export_document",
        help: tr_const!("Writes the embedded file of a document page"),
        params: &[PAGE, param("path", Kind::Str, tr_const!("Output file"))],
        needs_project: true,
        run: export_document,
    },
    Tool {
        name: "add_title_block_image",
        help: tr_const!("Adds an image (e.g. the company logo) to a title block of the project; without a file it shows the project logo"),
        params: &[
            param("title_block", Kind::Str, tr_const!("Title block of the project (list_title_blocks)")),
            optional("path", Kind::Str, tr_const!("PNG or JPEG file kept in the title block (default: the project logo)")),
            param("x", Kind::Num, tr_const!("Left edge in mm from the left of the title block area")),
            param("y", Kind::Num, tr_const!("Top edge in mm from the top of the title block area")),
            param("width", Kind::Num, tr_const!("Width in mm")),
            param("height", Kind::Num, tr_const!("Height in mm; the image keeps its aspect ratio inside")),
        ],
        needs_project: true,
        run: add_title_block_image,
    },
];

/// A PNG or JPEG file as a picture of a title block or the project logo.
pub fn read_picture(path: &str) -> Result<Picture, String> {
    let bytes = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    forms::picture(name, &bytes).ok_or_else(|| tr_fmt!("{} is not a PNG or JPEG image", name))
}

fn add_title_block_image(s: &mut Session, a: &Args) -> Result<Value, String> {
    let picture = a.str("path").filter(|p| !p.is_empty()).map(read_picture).transpose()?;
    let num = |k: &str| a.num(k).unwrap_or(0.0);
    let (x, y, w, h) = (num("x"), num("y"), num("width"), num("height"));
    if w <= 0.0 || h <= 0.0 {
        return Err(tr("Width and height must be greater than 0").into());
    }
    let prj = s.project_mut()?;
    let name = a.req("title_block");
    let Some(f) = prj.forms.title_blocks.iter_mut().find(|f| f.name == name) else {
        return Err(tr_fmt!("No title block of the project named {}", name));
    };
    let file = picture.as_ref().map(|p| p.file.name.clone());
    f.items.push(FormItem::image(p(x, y), p(x + w, y + h), picture));
    Ok(json!({ "title_block": name, "items": f.items.len(), "file": file }))
}

fn add_document_page(s: &mut Session, a: &Args) -> Result<Value, String> {
    let path = a.req("path");
    let bytes = std::fs::read(path).map_err(|e| tr_fmt!("Cannot read {}: {}", path, openecraft_i18n::io_error(&e)))?;
    let prj = s.project_mut()?;
    let after = match a.str("after") {
        Some(n) => page_index(prj, n)?,
        None => prj.pages.len() - 1,
    };
    let file = path.rsplit(['/', '\\']).next().unwrap_or(path);
    let i = document::add_page(prj, after, file, &bytes);
    if let Some(d) = a.str("description") {
        prj.pages[i].description = d.into();
    }
    let mut v = page_json(&prj.pages[i]);
    v["bytes"] = json!(bytes.len());
    Ok(v)
}

fn export_document(s: &mut Session, a: &Args) -> Result<Value, String> {
    let prj = s.project()?;
    let pg = &prj.pages[page_index(prj, a.req("page"))?];
    let doc = pg.document.as_ref().ok_or_else(|| tr_fmt!("Page {} has no embedded document", pg.name))?;
    let bytes = doc.bytes()?;
    let path = a.req("path");
    std::fs::write(path, &bytes).map_err(|e| tr_fmt!("Cannot write {}: {}", path, openecraft_i18n::io_error(&e)))?;
    Ok(json!({ "path": path, "name": doc.name, "bytes": bytes.len() }))
}

#[cfg(test)]
mod tests;
