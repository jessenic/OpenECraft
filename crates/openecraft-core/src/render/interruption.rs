//! Name and cross reference of interruption points: laid out along the conductor or
//! beside the arrow, on a side the user chose, or placed by hand.

use super::*;

/// Gap between the symbol bounds (which have a margin already) and the texts, and
/// between the two texts.
const GAP: f32 = 0.5;
const TEXT_GAP: f32 = 1.0;

/// One of the two texts of an interruption point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntrText {
    Label,
    Xref,
}

/// Size of the name: that of the symbol's label field.
pub fn label_size(def: &SymbolDef) -> f32 {
    def.prims.iter().find_map(|q| match q {
        Prim::Field { kind: FieldKind::Label, size, .. } => Some(*size),
        _ => None,
    })
    .unwrap_or(2.5)
}

/// Width of a name or cross reference as drawn (1.25 × size, Arial in the exports): the
/// shared estimate is too narrow for names of capitals and digits such as `L1_OUT`, and
/// the cross reference follows right after the name.
fn name_width(text: &str, size: f32) -> f32 {
    let em: f32 = text
        .chars()
        .map(|c| match c {
            '.' | ',' | ':' | ';' | '/' | '|' | '!' | '\'' | ' ' | '-' | '(' | ')' | '[' | ']' | 'i' | 'j' | 'l' | 'I' => 0.3,
            c if c as u32 >= 0x2e80 => 1.0,
            c if c.is_lowercase() => 0.56,
            _ => 0.72,
        })
        .sum();
    em * size * 1.25
}

/// Where the name and the cross reference are drawn.
pub fn interruption_places(def: &SymbolDef, s: &SymbolInst) -> (TextPlace, TextPlace) {
    let (label, xref) = side_places(def, s, s.text_side);
    (s.label_place.unwrap_or(label), s.xref_place.unwrap_or(xref))
}

/// The places of both texts on a side, in one line: left and right of the symbol the name
/// next to it and the cross reference after it (above the conductor when it runs on
/// that side), above and below the name before and the cross reference after the
/// symbol's centre line, so that a vertical conductor runs between them.
pub fn side_places(def: &SymbolDef, s: &SymbolInst, side: TextSide) -> (TextPlace, TextPlace) {
    let b = def.world_bounds(s.pos, s.rot, s.mirror);
    let c = b.center();
    let size = label_size(def);
    let w = name_width(&s.label, size);
    let place = |q: P, anchor: Anchor| TextPlace { offset: q - s.pos, anchor };
    let pin = def.pins.first().map(|pin| s.pos + pin.pos.transform(s.rot, s.mirror)).unwrap_or(p(c.x, b.min.y));
    let dir = c - pin;
    let side = match side {
        TextSide::Auto => {
            if dir.x.abs() > dir.y.abs() {
                // Horizontal arrows, mostly at the edge of the page: the name above the
                // conductor and the cross reference below it, running back from the
                // connection point along the conductor.
                let anchor = if dir.x > 0.0 { Anchor::Right } else { Anchor::Left };
                let (above, below) = (pin.y - GAP - size * 0.7, pin.y + GAP + SMALL_SIZE * 0.7);
                return (place(p(pin.x, above), anchor), place(p(pin.x, below), anchor));
            } else {
                // Vertical arrows: the name beside the arrow, the cross reference below it.
                let next = c.y + (size + SMALL_SIZE) * 0.7 + GAP;
                return if s.mirror {
                    let x = b.min.x - GAP;
                    (place(p(x, c.y), Anchor::Right), place(p(x, next), Anchor::Right))
                } else {
                    let x = b.max.x + GAP;
                    (place(p(x, c.y), Anchor::Left), place(p(x, next), Anchor::Left))
                };
            }
        }
        side => side,
    };
    let gap = if w > 0.0 { w + TEXT_GAP } else { 0.0 };
    // On the side of a horizontal conductor the line goes above it.
    let beside = |conductor: bool| if conductor && dir.x.abs() > dir.y.abs() { pin.y - GAP - size * 0.7 } else { c.y };
    match side {
        TextSide::Left => {
            let (x, y) = (b.min.x - GAP, beside(dir.x > 0.0));
            (place(p(x, y), Anchor::Right), place(p(x - gap, y), Anchor::Right))
        }
        TextSide::Above | TextSide::Below => {
            let y = if side == TextSide::Above { b.min.y - GAP - size * 0.7 } else { b.max.y + GAP + size * 0.7 };
            (place(p(c.x - TEXT_GAP / 2.0, y), Anchor::Right), place(p(c.x + TEXT_GAP / 2.0, y), Anchor::Left))
        }
        _ => {
            let (x, y) = (b.max.x + GAP, beside(dir.x < 0.0));
            (place(p(x, y), Anchor::Left), place(p(x + gap, y), Anchor::Left))
        }
    }
}

/// The cross reference of an interruption point: its text, ink and the page of its
/// first counterpart (None for a point without name, or whose reference property
/// field shows the references).
pub fn interruption_xref(prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> Option<(String, Ink, Option<usize>)> {
    let key = s.interruption_key();
    if references::has_field(def, s) || !idx.interruptions.contains_key(&key) {
        return None;
    }
    let others: Vec<Loc> = idx.interruption_targets(prj, &key, s.id);
    if others.is_empty() {
        return Some(("?".to_string(), Ink::Error, None));
    }
    let txt = others.iter().map(|l| l.reference_from(prj, page_i)).collect::<Vec<_>>().join(" ");
    Some((txt, Ink::CrossRef, others.first().map(|l| l.page)))
}

/// The name, drawn with the symbol graphics.
pub(super) fn draw_label(d: &mut dyn Draw, def: &SymbolDef, s: &SymbolInst, ink: Ink) {
    if s.label.is_empty() {
        return;
    }
    let (label, _) = interruption_places(def, s);
    let ink = if ink == Ink::Line { Ink::Text } else { ink };
    d.text(s.pos + label.offset, &s.label, label_size(def), label.anchor, VAnchor::Middle, ink);
}

/// The cross reference, drawn with the annotations.
pub(super) fn draw_xref(d: &mut dyn Draw, prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst) {
    let Some((txt, ink, target)) = interruption_xref(prj, idx, page_i, def, s) else { return };
    let (_, xref) = interruption_places(def, s);
    let q = s.pos + xref.offset;
    d.text(q, &txt, SMALL_SIZE, xref.anchor, VAnchor::Middle, ink);
    if let Some(page) = target {
        link_text(d, q, &txt, SMALL_SIZE, xref.anchor, page);
    }
}

/// Areas of the name and the cross reference, to pick and drag them.
pub fn interruption_text_bounds(prj: &Project, idx: &Index, page_i: usize, def: &SymbolDef, s: &SymbolInst) -> Vec<(IntrText, R)> {
    if def.func != Func::Interruption {
        return Vec::new();
    }
    let (label, xref) = interruption_places(def, s);
    let mut v = Vec::new();
    if !s.label.is_empty() {
        v.push((IntrText::Label, text_area(s.pos + label.offset, &s.label, label_size(def), label.anchor)));
    }
    if let Some((txt, _, _)) = interruption_xref(prj, idx, page_i, def, s) {
        v.push((IntrText::Xref, text_area(s.pos + xref.offset, &txt, SMALL_SIZE, xref.anchor)));
    }
    v
}

fn text_area(q: P, text: &str, size: f32, anchor: Anchor) -> R {
    let w = name_width(text, size).max(size);
    let x0 = match anchor {
        Anchor::Left => q.x,
        Anchor::Center => q.x - w / 2.0,
        Anchor::Right => q.x - w,
    };
    R::from_points(p(x0, q.y - size * 0.7), p(x0 + w, q.y + size * 0.7))
}

#[cfg(test)]
mod tests;
