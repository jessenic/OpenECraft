//! Fastening systems of the panel items: how an item is fixed to its face decides the
//! holes the NC data gets (tap drill, clearance or rivet nut hole, weld studs, none)
//! and the hardware in the bill of materials.

use super::{Face, ItemKind, PanelItem};
use crate::model::Project;
use crate::parts::mechanics::{nut_number, rivet_nut_number, screw_number, thread, washer_number};
use crate::parts::Part;
use openecraft_i18n::tr;
pub use crate::model::panel::Fastening;

impl Fastening {
    pub const ALL: [Fastening; 7] =
        [Fastening::Auto, Fastening::Tapped, Fastening::Through, Fastening::RivetNut, Fastening::Stud, Fastening::Clip, Fastening::Adhesive];

    pub fn title(self) -> &'static str {
        match self {
            Fastening::Auto => tr("Automatic"),
            Fastening::Tapped => tr("Tapped hole"),
            Fastening::Through => tr("Through hole with nut"),
            Fastening::RivetNut => tr("Rivet nut"),
            Fastening::Stud => tr("Weld stud"),
            Fastening::Clip => tr("Clip / snap-on"),
            Fastening::Adhesive => tr("Adhesive"),
        }
    }

    /// Key in the part data (`Fastening: Rivet nut`) and on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Fastening::Auto => "auto",
            Fastening::Tapped => "tapped",
            Fastening::Through => "through",
            Fastening::RivetNut => "rivet_nut",
            Fastening::Stud => "stud",
            Fastening::Clip => "clip",
            Fastening::Adhesive => "adhesive",
        }
    }

    pub fn from_key(s: &str) -> Option<Fastening> {
        let k = s.trim().to_lowercase().replace([' ', '-'], "_");
        Fastening::ALL.into_iter().find(|f| f.key() == k)
    }

    pub fn is_auto(&self) -> bool {
        *self == Fastening::Auto
    }

    /// The fastening actually used: the item's own, the part data `Fastening`, or by
    /// kind and face: devices on rails and on the door clip on, door ducts are glued,
    /// everything else is screwed into tapped holes on the plate and through clearance
    /// holes in the thin sheet of the walls and the door.
    pub fn resolve(it: &PanelItem, part: Option<&Part>) -> Fastening {
        if it.fastening != Fastening::Auto {
            return it.fastening;
        }
        if let Some(f) = part.and_then(|p| p.data_value("Fastening")).and_then(|v| Fastening::from_key(&v)).filter(|f| *f != Fastening::Auto) {
            return f;
        }
        match (it.kind, it.face) {
            (ItemKind::Cutout, _) => Fastening::Clip,
            (ItemKind::Device, Face::Plate) if it.seated_on_rail || it.z > 0.01 => Fastening::Clip,
            (ItemKind::Device, Face::Door) => Fastening::Clip,
            (ItemKind::Duct, Face::Door) => Fastening::Adhesive,
            (_, Face::Plate) => Fastening::Tapped,
            _ => Fastening::Through,
        }
    }

    /// The fixing points need a hole (or a stud) in the face.
    pub fn has_points(self) -> bool {
        matches!(self, Fastening::Tapped | Fastening::Through | Fastening::RivetNut | Fastening::Stud)
    }

    /// Diameter of the hole for a thread ("M5"): tap drill, clearance hole (ISO 273
    /// medium), rivet nut hole; the stud's nominal diameter for studs.
    pub fn hole_diameter(self, thread_spec: &str) -> Option<f32> {
        let nominal: f32 = thread(thread_spec)?[1..].parse().ok()?;
        Some(match self {
            Fastening::Through => match nominal as u32 {
                3 => 3.4,
                4 => 4.5,
                5 => 5.5,
                6 => 6.6,
                8 => 9.0,
                _ => nominal * 1.1,
            },
            Fastening::RivetNut => match nominal as u32 {
                3 => 5.0,
                4 => 6.0,
                5 => 7.0,
                6 => 9.0,
                8 => 11.0,
                _ => nominal * 1.4,
            },
            Fastening::Stud => nominal,
            _ => crate::production::drill_diameter(thread_spec)?,
        })
    }

    /// Operation of a fixing point in the NC lists (machine data, not translated).
    pub fn operation(self) -> &'static str {
        match self {
            Fastening::Tapped => "Drill + tap",
            Fastening::RivetNut => "Drill + rivet nut",
            Fastening::Stud => "Weld stud",
            _ => "Drill",
        }
    }

    /// Hardware per fixing point: (part number, count).
    pub fn hardware(self, thread_spec: &str) -> Vec<(String, u32)> {
        let Some(t) = thread(thread_spec) else { return Vec::new() };
        match self {
            Fastening::Tapped => vec![(screw_number(t), 1)],
            Fastening::Through => vec![(screw_number(t), 1), (nut_number(t), 1), (washer_number(t), 2)],
            Fastening::RivetNut => vec![(rivet_nut_number(t), 1), (screw_number(t), 1)],
            Fastening::Stud => vec![(nut_number(t), 1), (washer_number(t), 1)],
            _ => Vec::new(),
        }
    }
}

/// Screws, nuts, washers and rivet nuts of all fixing points of the layout:
/// (part number, count, enclosures).
pub fn fastening_material(prj: &Project) -> Vec<(String, u32, Vec<String>)> {
    let mut out: Vec<(String, u32, Vec<String>)> = Vec::new();
    for e in &prj.layout.enclosures {
        for face in Face::ALL {
            for h in crate::production::holes_on(e, &prj.parts, face) {
                for (part, n) in h.fastening.hardware(&h.thread_size) {
                    match out.iter_mut().find(|x| x.0 == part) {
                        Some(x) => {
                            x.1 += n;
                            if !x.2.contains(&e.name) {
                                x.2.push(e.name.clone());
                            }
                        }
                        None => out.push((part, n, vec![e.name.clone()])),
                    }
                }
            }
        }
    }
    out.sort_by(|a, b| crate::model::natural_cmp(&a.0, &b.0));
    out
}

#[cfg(test)]
mod tests;
