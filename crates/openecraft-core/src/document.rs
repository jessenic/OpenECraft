//! Embedded documents: a document page carries a file (PDF, Office document, image,
//! text …) inside the project. The page shows what the file is; the PDF export
//! attaches it to the page and it can be saved again from the project.

use openecraft_i18n::{tr, tr_fmt};
use crate::frame::PlotFrame;
use crate::geom::{p, P};
use crate::model::{PageKind, Project};
use crate::render::{Draw, Ink, VAnchor};
use crate::symbols::Anchor;
pub use crate::model::document::Document;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pdf,
    Word,
    Excel,
    PowerPoint,
    OpenDocument,
    Image,
    Text,
    Other,
}

impl Kind {
    pub fn title(self) -> &'static str {
        match self {
            Kind::Pdf => tr("PDF document"),
            Kind::Word => tr("Word document"),
            Kind::Excel => tr("Excel workbook"),
            Kind::PowerPoint => tr("PowerPoint presentation"),
            Kind::OpenDocument => tr("OpenDocument file"),
            Kind::Image => tr("Image"),
            Kind::Text => tr("Text file"),
            Kind::Other => tr("File"),
        }
    }
}

impl Document {
    pub fn new(name: &str, bytes: &[u8]) -> Document {
        Document { name: name.to_string(), data: base64_encode(bytes).into() }
    }

    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        base64_decode(&self.data).ok_or_else(|| tr_fmt!("The embedded document {} is damaged", self.name))
    }

    /// Size of the file in bytes.
    pub fn size(&self) -> usize {
        let pad = self.data.bytes().rev().take_while(|b| *b == b'=').count();
        (self.data.len() / 4 * 3).saturating_sub(pad)
    }

    pub fn extension(&self) -> String {
        self.name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
    }

    pub fn kind(&self) -> Kind {
        match self.extension().as_str() {
            "pdf" => Kind::Pdf,
            "doc" | "docx" | "rtf" => Kind::Word,
            "xls" | "xlsx" | "xlsm" | "csv" => Kind::Excel,
            "ppt" | "pptx" => Kind::PowerPoint,
            "odt" | "ods" | "odp" | "odg" => Kind::OpenDocument,
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "tif" | "tiff" | "webp" => Kind::Image,
            "txt" | "md" | "log" => Kind::Text,
            _ => Kind::Other,
        }
    }

    pub fn mime(&self) -> &'static str {
        match self.extension().as_str() {
            "pdf" => "application/pdf",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xls" => "application/vnd.ms-excel",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            "odt" => "application/vnd.oasis.opendocument.text",
            "ods" => "application/vnd.oasis.opendocument.spreadsheet",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "svg" => "image/svg+xml",
            "txt" | "csv" | "md" => "text/plain",
            _ => "application/octet-stream",
        }
    }
}

/// `12.3 kB`
pub fn size_text(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} kB", bytes as f32 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f32 / 1024.0 / 1024.0)
    }
}

/// Inserts a document page with the file after page `after`. Returns its index.
pub fn add_page(prj: &mut Project, after: usize, name: &str, bytes: &[u8]) -> usize {
    let at = crate::ops::add_page(prj, PageKind::Document, after + 1);
    let doc = Document::new(name, bytes);
    prj.pages[at].description = name.rsplit_once('.').map(|(n, _)| n).unwrap_or(name).to_string();
    prj.pages[at].document = Some(doc);
    at
}

/// Area of the file symbol on a document page (attachment in the PDF export).
pub fn icon_area(frame: &PlotFrame) -> (P, P) {
    let a = frame.area();
    let c = a.center();
    let h = (a.height() * 0.3).clamp(20.0, 60.0);
    let w = h * 0.75;
    (p(c.x - w / 2.0, c.y - h * 0.9), p(c.x + w / 2.0, c.y + h * 0.1))
}

/// Content of a document page: a file symbol with name, type and size.
pub fn draw(d: &mut dyn Draw, frame: &PlotFrame, doc: Option<&Document>) {
    let a = frame.area();
    let c = a.center();
    let Some(doc) = doc else {
        d.text(c, tr("No document embedded - choose a file in the page properties"), 4.0, Anchor::Center, VAnchor::Middle, Ink::Error);
        return;
    };
    let (min, max) = icon_area(frame);
    let fold = (max.x - min.x) * 0.3;
    let outline = [min, p(max.x - fold, min.y), p(max.x, min.y + fold), max, p(min.x, max.y)];
    d.poly(&outline, true, 0.5, Ink::Frame, false);
    d.poly(&[p(max.x - fold, min.y), p(max.x - fold, min.y + fold), p(max.x, min.y + fold)], false, 0.35, Ink::Frame, false);
    let ext = doc.extension().to_uppercase();
    let size = ((max.x - min.x) / ext.chars().count().max(3) as f32 * 1.3).min(8.0);
    d.text(p((min.x + max.x) / 2.0, (min.y + max.y) / 2.0 + fold / 2.0), &ext, size, Anchor::Center, VAnchor::Middle, Ink::Text);
    let y = max.y + 10.0;
    d.text(p(c.x, y), &doc.name, 5.0, Anchor::Center, VAnchor::Middle, Ink::Text);
    let info = format!("{}  -  {}", doc.kind().title(), size_text(doc.size()));
    d.text(p(c.x, y + 8.0), &info, 3.0, Anchor::Center, VAnchor::Middle, Ink::FrameText);
    d.text(p(c.x, y + 14.0), tr("Embedded document: attached to this page in the PDF export"), 2.5, Anchor::Center, VAnchor::Middle, Ink::FrameText);
}

pub use openecraft_parts::base64::{decode as base64_decode, encode as base64_encode};

#[cfg(test)]
mod tests;
