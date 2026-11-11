//! Data of `crate::panel` that the project file holds (see `docs/architecture.md`).


use super::is_zero;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Layout {
    pub enclosures: Vec<Enclosure>,
    /// Additional wire length per connection (stripping, reserve loops) in mm.
    pub wire_reserve: f32,
    /// Wire ends get ferrules (production wire list, bill of materials, 3D view), where
    /// no other termination is chosen for them (`terminations`).
    pub ferrules: bool,
    /// With ferrules: two wires ending at the same connection point share a twin
    /// ferrule, where neither has a termination chosen for it.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub twin_ferrules: bool,
    /// Terminations chosen for wire ends (ring terminals, …), see
    /// `crate::production::termination_of`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub terminations: Vec<WireTermination>,
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
    /// How single cables, wires and tubes are laid in rooms (by the key of their route),
    /// where not as the routing rules say ([`super::routing3d::RoutingRules::laying`]).
    #[serde(skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub laying: std::collections::BTreeMap<String, super::routing3d::Laying>,
    /// Cell temperatures the strings of the PV arrays are designed for.
    #[serde(skip_serializing_if = "PvDesign::is_default")]
    pub pv_design: PvDesign,
    /// Rules of the layout checks: clearances, collision classes, heights, thermal
    /// and manufacturing limits.
    #[serde(skip_serializing_if = "PanelRules::is_default")]
    pub checks: PanelRules,
    /// Machines, machining strategy, copper bars, jobs and production feedback.
    #[serde(skip_serializing_if = "super::manufacturing::Manufacturing::is_default")]
    pub manufacturing: super::manufacturing::Manufacturing,
    /// Buildings of the plant: their levels and shafts; rooms stand on their levels
    /// (see `crate::panel::buildings`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub buildings: Vec<Building>,
    /// Ways for cables and pipes between buildings, from a building entry of one to a
    /// building entry of another.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub site_links: Vec<SiteLink>,
    /// The plot the plant stands on: its boundary, which the buildings stay inside, and
    /// the reference point that ties the plant to a geographic coordinate system (see
    /// `crate::panel::plot`).
    #[serde(skip_serializing_if = "Plot::is_default")]
    pub plot: Plot,
}

/// How a wire end is terminated: stripped bare, with a ferrule or a crimped terminal
/// (sizes and parts in `crate::production`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Termination {
    /// Stripped, the bare conductor.
    Bare,
    /// Insulated ferrule (DIN 46228-4).
    Ferrule,
    /// Twin ferrule: two wires in one ferrule.
    TwinFerrule,
    /// Ring terminal (cable lug); uninsulated tubular cable lug from 10 mm².
    Ring,
    /// Fork (spade) terminal.
    Fork,
    /// Pin terminal.
    Pin,
    /// Flat plug (male tab 6.3 mm).
    Blade,
    /// Flat receptacle (female 6.3 mm).
    Receptacle,
}

/// The termination of the wire ends at a connection point.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WireTermination {
    /// Connection point (`-K1:13`), or a device (`-K1`: all its connection points).
    pub at: String,
    /// The other end of the wire (`-X1:3`): only that wire; empty: every wire at `at`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub to: String,
    pub kind: Termination,
}

impl Layout {
    /// Keeps the chosen terminations of wire ends when a device is renamed.
    pub fn rename_device(&mut self, old: &str, new: &str) {
        let rename = |s: &mut String| {
            if s == old {
                *s = new.to_string();
            } else if let Some(pin) = s.strip_prefix(old).and_then(|r| r.strip_prefix(':')) {
                *s = format!("{new}:{pin}");
            }
        };
        for t in &mut self.terminations {
            rename(&mut t.at);
            rename(&mut t.to);
        }
    }
}

/// The plot (parcel of land) of the plant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plot {
    /// Name, e.g. the cadastral parcel.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Corners of its boundary in the plant (x, z in mm); empty for none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outline: Vec<[f32; 2]>,
    /// The georeferenced point everything is measured from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<GeoReference>,
}

impl Plot {
    pub fn is_default(&self) -> bool {
        *self == Plot::default()
    }
}

/// A point of the plant with its coordinates in a geographic (projected) coordinate
/// reference system: the plant's x and z are measured from it towards east and north,
/// turned by the direction of grid north.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeoReference {
    /// Name, e.g. a survey point.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Where it is in the plant: x and z of the floor plan, mm.
    pub x: f32,
    pub z: f32,
    /// Its height above the plant's ground level (±0), mm.
    #[serde(skip_serializing_if = "is_zero")]
    pub y: f32,
    /// Its easting and northing in the coordinate reference system, m.
    pub easting: f64,
    pub northing: f64,
    /// Its height above the datum of the coordinate reference system (sea level), m.
    pub height: f64,
    /// Coordinate reference system, e.g. "EPSG:25832" (ETRS89 / UTM zone 32N).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub crs: String,
    /// Direction of grid north in the plant, degrees counter-clockwise seen from above
    /// from -z (at 0 north is towards the rear, at 90 towards -x).
    #[serde(skip_serializing_if = "is_zero")]
    pub north: f32,
    /// Its latitude and longitude (WGS 84), degrees, if known (IFC export).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
}

/// A building of the plant: levels on top of each other on which its rooms stand, and
/// shafts through its levels. Building coordinates: x and z of the floor plan from the
/// building's place in the plant, y up from its ground level (±0).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Building {
    pub id: u64,
    /// Location designation, e.g. "+B1".
    pub name: String,
    pub description: String,
    /// Place of the building's origin in the plant (mm, like the place of a line-up).
    pub site: SitePlace,
    /// The levels from the lowest up.
    pub levels: Vec<Level>,
    pub shafts: Vec<Shaft>,
    /// Stairs from level to level (see `crate::panel::stairs`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub stairs: Vec<Stair>,
    /// Roofs over the building; they cut the ceilings of the rooms below them (see
    /// `crate::panel::roofs`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub roofs: Vec<Roof>,
}

/// How a stair runs from its foot to its head.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StairKind {
    /// One straight flight.
    #[default]
    Straight,
    /// Two flights at a right angle with a quarter landing between them.
    QuarterLanding,
    /// Three flights, turned twice at a right angle, with two quarter landings.
    TwoQuarterLandings,
    /// Two flights side by side running in opposite directions, with a half landing.
    HalfLanding,
    /// Winding steps round a newel.
    Spiral,
}

/// The side a stair turns to on the way up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Turn {
    #[default]
    Left,
    Right,
}

/// A stair of a building from the floor of one level up to the floor of another.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stair {
    pub id: u64,
    pub name: String,
    pub kind: StairKind,
    pub turn: Turn,
    /// Middle of the front edge of its lowest step in the building's floor plan, mm.
    pub x: f32,
    pub z: f32,
    /// Direction of its first flight, degrees counter-clockwise seen from above: at 0
    /// it runs up towards +x.
    pub angle: f32,
    /// Width of its flights, mm.
    pub width: f32,
    /// Going (depth) of a step, mm.
    pub tread: f32,
    /// Ids of the level it starts on and of the level it leads up to.
    pub bottom: u64,
    pub top: u64,
}

impl Default for Stair {
    fn default() -> Self {
        Stair { id: 0, name: String::new(), kind: StairKind::Straight, turn: Turn::Left, x: 0.0, z: 0.0, angle: 0.0, width: 1000.0, tread: 270.0, bottom: 0, top: 0 }
    }
}

/// The form of a roof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RoofShape {
    Flat,
    /// Mono-pitch (shed) roof: one slope from the low eaves up to the high side.
    Shed,
    /// Gable roof: two slopes up to the ridge, gables at the ends.
    #[default]
    Gable,
    /// Hip roof: slopes on all four sides, a ridge along the longer side.
    Hip,
    /// Pyramid (tent) roof: four slopes up to one point.
    Pyramid,
}

/// A roof over a rectangle of a building's floor plan. The underside of its slopes
/// starts at the height of the eaves over the edges of the rectangle and rises at the
/// pitch towards the ridge; it may reach down through several levels.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Roof {
    pub id: u64,
    pub name: String,
    pub shape: RoofShape,
    /// Rear left corner and size of the rectangle it covers (the outer walls) in the
    /// building's floor plan, mm.
    pub x: f32,
    pub z: f32,
    pub width: f32,
    pub depth: f32,
    /// The ridge (gable, hip) or the slope (shed: it rises across it) runs along z
    /// instead of x.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub along_z: bool,
    /// Shed roofs: the low eaves at the rear (or left) instead of the front (or right).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub flip: bool,
    /// Height of the underside of the roof over the edges of the rectangle above the
    /// building's ground level, mm.
    pub eaves: f32,
    /// Pitch of the slopes, degrees.
    pub pitch: f32,
    /// How far it reaches out over the walls, mm.
    pub overhang: f32,
    /// Thickness of the roof (rafters, insulation, covering), mm.
    pub thickness: f32,
    /// Roof windows in its slopes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<RoofWindow>,
    /// Solar (PV) arrays on it (see `crate::panel::solar`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub solar: Vec<PvArray>,
}

impl Default for Roof {
    fn default() -> Self {
        Roof { id: 0, name: String::new(), shape: RoofShape::Gable, x: 0.0, z: 0.0, width: 10_000.0, depth: 8000.0, along_z: false, flip: false, eaves: 3000.0, pitch: 35.0, overhang: 400.0, thickness: 300.0, windows: Vec::new(), solar: Vec::new() }
    }
}

/// How the modules of a PV array are mounted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PvMounting {
    /// On rails parallel to the covering, fastened to the roof with roof hooks.
    #[default]
    Parallel,
    /// Flat roofs: rows of modules tilted towards one side on ballasted mounting bases.
    Tilted,
    /// Flat roofs: pairs of modules tilted towards both sides (east-west) on ballasted
    /// mounting bases.
    EastWest,
}

/// A solar (PV) array on a roof: rows of modules on one slope with their mounting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PvArray {
    pub id: u64,
    pub name: String,
    /// Its middle in the building's floor plan, mm.
    pub x: f32,
    pub z: f32,
    /// Modules side by side in a row (along the eaves, or across the direction they face).
    pub columns: u32,
    /// Rows of modules (up the slope, or behind each other; east-west: rows of pairs).
    pub rows: u32,
    /// The long side of the modules along the rows instead of up the slope.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub landscape: bool,
    /// Module part.
    pub module: String,
    /// Size of a module: short side, long side and frame height, mm.
    pub module_width: f32,
    pub module_length: f32,
    pub module_height: f32,
    pub mounting: PvMounting,
    /// Tilt of the modules against the roof (tilted and east-west mounting), degrees.
    pub tilt: f32,
    /// Flat roofs: the direction the modules face, degrees counter-clockwise seen from
    /// above (0: towards +z, the front).
    pub azimuth: f32,
    /// Distance from row to row (tilted and east-west mounting), mm; 0: so that a row
    /// does not shade the next one in winter.
    pub row_gap: f32,
    /// Greatest distance of the roof hooks or mounting bases along a rail, mm.
    pub span: f32,
    /// Ballast per module (tilted and east-west mounting), kg.
    pub ballast: f32,
    /// Mounting system: part number of its rail (parallel mounting) or mounting base;
    /// its accessories are the hooks, clamps, connectors and ballast. Empty: generic.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system: String,
    /// Device (PV generator) the modules belong to, e.g. "=PV+ROOF-G1"; its parts then
    /// count the modules in the bill of materials.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub device: String,
    /// The strings its modules are connected in.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub strings: Vec<PvString>,
}

/// A string of a PV array: modules in series, possibly several such strings in
/// parallel, on one MPP tracker of an inverter or charge controller.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PvString {
    pub id: u64,
    /// Name, e.g. "S1".
    pub name: String,
    /// Modules in series.
    pub series: u32,
    /// Strings of this kind in parallel.
    pub parallel: u32,
    /// Device tag of the inverter or charge controller.
    pub inverter: String,
    /// Its MPP tracker (input), from 1.
    pub mppt: u32,
}

impl Default for PvString {
    fn default() -> Self {
        PvString { id: 0, name: String::new(), series: 1, parallel: 1, inverter: String::new(), mppt: 1 }
    }
}

/// Cell temperatures for the design of PV strings, °C: the lowest (the highest open
/// circuit voltage), the mean (typical operation) and the highest (the lowest voltage).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PvDesign {
    pub t_min: f32,
    pub t_mean: f32,
    pub t_max: f32,
}

impl Default for PvDesign {
    fn default() -> Self {
        PvDesign { t_min: -10.0, t_mean: 25.0, t_max: 70.0 }
    }
}

impl PvDesign {
    pub fn is_default(&self) -> bool {
        *self == PvDesign::default()
    }
}

impl Default for PvArray {
    fn default() -> Self {
        PvArray {
            id: 0,
            name: String::new(),
            x: 0.0,
            z: 0.0,
            columns: 4,
            rows: 2,
            landscape: false,
            module: "GEN.PV-430W".into(),
            module_width: 1134.0,
            module_length: 1722.0,
            module_height: 30.0,
            mounting: PvMounting::Parallel,
            tilt: 15.0,
            azimuth: 0.0,
            row_gap: 0.0,
            span: 1200.0,
            ballast: 20.0,
            system: String::new(),
            device: String::new(),
            strings: Vec::new(),
        }
    }
}

/// A roof window (skylight) in a slope of a roof.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RoofWindow {
    pub id: u64,
    /// Its middle in the building's floor plan, mm.
    pub x: f32,
    pub z: f32,
    /// Width along the eaves and length up the slope, mm.
    pub width: f32,
    pub length: f32,
    /// Part (a window of the parts database), empty for a generic one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub part: String,
}

/// A level (storey) of a building.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Level {
    pub id: u64,
    /// Name, e.g. "B1", "GF", "L1".
    pub name: String,
    /// Height of the top of its floor above the building's ground level, mm (negative
    /// for basements).
    pub elevation: f32,
    /// Height from its floor to the floor of the level above, mm (ceiling slab
    /// included).
    pub height: f32,
}

/// A vertical shaft (riser) through levels of a building: cables, tubes and pipes run
/// through it from a room on one level into the rooms above and below that it
/// passes.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Shaft {
    pub id: u64,
    pub name: String,
    /// Rear left corner in the building's floor plan and size, mm.
    pub x: f32,
    pub z: f32,
    pub width: f32,
    pub depth: f32,
    /// Ids of its lowest and its highest level.
    pub bottom: u64,
    pub top: u64,
}

/// What carries the cables and pipes between two buildings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LinkKind {
    /// Cable trench, buried in the ground.
    #[default]
    Trench,
    /// Protective conduit or duct bank in the ground.
    Conduit,
    /// Cable bridge or tray above the ground.
    Bridge,
}

/// A way between two buildings: from building entry `a` (the id of an entry in an
/// outer wall of a room) to building entry `b` of a room of another building, through
/// the corners `points` in the plant.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SiteLink {
    pub id: u64,
    pub name: String,
    pub kind: LinkKind,
    pub a: u64,
    pub b: u64,
    /// Corners between the two entries (x, z in the plant, mm).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub points: Vec<[f32; 2]>,
    /// Length of the way, mm; 0: along its corners from entry to entry.
    #[serde(skip_serializing_if = "is_zero")]
    pub length: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Enclosure {
    pub id: u64,
    /// Location designation, e.g. "+CAB1".
    pub name: String,
    pub part: String,
    /// Device whose primary article is represented by this enclosure body.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub device_tag: String,
    /// Device accounting for this body as an additional article, not its mounting parent.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub owner_device: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bom_excluded: bool,
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    /// The source specifies no implicit mounting plate; explicit plate items still draw.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub no_implicit_plate: bool,
    /// Mounting-face origin in the enclosure frame; absent uses the legacy plate datum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mounting_origin: Option<[f32;3]>,
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
    /// Rooms: id of the level of a building the room is on, 0 for none; `site` is then
    /// measured in the building's floor plan and the room stands on the level's floor.
    #[serde(skip_serializing_if = "is_zero")]
    pub level: u64,
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
    /// Open mounting assembly: dimensions bound its working canvas, without a shell.
    Assembly,
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
    /// One separately placed terminal of the strip; empty for the whole device.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub terminal: String,
    /// This footprint represents its own part, including unnumbered terminal blocks.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub single_part: bool,
    /// Material ownership, independent of the physical mounting relationship.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub owner_device: String,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub bom_excluded: bool,
    pub part: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    /// Clockwise orientation in the face plane; none retains legacy size inference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<PanelRotation>,
    /// Height of the item (extent away from the plate).
    pub d: f32,
    /// Base height above the plate (devices on a rail sit on the rail).
    pub z: f32,
    #[serde(default, skip_serializing_if = "openecraft_parts::RailMount::is_centered")]
    pub rail_mount: openecraft_parts::RailMount,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub seated_on_rail: bool,
    /// Keep this instance's mounting datum instead of refreshing it from its part.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rail_mount_override: bool,
    /// Mounting datum across a vertical rail (otherwise across a horizontal rail).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rail_vertical: bool,
    /// Parent mounting item; coordinates remain in the enclosure's common frame.
    #[serde(skip_serializing_if = "is_zero")]
    pub mounted_on: u64,
    /// An explicit free placement must not acquire an inferred legacy rail parent.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub mounting_explicit: bool,
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
    /// Doors and windows: how their leaf opens.
    #[serde(skip_serializing_if = "Swing::is_default")]
    pub swing: Swing,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "i32", into = "i32")]
pub enum PanelRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl PanelRotation {
    pub fn degrees(self) -> i32 {
        match self {Self::Deg0=>0,Self::Deg90=>90,Self::Deg180=>180,Self::Deg270=>270}
    }

    pub fn from_degrees(degrees:i32)->Option<Self> {
        if degrees%90!=0 {return None}
        Some(match degrees.rem_euclid(360) {0=>Self::Deg0,90=>Self::Deg90,180=>Self::Deg180,_=>Self::Deg270})
    }

    pub fn is_quarter(self)->bool {matches!(self,Self::Deg90|Self::Deg270)}

    pub fn next(self)->Self {Self::from_degrees(self.degrees()+90).unwrap()}

    /// Rotates a top-left-origin coordinate into the turned bounding rectangle.
    pub fn map(self,[x,y]:[f32;2],[w,h]:[f32;2])->[f32;2] {
        match self {Self::Deg0=>[x,y],Self::Deg90=>[h-y,x],Self::Deg180=>[w-x,h-y],Self::Deg270=>[y,w-x]}
    }
}

impl From<PanelRotation> for i32 {
    fn from(value:PanelRotation)->Self {value.degrees()}
}

impl TryFrom<i32> for PanelRotation {
    type Error=&'static str;
    fn try_from(value:i32)->Result<Self,Self::Error> {
        Self::from_degrees(value).ok_or("Panel rotation must be a multiple of 90 degrees")
    }
}

impl super::Project {
    /// Refresh saved rail datums. Only move devices actually seated on a rail
    /// at their old datum; free or manually offset placements keep their position.
    pub fn refresh_rail_mounts(&mut self) {
        let mounts: std::collections::HashMap<_, _> = self.parts.iter().map(|p| (p.number.as_str(), p.effective_rail_mount())).collect();
        for e in &mut self.layout.enclosures {
            for i in 0..e.items.len() {
                let item = &e.items[i];
                if item.kind != ItemKind::Device {
                    continue;
                }
                let seated = e.items.iter().any(|rail| (!item.mounting_explicit || item.mounted_on==rail.id) && item.seated_on(rail));
                let item = &mut e.items[i];
                item.seated_on_rail = seated;
                if item.rail_mount_override {continue;}
                let Some(&new) = mounts.get(item.part.as_str()) else { continue };
                if seated {
                    if item.rail_vertical {item.x += item.rail_mount.center_offset - new.center_offset;}
                    else {item.y += item.rail_mount.center_offset - new.center_offset;}
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
    /// Stands on the floor; its leaf opens as its [`Swing`] says.
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

/// How the leaf (sash) of a door or window opens; left and right seen from inside the
/// room (from the side of the wall it was placed on).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Swing {
    /// Hinged at its left edge, swings into the room.
    #[default]
    LeftIn,
    RightIn,
    /// Hinged at its left edge, swings out of the room.
    LeftOut,
    RightOut,
    /// Slides along the wall to the left, in front of it inside the room.
    SlideLeft,
    SlideRight,
}

impl Swing {
    pub fn is_default(&self) -> bool {
        *self == Swing::LeftIn
    }

    /// Hinged at its right edge.
    pub fn right(self) -> bool {
        matches!(self, Swing::RightIn | Swing::RightOut)
    }

    /// Swings out of the room.
    pub fn outward(self) -> bool {
        matches!(self, Swing::LeftOut | Swing::RightOut)
    }

    /// Slides instead of swinging: -1 to the left, 1 to the right, 0 for hinged leaves.
    pub fn slide(self) -> f32 {
        match self {
            Swing::SlideLeft => -1.0,
            Swing::SlideRight => 1.0,
            _ => 0.0,
        }
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
    /// The outside of wall `n` of a room's outline (the façade where the wall is an
    /// outer wall of the building), seen from outside; `z` away from the wall.
    Outside(u16),
    /// The top of a room's ceiling (the roof of the building where no room is above),
    /// seen from above like the ceiling; `z` upwards.
    RoofTop,
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
    /// Explicit mounting plate with its authored size, thickness and position.
    MountingPlate,
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
