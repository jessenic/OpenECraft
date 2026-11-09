//! Enclosure systems (compact, floor-standing baying, terminal box, plastic, house
//! connection box, 19-inch rack) and the
//! components an enclosure is assembled from: body or frame, door, walls, side panels,
//! rear wall, gland plate, plinth and baying kits, each with its part and its
//! geometry for the 3D view and the export.

use super::{Enclosure, Face, ItemKind, PLATE_THICKNESS};
use crate::climate::Material;
use crate::parts::mechanics::{self, SystemData};
use crate::parts::vec3::V3;
use openecraft_i18n::tr;
pub use crate::model::panel::Build;

/// An enclosure system of the catalog.
#[derive(Debug, PartialEq)]
pub struct System {
    pub data: &'static SystemData,
}

pub static SYSTEMS: [System; 6] = [
    System { data: &mechanics::SYSTEMS[0] },
    System { data: &mechanics::SYSTEMS[1] },
    System { data: &mechanics::SYSTEMS[2] },
    System { data: &mechanics::SYSTEMS[3] },
    System { data: &mechanics::SYSTEMS[4] },
    System { data: &mechanics::SYSTEMS[5] },
];

impl System {
    pub fn id(&self) -> &'static str {
        self.data.id
    }

    pub fn title(&self) -> &'static str {
        match self.data.id {
            "compact" => tr("Compact enclosure (wall-mounted)"),
            "baying" => tr("Floor-standing baying system"),
            "terminal_box" => tr("Terminal box"),
            "plastic" => tr("Plastic enclosure"),
            mechanics::HOUSE_CONNECTION_SYSTEM => tr("House connection box"),
            mechanics::RACK_SYSTEM => tr("19-inch rack"),
            other => other,
        }
    }

    pub fn material(&self) -> Material {
        match self.data.material {
            "stainless_steel" => Material::StainlessSteel,
            "aluminium" => Material::Aluminium,
            "plastic" => Material::Plastic,
            _ => Material::SheetSteel,
        }
    }
}

pub fn by_id(id: &str) -> Option<&'static System> {
    SYSTEMS.iter().find(|s| s.data.id == id)
}

impl Build {
    pub fn is_default(&self) -> bool {
        *self == Build::default()
    }
}

/// What a component of an enclosure is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Monobloc body or the frame of a floor-standing system.
    Body,
    Door,
    /// A wall (side panel, rear wall, roof, bottom).
    Wall(Face),
    GlandPlate,
    MountingPlate,
    Plinth,
    BayingKit,
    /// The 19-inch mounting rails of a rack.
    RackRails,
    /// A wall or the ceiling (`Roof`) of a room.
    RoomWall(Face),
}

impl Role {
    pub fn title(self) -> &'static str {
        match self {
            Role::Body => tr("Enclosure body"),
            Role::Door => tr("Door"),
            Role::Wall(Face::Left) => tr("Left side panel"),
            Role::Wall(Face::Right) => tr("Right side panel"),
            Role::Wall(Face::Rear) => tr("Rear wall"),
            Role::Wall(Face::Roof) => tr("Roof"),
            Role::Wall(_) => tr("Bottom"),
            Role::GlandPlate => tr("Gland plate"),
            Role::MountingPlate => tr("Mounting plate"),
            Role::Plinth => tr("Plinth"),
            Role::BayingKit => tr("Baying kit"),
            Role::RackRails => tr("19-inch mounting rails"),
            Role::RoomWall(Face::Roof) => tr("Ceiling"),
            Role::RoomWall(Face::Inner(..)) => tr("Inner wall"),
            Role::RoomWall(_) => tr("Wall"),
        }
    }
}

/// A component of an enclosure: its part (empty if it is part of the body), how many
/// and its boxes in enclosure coordinates (y down; the plinth is below `height`).
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub role: Role,
    pub part: String,
    pub count: u32,
    pub boxes: Vec<(V3, V3)>,
}

/// Box of a wall of the enclosure (sheet thickness `t`).
pub fn wall_box(e: &Enclosure, face: Face) -> (V3, V3) {
    if e.is_room() {
        return super::rooms::wall_box(e, face);
    }
    let (w, h, d, t) = (e.width, e.height, e.depth, e.wall());
    match face {
        Face::Left => ([0.0, 0.0, 0.0], [t, h, d]),
        Face::Right => ([w - t, 0.0, 0.0], [w, h, d]),
        Face::Roof | Face::RoofTop => ([0.0, 0.0, 0.0], [w, t, d]),
        Face::Bottom => ([0.0, h - t, 0.0], [w, h, d]),
        Face::Door => ([0.0, 0.0, d], [w, h, d + t]),
        Face::Rear | Face::Plate | Face::Wall(_) | Face::Inner(..) | Face::Outside(_) => ([0.0, 0.0, 0.0], [w, h, t]),
    }
}

/// Direction out of the enclosure (or room) through a wall, the door or the ceiling.
pub fn outward(e: &Enclosure, face: Face) -> V3 {
    super::face_frame(e, face)[3].map(|v| -v)
}

/// The frame of a floor-standing system: four posts and the rails joining them at
/// the top and the bottom.
fn frame_boxes(e: &Enclosure, f: f32) -> Vec<(V3, V3)> {
    let (w, h, d) = (e.width, e.height, e.depth);
    let mut v = Vec::new();
    for x in [0.0, w - f] {
        for z in [0.0, d - f] {
            v.push(([x, 0.0, z], [x + f, h, z + f]));
        }
    }
    for y in [0.0, h - f] {
        for z in [0.0, d - f] {
            v.push(([f, y, z], [w - f, y + f, z + f]));
        }
        for x in [0.0, w - f] {
            v.push(([x, y, f], [x + f, y + f, d - f]));
        }
    }
    v
}

/// The components of an enclosure. Monobloc enclosures are one part (body with walls,
/// door and mounting plate); floor-standing systems are a frame with door, roof and
/// mounting plate plus side panels (or baying kits on bayed sides) and a rear wall.
pub fn components(e: &Enclosure) -> Vec<Component> {
    if e.is_assembly() {return Vec::new();}
    let (w, h, d) = (e.width, e.height, e.depth);
    if e.is_room() {
        // Room walls stand on the floor of the site, the ceiling covers the floor plan.
        let mut v: Vec<Component> = super::room_faces(e)
            .into_iter()
            .filter(|f| matches!(f, Face::Wall(_) | Face::Inner(_, 0)))
            .map(|f| Component { role: Role::RoomWall(f), part: String::new(), count: 1, boxes: vec![wall_box(e, f)] })
            .collect();
        v.push(Component { role: Role::RoomWall(Face::Roof), part: String::new(), count: 1, boxes: super::rooms::floor_cells(e) });
        return v;
    }
    let sys = e.system();
    let modular = sys.is_some_and(|s| s.data.bayable);
    let body = match sys {
        Some(s) => mechanics::body_number(s.data, w, h, d),
        None => e.part.clone(),
    };
    let one = |role, part: String, boxes| Component { role, part, count: 1, boxes };
    let frame = if modular { frame_boxes(e, sys.map(|s| s.data.frame).unwrap_or(0.0)) } else { Vec::new() };
    let mut v = vec![one(Role::Body, body, frame), one(Role::Door, String::new(), vec![wall_box(e, Face::Door)])];
    for (face, open) in [(Face::Left, e.build.left_open), (Face::Right, e.build.right_open)] {
        if !open {
            let part = sys.filter(|_| modular).map(|s| mechanics::side_number(s.data, h, d)).unwrap_or_default();
            v.push(one(Role::Wall(face), part, vec![wall_box(e, face)]));
        }
    }
    let rear = sys.filter(|_| modular).map(|s| mechanics::rear_number(s.data, w, h)).unwrap_or_default();
    v.push(one(Role::Wall(Face::Rear), rear, vec![wall_box(e, Face::Rear)]));
    v.push(one(Role::Wall(Face::Roof), String::new(), vec![wall_box(e, Face::Roof)]));
    if e.build.gland_plate {
        v.push(one(Role::GlandPlate, mechanics::gland_plate_number(w, d), vec![wall_box(e, Face::Bottom)]));
    } else {
        v.push(one(Role::Wall(Face::Bottom), String::new(), vec![wall_box(e, Face::Bottom)]));
    }
    if super::is_rack(e) {
        // Two pairs of rails instead of a mounting plate.
        let part = mechanics::rack_rail_number(super::rack_units(e));
        v.push(Component { role: Role::RackRails, part, count: 2, boxes: super::rack::rail_boxes(e) });
    } else if !e.no_implicit_plate && !e.items.iter().any(|i|i.kind==ItemKind::MountingPlate) {
        let (pw, ph) = e.plate_size();
        let offset=e.face_offset(Face::Plate);let z=e.mounting_z();
        let plate = ([offset.x,offset.y,z-PLATE_THICKNESS], [offset.x+pw,offset.y+ph,z]);
        v.push(one(Role::MountingPlate, String::new(), vec![plate]));
    }
    let p = e.build.plinth;
    if p > 0.0 {
        let s = 2.0;
        let boxes = vec![
            ([0.0, h, d - s], [w, h + p, d]),
            ([0.0, h, 0.0], [w, h + p, s]),
            ([0.0, h, s], [s, h + p, d - s]),
            ([w - s, h, s], [w, h + p, d - s]),
        ];
        let part = if e.build.plinth_included { String::new() } else { mechanics::plinth_number(w, d, p) };
        v.push(one(Role::Plinth, part, boxes));
    }
    let bayed = e.build.left_open as u32 + e.build.right_open as u32;
    if bayed > 0 {
        v.push(Component { role: Role::BayingKit, part: mechanics::BAYING_KIT.into(), count: bayed, boxes: Vec::new() });
    }
    v
}

/// Turns an enclosure into one of a system: its body part, material, plinth and (if
/// given) one of its sizes.
pub fn apply_system(e: &mut Enclosure, system: &str, size: Option<(f32, f32, f32)>) {
    let Some(s) = by_id(system) else {
        e.build.system.clear();
        return;
    };
    if let Some((w, h, d)) = size {
        (e.width, e.height, e.depth) = (w, h, d);
    }
    if e.build.system != system {
        e.build.plinth = s.data.plinth;
        if !s.data.bayable {
            e.build.left_open = false;
            e.build.right_open = false;
        }
    }
    e.build.system = system.into();
    e.climate.material = s.material();
    sync_part(e);
}

/// Keeps the part of a system enclosure in step with its size.
pub fn sync_part(e: &mut Enclosure) {
    if let Some(s) = e.system() {
        e.part = mechanics::body_number(s.data, e.width, e.height, e.depth);
    }
}

/// The system of an enclosure part (part data `Enclosure system: baying`).
pub fn system_of_part(part: &crate::parts::Part) -> Option<&'static System> {
    by_id(part.tech_value("Enclosure system")?.trim())
}

#[cfg(test)]
mod tests;
