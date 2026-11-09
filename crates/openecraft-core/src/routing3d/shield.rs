//! Shields of cables: which cables are shielded, how the shield is terminated at each
//! end (on a shield clamp rail, with a pigtail at the device, outside the panel, or
//! not at all), and the problems that follow from it.

use super::{Bundle, Class, Level, Problem, RouteKind, RoutedItem};
use crate::model::{Cable, Project};
use crate::parts::vec3::manhattan;
use openecraft_i18n::{tr, tr_fmt};

/// The cable has an overall shield: as set, otherwise bus cables and cable types
/// that name a shield (`CY`, `SY`, `(St)`, `C11Y`, "shield").
pub fn is_shielded(c: &Cable) -> bool {
    c.shielded.unwrap_or_else(|| {
        let t = c.cable_type.to_ascii_uppercase();
        c.bus.is_some() || ["CY", "SY", "(ST)", "C11Y", "SHIELD", "SCREEN"].iter().any(|m| t.contains(m))
    })
}

/// How the shield is terminated at an end.
#[derive(Clone, Debug, PartialEq)]
pub enum Termination {
    /// On a shield clamp rail (item id), at this distance from where the cable ends.
    Clamp { rail: u64, distance: f32 },
    /// With a pigtail of this length at the device.
    Pigtail(f32),
    /// Outside the panel (field device).
    Field,
    /// Not connected at this end.
    Open,
}

impl Termination {
    pub fn text(&self) -> String {
        match self {
            Termination::Clamp { distance, .. } => tr_fmt!("shield clamp ({} mm away)", format!("{distance:.0}")),
            Termination::Pigtail(l) => tr_fmt!("pigtail {} mm", format!("{l:.0}")),
            Termination::Field => tr("in the field").to_string(),
            Termination::Open => tr("not connected").to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShieldEnd {
    pub tag: String,
    pub termination: Termination,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShieldInfo {
    pub cable: String,
    pub bus: bool,
    pub ends: [ShieldEnd; 2],
}

/// Shield terminations of the routed shielded cables, and their problems: bus cables
/// whose shield is not connected at both ends or only with a pigtail, and shielded
/// cables bundled with power conductors.
pub fn analyse(prj: &Project, items: &[RoutedItem], bundles: &[Bundle]) -> (Vec<ShieldInfo>, Vec<Problem>) {
    let pos = crate::panel::positions(&prj.layout);
    let mut infos = Vec::new();
    let mut problems = Vec::new();
    for (idx, it) in items.iter().enumerate().filter(|(_, i)| i.kind == RouteKind::Cable && i.shielded) {
        let Some(c) = prj.cable(&it.key) else { continue };
        let ends = [0, 1].map(|k| {
            let end = &it.ends[k];
            let termination = match (end.enclosure, end.point) {
                _ if !c.shield_ends.connects(k) => Termination::Open,
                // A cable from the field ends at its gland: the shield is terminated at
                // the field device.
                (Some(_), Some(_)) if end.item.is_some_and(|id| !prj.layout.enclosures.iter().flat_map(|e|&e.items).any(|d|d.id==id && d.kind==crate::panel::ItemKind::Device && d.tag==end.tag)) => Termination::Field,
                (Some(ei), Some(at)) => {
                    let e = &prj.layout.enclosures[ei];
                    let rail = super::accessories(prj, ei, "Cable clamp rail").into_iter().map(|r| {
                        let c = crate::panel::face_point(e, r.face, [r.x + r.w / 2.0, r.y + r.h / 2.0, r.z + r.d]);
                        (r.id, manhattan([c[0] + pos[ei][0], c[1] + pos[ei][1], c[2] + pos[ei][2]], at))
                    });
                    match rail.min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0))) {
                        Some((id, distance)) => Termination::Clamp { rail: id, distance },
                        None => Termination::Pigtail(prj.layout.routing.pigtail),
                    }
                }
                _ => Termination::Field,
            };
            ShieldEnd { tag: end.tag.clone(), termination }
        });
        let bus = c.bus.is_some();
        if bus && !c.shield_ends.is_both() {
            problems.push(Problem { level: Level::Warning, message: tr_fmt!("Shield of bus cable {} is not connected at both ends", c.tag), target: None });
        }
        for e in &ends {
            if let (true, Termination::Pigtail(_)) = (bus, &e.termination) {
                problems.push(Problem { level: Level::Warning, message: tr_fmt!("Shield of {} is connected with a pigtail at {}: use a shield clamp", c.tag, e.tag), target: None });
            }
        }
        let power = bundles.iter().filter(|b| b.items.contains(&idx) && b.items.iter().any(|&i| items[i].class == Class::Power)).map(|b| b.length).sum::<f32>();
        if power > 0.0 {
            problems.push(Problem { level: Level::Warning, message: tr_fmt!("Shielded cable {} runs together with power conductors over {} mm", c.tag, format!("{power:.0}")), target: None });
        }
        infos.push(ShieldInfo { cable: c.tag.clone(), bus, ends });
    }
    (infos, problems)
}

#[cfg(test)]
mod tests;
