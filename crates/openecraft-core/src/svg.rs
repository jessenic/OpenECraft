//! SVG export (single page) and a printable HTML document (all pages).

use crate::geom::P;
use crate::model::Project;
use crate::render::{draw_page, Draw, Ink, Opts, TextStyle, VAnchor};
use crate::analysis::Analysis;
use crate::symbols::Anchor;
use std::fmt::Write;

pub struct SvgDraw {
    out: String,
}

fn color(ink: Ink) -> String {
    match ink {
        Ink::Frame | Ink::FrameText => "#555".into(),
        Ink::CrossRef => "#1a3d8f".into(),
        Ink::Error => "#c00".into(),
        Ink::Plate => "#f2f2f2".into(),
        Ink::Fill => "#dcdcdc".into(),
        Ink::Color([r, g, b]) => format!("#{r:02x}{g:02x}{b:02x}"),
        Ink::Paper => "#fff".into(),
        _ => "#000".into(),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

impl SvgDraw {
    fn stroke(&mut self, width: f32, ink: Ink, dashed: bool) -> String {
        let dash = if dashed { " stroke-dasharray=\"1.5 1\"" } else { "" };
        format!("stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"{dash}", color(ink), width.max(0.1))
    }
}

impl Draw for SvgDraw {
    fn line(&mut self, a: P, b: P, width: f32, ink: Ink, dashed: bool) {
        let s = self.stroke(width, ink, dashed);
        let _ = writeln!(self.out, "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" {s}/>", a.x, a.y, b.x, b.y);
    }

    fn poly(&mut self, pts: &[P], closed: bool, width: f32, ink: Ink, fill: bool) {
        let tag = if closed { "polygon" } else { "polyline" };
        let pts: Vec<String> = pts.iter().map(|q| format!("{:.2},{:.2}", q.x, q.y)).collect();
        let fill_s = if fill { color(ink) } else { "none".into() };
        let s = if fill && width <= 0.0 { String::new() } else { self.stroke(width, ink, false) };
        let _ = writeln!(self.out, "<{tag} points=\"{}\" fill=\"{fill_s}\" {s}/>", pts.join(" "));
    }

    fn circle(&mut self, c: P, r: f32, width: f32, ink: Ink, fill: bool) {
        let fill_s = if fill { color(ink) } else { "none".into() };
        let s = if fill && width <= 0.0 { String::new() } else { self.stroke(width, ink, false) };
        let _ = writeln!(self.out, "<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{fill_s}\" {s}/>", c.x, c.y, r);
    }

    fn text(&mut self, pos: P, text: &str, size: f32, h: Anchor, v: VAnchor, ink: Ink) {
        self.styled_text(pos, text, &TextStyle::plain(size, h, v), ink);
    }

    fn styled_text(&mut self, pos: P, text: &str, st: &TextStyle, ink: Ink) {
        if text.is_empty() {
            return;
        }
        let anchor = match st.h {
            Anchor::Left => "start",
            Anchor::Center => "middle",
            Anchor::Right => "end",
        };
        // Approximate vertical alignment via dy (dominant-baseline support varies).
        let dy = match st.v {
            VAnchor::Top => st.size * 0.95,
            VAnchor::Middle => st.size * 0.35,
            VAnchor::Bottom => -st.size * 0.1,
        };
        let mut attrs = String::new();
        if st.bold {
            attrs.push_str(" font-weight=\"bold\"");
        }
        if st.italic {
            attrs.push_str(" font-style=\"italic\"");
        }
        if st.underline {
            attrs.push_str(" text-decoration=\"underline\"");
        }
        if st.angle != 0.0 {
            // SVG rotates clockwise (y down); the text's angle is counter-clockwise.
            let _ = write!(attrs, " transform=\"rotate({:.2} {:.2} {:.2})\"", -st.angle, pos.x, pos.y);
        }
        let _ = writeln!(
            self.out,
            "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"{:.2}\" text-anchor=\"{anchor}\" fill=\"{}\"{attrs}>{}</text>",
            pos.x,
            pos.y + dy,
            st.size * 1.25,
            color(ink),
            esc(text)
        );
    }

    fn link_url(&mut self, min: P, max: P, url: &str) {
        let _ = writeln!(
            self.out,
            "<a href=\"{}\"><rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" fill=\"#000\" fill-opacity=\"0\"/></a>",
            esc(url),
            min.x,
            min.y,
            max.x - min.x,
            max.y - min.y
        );
    }

    fn image_quad(&mut self, img: &crate::model::ImageItem, corners: &[P; 4]) {
        let mime = img.file.mime();
        if !mime.starts_with("image/") || img.file.data.is_empty() {
            crate::render::image_placeholder(self, img);
            return;
        }
        let u = corners[1] - corners[0];
        let v = corners[3] - corners[0];
        let _ = writeln!(self.out,
            "<image width=\"1\" height=\"1\" preserveAspectRatio=\"none\" transform=\"matrix({:.6} {:.6} {:.6} {:.6} {:.6} {:.6})\" href=\"data:{mime};base64,{}\"/>",
            u.x, u.y, v.x, v.y, corners[0].x, corners[0].y, img.file.data);
    }

    fn image(&mut self, img: &crate::model::ImageItem) {
        let mime = img.file.mime();
        if !mime.starts_with("image/") || img.file.data.is_empty() {
            crate::render::image_placeholder(self, img);
            return;
        }
        let r = crate::geom::R::from_points(img.a, img.b);
        let _ = writeln!(
            self.out,
            "<image x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" preserveAspectRatio=\"none\" href=\"data:{mime};base64,{}\"/>",
            r.min.x,
            r.min.y,
            r.width(),
            r.height(),
            img.file.data
        );
    }
}

/// A free drawing of `w` × `h` mm on white.
pub fn drawing_svg(w: f32, h: f32, f: impl FnOnce(&mut SvgDraw)) -> String {
    let mut d = SvgDraw { out: String::new() };
    let _ = writeln!(
        d.out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.0}mm\" height=\"{h:.0}mm\" viewBox=\"0 0 {w:.2} {h:.2}\" font-family=\"Arial, Helvetica, sans-serif\">"
    );
    let _ = writeln!(d.out, "<rect x=\"0\" y=\"0\" width=\"{w:.2}\" height=\"{h:.2}\" fill=\"#fff\"/>");
    f(&mut d);
    d.out.push_str("</svg>\n");
    d.out
}

pub fn page_svg(prj: &Project, page_i: usize) -> String {
    page_svg_with(prj, &Analysis::with_builtin(prj), page_i)
}

/// One page as SVG, drawn with the host's analysis (its reports, its index).
pub fn page_svg_with(prj: &Project, an: &Analysis, page_i: usize) -> String {
    let idx = &an.index;
    let mut d = SvgDraw { out: String::new() };
    let (w, h) = crate::frame::of_page(prj, page_i).size();
    let _ = writeln!(
        d.out,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"0 0 {w} {h}\" font-family=\"Arial, Helvetica, sans-serif\">"
    );
    let _ = writeln!(d.out, "<rect x=\"0\" y=\"0\" width=\"{w}\" height=\"{h}\" fill=\"#fff\"/>");
    draw_page(&mut d, prj, idx, an, page_i, &Opts::default());
    d.out.push_str("</svg>\n");
    d.out
}

/// All pages in one HTML file, one page per printed sheet (A3 landscape).
/// Open in a browser and print to PDF.
pub fn print_document(prj: &Project) -> String {
    print_document_with(prj, &Analysis::with_builtin(prj))
}

/// All pages as HTML, drawn with the host's analysis.
pub fn print_document_with(prj: &Project, an: &Analysis) -> String {
    print_document_pages(prj, an, &(0..prj.pages.len()).collect::<Vec<_>>())
}

/// The print document of the pages `sel` (indices in project order).
pub fn print_document_pages(prj: &Project, an: &Analysis, sel: &[usize]) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "<!DOCTYPE html><html lang=\"{}\"><head><meta charset=\"utf-8\"><title>{}</title><style>\
         body {{ margin: 0; background: #888; }}\
         .page {{ page-break-after: always; background: #fff; margin: 0 auto 8mm auto; }}\
         @media print {{ body {{ background: none; }} .page {{ margin: 0; }} }}",
        // The language the texts are generated in (the export language).
        openecraft_i18n::lang().code(),
        esc(&prj.info.name)
    );
    // One named @page rule per paper size.
    let sizes: Vec<(f32, f32)> = sel.iter().map(|&i| crate::frame::of_page(prj, i).size()).collect();
    let mut distinct: Vec<(f32, f32)> = Vec::new();
    for s in &sizes {
        if !distinct.contains(s) {
            distinct.push(*s);
        }
    }
    for (n, (w, h)) in distinct.iter().enumerate() {
        let _ = write!(out, "@page s{n} {{ size: {w}mm {h}mm; margin: 0; }}");
    }
    out.push_str("</style></head><body>\n");
    for (k, (w, h)) in sizes.iter().enumerate() {
        let i = sel[k];
        let n = distinct.iter().position(|s| s == &(*w, *h)).unwrap_or(0);
        let _ = writeln!(out, "<div class=\"page\" style=\"page: s{n}; width: {w}mm; height: {h}mm\">");
        out.push_str(&page_svg_with(prj, an, i));
        out.push_str("</div>\n");
    }
    out.push_str("</body></html>\n");
    out
}

#[cfg(test)]
mod tests;
