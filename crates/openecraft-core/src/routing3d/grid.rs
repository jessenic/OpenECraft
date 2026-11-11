//! The space of a line-up (or a room, see `super::room`) as a grid of cubic cells, and
//! the obstacle-aware shortest way through it (A* with turn costs). Cells are free
//! inside the enclosures (in front of the mounting plate, behind the door), blocked by
//! the walls, by devices, rails, busbars and accessories (with a clearance) and by
//! keep-out reservations. Cable ducts and routing paths are channels: the way through
//! them is cheaper than through free air; a cable too thick for a duct keeps out of it,
//! one that fits enters and leaves the ducts of a cabinet through the slots of their
//! walls (never through their base or cover) and keeps off their walls outside them.
//! A stretch of a duct it would fill above the fill limit costs more.
//! The room around every cell is measured from what blocks the space (its boxes and
//! walls themselves, not the cells they block): a route keeps half its thickness from
//! all of it but the items at its ends (near its ends the clearance round the items
//! does not count, the items do). It turns only where its bend, rounded with the bend
//! radius, keeps clear too. A route leaves each of its ends straight on, as far as its
//! bend radius (through the box of its own device where the connection point lies
//! inside it), and between two of its bends it runs straight for two radii (the arcs
//! of both fit between them); a lane change with an S-bend is free of that. Where there
//! is no way like that, there is no route.
//! Routes keep apart from the ones routed before them by their thicknesses: none runs
//! inside another in free air (in a duct or routing path they lie side by side, but
//! where they come up to their ends), and none crosses the straight way out of an end
//! of another that is still to be searched. A route keeps apart from itself too: its
//! way never comes back across or alongside itself closer than it is thick
//! ([`meets_itself`]). Walls between bayed enclosures are crossed at cable passages
//! only.

use super::{Class, PathKind, ReserveMode, RouteKind, RoutingRules};
use crate::model::Project;
use crate::panel::{self, Enclosure, ItemKind, PanelItem, PLATE_Z};
use crate::parts::vec3::{add, dist, dot, scale, sub, V3};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

/// Cell blocked by a wall, the outside or a keep-out reservation.
const WALL: u8 = 2;
/// Cell blocked by an item (see `owner`): its clearance around it.
const ITEM: u8 = 1;
/// Cell blocked by the item itself (see `owner`).
const BODY: u8 = 3;
/// Cost factor of a channel that is already full.
const FULL: f32 = 10.0;
/// Upper limit of cells; a larger line-up gets coarser cells.
const MAX_CELLS: usize = 1_500_000;
/// Cost factor of a horizontal step in the open space of a room (see
/// [`Space::mark_open`]).
const OPEN: f32 = 10.0;
/// How far (mm, at least [`ROOM_CELLS`] cells) the room around the cells is measured
/// (see [`Space::measure`]): farther from everything a cell has room for anything.
const ROOM_CAP: f32 = 300.0;
const ROOM_CELLS: f32 = 8.0;
/// How far a bend of radius r rounding a right-angled corner lies inside it: (√2 − 1) r.
pub const CORNER_CUT: f32 = std::f32::consts::SQRT_2 - 1.0;
/// Most cells whose costs to the goal a search keeps for searching again.
const KEEP_COSTS: usize = 400_000;
/// Cells looked at along the way out of an end for the cell the route leaves it in.
const LEAD_CELLS: usize = 4;
/// How far a route may lean (aside per length) from the direction it leaves an end in,
/// on its way from the end point onto the line of cells it runs along from there.
pub const LEAN: f32 = 0.3;
/// What a step across another route costs where a route may cross one, in cells.
const CROSSING: f32 = 4.0;
/// Chords a bend is checked with where a way is held apart from itself
/// ([`meets_itself`]).
const BEND_CHORDS: usize = 3;

/// How far a route of bend radius `radius` runs along the direction it leaves an end
/// in to come onto a line `aside` mm beside it: an S-bend of two arcs of the radius
/// (two quarter turns where the line lies further aside than they reach).
pub fn onto_line(aside: f32, radius: f32) -> f32 {
    if aside >= 2.0 * radius {
        2.0 * radius
    } else {
        (aside * (4.0 * radius - aside)).max(0.0).sqrt()
    }
}
/// Edge (mm) of the coarse cells the solids are listed by ([`Space::point_clear`]), and
/// how far (mm) round a solid it is listed: the thickest route that is checked exactly.
const SOLID_CELL: f32 = 100.0;
const SOLID_REACH: f32 = 64.0;

/// How a cell can be passed: not at all, or freely (with half the thickness of room).
const NO: u8 = 0;
const FREE: u8 = 2;

/// The costs to the goal a search worked out (the cells known with their costs, the
/// highest), to search the same way again (the same channels full) without working
/// them out: to a goal nearby as well, less the cost from it to this one.
#[derive(Clone, Debug, PartialEq)]
pub struct GoalCosts {
    goal: usize,
    cells: Vec<(u32, f32)>,
    reach: f32,
}

/// A routing path in the grid and what runs through it.
#[derive(Clone, Debug, PartialEq)]
pub struct PathCells {
    pub enclosure: usize,
    pub id: u64,
    pub kind: PathKind,
    /// Allowed classes as bits; 0: all.
    pub classes: u8,
    /// Usable cross-section in mm² (without the reserve).
    pub capacity: f32,
    pub used: f32,
    pub items: usize,
    /// Not a routing path of the layout: the cable way of a 19-inch rack beside its
    /// mounting rails.
    pub implied: bool,
}

/// A stretch of a cable duct one cell long: the cross-section the fill limit allows in
/// it and what runs through it, mm².
#[derive(Clone, Copy, Debug, PartialEq)]
struct Section {
    capacity: f32,
    used: f32,
}

/// Where wires may cross from one enclosure into the next: the boundary `x` and the
/// opening's `y` and `z` ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Crossing {
    x: f32,
    y: [f32; 2],
    z: [f32; 2],
}

/// What blocks the space, to measure the room around the cells: a box (an item with
/// its clearance, a keep-out, a wall, a cabinet standing in a room) with the item it is
/// (0 for none), or the walls, the ceiling and the floor of a room (its floor plan at
/// an origin, the room's height).
#[derive(Clone, Debug, PartialEq)]
enum Solid {
    Box { lo: V3, hi: V3, owner: u64 },
    Room { plan: Vec<[f32; 2]>, origin: V3, height: f32 },
}

/// A cable duct of a cabinet: its box, the axis it runs along, and the heights above
/// its base of the closed rim of its slotted walls and of their top (its cover lies on
/// them). A route outside it keeps off it, one enters and leaves it through its walls
/// between the rim and the top, never through its base or its cover.
#[derive(Clone, Debug, PartialEq)]
struct DuctBox {
    lo: V3,
    hi: V3,
    axis: usize,
    /// Heights above the bottom of the box: of the top of its base, of the rim of its
    /// walls and of the underside of its cover.
    base: f32,
    rim: f32,
    top: f32,
}

/// What is routed.
#[derive(Clone, Copy, Debug)]
pub struct Request<'a> {
    pub kind: RouteKind,
    pub class: Class,
    /// Items the ends are at: their cells are passable and the room they take does not
    /// count, those of the items themselves only near the start and the end `at` (a
    /// route leaves a device, not runs through it).
    pub ignore: &'a [u64],
    pub at: [V3; 2],
    /// The direction the route leaves each end in (a unit vector along an axis; zero
    /// where it may leave any way): it runs straight out of the end, as far as half its
    /// bend radius where it can, before it bends.
    pub leave: [V3; 2],
    /// How far the route runs straight already before each end point (out of the
    /// nozzle of a device), mm.
    pub out: [f32; 2],
    /// Cross-section in mm² (capacity of the paths).
    pub area: f32,
    /// Extra cost of a turn (mm), from the bend radius.
    pub turn: f32,
    /// Half the outer diameter: the room the route keeps from what blocks the cells.
    pub half: f32,
}

/// A way through the grid.
#[derive(Clone, Debug, PartialEq)]
pub struct Way {
    /// Corner points in layout coordinates, from the start to the end point.
    pub points: Vec<V3>,
    pub cells: Vec<usize>,
    /// Length outside cable ducts and routing paths.
    pub free: f32,
}

/// Where a run ends and what its cells cost, `None` where it is blocked.
type RunEnd = Option<(usize, f32)>;

/// Where a search begins and ends: the cells from the one the route leaves its start
/// in to the one the search begins at (straight out of the start), the direction it
/// heads there (6: none), the cells from the one the search ends at to the one the
/// route arrives at its end in (straight into the end) and the direction it heads
/// along them (6: none).
struct Ends {
    lead_in: Vec<usize>,
    dir: usize,
    lead_out: Vec<usize>,
    arrive: usize,
}

pub struct Space {
    pub origin: V3,
    pub cell: f32,
    pub n: [usize; 3],
    /// The clearance the items were grown by ([`RoutingRules::clearance`]).
    clearance: f32,
    block: Vec<u8>,
    owner: Vec<u64>,
    /// Inner size (mm) of the cable duct a cell lies in, 0 for none.
    duct: Vec<f32>,
    /// The cable duct of a cabinet a cell lies in (index + 1 into `ducts`, 0 for none).
    duct_of: Vec<u16>,
    ducts: Vec<DuctBox>,
    /// Room (mm) from the centre of each cell outside the cable ducts of the cabinets
    /// to the nearest of them (index + 1, 0 for none within reach); empty until
    /// [`Space::measure`].
    duct_room: Vec<(f32, u16)>,
    /// Stretch of a cable duct a cell lies in: index + 1 into `sections`, 0 for none.
    section: Vec<u32>,
    sections: Vec<Section>,
    /// What blocks the space, until [`Space::measure`].
    solids: Vec<Solid>,
    /// The boxes of `solids` (indices) within [`SOLID_REACH`] of each coarse cell of
    /// [`SOLID_CELL`] mm, for the room at points between the centres of the cells;
    /// filled by [`Space::measure`].
    solid_cells: HashMap<[i32; 3], Vec<u32>>,
    /// Room (mm) from the centre of each cell to the nearest solid, which item that is
    /// (index + 1 into `owners`, 0 for none) and the room to the nearest solid of
    /// another item; empty until [`Space::measure`]: room everywhere.
    room: Vec<f32>,
    room_owner: Vec<u32>,
    room_other: Vec<f32>,
    owners: Vec<u64>,
    /// How far the cables and tubes routed through the space reach into each cell: the
    /// largest half thickness less the distance from its middle line (see
    /// [`Space::take`]).
    taken: Vec<f32>,
    /// Every route that took the cell was near one of its own ends there: routes
    /// converge there with the routes to the same end ([`Space::apart`]).
    taken_end: Vec<bool>,
    /// The axes (a bit each) the routes that took the cell run along there.
    taken_along: Vec<u8>,
    /// The item at the end the routes that took the cell were near there (0: none,
    /// `u64::MAX`: more than one).
    taken_item: Vec<u64>,
    /// The cell lies on the straight way out of an end of a route ([`Space::keep_ways_out`]):
    /// only routes near one of their own ends pass it.
    way_out: Vec<bool>,
    /// Routing path of a cell: index + 1 into `paths`, 0 for none.
    path: Vec<u16>,
    /// Classes a reservation keeps the cell for (bits); 0: any.
    reserved: Vec<u8>,
    /// Classes routed through the cell.
    occ: Vec<u8>,
    /// Classes routed within the segregation distance.
    zone: Vec<u8>,
    pub paths: Vec<PathCells>,
    crossings: Vec<Crossing>,
    /// Enclosure of each cell column along x (index into the layout), `usize::MAX`
    /// outside.
    column: Vec<usize>,
    /// How far (in cells) each cell of a room lies from its surfaces, `open_cells` + 1
    /// at most (see [`Space::mark_open`]); empty in a line-up.
    open: Vec<u8>,
    open_cells: u32,
}

thread_local! {
    /// The arrays of the searches on this thread, kept from one search to the next
    /// until [`release_search`].
    static SEARCH: RefCell<Search> = RefCell::default();
}

/// Frees the arrays the searches on this thread kept.
pub fn release_search() {
    SEARCH.with_borrow_mut(|s| *s = Search::default());
}

/// The arrays of the search over all states (cell × direction) and, for the costs to
/// the goal, over all cells; they grow to the largest space searched. An entry belongs
/// to the current search only when its stamp is the search's.
#[derive(Default)]
struct Search {
    best: Vec<f32>,
    prev: Vec<u32>,
    /// The state each state's way last turned at (where the stretch it lies on begins;
    /// `u32::MAX`: on the way out of the start), kept with `prev`.
    corner: Vec<u32>,
    /// The states expanded (a bit each), for a search that does not take them up again.
    closed: Vec<u64>,
    stamp: Vec<u32>,
    /// Cost from the cell to the goal without turns, final where `settled`.
    to_goal: Vec<f32>,
    reached: Vec<u32>,
    settled: Vec<u32>,
    /// The run out of each cell in each direction (cell × 6): where it ends
    /// (`u32::MAX`: blocked) and what it costs, where `run_stamp` is the search's.
    runs: Vec<(u32, f32)>,
    run_stamp: Vec<u32>,
    /// What a step into each cell costs for the request, up or down and along the
    /// other axes, and how it can be passed ([`NO`]: not at all), where `pass_stamp`
    /// is the search's.
    pass: Vec<(f32, f32, u8)>,
    pass_stamp: Vec<u32>,
    /// Which turns at each cell are known (bit from × 6 + to) and which of them have
    /// room for their bend, where `turn_stamp` is the search's.
    turn_known: Vec<u64>,
    turn_ok: Vec<u64>,
    turn_stamp: Vec<u32>,
    /// Which lane changes (cell, direction come in, direction aside, cells aside) have
    /// room for their S-bend.
    jogs: HashMap<(u32, u8, u8, u8), bool>,
    run: u32,
}

/// The memos of a search: what the steps into the cells cost, the runs out of them and
/// the turns in them.
struct Memo<'a> {
    pass: &'a mut [(f32, f32, u8)],
    pass_stamp: &'a mut [u32],
    runs: &'a mut [(u32, f32)],
    run_stamp: &'a mut [u32],
    turn_known: &'a mut [u64],
    turn_ok: &'a mut [u64],
    turn_stamp: &'a mut [u32],
    jogs: &'a mut HashMap<(u32, u8, u8, u8), bool>,
    run: u32,
}

/// The way of a search to the state it goes on from ([`Space::way_to`]): its corner
/// points and the boxes round its stretches and bends that a way on from it may come
/// back to.
#[derive(Default)]
struct OwnWay {
    points: Vec<V3>,
    boxes: Vec<(V3, V3)>,
    known: bool,
}

impl Search {
    /// Starts a search over `cells` cells.
    fn start(&mut self, cells: usize) -> u32 {
        if self.reached.len() < cells {
            *self = Search {
                best: vec![f32::INFINITY; cells * 7],
                prev: vec![u32::MAX; cells * 7],
                corner: vec![u32::MAX; cells * 7],
                closed: vec![0; (cells * 7).div_ceil(64)],
                stamp: vec![0; cells * 7],
                to_goal: vec![f32::INFINITY; cells],
                reached: vec![0; cells],
                settled: vec![0; cells],
                runs: vec![(u32::MAX, 0.0); cells * 6],
                run_stamp: vec![0; cells * 6],
                pass: vec![(-1.0, -1.0, NO); cells],
                pass_stamp: vec![0; cells],
                turn_known: vec![0; cells],
                turn_ok: vec![0; cells],
                turn_stamp: vec![0; cells],
                jogs: HashMap::new(),
                run: 0,
            };
        }
        self.jogs.clear();
        self.run = self.run.wrapping_add(1);
        if self.run == 0 {
            for v in [&mut self.stamp, &mut self.reached, &mut self.settled, &mut self.run_stamp, &mut self.pass_stamp, &mut self.turn_stamp] {
                v.fill(0);
            }
            self.run = 1;
        }
        self.run
    }
}

impl Space {
    /// A grid over the box from `min` to `max` with every cell blocked, with cells of
    /// `cell` mm (coarser for large boxes); its columns belong to no enclosure.
    pub(super) fn empty(min: V3, max: V3, cell: f32) -> Space {
        let size = [0, 1, 2].map(|i| (max[i] - min[i]).max(1.0));
        let mut cell = cell.max(5.0);
        while size.iter().map(|s| (s / cell).ceil() as usize).product::<usize>() > MAX_CELLS {
            cell *= 1.25;
        }
        let n = size.map(|s| (s / cell).ceil().max(1.0) as usize);
        let total = n[0] * n[1] * n[2];
        Space {
            origin: min,
            cell,
            n,
            clearance: 0.0,
            block: vec![WALL; total],
            owner: vec![0; total],
            duct: vec![0.0; total],
            duct_of: vec![0; total],
            ducts: Vec::new(),
            duct_room: Vec::new(),
            solids: Vec::new(),
            solid_cells: HashMap::new(),
            section: vec![0; total],
            sections: Vec::new(),
            room: Vec::new(),
            room_owner: Vec::new(),
            room_other: Vec::new(),
            owners: Vec::new(),
            taken: vec![f32::NEG_INFINITY; total],
            taken_end: vec![true; total],
            taken_along: vec![0; total],
            taken_item: vec![0; total],
            way_out: vec![false; total],
            path: vec![0; total],
            reserved: vec![0; total],
            occ: vec![0; total],
            zone: vec![0; total],
            paths: Vec::new(),
            crossings: Vec::new(),
            column: vec![usize::MAX; n[0]],
            open: Vec::new(),
            open_cells: 0,
        }
    }

    /// Measures how far (in cells) the free cells lie from the walls, the ceiling (the
    /// top of the grid) and the items standing or hanging in the room, up to `distance`:
    /// running through the room horizontally costs the more the farther from them (see
    /// [`Space::open_factor`]), so pipes and cables keep to the walls and the ceiling
    /// and drop straight to a device standing in the room.
    pub(super) fn mark_open(&mut self, distance: f32) {
        let k = ((distance / self.cell).ceil() as u32).clamp(1, 200);
        let total = self.block.len();
        let mut dist = vec![u32::MAX; total];
        let mut queue = std::collections::VecDeque::new();
        for (idx, d) in dist.iter_mut().enumerate() {
            if self.block[idx] != 0 || self.coords(idx)[1] == 0 {
                *d = 0;
                queue.push_back(idx);
            }
        }
        while let Some(idx) = queue.pop_front() {
            let d = dist[idx];
            if d >= k {
                continue;
            }
            for nd in 0..6 {
                let Some(ni) = self.step(idx, nd) else { continue };
                if dist[ni] == u32::MAX {
                    dist[ni] = d + 1;
                    queue.push_back(ni);
                }
            }
        }
        self.open = dist.into_iter().map(|d| d.min(k + 1) as u8).collect();
        self.open_cells = k;
    }

    /// The grid of a line-up (`pos`: [`panel::positions`]).
    pub fn build(prj: &Project, lineup: &[usize], pos: &[V3], rules: &RoutingRules) -> Space {
        let encs = &prj.layout.enclosures;
        let (mut min, mut max) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
        for &k in lineup {
            let e = &encs[k];
            let far = [pos[k][0] + e.width, pos[k][1] + e.height, pos[k][2] + e.depth];
            for i in 0..3 {
                min[i] = min[i].min(pos[k][i]);
                max[i] = max[i].max(far[i]);
            }
        }
        if lineup.is_empty() {
            (min, max) = ([0.0; 3], [1.0; 3]);
        }
        let mut s = Space::empty(min, max, rules.grid);
        s.clearance = rules.clearance.max(0.0);
        let (n, total, cell) = (s.n, s.block.len(), s.cell);
        for i in 0..n[0] {
            let x = s.origin[0] + (i as f32 + 0.5) * cell;
            s.column[i] = lineup.iter().copied().find(|&k| x >= pos[k][0] && x < pos[k][0] + encs[k].width).unwrap_or(usize::MAX);
        }
        // Inside the enclosures: in front of the mounting plate, behind the door.
        for idx in 0..total {
            let c = s.centre(idx);
            let k = s.column[s.coords(idx)[0]];
            if k == usize::MAX {
                continue;
            }
            let e = &encs[k];
            let l = [c[0] - pos[k][0], c[1] - pos[k][1], c[2] - pos[k][2]];
            let t = if e.is_assembly() {0.} else {e.wall()};
            let back=if e.no_implicit_plate {t} else {e.mounting_z().max(t)};
            if l[0] > t && l[0] < e.width - t && l[1] > t && l[1] < e.height - t && l[2] > back && l[2] < e.depth - t {
                s.block[idx] = 0;
            }
        }
        for &k in lineup {
            s.add_enclosure(&encs[k], k, pos[k], rules, prj.layout.duct_fill_limit, &item_parts(prj, &encs[k]));
            if panel::is_rack(&encs[k]) {
                s.add_rack_ways(&encs[k], k, pos[k]);
            }
        }
        let mut passages: Vec<panel::Passage> = Vec::new();
        for w in lineup.windows(2) {
            for x in panel::passages(&prj.layout, pos, w[0]) {
                let (y, z) = ([x.at[1] - x.half[0], x.at[1] + x.half[0]], [x.at[2] - x.half[1], x.at[2] + x.half[1]]);
                s.crossings.push(Crossing { x: x.at[0], y, z });
                passages.push(x);
            }
        }
        for &k in lineup {
            s.add_walls(&encs[k], pos[k], &passages);
        }
        s.measure();
        s
    }

    /// Adds what is in enclosure `k` (at `o`, enclosure coordinates + `o` = grid
    /// coordinates) to the free space: its items (those of `parts` as their parts,
    /// each a solid of its own: [`item_parts`]), reservations and routing paths.
    /// Cable ducts are channels on the mounting plate, in a room on every face, filled
    /// up to `limit` percent of their inside.
    pub(super) fn add_enclosure(&mut self, e: &Enclosure, k: usize, o: V3, rules: &RoutingRules, limit: f32, parts: &HashMap<u64, Vec<PanelItem>>) {
        let shift = |(a, b): (V3, V3), m: f32| ([a[0] + o[0] - m, a[1] + o[1] - m, a[2] + o[2] - m], [b[0] + o[0] + m, b[1] + o[1] + m, b[2] + o[2] + m]);
        for it in &e.items {
            let bx = shift(panel::item_box(e, it), 0.0);
            match it.kind {
                ItemKind::Device | ItemKind::Rail | ItemKind::Busbar | ItemKind::Accessory => {
                    let pieces: Vec<(u64, (V3, V3))> = match parts.get(&it.id) {
                        Some(subs) => subs.iter().enumerate().map(|(n, sub)| (super::part_id(it.id, n), panel::item_box(e, sub))).collect(),
                        None => vec![(it.id, panel::item_box(e, it))],
                    };
                    for (owner, piece) in pieces {
                        let (bx, around) = (shift(piece, 0.0), shift(piece, rules.clearance));
                        for idx in self.cells_in(self.widened(around)) {
                            if self.block[idx] == 0 {
                                self.block[idx] = ITEM;
                                self.owner[idx] = owner;
                            }
                        }
                        for idx in self.cells_in(bx) {
                            if self.block[idx] == ITEM && self.owner[idx] == owner {
                                self.block[idx] = BODY;
                            }
                        }
                        self.solids.push(Solid::Box { lo: around.0, hi: around.1, owner });
                    }
                }
                ItemKind::Duct if it.face == panel::Face::Plate || e.is_room() => {
                    // Its inside: across and as deep as it is, less its walls.
                    let size = (it.w.min(it.h).min(it.d) - 4.0).max(1.0);
                    let cells = self.cells_in(self.widened(bx));
                    cells.iter().for_each(|&idx| self.duct[idx] = size);
                    // Its stretches along it, one cell long, for the fill limit (the cells
                    // inside it, not those beside it that reach into it).
                    let inner = self.cells_in(bx);
                    let axis = (0..3).max_by(|&a, &b| (bx.1[a] - bx.0[a]).total_cmp(&(bx.1[b] - bx.0[b]))).unwrap_or(0);
                    let first = inner.iter().map(|&i| self.coords(i)[axis]).min().unwrap_or(0);
                    let last = inner.iter().map(|&i| self.coords(i)[axis]).max().unwrap_or(0);
                    let base = self.sections.len();
                    let capacity = panel::duct_capacity(it) * limit / 100.0;
                    self.sections.extend((first..=last).map(|_| Section { capacity, used: 0.0 }));
                    for idx in inner {
                        self.section[idx] = (base + self.coords(idx)[axis] - first) as u32 + 1;
                    }
                    // In a cabinet its walls bound the routes outside it and the ways
                    // into it (a tray in a room is open).
                    if !e.is_room() {
                        let (rim, top, base) = panel::duct_walls(it);
                        self.ducts.push(DuctBox { lo: bx.0, hi: bx.1, axis: usize::from(it.w < it.h), base, rim, top });
                        let mark = self.ducts.len() as u16;
                        cells.iter().for_each(|&idx| self.duct_of[idx] = mark);
                    }
                }
                _ => {}
            }
        }
        for r in &e.reservations {
            let bx = shift((r.min, r.max), 0.0);
            match r.mode {
                ReserveMode::Keepout => self.block_box(bx),
                ReserveMode::For(class) => {
                    for idx in self.cells_in(bx) {
                        self.reserved[idx] |= class.bit();
                    }
                }
            }
        }
        for p in &e.paths {
            let pts: Vec<V3> = p.points.iter().map(|q| [q[0] + o[0], q[1] + o[1], q[2] + o[2]]).collect();
            if pts.len() < 2 {
                continue;
            }
            self.paths.push(PathCells {
                enclosure: k,
                id: p.id,
                kind: p.kind,
                classes: p.classes.iter().fold(0, |m, c| m | c.bit()),
                capacity: p.capacity(),
                used: 0.0,
                items: 0,
                implied: false,
            });
            let mark = self.paths.len() as u16;
            // At least the cells around the centre line.
            let r = (p.width / 2.0).max(self.cell * 0.75);
            for idx in self.cells_near(&pts, r) {
                if self.path[idx] == 0 {
                    self.path[idx] = mark;
                }
            }
        }
    }

    /// The cable ways of a 19-inch rack `e` (at `o`): the space beside its mounting
    /// rails from its front to its rear is a channel the routes run along, leaving it
    /// for the devices in the rack; the rails themselves block the space.
    fn add_rack_ways(&mut self, e: &Enclosure, k: usize, o: V3) {
        let rails = panel::rack::rail_boxes(e);
        let (l, r) = (rails.iter().map(|b| b.0[0]).fold(f32::INFINITY, f32::min), rails.iter().map(|b| b.1[0]).fold(f32::NEG_INFINITY, f32::max));
        for (lo, hi) in rails {
            self.block_box((add(lo, o), add(hi, o)));
        }
        let t = e.wall();
        for (x0, x1) in [(t, l), (r, e.width - t)] {
            if x1 <= x0 {
                continue;
            }
            self.paths.push(PathCells { enclosure: k, id: 0, kind: PathKind::Any, classes: 0, capacity: 0.0, used: 0.0, items: 0, implied: true });
            let mark = self.paths.len() as u16;
            for idx in self.cells_in((add(o, [x0, t, PLATE_Z]), add(o, [x1, e.height - t, e.depth - t]))) {
                if self.path[idx] == 0 {
                    self.path[idx] = mark;
                }
            }
        }
    }

    /// The walls of enclosure `e` (at `o`): its sides, roof and bottom, the mounting
    /// plate with the rear wall and the door, the sides open at the cable `passages` to
    /// the bayed neighbours.
    fn add_walls(&mut self, e: &Enclosure, o: V3, passages: &[panel::Passage]) {
        if e.is_assembly() {return;}
        let (w, h, d, t) = (e.width, e.height, e.depth, e.wall());
        let far = add(o, [w, h, d]);
        let sides = [((o, [o[0] + t, far[1], far[2]]), o[0]), (([far[0] - t, o[1], o[2]], far), far[0])];
        for (bx, x) in sides {
            let openings: Vec<([f32; 2], [f32; 2])> = passages
                .iter()
                .filter(|p| (p.at[0] - x).abs() < 1.0)
                .map(|p| ([p.at[1] - p.half[0], p.at[1] + p.half[0]], [p.at[2] - p.half[1], p.at[2] + p.half[1]]))
                .collect();
            for (lo, hi) in cut_openings(bx, &openings) {
                self.solids.push(Solid::Box { lo, hi, owner: 0 });
            }
        }
        let back=if e.no_implicit_plate {t} else {e.mounting_z().max(t)};
        for (lo, hi) in [(o, [far[0], o[1] + t, far[2]]), ([o[0], far[1] - t, o[2]], far), (o, [far[0], far[1], o[2] + back]), ([o[0], o[1], far[2] - t], far)] {
            self.solids.push(Solid::Box { lo, hi, owner: 0 });
        }
    }

    /// Frees the cells whose centre passes the test, and lets their columns belong to
    /// enclosure `k`.
    pub(super) fn free_where(&mut self, k: usize, inside: impl Fn(V3) -> bool) {
        for idx in 0..self.block.len() {
            if inside(self.centre(idx)) {
                self.block[idx] = 0;
                let x = self.coords(idx)[0];
                self.column[x] = k;
            }
        }
    }

    /// The walls of a room with the floor `plan` (at `origin`, `height` high, y down
    /// from the ceiling), its ceiling and its floor, to measure the room from.
    pub(super) fn add_room(&mut self, plan: Vec<[f32; 2]>, origin: V3, height: f32) {
        self.solids.push(Solid::Room { plan, origin, height });
    }

    /// How far the room around the cells is measured.
    fn cap(&self) -> f32 {
        ROOM_CAP.max(ROOM_CELLS * self.cell)
    }

    /// Measures the room around the cells (once the space is complete): from the
    /// centre of each cell that is not a wall to the nearest of the solids, up to
    /// [`Space::cap`], and to the nearest one of another item.
    pub(super) fn measure(&mut self) {
        let total = self.block.len();
        let cap = self.cap();
        self.room = vec![cap; total];
        self.room_owner = vec![0; total];
        self.room_other = vec![cap; total];
        let solids = std::mem::take(&mut self.solids);
        for solid in &solids {
            match solid {
                Solid::Box { lo, hi, owner } => {
                    let key = match *owner {
                        0 => 0,
                        id => match self.owners.iter().position(|&o| o == id) {
                            Some(i) => i as u32 + 1,
                            None => {
                                self.owners.push(id);
                                self.owners.len() as u32
                            }
                        },
                    };
                    for idx in self.cells_in((lo.map(|v| v - cap), hi.map(|v| v + cap))) {
                        if self.block[idx] != WALL {
                            let d = box_distance(self.centre(idx), *lo, *hi);
                            self.note_room(idx, d, key);
                        }
                    }
                }
                Solid::Room { plan, origin, height } => {
                    for idx in 0..total {
                        if self.block[idx] == WALL {
                            continue;
                        }
                        let c = sub(self.centre(idx), *origin);
                        let d = c[1].min(height - c[1]).min(plan_distance(plan, [c[0], c[2]]));
                        self.note_room(idx, d.max(0.0), 0);
                    }
                }
            }
        }
        self.solids = solids;
        self.solid_cells.clear();
        for (i, solid) in self.solids.iter().enumerate() {
            if let Solid::Box { lo, hi, .. } = solid {
                let key = |v: f32| (v / SOLID_CELL).floor() as i32;
                let (a, b) = (lo.map(|v| key(v - SOLID_REACH)), hi.map(|v| key(v + SOLID_REACH)));
                for x in a[0]..=b[0] {
                    for y in a[1]..=b[1] {
                        for z in a[2]..=b[2] {
                            self.solid_cells.entry([x, y, z]).or_default().push(i as u32);
                        }
                    }
                }
            }
        }
        self.duct_room = vec![(cap, 0); total];
        for k in 0..self.ducts.len() {
            let (lo, hi) = (self.ducts[k].lo, self.ducts[k].hi);
            for idx in self.cells_in((lo.map(|v| v - cap), hi.map(|v| v + cap))) {
                let d = box_distance(self.centre(idx), lo, hi);
                if self.block[idx] != WALL && self.duct_of[idx] == 0 && d < self.duct_room[idx].0 {
                    self.duct_room[idx] = (d, k as u16 + 1);
                }
            }
        }
    }

    /// A solid of the item `key` lies `d` mm from the centre of a cell.
    fn note_room(&mut self, idx: usize, d: f32, key: u32) {
        if d < self.room[idx] {
            if key != self.room_owner[idx] {
                self.room_other[idx] = self.room[idx];
            }
            self.room[idx] = d;
            self.room_owner[idx] = key;
        } else if key != self.room_owner[idx] && d < self.room_other[idx] {
            self.room_other[idx] = d;
        }
    }

    /// Room (mm) from the centre of a cell to the nearest solid but those of the items
    /// `ignore`.
    fn room_for(&self, idx: usize, ignore: &[u64]) -> f32 {
        let Some(&room) = self.room.get(idx) else { return f32::INFINITY };
        match self.room_owner[idx] {
            0 => room,
            k if ignore.contains(&self.owners[k as usize - 1]) => self.room_other[idx],
            _ => room,
        }
    }

    /// The cell lies near an end of the request: within a few cells of it, or as far as
    /// the route runs straight out of it (its bend radius) and changes lanes into that
    /// (another radius, a cell to spare) at least: there routes to one end converge.
    fn near_end(&self, idx: usize, req: &Request) -> bool {
        self.near_point(self.centre(idx), req)
    }

    fn near_point(&self, p: V3, req: &Request) -> bool {
        let zone = self.end_zone(req);
        req.at.iter().any(|q| dist(p, *q) <= zone)
    }

    /// How far from its ends a request converges with the routes to the same end (and
    /// the clearance round the items there does not count): the straight way out and
    /// a bend, at least three cells.
    pub fn end_zone(&self, req: &Request) -> f32 {
        (3.0 * self.cell).max(2.0 * req.turn + self.cell) + req.half
    }

    /// The point `p` (between the centres of the cells) has half the request's thickness
    /// of room from every solid (but the items it ignores; near an end the clearance
    /// round an item does not count), worked out exactly from the solids near it; `None`
    /// for a request thicker than they are listed for.
    fn point_clear(&self, p: V3, req: &Request) -> Option<bool> {
        if req.half > SOLID_REACH {
            return None;
        }
        let near_end = self.near_point(p, req);
        let key = p.map(|v| (v / SOLID_CELL).floor() as i32);
        for &i in self.solid_cells.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
            let Solid::Box { lo, hi, owner } = &self.solids[i as usize] else { continue };
            if *owner != 0 && req.ignore.contains(owner) {
                continue;
            }
            let need = if *owner != 0 && near_end { req.half - self.clearance } else { req.half };
            if box_distance(p, *lo, *hi) < need {
                return Some(false);
            }
        }
        for solid in &self.solids {
            if let Solid::Room { plan, origin, height } = solid {
                let c = sub(p, *origin);
                if c[1].min(height - c[1]).min(plan_distance(plan, [c[0], c[2]])) < req.half {
                    return Some(false);
                }
            }
        }
        Some(true)
    }

    /// The cell has room for the request: half its thickness from the nearest solid
    /// (but the items it ignores), less `off`, how far the route runs from the cell's
    /// centre. Near an end the clearance round an item does not count (the route comes
    /// up to the terminals there), the item itself does.
    fn clear(&self, idx: usize, off: f32, req: &Request) -> bool {
        let room = self.room_for(idx, req.ignore) - off;
        room >= req.half || (self.room_owner.get(idx).is_some_and(|&o| o != 0) && self.near_end(idx, req) && room + self.clearance >= req.half)
    }

    /// The request keeps apart from the routes before it in the cell (in a cable duct
    /// or routing path they lie side by side, as far as it holds them; where it comes
    /// up to its ends it need not, where those routes were near their ends too: they
    /// converge on one gland or terminal) and off the ways out of the ends of the other
    /// routes ([`Space::keep_ways_out`]), which only a route out of an end beside them
    /// shares (in the wide cells of a room: any route near one of its ends).
    fn apart(&self, idx: usize, req: &Request) -> bool {
        if self.channel(idx, req) {
            return true;
        }
        let near = self.near_end(idx, req);
        if self.way_out[idx] && !self.way_out_of(idx, req) && (!near || self.cell < super::room::ROOM_GRID) {
            return false;
        }
        self.taken[idx] + req.half + 0.5 <= 0.0 || (near && self.taken_end[idx])
    }

    /// The cell lies on the way out of an end of the request (a cell beside it at most:
    /// the terminals of a row lie closer than the cells); round an end without a
    /// direction within two cells.
    fn way_out_of(&self, idx: usize, req: &Request) -> bool {
        let c = self.centre(idx);
        (0..2).any(|i| {
            let q = req.at[i];
            match Space::axis_of(req.leave[i]) {
                Some((axis, sign)) => (c[axis] - q[axis]) * sign > -self.cell && (0..3).filter(|&k| k != axis).all(|k| (c[k] - q[k]).abs() <= self.cell),
                None => dist(c, q) <= 2.0 * self.cell,
            }
        })
    }

    /// Stepping into the cell in direction `dir`, the request would run across the
    /// routes that took it (along an axis none of them runs along there): routes that
    /// converge on a device lie side by side, they do not run through each other; only
    /// on its own way out of an end, or where it comes up to the device another route
    /// ends at, it may cross ([`Space::crosses`], at a price). (Not in the wide cells
    /// of a room: two routes lie side by side in one of them.)
    fn across(&self, idx: usize, dir: usize, req: &Request) -> bool {
        self.crosses(idx, dir, req) && !req.ignore.contains(&self.taken_item[idx]) && !self.way_out_of(idx, req)
    }

    /// Stepping into the cell in direction `dir`, the request runs across the routes
    /// that took it.
    fn crosses(&self, idx: usize, dir: usize, req: &Request) -> bool {
        self.cell < super::room::ROOM_GRID && self.taken_along[idx] != 0 && self.taken_along[idx] & (1 << (dir / 2)) == 0 && self.taken[idx] + req.half + 0.5 > 0.0 && !self.channel(idx, req)
    }

    /// Keeps the straight ways out of the ends `a` and `b` of a route still to be
    /// searched (the cells it leaves them through, as far as its bend radius) free for
    /// the routes that end there: no other one runs across the terminal of a device or
    /// in front of a cable gland before the route to it is laid.
    pub fn keep_ways_out(&mut self, a: V3, b: V3, req: &Request) {
        let far = req.turn;
        let cells = (far / self.cell).ceil() as usize;
        for (i, (q, other)) in [(a, b), (b, a)].into_iter().enumerate() {
            let limit = self.lead_limit(q, req.leave[i], other, req.leave[1 - i]);
            if let Some((way, ..)) = self.lead(q, req.leave[i], limit, req.out[i], far, req, cells) {
                for idx in way {
                    self.way_out[idx] = true;
                }
            }
        }
    }

    /// A route of half thickness `half` along `points`: the routes after it keep apart
    /// from it, up to `reach` mm beyond its surface (half the thickness of the thickest
    /// of them), but near their ends where it was near one of its own
    /// (`ends`: the end points and how far from them, [`Space::end_zone`]; `items`: the
    /// items at its ends).
    pub fn take(&mut self, points: &[V3], half: f32, reach: f32, ends: &[(V3, f32)], items: &[u64]) {
        for idx in self.cells_near(points, half + reach) {
            let c = self.centre(idx);
            let d = points.windows(2).map(|s| dist_to_segment(c, s[0], s[1])).fold(f32::INFINITY, f32::min);
            self.taken[idx] = self.taken[idx].max(half - d);
            if ends.iter().any(|(q, zone)| dist(c, *q) <= *zone) {
                let item = items.first().copied().unwrap_or(u64::MAX);
                self.taken_item[idx] = match self.taken_item[idx] {
                    0 => item,
                    was if items.contains(&was) => was,
                    _ => u64::MAX,
                };
            } else {
                self.taken_end[idx] = false;
            }
            // The axes of the stretches it lies nearest to.
            for s in points.windows(2).filter(|s| dist_to_segment(c, s[0], s[1]) <= d + 0.5 * self.cell) {
                if let Some((axis, _)) = Space::axis_of(sub(s[1], s[0])) {
                    self.taken_along[idx] |= 1 << axis;
                }
            }
        }
    }

    /// The cells of a way found before other routes took their room are still apart from
    /// them.
    pub fn still_apart(&self, cells: &[usize], req: &Request) -> bool {
        cells.iter().all(|&i| self.apart(i, req))
    }

    /// A bend of radius `radius` rounding the corner at `corner` from the direction
    /// `from` into `to` keeps half the request's thickness from what blocks the space
    /// (but the items it ignores): every cell its arc runs through is passable with that
    /// much room (less how far the arc runs from the cell's centre), and where the arc
    /// passes the wall of a cable duct it does so as a route may ([`Space::duct_pass`]).
    fn bend_room(&self, corner: V3, from: V3, to: V3, radius: f32, req: &Request) -> bool {
        self.arc_room(corner, from, to, radius, std::f32::consts::FRAC_PI_2, req)
    }

    /// [`Space::bend_room`] for the first `turn` (radians) of the bend.
    fn arc_room(&self, corner: V3, from: V3, to: V3, radius: f32, turn: f32, req: &Request) -> bool {
        let n = ((radius * turn) / (self.cell * 0.5)).ceil().max(2.0) as usize;
        let mut last: Option<(usize, V3)> = None;
        (0..=n).all(|k| {
            let phi = turn * k as f32 / n as f32;
            let p = add(corner, add(scale(from, radius * (phi.sin() - 1.0)), scale(to, radius * (1.0 - phi.cos()))));
            self.point_room(p, &mut last, req)
        })
    }

    /// A lane change from the cell `idx`, come to in direction `dir`: `cells` cells
    /// across in direction `aside` and back into `dir`, drawn as an S-bend of two arcs
    /// of the bend radius (two quarter turns where it is wide enough for them), keeps
    /// half the request's thickness from what blocks the space, like [`Space::bend_room`].
    fn jog_room(&self, idx: usize, dir: usize, aside: usize, cells: usize, req: &Request) -> bool {
        let (u, v) = (unit(dir), unit(aside));
        let (c1, r) = (self.centre(idx), req.turn);
        let lb = cells as f32 * self.cell;
        if lb >= 2.0 * r {
            return self.bend_room(c1, u, v, r, req) && self.bend_room(add(c1, scale(v, lb)), v, u, r, req);
        }
        // Arcs of angle θ: across 2 r (1 − cos θ) = lb, along r sin θ on each side; the
        // second arc is the first turned round the middle of the jog.
        let theta = (1.0 - lb / (2.0 * r)).acos();
        let start = sub(c1, scale(u, r * theta.sin()));
        let centre = add(start, scale(v, r));
        let middle = add(c1, scale(v, lb / 2.0));
        let n = ((r * theta) / (self.cell * 0.5)).ceil().max(2.0) as usize;
        let first: Vec<V3> = (0..=n).map(|k| theta * k as f32 / n as f32).map(|phi| add(centre, add(scale(v, -r * phi.cos()), scale(u, r * phi.sin())))).collect();
        let second: Vec<V3> = first.iter().rev().skip(1).map(|q| sub(scale(middle, 2.0), *q)).collect();
        let mut last: Option<(usize, V3)> = None;
        first.into_iter().chain(second).all(|p| self.point_room(p, &mut last, req))
    }

    /// A point of a bend (after the one in `last`, which it becomes) has half the
    /// request's thickness of room from what blocks the space: its cell is passable
    /// (but for the item boxes the room is measured from), the room at the point itself
    /// where it can be worked out, otherwise that of its cell less how far the point
    /// lies from the cell's centre, and from the cell before it the bend passes the walls
    /// of the ducts as a route may.
    fn point_room(&self, p: V3, last: &mut Option<(usize, V3)>, req: &Request) -> bool {
        let Some(m) = self.cell_of(p) else { return false };
        let barred = self.block[m] == WALL || (self.reserved[m] != 0 && self.reserved[m] & req.class.bit() == 0) || (req.kind != RouteKind::Wire && self.duct[m] > 0.0 && self.duct[m] < 2.0 * req.half) || !self.apart(m, req);
        if barred {
            return false;
        }
        let room = match self.point_clear(p, req) {
            Some(ok) => ok,
            None => self.passable(m, req) && self.clear(m, dist(p, self.centre(m)), req),
        };
        if !room {
            return false;
        }
        if let Some((l, q)) = last.filter(|(l, _)| *l != m) {
            if !self.duct_pass(l, q, m, p, req) {
                return false;
            }
        }
        *last = Some((m, p));
        true
    }

    /// A route may pass from the point `p` in cell `a` to the point `q` in cell `b` as
    /// far as the walls of the cable ducts go: not into or out of a duct through its base
    /// or its cover (the point outside lies over or under the duct), through a wall only
    /// with half its thickness between the wall's rim and its top.
    fn duct_pass(&self, a: usize, p: V3, b: usize, q: V3, req: &Request) -> bool {
        let (da, db) = (self.duct_of[a], self.duct_of[b]);
        if da == db || (da != 0 && db != 0) {
            return true;
        }
        let d = &self.ducts[da.max(db) as usize - 1];
        let outside = if da == 0 { p } else { q };
        let over = (0..2).all(|k| outside[k] > d.lo[k] + 1e-3 && outside[k] < d.hi[k] - 1e-3);
        let z = outside[2] - d.lo[2];
        !over && self.in_band(z, d.rim, d.top, req.half)
    }

    /// Blocks the cells whose centre lies in a box (walls, cabinets, door swings,
    /// keep-outs), which the room is measured from as well.
    pub(super) fn block_box(&mut self, bx: (V3, V3)) {
        for idx in self.cells_in(self.widened(bx)) {
            self.block[idx] = WALL;
        }
        self.solids.push(Solid::Box { lo: bx.0, hi: bx.1, owner: 0 });
    }

    /// The box widened to at least a cell along every axis (about its middle), so that
    /// it covers the centre of a cell along each: a way between the centres of cells it
    /// does not cover never crosses it.
    fn widened(&self, (mut lo, mut hi): (V3, V3)) -> (V3, V3) {
        for k in 0..3 {
            let lack = self.cell - (hi[k] - lo[k]);
            if lack > 0.0 {
                lo[k] -= lack / 2.0;
                hi[k] += lack / 2.0;
            }
        }
        (lo, hi)
    }

    /// The cell a point lies in is blocked by a wall or an item (or outside the grid).
    #[cfg(test)]
    pub(super) fn blocked(&self, q: V3) -> bool {
        self.cell_of(q).is_none_or(|i| self.block[i] != 0)
    }

    pub fn coords(&self, idx: usize) -> [usize; 3] {
        [idx % self.n[0], (idx / self.n[0]) % self.n[1], idx / (self.n[0] * self.n[1])]
    }

    fn index(&self, c: [usize; 3]) -> usize {
        c[0] + self.n[0] * (c[1] + self.n[1] * c[2])
    }

    pub fn centre(&self, idx: usize) -> V3 {
        let c = self.coords(idx);
        [0, 1, 2].map(|i| self.origin[i] + (c[i] as f32 + 0.5) * self.cell)
    }

    /// The cell `cells` steps from `idx` in direction `dir`, if inside the grid.
    fn beyond(&self, idx: usize, dir: usize, cells: usize) -> Option<usize> {
        let (axis, up) = (dir / 2, dir % 2 == 1);
        let stride = [1, self.n[0], self.n[0] * self.n[1]][axis];
        let c = (idx / stride) % self.n[axis];
        if up {
            (c + cells < self.n[axis]).then(|| idx + cells * stride)
        } else {
            (c >= cells).then(|| idx - cells * stride)
        }
    }

    /// The cell `other` lies beside `idx`, across the axis of `dir` along one other axis,
    /// within `cells` steps: the direction towards it and how many steps.
    fn beside(&self, idx: usize, dir: usize, other: usize, cells: usize) -> Option<(usize, usize)> {
        let (c, o) = (self.coords(idx), self.coords(other));
        let mut found = None;
        for axis in (0..3).filter(|&k| k != dir / 2) {
            if c[axis] != o[axis] {
                if found.is_some() {
                    return None;
                }
                found = Some((axis * 2 + usize::from(o[axis] > c[axis]), c[axis].abs_diff(o[axis])));
            }
        }
        found.filter(|&(_, k)| c[dir / 2] == o[dir / 2] && k >= 1 && k <= cells)
    }

    /// The cell `other` lies ahead of `idx` in direction `dir`, within `cells` steps.
    fn on_line(&self, idx: usize, dir: usize, other: usize, cells: usize) -> bool {
        let (axis, up) = (dir / 2, dir % 2 == 1);
        let stride = [1, self.n[0], self.n[0] * self.n[1]][axis] as i64;
        let diff = other as i64 - idx as i64;
        let steps = diff / stride;
        let c = ((idx / stride as usize) % self.n[axis]) as i64;
        diff % stride == 0 && steps != 0 && (up == (steps > 0)) && steps.unsigned_abs() as usize <= cells && c + steps >= 0 && ((c + steps) as usize) < self.n[axis]
    }

    /// The neighbour of a cell in direction `dir` (axis × 2 + 1 for up), if any.
    fn step(&self, idx: usize, dir: usize) -> Option<usize> {
        let (axis, up) = (dir / 2, dir % 2 == 1);
        let mut c = self.coords(idx);
        if up {
            if c[axis] + 1 >= self.n[axis] {
                return None;
            }
            c[axis] += 1;
        } else {
            if c[axis] == 0 {
                return None;
            }
            c[axis] -= 1;
        }
        Some(self.index(c))
    }

    /// The cell a point lies in.
    pub fn cell_of(&self, q: V3) -> Option<usize> {
        let mut c = [0usize; 3];
        for i in 0..3 {
            let v = ((q[i] - self.origin[i]) / self.cell).floor();
            if v < 0.0 || v as usize >= self.n[i] {
                return None;
            }
            c[i] = v as usize;
        }
        Some(self.index(c))
    }

    /// Cells whose centre lies in a box.
    fn cells_in(&self, (a, b): (V3, V3)) -> Vec<usize> {
        let range = |i: usize| {
            let lo = ((a[i] - self.origin[i]) / self.cell - 0.5).ceil().max(0.0) as usize;
            let hi = ((b[i] - self.origin[i]) / self.cell - 0.5).floor();
            (lo, if hi < 0.0 { None } else { Some((hi as usize).min(self.n[i] - 1)) })
        };
        let ([x0, y0, z0], [x1, y1, z1]) = ([0, 1, 2].map(|i| range(i).0), [0, 1, 2].map(|i| range(i).1));
        let (Some(x1), Some(y1), Some(z1)) = (x1, y1, z1) else { return Vec::new() };
        let mut v = Vec::new();
        for z in z0..=z1 {
            for y in y0..=y1 {
                for x in x0..=x1 {
                    v.push(self.index([x, y, z]));
                }
            }
        }
        v
    }

    /// Cells whose centre lies within `r` of a polyline.
    fn cells_near(&self, pts: &[V3], r: f32) -> Vec<usize> {
        let mut v = Vec::new();
        for s in pts.windows(2) {
            let lo = [0, 1, 2].map(|i| s[0][i].min(s[1][i]) - r);
            let hi = [0, 1, 2].map(|i| s[0][i].max(s[1][i]) + r);
            for idx in self.cells_in((lo, hi)) {
                if dist_to_segment(self.centre(idx), s[0], s[1]) <= r + 1e-3 {
                    v.push(idx);
                }
            }
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Cells along a polyline, in order, without repeats.
    pub fn cells_along(&self, pts: &[V3]) -> Vec<usize> {
        let mut v: Vec<usize> = Vec::new();
        for s in pts.windows(2) {
            let len = dist(s[0], s[1]);
            let steps = (len / (self.cell * 0.5)).ceil().max(1.0) as usize;
            for k in 0..=steps {
                let t = k as f32 / steps as f32;
                let q = [0, 1, 2].map(|i| s[0][i] + (s[1][i] - s[0][i]) * t);
                if let Some(idx) = self.cell_of(q) {
                    if v.last() != Some(&idx) {
                        v.push(idx);
                    }
                }
            }
        }
        v
    }

    /// How the request can pass the cell (see [`NO`], [`SQUEEZED`], [`FREE`]), keeping
    /// apart from the other routes where `apart`.
    fn level(&self, idx: usize, req: &Request, apart: bool) -> u8 {
        let free = match self.block[idx] {
            0 => true,
            ITEM => req.ignore.contains(&self.owner[idx]),
            BODY => req.ignore.contains(&self.owner[idx]) && (req.at.iter().any(|q| dist(self.centre(idx), *q) <= 2.0 * self.cell) || self.out_of_end(idx, req)),
            _ => false,
        };
        // A cable only through ducts it fits into, in a cabinet's duct between its base
        // and its cover; only classes a reservation is for.
        if !free || (req.kind != RouteKind::Wire && self.duct[idx] > 0.0 && self.duct[idx] < 2.0 * req.half) || (self.reserved[idx] != 0 && self.reserved[idx] & req.class.bit() == 0) || (apart && !self.apart(idx, req)) {
            return NO;
        }
        if req.kind != RouteKind::Wire && self.duct_of[idx] != 0 {
            let d = &self.ducts[self.duct_of[idx] as usize - 1];
            if !self.in_band(self.centre(idx)[2] - d.lo[2], d.base, d.top, req.half) {
                return NO;
            }
        }
        // As thick as it is: half its thickness of room around it.
        if self.clear(idx, 0.0, req) {
            FREE
        } else {
            NO
        }
    }

    fn passable(&self, idx: usize, req: &Request) -> bool {
        self.level(idx, req, true) != NO
    }

    /// The cell of an item lies straight out of an end of the request at that item, in
    /// the direction the route leaves it (a cell aside at most): the way out of a
    /// connection point inside the box of its device (terminals behind a shroud, a
    /// switch on its module) is open as far as the box reaches.
    fn out_of_end(&self, idx: usize, req: &Request) -> bool {
        let c = self.centre(idx);
        (0..2).any(|i| {
            let Some((axis, sign)) = Space::axis_of(req.leave[i]) else { return false };
            let q = req.at[i];
            (c[axis] - q[axis]) * sign > 0.0 && (0..3).filter(|&k| k != axis).all(|k| (c[k] - q[k]).abs() <= self.cell) && self.cell_of(q).is_some_and(|at| self.owner[at] == self.owner[idx])
        })
    }

    /// The cell is passable for the request but for the other routes in it.
    fn open_for(&self, idx: usize, req: &Request) -> bool {
        self.level(idx, req, false) != NO
    }

    /// A route coming in direction `from` may turn at the cell into direction `to`: its
    /// bend, rounded with the bend radius, keeps half its thickness from what blocks
    /// the space.
    fn turn_room(&self, idx: usize, from: usize, to: usize, req: &Request) -> bool {
        self.bend_room(self.centre(idx), unit(from), unit(to), req.turn, req)
    }

    /// The path a cell belongs to, if the request may use it as a channel.
    fn path_for(&self, idx: usize, req: &Request) -> Option<&PathCells> {
        let p = self.paths.get((self.path[idx] as usize).checked_sub(1)?)?;
        (p.kind.accepts(req.kind) && (p.classes == 0 || p.classes & req.class.bit() != 0)).then_some(p)
    }

    /// The stretch of a cable duct a cell lies in.
    fn section_of(&self, idx: usize) -> Option<&Section> {
        self.sections.get((self.section[idx] as usize).checked_sub(1)?)
    }

    /// The cell is a channel for the request: a cable duct (not for tubes) or one of
    /// its routing paths.
    fn channel(&self, idx: usize, req: &Request) -> bool {
        (self.duct[idx] > 0.0 && req.kind != RouteKind::Tube) || self.path_for(idx, req).is_some()
    }

    /// Cost factor of a step into the cell (`axis` 1: up or down).
    fn factor(&self, idx: usize, axis: usize, req: &Request, rules: &RoutingRules) -> f32 {
        let channel = self.channel(idx, req);
        let mut f = if channel { 1.0 } else { rules.free_factor.max(1.0) };
        if !channel && axis != 1 {
            f *= self.open_factor(idx);
        }
        if self.path_for(idx, req).is_some_and(|p| p.capacity > 0.0 && p.used + req.area > p.capacity) {
            f *= FULL;
        }
        if self.section_of(idx).is_some_and(|s| s.capacity > 0.0 && s.used + req.area > s.capacity) {
            f *= FULL;
        }
        if self.zone[idx] & req.class.apart_bits() != 0 {
            f *= rules.segregation_factor.max(1.0);
        }
        f
    }

    /// Cost factor of a horizontal step into a cell of a room by how far it lies from
    /// the room's surfaces: none next to them, [`OPEN`] from the distance of
    /// [`Space::mark_open`] on, in between as far as it is.
    fn open_factor(&self, idx: usize) -> f32 {
        match self.open.get(idx) {
            Some(&d) if d > 1 => 1.0 + (OPEN - 1.0) * ((d - 1) as f32 / (self.open_cells.max(2) - 1) as f32).min(1.0),
            _ => 1.0,
        }
    }

    /// A route of half thickness `half` through a cell whose centre lies `z` above the
    /// bottom of a duct's box may lie between the heights `lo` and `hi` of the duct
    /// (its base and its cover, the rim and the top of its walls): the band meets the
    /// cell's extent; drawn, the route is laid at a height within both.
    fn in_band(&self, z: f32, lo: f32, hi: f32, half: f32) -> bool {
        (z - self.cell * 0.5).max(lo + half) <= (z + self.cell * 0.5).min(hi - half) + 1e-3
    }

    /// The walls of the cabinets' cable ducts let the step from cell `a` into its
    /// neighbour `b` in direction `dir` pass: not through the base or the cover of a
    /// duct, through its walls only with half the thickness between their rim and their
    /// top ([`Space::in_band`]), and outside a duct no nearer to it than half the
    /// thickness, unless heading into or out of it through its wall.
    fn duct_step(&self, a: usize, b: usize, dir: usize, req: &Request) -> bool {
        let (da, db) = (self.duct_of[a], self.duct_of[b]);
        if da != db && (da == 0 || db == 0) {
            if dir / 2 == 2 {
                return false;
            }
            let d = &self.ducts[da.max(db) as usize - 1];
            let z = self.centre(if da != 0 { a } else { b })[2] - d.lo[2];
            return self.in_band(z, d.rim, d.top, req.half);
        }
        let Some(&(room, k)) = self.duct_room.get(b).filter(|_| db == 0) else { return true };
        if k == 0 || room >= req.half {
            return true;
        }
        // Beside the duct (not over its cover) heading at its wall or away from it.
        let d = &self.ducts[k as usize - 1];
        let z = self.centre(b)[2];
        dir / 2 != d.axis && dir / 2 != 2 && z >= d.lo[2] && z <= d.hi[2]
    }

    /// Crossing between the cells `a` and `b` (neighbours along x) is allowed: in the
    /// same enclosure, or through a cable passage with `half` the thickness inside it.
    fn crossing_ok(&self, a: usize, b: usize, half: f32) -> bool {
        let (ca, cb) = (self.coords(a), self.coords(b));
        if self.column[ca[0]] == self.column[cb[0]] {
            return true;
        }
        let (pa, pb) = (self.centre(a), self.centre(b));
        self.crossings.iter().any(|c| {
            c.x >= pa[0].min(pb[0]) - 1e-3 && c.x <= pa[0].max(pb[0]) + 1e-3 && pa[1] >= c.y[0] + half - 1e-3 && pa[1] <= c.y[1] - half + 1e-3 && pa[2] >= c.z[0] + half - 1e-3 && pa[2] <= c.z[1] - half + 1e-3
        })
    }

    /// The step from cell `a` into its neighbour `b` in direction `dir` is open for
    /// the request.
    fn step_ok(&self, a: usize, b: usize, dir: usize, req: &Request) -> bool {
        self.passable(b, req) && !self.across(b, dir, req) && (dir / 2 != 0 || self.crossing_ok(a, b, req.half)) && self.duct_step(a, b, dir, req)
    }

    /// The cost of the step from cell `a` into its neighbour `b` in direction `dir`,
    /// `None` where it is not open; what the steps into the cells cost is remembered
    /// for the search.
    fn step_cost(&self, memo: &mut Memo, a: usize, b: usize, dir: usize, req: &Request, rules: &RoutingRules) -> Option<f32> {
        if memo.pass_stamp[b] != memo.run {
            memo.pass[b] = match self.level(b, req, true) {
                NO => (-1.0, -1.0, NO),
                level => (self.cell * self.factor(b, 1, req, rules), self.cell * self.factor(b, 0, req, rules), level),
            };
            memo.pass_stamp[b] = memo.run;
        }
        let (up, along, level) = memo.pass[b];
        if level == NO || self.across(b, dir, req) || (dir / 2 == 0 && !self.crossing_ok(a, b, req.half)) || !self.duct_step(a, b, dir, req) {
            return None;
        }
        // Where it may cross another route it still goes round it where that costs
        // little: a lane further out, not across the way down to the next terminal.
        let cross = if self.crosses(b, dir, req) { CROSSING * self.cell } else { 0.0 };
        Some(cross + if dir / 2 == 1 { up } else { along })
    }

    /// The nearest passable cell to a point (searching a few cells around it).
    fn nearest_passable(&self, q: V3, req: &Request) -> Option<usize> {
        let r = self.cell * LEAD_CELLS as f32;
        let lo = [q[0] - r, q[1] - r, q[2] - r];
        let hi = [q[0] + r, q[1] + r, q[2] + r];
        if let Some(idx) = self.cell_of(q).filter(|&i| self.passable(i, req)) {
            return Some(idx);
        }
        self.cells_in((lo, hi)).into_iter().filter(|&i| self.passable(i, req)).min_by(|&a, &b| dist(self.centre(a), q).total_cmp(&dist(self.centre(b), q)).then(a.cmp(&b)))
    }

    /// The axis a direction runs along and which way (±1); `None` for one along no axis.
    fn axis_of(leave: V3) -> Option<(usize, f32)> {
        let axis = (0..3).max_by(|&i, &j| leave[i].abs().total_cmp(&leave[j].abs()))?;
        let along = leave[axis].abs();
        (along >= 1e-3 && along >= 0.9 * dist(leave, [0.0; 3])).then(|| (axis, leave[axis].signum()))
    }

    /// How far along its axis the way out of an end at `q` in the direction `leave` may
    /// go at most, so that it does not pass the way out of the `other` end (leaving it
    /// in `other_leave`) when that lies ahead: to the middle between the two where they
    /// face each other, before the other end where it would run into it; `None` for no
    /// limit.
    fn lead_limit(&self, q: V3, leave: V3, other: V3, other_leave: V3) -> Option<f32> {
        let (axis, sign) = Space::axis_of(leave)?;
        if (other[axis] - q[axis]) * sign <= 0.0 {
            return None;
        }
        if Space::axis_of(other_leave).is_some_and(|(oa, os)| oa == axis && os == -sign) {
            return Some((q[axis] + other[axis]) / 2.0);
        }
        let lateral = (0..3).filter(|&i| i != axis).map(|i| (q[i] - other[i]).abs()).fold(0.0, f32::max);
        (lateral <= 2.0 * self.cell).then_some(other[axis])
    }

    /// The way out of an end at `q` in the direction `leave`: the cells from the first
    /// passable one along it (its centre ahead of `q`, no other item before it) straight
    /// on, until the route is (and has come onto the line of these cells, see [`LEAN`]
    /// and [`onto_line`], where `q` lies beside them)
    /// `far` mm (with the `out` it runs straight already) out of the end, as far as
    /// `cells` cells at most and never beyond `limit` along the axis, the direction, and
    /// whether it got that far (or to the limit). `None` where `leave` runs along no
    /// axis or none of the first cells along it is passable.
    #[allow(clippy::too_many_arguments)]
    fn lead(&self, q: V3, leave: V3, limit: Option<f32>, out: f32, far: f32, req: &Request, cells: usize) -> Option<(Vec<usize>, usize, bool)> {
        let (axis, sign) = Space::axis_of(leave)?;
        let dir = axis * 2 + usize::from(sign > 0.0);
        let within = |idx: usize| limit.is_none_or(|l| (self.centre(idx)[axis] - l) * sign <= 0.0);
        let ahead = |idx: usize| (self.centre(idx)[axis] - q[axis]) * sign + out;
        // The cells straight out of `from`, and whether they run far enough.
        let from = |from: V3| -> Option<(Vec<usize>, bool)> {
            let mut first = None;
            for k in 0..LEAD_CELLS {
                let mut p = from;
                p[axis] += sign * (k as f32 + 0.5) * self.cell;
                let Some(idx) = self.cell_of(p) else { break };
                if !within(idx) {
                    break;
                }
                if self.passable(idx, req) && !self.across(idx, dir, req) {
                    first = Some(idx);
                    break;
                }
                // Never through another item to a cell behind it.
                if self.block[idx] == BODY && !req.ignore.contains(&self.owner[idx]) {
                    break;
                }
            }
            let mut way = vec![first?];
            // Onto the line of its cells, where the end does not lie in them (the way
            // straight out of it is barred): leaning towards it all the way, or with an
            // S-bend before its first bend.
            let c = self.centre(way[0]);
            let aside = (0..3).filter(|&k| k != axis).map(|k| (c[k] - q[k]).powi(2)).sum::<f32>().sqrt();
            let beside = (0..3).any(|k| k != axis && (c[k] - q[k]).abs() > self.cell / 2.0 + 0.01);
            let far = if beside { far.max((aside / LEAN).min(onto_line(aside, far) + far) - 1.0) } else { far };
            let mut cut = false;
            for _ in 0..cells + (2.0 * far / self.cell) as usize {
                let last = way[way.len() - 1];
                if ahead(last) >= far {
                    break;
                }
                match self.step(last, dir) {
                    Some(next) if !within(next) => {
                        cut = true;
                        break;
                    }
                    Some(next) if self.step_ok(last, next, dir, req) => way.push(next),
                    _ => break,
                }
            }
            let full = cut || ahead(way[way.len() - 1]) >= far;
            Some((way, full))
        };
        // Barred before it ran `far` straight out of the end, the way may leave it from
        // the cell beside (a cell aside at most), where it runs farther.
        let mut best = from(q);
        if best.as_ref().is_none_or(|c| !c.1) {
            for other in (0..3).filter(|&k| k != axis) {
                for side in [-1.0, 1.0] {
                    let mut p = q;
                    p[other] += side * self.cell;
                    if let Some(cells) = from(p).filter(|c| best.as_ref().is_none_or(|b| c.0.len() > b.0.len())) {
                        best = Some(cells);
                    }
                }
            }
        }
        let (way, full) = best?;
        Some((way, dir, full))
    }

    /// The cheapest way from `a` to `b`, or `None` if there is none.
    pub fn route(&self, a: V3, b: V3, req: &Request, rules: &RoutingRules) -> Option<Way> {
        self.route_with(a, b, req, rules, None).0
    }

    /// [`Space::route`] with the costs to the goal of a search of the same request before
    /// (where they still hold), and the costs it used.
    pub fn route_with(&self, a: V3, b: V3, req: &Request, rules: &RoutingRules, known: Option<&GoalCosts>) -> (Option<Way>, Option<GoalCosts>) {
        // Ends closer than a cell that face each other (the nozzles of two fittings in
        // line): straight from one to the other.
        let facing = Space::axis_of(req.leave[0]).zip(Space::axis_of(req.leave[1])).is_some_and(|(x, y)| x.0 == y.0 && x.1 == -y.1);
        if facing && dist(a, b) <= self.cell {
            let mut cells: Vec<usize> = [self.cell_of(a), self.cell_of(b)].into_iter().flatten().filter(|&i| self.passable(i, req)).collect();
            cells.dedup();
            return (Some(Way { points: vec![a, b], cells, free: 0.0 }), known.cloned());
        }
        // The route runs straight out of its ends for its bend radius and on between its
        // bends for two of them (the arcs of both fit between), a cell to spare.
        let straight = (2.0 * req.turn / self.cell).ceil() as usize + 1;
        let far = req.turn;
        let cells = (far / self.cell).ceil() as usize;
        let lead_in = self.lead(a, req.leave[0], self.lead_limit(a, req.leave[0], b, req.leave[1]), req.out[0], far, req, cells);
        let lead_out = self.lead(b, req.leave[1], self.lead_limit(b, req.leave[1], a, req.leave[0]), req.out[1], far, req, cells);
        // An end without a direction is left from the nearest passable cell; one the
        // route cannot leave straight on for its bend radius cannot be left at all.
        let mut ends = Ends { lead_in: Vec::new(), dir: 6, lead_out: Vec::new(), arrive: 6 };
        match lead_in {
            Some((cells, dir, true)) => (ends.lead_in, ends.dir) = (cells, dir),
            Some(_) => return (None, None),
            None if Space::axis_of(req.leave[0]).is_some() => return (None, None),
            None => match self.nearest_passable(a, req) {
                Some(idx) => ends.lead_in = vec![idx],
                None => return (None, None),
            },
        }
        match lead_out {
            Some((mut cells, dir, true)) => {
                cells.reverse();
                ends.arrive = dir ^ 1;
                ends.lead_out = cells;
            }
            Some(_) => return (None, None),
            None if Space::axis_of(req.leave[1]).is_some() => return (None, None),
            None => match self.nearest_passable(b, req) {
                Some(idx) => ends.lead_out = vec![idx],
                None => return (None, None),
            },
        }
        let mut costs = known.cloned();
        SEARCH.with_borrow_mut(|search| {
            // A way that came back to itself through a state taken up again (its way
            // there changed after the search went on from it): once more, taking up none.
            let way = match self.search(search, a, b, &ends, req, rules, straight, &mut costs, false) {
                Some(way) if self.way_meets_itself(&way.points, req) => self.search(search, a, b, &ends, req, rules, straight, &mut costs, true).filter(|w| !self.way_meets_itself(&w.points, req)),
                way => way,
            };
            (way, costs)
        })
    }

    /// The way a search took to `state`, into `own`: from the start `a` (out of the
    /// nozzle of its device, where the route comes out of one) along the way out of it
    /// and round the turns before the state (`corner`).
    fn way_to(&self, own: &mut OwnWay, corner: &[u32], state: usize, ends: &Ends, a: V3, req: &Request) {
        let points = &mut own.points;
        points.clear();
        points.push(self.centre(state / 7));
        let mut s = state;
        while corner[s] != u32::MAX {
            let from = corner[s] as usize;
            let (cf, cs) = (self.coords(from / 7), self.coords(s / 7));
            // A lane change: aside first, then on in the direction of the state.
            let along = (s % 7) / 2;
            if let Some(k) = (0..3).find(|&k| k != along && cf[k] != cs[k]) {
                let mut c = cf;
                c[k] = cs[k];
                points.push(self.centre(self.index(c)));
            }
            points.push(self.centre(from / 7));
            s = from;
        }
        points.push(self.centre(ends.lead_in[0]));
        points.push(a);
        if req.out[0] > 0.0 {
            points.push(sub(a, scale(req.leave[0], req.out[0])));
        }
        points.reverse();
        simplify_in_place(points);
        // The boxes round its stretches but the last, and round the bends at its corners
        // (between the corner and where the bend begins and ends).
        let n = points.len();
        own.boxes.clear();
        let bounds = |p: V3, q: V3| ([0, 1, 2].map(|k| p[k].min(q[k])), [0, 1, 2].map(|k| p[k].max(q[k])));
        for i in 0..n.saturating_sub(2) {
            own.boxes.push(bounds(points[i], points[i + 1]));
        }
        for i in 1..n.saturating_sub(1) {
            let (c, lu, lv) = (points[i], dist(points[i - 1], points[i]), dist(points[i], points[i + 1]));
            let r = req.turn.min(lu).min(lv);
            own.boxes.push(bounds(add(c, scale(sub(points[i - 1], c), r / lu)), add(c, scale(sub(points[i + 1], c), r / lv))));
        }
        own.known = true;
    }

    /// The way of a search so far (`own`, [`Space::way_to`]) going on through `new`,
    /// and, where that brings it to its goal, along the way into the end `b`, comes
    /// back to itself ([`meets_itself`]).
    fn meets_own_way(&self, own: &OwnWay, new: &[V3], end: Option<(&Ends, V3)>, req: &Request) -> bool {
        let way = &own.points;
        let n = way.len();
        if n < 2 || n + new.len() + end.map_or(0, |(e, _)| e.lead_out.len()) < 4 {
            return false;
        }
        // Only where the way on (with the bend into it) comes near one of the stretches
        // or bends before the last stretch: next to that one it lies anyway.
        let apart = own_room(req);
        let last = way[n - 1];
        let (mut lo, mut hi) = (last, last);
        let mut grow = |q: V3| {
            for k in 0..3 {
                (lo[k], hi[k]) = (lo[k].min(q[k]), hi[k].max(q[k]));
            }
        };
        let back = sub(way[n - 2], last);
        let turns = new.first().is_none_or(|q| {
            let on = sub(*q, last);
            (0..3).any(|k| (back[(k + 1) % 3] * on[(k + 2) % 3] - back[(k + 2) % 3] * on[(k + 1) % 3]).abs() > 1e-3)
        });
        if turns {
            let l = dist(way[n - 2], last);
            grow(add(last, scale(back, req.turn.min(l) / l)));
        }
        for q in new {
            grow(*q);
        }
        if let Some((ends, b)) = end {
            for &i in &ends.lead_out[1..] {
                grow(self.centre(i));
            }
            grow(b);
        }
        if !own.boxes.iter().any(|(blo, bhi)| (0..3).all(|k| blo[k] <= hi[k] + apart && bhi[k] >= lo[k] - apart)) {
            return false;
        }
        let mut points = way.clone();
        points.extend_from_slice(new);
        if let Some((ends, b)) = end {
            points.extend(ends.lead_out[1..].iter().map(|&i| self.centre(i)));
            points.push(b);
            if req.out[1] > 0.0 {
                points.push(sub(b, scale(req.leave[1], req.out[1])));
            }
        }
        simplify_in_place(&mut points);
        meets_itself(&points, req.turn, apart, n.saturating_sub(2)).is_some()
    }

    /// The way along `points` from end to end of a request, out of the nozzles of the
    /// devices at its ends where it comes out of them, comes back to itself.
    fn way_meets_itself(&self, points: &[V3], req: &Request) -> bool {
        let mut all = Vec::with_capacity(points.len() + 2);
        for (i, q) in [points.first(), points.last()].into_iter().enumerate() {
            if i == 1 {
                all.extend_from_slice(points);
            }
            if let Some(q) = q.filter(|_| req.out[i] > 0.0) {
                all.push(sub(*q, scale(req.leave[i], req.out[i])));
            }
        }
        simplify_in_place(&mut all);
        meets_itself(&all, req.turn, own_room(req), 0).is_some()
    }

    /// The A* search of [`Space::route`] with the arrays of `search`, from the start to
    /// the goal of `ends`: a turn is made only where its bend has room, and followed by
    /// `straight` cells straight on (one at least), unless the way changes lanes with an
    /// S-bend (a turn back into the direction it came within fewer cells, then straight
    /// on); the way out of the start and into the goal is given by `ends`, the goal
    /// reached heading along the way into the end. The way keeps apart from itself as
    /// from the other routes: no state is reached along a way that comes back to itself
    /// there ([`meets_itself`]); `strict`: a state the search went on from is not taken
    /// up again more cheaply (the ways through it stay as they were checked). It takes
    /// the costs to the goal from `costs` where known, otherwise works them out and
    /// keeps them there.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    fn search(&self, search: &mut Search, a: V3, b: V3, ends: &Ends, req: &Request, rules: &RoutingRules, straight: usize, costs: &mut Option<GoalCosts>, strict: bool) -> Option<Way> {
        let (start, goal) = (ends.lead_in[ends.lead_in.len() - 1], ends.lead_out[0]);
        // States: cell × direction it was entered in (6: none).
        let run = search.start(self.block.len());
        // The costs known lead to their own goal: from this one they are less by the
        // cost from here to there (at least `reach` where it is not known).
        let (reach, offset) = match costs {
            Some(known) => {
                for &(idx, c) in &known.cells {
                    (search.to_goal[idx as usize], search.settled[idx as usize]) = (c, run);
                }
                let offset = if known.goal == goal { 0.0 } else if search.settled[goal] == run { search.to_goal[goal] } else { known.reach };
                (known.reach, offset)
            }
            None => {
                let (reach, cells) = self.costs_to_goal(search, run, start, goal, req, rules);
                // The start not reached even without turns: there is no way.
                let reached = search.settled[start] == run;
                if cells.len() <= KEEP_COSTS {
                    *costs = Some(GoalCosts { goal, cells: cells.into_iter().map(|i| (i as u32, search.to_goal[i])).collect(), reach });
                }
                if !reached {
                    return None;
                }
                (reach, 0.0)
            }
        };
        let Search { best, prev, corner, closed, stamp, to_goal, settled, runs, run_stamp, pass, pass_stamp, turn_known, turn_ok, turn_stamp, jogs, .. } = search;
        if strict {
            closed.fill(0);
        }
        let mut memo = Memo { pass, pass_stamp, runs, run_stamp, turn_known, turn_ok, turn_stamp, jogs, run };
        let mut heap = BinaryHeap::new();
        let gc = self.coords(goal);
        // Lower bound of the rest: the cost of the cells to the goal (at least `reach`
        // where it is not known) and the turns, one fewer than the axes it still lies
        // apart on when heading towards it along one of them.
        let h = |state: usize| {
            let (idx, dir) = (state / 7, state % 7);
            let c = self.coords(idx);
            let apart = (0..3).filter(|&i| c[i] != gc[i]).count();
            let towards = dir < 6 && {
                let axis = dir / 2;
                if dir % 2 == 1 { c[axis] < gc[axis] } else { c[axis] > gc[axis] }
            };
            let turns = if apart == 0 { 0 } else if dir == 6 || towards { apart - 1 } else { apart };
            let known = if settled[idx] == run { to_goal[idx] } else { reach } - offset;
            let cells = known.max((0..3).map(|i| c[i].abs_diff(gc[i])).sum::<usize>() as f32 * self.cell);
            cells + turns as f32 * req.turn
        };
        let key = |g: f32, state: usize| Reverse(((g + h(state)).to_bits(), state));
        // The way out of the start costs its cells.
        let first = start * 7 + ends.dir;
        let g0 = ends.lead_in.windows(2).map(|w| self.step_cost(&mut memo, w[0], w[1], ends.dir, req, rules).unwrap_or(0.0)).sum::<f32>();
        (best[first], prev[first], corner[first], stamp[first]) = (g0, u32::MAX, u32::MAX, run);
        heap.push(key(g0, first));
        let jump = straight.max(1);
        let mut found = None;
        let mut own = OwnWay::default();
        while let Some(Reverse(popped)) = heap.pop() {
            let state = popped.1;
            let (idx, dir) = (state / 7, state % 7);
            let g = best[state];
            // A state reached again more cheaply was expanded with its better cost.
            if key(g, state).0 != popped {
                continue;
            }
            if idx == goal && (ends.arrive == 6 || dir == ends.arrive) {
                found = Some(state);
                break;
            }
            if strict {
                closed[state / 64] |= 1 << (state % 64);
            }
            // The way to this state is worked out when a way on from it is first checked.
            own.known = false;
            // A state reached more cheaply than before, from this one: on through the
            // points `$new` (the corners of the way on and where it ends), its last turn
            // at the state `$corner`; not where the way would come back to itself.
            macro_rules! relax {
                ($ns:expr, $ng:expr, $corner:expr, $new:expr) => {{
                    let (ns, ng): (usize, f32) = ($ns, $ng);
                    if (stamp[ns] != run || ng < best[ns]) && !(strict && closed[ns / 64] >> (ns % 64) & 1 == 1) {
                        if !own.known {
                            self.way_to(&mut own, corner, state, ends, a, req);
                        }
                        let done = ns / 7 == goal && (ends.arrive == 6 || ns % 7 == ends.arrive);
                        if !self.meets_own_way(&own, $new, done.then_some((ends, b)), req) {
                            (best[ns], prev[ns], corner[ns], stamp[ns]) = (ng, state as u32, $corner, run);
                            heap.push(key(ng, ns));
                        }
                    }
                }};
            }
            // The state is known already at no more than this cost: no way through
            // here, which costs at least that, improves it.
            macro_rules! settled_under {
                ($ns:expr, $floor:expr) => {{
                    let (ns, floor): (usize, f32) = ($ns, $floor);
                    stamp[ns] == run && best[ns] <= floor
                }};
            }
            for nd in 0..6 {
                if nd == dir {
                    // Straight on, one cell.
                    if let Some((ni, cost)) = self.step(idx, nd).and_then(|ni| self.step_cost(&mut memo, idx, ni, nd, req, rules).map(|c| (ni, c))) {
                        relax!(ni * 7 + nd, g + cost, corner[state], &[self.centre(ni)]);
                    }
                    continue;
                }
                if dir < 6 && nd == dir ^ 1 {
                    continue; // no way back
                }
                // A turn (none at a start without direction), then straight on for
                // `jump` cells (or to the goal, heading into its end), where all of them
                // are open: where its bend has room, otherwise only as the first half of
                // a lane change, whose S-bend is checked as a whole.
                let turn_ok = dir == 6 || self.turn_memo(&mut memo, idx, dir, nd, req);
                // At the goal a turn into the way into the end: the last corner lies where
                // that way begins, as the first one may where the way out of the start ends.
                // (The second half of its bend runs along that way, which is open.)
                if idx == goal && nd == ends.arrive && dir < 6 {
                    if turn_ok || self.arc_room(self.centre(idx), unit(dir), unit(nd), req.turn, std::f32::consts::FRAC_PI_4, req) {
                        relax!(goal * 7 + nd, g + req.turn, state as u32, &[]);
                    }
                    continue;
                }
                let run_cells = if dir < 6 { jump } else { 1 };
                let (mut at, mut ng) = (idx, g + if dir < 6 { req.turn } else { 0.0 });
                for k in 1..=run_cells {
                    let Some((ni, cost)) = self.step(at, nd).and_then(|ni| self.step_cost(&mut memo, at, ni, nd, req, rules).map(|c| (ni, c))) else {
                        at = usize::MAX;
                        break;
                    };
                    ng += cost;
                    at = ni;
                    if at == goal {
                        // Heading into the end the run ends here; across it, it runs on.
                        if ends.arrive == 6 || nd == ends.arrive {
                            break;
                        }
                        continue;
                    }
                    // The goal beside the run, fewer cells aside than a run, back in the
                    // direction it came (the way into the end running on): a lane change
                    // with an S-bend into it.
                    if let Some((sd, k2)) = (straight > 1 && dir < 6).then(|| self.beside(at, nd, goal, straight - 1)).flatten() {
                        if sd == dir && (ends.arrive == 6 || sd == ends.arrive) && self.jog_memo(&mut memo, idx, dir, nd, k, req) {
                            let (mut c, mut cost) = (at, ng + req.turn);
                            for _ in 0..k2 {
                                match self.step(c, sd).and_then(|ni| self.step_cost(&mut memo, c, ni, sd, req, rules).map(|x| (ni, x))) {
                                    Some((ni, x)) => (c, cost) = (ni, cost + x),
                                    None => break,
                                }
                            }
                            if c == goal {
                                relax!(goal * 7 + sd, cost, state as u32, &[self.centre(at), self.centre(goal)]);
                            }
                        }
                    }
                    if dir < 6 && k < straight && self.jog_memo(&mut memo, idx, dir, nd, k, req) {
                        // Fewer cells aside than a run, then back into the direction it
                        // came and a run: a lane change with an S-bend.
                        // The run lands `jump` cells on (unless it ends at the goal
                        // before): known cheaper already, it is not looked at.
                        let floor = ng + req.turn + jump as f32 * self.cell;
                        if self.beyond(at, dir, jump).is_some_and(|end| settled_under!(end * 7 + dir, floor)) && !self.on_line(at, dir, goal, jump) {
                            continue;
                        }
                        if let Some((fwd, fg)) = self.run_memo(&mut memo, at, dir, jump, goal, ends.arrive, req, rules) {
                            relax!(fwd * 7 + dir, ng + req.turn + fg, state as u32, &[self.centre(at), self.centre(fwd)]);
                        }
                    }
                }
                if at != usize::MAX && turn_ok {
                    relax!(at * 7 + nd, ng, state as u32, &[self.centre(at)]);
                }
            }
        }
        let mut state = found?;
        // The cells of the way: the runs between the states filled in (a step aside
        // first, then the run in the direction the state was entered in).
        let mut cells: Vec<usize> = vec![state / 7];
        while prev[state] != u32::MAX {
            let from = prev[state] as usize;
            let (ca, cb) = (self.coords(from / 7), self.coords(state / 7));
            let along = (state % 7) / 2;
            let aside = (0..3).find(|&k| k != along && ca[k] != cb[k]);
            let mut c = cb;
            for axis in [Some(along), aside].into_iter().flatten() {
                while c[axis] != ca[axis] {
                    c[axis] = if ca[axis] < cb[axis] { c[axis] - 1 } else { c[axis] + 1 };
                    cells.push(self.index(c));
                }
            }
            state = from;
        }
        cells.reverse();
        let mut all: Vec<usize> = ends.lead_in[..ends.lead_in.len() - 1].to_vec();
        all.extend(cells);
        all.extend_from_slice(&ends.lead_out[1..]);
        let mut points = vec![a];
        points.extend(all.iter().map(|&i| self.centre(i)));
        points.push(b);
        let points = simplify(points);
        let free = all.iter().filter(|&&i| !self.channel(i, req)).count() as f32 * self.cell;
        Some(Way { points, cells: all, free })
    }

    /// [`Space::jog_room`], remembered for the search.
    fn jog_memo(&self, memo: &mut Memo, idx: usize, dir: usize, aside: usize, cells: usize, req: &Request) -> bool {
        let key = (idx as u32, dir as u8, aside as u8, cells as u8);
        if let Some(&ok) = memo.jogs.get(&key) {
            return ok;
        }
        let ok = self.jog_room(idx, dir, aside, cells, req);
        memo.jogs.insert(key, ok);
        ok
    }

    /// [`Space::turn_room`], remembered for the search.
    fn turn_memo(&self, memo: &mut Memo, idx: usize, from: usize, to: usize, req: &Request) -> bool {
        if memo.turn_stamp[idx] != memo.run {
            (memo.turn_known[idx], memo.turn_ok[idx], memo.turn_stamp[idx]) = (0, 0, memo.run);
        }
        let bit = 1u64 << (from * 6 + to);
        if memo.turn_known[idx] & bit == 0 {
            memo.turn_known[idx] |= bit;
            if self.turn_room(idx, from, to, req) {
                memo.turn_ok[idx] |= bit;
            }
        }
        memo.turn_ok[idx] & bit != 0
    }

    /// A run of `cells` cells from `at` in direction `dir` (fewer up to the `goal` where
    /// it heads into its end, `arrive`), where all of them are open: the cell it ends
    /// at and what its cells cost (remembered for the search).
    #[allow(clippy::too_many_arguments)]
    fn run_memo(&self, memo: &mut Memo, at: usize, dir: usize, cells: usize, goal: usize, arrive: usize, req: &Request, rules: &RoutingRules) -> RunEnd {
        let key = at * 6 + dir;
        if memo.run_stamp[key] != memo.run {
            let (mut end, mut g) = (at, 0.0);
            let mut open = true;
            for _ in 0..cells {
                let Some((ni, cost)) = self.step(end, dir).and_then(|ni| self.step_cost(memo, end, ni, dir, req, rules).map(|c| (ni, c))) else {
                    open = false;
                    break;
                };
                g += cost;
                end = ni;
                if end == goal && (arrive == 6 || dir == arrive) {
                    break;
                }
            }
            memo.runs[key] = if open { (end as u32, g) } else { (u32::MAX, 0.0) };
            memo.run_stamp[key] = memo.run;
        }
        let (e, c) = memo.runs[key];
        (e != u32::MAX).then_some((e as usize, c))
    }

    /// The costs from the cells to `goal` without turns and without the other routes
    /// (Dijkstra backwards: they only make the way longer), until the one of `start` is
    /// known. Gives the
    /// highest known cost (the other cells cost at least as much) and the cells known.
    fn costs_to_goal(&self, search: &mut Search, run: u32, start: usize, goal: usize, req: &Request, rules: &RoutingRules) -> (f32, Vec<usize>) {
        let Search { to_goal, reached, settled, .. } = search;
        let mut heap = BinaryHeap::new();
        (to_goal[goal], reached[goal]) = (0.0, run);
        heap.push(Reverse((0.0f32.to_bits(), goal)));
        let mut reach = 0.0;
        let mut known = Vec::new();
        while let Some(Reverse((d, idx))) = heap.pop() {
            if settled[idx] == run {
                continue;
            }
            settled[idx] = run;
            known.push(idx);
            reach = f32::from_bits(d);
            if idx == start {
                break;
            }
            // Entering this cell from a neighbour costs its factor.
            for nd in 0..6 {
                let Some(ni) = self.step(idx, nd) else { continue };
                if settled[ni] == run || !self.open_for(ni, req) || (nd / 2 == 0 && !self.crossing_ok(ni, idx, req.half)) {
                    continue;
                }
                let nd = reach + self.cell * self.factor(idx, nd / 2, req, rules);
                if reached[ni] != run || nd < to_goal[ni] {
                    (to_goal[ni], reached[ni]) = (nd, run);
                    heap.push(Reverse((nd.to_bits(), ni)));
                }
            }
        }
        (reach, known)
    }

    /// The cross-sections in the routing paths and in the stretches of the cable ducts,
    /// for [`Space::full_as`].
    pub fn loads(&self) -> Vec<f32> {
        self.paths.iter().map(|p| p.used).chain(self.sections.iter().map(|s| s.used)).collect()
    }

    /// For a route of cross-section `area`, the routing paths and the stretches of the
    /// cable ducts are full or not as they were with the cross-sections `before` in them
    /// ([`Space::loads`]).
    pub fn full_as(&self, before: &[f32], area: f32) -> bool {
        let full = |used: f32, capacity: f32| capacity > 0.0 && used + area > capacity;
        let now = self.paths.iter().map(|p| (p.used, p.capacity)).chain(self.sections.iter().map(|s| (s.used, s.capacity)));
        before.len() == self.paths.len() + self.sections.len() && now.zip(before).all(|((used, capacity), &old)| full(old, capacity) == full(used, capacity))
    }

    /// A way that was routed before (frozen) is still possible: its cells are passable
    /// and it does not cross a wall outside the passages.
    pub fn still_free(&self, points: &[V3], req: &Request) -> bool {
        let cells = self.cells_along(points);
        // The ends may lie in the walls (cable glands) or in their devices.
        let inner = if cells.len() > 2 { &cells[1..cells.len() - 1] } else { &[][..] };
        inner.iter().all(|&i| self.passable(i, req)) && cells.windows(2).all(|w| self.coords(w[0])[0] == self.coords(w[1])[0] || self.crossing_ok(w[0], w[1], req.half))
    }

    /// Registers what runs through the cells: its class (segregation) and its
    /// cross-section in the routing paths it uses.
    pub fn occupy(&mut self, cells: &[usize], class: Class, area: f32) {
        let mut used: Vec<u16> = Vec::new();
        for &i in cells {
            self.occ[i] |= class.bit();
            if self.path[i] != 0 && !used.contains(&self.path[i]) {
                used.push(self.path[i]);
            }
        }
        for p in used {
            let pc = &mut self.paths[p as usize - 1];
            pc.used += area;
            pc.items += 1;
        }
    }

    /// Registers the cross-section of a route through the cells in the stretches of the
    /// cable ducts it runs through (once in each).
    pub fn load_ducts(&mut self, cells: &[usize], area: f32) {
        let mut seen: Vec<u32> = cells.iter().map(|&i| self.section[i]).filter(|&s| s != 0).collect();
        seen.sort_unstable();
        seen.dedup();
        for s in seen {
            self.sections[s as usize - 1].used += area;
        }
    }

    /// Recomputes the segregation zones: every cell within `distance` of a cell that
    /// classes run through.
    pub fn update_zones(&mut self, distance: f32) {
        let k = (distance / self.cell).ceil().max(0.0) as usize;
        let mut zone = self.occ.clone();
        for axis in 0..3 {
            let src = zone.clone();
            for (idx, &bits) in src.iter().enumerate() {
                if bits == 0 {
                    continue;
                }
                let c = self.coords(idx);
                let lo = c[axis].saturating_sub(k);
                let hi = (c[axis] + k).min(self.n[axis] - 1);
                for v in lo..=hi {
                    let mut nc = c;
                    nc[axis] = v;
                    let ni = self.index(nc);
                    zone[ni] |= bits;
                }
            }
        }
        self.zone = zone;
    }

    /// Classes routed through the cells (for checks).
    pub fn classes_at(&self, cells: &[usize]) -> u8 {
        cells.iter().fold(0, |m, &i| m | self.occ[i])
    }
}

/// A slab with rectangular `openings` ([y range], [z range]) cut out of it, as boxes.
fn cut_openings(slab: (V3, V3), openings: &[([f32; 2], [f32; 2])]) -> Vec<(V3, V3)> {
    let mut boxes = vec![slab];
    for &(y, z) in openings {
        let mut next = Vec::new();
        for (lo, hi) in boxes {
            if y[0] >= hi[1] || y[1] <= lo[1] || z[0] >= hi[2] || z[1] <= lo[2] {
                next.push((lo, hi));
                continue;
            }
            // Above and below the opening across the whole slab, beside it between.
            if y[0] > lo[1] {
                next.push((lo, [hi[0], y[0], hi[2]]));
            }
            if y[1] < hi[1] {
                next.push(([lo[0], y[1], lo[2]], hi));
            }
            let (ya, yb) = (y[0].max(lo[1]), y[1].min(hi[1]));
            if z[0] > lo[2] {
                next.push(([lo[0], ya, lo[2]], [hi[0], yb, z[0]]));
            }
            if z[1] < hi[2] {
                next.push(([lo[0], ya, z[1]], [hi[0], yb, hi[2]]));
            }
        }
        boxes = next;
    }
    boxes
}

/// Distance of a point from a box (0 inside).
fn box_distance(q: V3, lo: V3, hi: V3) -> f32 {
    let d = [0, 1, 2].map(|i| (lo[i] - q[i]).max(q[i] - hi[i]).max(0.0));
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Distance of a point of a floor plan from its outline.
fn plan_distance(plan: &[[f32; 2]], q: [f32; 2]) -> f32 {
    let mut best = f32::INFINITY;
    for i in 0..plan.len() {
        let (a, b) = (plan[i], plan[(i + 1) % plan.len()]);
        let ab = [b[0] - a[0], b[1] - a[1]];
        let l2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if l2 <= f32::EPSILON { 0.0 } else { (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1]) / l2).clamp(0.0, 1.0) };
        let p = [a[0] + ab[0] * t, a[1] + ab[1] * t];
        best = best.min((q[0] - p[0]).hypot(q[1] - p[1]));
    }
    best
}

/// Unit vector of a direction (axis × 2 + 1 for up).
fn unit(dir: usize) -> V3 {
    let mut v = [0.0; 3];
    v[dir / 2] = if dir % 2 == 1 { 1.0 } else { -1.0 };
    v
}

fn dist_to_segment(q: V3, a: V3, b: V3) -> f32 {
    let ab = sub(b, a);
    let l2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
    let t = if l2 <= f32::EPSILON { 0.0 } else { (((q[0] - a[0]) * ab[0] + (q[1] - a[1]) * ab[1] + (q[2] - a[2]) * ab[2]) / l2).clamp(0.0, 1.0) };
    dist(q, add(a, scale(ab, t)))
}

/// The least distance between the segments `a`–`b` and `c`–`d`, and where along each
/// of them (0 to 1) they come that near.
fn segment_gap(a: V3, b: V3, c: V3, d: V3) -> (f32, f32, f32) {
    let (u, v, w) = (sub(b, a), sub(d, c), sub(a, c));
    let (uu, uv, vv, uw, vw) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let den = uu * vv - uv * uv;
    let mut s = if den <= 1e-6 * uu * vv { 0.0 } else { ((uv * vw - vv * uw) / den).clamp(0.0, 1.0) };
    let t = if vv > 1e-9 { ((uv * s + vw) / vv).clamp(0.0, 1.0) } else { 0.0 };
    s = if uu > 1e-9 { ((uv * t - uw) / uu).clamp(0.0, 1.0) } else { 0.0 };
    (dist(add(a, scale(u, s)), add(c, scale(v, t))), s, t)
}

/// Where the way along `points` (its corner points), bent round its right-angled
/// corners with `radius` (less where a stretch beside the corner is shorter), comes
/// back to itself: a place on one of its stretches from the stretch `from` on, or on
/// the bend into one of them, that lies nearer than `apart` to a place more than twice
/// that far back along the way (round a corner or along a lane change the way lies
/// that near to itself, no further along it than that); `None` where it keeps apart
/// from itself.
pub fn meets_itself(points: &[V3], radius: f32, apart: f32, from: usize) -> Option<V3> {
    let n = points.len();
    if n < 4 {
        return None;
    }
    // How far along the way each corner lies.
    let mut along = Vec::with_capacity(n);
    along.push(0.0);
    for s in points.windows(2) {
        along.push(along[along.len() - 1] + dist(s[0], s[1]));
    }
    if along[n - 1] <= 2.0 * apart {
        return None;
    }
    // The pieces: the stretches and the chords of the bends, each from how far to how
    // far along the way it runs.
    let mut pieces: Vec<(V3, V3, f32, f32)> = Vec::with_capacity((BEND_CHORDS + 1) * n);
    let mut first_new = 0;
    for i in 0..n - 1 {
        if i == from {
            first_new = pieces.len();
        }
        if i > 0 {
            let (lu, lv) = (along[i] - along[i - 1], along[i + 1] - along[i]);
            let (u, v) = (sub(points[i], points[i - 1]), sub(points[i + 1], points[i]));
            let r = radius.min(lu).min(lv);
            if r > 1e-3 && dot(u, v).abs() <= 1e-3 * lu * lv {
                let (u, v) = (scale(u, 1.0 / lu), scale(v, 1.0 / lv));
                let at = |k: usize| {
                    let phi = std::f32::consts::FRAC_PI_2 * k as f32 / BEND_CHORDS as f32;
                    add(points[i], add(scale(u, r * (phi.sin() - 1.0)), scale(v, r * (1.0 - phi.cos()))))
                };
                let step = 2.0 * r / BEND_CHORDS as f32;
                for k in 0..BEND_CHORDS {
                    pieces.push((at(k), at(k + 1), along[i] - r + step * k as f32, along[i] - r + step * (k + 1) as f32));
                }
            }
        }
        pieces.push((points[i], points[i + 1], along[i], along[i + 1]));
    }
    let far = 2.0 * apart;
    for (k, p) in pieces.iter().enumerate().skip(first_new) {
        let (plo, phi) = ([0, 1, 2].map(|i| p.0[i].min(p.1[i]) - apart), [0, 1, 2].map(|i| p.0[i].max(p.1[i]) + apart));
        for o in &pieces[..k] {
            // Too near along the way, or their boxes apart.
            if p.3 - o.2 <= far || (0..3).any(|i| o.0[i].max(o.1[i]) < plo[i] || o.0[i].min(o.1[i]) > phi[i]) {
                continue;
            }
            // Where the two come nearest, and from the ends of each to the other.
            let (gap, s, t) = segment_gap(p.0, p.1, o.0, o.1);
            let ends = [(0.0, segment_gap(p.0, p.0, o.0, o.1)), (1.0, segment_gap(p.1, p.1, o.0, o.1))];
            let back = [(segment_gap(p.0, p.1, o.0, o.0), 0.0), (segment_gap(p.0, p.1, o.1, o.1), 1.0)];
            let pairs = [(gap, s, t), (ends[0].1 .0, ends[0].0, ends[0].1 .2), (ends[1].1 .0, ends[1].0, ends[1].1 .2), (back[0].0 .0, back[0].0 .1, back[0].1), (back[1].0 .0, back[1].0 .1, back[1].1)];
            for (gap, s, t) in pairs {
                if gap < apart && (p.2 + (p.3 - p.2) * s) - (o.2 + (o.3 - o.2) * t) > far {
                    return Some(add(p.0, scale(sub(p.1, p.0), s)));
                }
            }
        }
    }
    None
}

/// How far a route keeps from itself, centre to centre: its thickness, as from another
/// route of its kind.
fn own_room(req: &Request) -> f32 {
    2.0 * req.half + 0.5
}

/// The items of an enclosure that are solids part by part in the routing space (by
/// id, with their parts as items): a terminal strip across the bars, each terminal a
/// part of its own, so that a route to one of them does not pass through the others.
pub(super) fn item_parts(prj: &Project, e: &Enclosure) -> HashMap<u64, Vec<PanelItem>> {
    e.items.iter().filter_map(|it| panel::busbars::terminals_on_bars(prj, it).map(|places| (it.id, places.into_iter().map(|(sub, ..)| sub).collect()))).collect()
}

/// Removes repeated points and points in the middle of straight stretches.
pub fn simplify(mut points: Vec<V3>) -> Vec<V3> {
    simplify_in_place(&mut points);
    points
}

/// [`simplify`] of the points where they are.
fn simplify_in_place(v: &mut Vec<V3>) {
    let mut n = 0;
    for i in 0..v.len() {
        let q = v[i];
        if n > 0 && dist(v[n - 1], q) < 1e-3 {
            continue;
        }
        if n >= 2 {
            let (a, b) = (v[n - 2], v[n - 1]);
            let (d1, d2) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [q[0] - b[0], q[1] - b[1], q[2] - b[2]]);
            let cross = [d1[1] * d2[2] - d1[2] * d2[1], d1[2] * d2[0] - d1[0] * d2[2], d1[0] * d2[1] - d1[1] * d2[0]];
            let dot = d1[0] * d2[0] + d1[1] * d2[1] + d1[2] * d2[2];
            if cross.iter().all(|c| c.abs() < 1e-3) && dot > 0.0 {
                n -= 1;
            }
        }
        v[n] = q;
        n += 1;
    }
    v.truncate(n);
}

/// Length of a polyline.
pub fn length(points: &[V3]) -> f32 {
    points.windows(2).map(|s| dist(s[0], s[1])).sum()
}

/// Corners of a polyline (direction changes) and the length of the shortest stretch
/// between two of them.
pub fn bends(points: &[V3]) -> (usize, f32) {
    let n = points.len().saturating_sub(2);
    let inner = points.windows(2).skip(1).take(points.len().saturating_sub(3)).map(|s| dist(s[0], s[1])).fold(f32::INFINITY, f32::min);
    (n, inner)
}

/// The bend radius of each corner of a way: `radius`, the least the route may bend
/// with, at every corner (the 3D view bends corners too close for separate arcs as one
/// curve of it), none at the ends.
pub fn corner_radii(points: &[V3], radius: f32) -> Vec<f32> {
    let n = points.len();
    (0..n).map(|i| if i == 0 || i + 1 == n { 0.0 } else { radius }).collect()
}

/// Stretches of a polyline between two bends shorter than `least`: between corners
/// of the grid (the short ways from the ends onto it and out of the nozzles are no
/// bends), not counting the lane changes with an S-bend (a stretch between two bends
/// that turn it back into the direction it came).
pub fn close_bends(points: &[V3], least: f32) -> usize {
    let sign = |v: f32| if v > 1e-3 { 1 } else if v < -1e-3 { -1 } else { 0 };
    let heading = |a: V3, b: V3| [0, 1, 2].map(|i| sign(b[i] - a[i]));
    let along_axis = |h: [i8; 3]| h.iter().filter(|v| **v != 0).count() == 1;
    let n = points.len();
    if n < 4 {
        return 0;
    }
    (1..n - 2)
        .filter(|&i| {
            // The stretch from corner i to corner i + 1.
            let (before, along, after) = (heading(points[i - 1], points[i]), heading(points[i], points[i + 1]), heading(points[i + 1], points[i + 2]));
            along_axis(before) && along_axis(along) && along_axis(after) && before != after && dist(points[i], points[i + 1]) < least
        })
        .count()
}

#[cfg(test)]
mod tests;
