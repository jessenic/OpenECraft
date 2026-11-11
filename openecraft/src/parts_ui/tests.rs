use super::*;
use openecraft_core::sample::demo_project;

fn read(kind: ModelKind, bytes: &str) -> pe::Model {
    pe::read_model(kind, "C:/models/part.x", bytes.as_bytes()).unwrap()
}

#[test]
fn imported_models_go_to_the_requested_part() {
    let mut prj = demo_project();
    let mut db = PartsDb::default();
    let mut ui = PartsUi::default();
    let svg = r#"<svg width="45mm" height="74mm" viewBox="0 0 45 74"><rect x="0" y="0" width="45" height="74"/></svg>"#;
    // Without a requested target nothing happens.
    assert!(ui.assign_model("front.svg", read(ModelKind::Front2d, svg), &mut db, &mut prj).is_err());

    ui.model_target = Some((Source::Database, "GEN.CT-9A-24DC".into()));
    ui.assign_model("front.svg", read(ModelKind::Front2d, svg), &mut db, &mut prj).unwrap();
    assert_eq!(db.get("GEN.CT-9A-24DC").unwrap().model_2d.as_ref().unwrap().prims.len(), 1);
    assert!(ui.model_target.is_none());

    // Project parts are found as well.
    let obj = "v 0 0 0\nv 30 0 0\nv 30 20 0\nv 0 20 10\nf 1 2 3\nf 1 3 4\n";
    prj.parts.push(Part { number: "NEW.PART".into(), ..Default::default() });
    ui.model_target = Some((Source::Project, "NEW.PART".into()));
    let msg = ui.assign_model("C:/models/part.obj", pe::read_model(ModelKind::Mesh3d, "C:/models/part.obj", obj.as_bytes()).unwrap(), &mut db, &mut prj).unwrap();
    assert!(msg.contains("part.obj"), "{msg}");
    let part = prj.parts.iter().find(|p| p.number == "NEW.PART").unwrap();
    assert_eq!(part.model_3d.as_ref().unwrap().triangles.len(), 2);

    // Files that are no model are rejected when they are read.
    assert!(pe::read_model(ModelKind::Mesh3d, "x.step", b"no step file").is_err());
}

#[test]
fn dropped_parts_come_from_the_project_the_database_the_catalog_or_the_server() {
    let mut prj = Project::default();
    // The default database holds the example catalog; this one is empty.
    let mut db = PartsDb { parts: Vec::new() };
    assert_eq!(drop_source(&prj, &db, "GEN.LMP-GN-24"), DropSource::Catalog);
    assert_eq!(drop_source(&prj, &db, "SRV.ONLY-1"), DropSource::Server);
    db.parts.push(Part { number: "SRV.ONLY-1".into(), ..Default::default() });
    assert_eq!(drop_source(&prj, &db, "SRV.ONLY-1"), DropSource::Local, "imported into the database");
    prj.parts.push(Part { number: "PRJ.OWN".into(), ..Default::default() });
    assert_eq!(drop_source(&prj, &db, "PRJ.OWN"), DropSource::Local);
}

#[test]
fn part_selection_keeps_its_size() {
    let (mut prj, db) = (demo_project(), PartsDb { parts: parts::default_catalog() });
    let mut parts_ui = PartsUi::default();
    parts_ui.select_for("-K1");
    let ctx = egui::Context::default();
    let mut heights = Vec::new();
    for _ in 0..40 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1600., 1200.))), ..Default::default() };
        ctx.run_ui(input, |ui| parts_ui.selector(&ui.ctx().clone(), &db, &mut prj)).textures_delta.clear();
        heights.push(ctx.memory(|m| m.area_rect(egui::Id::unique(SELECTOR))).expect("window shown").height());
    }
    // It grew by its rows above and below the list every frame, up to the screen.
    assert!(heights[39] < 700. && heights[39] == heights[20], "{heights:?}");
}
