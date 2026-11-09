//! Structure boxes: rectangles on a schematic page whose devices take other structure
//! identifiers (`=` function, `+` location …) than the page (see
//! [`crate::structure::scope`]).
//!
//! Tags stay stored complete. A device takes the structure of the place of its main
//! function: an aspect of its tag equal to the structure in effect there is
//! inherited, any other aspect is the device's own and stays. When a main function
//! moves into or out of a box, or a box is moved, resized, changed or deleted,
//! [`follow`] renames the device (all its functions and data) so that its inherited
//! aspects match the new place; a tag that exists already is refused and reported
//! instead of merging two devices.

use openecraft_i18n::tr_fmt;
use crate::geom::{p, P, R};
use crate::issue::{Issue, Severity};
use crate::model::{Page, Project};
use crate::structure::{self, Aspect, Dt, StructureBox};
use crate::symbols::Func;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Value of `aspect` set by the innermost box at `q` (None: no box sets it).
fn box_value(page: &Page, q: P, aspect: Aspect) -> Option<String> {
    structure::box_for(page, q, aspect).map(|b| b.structure.get(aspect).to_string())
}

/// The tag a device placed at `old_pos` on `old` gets at `new_pos` on `new`: aspects
/// whose structure box changed and that were inherited take the new value. Changes
/// of the page structure itself are left to `ops::set_page_structure`.
pub fn moved_tag(old: &Page, old_pos: P, new: &Page, new_pos: P, tag: &str) -> String {
    let (before, after) = (structure::scope(old, old_pos), structure::scope(new, new_pos));
    let mut dt = Dt::parse(tag);
    for a in Aspect::ALL {
        let (was, now) = (box_value(old, old_pos, a), box_value(new, new_pos, a));
        if (was.is_some() || now.is_some()) && was != now && dt.get(a) == before.get(a) {
            *dt.get_mut(a) = after.get(a).to_string();
        }
    }
    if dt == Dt::parse(tag) { tag.to_string() } else { dt.tag() }
}

/// Lets the devices follow the structure boxes after a change from `before` to `prj`
/// (one undo step): main functions that kept their tag but now lie in another
/// structure scope get their device renamed. Returns the devices that could not
/// follow, with the reason.
pub fn follow(prj: &mut Project, before: &Project) -> Vec<String> {
    // Called after every edit in the editor: without boxes there is nothing to do.
    let boxed: HashMap<u64, &Page> = before.pages.iter().filter(|p| !p.structure_boxes.is_empty()).map(|p| (p.id, p)).collect();
    if boxed.is_empty() && prj.pages.iter().all(|p| p.structure_boxes.is_empty()) {
        return Vec::new();
    }
    let mut moved: Vec<(String, String, String)> = Vec::new();
    for page in &prj.pages {
        let old = match boxed.get(&page.id) {
            Some(old) => *old,
            None if page.structure_boxes.is_empty() => continue,
            None => match before.pages.iter().find(|o| o.id == page.id) {
                Some(old) => old,
                None => continue,
            },
        };
        if page == old {
            continue;
        }
        let was: HashMap<u64, _> = old.symbols.iter().map(|s| (s.id, s)).collect();
        for s in &page.symbols {
            let Some(o) = was.get(&s.id).filter(|o| !s.tag.is_empty() && o.tag == s.tag) else { continue };
            let to = moved_tag(old, o.pos, page, s.pos, &s.tag);
            if to != s.tag {
                moved.push((s.sym.clone(), s.tag.clone(), to));
            }
        }
    }
    if moved.is_empty() {
        return Vec::new();
    }
    // Only main functions decide the structure of their device.
    let symbols = prj.symbol_table();
    let mut wanted: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (sym, from, to) in moved {
        if matches!(symbols.get(&sym).map(|d| d.func), Some(Func::Main { .. })) {
            wanted.entry(from).or_default().insert(to);
        }
    }
    let mut problems = Vec::new();
    let mut existing = structure::device_tags(prj);
    for (from, targets) in wanted {
        if targets.len() > 1 {
            let list: Vec<&str> = targets.iter().map(String::as_str).collect();
            problems.push(tr_fmt!("{} keeps its structure: its main functions lie in different structure boxes ({})", from, list.join(", ")));
            continue;
        }
        let Some(to) = targets.into_iter().next() else { continue };
        if existing.contains(&to) {
            problems.push(tr_fmt!("{} keeps its structure: {} exists already", from, to));
            continue;
        }
        if let Some(e) = crate::ops::rename_problem(prj, &from, &to) {
            problems.push(e);
            continue;
        }
        crate::ops::rename_device(prj, &from, &to);
        existing.remove(&from);
        existing.insert(to);
    }
    problems
}

/// Structure boxes that overlap without one lying inside the other (their common
/// area has no clear structure), boxes that set no identifier, and devices inside a
/// box whose tag has another value for an aspect the box sets.
pub fn problems(prj: &Project) -> Vec<Issue> {
    let mut v = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        for (a, _) in structure::overlapping(pg) {
            v.push(crate::issue!(Severity::Warning, Some((pi, a)), "Structure boxes on page {} overlap without one lying inside the other: devices in both take the structure of the smaller one", pg.name));
        }
        for b in pg.structure_boxes.iter().filter(|b| b.identifiers().is_empty()) {
            v.push(crate::issue!(Severity::Info, Some((pi, b.id)), "Structure box on page {} sets no structure identifier", pg.name));
        }
        if pg.structure_boxes.is_empty() {
            continue;
        }
        for s in pg.symbols.iter().filter(|s| !s.tag.is_empty() && !crate::device_boxes::continues_device(prj, pg, s)) {
            let around = structure::boxes_at(pg, s.pos);
            let Some(inner) = around.last() else { continue };
            let (want, have) = (structure::scope(pg, s.pos), Dt::parse(&s.tag));
            if Aspect::ALL.into_iter().any(|a| around.iter().any(|b| !b.structure.get(a).is_empty()) && want.get(a) != have.get(a)) {
                v.push(crate::issue!(Severity::Warning, Some((pi, s.id)), "{} is inside the structure box {} but has another structure", s.tag, inner.identifiers()));
            }
        }
    }
    v
}

/// A new structure box with the corners `a` and `b` (in any order).
pub fn new_box(prj: &mut Project, a: P, b: P, structure: Dt, description: &str) -> StructureBox {
    let r = R::from_points(a, b);
    StructureBox { id: prj.new_id(), a: r.min, b: r.max, structure: clean(structure), description: description.to_string(), ..Default::default() }
}

/// The aspects of a box without prefix signs and blanks, without a device name.
pub fn clean(dt: Dt) -> Dt {
    let mut out = Dt::default();
    for a in Aspect::ALL {
        *out.get_mut(a) = structure::clean_aspect(dt.get(a));
    }
    out
}

/// Lines of the box label: the identifiers it sets and the description.
pub fn label_lines(b: &StructureBox) -> Vec<String> {
    [b.identifiers(), b.description.clone()].into_iter().filter(|l| !l.is_empty()).collect()
}

/// Where the label lines start: inside the chosen corner, shifted by the offset.
pub fn label_pos(b: &StructureBox) -> P {
    let r = b.rect();
    let m = 1.0;
    let corner = match b.label {
        structure::BoxCorner::TopLeft => p(r.min.x + m, r.min.y + m),
        structure::BoxCorner::TopRight => p(r.max.x - m, r.min.y + m),
        structure::BoxCorner::BottomLeft => p(r.min.x + m, r.max.y - m),
        structure::BoxCorner::BottomRight => p(r.max.x - m, r.max.y - m),
    };
    corner + b.label_offset
}

/// Distance of `q` from the outline of the box.
pub fn outline_distance(b: &StructureBox, q: P) -> f32 {
    let r = b.rect();
    let corners = [r.min, p(r.max.x, r.min.y), r.max, p(r.min.x, r.max.y)];
    (0..4).map(|i| crate::geom::dist_to_segment(q, corners[i], corners[(i + 1) % 4])).fold(f32::INFINITY, f32::min)
}

#[cfg(test)]
mod tests;
