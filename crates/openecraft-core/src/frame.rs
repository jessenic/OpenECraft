//! Plot frames: paper format and orientation of a page, its border, the column /
//! row strips for cross references and the title block area. The project chooses a
//! default frame, every page may use another one; companies define their own frames.

use openecraft_i18n::{tr, tr_const};
use crate::geom::{p, P, R};
use crate::model::{PageKind, Project};
pub use crate::model::frame::{Labels, Paper, PlotFrame};

/// Width of the title block forms at scale 1 (they shrink on narrower frames).
pub const TITLE_W: f32 = 400.0;
/// Height of the title block forms at scale 1.
pub const TITLE_H: f32 = 32.0;

/// Frame of pages that do not choose one and whose project sets no default.
pub const DEFAULT: &str = "A3 landscape";

impl Paper {
    pub const ALL: [Paper; 9] = [Paper::A0, Paper::A1, Paper::A2, Paper::A3, Paper::A4, Paper::Letter, Paper::Legal, Paper::Tabloid, Paper::Custom];

    /// Portrait size in mm (short side first).
    pub fn size(self) -> Option<(f32, f32)> {
        Some(match self {
            Paper::A0 => (841.0, 1189.0),
            Paper::A1 => (594.0, 841.0),
            Paper::A2 => (420.0, 594.0),
            Paper::A3 => (297.0, 420.0),
            Paper::A4 => (210.0, 297.0),
            Paper::Letter => (215.9, 279.4),
            Paper::Legal => (215.9, 355.6),
            Paper::Tabloid => (279.4, 431.8),
            Paper::Custom => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Paper::A0 => "A0",
            Paper::A1 => "A1",
            Paper::A2 => "A2",
            Paper::A3 => "A3",
            Paper::A4 => "A4",
            Paper::Letter => "Letter",
            Paper::Legal => "Legal",
            Paper::Tabloid => "Tabloid",
            Paper::Custom => tr("Custom"),
        }
    }

    pub fn from_name(s: &str) -> Option<Paper> {
        Paper::ALL.into_iter().find(|p| p.name().eq_ignore_ascii_case(s.trim()) || (*p == Paper::Custom && s.trim().eq_ignore_ascii_case("custom")))
    }
}

impl Labels {
    pub fn label(self, i: usize) -> String {
        match self {
            Labels::Numbers => (i + 1).to_string(),
            Labels::ZeroBasedNumbers => i.to_string(),
            Labels::Letters => {
                let mut n = i + 1;
                let mut s = Vec::new();
                while n > 0 {
                    s.push((b'A' + ((n - 1) % 26) as u8) as char);
                    n = (n - 1) / 26;
                }
                s.iter().rev().collect()
            }
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Labels::Numbers => tr("Numbers (1, 2, 3)"),
            Labels::ZeroBasedNumbers => tr("Numbers (0, 1, 2)"),
            Labels::Letters => tr("Letters (A, B, C)"),
        }
    }
}

impl Default for PlotFrame {
    fn default() -> Self {
        PlotFrame {
            name: DEFAULT.into(),
            visible: true,
            paper: Paper::A3,
            landscape: true,
            custom: [420.0, 297.0],
            margin: 10.0,
            strip: 5.0,
            columns: 10,
            rows: 6,
            column_labels: Labels::Numbers,
            row_labels: Labels::Letters,
            strips: true,
        }
    }
}

impl PlotFrame {
    fn new(name: &str, paper: Paper, landscape: bool, columns: usize, rows: usize) -> PlotFrame {
        PlotFrame { name: name.into(), paper, landscape, columns, rows, ..Default::default() }
    }

    /// Paper width and height in mm.
    pub fn size(&self) -> (f32, f32) {
        match self.paper.size() {
            Some((s, l)) if self.landscape => (l, s),
            Some(portrait) => portrait,
            None => (self.custom[0].max(50.0), self.custom[1].max(50.0)),
        }
    }

    pub fn width(&self) -> f32 {
        self.size().0
    }

    pub fn height(&self) -> f32 {
        self.size().1
    }

    /// The border.
    pub fn border(&self) -> R {
        let (w, h) = self.size();
        let m = self.margin.clamp(0.0, w.min(h) / 4.0);
        R { min: p(m, m), max: p(w - m, h - m) }
    }

    fn strip_w(&self) -> f32 {
        if self.strips { self.strip.max(0.0) } else { 0.0 }
    }

    /// Scale of the title block forms (they are drawn for a 400 mm wide border).
    pub fn title_scale(&self) -> f32 {
        (self.border().width() / TITLE_W).min(1.0)
    }

    /// Top left corner of the title block.
    pub fn title_origin(&self) -> P {
        let b = self.border();
        p(b.min.x, b.max.y - TITLE_H * self.title_scale())
    }

    /// Drawing area: inside the strips, above the title block.
    pub fn area(&self) -> R {
        let b = self.border();
        let s = self.strip_w();
        R { min: p(b.min.x + s, b.min.y + s), max: p(b.max.x, self.title_origin().y) }
    }

    pub fn column_width(&self) -> f32 {
        self.area().width() / self.columns.max(1) as f32
    }

    pub fn row_height(&self) -> f32 {
        self.area().height() / self.rows.max(1) as f32
    }

    /// Column index (0-based) of a page position.
    pub fn column_index(&self, pos: P) -> usize {
        let c = ((pos.x - self.area().min.x) / self.column_width()).floor() as i64;
        c.clamp(0, self.columns.max(1) as i64 - 1) as usize
    }

    pub fn row_index(&self, pos: P) -> usize {
        let r = ((pos.y - self.area().min.y) / self.row_height()).floor() as i64;
        r.clamp(0, self.rows.max(1) as i64 - 1) as usize
    }

    /// Column label of a position, as used in cross references (`/page.column`).
    pub fn column_of(&self, pos: P) -> String {
        self.column_labels.label(self.column_index(pos))
    }

    pub fn row_of(&self, pos: P) -> String {
        self.row_labels.label(self.row_index(pos))
    }

    /// `A3 landscape, 420 x 297 mm`
    pub fn summary(&self) -> String {
        let (w, h) = self.size();
        let orientation = if w >= h { tr("landscape") } else { tr("portrait") };
        format!("{} {orientation}, {w:.0} x {h:.0} mm", self.paper.name())
    }
}

/// Page kinds with column / row strips.
pub fn has_strips(kind: PageKind) -> bool {
    matches!(kind, PageKind::Schematic | PageKind::Overview)
}

/// The built-in frames.
pub fn builtin() -> Vec<PlotFrame> {
    BUILTIN.iter().map(|(name, paper, landscape, columns, rows)| PlotFrame::new(name, *paper, *landscape, *columns, *rows)).collect()
}

const BUILTIN: [(&str, Paper, bool, usize, usize); 9] = [
    (tr_const!("A3 landscape"), Paper::A3, true, 10, 6),
    (tr_const!("A3 portrait"), Paper::A3, false, 8, 10),
    (tr_const!("A4 landscape"), Paper::A4, true, 8, 5),
    (tr_const!("A4 portrait"), Paper::A4, false, 6, 8),
    (tr_const!("A2 landscape"), Paper::A2, true, 12, 8),
    (tr_const!("A1 landscape"), Paper::A1, true, 16, 10),
    (tr_const!("A0 landscape"), Paper::A0, true, 20, 14),
    (tr_const!("Letter landscape"), Paper::Letter, true, 8, 5),
    (tr_const!("Tabloid landscape"), Paper::Tabloid, true, 10, 6),
];

pub fn is_builtin(name: &str) -> bool {
    BUILTIN.iter().any(|b| b.0 == name)
}

/// Shown name: built-in frames are translated.
pub fn display_name(name: &str) -> String {
    match BUILTIN.iter().find(|b| b.0 == name) {
        Some(b) => tr(b.0).to_string(),
        None => name.to_string(),
    }
}

/// Names of all frames of a project (its own first).
pub fn names(prj: &Project) -> Vec<String> {
    let mut v: Vec<String> = prj.forms.frames.iter().map(|f| f.name.clone()).collect();
    for (name, ..) in BUILTIN {
        if !v.iter().any(|n| n == name) {
            v.push(name.to_string());
        }
    }
    v
}

/// A frame by name: the project's own ones take precedence; unknown names give
/// the default frame.
pub fn find(prj: &Project, name: &str) -> PlotFrame {
    if let Some(f) = prj.forms.frames.iter().find(|f| f.name == name) {
        return f.clone();
    }
    match BUILTIN.iter().find(|b| b.0 == name) {
        Some((name, paper, landscape, columns, rows)) => PlotFrame::new(name, *paper, *landscape, *columns, *rows),
        None => PlotFrame::default(),
    }
}

/// Default frame of the project.
pub fn project_default(prj: &Project) -> PlotFrame {
    let name = &prj.forms.default_frame;
    find(prj, if name.is_empty() { DEFAULT } else { name })
}

/// Name of the frame a page uses.
pub fn page_frame_name(prj: &Project, page_i: usize) -> &str {
    let own = &prj.pages[page_i].frame;
    let name = if !own.is_empty() { own } else { &prj.forms.default_frame };
    if name.is_empty() { DEFAULT } else { name }
}

/// Frame of a page.
pub fn of_page(prj: &Project, page_i: usize) -> PlotFrame {
    find(prj, page_frame_name(prj, page_i))
}

/// Adds or replaces a frame of the project.
pub fn set(prj: &mut Project, frame: PlotFrame) {
    match prj.forms.frames.iter_mut().find(|f| f.name == frame.name) {
        Some(f) => *f = frame,
        None => prj.forms.frames.push(frame),
    }
}

/// Cross reference of a page position: `/3.4` (page name and column), formatted after
/// the project's cross reference settings (prefix, separator, row).
pub fn reference(prj: &Project, page_i: usize, pos: P) -> String {
    let x = &prj.xref;
    let frame = of_page(prj, page_i);
    let row = if x.row { format!("{}{}", x.separator, frame.row_of(pos)) } else { String::new() };
    format!("{}{}{}{}{row}", x.prefix, prj.pages[page_i].name, x.separator, frame.column_of(pos))
}

#[cfg(test)]
mod tests;
