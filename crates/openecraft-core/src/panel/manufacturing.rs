//! Manufacturability of the panel layout: holes and cutouts too close to the sheet
//! edge or to each other, rails, ducts and busbars too short to cut or longer than
//! the stock, devices overhanging the end of their rail or rail devices without a
//! rail, and the load on the mounting plate and the door.

use super::checks::{data_number, item_name, CheckKind};
use super::{part_by_number, Enclosure, Face, ItemKind};
use crate::model::Project;
use crate::production::{holes_on, openings, Opening};
use crate::issue::Severity;
use openecraft_i18n::tr_fmt;
use std::collections::BTreeSet;

/// Distance from a point to an opening's outline (negative inside).
fn opening_distance(o: &Opening, q: crate::geom::P) -> f32 {
    if o.round {
        return q.dist(o.center) - o.w / 2.0;
    }
    let l = o.local(q);
    let dx = l.x.abs() - o.w / 2.0;
    let dy = l.y.abs() - o.h / 2.0;
    if dx > 0.0 && dy > 0.0 {
        (dx * dx + dy * dy).sqrt()
    } else {
        dx.max(dy)
    }
}

/// Free web between two openings (approximated by their bounding boxes for round
/// ones against rectangular ones, and for turned ones).
fn opening_gap(a: &Opening, b: &Opening) -> f32 {
    if a.round && b.round {
        return a.center.dist(b.center) - (a.w + b.w) / 2.0;
    }
    let (ra, rb) = (a.bounds(), b.bounds());
    let dx = (ra.center().x - rb.center().x).abs() - (ra.width() + rb.width()) / 2.0;
    let dy = (ra.center().y - rb.center().y).abs() - (ra.height() + rb.height()) / 2.0;
    if dx > 0.0 && dy > 0.0 {
        (dx * dx + dy * dy).sqrt()
    } else {
        dx.max(dy)
    }
}

/// Weight of the parts of a device, kg.
fn device_weight(prj: &Project, tag: &str) -> f32 {
    prj.devices
        .get(tag)
        .map(|d| d.parts.iter().filter_map(|r| part_by_number(prj, &r.number).map(|p| p.weight * r.quantity() as f32)).sum())
        .unwrap_or(0.0)
}

pub(super) fn check(prj: &Project, e: &Enclosure, push: &mut impl FnMut(CheckKind, Severity, Vec<u64>, String)) {
    let rules = &prj.layout.checks;
    let name = |id: u64| item_name(e, id);
    let mm = |v: f32| format!("{v:.1}");
    let m = CheckKind::Manufacturing;
    for face in Face::ALL {
        if !e.items.iter().any(|i| i.face == face) {
            continue;
        }
        let (fw, fh) = e.face_size(face);
        let holes = holes_on(e, &prj.parts, face);
        let opens = openings(e, &prj.parts, face);
        let edge = |q: crate::geom::P| q.x.min(q.y).min(fw - q.x).min(fh - q.y);
        let mut reported = BTreeSet::new();
        for (i, h) in holes.iter().enumerate() {
            let d = edge(h.pos) - h.diameter / 2.0;
            if d < rules.hole_edge - 0.01 && reported.insert((h.item, h.item)) {
                push(
                    m,
                    Severity::Warning,
                    vec![h.item],
                    tr_fmt!("{}: hole Ø{} of {} on the {} is {} mm from the edge, at least {} mm required", e.name, mm(h.diameter), name(h.item), face.title(), mm(d.max(0.0)), mm(rules.hole_edge)),
                );
            }
            for g in &holes[i + 1..] {
                if g.item == h.item {
                    continue;
                }
                let dist = g.pos.dist(h.pos);
                // Items fixed with the same screw share the hole.
                if dist < 0.5 && (g.diameter - h.diameter).abs() < 0.01 {
                    continue;
                }
                let web = dist - (g.diameter + h.diameter) / 2.0;
                let key = (h.item.min(g.item), h.item.max(g.item));
                if web < rules.hole_web - 0.01 && reported.insert(key) {
                    push(
                        m,
                        Severity::Warning,
                        vec![h.item, g.item],
                        tr_fmt!("{}: holes of {} and {} on the {} are too close ({} mm web, at least {} mm)", e.name, name(h.item), name(g.item), face.title(), mm(web.max(0.0)), mm(rules.hole_web)),
                    );
                }
            }
            for o in opens.iter().filter(|o| o.item != h.item) {
                let web = opening_distance(o, h.pos) - h.diameter / 2.0;
                let key = (h.item.min(o.item), h.item.max(o.item));
                if web < rules.hole_web - 0.01 && reported.insert(key) {
                    push(
                        m,
                        Severity::Warning,
                        vec![h.item, o.item],
                        tr_fmt!("{}: hole of {} is too close to the cutout of {} on the {}", e.name, name(h.item), name(o.item), face.title()),
                    );
                }
            }
        }
        // The free edge of the door and the walls (frame, reinforcement, hinges) takes no
        // cutout either.
        let edge_web = if matches!(face, Face::Plate | Face::Bottom | Face::Roof) { rules.cutout_web } else { rules.cutout_web.max(e.flange()) };
        for (i, o) in opens.iter().enumerate() {
            let d = if o.round {
                edge(o.center) - o.w / 2.0
            } else {
                let b = o.bounds();
                b.min.x.min(b.min.y).min(fw - b.max.x).min(fh - b.max.y)
            };
            if d < edge_web - 0.01 {
                push(
                    m,
                    Severity::Warning,
                    vec![o.item],
                    tr_fmt!("{}: cutout of {} on the {} is {} mm from the edge, at least {} mm required", e.name, name(o.item), face.title(), mm(d.max(0.0)), mm(edge_web)),
                );
            }
            // The front of a device or accessory covers its cutout.
            if let Some(it) = e.item(o.item).filter(|it| matches!(it.kind, ItemKind::Device | ItemKind::Accessory)) {
                let b = o.bounds();
                let beyond = (it.x - b.min.x).max(it.y - b.min.y).max(b.max.x - (it.x + it.w)).max(b.max.y - (it.y + it.h));
                if beyond > 0.05 {
                    push(
                        m,
                        Severity::Warning,
                        vec![o.item],
                        tr_fmt!("{}: cutout of {} on the {} reaches {} mm beyond its front", e.name, name(o.item), face.title(), mm(beyond)),
                    );
                }
            }
            for q in opens[i + 1..].iter().filter(|q| q.item != o.item) {
                let web = opening_gap(o, q);
                if web < rules.cutout_web - 0.01 {
                    push(
                        m,
                        Severity::Warning,
                        vec![o.item, q.item],
                        tr_fmt!("{}: cutouts of {} and {} on the {} are too close ({} mm web, at least {} mm)", e.name, name(o.item), name(q.item), face.title(), mm(web.max(0.0)), mm(rules.cutout_web)),
                    );
                }
            }
        }
    }
    for it in e.items.iter().filter(|i| i.kind.is_profile()) {
        let len = it.length();
        if rules.min_profile > 0.0 && len < rules.min_profile - 0.01 {
            push(m, Severity::Warning, vec![it.id], tr_fmt!("{}: {} of {} mm is shorter than {} mm and hard to cut", e.name, it.kind.title(), mm(len), mm(rules.min_profile)));
        }
        let stock = part_by_number(prj, &it.part).and_then(|p| data_number(p, "Stock length")).unwrap_or(rules.stock_length);
        if stock > 0.0 && len > stock + 0.01 {
            push(
                m,
                Severity::Info,
                vec![it.id],
                tr_fmt!("{}: {} of {} mm is longer than the stock length of {} mm and has to be joined", e.name, it.kind.title(), mm(len), mm(stock)),
            );
        }
    }
    // Devices on rails: within the rail; rail devices need one.
    for it in e.items.iter().filter(|i| i.kind == ItemKind::Device && i.face == Face::Plate) {
        let anchor = it.rail_anchor();
        // Older layouts can record only a raised device and an approximate rail profile.
        let rail = e.items.iter().find(|r| it.seated_on(r)).or_else(|| {
            e.items.iter().find(|r| {
                it.z > 0. && r.kind == ItemKind::Rail && r.face == it.face && r.horizontal()
                    && (r.x..=r.x + r.w).contains(&anchor.x) && (anchor.y - r.center().y).abs() <= it.h / 2.
            })
        });
        match rail {
            Some(r) if it.x < r.x - 0.01 || it.x + it.w > r.x + r.w + 0.01 => {
                push(m, Severity::Warning, vec![it.id, r.id], tr_fmt!("{}: {} overhangs the end of its mounting rail", e.name, name(it.id)));
            }
            Some(_) => {}
            None => {
                let on_rail = part_by_number(prj, &it.part).is_some_and(|p| p.mounting.to_lowercase().contains("rail"));
                if on_rail && it.z <= 0.01 {
                    push(m, Severity::Warning, vec![it.id], tr_fmt!("{}: {} is made for a mounting rail but sits directly on the plate", e.name, name(it.id)));
                }
            }
        }
    }
    // Load on the mounting plate and the door (enclosure part data `Plate load: 60 kg`).
    let enclosure = part_by_number(prj, &e.part);
    for (face, key) in [(Face::Plate, "Plate load"), (Face::Door, "Door load")] {
        let Some(limit) = enclosure.and_then(|p| data_number(p, key)).filter(|v| *v > 0.0) else { continue };
        let load: f32 = e
            .items
            .iter()
            .filter(|i| i.face == face)
            .map(|i| if i.kind == ItemKind::Device { device_weight(prj, &i.tag) } else { part_by_number(prj, &i.part).map(|p| p.weight).unwrap_or(0.0) })
            .sum();
        if load > limit + 0.001 {
            push(m, Severity::Warning, Vec::new(), tr_fmt!("{}: {} kg on the {} exceed its load of {} kg", e.name, mm(load), face.title(), mm(limit)));
        }
    }
}

#[cfg(test)]
mod tests;
