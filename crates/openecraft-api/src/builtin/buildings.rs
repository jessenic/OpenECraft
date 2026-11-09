//! Building commands: buildings of the plant with their levels, shafts, stairs and
//! roofs (with roof windows and PV arrays), rooms on the levels, and links (cable
//! trenches, conduits, cable bridges) between the building entries of different
//! buildings ([`openecraft_core::panel::buildings`], [`openecraft_core::panel::stairs`],
//! [`openecraft_core::panel::roofs`], [`openecraft_core::panel::solar`]); the plot with its
//! boundary and the georeferenced point ([`openecraft_core::panel::plot`]).

use crate::builtin::common::{int_in, num_in, NO_LIMIT, NO_MAX};
use crate::params::{optional, param, Args, Kind};
use crate::{Info, Plugin, Registry};
use super::{add_commands, info, Spec};
use openecraft_core::model::Project;
use openecraft_core::panel::buildings::{self as bld, SHAFT_SIZE};
use openecraft_core::panel::solar::strings;
use openecraft_core::panel::{self, plot, roofs, solar, stairs, LinkKind, Opening, PvMounting, RoofShape, SitePlace, StairKind, Turn};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use serde_json::{json, Value};

pub struct Buildings;

impl Plugin for Buildings {
    fn info(&self) -> Info {
        info("buildings", tr_const!("Buildings"), tr_const!("Buildings of the plant with levels, shafts, stairs and roofs with PV arrays, rooms on the levels and links between buildings"))
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        add_commands(reg, COMMANDS)
    }
}

const BUILDING: &str = tr_const!("Building (location, e.g. +B1)");
const LEVEL: &str = tr_const!("Level: its name or number (1 = the lowest)");
const SHAFT: &str = tr_const!("Shaft: its name");
const LINK: &str = tr_const!("Link between buildings: its name");
const KIND: &str = tr_const!("trench, conduit or bridge");
const STAIR: &str = tr_const!("Stair: its name");
const STAIR_KIND: &str = tr_const!("straight, quarter_landing, two_quarter_landings, half_landing or spiral");
const STAIR_TURN: &str = tr_const!("left or right: the side the stair turns to on the way up");
const STAIR_ANGLE: &str = tr_const!("Direction of the first flight in degrees, counter-clockwise seen from above (0: towards +x)");
const STAIR_X: &str = tr_const!("Middle of the front edge of the lowest step in the floor plan of the building, x in mm");
const STAIR_Z: &str = tr_const!("Middle of the front edge of the lowest step in the floor plan of the building, z in mm");
const ROOF: &str = tr_const!("Roof: its name");
const ROOF_SHAPE: &str = tr_const!("flat, shed, gable, hip or pyramid");
const ROOF_X: &str = tr_const!("Rear left corner of the covered rectangle (outer walls) in the floor plan of the building, x in mm");
const ROOF_Z: &str = tr_const!("Rear left corner of the covered rectangle (outer walls) in the floor plan of the building, z in mm");
const ROOF_EAVES: &str = tr_const!("Height of the underside of the roof over the walls above the ground level of the building in mm");
const ROOF_ALONG_Z: &str = tr_const!("The ridge (or the slope of a shed roof) runs along z instead of x");
const ROOF_FLIP: &str = tr_const!("Shed roof: the low eaves at the rear or left instead of the front or right");
const WINDOW_X: &str = tr_const!("Middle of the window in the floor plan of the building, x in mm");
const WINDOW_Z: &str = tr_const!("Middle of the window in the floor plan of the building, z in mm");
const PV_X: &str = tr_const!("Middle of the PV array in the floor plan of the building, x in mm");
const PV_Z: &str = tr_const!("Middle of the PV array in the floor plan of the building, z in mm");
const PV_COLUMNS: &str = tr_const!("Modules side by side in a row");
const PV_ROWS: &str = tr_const!("Rows of modules (east-west: rows of pairs)");
const PV_LANDSCAPE: &str = tr_const!("The long side of the modules along the rows");
const PV_MODULE: &str = tr_const!("Part number of the module (its size comes from the part)");
const PV_MOUNTING: &str = tr_const!("parallel (on roof hooks), tilted or east_west (flat roofs)");
const PV_TILT: &str = tr_const!("Tilt of the modules on a flat roof in degrees");
const PV_AZIMUTH: &str = tr_const!("Flat roofs: direction the modules face in degrees, counter-clockwise seen from above (0: towards +z)");
const PV_ROW_GAP: &str = tr_const!("Distance from row to row on a flat roof in mm (0: free of shade in winter)");
const PV_SPAN: &str = tr_const!("Greatest distance of the roof hooks along a rail in mm");
const PV_BALLAST: &str = tr_const!("Ballast per module on a flat roof in kg");
const PV_SYSTEM: &str = tr_const!("Mounting system: part number of its rail or mounting base, whose accessories are the hooks, clamps and ballast (empty: generic)");
const PV_DEVICE: &str = tr_const!("Device (PV generator) the modules belong to; its parts then count them in the bill of materials");
const PV_ARRAY: &str = tr_const!("PV array: its name");
const PV_SERIES: &str = tr_const!("Modules in series");
const PV_PARALLEL: &str = tr_const!("Strings of this kind in parallel (default 1)");
const PV_INVERTER: &str = tr_const!("Device tag of the inverter or charge controller");
const PV_MPPT: &str = tr_const!("Its MPP tracker (input), from 1");

pub static COMMANDS: &[Spec] = &[
    Spec {
        name: "list_buildings",
        help: tr_const!("Lists the buildings with their levels, the rooms on them, their shafts and the rooms these join, and the links between buildings"),
        params: &[optional("building", Kind::Str, BUILDING)],
        run: list_buildings,
    },
    Spec {
        name: "add_building",
        help: tr_const!("Adds a building with one level at ground level; without x and z it goes beside what stands in the plant already"),
        params: &[
            param("name", Kind::Str, BUILDING),
            optional("x", Kind::Num, tr_const!("Origin of the building in the plant, x in mm")),
            optional("z", Kind::Num, tr_const!("Origin of the building in the plant, z (towards the front) in mm")),
            optional("angle", Kind::Num, tr_const!("Turn about the vertical in degrees, counter-clockwise seen from above")),
            optional("description", Kind::Str, tr_const!("Description")),
        ],
        run: add_building,
    },
    Spec {
        name: "set_building",
        help: tr_const!("Moves, turns, renames or removes a building; its rooms move with it, removed they stay where they stand"),
        params: &[
            param("building", Kind::Str, BUILDING),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("x", Kind::Num, tr_const!("Origin of the building in the plant, x in mm")),
            optional("z", Kind::Num, tr_const!("Origin of the building in the plant, z (towards the front) in mm")),
            optional("angle", Kind::Num, tr_const!("Turn about the vertical in degrees, counter-clockwise seen from above")),
            optional("description", Kind::Str, tr_const!("Description")),
            optional("remove", Kind::Bool, tr_const!("Remove the building")),
        ],
        run: set_building,
    },
    Spec {
        name: "add_level",
        help: tr_const!("Adds a level to a building: by default on top of the highest one, 3000 mm high; a negative elevation makes a basement"),
        params: &[
            param("building", Kind::Str, BUILDING),
            optional("name", Kind::Str, tr_const!("Name (default L0, L1, … above and B1, B2, … below the ground)")),
            optional("elevation", Kind::Num, tr_const!("Height of its floor above the ground level of the building in mm")),
            optional("height", Kind::Num, tr_const!("Height from its floor to the floor of the level above in mm")),
        ],
        run: add_level,
    },
    Spec {
        name: "set_level",
        help: tr_const!("Renames a level, changes its elevation or height (the rooms on it move with it) or removes it (its rooms stay where they stand)"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("level", Kind::Str, LEVEL),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("elevation", Kind::Num, tr_const!("Height of its floor above the ground level of the building in mm")),
            optional("height", Kind::Num, tr_const!("Height from its floor to the floor of the level above in mm")),
            optional("remove", Kind::Bool, tr_const!("Remove the level")),
        ],
        run: set_level,
    },
    Spec {
        name: "place_on_level",
        help: tr_const!("Puts a room on a level of a building at a place of the building's floor plan (rear left corner of the room's outline); without x and z next to the other rooms of the level; without building out of its building"),
        params: &[
            param("room", Kind::Str, tr_const!("Room (location, e.g. +ROOM1)")),
            optional("building", Kind::Str, tr_const!("Building; empty to take the room out of its building")),
            optional("level", Kind::Str, LEVEL),
            optional("x", Kind::Num, tr_const!("x in the floor plan of the building in mm")),
            optional("z", Kind::Num, tr_const!("z in the floor plan of the building (towards the front) in mm")),
            optional("angle", Kind::Num, tr_const!("Turn about the vertical in degrees, counter-clockwise seen from above")),
        ],
        run: place_on_level,
    },
    Spec {
        name: "add_shaft",
        help: tr_const!("Adds a shaft to a building from one level up to another: cables, tubes and pipes run through it into the rooms above and below that it passes"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("x", Kind::Num, tr_const!("Rear left corner in the floor plan of the building, x in mm")),
            param("z", Kind::Num, tr_const!("Rear left corner in the floor plan of the building, z (towards the front) in mm")),
            optional("width", Kind::Num, tr_const!("Width in mm (default 600)")),
            optional("depth", Kind::Num, tr_const!("Depth in mm (default 600)")),
            optional("bottom", Kind::Str, tr_const!("Lowest level (default the lowest of the building)")),
            optional("top", Kind::Str, tr_const!("Highest level (default the highest of the building)")),
            optional("name", Kind::Str, tr_const!("Name (default S1, S2, …)")),
        ],
        run: add_shaft,
    },
    Spec {
        name: "set_shaft",
        help: tr_const!("Moves or resizes a shaft, changes the levels it runs through, renames or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("shaft", Kind::Str, SHAFT),
            optional("x", Kind::Num, tr_const!("Rear left corner in the floor plan of the building, x in mm")),
            optional("z", Kind::Num, tr_const!("Rear left corner in the floor plan of the building, z (towards the front) in mm")),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("depth", Kind::Num, tr_const!("Depth in mm")),
            optional("bottom", Kind::Str, tr_const!("Lowest level")),
            optional("top", Kind::Str, tr_const!("Highest level")),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the shaft")),
        ],
        run: set_shaft,
    },
    Spec {
        name: "add_stair",
        help: tr_const!("Adds a stair to a building from the floor of one level up to the floor of another: straight, turned once or twice at quarter landings, with a half landing or spiral; its steps follow from the difference in height"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("x", Kind::Num, STAIR_X),
            param("z", Kind::Num, STAIR_Z),
            optional("kind", Kind::Str, STAIR_KIND),
            optional("turn", Kind::Str, STAIR_TURN),
            optional("angle", Kind::Num, STAIR_ANGLE),
            optional("width", Kind::Num, tr_const!("Width of the flights in mm (default 1000)")),
            optional("tread", Kind::Num, tr_const!("Going of a step in mm (default 270)")),
            optional("bottom", Kind::Str, tr_const!("Level it starts on (default the lowest of the building)")),
            optional("top", Kind::Str, tr_const!("Level it leads up to (default the next one above)")),
            optional("name", Kind::Str, tr_const!("Name (default ST1, ST2, …)")),
        ],
        run: add_stair,
    },
    Spec {
        name: "set_stair",
        help: tr_const!("Moves, turns or resizes a stair, changes its kind or the levels it joins, renames or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("stair", Kind::Str, STAIR),
            optional("x", Kind::Num, STAIR_X),
            optional("z", Kind::Num, STAIR_Z),
            optional("kind", Kind::Str, STAIR_KIND),
            optional("turn", Kind::Str, STAIR_TURN),
            optional("angle", Kind::Num, STAIR_ANGLE),
            optional("width", Kind::Num, tr_const!("Width in mm")),
            optional("tread", Kind::Num, tr_const!("Going of a step in mm")),
            optional("bottom", Kind::Str, tr_const!("Lowest level")),
            optional("top", Kind::Str, tr_const!("Highest level")),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the stair")),
        ],
        run: set_stair,
    },
    Spec {
        name: "add_roof",
        help: tr_const!("Adds a roof to a building: flat, shed, gable, hip or pyramid; without a rectangle it covers the rooms of the building with its eaves on top of the highest level. A roof lower than the ceilings of the rooms below it slopes their ceilings"),
        params: &[
            param("building", Kind::Str, BUILDING),
            optional("shape", Kind::Str, ROOF_SHAPE),
            optional("x", Kind::Num, ROOF_X),
            optional("z", Kind::Num, ROOF_Z),
            optional("width", Kind::Num, tr_const!("Width of the covered rectangle in mm")),
            optional("depth", Kind::Num, tr_const!("Depth of the covered rectangle in mm")),
            optional("along_z", Kind::Bool, ROOF_ALONG_Z),
            optional("flip", Kind::Bool, ROOF_FLIP),
            optional("eaves", Kind::Num, ROOF_EAVES),
            optional("pitch", Kind::Num, tr_const!("Pitch of the slopes in degrees")),
            optional("overhang", Kind::Num, tr_const!("Overhang over the walls in mm (default 400)")),
            optional("thickness", Kind::Num, tr_const!("Thickness of the roof in mm (default 300)")),
            optional("name", Kind::Str, tr_const!("Name (default R1, R2, …)")),
        ],
        run: add_roof,
    },
    Spec {
        name: "set_roof",
        help: tr_const!("Changes the shape, the rectangle, the eaves, the pitch, the overhang or the thickness of a roof, renames or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            optional("shape", Kind::Str, ROOF_SHAPE),
            optional("x", Kind::Num, ROOF_X),
            optional("z", Kind::Num, ROOF_Z),
            optional("width", Kind::Num, tr_const!("Width of the covered rectangle in mm")),
            optional("depth", Kind::Num, tr_const!("Depth of the covered rectangle in mm")),
            optional("along_z", Kind::Bool, ROOF_ALONG_Z),
            optional("flip", Kind::Bool, ROOF_FLIP),
            optional("eaves", Kind::Num, ROOF_EAVES),
            optional("pitch", Kind::Num, tr_const!("Pitch of the slopes in degrees")),
            optional("overhang", Kind::Num, tr_const!("Overhang over the walls in mm")),
            optional("thickness", Kind::Num, tr_const!("Thickness of the roof in mm")),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the roof")),
        ],
        run: set_roof,
    },
    Spec {
        name: "add_roof_window",
        help: tr_const!("Adds a roof window to a slope of a roof; without x and z halfway up its front or right slope"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            optional("x", Kind::Num, WINDOW_X),
            optional("z", Kind::Num, WINDOW_Z),
            optional("width", Kind::Num, tr_const!("Width along the eaves in mm (default 780)")),
            optional("length", Kind::Num, tr_const!("Length up the slope in mm (default 1180)")),
            optional("part", Kind::Str, tr_const!("Part number of the window")),
        ],
        run: add_roof_window,
    },
    Spec {
        name: "set_roof_window",
        help: tr_const!("Moves or resizes a roof window, changes its part or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            param("window", Kind::Int, tr_const!("Id of the roof window")),
            optional("x", Kind::Num, WINDOW_X),
            optional("z", Kind::Num, WINDOW_Z),
            optional("width", Kind::Num, tr_const!("Width along the eaves in mm")),
            optional("length", Kind::Num, tr_const!("Length up the slope in mm")),
            optional("part", Kind::Str, tr_const!("Part number of the window")),
            optional("remove", Kind::Bool, tr_const!("Remove the roof window")),
        ],
        run: set_roof_window,
    },
    Spec {
        name: "add_pv_array",
        help: tr_const!("Adds a PV array to a roof: rows of modules on rails and roof hooks parallel to a slope, or on a flat roof tilted or east-west on ballasted mounting bases; without x and z in the middle of the front or right slope. The modules and the mounting material go into the bill of materials"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            optional("x", Kind::Num, PV_X),
            optional("z", Kind::Num, PV_Z),
            optional("columns", Kind::Int, PV_COLUMNS),
            optional("rows", Kind::Int, PV_ROWS),
            optional("landscape", Kind::Bool, PV_LANDSCAPE),
            optional("module", Kind::Str, PV_MODULE),
            optional("mounting", Kind::Str, PV_MOUNTING),
            optional("tilt", Kind::Num, PV_TILT),
            optional("azimuth", Kind::Num, PV_AZIMUTH),
            optional("row_gap", Kind::Num, PV_ROW_GAP),
            optional("span", Kind::Num, PV_SPAN),
            optional("ballast", Kind::Num, PV_BALLAST),
            optional("system", Kind::Str, PV_SYSTEM),
            optional("device", Kind::Str, PV_DEVICE),
            optional("name", Kind::Str, tr_const!("Name (default PV1, PV2, …)")),
        ],
        run: add_pv_array,
    },
    Spec {
        name: "set_pv_array",
        help: tr_const!("Moves a PV array, changes its modules, their number and orientation, its mounting or mounting system, renames or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            param("array", Kind::Str, PV_ARRAY),
            optional("x", Kind::Num, PV_X),
            optional("z", Kind::Num, PV_Z),
            optional("columns", Kind::Int, PV_COLUMNS),
            optional("rows", Kind::Int, PV_ROWS),
            optional("landscape", Kind::Bool, PV_LANDSCAPE),
            optional("module", Kind::Str, PV_MODULE),
            optional("mounting", Kind::Str, PV_MOUNTING),
            optional("tilt", Kind::Num, PV_TILT),
            optional("azimuth", Kind::Num, PV_AZIMUTH),
            optional("row_gap", Kind::Num, PV_ROW_GAP),
            optional("span", Kind::Num, PV_SPAN),
            optional("ballast", Kind::Num, PV_BALLAST),
            optional("system", Kind::Str, PV_SYSTEM),
            optional("device", Kind::Str, PV_DEVICE),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the PV array")),
        ],
        run: set_pv_array,
    },
    Spec {
        name: "add_pv_string",
        help: tr_const!("Connects modules of a PV array as a string: modules in series, strings of this kind in parallel, on an MPP tracker of an inverter or charge controller; without series the modules no string takes yet, without inverter the first device with PV input data"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            param("array", Kind::Str, PV_ARRAY),
            optional("series", Kind::Int, PV_SERIES),
            optional("parallel", Kind::Int, PV_PARALLEL),
            optional("inverter", Kind::Str, PV_INVERTER),
            optional("mppt", Kind::Int, PV_MPPT),
            optional("name", Kind::Str, tr_const!("Name (default S1, S2, …)")),
        ],
        run: add_pv_string,
    },
    Spec {
        name: "set_pv_string",
        help: tr_const!("Changes the modules in series or in parallel of a string, its inverter or MPP tracker, renames or removes it"),
        params: &[
            param("building", Kind::Str, BUILDING),
            param("roof", Kind::Str, ROOF),
            param("array", Kind::Str, PV_ARRAY),
            param("string", Kind::Str, tr_const!("String: its name")),
            optional("series", Kind::Int, PV_SERIES),
            optional("parallel", Kind::Int, PV_PARALLEL),
            optional("inverter", Kind::Str, PV_INVERTER),
            optional("mppt", Kind::Int, PV_MPPT),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the string")),
        ],
        run: set_pv_string,
    },
    Spec {
        name: "pv_strings",
        help: tr_const!("Lists the strings of all PV arrays with their voltages (highest, mean, lowest), currents and power at the design temperatures and the problems of the strings; sets the cell temperatures they are designed for"),
        params: &[
            optional("t_min", Kind::Num, tr_const!("Lowest cell temperature in °C (default -10): the highest open-circuit voltage")),
            optional("t_mean", Kind::Num, tr_const!("Mean cell temperature in °C (default 25)")),
            optional("t_max", Kind::Num, tr_const!("Highest cell temperature in °C (default 70): the lowest voltage")),
        ],
        run: pv_strings,
    },
    Spec {
        name: "add_site_link",
        help: tr_const!("Links a building entry of one room to a building entry of a room in another building: a cable trench, a conduit or a cable bridge, along which cables, tubes and pipes are routed from building to building"),
        params: &[
            param("from", Kind::Str, tr_const!("Room with the first building entry")),
            param("to", Kind::Str, tr_const!("Room with the second building entry")),
            optional("kind", Kind::Str, KIND),
            optional("from_entry", Kind::Int, tr_const!("Id of the building entry in the first room (default its first one)")),
            optional("to_entry", Kind::Int, tr_const!("Id of the building entry in the second room (default its first one)")),
            optional("points", Kind::Points, tr_const!("Corners of the way in the plant (x, z in mm)")),
            optional("length", Kind::Num, tr_const!("Length in mm (default along the way)")),
            optional("name", Kind::Str, tr_const!("Name (default SL1, SL2, …)")),
        ],
        run: add_site_link,
    },
    Spec {
        name: "set_site_link",
        help: tr_const!("Changes the kind, the corners or the length of a link between buildings, renames or removes it"),
        params: &[
            param("link", Kind::Str, LINK),
            optional("kind", Kind::Str, KIND),
            optional("points", Kind::Points, tr_const!("Corners of the way in the plant (x, z in mm)")),
            optional("length", Kind::Num, tr_const!("Length in mm; 0 along the way")),
            optional("name", Kind::Str, tr_const!("New name")),
            optional("remove", Kind::Bool, tr_const!("Remove the link")),
        ],
        run: set_site_link,
    },
    Spec {
        name: "plot",
        help: tr_const!("Shows the plot: its boundary with area and length, the reference point and what stands outside the boundary"),
        params: &[],
        run: plot,
    },
    Spec {
        name: "set_plot",
        help: tr_const!("Draws the boundary of the plot, which all buildings have to stay inside (the check reports rooms, cabinets and roofs outside it), or renames the plot"),
        params: &[
            optional("outline", Kind::Points, tr_const!("Corners of the plot boundary in the plant (x, z in mm)")),
            optional("name", Kind::Str, tr_const!("Name of the plot, e.g. the cadastral parcel")),
            optional("remove", Kind::Bool, tr_const!("Remove the boundary")),
        ],
        run: set_plot,
    },
    Spec {
        name: "set_reference_point",
        help: tr_const!("Sets the georeferenced point of the plant: where it is in the plant and its coordinates; every point of the plant is measured from it"),
        params: &[
            optional("x", Kind::Num, tr_const!("Where the point is in the plant, x in mm")),
            optional("z", Kind::Num, tr_const!("Where the point is in the plant, z (towards the front) in mm")),
            optional("y", Kind::Num, tr_const!("Its height above the ground level of the plant in mm")),
            optional("easting", Kind::Num, tr_const!("Its easting in the coordinate reference system in m")),
            optional("northing", Kind::Num, tr_const!("Its northing in the coordinate reference system in m")),
            optional("height", Kind::Num, tr_const!("Its height above sea level (datum of the coordinate reference system) in m")),
            optional("crs", Kind::Str, tr_const!("Coordinate reference system, e.g. EPSG:25832")),
            optional("north", Kind::Num, tr_const!("Direction of grid north in degrees, counter-clockwise seen from above from -z (0: towards the rear)")),
            optional("latitude", Kind::Num, tr_const!("Latitude (WGS 84) in degrees")),
            optional("longitude", Kind::Num, tr_const!("Longitude (WGS 84) in degrees")),
            optional("name", Kind::Str, tr_const!("Name of the point, e.g. a survey point")),
            optional("remove", Kind::Bool, tr_const!("Remove the reference point")),
        ],
        run: set_reference_point,
    },
    Spec {
        name: "locate",
        help: tr_const!("Gives the coordinates of a point: from x and z of the plant its easting, northing and height, or from easting and northing its place in the plant; and whether it lies on the plot"),
        params: &[
            optional("x", Kind::Num, tr_const!("x in the plant in mm")),
            optional("z", Kind::Num, tr_const!("z in the plant (towards the front) in mm")),
            optional("y", Kind::Num, tr_const!("Height above the ground level of the plant in mm")),
            optional("easting", Kind::Num, tr_const!("Easting in m")),
            optional("northing", Kind::Num, tr_const!("Northing in m")),
        ],
        run: locate,
    },
    Spec {
        name: "measure",
        help: tr_const!("Measures from one point of the plant to another: distance, how far east and north, and the direction from grid north"),
        params: &[param("points", Kind::Points, tr_const!("The two points in the plant (x, z in mm)"))],
        run: measure,
    },
];

fn building_arg(prj: &Project, a: &Args, name: &str) -> Result<usize, String> {
    let key = a.req(name);
    bld::building_index(&prj.layout, key).ok_or_else(|| tr_fmt!("No building named {}", key))
}

fn level_arg(prj: &Project, bi: usize, key: &str) -> Result<u64, String> {
    let b = &prj.layout.buildings[bi];
    bld::level_index(b, key).map(|li| b.levels[li].id).ok_or_else(|| tr_fmt!("No level {} in {}", key, b.name))
}

fn room_arg(prj: &Project, name: &str) -> Result<usize, String> {
    let i = prj.layout.enclosures.iter().position(|e| e.name == name.trim()).ok_or_else(|| tr_fmt!("No enclosure named {}", name))?;
    if !prj.layout.enclosures[i].is_room() {
        return Err(tr_fmt!("{} is not a room", name));
    }
    Ok(i)
}

fn kind_arg(a: &Args) -> Result<Option<LinkKind>, String> {
    a.str("kind").map(|k| LinkKind::from_key(k).ok_or_else(|| tr_fmt!("Unknown kind: {}", k))).transpose()
}

fn level_name(prj: &Project, id: u64) -> String {
    bld::level_by_id(&prj.layout, id).map(|l| l.name.clone()).unwrap_or_default()
}

fn building_json(prj: &Project, bi: usize) -> Value {
    let layout = &prj.layout;
    let b = &layout.buildings[bi];
    let poses = panel::site_poses(prj);
    let name = |ri: usize| layout.enclosures[ri].name.clone();
    let levels: Vec<Value> = b
        .levels
        .iter()
        .map(|l| {
            let rooms: Vec<Value> = bld::rooms_on(layout, l.id)
                .into_iter()
                .map(|ri| {
                    let e = &layout.enclosures[ri];
                    let p = e.site.unwrap_or_default();
                    let mut room = json!({ "room": e.name, "x": p.x, "z": p.z, "angle": p.angle, "height": e.height, "placed": e.site.is_some() });
                    // Under a roof that slopes its ceiling: its lowest and highest ceiling.
                    if let Some(c) = roofs::ceiling_with(prj, &poses, ri) {
                        let (lo, hi) = c.range(&roofs::floor_polys(e));
                        room["ceiling"] = json!({ "lowest": lo, "highest": hi });
                    }
                    room
                })
                .collect();
            json!({ "name": l.name, "elevation": l.elevation, "height": l.height, "rooms": rooms })
        })
        .collect();
    let shafts: Vec<Value> = b
        .shafts
        .iter()
        .map(|s| {
            let rooms: Vec<String> = bld::shaft_rooms(prj, &poses, bi, s).into_iter().map(name).collect();
            json!({ "name": s.name, "x": s.x, "z": s.z, "width": s.width, "depth": s.depth, "bottom": level_name(prj, s.bottom), "top": level_name(prj, s.top), "rooms": rooms })
        })
        .collect();
    let stairs: Vec<Value> = b
        .stairs
        .iter()
        .map(|s| {
            let g = stairs::geometry(b, s);
            json!({
                "name": s.name, "kind": s.kind.key(), "turn": s.turn.key(), "x": s.x, "z": s.z, "angle": s.angle, "width": s.width, "tread": s.tread,
                "bottom": level_name(prj, s.bottom), "top": level_name(prj, s.top),
                "risers": g.as_ref().map(|g| g.risers), "rise": g.as_ref().map(|g| g.rise),
            })
        })
        .collect();
    let roof_list: Vec<Value> = b
        .roofs
        .iter()
        .map(|r| {
            let windows: Vec<Value> = r.windows.iter().map(|w| json!({ "id": w.id, "x": w.x, "z": w.z, "width": w.width, "length": w.length, "part": w.part })).collect();
            let arrays: Vec<Value> = r.solar.iter().map(|a| pv_json(prj, a)).collect();
            json!({
                "name": r.name, "shape": r.shape.key(), "x": r.x, "z": r.z, "width": r.width, "depth": r.depth, "along_z": r.along_z, "flip": r.flip,
                "eaves": r.eaves, "pitch": r.pitch, "overhang": r.overhang, "thickness": r.thickness, "ridge": roofs::ridge_height(r), "windows": windows,
                "solar": arrays,
            })
        })
        .collect();
    json!({
        "name": b.name,
        "description": b.description,
        "x": b.site.x,
        "z": b.site.z,
        "angle": b.site.angle,
        "geo": prj.layout.plot.reference.as_ref().map(|r| geo_json(r, [b.site.x, 0.0, b.site.z])),
        "levels": levels,
        "shafts": shafts,
        "stairs": stairs,
        "roofs": roof_list,
    })
}

fn link_json(prj: &Project, link: &openecraft_core::panel::SiteLink) -> Value {
    let layout = &prj.layout;
    let poses = panel::site_poses(prj);
    let end = |id: u64| bld::entry_room(layout, id).map(|(ri, _)| layout.enclosures[ri].name.clone());
    json!({
        "name": link.name,
        "kind": link.kind.key(),
        "from": end(link.a),
        "from_entry": link.a,
        "to": end(link.b),
        "to_entry": link.b,
        "points": link.points,
        "length": bld::site_link_length(prj, &poses, link),
    })
}

fn list_buildings(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let list: Vec<usize> = match a.str("building") {
        Some(_) => vec![building_arg(prj, a, "building")?],
        None => (0..prj.layout.buildings.len()).collect(),
    };
    let links: Vec<Value> = prj.layout.site_links.iter().map(|l| link_json(prj, l)).collect();
    let problems: Vec<String> = bld::problems(prj).iter().map(|i| i.message()).collect();
    let mut out = json!({ "buildings": list.into_iter().map(|bi| building_json(prj, bi)).collect::<Vec<_>>(), "links": links, "problems": problems });
    if !prj.layout.plot.is_default() {
        out["plot"] = plot_json(prj);
    }
    Ok(out)
}

fn place_of(a: &Args, old: SitePlace) -> SitePlace {
    SitePlace { x: a.num("x").unwrap_or(old.x), z: a.num("z").unwrap_or(old.z), angle: a.num("angle").unwrap_or(old.angle) }
}

fn add_building(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = bld::add_building(prj, a.req("name"))?;
    let b = &mut prj.layout.buildings[bi];
    b.site = place_of(a, b.site);
    if let Some(d) = a.str("description") {
        b.description = d.into();
    }
    Ok(building_json(prj, bi))
}

fn set_building(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    if a.flag("remove") {
        let name = prj.layout.buildings[bi].name.clone();
        bld::remove_building(prj, bi);
        return Ok(json!({ "removed": name }));
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        if prj.layout.buildings.iter().enumerate().any(|(k, b)| k != bi && b.name == n) || prj.layout.enclosures.iter().any(|e| e.name == n) {
            return Err(tr_fmt!("{} exists already", n));
        }
        prj.layout.buildings[bi].name = n.into();
    }
    let b = &mut prj.layout.buildings[bi];
    b.site = place_of(a, b.site);
    if let Some(d) = a.str("description") {
        b.description = d.into();
    }
    Ok(building_json(prj, bi))
}

fn add_level(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    bld::add_level(prj, bi, a.str("name"), a.num("elevation"), a.num("height"))?;
    Ok(building_json(prj, bi))
}

fn set_level(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let id = level_arg(prj, bi, a.req("level"))?;
    if a.flag("remove") {
        bld::remove_level(prj, bi, id)?;
    } else {
        bld::set_level(prj, bi, id, a.str("name"), a.num("elevation"), a.num("height"))?;
    }
    Ok(building_json(prj, bi))
}

fn place_on_level(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let ri = room_arg(prj, a.req("room"))?;
    let Some(bi) = a.str("building").filter(|b| !b.trim().is_empty()).map(|_| building_arg(prj, a, "building")).transpose()? else {
        bld::set_room_level(prj, ri, None, None)?;
        return Ok(json!({ "room": prj.layout.enclosures[ri].name, "building": Value::Null }));
    };
    let level = match a.str("level") {
        Some(l) => level_arg(prj, bi, l)?,
        None => prj.layout.buildings[bi].levels.iter().find(|l| l.elevation.abs() < 0.5).or(prj.layout.buildings[bi].levels.first()).map(|l| l.id).ok_or_else(|| tr("No such level").to_string())?,
    };
    let place = (a.num("x").is_some() || a.num("z").is_some() || a.num("angle").is_some()).then(|| place_of(a, SitePlace::default()));
    bld::set_room_level(prj, ri, Some(level), place)?;
    Ok(building_json(prj, bi))
}

/// The shaft of building `bi` named by `shaft`.
fn shaft_arg(prj: &Project, bi: usize, a: &Args) -> Result<usize, String> {
    let key = a.req("shaft").trim();
    let b = &prj.layout.buildings[bi];
    b.shafts.iter().position(|s| s.name.eq_ignore_ascii_case(key)).ok_or_else(|| tr_fmt!("No shaft {} in {}", key, b.name))
}

fn add_shaft(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let b = &prj.layout.buildings[bi];
    let (first, last) = (b.levels.first().map_or(0, |l| l.id), b.levels.last().map_or(0, |l| l.id));
    let bottom = a.str("bottom").map(|l| level_arg(prj, bi, l)).transpose()?.unwrap_or(first);
    let top = a.str("top").map(|l| level_arg(prj, bi, l)).transpose()?.unwrap_or(last);
    let size = |n: &str| num_in(a, n, 100.0, NO_MAX).map(|v| v.unwrap_or(SHAFT_SIZE));
    let (x, z) = (a.num("x").unwrap_or_default(), a.num("z").unwrap_or_default());
    bld::add_shaft(prj, bi, a.str("name"), x, z, size("width")?, size("depth")?, bottom, top)?;
    Ok(building_json(prj, bi))
}

fn set_shaft(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let si = shaft_arg(prj, bi, a)?;
    if a.flag("remove") {
        prj.layout.buildings[bi].shafts.remove(si);
        return Ok(building_json(prj, bi));
    }
    let mut s = prj.layout.buildings[bi].shafts[si].clone();
    s.x = a.num("x").unwrap_or(s.x);
    s.z = a.num("z").unwrap_or(s.z);
    s.width = a.num("width").unwrap_or(s.width);
    s.depth = a.num("depth").unwrap_or(s.depth);
    if let Some(l) = a.str("bottom") {
        s.bottom = level_arg(prj, bi, l)?;
    }
    if let Some(l) = a.str("top") {
        s.top = level_arg(prj, bi, l)?;
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        s.name = n.into();
    }
    let s = bld::checked_shaft(&prj.layout.buildings[bi], s)?;
    prj.layout.buildings[bi].shafts[si] = s;
    Ok(building_json(prj, bi))
}

fn stair_kind_arg(a: &Args) -> Result<Option<StairKind>, String> {
    a.str("kind").map(|k| StairKind::from_key(k).ok_or_else(|| tr_fmt!("Unknown kind: {}", k))).transpose()
}

fn turn_arg(a: &Args) -> Result<Option<Turn>, String> {
    a.str("turn").map(|k| Turn::from_key(k).ok_or_else(|| tr_fmt!("Unknown turn: {}", k))).transpose()
}

fn shape_arg(a: &Args) -> Result<Option<RoofShape>, String> {
    a.str("shape").map(|k| RoofShape::from_key(k).ok_or_else(|| tr_fmt!("Unknown roof shape: {}", k))).transpose()
}

/// The stair of building `bi` named by `stair`.
fn stair_arg(prj: &Project, bi: usize, a: &Args) -> Result<usize, String> {
    let key = a.req("stair").trim();
    let b = &prj.layout.buildings[bi];
    b.stairs.iter().position(|s| s.name.eq_ignore_ascii_case(key)).ok_or_else(|| tr_fmt!("No stair {} in {}", key, b.name))
}

/// The roof of building `bi` named by `roof`.
fn roof_arg(prj: &Project, bi: usize, a: &Args) -> Result<usize, String> {
    let key = a.req("roof").trim();
    let b = &prj.layout.buildings[bi];
    b.roofs.iter().position(|r| r.name.eq_ignore_ascii_case(key)).ok_or_else(|| tr_fmt!("No roof {} in {}", key, b.name))
}

/// Applies the optional arguments of a stair command.
fn stair_args(prj: &Project, bi: usize, a: &Args, mut s: panel::Stair) -> Result<panel::Stair, String> {
    s.x = a.num("x").unwrap_or(s.x);
    s.z = a.num("z").unwrap_or(s.z);
    s.angle = a.num("angle").unwrap_or(s.angle);
    s.kind = stair_kind_arg(a)?.unwrap_or(s.kind);
    s.turn = turn_arg(a)?.unwrap_or(s.turn);
    s.width = num_in(a, "width", 500.0, 5000.0)?.unwrap_or(s.width);
    s.tread = num_in(a, "tread", 150.0, 500.0)?.unwrap_or(s.tread);
    if let Some(l) = a.str("bottom") {
        s.bottom = level_arg(prj, bi, l)?;
    }
    if let Some(l) = a.str("top") {
        s.top = level_arg(prj, bi, l)?;
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        s.name = n.into();
    }
    stairs::checked_stair(&prj.layout.buildings[bi], s)
}

fn add_stair(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let bottom = match a.str("bottom") {
        Some(l) => level_arg(prj, bi, l)?,
        None => prj.layout.buildings[bi].levels.first().map(|l| l.id).ok_or_else(|| tr("No such level").to_string())?,
    };
    let top = a.str("top").map(|l| level_arg(prj, bi, l)).transpose()?;
    let id = stairs::add_stair(prj, bi, StairKind::Straight, a.num("x").unwrap_or_default(), a.num("z").unwrap_or_default(), bottom, top)?;
    let k = prj.layout.buildings[bi].stairs.iter().position(|s| s.id == id).unwrap_or_default();
    let s = stair_args(prj, bi, a, prj.layout.buildings[bi].stairs[k].clone());
    match s {
        Ok(s) => prj.layout.buildings[bi].stairs[k] = s,
        Err(e) => {
            prj.layout.buildings[bi].stairs.remove(k);
            return Err(e);
        }
    }
    Ok(building_json(prj, bi))
}

fn set_stair(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = stair_arg(prj, bi, a)?;
    if a.flag("remove") {
        prj.layout.buildings[bi].stairs.remove(k);
    } else {
        let s = stair_args(prj, bi, a, prj.layout.buildings[bi].stairs[k].clone())?;
        prj.layout.buildings[bi].stairs[k] = s;
    }
    Ok(building_json(prj, bi))
}

/// Applies the optional arguments of a roof command.
fn roof_args(a: &Args, mut r: panel::Roof) -> Result<panel::Roof, String> {
    r.shape = shape_arg(a)?.unwrap_or(r.shape);
    r.x = a.num("x").unwrap_or(r.x);
    r.z = a.num("z").unwrap_or(r.z);
    r.width = num_in(a, "width", 500.0, NO_MAX)?.unwrap_or(r.width);
    r.depth = num_in(a, "depth", 500.0, NO_MAX)?.unwrap_or(r.depth);
    if a.value("along_z").is_some() {
        r.along_z = a.flag("along_z");
    }
    if a.value("flip").is_some() {
        r.flip = a.flag("flip");
    }
    r.eaves = a.num("eaves").unwrap_or(r.eaves);
    r.pitch = num_in(a, "pitch", 0.0, 75.0)?.unwrap_or(r.pitch);
    r.overhang = num_in(a, "overhang", 0.0, 3000.0)?.unwrap_or(r.overhang);
    r.thickness = num_in(a, "thickness", 50.0, 1000.0)?.unwrap_or(r.thickness);
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        r.name = n.into();
    }
    Ok(roofs::checked_roof(r))
}

fn add_roof(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let shape = shape_arg(a)?.unwrap_or_default();
    let id = roofs::add_roof(prj, bi, shape)?;
    let k = prj.layout.buildings[bi].roofs.iter().position(|r| r.id == id).unwrap_or_default();
    match roof_args(a, prj.layout.buildings[bi].roofs[k].clone()) {
        Ok(r) => prj.layout.buildings[bi].roofs[k] = r,
        Err(e) => {
            prj.layout.buildings[bi].roofs.remove(k);
            return Err(e);
        }
    }
    Ok(building_json(prj, bi))
}

fn set_roof(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    if a.flag("remove") {
        prj.layout.buildings[bi].roofs.remove(k);
    } else {
        let r = roof_args(a, prj.layout.buildings[bi].roofs[k].clone())?;
        prj.layout.buildings[bi].roofs[k] = r;
    }
    Ok(building_json(prj, bi))
}

/// Applies the optional arguments of a roof window command.
fn window_args(a: &Args, w: &mut panel::RoofWindow) -> Result<(), String> {
    w.x = a.num("x").unwrap_or(w.x);
    w.z = a.num("z").unwrap_or(w.z);
    w.width = num_in(a, "width", 200.0, 5000.0)?.unwrap_or(w.width);
    w.length = num_in(a, "length", 200.0, 5000.0)?.unwrap_or(w.length);
    if let Some(p) = a.str("part") {
        w.part = p.trim().into();
    }
    Ok(())
}

fn add_roof_window(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    let at = match (a.num("x"), a.num("z")) {
        (Some(x), Some(z)) => Some([x, z]),
        _ => None,
    };
    let id = roofs::add_window(prj, bi, k, at)?;
    let windows = &mut prj.layout.buildings[bi].roofs[k].windows;
    let w = windows.iter_mut().find(|w| w.id == id).ok_or_else(|| tr("No such roof window").to_string())?;
    if let Err(e) = window_args(a, w) {
        windows.retain(|w| w.id != id);
        return Err(e);
    }
    Ok(building_json(prj, bi))
}

fn set_roof_window(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    let id = int_in(a, "window", 1, NO_LIMIT)?.unwrap_or_default() as u64;
    let windows = &mut prj.layout.buildings[bi].roofs[k].windows;
    let i = windows.iter().position(|w| w.id == id).ok_or_else(|| tr("No such roof window").to_string())?;
    if a.flag("remove") {
        windows.remove(i);
    } else {
        let mut w = windows[i].clone();
        window_args(a, &mut w)?;
        windows[i] = w;
    }
    Ok(building_json(prj, bi))
}

fn pv_json(prj: &Project, a: &panel::PvArray) -> Value {
    let material: Vec<Value> = solar::material(prj, a).into_iter().map(|(part, qty)| json!({ "part": part, "qty": qty })).collect();
    json!({
        "id": a.id, "name": a.name, "x": a.x, "z": a.z, "columns": a.columns, "rows": a.rows, "landscape": a.landscape,
        "module": a.module, "module_size": [a.module_width, a.module_length, a.module_height], "modules": solar::module_count(a),
        "peak_kw": solar::peak_power(prj, a), "mounting": a.mounting.key(), "tilt": a.tilt, "azimuth": a.azimuth, "row_gap": a.row_gap,
        "span": a.span, "ballast": a.ballast, "system": a.system, "device": a.device, "material": material,
        "strings": a.strings.iter().map(|s| string_json(prj, a, s)).collect::<Vec<_>>(),
    })
}

fn string_json(prj: &Project, a: &panel::PvArray, s: &panel::PvString) -> Value {
    let values = strings::module_data(prj, &a.module).map(|m| {
        let v = strings::values(&m, &prj.layout.pv_design, s);
        json!({
            "voltage_max": v.voc_max, "voltage_mean": v.vmp_mean, "voltage_min": v.vmp_min, "voltage_mpp_cold": v.vmp_max, "voc": v.voc,
            "current": v.imp, "short_circuit_max": v.isc_max, "power": v.power, "power_hot": v.power_hot,
        })
    });
    json!({ "id": s.id, "name": s.name, "series": s.series, "parallel": s.parallel, "modules": s.series * s.parallel, "inverter": s.inverter, "mppt": s.mppt, "values": values })
}

/// The array named by `array` on the roof named by `roof` of the building `building`.
fn array_args(prj: &Project, a: &Args) -> Result<(usize, usize, usize), String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    let i = pv_arg(prj, bi, k, a)?;
    Ok((bi, k, i))
}

/// Applies the optional arguments of a string command.
fn string_args(a: &Args, mut s: panel::PvString) -> Result<panel::PvString, String> {
    s.series = int_in(a, "series", 1, 200)?.map_or(s.series, |n| n as u32);
    s.parallel = int_in(a, "parallel", 1, 50)?.map_or(s.parallel, |n| n as u32);
    s.mppt = int_in(a, "mppt", 1, 64)?.map_or(s.mppt, |n| n as u32);
    if let Some(t) = a.str("inverter") {
        s.inverter = t.trim().into();
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        s.name = n.into();
    }
    Ok(strings::checked_string(s))
}

fn add_pv_string(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (bi, k, i) = array_args(prj, a)?;
    let id = strings::add_string(prj, bi, k, i)?;
    let list = &mut prj.layout.buildings[bi].roofs[k].solar[i].strings;
    let j = list.iter().position(|s| s.id == id).unwrap_or_default();
    match string_args(a, list[j].clone()) {
        Ok(s) => list[j] = s,
        Err(e) => {
            list.remove(j);
            return Err(e);
        }
    }
    Ok(pv_json(prj, &prj.layout.buildings[bi].roofs[k].solar[i]))
}

fn set_pv_string(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (bi, k, i) = array_args(prj, a)?;
    let key = a.req("string").trim();
    let arr = &prj.layout.buildings[bi].roofs[k].solar[i];
    let j = arr.strings.iter().position(|s| s.name.eq_ignore_ascii_case(key)).ok_or_else(|| tr_fmt!("No string {} in PV array {}", key, arr.name))?;
    if a.flag("remove") {
        prj.layout.buildings[bi].roofs[k].solar[i].strings.remove(j);
    } else {
        let s = string_args(a, arr.strings[j].clone())?;
        prj.layout.buildings[bi].roofs[k].solar[i].strings[j] = s;
    }
    Ok(pv_json(prj, &prj.layout.buildings[bi].roofs[k].solar[i]))
}

fn pv_strings(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let mut d = prj.layout.pv_design;
    d.t_min = num_in(a, "t_min", -60.0, 40.0)?.unwrap_or(d.t_min);
    d.t_mean = num_in(a, "t_mean", -40.0, 70.0)?.unwrap_or(d.t_mean);
    d.t_max = num_in(a, "t_max", 0.0, 110.0)?.unwrap_or(d.t_max);
    if d.t_min > d.t_mean || d.t_mean > d.t_max {
        return Err(tr("The temperatures must rise from t_min over t_mean to t_max").to_string());
    }
    prj.layout.pv_design = d;
    let list: Vec<Value> = strings::all_strings(prj)
        .into_iter()
        .map(|r| {
            let mut v = string_json(prj, r.array, r.string);
            v["building"] = json!(r.building.name);
            v["roof"] = json!(r.roof.name);
            v["array"] = json!(r.array.name);
            v
        })
        .collect();
    let problems: Vec<String> = strings::problems(prj).iter().map(|i| i.message()).collect();
    Ok(json!({ "t_min": d.t_min, "t_mean": d.t_mean, "t_max": d.t_max, "strings": list, "inverters": strings::inverters(prj), "problems": problems }))
}

/// The PV array of roof `k` of building `bi` named by `array`.
fn pv_arg(prj: &Project, bi: usize, k: usize, a: &Args) -> Result<usize, String> {
    let key = a.req("array").trim();
    let r = &prj.layout.buildings[bi].roofs[k];
    r.solar.iter().position(|s| s.name.eq_ignore_ascii_case(key)).ok_or_else(|| tr_fmt!("No PV array {} on roof {}", key, r.name))
}

/// Applies the optional arguments of a PV array command.
fn pv_args(prj: &Project, a: &Args, mut s: panel::PvArray) -> Result<panel::PvArray, String> {
    s.x = a.num("x").unwrap_or(s.x);
    s.z = a.num("z").unwrap_or(s.z);
    s.columns = int_in(a, "columns", 1, 200)?.map_or(s.columns, |n| n as u32);
    s.rows = int_in(a, "rows", 1, 100)?.map_or(s.rows, |n| n as u32);
    if a.value("landscape").is_some() {
        s.landscape = a.flag("landscape");
    }
    if let Some(m) = a.str("module") {
        solar::set_module(prj, &mut s, m)?;
    }
    if let Some(m) = a.str("mounting") {
        s.mounting = PvMounting::from_key(m).ok_or_else(|| tr_fmt!("Unknown mounting: {}", m))?;
    }
    s.tilt = num_in(a, "tilt", 0.0, 60.0)?.unwrap_or(s.tilt);
    s.azimuth = a.num("azimuth").unwrap_or(s.azimuth);
    s.row_gap = num_in(a, "row_gap", 0.0, 20_000.0)?.unwrap_or(s.row_gap);
    s.span = num_in(a, "span", 300.0, 3000.0)?.unwrap_or(s.span);
    s.ballast = num_in(a, "ballast", 0.0, 500.0)?.unwrap_or(s.ballast);
    if let Some(p) = a.str("system").map(str::trim) {
        if !p.is_empty() && !prj.parts.iter().chain(openecraft_core::parts::catalog()).any(|q| q.number == p) {
            return Err(tr_fmt!("No part {} in the project", p));
        }
        s.system = p.into();
    }
    if let Some(d) = a.str("device") {
        s.device = d.trim().into();
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        s.name = n.into();
    }
    Ok(solar::checked_array(s))
}

fn add_pv_array(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    let at = match (a.num("x"), a.num("z")) {
        (Some(x), Some(z)) => Some([x, z]),
        _ => None,
    };
    let id = solar::add_array(prj, bi, k, at)?;
    let i = prj.layout.buildings[bi].roofs[k].solar.iter().position(|s| s.id == id).unwrap_or_default();
    match pv_args(prj, a, prj.layout.buildings[bi].roofs[k].solar[i].clone()) {
        Ok(s) => prj.layout.buildings[bi].roofs[k].solar[i] = s,
        Err(e) => {
            prj.layout.buildings[bi].roofs[k].solar.remove(i);
            return Err(e);
        }
    }
    Ok(building_json(prj, bi))
}

fn set_pv_array(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let bi = building_arg(prj, a, "building")?;
    let k = roof_arg(prj, bi, a)?;
    let i = pv_arg(prj, bi, k, a)?;
    if a.flag("remove") {
        prj.layout.buildings[bi].roofs[k].solar.remove(i);
    } else {
        let s = pv_args(prj, a, prj.layout.buildings[bi].roofs[k].solar[i].clone())?;
        prj.layout.buildings[bi].roofs[k].solar[i] = s;
    }
    Ok(building_json(prj, bi))
}

/// A building entry of room `ri`: the one with id `entry`, otherwise its first one.
fn entry_of(prj: &Project, ri: usize, entry: Option<i64>) -> Result<u64, String> {
    let e = &prj.layout.enclosures[ri];
    let mut entries = e.items.iter().filter(|i| i.opening == Opening::Entry);
    let found = match entry {
        Some(id) => entries.find(|i| i.id as i64 == id),
        None => entries.next(),
    };
    found.map(|i| i.id).ok_or_else(|| tr_fmt!("{} has no such building entry", e.name))
}

fn add_site_link(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let (ra, rb) = (room_arg(prj, a.req("from"))?, room_arg(prj, a.req("to"))?);
    let ea = entry_of(prj, ra, int_in(a, "from_entry", 1, NO_LIMIT)?)?;
    let eb = entry_of(prj, rb, int_in(a, "to_entry", 1, NO_LIMIT)?)?;
    let kind = kind_arg(a)?.unwrap_or_default();
    let id = bld::add_site_link(prj, ea, eb, kind, a.points("points"), a.str("name"))?;
    if let Some(l) = num_in(a, "length", 0.0, NO_MAX)? {
        if let Some(link) = prj.layout.site_links.iter_mut().find(|k| k.id == id) {
            link.length = l;
        }
    }
    let link = prj.layout.site_links.iter().find(|k| k.id == id).cloned().unwrap_or_default();
    Ok(link_json(prj, &link))
}

fn set_site_link(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let key = a.req("link").trim().to_string();
    let k = prj.layout.site_links.iter().position(|l| l.name.eq_ignore_ascii_case(&key)).ok_or_else(|| tr_fmt!("No link named {}", key))?;
    if a.flag("remove") {
        let l = prj.layout.site_links.remove(k);
        return Ok(json!({ "removed": l.name }));
    }
    let kind = kind_arg(a)?;
    let length = num_in(a, "length", 0.0, NO_MAX)?;
    let link = &mut prj.layout.site_links[k];
    if let Some(kind) = kind {
        link.kind = kind;
    }
    if a.value("points").is_some() {
        link.points = a.points("points");
    }
    if let Some(l) = length {
        link.length = l;
    }
    if let Some(n) = a.str("name").map(str::trim).filter(|n| !n.is_empty()) {
        link.name = n.into();
    }
    let link = prj.layout.site_links[k].clone();
    Ok(link_json(prj, &link))
}

/// What stands, by name: a room, a cabinet or a roof ("+B1 R1").
fn standing_name(prj: &Project, s: plot::Standing) -> String {
    let layout = &prj.layout;
    match s {
        plot::Standing::Room(i) | plot::Standing::Cabinet(i) => layout.enclosures[i].name.clone(),
        plot::Standing::Roof(bi, k) => format!("{} {}", layout.buildings[bi].name, layout.buildings[bi].roofs[k].name),
    }
}

/// Easting, northing and height of a point of the plant, rounded to the millimetre.
fn geo_json(r: &panel::GeoReference, q: [f32; 3]) -> Value {
    let g = plot::to_geo(r, q);
    let mm = |v: f64| (v * 1000.0).round() / 1000.0;
    json!({ "easting": mm(g[0]), "northing": mm(g[1]), "height": mm(g[2]) })
}

fn plot_json(prj: &Project) -> Value {
    let p = &prj.layout.plot;
    let reference = p.reference.as_ref().map(|r| {
        json!({
            "name": r.name, "x": r.x, "z": r.z, "y": r.y, "easting": r.easting, "northing": r.northing, "height": r.height, "crs": r.crs, "north": r.north,
            "latitude": r.latitude, "longitude": r.longitude, "plant_origin": geo_json(r, [0.0; 3]),
        })
    });
    let outside: Vec<String> = plot::outside(prj).into_iter().map(|s| standing_name(prj, s)).collect();
    json!({
        "name": p.name,
        "outline": p.outline,
        "area": (plot::area(&p.outline) * 100.0).round() / 100.0,
        "perimeter": (plot::perimeter(&p.outline) * 1000.0).round() / 1000.0,
        "reference": reference,
        "outside": outside,
    })
}

fn plot(prj: &mut Project, _: &Args) -> Result<Value, String> {
    Ok(plot_json(prj))
}

fn set_plot(prj: &mut Project, a: &Args) -> Result<Value, String> {
    if a.flag("remove") {
        plot::set_outline(prj, Vec::new())?;
    } else if a.value("outline").is_some() {
        plot::set_outline(prj, a.points("outline"))?;
    }
    if let Some(n) = a.str("name") {
        prj.layout.plot.name = n.trim().into();
    }
    Ok(plot_json(prj))
}

fn set_reference_point(prj: &mut Project, a: &Args) -> Result<Value, String> {
    if a.flag("remove") {
        plot::set_reference(prj, None)?;
        return Ok(plot_json(prj));
    }
    let mut r = prj.layout.plot.reference.clone().unwrap_or_default();
    r.x = a.num("x").unwrap_or(r.x);
    r.z = a.num("z").unwrap_or(r.z);
    r.y = a.num("y").unwrap_or(r.y);
    r.easting = a.real("easting").unwrap_or(r.easting);
    r.northing = a.real("northing").unwrap_or(r.northing);
    r.height = a.real("height").unwrap_or(r.height);
    r.north = a.num("north").unwrap_or(r.north);
    if let Some(c) = a.str("crs") {
        r.crs = c.trim().into();
    }
    if let Some(n) = a.str("name") {
        r.name = n.trim().into();
    }
    if a.value("latitude").is_some() {
        r.latitude = a.real("latitude");
    }
    if a.value("longitude").is_some() {
        r.longitude = a.real("longitude");
    }
    plot::set_reference(prj, Some(r))?;
    Ok(plot_json(prj))
}

fn locate(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let r = plot::reference(&prj.layout);
    let q = match (a.real("easting"), a.real("northing")) {
        (Some(e), Some(n)) => {
            let height = r.height + f64::from(a.num("y").unwrap_or(0.0) - r.y) / 1000.0;
            plot::from_geo(&r, [e, n, height])
        }
        (None, None) => match (a.num("x"), a.num("z")) {
            (Some(x), Some(z)) => [x, a.num("y").unwrap_or(0.0), z],
            _ => return Err(tr("Give x and z of the plant or easting and northing").to_string()),
        },
        _ => return Err(tr("Give x and z of the plant or easting and northing").to_string()),
    };
    let d = plot::from_reference(&r, [q[0], q[2]]);
    let outline = &prj.layout.plot.outline;
    let on_plot = (!outline.is_empty()).then(|| plot::inside_or_on(outline, [q[0], q[2]]));
    let round = |v: f32| (v * 10.0).round() / 10.0;
    Ok(json!({
        "x": round(q[0]), "z": round(q[2]), "y": round(q[1]),
        "geo": prj.layout.plot.reference.as_ref().map(|r| geo_json(r, q)),
        "east": (d[0] * 1000.0).round() / 1000.0, "north": (d[1] * 1000.0).round() / 1000.0,
        "on_plot": on_plot,
    }))
}

fn measure(prj: &mut Project, a: &Args) -> Result<Value, String> {
    let [pa, pb] = a.points("points")[..] else { return Err(tr("Give two points").to_string()) };
    let m = plot::measure(&prj.layout, pa, pb);
    let r3 = |v: f64| (v * 1000.0).round() / 1000.0;
    Ok(json!({ "distance": (m.distance * 10.0).round() / 10.0, "east": r3(m.east), "north": r3(m.north), "bearing": (m.bearing * 100.0).round() / 100.0 }))
}

#[cfg(test)]
mod tests;
