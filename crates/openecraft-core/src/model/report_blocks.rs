//! Data of `crate::report_blocks` that the project file holds (see `docs/architecture.md`).

use super::*;

use serde::{Deserialize, Serialize};

/// What a report page shows.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportPart {
    /// Report block that generated the page (output pages: source project and block).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub block: String,
    /// Report scheme; empty: the form of the report.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scheme: String,
    /// Only the rows of this value of the group column; none: all rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// First row shown.
    pub first: usize,
    /// Fixed content output from another project.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frozen: Option<PageTable>,
}

/// The content of one report page, ready to draw.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PageTable {
    pub title: String,
    pub headers: Vec<String>,
    /// Relative column widths.
    pub widths: Vec<f32>,
    pub rows: Vec<Vec<String>>,
    #[serde(skip)]
    pub targets: Vec<Option<(usize, u64)>>,
    /// Rows the row height is fitted to.
    pub per_page: usize,
    /// Text size of the rows (mm); 0: fitted.
    pub text_size: f32,
    /// Rows of the report that follow this page.
    pub more: usize,
    /// The rows continue on the next page of the report block.
    pub continued: bool,
    /// Terminal line-up drawn above the table.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lineup: Vec<LineupBox>,
    /// Project the page was output from.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// Drawn by the graphical form of the report (all columns in the report's order).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub graphic: bool,
    /// Drawn by the user's form (all columns in the report's order).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<super::forms::FormLayout>,
    /// Column names as stored in forms (English), for the placeholders of `layout`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// Total row below the last row (a cell per column); empty: none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub totals: Vec<String>,
    /// Every second row shaded.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub shade_rows: bool,
    /// Alignment per column; empty: all left.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub align: Vec<crate::symbols::Anchor>,
}

/// One element of the terminal line-up drawing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LineupBox {
    pub label: String,
    pub width: f32,
    pub jumper: u32,
    /// End bracket or end plate.
    pub accessory: bool,
    pub spare: bool,
}

/// A set of report pages generated together.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReportBlock {
    pub name: String,
    pub kind: ReportId,
    /// Report scheme; empty: the form of the report.
    pub scheme: String,
    /// New pages for every value of the group column (strip, plug, cable, device …).
    pub per_group: bool,
    /// Groups to generate (`-X*`, several separated by `,`); empty: all.
    pub groups: String,
    /// Structure of the generated pages.
    pub function: String,
    pub location: String,
}
