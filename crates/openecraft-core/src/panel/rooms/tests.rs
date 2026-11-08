use super::*;
use crate::geom::p;
use crate::model::DeviceData;
use crate::parts::{MountingData, Part, PartRef};

fn part(number: &str, wall: bool) -> Part {
    Part { number: number.into(), width: 130.0, height: 186.0, depth: 70.0, mounting_data: MountingData { wall_mount: wall, ..Default::default() }, ..Default::default() }
}

/// A room 4 x 3 m, 2.8 m high, and a cabinet; `-T1` has a wall-mountable part, `-F1`
/// one for rails.
fn project() -> (Project, usize, usize) {
    let mut prj = Project { parts: vec![part("VIC.MPPT", true), Part { number: "GEN.MCB".into(), width: 18.0, height: 90.0, depth: 70.0, ..Default::default() }], ..Default::default() };
    for (tag, number) in [("-T1", "VIC.MPPT"), ("-F1", "GEN.MCB")] {
        prj.devices.insert(tag.into(), DeviceData { parts: vec![PartRef { number: number.into(), count: 1, total:false }], ..Default::default() });
    }
    let room = new_room(&mut prj, "+ROOM1", 4000.0, 3000.0, 2800.0);
    prj.layout.enclosures.push(room);
    let cab = Enclosure { id: prj.new_id(), name: "+CAB1".into(), width: 600.0, height: 1800.0, depth: 400.0, ..Default::default() };
    prj.layout.enclosures.push(cab);
    (prj, 0, 1)
}

#[test]
fn wall_and_outside_mounting_use_the_imported_rail_datum() {
    let (prj, room, _) = project();
    for face in [Face::Wall(0), Face::Outside(0), Face::RoofTop] {
        let mut enclosure = prj.layout.enclosures[room].clone();
        let rail = PanelItem { kind: ItemKind::Rail, face, x: 0., y: 182.5, w: 300., h: 35., ..Default::default() };
        let mut device = PanelItem {
            kind: ItemKind::Device, face, part: "GEN.MCB".into(), x: 100., y: 100., w: 18., h: 100.,
            rail_mount: openecraft_parts::RailMount { center_offset: 50., ..Default::default() },
            ..Default::default()
        };
        assert!(!rail.rect().contains(device.center()));
        assert!(rail.rect().contains(device.rail_anchor()));
        enclosure.items.push(rail);
        assert!(mount_ok(&prj, &enclosure, &device), "{face:?}");
        device.rail_mount.center_offset = 0.;
        assert!(!mount_ok(&prj, &enclosure, &device), "{face:?}");
    }
}

/// An L-shaped floor plan: 6 m wide at the rear, 3 m deep on the left, 6 m deep on the
/// right side of the L.
const L_SHAPE: [[f32; 2]; 6] = [[0.0, 0.0], [6000.0, 0.0], [6000.0, 6000.0], [3000.0, 6000.0], [3000.0, 3000.0], [0.0, 3000.0]];

#[test]
fn a_new_room_is_a_rectangle_of_four_walls() {
    let (prj, r, c) = project();
    let room = &prj.layout.enclosures[r];
    assert!(room.is_room() && !prj.layout.enclosures[c].is_room());
    assert_eq!(room.faces()[..6], [Face::Wall(0), Face::Wall(1), Face::Wall(2), Face::Wall(3), Face::Roof, Face::Bottom]);
    assert_eq!(room.faces()[6..], [Face::Outside(0), Face::Outside(1), Face::Outside(2), Face::Outside(3), Face::RoofTop], "and the outside");
    assert_eq!(room_face_from_key(room, "ceiling"), Some(Face::Roof));
    assert_eq!(room_face_from_key(room, "WALL3"), Some(Face::Wall(2)));
    assert_eq!(room_face_from_key(room, "wall0"), None);
    assert_eq!(room_face_key(room, Face::Wall(2)), "wall3");
    assert_eq!(room.face_size(Face::Wall(0)), (4000.0, 2800.0));
    assert_eq!(room.face_size(Face::Wall(1)), (3000.0, 2800.0));
    assert_eq!(room.face_size(Face::Roof), (4000.0, 3000.0));
    assert_eq!(room.face_size(Face::Wall(9)), (0.0, 0.0), "no such wall");
    // Walls and ceiling only, no door leaf, no mounting plate.
    let roles: Vec<_> = super::super::components(room).into_iter().map(|c| c.role).collect();
    assert_eq!(roles.len(), 5);
    assert!(roles.iter().all(|r| matches!(r, super::super::Role::RoomWall(_))));
}

#[test]
fn walls_are_seen_from_inside_the_room() {
    let (prj, r, _) = project();
    let room = &prj.layout.enclosures[r];
    // The rear wall: x to the right, items stand off towards the front.
    let rear = super::super::face_frame(room, Face::Wall(0));
    assert_eq!(rear, [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    // The right wall (x = 4000): seen from inside its left end is at the rear.
    let right = super::super::face_frame(room, Face::Wall(1));
    assert_eq!(right, [[4000.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]);
    // The front wall runs from right to left in plan.
    let front = super::super::face_frame(room, Face::Wall(2));
    assert_eq!(front, [[4000.0, 0.0, 3000.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, -1.0]]);
    // Wall boxes lie outside the outline and close the corners.
    let (a, b) = super::super::wall_box(room, Face::Wall(0));
    assert_eq!((a, b), ([-ROOM_WALL, -ROOM_WALL, -ROOM_WALL], [4000.0 + ROOM_WALL, 2800.0, 0.0]));
}

#[test]
fn outlines_are_cleaned_up_and_checked() {
    // Repeated corners, a corner in the middle of a wall and the closing corner go.
    let drawn = [[0.0, 0.0], [2000.0, 0.0], [2000.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0], [0.0, 0.0]];
    assert_eq!(normalize_outline(&drawn).unwrap(), vec![[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [0.0, 3000.0]]);
    assert!(normalize_outline(&L_SHAPE).is_ok());
    assert!(check_outline(&[[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0]]).unwrap_err().contains("four"));
    assert!(check_outline(&[[0.0, 0.0], [4000.0, 0.0], [4000.0, 3000.0], [100.0, 2000.0]]).unwrap_err().contains("right angle"));
    assert!(check_outline(&[[0.0, 0.0], [4000.0, 0.0], [4000.0, 20.0], [0.0, 20.0]]).unwrap_err().contains("shorter"));
    // A figure eight: walls 1 and 5 cross.
    let eight = [[0.0, 0.0], [2000.0, 0.0], [2000.0, 2000.0], [1000.0, 2000.0], [1000.0, -1000.0], [0.0, -1000.0]];
    assert!(check_outline(&eight).unwrap_err().contains("cross"));
    // A wall running back over the one before.
    let back = [[0.0, 0.0], [6000.0, 0.0], [6000.0, 3000.0], [3000.0, 3000.0], [3000.0, 0.0]];
    assert!(normalize_outline(&back).unwrap_err().contains("cross"));
}

#[test]
fn walls_move_and_change_length() {
    let mut room = Enclosure { kind: EnclosureKind::Room, height: 2800.0, ..Default::default() };
    set_outline(&mut room, &L_SHAPE).unwrap();
    assert_eq!((room.width, room.depth), (6000.0, 6000.0));
    let ws = walls(&room);
    assert_eq!(ws.len(), 6);
    // The inner walls of the L face into the room.
    assert_eq!(ws[3].normal, [1.0, 0.0]);
    assert_eq!(ws[4].normal, [0.0, -1.0]);
    // Wall 2 (x = 6000) moved out by 500 mm: the room gets wider.
    let moved = moved_wall(&room.outline, 1, 500.0).unwrap();
    assert_eq!((moved[1], moved[2]), ([6500.0, 0.0], [6500.0, 6000.0]));
    // Moving the inner wall into the other one is refused.
    assert!(moved_wall(&room.outline, 3, -3000.0).is_err());
    // The rear wall made 5 m long: the right wall moves with its end.
    let shorter = wall_length_set(&room.outline, 0, 5000.0).unwrap();
    assert_eq!((shorter[1], shorter[2]), ([5000.0, 0.0], [5000.0, 6000.0]));
    // The ceiling covers the L only.
    let cells = floor_cells(&room);
    let area: f32 = cells.iter().map(|(a, b)| (b[0] - a[0]) * (b[2] - a[2])).sum();
    assert_eq!(area, 6000.0 * 3000.0 + 3000.0 * 3000.0);
    assert!(set_outline(&mut Enclosure::default(), &L_SHAPE).is_err(), "cabinets have no outline");
}

#[test]
fn cabinets_stand_in_the_room() {
    let (mut prj, r, c) = project();
    prj.layout.enclosures[r].site = Some(SitePlace { x: 10000.0, z: 5000.0, angle: 0.0 });
    set_room(&mut prj, c, Some(r), None).unwrap();
    assert_eq!(room_of(&prj.layout, c), Some(r));
    assert_eq!(cabinets_in(&prj.layout, r), vec![c]);
    // Without a place: at the rear wall, from the left corner.
    assert_eq!(site_poses(&prj)[c].at, [10000.0, 0.0, 5000.0]);
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 1000.0, z: 500.0, angle: 0.0 })).unwrap();
    assert_eq!(site_poses(&prj)[c].at, [11000.0, 0.0, 5500.0]);
    assert_eq!(room_place(&prj, c), Some(SitePlace { x: 1000.0, z: 500.0, angle: 0.0 }));
    assert_eq!(footprint(&prj, r, c)[2], [1600.0, 900.0]);
    // A turned room turns its cabinets with it.
    prj.layout.enclosures[r].site = Some(SitePlace { x: 0.0, z: 0.0, angle: 90.0 });
    let pose = site_poses(&prj)[c];
    assert!((pose.angle - std::f32::consts::FRAC_PI_2).abs() < 1e-5);
    // Rooms cannot be put into rooms or bayed.
    assert!(set_room(&mut prj, r, Some(r), None).is_err());
    assert!(set_room(&mut prj, c, Some(c), None).is_err());
    assert!(!super::super::bay_right_of(&mut prj, c, r));
    set_room(&mut prj, c, None, None).unwrap();
    assert_eq!(room_of(&prj.layout, c), None);
}

#[test]
fn cabinets_outside_the_room_are_reported() {
    let (mut prj, r, c) = project();
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 3800.0, z: 0.0, angle: 0.0 })).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Outside && f.message.contains("+CAB1")), "{findings:?}");
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 0.0, z: 0.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty(), "against the walls in the corner");
    prj.layout.enclosures[c].height = 3000.0;
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.message.contains("ceiling")), "{findings:?}");
    // In the cut-out corner of an L the cabinet stands outside.
    prj.layout.enclosures[c].height = 1800.0;
    set_outline(&mut prj.layout.enclosures[r], &L_SHAPE).unwrap();
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 1000.0, z: 4000.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).iter().any(|f| f.kind == CheckKind::Outside));
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 4000.0, z: 4000.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
}

#[test]
fn only_wall_mountable_parts_go_directly_on_the_wall() {
    let (mut prj, r, c) = project();
    let rear = Face::Wall(0);
    let id = mount_on_wall(&mut prj, r, "-T1", rear, p(500.0, 1000.0)).unwrap();
    let it = prj.layout.enclosures[r].item(id).unwrap();
    assert_eq!((it.face, it.z, it.w), (rear, 0.0, 130.0));
    // On the rear wall it stands off towards the front of the room.
    let (lo, hi) = super::super::item_box(&prj.layout.enclosures[r], it);
    assert_eq!((lo[0], lo[2], hi[2]), (500.0, 0.0, 70.0));
    // A circuit breaker needs a rail.
    assert!(mount_on_wall(&mut prj, r, "-F1", rear, p(1500.0, 1000.0)).is_err());
    assert_eq!(prj.layout.enclosures[r].items.len(), 1);
    let rail = add_wall_profile(&mut prj, r, ItemKind::Rail, super::super::RAIL_PART, rear, p(1400.0, 1050.0), p(1800.0, 1050.0)).unwrap();
    let f1 = mount_on_wall(&mut prj, r, "-F1", rear, p(1500.0, 1000.0)).unwrap();
    let rail_top = prj.layout.enclosures[r].item(rail).map(|i| i.z + i.d).unwrap();
    assert_eq!(prj.layout.enclosures[r].item(f1).unwrap().z, rail_top, "snapped onto the rail");
    // Cable ducts on the ceiling; parts without device tag as well.
    add_wall_profile(&mut prj, r, ItemKind::Duct, super::super::DUCT_PART, Face::Roof, p(100.0, 500.0), p(3000.0, 500.0)).unwrap();
    assert!(mount_on_wall(&mut prj, r, "VIC.MPPT", Face::Wall(3), p(500.0, 1000.0)).is_ok());
    assert!(mount_on_wall(&mut prj, r, "UNKNOWN", Face::Wall(3), p(500.0, 1000.0)).is_err());
    assert!(mount_on_wall(&mut prj, r, "-T1", Face::Wall(7), p(0.0, 0.0)).is_err(), "no such wall");
    assert!(mount_on_wall(&mut prj, c, "-T1", rear, p(0.0, 0.0)).is_err(), "not a room");
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
    // Moved off the rail, the breaker is reported.
    let it = prj.layout.enclosures[r].items.iter_mut().find(|i| i.id == f1).unwrap();
    it.y = 2000.0;
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Mounting && f.items == vec![f1]), "{findings:?}");
}

#[test]
fn doors_and_windows_in_the_walls() {
    let (mut prj, r, c) = project();
    // A door in the middle of the rear wall, standing on the floor.
    let door = add_opening(&mut prj, r, Opening::Door, Face::Wall(0), None, None, None).unwrap();
    let room = &prj.layout.enclosures[r];
    let d = room.item(door).unwrap();
    assert_eq!((d.kind, d.x, d.w, d.h, opening_sill(room, d)), (ItemKind::Cutout, 1500.0, 1000.0, 2100.0, 0.0));
    // A window on the right wall with its sill at 900 mm.
    let window = add_opening(&mut prj, r, Opening::Window, Face::Wall(1), Some(1000.0), Some((800.0, 1000.0)), None).unwrap();
    let room = &prj.layout.enclosures[r];
    assert_eq!(opening_sill(room, room.item(window).unwrap()), WINDOW_SILL);
    assert!(add_opening(&mut prj, r, Opening::Door, Face::Roof, None, None, None).is_err(), "not in the ceiling");
    assert!(add_opening(&mut prj, r, Opening::Door, Face::Wall(9), None, None, None).is_err(), "no such wall");
    assert!(add_opening(&mut prj, r, Opening::Cutout, Face::Wall(0), None, None, None).is_err());
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
    // The door swings 1 m into the room in front of it.
    let room = &prj.layout.enclosures[r];
    assert_eq!(door_zone(room, room.item(door).unwrap()), Some([[1500.0, 0.0], [2500.0, 0.0], [2500.0, 1000.0], [1500.0, 1000.0]]));
    assert_eq!(door_zone(room, room.item(window).unwrap()), None);
    // A cabinet in its swing is reported, one beside it is not.
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 1800.0, z: 600.0, angle: 0.0 })).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Service && f.items == vec![door] && f.message.contains("+CAB1")), "{findings:?}");
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 2500.0, z: 0.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty(), "right of the door, touching its swing");
    // A rail across the door collides with it.
    let rail = add_wall_profile(&mut prj, r, ItemKind::Rail, super::super::RAIL_PART, Face::Wall(0), p(1000.0, 1200.0), p(2000.0, 1200.0)).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Collision && f.items.contains(&rail) && f.message.contains("Door")), "{findings:?}");
}

#[test]
fn inner_walls_divide_the_room() {
    let (mut prj, r, c) = project();
    // Across the room at x = 2000, from the rear wall to 2 m before the front wall.
    let id = add_inner_wall(&mut prj, r, [2000.0, 0.0], [2000.0, 2000.0], 100.0).unwrap();
    let room = &prj.layout.enclosures[r];
    assert_eq!(room.faces()[..8], [Face::Wall(0), Face::Wall(1), Face::Wall(2), Face::Wall(3), Face::Inner(id, 0), Face::Inner(id, 1), Face::Roof, Face::Bottom]);
    assert_eq!(room_face_key(room, Face::Inner(id, 1)), "inner1b");
    assert_eq!(room_face_from_key(room, "INNER1A"), Some(Face::Inner(id, 0)));
    assert_eq!(room_face_from_key(room, "inner2a"), None);
    assert!(room_face_title(room, Face::Inner(id, 0)).contains("side A"));
    // Both sides are 2 m long and face away from each other.
    let [a, b] = inner_sides(&room.inner_walls[0]);
    assert_eq!((a.length(), a.normal, b.normal), (2000.0, [1.0, 0.0], [-1.0, 0.0]));
    assert_eq!(room.face_size(Face::Inner(id, 1)), (2000.0, 2800.0));
    let (lo, hi) = super::super::wall_box(room, Face::Inner(id, 0));
    assert_eq!((lo, hi), ([1950.0, 0.0, 0.0], [2050.0, 2800.0, 2000.0]));
    assert_eq!(super::super::components(room).len(), 6, "four walls, the inner wall, the ceiling");
    // Seen from side A (x = 2050) devices stand off towards +x.
    let dev = mount_on_wall(&mut prj, r, "-T1", Face::Inner(id, 0), p(500.0, 1000.0)).unwrap();
    let room = &prj.layout.enclosures[r];
    let (lo, _) = super::super::item_box(room, room.item(dev).unwrap());
    assert_eq!(lo[0], 2050.0);
    // A door through it: as deep as the wall is thick.
    let door = add_opening(&mut prj, r, Opening::Door, Face::Inner(id, 1), Some(800.0), None, None).unwrap();
    assert_eq!(prj.layout.enclosures[r].item(door).map(|d| (d.z, d.d)), Some((-100.0, 100.0)));
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
    // A cabinet standing in it is reported.
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 1800.0, z: 2500.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty(), "past its end");
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 1800.0, z: 1500.0, angle: 0.0 })).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).iter().any(|f| f.message.contains("inner wall")));
    // Moved, lengthened, refused when it leaves the room or runs askew.
    let moved = moved_inner_wall(&prj.layout.enclosures[r], id, 500.0, Some(2500.0), Some(150.0)).unwrap();
    assert_eq!((moved.a, moved.b, moved.thickness), ([2500.0, 0.0], [2500.0, 2500.0], 150.0));
    assert!(moved_inner_wall(&prj.layout.enclosures[r], id, 0.0, Some(4000.0), None).unwrap_err().contains("leaves"));
    assert!(add_inner_wall(&mut prj, r, [0.0, 0.0], [1000.0, 1000.0], 100.0).unwrap_err().contains("right angles"));
    // Removed with everything on it.
    remove_inner_wall(&mut prj.layout.enclosures[r], id);
    let room = &prj.layout.enclosures[r];
    assert!(room.inner_walls.is_empty() && room.item(dev).is_none() && room.item(door).is_none());
}

#[test]
fn turning_a_cabinet_keeps_its_footprint_in_place() {
    let (mut prj, r, c) = project();
    let centre = |prj: &Project| {
        let (lo, hi) = bounds(&footprint(prj, r, c));
        [(lo[0] + hi[0]) / 2.0, (lo[1] + hi[1]) / 2.0]
    };
    // In the middle of the room it turns about its centre; four turns bring it back.
    let middle = SitePlace { x: 1700.0, z: 1300.0, angle: 0.0 };
    set_room(&mut prj, c, Some(r), Some(middle)).unwrap();
    let before = centre(&prj);
    let place = turned_place(&prj, c, 90.0).unwrap();
    assert_eq!(place.angle, 90.0);
    prj.layout.enclosures[c].site = Some(place);
    let after = centre(&prj);
    assert!((before[0] - after[0]).abs() < 1.0 && (before[1] - after[1]).abs() < 1.0, "{before:?} {after:?}");
    // 600 wide, 400 deep: turned it is 400 wide and 600 deep.
    let (lo, hi) = bounds(&footprint(&prj, r, c));
    assert!((hi[0] - lo[0] - 400.0).abs() < 1.0 && (hi[1] - lo[1] - 600.0).abs() < 1.0);
    for _ in 0..3 {
        prj.layout.enclosures[c].site = turned_place(&prj, c, 90.0);
    }
    assert_eq!(prj.layout.enclosures[c].site, Some(middle));
    // In the rear left corner it stays inside the room instead of crossing the wall.
    prj.layout.enclosures[c].site = Some(SitePlace { x: 0.0, z: 0.0, angle: 0.0 });
    for _ in 0..4 {
        prj.layout.enclosures[c].site = turned_place(&prj, c, 90.0);
        assert!(stands_inside(&prj, r, c), "{:?}", prj.layout.enclosures[c].site);
    }
    assert_eq!(turned_place(&prj, r, 90.0), None);
}

/// A pump (stands on the floor) as `-P1` and its part, added to [`project`].
fn with_pump(prj: &mut Project) {
    let pump = Part { number: "GEN.PUMP".into(), width: 300.0, height: 400.0, depth: 600.0, mounting_data: MountingData { floor_mount: true, ..Default::default() }, ..Default::default() };
    prj.parts.push(pump);
    prj.devices.insert("-P1".into(), DeviceData { parts: vec![PartRef { number: "GEN.PUMP".into(), count: 1, total:false }], ..Default::default() });
}

#[test]
fn the_floor_is_seen_from_above_like_the_floor_plan() {
    let (mut prj, r, _) = project();
    let room = &prj.layout.enclosures[r];
    assert_eq!(room_face_from_key(room, "floor"), Some(Face::Bottom));
    assert_eq!(room_face_key(room, Face::Bottom), "floor");
    assert_eq!(room.face_title(Face::Bottom), "Room floor");
    assert_eq!(room.face_size(Face::Bottom), (4000.0, 3000.0));
    // x and z of the floor plan, standing upwards from the floor (y down from the ceiling).
    assert_eq!(super::super::face_frame(room, Face::Bottom), [[0.0, 2800.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]]);
    // A pump stands on it: its depth along the floor, its height upwards.
    with_pump(&mut prj);
    let id = mount_on_wall(&mut prj, r, "-P1", Face::Bottom, p(1000.0, 500.0)).unwrap();
    let room = &prj.layout.enclosures[r];
    let it = room.item(id).unwrap();
    assert_eq!((it.w, it.h, it.d, it.z), (300.0, 600.0, 400.0, 0.0));
    assert_eq!(super::super::item_box(room, it), ([1000.0, 2400.0, 500.0], [1300.0, 2800.0, 1100.0]));
    assert_eq!(floor_footprint(room, it), [[1000.0, 500.0], [1300.0, 500.0], [1300.0, 1100.0], [1000.0, 1100.0]]);
    assert!(super::super::validate(&prj, room).is_empty());
}

#[test]
fn only_floor_standing_parts_go_on_the_floor() {
    let (mut prj, r, _) = project();
    with_pump(&mut prj);
    // A wall-mounted part or a circuit breaker does not stand on the floor.
    assert!(mount_on_wall(&mut prj, r, "-T1", Face::Bottom, p(500.0, 500.0)).is_err());
    assert!(mount_on_wall(&mut prj, r, "-F1", Face::Bottom, p(500.0, 500.0)).is_err());
    assert!(prj.layout.enclosures[r].items.is_empty());
    // A pump does not go on the wall, and rails do not go on the floor.
    assert!(mount_on_wall(&mut prj, r, "-P1", Face::Wall(0), p(500.0, 500.0)).is_err());
    assert!(add_wall_profile(&mut prj, r, ItemKind::Rail, super::super::RAIL_PART, Face::Bottom, p(0.0, 100.0), p(1000.0, 100.0)).is_err());
    // Without a device tag as well; a device moved onto the floor is reported.
    assert!(mount_on_wall(&mut prj, r, "GEN.PUMP", Face::Bottom, p(2000.0, 500.0)).is_ok());
    let t1 = mount_on_wall(&mut prj, r, "-T1", Face::Wall(0), p(500.0, 1000.0)).unwrap();
    prj.layout.enclosures[r].items.iter_mut().find(|i| i.id == t1).unwrap().face = Face::Bottom;
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Mounting && f.items == vec![t1] && f.message.contains("floor")), "{findings:?}");
}

#[test]
fn floor_standing_parts_keep_clear_of_doors_walls_and_cabinets() {
    let (mut prj, r, c) = project();
    with_pump(&mut prj);
    let door = add_opening(&mut prj, r, Opening::Door, Face::Wall(0), None, None, None).unwrap();
    let pump = mount_on_wall(&mut prj, r, "-P1", Face::Bottom, p(1800.0, 200.0)).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Service && f.items == vec![pump, door] && f.message.contains("-P1")), "{findings:?}");
    // Beside the door it stands free; on the cabinet's place it does not.
    let move_to = |prj: &mut Project, x: f32, y: f32| {
        let it = prj.layout.enclosures[r].items.iter_mut().find(|i| i.id == pump).unwrap();
        (it.x, it.y) = (x, y);
    };
    move_to(&mut prj, 3000.0, 200.0);
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
    set_room(&mut prj, c, Some(r), Some(SitePlace { x: 2800.0, z: 0.0, angle: 0.0 })).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Collision && f.items == vec![pump] && f.message.contains("+CAB1")), "{findings:?}");
    // Outside the room, and in an inner wall.
    set_room(&mut prj, c, None, None).unwrap();
    move_to(&mut prj, 3800.0, 200.0);
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Outside && f.items == vec![pump]), "{findings:?}");
    move_to(&mut prj, 3000.0, 200.0);
    add_inner_wall(&mut prj, r, [3100.0, 0.0], [3100.0, 1500.0], 100.0).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Collision && f.items == vec![pump] && f.message.contains("inner wall")), "{findings:?}");
}

#[test]
fn pipe_runs_stand_off_the_walls_and_the_ceiling() {
    let (mut prj, r, c) = project();
    // On the rear wall 100 mm into the room, at right angles, snapped.
    let id = add_pipe_run(&mut prj, r, Face::Wall(0), &[p(502.0, 1000.0), p(2500.0, 1000.0), p(2500.0, 400.0)], None, None, None).unwrap();
    let path = prj.layout.enclosures[r].paths.iter().find(|x| x.id == id).unwrap().clone();
    assert_eq!(path.points, vec![[500.0, 1000.0, 100.0], [2500.0, 1000.0, 100.0], [2500.0, 400.0, 100.0]]);
    assert_eq!((path.name.as_str(), path.kind, path.classes.clone(), path.width, path.face), ("PR1", PathKind::Tubes, vec![Class::Fluid], PIPE_RUN_WIDTH, Some(Face::Wall(0))));
    let (face, pts, offset) = pipe_run_on_face(&prj.layout.enclosures[r], &path).unwrap();
    assert_eq!((face, pts, offset), (Face::Wall(0), vec![p(500.0, 1000.0), p(2500.0, 1000.0), p(2500.0, 400.0)], 100.0));
    // The right wall (x = 4000), from the rear towards the front.
    let id = add_pipe_run(&mut prj, r, Face::Wall(1), &[p(500.0, 1000.0), p(1500.0, 1000.0)], Some(150.0), Some(60.0), Some("Water")).unwrap();
    let path = prj.layout.enclosures[r].paths.iter().find(|x| x.id == id).unwrap();
    assert_eq!((path.points.clone(), path.width, path.name.as_str()), (vec![[3850.0, 1000.0, 500.0], [3850.0, 1000.0, 1500.0]], 60.0, "Water"));
    // 300 mm below the ceiling, and along the side of an inner wall.
    let id = add_pipe_run(&mut prj, r, Face::Roof, &[p(1000.0, 500.0), p(3000.0, 500.0)], None, None, None).unwrap();
    assert_eq!(prj.layout.enclosures[r].paths.iter().find(|x| x.id == id).unwrap().points[0], [1000.0, 300.0, 500.0]);
    let wall = add_inner_wall(&mut prj, r, [2000.0, 1000.0], [2000.0, 3000.0], 100.0).unwrap();
    let id = add_pipe_run(&mut prj, r, Face::Inner(wall, 1), &[p(100.0, 800.0), p(1500.0, 800.0)], None, None, None).unwrap();
    let room = &prj.layout.enclosures[r];
    let path = room.paths.iter().find(|x| x.id == id).unwrap();
    assert_eq!(path.points[0], super::super::face_point(room, Face::Inner(wall, 1), [100.0, 800.0, 100.0]));
    assert_eq!(pipe_run_on_face(room, path).unwrap().1, vec![p(100.0, 800.0), p(1500.0, 800.0)]);
    assert_eq!(path.name, "PR4");
    // Diagonals, single points, unknown faces and cabinets are refused.
    assert!(add_pipe_run(&mut prj, r, Face::Wall(0), &[p(0.0, 0.0), p(500.0, 500.0)], None, None, None).is_err());
    assert!(add_pipe_run(&mut prj, r, Face::Wall(0), &[p(0.0, 0.0), p(1.0, 0.0)], None, None, None).is_err());
    assert!(add_pipe_run(&mut prj, r, Face::Wall(9), &[p(0.0, 0.0), p(500.0, 0.0)], None, None, None).is_err());
    assert!(add_pipe_run(&mut prj, c, Face::Plate, &[p(0.0, 0.0), p(500.0, 0.0)], None, None, None).is_err());
}

#[test]
fn pipe_runs_keep_clear_of_doors_and_windows() {
    let (mut prj, r, _) = project();
    // The door in the middle of the rear wall reaches 2.1 m high (y from 700 mm).
    let door = add_opening(&mut prj, r, Opening::Door, Face::Wall(0), None, None, None).unwrap();
    add_pipe_run(&mut prj, r, Face::Wall(0), &[p(500.0, 400.0), p(3500.0, 400.0)], None, None, None).unwrap();
    add_pipe_run(&mut prj, r, Face::Roof, &[p(500.0, 400.0), p(3500.0, 400.0)], None, None, None).unwrap();
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty(), "above the door");
    add_pipe_run(&mut prj, r, Face::Wall(0), &[p(500.0, 1000.0), p(3500.0, 1000.0)], None, None, None).unwrap();
    let findings = super::super::validate(&prj, &prj.layout.enclosures[r]);
    assert!(findings.iter().any(|f| f.kind == CheckKind::Collision && f.items == vec![door] && f.message.contains("PR3") && f.message.contains("Door")), "{findings:?}");
}

#[test]
fn wall_penetrations_lead_into_the_room_behind_the_wall() {
    let (mut prj, r, _) = project();
    let pen = add_opening(&mut prj, r, Opening::Penetration, Face::Wall(1), Some(500.0), None, Some(1000.0)).unwrap();
    let room = &prj.layout.enclosures[r];
    let it = room.item(pen).unwrap();
    assert_eq!((it.w, it.h, opening_sill(room, it)), (PENETRATION_SIZE.0, PENETRATION_SIZE.1, 1000.0));
    assert_eq!(super::super::item_name(room, pen), "Wall penetration");
    let id = add_inner_wall(&mut prj, r, [2000.0, 0.0], [2000.0, 2000.0], 100.0).unwrap();
    assert!(add_opening(&mut prj, r, Opening::Penetration, Face::Inner(id, 0), None, None, None).is_err(), "outer walls only");
    // Nothing behind the wall.
    let poses = site_poses(&prj);
    assert_eq!(penetration_link(&prj, &poses, r, prj.layout.enclosures[r].item(pen).unwrap()), None);
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).iter().any(|f| f.items == vec![pen] && f.kind == CheckKind::Outside));
    // A room behind it, 1.2 m away: too far; 300 mm away: reached.
    let other = new_room(&mut prj, "+ROOM2", 3000.0, 3000.0, 2800.0);
    prj.layout.enclosures.push(other);
    let r2 = prj.layout.enclosures.len() - 1;
    let wall = walls(&prj.layout.enclosures[r])[1].a[0];
    for (gap, reached) in [(1200.0, false), (300.0, true)] {
        super::super::set_site(&mut prj, r2, Some(SitePlace { x: wall + gap, z: 0.0, angle: 0.0 }));
        let poses = site_poses(&prj);
        let link = penetration_link(&prj, &poses, r, prj.layout.enclosures[r].item(pen).unwrap());
        assert_eq!(link.map(|l| l.to), reached.then_some(r2), "{gap}");
    }
    assert_eq!(wall_links(&prj).len(), 1);
    assert!(super::super::validate(&prj, &prj.layout.enclosures[r]).is_empty());
}
