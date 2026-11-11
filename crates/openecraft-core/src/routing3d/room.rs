//! Routing through a room ([`crate::panel::rooms`]): the room's free space between its
//! walls, its inner walls, the cabinets standing in it and the swing of its doors, with
//! the cable ducts and pipe runs on its walls, ceiling and floor as channels; and the
//! way from a device in a cabinet into the room through a cable gland of the cabinet.
//! A route between ends in different line-ups of one room runs in legs: through the
//! cabinet to its gland, through the room, through the other cabinet; between rooms
//! also through the rooms between them and the wall penetrations joining them.
//!
//! Room coordinates are those of the room's items (x and z of the floor plan, y down
//! from the ceiling); in the grid they are shifted to the room's place in the layout
//! ([`crate::panel::positions`]) like the coordinates of an enclosure.

use super::grid::Space;
use super::{entries, End, Glands, RoutingRules};
use crate::model::Project;
use crate::panel::{self, Face, PanelItem, SitePose};
use crate::parts::vec3::{add, manhattan, norm, sub, V3};
use openecraft_i18n::tr;
use std::collections::{HashMap, VecDeque};

/// Faces of a cabinet through which a route leaves it into the room; one hanging on
/// the wall also through its bottom ([`room_side`], [`shafts_below`]).
const ROOM_SIDES: [Face; 4] = [Face::Roof, Face::Left, Face::Right, Face::Rear];
/// How far a shaft below a gland in the bottom of a hanging cabinet reaches beyond the
/// gland on every side, mm.
const SHAFT_MARGIN: f32 = 50.0;

/// Whether a route leaves cabinet `ci` into the room through `face`: its roof, sides and
/// rear, and the bottom of a cabinet hanging on the wall above the floor.
fn room_side(prj: &Project, ci: usize, face: Face) -> bool {
    ROOM_SIDES.contains(&face) || (face == Face::Bottom && hangs(prj, ci))
}

/// The line-up of cabinet `ci` hangs on the wall: its bottom is above the floor.
fn hangs(prj: &Project, ci: usize) -> bool {
    let l = &prj.layout;
    panel::lineup_of(l, ci).first().and_then(|&i| l.enclosures[i].elevation).is_some_and(|h| h > 0.0)
}


/// How far outside its cable gland a route through the room starts, mm.
const OUTSIDE: f32 = 60.0;
/// Smallest edge of the cells of a room's grid, mm: rooms are large, their routes
/// run along walls and pipe runs.
pub const ROOM_GRID: f32 = 50.0;
/// Routes in a room keep within this distance of its walls, its ceiling and what
/// stands in it, where they can.
pub const KEEP_TO_SURFACES: f32 = 300.0;

/// A point of cabinet `ci` (enclosure coordinates) in room `ri` (room coordinates).
pub fn cabinet_in_room(prj: &Project, poses: &[SitePose], ri: usize, ci: usize, q: V3) -> V3 {
    let (r, c) = (&prj.layout.enclosures[ri], &prj.layout.enclosures[ci]);
    let l = poses[ri].local(poses[ci].apply([q[0], c.height - q[1], q[2]]));
    [l[0], r.height - l[1], l[2]]
}

/// The box cabinet `ci` takes in room `ri` (room coordinates), from the floor (with its
/// plinth) to its top.
pub fn cabinet_box(prj: &Project, poses: &[SitePose], ri: usize, ci: usize) -> (V3, V3) {
    let (r, c) = (&prj.layout.enclosures[ri], &prj.layout.enclosures[ci]);
    let (lo, hi) = panel::bounds(&panel::footprint(prj, ri, ci));
    let top = poses[ci].at[1] - poses[ri].at[1] + c.height;
    ([lo[0], r.height - top, lo[1]], [hi[0], r.height, hi[1]])
}

/// Below a cabinet hanging on the wall, the shafts from the cable glands in its bottom
/// down to the floor and forward out from under it (room coordinates): routes come up
/// through them to the glands.
pub fn shafts_below(prj: &Project, poses: &[SitePose], ri: usize, ci: usize) -> Vec<(V3, V3)> {
    if !hangs(prj, ci) {
        return Vec::new();
    }
    let (r, e) = (&prj.layout.enclosures[ri], &prj.layout.enclosures[ci]);
    entries(prj, ci)
        .into_iter()
        .filter(|g| g.face == Face::Bottom)
        .map(|g| {
            let at = panel::face_point(e, Face::Bottom, [g.x + g.w / 2.0, g.y + g.h / 2.0, 0.0]);
            let half = g.w.max(g.h) / 2.0 + SHAFT_MARGIN;
            let (a, b) = (cabinet_in_room(prj, poses, ri, ci, at), cabinet_in_room(prj, poses, ri, ci, [at[0], at[1], e.depth + half]));
            let (lo, hi) = ([0, 1, 2].map(|k| a[k].min(b[k])), [0, 1, 2].map(|k| a[k].max(b[k])));
            ([lo[0] - half, lo[1], lo[2] - half], [hi[0] + half, r.height, hi[2] + half])
        })
        .collect()
}

/// The boxes cabinet `ci` blocks in room `ri` (room coordinates): its box
/// ([`cabinet_box`]), below a cabinet hanging on the wall without its shafts
/// ([`shafts_below`]).
pub fn cabinet_blocks(prj: &Project, poses: &[SitePose], ri: usize, ci: usize) -> Vec<(V3, V3)> {
    let (lo, hi) = cabinet_box(prj, poses, ri, ci);
    let shafts = shafts_below(prj, poses, ri, ci);
    let Some(bottom) = shafts.iter().map(|s| s.0[1]).reduce(f32::min) else { return vec![(lo, hi)] };
    // The cabinet itself, and the space below it in the plan cells between the edges of
    // the footprint and of the shafts that no shaft takes.
    let mut out = vec![(lo, [hi[0], bottom, hi[2]])];
    let edges = |k: usize| {
        let mut v: Vec<f32> = [lo[k], hi[k]].into_iter().chain(shafts.iter().flat_map(|s| [s.0[k], s.1[k]])).map(|c| c.clamp(lo[k], hi[k])).collect();
        v.sort_by(f32::total_cmp);
        v.dedup();
        v
    };
    let (xs, zs) = (edges(0), edges(2));
    for x in xs.windows(2) {
        for z in zs.windows(2) {
            let mid = [(x[0] + x[1]) / 2.0, (z[0] + z[1]) / 2.0];
            if !shafts.iter().any(|s| mid[0] > s.0[0] && mid[0] < s.1[0] && mid[1] > s.0[2] && mid[1] < s.1[2]) {
                out.push(([x[0], bottom, z[0]], [x[1], hi[1], z[1]]));
            }
        }
    }
    out
}

/// The grid of room `ri` (`pos`: [`panel::positions`], `poses`: [`panel::site_poses`]).
pub fn room_space(prj: &Project, ri: usize, pos: &[V3], poses: &[SitePose], rules: &RoutingRules) -> Space {
    let e = &prj.layout.enclosures[ri];
    let o = pos[ri];
    let pts = panel::outline(e);
    let (lo, hi) = panel::bounds(&pts);
    let mut s = Space::empty([o[0] + lo[0], o[1], o[2] + lo[1]], [o[0] + hi[0], o[1] + e.height, o[2] + hi[1]], rules.grid.max(ROOM_GRID));
    s.free_where(ri, |c| {
        let l = sub(c, o);
        l[1] > 0.0 && l[1] < e.height && panel::inside_outline(&pts, [l[0], l[2]])
    });
    let shift = |(a, b): (V3, V3)| (add(a, o), add(b, o));
    let plan_box = |fp: &[[f32; 2]], top: f32| {
        let (lo, hi) = panel::bounds(fp);
        shift(([lo[0], e.height - top, lo[1]], [hi[0], e.height, hi[1]]))
    };
    for w in &e.inner_walls {
        s.block_box(plan_box(&panel::inner_footprint(w), e.height));
    }
    for door in &e.items {
        if let Some(zone) = panel::door_zone(e, door) {
            s.block_box(plan_box(&zone, door.h));
        }
    }
    for ci in panel::cabinets_in(&prj.layout, ri) {
        for b in cabinet_blocks(prj, poses, ri, ci) {
            s.block_box(shift(b));
        }
    }
    s.add_enclosure(e, ri, o, rules, prj.layout.duct_fill_limit, &super::grid::item_parts(prj, e));
    s.add_room(pts, o, e.height);
    s.mark_open(KEEP_TO_SURFACES);
    s.measure();
    s
}

/// Where a route of `diameter` leaves the cabinet `ci` standing in room `ri` towards
/// `near` (room coordinates): a cable gland on the roof, a side or the rear of the
/// cabinet (or the bottom of one hanging on the wall, towards a place below it),
/// otherwise of its line-up ([`Glands::pick`]), otherwise the middle of its roof
/// (`true`). Gives the end in the cabinet (leaving it into the cabinet), where the way
/// through the room starts (layout coordinates) and the direction it leaves there,
/// away from the cabinet; `Err` with the cabinet where another cable took every gland.
#[allow(clippy::too_many_arguments)]
pub(super) fn room_entry(prj: &Project, pos: &[V3], poses: &[SitePose], ri: usize, ci: usize, near: V3, glands: &mut Glands, diameter: f32) -> Result<(End, V3, V3, bool), usize> {
    // Through the bottom of a cabinet hanging on the wall only to a place below it.
    let below = |k: usize| near[1] > prj.layout.enclosures[ri].height - (poses[k].at[1] - poses[ri].at[1]);
    let way_out = |k: usize, face: Face| room_side(prj, k, face) && (face != Face::Bottom || below(k));
    let has_gland = |k: usize| entries(prj, k).iter().any(|g| way_out(k, g.face));
    let lineup = panel::lineup_of(&prj.layout, ci);
    let cabinets = if has_gland(ci) { vec![ci] } else { lineup };
    // The end inside cabinet `k` at a point of its face, the way through the room
    // starting outside it, and the directions out of both.
    let through = |k: usize, face: Face, c: [f32; 2], tag: String, item: Option<u64>| {
        let e = &prj.layout.enclosures[k];
        let at = |depth: f32| panel::face_point(e, face, [c[0], c[1], depth]);
        let leave = norm(sub(at(6.0), at(5.0)));
        let end = End { tag, enclosure: Some(k), point: Some(add(at(5.0), pos[k])), item, leave: Some(leave), ..Default::default() };
        let in_room = |depth: f32| cabinet_in_room(prj, poses, ri, k, at(depth));
        let outside = in_room(-OUTSIDE);
        (end, add(outside, pos[ri]), norm(sub(outside, in_room(-OUTSIDE + 1.0))))
    };
    let mut candidates: Vec<(&PanelItem, f32)> = Vec::new();
    let mut cabinet_of: HashMap<u64, usize> = HashMap::new();
    for k in cabinets {
        let e = &prj.layout.enclosures[k];
        for g in entries(prj, k).into_iter().filter(|g| way_out(k, g.face)) {
            let c = [g.x + g.w / 2.0, g.y + g.h / 2.0];
            let outside = cabinet_in_room(prj, poses, ri, k, panel::face_point(e, g.face, [c[0], c[1], -OUTSIDE]));
            candidates.push((g, manhattan(outside, near)));
            cabinet_of.insert(g.id, k);
        }
    }
    match glands.pick(prj, candidates, diameter) {
        super::Entry::Through(g) => {
            let k = cabinet_of[&g.id];
            let (end, at, leave) = through(k, g.face, [g.x + g.w / 2.0, g.y + g.h / 2.0], g.part.clone(), Some(g.id));
            Ok((end, at, leave, false))
        }
        super::Entry::Taken => Err(ci),
        super::Entry::None => {
            let e = &prj.layout.enclosures[ci];
            let (end, at, leave) = through(ci, Face::Roof, [e.width / 2.0, e.depth / 2.0], tr("Roof").to_string(), None);
            Ok((end, at, leave, true))
        }
    }
}

/// A stretch of a route to be routed in one line-up or room: from `a` to `b` (layout
/// coordinates), leaving each of them in the direction `leave` (zero: any way),
/// passing the items `ignore` (the cable glands it goes through).
#[derive(Clone, Debug, PartialEq)]
pub(super) struct LegPlan {
    pub lineup: usize,
    pub a: V3,
    pub b: V3,
    pub leave: [V3; 2],
    pub ignore: Vec<u64>,
}

/// The rooms a route passes from room `a` to room `b`: the ways through their walls
/// in order, through as few walls as possible (empty within one room); `None` if no
/// chain of wall penetrations joins them.
pub(super) fn room_chain(links: &[panel::WallLink], a: usize, b: usize) -> Option<Vec<panel::WallLink>> {
    let both: Vec<panel::WallLink> = links.iter().flat_map(|l| [*l, l.reversed()]).collect();
    let mut came: HashMap<usize, Option<panel::WallLink>> = HashMap::from([(a, None)]);
    let mut queue = VecDeque::from([a]);
    while let Some(r) = queue.pop_front() {
        if r == b {
            break;
        }
        for l in both.iter().filter(|l| l.from == r) {
            if let std::collections::hash_map::Entry::Vacant(v) = came.entry(l.to) {
                v.insert(Some(*l));
                queue.push_back(l.to);
            }
        }
    }
    let mut chain = Vec::new();
    let mut r = b;
    while let Some(l) = *came.get(&r)? {
        chain.push(l);
        r = l.from;
    }
    chain.reverse();
    Some(chain)
}

/// Of the wall penetrations joining the rooms of `l`, the one with the shortest way
/// from `from` (room coordinates of `l.from`) through it to `to` (of `l.to`).
pub(super) fn nearest_link(links: &[panel::WallLink], l: panel::WallLink, from: V3, to: V3) -> panel::WallLink {
    let way = |k: &panel::WallLink| manhattan(from, k.a) + manhattan(k.b, to);
    links
        .iter()
        .flat_map(|k| [*k, k.reversed()])
        .filter(|k| k.from == l.from && k.to == l.to)
        .min_by(|a, b| way(a).total_cmp(&way(b)).then(a.item.cmp(&b.item)))
        .unwrap_or(l)
}

/// The legs of a route between two placed ends in rooms `rooms` (in a room itself or
/// in a cabinet standing there): from a device in a cabinet to its entry, through the
/// rooms and the wall penetrations between them (`links`: [`panel::wall_links`]), from
/// the other entry to the other device. Gives the legs, whether an entry is the middle
/// of a roof and the length of the ways through the walls; `Err` where there is no way
/// (with the cabinet whose cable glands are all taken, where that is why).
#[allow(clippy::too_many_arguments)]
pub(super) fn plan_legs(prj: &Project, pos: &[V3], poses: &[SitePose], lineup_of: &dyn Fn(usize) -> Option<usize>, rooms: [usize; 2], links: &[panel::WallLink], ends: &[End; 2], taken: &mut Glands, diameter: f32) -> Result<(Vec<LegPlan>, bool, f32), Option<usize>> {
    let mut chain = if rooms[0] == rooms[1] { Vec::new() } else { room_chain(links, rooms[0], rooms[1]).ok_or(None)? };
    // An end in room coordinates of its room.
    let in_room = |ri: usize, end: &End| -> Option<V3> {
        let (k, q) = (end.enclosure?, end.point?);
        Some(if k == ri { sub(q, pos[ri]) } else { cabinet_in_room(prj, poses, ri, k, sub(q, pos[k])) })
    };
    let at = [in_room(rooms[0], &ends[0]).ok_or(None)?, in_room(rooms[1], &ends[1]).ok_or(None)?];
    // Through the nearest of the wall penetrations joining two rooms, beside the routes
    // that went through it before.
    for i in 0..chain.len() {
        let from = if i == 0 { at[0] } else { chain[i - 1].b };
        let to = chain.get(i + 1).map_or(at[1], |l| l.a);
        chain[i] = taken.through_wall(prj, nearest_link(links, chain[i], from, to), diameter, from, to);
    }
    // What each end heads for in its room: the other end, or the wall penetration.
    let near = [chain.first().map_or(at[1], |l| l.a), chain.last().map_or(at[0], |l| l.b)];
    let mut roof = false;
    let zero = [0.0; 3];
    let mut through = [(ends[0].point.ok_or(None)?, ends[0].leave.unwrap_or(zero)), (ends[1].point.ok_or(None)?, ends[1].leave.unwrap_or(zero))];
    let mut legs: [Option<LegPlan>; 2] = [None, None];
    let mut glands: Vec<u64> = Vec::new();
    for i in 0..2 {
        let (k, ri) = (ends[i].enclosure.ok_or(None)?, rooms[i]);
        if k == ri {
            continue;
        }
        let (entry, start, out, by_roof) = room_entry(prj, pos, poses, ri, k, near[i], taken, diameter).map_err(Some)?;
        roof |= by_roof;
        glands.extend(entry.item);
        let inside = (entry.point.ok_or(None)?, entry.leave.unwrap_or(zero));
        let (a, b) = if i == 0 { (through[0], inside) } else { (inside, through[1]) };
        legs[i] = Some(LegPlan { lineup: entry.enclosure.and_then(lineup_of).ok_or(None)?, a: a.0, b: b.0, leave: [a.1, b.1], ignore: Vec::new() });
        through[i] = (start, out);
    }
    // Through every room of the chain, from where the route comes in to where it leaves.
    let mut v: Vec<LegPlan> = legs[0].take().into_iter().collect();
    let mut from = (rooms[0], through[0]);
    for l in &chain {
        v.push(LegPlan { lineup: lineup_of(from.0).ok_or(None)?, a: from.1 .0, b: add(l.a, pos[l.from]), leave: [from.1 .1, l.leave[0]], ignore: Vec::new() });
        from = (l.to, (add(l.b, pos[l.to]), l.leave[1]));
    }
    v.push(LegPlan { lineup: lineup_of(from.0).ok_or(None)?, a: from.1 .0, b: through[1].0, leave: [from.1 .1, through[1].1], ignore: Vec::new() });
    v.extend(legs[1].take());
    for leg in &mut v {
        leg.ignore = glands.clone();
    }
    Ok((v, roof, chain.iter().map(|l| l.length).sum()))
}

#[cfg(test)]
mod tests;
