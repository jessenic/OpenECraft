//! Forms: title blocks and report forms. A title block is a list of lines,
//! rectangles and texts with placeholders (`{project}`, `{page}`, `{sheet}` …) that
//! is drawn into the title block area at the bottom of a page. Companies define their
//! own title blocks and share them as files; report forms choose the columns, their
//! order and the rows per report page.

use openecraft_i18n::{tr, tr_const};
use crate::geom::{p, P};
use crate::model::{PageKind, Project, ReportId, ReportKind};
use crate::params::Text;
use crate::render::{Draw, Ink, VAnchor};
use crate::table::header_matches;
use crate::symbols::Anchor;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeMap;
pub use crate::model::forms::{FilterOp, Form, FormItem, FormLayout, Forms, Picture, ReportForm, ReportScheme, RowFilter, Show, SortKey, Stamp};

/// Size of the title block area (origin at its top left corner).
pub const AREA_W: f32 = crate::frame::TITLE_W;
pub const AREA_H: f32 = crate::frame::TITLE_H;

pub const STANDARD: &str = "Standard";
pub const COMPACT: &str = "Compact";
pub const NONE: &str = "None";

impl FormItem {
    pub fn kind_name(&self) -> &'static str {
        match self {
            FormItem::Line { .. } => tr("Line"),
            FormItem::Rect { .. } => tr("Rectangle"),
            FormItem::Text { .. } => tr("Text"),
            FormItem::Image { picture: Some(_), .. } => tr("Image"),
            FormItem::Image { picture: None, .. } => tr("Project logo"),
            FormItem::Qr { .. } => tr("QR code"),
        }
    }

    pub fn line(a: P, b: P, width: f32) -> FormItem {
        FormItem::Line { a, b, width, color: None, dashed: false, show: Show::default() }
    }

    pub fn rect(a: P, b: P, width: f32, fill: bool) -> FormItem {
        FormItem::Rect { a, b, width, fill, color: None, dashed: false, show: Show::default() }
    }

    /// A plain text (no label, no limit, not rotated).
    pub fn text(pos: P, text: &str, size: f32, anchor: Anchor) -> FormItem {
        FormItem::Text { pos, text: text.into(), size, anchor, label: false, max_width: 0.0, bold: false, italic: false, angle: 0.0, color: None, show: Show::default() }
    }

    /// A label: a text in the frame colour.
    pub fn label(pos: P, text: &str, size: f32, max_width: f32) -> FormItem {
        let mut t = FormItem::text(pos, text, size, Anchor::Left);
        if let FormItem::Text { label, max_width: m, .. } = &mut t {
            (*label, *m) = (true, max_width);
        }
        t
    }

    pub fn image(a: P, b: P, picture: Option<Picture>) -> FormItem {
        FormItem::Image { a, b, picture, show: Show::default() }
    }

    pub fn qr(a: P, size: f32, text: &str) -> FormItem {
        FormItem::Qr { a, size, text: text.into(), show: Show::default() }
    }

    /// Where the element is drawn.
    pub fn show(&self) -> &Show {
        match self {
            FormItem::Line { show, .. } | FormItem::Rect { show, .. } | FormItem::Text { show, .. } | FormItem::Image { show, .. } | FormItem::Qr { show, .. } => show,
        }
    }

    pub fn show_mut(&mut self) -> &mut Show {
        match self {
            FormItem::Line { show, .. } | FormItem::Rect { show, .. } | FormItem::Text { show, .. } | FormItem::Image { show, .. } | FormItem::Qr { show, .. } => show,
        }
    }

    /// Corners of the area of an image or QR code.
    fn area(&self) -> Option<(P, P)> {
        match self {
            FormItem::Image { a, b, .. } => Some((p(a.x.min(b.x), a.y.min(b.y)), p(a.x.max(b.x), a.y.max(b.y)))),
            FormItem::Qr { a, size, .. } => Some((*a, *a + p(*size, *size))),
            _ => None,
        }
    }

    /// Distance from `q` (title block coordinates) for picking in the editor.
    pub fn distance(&self, q: P) -> f32 {
        match self {
            FormItem::Line { a, b, .. } => crate::geom::dist_to_segment(q, *a, *b),
            FormItem::Rect { a, b, .. } => {
                let c = [*a, p(b.x, a.y), *b, p(a.x, b.y), *a];
                c.windows(2).map(|s| crate::geom::dist_to_segment(q, s[0], s[1])).fold(f32::INFINITY, f32::min)
            }
            FormItem::Image { .. } | FormItem::Qr { .. } => {
                let (lo, hi) = self.area().unwrap_or_default();
                let dx = (lo.x - q.x).max(q.x - hi.x).max(0.0);
                let dy = (lo.y - q.y).max(q.y - hi.y).max(0.0);
                (dx * dx + dy * dy).sqrt()
            }
            FormItem::Text { pos, text, size, anchor, .. } => {
                let w = text_width(text, *size);
                let x0 = match anchor {
                    Anchor::Left => pos.x,
                    Anchor::Center => pos.x - w / 2.0,
                    Anchor::Right => pos.x - w,
                };
                let dx = (x0 - q.x).max(q.x - (x0 + w)).max(0.0);
                let dy = ((q.y - pos.y).abs() - size * 0.6).max(0.0);
                (dx * dx + dy * dy).sqrt()
            }
        }
    }

    pub fn translate(&mut self, d: P) {
        match self {
            FormItem::Line { a, b, .. } | FormItem::Rect { a, b, .. } | FormItem::Image { a, b, .. } => {
                *a = *a + d;
                *b = *b + d;
            }
            FormItem::Text { pos, .. } | FormItem::Qr { a: pos, .. } => *pos = *pos + d,
        }
    }
}

/// Page types of the conditions of elements and of the title blocks by page type:
/// (key, title).
pub const PAGE_TYPES: [(&str, &str); 7] = [
    ("schematic", tr_const!("Schematic")),
    ("cover", tr_const!("Cover")),
    ("report", tr_const!("Report")),
    ("mounting", tr_const!("Mounting")),
    ("overview", tr_const!("Overview")),
    ("graphic", tr_const!("Graphics")),
    ("document", tr_const!("Document")),
];

/// Key of the page type of a page kind (`PAGE_TYPES`).
pub fn page_type(kind: PageKind) -> &'static str {
    match kind {
        PageKind::Schematic => "schematic",
        PageKind::Cover => "cover",
        PageKind::Report(_) => "report",
        PageKind::Mounting(_) => "mounting",
        PageKind::Overview => "overview",
        PageKind::Graphic => "graphic",
        PageKind::Document => "document",
    }
}

impl FilterOp {
    pub const ALL: [FilterOp; 7] =
        [FilterOp::Contains, FilterOp::NotContains, FilterOp::Equals, FilterOp::NotEquals, FilterOp::StartsWith, FilterOp::Empty, FilterOp::NotEmpty];

    pub fn title(self) -> &'static str {
        match self {
            FilterOp::Contains => tr("contains"),
            FilterOp::NotContains => tr("does not contain"),
            FilterOp::Equals => tr("is"),
            FilterOp::NotEquals => tr("is not"),
            FilterOp::StartsWith => tr("starts with"),
            FilterOp::Empty => tr("is empty"),
            FilterOp::NotEmpty => tr("is not empty"),
        }
    }

    /// Needs a value to compare with.
    pub fn has_value(self) -> bool {
        !matches!(self, FilterOp::Empty | FilterOp::NotEmpty)
    }
}

impl RowFilter {
    pub fn passes(&self, cell: &str) -> bool {
        let (c, v) = (cell.to_lowercase(), self.value.trim().to_lowercase());
        match self.op {
            FilterOp::Contains => c.contains(&v),
            FilterOp::NotContains => !c.contains(&v),
            FilterOp::Equals => c == v,
            FilterOp::NotEquals => c != v,
            FilterOp::StartsWith => c.starts_with(&v),
            FilterOp::Empty => c.trim().is_empty(),
            FilterOp::NotEmpty => !c.trim().is_empty(),
        }
    }
}

impl Default for ReportScheme {
    fn default() -> Self {
        ReportScheme { name: String::new(), kind: ReportKind::DeviceList.into(), form: ReportForm::default() }
    }
}

/// File format for exchanging title blocks, report schemes and plot frames (also the
/// company's form library on the parts server, `openecraft_parts::api::FORMS_LIBRARY`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FormLibrary {
    pub title_blocks: Vec<Form>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schemes: Vec<ReportScheme>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<crate::model::frame::PlotFrame>,
}

impl FormLibrary {
    pub fn from_json(text: &str) -> Result<FormLibrary, String> {
        serde_json::from_str(text).map_err(|e| e.to_string())
    }

    /// The project's own title blocks, report schemes and plot frames.
    pub fn of(prj: &Project) -> FormLibrary {
        FormLibrary { title_blocks: prj.forms.title_blocks.clone(), schemes: prj.forms.schemes.clone(), frames: prj.forms.frames.clone() }
    }

    pub fn is_empty(&self) -> bool {
        self.title_blocks.is_empty() && self.schemes.is_empty() && self.frames.is_empty()
    }
}

/// Adds the title blocks, report schemes and plot frames of a library to the
/// project; ones with the same name (schemes: of the same report) are replaced.
/// Returns the numbers of title blocks and schemes.
pub fn merge(prj: &mut Project, lib: FormLibrary) -> (usize, usize) {
    let n = (lib.title_blocks.len(), lib.schemes.len());
    for f in lib.frames {
        crate::frame::set(prj, f);
    }
    for f in lib.title_blocks {
        match prj.forms.title_blocks.iter_mut().find(|x| x.name == f.name) {
            Some(x) => *x = f,
            None => prj.forms.title_blocks.push(f),
        }
    }
    for s in lib.schemes {
        match prj.forms.schemes.iter_mut().find(|x| x.name == s.name && x.kind == s.kind) {
            Some(x) => *x = s,
            None => prj.forms.schemes.push(s),
        }
    }
    n
}

/// Report form of a report: the scheme of that name, otherwise the report's form.
pub fn report_form<'a>(prj: &'a Project, kind: impl Into<ReportId>, scheme: &str) -> Option<&'a ReportForm> {
    let kind = kind.into();
    let named = prj.forms.schemes.iter().find(|s| !scheme.is_empty() && s.kind == kind && s.name == scheme);
    named.map(|s| &s.form).or_else(|| prj.forms.reports.get(&kind))
}

/// Renames a scheme together with the report blocks and pages that use it. Fails for
/// empty names and names the report already has.
pub fn rename_scheme(prj: &mut Project, kind: impl Into<ReportId>, old: &str, new: &str) -> bool {
    let kind = kind.into();
    let new = new.trim();
    if new.is_empty() || prj.forms.schemes.iter().any(|s| s.kind == kind && s.name == new) {
        return false;
    }
    let Some(s) = prj.forms.schemes.iter_mut().find(|s| s.kind == kind && s.name == old) else { return false };
    s.name = new.to_string();
    for b in prj.report_blocks.iter_mut().filter(|b| b.kind == kind && b.scheme == old) {
        b.scheme = new.to_string();
    }
    for pg in prj.pages.iter_mut().filter(|p| p.kind == PageKind::Report(kind)) {
        if let Some(r) = pg.report.as_mut().filter(|r| r.scheme == old) {
            r.scheme = new.to_string();
        }
    }
    true
}

/// Names of the schemes of a report.
pub fn scheme_names(prj: &Project, kind: impl Into<ReportId>) -> Vec<String> {
    let kind = kind.into();
    prj.forms.schemes.iter().filter(|s| s.kind == kind).map(|s| s.name.clone()).collect()
}

fn label(items: &mut Vec<FormItem>, x: f32, y: f32, text: &'static str) {
    items.push(FormItem::label(p(x + 1.0, y + 1.8), tr(text), 1.6, 0.0));
}

fn value(items: &mut Vec<FormItem>, x: f32, y: f32, text: &str, size: f32, max_width: f32) {
    let mut t = FormItem::text(p(x + 2.0, y), text, size, Anchor::Left);
    if let FormItem::Text { max_width: m, .. } = &mut t {
        *m = max_width;
    }
    items.push(t);
}

fn line(items: &mut Vec<FormItem>, a: P, b: P, width: f32) {
    items.push(FormItem::line(a, b, width));
}

/// The built-in title blocks.
pub fn builtin() -> Vec<Form> {
    let (w, h) = (AREA_W, AREA_H);
    let ym = h / 2.0;
    let mut std = Vec::new();
    line(&mut std, p(0.0, 0.0), p(w, 0.0), 0.5);
    for x in [80.0, 140.0, 200.0, 320.0, 360.0] {
        line(&mut std, p(x, 0.0), p(x, h), 0.25);
    }
    for (x0, x1) in [(80.0, 140.0), (140.0, 200.0), (320.0, 360.0), (360.0, w)] {
        line(&mut std, p(x0, ym), p(x1, ym), 0.25);
    }
    label(&mut std, 0.0, 0.0, "Company");
    value(&mut std, 0.0, ym + 2.0, "{company}", 4.0, 76.0);
    label(&mut std, 80.0, 0.0, "Date");
    value(&mut std, 80.0, 9.0, "{date}", 2.5, 56.0);
    label(&mut std, 80.0, ym, "Creator");
    value(&mut std, 80.0, ym + 9.0, "{creator}", 2.5, 56.0);
    label(&mut std, 140.0, 0.0, "Revision");
    value(&mut std, 140.0, 9.0, "{revision}", 2.5, 56.0);
    label(&mut std, 140.0, ym, "Software");
    value(&mut std, 140.0, ym + 9.0, "{software}", 2.5, 56.0);
    label(&mut std, 200.0, 0.0, "Project / page description");
    value(&mut std, 200.0, 9.0, "{project}", 3.5, 116.0);
    value(&mut std, 200.0, 21.0, "{page.description}", 3.0, 116.0);
    label(&mut std, 320.0, 0.0, "Page");
    value(&mut std, 320.0, 9.5, "{page}", 4.0, 36.0);
    label(&mut std, 320.0, ym, "Type");
    value(&mut std, 320.0, ym + 9.0, "{page.type}", 2.5, 36.0);
    label(&mut std, 360.0, 0.0, "Sheet");
    value(&mut std, 360.0, 9.5, "{sheet} / {sheets}", 3.5, 36.0);
    label(&mut std, 360.0, ym, "Project");
    value(&mut std, 360.0, ym + 9.0, "{project}", 2.2, 36.0);

    let mut compact = Vec::new();
    line(&mut compact, p(0.0, 0.0), p(w, 0.0), 0.5);
    for x in [120.0, 300.0, 350.0] {
        line(&mut compact, p(x, 0.0), p(x, h), 0.25);
    }
    value(&mut compact, 0.0, 10.0, "{company}", 3.5, 115.0);
    value(&mut compact, 0.0, 22.0, "{project}", 3.0, 115.0);
    value(&mut compact, 120.0, 10.0, "{page.description}", 3.5, 175.0);
    value(&mut compact, 120.0, 22.0, "{page.structure}", 2.5, 175.0);
    label(&mut compact, 300.0, 0.0, "Revision / date");
    value(&mut compact, 300.0, 12.0, "{revision}", 3.0, 46.0);
    value(&mut compact, 300.0, 22.0, "{date}", 2.2, 46.0);
    label(&mut compact, 350.0, 0.0, "Page");
    value(&mut compact, 350.0, 12.0, "{page}", 4.0, 46.0);
    value(&mut compact, 350.0, 22.0, "{sheet} / {sheets}", 2.5, 46.0);

    vec![
        Form { name: STANDARD.into(), items: std },
        Form { name: COMPACT.into(), items: compact },
        Form { name: NONE.into(), items: Vec::new() },
    ]
}

/// Names of all title blocks of a project (custom ones first).
pub fn names(prj: &Project) -> Vec<String> {
    let mut v: Vec<String> = prj.forms.title_blocks.iter().map(|f| f.name.clone()).collect();
    for f in builtin() {
        if !v.contains(&f.name) {
            v.push(f.name);
        }
    }
    v
}

/// A title block by name: the project's own ones take precedence over the built-in
/// ones; unknown names give the standard title block.
pub fn find<'a>(prj: &'a Project, name: &str) -> Cow<'a, Form> {
    if let Some(f) = prj.forms.title_blocks.iter().find(|f| f.name == name) {
        return Cow::Borrowed(f);
    }
    let mut all = builtin();
    let i = all.iter().position(|f| f.name == name).unwrap_or(0);
    Cow::Owned(all.swap_remove(i))
}

/// Name of the title block used on a page: its own, the one of its page type, the
/// one of its plot frame, the project default, the standard one.
pub fn title_block_name(prj: &Project, page_i: usize) -> String {
    let page = &prj.pages[page_i];
    let by_type = prj.forms.page_type_title_blocks.get(page_type(page.kind));
    let frame = crate::frame::of_page(prj, page_i).title_block;
    let names = [Some(&page.title_block), by_type, Some(&frame), Some(&prj.forms.default_title_block)];
    let name = names.into_iter().flatten().find(|n| !n.is_empty()).cloned();
    name.unwrap_or_else(|| STANDARD.into())
}

/// Title block used on a page.
pub fn title_block(prj: &Project, page_i: usize) -> Cow<'_, Form> {
    find(prj, &title_block_name(prj, page_i))
}

fn structure_description(prj: &Project, aspect: crate::structure::Aspect, id: &str) -> String {
    prj.structures.entry(aspect, id).map(|e| e.description.clone()).unwrap_or_default()
}

/// Value of one placeholder (without braces); `None` for unknown names.
pub fn placeholder(prj: &Project, page_i: usize, name: &str) -> Option<String> {
    let info = &prj.info;
    let page = &prj.pages[page_i];
    let rev = crate::revision::last_released(prj);
    let v = match name {
        "project" => info.name.clone(),
        "project.description" => info.description.lines().collect::<Vec<_>>().join(" "),
        "company" => info.company.clone(),
        "creator" => info.creator.clone(),
        "date" => info.date.clone(),
        "revision" => info.revision.clone(),
        "software" => "OpenECraft".into(),
        "page" => crate::structure::page_full_name(page),
        "page.name" => page.name.clone(),
        "page.description" => page.description.clone(),
        "page.structure" => crate::structure::page_structure(page),
        "page.function" => page.function.clone(),
        "page.location" => page.location.clone(),
        "page.installation" => page.installation.clone(),
        "page.product" => page.product.clone(),
        "page.function.description" => structure_description(prj, crate::structure::Aspect::Function, &page.function),
        "page.location.description" => structure_description(prj, crate::structure::Aspect::Location, &page.location),
        "page.type" => match page.kind {
            PageKind::Schematic => tr("Schematic"),
            PageKind::Cover => tr("Cover"),
            PageKind::Report(_) => tr("Report"),
            PageKind::Mounting(_) => tr("Mounting"),
            PageKind::Overview => tr("Overview"),
            PageKind::Graphic => tr("Graphics"),
            PageKind::Document => tr("Document"),
        }
        .into(),
        "sheet" => (page_i + 1).to_string(),
        "sheets" => prj.pages.len().to_string(),
        "page.previous" => page_i.checked_sub(1).map(|i| crate::structure::page_full_name(&prj.pages[i])).unwrap_or_default(),
        "page.next" => prj.pages.get(page_i + 1).map(crate::structure::page_full_name).unwrap_or_default(),
        "rev.name" => rev.map(|r| r.name.clone()).unwrap_or_default(),
        "rev.date" => rev.map(|r| r.date.clone()).unwrap_or_default(),
        "rev.author" => rev.map(|r| r.author.clone()).unwrap_or_default(),
        "rev.description" => rev.map(|r| r.description.clone()).unwrap_or_default(),
        "rev.status" => revision_status(prj),
        "rev.approved_by" | "rev.approved_date" => revision_placeholder(prj, &format!("1.{}", &name[4..]))?,
        _ => match name.strip_prefix("prop:") {
            Some(key) => prop_placeholder(prj, page, key),
            None => revision_placeholder(prj, name.strip_prefix("rev.")?)?,
        },
    };
    Some(v)
}

/// `{rev.N.field}`: a field of the N-th last released revision (1: the last one),
/// for revision tables in title blocks; empty when there are fewer revisions.
fn revision_placeholder(prj: &Project, name: &str) -> Option<String> {
    let (n, field) = name.split_once('.')?;
    let n: usize = n.parse().ok().filter(|n| *n > 0)?;
    let released = |r: &&crate::revision::Revision| r.state == crate::revision::State::Released;
    let i = prj.revisions.iter().enumerate().rev().filter(|(_, r)| released(r)).nth(n - 1).map(|(i, _)| i);
    let rev = i.map(|i| &prj.revisions[i]);
    let v = |f: fn(&crate::revision::Revision) -> &String| rev.map(|r| f(r).clone()).unwrap_or_default();
    Some(match field {
        "name" => v(|r| &r.name),
        "date" => v(|r| &r.date),
        "author" => v(|r| &r.author),
        "description" => v(|r| &r.description),
        "approved_by" => i.map(|i| approvals(prj, i).0).unwrap_or_default(),
        "approved_date" => i.map(|i| approvals(prj, i).1).unwrap_or_default(),
        _ => return None,
    })
}

/// Who approved revision `i` in its last review round (approvers of the project
/// workflow, in the order they decided) and the date of the last approval.
pub fn approvals(prj: &Project, i: usize) -> (String, String) {
    let ok: Vec<_> = crate::revision::decisions(prj, i).into_iter().filter(|a| a.approved).collect();
    let users: Vec<&str> = ok.iter().map(|a| a.user.as_str()).collect();
    let date = ok.iter().map(|a| a.date.as_str()).max().unwrap_or("");
    (users.join(", "), date.to_string())
}

/// State of the project in the revision workflow: the status of the revision being
/// prepared, otherwise "released" (or "draft" before the first release).
fn revision_status(prj: &Project) -> String {
    let status = match crate::revision::open_revision(prj) {
        Some(i) => crate::revision::status(prj, i),
        None if crate::revision::last_released(prj).is_some() => crate::revision::Status::Released,
        None => crate::revision::Status::Draft,
    };
    status.title().into()
}

/// Text of the stamp across the pages in the current state of the revision
/// workflow (placeholders not filled yet); `None` when there is none.
pub fn stamp_text(prj: &Project) -> Option<&str> {
    use crate::revision::Status;
    let s = &prj.forms.stamp;
    let text = match crate::revision::open_revision(prj).map(|i| crate::revision::status(prj, i)) {
        Some(Status::Draft) => &s.draft,
        Some(Status::InReview | Status::Rejected) => &s.review,
        Some(Status::Approved) => &s.approved,
        Some(Status::Released) => &s.released,
        None if crate::revision::last_released(prj).is_some() => &s.released,
        None => &s.draft,
    };
    Some(text.as_str()).filter(|t| !t.trim().is_empty())
}

/// Draws the stamp across the drawing area of a page (below its content).
pub fn draw_stamp(d: &mut dyn Draw, prj: &Project, page_i: usize) {
    let Some(text) = stamp_text(prj) else { return };
    let text = fill(text, prj, page_i);
    let s = &prj.forms.stamp;
    let area = crate::frame::of_page(prj, page_i).area();
    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(1).max(1) as f32;
    let lines = text.lines().count().max(1) as f32;
    // Fitted: the longest line along 70 % of the page's diagonal at the angle.
    let (sin, cos) = s.angle.to_radians().sin_cos();
    let reach = (area.width() / cos.abs().max(0.01)).min(area.height() / sin.abs().max(0.01)) * 0.7;
    let size = if s.size > 0.0 { s.size } else { (reach / (longest * 0.62)).clamp(5.0, 80.0) };
    let style = crate::render::TextStyle { angle: s.angle, bold: true, ..crate::render::TextStyle::plain(size, Anchor::Center, VAnchor::Middle) };
    let c = area.center();
    for (i, l) in text.lines().enumerate() {
        // Lines below each other across the text's direction.
        let off = (i as f32 - (lines - 1.0) / 2.0) * size * 1.3;
        d.styled_text(c + p(off * sin, off * cos), l, &style, Ink::Color(s.color));
    }
}

/// `{prop:key}`: a project or page property of the database (shown with its unit),
/// otherwise a free project property by name.
fn prop_placeholder(prj: &Project, page: &crate::model::Page, key: &str) -> String {
    use crate::properties::{self, ObjectClass, Object};
    match properties::valid_key(prj, key).map(|d| d.class) {
        Some(ObjectClass::Project) => properties::display(prj, &Object::Project, key),
        Some(ObjectClass::Page) => properties::display(prj, &Object::Page(page.id), key),
        _ => prj.info.properties.get(key).cloned().unwrap_or_default(),
    }
}

/// Placeholders offered in the title block editor: (placeholder, meaning).
pub const PLACEHOLDERS: [(&str, &str); 37] = [
    ("{project}", tr_const!("Project name")),
    ("{project.description}", tr_const!("Project description")),
    ("{company}", tr_const!("Company")),
    ("{creator}", tr_const!("Created by")),
    ("{date}", tr_const!("Date")),
    ("{revision}", tr_const!("Project revision")),
    ("{software}", tr_const!("OpenECraft")),
    ("{page}", tr_const!("Full page name (=A1+CAB1/3)")),
    ("{page.name}", tr_const!("Page number")),
    ("{page.description}", tr_const!("Page description")),
    ("{page.structure}", tr_const!("Page structure (=A1+CAB1)")),
    ("{page.function}", tr_const!("Function of the page")),
    ("{page.location}", tr_const!("Location of the page")),
    ("{page.installation}", tr_const!("Installation site of the page")),
    ("{page.product}", tr_const!("Higher-level product of the page")),
    ("{page.function.description}", tr_const!("Description of the page's function (structure management)")),
    ("{page.location.description}", tr_const!("Description of the page's location (structure management)")),
    ("{page.type}", tr_const!("Page type")),
    ("{sheet}", tr_const!("Sheet number")),
    ("{sheets}", tr_const!("Number of sheets")),
    ("{page.previous}", tr_const!("Previous page")),
    ("{page.next}", tr_const!("Next page")),
    ("{rev.name}", tr_const!("Last revision")),
    ("{rev.date}", tr_const!("Date of the last revision")),
    ("{rev.author}", tr_const!("Author of the last revision")),
    ("{rev.description}", tr_const!("Description of the last revision")),
    ("{rev.1.name}", tr_const!("Revision table: index of the last revision (2: the one before …)")),
    ("{rev.1.date}", tr_const!("Revision table: date of the last revision (2: the one before …)")),
    ("{rev.1.author}", tr_const!("Revision table: author of the last revision (2: the one before …)")),
    ("{rev.1.description}", tr_const!("Revision table: description of the last revision (2: the one before …)")),
    ("{rev.approved_by}", tr_const!("Who approved the last revision (review); {rev.2.approved_by}: the one before")),
    ("{rev.approved_date}", tr_const!("Date of the approval of the last revision")),
    ("{rev.status}", tr_const!("State of the project: draft, in review, approved, released")),
    ("{prop:Name}", "Project property \"Name\""),
    ("{prop:project.customer}", tr_const!("Project property of the property database (by key)")),
    ("{prop:page.drawing_number}", tr_const!("Page property of the property database (by key)")),
    ("{{ }}", tr_const!("Literal braces")),
];

/// Replaces the placeholders of a text; unknown ones stay as they are.
pub fn fill(text: &str, prj: &Project, page_i: usize) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        let rest = &text[i..];
        if rest.starts_with("{{") || rest.starts_with("}}") {
            out.push_str(&rest[..1]);
            i += 2;
            continue;
        }
        if let (true, Some(end)) = (rest.starts_with('{'), rest.find('}')) {
            match placeholder(prj, page_i, rest[1..end].trim()) {
                Some(v) => out.push_str(&v),
                None => out.push_str(&rest[..=end]),
            }
            i += end + 1;
            continue;
        }
        let c = rest.chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

/// Estimated width of a text (same estimate as the canvas hit tests).
pub fn text_width(text: &str, size: f32) -> f32 {
    text.chars().count() as f32 * size * 0.62
}

/// Draws a title block for a page into the title block area of its plot frame.
pub fn draw(d: &mut dyn Draw, form: &Form, prj: &Project, page_i: usize) {
    let frame = crate::frame::of_page(prj, page_i);
    draw_at(d, form, prj, page_i, frame.title_origin(), frame.title_scale());
}

/// Draws a title block with its top left corner at `o`, scaled by `k`.
pub fn draw_at(d: &mut dyn Draw, form: &Form, prj: &Project, page_i: usize, o: P, k: f32) {
    let page_type = Some(page_type(prj.pages[page_i].kind));
    draw_items(d, &form.items, o, k, &Context { logo: prj.info.logo.as_ref(), page_type, value: &|text| fill(text, prj, page_i) });
}

/// What the elements of a form are drawn with.
pub struct Context<'a> {
    /// Drawn by images without a picture.
    pub logo: Option<&'a Picture>,
    /// Type of the page (`PAGE_TYPES`) for the conditions of the elements; `None`
    /// draws every element (editor).
    pub page_type: Option<&'a str>,
    /// Fills the placeholders of texts, QR codes and conditions.
    pub value: &'a dyn Fn(&str) -> String,
}

/// Whether an element is drawn in a context.
pub fn shown(show: &Show, ctx: &Context) -> bool {
    let Some(page_type) = ctx.page_type else { return true };
    (show.page_types.is_empty() || show.page_types.iter().any(|t| t == page_type)) && (show.filled.trim().is_empty() || !(ctx.value)(&show.filled).trim().is_empty())
}

/// Draws a QR code of `text` as runs of dark modules, `size` mm wide, top left at `a`.
pub fn draw_qr(d: &mut dyn Draw, a: P, size: f32, text: &str, ink: Ink) {
    let Some(qr) = crate::qr::Qr::encode(text) else { return };
    let m = size / qr.size as f32;
    for y in 0..qr.size {
        let mut x = 0;
        while x < qr.size {
            if !qr.dark(x, y) {
                x += 1;
                continue;
            }
            let start = x;
            while qr.dark(x, y) {
                x += 1;
            }
            d.rect(a + p(start as f32 * m, y as f32 * m), a + p(x as f32 * m, (y + 1) as f32 * m), 0.0, ink, true);
        }
    }
}

/// Texture id of a picture: the same for the same file, apart from the ids of the
/// images placed on pages (counted up from 1).
pub fn picture_id(pic: &Picture) -> u64 {
    // FNV-1a over the length and both ends of the data (cheap for large files).
    let data = pic.file.data.as_bytes();
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let ends = data.iter().take(512).chain(data.iter().rev().take(512));
    for b in data.len().to_le_bytes().iter().chain(ends) {
        h = (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3);
    }
    h | 1 << 63
}

/// The picture as an image fitted into `a`..`b` (keeping its aspect ratio, centred).
pub fn fitted_image(pic: &Picture, a: P, b: P) -> crate::model::ImageItem {
    let r = crate::geom::R::from_points(a, b);
    let (w, h) = (r.width(), r.height());
    let size = if pic.px[0] == 0 || pic.px[1] == 0 {
        p(w, h)
    } else {
        let k = (w / pic.px[0] as f32).min(h / pic.px[1] as f32);
        p(pic.px[0] as f32 * k, pic.px[1] as f32 * k)
    };
    let min = r.center() - size * 0.5;
    crate::model::ImageItem { id: picture_id(pic), a: min, b: min + size, file: pic.file.clone(), px: pic.px, layer: String::new() }
}

/// A picture from a PNG or JPEG file; `None` for other files.
pub fn picture(name: &str, bytes: &[u8]) -> Option<Picture> {
    let px = crate::graphics::image_pixels(bytes)?;
    Some(Picture { file: crate::model::document::Document::new(name, bytes), px })
}

/// The pictures drawn on a page by its title block, its drawn report form and the
/// cover page, for loading them as textures.
pub fn page_pictures(prj: &Project, page_i: usize) -> Vec<Picture> {
    let logo = prj.info.logo.as_ref();
    let page = &prj.pages[page_i];
    let mut items: Vec<FormItem> = title_block(prj, page_i).items.clone();
    if let PageKind::Report(kind) = page.kind {
        let scheme = page.report.as_ref().map(|r| r.scheme.as_str()).unwrap_or("");
        if let Some(l) = report_form(prj, kind, scheme).and_then(|f| f.layout.as_ref()) {
            items.extend(l.header.iter().chain(&l.row).cloned());
        }
    }
    let from_items = items.iter().filter_map(|it| match it {
        FormItem::Image { picture, .. } => picture.as_ref().or(logo),
        _ => None,
    });
    let cover = logo.filter(|_| page.kind == PageKind::Cover);
    let mut out: Vec<Picture> = Vec::new();
    for pic in from_items.chain(cover) {
        if !out.contains(pic) {
            out.push(pic.clone());
        }
    }
    out
}

/// Draws form items with their origin at `o`, scaled by `k`.
pub fn draw_items(d: &mut dyn Draw, items: &[FormItem], o: P, k: f32, ctx: &Context) {
    let at = |q: P| o + q * k;
    let value = ctx.value;
    for item in items.iter().filter(|it| shown(it.show(), ctx)) {
        match item {
            FormItem::Line { a, b, width, color, dashed, .. } => d.line(at(*a), at(*b), width * k, color.map(Ink::Color).unwrap_or(Ink::Frame), *dashed),
            FormItem::Rect { a, b, width, fill, color, dashed, .. } => {
                let ink = color.map(Ink::Color).unwrap_or(Ink::Frame);
                if *fill {
                    d.rect(at(*a), at(*b), 0.0, color.map(Ink::Color).unwrap_or(Ink::Plate), true);
                }
                if *width > 0.0 && *dashed {
                    let (a, b) = (at(*a), at(*b));
                    let c = [a, p(b.x, a.y), b, p(a.x, b.y), a];
                    for s in c.windows(2) {
                        d.line(s[0], s[1], width * k, ink, true);
                    }
                } else if *width > 0.0 {
                    d.rect(at(*a), at(*b), width * k, ink, false);
                }
            }
            FormItem::Text { pos, text, size, anchor, label, max_width, bold, italic, angle, color, .. } => {
                let ink = color.map(Ink::Color).unwrap_or(if *label { Ink::FrameText } else { Ink::Text });
                let value = value(text);
                // Further lines follow below the first one in the text's direction.
                let (sin, cos) = angle.to_radians().sin_cos();
                for (i, l) in value.lines().enumerate() {
                    let w = text_width(l, *size);
                    let size = if *max_width > 0.0 && w > *max_width { (size * max_width / w).max(1.0) } else { *size };
                    let dy = i as f32 * size * 1.4;
                    let pos = at(*pos + p(dy * sin, dy * cos));
                    if *bold || *italic || *angle != 0.0 {
                        let plain = crate::render::TextStyle::plain(size * k, *anchor, VAnchor::Middle);
                        let style = crate::render::TextStyle { angle: *angle, bold: *bold, italic: *italic, ..plain };
                        d.styled_text(pos, l, &style, ink);
                    } else {
                        d.text(pos, l, size * k, *anchor, VAnchor::Middle, ink);
                    }
                }
            }
            FormItem::Image { a, b, picture, .. } => {
                if let Some(pic) = picture.as_ref().or(ctx.logo) {
                    d.image(&fitted_image(pic, at(*a), at(*b)));
                }
            }
            FormItem::Qr { a, size, text, .. } => {
                let text = value(text);
                if !text.trim().is_empty() {
                    draw_qr(d, at(*a), size * k, &text, Ink::Text);
                }
            }
        }
    }
}

/// Replaces the column placeholders of a text with the cells of a report row; a
/// placeholder names a column by its key (`keys`, English) or its title (`headers`).
/// Other placeholders stay as they are.
pub fn fill_row(text: &str, keys: &[String], headers: &[String], row: &[String]) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(escaped) = after.strip_prefix("{{") {
            out.push_str("{{");
            rest = escaped;
            continue;
        }
        let Some(end) = after.find('}') else {
            out.push_str(after);
            return out;
        };
        let name = after[1..end].trim();
        let col = keys.iter().position(|k| k == name).or_else(|| headers.iter().position(|h| h == name));
        match col {
            Some(i) => out.push_str(&row.get(i).map(|c| c.replace('{', "{{").replace('}', "}}")).unwrap_or_default()),
            None => out.push_str(&after[..=end]),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Draws a report page with the user's form: the header once, the row drawing for
/// every row (`o`: top left corner of the drawing area). Rows link to where their
/// entry is in the schematic.
pub fn draw_layout(d: &mut dyn Draw, layout: &FormLayout, t: &crate::model::report_blocks::PageTable, prj: &Project, page_i: usize, o: P) {
    let page_value = |text: &str| fill(&text.replace("{report}", &t.title.replace('{', "{{").replace('}', "}}")), prj, page_i);
    let (logo, page_type) = (prj.info.logo.as_ref(), Some(page_type(prj.pages[page_i].kind)));
    draw_items(d, &layout.header, o, 1.0, &Context { logo, page_type, value: &page_value });
    let bounds = items_bounds(&layout.row);
    for (ri, row) in t.rows.iter().enumerate() {
        let at = o + layout.first_row + layout.step * ri as f32;
        draw_items(d, &layout.row, at, 1.0, &Context { logo, page_type, value: &|text| page_value(&fill_row(text, &t.keys, &t.headers, row)) });
        if let (Some(Some((page, _))), Some((a, b))) = (t.targets.get(ri), bounds) {
            d.link(at + a, at + b, *page);
        }
    }
}

/// Smallest rectangle around form items (texts by their estimated width).
pub fn items_bounds(items: &[FormItem]) -> Option<(P, P)> {
    let mut pts = Vec::new();
    for it in items {
        match it {
            FormItem::Line { a, b, .. } | FormItem::Rect { a, b, .. } | FormItem::Image { a, b, .. } => pts.extend([*a, *b]),
            FormItem::Qr { a, size, .. } => pts.extend([*a, *a + p(*size, *size)]),
            FormItem::Text { pos, text, size, anchor, .. } => {
                let w = text_width(text, *size);
                let x = match anchor {
                    Anchor::Left => pos.x,
                    Anchor::Center => pos.x - w / 2.0,
                    Anchor::Right => pos.x - w,
                };
                pts.extend([p(x, pos.y - size / 2.0), p(x + w, pos.y + size / 2.0)]);
            }
        }
    }
    let first = *pts.first()?;
    Some(pts.iter().fold((first, first), |(lo, hi), q| (p(lo.x.min(q.x), lo.y.min(q.y)), p(hi.x.max(q.x), hi.y.max(q.y)))))
}

/// A form to start from: the report's columns (`keys` with their `titles`) as a
/// table of `width` mm, a heading with the report name, column titles and one line
/// per row.
pub fn starting_layout(keys: &[&str], titles: &[&str], widths: &[f32], width: f32) -> FormLayout {
    let total: f32 = widths.iter().sum::<f32>().max(0.01);
    let mut xs = vec![0.0];
    for w in widths {
        xs.push(xs.last().unwrap_or(&0.0) + w / total * width);
    }
    let (top, row_h) = (19.0, 6.0);
    let mut header = vec![
        FormItem::text(p(0.0, 9.0), "{report}", 6.0, Anchor::Left),
        FormItem::rect(p(0.0, top), p(width, top + row_h), 0.35, false),
    ];
    let mut row = vec![FormItem::line(p(0.0, row_h), p(width, row_h), 0.1)];
    for (i, key) in keys.iter().enumerate() {
        let (x, w) = (xs[i], xs[i + 1] - xs[i]);
        let title = titles.get(i).copied().unwrap_or(key);
        header.push(FormItem::label(p(x + 1.2, top + row_h / 2.0), title, 2.4, w - 2.0));
        let mut cell = FormItem::text(p(x + 1.2, row_h / 2.0), &format!("{{{key}}}"), 2.2, Anchor::Left);
        if let FormItem::Text { max_width, .. } = &mut cell {
            *max_width = w - 2.0;
        }
        row.push(cell);
        if i > 0 {
            row.push(FormItem::line(p(x, 0.0), p(x, row_h), 0.15));
        }
    }
    row.push(FormItem::line(p(0.0, 0.0), p(0.0, row_h), 0.3));
    row.push(FormItem::line(p(width, 0.0), p(width, row_h), 0.3));
    FormLayout { header, row, first_row: p(0.0, top + row_h), step: p(0.0, row_h) }
}

/// Indices of the report columns shown by a form (all if it chooses none or none
/// of its columns exist).
pub fn report_columns(form: Option<&ReportForm>, headers: &[Text]) -> Vec<usize> {
    let chosen: Vec<usize> = form
        .map(|f| f.columns.iter().filter_map(|c| headers.iter().position(|h| header_matches(h, c))).collect())
        .unwrap_or_default();
    if chosen.is_empty() {
        (0..headers.len()).collect()
    } else {
        chosen
    }
}

/// The value a form stores for a header (by key or any of its translations).
pub fn header_value<'a, T>(map: &'a BTreeMap<String, T>, header: &Text) -> Option<&'a T> {
    map.iter().find(|(k, _)| header_matches(header, k)).map(|(_, v)| v)
}

#[cfg(test)]
mod tests;
