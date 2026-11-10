//! Panel layout (in the spirit of EPLAN Pro Panel): enclosures with a mounting
//! plate, mounting rails, cable ducts and devices placed with their real dimensions
//! from the parts data, collision checks and virtual wiring through the duct network.
//!
//! Coordinates on the mounting plate are millimetres, origin at the top left corner
//! of the plate, x to the right, y downwards (like schematic pages). `z` is the height
//! above the plate surface. Items on the door (door devices, door cutouts) use the
//! door's front view: origin at the top left corner of the enclosure front, `z` is the
//! distance behind the door. Items on the walls (sides, rear, roof, bottom) use the
//! wall seen from the outside, `z` is the distance from the wall's inside surface
//! into the enclosure (negative: sticking out).

use openecraft_i18n::tr;
use crate::geom::{p, P, R};
use crate::model::Project;
use crate::netlist::Connection;
use crate::parts::vec3::{box_triangles, surfaces_intersect, Tri, V3};
use crate::parts::{Exit, FnKind};
use std::collections::{BTreeSet, HashMap, HashSet};

mod assembly;
pub mod buildings;
pub mod busbars;
mod checks;
mod drives;
mod fastening;
mod fill;
mod lineup;
mod macros;
mod manufacturing;
mod mounting;
mod placement;
pub mod plot;
mod points;
pub mod rack;
pub mod roofs;
pub mod solar;
mod rooms;
pub mod stairs;
mod system;
pub mod thermal;
pub use assembly::{assembly, mounted, Assembly, Piece};
pub use mounting::{can_mount, carry_mounts, mounted_items, mounting_height, remove_items};
pub use checks::{box_gap, class_collisions, class_title, clearances, data_number, device_class, device_kinds, floor_height, heights, item_class, item_name, validate, CheckKind, ClassRule, Clearances, Finding, PanelRules, CLASSES};
pub use drives::{drives, Drive, DriveKind, DRIVE, DRIVEN_BY};
pub use fastening::{fastening_material, Fastening};
pub use fill::{duct_capacity, duct_fill, duct_inside, duct_lanes, duct_lanes_in_order, wire_area, DuctContent, DuctFill, DuctLoads, LANE_GAP, OVERFILL_COST};
pub use lineup::{add_facing_cutout, add_passage, bay_right_of, lineup_of, lineups, neighbours, next_passage_place, passages, positions, route_across, set_site, site_poses, unbay, Leg, Passage, SitePlace, SitePose, LINEUP_GAP, PASSAGE_DIAMETER};
pub use macros::{all_macros, builtin_macros, group_members, insert_macro, macro_from_items, ItemGroup, PanelMacro};
pub use placement::{align, apply_anchors, apply_one_anchor, arrange_on_rail, capture_anchors, distribute, pattern_copy, Align, AlignTo, Anchor};
pub use points::{device_points, exit_vector, nozzles, point_of, PlacedPoint};
pub use rack::{is_rack, mount_in_rack, rack_units, RACK_SYSTEM, RACK_UNIT, RACK_WIDTH};
pub use rooms::{
    add_inner_wall, add_opening, add_pipe_run, add_wall_profile, pipe_run_offset, pipe_run_on_face, PIPE_RUN_WIDTH, inner_sides, moved_inner_wall, remove_inner_wall, side, INNER_WALL, door_zone, opening_sill, set_opening, SLIDE_DEPTH, entries, entry_direction, entry_point, ENTRY_SILL, ENTRY_SIZE, DOOR_SIZE, WINDOW_SILL, WINDOW_SIZE, bounds, cabinet_problems, cabinets_in, check_outline, floor_cells, floor_footprint, floor_problems, footprint, inside_outline, mount_ok, mount_on_wall, moved_wall, new_room, normalize_outline, outline, room_face_from_key, room_face_key, room_face_title, room_faces,
    room_of, room_place, rooms, set_outline, turned_place, set_room, stand_on_floor, at_mounting_height, stands_inside, wall_length_set, walls, Wall, MIN_WALL, ROOM_WALL,
    penetration_link, wall_links, WallLink, Via, MAX_WALL_THICKNESS, PENETRATION_SIZE, PENETRATION_TOP, is_outside, outside_building, through_wall,
};
pub(crate) use rooms::inner_footprint;
pub use system::{apply_system, components, outward, sync_part, system_of_part, wall_box, Build, Component, Role, System, SYSTEMS};
pub use crate::model::panel::{Building, Enclosure, EnclosureKind, Face, GeoReference, InnerWall, ItemKind, Layout, Level, LinkKind, Opening, PanelItem, PanelRotation, Plot, PvArray, PvDesign, PvMounting, PvString, Roof, RoofShape, RoofWindow, Shaft, SiteLink, Stair, StairKind, Swing, Turn};

/// Distance between enclosure wall and mounting plate edge.
pub const PLATE_MARGIN: f32 = 25.0;
/// Distance of the plate surface from the rear wall.
pub const PLATE_Z: f32 = 20.0;
pub const PLATE_THICKNESS: f32 = 3.0;
/// Snap grid on the mounting plate.
pub const SNAP: f32 = 5.0;
/// Mounting rail profile (width × height) if the part has no data.
pub const RAIL_PROFILE: (f32, f32) = (35.0, 7.5);
/// Cable duct profile (width × depth) if the part has no data.
pub const DUCT_PROFILE: (f32, f32) = (40.0, 60.0);
/// Busbar system profile (width × height) if the part has no data.
pub const BUSBAR_PROFILE: (f32, f32) = (75.0, 40.0);
/// Device size if the part has no mounting data.
pub const DEFAULT_DEVICE: (f32, f32, f32) = (40.0, 40.0, 40.0);

pub const RAIL_PART: &str = "GEN.RAIL-TS35-1M";
pub const DUCT_PART: &str = "GEN.DUCT-40X60";
/// Smaller cable duct for doors.
pub const DOOR_DUCT_PART: &str = "GEN.DUCT-25X30";
pub const ENCLOSURE_PART: &str = "GEN.ENC-600X800";
pub const BUSBAR_PART: &str = "GEN.BB-3X12X5";
/// Door frame: door devices keep this distance from the door edges.
pub const DOOR_FRAME: f32 = 20.0;
/// Wire loop across the door hinge.
pub const DOOR_LOOP: f32 = 150.0;

impl Face {
    pub const ALL: [Face; 7] = [Face::Plate, Face::Door, Face::Left, Face::Right, Face::Rear, Face::Roof, Face::Bottom];
    pub const WALLS: [Face; 5] = [Face::Left, Face::Right, Face::Rear, Face::Roof, Face::Bottom];

    pub fn title(self) -> &'static str {
        match self {
            Face::Plate => tr("Mounting plate"),
            Face::Door => tr("Door"),
            Face::Left => tr("Left side"),
            Face::Right => tr("Right side"),
            Face::Rear => tr("Rear wall"),
            Face::Roof => tr("Roof"),
            Face::Bottom => tr("Bottom"),
            Face::Wall(_) => tr("Wall"),
            Face::Inner(..) => tr("Inner wall"),
            Face::Outside(_) => tr("Outside wall"),
            Face::RoofTop => tr("Roof top"),
        }
    }

    /// Key of the face in files and on the command line (all walls of a room: `wall`,
    /// see [`room_face_key`]).
    pub fn key(self) -> &'static str {
        match self {
            Face::Plate => "plate",
            Face::Door => "door",
            Face::Left => "left",
            Face::Right => "right",
            Face::Rear => "rear",
            Face::Roof => "roof",
            Face::Bottom => "bottom",
            Face::Wall(_) => "wall",
            Face::Inner(..) => "inner",
            Face::Outside(_) => "outside",
            Face::RoofTop => "rooftop",
        }
    }

    pub fn from_key(key: &str) -> Option<Face> {
        let k = key.trim().to_lowercase();
        Face::ALL.into_iter().find(|f| f.key() == k)
    }

    /// Sort order of the faces: as in [`Face::ALL`], then the walls of a room.
    pub fn order(self) -> u16 {
        match self {
            Face::Wall(n) => Face::ALL.len() as u16 + n,
            Face::Inner(_, s) => u16::MAX - 4 + s as u16,
            Face::Outside(n) => (u16::MAX / 2).saturating_add(n),
            Face::RoofTop => u16::MAX - 1,
            f => Face::ALL.iter().position(|x| *x == f).unwrap_or(0) as u16,
        }
    }

    /// Side, rear, roof or bottom wall.
    pub fn is_wall(self) -> bool {
        !matches!(self, Face::Plate | Face::Door)
    }

    /// Distance of the face's origin from the top left corner of the enclosure (front
    /// view; walls are shown on their own).
    pub fn origin(self) -> f32 {
        match self {
            Face::Plate => PLATE_MARGIN,
            _ => 0.0,
        }
    }
}

impl Opening {
    pub const ALL: [Opening; 5] = [Opening::Cutout, Opening::Door, Opening::Window, Opening::Penetration, Opening::Entry];

    pub fn title(self) -> &'static str {
        match self {
            Opening::Cutout => tr("Cutout"),
            Opening::Door => tr("Door"),
            Opening::Window => tr("Window"),
            Opening::Penetration => tr("Wall penetration"),
            Opening::Entry => tr("Building entry"),
        }
    }

    /// Key on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Opening::Cutout => "cutout",
            Opening::Door => "door",
            Opening::Window => "window",
            Opening::Penetration => "penetration",
            Opening::Entry => "entry",
        }
    }

    /// Default width and height in a wall.
    pub fn size(self) -> (f32, f32) {
        match self {
            Opening::Door => DOOR_SIZE,
            Opening::Penetration => PENETRATION_SIZE,
            Opening::Entry => ENTRY_SIZE,
            _ => WINDOW_SIZE,
        }
    }

    pub fn from_key(key: &str) -> Option<Opening> {
        let k = key.trim().to_lowercase();
        Opening::ALL.into_iter().find(|o| o.key() == k)
    }
}

impl Swing {
    pub const ALL: [Swing; 6] = [Swing::LeftIn, Swing::RightIn, Swing::LeftOut, Swing::RightOut, Swing::SlideLeft, Swing::SlideRight];

    pub fn title(self) -> &'static str {
        match self {
            Swing::LeftIn => tr("Hinged left, opens inwards"),
            Swing::RightIn => tr("Hinged right, opens inwards"),
            Swing::LeftOut => tr("Hinged left, opens outwards"),
            Swing::RightOut => tr("Hinged right, opens outwards"),
            Swing::SlideLeft => tr("Sliding to the left"),
            Swing::SlideRight => tr("Sliding to the right"),
        }
    }

    /// Key on the command line.
    pub fn key(self) -> &'static str {
        match self {
            Swing::LeftIn => "left_in",
            Swing::RightIn => "right_in",
            Swing::LeftOut => "left_out",
            Swing::RightOut => "right_out",
            Swing::SlideLeft => "slide_left",
            Swing::SlideRight => "slide_right",
        }
    }

    pub fn from_key(key: &str) -> Option<Swing> {
        let k = key.trim().to_lowercase();
        Swing::ALL.into_iter().find(|o| o.key() == k)
    }
}

impl ItemKind {
    pub fn title(self) -> &'static str {
        match self {
            ItemKind::Device => tr("Device"),
            ItemKind::Rail => tr("Mounting rail"),
            ItemKind::Duct => tr("Cable duct"),
            ItemKind::Busbar => tr("Busbar system"),
            ItemKind::Cutout => tr("Cutout"),
            ItemKind::Accessory => tr("Accessory"),
            ItemKind::MountingPlate => tr("Mounting plate"),
        }
    }

    /// Drawing / picking order: higher is on top.
    pub fn layer(self) -> u8 {
        match self {
            ItemKind::Cutout => 1,
            ItemKind::MountingPlate => 0,
            ItemKind::Rail => 2,
            ItemKind::Duct => 3,
            ItemKind::Busbar => 4,
            ItemKind::Accessory => 5,
            ItemKind::Device => 6,
        }
    }

    /// Profiles drawn from a start to an end point.
    pub fn is_profile(self) -> bool {
        matches!(self, ItemKind::Rail | ItemKind::Duct | ItemKind::Busbar)
    }
}

impl PanelItem {
    pub fn rect(&self) -> R {
        R::from_points(p(self.x, self.y), p(self.x + self.w, self.y + self.h))
    }

    pub fn center(&self) -> P {
        p(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn rail_anchor(&self) -> P {
        self.rail_anchor_for(self.rail_vertical)
    }

    fn rail_anchor_for(&self,vertical:bool)->P {
        self.center()+if vertical {p(self.rail_mount.center_offset,0.)} else {p(0.,self.rail_mount.center_offset)}
    }

    pub fn seated_on(&self, rail: &PanelItem) -> bool {
        let anchor = self.rail_anchor();
        self.kind == ItemKind::Device && rail.kind == ItemKind::Rail && self.rail_vertical!=rail.horizontal()
            && self.face == rail.face && if rail.horizontal() {(rail.x..=rail.x+rail.w).contains(&anchor.x) && (anchor.y-rail.center().y).abs()<0.01}
            else {(rail.y..=rail.y+rail.h).contains(&anchor.y) && (anchor.x-rail.center().x).abs()<0.01}
            && (self.z + self.rail_mount.clip_on - rail.z - rail.d).abs() < 0.01
    }

    fn seat_on(&mut self, rail: &PanelItem) {
        self.rail_vertical = !rail.horizontal();
        if rail.horizontal() {self.y = rail.center().y - self.h / 2.0 - self.rail_mount.center_offset;}
        else {self.x=rail.center().x-self.w/2.-self.rail_mount.center_offset;}
        self.z = rail.z + rail.d - self.rail_mount.clip_on;
        self.seated_on_rail = true;
        self.mounted_on = rail.id;
    }

    pub fn horizontal(&self) -> bool {
        self.w >= self.h
    }

    /// Rails and ducts: the running length.
    pub fn length(&self) -> f32 {
        self.w.max(self.h)
    }

    /// Centre line of a rail / duct.
    pub fn center_line(&self) -> (P, P) {
        if self.horizontal() {
            let y = self.y + self.h / 2.0;
            (p(self.x, y), p(self.x + self.w, y))
        } else {
            let x = self.x + self.w / 2.0;
            (p(x, self.y), p(x, self.y + self.h))
        }
    }

    /// Rotates by 90° around the centre.
    pub fn rotate(&mut self) {
        let c = self.center();
        self.rotation=self.rotation.map(PanelRotation::next);
        std::mem::swap(&mut self.w, &mut self.h);
        self.x = c.x - self.w / 2.0;
        self.y = c.y - self.h / 2.0;
    }
}

impl Default for Enclosure {
    fn default() -> Self {
        Enclosure {
            id: 0,
            name: "+CAB1".into(),
            part: String::new(),
            device_tag:String::new(),
            owner_device:String::new(),
            bom_excluded:false,
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            no_implicit_plate:false,
            open_routing:false,
            mounting_origin:None,
            door_clearance: 0.0,
            items: Vec::new(),
            climate: Default::default(),
            build: Default::default(),
            groups: Vec::new(),
            bayed_to: 0,
            paths: Vec::new(),
            reservations: Vec::new(),
            elevation: None,
            site: None,
            kind: EnclosureKind::Cabinet,
            room: 0,
            outline: Vec::new(),
            inner_walls: Vec::new(),
            level: 0,
        }
    }
}

impl Enclosure {
    pub fn is_assembly(&self)->bool {self.kind==EnclosureKind::Assembly}

    pub fn is_room(&self) -> bool {
        self.kind == EnclosureKind::Room
    }

    /// Faces items can be mounted on: all of a cabinet; the walls and the ceiling of a
    /// room ([`room_faces`]).
    pub fn faces(&self) -> Vec<Face> {
        if self.is_assembly() {vec![Face::Plate]}
        else if self.is_room() {
            room_faces(self)
        } else {
            Face::ALL.to_vec()
        }
    }

    /// Title of a face (see [`room_face_title`] for rooms).
    pub fn face_title(&self, face: Face) -> String {
        if self.is_room() {
            room_face_title(self, face)
        } else {
            face.title().to_string()
        }
    }

    /// Key of a face on the command line (see [`room_face_key`] for rooms).
    pub fn face_key(&self, face: Face) -> String {
        if self.is_room() {
            room_face_key(self, face)
        } else {
            face.key().to_string()
        }
    }

    pub fn plate_size(&self) -> (f32, f32) {
        let offset=self.face_offset(Face::Plate);
        ((self.width - 2.0 * offset.x).max(10.0), (self.height - 2.0 * offset.y).max(10.0))
    }

    pub fn face_offset(&self,face:Face)->P {
        if face==Face::Plate {
            if let Some(origin)=self.mounting_origin {return p(origin[0],origin[1]);}
            if self.is_assembly() {return p(0.,0.);}
        }
        p(face.origin(),face.origin())
    }

    pub fn mounting_z(&self)->f32 {
        self.mounting_origin.map(|o|o[2]).unwrap_or(if self.is_assembly() {0.} else {PLATE_Z})
    }

    pub fn plate_rect(&self) -> R {
        let (w, h) = self.plate_size();
        R::from_points(p(0.0, 0.0), p(w, h))
    }

    pub fn item(&self, id: u64) -> Option<&PanelItem> {
        self.items.iter().find(|i| i.id == id)
    }

    /// Size of a face (mounting plate, door or wall).
    pub fn face_size(&self, face: Face) -> (f32, f32) {
        if self.is_room() {
            return rooms::face_size(self, face);
        }
        match face {
            Face::Plate => self.plate_size(),
            Face::Door | Face::Rear | Face::Wall(_) | Face::Inner(..) | Face::Outside(_) => (self.width, self.height),
            Face::Left | Face::Right => (self.depth, self.height),
            Face::Roof | Face::Bottom | Face::RoofTop => (self.width, self.depth),
        }
    }

    /// Size of the front view of a face: the plate is shown inside the enclosure
    /// front, the walls on their own.
    pub fn view_size(&self, face: Face) -> (f32, f32) {
        match face {
            Face::Plate => (self.width, self.height),
            f => self.face_size(f),
        }
    }

    /// Room in front of a face: how far items may reach into the enclosure.
    pub fn face_room(&self, face: Face) -> f32 {
        if self.is_assembly() {return self.depth.max(0.0);}
        let t = self.wall();
        if self.is_room() {
            return rooms::face_room(self, face);
        }
        match face {
            Face::Plate => self.depth - self.mounting_z() - PLATE_THICKNESS - self.door_clearance,
            Face::Door => self.depth - PLATE_Z,
            Face::Left | Face::Right => self.width - 2.0 * t,
            Face::Rear | Face::Wall(_) | Face::Inner(..) | Face::Outside(_) => PLATE_Z - PLATE_THICKNESS - t,
            Face::Roof | Face::Bottom | Face::RoofTop => self.height - 2.0 * t,
        }
    }

    /// Sheet thickness of walls and door.
    pub fn wall(&self) -> f32 {
        if self.is_room() {
            return ROOM_WALL;
        }
        self.system().map(|s| s.data.wall).unwrap_or(WALL)
    }

    /// Free edge of the door and the walls where nothing can be mounted (none in a
    /// room).
    pub fn flange(&self) -> f32 {
        if self.is_room() {
            return 0.0;
        }
        self.system().map(|s| s.data.flange).unwrap_or(DOOR_FRAME)
    }

    pub fn system(&self) -> Option<&'static System> {
        system::by_id(&self.build.system)
    }

    pub fn group(&self, id: u64) -> Option<&ItemGroup> {
        self.groups.iter().find(|g| g.id == id)
    }
}

/// Sheet thickness of enclosures without a system.
pub const WALL: f32 = 2.0;

/// Frame of a face in enclosure coordinates: origin (face coordinates 0, 0 on the
/// mounting surface), direction of face x, of face y and of `z` (into the enclosure).
pub fn face_frame(e: &Enclosure, face: Face) -> [V3; 4] {
    if e.is_room() {
        return rooms::frame(e, face);
    }
    let (w, h, d, t) = (e.width, e.height, e.depth, e.wall());
    match face {
        Face::Plate => {let offset=e.face_offset(face);[[offset.x,offset.y,e.mounting_z()],[1.,0.,0.],[0.,1.,0.],[0.,0.,1.]]},
        Face::Door => [[0.0, 0.0, d], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]],
        Face::Left => [[t, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 0.0, 0.0]],
        Face::Right => [[w - t, 0.0, d], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]],
        Face::Rear => [[w, 0.0, t], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        Face::Roof | Face::RoofTop => [[0.0, t, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]],
        Face::Bottom => [[0.0, h - t, d], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]],
        Face::Wall(_) | Face::Inner(..) | Face::Outside(_) => [[w, 0.0, t], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
    }
}

/// A point of a face (face x, face y, distance into the enclosure) in enclosure
/// coordinates.
pub fn face_point(e: &Enclosure, face: Face, v: V3) -> V3 {
    in_frame(&face_frame(e, face), v)
}

/// A point of a face in enclosure coordinates, given the face's [`face_frame`]: for
/// many points of one face (the frame of a room's face takes its outline).
fn in_frame([o, u, w, n]: &[V3; 4], v: V3) -> V3 {
    [0, 1, 2].map(|k| o[k] + v[0] * u[k] + v[1] * w[k] + v[2] * n[k])
}

/// The face's frame turns models inside out (left-handed): triangles are flipped.
fn mirrors([_, u, w, n]: &[V3; 4]) -> bool {
    crate::parts::vec3::dot(crate::parts::vec3::cross(*u, *w), *n) < 0.0
}

/// 3D box of an item in enclosure coordinates: x to the right and y downwards from
/// the top left corner of the enclosure front, z from the rear wall towards the door.
pub fn item_box(e: &Enclosure, it: &PanelItem) -> (V3, V3) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    let frame = face_frame(e, it.face);
    for c in 0..8 {
        let v = [
            if c & 1 == 0 { it.x } else { it.x + it.w },
            if c & 2 == 0 { it.y } else { it.y + it.h },
            if c & 4 == 0 { it.z } else { it.z + it.d },
        ];
        let q = in_frame(&frame, v);
        for k in 0..3 {
            min[k] = min[k].min(q[k]);
            max[k] = max[k].max(q[k]);
        }
    }
    (min, max)
}

/// Point of a placed model ([`place_mesh`] coordinates) in enclosure coordinates.
/// Door devices face outwards: their front is at the door; on the plate and the walls
/// devices face into the enclosure.
pub fn to_enclosure(e: &Enclosure, it: &PanelItem, v: V3) -> V3 {
    match it.face {
        Face::Door => [v[0], v[1], e.depth - 2.0 * it.z - it.d + v[2]],
        f => face_point(e, f, v),
    }
}

/// Triangles of a placed model ([`place_mesh`] coordinates) in enclosure coordinates,
/// keeping their outside.
pub fn place_triangles(e: &Enclosure, it: &PanelItem, tris: Vec<Tri>) -> Vec<Tri> {
    // As `to_enclosure`, with the face's frame made once.
    let frame = face_frame(e, it.face);
    let flip = it.face != Face::Door && mirrors(&frame);
    let door_z = e.depth - 2.0 * it.z - it.d;
    let place = |v: V3| if it.face == Face::Door { [v[0], v[1], door_z + v[2]] } else { in_frame(&frame, v) };
    tris.into_iter()
        .map(|t| {
            let t = t.map(place);
            if flip { [t[0], t[2], t[1]] } else { t }
        })
        .collect()
}

/// The box between `min` and `max` in plate coordinates, wound like a placed model
/// ([`place_mesh`]): inside out, as the plate's y points down. The 3D view and the
/// exports turn such meshes right side out with their y; a box wound outward there
/// (`box_triangles`) comes out inside out, its far faces drawn instead of its near
/// ones.
pub fn plate_box(min: V3, max: V3) -> Vec<Tri> {
    box_triangles(min, max).into_iter().map(|t| [t[0], t[2], t[1]]).collect()
}

/// Surface of an item in enclosure coordinates: the part's 3D model, or its box.
pub fn item_surface(e: &Enclosure, it: &PanelItem, parts: &[crate::parts::Part]) -> Vec<Tri> {
    let mesh = parts.iter().find(|p| it.kind == ItemKind::Device && p.number == it.part).and_then(|p| p.model_3d.as_ref());
    match mesh.map(|m| place_mesh(m, it)).filter(|t| !t.is_empty()) {
        Some(tris) => place_triangles(e, it, tris),
        None => {
            let (min, max) = item_box(e, it);
            plate_box(min, max)
        }
    }
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            enclosures: Vec::new(),
            wire_reserve: 100.0,
            ferrules: true,
            duct_fill_limit: 70.0,
            hardware: true,
            macros: Vec::new(),
            routing: Default::default(),
            frozen: Vec::new(),
            laying: Default::default(),
            pv_design: Default::default(),
            checks: PanelRules::default(),
            manufacturing: Default::default(),
            buildings: Vec::new(),
            site_links: Vec::new(),
            plot: Default::default(),
        }
    }
}

impl Layout {
    pub fn placed_tags(&self) -> HashSet<&str> {
        self.enclosures
            .iter()
            .flat_map(|e| e.items.iter())
            .filter(|i| i.kind == ItemKind::Device)
            .map(|i| i.tag.as_str())
            .collect()
    }

    /// Enclosure index and item of a placed device.
    pub fn find_device(&self, tag: &str) -> Option<(usize, &PanelItem)> {
        self.enclosures.iter().enumerate().find_map(|(ei, e)| {
            e.items.iter().find(|i| i.kind == ItemKind::Device && i.tag == tag).map(|i| (ei, i))
        })
    }
}

/// Snaps a value to the plate grid.
pub fn snap(v: f32) -> f32 {
    (v / SNAP).round() * SNAP
}

// ---------------------------------------------------------------------- creation

/// The first name `prefix` + number (`+CAB3`) no enclosure or room of the layout has.
pub fn free_name(layout: &Layout, prefix: &str) -> String {
    (1..).map(|n| format!("{prefix}{n}")).find(|name| layout.enclosures.iter().all(|e| &e.name != name)).unwrap_or_default()
}

/// Creates an enclosure, taking the dimensions from a part if it has them.
pub fn new_enclosure(prj: &mut Project, part: &str, name: &str) -> Enclosure {
    let mut e = Enclosure { id: prj.new_id(), name: name.into(), part: part.into(), ..Default::default() };
    if let Some(pd) = part_by_number(prj, part) {
        if pd.width > 0.0 && pd.height > 0.0 {
            e.width = pd.width;
            e.height = pd.height;
            e.depth = if pd.depth > 0.0 { pd.depth } else { e.depth };
        }
        if let Some(s) = system_of_part(pd) {
            apply_system(&mut e, s.id(), None);
        }
        // An enclosure delivered with its plinth (part data `Plinth: 200`).
        if let Some(h) = pd.tech_value("Plinth").and_then(|v| v.trim().trim_end_matches("mm").trim().parse::<f32>().ok()).filter(|h| *h > 0.0) {
            e.build.plinth = h;
            e.build.plinth_included = true;
        }
    }
    e
}

/// Rail or duct between two points (orientation by the dominant direction).
pub fn new_profile(prj: &mut Project, kind: ItemKind, part: &str, a: P, b: P) -> PanelItem {
    let (pw, pd) = profile(prj, kind, part);
    let horizontal = (b.x - a.x).abs() >= (b.y - a.y).abs();
    let (x, y, w, h) = if horizontal {
        (a.x.min(b.x), a.y - pw / 2.0, (b.x - a.x).abs(), pw)
    } else {
        (a.x - pw / 2.0, a.y.min(b.y), pw, (b.y - a.y).abs())
    };
    PanelItem { id: prj.new_id(), kind, part: part.into(), x, y, w: w.max(SNAP), h: h.max(SNAP), d: pd, ..Default::default() }
}

/// The bars of the busbar system `bar` run along x: a system as wide across its bars
/// as its part lies the way that fits, however short it is (a section of a 185 mm
/// system shorter than the system is wide); otherwise along its longer side.
pub fn bars_along_x(parts: &[crate::parts::Part], bar: &PanelItem) -> bool {
    let (wide, _) = part_profile(ItemKind::Busbar, parts.iter().find(|x| x.number == bar.part));
    let (across_y, across_x) = ((bar.h - wide).abs() < 0.5, (bar.w - wide).abs() < 0.5);
    if across_y != across_x {
        across_y
    } else {
        bar.w >= bar.h
    }
}

/// Profile width and height of rails / ducts (from the part or the defaults).
pub fn profile(prj: &Project, kind: ItemKind, part: &str) -> (f32, f32) {
    part_profile(kind, prj.parts.iter().find(|x| x.number == part))
}

/// Profile width and height of a rail / duct part (the defaults without data).
pub fn part_profile(kind: ItemKind, part: Option<&crate::parts::Part>) -> (f32, f32) {
    match part {
        Some(pd) if pd.height > 0.0 && pd.depth > 0.0 => (pd.height, pd.depth),
        _ => match kind {
            ItemKind::Rail => RAIL_PROFILE,
            ItemKind::Busbar => BUSBAR_PROFILE,
            _ => DUCT_PROFILE,
        },
    }
}

/// Product subgroup of the parts placed as profiles of this kind.
pub fn profile_subgroup(kind: ItemKind) -> Option<&'static str> {
    match kind {
        ItemKind::Rail => Some("Mounting rail"),
        ItemKind::Duct => Some("Cable duct"),
        ItemKind::Busbar => Some("Busbar system"),
        _ => None,
    }
}

/// A part that can be placed as a rail, duct or busbar system.
#[derive(Clone, Debug, PartialEq)]
pub struct ProfileChoice {
    pub number: String,
    pub description: String,
    /// Generic part of the built-in catalog (otherwise from the parts store or the
    /// project).
    pub generic: bool,
}

/// Parts for profiles of this kind: the generic ones of the built-in catalog and those
/// in `sources` (parts store, project parts) of the matching subgroup, each once,
/// generic ones first, then by part number.
pub fn profile_choices<'a>(kind: ItemKind, sources: impl IntoIterator<Item = &'a crate::parts::Part>) -> Vec<ProfileChoice> {
    let Some(sub) = profile_subgroup(kind) else { return Vec::new() };
    let catalog = crate::parts::catalog();
    let mut v: Vec<ProfileChoice> = Vec::new();
    for pt in catalog.iter().chain(sources) {
        if pt.subgroup == sub && !v.iter().any(|c| c.number == pt.number) {
            let generic = catalog.iter().any(|c| c.number == pt.number);
            v.push(ProfileChoice { number: pt.number.clone(), description: pt.description.clone(), generic });
        }
    }
    v.sort_by(|a, b| b.generic.cmp(&a.generic).then(crate::model::natural_cmp(&a.number, &b.number)));
    v
}

/// Gives a rail, duct or busbar system of enclosure `ei` another part (from the project
/// parts): width across and height from the part data, centre line and length stay;
/// devices on a rail follow its new height.
pub fn change_profile(prj: &mut Project, ei: usize, id: u64, part: &str) -> bool {
    let Some(old) = prj.layout.enclosures.get(ei).and_then(|e| e.item(id)).filter(|i| i.kind.is_profile()).cloned() else { return false };
    let (pw, pd) = profile(prj, old.kind, part);
    let e = &mut prj.layout.enclosures[ei];
    let Some(it) = e.items.iter_mut().find(|i| i.id == id) else { return false };
    let c = it.center();
    if it.horizontal() {
        it.y = c.y - pw / 2.0;
        it.h = pw;
    } else {
        it.x = c.x - pw / 2.0;
        it.w = pw;
    }
    it.d = pd;
    it.part = part.into();
    if old.kind == ItemKind::Rail {
        let on_rail: Vec<usize> = (0..e.items.len()).filter(|&j| e.items[j].seated_on(&old)).collect();
        for j in on_rail {
            let mut dev = e.items[j].clone();
            snap_to_rail(e, &mut dev);
            e.items[j] = dev;
        }
    }
    crate::parts::replace_device_part(prj,&old.tag,&old.part,part);
    true
}

/// A device that can be placed: tag, size and whether the size came from part data.
#[derive(Clone, Debug, PartialEq)]
pub struct DeviceSize {
    pub tag: String,
    pub part: String,
    pub w: f32,
    pub h: f32,
    pub d: f32,
    pub from_part: bool,
}

/// Number of copies of a model of width `model_w` side by side in an item of width
/// `item_w` (terminal strips show one model per terminal).
pub fn tiles(item_w: f32, model_w: f32) -> usize {
    if model_w <= 0.0 {
        return 1;
    }
    let n = (item_w / model_w).round();
    if n >= 2.0 && (n * model_w - item_w).abs() < 0.5 {
        n as usize
    } else {
        1
    }
}

/// The item is turned by a quarter turn against a part of height `part_h`: the
/// part's height lies along x.
pub fn item_rotated(it: &PanelItem, part_h: f32) -> bool {
    item_rotation(it,part_h).is_quarter()
}

/// Turns selected items about their centres, keeping room openings fixed.
pub fn rotate_items(e:&mut Enclosure,ids:&[u64]) {
    for i in 0..e.items.len() {
        if ids.contains(&e.items[i].id) && e.items[i].opening.is_cutout() {
            let mut item=e.items[i].clone();
            item.rotate();
            snap_to_rail(e,&mut item);
            e.items[i]=item;
        }
    }
}

pub fn item_rotation(it:&PanelItem,part_h:f32)->PanelRotation {
    if let Some(rotation)=it.rotation {return rotation}
    let same = |a: f32, b: f32| (a - b).abs() < 0.5;
    if !same(it.h, part_h) && same(it.w, part_h) {PanelRotation::Deg90} else {PanelRotation::Deg0}
}

pub fn device_rotation(prj:&Project,it:&PanelItem)->PanelRotation {
    if let Some(rotation)=it.rotation {return rotation}
    let assembled=(!it.single_part && it.terminal.is_empty()).then(||assembly(prj,&it.tag)).flatten().map(|a|a.h);
    let height=assembled.or_else(||part_by_number(prj,&it.part).and_then(|part|part.size()).map(|size|size[1])).unwrap_or(it.h);
    item_rotation(it,height)
}

/// Places a coordinate of one part cell into an oriented item's face footprint.
pub fn front_point(it:&PanelItem,rotation:PanelRotation,at:f32,len:f32,[u,v]:[f32;2])->P {
    match rotation {
        PanelRotation::Deg0=>p(it.x+at+u*len,it.y+v*it.h),
        PanelRotation::Deg90=>p(it.x+(1.-v)*it.w,it.y+at+u*len),
        PanelRotation::Deg180=>p(it.x+it.w-at-u*len,it.y+(1.-v)*it.h),
        PanelRotation::Deg270=>p(it.x+v*it.w,it.y+it.h-at-u*len),
    }
}

pub fn front_cell(it:&PanelItem,rotation:PanelRotation,at:f32,len:f32)->R {
    R::from_points(front_point(it,rotation,at,len,[0.,0.]),front_point(it,rotation,at,len,[1.,1.]))
}

/// Triangles of a part's 3D model placed into a device on the plate, in plate
/// coordinates (x right, y down, z = height above the plate). The model is fitted
/// into the device box; rotated devices turn the model by 90° about the front axis,
/// a strip of equal models repeats it. On the floor the model stands up. The
/// winding turns inside out with the model's y (up) becoming the plate's (down):
/// placed meshes are wound like [`plate_box`], the 3D view and the exports turn them
/// right side out again with their y.
pub fn place_mesh(mesh: &crate::parts::Mesh3d, it: &PanelItem) -> Vec<[[f32; 3]; 3]> {
    let [mw, mh, md] = mesh.size();
    if it.face == Face::Bottom {
        return place_mesh_in(mesh, it, 0.0, if item_rotated(it, md) { it.h } else { it.w });
    }
    let rotated = item_rotated(it, mh);
    let n = if rotated { 1 } else { tiles(it.w, mw) };
    let cell = (if rotated { it.h } else { it.w }) / n as f32;
    (0..n).flat_map(|k| place_mesh_in(mesh, it, k as f32 * cell, cell)).collect()
}

/// A part's 3D model fitted into the stretch `at .. at + len` along a device (along
/// x, or along y for rotated devices) and the full height and depth.
pub fn place_mesh_in(mesh: &crate::parts::Mesh3d, it: &PanelItem, at: f32, len: f32) -> Vec<[[f32; 3]; 3]> {
    let [mw, mh, md] = mesh.size();
    if mw <= 0.0 || mh <= 0.0 || md <= 0.0 {
        return Vec::new();
    }
    let (min, _) = mesh.bounds();
    let tf = |v: [f32; 3]| place_vertex(it, min, [mw, mh, md], at, len, v);
    mesh.triangles.iter().map(|t| [tf(mesh.vertices[t[0] as usize]), tf(mesh.vertices[t[1] as usize]), tf(mesh.vertices[t[2] as usize])]).collect()
}

/// A point of a part's 3D model placed like [`place_mesh`] places the model.
pub fn place_point(mesh: &crate::parts::Mesh3d, it: &PanelItem, v: V3) -> V3 {
    let [mw, mh, md] = mesh.size();
    let natural_height=if it.face==Face::Bottom {md} else {mh};
    let len = if item_rotated(it,natural_height) { it.h } else { it.w };
    place_vertex(it, mesh.bounds().0, [mw, mh, md].map(|s| s.max(1e-3)), 0.0, len, v)
}

/// A vertex of a model with the bounds `min` and the size `[mw, mh, md]` placed into
/// the stretch `at .. at + len` along the item (see [`place_mesh_in`]).
fn place_vertex(it: &PanelItem, min: V3, [mw, mh, md]: [f32; 3], at: f32, len: f32, v: V3) -> V3 {
    // On the floor the model stands up out of it with its front (z) towards the
    // larger y of the floor plan; turned, its depth lies along x.
    let floor = it.face == Face::Bottom;
    let (x, y_up, z) = (v[0] - min[0], v[1] - min[1], v[2] - min[2]);
    if floor {
        let inferred=item_rotation(it,md);
        let rotation=if it.rotation.is_none() && inferred==PanelRotation::Deg90 {PanelRotation::Deg270} else {inferred};
        let q=front_point(it,rotation,at,len,[x/mw,z/md]);
        [q.x,q.y,it.z+y_up*(it.d/mh)]
    } else {
        let q=front_point(it,item_rotation(it,mh),at,len,[x/mw,(mh-y_up)/mh]);
        [q.x,q.y,it.z+z*(it.d/md)]
    }
}

/// A place along a device for one part: offset and length along the device (along
/// x, or along y for rotated devices).
#[derive(Clone, Debug, PartialEq)]
pub struct Cell {
    pub at: f32,
    pub len: f32,
    pub part: String,
}

pub(crate) fn part_by_number<'a>(prj: &'a Project, number: &str) -> Option<&'a crate::parts::Part> {
    prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == number)
}

/// Terminal strips: one cell per element of the strip layout (terminals, end plates,
/// end brackets), scaled to the item. Empty for other items.
pub fn strip_cells(prj: &Project, it: &PanelItem) -> Vec<Cell> {
    let Some(main) = part_by_number(prj, &it.part).filter(|_| it.kind == ItemKind::Device) else { return Vec::new() };
    if !crate::strips::is_terminal_part(main) {
        return Vec::new();
    }
    if it.single_part || !it.terminal.is_empty() {
        let rotated=device_rotation(prj,it).is_quarter();
        return vec![Cell {at:0.,len:if rotated {it.h} else {it.w},part:it.part.clone()}];
    }
    let layout = crate::strips::strip_layout(prj, &it.tag);
    let total: f32 = layout.iter().map(|e| e.width).sum();
    if total <= 0.0 {
        return Vec::new();
    }
    let rotated = device_rotation(prj,it).is_quarter();
    let scale = (if rotated { it.h } else { it.w }) / total;
    let mut at = 0.0;
    layout
        .into_iter()
        .map(|e| {
            let cell = Cell { at, len: e.width * scale, part: e.part };
            at += cell.len;
            cell
        })
        .collect()
}

/// Colours of a slotted cable duct and its cover.
const DUCT_COLORS: ([u8; 3], [u8; 3]) = ([150, 153, 150], [166, 169, 166]);

/// The walls of a slotted cable duct: the heights above its base of their closed rim
/// and of their top (the cover lies on it), and their thickness.
pub fn duct_walls(it: &PanelItem) -> (f32, f32, f32) {
    let t = 2.0f32.min(it.w.min(it.h) / 4.0).min(it.d / 4.0);
    ((it.d * 0.25).max(t), it.d - 2.5f32.min(it.d / 4.0), t)
}

/// The parts of a slotted cable duct (enclosure coordinates, colour, whether it is a
/// finger): the base, the side walls slotted into fingers above a closed rim, and the
/// `cover` on top. Where another duct of the face joins it, its wall there is cut out
/// (the ducts open into each other).
pub fn duct_boxes(e: &Enclosure, it: &PanelItem, cover: bool) -> Vec<((V3, V3), [u8; 3], bool)> {
    let (wall, cover_color) = DUCT_COLORS;
    let frame = face_frame(e, it.face);
    let along_x = it.w >= it.h;
    let (len, width) = if along_x { (it.w, it.h) } else { (it.h, it.w) };
    let (rim, top, t) = duct_walls(it);
    let mut out = Vec::new();
    // A box from ranges along, across and above the face.
    let mut add = |a: (f32, f32), c: (f32, f32), z: (f32, f32), color: [u8; 3], finger: bool| {
        let (x, y) = if along_x { (a, c) } else { (c, a) };
        let p = in_frame(&frame, [it.x + x.0, it.y + y.0, it.z + z.0]);
        let q = in_frame(&frame, [it.x + x.1, it.y + y.1, it.z + z.1]);
        out.push((([0, 1, 2].map(|k| p[k].min(q[k])), [0, 1, 2].map(|k| p[k].max(q[k]))), color, finger));
    };
    // Where other ducts join: the stretch along the duct and which of its walls (the
    // one at across 0, the one at `width`) they open.
    let (a0, c0) = if along_x { (it.x, it.y) } else { (it.y, it.x) };
    let mut joins: Vec<(f32, f32, [bool; 2])> = Vec::new();
    for o in e.items.iter().filter(|o| o.id != it.id && o.kind == ItemKind::Duct && o.face == it.face) {
        let (lo, hi) = ([it.x.max(o.x - 1.0), it.y.max(o.y - 1.0)], [(it.x + it.w).min(o.x + o.w + 1.0), (it.y + it.h).min(o.y + o.h + 1.0)]);
        if lo[0] >= hi[0] || lo[1] >= hi[1] {
            continue;
        }
        let (along, across) = if along_x { ((lo[0], hi[0]), (lo[1], hi[1])) } else { ((lo[1], hi[1]), (lo[0], hi[0])) };
        joins.push((along.0 - a0, along.1 - a0, [across.0 - c0 <= t + 0.5, across.1 - c0 >= width - t - 0.5]));
    }
    add((0.0, len), (0.0, width), (0.0, t), wall, false);
    // Fingers about every 12.5 mm with the slots between them, one at each end.
    let finger = 6.0f32.min(len);
    let n = ((len - finger) / 12.5).floor().max(0.0) as usize + 1;
    let step = if n > 1 { (len - finger) / (n - 1) as f32 } else { 0.0 };
    for (side, c) in [(0.0, t), (width - t, width)].into_iter().enumerate() {
        let cuts: Vec<(f32, f32)> = joins.iter().filter(|j| j.2[side]).map(|j| (j.0, j.1)).collect();
        // The closed rim, in pieces between the joins.
        let mut pieces: Vec<(f32, f32)> = vec![(0.0, len)];
        for &(s0, s1) in &cuts {
            pieces = pieces.into_iter().flat_map(|(a, b)| [(a, b.min(s0)), (a.max(s1), b)]).filter(|(a, b)| b - a > 0.01).collect();
        }
        for (a, b) in pieces {
            add((a, b), c, (t, rim), wall, false);
        }
        for i in 0..n {
            let a = i as f32 * step;
            if cuts.iter().any(|&(s0, s1)| a + finger > s0 && a < s1) {
                continue;
            }
            add((a, a + finger), c, (rim, top), wall, true);
        }
    }
    if cover {
        add((0.0, len), (0.0, width), (top, it.d), cover_color, false);
    }
    out
}

/// The 3D model of a device in plate coordinates: the part's model, for terminal
/// strips the model of every element in its cell (a box where a part has none).
/// `None` for items without a model.
pub fn device_mesh(prj: &Project, it: &PanelItem) -> Option<Vec<[[f32; 3]; 3]>> {
    device_mesh_painted(prj, it).map(|v| v.into_iter().map(|t| t.0).collect())
}

/// Like [`device_mesh`], with the colour of each triangle where its model has one.
pub fn device_mesh_painted(prj: &Project, it: &PanelItem) -> Option<Vec<(Tri, Option<[u8; 3]>)>> {
    if it.kind == ItemKind::Busbar {
        return Some(busbars::mesh(prj, it));
    }
    if it.kind != ItemKind::Device {
        return None;
    }
    // A placed model repeats the model's triangles in their order.
    let painted = |mesh: &crate::parts::Mesh3d, tris: Vec<Tri>| -> Vec<(Tri, Option<[u8; 3]>)> {
        let n = mesh.triangles.len().max(1);
        tris.into_iter().enumerate().map(|(i, t)| (t, mesh.color(i % n))).collect()
    };
    // Accessories mounted on the main part: every piece in its box.
    if let Some(a) = mounted(prj, it) {
        let mut out = Vec::new();
        for piece in a.pieces() {
            let sub = a.piece_item(it, piece);
            match piece.part.model_3d.as_ref() {
                Some(mesh) => out.extend(painted(mesh, place_mesh(mesh, &sub))),
                None => out.extend(plate_box([sub.x, sub.y, sub.z], [sub.x + sub.w, sub.y + sub.h, sub.z + sub.d]).into_iter().map(|t| (t, None))),
            }
        }
        return Some(out);
    }
    // Busbar terminals across the bars: each upright on its bar.
    if let Some(places) = busbars::terminals_on_bars(prj, it) {
        let mut out = Vec::new();
        for (sub, number, _) in places {
            match part_by_number(prj, &number).and_then(|p| p.model_3d.as_ref()) {
                Some(mesh) => out.extend(painted(mesh, place_mesh(mesh, &sub))),
                None => out.extend(plate_box([sub.x, sub.y, sub.z], [sub.x + sub.w, sub.y + sub.h, sub.z + sub.d]).into_iter().map(|t| (t, None))),
            }
        }
        return Some(out);
    }
    let cells = strip_cells(prj, it);
    if cells.is_empty() {
        let mesh = part_by_number(prj, &it.part)?.model_3d.as_ref()?;
        let mut tris = place_mesh(mesh, it);
        // A motor turned end for end by its drive (its shaft towards the machine).
        if let Some((k, c)) = drives::turned_motor(prj, it) {
            tris = tris.into_iter().map(|t| t.map(|mut v| {
                v[k] = 2.0 * c - v[k];
                v
            })).map(|t| [t[0], t[2], t[1]]).collect();
        }
        let tris = painted(mesh, tris);
        return (!tris.is_empty()).then_some(tris);
    }
    let rotation = device_rotation(prj,it);
    let mut out = Vec::new();
    for c in cells {
        match part_by_number(prj, &c.part).and_then(|p| p.model_3d.as_ref()) {
            Some(mesh) => out.extend(painted(mesh, place_mesh_in(mesh, it, c.at, c.len))),
            None => {
                let cell=front_cell(it,rotation,c.at,c.len);
                let (min,max)=([cell.min.x,cell.min.y,it.z],[cell.max.x,cell.max.y,it.z+it.d]);
                out.extend(plate_box(min, max).into_iter().map(|t| (t, None)));
            }
        }
    }
    Some(out)
}

/// Footprint of a device from its parts (see [`assembly`]): the main part gives the
/// height, accessories in front add their depth, accessories beside it their width.
/// Terminal strips are as long as their strip layout (terminals and spare terminals)
/// and as deep as their deepest part.
pub fn device_size(prj: &Project, tag: &str) -> Option<DeviceSize> {
    let Some(data) = prj.devices.get(tag) else { return group_size(prj, tag) };
    if let Some(a) = assembly(prj, tag) {
        return Some(DeviceSize { tag: tag.into(), part: a.main.part.number.clone(), w: a.w, h: a.h, d: a.d, from_part: true });
    }
    let parts: Vec<_> = data
        .parts
        .iter()
        .filter_map(|r| prj.parts.iter().find(|x| x.number == r.number).map(|pd| (pd, r.quantity())))
        .collect();
    let (main, [mw, mh, _]) = parts.iter().find_map(|(pd, _)| pd.size().map(|dims| (*pd, dims)))?;
    let length: f32 = crate::strips::strip_layout(prj, tag).iter().map(|e| e.width).sum();
    let w = if length > 0.0 { length } else { mw };
    Some(DeviceSize { tag: tag.into(), part: main.number.clone(), w, h: mh, d: strip_depth(&parts), from_part: true })
}

/// The main part of a terminal strip: all its functions are terminals.
pub(crate) fn is_strip_part(main: &crate::parts::Part) -> bool {
    !main.functions.is_empty() && main.functions.iter().all(|f| f.kind == FnKind::Terminal)
}

/// Depth of a terminal strip: its parts stand side by side on the rail, the deepest
/// counts.
pub(crate) fn strip_depth(parts: &[(&crate::parts::Part, u32)]) -> f32 {
    parts.iter().filter_map(|(p, _)| p.size()).map(|s| s[2]).fold(0.0, f32::max)
}

/// Footprint of a fluid device group (valve terminal, service unit) placed as one
/// item: its base part if that has mounting data, otherwise its members side by side.
pub fn group_size(prj: &Project, tag: &str) -> Option<DeviceSize> {
    let g = prj.fluid.groups.iter().find(|g| g.tag == tag)?;
    if let Some([w, h, d]) = part_by_number(prj, &g.part).and_then(|p| p.size()) {
        return Some(DeviceSize { tag: tag.into(), part: g.part.clone(), w, h, d, from_part: true });
    }
    let members: Vec<DeviceSize> = g.members.iter().filter_map(|m| device_size(prj, m)).collect();
    if members.is_empty() {
        return None;
    }
    let w = members.iter().map(|m| m.w).sum();
    let h = members.iter().map(|m| m.h).fold(0.0, f32::max);
    let d = members.iter().map(|m| m.d).fold(0.0, f32::max);
    Some(DeviceSize { tag: tag.into(), part: g.part.clone(), w, h, d, from_part: true })
}

/// Where a device is in the panel layout: placed itself, as a member of a placed
/// fluid device group (valve terminal, service unit), or – for an electrical device
/// linked to a fluid device (solenoid coil) – where that fluid device is.
pub fn locate<'a>(prj: &'a Project, tag: &str) -> Option<(usize, &'a PanelItem)> {
    let direct = |t: &str| {
        prj.layout.find_device(t).or_else(|| prj.fluid.groups.iter().find(|g| g.members.iter().any(|m| m == t)).and_then(|g| prj.layout.find_device(&g.tag)))
    };
    direct(tag).or_else(|| crate::electrofluid::fluid_devices_of(prj, tag).into_iter().find_map(|f| direct(f)))
}

/// Resolve a terminal by its designation, with legacy whole-strip placement as a
/// fallback. Ambiguous terminal positions never choose an arbitrary footprint; a strip
/// placed whole more than once resolves to its first placement.
pub fn locate_symbol<'a>(prj:&'a Project,s:&crate::model::SymbolInst)->Option<(usize,&'a PanelItem)> {
    if prj.symbol(&s.sym).is_none_or(|d|d.func!=crate::symbols::Func::Terminal) {
        let all:Vec<_>=prj.layout.enclosures.iter().enumerate().flat_map(|(ei,e)|e.items.iter().map(move |it|(ei,it)))
            .filter(|(_,it)|it.kind==ItemKind::Device && it.tag==s.tag).collect();
        if !all.iter().any(|(_,it)|it.single_part) {return locate(prj,&s.tag);}
        let selected=s.selected_part_number(prj);
        let matching:Vec<_>=all.iter().copied().filter(|(_,it)|selected.is_none_or(|part|part==it.part)).collect();
        return if matching.len()==1 {Some(matching[0])} else if all.len()==1 {Some(all[0])} else {None};
    }
    let find=|label:&str| {
        let mut matches=prj.layout.enclosures.iter().enumerate().flat_map(|(ei,e)|e.items.iter().map(move |it|(ei,it)))
            .filter(|(_,it)|it.kind==ItemKind::Device && it.tag==s.tag && it.terminal==label);
        let first=matches.next();if matches.next().is_some() {None} else {first}
    };
    let exact=prj.layout.enclosures.iter().flat_map(|e|&e.items).any(|it|it.kind==ItemKind::Device && it.tag==s.tag && it.terminal==s.label);
    if exact {return find(&s.label);}
    prj.layout.enclosures.iter().enumerate().flat_map(|(ei,e)|e.items.iter().map(move |it|(ei,it)))
        .find(|(_,it)|it.kind==ItemKind::Device && it.tag==s.tag && it.terminal.is_empty())
}

/// Devices with parts that are not placed in any enclosure yet, and fluid device
/// groups whose members are not placed. Devices whose parts have no mounting data
/// (e.g. motors in the field) are not listed, nor members of a placed group and coils
/// on a placed valve.
pub fn devices_to_place(prj: &Project) -> Vec<DeviceSize> {
    let groups = prj.fluid.groups.iter().map(|g| &g.tag).filter(|t| !t.is_empty() && !prj.devices.contains_key(*t));
    let mut v: Vec<DeviceSize> = prj
        .devices
        .keys()
        .chain(groups)
        .filter(|t| locate(prj, t).is_none())
        .filter(|t| !prj.fluid.groups.iter().any(|g| g.tag == **t && g.members.iter().any(|m| prj.layout.find_device(m).is_some())))
        .filter_map(|t| device_size(prj, t))
        .collect();
    v.sort_by(|a, b| crate::model::natural_cmp(&a.tag, &b.tag));
    v.dedup_by(|a, b| a.tag == b.tag);
    v
}

/// Places a device on the mounting plate with its top left corner at `at` (snapped,
/// rail aware).
pub fn place_device(prj: &mut Project, enc: usize, tag: &str, at: P) -> Option<u64> {
    place_device_on(prj, enc, tag, at, Face::Plate)
}

/// Places a device on the mounting plate or the door.
pub fn place_device_on(prj: &mut Project, enc: usize, tag: &str, at: P, face: Face) -> Option<u64> {
    let e = prj.layout.enclosures.get(enc)?;
    let mut item = device_preview(prj, e, tag, at, face);
    let id = prj.new_id();
    item.id = id;
    prj.layout.enclosures[enc].items.push(item);
    Some(id)
}

/// The mounted position and dimensions used both for the placement ghost and the final drop.
pub fn device_preview(prj: &Project, e: &Enclosure, tag: &str, at: P, face: Face) -> PanelItem {
    let size = device_size(prj, tag).unwrap_or(DeviceSize {
        tag: tag.into(),
        part: String::new(),
        w: DEFAULT_DEVICE.0,
        h: DEFAULT_DEVICE.1,
        d: DEFAULT_DEVICE.2,
        from_part: false,
    });
    let rail_mount = part_by_number(prj, &size.part).map(|p| p.effective_rail_mount()).unwrap_or_default();
    let mut item = PanelItem {
        rail_mount,
        kind: ItemKind::Device,
        tag: tag.into(),
        part: size.part,
        x: snap(at.x),
        y: snap(at.y),
        w: size.w,
        h: size.h,
        d: size.d,
        face,
        ..Default::default()
    };
    stand_on_floor(e, &mut item);
    snap_device(e, &mut item);
    item
}

/// Technical value of a part as a number ("Outside: 25" → 25).
pub fn tech_number(part: &crate::parts::Part, name: &str) -> Option<f32> {
    part.dimension_mm(name)
}

/// Clamping range of a cable gland part in mm (technical data "Clamping range min" /
/// "max"), from the project's parts or the catalog.
pub fn clamping_range(prj: &Project, part: &str) -> Option<(f32, f32)> {
    let p = prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == part)?;
    Some((tech_number(p, "Clamping range min")?, tech_number(p, "Clamping range max")?))
}

/// Faces an accessory can be mounted on (part data `Faces: Left, Right`); all faces
/// without data.
pub fn allowed_faces(part: &crate::parts::Part) -> Vec<Face> {
    match part.data_value("Faces").as_deref() {
        Some(list) => list.split([',', ';']).filter_map(Face::from_key).collect(),
        None => Face::ALL.to_vec(),
    }
}

/// An accessory (fan, gland, light, …) of a part with its top left corner at `at` on
/// a face: its size from the part, sticking out of the face as far as the part data
/// `Outside` says.
pub fn new_accessory(prj: &mut Project, part: &str, face: Face, at: P) -> PanelItem {
    let pd = part_by_number(prj, part).cloned();
    let [w, h, d] = pd.as_ref().and_then(|p| p.size()).unwrap_or([DEFAULT_DEVICE.0, DEFAULT_DEVICE.1, DEFAULT_DEVICE.2]);
    let outside = pd.as_ref().and_then(|p| tech_number(p, "Outside")).unwrap_or(0.0);
    PanelItem { id: prj.new_id(), kind: ItemKind::Accessory, part: part.into(), x: snap(at.x), y: snap(at.y), w, h, d, z: -outside, face, ..Default::default() }
}

/// Places an accessory on a face of an enclosure; on the wall of a room a part with a
/// mounting height (technical data) goes at that height.
pub fn place_accessory(prj: &mut Project, enc: usize, part: &str, face: Face, at: P) -> Option<u64> {
    prj.layout.enclosures.get(enc)?;
    let mut item = new_accessory(prj, part, face, at);
    stand_on_floor(&prj.layout.enclosures[enc], &mut item);
    // Outlets and switches go on the walls of a room at their usual height.
    if let Some(height) = part_by_number(prj, part).and_then(|p| tech_number(p, "Mounting height")) {
        at_mounting_height(&prj.layout.enclosures[enc], &mut item, height);
    }
    let id = item.id;
    prj.layout.enclosures[enc].items.push(item);
    Some(id)
}

/// Devices whose centre lies on a horizontal rail are centred on it and mounted on
/// top of it; otherwise they sit directly on the plate (or a wall of a room).
pub fn snap_to_rail(e: &Enclosure, item: &mut PanelItem) {
    item.seated_on_rail = false;
    if let Some(rail)=e.item(item.mounted_on).filter(|i|i.kind==ItemKind::Rail) {item.mounted_on=rail.mounted_on;}
    if rack::snap_into_rack(e, item) {
        return;
    }
    if item.kind != ItemKind::Device || (item.face != Face::Plate && !e.is_room()) {
        return;
    }
    let distance=|rail:&PanelItem| {
        let anchor=item.rail_anchor_for(!rail.horizontal());
        if rail.horizontal() {(anchor.y-rail.center().y).abs()} else {(anchor.x-rail.center().x).abs()}
    };
    let rail = e
        .items
        .iter()
        .filter(|r| {
            r.kind == ItemKind::Rail
                && r.face == item.face
                && if r.horizontal() {
                    let c=item.rail_anchor_for(false);(r.x..=r.x+r.w).contains(&c.x) && distance(r)<=item.h/2.
                } else {
                    let c=item.rail_anchor_for(true);(r.y..=r.y+r.h).contains(&c.y) && distance(r)<=item.w/2.
                }
        })
        .min_by(|a,b|distance(a).total_cmp(&distance(b)));
    match rail {
        Some(r) => item.seat_on(r),
        // Over a busbar system: mounted onto its bars.
        None => {
            item.rail_vertical=false;
            let support=e.item(item.mounted_on).filter(|i|i.kind==ItemKind::MountingPlate).map(|i|i.z+i.d)
                .unwrap_or(if item.mounting_explicit {item.z} else {0.});
            let on = busbars(e, item).next().map_or(support, |b| b.z + b.d);
            item.z = on;
        }
    }
}

/// Snap a single device to its support and to a nearby free side of another
/// device in the same row. Use the same result for the ghost, placement and drag.
pub fn snap_device(e: &Enclosure, item: &mut PanelItem) {
    snap_to_rail(e, item);
    if item.kind != ItemKind::Device || item.face != Face::Plate || is_rack(e) || e.is_room() || item.rail_vertical {
        return;
    }
    const REACH: f32 = 4.0;
    let mut best = None;
    let mut distance = f32::INFINITY;
    let area = mounting_area(e, item.face);
    let support_z = item.z + item.rail_mount.clip_on;
    let neighbors = || {
        e.items
            .iter()
            .filter(|n| n.kind == ItemKind::Device && n.id != item.id && n.face == item.face && (n.z + n.rail_mount.clip_on - support_z).abs() < 0.01)
    };
    for n in neighbors() {
        let y = n.rail_anchor().y - item.h / 2. - item.rail_mount.center_offset;
        if (y - item.y).abs() > REACH {
            continue;
        }
        for x in [n.x - item.w, n.x + n.w] {
            let dx = x - item.x;
            let dy = y - item.y;
            let d = dx * dx + dy * dy;
            if dx.abs() > REACH || d >= distance {
                continue;
            }
            let mut candidate = item.clone();
            candidate.x = x;
            candidate.y = y;
            snap_to_rail(e, &mut candidate);
            let rect = candidate.rect();
            if (candidate.y - y).abs() > 0.01 || (candidate.z - item.z).abs() > 0.01 || !area.contains_rect(&rect) {
                continue;
            }
            if neighbors().any(|other| {
                let r = other.rect();
                rect.min.x < r.max.x - 0.01 && r.min.x < rect.max.x - 0.01 && rect.min.y < r.max.y - 0.01 && r.min.y < rect.max.y - 0.01
            }) {
                continue;
            }
            best = Some(candidate);
            distance = d;
        }
    }
    if let Some(candidate) = best {
        *item = candidate;
    }
}

/// The busbar systems of the item's face it lies over.
fn busbars<'a>(e: &'a Enclosure, item: &'a PanelItem) -> impl Iterator<Item = &'a PanelItem> {
    let r = item.rect();
    e.items.iter().filter(move |b| {
        let rb = b.rect();
        b.kind == ItemKind::Busbar && b.face == item.face && b.id != item.id && r.min.x < rb.max.x && rb.min.x < r.max.x && r.min.y < rb.max.y && rb.min.y < r.max.y
    })
}

/// The busbar system a device is mounted on (by id): one it lies over, the device in
/// front of its bars. Devices on the same busbar system are connected by its bars.
pub fn busbar_of(e: &Enclosure, item: &PanelItem) -> Option<u64> {
    if item.kind != ItemKind::Device {
        return None;
    }
    busbars(e, item).find(|b| item.z >= b.z + b.d - 0.5).map(|b| b.id)
}

/// How far apart (mm) the busbar systems of two bayed enclosures may end at their common
/// side to be joined through it.
pub const BUSBAR_JOINT: f32 = 100.0;

/// The busbar system a device is mounted on, across a line-up: the enclosure and the
/// busbar item it begins with on the left. The busbar systems of bayed enclosures that
/// meet at their common side (the same part at the same height, their ends no more
/// than [`BUSBAR_JOINT`] mm apart) are one system, joined through the side; the devices
/// on it are connected by its bars in whichever of the enclosures they are (`pos`:
/// [`positions`]).
pub fn busbar_system(layout: &Layout, pos: &[V3], ei: usize, item: &PanelItem) -> Option<(usize, u64)> {
    let mut at = (ei, busbar_of(layout.enclosures.get(ei)?, item)?);
    while let Some(left) = joined_on_the_left(layout, pos, at) {
        at = left;
    }
    Some(at)
}

/// The busbar system in the enclosure bayed on the left that the busbar item `bar` of
/// an enclosure is joined with.
fn joined_on_the_left(layout: &Layout, pos: &[V3], (ei, bar): (usize, u64)) -> Option<(usize, u64)> {
    let left = neighbours(layout, ei).0?;
    let (e, l) = (&layout.enclosures[ei], &layout.enclosures[left]);
    let b = e.items.iter().find(|i| i.id == bar)?;
    let height = |k: usize, it: &PanelItem| pos[k][1] + it.y;
    let other = l.items.iter().find(|o| {
        let gap = (b.x + PLATE_MARGIN) + (l.width - (o.x + o.w + PLATE_MARGIN));
        o.kind == ItemKind::Busbar && o.face == b.face && o.part == b.part && (height(left, o) - height(ei, b)).abs() < 1.0 && (o.h - b.h).abs() < 1.0 && gap <= BUSBAR_JOINT
    })?;
    Some((left, other.id))
}

// ---------------------------------------------------------------------- checks

/// Pairs of items that occupy the same space (or are mounted over a cutout). Boxes
/// are checked first; where a device with a 3D model is involved, the models
/// themselves must penetrate each other (also door devices against the equipment
/// of the mounting plate).
pub fn collisions(e: &Enclosure, parts: &[crate::parts::Part]) -> Vec<(u64, u64)> {
    let mut v = Vec::new();
    for (i, a) in e.items.iter().enumerate() {
        for b in &e.items[i + 1..] {
            let ra = a.rect();
            let rb = b.rect();
            let eps = 0.01;
            let xy = ra.min.x < rb.max.x - eps && rb.min.x < ra.max.x - eps && ra.min.y < rb.max.y - eps && rb.min.y < ra.max.y - eps;
            if a.kind == ItemKind::Cutout || b.kind == ItemKind::Cutout {
                // Anything fastened to the plate / door must not sit over an opening;
                // accessories (fans, glands) are mounted into theirs.
                let other = if a.kind == ItemKind::Cutout { b } else { a };
                if a.face == b.face && xy && !matches!(other.kind, ItemKind::Cutout | ItemKind::Accessory | ItemKind::MountingPlate) && other.z <= eps {
                    v.push((a.id, b.id));
                }
                continue;
            }
            let (ba, bb) = (item_box(e, a), item_box(e, b));
            if !(0..3).all(|k| ba.0[k] < bb.1[k] - eps && bb.0[k] < ba.1[k] - eps) {
                continue;
            }
            // Crossing rails / ducts of the same kind are joined, not colliding.
            let joint = a.face == b.face && a.kind == b.kind && a.kind != ItemKind::Device && a.horizontal() != b.horizontal();
            if joint {
                continue;
            }
            let modelled = |it: &PanelItem| it.kind == ItemKind::Device && parts.iter().any(|p| p.number == it.part && p.model_3d.is_some());
            if (modelled(a) || modelled(b)) && !surfaces_intersect(&item_surface(e, a, parts), &item_surface(e, b, parts)) {
                continue;
            }
            v.push((a.id, b.id));
        }
    }
    v
}

/// Area of a face where items can be mounted: the plate, the door and the walls
/// within their flanges.
pub fn mounting_area(e: &Enclosure, face: Face) -> R {
    let (w, h) = e.face_size(face);
    let f = if face == Face::Plate { 0.0 } else { e.flange().min(w / 3.0).min(h / 3.0) };
    R::from_points(p(f, f), p(w - f, h - f))
}

/// Items that leave the mounting plate (door and walls: their area inside the flanges)
/// or reach further into the enclosure than it allows (door devices: the mounting
/// plate), and items on a side that is open (bayed).
pub fn outside(e: &Enclosure) -> Vec<u64> {
    let room = |f: Face| e.face_room(f);
    e.items
        .iter()
        .filter(|i| {
            let area = mounting_area(e, i.face).expand(0.01);
            let open = (i.face == Face::Left && e.build.left_open) || (i.face == Face::Right && e.build.right_open);
            !area.contains_rect(&i.rect())
                || open
                || match i.face {
                    Face::Door => item_box(e, i).0[2] < PLATE_Z,
                    f => i.z + i.d > room(f),
                }
        })
        .map(|i| i.id)
        .collect()
}

// ---------------------------------------------------------------------- routing

#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    /// Path on the plate (plate coordinates).
    pub points: Vec<P>,
    /// Length along the plate (without the vertical parts and reserve).
    pub plate_length: f32,
    pub via_ducts: bool,
    /// Stretches of the ducts the wire runs through.
    pub runs: Vec<DuctRun>,
}

/// Stretch of a duct a wire runs through: from / to along the duct's axis (x of
/// horizontal, y of vertical ducts), `from <= to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DuctRun {
    pub duct: u64,
    pub from: f32,
    pub to: f32,
}

/// Where a wire starts on the plate and in which direction it leaves its connection
/// point (`None`: no direction, e.g. the device centre).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WireEnd {
    pub pos: P,
    pub exit: Option<Exit>,
}

impl WireEnd {
    pub fn at(pos: P) -> WireEnd {
        WireEnd { pos, exit: None }
    }

    /// `q` lies in the direction the wire leaves in.
    fn ahead(&self, q: P) -> bool {
        let a = self.pos;
        match self.exit {
            Some(Exit::Up) => q.y < a.y - 0.5,
            Some(Exit::Down) => q.y > a.y + 0.5,
            Some(Exit::Left) => q.x < a.x - 0.5,
            Some(Exit::Right) => q.x > a.x + 0.5,
            Some(Exit::Front) | None => true,
        }
    }
}

/// Manhattan length of a polyline.
fn manhattan(a: P, b: P) -> f32 {
    (a.x - b.x).abs() + (a.y - b.y).abs()
}

fn closest_on_segment(q: P, a: P, b: P) -> P {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 <= f32::EPSILON {
        return a;
    }
    let t = (((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / l2).clamp(0.0, 1.0);
    a + ab * t
}

/// How far past the devices a wire without cable ducts runs (mm, see [`direct_route`]).
const DIRECT_CLEAR: f32 = 25.0;

/// The way of a wire in an enclosure without cable ducts: out of each connection point
/// in the direction it leaves it, past the devices there (the row on its rail), then
/// across (below a row both leave downwards, above one both leave upwards).
fn direct_route(e: &Enclosure, from: WireEnd, to: WireEnd) -> Route {
    let devices: Vec<R> = e.items.iter().filter(|i| i.kind == ItemKind::Device && i.face == Face::Plate).map(|i| i.rect()).collect();
    let out = |end: WireEnd| -> P {
        let mut q = end.pos;
        for _ in 0..devices.len() {
            // On its outline too (the connection points of most devices).
            let on = |r: &&R| q.x >= r.min.x - 0.5 && q.x <= r.max.x + 0.5 && q.y >= r.min.y - 0.5 && q.y <= r.max.y + 0.5;
            let Some(r) = devices.iter().find(on) else { break };
            q = match end.exit {
                Some(Exit::Up) => p(q.x, r.min.y - DIRECT_CLEAR),
                Some(Exit::Down) => p(q.x, r.max.y + DIRECT_CLEAR),
                Some(Exit::Left) => p(r.min.x - DIRECT_CLEAR, q.y),
                Some(Exit::Right) => p(r.max.x + DIRECT_CLEAR, q.y),
                Some(Exit::Front) | None => break,
            };
        }
        q
    };
    let (a, b) = (from.pos, to.pos);
    let (a1, b1) = (out(from), out(to));
    let vertical = |x: Option<Exit>| matches!(x, Some(Exit::Up | Exit::Down));
    // Moved on (`more`: downwards or to the right) past every device the run across at
    // `at` from `s` to `t` (along x when `along_x`) passes through.
    let past = |mut at: f32, s: f32, t: f32, along_x: bool, more: bool| {
        for _ in 0..devices.len() {
            let (lo, hi) = (s.min(t), s.max(t));
            let hit = devices.iter().find(|r| match along_x {
                true => at > r.min.y && at < r.max.y && r.max.x > lo && r.min.x < hi,
                false => at > r.min.x && at < r.max.x && r.max.y > lo && r.min.y < hi,
            });
            let Some(r) = hit else { break };
            at = match (along_x, more) {
                (true, true) => r.max.y + DIRECT_CLEAR,
                (true, false) => r.min.y - DIRECT_CLEAR,
                (false, true) => r.max.x + DIRECT_CLEAR,
                (false, false) => r.min.x - DIRECT_CLEAR,
            };
        }
        at
    };
    let mut points = vec![a, a1];
    match (from.exit, to.exit) {
        (Some(Exit::Down), Some(Exit::Down)) | (Some(Exit::Up), Some(Exit::Up)) => {
            let down = from.exit == Some(Exit::Down);
            let y = past(if down { a1.y.max(b1.y) } else { a1.y.min(b1.y) }, a1.x, b1.x, true, down);
            points.extend([p(a1.x, y), p(b1.x, y)]);
        }
        (Some(Exit::Left), Some(Exit::Left)) | (Some(Exit::Right), Some(Exit::Right)) => {
            let right = from.exit == Some(Exit::Right);
            let x = past(if right { a1.x.max(b1.x) } else { a1.x.min(b1.x) }, a1.y, b1.y, false, right);
            points.extend([p(x, a1.y), p(x, b1.y)]);
        }
        // Opposite: round the devices between at the nearer side.
        (Some(Exit::Up), Some(Exit::Down)) | (Some(Exit::Down), Some(Exit::Up)) => {
            let (left, right) = (past(a1.x, a1.y, b1.y, false, false), past(a1.x, a1.y, b1.y, false, true));
            let x = if (a1.x - left).abs() + (b1.x - left).abs() <= (right - a1.x).abs() + (right - b1.x).abs() { left } else { right };
            points.extend([p(x, a1.y), p(x, b1.y)]);
        }
        (Some(Exit::Left), Some(Exit::Right)) | (Some(Exit::Right), Some(Exit::Left)) => {
            let (up, down) = (past(a1.y, a1.x, b1.x, true, false), past(a1.y, a1.x, b1.x, true, true));
            let y = if (a1.y - up).abs() + (b1.y - up).abs() <= (down - a1.y).abs() + (down - b1.y).abs() { up } else { down };
            points.extend([p(a1.x, y), p(b1.x, y)]);
        }
        (x, _) if vertical(x) => points.push(p(a1.x, b1.y)),
        _ => points.push(p(b1.x, a1.y)),
    }
    points.extend([b1, b]);
    points.dedup_by(|q, r| q.same(*r));
    let plate_length = points.windows(2).map(|s| manhattan(s[0], s[1])).sum();
    Route { points, plate_length, via_ducts: false, runs: Vec::new() }
}

/// Routes a wire from `a` to `b` through the cable duct network (shortest path).
/// Without ducts the wire runs directly (orthogonally, see [`direct_route`]).
pub fn route(e: &Enclosure, a: P, b: P) -> Route {
    route_ends(e, WireEnd::at(a), WireEnd::at(b))
}

/// Like [`route`]; each end enters the nearest duct in the direction it leaves its
/// connection point (a terminal at the top goes into the duct above), or the nearest
/// duct if there is none in that direction.
pub fn route_ends(e: &Enclosure, from: WireEnd, to: WireEnd) -> Route {
    route_weighted(e, from, to, &|_, _, _| 1.0)
}

/// Route on the door (door coordinates) through the cable ducts on the door.
pub fn route_door(e: &Enclosure, from: P, to: P) -> Route {
    let ducts = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Door).map(|i| PanelItem { face: Face::Plate, ..i.clone() }).collect();
    route_ends(&Enclosure { items: ducts, ..Default::default() }, WireEnd::at(from), WireEnd::at(to))
}

/// Like [`route_ends`], but the way along a duct counts `cost(duct id, from, to)`
/// times its length (`from <= to` along the duct's axis), so that the shortest path
/// avoids expensive stretches. The route's length stays the real length.
pub fn route_weighted(e: &Enclosure, from: WireEnd, to: WireEnd, cost: &dyn Fn(u64, f32, f32) -> f32) -> Route {
    let (a, b) = (from.pos, to.pos);
    let direct = || direct_route(e, from, to);
    let ducts: Vec<&PanelItem> = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Plate).collect();
    if ducts.is_empty() {
        return direct();
    }
    // Nodes: per duct, the points on its centre line that matter.
    let mut nodes: Vec<P> = vec![a, b];
    let mut on_duct: Vec<Vec<usize>> = vec![Vec::new(); ducts.len()];
    let mut edges: Vec<(usize, usize, f32)> = Vec::new();
    let add_node = |nodes: &mut Vec<P>, q: P| {
        nodes.push(q);
        nodes.len() - 1
    };
    for (di, d) in ducts.iter().enumerate() {
        let (s, t) = d.center_line();
        let ns = add_node(&mut nodes, s);
        let nt = add_node(&mut nodes, t);
        on_duct[di].extend([ns, nt]);
    }
    // Junctions between ducts (crossing, T or touching at the ends).
    for i in 0..ducts.len() {
        for j in i + 1..ducts.len() {
            let (ra, rb) = (ducts[i].rect().expand(1.0), ducts[j].rect().expand(1.0));
            let overlap = ra.min.x <= rb.max.x && rb.min.x <= ra.max.x && ra.min.y <= rb.max.y && rb.min.y <= ra.max.y;
            if !overlap {
                continue;
            }
            let (ai, aj) = (ducts[i].center_line(), ducts[j].center_line());
            // Closest pair of points between the two centre lines (good enough for
            // axis parallel ducts).
            let candidates = [aj.0, aj.1, closest_on_segment(ai.0, aj.0, aj.1), closest_on_segment(ai.1, aj.0, aj.1)];
            let mut best: Option<(P, P, f32)> = None;
            for c in candidates {
                let on_i = closest_on_segment(c, ai.0, ai.1);
                let on_j = closest_on_segment(on_i, aj.0, aj.1);
                let dist = manhattan(on_i, on_j);
                if best.map(|bst| dist < bst.2).unwrap_or(true) {
                    best = Some((on_i, on_j, dist));
                }
            }
            if let Some((pi, pj, dist)) = best {
                let ni = add_node(&mut nodes, pi);
                let nj = add_node(&mut nodes, pj);
                on_duct[i].push(ni);
                on_duct[j].push(nj);
                edges.push((ni, nj, dist));
            }
        }
    }
    // Entries: a wire leaves its device into the nearest duct in its direction (not
    // across the plate to some farther duct).
    for (end, w) in [(0usize, from), (1usize, to)] {
        let q = w.pos;
        let all: Vec<(usize, P, f32)> = ducts
            .iter()
            .enumerate()
            .map(|(di, d)| {
                let (s, t) = d.center_line();
                let c = closest_on_segment(q, s, t);
                (di, c, manhattan(q, c))
            })
            .collect();
        let ahead: Vec<(usize, P, f32)> = all.iter().copied().filter(|e| w.ahead(e.1)).collect();
        let entries = if ahead.is_empty() { all } else { ahead };
        let nearest = entries.iter().map(|e| e.2).fold(f32::INFINITY, f32::min);
        for (di, c, dist) in entries.into_iter().filter(|e| e.2 <= nearest + 1.0) {
            let n = add_node(&mut nodes, c);
            on_duct[di].push(n);
            edges.push((end, n, dist));
        }
    }
    // Along each duct: connect consecutive nodes.
    let mut along: HashMap<(usize, usize), usize> = HashMap::new();
    for (di, d) in ducts.iter().enumerate() {
        let horizontal = d.horizontal();
        let list = &mut on_duct[di];
        list.sort_by(|&x, &y| {
            let (px, py) = (nodes[x], nodes[y]);
            if horizontal { px.x.total_cmp(&py.x) } else { px.y.total_cmp(&py.y) }
        });
        for w in list.windows(2) {
            edges.push((w[0], w[1], nodes[w[0]].dist(nodes[w[1]])));
            along.insert((w[0].min(w[1]), w[0].max(w[1])), di);
        }
    }
    // Dijkstra (small graphs, simple O(n²)) on the weighted lengths; `real` keeps the
    // real length of the path.
    let axis = |di: usize, q: P| if ducts[di].horizontal() { q.x } else { q.y };
    let n = nodes.len();
    let mut adj: Vec<Vec<(usize, f32, f32)>> = vec![Vec::new(); n];
    for &(x, y, len) in &edges {
        let weight = match along.get(&(x.min(y), x.max(y))) {
            Some(&di) => {
                let (s, t) = (axis(di, nodes[x]), axis(di, nodes[y]));
                len * cost(ducts[di].id, s.min(t), s.max(t)).max(1.0)
            }
            None => len,
        };
        adj[x].push((y, weight, len));
        adj[y].push((x, weight, len));
    }
    let mut dist = vec![f32::INFINITY; n];
    let mut real = vec![0.0f32; n];
    let mut prev = vec![usize::MAX; n];
    let mut done = vec![false; n];
    dist[0] = 0.0;
    for _ in 0..n {
        let Some(u) = (0..n).filter(|&i| !done[i] && dist[i].is_finite()).min_by(|&x, &y| dist[x].total_cmp(&dist[y])) else {
            break;
        };
        if u == 1 {
            break;
        }
        done[u] = true;
        for &(v, w, len) in &adj[u] {
            if dist[u] + w < dist[v] {
                dist[v] = dist[u] + w;
                real[v] = real[u] + len;
                prev[v] = u;
            }
        }
    }
    if !dist[1].is_finite() {
        return direct();
    }
    let mut path = vec![1];
    while *path.last().unwrap() != 0 {
        path.push(prev[*path.last().unwrap()]);
    }
    path.reverse();
    let mut runs: Vec<DuctRun> = Vec::new();
    for w in path.windows(2) {
        let Some(&di) = along.get(&(w[0].min(w[1]), w[0].max(w[1]))) else { continue };
        let (s, t) = (axis(di, nodes[w[0]]), axis(di, nodes[w[1]]));
        let run = DuctRun { duct: ducts[di].id, from: s.min(t), to: s.max(t) };
        match runs.last_mut() {
            Some(last) if last.duct == run.duct && (last.to - run.from).abs() < 0.01 => last.to = run.to,
            Some(last) if last.duct == run.duct && (run.to - last.from).abs() < 0.01 => last.from = run.from,
            _ => runs.push(run),
        }
    }
    let mut points: Vec<P> = Vec::new();
    for idx in path {
        let q = nodes[idx];
        if let Some(&last) = points.last() {
            // Orthogonal legs for the way into / out of the duct.
            let last: P = last;
            if last.x != q.x && last.y != q.y {
                points.push(p(q.x, last.y));
            }
        }
        if points.last().map(|l: &P| !l.same(q)).unwrap_or(true) {
            points.push(q);
        }
    }
    Route { points, plate_length: real[1], via_ducts: true, runs }
}

/// A routed wire of the virtual wiring.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedWire {
    pub enclosure: usize,
    pub from: String,
    pub to: String,
    pub net: String,
    pub color: String,
    pub cross_section: String,
    /// Wire number of the connection.
    pub number: String,
    /// Cable the wire is a core of; empty for a single wire.
    pub cable: String,
    pub route: Route,
    /// Total length including the way down to the plate and the reserve.
    pub length: f32,
    /// (from device item, to device item)
    pub items: (u64, u64),
    /// How exactly the two ends are known.
    pub ends: (EndKind, EndKind),
    /// Connection point of an end on the door (enclosure coordinates, door closed);
    /// the wire runs from there along the door to the hinge and over the hinge loop.
    pub door: (Option<V3>, Option<V3>),
    /// Routes on the door (door coordinates) of the two ends: from the connection point
    /// to the hinge; between two door devices the whole wire is the first one. The
    /// second end is in the last enclosure of the wire.
    pub door_routes: (Option<Route>, Option<Route>),
    /// Connection point of an end on the plate or a wall (enclosure coordinates; the
    /// middle of the device front without one): the wire runs from there to the plate.
    pub terminals: (Option<V3>, Option<V3>),
    /// Further stretches through cable passages into neighbouring enclosures of the
    /// line-up; empty if the wire stays in `enclosure`.
    pub legs: Vec<Leg>,
}

/// Where a routed wire end comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndKind {
    /// The device centre: no connection point of its parts matches.
    Centre,
    /// A connection point generated from the function templates.
    Generated,
    /// A connection point from the part data.
    Exact,
}

impl RoutedWire {
    /// How the wire is laid, for lists.
    pub fn routing(&self) -> &'static str {
        if !self.legs.is_empty() {
            return tr("through cable passages into the neighbouring enclosure");
        }
        match (self.on_door(), self.door.0.is_some() || self.door.1.is_some(), self.route.via_ducts) {
            (true, _, _) => tr("on the door"),
            (_, true, true) => tr("over the door hinge, via cable ducts"),
            (_, true, false) => tr("over the door hinge, direct"),
            (_, _, true) => tr("via cable ducts"),
            _ => tr("direct (no ducts)"),
        }
    }

    /// The routes on the mounting plates: (enclosure, route) of the first enclosure and
    /// of the legs.
    pub fn stretches(&self) -> impl Iterator<Item = (usize, &Route)> {
        std::iter::once((self.enclosure, &self.route)).chain(self.legs.iter().map(|l| (l.enclosure, &l.route)))
    }

    /// Like [`RoutedWire::stretches`], with the cable passages (line-up coordinates)
    /// each stretch comes in through and leaves through.
    pub fn stretch_passages(&self) -> Vec<(usize, &Route, Option<V3>, Option<V3>)> {
        let entries = std::iter::once(None).chain(self.legs.iter().map(|l| Some(l.passage)));
        let exits = self.legs.iter().map(|l| Some(l.passage)).chain(std::iter::once(None));
        self.stretches().zip(entries.zip(exits)).map(|((e, r), (a, b))| (e, r, a, b)).collect()
    }

    /// Names of the enclosures the wire runs through ("+CAB1 / +CAB2").
    pub fn enclosure_names(&self, layout: &Layout) -> String {
        self.stretches().filter_map(|(e, _)| layout.enclosures.get(e)).map(|e| e.name.as_str()).collect::<Vec<_>>().join(" / ")
    }

    /// Enclosure of the second end.
    pub fn last_enclosure(&self) -> usize {
        self.legs.last().map(|l| l.enclosure).unwrap_or(self.enclosure)
    }

    /// The wire runs through the enclosure.
    pub fn in_enclosure(&self, ei: usize) -> bool {
        self.stretches().any(|(e, _)| e == ei)
    }

    /// Length on the mounting plates and through the cable passages.
    pub fn plate_length(&self) -> f32 {
        self.route.plate_length + self.legs.iter().map(|l| l.route.plate_length + l.passage_length).sum::<f32>()
    }

    /// Polylines of the wire on a face of an enclosure (plate or door coordinates; none
    /// on the walls).
    pub fn paths_in(&self, ei: usize, face: Face) -> Vec<&[P]> {
        match face {
            Face::Plate => self.stretches().filter(|(e, _)| *e == ei).map(|(_, r)| r.points.as_slice()).collect(),
            Face::Door => {
                let first = self.door_routes.0.as_ref().filter(|_| self.enclosure == ei);
                let last = self.door_routes.1.as_ref().filter(|_| self.last_enclosure() == ei);
                [first, last].into_iter().flatten().map(|r| r.points.as_slice()).collect()
            }
            _ => Vec::new(),
        }
        .into_iter()
        .filter(|p| p.len() > 1)
        .collect()
    }

    /// Both ends are on the door: the wire stays there.
    pub fn on_door(&self) -> bool {
        self.door.0.is_some() && self.door.1.is_some()
    }

    /// Stable identification of a wire across re-analysis (its two ends).
    pub fn key(&self) -> (String, String) {
        (self.from.clone(), self.to.clone())
    }
}

/// Distance from `q` to a route polyline.
pub fn route_distance(points: &[P], q: P) -> f32 {
    points.windows(2).map(|s| crate::geom::dist_to_segment(q, s[0], s[1])).fold(f32::INFINITY, f32::min)
}

/// Indices of the wires of an enclosure whose route on `face` passes within `tol` of
/// `q`, nearest first.
pub fn wires_at(wiring: &[RoutedWire], enclosure: usize, face: Face, q: P, tol: f32) -> Vec<usize> {
    let mut v: Vec<(usize, f32)> = wiring
        .iter()
        .enumerate()
        .filter(|(_, w)| w.in_enclosure(enclosure))
        .map(|(i, w)| (i, w.paths_in(enclosure, face).into_iter().map(|pts| route_distance(pts, q)).fold(f32::INFINITY, f32::min)))
        .filter(|(_, d)| *d <= tol)
        .collect();
    v.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    v.into_iter().map(|(i, _)| i).collect()
}

/// Clicking repeatedly on overlapping wires steps through them: the candidate after
/// the current one, or the first.
pub fn next_candidate(candidates: &[usize], current: Option<usize>) -> Option<usize> {
    match current.and_then(|c| candidates.iter().position(|x| *x == c)) {
        Some(pos) => Some(candidates[(pos + 1) % candidates.len()]),
        None => candidates.first().copied(),
    }
}

/// Unrouted connection (at least one device not placed or in different enclosures).
#[derive(Clone, Debug)]
pub struct OpenWire {
    pub from: String,
    pub to: String,
    pub net: String,
    pub reason: &'static str,
}

/// Virtual wiring: routes the point-to-point connections of the schematic (see
/// [`connections`]) between placed devices through the cable ducts. Devices mounted
/// on the same busbar system are connected by its bars, not by a wire.
pub fn virtual_wiring(prj: &Project, conns: &[Connection]) -> (Vec<RoutedWire>, Vec<OpenWire>) {
    let mut routed = Vec::new();
    let mut ends = Vec::new();
    let mut open = Vec::new();
    let mut points: HashMap<u64, Vec<PlacedPoint>> = HashMap::new();
    let pos = positions(&prj.layout);
    for c in conns {
        let sa = &prj.pages[c.a.page].symbols[c.a.sym];
        let sb = &prj.pages[c.b.page].symbols[c.b.sym];
        let from = crate::netlist::pin_designation(prj, c.a);
        let to = crate::netlist::pin_designation(prj, c.b);
        if sa.tag == sb.tag && (sa.label==sb.label || prj.symbol(&sa.sym).is_none_or(|d|d.func!=crate::symbols::Func::Terminal)
            || prj.symbol(&sb.sym).is_none_or(|d|d.func!=crate::symbols::Func::Terminal)) {
            continue; // internal connection (e.g. bridged on the device)
        }
        let (Some((ea, ia)), Some((eb, ib))) = (locate_symbol(prj, sa), locate_symbol(prj, sb)) else {
            open.push(OpenWire { from, to, net: c.net.clone(), reason: "device not placed" });
            continue;
        };
        if sa.tag==sb.tag && ia.id==ib.id {continue;}
        if busbar_system(&prj.layout, &pos, ea, ia).is_some_and(|system| busbar_system(&prj.layout, &pos, eb, ib) == Some(system)) {
            continue; // connected by the bars of the busbar system both are mounted on
        }
        let mut end = |ei: usize, it: &PanelItem, s, pin| {
            let e = &prj.layout.enclosures[ei];
            let placed = points.entry(it.id).or_insert_with(|| device_points(prj, it));
            let point = point_of(prj, placed, s, pin);
            let kind = match point {
                None => EndKind::Centre,
                Some(pt) if pt.exact => EndKind::Exact,
                Some(_) => EndKind::Generated,
            };
            let (w, extra, door_route) = wiring_end(e, it, point);
            let door = (it.face == Face::Door).then(|| door_point(e, it, point));
            let c = it.center();
            let at = point.map(|pt| pt.pos).unwrap_or([c.x, c.y, it.z + it.d]);
            let terminal = (it.face != Face::Door).then(|| face_point(e, it.face, at));
            (w, extra, kind, door, door_route, terminal)
        };
        let (pa, la, ka, da, ra, ta) = end(ea, ia, sa, c.a.pin);
        let (pb, lb, kb, db, rb, tb) = end(eb, ib, sb, c.b.pin);
        let e = &prj.layout.enclosures[ea];
        let (r, length, door_routes, legs) = match (da, db) {
            // Both ends on the door: along the door, not into the cabinet.
            (Some(a), Some(b)) if ea == eb => {
                let on_door = route_door(e, p(a[0], a[1]), p(b[0], b[1]));
                let bundle = door_bundle(e, on_door.via_ducts);
                let length = (a[2] - bundle).abs() + on_door.plate_length + (b[2] - bundle).abs();
                (Route { points: Vec::new(), plate_length: 0.0, via_ducts: false, runs: Vec::new() }, length, (Some(on_door), None), Vec::new())
            }
            _ if ea == eb => {
                let r = route_ends(e, pa, pb);
                let length = r.plate_length + la + lb;
                (r, length, (ra, rb), Vec::new())
            }
            // Into another enclosure of the line-up through the cable passages.
            _ => match route_across(&prj.layout, &pos, (ea, pa), (eb, pb)) {
                Ok((r, legs)) => {
                    let plate = r.plate_length + legs.iter().map(|l| l.route.plate_length + l.passage_length).sum::<f32>();
                    (r, plate + la + lb, (ra, rb), legs)
                }
                Err(reason) => {
                    open.push(OpenWire { from, to, net: c.net.clone(), reason });
                    continue;
                }
            },
        };
        ends.push((pa, pb));
        let length = length + prj.layout.wire_reserve;
        routed.push(RoutedWire {
            enclosure: ea,
            from,
            to,
            net: c.net.clone(),
            color: c.color.clone(),
            cross_section: c.cross_section.clone(),
            number: c.number.clone(),
            cable: c.cable.clone(),
            route: r,
            length,
            items: (ia.id, ib.id),
            ends: (ka, kb),
            door: (da, db),
            door_routes,
            terminals: (ta, tb),
            legs,
        });
    }
    reroute_overfull(prj, &mut routed, &ends);
    (routed, open)
}

/// Second pass of the virtual wiring when a duct is filled above the limit: all wires
/// are routed again one by one, thickest first, and keep out of stretches they would
/// fill above the limit unless every other way is more than [`OVERFILL_COST`] times
/// longer. Without overfull ducts the shortest routes stay.
fn reroute_overfull(prj: &Project, routed: &mut [RoutedWire], ends: &[(WireEnd, WireEnd)]) {
    let limit = prj.layout.duct_fill_limit;
    if !duct_fill(prj, routed, &[]).iter().any(|f| f.percent > limit) {
        return;
    }
    let areas: Vec<f32> = routed.iter().map(|w| wire_area(&w.cross_section)).collect();
    let mut order: Vec<usize> = (0..routed.len()).collect();
    order.sort_by(|&a, &b| areas[b].total_cmp(&areas[a]));
    let mut loads = DuctLoads::new(prj);
    // Wires into other enclosures keep their routes and load the ducts first.
    for (w, area) in routed.iter().zip(&areas).filter(|(w, _)| !w.legs.is_empty()) {
        for (_, r) in w.stretches() {
            loads.add(r, *area);
        }
    }
    order.retain(|&i| !routed[i].on_door() && routed[i].legs.is_empty());
    for i in order {
        let e = &prj.layout.enclosures[routed[i].enclosure];
        let (from, to) = ends[i];
        let area = areas[i];
        let r = route_weighted(e, from, to, &|duct, s, t| loads.cost(duct, s, t, area));
        loads.add(&r, area);
        let w = &mut routed[i];
        w.length += r.plate_length - w.route.plate_length;
        w.route = r;
    }
}

/// Distance of the wire bundle on the door behind the door leaf.
pub const DOOR_BUNDLE: f32 = 15.0;

/// Connection point of a door device in enclosure coordinates (door closed); without
/// a connection point the centre of the device's rear.
pub fn door_point(e: &Enclosure, it: &PanelItem, point: Option<&PlacedPoint>) -> V3 {
    match point {
        Some(pt) => [pt.pos[0], pt.pos[1], e.depth - pt.pos[2]],
        None => [it.x + it.w / 2.0, it.y + it.h / 2.0, e.depth - it.z - it.d],
    }
}

/// Depth of the wires on the door: in the middle of the door ducts they run through,
/// otherwise a bundle just behind the door leaf.
pub fn door_bundle(e: &Enclosure, via_ducts: bool) -> f32 {
    let duct = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Door).map(|i| i.z + i.d / 2.0).reduce(f32::min);
    match duct {
        Some(z) if via_ducts => e.depth - z,
        _ => e.depth - DOOR_BUNDLE,
    }
}

/// 3D path of a wire between two door devices along its door route (all on the door).
pub fn door_jumper_path(e: &Enclosure, a: V3, b: V3, route: &Route) -> Vec<V3> {
    let bundle = door_bundle(e, route.via_ducts);
    let mut v = vec![a];
    v.extend(route.points.iter().map(|q| [q.x, q.y, bundle]));
    v.push(b);
    v
}

/// 3D path of a door wire end from its connection point to where it joins the wiring
/// on the plate (`plate`, enclosure coordinates): back into the door ducts (or the
/// bundle behind the door), along its door route to the hinge, over the hinge loop to
/// the plate. The first `n` points lie on the door and swing with it.
pub fn door_path(e: &Enclosure, at: V3, route: &Route, plate: V3) -> (Vec<V3>, usize) {
    let bundle = door_bundle(e, route.via_ducts);
    let mut pts = vec![at];
    pts.extend(route.points.iter().map(|q| [q.x, q.y, bundle]));
    let hinge = pts.last().copied().unwrap_or(at);
    let n = pts.len();
    // Down beside the hinge, clear of the cable duct along the plate's edge, then
    // straight into the duct through its wall.
    pts.push([hinge[0], plate[1], e.depth - 3.0 * DOOR_BUNDLE]);
    pts.push([hinge[0], plate[1], plate[2]]);
    pts.push(plate);
    (pts, n)
}

/// Where a wire of a device joins the wiring on the plate, and the length from its
/// connection point to there: down to the plate, or for door devices along the door
/// (through its cable ducts) to the hinge (left) and across the hinge loop; door
/// devices also return their route on the door. Without a connection point the wire
/// starts at the device centre, at its front.
pub fn wiring_end(e: &Enclosure, it: &PanelItem, point: Option<&PlacedPoint>) -> (WireEnd, f32, Option<Route>) {
    let (at, height) = match point {
        Some(pt) => (p(pt.pos[0], pt.pos[1]), pt.pos[2]),
        None => (it.center(), it.z + it.d),
    };
    match it.face {
        Face::Plate => (WireEnd { pos: at, exit: point.map(|pt| pt.exit) }, height, None),
        Face::Door => {
            let (_, ph) = e.plate_size();
            let r = route_door(e, at, p(DOOR_FRAME / 2.0, at.y));
            let hinge = r.points.last().copied().unwrap_or(at);
            let length = r.plate_length + height + DOOR_LOOP + (e.depth - PLATE_Z);
            (WireEnd::at(p(0.0, (hinge.y - PLATE_MARGIN).clamp(0.0, ph))), length, Some(r))
        }
        wall => {
            // Devices on the walls (fans, lights): straight to the nearest plate edge.
            let at = face_point(e, wall, [at.x, at.y, height]);
            let (pw, ph) = e.plate_size();
            let q = p((at[0] - PLATE_MARGIN).clamp(0.0, pw), (at[1] - PLATE_MARGIN).clamp(0.0, ph));
            let length = (at[2] - PLATE_Z).abs() + (at[0] - PLATE_MARGIN - q.x).abs() + (at[1] - PLATE_MARGIN - q.y).abs();
            (WireEnd::at(q), length, None)
        }
    }
}

/// Physical device bindings remain material devices without a schematic function.
pub fn material_tags(prj:&Project)->std::collections::BTreeSet<&String> {
    prj.layout.enclosures.iter().flat_map(|e|std::iter::once(&e.device_tag)
        .chain(e.items.iter().filter(|i|i.kind==ItemKind::Device).map(|i|&i.tag)))
        .filter(|tag|!tag.is_empty()).collect()
}

/// Material for the BOM: (part number, quantity, where used) of the enclosures and
/// their components, rails, ducts, busbars, accessories and fastening hardware. Rails /
/// ducts are counted by cut length.
pub fn layout_material(prj: &Project) -> Vec<(String, f32, Vec<String>)> {
    let mut out: Vec<(String, f32, Vec<String>)> = Vec::new();
    let add = |out: &mut Vec<(String, f32, Vec<String>)>, part: &str, qty: f32, used: String| {
        if part.is_empty() {
            return;
        }
        match out.iter_mut().find(|x| x.0 == part) {
            Some(x) => {
                x.1 += qty;
                if !x.2.contains(&used) {
                    x.2.push(used);
                }
            }
            None => out.push((part.to_string(), qty, vec![used])),
        }
    };
    for e in &prj.layout.enclosures {
        if !e.bom_excluded && !crate::parts::excluded_from_bom(prj,&e.device_tag) {
            for c in components(e) {add(&mut out, &c.part, c.count as f32, e.name.clone());}
        }
        let billed=|i:&&PanelItem|!i.bom_excluded && !crate::parts::excluded_from_bom(prj,&i.tag);
        for a in e.items.iter().filter(|i| matches!(i.kind,ItemKind::Accessory|ItemKind::MountingPlate)).filter(billed) {
            add(&mut out, &a.part, 1.0, e.name.clone());
        }
        let standoffs: usize = e
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Rail && i.face == Face::Plate && mounting_height(e,i).is_some_and(|h|h>0.01))
            .filter(|i| prj.layout.hardware || !(i.mounting_explicit || i.mounted_on != 0))
            .map(|i| crate::production::item_holes(i, &prj.parts).len())
            .sum();
        if standoffs > 0 {
            add(&mut out, crate::parts::mechanics::RAIL_STANDOFF, standoffs as f32, e.name.clone());
        }
        let mut lengths: Vec<(String, f32)> = Vec::new();
        for i in e.items.iter().filter(|i| i.kind.is_profile()).filter(billed) {
            match lengths.iter_mut().find(|x| x.0 == i.part) {
                Some(x) => x.1 += i.length(),
                None => lengths.push((i.part.clone(), i.length())),
            }
        }
        for (part, mm) in lengths {
            let pd = prj.parts.iter().find(|x| x.number == part);
            // Parts sold per metre are counted in metres, others in pieces of their length.
            let qty = match pd {
                Some(pd) if pd.quantity_unit == "m" => (mm / 1000.0 * 100.0).ceil() / 100.0,
                Some(pd) if pd.width > 0.0 => (mm / pd.width).ceil(),
                _ => (mm / 1000.0 * 100.0).ceil() / 100.0,
            };
            add(&mut out, &part, qty, e.name.clone());
        }
        let holders = crate::production::busbar_holders(e);
        if holders > 0 {
            add(&mut out, crate::production::BUSBAR_HOLDER, holders as f32, e.name.clone());
        }
    }
    // The modules and the mounting of the PV arrays on the roofs.
    for b in &prj.layout.buildings {
        for a in b.roofs.iter().flat_map(|r| &r.solar) {
            for (part, qty) in solar::material(prj, a) {
                add(&mut out, &part, qty, format!("{} {}", b.name, a.name));
            }
        }
    }
    if prj.layout.hardware {
        for (part, qty, used) in fastening_material(prj) {
            for (k, name) in used.into_iter().enumerate() {
                add(&mut out, &part, if k == 0 { qty as f32 } else { 0.0 }, name);
            }
        }
    }
    out
}

/// Item ids involved in any problem of the checks (warnings, for highlighting).
pub fn problem_items(prj: &Project, e: &Enclosure) -> BTreeSet<u64> {
    problems_of(&validate(prj, e))
}

/// Item ids involved in the warnings among `findings` of the checks.
pub fn problems_of(findings: &[Finding]) -> BTreeSet<u64> {
    findings.iter().filter(|f| f.severity == crate::issue::Severity::Warning).flat_map(|f| f.items.iter().copied()).collect()
}

#[cfg(test)]
mod tests;
