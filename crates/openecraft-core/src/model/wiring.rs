//! Data of `crate::wiring` that the project file holds (see `docs/architecture.md`).


use serde::{Deserialize, Serialize};
use crate::geom::{p, P};
use crate::symbols::Anchor;
use super::Wire;

/// A placed label reads its text from its owning wire, including after edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum WireLabelKind {
    #[default]
    Core,
    Cable,
    Number,
    Color,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WireLabel {
    pub kind: WireLabelKind,
    /// Attachment along the wire, from 0 at `a` to 1 at `b`.
    pub along: f32,
    /// Drawing offset from the attachment, in mm.
    pub offset: P,
    pub size: f32,
    pub anchor: Anchor,
    pub angle: f32,
    pub color: Option<[u8; 3]>,
}

impl Default for WireLabel {
    fn default() -> Self {
        Self { kind: WireLabelKind::Core, along: 0.5, offset: p(0., -1.), size: 1.8, anchor: Anchor::Left, angle: 0., color: None }
    }
}

impl WireLabel {
    pub fn attachment(&self, wire: &Wire) -> P {
        wire.a + (wire.b - wire.a) * self.along
    }

    pub fn position(&self, wire: &Wire) -> P {
        self.attachment(wire) + self.offset
    }

    pub fn text<'a>(&self, wire: &'a Wire) -> &'a str {
        match self.kind {
            WireLabelKind::Core => &wire.core,
            WireLabelKind::Cable => &wire.cable,
            WireLabelKind::Number => &wire.number,
            WireLabelKind::Color => &wire.color,
        }
    }

    pub fn is_valid(&self) -> bool {
        (0.0..=1.0).contains(&self.along) && self.offset.x.is_finite() && self.offset.y.is_finite()
            && self.size.is_finite() && self.size >= 0.5 && self.angle.is_finite()
    }

    /// Keeps the drawing position when its wire is joined to another wire.
    pub fn rebase(&mut self, from: &Wire, a: P, b: P) {
        let at = self.attachment(from);
        let pos = at + self.offset;
        let dir = b - a;
        let len2 = dir.x * dir.x + dir.y * dir.y;
        self.along = if len2 > 0.0 { (((at - a).x * dir.x + (at - a).y * dir.y) / len2).clamp(0., 1.) } else { 0. };
        self.offset = pos - (a + dir * self.along);
    }
}

/// Mark of a wire, keyed by its two ends.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct WiringMark {
    pub from: String,
    pub to: String,
    pub mark: Mark,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Properties of the wire when it was marked.
    pub color: String,
    pub cross_section: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub number: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mark {
    #[default]
    Done,
    /// Could not be wired (wrong length, missing part …), see the note.
    Problem,
}

#[cfg(test)]
mod tests;
