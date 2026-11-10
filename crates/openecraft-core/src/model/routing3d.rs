//! Data of `crate::routing3d` that the project file holds (see `docs/architecture.md`).


use serde::{Deserialize, Serialize};
use openecraft_parts::vec3::V3;

/// Ends at which the shield of a cable is connected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShieldEnds {
    #[default]
    Both,
    First,
    Second,
    None,
}

/// A routing reservation: a box of an enclosure (enclosure coordinates).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reservation {
    pub id: u64,
    pub name: String,
    pub min: V3,
    pub max: V3,
    pub mode: ReserveMode,
}

/// What a reservation keeps the space for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReserveMode {
    /// Nothing is routed through it (room for later devices, the swing of the door).
    #[default]
    Keepout,
    /// Only this class is routed through it.
    For(Class),
}

/// Segregation class of what is routed: classes that must keep apart avoid each
/// other's neighbourhood ([`RoutingRules::segregation`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Class {
    /// Mains and motor circuits (phase, neutral, PE, more than 50 V AC / 120 V DC).
    Power,
    /// Control circuits (24 V DC …).
    #[default]
    Control,
    /// Signals, analogue values, bus and network cables, shielded cables.
    Signal,
    /// Hoses and pipes.
    Fluid,
}

/// Rules of the 3D routing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingRules {
    /// Edge length of the routing grid, mm.
    pub grid: f32,
    /// Distance routes keep from devices, rails and busbars, mm.
    pub clearance: f32,
    /// A way through free air costs this many times the way in a duct or path.
    pub free_factor: f32,
    /// Distance between classes that keep apart (power – signal, power – fluid), mm.
    pub segregation: f32,
    /// A way closer than the segregation distance costs this many times more.
    pub segregation_factor: f32,
    /// Wires are routed in 3D as well (otherwise through the cable ducts on the plate).
    pub wires_3d: bool,
    /// Opt in to retaining impossible ahead searches when channel fill changes.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub reuse_failed_searches: bool,
    /// Smallest bend radius as a multiple of the outer diameter: wires, cables, tubes.
    pub bend_wire: f32,
    pub bend_cable: f32,
    pub bend_tube: f32,
    /// Cut lengths are rounded up to these steps (mm): wires, cables, tubes.
    pub cut_wire: f32,
    pub cut_cable: f32,
    pub cut_tube: f32,
    /// Length added to each cable end in the enclosure (stripping, connection), mm.
    pub cable_reserve: f32,
    /// Length of a shield pigtail where there is no shield clamp, mm.
    pub pigtail: f32,
    /// How cables, wires and tubes are laid in rooms unless set for one of them
    /// ([`super::panel::Layout::laying`]).
    #[serde(skip_serializing_if = "Laying::is_surface")]
    pub laying: Laying,
    /// Depth of the slots cut into the walls for flush-mounted lines: the line lies this
    /// deep behind the surface of the wall (its middle half its thickness less), mm.
    pub slot_depth: f32,
    /// Depth of the lines laid in the floor below its surface (in the screed or on the
    /// raw floor below it), mm.
    pub floor_depth: f32,
    /// The installation zones flush-mounted lines run in (by default those of DIN
    /// 18015-3).
    pub zones: Vec<InstallationZone>,
}

/// How a cable, wire or tube is laid in a room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Laying {
    /// On the walls, the ceiling and the floor, in cable ducts and along pipe runs.
    #[default]
    Surface,
    /// Flush-mounted in slots in the walls, along the installation zones.
    Wall,
    /// Flush-mounted in the floor, down and up the walls in the vertical zones.
    Floor,
}

impl Laying {
    pub fn is_surface(&self) -> bool {
        *self == Laying::Surface
    }
}

/// Where an installation zone lies in a room.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ZoneKind {
    /// Along the walls, from `from` to `to` above the floor.
    #[default]
    AboveFloor,
    /// Along the walls, from `from` to `to` below the ceiling.
    BelowCeiling,
    /// Up the walls, from `from` to `to` beside the corners of the room.
    BesideCorners,
    /// Up the walls, from `from` to `to` beside the doors.
    BesideDoors,
    /// Up the walls, from `from` to `to` beside the windows.
    BesideWindows,
    /// In the floor along the walls, from `from` to `to` away from them.
    Floor,
    /// In the ceiling along the walls, from `from` to `to` away from them (shown, not
    /// routed through).
    Ceiling,
}

/// An installation zone: a band of the walls, the floor or the ceiling of a room in
/// which flush-mounted lines run (DIN 18015-3).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstallationZone {
    /// Short name, e.g. "ZW-u".
    pub name: String,
    pub kind: ZoneKind,
    /// The band from `from` to `to` (see [`ZoneKind`]), mm.
    pub from: f32,
    pub to: f32,
    /// Lines run in it (the middle zone of kitchens only where it is switched on).
    pub on: bool,
}

/// A routing path drawn in an enclosure: a channel through free space (cable tray,
/// cable clamp rail, tube guide) that routes prefer.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoutingPath {
    pub id: u64,
    pub name: String,
    pub kind: PathKind,
    /// Corner points in enclosure coordinates (x right, y down from the top left
    /// front corner, z from the rear wall).
    pub points: Vec<V3>,
    /// Width of the path: diameter of the space its contents may take, mm.
    pub width: f32,
    /// Classes allowed on the path; empty: all.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub classes: Vec<Class>,
    /// Share of the capacity kept free for later additions, percent.
    pub reserve: f32,
    /// The face of a room the path runs along (pipe runs, see
    /// `crate::panel::add_pipe_run`); `None` for paths through an enclosure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub face: Option<super::panel::Face>,
}

/// What a routing path is for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PathKind {
    #[default]
    Any,
    Wires,
    Cables,
    Tubes,
}

/// A route frozen for production: the recalculation keeps it while it is still
/// possible and its ends have not moved.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FrozenRoute {
    pub key: String,
    pub kind: RouteKind,
    /// Id of the enclosure the points are relative to.
    pub enclosure: u64,
    /// Corner points relative to that enclosure.
    pub points: Vec<V3>,
    /// Length when it was frozen, mm.
    pub length: f32,
    /// A route through a room into its cabinets: its legs, each relative to its own
    /// enclosure (`points` is then empty).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub legs: Vec<FrozenLeg>,
}

/// A leg of a frozen route (see [`FrozenRoute::legs`]).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FrozenLeg {
    /// Id of the enclosure (cabinet or room) the points are relative to.
    pub enclosure: u64,
    pub points: Vec<V3>,
}

/// What is routed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RouteKind {
    #[default]
    Wire,
    Cable,
    Tube,
}
