//! 3D routing of the panel layout: wires, cables and tubes through the line-ups of
//! enclosures, in cable ducts, along routing paths or freely through the air around
//! the devices, with segregation of classes (power, control, signal, fluid), routing
//! reservations, bend radii, cable bundles and their branches, shield handling and a
//! recalculation that keeps routes frozen for production as long as they are valid.
//!
//! Wires are routed through the cable ducts by [`crate::panel::virtual_wiring`]; with
//! [`RoutingRules::wires_3d`] they are routed in 3D like cables and tubes, and so are
//! always the wires of a room and of a cabinet without cable ducts on its mounting
//! plate, from connection point to connection point ([`between_points`]): there is no
//! duct to lay them in, they keep clear of everything and of each other like a cable,
//! and one that finds no way is an error. All routes are polylines in layout
//! coordinates ([`crate::panel::positions`]: enclosure coordinates of the whole layout,
//! x right, y down, z from the rear walls).

use crate::model::{Cable, Project};
use crate::netlist::{Connection, Netlist, PinRef};
use crate::panel::{self, Face, ItemKind, Layout, PanelItem, RoutedWire, DUCT_PROFILE, PLATE_MARGIN, PLATE_Z};
use crate::parts::vec3::{add, dist, dot, manhattan, norm, scale, sub, V3};
use openecraft_i18n::{tr, tr_const, tr_fmt};
use std::collections::{BTreeMap, HashMap};

pub mod bundles;
pub mod flush;
pub mod grid;
pub mod room;
pub mod shield;
pub use bundles::{Branch, Bundle};
pub use shield::{is_shielded, ShieldEnd, ShieldInfo, Termination};
pub use crate::model::routing3d::ShieldEnds;
pub use crate::model::routing3d::{Class, FrozenLeg, FrozenRoute, InstallationZone, Laying, PathKind, Reservation, ReserveMode, RouteKind, RoutingPath, RoutingRules, ZoneKind};

// ---------------------------------------------------------------------- rules

impl Class {
    pub const ALL: [Class; 4] = [Class::Power, Class::Control, Class::Signal, Class::Fluid];

    pub fn title(self) -> &'static str {
        match self {
            Class::Power => tr("Power"),
            Class::Control => tr("Control"),
            Class::Signal => tr("Signal"),
            Class::Fluid => tr("Fluid"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Class::Power => "power",
            Class::Control => "control",
            Class::Signal => "signal",
            Class::Fluid => "fluid",
        }
    }

    pub fn from_key(key: &str) -> Option<Class> {
        Class::ALL.into_iter().find(|c| c.key() == key.trim().to_ascii_lowercase())
    }

    pub fn bit(self) -> u8 {
        1 << self as u8
    }

    /// The two classes must be kept apart: power from signals and from fluids.
    pub fn keeps_apart(self, other: Class) -> bool {
        matches!((self, other), (Class::Power, Class::Signal) | (Class::Signal, Class::Power) | (Class::Power, Class::Fluid) | (Class::Fluid, Class::Power))
    }

    /// Bits of the classes this one keeps apart from.
    pub fn apart_bits(self) -> u8 {
        Class::ALL.into_iter().filter(|c| self.keeps_apart(*c)).fold(0, |m, c| m | c.bit())
    }
}

impl RouteKind {
    pub fn title(self) -> &'static str {
        match self {
            RouteKind::Wire => tr("Wire"),
            RouteKind::Cable => tr("Cable"),
            RouteKind::Tube => tr("Tube"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            RouteKind::Wire => "wire",
            RouteKind::Cable => "cable",
            RouteKind::Tube => "tube",
        }
    }
}

impl PathKind {
    pub const ALL: [PathKind; 4] = [PathKind::Any, PathKind::Wires, PathKind::Cables, PathKind::Tubes];

    pub fn title(self) -> &'static str {
        match self {
            PathKind::Any => tr("Wires, cables and tubes"),
            PathKind::Wires => tr("Wires"),
            PathKind::Cables => tr("Cables"),
            PathKind::Tubes => tr("Tubes"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            PathKind::Any => "any",
            PathKind::Wires => "wires",
            PathKind::Cables => "cables",
            PathKind::Tubes => "tubes",
        }
    }

    pub fn from_key(key: &str) -> Option<PathKind> {
        PathKind::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    pub fn accepts(self, kind: RouteKind) -> bool {
        matches!((self, kind), (PathKind::Any, _) | (PathKind::Wires, RouteKind::Wire) | (PathKind::Cables, RouteKind::Cable) | (PathKind::Tubes, RouteKind::Tube))
    }
}

impl RoutingPath {
    /// Usable cross-section in mm² (without the reserve).
    pub fn capacity(&self) -> f32 {
        std::f32::consts::PI / 4.0 * self.width * self.width * (1.0 - self.reserve.clamp(0.0, 100.0) / 100.0)
    }

    pub fn length(&self) -> f32 {
        grid::length(&self.points)
    }
}

impl ReserveMode {
    pub fn title(self) -> String {
        match self {
            ReserveMode::Keepout => tr("Keep free").to_string(),
            ReserveMode::For(c) => tr_fmt!("Only {}", c.title()),
        }
    }
}

impl ShieldEnds {
    pub const ALL: [ShieldEnds; 4] = [ShieldEnds::Both, ShieldEnds::First, ShieldEnds::Second, ShieldEnds::None];

    pub fn is_both(&self) -> bool {
        *self == ShieldEnds::Both
    }

    pub fn title(self) -> &'static str {
        match self {
            ShieldEnds::Both => tr("Both ends"),
            ShieldEnds::First => tr("First end only"),
            ShieldEnds::Second => tr("Second end only"),
            ShieldEnds::None => tr("Not connected"),
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ShieldEnds::Both => "both",
            ShieldEnds::First => "first",
            ShieldEnds::Second => "second",
            ShieldEnds::None => "none",
        }
    }

    pub fn from_key(key: &str) -> Option<ShieldEnds> {
        ShieldEnds::ALL.into_iter().find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    /// The shield is connected at end 0 or 1.
    pub fn connects(self, end: usize) -> bool {
        matches!((self, end), (ShieldEnds::Both, _) | (ShieldEnds::First, 0) | (ShieldEnds::Second, 1))
    }
}

impl Default for RoutingRules {
    fn default() -> Self {
        RoutingRules {
            grid: 25.0,
            clearance: 5.0,
            free_factor: 2.0,
            segregation: 100.0,
            segregation_factor: 4.0,
            wires_3d: false,
            reuse_failed_searches: false,
            bend_wire: 4.0,
            bend_cable: 4.0,
            bend_tube: 5.0,
            cut_wire: 10.0,
            cut_cable: 100.0,
            cut_tube: 50.0,
            cable_reserve: 300.0,
            pigtail: 50.0,
            laying: Laying::Surface,
            slot_depth: 30.0,
            floor_depth: 60.0,
            zones: german_zones(),
        }
    }
}

/// The installation zones of DIN 18015-3:2016 for the walls (horizontal 15 - 45 cm above
/// the floor and below the ceiling, the middle one 100 - 130 cm above the floor for
/// rooms with worktops, switched off; vertical 10 - 30 cm beside doors, windows and
/// corners) and the ceiling (20 - 50 cm from the walls), and a zone in the floor like
/// the one of the ceiling (the standard has none in the floor).
pub fn german_zones() -> Vec<InstallationZone> {
    let z = |name: &str, kind: ZoneKind, from: f32, to: f32, on: bool| InstallationZone { name: name.into(), kind, from, to, on };
    vec![
        z("ZW-o", ZoneKind::BelowCeiling, 150.0, 450.0, true),
        z("ZW-m", ZoneKind::AboveFloor, 1000.0, 1300.0, false),
        z("ZW-u", ZoneKind::AboveFloor, 150.0, 450.0, true),
        z("ZS-t", ZoneKind::BesideDoors, 100.0, 300.0, true),
        z("ZS-f", ZoneKind::BesideWindows, 100.0, 300.0, true),
        z("ZS-e", ZoneKind::BesideCorners, 100.0, 300.0, true),
        z("ZD-r", ZoneKind::Ceiling, 200.0, 500.0, true),
        z("ZF", ZoneKind::Floor, 200.0, 500.0, true),
    ]
}

impl Laying {
    pub const ALL: [Laying; 3] = [Laying::Surface, Laying::Wall, Laying::Floor];

    pub fn key(&self) -> &'static str {
        match self {
            Laying::Surface => "surface",
            Laying::Wall => "wall",
            Laying::Floor => "floor",
        }
    }

    pub fn from_key(key: &str) -> Option<Laying> {
        Self::ALL.into_iter().find(|l| l.key().eq_ignore_ascii_case(key.trim()))
    }

    /// Untranslated title (`tr` where shown).
    pub fn title(&self) -> &'static str {
        match self {
            Laying::Surface => tr_const!("Surface-mounted"),
            Laying::Wall => tr_const!("Flush-mounted in the wall"),
            Laying::Floor => tr_const!("Flush-mounted in the floor"),
        }
    }
}

impl ZoneKind {
    pub const ALL: [ZoneKind; 7] = [ZoneKind::AboveFloor, ZoneKind::BelowCeiling, ZoneKind::BesideCorners, ZoneKind::BesideDoors, ZoneKind::BesideWindows, ZoneKind::Floor, ZoneKind::Ceiling];

    pub fn key(&self) -> &'static str {
        match self {
            ZoneKind::AboveFloor => "above_floor",
            ZoneKind::BelowCeiling => "below_ceiling",
            ZoneKind::BesideCorners => "beside_corners",
            ZoneKind::BesideDoors => "beside_doors",
            ZoneKind::BesideWindows => "beside_windows",
            ZoneKind::Floor => "floor",
            ZoneKind::Ceiling => "ceiling",
        }
    }

    pub fn from_key(key: &str) -> Option<ZoneKind> {
        Self::ALL.into_iter().find(|k| k.key().eq_ignore_ascii_case(key.trim()))
    }

    /// Untranslated title (`tr` where shown).
    pub fn title(&self) -> &'static str {
        match self {
            ZoneKind::AboveFloor => tr_const!("Horizontal, above the floor"),
            ZoneKind::BelowCeiling => tr_const!("Horizontal, below the ceiling"),
            ZoneKind::BesideCorners => tr_const!("Vertical, beside the corners"),
            ZoneKind::BesideDoors => tr_const!("Vertical, beside the doors"),
            ZoneKind::BesideWindows => tr_const!("Vertical, beside the windows"),
            ZoneKind::Floor => tr_const!("In the floor, along the walls"),
            ZoneKind::Ceiling => tr_const!("In the ceiling, along the walls"),
        }
    }
}

impl RoutingRules {
    pub fn is_default(&self) -> bool {
        *self == RoutingRules::default()
    }

    /// Smallest bend radius (mm) of a route of `kind` with the outer `diameter`.
    pub fn bend_radius(&self, kind: RouteKind, diameter: f32) -> f32 {
        self.bend(kind) * diameter
    }

    fn bend(&self, kind: RouteKind) -> f32 {
        match kind {
            RouteKind::Wire => self.bend_wire,
            RouteKind::Cable => self.bend_cable,
            RouteKind::Tube => self.bend_tube,
        }
    }

    fn cut(&self, kind: RouteKind) -> f32 {
        match kind {
            RouteKind::Wire => self.cut_wire,
            RouteKind::Cable => self.cut_cable,
            RouteKind::Tube => self.cut_tube,
        }
    }
}

// ---------------------------------------------------------------------- results

/// An end of a route.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct End {
    /// Device tag, or the cable gland / "field".
    pub tag: String,
    /// Enclosure (index) the end is in; `None` in the field.
    pub enclosure: Option<usize>,
    /// The end point in layout coordinates.
    pub point: Option<V3>,
    /// Item the route ends at (device, cable gland).
    pub item: Option<u64>,
    /// The parts of the item the route (its cores) ends at, where the item has parts a
    /// route may not pass through (the terminals of a strip across the bars,
    /// [`part_id`]); empty: the whole item.
    pub parts: Vec<u64>,
    /// The connection points the cores land on where they part at the end (layout
    /// coordinates; `point` is their middle); empty for one.
    pub cores: Vec<V3>,
    /// The direction each of the cores leaves its connection point in.
    pub exits: Vec<V3>,
    /// The designation of each of those connection points (`-X1:3`).
    pub pins: Vec<String>,
    /// The end of the nozzle of the device a tube ends at (layout coordinates): the
    /// route comes to `point` straight out of it and goes on into it.
    pub nozzle: Option<V3>,
    /// The direction the route leaves `point` in, where known (a unit vector): out of
    /// the nozzle, away from the wall of a cable gland, the way the cores leave their
    /// connection points or away from the face a device is mounted on.
    pub leave: Option<V3>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Routed,
    /// Kept from the frozen route.
    Frozen,
    /// The frozen route was no longer possible and was routed again.
    Rerouted,
    /// Not routed, with the reason.
    Unrouted(String),
}

/// A routed wire, cable or tube.
#[derive(Clone, Debug, PartialEq)]
pub struct RoutedItem {
    pub kind: RouteKind,
    /// Stable key across recalculations: wires `from|to`, cables their tag, tubes
    /// `hose:from|to`.
    pub key: String,
    pub label: String,
    pub class: Class,
    pub ends: [End; 2],
    /// Enclosures the route runs through, in order.
    pub enclosures: Vec<usize>,
    pub points: Vec<V3>,
    /// Bend radius of each corner of `points` (0 at the ends of the legs): the bend
    /// radius, smaller where the bend would not keep clear of what is around it.
    pub radii: Vec<f32>,
    /// A route through a room into its cabinets runs in legs (one per line-up or
    /// room, `points` is their concatenation); empty for routes in one line-up.
    pub legs: Vec<RouteLeg>,
    /// Length in the panel layout including the reserves, mm.
    pub length: f32,
    /// Cut length (rounded up), mm.
    pub cut: f32,
    /// Outer diameter, mm.
    pub diameter: f32,
    /// The least bend radius of the route (from its thickness and the rules), mm.
    pub bend_radius: f32,
    /// Length outside ducts and routing paths, mm.
    pub free: f32,
    pub bends: usize,
    pub shielded: bool,
    pub status: Status,
    /// Index into the routed wires (wires only).
    pub wire: Option<usize>,
    /// How it is laid in rooms (its stretches through rooms run in the walls or the
    /// floor where flush-mounted).
    pub laying: Laying,
}

/// A stretch of a route in one line-up or room.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteLeg {
    /// Enclosure (index) the stretch starts in: a cabinet of the line-up, or the room.
    pub enclosure: usize,
    pub points: Vec<V3>,
    /// Bend radius of each corner (see [`RoutedItem::radii`]).
    pub radii: Vec<f32>,
}

impl RoutedItem {
    pub fn routed(&self) -> bool {
        !matches!(self.status, Status::Unrouted(_))
    }

    /// The stretches of the route with the enclosure each starts in: its legs, or all
    /// of its points (`None`). No segment joins two legs: their points are relative to
    /// different places of the layout.
    pub fn stretches(&self) -> Vec<(Option<usize>, &[V3])> {
        if self.legs.is_empty() {
            vec![(None, self.points.as_slice())]
        } else {
            self.legs.iter().map(|l| (Some(l.enclosure), l.points.as_slice())).collect()
        }
    }

    /// The bend radii of the corners of each of [`RoutedItem::stretches`].
    pub fn stretch_radii(&self) -> Vec<&[f32]> {
        if self.legs.is_empty() {
            vec![self.radii.as_slice()]
        } else {
            self.legs.iter().map(|l| l.radii.as_slice()).collect()
        }
    }

    /// The stretches of the route in the line-up of enclosure `ei` (what its plate view
    /// shows): legs through rooms and other line-ups are left out.
    pub fn stretches_in_lineup(&self, layout: &Layout, ei: usize) -> Vec<&[V3]> {
        let lineup = panel::lineup_of(layout, ei);
        self.stretches().into_iter().filter(|(leg, _)| leg.is_none_or(|k| lineup.contains(&k))).map(|(_, points)| points).collect()
    }

    pub fn status_text(&self) -> String {
        match &self.status {
            Status::Routed if self.kind == RouteKind::Wire && self.free == 0.0 => tr("in the cable ducts").to_string(),
            Status::Routed => tr("routed").to_string(),
            Status::Frozen => tr("frozen").to_string(),
            Status::Rerouted => tr("rerouted (frozen route no longer possible)").to_string(),
            Status::Unrouted(reason) => tr_fmt!("not routed: {}", reason),
        }
    }
}

/// Use of a routing path.
#[derive(Clone, Debug, PartialEq)]
pub struct PathUse {
    pub enclosure: usize,
    pub path: u64,
    pub items: usize,
    /// Cross-section of the contents, mm².
    pub area: f32,
    /// Usable cross-section, mm².
    pub capacity: f32,
}

/// Length of a frozen route against its current length.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub key: String,
    pub label: String,
    pub before: f32,
    /// `None`: the item no longer exists.
    pub after: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub level: Level,
    pub message: String,
    /// Page and symbol of what the problem is about (a connection point of the cable,
    /// tube or wire), where known: the messages pane jumps there.
    pub target: Option<(usize, u64)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Routing {
    pub items: Vec<RoutedItem>,
    pub bundles: Vec<Bundle>,
    pub branches: Vec<Branch>,
    pub shields: Vec<ShieldInfo>,
    pub paths: Vec<PathUse>,
    pub changes: Vec<Change>,
    pub problems: Vec<Problem>,
}

impl Routing {
    pub fn item(&self, key: &str) -> Option<&RoutedItem> {
        self.items.iter().find(|i| i.key == key)
    }

    /// Bundles an item runs in.
    pub fn bundles_of(&self, item: usize) -> Vec<usize> {
        (0..self.bundles.len()).filter(|&b| self.bundles[b].items.contains(&item)).collect()
    }

    /// What of this routing, of an earlier state of a project with `enclosures`
    /// enclosures, is shown until the routing of the current state is ready: the cables
    /// and tubes in enclosures that still exist. Wires (their indices may have changed),
    /// bundles and branches are left out.
    pub fn interim(&self, enclosures: usize) -> Routing {
        let fits = |k: usize| k < enclosures;
        let kept = |i: &&RoutedItem| {
            i.kind != RouteKind::Wire && i.enclosures.iter().all(|&k| fits(k)) && i.legs.iter().all(|l| fits(l.enclosure)) && i.ends.iter().all(|e| e.enclosure.is_none_or(fits))
        };
        Routing {
            items: self.items.iter().filter(kept).cloned().collect(),
            paths: self.paths.iter().filter(|p| fits(p.enclosure)).cloned().collect(),
            shields: self.shields.clone(),
            changes: self.changes.clone(),
            problems: self.problems.clone(),
            ..Routing::default()
        }
    }
}

// ---------------------------------------------------------------------- analysis

/// Key of a routed wire.
pub fn wire_key(w: &RoutedWire) -> String {
    format!("{}|{}", w.from, w.to)
}

/// The wire is routed in 3D (instead of along the cable ducts of the mounting plate):
/// every wire with [`RoutingRules::wires_3d`], the wires of a room and of a cabinet
/// without cable ducts always ([`between_points`]), never a wire to the door.
pub fn wire_in_3d(layout: &Layout, w: &RoutedWire) -> bool {
    (layout.routing.wires_3d || between_points(layout, w)) && w.door == (None, None)
}

/// The wire runs from connection point to connection point through the space (not
/// down to the mounting plate and along it): in a room (it has no mounting plate, the
/// wires run between the devices on its walls like cables), and in a cabinet without
/// cable ducts on its mounting plate (at either end of the wire): without ducts to lay
/// them in, wires laid along the plate would run through the devices and each other.
pub fn between_points(layout: &Layout, w: &RoutedWire) -> bool {
    let open = |k: usize| layout.enclosures.get(k).is_some_and(|e| e.is_room() || !e.items.iter().any(|i| i.kind == ItemKind::Duct && i.face == Face::Plate));
    open(w.enclosure) || open(w.last_enclosure())
}

/// Class of a connection from its potential (kind and voltage).
pub fn class_of_net(prj: &Project, net: &str) -> Class {
    use crate::potentials::PotentialKind;
    let Some(p) = prj.potential(net) else { return Class::Control };
    let volts: f32 = p.voltage.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<String>().parse().unwrap_or(0.0);
    let ac = p.voltage.to_ascii_uppercase().contains("AC");
    match p.kind {
        PotentialKind::L | PotentialKind::N | PotentialKind::Pe => Class::Power,
        PotentialKind::Signal => Class::Signal,
        _ if (ac && volts > 50.0) || volts > 120.0 => Class::Power,
        _ => Class::Control,
    }
}

/// Class of a cable: signal for shielded and bus cables, otherwise the class of its
/// cores (the highest).
pub fn class_of_cable(prj: &Project, c: &Cable, conns: &[&Connection]) -> Class {
    if is_shielded(c) {
        return Class::Signal;
    }
    conns.iter().map(|x| class_of_net(prj, &x.net)).min().unwrap_or(Class::Control)
}

/// Outer diameter of a cable: as set, otherwise from its cores and cross-section.
pub fn cable_diameter(c: &Cable) -> f32 {
    if c.diameter > 0.0 {
        return c.diameter;
    }
    let n = c.core_list().len().max(2) as f32;
    let core = crate::harness::outer_diameter(&c.cross_section);
    // Cores laid up round, plus the sheath (and the shield).
    core * (1.0 + 0.5 * n).sqrt() * 1.2 + 2.0 + if is_shielded(c) { 1.0 } else { 0.0 }
}

/// Outer diameter of a tube from its nominal size ("8 mm", "DN25").
pub fn tube_diameter(size: &str) -> f32 {
    let digits: String = size.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    digits.replace(',', ".").parse::<f32>().ok().filter(|v| *v > 0.0).unwrap_or(8.0)
}

fn part_subgroup<'a>(prj: &'a Project, number: &str) -> Option<&'a str> {
    prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == number).map(|p| p.subgroup.as_str())
}

/// Items of a subgroup (accessories) in an enclosure.
pub fn accessories<'a>(prj: &'a Project, ei: usize, subgroup: &str) -> Vec<&'a PanelItem> {
    prj.layout.enclosures[ei].items.iter().filter(|i| i.kind == ItemKind::Accessory && part_subgroup(prj, &i.part) == Some(subgroup)).collect()
}

/// The ways for cables into an enclosure: cable glands and cable entry flanges (a
/// membrane taking several large cables).
pub fn entries(prj: &Project, ei: usize) -> Vec<&PanelItem> {
    let mut v = accessories(prj, ei, "Cable gland");
    v.extend(accessories(prj, ei, "Cable entry"));
    v
}

/// The cable glands the cables routed so far go through (by item id), and where they
/// go through them; the places the cables and tubes took in the wall penetrations they
/// pass (by item id: how far beside and below the middle of the opening, and how thick).
#[derive(Default)]
struct Glands {
    taken: HashMap<u64, usize>,
    through: HashMap<u64, Vec<V3>>,
    passed: HashMap<u64, Vec<([f32; 2], f32)>>,
}

impl Glands {
    /// Where a cable of `diameter` goes through the entry `g` (`at`: a point of its
    /// face by the offset from its middle): a cable gland at its middle; a cable entry
    /// flange at a place of its own along its membrane (part data `Cutout`, the places
    /// the cable's thickness and a centimetre apart), the free one nearest to where the
    /// cable heads (`near`), the middle where none is free.
    fn slot(&mut self, prj: &Project, g: &PanelItem, diameter: f32, at: impl Fn([f32; 2]) -> V3, near: V3) -> V3 {
        let middle = at([0.0; 2]);
        let cutout = (part_subgroup(prj, &g.part) == Some("Cable entry")).then(|| panel::part_by_number(prj, &g.part).and_then(|p| p.tech_value("Cutout")).map(panel::busbars::numbers)).flatten().filter(|c| c.len() >= 2);
        let through = self.through.entry(g.id).or_default();
        let q = match cutout {
            Some(c) => {
                let (long, axis) = if c[0] >= c[1] { (c[0], 0) } else { (c[1], 1) };
                let pitch = diameter + 10.0;
                let n = ((long - diameter) / 2.0 / pitch).floor().max(0.0) as i32;
                let free = |q: &V3| through.iter().all(|t| dist(*t, *q) >= pitch - 1.0);
                (-n..=n)
                    .map(|k| {
                        let mut off = [0.0; 2];
                        off[axis] = k as f32 * pitch;
                        at(off)
                    })
                    .filter(free)
                    .min_by(|a, b| manhattan(*a, near).total_cmp(&manhattan(*b, near)))
                    .unwrap_or(middle)
            }
            None => middle,
        };
        through.push(q);
        q
    }

    /// The way through a wall penetration for a cable or tube of `diameter` that comes
    /// from `from` in the room before it and goes on to `to` in the room behind (room
    /// coordinates of each): at a place of its own in the opening, its thickness and a
    /// centimetre from the ones that went through before it, the free place nearest to
    /// the middle (in the middle row first, on the side its nearer end lies on: there
    /// it has the least room to pass the ones beside it before the wall); in the middle
    /// where none is free. Other ways between rooms as they are.
    fn through_wall(&mut self, prj: &Project, link: panel::WallLink, diameter: f32, from: V3, to: V3) -> panel::WallLink {
        if link.via != panel::Via::Wall {
            return link;
        }
        // The opening is an item of one of the two rooms: places count to the right
        // seen from inside that one.
        let item = |ri: usize| prj.layout.enclosures.get(ri).and_then(|e| e.item(link.item));
        let (it, side) = match (item(link.from), item(link.to)) {
            (Some(it), _) => (it, 1.0),
            (None, Some(it)) => (it, -1.0),
            (None, None) => return link,
        };
        // Along the wall, across the way through it: the same way seen from both rooms.
        let across = |leave: V3, by: f32| [-leave[2] * by, 0.0, leave[0] * by];
        let (ua, ub) = (across(link.leave[0], 1.0), across(link.leave[1], -1.0));
        let comes = if dist(from, link.a) <= dist(to, link.b) { dot(sub(from, link.a), ua) } else { dot(sub(to, link.b), ub) } * side;
        let pitch = diameter + 10.0;
        let most = |size: f32| ((size - diameter) / 2.0 / pitch).floor().max(0.0) as i32;
        let (n, m) = (most(it.w), most(it.h));
        let taken = self.passed.entry(link.item).or_default();
        let free = |off: [f32; 2]| taken.iter().all(|(t, d)| (t[0] - off[0]).hypot(t[1] - off[1]) >= (diameter + d) / 2.0 + 9.0);
        let off = (-m..=m)
            .flat_map(|j| (-n..=n).map(move |k| (j, k)))
            .filter(|&(j, k)| free([k as f32 * pitch, j as f32 * pitch]))
            .min_by_key(|&(j, k)| (j.abs(), k.abs(), k as f32 * comes < 0.0, j, k))
            .map_or([0.0; 2], |(j, k)| [k as f32 * pitch, j as f32 * pitch]);
        taken.push((off, diameter));
        let down = [0.0, off[1], 0.0];
        panel::WallLink { a: add(add(link.a, scale(ua, off[0] * side)), down), b: add(add(link.b, scale(ub, off[0] * side)), down), ..link }
    }

    /// The entry for a cable of `diameter` among the `candidates` (each with how far it
    /// is from where the cable heads): a cable gland takes one cable, a cable entry
    /// flange as many as have places along its membrane ([`Glands::slot`]). Of the free
    /// ones the nearest whose clamping range the diameter fits, otherwise the nearest
    /// free one; [`Entry::Taken`] where another cable took every one of them.
    fn pick<'a>(&mut self, prj: &Project, candidates: Vec<(&'a PanelItem, f32)>, diameter: f32) -> Entry<'a> {
        if candidates.is_empty() {
            return Entry::None;
        }
        let free = |g: &PanelItem| part_subgroup(prj, &g.part) == Some("Cable entry") || !self.taken.contains_key(&g.id);
        let fits = |g: &PanelItem| panel::clamping_range(prj, &g.part).is_none_or(|(min, max)| diameter >= min && diameter <= max);
        let key = |g: &PanelItem, d: f32| (!fits(g), d);
        let Some(best) = candidates.into_iter().filter(|(g, _)| free(g)).min_by(|a, b| key(a.0, a.1).partial_cmp(&key(b.0, b.1)).unwrap_or(std::cmp::Ordering::Equal).then(a.0.id.cmp(&b.0.id))) else {
            return Entry::Taken;
        };
        *self.taken.entry(best.0.id).or_default() += 1;
        Entry::Through(best.0)
    }
}

/// Where a cable goes into an enclosure ([`Glands::pick`]).
enum Entry<'a> {
    /// Through this cable gland or entry flange.
    Through(&'a PanelItem),
    /// The enclosure has no cable entry.
    None,
    /// Another cable took every cable gland of the enclosure.
    Taken,
}

/// Where a device is reached (layout coordinates): the middle of the connection points
/// the schematic connection points `pins` (the cores of a cable) land on, the cores
/// parting there to each of them, otherwise the middle of its front; devices of a placed
/// valve terminal at the terminal ([`panel::locate`]).
fn device_end(prj: &Project, pos: &[V3], tag: &str, pins: &[PinRef]) -> End {
    let placement=if let Some(pin)=pins.first() {panel::locate_symbol(prj,&prj.pages[pin.page].symbols[pin.sym])} else {panel::locate(prj,tag)};
    match placement {
        Some((ei, it)) => {
            let e = &prj.layout.enclosures[ei];
            if let Some((q, into)) = e.is_room().then(|| panel::through_wall(e, it)).flatten() {
                // On the outside of a room: the way runs through the wall behind it.
                return End { tag: tag.into(), enclosure: Some(ei), point: Some(add(q, pos[ei])), item: Some(it.id), leave: Some(into), ..Default::default() };
            }
            let mut at: Vec<(V3, V3)> = Vec::new();
            let mut designations: Vec<String> = Vec::new();
            let mut owners=std::collections::BTreeSet::new();
            for pin in pins {
                let symbol=&prj.pages[pin.page].symbols[pin.sym];
                let Some((pi,item))=panel::locate_symbol(prj,symbol) else {return End {tag:tag.into(),..Default::default()};};
                if pi!=ei || item.face!=it.face {return End {tag:tag.into(),..Default::default()};}
                owners.insert(item.id);
                let placed=panel::device_points(prj,item);
                if let Some(p)=panel::point_of(prj,&placed,symbol,pin.pin) {
                    if !at.iter().any(|a|a.0==p.pos) {at.push((p.pos,panel::exit_vector(p.exit)));designations.push(crate::netlist::pin_designation(prj,*pin));}
                }
            }
            let q = match at.len() {
                0 => [it.x + it.w / 2.0, it.y + it.h / 2.0, it.z + it.d],
                n => at.iter().fold([0.0; 3], |s, q| add(s, q.0)).map(|v| v / n as f32),
            };
            let in_layout = |q: V3| add(panel::face_point(e, it.face, q), pos[ei]);
            let direction = |x: V3| sub(in_layout(add(q, x)), in_layout(q));
            // The route leaves the way the cores leave (where they agree), otherwise
            // away from the face. Cores that come to packed terminals from the front
            // (see `panel::points::TIGHT_TERMINALS`) part from a cable that comes
            // along the strip like the other cores.
            let along = at.iter().filter(|p| p.1[2].abs() < 0.5).fold([0.0; 3], |s, p| add(s, p.1));
            let out = if at.len() > 1 && dist(along, [0.0; 3]) >= 0.5 { along } else { at.iter().fold([0.0; 3], |s, p| add(s, p.1)) };
            let leave = if at.is_empty() || dist(out, [0.0; 3]) < 0.5 { direction([0.0, 0.0, 1.0]) } else { direction(out) };
            // The terminals of a strip across the bars the points lie on: the route may
            // pass through those, not through the strip's other terminals.
            let mut parts: Vec<u64> = Vec::new();
            if owners.len()>1 {parts.extend(owners);}
            if let Some(places) = panel::busbars::terminals_on_bars(prj, it) {
                let inside = |sub: &PanelItem, p: V3| p[0] >= sub.x - 0.01 && p[0] <= sub.x + sub.w + 0.01 && p[1] >= sub.y - 0.01 && p[1] <= sub.y + sub.h + 0.01;
                for (k, (sub, ..)) in places.iter().enumerate() {
                    if at.iter().any(|(p, _)| inside(sub, *p)) || (at.is_empty() && inside(sub, q)) {
                        parts.push(part_id(it.id, k));
                    }
                }
            }
            let (cores, exits): (Vec<V3>, Vec<V3>) = if at.len() > 1 { at.into_iter().map(|(q, x)| (in_layout(q), sub(in_layout(add(q, x)), in_layout(q)))).unzip() } else { Default::default() };
            let pins = if cores.is_empty() { Vec::new() } else { designations };
            End { tag: tag.into(), enclosure: Some(ei), point: Some(in_layout(q)), item: Some(it.id), parts, cores, exits, pins, nozzle: None, leave: Some(norm(leave)) }
        }
        None => End { tag: tag.into(), ..Default::default() },
    }
}

/// The id of part `k` of the item `item` in the routing space (a terminal of a strip
/// across the bars): an id of its own, above the ids of the items.
pub fn part_id(item: u64, k: usize) -> u64 {
    item | (k as u64 + 1) << 48
}

/// The nozzles of the device at the end `end` of a tube (layout coordinates: their ends
/// and the directions out of them) and which of them is for its connection point `pin`.
fn end_nozzles(prj: &Project, pos: &[V3], end: &End, pin: PinRef) -> Option<(Vec<(V3, V3)>, usize)> {
    let s = &prj.pages[pin.page].symbols[pin.sym];
    let (ei, it) = panel::locate_symbol(prj, s).filter(|(_, it)| end.item == Some(it.id))?;
    let def = prj.symbol(&s.sym)?;
    let name = s.pins.get(pin.pin).map(String::as_str).or_else(|| def.pins.get(pin.pin).map(|d| d.label))?;
    let all = panel::nozzles(prj, it);
    let k = all.iter().position(|n| n.0 == name)?;
    let e = &prj.layout.enclosures[ei];
    let in_layout = |q: V3| add(panel::face_point(e, it.face, q), pos[ei]);
    let at = all.iter().map(|(_, at, dir)| (in_layout(*at), sub(in_layout(add(*at, *dir)), in_layout(*at)))).collect();
    Some((at, k))
}

/// The two nozzles at the ends of a fitting in a pipe (valve, meter): one opposite the other.
fn in_line(nozzles: &[(V3, V3)]) -> bool {
    nozzles.len() == 2 && crate::parts::vec3::dot(nozzles[0].1, nozzles[1].1) < -0.99
}

/// The fittings in a pipe (see [`in_line`]) that are mounted the other way round, each
/// pipe at the end it comes from (as near as possible to where its other end is): the
/// items at the ends of `hoses` that turned are that much nearer.
fn turned_fittings(prj: &Project, pos: &[V3], hoses: &[(crate::fluid::Hose, [End; 2])]) -> Vec<u64> {
    let mut ways: HashMap<u64, [f32; 2]> = HashMap::new();
    for (h, ends) in hoses {
        for (i, pin) in [h.a, h.b].into_iter().enumerate() {
            let (Some(item), Some(to)) = (ends[i].item, ends[1 - i].point) else { continue };
            let Some((at, k)) = end_nozzles(prj, pos, &ends[i], pin).filter(|(at, _)| in_line(at)) else { continue };
            let way = ways.entry(item).or_default();
            way[0] += dist(at[k].0, to);
            way[1] += dist(at[1 - k].0, to);
        }
    }
    let mut turned: Vec<u64> = ways.into_iter().filter(|(_, w)| w[1] + 1.0 < w[0]).map(|(id, _)| id).collect();
    turned.sort_unstable();
    turned
}

/// A tube at the end `end` at the connection point `pin` of its device ends at the nozzle
/// of the device's model for it (the other end of a fitting `turned` round), the route
/// coming to it from `out` mm straight out of it.
fn at_nozzle(prj: &Project, pos: &[V3], end: &mut End, pin: PinRef, out: f32, turned: &[u64]) {
    let Some((at, k)) = end_nozzles(prj, pos, end, pin) else { return };
    let k = if in_line(&at) && end.item.is_some_and(|id| turned.contains(&id)) { 1 - k } else { k };
    let (tip, dir) = at[k];
    end.nozzle = Some(tip);
    end.point = Some(add(tip, scale(dir, out)));
    end.leave = Some(norm(dir));
}

/// The schematic connection points of `cores` at the device `tag`.
fn pins_at(prj: &Project, cores: &[&Connection], tag: &str) -> Vec<PinRef> {
    let at = |p: &PinRef| prj.pages[p.page].symbols[p.sym].tag == tag;
    cores.iter().flat_map(|c| [c.a, c.b]).filter(at).collect()
}

/// Where a cable or tube from the field enters the enclosure: a cable gland
/// ([`Glands::pick`], through it where [`Glands::slot`] says), otherwise the middle of
/// the bottom.
fn entry_end(prj: &Project, pos: &[V3], ei: usize, near: V3, glands: &mut Glands, diameter: f32) -> End {
    let e = &prj.layout.enclosures[ei];
    let at = |it: &PanelItem, off: [f32; 2], depth: f32| add(panel::face_point(e, it.face, [it.x + it.w / 2.0 + off[0], it.y + it.h / 2.0 + off[1], depth]), pos[ei]);
    let candidates: Vec<(&PanelItem, f32)> = entries(prj, ei).into_iter().map(|g| (g, manhattan(at(g, [0.0; 2], 5.0), near))).collect();
    match glands.pick(prj, candidates, diameter) {
        Entry::Through(g) => {
            let point = glands.slot(prj, g, diameter, |off| at(g, off, 5.0), near);
            End { tag: g.part.clone(), enclosure: Some(ei), point: Some(point), item: Some(g.id), leave: Some(norm(sub(at(g, [0.0; 2], 6.0), at(g, [0.0; 2], 5.0)))), ..Default::default() }
        }
        // Through the middle of the bottom: where every gland is taken, an end that
        // the routing reports ([`gland_taken`]).
        Entry::None | Entry::Taken => {
            let q = [e.width / 2.0, e.height - e.wall() - 5.0, (PLATE_Z + e.depth) / 2.0];
            End { tag: tr("Bottom").to_string(), enclosure: Some(ei), point: Some(add(q, pos[ei])), item: None, leave: Some([0.0, -1.0, 0.0]), ..Default::default() }
        }
    }
}

/// What is to be routed.
#[derive(Clone, PartialEq)]
struct Job {
    kind: RouteKind,
    key: String,
    label: String,
    class: Class,
    ends: [End; 2],
    diameter: f32,
    shielded: bool,
    wire: Option<usize>,
    /// Route already known (wires through the cable ducts).
    fixed: Option<Vec<V3>>,
    /// Length to add (wires: from the connection points to the plate).
    extra: f32,
    /// Smallest bend radius in mm; 0: from the rules.
    bend: f32,
    /// Length set in the schematic (hoses), mm; 0: not set.
    set_length: f32,
    /// Page and symbol of a connection point of it, for the messages about it.
    target: Option<(usize, u64)>,
    /// How it is laid in rooms.
    laying: Laying,
}

impl Job {
    /// Cross-section (mm²) and smallest bend radius (mm).
    fn area_radius(&self, rules: &RoutingRules) -> (f32, f32) {
        let area = std::f32::consts::PI / 4.0 * self.diameter * self.diameter;
        (area, if self.bend > 0.0 { self.bend } else { rules.bend(self.kind) * self.diameter })
    }

    /// What is routed along a leg through `space` (passing the items `ignore`).
    fn request<'a>(&self, leg: &room::LegPlan, space: &grid::Space, ignore: &'a [u64], rules: &RoutingRules) -> grid::Request<'a> {
        let (area, radius) = self.area_radius(rules);
        let out = |i: usize| self.ends[i].nozzle.zip(self.ends[i].point).map_or(0.0, |(tip, q)| dist(tip, q));
        grid::Request { kind: self.kind, class: self.class, ignore, at: [leg.a, leg.b], leave: leg.leave, out: [out(0), out(1)], area, turn: (space.cell * 0.5).max(radius), half: self.diameter / 2.0 }
    }

    /// The items a leg may pass through: the devices at the ends, its cable glands.
    fn ignore(&self, leg: &room::LegPlan) -> Vec<u64> {
        let own = |e: &End| if e.parts.is_empty() { e.item.into_iter().collect() } else { e.parts.clone() };
        self.ends.iter().flat_map(own).chain(leg.ignore.iter().copied()).collect()
    }
}

/// The legs of a route, whether it enters a cabinet through its roof, the length through
/// the walls.
type LegsPlan = (Vec<room::LegPlan>, bool, f32);

/// Threads the searches of one class are spread over: each keeps arrays as large as the
/// largest space it searches.
const ROUTING_THREADS: usize = 4;
/// How far at least the route of a tube comes to the nozzle of its device straight out
/// of it, mm.
const NOZZLE_OUT: f32 = 30.0;
/// Bend radius of a pipe in diameters: a long-radius elbow.
const PIPE_ELBOW: f32 = 1.5;

/// The two end tags of a cable: the device most of its cores start at, and the one
/// most of them end at.
fn cable_ends(prj: &Project, conns: &[&Connection]) -> Option<(String, String)> {
    let tag = |p: crate::netlist::PinRef| prj.pages[p.page].symbols[p.sym].tag.clone();
    let most = |tags: Vec<String>| {
        let mut count: BTreeMap<String, usize> = BTreeMap::new();
        for t in tags {
            *count.entry(t).or_default() += 1;
        }
        count.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|x| x.0)
    };
    let a = most(conns.iter().map(|c| tag(c.a)).collect())?;
    let b = most(conns.iter().map(|c| tag(c.b)).filter(|t| *t != a).collect())?;
    Some((a, b))
}

/// The two end tags of a cable: from its cores, bus cables between the stations of
/// their bus connection.
pub fn cable_end_tags(prj: &Project, c: &Cable, cores: &[&Connection]) -> Option<(String, String)> {
    let bus = || {
        let link = crate::plc_hw::cable_links(prj, &c.tag).into_iter().next()?;
        let tag = |s: &str| prj.plc.station(s).map(|s| s.tag.clone()).filter(|t| !t.is_empty());
        Some((tag(&link.a.station)?, tag(&link.b.station)?))
    };
    cable_ends(prj, cores).or_else(bus)
}

/// Where a cable or tube from outside the building comes into room `ri`: the building
/// entry nearest to `near` (layout coordinates); `None` for a room without one.
fn building_entry(prj: &Project, pos: &[V3], ri: usize, near: V3) -> Option<End> {
    let e = &prj.layout.enclosures[ri];
    let at = |it: &PanelItem| panel::entry_point(e, it).map(|q| add(q, pos[ri]));
    let it = panel::entries(e).into_iter().filter(|it| at(it).is_some()).min_by(|a, b| manhattan(at(a).unwrap_or(near), near).total_cmp(&manhattan(at(b).unwrap_or(near), near)).then(a.id.cmp(&b.id)))?;
    Some(End { tag: tr("Building entry").to_string(), enclosure: Some(ri), point: at(it), item: Some(it.id), leave: panel::entry_direction(e, it), ..Default::default() })
}

/// A point `p` of enclosure `k` (layout coordinates) where it is in room `ri` (layout
/// coordinates of the room), by where the enclosure stands in the room.
fn room_point(prj: &Project, pos: &[V3], k: usize, ri: usize, p: V3) -> V3 {
    let poses = panel::site_poses(prj);
    let (e, r) = (&prj.layout.enclosures[k], &prj.layout.enclosures[ri]);
    // Layout coordinates have y down from the top, the site y up from the floor.
    let q = sub(p, pos[k]);
    let w = poses[ri].local(poses[k].apply([q[0], e.height - q[1], q[2]]));
    add([w[0], r.height - w[1], w[2]], pos[ri])
}

/// The location of a device tag (`=F1+LOC-K1` → `LOC`), empty without one.
fn location_of(tag: &str) -> &str {
    let structure = tag.split('-').next().unwrap_or_default();
    structure.rsplit_once('+').map_or("", |(_, loc)| loc)
}

/// A device outside the building: its location is none of the rooms and enclosures
/// of the layout.
fn outside(prj: &Project, tag: &str) -> bool {
    let loc = location_of(tag);
    !loc.is_empty() && !prj.layout.enclosures.iter().any(|e| e.name.trim_start_matches('+') == loc)
}

/// The ends of a cable or tube between two devices: placed devices, or the cable
/// entry of the enclosure the other end is in (the building entry of a room for a
/// device outside the building, also when that device's enclosure stands in the room:
/// the route then comes in through the building entry and on into the enclosure).
/// `None` if neither end is placed, or if the placed one is in a room and the other
/// one is not outside or the room has no building entry.
/// `pins`: the schematic connection points of the cores at `a` and `b`.
fn ends_between(prj: &Project, pos: &[V3], a: &str, b: &str, pins: [&[PinRef]; 2], glands: &mut Glands, diameter: f32) -> Option<[End; 2]> {
    let (ea, eb) = (device_end(prj, pos, a, pins[0]), device_end(prj, pos, b, pins[1]));
    let in_room = |e: &End| e.enclosure.is_some_and(|k| prj.layout.enclosures[k].is_room());
    let entry = |e: &End, p: V3, other: &str| -> Option<End> {
        let k = e.enclosure?;
        let ri = panel::room_of(&prj.layout, k).filter(|_| outside(prj, other))?;
        building_entry(prj, pos, ri, room_point(prj, pos, k, ri, p))
    };
    match (ea.point, eb.point) {
        (None, None) => None,
        (Some(p), None) if in_room(&ea) => outside(prj, b).then(|| building_entry(prj, pos, ea.enclosure?, p).map(|e| [ea.clone(), End { tag: b.into(), ..e }])).flatten(),
        (None, Some(p)) if in_room(&eb) => outside(prj, a).then(|| building_entry(prj, pos, eb.enclosure?, p).map(|e| [End { tag: a.into(), ..e }, eb.clone()])).flatten(),
        (Some(p), None) if entry(&ea, p, b).is_some() => entry(&ea, p, b).map(|e| [ea.clone(), End { tag: b.into(), ..e }]),
        (None, Some(p)) if entry(&eb, p, a).is_some() => entry(&eb, p, a).map(|e| [End { tag: a.into(), ..e }, eb.clone()]),
        // Both in one device group: part of its base.
        _ if ea.item.is_some() && ea.item == eb.item => None,
        (Some(p), None) => Some([ea.clone(), End { tag: b.into(), ..entry_end(prj, pos, ea.enclosure?, p, glands, diameter) }]),
        (None, Some(p)) => Some([End { tag: a.into(), ..entry_end(prj, pos, eb.enclosure?, p, glands, diameter) }, eb]),
        _ => Some([ea, eb]),
    }
}

/// The 3D route of a wire routed through the cable ducts: its stretches on the
/// plates at duct depth, joined through the cable passages.
fn wire_points(pos: &[V3], w: &RoutedWire) -> Vec<V3> {
    let mut v = Vec::new();
    for (ei, route, entry, exit) in w.stretch_passages() {
        let z = if route.via_ducts { PLATE_Z + DUCT_PROFILE.1 / 2.0 } else { PLATE_Z + 5.0 };
        v.extend(entry);
        v.extend(route.points.iter().map(|q| add([PLATE_MARGIN + q.x, PLATE_MARGIN + q.y, z], pos[ei])));
        v.extend(exit);
    }
    grid::simplify(v)
}

/// Routes everything of the panel layout in 3D: cables (as a whole, field cables to
/// their cable gland), tubes, and the wires (`wiring`; with
/// [`RoutingRules::wires_3d`] their routes are replaced by the 3D routes). Classes
/// are routed one after the other (power first), so that later ones keep their
/// distance; frozen routes are kept while they are still possible.
pub fn analyse(prj: &Project, wiring: &mut [RoutedWire], conns: &[Connection], open: &Netlist) -> Routing {
    analyse_with(prj, wiring, conns, open, &|_, _| {})
}

/// Like [`analyse`], telling `progress` how many of all routes are done before each.
pub fn analyse_with(prj: &Project, wiring: &mut [RoutedWire], conns: &[Connection], open: &Netlist, progress: &dyn Fn(usize, usize)) -> Routing {
    analyse_cached(prj, wiring, conns, open, &mut Cache::default(), progress)
}

/// What a routing keeps for the next one: its inputs and result, reused while they
/// stay the same (edits of the schematic that change no route).
#[derive(Clone, Default)]
pub struct Cache {
    last: Option<Box<Last>>,
}

#[derive(Clone)]
struct Last {
    /// The project without its pages: the routing takes them only through the jobs.
    prj: Project,
    lang: openecraft_i18n::Lang,
    jobs: Vec<Job>,
    /// The wiring before and after the routing.
    wiring: Vec<RoutedWire>,
    routed: Vec<RoutedWire>,
    routing: Routing,
}

/// Like [`analyse_with`], taking the routing from `cache` when nothing it depends on
/// changed, and keeping this one there.
pub fn analyse_cached(prj: &Project, wiring: &mut [RoutedWire], conns: &[Connection], open: &Netlist, cache: &mut Cache, progress: &dyn Fn(usize, usize)) -> Routing {
    match lookup(prj, wiring, conns, open, cache) {
        Ok(routing) => routing,
        Err(key) => {
            let (rest, lang, jobs) = *key;
            let before = wiring.to_vec();
            let routing = route(prj, wiring, jobs.clone(), Glands::default(), progress, crate::parallel::threads().min(ROUTING_THREADS));
            cache.last = Some(Box::new(Last { prj: rest, lang, jobs, wiring: before, routed: wiring.to_vec(), routing: routing.clone() }));
            routing
        }
    }
}

/// The routing kept in `cache` if nothing it depends on changed (`wiring` routed as
/// with it); `None` where it would have to be searched.
pub fn kept(prj: &Project, wiring: &mut [RoutedWire], conns: &[Connection], open: &Netlist, cache: &Cache) -> Option<Routing> {
    lookup(prj, wiring, conns, open, cache).ok()
}

/// The kept routing, or what it is kept for: the project without its pages, the
/// language and the jobs.
fn lookup(prj: &Project, wiring: &mut [RoutedWire], conns: &[Connection], open: &Netlist, cache: &Cache) -> Result<Routing, Box<(Project, openecraft_i18n::Lang, Vec<Job>)>> {
    let jobs = jobs(prj, wiring, conns, open);
    let lang = openecraft_i18n::lang();
    let rest = Project { pages: Vec::new(), ..prj.clone() };
    match cache.last.as_deref().filter(|l| l.lang == lang && l.jobs == jobs && l.wiring == wiring && l.prj == rest) {
        Some(last) => {
            wiring.clone_from_slice(&last.routed);
            Ok(last.routing.clone())
        }
        None => Err(Box::new((rest, lang, jobs))),
    }
}

/// What is to be routed: the cables, hoses and the wires that are no cores of a cable,
/// in the order they are routed (the cable glands of the line-ups taken, the thickest
/// cable first).
fn jobs(prj: &Project, wiring: &[RoutedWire], conns: &[Connection], open: &Netlist) -> Vec<Job> {
    let rules = &prj.layout.routing;
    let pos = panel::positions(&prj.layout);
    let mut jobs: Vec<Job> = Vec::new();
    let mut glands = Glands::default();
    let target = |p: &crate::netlist::PinRef| (p.page, prj.pages[p.page].symbols[p.sym].id);
    // Cables
    let mut cables: Vec<&Cable> = prj.cables.iter().collect();
    cables.sort_by(|a, b| cable_diameter(b).total_cmp(&cable_diameter(a)).then(a.tag.cmp(&b.tag)));
    for c in cables {
        let cores: Vec<&Connection> = conns.iter().filter(|x| x.cable == c.tag).collect();
        let Some((a, b)) = cable_end_tags(prj, c, &cores) else { continue };
        let Some(ends) = ends_between(prj, &pos, &a, &b, [&pins_at(prj, &cores, &a), &pins_at(prj, &cores, &b)], &mut glands, cable_diameter(c)) else { continue };
        let shielded = is_shielded(c);
        jobs.push(Job {
            kind: RouteKind::Cable,
            key: c.tag.clone(),
            label: c.tag.clone(),
            class: class_of_cable(prj, c, &cores),
            ends,
            diameter: cable_diameter(c),
            shielded,
            wire: None,
            fixed: None,
            extra: 2.0 * rules.cable_reserve,
            bend: 0.0,
            set_length: 0.0,
            target: cores.first().map(|c| target(&c.a)),
            laying: Laying::Surface,
        });
    }
    // Hoses, tubes and pipes with their configuration: bend radius, and the insertion
    // depth at both ends for the cut length. They end at the nozzles of their devices,
    // the routes coming to them twice as far straight out of them as they are thick.
    let tag = |p: crate::netlist::PinRef| prj.pages[p.page].symbols[p.sym].tag.clone();
    let hoses: Vec<(crate::fluid::Hose, [End; 2])> = crate::fluid::hoses(prj, open).into_iter().filter_map(|h| ends_between(prj, &pos, &tag(h.a), &tag(h.b), [&[], &[]], &mut glands, tube_diameter(&h.size)).map(|e| (h, e))).collect();
    let turned = turned_fittings(prj, &pos, &hoses);
    for (h, mut ends) in hoses {
        let diameter = tube_diameter(&h.size);
        let out = (2.0 * diameter).max(NOZZLE_OUT);
        for (end, pin) in ends.iter_mut().zip([h.a, h.b]) {
            at_nozzle(prj, &pos, end, pin, out, &turned);
        }
        // Nozzles facing each other closer than that: straight from one to the other,
        // the route from between them.
        if let (Some(ta), Some(tb), Some(da), Some(db)) = (ends[0].nozzle, ends[1].nozzle, ends[0].leave, ends[1].leave) {
            let gap = dist(ta, tb);
            if crate::parts::vec3::dot(da, db) < -0.99 && gap < 2.0 * out + 1.0 {
                let out = (gap / 2.0 - 1.0).max(0.0);
                ends[0].point = Some(add(ta, scale(da, out)));
                ends[1].point = Some(add(tb, scale(db, out)));
            }
        }
        let label = if h.number.is_empty() { format!("{} – {}", h.from, h.to) } else { h.number.clone() };
        jobs.push(Job {
            kind: RouteKind::Tube,
            key: format!("hose:{}|{}", h.from, h.to),
            label,
            class: Class::Fluid,
            ends,
            diameter,
            shielded: false,
            wire: None,
            fixed: None,
            extra: 2.0 * prj.fluid.allowance,
            // Pipes are bent with elbows, the others with the bend radius of the rules.
            bend: if h.config.bend_radius > 0.0 { h.config.bend_radius } else if h.config.line == crate::model::fluid::LineType::Pipe { PIPE_ELBOW * diameter } else { 0.0 },
            set_length: h.length * 1000.0,
            target: Some(target(&h.a)),
            laying: Laying::Surface,
        });
    }
    // Wires: cable cores run in their cable.
    let net_of: HashMap<(String, String), &Connection> = conns
        .iter()
        .map(|c| ((crate::netlist::pin_designation(prj, c.a), crate::netlist::pin_designation(prj, c.b)), c))
        .collect();
    for (i, w) in wiring.iter().enumerate() {
        let conn = net_of.get(&(w.from.clone(), w.to.clone())).or_else(|| net_of.get(&(w.to.clone(), w.from.clone())));
        if conn.is_some_and(|c| !c.cable.is_empty()) {
            continue;
        }
        let class = class_of_net(prj, &w.net);
        let end = |tag: &str, ei: usize, item: u64, q: Option<&crate::geom::P>| End {
            tag: tag.into(),
            enclosure: Some(ei),
            point: q.map(|q| add([PLATE_MARGIN + q.x, PLATE_MARGIN + q.y, PLATE_Z + 5.0], pos[ei])),
            item: Some(item),
            ..Default::default()
        };
        let last = w.stretches().last().map(|s| s.1).unwrap_or(&w.route);
        let free = wire_in_3d(&prj.layout, w) && !w.route.points.is_empty();
        // In a room and in a cabinet without cable ducts from connection point to
        // connection point (there is no plate, or no duct on it, to come down to).
        let direct = free && between_points(&prj.layout, w);
        let ends = match (direct, conn) {
            (true, Some(c)) => [c.a, c.b].map(|p| device_end(prj, &pos, &prj.pages[p.page].symbols[p.sym].tag, std::slice::from_ref(&p))),
            _ => [end(&w.from, w.enclosure, w.items.0, w.route.points.first()), end(&w.to, w.last_enclosure(), w.items.1, last.points.last())],
        };
        let fixed = (!free).then(|| wire_points(&pos, w));
        jobs.push(Job {
            kind: RouteKind::Wire,
            key: wire_key(w),
            label: format!("{} – {}", w.from, w.to),
            class,
            ends,
            diameter: crate::harness::outer_diameter(&w.cross_section),
            shielded: false,
            wire: Some(i),
            fixed,
            // Laid from connection point to connection point it needs its reserve only,
            // along the plate also the ways from the connection points down to it.
            extra: if direct { prj.layout.wire_reserve } else { w.length - w.plate_length() },
            bend: 0.0,
            set_length: 0.0,
            target: conn.map(|c| target(&c.a)),
            laying: Laying::Surface,
        });
    }
    // The cables and tubes before the wires within a class, the thickest first within a
    // kind: they take the straightest ways, the thinner ones go round them more easily.
    // Of those as thick in the cabinets the shortest first: between devices side by
    // side the inner ones lie nearest to them, the outer ones go round them and cross
    // none. (On the walls of a room the long ones go first: they have fewer ways.)
    let order = |k: RouteKind| match k {
        RouteKind::Cable => 0,
        RouteKind::Tube => 1,
        RouteKind::Wire => 2,
    };
    let in_room = |e: &End| e.enclosure.is_some_and(|k| prj.layout.enclosures[k].is_room());
    let span = |j: &Job| match (j.ends[0].point, j.ends[1].point) {
        (Some(a), Some(b)) if !in_room(&j.ends[0]) && !in_room(&j.ends[1]) => dist(a, b),
        _ => f32::INFINITY,
    };
    // Flush-mounted or on the surface in rooms: as set for it, otherwise as the rules say.
    for j in &mut jobs {
        j.laying = prj.layout.laying.get(&j.key).copied().unwrap_or(prj.layout.routing.laying);
    }
    jobs.sort_by(|a, b| a.class.cmp(&b.class).then(order(a.kind).cmp(&order(b.kind))).then(b.diameter.total_cmp(&a.diameter)).then(span(a).total_cmp(&span(b))).then(a.key.cmp(&b.key)));
    jobs
}

/// The enclosure an end of a route comes into through no cable gland although it has
/// some: every one of them is taken by another cable ([`Glands::pick`]).
fn gland_taken(prj: &Project, ends: &[End; 2]) -> Option<usize> {
    ends.iter().find_map(|e| e.enclosure.filter(|&ei| e.item.is_none() && e.point.is_some() && !prj.layout.enclosures[ei].is_room() && !entries(prj, ei).is_empty()))
}

type Ahead = (Option<grid::Way>, Vec<f32>, Option<grid::GoalCosts>);

fn finish_ahead(space:&grid::Space, req:&grid::Request, area:f32, reuse_failed:bool, entry:Option<Ahead>, search:impl FnOnce(Option<&grid::GoalCosts>)->Option<grid::Way>)->Option<grid::Way> {
    match entry {
        // Within one class, routing only fills paths and adds obstacles. An impossible
        // route cannot become possible, even when a path's fill state changed.
        Some((None,loads,_)) if reuse_failed || space.full_as(&loads,area)=>None,
        Some((Some(way),loads,_)) if space.full_as(&loads,area) && space.still_apart(&way.cells,req)=>Some(way),
        entry=>search(entry.as_ref().and_then(|e|e.2.as_ref())),
    }
}

/// Routes the `jobs` one after the other, the searches of a class on up to `workers`
/// threads at the same time (with the same routes).
#[allow(clippy::too_many_lines)]
fn route(prj: &Project, wiring: &mut [RoutedWire], jobs: Vec<Job>, mut glands: Glands, progress: &dyn Fn(usize, usize), workers: usize) -> Routing {
    let rules = &prj.layout.routing;
    let pos = panel::positions(&prj.layout);
    let lineups = panel::lineups(&prj.layout);
    let lineup_of = |ei: usize| lineups.iter().position(|lu| lu.contains(&ei));
    let poses = panel::site_poses(prj);
    // The room an end is in: the room itself, or the room its cabinet stands in.
    let room_of_end = |ei: usize| if prj.layout.enclosures[ei].is_room() { Some(ei) } else { panel::room_of(&prj.layout, ei) };
    let is_room = |lu: usize| lineups[lu].len() == 1 && prj.layout.enclosures[lineups[lu][0]].is_room();
    // A leg through a room of a flush-mounted route runs in its walls or floor.
    let flush_leg = |job: &Job, leg: &room::LegPlan| job.laying != Laying::Surface && job.fixed.is_none() && is_room(leg.lineup);
    let links = panel::wall_links(prj);
    let mut spaces: HashMap<usize, grid::Space> = HashMap::new();
    // The class each space's segregation zones were last computed for.
    let mut zoned: HashMap<usize, Class> = HashMap::new();
    let frozen: HashMap<&str, &FrozenRoute> = prj.layout.frozen.iter().map(|f| (f.key.as_str(), f)).collect();
    let mut items: Vec<RoutedItem> = Vec::new();
    let mut problems: Vec<Problem> = Vec::new();
    let mut roof_entries: Vec<String> = Vec::new();
    let total = jobs.len();
    // How far beyond its surface a cable or tube keeps the ones routed after it: half the
    // thickness of the thickest.
    let reach = jobs.iter().filter(|j| j.kind != RouteKind::Wire).map(|j| j.diameter / 2.0).fold(0.0, f32::max);
    let make_space = |lu: usize| if is_room(lu) { room::room_space(prj, lineups[lu][0], &pos, &poses, rules) } else { grid::Space::build(prj, &lineups[lu], &pos, rules) };
    // The cable entries the ends of the jobs go through are taken: a route that leaves
    // its cabinet looks for another one.
    for end in jobs.iter().flat_map(|j| &j.ends) {
        if let (Some(ei), Some(id)) = (end.enclosure, end.item) {
            if entries(prj, ei).iter().any(|g| g.id == id) {
                *glands.taken.entry(id).or_default() += 1;
                glands.through.entry(id).or_default().extend(end.point);
            }
        }
    }
    // Cables and tubes between line-ups that stand in no room (by key).
    let mut outside: Vec<String> = Vec::new();
    // The legs of each job: in one line-up (or room) at once, otherwise through the rooms
    // the ends are in and the wall penetrations between them (with whether it enters a
    // cabinet through its roof and the length through the walls); or why not (and the
    // enclosure whose cable glands are all taken, where that is why).
    let plans: Vec<Result<LegsPlan, (String, Option<usize>)>> = jobs
        .iter()
        .map(|job| {
            let known = job.fixed.is_some();
            let lu = job.ends.iter().find_map(|e| e.enclosure).and_then(lineup_of);
            let (Some(lu), Some(a), Some(b)) = (lu, job.ends[0].point.or(known.then_some([0.0; 3])), job.ends[1].point.or(known.then_some([0.0; 3]))) else {
                return Err((tr("not in the panel layout").to_string(), None));
            };
            if let Some(ei) = gland_taken(prj, &job.ends).filter(|_| !known && job.kind != RouteKind::Wire) {
                return Err((tr("no free cable gland").to_string(), Some(ei)));
            }
            if job.ends.iter().filter_map(|e| e.enclosure).all(|ei| lineup_of(ei) == Some(lu)) {
                let leave = [job.ends[0].leave.unwrap_or([0.0; 3]), job.ends[1].leave.unwrap_or([0.0; 3])];
                return Ok((vec![room::LegPlan { lineup: lu, a, b, leave, ignore: Vec::new() }], false, 0.0));
            }
            let rooms: Vec<Option<usize>> = job.ends.iter().map(|e| e.enclosure.and_then(room_of_end)).collect();
            match (rooms[0], rooms[1]) {
                (Some(r), Some(other)) if !known && job.kind != RouteKind::Wire => room::plan_legs(prj, &pos, &poses, &lineup_of, [r, other], &links, &job.ends, &mut glands, job.diameter).map_err(|taken| match taken {
                    Some(ei) => (tr("no free cable gland").to_string(), Some(ei)),
                    None => (if r != other { tr("no wall penetration leads from room to room") } else { tr("not in the same line-up or room") }.to_string(), None),
                }),
                // Between line-ups that stand in no room: in each of them to its cable
                // entry, the way between them is not part of the layout.
                (None, None) if !known && job.kind != RouteKind::Wire => {
                    let mut legs = Vec::new();
                    for (i, end) in job.ends.iter().enumerate() {
                        let (Some(ei), Some(q)) = (end.enclosure, end.point) else { return Err((tr("not in the panel layout").to_string(), None)) };
                        let entry = entry_end(prj, &pos, ei, q, &mut glands, job.diameter);
                        if entry.item.is_none() && !entries(prj, ei).is_empty() {
                            return Err((tr("no free cable gland").to_string(), Some(ei)));
                        }
                        let (Some(lineup), Some(at)) = (lineup_of(ei), entry.point) else { return Err((tr("not in the panel layout").to_string(), None)) };
                        let (own, through) = ((q, end.leave.unwrap_or([0.0; 3])), (at, entry.leave.unwrap_or([0.0; 3])));
                        let (a, b) = if i == 0 { (own, through) } else { (through, own) };
                        legs.push(room::LegPlan { lineup, a: a.0, b: b.0, leave: [a.1, b.1], ignore: entry.item.into_iter().collect() });
                    }
                    outside.push(job.key.clone());
                    Ok((legs, false, 0.0))
                }
                _ => Err((tr("not in the same line-up or room").to_string(), None)),
            }
        })
        .collect();
    // The legs of a frozen job as frozen, where the route is still the same (without the
    // ways into the nozzles, added to the route again).
    let thawed = |job: &Job, legs: &[room::LegPlan]| {
        let mut t = frozen.get(job.key.as_str()).and_then(|f| thaw(prj, &pos, f)).filter(|t| t.len() == legs.len()).unwrap_or_default();
        let near = |q: Option<&V3>, tip: Option<V3>| q.zip(tip).is_some_and(|(q, tip)| dist(*q, tip) < 0.5);
        if let Some(first) = t.first_mut().filter(|p| near(p.first(), job.ends[0].nozzle)) {
            first.remove(0);
        }
        if let Some(last) = t.last_mut().filter(|p| near(p.last(), job.ends[1].nozzle)) {
            last.pop();
        }
        t
    };
    let thawed_ok = |points: &[V3], leg: &room::LegPlan, space: &grid::Space, req: &grid::Request| {
        let near = |q: Option<&V3>, e: V3| q.is_some_and(|q| manhattan(*q, e) <= 2.0 * space.cell);
        near(points.first(), leg.a) && near(points.last(), leg.b) && space.still_free(points, req)
    };
    // The searches of a class run at the same time against the spaces as they are when
    // the class begins: within a class a route changes the next ones only through the
    // routing paths it fills, so a way found ahead is taken where none of the paths
    // became full for it since, otherwise it is searched again (the same routes as one
    // after the other).
    // A way found ahead with the use of the routing paths of its space then and the costs
    // to its goal (to search it again without working them out).
    let mut ahead: HashMap<(usize, usize), Ahead> = HashMap::new();
    // The legs (job, leg) searched at the same time against the spaces as they are.
    let search_ahead = |tasks: &[(usize, usize)], spaces: &HashMap<usize, grid::Space>| -> Vec<Ahead> {
        crate::parallel::par_map_n(tasks, workers, |&(k, i)| {
            let (job, leg) = (&jobs[k], &plans[k].as_ref().map(|p| &p.0[i]).expect("planned"));
            let space = &spaces[&leg.lineup];
            let ignore = job.ignore(leg);
            let req = job.request(leg, space, &ignore, rules);
            let (way, costs) = space.route_with(leg.a, leg.b, &req, rules, None);
            (way, space.loads(), costs)
        })
    };
    // The wires through the cable ducts fill them before anything is routed: cables and
    // tubes keep out of the stretches they would fill above the limit.
    for (job, plan) in jobs.iter().zip(&plans) {
        let (Some(points), Ok((legs, ..))) = (&job.fixed, plan) else { continue };
        for leg in legs {
            let space = spaces.entry(leg.lineup).or_insert_with(|| make_space(leg.lineup));
            let cells = space.cells_along(points);
            space.load_ducts(&cells, job.area_radius(rules).0);
        }
    }
    // The ways out of the ends of everything to be searched stay free for the routes that
    // end there, whichever is searched first.
    for (job, plan) in jobs.iter().zip(&plans) {
        let (None, Ok((legs, ..))) = (&job.fixed, plan) else { continue };
        for leg in legs.iter().filter(|leg| !flush_leg(job, leg)) {
            let space = spaces.entry(leg.lineup).or_insert_with(|| make_space(leg.lineup));
            let ignore = job.ignore(leg);
            let req = job.request(leg, space, &ignore, rules);
            space.keep_ways_out(leg.a, leg.b, &req);
        }
    }
    for (n, job) in jobs.iter().enumerate() {
        progress(n, total);
        if workers > 1 && (n == 0 || jobs[n - 1].class != job.class) {
            let batch_end = jobs[n..].iter().position(|j| j.class != job.class).map_or(jobs.len(), |k| n + k);
            for leg in (n..batch_end).filter_map(|k| plans[k].as_ref().ok()).flat_map(|p| &p.0) {
                let space = spaces.entry(leg.lineup).or_insert_with(|| make_space(leg.lineup));
                if zoned.get(&leg.lineup) != Some(&job.class) {
                    space.update_zones(rules.segregation);
                    zoned.insert(leg.lineup, job.class);
                }
            }
            let mut tasks: Vec<(usize, usize)> = Vec::new();
            for k in (n..batch_end).filter(|&k| jobs[k].fixed.is_none()) {
                let Ok((legs, ..)) = &plans[k] else { continue };
                let frozen_legs = thawed(&jobs[k], legs);
                for (i, leg) in legs.iter().enumerate().filter(|(_, leg)| !flush_leg(&jobs[k], leg)) {
                    let space = &spaces[&leg.lineup];
                    let ignore = jobs[k].ignore(leg);
                    if !frozen_legs.get(i).is_some_and(|p| thawed_ok(p, leg, space, &jobs[k].request(leg, space, &ignore, rules))) {
                        tasks.push((k, i));
                    }
                }
            }
            ahead = tasks.iter().copied().zip(search_ahead(&tasks, &spaces)).collect();
        }
        let mut item = RoutedItem {
            kind: job.kind,
            key: job.key.clone(),
            label: job.label.clone(),
            class: job.class,
            ends: job.ends.clone(),
            enclosures: Vec::new(),
            points: Vec::new(),
            radii: Vec::new(),
            legs: Vec::new(),
            length: 0.0,
            cut: 0.0,
            diameter: job.diameter,
            bend_radius: job.area_radius(rules).1,
            free: 0.0,
            bends: 0,
            shielded: job.shielded,
            status: Status::Routed,
            wire: job.wire,
            laying: job.laying,
        };
        // Wires through the cable ducts are routed already (also those on the door).
        let known = job.fixed.is_some();
        let (legs, through_walls) = match &plans[n] {
            Ok((legs, roof, walls)) => {
                if *roof {
                    roof_entries.push(job.key.clone());
                }
                (legs, *walls)
            }
            Err((why, taken)) => {
                if let Some(ei) = taken {
                    problems.push(Problem { level: Level::Error, message: tr_fmt!("{} {} cannot be routed: every cable gland of {} is taken by another cable. Add a cable gland for it.", job.kind.title(), job.label, prj.layout.enclosures[*ei].name), target: job.target });
                }
                item.status = Status::Unrouted(why.clone());
                items.push(item);
                continue;
            }
        };
        let (area, radius) = job.area_radius(rules);
        let is_frozen = frozen.contains_key(job.key.as_str());
        let thawed = thawed(job, legs);
        let mut ways: Vec<(usize, Vec<V3>, f32, Vec<f32>)> = Vec::new();
        let mut rerouted = false;
        let mut no_zone = false;
        for (i, leg) in legs.iter().enumerate() {
            // Flush-mounted through a room: along its installation zones, in its walls or
            // floor (apart from the routes through the room's air).
            if flush_leg(job, leg) {
                let ri = lineups[leg.lineup][0];
                match flush::route(prj, ri, sub(leg.a, pos[ri]), sub(leg.b, pos[ri]), job.laying, job.diameter, rules) {
                    Some(points) => {
                        let points: Vec<V3> = points.into_iter().map(|q| add(q, pos[ri])).collect();
                        let radii = grid::corner_radii(&points, radius);
                        ways.push((leg.lineup, points, 0.0, radii));
                        continue;
                    }
                    None => {
                        no_zone = true;
                        ways.clear();
                        break;
                    }
                }
            }
            let space = spaces.entry(leg.lineup).or_insert_with(|| make_space(leg.lineup));
            // Classes that come later keep apart from those routed before them.
            if zoned.get(&leg.lineup) != Some(&job.class) {
                space.update_zones(rules.segregation);
                zoned.insert(leg.lineup, job.class);
            }
            let ignore = job.ignore(leg);
            let req = job.request(leg, space, &ignore, rules);
            let way = match (&job.fixed, thawed.get(i)) {
                (Some(points), _) => Some((points.clone(), space.cells_along(points), 0.0)),
                (None, Some(points)) if thawed_ok(points, leg, space, &req) => Some((points.clone(), space.cells_along(points), 0.0)),
                (None, _) => {
                    rerouted |= is_frozen;
                    // Found ahead: taken where no routing path became full for it since and
                    // no route took its room (the same as searched now); otherwise searched
                    // again, with the costs to the goal worked out then where they still hold.
                    let entry = ahead.remove(&(n, i));
                    finish_ahead(space,&req,area,rules.reuse_failed_searches,entry,|costs|space.route_with(leg.a,leg.b,&req,rules,costs).0)
                    .map(|w| (w.points, w.cells, w.free))
                }
            };
            let Some((points, cells, free)) = way else {
                ways.clear();
                break;
            };
            space.occupy(&cells, job.class, area);
            if !known {
                space.load_ducts(&cells, area);
            }
            // The routes after it keep apart from it (not from the way of a wire along the
            // cable ducts of the plate: it is laid out with the others in the 3D view).
            if job.fixed.is_none() {
                let zone = space.end_zone(&req);
                space.take(&points, job.diameter / 2.0, reach, &[(leg.a, zone), (leg.b, zone)], req.ignore);
            }
            let radii = grid::corner_radii(&points, radius);
            ways.push((leg.lineup, points, free, radii));
        }
        if rerouted {
            item.status = Status::Rerouted;
            problems.push(Problem { level: Level::Warning, message: tr_fmt!("Frozen route of {} is no longer possible and was routed again", job.label), target: job.target });
        } else if is_frozen && !known {
            item.status = Status::Frozen;
        }
        if ways.is_empty() && no_zone {
            item.status = Status::Unrouted(tr("no way through the installation zones").to_string());
            problems.push(Problem { level: Level::Error, message: tr_fmt!("{} {} cannot be laid flush-mounted: the installation zones of the room do not join its ends", job.kind.title(), job.label), target: job.target });
            items.push(item);
            continue;
        }
        if ways.is_empty() {
            item.status = Status::Unrouted(tr("no way through the enclosure").to_string());
            problems.push(Problem { level: Level::Error, message: tr_fmt!("{} {} cannot be routed: no way that keeps half its thickness clear of everything and bends with its radius of {} mm. Move its ends or make room for it.", job.kind.title(), job.label, format!("{radius:.0}")), target: job.target });
            items.push(item);
            continue;
        }
        let mut length = 0.0;
        for (_, points, free, _) in &ways {
            let (bends, shortest) = grid::bends(points);
            // Corners are rounded with the bend radius where the stretches allow it.
            let rounding = if bends > 0 { bends as f32 * radius.min(shortest / 2.0).max(0.0) * (2.0 - std::f32::consts::FRAC_PI_2) } else { 0.0 };
            length += grid::length(points) - rounding;
            item.bends += bends;
            item.free += if known { 0.0 } else { *free };
        }
        length += through_walls;
        // Tubes go on straight into the nozzles of their devices.
        if let (Some(tip), Some((_, points, _, radii))) = (job.ends[0].nozzle, ways.first_mut()) {
            length += dist(tip, points[0]);
            points.insert(0, tip);
            radii.insert(0, 0.0);
            radii[1] = radius;
        }
        if let (Some(tip), Some((_, points, _, radii))) = (job.ends[1].nozzle, ways.last_mut()) {
            length += dist(tip, points[points.len() - 1]);
            points.push(tip);
            if let Some(r) = radii.last_mut() {
                *r = radius;
            }
            radii.push(0.0);
        }
        item.length = match (known, job.wire) {
            (true, Some(wi)) => wiring[wi].length,
            _ => length + job.extra,
        };
        // Stretches between bends need room for the arcs of both (the bend radius each);
        // the ways out of the nozzles count, the lane changes are left out.
        let close_bends = ways.iter().any(|(_, points, ..)| grid::close_bends(points, 2.0 * radius) > 0);
        if close_bends && job.kind != RouteKind::Wire {
            problems.push(Problem { level: Level::Warning, message: tr_fmt!("{} {}: bends closer than the bend radius ({} mm)", job.kind.title(), job.label, format!("{radius:.0}")), target: job.target });
        }
        // A hose with its length set in the schematic must reach along its route.
        if job.set_length > 0.0 && job.set_length + 0.5 < item.length {
            let (set, routed) = (format!("{:.2}", job.set_length / 1000.0), format!("{:.2}", item.length / 1000.0));
            problems.push(Problem { level: Level::Warning, message: tr_fmt!("Hose {}: length {} m shorter than its 3D route of {} m", job.label, set, routed), target: job.target });
        }
        let step = rules.cut(job.kind).max(1.0);
        item.cut = (item.length / step).ceil() * step;
        for (lu, points, _, radii) in &ways {
            let mut encs: Vec<usize> = Vec::new();
            for q in points {
                if let Some(k) = lineups[*lu].iter().copied().find(|&k| is_room(*lu) || (q[0] >= pos[k][0] - 0.5 && q[0] <= pos[k][0] + prj.layout.enclosures[k].width + 0.5)) {
                    if encs.last() != Some(&k) {
                        encs.push(k);
                    }
                }
            }
            if ways.len() > 1 {
                let enclosure = encs.first().copied().unwrap_or(lineups[*lu][0]);
                item.legs.push(RouteLeg { enclosure, points: points.clone(), radii: radii.clone() });
            }
            for k in encs {
                if item.enclosures.last() != Some(&k) {
                    item.enclosures.push(k);
                }
            }
            item.points.extend(points.iter().copied());
            item.radii.extend(radii.iter().copied());
        }
        if let (Some(wi), false) = (job.wire, known) {
            apply_to_wire(prj, &pos, &mut wiring[wi], &item.points, job.extra);
            item.length = wiring[wi].length;
            item.cut = (item.length / step).ceil() * step;
        }
        items.push(item);
    }
    let (bundles, branches) = bundles::find(&items, rules.grid.max(5.0));
    // Classes that keep apart in one bundle; routes that end at the same device (the
    // power cable and the pipes of a heat pump) meet at it, where they leave it
    // ([`grid::Request::at`]) and come up to it.
    let at_common_end = |b: &Bundle| {
        let ends = |i: usize| items[i].ends.iter().filter_map(|e| e.item).collect::<Vec<u64>>();
        let first = b.items.first().map_or_else(Vec::new, |&i| ends(i));
        b.length <= 4.0 * rules.grid.max(5.0) && first.iter().any(|id| b.items.iter().all(|&i| ends(i).contains(id)))
    };
    for b in bundles.iter().filter(|b| !at_common_end(b)) {
        let classes: Vec<Class> = b.items.iter().map(|&i| items[i].class).collect();
        if classes.iter().any(|a| classes.iter().any(|c| a.keeps_apart(*c))) {
            // Shielded cables are reported with their shield.
            let labels: Vec<&str> = b.items.iter().filter(|&&i| items[i].kind != RouteKind::Wire && !items[i].shielded).map(|&i| items[i].label.as_str()).collect();
            if !labels.is_empty() {
                problems.push(Problem { level: Level::Warning, message: tr_fmt!("Segregation: {} run together with power conductors over {} mm", labels.join(", "), format!("{:.0}", b.length)), target: None });
            }
        }
    }
    let mut paths = Vec::new();
    for space in spaces.values() {
        for p in space.paths.iter().filter(|p| !p.implied) {
            paths.push(PathUse { enclosure: p.enclosure, path: p.id, items: p.items, area: p.used, capacity: p.capacity });
            if p.capacity > 0.0 && p.used > p.capacity {
                let e = &prj.layout.enclosures[p.enclosure];
                let name = e.paths.iter().find(|x| x.id == p.id).map(|x| x.name.clone()).unwrap_or_default();
                problems.push(Problem { level: Level::Warning, message: tr_fmt!("{}: routing path {} is full ({} % of its usable cross-section)", e.name, name, format!("{:.0}", p.used / p.capacity * 100.0)), target: None });
            }
        }
    }
    paths.sort_by(|a, b| a.enclosure.cmp(&b.enclosure).then(a.path.cmp(&b.path)));
    let (shields, shield_problems) = shield::analyse(prj, &items, &bundles);
    problems.extend(shield_problems);
    for it in items.iter().filter(|i| i.kind != RouteKind::Wire && i.ends.iter().any(|e| e.enclosure.is_some() && e.item.is_none())) {
        problems.push(Problem { level: Level::Info, message: tr_fmt!("{} {} enters through the bottom: no cable gland in the enclosure", it.kind.title(), it.label), target: None });
    }
    for it in items.iter().filter(|i| i.routed() && outside.contains(&i.key)) {
        problems.push(Problem { level: Level::Info, message: tr_fmt!("{} {} is routed to the cable entries of its enclosures: they stand in no common room, the way between them is not part of the layout", it.kind.title(), it.label), target: None });
    }
    for it in items.iter().filter(|i| roof_entries.contains(&i.key)) {
        problems.push(Problem { level: Level::Info, message: tr_fmt!("{} {} enters through the roof: no cable gland on the roof, sides or rear", it.kind.title(), it.label), target: None });
    }
    let mut changes = Vec::new();
    for f in &prj.layout.frozen {
        let after = items.iter().find(|i| i.key == f.key && i.routed()).map(|i| i.length);
        if after.is_none_or(|a| (a - f.length).abs() > 1.0) {
            let label = items.iter().find(|i| i.key == f.key).map(|i| i.label.clone()).unwrap_or_else(|| f.key.clone());
            changes.push(Change { key: f.key.clone(), label, before: f.length, after });
        }
    }
    grid::release_search();
    Routing { items, bundles, branches, shields, paths, changes, problems }
}

/// Replaces the route of a wire by its 3D route: the stretches per enclosure as
/// seen on the plates, the cable ducts it runs through, the cable passages, and its
/// length (`extra`: from the connection points to the plate and the reserve).
fn apply_to_wire(prj: &Project, pos: &[V3], w: &mut RoutedWire, points: &[V3], extra: f32) {
    let encs = &prj.layout.enclosures;
    let enclosure_at = |q: V3| (0..encs.len()).find(|&k| q[0] >= pos[k][0] - 0.5 && q[0] <= pos[k][0] + encs[k].width + 0.5);
    // Split at the boundaries between enclosures: with a point where a stretch crosses
    // the side of one (the cable passage it runs through).
    let mut cut: Vec<V3> = Vec::with_capacity(points.len());
    for s in points.windows(2) {
        cut.push(s[0]);
        let mut sides: Vec<f32> = (0..encs.len()).flat_map(|k| [pos[k][0], pos[k][0] + encs[k].width]).filter(|x| (x - s[0][0]) * (x - s[1][0]) < -1e-3).collect();
        sides.sort_by(|a, b| (a - s[0][0]).abs().total_cmp(&(b - s[0][0]).abs()));
        sides.dedup_by(|a, b| (*a - *b).abs() < 1e-3);
        for x in sides {
            cut.push(add(s[0], scale(sub(s[1], s[0]), (x - s[0][0]) / (s[1][0] - s[0][0]))));
        }
    }
    cut.extend(points.last());
    let mut stretches: Vec<(usize, Vec<V3>, Option<V3>)> = Vec::new();
    for s in cut.windows(2) {
        let mid = [(s[0][0] + s[1][0]) / 2.0, (s[0][1] + s[1][1]) / 2.0, (s[0][2] + s[1][2]) / 2.0];
        let Some(k) = enclosure_at(mid) else { continue };
        match stretches.last_mut() {
            Some(last) if last.0 == k => last.1.push(s[1]),
            _ => {
                let entry = (!stretches.is_empty()).then_some(s[0]);
                stretches.push((k, vec![s[0], s[1]], entry));
            }
        }
    }
    let to_route = |k: usize, pts: &[V3]| {
        let e = &encs[k];
        let (pw, ph) = e.plate_size();
        let mut plate: Vec<crate::geom::P> = Vec::new();
        for q in pts {
            let l = sub(*q, pos[k]);
            let pq = crate::geom::p((l[0] - PLATE_MARGIN).clamp(0.0, pw), (l[1] - PLATE_MARGIN).clamp(0.0, ph));
            if plate.last().is_none_or(|x| !x.same(pq)) {
                plate.push(pq);
            }
        }
        let runs = duct_runs(e, &pts.iter().map(|q| sub(*q, pos[k])).collect::<Vec<_>>());
        panel::Route { points: plate, plate_length: grid::length(pts), via_ducts: !runs.is_empty(), runs }
    };
    let mut it = stretches.into_iter();
    let Some((k0, p0, _)) = it.next() else { return };
    w.enclosure = k0;
    w.route = to_route(k0, &p0);
    w.legs = it.map(|(k, pts, entry)| panel::Leg { enclosure: k, route: to_route(k, &pts), passage: entry.unwrap_or(pts[0]), passage_length: 0.0 }).collect();
    w.length = w.plate_length() + extra;
}

/// Stretches of the ducts a 3D route (enclosure coordinates) runs through: on the
/// mounting plate of a cabinet, on every face of a room. Every segment counts with the
/// part of it inside the duct, also one that runs on beyond the duct (into the crossing
/// duct at a corner, past its end).
pub(crate) fn duct_runs(e: &panel::Enclosure, pts: &[V3]) -> Vec<panel::DuctRun> {
    let mut runs: Vec<panel::DuctRun> = Vec::new();
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let dir = sub(b, a);
        for d in e.items.iter().filter(|i| i.kind == ItemKind::Duct && (i.face == Face::Plate || e.is_room())) {
            let (lo, hi) = panel::item_box(e, d);
            let (mut t0, mut t1) = (0.0f32, 1.0f32);
            for i in 0..3 {
                let (l, h) = (lo[i] - 1.0, hi[i] + 1.0);
                if dir[i].abs() < 1e-6 {
                    if a[i] < l || a[i] > h {
                        t1 = -1.0;
                    }
                    continue;
                }
                let (u, v) = ((l - a[i]) / dir[i], (h - a[i]) / dir[i]);
                t0 = t0.max(u.min(v));
                t1 = t1.min(u.max(v));
            }
            if t1 < t0 {
                continue;
            }
            // Along the duct in the coordinates of its face.
            let [o, u, w, _] = panel::face_frame(e, d.face);
            let along = if d.horizontal() { u } else { w };
            let at = |t: f32| dot(sub(add(a, scale(dir, t)), o), along) / dot(along, along);
            let (s0, s1) = (at(t0), at(t1));
            if (s1 - s0).abs() > 0.01 {
                runs.push(panel::DuctRun { duct: d.id, from: s0.min(s1), to: s0.max(s1) });
            }
        }
    }
    runs
}

/// The cables and tubes of a routing that run through cable ducts, for their fill
/// levels (the wires are counted with their routes).
pub fn duct_contents(prj: &Project, routing: &Routing) -> Vec<panel::DuctContent> {
    let pos = panel::positions(&prj.layout);
    let mut out = Vec::new();
    for it in routing.items.iter().filter(|i| i.kind != RouteKind::Wire && i.routed()) {
        for (leg, pts) in it.stretches() {
            let Some(start) = leg.or(it.enclosures.first().copied()) else { continue };
            for k in panel::lineup_of(&prj.layout, start) {
                let runs = duct_runs(&prj.layout.enclosures[k], &pts.iter().map(|q| sub(*q, pos[k])).collect::<Vec<_>>());
                if !runs.is_empty() {
                    out.push(panel::DuctContent { enclosure: k, runs, diameter: it.diameter });
                }
            }
        }
    }
    out
}

/// Freezes the routes of the given keys (all routed items without keys) for
/// production. Returns how many.
pub fn freeze(prj: &mut Project, routing: &Routing, keys: &[String]) -> usize {
    let pos = panel::positions(&prj.layout);
    let mut n = 0;
    for it in routing.items.iter().filter(|i| i.routed() && (keys.is_empty() || keys.contains(&i.key))) {
        let Some(&k) = it.enclosures.first() else { continue };
        let encs = &prj.layout.enclosures;
        let relative = |k: usize, pts: &[V3]| pts.iter().map(|q| sub(*q, pos[k])).collect::<Vec<V3>>();
        let f = if it.legs.is_empty() {
            FrozenRoute { key: it.key.clone(), kind: it.kind, enclosure: encs[k].id, points: relative(k, &it.points), length: it.length, legs: Vec::new() }
        } else {
            let legs = it.legs.iter().map(|l| FrozenLeg { enclosure: encs[l.enclosure].id, points: relative(l.enclosure, &l.points) }).collect();
            FrozenRoute { key: it.key.clone(), kind: it.kind, enclosure: encs[k].id, points: Vec::new(), length: it.length, legs }
        };
        prj.layout.frozen.retain(|x| x.key != it.key);
        prj.layout.frozen.push(f);
        n += 1;
    }
    prj.layout.frozen.sort_by(|a, b| a.key.cmp(&b.key));
    n
}

/// The points of a frozen route in layout coordinates, leg by leg (one leg for routes
/// in one line-up); `None` if an enclosure it is relative to no longer exists.
fn thaw(prj: &Project, pos: &[V3], f: &FrozenRoute) -> Option<Vec<Vec<V3>>> {
    let at = |id: u64| prj.layout.enclosures.iter().position(|e| e.id == id).map(|k| pos[k]);
    let shift = |id: u64, pts: &[V3]| at(id).map(|o| pts.iter().map(|q| add(*q, o)).collect::<Vec<V3>>());
    if f.legs.is_empty() {
        return Some(vec![shift(f.enclosure, &f.points)?]);
    }
    f.legs.iter().map(|l| shift(l.enclosure, &l.points)).collect()
}

/// Releases frozen routes (all without keys). Returns how many.
pub fn release(prj: &mut Project, keys: &[String]) -> usize {
    let before = prj.layout.frozen.len();
    prj.layout.frozen.retain(|f| !keys.is_empty() && !keys.contains(&f.key));
    before - prj.layout.frozen.len()
}

#[cfg(test)]
mod tests;
