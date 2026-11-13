//! Data of `crate::forms` that the project file holds (see `docs/architecture.md`).


use crate::geom::P;
use super::ReportId;
use super::document::Document;
use crate::symbols::Anchor;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Forms {
    /// Title blocks defined for the project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub title_blocks: Vec<Form>,
    /// Title block of pages that do not choose one; empty: the standard one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_title_block: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub reports: BTreeMap<ReportId, ReportForm>,
    /// Named report forms.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub schemes: Vec<ReportScheme>,
    /// Plot frames defined for the project.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frames: Vec<super::frame::PlotFrame>,
    /// Plot frame of pages that do not choose one; empty: A3 landscape.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_frame: String,
    /// Title blocks by page type (`schematic`, `report` …, `crate::forms::PAGE_TYPES`)
    /// for pages that do not choose one; before the plot frame's and the project default.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub page_type_title_blocks: BTreeMap<String, String>,
    /// Stamp drawn across every page while the project is not released.
    #[serde(skip_serializing_if = "Stamp::is_off")]
    pub stamp: Stamp,
}

/// A large text across the pages that follows the revision workflow, e.g. "DRAFT"
/// while a revision is being prepared; texts take placeholders, empty: no stamp.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stamp {
    /// While a revision is a draft (also before the first release).
    pub draft: String,
    /// While a revision is in review (or was rejected).
    pub review: String,
    /// When the revision in review is approved but not released yet.
    pub approved: String,
    /// When every revision is released.
    pub released: String,
    /// Text height in mm; 0: fitted to the page.
    pub size: f32,
    /// Degrees, counter-clockwise.
    pub angle: f32,
    pub color: [u8; 3],
}

impl Default for Stamp {
    fn default() -> Self {
        Stamp { draft: String::new(), review: String::new(), approved: String::new(), released: String::new(), size: 0.0, angle: 30.0, color: [230, 150, 150] }
    }
}

impl Stamp {
    pub fn is_off(&self) -> bool {
        [&self.draft, &self.review, &self.approved, &self.released].iter().all(|t| t.trim().is_empty())
    }
}

/// A named report form of one report, e.g. "Terminals for the workshop"; report
/// blocks choose it by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportScheme {
    pub name: String,
    pub kind: ReportId,
    pub form: ReportForm,
}

/// Layout of a report page.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportForm {
    /// Heading of the page; empty: the name of the report.
    pub title: String,
    /// Columns shown, in this order (header names); empty: all columns.
    pub columns: Vec<String>,
    /// Rows per page; 0: the default.
    pub rows: usize,
    /// Only rows that pass all filters are shown.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<RowFilter>,
    /// Sort order (first key first); empty: the order of the report.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub sort: Vec<SortKey>,
    /// Column that groups the rows: report blocks start new pages for every value;
    /// empty: the first column.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub group: String,
    /// Values of the first shown column that repeat the row above are left blank.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub blank_repeats: bool,
    /// Relative column widths by header name (instead of the report's).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub widths: BTreeMap<String, f32>,
    /// Text size of the rows (mm); 0: fitted to the rows per page.
    pub text_size: f32,
    /// Drawn as a graphic (terminal and cable diagrams) instead of a table.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub graphic: bool,
    /// Drawn by the user's own form (form editor) instead of a table.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<FormLayout>,
    /// Columns summed in a total row below the last row (header names).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<String>,
    /// Every second row shaded.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shade_rows: bool,
    /// Alignment of columns by header name (default: left).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub align: BTreeMap<String, Anchor>,
}

/// A report form drawn by the user: graphics drawn once per page and a row drawing
/// repeated for every row of the page. Texts take the column names of the report as
/// placeholders (`{Terminal}`, `{Target}`; the English name or the one shown) besides
/// those of title blocks (`{project}`, `{page}` …). Coordinates in mm from the top
/// left corner of the drawing area of the plot frame.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FormLayout {
    /// Drawn once per page.
    pub header: Vec<FormItem>,
    /// Drawn for every row, relative to the row's origin.
    pub row: Vec<FormItem>,
    /// Origin of the first row.
    pub first_row: P,
    /// From one row to the next: downwards for lists, sideways to draw the rows
    /// side by side (e.g. terminals of a strip).
    pub step: P,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SortKey {
    pub column: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub descending: bool,
}

/// Keeps the rows whose cell in `column` passes (case-insensitive).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RowFilter {
    pub column: String,
    pub op: FilterOp,
    pub value: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilterOp {
    #[default]
    Contains,
    NotContains,
    Equals,
    NotEquals,
    StartsWith,
    Empty,
    NotEmpty,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Form {
    pub name: String,
    pub items: Vec<FormItem>,
}

/// When an element is drawn; the default: always.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Show {
    /// Drawn only when this text (with placeholders) is not empty, e.g. `{rev.2.name}`
    /// for a row of a revision table.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub filled: String,
    /// Drawn only on pages of these types (`schematic`, `report` …); empty: on all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub page_types: Vec<String>,
}

impl Show {
    pub fn is_always(&self) -> bool {
        self.filled.trim().is_empty() && self.page_types.is_empty()
    }
}

/// Elements of title blocks and drawn forms. `color` replaces the frame or text
/// colour; `show` limits where the element is drawn.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FormItem {
    Line {
        a: P,
        b: P,
        width: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<[u8; 3]>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dashed: bool,
        #[serde(default, skip_serializing_if = "Show::is_always")]
        show: Show,
    },
    Rect {
        a: P,
        b: P,
        width: f32,
        #[serde(default)]
        fill: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<[u8; 3]>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dashed: bool,
        #[serde(default, skip_serializing_if = "Show::is_always")]
        show: Show,
    },
    /// Text with placeholders. `label` texts are drawn in the frame colour;
    /// `max_width` > 0 shrinks long values to fit; `angle` in degrees,
    /// counter-clockwise.
    Text {
        pos: P,
        text: String,
        size: f32,
        anchor: Anchor,
        #[serde(default)]
        label: bool,
        #[serde(default)]
        max_width: f32,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        bold: bool,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        italic: bool,
        #[serde(default, skip_serializing_if = "is_zero")]
        angle: f32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        color: Option<[u8; 3]>,
        #[serde(default, skip_serializing_if = "Show::is_always")]
        show: Show,
    },
    /// A bitmap fitted into the rectangle `a`..`b` keeping its aspect ratio, e.g.
    /// the company logo; without a picture the logo of the project.
    Image {
        a: P,
        b: P,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Show::is_always")]
        show: Show,
    },
    /// A QR code of `size` mm with its top left corner at `a`; the text takes
    /// placeholders (e.g. a link to the project in the document management).
    Qr {
        a: P,
        size: f32,
        text: String,
        #[serde(default, skip_serializing_if = "Show::is_always")]
        show: Show,
    },
}

/// An embedded bitmap (PNG, JPEG) with its size in pixels.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Picture {
    pub file: Document,
    pub px: [u32; 2],
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}
