//! Data of `crate::panel` that the project file holds (see `docs/architecture.md`).


use super::is_zero;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub enclosures: Vec<Enclosure>,
    /// Additional wire length per connection (stripping, reserve loops) in mm.
    pub wire_reserve: f32,
    /// Wire ends get ferrules (production wire list, bill of materials).
    pub ferrules: bool,
    /// Highest fill level of the cable ducts in percent (project check).
    pub duct_fill_limit: f32,
    /// Screws, nuts and rivet nuts of the fastening systems in the bill of materials.
    pub hardware: bool,
    /// 3D macros of the project (groups of items to insert again).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub macros: Vec<PanelMacro>,
    /// Rules of the 3D routing.
    #[serde(skip_serializing_if = "super::routing3d::RoutingRules::is_default")]
    pub routing: super::routing3d::RoutingRules,
    /// Routes frozen for production.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub frozen: Vec<super::routing3d::FrozenRoute>,
    /// Rules of the layout checks: clearances, collision classes, heights, thermal
    /// and manufacturing limits.
    #[serde(skip_serializing_if = "PanelRules::is_default")]
    pub checks: PanelRules,
    /// Machines, machining strategy, copper bars, jobs and production feedback.
    #[serde(skip_serializing_if = "super::manufacturing::Manufacturing::is_default")]
    pub manufacturing: super::manufacturing::Manufacturing,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Enclosure {
    pub id: u64,
    /// Location designation, e.g. "+CAB1".
    pub name: String,
    pub part: String,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    /// Space kept free behind the door (door-mounted devices, handles), mm.
    pub door_clearance: f32,
    pub items: Vec<PanelItem>,
    /// Installation and temperatures for the climate calculation.
    #[serde(skip_serializing_if = "super::climate::Climate::is_default")]
    pub climate: super::climate::Climate,
    /// Enclosure system and its components (plinth, side panels, gland plate).
    #[serde(skip_serializing_if = "Build::is_default")]
    pub build: Build,
    /// Groups (assemblies) of items.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ItemGroup>,
    /// Id of the enclosure this one is bayed to with its left side: it stands right of
    /// that one in a line-up. 0 for none.
    #[serde(skip_serializing_if = "is_zero")]
    pub bayed_to: u64,
    /// Routing paths through the free space (3D routing).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<super::routing3d::RoutingPath>,
    /// Space kept free, or for one class only (3D routing).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub reservations: Vec<super::routing3d::Reservation>,
    /// Height of the enclosure's bottom (without plinth) above the floor, mm; unknown
    /// for wall mounted enclosures (heights are not checked).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elevation: Option<f32>,
    /// Place of the line-up in the room (set on its left enclosure); otherwise from the
    /// plant layout or next to the other line-ups ([`site_poses`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<SitePlace>,
    /// A cabinet or a room (see `crate::panel::rooms`).
    #[serde(skip_serializing_if = "EnclosureKind::is_cabinet")]
    pub kind: EnclosureKind,
    /// Id of the room the line-up stands in (set on its left enclosure), 0 for none;
    /// `site` is then measured from the room's rear left inside corner.
    #[serde(skip_serializing_if = "is_zero")]
    pub room: u64,
    /// Rooms: corners of the floor plan (x, z in mm, inside surfaces of the walls, walls
    /// at right angles); empty for the rectangle of `width` × `depth`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<[f32; 2]>,
    /// Rooms: walls inside the room.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub inner_walls: Vec<InnerWall>,
}

/// What an [`Enclosure`] is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EnclosureKind {
    #[default]
    Cabinet,
    /// A room of a building: cabinets stand in it; its walls (`Wall(n)` along its
    /// outline) and ceiling (`Roof`) carry rails, cable ducts and wall-mounted devices.
    /// It has no mounting plate, door or floor slab.
    Room,
}

impl EnclosureKind {
    pub fn is_cabinet(&self) -> bool {
        *self == EnclosureKind::Cabinet
    }
}

/// A group (assembly) of items in an enclosure.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemGroup {
    pub id: u64,
    pub name: String,
    /// Macro it was inserted from.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
}

/// Place of a line-up in the room, floor plan in mm like the 3D view (x to the right,
/// z towards the front): the rear left corner of the line-up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SitePlace {
    pub x: f32,
    pub z: f32,
    /// Turn about the vertical in degrees, counter-clockwise seen from above: at 0 the
    /// fronts face +z, at 90 they face +x.
    pub angle: f32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelItem {
    pub id: u64,
    pub kind: ItemKind,
    /// Device tag (devices only).
    pub tag: String,
    pub part: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Height of the item (extent away from the plate).
    pub d: f32,
    /// Base height above the plate (devices on a rail sit on the rail).
    pub z: f32,
    #[serde(default, skip_serializing_if = "openecraft_parts::RailMount::is_centered")]
    pub rail_mount: openecraft_parts::RailMount,
    /// Round cutout (diameter = width).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub round: bool,
    #[serde(skip_serializing_if = "is_plate")]
    pub face: Face,
    #[serde(skip_serializing_if = "Fastening::is_auto")]
    pub fastening: Fastening,
    /// Parametric position relative to the edges of its face.
    #[serde(skip_serializing_if = "Anchor::is_free")]
    pub anchor: Anchor,
    /// Group (assembly) the item belongs to, 0 for none.
    #[serde(skip_serializing_if = "is_zero")]
    pub group: u64,
    /// Collision class, empty: from the part or by kind ([`item_class`]).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub class: String,
    /// Cutouts in the wall of a room: a door or a window.
    #[serde(skip_serializing_if = "Opening::is_cutout")]
    pub opening: Opening,
}

impl super::Project {
    /// Refresh saved rail datums. Only move devices actually seated on a rail
    /// at their old datum; free or manually offset placements keep their position.
    pub fn refresh_rail_mounts(&mut self) {
        let mounts: std::collections::HashMap<_, _> = self.parts.iter().map(|p| (p.number.as_str(), p.effective_rail_mount())).collect();
        for e in &mut self.layout.enclosures {
            for i in 0..e.items.len() {
                let item = &e.items[i];
                let Some(&new) = mounts.get(item.part.as_str()) else { continue };
                if item.kind != ItemKind::Device || item.rail_mount == new {
                    continue;
                }
                let anchor = item.rail_anchor();
                let seated = e.items.iter().any(|r| {
                    r.kind == ItemKind::Rail
                        && r.face == item.face
                        && r.horizontal()
                        && anchor.x >= r.x
                        && anchor.x <= r.x + r.w
                        && (anchor.y - r.center().y).abs() < 0.01
                        && (item.z + item.rail_mount.clip_on - r.z - r.d).abs() < 0.01
                });
                let item = &mut e.items[i];
                if seated {
                    item.y += item.rail_mount.center_offset - new.center_offset;
                    item.z += item.rail_mount.clip_on - new.clip_on;
                }
                item.rail_mount = new;
            }
        }
    }
}

/// What a cutout is: an opening for cables, fans, … or a door or window in the wall of
/// a room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Opening {
    #[default]
    Cutout,
    /// Stands on the floor; its leaf swings into the room, hinged at its left edge
    /// seen from inside.
    Door,
    Window,
    /// A wall penetration (core hole, cable transit): cables, tubes and pipes pass
    /// through it into the room behind the wall.
    Penetration,
    /// A building entry (house lead-in) in an outer wall: the supply cable, the water
    /// main and other service lines come into the building through it; cables and
    /// pipes of a device in the room whose other end is outside run to it.
    Entry,
}

impl Opening {
    pub fn is_cutout(&self) -> bool {
        *self == Opening::Cutout
    }
}

pub(crate) fn is_plate(f: &Face) -> bool {
    *f == Face::Plate
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Fastening {
    /// From the part data, otherwise by kind and face (see [`Fastening::resolve`]).
    #[default]
    Auto,
    /// Screwed into a tapped hole.
    Tapped,
    /// Screw through a clearance hole with nut and washers.
    Through,
    /// Screwed into a rivet nut.
    RivetNut,
    /// Nut on a welded threaded stud (no hole).
    Stud,
    /// Snapped on (rails), clamped into its cutout (door devices, fans, glands).
    Clip,
    /// Glued (self-adhesive ducts and bases).
    Adhesive,
}

/// Parametric position of an item on its face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Anchor {
    pub x: Align,
    pub y: Align,
}

/// Placement along one axis of a face.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Align {
    /// Where it was put.
    #[default]
    Free,
    /// Distance of the item's start (left / top edge) from the face's start.
    Start(f32),
    /// Offset of the item's centre from the face's centre.
    Center(f32),
    /// Distance of the item's end (right / bottom edge) from the face's end.
    End(f32),
    /// From the face's start to its end with these distances: the item's length
    /// follows the face (cable ducts, rails).
    Stretch(f32, f32),
}

/// A wire's stretch in a duct.
pub struct Stretch {
    pub from: f32,
    pub to: f32,
    pub area: f32,
}

/// Mounting surface of an item.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Face {
    #[default]
    Plate,
    Door,
    /// Left side wall, seen from the left (the front is on the right).
    Left,
    /// Right side wall, seen from the right (the front is on the left).
    Right,
    /// Rear wall, seen from behind.
    Rear,
    /// Roof, seen from above (the front at the bottom).
    Roof,
    /// Bottom (gland plate), seen from below (the front at the bottom).
    Bottom,
    /// Wall `n` of a room's outline (from corner `n` to the next one), seen from inside
    /// the room (rooms only, not in `Face::ALL`).
    Wall(u16),
    /// Side 0 or 1 of the inner wall of a room with this id ([`InnerWall`]).
    Inner(u64, u8),
}

/// A wall inside a room (partition): its centre line from `a` to `b` along x or z, as
/// high as the room.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InnerWall {
    pub id: u64,
    pub a: [f32; 2],
    pub b: [f32; 2],
    pub thickness: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemKind {
    #[default]
    Device,
    Rail,
    Duct,
    /// Copper busbar system (several parallel bars on holders).
    Busbar,
    /// Opening in the mounting plate (rectangular or round).
    Cutout,
    /// Enclosure accessory without a device tag (fan, filter, cable gland, light,
    /// handle, PE bar).
    Accessory,
}

/// How an enclosure is built: its system and the optional components.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Build {
    /// Enclosure system ("compact", "baying", …), empty for a plain enclosure.
    pub system: String,
    /// Plinth height (mm), 0 without plinth.
    pub plinth: f32,
    /// The plinth comes with the enclosure part (part data `Plinth`), it is no part of
    /// its own.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub plinth_included: bool,
    /// Side bayed to the neighbouring enclosure: no side panel, a baying kit instead.
    pub left_open: bool,
    pub right_open: bool,
    /// The bottom is a removable gland plate.
    pub gland_plate: bool,
}

/// A 3D macro: items with their positions relative to each other (and anchors).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelMacro {
    pub name: String,
    pub description: String,
    /// The face whose items are placed relative to the insertion point.
    #[serde(skip_serializing_if = "is_plate")]
    pub face: Face,
    pub items: Vec<PanelItem>,
}

/// Rules of the panel layout checks ([`super::Layout::checks`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PanelRules {
    /// Free gap between devices side by side (mounting clearance), mm.
    pub device_gap: f32,
    /// Free space between devices and cable ducts for the wiring, mm.
    pub duct_gap: f32,
    /// Distance of devices from the edge of the mounting plate, mm.
    pub edge_distance: f32,
    /// Free space in front of every device on the plate for operation and
    /// replacement, mm.
    pub service_space: f32,
    /// Devices between these heights above the floor (IEC 60204-1 11.2.1), mm;
    /// 0: not checked.
    pub operating_min: f32,
    pub operating_max: f32,
    /// Manual actuators on the door (IEC 60204-1 10.1.2), mm; 0: not checked.
    pub actuator_min: f32,
    pub actuator_max: f32,
    /// Terminals at least this high above the floor, mm.
    pub terminal_min: f32,
    /// Highest air temperature at devices whose parts have no data, °C; 0: not checked.
    pub device_temperature: f32,
    /// Temperature rise at the roof against the middle (IEC/TR 60890 factor c).
    pub stratification: f32,
    /// Smallest web of sheet between holes and from holes to cutouts, mm.
    pub hole_web: f32,
    /// Smallest distance of a hole's edge from the edge of the sheet, mm.
    pub hole_edge: f32,
    /// Smallest web between cutouts and from cutouts to the edge of the sheet, mm.
    pub cutout_web: f32,
    /// Shortest rail, duct or busbar to cut, mm.
    pub min_profile: f32,
    /// Stock length of rails, ducts and busbars whose parts give none, mm.
    pub stock_length: f32,
    /// Distances between collision classes, and classes that may overlap.
    pub classes: Vec<ClassRule>,
}

/// What applies between the items of two collision classes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ClassRule {
    /// Class keys, `*` for any class.
    pub a: String,
    pub b: String,
    /// Smallest distance between the items, mm.
    pub distance: f32,
    /// The items may overlap (mounted into each other): no collision.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub ignore: bool,
}
