//! Buildings and the plant: a building ([`Building`]) has levels on top of each other
//! and shafts through them; its rooms stand on its levels, at places in the building's
//! floor plan, and move with the building. Shafts join the rooms on different levels
//! they pass; links between buildings ([`SiteLink`]: a cable trench, a conduit or a
//! cable bridge) join a building entry of one building to a building entry of another.
//! The routing runs cables, tubes and pipes through both like through a wall
//! penetration ([`shaft_links`], [`site_ways`]).
//!
//! Building coordinates: x and z of the floor plan from the building's place in the
//! plant ([`building_pose`]), y up from its ground level.

use super::rooms::{convex_overlap, PASS_OUT};
use super::{entry_direction, entry_point, floor_footprint, footprint, inside_outline, outline, rooms, side, site_poses, cabinets_in, Enclosure, ItemKind, Layout, Opening, PanelItem, SitePlace, SitePose, Via, WallLink, Face};
use crate::issue::{Issue, Severity};
use crate::model::panel::{Building, Level, LinkKind, Shaft, SiteLink};
use crate::model::Project;
use crate::parts::vec3::V3;
use openecraft_i18n::{tr, tr_fmt};

/// Height of a new level (floor to floor), mm.
pub const LEVEL_HEIGHT: f32 = 3000.0;
/// Ceiling slab of a level: a new room on a level is this much lower than the level.
pub const SLAB: f32 = 300.0;
/// Width and depth of a new shaft, mm.
pub const SHAFT_SIZE: f32 = 600.0;
/// Gap between a new building and what stands in the plant already, mm.
pub const BUILDING_GAP: f32 = 10_000.0;
/// Depth of a cable trench or conduit below the ground, mm.
pub const TRENCH_DEPTH: f32 = 800.0;
/// Smallest height of a level and smallest width and depth of a shaft, mm.
const MIN_LEVEL: f32 = 1000.0;
const MIN_SHAFT: f32 = 100.0;

impl LinkKind {
    pub const ALL: [LinkKind; 3] = [LinkKind::Trench, LinkKind::Conduit, LinkKind::Bridge];

    pub fn title(self) -> &'static str {
        match self {
            LinkKind::Trench => tr("Cable trench"),
            LinkKind::Conduit => tr("Conduit"),
            LinkKind::Bridge => tr("Cable bridge"),
        }
    }

    /// Key on the command line.
    pub fn key(self) -> &'static str {
        match self {
            LinkKind::Trench => "trench",
            LinkKind::Conduit => "conduit",
            LinkKind::Bridge => "bridge",
        }
    }

    pub fn from_key(key: &str) -> Option<LinkKind> {
        let k = key.trim().to_lowercase();
        LinkKind::ALL.into_iter().find(|x| x.key() == k)
    }
}

/// Where a building stands in the plant: its origin on the ground, turned about the
/// vertical.
pub fn building_pose(b: &Building) -> SitePose {
    SitePose { at: [b.site.x, 0.0, b.site.z], angle: b.site.angle.to_radians() }
}

/// The building and the index of a level (by its id).
pub fn find_level(layout: &Layout, id: u64) -> Option<(usize, usize)> {
    if id == 0 {
        return None;
    }
    layout.buildings.iter().enumerate().find_map(|(bi, b)| b.levels.iter().position(|l| l.id == id).map(|li| (bi, li)))
}

/// A level by its id.
pub fn level_by_id(layout: &Layout, id: u64) -> Option<&Level> {
    find_level(layout, id).map(|(bi, li)| &layout.buildings[bi].levels[li])
}

/// The building a room stands in (on one of its levels).
pub fn building_of(layout: &Layout, ri: usize) -> Option<usize> {
    let e = layout.enclosures.get(ri).filter(|e| e.is_room())?;
    find_level(layout, e.level).map(|(bi, _)| bi)
}

/// A building by its name (with or without the leading `+`).
pub fn building_index(layout: &Layout, name: &str) -> Option<usize> {
    let n = name.trim();
    layout.buildings.iter().position(|b| b.name == n).or_else(|| layout.buildings.iter().position(|b| !n.is_empty() && b.name.trim_start_matches('+') == n.trim_start_matches('+')))
}

/// A level of building `bi` by its name or number (1 = the lowest).
pub fn level_index(b: &Building, key: &str) -> Option<usize> {
    let k = key.trim();
    b.levels.iter().position(|l| l.name.eq_ignore_ascii_case(k)).or_else(|| k.parse::<usize>().ok().filter(|n| *n >= 1 && *n <= b.levels.len()).map(|n| n - 1))
}

/// The rooms on a level.
pub fn rooms_on(layout: &Layout, level: u64) -> Vec<usize> {
    rooms(layout).into_iter().filter(|&r| level != 0 && layout.enclosures[r].level == level).collect()
}

/// The rooms of a building, level by level from the lowest up.
pub fn building_rooms(layout: &Layout, bi: usize) -> Vec<usize> {
    layout.buildings.get(bi).map_or_else(Vec::new, |b| b.levels.iter().flat_map(|l| rooms_on(layout, l.id)).collect())
}

/// Floor plan corners (x, z in the plant) of everything standing in the plant, and of
/// the rooms of building `except` not.
fn placed_corners(prj: &Project, except: Option<usize>) -> Vec<[f32; 2]> {
    let poses = site_poses(prj);
    let layout = &prj.layout;
    let mut out = Vec::new();
    for (i, e) in layout.enclosures.iter().enumerate() {
        if except.is_some_and(|b| building_of(layout, i) == Some(b)) {
            continue;
        }
        let pts = if e.is_room() { outline(e) } else { vec![[0.0, 0.0], [e.width, 0.0], [e.width, e.depth], [0.0, e.depth]] };
        out.extend(pts.into_iter().map(|[x, z]| {
            let q = poses[i].apply([x, 0.0, z]);
            [q[0], q[2]]
        }));
    }
    out
}

/// Names of levels: `B1`, `B2`, … below the ground, `L0`, `L1`, … from it up.
fn level_name(b: &Building, elevation: f32) -> String {
    let below = elevation < -0.5;
    let n = b.levels.iter().filter(|l| (l.elevation < -0.5) == below && if below { l.elevation > elevation } else { l.elevation < elevation }).count();
    let base = if below { format!("B{}", n + 1) } else { format!("L{n}") };
    unique(b.levels.iter().map(|l| l.name.as_str()), base)
}

/// `base`, or `base`-2, -3, … if a name in `taken` is that already.
fn unique<'a>(taken: impl Iterator<Item = &'a str> + Clone, base: String) -> String {
    if !taken.clone().any(|t| t == base) {
        return base;
    }
    (2..).map(|k| format!("{base}-{k}")).find(|n| !taken.clone().any(|t| t == n)).unwrap_or(base)
}

/// A name for a new building: "+B1", "+B2", … not used by a building or an enclosure.
pub fn free_building_name(layout: &Layout) -> String {
    let taken = |n: &str| layout.buildings.iter().any(|b| b.name == n) || layout.enclosures.iter().any(|e| e.name == n);
    (1..).map(|n| format!("+B{n}")).find(|n| !taken(n)).unwrap_or_default()
}

/// Adds a building with one level at ground level, right of what stands in the plant
/// already; gives its index.
pub fn add_building(prj: &mut Project, name: &str) -> Result<usize, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(tr("A building needs a name").to_string());
    }
    if building_index(&prj.layout, name).is_some_and(|b| prj.layout.buildings[b].name == name) || prj.layout.enclosures.iter().any(|e| e.name == name) {
        return Err(tr_fmt!("{} exists already", name));
    }
    let corners = placed_corners(prj, None);
    let x = if corners.is_empty() { 0.0 } else { super::bounds(&corners).1[0] + BUILDING_GAP };
    let id = prj.new_id().max(1);
    let level = Level { id: prj.new_id().max(1), name: "L0".into(), elevation: 0.0, height: LEVEL_HEIGHT };
    prj.layout.buildings.push(Building { id, name: name.into(), site: SitePlace { x: x.round(), z: 0.0, angle: 0.0 }, levels: vec![level], ..Default::default() });
    Ok(prj.layout.buildings.len() - 1)
}

/// Takes a room off its level: it stays where it stands in the plant.
fn release_room(prj: &mut Project, poses: &[SitePose], ri: usize) {
    let p = poses[ri];
    let e = &mut prj.layout.enclosures[ri];
    e.level = 0;
    e.site = Some(SitePlace { x: p.at[0].round(), z: p.at[2].round(), angle: p.angle.to_degrees().rem_euclid(360.0) });
    e.elevation = (p.at[1].abs() > 0.5).then_some(p.at[1].round());
}

/// Removes a building; its rooms stay where they stand in the plant.
pub fn remove_building(prj: &mut Project, bi: usize) {
    if bi >= prj.layout.buildings.len() {
        return;
    }
    let poses = site_poses(prj);
    for ri in building_rooms(&prj.layout, bi) {
        release_room(prj, &poses, ri);
    }
    prj.layout.buildings.remove(bi);
}

/// What overlaps a level of building `bi` from `elevation` `height` mm up (leaving out
/// level `except`): the name of the first other level in that range.
fn overlapping_level(b: &Building, except: u64, elevation: f32, height: f32) -> Option<String> {
    b.levels.iter().filter(|l| l.id != except).find(|l| elevation < l.elevation + l.height - 0.5 && l.elevation < elevation + height - 0.5).map(|l| l.name.clone())
}

fn sort_levels(b: &mut Building) {
    b.levels.sort_by(|x, y| x.elevation.total_cmp(&y.elevation));
}

/// Adds a level to building `bi`: by default on top of the highest one,
/// [`LEVEL_HEIGHT`] high; a negative elevation makes it a basement. Gives its id.
pub fn add_level(prj: &mut Project, bi: usize, name: Option<&str>, elevation: Option<f32>, height: Option<f32>) -> Result<u64, String> {
    let b = prj.layout.buildings.get(bi).ok_or_else(|| tr("No such building").to_string())?;
    let top = b.levels.iter().map(|l| l.elevation + l.height).fold(None, |m: Option<f32>, v| Some(m.map_or(v, |m| m.max(v))));
    let elevation = elevation.unwrap_or(top.unwrap_or(0.0));
    let height = height.unwrap_or(LEVEL_HEIGHT);
    if height.is_nan() || height < MIN_LEVEL || !elevation.is_finite() {
        return Err(tr_fmt!("A level is at least {} mm high", MIN_LEVEL));
    }
    if let Some(other) = overlapping_level(b, 0, elevation, height) {
        return Err(tr_fmt!("The level would overlap level {}", other));
    }
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if b.levels.iter().any(|l| l.name == n) => return Err(tr_fmt!("{} exists already", n)),
        Some(n) => n.to_string(),
        None => level_name(b, elevation),
    };
    let id = prj.new_id().max(1);
    let b = &mut prj.layout.buildings[bi];
    b.levels.push(Level { id, name, elevation, height });
    sort_levels(b);
    Ok(id)
}

/// Changes the name, elevation or height of a level; the rooms on it move with it.
pub fn set_level(prj: &mut Project, bi: usize, id: u64, name: Option<&str>, elevation: Option<f32>, height: Option<f32>) -> Result<(), String> {
    let b = prj.layout.buildings.get(bi).ok_or_else(|| tr("No such building").to_string())?;
    let l = b.levels.iter().find(|l| l.id == id).ok_or_else(|| tr("No such level").to_string())?;
    let (elevation, height) = (elevation.unwrap_or(l.elevation), height.unwrap_or(l.height));
    if height.is_nan() || height < MIN_LEVEL || !elevation.is_finite() {
        return Err(tr_fmt!("A level is at least {} mm high", MIN_LEVEL));
    }
    if let Some(other) = overlapping_level(b, id, elevation, height) {
        return Err(tr_fmt!("The level would overlap level {}", other));
    }
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    if let Some(n) = name.filter(|n| b.levels.iter().any(|l| l.id != id && l.name == *n)) {
        return Err(tr_fmt!("{} exists already", n));
    }
    let b = &mut prj.layout.buildings[bi];
    if let Some(l) = b.levels.iter_mut().find(|l| l.id == id) {
        l.elevation = elevation;
        l.height = height;
        if let Some(n) = name {
            l.name = n.into();
        }
    }
    sort_levels(b);
    Ok(())
}

/// Removes a level: its rooms leave the building (they stay where they stand), shafts
/// ending on it end on the next level they pass instead (or go with it).
pub fn remove_level(prj: &mut Project, bi: usize, id: u64) -> Result<(), String> {
    let b = prj.layout.buildings.get(bi).ok_or_else(|| tr("No such building").to_string())?;
    if !b.levels.iter().any(|l| l.id == id) {
        return Err(tr("No such level").to_string());
    }
    if b.levels.len() == 1 {
        return Err(tr("A building keeps at least one level").to_string());
    }
    let poses = site_poses(prj);
    for ri in rooms_on(&prj.layout, id) {
        release_room(prj, &poses, ri);
    }
    let b = &mut prj.layout.buildings[bi];
    let ids: Vec<u64> = b.levels.iter().map(|l| l.id).collect();
    let k = ids.iter().position(|&l| l == id).unwrap_or(0);
    let pos = |l: u64| ids.iter().position(|&x| x == l);
    b.shafts.retain_mut(|s| {
        let (Some(lo), Some(hi)) = (pos(s.bottom), pos(s.top)) else { return true };
        if lo == hi && lo == k {
            return false;
        }
        let (above, below) = (ids.get(k + 1).copied(), k.checked_sub(1).map(|b| ids[b]));
        match (s.bottom == id, s.top == id) {
            (true, _) if above.is_none() => false,
            (_, true) if below.is_none() => false,
            (b, t) => {
                if b {
                    s.bottom = above.unwrap_or(s.bottom);
                }
                if t {
                    s.top = below.unwrap_or(s.top);
                }
                true
            }
        }
    });
    b.levels.retain(|l| l.id != id);
    Ok(())
}

/// Puts a room on a level of a building at a place of the building's floor plan
/// (`None`: next to the other rooms of the level), or takes it out of its building
/// (`level` `None`; it stays where it stands in the plant).
pub fn set_room_level(prj: &mut Project, ri: usize, level: Option<u64>, place: Option<SitePlace>) -> Result<(), String> {
    let e = prj.layout.enclosures.get(ri).ok_or_else(|| tr_fmt!("No enclosure {}", ri))?;
    if !e.is_room() {
        return Err(tr_fmt!("{} is not a room", e.name));
    }
    match level {
        None => {
            let poses = site_poses(prj);
            release_room(prj, &poses, ri);
        }
        Some(id) => {
            find_level(&prj.layout, id).ok_or_else(|| tr("No such level").to_string())?;
            let e = &mut prj.layout.enclosures[ri];
            e.level = id;
            e.site = place;
            e.elevation = None;
        }
    }
    Ok(())
}

/// Default height of a new room on a level: the level's height less its ceiling slab.
pub fn room_height_on(layout: &Layout, level: u64) -> f32 {
    level_by_id(layout, level).map_or(LEVEL_HEIGHT, |l| l.height) - SLAB
}

/// Adds a new room of 4 × 3 m, on a level next to its other rooms (as high as the
/// level less its slab) or, without one, outside the buildings; its index.
pub fn add_room(prj: &mut Project, name: &str, level: Option<u64>) -> Result<usize, String> {
    if let Some(id) = level {
        find_level(&prj.layout, id).ok_or_else(|| tr("No such level").to_string())?;
    }
    let height = level.map_or(2800.0, |l| room_height_on(&prj.layout, l));
    let room = super::new_room(prj, name, 4000.0, 3000.0, height);
    prj.layout.enclosures.push(room);
    let ri = prj.layout.enclosures.len() - 1;
    if level.is_some() {
        set_room_level(prj, ri, level, None)?;
    }
    Ok(ri)
}

/// Next free place of a room on a level (building floor plan): right of the rooms that
/// have places there already, along the rear.
pub(super) fn next_place(layout: &Layout, along: &mut std::collections::HashMap<u64, f32>, ri: usize) -> SitePlace {
    let e = &layout.enclosures[ri];
    let start = *along.entry(e.level).or_insert_with(|| {
        rooms_on(layout, e.level)
            .into_iter()
            .filter_map(|r| {
                let o = &layout.enclosures[r];
                let p = o.site?;
                let (lo, hi) = super::bounds(&outline(o));
                // The rear right corner of a turned room roughly: its far x.
                let (s, c) = p.angle.to_radians().sin_cos();
                Some([lo[0], hi[0]].into_iter().flat_map(|x| [lo[1], hi[1]].map(|z| p.x + x * c + z * s)).fold(f32::NEG_INFINITY, f32::max))
            })
            .fold(0.0, f32::max)
    });
    let (lo, hi) = super::bounds(&outline(e));
    along.insert(e.level, start + (hi[0] - lo[0]));
    SitePlace { x: start - lo[0], z: -lo[1], angle: 0.0 }
}

/// Adds a shaft to building `bi` from level `bottom` up to level `top` (ids), its rear
/// left corner at `x`, `z` of the building's floor plan. Gives its id.
#[allow(clippy::too_many_arguments)]
pub fn add_shaft(prj: &mut Project, bi: usize, name: Option<&str>, x: f32, z: f32, width: f32, depth: f32, bottom: u64, top: u64) -> Result<u64, String> {
    let b = prj.layout.buildings.get(bi).ok_or_else(|| tr("No such building").to_string())?;
    let shaft = checked_shaft(b, Shaft { id: 0, name: name.map(str::trim).unwrap_or_default().into(), x, z, width, depth, bottom, top })?;
    let n = b.shafts.len() + 1;
    let name = if shaft.name.is_empty() { unique(b.shafts.iter().map(|s| s.name.as_str()), format!("S{n}")) } else { shaft.name.clone() };
    let id = prj.new_id().max(1);
    prj.layout.buildings[bi].shafts.push(Shaft { id, name, ..shaft });
    Ok(id)
}

/// A shaft of building `b` checked: at least [`MIN_SHAFT`] wide and deep, from one of
/// its levels up to another (turned round if given from the top down).
pub fn checked_shaft(b: &Building, mut s: Shaft) -> Result<Shaft, String> {
    if [s.width, s.depth].iter().any(|v| v.is_nan() || *v < MIN_SHAFT) || !s.x.is_finite() || !s.z.is_finite() {
        return Err(tr_fmt!("A shaft is at least {} mm wide and deep", MIN_SHAFT));
    }
    let at = |id: u64| b.levels.iter().position(|l| l.id == id);
    let (Some(lo), Some(hi)) = (at(s.bottom), at(s.top)) else { return Err(tr("No such level").to_string()) };
    if lo > hi {
        std::mem::swap(&mut s.bottom, &mut s.top);
    }
    Ok(s)
}

/// The rooms of building `bi` that shaft `s` passes (its middle lies in their floor
/// plans, on its levels), from the lowest up.
pub fn shaft_rooms(prj: &Project, poses: &[SitePose], bi: usize, s: &Shaft) -> Vec<usize> {
    let layout = &prj.layout;
    let Some(b) = layout.buildings.get(bi) else { return Vec::new() };
    let at = |id: u64| b.levels.iter().position(|l| l.id == id);
    let (Some(lo), Some(hi)) = (at(s.bottom), at(s.top)) else { return Vec::new() };
    let centre = building_pose(b).apply([s.x + s.width / 2.0, 0.0, s.z + s.depth / 2.0]);
    let mut out: Vec<usize> = b.levels[lo.min(hi)..=hi.max(lo)]
        .iter()
        .flat_map(|l| rooms_on(layout, l.id))
        .filter(|&r| {
            let q = poses[r].local(centre);
            inside_outline(&outline(&layout.enclosures[r]), [q[0], q[2]])
        })
        .collect();
    out.sort_by(|&x, &y| poses[x].at[1].total_cmp(&poses[y].at[1]));
    out
}

/// The ways through the shafts of the buildings: from every room a shaft passes up into
/// the next one it passes, from below the ceiling to above the floor.
pub fn shaft_links(prj: &Project, poses: &[SitePose]) -> Vec<WallLink> {
    let layout = &prj.layout;
    let mut out = Vec::new();
    for (bi, b) in layout.buildings.iter().enumerate() {
        let bp = building_pose(b);
        for s in &b.shafts {
            let centre = bp.apply([s.x + s.width / 2.0, 0.0, s.z + s.depth / 2.0]);
            for w in shaft_rooms(prj, poses, bi, s).windows(2) {
                let (lower, upper) = (w[0], w[1]);
                let (el, eu) = (&layout.enclosures[lower], &layout.enclosures[upper]);
                let gap = poses[upper].at[1] - (poses[lower].at[1] + el.height);
                if gap < -0.5 {
                    continue;
                }
                let (ql, qu) = (poses[lower].local(centre), poses[upper].local(centre));
                out.push(WallLink {
                    from: lower,
                    item: s.id,
                    to: upper,
                    via: Via::Shaft,
                    a: [ql[0], PASS_OUT, ql[2]],
                    b: [qu[0], eu.height - PASS_OUT, qu[2]],
                    length: gap.max(0.0) + 2.0 * PASS_OUT,
                    leave: [[0.0, 1.0, 0.0], [0.0, -1.0, 0.0]],
                });
            }
        }
    }
    out
}

/// Floor plan corners of a shaft in the plant.
pub fn shaft_corners(b: &Building, s: &Shaft) -> [[f32; 2]; 4] {
    let bp = building_pose(b);
    [[s.x, s.z], [s.x + s.width, s.z], [s.x + s.width, s.z + s.depth], [s.x, s.z + s.depth]].map(|[x, z]| {
        let q = bp.apply([x, 0.0, z]);
        [q[0], q[2]]
    })
}

/// The room and the item of a building entry (by its id).
pub fn entry_room(layout: &Layout, id: u64) -> Option<(usize, &PanelItem)> {
    rooms(layout).into_iter().find_map(|r| layout.enclosures[r].items.iter().find(|i| i.id == id && i.opening == Opening::Entry).map(|it| (r, it)))
}

/// Where a building entry of room `ri` comes out of the building: the middle of the
/// opening on the outside of the wall, in the plant (y up from the ground).
fn entry_outside(e: &Enclosure, pose: SitePose, it: &PanelItem) -> Option<V3> {
    let w = side(e, it.face)?;
    let (s, r, n) = (w.start(), w.right(), w.normal);
    let (along, t) = (it.x + it.w / 2.0, e.wall());
    let y = it.y + it.h / 2.0;
    Some(pose.apply([s[0] + r[0] * along - n[0] * t, e.height - y, s[1] + r[1] * along - n[1] * t]))
}

/// The way of a link between buildings in the plant (y up from the ground): out of
/// building entry `a`, down into the ground (a trench or conduit) or up to the height
/// of the higher entry (a cable bridge), through its corners, to building entry `b`.
pub fn site_link_path(prj: &Project, poses: &[SitePose], link: &SiteLink) -> Option<Vec<V3>> {
    let layout = &prj.layout;
    let ((ra, ia), (rb, ib)) = (entry_room(layout, link.a)?, entry_room(layout, link.b)?);
    let pa = entry_outside(&layout.enclosures[ra], poses[ra], ia)?;
    let pb = entry_outside(&layout.enclosures[rb], poses[rb], ib)?;
    let y = match link.kind {
        LinkKind::Bridge => pa[1].max(pb[1]),
        _ => -TRENCH_DEPTH,
    };
    let mut v = vec![pa, [pa[0], y, pa[2]]];
    v.extend(link.points.iter().map(|q| [q[0], y, q[1]]));
    v.push([pb[0], y, pb[2]]);
    v.push(pb);
    v.dedup_by(|p, q| (0..3).all(|k| (p[k] - q[k]).abs() < 0.01));
    Some(v)
}

/// Length of a link between buildings, mm: as given, otherwise along its way
/// ([`site_link_path`]).
pub fn site_link_length(prj: &Project, poses: &[SitePose], link: &SiteLink) -> Option<f32> {
    let path = site_link_path(prj, poses, link)?;
    if link.length > 0.0 {
        return Some(link.length);
    }
    Some(path.windows(2).map(|w| (0..3).map(|k| (w[1][k] - w[0][k]).powi(2)).sum::<f32>().sqrt()).sum())
}

/// The ways between buildings: through every link, from in front of its building entry
/// in one room to in front of the other one.
pub fn site_ways(prj: &Project, poses: &[SitePose]) -> Vec<WallLink> {
    let layout = &prj.layout;
    layout
        .site_links
        .iter()
        .filter_map(|link| {
            let ((ra, ia), (rb, ib)) = (entry_room(layout, link.a)?, entry_room(layout, link.b)?);
            if ra == rb {
                return None;
            }
            let (ea, eb) = (&layout.enclosures[ra], &layout.enclosures[rb]);
            let length = site_link_length(prj, poses, link)? + ea.wall() + eb.wall() + 2.0 * PASS_OUT;
            Some(WallLink { from: ra, item: link.id, to: rb, via: Via::Site, a: entry_point(ea, ia)?, b: entry_point(eb, ib)?, length, leave: [entry_direction(ea, ia)?, entry_direction(eb, ib)?] })
        })
        .collect()
}

/// Adds a link between building entries `a` and `b` (item ids) through the corners
/// `points` of the plant. Gives its id.
pub fn add_site_link(prj: &mut Project, a: u64, b: u64, kind: LinkKind, points: Vec<[f32; 2]>, name: Option<&str>) -> Result<u64, String> {
    let layout = &prj.layout;
    let (Some((ra, _)), Some((rb, _))) = (entry_room(layout, a), entry_room(layout, b)) else {
        return Err(tr("A link joins two building entries").to_string());
    };
    if ra == rb {
        return Err(tr("A link joins building entries of two different rooms").to_string());
    }
    let n = layout.site_links.len() + 1;
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => unique(layout.site_links.iter().map(|l| l.name.as_str()), format!("SL{n}")),
    };
    let id = prj.new_id().max(1);
    prj.layout.site_links.push(SiteLink { id, name, kind, a, b, points, length: 0.0 });
    Ok(id)
}

/// The cells of a room's floor plan ([`super::floor_cells`]) in the plant (x, z), turned
/// with the room.
fn plan_cells(prj: &Project, poses: &[SitePose], ri: usize) -> Vec<[[f32; 2]; 4]> {
    let e = &prj.layout.enclosures[ri];
    super::floor_cells(e)
        .into_iter()
        .map(|(lo, hi)| {
            [[lo[0], lo[2]], [hi[0], lo[2]], [hi[0], hi[2]], [lo[0], hi[2]]].map(|[x, z]| {
                let q = poses[ri].apply([x, 0.0, z]);
                [q[0], q[2]]
            })
        })
        .collect()
}

/// The problems of the buildings: rooms on levels that are gone, higher than their
/// level or overlapping other rooms of their level; levels overlapping each other;
/// shafts that join no rooms, and cabinets or floor-standing devices standing in a
/// shaft; links between buildings whose building entries are gone; the problems of
/// their stairs ([`super::stairs::problems`]), roofs ([`super::roofs::problems`]) and
/// PV arrays ([`super::solar::problems`]), and what stands outside the plot boundary
/// ([`super::plot::problems`]).
pub fn problems(prj: &Project) -> Vec<Issue> {
    let layout = &prj.layout;
    let poses = site_poses(prj);
    let mut v = Vec::new();
    for ri in rooms(layout) {
        let e = &layout.enclosures[ri];
        if e.level != 0 && find_level(layout, e.level).is_none() {
            v.push(crate::issue!(Severity::Warning, None, "{}: the level of the room no longer exists", e.name));
        }
    }
    for (bi, b) in layout.buildings.iter().enumerate() {
        if layout.buildings.iter().filter(|o| o.name == b.name).count() > 1 && layout.buildings.iter().position(|o| o.name == b.name) == Some(bi) {
            v.push(crate::issue!(Severity::Warning, None, "Building {} is defined more than once", b.name));
        }
        for (k, l) in b.levels.iter().enumerate() {
            if let Some(o) = b.levels[k + 1..].iter().find(|o| l.elevation < o.elevation + o.height - 0.5 && o.elevation < l.elevation + l.height - 0.5) {
                v.push(crate::issue!(Severity::Warning, None, "{}: levels {} and {} overlap", b.name, l.name, o.name));
            }
            let on = rooms_on(layout, l.id);
            for (i, &ri) in on.iter().enumerate() {
                let e = &layout.enclosures[ri];
                if e.height > l.height + 0.5 {
                    v.push(crate::issue!(Severity::Warning, None, "{}: room {} is higher than level {}", b.name, e.name, l.name));
                }
                let cells = plan_cells(prj, &poses, ri);
                for &rj in &on[i + 1..] {
                    let other = plan_cells(prj, &poses, rj);
                    if cells.iter().any(|c| other.iter().any(|o| convex_overlap(c, o))) {
                        v.push(crate::issue!(Severity::Warning, None, "{}: rooms {} and {} overlap on level {}", b.name, e.name, layout.enclosures[rj].name, l.name));
                    }
                }
            }
        }
        for s in &b.shafts {
            let passed = shaft_rooms(prj, &poses, bi, s);
            if checked_shaft(b, s.clone()).is_err() {
                v.push(crate::issue!(Severity::Warning, None, "{}: shaft {} has no valid levels or size", b.name, s.name));
            } else if passed.len() < 2 {
                v.push(crate::issue!(Severity::Warning, None, "{}: shaft {} joins no two rooms", b.name, s.name));
            }
            let corners = shaft_corners(b, s);
            for &ri in &passed {
                let e = &layout.enclosures[ri];
                // The shaft in the room's floor plan.
                let local = corners.map(|[x, z]| {
                    let q = poses[ri].local([x, 0.0, z]);
                    [q[0], q[2]]
                });
                for ci in cabinets_in(layout, ri) {
                    if convex_overlap(&footprint(prj, ri, ci), &local) {
                        v.push(crate::issue!(Severity::Warning, None, "{}: {} stands in shaft {}", e.name, layout.enclosures[ci].name, s.name));
                    }
                }
                for it in e.items.iter().filter(|i| i.face == Face::Bottom && matches!(i.kind, ItemKind::Device | ItemKind::Accessory)) {
                    if convex_overlap(&floor_footprint(e, it), &local) {
                        v.push(crate::issue!(Severity::Warning, None, "{}: {} stands in shaft {}", e.name, super::item_name(e, it.id), s.name));
                    }
                }
            }
        }
    }
    for link in &layout.site_links {
        match (entry_room(layout, link.a), entry_room(layout, link.b)) {
            (Some((ra, _)), Some((rb, _))) if ra == rb => v.push(crate::issue!(Severity::Warning, None, "Link {} joins two building entries of the same room", link.name)),
            (Some(_), Some(_)) => {}
            _ => v.push(crate::issue!(Severity::Warning, None, "Link {}: a building entry it joins no longer exists", link.name)),
        }
    }
    v.extend(super::stairs::problems(prj));
    v.extend(super::roofs::problems(prj));
    v.extend(super::solar::problems(prj));
    v.extend(super::solar::strings::problems(prj));
    v.extend(super::plot::problems(prj));
    v
}

#[cfg(test)]
mod tests;
