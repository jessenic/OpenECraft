use super::*;
use openecraft_core::panel::buildings as bld;
use openecraft_core::parts::vec3;

/// A building with two levels and a room outside it.
fn building_and_room() -> (Project, [u64; 2], usize) {
    let mut prj = Project::default();
    let bi = bld::add_building(&mut prj, "+B1").unwrap();
    bld::add_level(&mut prj, bi, None, None, None).unwrap();
    let levels = [0, 1].map(|k| prj.layout.buildings[bi].levels[k].id);
    let free = bld::add_room(&mut prj, "+ROOM1", None).unwrap();
    (prj, levels, free)
}

#[test]
fn new_rooms_go_onto_the_level_selected_in_the_navigator() {
    let (mut prj, [l0, l1], free) = building_and_room();
    let mut ui = PanelUi::default();
    ui.new_room(&mut prj);
    assert_eq!(bld::building_of(&prj.layout, ui.enclosure), None, "no level selected");
    ui.nav_action(&mut prj, NavAction::Level(l1));
    ui.new_room(&mut prj);
    assert_eq!(bld::rooms_on(&prj.layout, l1), vec![ui.enclosure]);
    assert_eq!(ui.nav_level, Some(l1), "the level stays selected for more rooms");
    ui.nav_action(&mut prj, NavAction::Level(l1));
    assert_eq!(ui.nav_level, None, "a second click unselects it");
    ui.nav_action(&mut prj, NavAction::NewRoomOn(l0));
    assert_eq!(bld::rooms_on(&prj.layout, l0), vec![ui.enclosure], "context menu of a level");
    assert!(bld::rooms_on(&prj.layout, l0).iter().all(|&r| r != free));
    // A removed level is forgotten.
    ui.nav_level = Some(999_999);
    ui.new_room(&mut prj);
    assert_eq!((ui.nav_level, bld::building_of(&prj.layout, ui.enclosure)), (None, None));
}

#[test]
fn rooms_dropped_onto_a_level_stand_on_it() {
    let (mut prj, [l0, l1], free) = building_and_room();
    let mut ui = PanelUi::default();
    ui.nav_action(&mut prj, NavAction::Put(free, l0));
    assert_eq!(bld::rooms_on(&prj.layout, l0), vec![free]);
    assert!(!ui.status.is_empty());
    ui.nav_action(&mut prj, NavAction::Put(free, l1));
    assert_eq!((bld::rooms_on(&prj.layout, l0), bld::rooms_on(&prj.layout, l1)), (vec![], vec![free]), "moved to another level");
    ui.nav_action(&mut prj, NavAction::TakeOut(free));
    assert_eq!(bld::building_of(&prj.layout, free), None);
    prj.layout.enclosures.push(panel::Enclosure { name: "+CAB1".into(), ..Default::default() });
    let cabinet = prj.layout.enclosures.len() - 1;
    ui.nav_action(&mut prj, NavAction::Put(cabinet, l0));
    assert!(bld::rooms_on(&prj.layout, l0).is_empty(), "only rooms stand on levels: {}", ui.status);
}

#[test]
fn a_room_clicked_in_the_navigator_keeps_3d_or_shows_its_floor_plan() {
    let (mut prj, [l0, _], free) = building_and_room();
    let on_level = bld::add_room(&mut prj, "+ROOM2", Some(l0)).unwrap();
    let mut ui = PanelUi { view: View::ThreeD, ..Default::default() };
    ui.nav_action(&mut prj, NavAction::Open(on_level));
    assert_eq!((ui.enclosure, ui.view), (on_level, View::ThreeD));
    for view in [View::Front, View::Plan] {
        ui.view = view;
        ui.nav_action(&mut prj, NavAction::Open(free));
        assert_eq!((ui.enclosure, ui.view, ui.in_room), (free, View::Plan, true), "{view:?}");
    }
    // From the site too.
    ui.nav_action(&mut prj, NavAction::Site(None));
    ui.nav_action(&mut prj, NavAction::Open(on_level));
    assert_eq!((ui.site, ui.enclosure, ui.view), (false, on_level, View::Plan));
}

#[test]
fn the_site_shows_the_site_plan_or_the_whole_plant_in_3d() {
    let (mut prj, [l0, _], free) = building_and_room();
    let on_level = bld::add_room(&mut prj, "+ROOM2", Some(l0)).unwrap();
    let mut ui = PanelUi { view: View::Front, ..Default::default() };
    ui.nav_action(&mut prj, NavAction::Site(None));
    assert_eq!((ui.site, ui.view), (true, View::Plan), "a 2D view becomes the site plan");
    ui.view = View::ThreeD;
    ui.enclosure = free;
    let shown: Vec<usize> = ui.shown_3d(&prj, free).into_iter().map(|s| s.0).collect();
    assert_eq!(shown, (0..prj.layout.enclosures.len()).collect::<Vec<_>>(), "every room of the plant");
    assert_eq!((ui.buildings_3d(&prj, free), ui.view_frame(&prj, free)), (vec![0], None));
    // A building clicked is highlighted in it; the 3D view stays.
    ui.nav_action(&mut prj, NavAction::Site(Some(0)));
    assert_eq!((ui.site_plan.building, ui.view), (Some(0), View::ThreeD));
    // A wall view or a drawing tool leaves the site.
    ui.front_face(Face::Wall(0), false);
    assert!(!ui.site);
    ui.nav_action(&mut prj, NavAction::Site(None));
    ui.choose_tool(Tool::Walls);
    assert!(!ui.site);
    assert!(on_level != free);
}

#[test]
fn a_level_clicked_in_the_3d_view_shows_its_rooms_side_by_side() {
    let (mut prj, [l0, l1], free) = building_and_room();
    let a = bld::add_room(&mut prj, "+ROOM2", Some(l0)).unwrap();
    let b = bld::add_room(&mut prj, "+ROOM3", Some(l0)).unwrap();
    let up = bld::add_room(&mut prj, "+ROOM4", Some(l1)).unwrap();
    prj.layout.enclosures.push(panel::Enclosure { name: "+CAB1".into(), width: 600.0, height: 2000.0, depth: 400.0, ..Default::default() });
    let cabinet = prj.layout.enclosures.len() - 1;
    panel::set_room(&mut prj, cabinet, Some(b), None).unwrap();
    let mut ui = PanelUi { view: View::ThreeD, enclosure: free, ..Default::default() };
    ui.nav_action(&mut prj, NavAction::Level(l0));
    let shown = ui.shown_3d(&prj, free);
    assert_eq!(shown.iter().map(|s| s.0).collect::<Vec<_>>(), vec![a, b, cabinet], "the rooms of the level with the cabinets in them");
    // At their places in the plant: next to each other.
    let poses = panel::site_poses(&prj);
    assert!(shown.iter().all(|&(k, pose)| pose.at == poses[k].at));
    assert!(poses[b].at[0] >= poses[a].at[0] + prj.layout.enclosures[a].width - 0.5);
    assert_eq!((ui.level_3d(), ui.buildings_3d(&prj, free), ui.view_frame(&prj, free), ui.nav_level), (Some(l0), vec![0], None, Some(l0)));
    ui.nav_action(&mut prj, NavAction::Level(l1));
    assert_eq!(ui.shown_3d(&prj, free).into_iter().map(|s| s.0).collect::<Vec<_>>(), vec![up]);
    // Clicked again: all of the plant, the building highlighted.
    ui.nav_action(&mut prj, NavAction::Level(l1));
    assert_eq!((ui.level_3d(), ui.site, ui.site_plan.building, ui.nav_level), (None, true, Some(0), None));
    assert_eq!(ui.shown_3d(&prj, free).len(), prj.layout.enclosures.len());
    // In a 2D view a level is only chosen for new rooms.
    let mut ui = PanelUi { view: View::Plan, ..Default::default() };
    ui.nav_action(&mut prj, NavAction::Level(l0));
    assert_eq!((ui.site, ui.level_3d(), ui.nav_level), (false, None, Some(l0)));
}

#[test]
fn buildings_get_new_levels_and_the_site_new_buildings_from_their_menus() {
    let (mut prj, _, _) = building_and_room();
    let mut ui = PanelUi::default();
    ui.nav_action(&mut prj, NavAction::NewLevel(0, false));
    let b = &prj.layout.buildings[0];
    let top = b.levels.last().unwrap();
    assert_eq!((b.levels.len(), top.elevation, ui.nav_level), (3, 6000.0, Some(top.id)), "on top, selected for new rooms");
    ui.nav_action(&mut prj, NavAction::NewLevel(0, true));
    let b = &prj.layout.buildings[0];
    assert_eq!((b.levels.len(), b.levels[0].elevation), (4, -bld::LEVEL_HEIGHT), "a basement");
    ui.nav_action(&mut prj, NavAction::NewBuilding);
    assert_eq!(prj.layout.buildings.iter().map(|b| b.name.as_str()).collect::<Vec<_>>(), vec!["+B1", "+B2"]);
    assert_eq!((ui.site, ui.site_plan.building), (true, Some(1)));
}

#[test]
fn fill_colours_follow_the_limit() {
    let green = fill_color(10.0, 70.0);
    assert_eq!(fill_color(56.0, 70.0), green, "up to 80 % of the limit");
    let orange = fill_color(60.0, 70.0);
    assert_ne!(orange, green);
    assert_eq!(fill_color(70.0, 70.0), orange, "the limit itself is still allowed");
    let red = fill_color(70.5, 70.0);
    assert!(red != orange && red != green);
}

#[test]
fn profile_end_handles_resize_without_moving_the_opposite_end() {
    let original = PanelItem { kind: ItemKind::Rail, x: 100.0, y: 50.0, w: 200.0, h: 35.0, ..Default::default() };
    let mut item = original.clone();
    resize_profile_end(&mut item, &original, ProfileEnd::Start, 70.0);
    assert_eq!((item.x, item.w, item.y, item.h), (70.0, 230.0, 50.0, 35.0));

    resize_profile_end(&mut item, &original, ProfileEnd::End, 340.0);
    assert_eq!((item.x, item.w), (100.0, 240.0));
    resize_profile_end(&mut item, &original, ProfileEnd::End, 6000.0);
    assert_eq!(item.w, 5000.0, "the length matches the property editor's maximum");

    let vertical = PanelItem { kind: ItemKind::Duct, x: 20.0, y: 100.0, w: 40.0, h: 200.0, ..Default::default() };
    item = vertical.clone();
    resize_profile_end(&mut item, &vertical, ProfileEnd::End, 245.0);
    assert_eq!((item.x, item.y, item.w, item.h), (20.0, 100.0, 40.0, 145.0));

    resize_profile_end(&mut item, &vertical, ProfileEnd::End, 104.0);
    assert_eq!(item.h, 10.0, "the length cannot be shorter than the property editor's minimum");
}

#[test]
fn selected_profile_can_be_resized_with_its_mounted_group_members_selected() {
    let rail = PanelItem { id: 1, kind: ItemKind::Rail, ..Default::default() };
    let device = PanelItem { id: 2, kind: ItemKind::Device, ..Default::default() };
    let mut enclosure = Enclosure { items: vec![rail, device], ..Default::default() };
    let selection = [1, 2].into_iter().collect();
    assert_eq!(selected_profile(&enclosure, &selection, Face::Plate).map(|i| i.id), Some(1));

    enclosure.items.push(PanelItem { id: 3, kind: ItemKind::Rail, ..Default::default() });
    let selection = [1, 2, 3].into_iter().collect();
    assert!(selected_profile(&enclosure, &selection, Face::Plate).is_none());
}

#[test]
fn door_sides_have_their_own_tools() {
    let mut ui = PanelUi { face: Face::Door, ..Default::default() };
    assert!(!ui.tool_allowed(&Tool::Duct) && ui.tool_allowed(&Tool::Cutout));
    ui.door_inside = true;
    assert!(ui.tool_allowed(&Tool::Duct) && !ui.tool_allowed(&Tool::Cutout));
    assert!(!ui.tool_allowed(&Tool::Rail) && !ui.tool_allowed(&Tool::Busbar));
    assert_eq!(ui.profile_here_for(&Tool::Duct), Some((ItemKind::Duct, panel::DOOR_DUCT_PART.into())));
    ui.face = Face::Plate;
    assert!(ui.tool_allowed(&Tool::Rail) && ui.tool_allowed(&Tool::Duct));
    assert_eq!(ui.profile_here_for(&Tool::Duct), Some((ItemKind::Duct, panel::DUCT_PART.into())));
}

#[test]
fn chosen_profile_parts_are_kept_per_kind_and_door() {
    let mut ui = PanelUi::default();
    ui.choose_profile_part(ItemKind::Duct, "GEN.DUCT-60X80");
    ui.choose_profile_part(ItemKind::Rail, "GEN.RAIL-TS35X15-1M");
    assert_eq!(ui.profile_here_for(&Tool::Duct), Some((ItemKind::Duct, "GEN.DUCT-60X80".into())));
    assert_eq!(ui.profile_here_for(&Tool::Rail), Some((ItemKind::Rail, "GEN.RAIL-TS35X15-1M".into())));
    assert_eq!(ui.profile_here_for(&Tool::Busbar), Some((ItemKind::Busbar, panel::BUSBAR_PART.into())));
    ui.face = Face::Door;
    assert_eq!(ui.profile_here_for(&Tool::Duct), Some((ItemKind::Duct, panel::DOOR_DUCT_PART.into())), "the door keeps its own choice");
}

#[test]
fn ducts_behind_the_door_are_hidden_from_the_front() {
    let duct = PanelItem { kind: ItemKind::Duct, face: Face::Door, ..Default::default() };
    let device = PanelItem { kind: ItemKind::Device, face: Face::Door, ..Default::default() };
    let plate_duct = PanelItem { kind: ItemKind::Duct, ..Default::default() };
    assert!(hidden_behind_door(false, &duct));
    assert!(!hidden_behind_door(true, &duct));
    assert!(!hidden_behind_door(false, &device) && !hidden_behind_door(false, &plate_duct));
}

#[test]
fn the_inside_of_the_door_is_mirrored() {
    let e = Enclosure { width: 600.0, ..Default::default() };
    let mut ui = PanelUi::default();
    assert_eq!(ui.mirror(&e), None);
    ui.face = Face::Door;
    assert_eq!(ui.mirror(&e), None);
    ui.door_inside = true;
    assert_eq!(ui.mirror(&e), Some(600.0));
}

#[test]
fn the_3d_view_shows_all_enclosures_at_their_places() {
    let mut prj = Project::default();
    for name in ["+A", "+B"] {
        prj.layout.enclosures.push(Enclosure { name: name.into(), ..Default::default() });
    }
    panel::set_site(&mut prj, 1, Some(SitePlace { x: 3000.0, z: 1000.0, angle: 90.0 }));
    let mut ui = PanelUi::default();
    assert_eq!(ui.shown_3d(&prj, 0).iter().map(|s| s.0).collect::<Vec<_>>(), vec![0]);
    ui.show_site = true;
    let shown = ui.shown_3d(&prj, 0);
    assert_eq!(shown.len(), 2);
    assert_eq!(shown[1].1.at, [3000.0, 0.0, 1000.0]);
    let cam = view3d::default_cam(&prj, &shown);
    assert!(cam.target[0] > 600.0 && cam.target[2] > 0.0, "the camera looks at both");
}

#[test]
fn focusing_frames_the_whole_view_unless_zoomed_in() {
    let mut prj = Project::default();
    let item = PanelItem { id: 5, x: 100.0, y: 50.0, w: 40.0, h: 80.0, ..Default::default() };
    prj.layout.enclosures.push(Enclosure { name: "+A".into(), width: 800.0, height: 2000.0, items: vec![item], ..Default::default() });
    let mut ui = PanelUi::default();
    let full = view3d::default_cam(&prj, &ui.shown_3d(&prj, 0));
    ui.focus_item(&prj, 0, 5);
    assert_eq!(ui.cam.as_ref().map(|c| (c.target, c.dist)), Some((full.target, full.dist)), "Fit shows the whole enclosure");
    ui.cam.as_mut().unwrap().dist = full.dist / 3.0;
    ui.focus_item(&prj, 0, 5);
    let near = ui.cam.as_ref().unwrap();
    assert!(near.target[1] > 1800.0 && near.dist == full.dist / 3.0, "zoomed in, the camera turns to the item near the top");
    ui.cam.as_mut().unwrap().dist = full.dist * 2.0;
    ui.focus_item(&prj, 0, 5);
    assert_eq!(ui.cam.as_ref().unwrap().target, full.target, "zoomed out, everything stays in view");
}

#[test]
fn rooms_show_their_walls_and_cabinets() {
    let mut prj = Project::default();
    prj.layout.enclosures.push(Enclosure { name: "+A".into(), id: 7, ..Default::default() });
    let mut ui = PanelUi::default();
    ui.new_room(&mut prj);
    ui.new_room(&mut prj);
    assert_eq!(prj.layout.enclosures[2].name, "+ROOM2");
    let room = 1;
    ui.enclosure = room;
    assert!(ui.enclosure_index(&prj).is_some() && ui.in_room);
    assert_eq!((ui.face, ui.view), (Face::Wall(0), View::Plan));
    assert!(ui.tool_allowed(&Tool::Rail) && ui.tool_allowed(&Tool::Duct) && ui.tool_allowed(&Tool::Walls));
    assert!(!ui.tool_allowed(&Tool::Busbar) && !ui.tool_allowed(&Tool::Cutout));
    panel::set_room(&mut prj, 0, Some(room), Some(SitePlace { x: 500.0, z: 200.0, angle: 0.0 })).unwrap();
    let shown = ui.shown_3d(&prj, room);
    assert_eq!(shown.iter().map(|s| s.0).collect::<Vec<_>>(), vec![room, 0]);
    assert_eq!(shown[1].1.at, [500.0, 0.0, 200.0], "in the room's coordinates");
    // Back to a cabinet: its faces again.
    ui.enclosure = 0;
    ui.enclosure_index(&prj);
    assert!(!ui.in_room && !ui.tool_allowed(&Tool::Walls));
    assert_eq!(ui.view, View::Front, "no floor plan of a cabinet");
}

#[test]
fn walls_are_drawn_at_right_angles_and_closed() {
    // Corners snap to the grid and follow the last one at a right angle.
    assert_eq!(plan::ortho_corner(None, p(1020.0, 1980.0)), p(1000.0, 2000.0));
    assert_eq!(plan::ortho_corner(Some(p(0.0, 0.0)), p(3990.0, 220.0)), p(4000.0, 0.0));
    assert_eq!(plan::ortho_corner(Some(p(4000.0, 0.0)), p(4100.0, 2980.0)), p(4000.0, 3000.0));
    // An L drawn without its last corner is closed at a right angle.
    let drawn = [p(0.0, 0.0), p(6000.0, 0.0), p(6000.0, 6000.0), p(3000.0, 6000.0), p(3000.0, 3000.0)];
    let closed = plan::closed_outline(&drawn).unwrap();
    assert_eq!(closed.len(), 6);
    assert!(closed.contains(&[0.0, 3000.0]));
    assert!(plan::closed_outline(&drawn[..2]).is_err(), "too few walls");
    // Finishing the drawing gives the room its outline.
    let mut prj = Project::default();
    let mut ui = PanelUi::default();
    ui.new_room(&mut prj);
    ui.wall_points = drawn.to_vec();
    ui.finish_walls(&mut prj, 0);
    assert_eq!(panel::walls(&prj.layout.enclosures[0]).len(), 6);
    assert!(ui.wall_points.is_empty() && ui.tool == Tool::Select);
}

#[test]
fn doors_and_windows_are_placed_in_rooms_only() {
    let mut prj = Project::default();
    prj.layout.enclosures.push(Enclosure { name: "+A".into(), ..Default::default() });
    let mut ui = PanelUi::default();
    ui.enclosure_index(&prj);
    assert!(!ui.tool_allowed(&Tool::Opening(Opening::Door)), "not in a cabinet");
    ui.new_room(&mut prj);
    ui.view = View::ThreeD;
    ui.choose_tool(Tool::Opening(Opening::Window));
    assert_eq!((ui.tool.clone(), ui.view), (Tool::Opening(Opening::Window), View::Plan), "placed in the floor plan");
    ui.front_face(Face::Wall(1), false);
    ui.choose_tool(Tool::Opening(Opening::Door));
    assert_eq!(ui.view, View::Front, "or in the view of a wall");
}

#[test]
fn inner_walls_are_drawn_as_a_chain() {
    let mut prj = Project::default();
    let mut ui = PanelUi::default();
    ui.new_room(&mut prj);
    ui.choose_tool(Tool::InnerWalls);
    assert_eq!((ui.tool.clone(), ui.view), (Tool::InnerWalls, View::Plan));
    let id = panel::add_inner_wall(&mut prj, 0, [2000.0, 0.0], [2000.0, 1500.0], 100.0).unwrap();
    // Its sides are in the walls menu's faces and take doors.
    let faces = prj.layout.enclosures[0].faces();
    assert!(faces.contains(&Face::Inner(id, 0)) && faces.contains(&Face::Inner(id, 1)));
    ui.front_face(Face::Inner(id, 1), false);
    ui.choose_tool(Tool::Opening(Opening::Door));
    assert_eq!(ui.view, View::Front, "placed in the view of the side");
}

#[test]
fn ducts_in_front_of_devices_are_painted_over_them() {
    // Seen from the left, the vertical duct at the left edge of the demo's mounting
    // plate is in front of the devices; it was always painted behind them.
    let prj = openecraft_core::sample::demo_project();
    let e = &prj.layout.enclosures[0];
    let shown: Vec<&PanelItem> = e.items.iter().filter(|i| i.face == Face::Plate && matches!(i.kind, ItemKind::Duct | ItemKind::Device)).collect();
    let boxes: Vec<_> = shown.iter().map(|i| panel::item_box(e, i)).collect();
    let duct = (0..shown.len()).filter(|&i| shown[i].kind == ItemKind::Duct && shown[i].h > shown[i].w).min_by(|&a, &b| shown[a].x.total_cmp(&shown[b].x)).unwrap();
    let eye = [-3000.0, e.height / 2.0, e.depth + 500.0];
    let order = openecraft_core::parts::vec3::paint_order(&boxes, eye);
    let rank = |i: usize| order.iter().position(|&k| k == i).unwrap();
    let devices: Vec<usize> = (0..shown.len()).filter(|&i| shown[i].kind == ItemKind::Device).collect();
    assert!(!devices.is_empty());
    for d in devices {
        assert!(rank(d) < rank(duct), "{} before the duct", shown[d].tag);
    }
}

#[test]
fn wires_routed_through_ducts_lie_inside_the_duct_boxes() {
    // The 3D view draws the pieces of a wire inside a duct box over that duct; the
    // stretches between the first and the last point of a route run in the ducts.
    let prj = openecraft_core::sample::demo_project();
    let an = Analysis::build(&prj);
    let e = &prj.layout.enclosures[0];
    let ducts: Vec<_> = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Plate).map(|i| panel::item_box(e, i)).collect();
    let z = panel::DUCT_PROFILE.1 * 0.5;
    let mut checked = 0;
    for w in an.wiring.iter().filter(|w| w.enclosure == 0 && w.route.via_ducts) {
        let pts: Vec<[f32; 3]> = w.route.points.iter().map(|q| [PLATE_MARGIN + q.x, PLATE_MARGIN + q.y, PLATE_Z + z]).collect();
        for q in pts.iter().skip(1).take(pts.len().saturating_sub(2)) {
            assert!(ducts.iter().any(|(lo, hi)| (0..3).all(|k| q[k] >= lo[k] - 0.5 && q[k] <= hi[k] + 0.5)), "{} at {q:?}", w.number);
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn cable_cores_are_drawn_with_their_cable() {
    // The link cable between two batteries in the rack is drawn by the 3D routing; its
    // cores are not drawn again as wires over the rear of the rack.
    let prj = openecraft_core::sample::example("building_services").unwrap();
    let an = Analysis::checked(&prj);
    let rack = prj.layout.enclosures.iter().position(|e| e.name == "+RACK").unwrap();
    assert!(an.wiring.iter().any(|w| w.cable == "=ESS-W97" && w.in_enclosure(rack)));
    assert!(an.routing.item("=ESS-W97").is_some_and(|c| c.routed()));
    let mut ui = PanelUi::default();
    let shown = ui.shown_3d(&prj, rack);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, rack, &shown);
    let cores = scene.picks.iter().filter(|(_, _, t)| matches!(t, routing_ui::RouteRef::Wire(wi) if an.wiring[*wi].cable == "=ESS-W97")).count();
    assert_eq!(cores, 0);
    assert!(scene.picks.iter().any(|(_, _, t)| matches!(t, routing_ui::RouteRef::Item(key) if key == "=ESS-W97")));
    // The battery cable parts in front of the battery into its cores to + and -: each a
    // smooth curve out of the end of the cable into its connection point, coming in
    // against the direction it leaves it in; they are seen without the cable being
    // selected.
    let w71 = an.routing.item("=ESS-W71").unwrap();
    let end = w71.ends.iter().find(|e| e.tag == "=ESS+RACK-G1").unwrap();
    assert_eq!((end.cores.len(), end.exits.len()), (2, 2));
    let lines = ui.routing_lines_3d(&prj, &an, &[rack], &|_, q| q, None);
    let lines: Vec<_> = lines.iter().filter(|l| matches!(&l.target, routing_ui::RouteRef::Item(k) if k == "=ESS-W71")).collect();
    let (pos, (_, pose)) = (panel::positions(&prj.layout)[rack], shown[0]);
    let view = |q: [f32; 3]| pose.apply([q[0] - pos[0], prj.layout.enclosures[rack].height - (q[1] - pos[1]), q[2] - pos[2]]);
    for (core, exit) in end.cores.iter().zip(&end.exits) {
        let line = lines.iter().find(|l| l.points.last() == Some(core) && l.diameter < w71.diameter).unwrap_or_else(|| panic!("{core:?}"));
        let dirs: Vec<[f32; 3]> = line.points.windows(2).map(|s| vec3::norm(vec3::sub(s[1], s[0]))).collect();
        assert!(dirs.windows(2).all(|d| vec3::dot(d[0], d[1]) > 0.7), "{:?}", line.points);
        assert!(vec3::dot(dirs[dirs.len() - 1], vec3::norm(*exit)) < -0.9, "{:?}", line.points);
        let start = line.points[0];
        assert!(lines.iter().any(|l| l.diameter == w71.diameter && l.points.iter().any(|q| vec3::dist(*q, start) <= w71.diameter / 2.0 + 0.01)), "{start:?}");
        let at = view(*core);
        assert!(scene.wires.iter().any(|w| [w.line.a, w.line.b].iter().any(|q| (0..3).all(|k| (q[k] - at[k]).abs() < 0.5)) && w.line.a != w.line.b), "{core:?} not drawn");
    }
}

#[test]
fn cables_run_on_through_the_cable_glands_into_the_room() {
    // The motor cable of pump 1 leaves its cabinet through the gland in the roof: the
    // stretch through the cabinet and the one through the room are one line through it.
    let prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = Analysis::build(&prj);
    let room = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    let w1 = an.routing.item("=P1-W1").unwrap();
    assert_eq!(w1.legs.len(), 2);
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, room);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, room, &shown);
    let k = w1.legs[0].enclosure;
    let (pos, pose) = (panel::positions(&prj.layout)[k], shown.iter().find(|s| s.0 == k).unwrap().1);
    let gland = *w1.legs[0].points.last().unwrap();
    let g = pose.apply([gland[0] - pos[0], prj.layout.enclosures[k].height - (gland[1] - pos[1]), gland[2] - pos[2]]);
    // A piece of the cable crosses the roof just above the gland.
    let above = g[1] + 20.0;
    let through = scene.picks.iter().filter(|(_, _, t)| matches!(t, routing_ui::RouteRef::Item(key) if key == "=P1-W1")).any(|(a, b, _)| {
        let (lo, hi) = if a[1] < b[1] { (a, b) } else { (b, a) };
        if !(lo[1] <= above && hi[1] >= above) || hi[1] - lo[1] < 1e-3 {
            return false;
        }
        let t = (above - lo[1]) / (hi[1] - lo[1]);
        let q = [0, 1, 2].map(|i| lo[i] + (hi[i] - lo[i]) * t);
        (q[0] - g[0]).hypot(q[2] - g[2]) < 30.0
    });
    assert!(through, "no piece of the cable through the gland at {g:?}");
}

#[test]
fn wires_run_through_the_cable_passages_in_one_bend() {
    // Wires from +CAB1 to +CAB2 of the pump room pass the cable passage between them
    // bent round, not with a corner where their stretches in the two cabinets meet.
    let prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = Analysis::build(&prj);
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, 0);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
    let pos = panel::positions(&prj.layout);
    let mut checked = 0;
    for (wi, w) in an.wiring.iter().enumerate().filter(|(_, w)| w.legs.len() == 1 && w.cable.is_empty()) {
        let (k, pose) = shown.iter().copied().find(|s| s.0 == w.enclosure).unwrap();
        let q = w.legs[0].passage;
        let p = pose.apply([q[0] - pos[k][0], prj.layout.enclosures[k].height - (q[1] - pos[k][1]), q[2] - pos[k][2]]);
        let segs: Vec<_> = scene.picks.iter().filter(|(_, _, t)| matches!(t, routing_ui::RouteRef::Wire(i) if *i == wi)).collect();
        let near = |a: [f32; 3]| vec3::dist(a, p) < 40.0;
        for s in segs.windows(2).filter(|s| vec3::dist(s[0].1, s[1].0) < 0.01 && near(s[0].1)) {
            let (u, v) = (vec3::norm(vec3::sub(s[0].1, s[0].0)), vec3::norm(vec3::sub(s[1].1, s[1].0)));
            assert!(vec3::dot(u, v) > 0.6, "{} turns sharply at {:?}: {:?}", w.number, s[0].1, segs.iter().filter(|t| near(t.0)).map(|t| (t.0, t.1)).collect::<Vec<_>>());
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn routed_pipes_are_round_tubes_bent_round_their_corners() {
    let prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = Analysis::build(&prj);
    let room = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    let pipe = an.routing.items.iter().find(|i| i.kind == routing3d::RouteKind::Tube && i.routed() && i.points.len() > 3).expect("a pipe with corners");
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, room);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, room, &shown);
    assert!(!scene.tubes.is_empty());
    // Its corners are arcs: more segments than the route has corners.
    let segments: Vec<_> = scene.picks.iter().filter(|(_, _, t)| matches!(t, routing_ui::RouteRef::Item(k) if *k == pipe.key)).collect();
    assert!(segments.len() > pipe.points.len(), "{} segments, {} points", segments.len(), pipe.points.len());
    assert!(scene.wires.iter().any(|w| w.line.tube && w.line.diameter == pipe.diameter));
}

#[test]
fn cables_are_picked_on_the_plate_like_wires() {
    let prj = openecraft_core::sample::demo_project();
    let an = Analysis::checked(&prj);
    let pos = panel::positions(&prj.layout);
    // The middle of a segment of a cable's route over the plate of a cabinet.
    let (ei, index, q) = an
        .routing
        .items
        .iter()
        .enumerate()
        .filter(|(_, it)| it.kind == routing3d::RouteKind::Cable && it.routed())
        .find_map(|(i, it)| {
            let ei = *it.enclosures.first()?;
            let pts = it.stretches_in_lineup(&prj.layout, ei).into_iter().find(|s| s.len() >= 2)?;
            let (a, b) = (pts[0], pts[1]);
            Some((ei, i, p((a[0] + b[0]) / 2.0 - pos[ei][0] - PLATE_MARGIN, (a[1] + b[1]) / 2.0 - pos[ei][1] - PLATE_MARGIN)))
        })
        .expect("a cable over a plate");
    let mut ui = PanelUi::default();
    assert!(ui.cables_at(&prj, &an, ei, q, 2.0).contains(&index));
    assert!(ui.cables_at(&prj, &an, ei, p(-5000.0, -5000.0), 2.0).is_empty());
    // Picked, it is shown as in the 3D view: no wire or item selected with it.
    ui.selection.insert(7);
    ui.selected_wire = Some(("-K1:A1".into(), "-K2:A1".into()));
    ui.select_route(&an, &an.routing.items[index].key);
    assert_eq!(ui.selected_route.as_deref(), Some(an.routing.items[index].key.as_str()));
    assert!(ui.selected_wire.is_none() && ui.selection.is_empty() && ui.status.contains(&an.routing.items[index].label));
}

#[test]
fn wires_entering_the_ducts_take_out_the_fingers_they_pass() {
    // Shown, the wires pass through the walls of the ducts: the fingers there are left out.
    let prj = openecraft_core::sample::demo_project();
    let an = Analysis::build(&prj);
    let faces = |show_wires: bool| {
        let mut ui = PanelUi { show_wires, ..Default::default() };
        let shown = ui.shown_3d(&prj, 0);
        ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown).faces.len()
    };
    let (hidden, shown) = (faces(false), faces(true));
    assert!(shown < hidden && (hidden - shown) % 6 == 0, "{hidden} faces, {shown} with the wires");
}

#[test]
fn wires_running_together_lie_side_by_side() {
    // In the ducts of the demo cabinet many wires run the same way: no two straight runs
    // of different wires lie inside each other (but at their ends, where they leave their
    // devices over them).
    let prj = openecraft_core::sample::demo_project();
    let an = Analysis::build(&prj);
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, 0);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
    let axis = |a: [f32; 3], b: [f32; 3]| {
        let along: Vec<usize> = (0..3).filter(|&k| (a[k] - b[k]).abs() > 0.01).collect();
        (along.len() == 1 && (a[along[0]] - b[along[0]]).abs() > 30.0).then(|| along[0])
    };
    // The pieces of each wire without the stretch at either end of each piece.
    type Segment = ([f32; 3], [f32; 3]);
    let mut pieces: Vec<(usize, Vec<Segment>)> = Vec::new();
    for (a, b, t) in &scene.picks {
        let routing_ui::RouteRef::Wire(wi) = t else { continue };
        match pieces.last_mut() {
            Some((w, segs)) if w == wi && segs.last().is_some_and(|s| s.1 == *a) => segs.push((*a, *b)),
            _ => pieces.push((*wi, vec![(*a, *b)])),
        }
    }
    let runs: Vec<(usize, usize, [f32; 3], [f32; 3])> = pieces
        .iter()
        .flat_map(|(wi, segs)| segs.iter().skip(1).take(segs.len().saturating_sub(2)).filter_map(move |(a, b)| axis(*a, *b).map(|k| (*wi, k, *a, *b))))
        .collect();
    let radius = |wi: usize| openecraft_core::harness::outer_diameter(&an.wiring[wi].cross_section) / 2.0;
    let mut together = 0;
    for (i, &(w1, k1, a1, b1)) in runs.iter().enumerate() {
        for &(w2, k2, a2, b2) in &runs[i + 1..] {
            if w1 == w2 || k1 != k2 {
                continue;
            }
            let overlap = a1[k1].max(b1[k1]).min(a2[k1].max(b2[k1])) - a1[k1].min(b1[k1]).max(a2[k1].min(b2[k1]));
            let apart = (0..3).filter(|&k| k != k1).map(|k| (a1[k] - a2[k]).powi(2)).sum::<f32>().sqrt();
            if overlap > 10.0 && apart < radius(w1) + radius(w2) + 2.0 {
                together += 1;
                assert!(apart >= radius(w1) + radius(w2) - 0.01, "{} and {} inside each other: {a1:?} {b1:?} / {a2:?} {b2:?}", an.wiring[w1].number, an.wiring[w2].number);
            }
        }
    }
    assert!(together > 5, "{together} runs side by side");
}

#[test]
fn wires_and_cables_laid_side_by_side_keep_in_front_of_the_plate() {
    // In the cabinets of the demo and of the building (their view: the plate in front of
    // z 0) every wire and cable drawn inside stays in front of the mounting plate with
    // its whole thickness.
    let demo = openecraft_core::sample::demo_project();
    let building = openecraft_core::sample::example("building_services").unwrap();
    for (prj, name) in [(&demo, "+CAB1"), (&building, "+CC1")] {
        let an = Analysis::build(prj);
        let ei = prj.layout.enclosures.iter().position(|e| e.name.ends_with(name)).unwrap();
        let e = &prj.layout.enclosures[ei];
        let mut ui = PanelUi { enclosure: ei, show_wires: true, ..Default::default() };
        let shown = ui.shown_3d(prj, ei);
        let scene = ui.scene_3d(&egui::Context::default(), prj, &an, ei, &shown);
        let inside = |q: &[f32; 3]| q[0] > 0.0 && q[0] < e.width && q[1] > 0.0 && q[1] < e.height && q[2] < e.depth;
        let mut checked = 0;
        for (a, b, t) in &scene.picks {
            let radius = match t {
                routing_ui::RouteRef::Wire(wi) if an.wiring[*wi].door == (None, None) => openecraft_core::harness::outer_diameter(&an.wiring[*wi].cross_section) / 2.0,
                routing_ui::RouteRef::Item(key) => an.routing.item(key).map_or(0.0, |i| i.diameter / 2.0),
                _ => continue,
            };
            for q in [a, b].into_iter().filter(|q| inside(q)) {
                assert!(q[2] >= PLATE_Z + radius - 0.01, "{name}: {q:?} behind the plate (a cable: {})", matches!(t, routing_ui::RouteRef::Item(_)));
                checked += 1;
            }
        }
        assert!(checked > 100, "{name}: {checked}");
    }
}

#[test]
fn wires_pass_in_front_of_the_busbars() {
    // In the main distribution of the building the wires from the circuit breakers to
    // the duct above them cross the busbar: in front of it, not through it.
    let prj = openecraft_core::sample::example("building_services").unwrap();
    let an = Analysis::build(&prj);
    let ei = prj.layout.enclosures.iter().position(|e| e.items.iter().any(|i| i.kind == ItemKind::Busbar)).expect("busbars");
    let e = &prj.layout.enclosures[ei];
    let mut ui = PanelUi { enclosure: ei, show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, ei);
    let pose = shown.iter().find(|s| s.0 == ei).unwrap().1;
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, ei, &shown);
    let busbars: Vec<([f32; 3], [f32; 3])> = e
        .items
        .iter()
        .filter(|i| i.kind == ItemKind::Busbar)
        .map(|i| {
            let (a, b) = panel::item_box(e, i);
            let (lo, hi) = (pose.apply([a[0], e.height - b[1], a[2]]), pose.apply([b[0], e.height - a[1], b[2]]));
            ([0, 1, 2].map(|k| lo[k].min(hi[k]) + 0.5), [0, 1, 2].map(|k| lo[k].max(hi[k]) - 0.5))
        })
        .collect();
    let mut crossing = 0;
    for (a, b, t) in &scene.picks {
        if !matches!(t, routing_ui::RouteRef::Wire(_)) {
            continue;
        }
        for bx in &busbars {
            if openecraft_core::parts::vec3::segment_in_box(*a, *b, *bx).is_some() {
                crossing += 1;
            }
        }
    }
    assert_eq!(crossing, 0, "wire segments through a busbar");
}

#[test]
fn face_summaries_count_openings_apart() {
    let item = |kind: ItemKind, opening: Opening| PanelItem { kind, opening, ..Default::default() };
    let rail = item(ItemKind::Rail, Opening::Cutout);
    let window = item(ItemKind::Cutout, Opening::Window);
    let door = item(ItemKind::Cutout, Opening::Door);
    let device = item(ItemKind::Device, Opening::Cutout);
    assert_eq!(face_summary(&[]), "empty");
    assert_eq!(face_summary(&[&window]), "1 window");
    assert_eq!(face_summary(&[&rail, &device, &device, &door]), "1 rail, 2 devices, 1 door");
}

#[test]
fn inner_walls_end_on_the_walls_of_the_room() {
    let room = panel::new_room(&mut Project::default(), "+ROOM1", 4030.0, 3000.0, 2800.0);
    let walls = panel::walls(&room);
    // The grid alone would leave 50 mm to the rear wall and 30 mm to the right one.
    assert_eq!(plan::inner_corner(&walls, None, p(2000.0, 40.0)), p(2000.0, 0.0));
    assert_eq!(plan::inner_corner(&walls, Some(p(2000.0, 1000.0)), p(4010.0, 1000.0)), p(4030.0, 1000.0));
    // Away from the walls the grid stays.
    assert_eq!(plan::inner_corner(&walls, Some(p(2000.0, 0.0)), p(2000.0, 1490.0)), p(2000.0, 1500.0));
}

#[test]
fn new_enclosures_and_rooms_take_free_names() {
    let mut prj = Project::default();
    let mut ui = PanelUi::default();
    ui.new_room(&mut prj);
    ui.new_enclosure_dialog(&prj, &PartsDb::default());
    assert_eq!(ui.new_enclosure.as_ref().map(|n| n.name.as_str()), Some("+CAB1"));
    ui.new_room(&mut prj);
    assert_eq!(prj.layout.enclosures[1].name, "+ROOM2");
}

#[test]
fn pipe_runs_and_the_floor_belong_to_rooms() {
    let mut prj = Project::default();
    let mut ui = PanelUi::default();
    // Not in a cabinet.
    let cab = panel::new_enclosure(&mut prj, panel::ENCLOSURE_PART, "+CAB1");
    prj.layout.enclosures.push(cab);
    ui.enclosure_index(&prj);
    ui.choose_tool(Tool::PipeRun);
    assert_eq!(ui.tool, Tool::Select);
    // In a room on any of its faces, the floor among them.
    ui.new_room(&mut prj);
    let ri = ui.enclosure_index(&prj).unwrap();
    assert!(prj.layout.enclosures[ri].faces().contains(&Face::Bottom));
    ui.front_face(Face::Bottom, false);
    ui.choose_tool(Tool::PipeRun);
    assert_eq!((ui.tool.clone(), ui.view, ui.face), (Tool::PipeRun, View::Front, Face::Bottom));
    // Corners at right angles, along the longer direction.
    assert_eq!(ortho(Some(p(0.0, 0.0)), p(500.0, 40.0)), p(500.0, 0.0));
    assert_eq!(ortho(Some(p(0.0, 0.0)), p(40.0, -500.0)), p(0.0, -500.0));
    assert_eq!(ortho(None, p(40.0, 50.0)), p(40.0, 50.0));
}

#[test]
fn a_device_from_the_schematic_is_shown_where_it_is_placed() {
    let prj = openecraft_core::sample::example("building_services").unwrap();
    let mut ui = PanelUi::default();
    assert!(!ui.focus_device(&prj, "=NOPE-X1", false), "not placed");
    // On the floor of a room: the floor plan with the device selected.
    let (ei, it) = panel::locate(&prj, "=WAT+TR-GP1").expect("booster pump placed");
    assert!(ui.focus_device(&prj, "=WAT+TR-GP1", false));
    assert_eq!((ui.enclosure, ui.view, ui.selection.iter().copied().collect::<Vec<_>>()), (ei, View::Plan, vec![it.id]));
    // In 3D the camera looks at it from close by.
    assert!(ui.focus_device(&prj, "=WAT+TR-GP1", true));
    assert_eq!(ui.view, View::ThreeD);
    let cam = ui.cam.expect("camera");
    let full = view3d::default_cam(&prj, &ui.shown_3d(&prj, ei));
    assert!(cam.dist < full.dist, "{} {}", cam.dist, full.dist);
    // On a mounting plate: the front view of its cabinet.
    let (ci, _) = panel::locate(&prj, "=CTL+CC1-A1").expect("PLC placed");
    assert!(ui.focus_device(&prj, "=CTL+CC1-A1", false));
    assert_eq!((ui.enclosure, ui.view, ui.face), (ci, View::Front, Face::Plate));
}

/// Shapes of one frame of the 3D view of the room of `prj`, and the smallest alpha of
/// a filled shape.
fn room_frame(ui: &mut PanelUi, prj: &mut Project, an: &openecraft_core::analysis::Analysis) -> (usize, u8) {
    let room = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    (ui.enclosure, ui.view) = (room, View::ThreeD);
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))), ..Default::default() };
    let mut out = ctx.run_ui(input, |ui_| ui.view_3d(ui_, prj, room, an));
    out.textures_delta.clear();
    let mut alpha = 255;
    for s in &out.shapes {
        // Unfilled paths are outlines (the icons of the view bar), not faces.
        if let egui::Shape::Path(p) = &s.shape {
            if p.fill.a() > 0 {
                alpha = alpha.min(p.fill.a());
            }
        }
    }
    (out.shapes.len(), alpha)
}

#[test]
fn the_camera_glides_to_a_standard_view_over_the_frames() {
    let mut prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    let mut ui = PanelUi::default();
    room_frame(&mut ui, &mut prj, &an);
    let start = ui.cam.unwrap();
    let (yaw, pitch) = nav3d::Side::Top.angles();
    let goal = Orbit { yaw, pitch, ..start };
    ui.cam_goal = Some((goal, start));
    let room = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..200 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))), predicted_dt: 1.0 / 60.0, ..Default::default() };
        ctx.run_ui(input, |ui_| ui.view_3d(ui_, &mut prj, room, &an)).textures_delta.clear();
        if ui.cam_goal.is_none() {
            break;
        }
    }
    assert_eq!(ui.cam, Some(goal), "arrived at the view from above");
}

#[test]
fn walls_in_front_and_the_ceiling_can_be_left_out() {
    let mut prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    let mut ui = PanelUi::default();
    let (all, alpha) = room_frame(&mut ui, &mut prj, &an);
    assert!(alpha < 200, "walls in front are see-through");
    ui.hide_ceiling = true;
    let (no_ceiling, _) = room_frame(&mut ui, &mut prj, &an);
    assert!(no_ceiling < all, "{no_ceiling} {all}");
    ui.hide_front_walls = true;
    let (open, _) = room_frame(&mut ui, &mut prj, &an);
    assert!(open < no_ceiling, "{open} {no_ceiling}");
    // Opaque walls in front.
    let mut ui = PanelUi { wall_opacity: 1.0, ..Default::default() };
    assert_eq!(room_frame(&mut ui, &mut prj, &an).1, 255);
}

#[test]
fn wires_take_their_own_colours() {
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let blue = rgb(openecraft_core::potentials::color_rgb("BU").unwrap());
    assert_eq!(wire_look("BU", false, false, false), (blue, None, 1.5));
    let (green, yellow) = (rgb(openecraft_core::potentials::color_rgb("GN").unwrap()), rgb(openecraft_core::potentials::color_rgb("YE").unwrap()));
    assert_eq!(wire_look("gn/ye", false, false, false), (green, Some(yellow), 1.5), "two colours: a stripe");
    assert_eq!(wire_look("", false, false, false).0, WIRE_COLOR, "no colour code");
    assert_eq!(wire_look("BU", true, false, false).0, WIRE_HIGHLIGHT);
    assert_eq!(wire_look("BU", false, true, false).0, WIRE_DONE);
    assert_eq!(wire_look("BU", false, false, true).0, blue.gamma_multiply(0.3), "dimmed while another is selected");
}

#[test]
fn wires_in_3d_have_their_colour_and_size() {
    let mut prj = openecraft_core::sample::example("cabinet_lineup").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    let colors: std::collections::HashSet<Color32> = an.wiring.iter().filter_map(|w| openecraft_core::potentials::color_rgb(&w.color)).map(|c| Color32::from_rgb(c[0], c[1], c[2])).collect();
    assert!(colors.len() > 1, "the example has wires of several colours");
    let e = an.wiring[0].enclosure;
    // Lines of one frame of the 3D view: their colours and the widest.
    let frame = |ui: &mut PanelUi, prj: &mut Project| {
        let ctx = egui::Context::default();
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui_| ui.view_3d(ui_, prj, e, &an));
        out.textures_delta.clear();
        let mut seen = std::collections::HashSet::new();
        let mut widest = 0.0f32;
        for s in &out.shapes {
            if let egui::Shape::LineSegment { stroke, .. } = &s.shape {
                seen.insert(stroke.color);
                widest = widest.max(stroke.width);
            }
        }
        (seen, widest)
    };
    let mut ui = PanelUi { enclosure: e, view: View::ThreeD, ..Default::default() };
    let (seen, far) = frame(&mut ui, &mut prj);
    assert!(colors.iter().filter(|c| seen.contains(c)).count() > 1, "drawn in their colours");
    // Closer, the wires are thicker on the screen.
    let cam = ui.cam.as_mut().unwrap();
    cam.dist *= 0.2;
    let (_, near) = frame(&mut ui, &mut prj);
    assert!(near > far, "{near} {far}");
}

/// Draws one frame of the 3D view of the room of `prj`; the shapes it painted.
fn room_shapes(ui: &mut PanelUi, prj: &mut Project, an: &openecraft_core::analysis::Analysis, yaw: f32) -> Vec<egui::Shape> {
    let room = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    (ui.enclosure, ui.view) = (room, View::ThreeD);
    if let Some(cam) = ui.cam.as_mut() {
        cam.yaw = yaw;
    }
    let ctx = egui::Context::default();
    let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))), ..Default::default() };
    let mut out = ctx.run_ui(input, |ui_| ui.view_3d(ui_, prj, room, an));
    out.textures_delta.clear();
    out.shapes.into_iter().map(|s| s.shape).collect()
}

#[test]
fn the_3d_scene_is_built_again_only_when_what_it_shows_changes() {
    let mut prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    let mut ui = PanelUi::default();
    let id = |ui: &PanelUi| ui.scene.as_ref().map(|s| s.id);
    let first = room_shapes(&mut ui, &mut prj, &an, 0.5);
    let built = id(&ui);
    assert!(built.is_some());
    // Turning the camera only projects the scene again.
    let turned = room_shapes(&mut ui, &mut prj, &an, 1.5);
    assert_eq!(id(&ui), built);
    assert_ne!(first.len(), 0);
    assert_ne!(format!("{first:?}"), format!("{turned:?}"));
    // A selection, the ceiling, a moved item and a new analysis build it again.
    let room = ui.enclosure;
    let item = prj.layout.enclosures[room].items[0].id;
    ui.selection.insert(item);
    room_shapes(&mut ui, &mut prj, &an, 1.5);
    let selected = id(&ui);
    assert_ne!(selected, built);
    ui.hide_ceiling = true;
    room_shapes(&mut ui, &mut prj, &an, 1.5);
    assert_ne!(id(&ui), selected);
    let hidden = id(&ui);
    prj.layout.enclosures[room].items[0].x += 10.0;
    room_shapes(&mut ui, &mut prj, &an, 1.5);
    assert_ne!(id(&ui), hidden);
    let moved = id(&ui);
    room_shapes(&mut ui, &mut prj, &an, 1.5);
    assert_eq!(id(&ui), moved);
    let an = openecraft_core::analysis::Analysis::build(&prj);
    room_shapes(&mut ui, &mut prj, &an, 1.5);
    assert_ne!(id(&ui), moved);
}

#[test]
fn the_graphics_card_draws_the_scene_and_egui_the_rest() {
    let mut prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    let mut painted = PanelUi::default();
    let mut gpu = PanelUi { gpu_3d: true, ..Default::default() };
    let shapes = room_shapes(&mut gpu, &mut prj, &an, 0.5);
    let callbacks = shapes.iter().filter(|s| matches!(s, egui::Shape::Callback(_))).count();
    assert_eq!(callbacks, 1);
    // No faces painted by egui: the background, labels, outlines and the hint only.
    let all = room_shapes(&mut painted, &mut prj, &an, 0.5);
    assert!(shapes.len() * 4 < all.len(), "{} {}", shapes.len(), all.len());
    assert!(!all.iter().any(|s| matches!(s, egui::Shape::Callback(_))));
}

#[test]
fn clicking_in_the_3d_view_picks_from_the_scene_with_the_graphics_card_too() {
    let mut prj = openecraft_core::sample::example("pump_room").unwrap();
    let an = openecraft_core::analysis::Analysis::build(&prj);
    for gpu_3d in [false, true] {
        let mut ui = PanelUi { gpu_3d, ..Default::default() };
        room_shapes(&mut ui, &mut prj, &an, 0.5);
        let room = ui.enclosure;
        let scene = ui.scene.clone().unwrap();
        let cam = ui.cam.unwrap();
        let (eye, right, up, fwd) = cam.basis();
        let rect = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0));
        let camera = gpu3d::Camera { eye, right, up, fwd, f: rect.height() * 1.3 };
        let prims = view3d::project(&scene, &camera, rect, ui.wall_opacity, false);
        // The centre of a device face that nothing picked is drawn over, clicked.
        let centre = |pts: &[egui::Pos2]| (pts.iter().fold(egui::Vec2::ZERO, |a, q| a + q.to_vec2()) / pts.len() as f32).to_pos2();
        let top = |q: egui::Pos2| prims.iter().rev().find_map(|p| match p {
            view3d::Prim::Face { pts, n, item: Some(item), .. } if super::view3d::point_in_convex(q, &pts[..*n as usize]) => Some(item.1),
            _ => None,
        });
        let target = prims.iter().rev().find_map(|p| match p {
            view3d::Prim::Face { pts, n, item: Some((k, id)), .. } if prj.layout.enclosures[*k].item(*id).is_some_and(|i| i.kind == ItemKind::Device) => {
                let at = centre(&pts[..*n as usize]);
                (top(at) == Some(*id) && rect.contains(at)).then_some((at, *id))
            }
            _ => None,
        });
        let Some((at, id)) = target else { panic!("no device face") };
        let ctx = egui::Context::default();
        for (t, pressed) in [(0.0, None), (0.1, Some(true)), (0.15, Some(false))] {
            let mut events = vec![egui::Event::PointerMoved(at)];
            events.extend(pressed.map(|pressed| egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }));
            let input = egui::RawInput { screen_rect: Some(rect), time: Some(t), events, ..Default::default() };
            ctx.run_ui(input, |ui_| ui.view_3d(ui_, &mut prj, room, &an)).textures_delta.clear();
        }
        assert!(ui.selection.contains(&id), "gpu {gpu_3d}");
    }
}

#[test]
fn a_selected_cable_shows_its_properties() {
    // As a selected wire: the panel shows the cable with its route and the devices at
    // its ends, not the properties of the enclosure.
    let mut prj = openecraft_core::sample::demo_project();
    let an = Analysis::checked(&prj);
    let cable = an.routing.items.iter().find(|i| i.kind == routing3d::RouteKind::Cable && i.routed()).expect("a routed cable");
    let device = cable.ends.iter().filter_map(|e| prj.layout.enclosures.get(e.enclosure?)?.item(e.item?)).find(|i| !i.tag.is_empty()).expect("a device at an end").tag.clone();
    // Its cores with the connection points they join.
    let core = an.connections.iter().find(|c| c.cable == cable.key).expect("a core of the cable");
    let points = [core.a, core.b].map(|pin| openecraft_core::netlist::pin_designation(&prj, pin));
    let mut ui = PanelUi { selected_route: Some(cable.key.clone()), ..Default::default() };
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |c| {
        egui::CentralPanel::default().show(c, |u| ui.properties(u, &mut prj, &an, &PartsDb::default()));
    });
    fn texts(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.job.text.clone()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
            _ => {}
        }
    }
    out.textures_delta.clear();
    let mut shown = Vec::new();
    out.shapes.iter().for_each(|c| texts(&c.shape, &mut shown));
    for want in [cable.label.as_str(), tr("Free air"), tr("Connected devices"), device.as_str(), tr("Cores"), points[0].as_str(), points[1].as_str()] {
        assert!(shown.iter().any(|t| t.contains(want)), "{want} in {shown:?}");
    }
    assert!(!shown.iter().any(|t| t == tr("Enclosure")), "{shown:?}");
}

#[test]
fn wires_are_laid_out_on_another_thread_with_their_progress_shown() {
    let prj = openecraft_core::sample::demo_project();
    let an = Analysis::build(&prj);
    let ctx = egui::Context::default();
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    ui.layouts.wait = 0;
    let shown = ui.shown_3d(&prj, 0);
    let first = ui.scene_3d(&ctx, &prj, &an, 0, &shown);
    // With threads the scene does not wait for the layout: its wires are shown as routed
    // until it is done (without threads it is done at once).
    if let Some(key) = first.waiting {
        assert!(ui.layouts.running(key) && ui.layouts.progress().is_some_and(|p| (0.0..=1.0).contains(&p)));
        while ui.layouts.progress().is_some() {
            ui.layouts.collect();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let laid = ui.scene_3d(&ctx, &prj, &an, 0, &shown);
    assert!(laid.waiting.is_none() && ui.layouts.progress().is_none());
    assert!(first.waiting.is_none() || laid.id != first.id);
    assert_eq!(ui.scene_3d(&ctx, &prj, &an, 0, &shown).id, laid.id, "kept");
    // A selection draws it again with the same layout, none worked out again.
    let wire = an.wiring.iter().position(|w| w.in_enclosure(0)).unwrap();
    ui.selected_wire = Some((an.wiring[wire].from.clone(), an.wiring[wire].to.clone()));
    let selected = ui.scene_3d(&ctx, &prj, &an, 0, &shown);
    assert!(selected.id != laid.id && selected.waiting.is_none() && ui.layouts.progress().is_none());
    // During a gesture none is worked out: shown as routed.
    let mut moved = prj.clone();
    let device = moved.layout.enclosures[0].items.iter().position(|i| i.kind == ItemKind::Device).unwrap();
    moved.layout.enclosures[0].items[device].x += 30.0;
    ui.drag = Some(MoveDrag { start: openecraft_core::geom::p(0.0, 0.0), originals: Vec::new(), resize: None });
    let dragged = ui.scene_3d(&ctx, &moved, &an, 0, &shown);
    assert!(dragged.waiting.is_some() && ui.layouts.progress().is_none());
}

#[test]
fn cores_part_from_their_cable_without_lying_in_each_other() {
    // The five cores of the main cable to the AC distribution part in front of its
    // terminals: each from its own place in the cable, none inside another where they
    // start, thinner than half the cable.
    let prj = openecraft_core::sample::example("building_services").unwrap();
    let an = Analysis::build(&prj);
    let acd = prj.layout.enclosures.iter().position(|e| e.name == "+ACD").unwrap();
    let w12 = an.routing.item("=MD-W12").unwrap();
    let ui = PanelUi::default();
    let lines = ui.routing_lines_3d(&prj, &an, &[acd], &|_, q| q, None);
    let at = w12.ends.iter().find(|e| e.tag == "=ESS+ACD-X0").unwrap();
    let cores: Vec<_> = lines.iter().filter(|l| l.as_is && at.cores.iter().any(|q| l.points.last() == Some(q))).collect();
    assert_eq!(cores.len(), 5);
    for (i, a) in cores.iter().enumerate() {
        assert!(a.diameter < w12.diameter * 0.5);
        for b in &cores[i + 1..] {
            assert!(vec3::dist(a.points[0], b.points[0]) >= a.diameter - 0.01, "{:?} {:?}", a.points[0], b.points[0]);
        }
    }
}

#[test]
fn a_cable_ending_behind_a_corner_ends_on_its_bend() {
    // Along x, round a corner (bend radius 100) and 110 on along y to the middle of two
    // connection points: the cores part 60 before them, 50 behind the corner, where the
    // bend of the cable has not ended yet.
    let way = [[0.0, 0.0, 0.0], [500.0, 0.0, 0.0], [500.0, 60.0, 0.0], [500.0, 110.0, 0.0]];
    let at = [([495.0, 110.0, 0.0], [0.0, -1.0, 0.0]), ([505.0, 110.0, 0.0], [0.0, -1.0, 0.0])];
    let (keep, tail, cores) = routing_ui::fan_out(&way, &[0.0, 100.0, 0.0, 0.0], &at, 20.0, 5.0, None);
    // The cable keeps its way up to the corner and ends on the bend round it, as far
    // behind the corner (along y) as it was cut.
    assert_eq!(keep, 1);
    assert!(vec3::dist(tail[0], [400.0, 0.0, 0.0]) < 0.01 && tail.len() > 3, "{tail:?}");
    assert!(tail.iter().all(|q| (vec3::dist(*q, [400.0, 100.0, 0.0]) - 100.0).abs() < 0.01), "{tail:?}");
    let end = tail[tail.len() - 1];
    assert!((end[1] - 50.0).abs() < 0.01, "{tail:?}");
    // The cores go on from there the way the bend runs.
    for core in &cores {
        let on = vec3::norm(vec3::sub(core[1], core[0]));
        assert!(vec3::dist(core[0], end) < 10.0 && on[0] > 0.3 && on[1] > 0.7, "{core:?}");
    }
    // Further behind the corner nothing changes: it ends on its straight way.
    let long = [[0.0, 0.0, 0.0], [500.0, 0.0, 0.0], [500.0, 300.0, 0.0], [500.0, 350.0, 0.0]];
    let at = [([495.0, 350.0, 0.0], [0.0, -1.0, 0.0]), ([505.0, 350.0, 0.0], [0.0, -1.0, 0.0])];
    let (keep, tail, _) = routing_ui::fan_out(&long, &[0.0, 100.0, 0.0, 0.0], &at, 20.0, 5.0, None);
    assert_eq!((keep, tail), (2, vec![[500.0, 290.0, 0.0]]));
}

#[test]
fn the_faces_of_the_busbars_point_outward() {
    // Seen in the view, every face of the bars and their holders points away from the
    // system (a bar wound inside out shows its far faces, which fight with the holders
    // behind them); and nothing of a busbar lies in the plane of the plate.
    let prj = openecraft_core::sample::example("industrial_main_distribution").unwrap();
    let an = Analysis::build(&prj);
    let mut ui = PanelUi { enclosure: 0, show_wires: false, ..Default::default() };
    let shown = ui.shown_3d(&prj, 0);
    let s = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
    let e = &prj.layout.enclosures[0];
    let mut checked = 0;
    for bar in e.items.iter().filter(|i| i.kind == ItemKind::Busbar) {
        let (lo, hi) = panel::item_box(e, bar);
        let (lo, hi) = ([lo[0], e.height - hi[1], lo[2]], [hi[0], e.height - lo[1], hi[2]]);
        for f in s.faces.iter().filter(|f| f.item == Some((0, bar.id))) {
            let n = f.n as usize;
            let mid = [0, 1, 2].map(|k| f.pts[..n].iter().map(|p| p[k]).sum::<f32>() / n as f32);
            // A face on the outside of the system's box points out of it.
            let axis = (0..3).max_by(|&i, &j| f.normal[i].abs().total_cmp(&f.normal[j].abs())).unwrap();
            let out = if (mid[axis] - hi[axis]).abs() < 0.01 { 1.0 } else if (mid[axis] - lo[axis]).abs() < 0.01 { -1.0 } else { continue };
            assert!(f.normal[axis] * out > 0.99, "{} face at {mid:?} with normal {:?}", bar.part, f.normal);
            checked += 1;
        }
    }
    assert!(checked > 50, "{checked}");
}

#[test]
fn the_cores_of_a_cable_stay_between_their_ends() {
    // A cable up along y (z 200 in front of the plate) that turns towards the plate
    // right before its three terminals, 185 apart up the plate at z 90: the cores part
    // in the turn, heading at the plate, and must not swing on past the terminals
    // (into the bars they sit on) before they come up to them.
    let way = [[0.0, 0.0, 200.0], [0.0, 700.0, 200.0], [0.0, 705.0, 90.0], [0.0, 890.0, 90.0]];
    let at = [([0.0, 705.0, 90.0], [0.0, -1.0, 0.0]), ([0.0, 890.0, 90.0], [0.0, -1.0, 0.0]), ([0.0, 1075.0, 90.0], [0.0, -1.0, 0.0])];
    let (_, tail, cores) = routing_ui::fan_out(&way, &[0.0, 110.0, 110.0, 0.0], &at, 55.0, 14.0, None);
    let end = tail[tail.len() - 1];
    for (core, (q, _)) in cores.iter().zip(&at) {
        let start = core[0];
        assert!(vec3::dist(start, end) < 30.0, "{start:?} off the cable's end {end:?}");
        for p in core {
            assert!(p[2] >= start[2].min(q[2]) - 0.01 && p[2] <= start[2].max(q[2]) + 0.01 && p[1] <= q[1] + 0.01, "{core:?}");
        }
    }
    // The terminal strip's box reaches to z 125: the cable ends clear of it (its
    // radius and more in front), the cores to the upper two terminals pass the lower
    // ones in front of the box (a core's radius and more), the core to the first comes
    // down into it without rising first.
    let strip = (([-20.0, 680.0, 20.0], [20.0, 1100.0, 125.0]), [0.0, 0.0, 1.0]);
    let (_, tail, cores) = routing_ui::fan_out(&way, &[0.0, 110.0, 110.0, 0.0], &at, 55.0, 14.0, Some(strip));
    assert!(tail.iter().all(|p| p[2] >= 125.0 + 27.5 + 1.0 - 0.01), "{tail:?}");
    for (k, (core, (q, _))) in cores.iter().zip(&at).enumerate() {
        let passing: Vec<&[f32; 3]> = core.iter().filter(|p| at[..k].iter().any(|(o, _)| (p[1] - o[1]).abs() < 30.0)).collect();
        assert!(passing.iter().all(|p| p[2] >= 125.0 + 7.0 + 1.0 - 0.5), "core {k}: {core:?}");
        assert!(core.iter().all(|p| p[2] <= 250.0 && p[1] <= q[1] + 0.01), "core {k}: {core:?}");
        if k == 0 {
            assert!(core.iter().all(|p| p[2] <= core[0][2] + 0.01), "{core:?}");
        }
    }
}

#[test]
fn a_cable_that_is_not_routed_is_marked_in_the_3d_view() {
    // The motor cable of the demo with half a metre of clearance: not drawn, a
    // warning at both of its ends in the cabinet instead.
    let mut prj = openecraft_core::sample::demo_project();
    panel::place_accessory(&mut prj, 0, "GEN.GLAND-M25", Face::Bottom, openecraft_core::geom::p(80.0, 100.0)).unwrap();
    prj.layout.routing.clearance = 500.0;
    let an = Analysis::build(&prj);
    assert!(!an.routing.item("-W1").unwrap().routed());
    let mut ui = PanelUi { enclosure: 0, show_wires: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, 0);
    let s = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
    let warnings: Vec<&scene3d::SceneLabel> = s.labels.iter().filter(|l| l.warning && l.tag.contains("-W1")).collect();
    assert_eq!(warnings.len(), 2, "{:?}", s.labels.iter().filter(|l| l.warning).map(|l| &l.tag).collect::<Vec<_>>());
    assert!(warnings.iter().all(|l| l.tag.contains(&openecraft_i18n::tr("not routed: {}").replace("{}", ""))), "{}", warnings[0].tag);
    assert!(!s.tube_paths.iter().any(|(_, _, t)| matches!(t, routing_ui::RouteRef::Item(k) if k == "-W1")));
}

/// Where the segments `a`–`b` and `c`–`d` come nearest to each other: the points on both.
fn segment_nearest(a: [f32; 3], b: [f32; 3], c: [f32; 3], d: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    use openecraft_core::parts::vec3::{add, dot, sub};
    let (u, v, w) = (sub(b, a), sub(d, c), sub(a, c));
    let (uu, uv, vv, uw, vw) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let den = uu * vv - uv * uv;
    let mut s = if den.abs() < 1e-6 { 0.0 } else { ((uv * vw - vv * uw) / den).clamp(0.0, 1.0) };
    let t = if vv > 1e-9 { (uv * s + vw) / vv } else { 0.0 }.clamp(0.0, 1.0);
    s = if uu > 1e-9 { (uv * t - uw) / uu } else { 0.0 }.clamp(0.0, 1.0);
    (add(a, u.map(|x| x * s)), add(c, v.map(|x| x * t)))
}

/// Least distance from the segment `a`–`b` to a box (sampled along the segment).
fn box_gap(a: [f32; 3], b: [f32; 3], (lo, hi): ([f32; 3], [f32; 3])) -> f32 {
    (0..=12)
        .map(|k| {
            let t = k as f32 / 12.0;
            let q = [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
            let d = [0, 1, 2].map(|i| (lo[i] - q[i]).max(q[i] - hi[i]).max(0.0));
            (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
        })
        .fold(f32::INFINITY, f32::min)
}

/// The locknut of a cable gland inside its enclosure (view coordinates), the hole of the
/// gland through its wall, the box of the whole gland, its id and a name.
type Locknut = (([f32; 3], [f32; 3]), ([f32; 3], [f32; 3]), ([f32; 3], [f32; 3]), u64, String);

/// The segment `a`–`b` runs through the box (exactly, however thin the box).
fn segment_crosses_box(a: [f32; 3], b: [f32; 3], (lo, hi): ([f32; 3], [f32; 3])) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for k in 0..3 {
        let d = b[k] - a[k];
        if d.abs() < 1e-9 {
            if a[k] < lo[k] || a[k] > hi[k] {
                return false;
            }
            continue;
        }
        let (u, v) = ((lo[k] - a[k]) / d, (hi[k] - a[k]) / d);
        t0 = t0.max(u.min(v));
        t1 = t1.min(u.max(v));
    }
    t0 <= t1
}

fn within_box(q: [f32; 3], (lo, hi): ([f32; 3], [f32; 3])) -> bool {
    (0..3).all(|k| q[k] >= lo[k] && q[k] <= hi[k])
}

/// Distance of a point (x, z) from the edges of a floor plan.
fn outline_gap(pts: &[[f32; 2]], q: [f32; 2]) -> f32 {
    let mut least = f32::INFINITY;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        let (ab, aq) = ([b[0] - a[0], b[1] - a[1]], [q[0] - a[0], q[1] - a[1]]);
        let len2 = ab[0] * ab[0] + ab[1] * ab[1];
        let t = if len2 < 1e-9 { 0.0 } else { ((aq[0] * ab[0] + aq[1] * ab[1]) / len2).clamp(0.0, 1.0) };
        let p = [a[0] + ab[0] * t, a[1] + ab[1] * t];
        least = least.min(((q[0] - p[0]).powi(2) + (q[1] - p[1]).powi(2)).sqrt());
    }
    least
}

/// What the wires, cables and tubes drawn in the 3D views of a project (every line-up
/// and room) clip: each other and themselves (beyond the overlap the layout leaves
/// where they are packed: a millimetre and a half between wires, two and a half
/// millimetres where a cable or pipe is involved), the devices, busbars, rails and
/// accessories, the bases, walls (but the fingers taken out) and covers of the cable
/// ducts, the walls, plate and door of a cabinet (but where they go out through a
/// gland or a passage), and the walls, floor and ceiling of a room; a line per clip.
fn clipping_in_3d(prj: &Project) -> Vec<String> {
    use openecraft_core::panel::{self, Face, ItemKind, Role, PLATE_Z};
    use openecraft_core::parts::vec3::V3;
    let an = Analysis::build(prj);
    let pos = panel::positions(&prj.layout);
    let tol = 0.3;
    let mut out = Vec::new();
    let scenes: Vec<usize> = (0..prj.layout.enclosures.len()).filter(|&k| prj.layout.enclosures[k].is_room() || panel::lineup_of(&prj.layout, k).first() == Some(&k)).collect();
    for ei in scenes {
        let mut ui = PanelUi { enclosure: ei, show_wires: true, ..Default::default() };
        let shown = ui.shown_3d(prj, ei);
        // The tubes are laid out on another thread: the scene with them laid out.
        let ctx = egui::Context::default();
        let mut s = ui.scene_3d(&ctx, prj, &an, ei, &shown);
        let started = std::time::Instant::now();
        while s.waiting.is_some() && started.elapsed().as_secs() < 600 {
            std::thread::sleep(std::time::Duration::from_millis(20));
            s = ui.scene_3d(&ctx, prj, &an, ei, &shown);
        }
        assert!(s.waiting.is_none(), "the layout of {} was not done", prj.layout.enclosures[ei].name);
        let view = prj.layout.enclosures[ei].name.clone();
        let label = |t: &routing_ui::RouteRef| match t {
            routing_ui::RouteRef::Wire(wi) => format!("wire {} - {}", an.wiring[*wi].from, an.wiring[*wi].to),
            routing_ui::RouteRef::Item(key) => key.clone(),
            routing_ui::RouteRef::None => "?".into(),
        };
        let ends_of = |t: &routing_ui::RouteRef| -> Vec<u64> {
            match t {
                routing_ui::RouteRef::Wire(wi) => vec![an.wiring[*wi].items.0, an.wiring[*wi].items.1],
                routing_ui::RouteRef::Item(key) => an.routing.item(key).map(|i| i.ends.iter().filter_map(|e| e.item).collect()).unwrap_or_default(),
                routing_ui::RouteRef::None => Vec::new(),
            }
        };
        // The tubes: way, radius, name, the items at the ends.
        let tubes: Vec<(Vec<V3>, f32, String, Vec<u64>)> = s.tube_paths.iter().map(|(p, r, t)| (p.clone(), *r, label(t), ends_of(t))).collect();
        // Wires to the door run along the door leaf: not held to the shell.
        let on_door: Vec<bool> = s.tube_paths.iter().map(|(_, _, t)| matches!(t, routing_ui::RouteRef::Wire(wi) if an.wiring[*wi].door != (None, None))).collect();
        // Solids (view coordinates) with the item they are (0: none) and a name; holes
        // through the shells a tube may pass: glands, passages, openings.
        let mut solids: Vec<((V3, V3), u64, String)> = Vec::new();
        // The cable trays of a room: inside one a tube is off the room's surfaces.
        let mut trays: Vec<(V3, V3)> = Vec::new();
        // The boxes of the items (view coordinates) by id.
        let mut boxes: std::collections::HashMap<u64, (V3, V3)> = std::collections::HashMap::new();
        // Holes with how far round them a tube may still pass (through the shell).
        let mut holes: Vec<((V3, V3), f32)> = Vec::new();
        // The locknuts of the cable glands inside, with the hole of each through its
        // wall and the box of the whole gland.
        let mut nuts: Vec<Locknut> = Vec::new();
        let grown = |(lo, hi): (V3, V3), g: f32| (lo.map(|v| v - g), hi.map(|v| v + g));
        for &(k, pose) in &shown {
            let e = &prj.layout.enclosures[k];
            let h = e.height;
            let seen = |(lo, hi): (V3, V3)| -> (V3, V3) {
                let pts: Vec<V3> = (0..8).map(|c| pose.apply([[lo[0], hi[0]][c & 1], h - [lo[1], hi[1]][c >> 1 & 1], [lo[2], hi[2]][c >> 2]])).collect();
                ([0, 1, 2].map(|i| pts.iter().map(|q| q[i]).fold(f32::INFINITY, f32::min)), [0, 1, 2].map(|i| pts.iter().map(|q| q[i]).fold(f32::NEG_INFINITY, f32::max)))
            };
            let name = |it: &openecraft_core::panel::PanelItem| if it.tag.is_empty() { format!("{} {}", e.name, it.part) } else { it.tag.clone() };
            for it in &e.items {
                let bx = panel::item_box(e, it);
                boxes.insert(it.id, seen(bx));
                if it.kind == ItemKind::Duct && e.is_room() {
                    trays.push(seen(bx));
                }
                match it.kind {
                    // A terminal strip across the bars: each terminal a solid of its own,
                    // so that a core to one of them may not pass through the others.
                    ItemKind::Device if panel::busbars::terminals_on_bars(prj, it).is_some() => {
                        for (sub, _, label) in panel::busbars::terminals_on_bars(prj, it).unwrap_or_default() {
                            solids.push((seen(panel::item_box(e, &sub)), it.id, format!("{}:{label}", name(it))));
                        }
                    }
                    ItemKind::Device | ItemKind::Busbar | ItemKind::Rail => solids.push((seen(bx), it.id, name(it))),
                    ItemKind::Accessory => {
                        // Cable glands and the membranes of cable entry flanges.
                        if openecraft_core::routing3d::entries(prj, k).iter().any(|g| g.id == it.id) {
                            holes.push((seen(bx), e.wall() + 8.0));
                            // Its locknut inside: in the way of every cable but the one through it.
                            if it.z + it.d > 0.0 {
                                let nut = openecraft_core::panel::PanelItem { z: 0.0, d: it.z + it.d, ..it.clone() };
                                let hole = openecraft_core::panel::PanelItem { z: -e.wall(), d: e.wall(), ..it.clone() };
                                nuts.push((seen(panel::item_box(e, &nut)), seen(panel::item_box(e, &hole)), seen(bx), it.id, format!("{} locknut", name(it))));
                            }
                        } else {
                            solids.push((seen(bx), it.id, name(it)));
                        }
                    }
                    ItemKind::Duct if it.face == Face::Plate && !e.is_room() => {
                        for (part, _, finger) in panel::duct_boxes(e, it, true) {
                            let b = seen(part);
                            let drawn = s.faces.iter().filter(|f| f.item == Some((k, it.id))).any(|f| (0..f.n as usize).all(|i| within_box(f.pts[i], grown(b, 0.01))));
                            if !finger || drawn {
                                solids.push((b, it.id, format!("{} duct at ({:.0}, {:.0}){}", e.name, it.x, it.y, if finger { " (finger)" } else { "" })));
                            }
                        }
                    }
                    ItemKind::Cutout => holes.push((seen(bx), e.wall() + 1.0)),
                    _ => {}
                }
            }
            if e.is_room() {
                for c in panel::components(e).into_iter().filter(|c| matches!(c.role, Role::RoomWall(Face::Inner(..)))) {
                    for bx in c.boxes {
                        solids.push((seen(bx), 0, format!("{} inner wall", e.name)));
                    }
                }
            } else {
                for p in panel::passages(&prj.layout, &pos, k) {
                    let c = [p.at[0] - pos[k][0], p.at[1] - pos[k][1], p.at[2] - pos[k][2]];
                    let t = e.wall() + 4.0;
                    holes.push((seen(([c[0] - t, c[1] - p.half[0] - 1.0, c[2] - p.half[1] - 1.0], [c[0] + t, c[1] + p.half[0] + 1.0, c[2] + p.half[1] + 1.0])), 0.0));
                }
            }
        }
        let in_hole = |q: V3, r: f32| holes.iter().any(|(b, g)| within_box(q, grown(*b, g + 2.0 * r)));
        // Points along a tube, every 10 mm at least.
        let samples = |way: &[V3]| -> Vec<V3> {
            let mut v = Vec::new();
            for seg in way.windows(2) {
                let n = (vec3::dist(seg[0], seg[1]) / 10.0).ceil().max(1.0) as usize;
                for k in 0..n {
                    let t = k as f32 / n as f32;
                    v.push([0, 1, 2].map(|i| seg[0][i] + (seg[1][i] - seg[0][i]) * t));
                }
            }
            v.extend(way.last().copied());
            v
        };
        let room = prj.layout.enclosures[ei].is_room().then(|| (panel::outline(&prj.layout.enclosures[ei]), prj.layout.enclosures[ei].height));
        for (ti, (way, r, what, ends)) in tubes.iter().enumerate() {
            let r = *r;
            // Solids; the item a tube ends at only where it comes up to its end there.
            let zone = (4.0 * r + 20.0).max(3.0 * prj.layout.routing.grid + r);
            let at_end = |b: &(V3, V3)| [way.first(), way.last()].into_iter().flatten().any(|q| within_box(*q, grown(*b, zone)));
            for seg in way.windows(2) {
                for (b, id, name) in &solids {
                    if ends.contains(id) && at_end(b) {
                        continue;
                    }
                    let gap = box_gap(seg[0], seg[1], *b);
                    if gap < r - tol {
                        out.push(format!("{view}: {what} (r {r:.1}) into {name} [({:.1}, {:.1}, {:.1}) - ({:.1}, {:.1}, {:.1})] by {:.1} mm, segment ({:.1}, {:.1}, {:.1}) - ({:.1}, {:.1}, {:.1})", b.0[0], b.0[1], b.0[2], b.1[0], b.1[1], b.1[2], r - gap, seg[0][0], seg[0][1], seg[0][2], seg[1][0], seg[1][1], seg[1][2]));
                        break;
                    }
                }
                // The cable through a gland (or ending at it, or drawn up to it where the
                // room behind is not shown) passes its locknut; every other one keeps clear.
                for (b, hole, gland, id, name) in &nuts {
                    let through = ends.contains(id) || way.windows(2).any(|s| segment_crosses_box(s[0], s[1], *hole)) || [way.first(), way.last()].into_iter().flatten().any(|q| within_box(*q, grown(*gland, 1.0)));
                    let gap = box_gap(seg[0], seg[1], *b);
                    if !through && gap < r - tol {
                        out.push(format!("{view}: {what} (r {r:.1}) into {name} by {:.1} mm", r - gap));
                        break;
                    }
                }
            }
            // Shells of the cabinets shown, and the room.
            for q in samples(way) {
                if in_hole(q, r) || on_door[ti] {
                    continue;
                }
                let mut in_cabinet = false;
                for &(k, pose) in &shown {
                    let e = &prj.layout.enclosures[k];
                    if e.is_room() {
                        continue;
                    }
                    let l = pose.local(q);
                    let (w, h, d, t) = (e.width, e.height, e.depth, e.wall());
                    if l[0] <= 0.0 || l[0] >= w || l[1] <= 0.0 || l[1] >= h || l[2] <= 0.0 || l[2] >= d {
                        continue;
                    }
                    in_cabinet = true;
                    let bad = if l[0] < t + r - tol {
                        Some("left wall")
                    } else if l[0] > w - t - r + tol {
                        Some("right wall")
                    } else if l[1] < t + r - tol {
                        Some("bottom")
                    } else if l[1] > h - t - r + tol {
                        Some("roof")
                    } else if l[2] < PLATE_Z + r - tol {
                        Some("mounting plate")
                    } else if l[2] > d - t - r + tol {
                        Some("door")
                    } else {
                        None
                    };
                    if let Some(bad) = bad {
                        out.push(format!("{view}: {what} into the {bad} of {} at ({:.0}, {:.0}, {:.0})", e.name, l[0], l[1], l[2]));
                    }
                }
                let in_tray = trays.iter().any(|b| within_box(q, grown(*b, 1.0)));
                // Up to the device at its end a tube comes as near to the room's walls as
                // the device stands.
                let at_end = ends.iter().filter_map(|id| boxes.get(id)).any(|b| within_box(q, grown(*b, 2.0 * r + 1.0)));
                if let (Some((outline, height)), false, false, false) = (&room, in_cabinet, in_tray, at_end) {
                    let inside = panel::inside_outline(outline, [q[0], q[2]]);
                    let gap = outline_gap(outline, [q[0], q[2]]);
                    if !inside || gap < r - tol {
                        out.push(format!("{view}: {what} into a wall of the room at ({:.0}, {:.0}, {:.0})", q[0], q[1], q[2]));
                    } else if q[1] < r - tol {
                        out.push(format!("{view}: {what} into the floor at ({:.0}, {:.0}, {:.0})", q[0], q[1], q[2]));
                    } else if q[1] > height - r + tol {
                        out.push(format!("{view}: {what} into the ceiling at ({:.0}, {:.0}, {:.0})", q[0], q[1], q[2]));
                    }
                    // The cabinets standing in the room (but through their glands).
                    for &(k, pose) in shown.iter().filter(|s| !prj.layout.enclosures[s.0].is_room()) {
                        let e = &prj.layout.enclosures[k];
                        let l = pose.local(q);
                        let d = [(-l[0]).max(l[0] - e.width), (-l[1]).max(l[1] - e.height), (-l[2]).max(l[2] - e.depth)].map(|v| v.max(0.0));
                        let gap = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
                        if gap < r - tol {
                            out.push(format!("{view}: {what} into cabinet {} by {:.1} mm at ({:.0}, {:.0}, {:.0})", e.name, r - gap, q[0], q[1], q[2]));
                        }
                    }
                }
            }
        }
        // Each other and themselves: the segments in a grid of cells.
        let cell = 100.0;
        let mut grid: std::collections::HashMap<[i32; 3], Vec<(usize, usize)>> = std::collections::HashMap::new();
        let key = |v: f32| (v / cell).floor() as i32;
        let cells_of = |a: V3, b: V3, r: f32| -> Vec<[i32; 3]> {
            let lo = [0, 1, 2].map(|k| key(a[k].min(b[k]) - r));
            let hi = [0, 1, 2].map(|k| key(a[k].max(b[k]) + r));
            let mut v = Vec::new();
            for x in lo[0]..=hi[0] {
                for y in lo[1]..=hi[1] {
                    for z in lo[2]..=hi[2] {
                        v.push([x, y, z]);
                    }
                }
            }
            v
        };
        let along: Vec<Vec<f32>> = tubes
            .iter()
            .map(|(way, ..)| {
                let mut v = vec![0.0];
                for seg in way.windows(2) {
                    v.push(v[v.len() - 1] + vec3::dist(seg[0], seg[1]));
                }
                v
            })
            .collect();
        for (i, (way, r, ..)) in tubes.iter().enumerate() {
            for j in 0..way.len().saturating_sub(1) {
                for c in cells_of(way[j], way[j + 1], *r) {
                    grid.entry(c).or_default().push((i, j));
                }
            }
        }
        let mut reported: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
        for (i, (way, r, what, ends)) in tubes.iter().enumerate() {
            for j in 0..way.len().saturating_sub(1) {
                let mut seen: Vec<(usize, usize)> = Vec::new();
                for c in cells_of(way[j], way[j + 1], *r) {
                    for &(i2, j2) in grid.get(&c).map(Vec::as_slice).unwrap_or(&[]) {
                        if (i2, j2) <= (i, j) || seen.contains(&(i2, j2)) {
                            continue;
                        }
                        seen.push((i2, j2));
                        let (way2, r2, what2, _) = &tubes[i2];
                        if i2 == i {
                            // The same tube: far enough along it to be another part.
                            if (along[i][j] - along[i][j2]).abs() < 4.0 * r + vec3::dist(way[j], way[j + 1]) {
                                continue;
                            }
                        } else if what2 == what {
                            continue; // the cores of a cable with it and each other
                        }
                        // Two tubes ending at one connection point converge there, and
                        // the cores of a cable part at its end item where the wires start
                        // (told by where the two come nearest to each other); not for
                        // long: two cables through one entry lie side by side.
                        let (near, other) = segment_nearest(way[j], way[j + 1], way2[j2], way2[j2 + 1]);
                        let ends_at = |w: &[V3]| [w.first(), w.last()].into_iter().flatten().copied().collect::<Vec<V3>>();
                        let shared = ends_at(way).iter().any(|e| ends_at(way2).iter().any(|f| vec3::dist(*e, *f) < r + r2 + 1.0) && vec3::dist(*e, near) < (60.0 * (r + r2)).min(300.0));
                        // At a device both end at they converge as far as the routing lets
                        // them: three of its cells from the device at least.
                        let (_, _, _, ends2) = &tubes[i2];
                        let zone = (4.0 * (r + r2) + 20.0).max(3.0 * prj.layout.routing.grid + r + r2);
                        let same_item = ends.iter().any(|id| ends2.contains(id) && boxes.get(id).is_some_and(|b| within_box(near, grown(*b, zone))));
                        // (A tube with itself converges nowhere.)
                        if i2 != i && (shared || same_item) {
                            continue;
                        }
                        // Pushed apart until they overlap by no more than the layout leaves
                        // where they are packed.
                        let wires = what.starts_with("wire ") && what2.starts_with("wire ");
                        let left = if wires { 1.5 } else { 2.5 };
                        let gap = vec3::dist(near, other);
                        if gap < r + r2 - left && reported.insert((i.min(i2), i.max(i2))) {
                            out.push(format!("{view}: {what} and {what2} by {:.1} mm at ({:.0}, {:.0}, {:.0})", r + r2 - gap, near[0], near[1], near[2]));
                        }
                    }
                }
            }
        }
    }
    out
}

#[test]
fn nothing_drawn_in_3d_clips_anything() {
    let example = |key: &str| openecraft_core::sample::example(key).unwrap();
    let projects = [
        ("demo", openecraft_core::sample::demo_project()),
        ("pump_room", example("pump_room")),
        ("building_services", example("building_services")),
        ("pv_transformer_metering", example("pv_transformer_metering")),
        ("industrial_main_distribution", example("industrial_main_distribution")),
    ];
    for (name, prj) in projects {
        let clips = clipping_in_3d(&prj);
        assert!(clips.is_empty(), "{name}: {} clips:\n{}", clips.len(), clips.join("\n"));
    }
}

/// The tubes drawn in the 3D views of a project and those of them that bend tighter
/// than their bend radius somewhere ([`bundles::tight_bends`]).
fn tight_in_3d(prj: &Project) -> (usize, Vec<String>) {
    use openecraft_core::routing3d::{bundles, RouteKind};
    let an = Analysis::build(prj);
    let (mut all, mut tight) = (0, Vec::new());
    let scenes: Vec<usize> = (0..prj.layout.enclosures.len()).filter(|&k| prj.layout.enclosures[k].is_room() || openecraft_core::panel::lineup_of(&prj.layout, k).first() == Some(&k)).collect();
    for ei in scenes {
        let mut ui = PanelUi { enclosure: ei, show_wires: true, ..Default::default() };
        let shown = ui.shown_3d(prj, ei);
        let ctx = egui::Context::default();
        let mut s = ui.scene_3d(&ctx, prj, &an, ei, &shown);
        while s.waiting.is_some() {
            std::thread::sleep(std::time::Duration::from_millis(20));
            s = ui.scene_3d(&ctx, prj, &an, ei, &shown);
        }
        for (points, half, target) in &s.tube_paths {
            let (radius, name) = match target {
                routing_ui::RouteRef::Wire(wi) => (prj.layout.routing.bend_radius(RouteKind::Wire, 2.0 * half), format!("{} - {}", an.wiring[*wi].from, an.wiring[*wi].to)),
                routing_ui::RouteRef::Item(key) => match an.routing.item(key) {
                    // The cores of a cable are as thin as wires.
                    Some(it) if (2.0 * half - it.diameter).abs() < 0.01 => (it.radii.iter().copied().fold(0.0, f32::max), key.clone()),
                    _ => continue,
                },
                routing_ui::RouteRef::None => continue,
            };
            all += 1;
            if let Some((at, r)) = bundles::tight_bends(points, radius).first() {
                tight.push(format!("{}: {name} with {r:.0} of {radius:.0} mm at ({:.0}, {:.0}, {:.0})", prj.layout.enclosures[ei].name, at[0], at[1], at[2]));
            }
        }
    }
    (all, tight)
}

#[test]
fn tubes_drawn_in_3d_keep_their_bend_radius_where_they_do_not_cross() {
    // The cabinets without cable ducts: every conductor is laid by the 3D routing, bent
    // with its radius, laid on its lane without a step and drawn straight out of its
    // connection points, and routes cross each other only where they come up to their
    // ends. There, and where thin wires come onto their lanes a little tighter than
    // their radius, the view still draws a bend too tight: about one tube in six in
    // example 13 (three of them on its doors, laid along the plate), one in ten in
    // example 14; it was more than half of them.
    for (key, most) in [("pv_transformer_metering", 0.2), ("industrial_main_distribution", 0.12)] {
        let (all, tight) = tight_in_3d(&openecraft_core::sample::example(key).unwrap());
        assert!(all > 50 && (tight.len() as f32) <= most * all as f32, "{key}: {} of {all}:\n{}", tight.len(), tight.join("\n"));
    }
}

/// A house: two levels, a room on each, a stair between them and a gable roof low over
/// the attic with a roof window; the index of the attic.
fn house() -> (Project, usize) {
    let mut prj = Project::default();
    let bi = bld::add_building(&mut prj, "+H1").unwrap();
    bld::add_level(&mut prj, bi, None, None, None).unwrap();
    let levels = [0, 1].map(|k| prj.layout.buildings[bi].levels[k].id);
    let mut rooms = [0; 2];
    for (k, level) in levels.into_iter().enumerate() {
        rooms[k] = bld::add_room(&mut prj, &format!("+R{k}"), Some(level)).unwrap();
        let e = &mut prj.layout.enclosures[rooms[k]];
        (e.width, e.depth) = (8000.0, 6000.0);
        e.site = Some(SitePlace::default());
    }
    panel::stairs::add_stair(&mut prj, bi, panel::StairKind::QuarterLanding, 1000.0, 5000.0, levels[0], None).unwrap();
    let roof = panel::roofs::add_roof(&mut prj, bi, panel::RoofShape::Gable).unwrap();
    let r = prj.layout.buildings[bi].roofs.iter_mut().find(|r| r.id == roof).unwrap();
    r.eaves = 4000.0;
    panel::roofs::add_window(&mut prj, bi, 0, None).unwrap();
    (prj, rooms[1])
}

#[test]
fn the_3d_view_shows_the_whole_building_with_stairs_and_roof() {
    let (prj, attic) = house();
    let an = Analysis::build(&prj);
    let mut ui = PanelUi { in_room: true, ..Default::default() };
    // The room alone: its walls cut by the slope, the roof window seen from inside.
    let shown = ui.shown_3d(&prj, attic);
    assert_eq!(shown.len(), 1);
    let alone = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    assert!(alone.faces.iter().any(|f| f.decal && f.layer == 1), "the roof window inside");
    // The whole building: both rooms, the slabs, the stair and the roof above the attic.
    ui.toggle_building();
    let shown = ui.shown_3d(&prj, attic);
    assert_eq!(shown.len(), 2);
    let all = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    assert!(all.faces.len() > alone.faces.len() + 50, "{} / {}", all.faces.len(), alone.faces.len());
    let ridge = panel::roofs::ridge_height(&prj.layout.buildings[0].roofs[0]) - 3000.0;
    assert!(all.bounds.1[1] > ridge, "the roof over the attic: {:?}", all.bounds);
    assert!(all.bounds.0[1] < -2900.0, "the ground floor below it");
    // Without ceilings and roofs: no roof faces any more.
    ui.hide_ceiling = true;
    let open = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    assert!(open.bounds.1[1] < ridge, "{:?}", open.bounds);
    // All enclosures of the plant: the building in the plant's coordinates.
    ui.hide_ceiling = false;
    ui.show_site = true;
    let shown = ui.shown_3d(&prj, attic);
    let site = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    assert!(site.bounds.1[1] > ridge + 3000.0 - 1.0 && site.bounds.0[1] > -400.0, "{:?}", site.bounds);
}

#[test]
fn pv_arrays_lie_on_the_roof_of_the_whole_building() {
    let (mut prj, attic) = house();
    let mut ui = PanelUi { in_room: true, ..Default::default() };
    ui.toggle_building();
    let faces = |ui: &mut PanelUi, prj: &Project| {
        let an = Analysis::build(prj);
        let shown = ui.shown_3d(prj, attic);
        ui.scene_3d(&egui::Context::default(), prj, &an, attic, &shown).faces.len()
    };
    let before = faces(&mut ui, &prj);
    // Two rows of four modules on the rear slope: each module ten faces, each rail four.
    panel::solar::add_array(&mut prj, 0, 0, Some([4000.0, 1500.0])).unwrap();
    assert!(panel::solar::problems(&prj).is_empty());
    assert_eq!(faces(&mut ui, &prj), before + 8 * 10 + 4 * 4);
    // Hidden with the roof.
    ui.hide_ceiling = true;
    let (mut bare, _) = house();
    bare.layout.buildings[0].roofs[0].solar.clear();
    let hidden = faces(&mut ui, &prj);
    assert_eq!(hidden, faces(&mut ui, &bare));
}

#[test]
fn installation_zones_and_opened_walls_are_seen_through() {
    let (prj, attic) = house();
    let an = Analysis::build(&prj);
    let mut ui = PanelUi { in_room: true, ..Default::default() };
    let shown = ui.shown_3d(&prj, attic);
    let plain = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    assert!(!plain.sides.iter().any(|s| s.glass));
    ui.show_zones = true;
    let zones = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    let bands = openecraft_core::routing3d::flush::bands(&prj, attic, &prj.layout.routing).len();
    assert_eq!(zones.sides.iter().filter(|s| s.glass).count(), bands);
    // Opened: the walls of the room are glass too, seen from inside and outside.
    ui.show_zones = false;
    ui.open_walls = true;
    let open = ui.scene_3d(&egui::Context::default(), &prj, &an, attic, &shown);
    let walls = open.sides.iter().filter(|s| s.glass).count();
    assert!(walls >= 4, "{walls}");
    let eye = [100_000.0, 100_000.0, 100_000.0];
    assert!(open.sides.iter().filter(|s| s.glass).all(|s| s.layer(eye, true) == Some(OUTER_WALLS)));
}

#[test]
fn wire_ends_show_their_ferrules_or_the_bare_copper() {
    let mut prj = openecraft_core::sample::demo_project();
    let mut ui = PanelUi { show_wires: true, ..Default::default() };
    let rgb = |c: [u8; 3]| egui::Color32::from_rgb(c[0], c[1], c[2]);
    let (sleeve, copper) = (rgb(openecraft_core::production::SLEEVE_RGB), rgb(openecraft_core::production::COPPER_RGB));
    for ferrules in [true, false] {
        prj.layout.ferrules = ferrules;
        let an = Analysis::build(&prj);
        let shown = ui.shown_3d(&prj, 0);
        let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
        let count = |c: egui::Color32| scene.wires.iter().filter(|w| w.line.tube && w.line.color == c).count();
        let (with, bare) = (count(sleeve), count(copper));
        assert!(if ferrules { with > 0 && bare == 0 } else { with == 0 && bare > 0 }, "ferrules {ferrules}: {with} sleeve, {bare} copper pieces");
    }
    // Ring terminals chosen at a device: their rings are flat faces among the tubes,
    // their insulated sleeves in the colour of the cross-section.
    prj.layout.ferrules = true;
    let an = Analysis::build(&prj);
    let device = an.wiring.iter().find(|w| w.cable.is_empty()).map(|w| w.from.rsplit_once(':').unwrap().0.to_string()).unwrap();
    let before = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &ui.shown_3d(&prj, 0)).tubes.len();
    openecraft_core::production::set_termination(&mut prj, &device, "", Some(openecraft_core::production::Termination::Ring));
    let an = Analysis::build(&prj);
    let shown = ui.shown_3d(&prj, 0);
    let scene = ui.scene_3d(&egui::Context::default(), &prj, &an, 0, &shown);
    assert!(scene.tubes.len() > before, "the rings add faces: {before} -> {}", scene.tubes.len());
    let insulated = [[210, 40, 40], [40, 90, 200], [235, 200, 40]].map(rgb);
    assert!(scene.wires.iter().any(|w| w.line.tube && insulated.contains(&w.line.color)), "insulated sleeves");
}

#[test]
fn cables_leaving_the_enclosure_have_their_core_ends() {
    // -W1 runs from the cabinet to the motor: its cores are not in the panel wiring.
    let mut prj = openecraft_core::sample::demo_project();
    let an = Analysis::build(&prj);
    assert!(an.wiring.iter().all(|w| w.cable != "-W1"));
    let ends = terminations::core_ends(&prj, &an, "-W1");
    assert!(ends.iter().any(|(at, _, _)| at == "-X1:1"), "{ends:?}");
    let other = ends.iter().find(|(at, _, _)| at == "-X1:1").unwrap().1.clone();
    openecraft_core::production::set_termination(&mut prj, "-X1:1", &other, Some(openecraft_core::production::Termination::Fork));
    assert!(!routing_ui::core_end(&prj, &an, "-W1", "-X1:1").is_empty());
}
