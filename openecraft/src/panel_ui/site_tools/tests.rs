use super::*;
use openecraft_core::panel::buildings as bld;

#[test]
fn the_boundary_is_drawn_corner_by_corner_and_closed() {
    let mut prj = Project::default();
    let mut ui = PanelUi { view: View::ThreeD, ..Default::default() };
    ui.choose_site_tool(SiteTool::Boundary);
    assert_eq!((ui.site, ui.view, ui.site_tool), (true, View::Plan, SiteTool::Boundary), "the site plan in 2D");
    for q in [[0.0, 0.0], [30_000.0, 0.0], [30_000.0, 20_000.0]] {
        ui.site_click(&mut prj, q, false);
    }
    ui.site_key(&mut prj, Key::Backspace);
    assert_eq!(ui.site_points.len(), 2, "Backspace takes back the last corner");
    ui.site_click(&mut prj, [30_000.0, 20_000.0], false);
    ui.site_click(&mut prj, [0.0, 20_000.0], false);
    ui.site_click(&mut prj, [5.0, 0.0], true);
    assert_eq!(prj.layout.plot.outline.len(), 4, "closed on the first corner");
    assert_eq!((ui.site_tool, ui.site_points.len()), (SiteTool::Select, 0));
    assert!(ui.status.contains("600.0"), "{}", ui.status);
    // A boundary crossing itself stays to be corrected; Enter closes it too.
    ui.choose_site_tool(SiteTool::Boundary);
    for q in [[0.0, 0.0], [10_000.0, 10_000.0], [10_000.0, 0.0], [0.0, 10_000.0]] {
        ui.site_click(&mut prj, q, false);
    }
    ui.site_key(&mut prj, Key::Enter);
    assert_eq!((ui.site_points.len(), prj.layout.plot.outline[1]), (4, [30_000.0, 0.0]), "{}", ui.status);
    ui.site_key(&mut prj, Key::Escape);
    ui.site_key(&mut prj, Key::Escape);
    assert_eq!((ui.site_points.len(), ui.site_tool), (0, SiteTool::Select));
}

#[test]
fn rooms_outside_the_boundary_are_named_when_it_is_closed() {
    let mut prj = Project::default();
    bld::add_building(&mut prj, "+B1").unwrap();
    let l0 = prj.layout.buildings[0].levels[0].id;
    bld::add_room(&mut prj, "+R1", Some(l0)).unwrap();
    let mut ui = PanelUi::default();
    ui.choose_site_tool(SiteTool::Boundary);
    for q in [[2000.0, -2000.0], [20_000.0, -2000.0], [20_000.0, 20_000.0], [2000.0, 20_000.0]] {
        ui.site_click(&mut prj, q, false);
    }
    ui.site_key(&mut prj, Key::Enter);
    assert!(ui.status.contains("+R1"), "{}", ui.status);
}

#[test]
fn the_reference_point_is_placed_and_points_are_measured() {
    let mut prj = Project::default();
    prj.layout.plot.reference = Some(GeoReference { easting: 500_000.0, northing: 5_000_000.0, crs: "EPSG:25832".into(), ..Default::default() });
    let mut ui = PanelUi::default();
    ui.choose_site_tool(SiteTool::Reference);
    ui.site_click(&mut prj, [1000.0, 2000.0], false);
    let r = prj.layout.plot.reference.clone().unwrap();
    assert_eq!((r.x, r.z, r.easting, r.crs.as_str(), ui.site_tool), (1000.0, 2000.0, 500_000.0, "EPSG:25832", SiteTool::Select), "the coordinates are kept");
    assert!(site_plan::coordinates(&prj, [4000.0, -2000.0]).contains("E 500003.000  N 5000004.000"));
    ui.choose_site_tool(SiteTool::Measure);
    ui.site_click(&mut prj, [0.0, 0.0], false);
    ui.site_click(&mut prj, [3000.0, -4000.0], false);
    assert!(ui.status.starts_with("5.000 m"), "{}", ui.status);
    ui.site_click(&mut prj, [0.0, 0.0], false);
    assert_eq!(ui.site_points.len(), 1, "a third click starts again");
    // Chosen again, the tool stops.
    ui.choose_site_tool(SiteTool::Measure);
    assert_eq!(ui.site_tool, SiteTool::Select);
}
