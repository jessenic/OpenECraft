use super::*;
use crate::geom::p;
use crate::model::DeviceData;
use crate::parts::{MountingData, Part, PartRef};
use crate::panel::{add_opening, mount_on_wall, new_room, set_room, through_wall, wall_links, Face};

/// A building `+B1` with two levels (L0 and L1, 3 m each) and a 4 x 3 m room on each,
/// `+R0` below and `+R1` above.
fn building() -> (Project, usize, [usize; 2]) {
    let mut prj = Project::default();
    let bi = add_building(&mut prj, "+B1").unwrap();
    add_level(&mut prj, bi, None, None, None).unwrap();
    let levels: Vec<u64> = prj.layout.buildings[bi].levels.iter().map(|l| l.id).collect();
    let mut rooms = [0; 2];
    for (k, level) in levels.into_iter().enumerate() {
        let room = new_room(&mut prj, &format!("+R{k}"), 4000.0, 3000.0, LEVEL_HEIGHT - SLAB);
        prj.layout.enclosures.push(room);
        rooms[k] = prj.layout.enclosures.len() - 1;
        set_room_level(&mut prj, rooms[k], Some(level), Some(SitePlace::default())).unwrap();
    }
    (prj, bi, rooms)
}

fn level_ids(prj: &Project, bi: usize) -> Vec<u64> {
    prj.layout.buildings[bi].levels.iter().map(|l| l.id).collect()
}

#[test]
fn rooms_stand_on_the_levels_of_their_building() {
    let (mut prj, bi, [r0, r1]) = building();
    let b = &prj.layout.buildings[bi];
    assert_eq!(b.levels.iter().map(|l| (l.name.as_str(), l.elevation, l.height)).collect::<Vec<_>>(), vec![("L0", 0.0, 3000.0), ("L1", 3000.0, 3000.0)]);
    let poses = site_poses(&prj);
    assert_eq!(poses[r0].at, [0.0, 0.0, 0.0]);
    assert_eq!(poses[r1].at, [0.0, 3000.0, 0.0]);
    assert_eq!(building_rooms(&prj.layout, bi), vec![r0, r1]);
    assert_eq!(building_of(&prj.layout, r1), Some(bi));
    // The building moves and turns with its rooms.
    prj.layout.buildings[bi].site = SitePlace { x: 10_000.0, z: 5000.0, angle: 90.0 };
    let poses = site_poses(&prj);
    assert_eq!(poses[r1].at, [10_000.0, 3000.0, 5000.0]);
    assert!((poses[r1].angle - std::f32::consts::FRAC_PI_2).abs() < 1e-6);
    // A level moved up takes its rooms along.
    let l1 = level_ids(&prj, bi)[1];
    set_level(&mut prj, bi, l1, None, Some(3500.0), None).unwrap();
    assert_eq!(site_poses(&prj)[r1].at[1], 3500.0);
}

#[test]
fn new_buildings_go_beside_what_stands_in_the_plant() {
    let (mut prj, bi, _) = building();
    assert_eq!(prj.layout.buildings[bi].site.x, 0.0);
    let b2 = add_building(&mut prj, "+B2").unwrap();
    assert_eq!(prj.layout.buildings[b2].site.x, 4000.0 + BUILDING_GAP);
    assert!(add_building(&mut prj, "+B2").unwrap_err().contains("exists"));
    assert!(add_building(&mut prj, "+R0").unwrap_err().contains("exists"), "a room's name");
    assert_eq!(building_index(&prj.layout, "B2"), Some(b2));
}


#[test]
fn levels_do_not_overlap_and_basements_go_below() {
    let (mut prj, bi, _) = building();
    assert!(add_level(&mut prj, bi, None, Some(4000.0), None).unwrap_err().contains("L1"));
    assert!(add_level(&mut prj, bi, None, None, Some(500.0)).is_err(), "too low");
    let basement = add_level(&mut prj, bi, None, Some(-3000.0), None).unwrap();
    let top = add_level(&mut prj, bi, Some("Roof"), None, None).unwrap();
    let b = &prj.layout.buildings[bi];
    assert_eq!(b.levels.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), vec!["B1", "L0", "L1", "Roof"]);
    assert_eq!(b.levels[0].id, basement);
    assert_eq!(b.levels[3].elevation, 6000.0);
    assert_eq!(level_index(b, "roof"), Some(3));
    assert_eq!(level_index(b, "2"), Some(1));
    assert!(set_level(&mut prj, bi, top, Some("L1"), None, None).unwrap_err().contains("exists"));
}

#[test]
fn removing_a_level_leaves_its_rooms_where_they_stand() {
    let (mut prj, bi, [r0, r1]) = building();
    let [l0, l1] = level_ids(&prj, bi)[..] else { panic!() };
    let s = add_shaft(&mut prj, bi, None, 100.0, 100.0, SHAFT_SIZE, SHAFT_SIZE, l0, l1).unwrap();
    prj.layout.buildings[bi].site = SitePlace { x: 1000.0, z: 0.0, angle: 0.0 };
    remove_level(&mut prj, bi, l1).unwrap();
    let e = &prj.layout.enclosures[r1];
    assert_eq!((e.level, e.site.map(|s| s.x), e.elevation), (0, Some(1000.0), Some(3000.0)));
    assert_eq!(site_poses(&prj)[r1].at, [1000.0, 3000.0, 0.0]);
    // The shaft ends on the level that is left.
    let shaft = &prj.layout.buildings[bi].shafts[0];
    assert_eq!((shaft.id, shaft.bottom, shaft.top), (s, l0, l0));
    assert!(remove_level(&mut prj, bi, l0).unwrap_err().contains("at least one"));
    assert_eq!(prj.layout.enclosures[r0].level, l0);
    remove_building(&mut prj, bi);
    assert!(prj.layout.buildings.is_empty() && prj.layout.enclosures[r0].level == 0);
}

#[test]
fn rooms_without_a_place_line_up_on_their_level() {
    let (mut prj, bi, [r0, _]) = building();
    let l0 = level_ids(&prj, bi)[0];
    let room = new_room(&mut prj, "+R2", 2000.0, 2000.0, 2700.0);
    prj.layout.enclosures.push(room);
    let r2 = prj.layout.enclosures.len() - 1;
    set_room_level(&mut prj, r2, Some(l0), None).unwrap();
    assert_eq!(site_poses(&prj)[r2].at, [4000.0, 0.0, 0.0], "right of {r0}");
    assert!(problems(&prj).is_empty(), "{:?}", problems(&prj).iter().map(|i| i.message()).collect::<Vec<_>>());
    assert!(set_room_level(&mut prj, r2, Some(999_999), None).is_err());
}

#[test]
fn shafts_join_the_rooms_they_pass() {
    let (mut prj, bi, [r0, r1]) = building();
    let [l0, l1] = level_ids(&prj, bi)[..] else { panic!() };
    // Given from the top down: turned round.
    let s = add_shaft(&mut prj, bi, None, 3000.0, 2000.0, SHAFT_SIZE, SHAFT_SIZE, l1, l0).unwrap();
    let shaft = prj.layout.buildings[bi].shafts[0].clone();
    assert_eq!((shaft.name.as_str(), shaft.bottom, shaft.top), ("S1", l0, l1));
    let poses = site_poses(&prj);
    assert_eq!(shaft_rooms(&prj, &poses, bi, &shaft), vec![r0, r1]);
    let links: Vec<WallLink> = wall_links(&prj).into_iter().filter(|l| l.via == Via::Shaft).collect();
    assert_eq!(links.len(), 1);
    let l = links[0];
    assert_eq!((l.from, l.to, l.item), (r0, r1, s));
    // Below the ceiling of the lower room, above the floor of the upper one.
    assert_eq!(l.a, [3300.0, PASS_OUT, 2300.0]);
    assert_eq!(l.b, [3300.0, 2700.0 - PASS_OUT, 2300.0]);
    assert_eq!(l.length, SLAB + 2.0 * PASS_OUT);
    assert!(problems(&prj).is_empty());
    // A shaft outside the rooms joins nothing.
    add_shaft(&mut prj, bi, Some("S9"), 8000.0, 0.0, SHAFT_SIZE, SHAFT_SIZE, l0, l1).unwrap();
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert!(msgs.iter().any(|m| m.contains("S9") && m.contains("no two rooms")), "{msgs:?}");
    assert!(add_shaft(&mut prj, bi, None, 0.0, 0.0, 50.0, 600.0, l0, l1).is_err(), "too narrow");
}

#[test]
fn cabinets_keep_out_of_shafts() {
    let (mut prj, bi, [r0, _]) = building();
    let [l0, l1] = level_ids(&prj, bi)[..] else { panic!() };
    add_shaft(&mut prj, bi, None, 0.0, 0.0, SHAFT_SIZE, SHAFT_SIZE, l0, l1).unwrap();
    let cab = Enclosure { id: prj.new_id(), name: "+CAB1".into(), width: 600.0, height: 1800.0, depth: 400.0, ..Default::default() };
    prj.layout.enclosures.push(cab);
    let ci = prj.layout.enclosures.len() - 1;
    set_room(&mut prj, ci, Some(r0), Some(SitePlace { x: 300.0, z: 0.0, angle: 0.0 })).unwrap();
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert!(msgs.iter().any(|m| m.contains("+CAB1 stands in shaft S1")), "{msgs:?}");
    set_room(&mut prj, ci, Some(r0), Some(SitePlace { x: 2000.0, z: 0.0, angle: 0.0 })).unwrap();
    assert!(problems(&prj).is_empty());
}

#[test]
fn rooms_on_one_level_must_not_overlap_or_be_higher_than_it() {
    let (mut prj, bi, [r0, _]) = building();
    let l0 = level_ids(&prj, bi)[0];
    let room = new_room(&mut prj, "+R2", 2000.0, 2000.0, 3200.0);
    prj.layout.enclosures.push(room);
    let r2 = prj.layout.enclosures.len() - 1;
    set_room_level(&mut prj, r2, Some(l0), Some(SitePlace { x: 3000.0, z: 1000.0, angle: 0.0 })).unwrap();
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert!(msgs.iter().any(|m| m.contains("+R0 and +R2 overlap")), "{msgs:?}");
    assert!(msgs.iter().any(|m| m.contains("+R2 is higher than level L0")), "{msgs:?}");
    assert_eq!(r0, 0);
}

/// Two buildings with a room each, a building entry in the front wall of each.
fn two_buildings() -> (Project, [usize; 2], [u64; 2]) {
    let mut prj = Project::default();
    let mut rooms = [0; 2];
    let mut entries = [0; 2];
    for k in 0..2 {
        let bi = add_building(&mut prj, &format!("+B{}", k + 1)).unwrap();
        let level = prj.layout.buildings[bi].levels[0].id;
        let room = new_room(&mut prj, &format!("+R{}", k + 1), 4000.0, 3000.0, 2700.0);
        prj.layout.enclosures.push(room);
        rooms[k] = prj.layout.enclosures.len() - 1;
        set_room_level(&mut prj, rooms[k], Some(level), Some(SitePlace::default())).unwrap();
        entries[k] = add_opening(&mut prj, rooms[k], Opening::Entry, Face::Wall(2), Some(1900.0), None, None).unwrap();
    }
    (prj, rooms, entries)
}

#[test]
fn links_join_buildings_through_their_entries() {
    let (mut prj, [r1, r2], [a, b]) = two_buildings();
    assert_eq!(prj.layout.buildings[1].site.x, 14_000.0);
    let id = add_site_link(&mut prj, a, b, LinkKind::Trench, vec![[2000.0, 6000.0], [16_000.0, 6000.0]], None).unwrap();
    let link = prj.layout.site_links[0].clone();
    assert_eq!((link.id, link.name.as_str()), (id, "SL1"));
    let poses = site_poses(&prj);
    let path = site_link_path(&prj, &poses, &link).unwrap();
    // Out of the front wall at the entry's middle, down into the trench, along it, up.
    let y = ENTRY_SILL_MID;
    assert_eq!(path.first(), Some(&[2000.0, y, 3000.0 + ROOM_WALL]));
    assert_eq!(path[1], [2000.0, -TRENCH_DEPTH, 3000.0 + ROOM_WALL]);
    assert_eq!(path.last(), Some(&[16_000.0, y, 3000.0 + ROOM_WALL]));
    let length = site_link_length(&prj, &poses, &link).unwrap();
    let expected = 2.0 * (y + TRENCH_DEPTH) + 2.0 * (6000.0 - 3000.0 - ROOM_WALL) + 14_000.0;
    assert!((length - expected).abs() < 0.5, "{length} {expected}");
    let ways: Vec<WallLink> = wall_links(&prj).into_iter().filter(|l| l.via == Via::Site).collect();
    assert_eq!(ways.len(), 1);
    assert_eq!((ways[0].from, ways[0].to, ways[0].item), (r1, r2, id));
    assert!((ways[0].length - (expected + 2.0 * (ROOM_WALL + PASS_OUT))).abs() < 0.5);
    // A given length counts instead.
    prj.layout.site_links[0].length = 50_000.0;
    assert_eq!(site_link_length(&prj, &poses, &prj.layout.site_links[0]), Some(50_000.0));
    // Entries that are gone.
    assert!(add_site_link(&mut prj, a, a, LinkKind::Bridge, Vec::new(), None).is_err());
    prj.layout.enclosures[r2].items.clear();
    let msgs: Vec<String> = problems(&prj).iter().map(|i| i.message()).collect();
    assert!(msgs.iter().any(|m| m.contains("SL1") && m.contains("no longer exists")), "{msgs:?}");
    assert!(wall_links(&prj).is_empty());
}

/// Middle of a new building entry above the floor.
const ENTRY_SILL_MID: f32 = super::super::ENTRY_SILL + super::super::ENTRY_SIZE.1 / 2.0;

#[test]
fn a_cable_bridge_runs_at_the_height_of_the_higher_entry() {
    let (mut prj, _, [a, b]) = two_buildings();
    add_site_link(&mut prj, a, b, LinkKind::Bridge, Vec::new(), Some("BR")).unwrap();
    let poses = site_poses(&prj);
    let path = site_link_path(&prj, &poses, &prj.layout.site_links[0]).unwrap();
    assert_eq!(path.len(), 2, "straight across: {path:?}");
    assert_eq!(LinkKind::from_key("BRIDGE"), Some(LinkKind::Bridge));
}

fn lamp(prj: &mut Project) {
    prj.parts.push(Part { number: "GEN.LAMP".into(), width: 200.0, height: 200.0, depth: 150.0, mounting_data: MountingData { wall_mount: true, ..Default::default() }, ..Default::default() });
    prj.devices.insert("-E1".into(), DeviceData { parts: vec![PartRef { number: "GEN.LAMP".into(), count: 1, total:false }], ..Default::default() });
}

#[test]
fn devices_go_on_the_outside_of_the_building() {
    let (mut prj, _, [r0, r1]) = building();
    lamp(&mut prj);
    let room = &prj.layout.enclosures[r0];
    // The outside of the front wall: seen from outside, from its left end (x = 0).
    assert_eq!(super::super::face_frame(room, Face::Outside(2)), [[0.0, 0.0, 3000.0 + ROOM_WALL], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    assert_eq!(room.face_key(Face::Outside(2)), "outside3");
    assert_eq!(super::super::room_face_from_key(room, "outside3"), Some(Face::Outside(2)));
    assert_eq!(super::super::room_face_from_key(room, "rooftop"), Some(Face::RoofTop));
    assert!(room.faces().contains(&Face::RoofTop));
    let id = mount_on_wall(&mut prj, r0, "-E1", Face::Outside(2), p(1000.0, 300.0)).unwrap();
    let e = &prj.layout.enclosures[r0];
    let it = e.item(id).unwrap();
    let (min, max) = super::super::item_box(e, it);
    assert_eq!((min[2], max[2]), (3000.0 + ROOM_WALL, 3000.0 + ROOM_WALL + 150.0), "stands off the façade");
    // Its cable comes into the room through the wall behind it.
    let (q, into) = through_wall(e, it).unwrap();
    assert_eq!(q, [1100.0, 400.0, 3000.0 - PASS_OUT]);
    assert_eq!(into, [0.0, 0.0, -1.0]);
    let poses = site_poses(&prj);
    assert!(super::super::outside_building(&prj, &poses, r0, it));
    assert!(super::super::validate(&prj, e).is_empty(), "{:?}", super::super::validate(&prj, e).iter().map(|f| &f.message).collect::<Vec<_>>());
    // On the roof of the lower room the upper room stands: not outside.
    let id = mount_on_wall(&mut prj, r0, "-E1", Face::RoofTop, p(1000.0, 1000.0)).unwrap();
    let e = &prj.layout.enclosures[r0];
    let it = e.item(id).unwrap();
    assert!(!super::super::outside_building(&prj, &poses, r0, it));
    let found = super::super::validate(&prj, e);
    assert!(found.iter().any(|f| f.message.contains("another room is above it")), "{:?}", found.iter().map(|f| &f.message).collect::<Vec<_>>());
    // On the roof of the upper room it is.
    let id = mount_on_wall(&mut prj, r1, "-E1", Face::RoofTop, p(1000.0, 1000.0)).unwrap();
    let e = &prj.layout.enclosures[r1];
    assert!(super::super::outside_building(&prj, &poses, r1, e.item(id).unwrap()));
    assert_eq!(through_wall(e, e.item(id).unwrap()).unwrap(), ([1100.0, PASS_OUT, 1075.0], [0.0, 1.0, 0.0]), "standing: 150 deep");
}

#[test]
fn a_wall_with_a_room_behind_it_is_no_facade() {
    let (mut prj, bi, [r0, _]) = building();
    lamp(&mut prj);
    let l0 = level_ids(&prj, bi)[0];
    let room = new_room(&mut prj, "+R2", 4000.0, 3000.0, 2700.0);
    prj.layout.enclosures.push(room);
    let r2 = prj.layout.enclosures.len() - 1;
    // Right behind the front wall of +R0.
    set_room_level(&mut prj, r2, Some(l0), Some(SitePlace { x: 0.0, z: 3000.0 + 2.0 * ROOM_WALL, angle: 0.0 })).unwrap();
    let id = mount_on_wall(&mut prj, r0, "-E1", Face::Outside(2), p(1000.0, 300.0)).unwrap();
    let e = &prj.layout.enclosures[r0];
    let found = super::super::validate(&prj, e);
    assert!(found.iter().any(|f| f.items == vec![id] && f.message.contains("another room is behind it")), "{:?}", found.iter().map(|f| &f.message).collect::<Vec<_>>());
}

#[test]
fn building_ids_count_for_new_ids() {
    let (mut prj, _, _) = building();
    let max = prj.layout.buildings[0].levels.iter().map(|l| l.id).max().unwrap();
    prj.next_id = 1;
    prj.fix_next_id();
    assert!(prj.next_id > max);
}

use super::super::ROOM_WALL;
