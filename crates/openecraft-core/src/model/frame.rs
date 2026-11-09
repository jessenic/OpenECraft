//! Data of `crate::frame` that the project file holds (see `docs/architecture.md`).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PlotFrame {
    pub name: String,
    /// Draw the generated border and title block (imported pages may carry their own).
    pub visible: bool,
    pub paper: Paper,
    pub landscape: bool,
    /// Width and height (as drawn) of custom paper in mm.
    pub custom: [f32; 2],
    /// Distance of the border from the paper edge.
    pub margin: f32,
    /// Width of the column and row strips.
    pub strip: f32,
    pub columns: usize,
    pub rows: usize,
    pub column_labels: Labels,
    pub row_labels: Labels,
    /// Column / row strips (cross references) on schematic and overview pages.
    pub strips: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Paper {
    A0,
    A1,
    A2,
    A3,
    A4,
    Letter,
    Legal,
    Tabloid,
    /// Width and height from [`PlotFrame::custom`].
    Custom,
}

/// How columns or rows are labelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Labels {
    /// 1, 2, 3 …
    Numbers,
    ZeroBasedNumbers,
    /// A, B, … Z, AA, AB …
    Letters,
}
