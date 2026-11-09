use super::*;
use crate::analysis::Analysis;
use crate::sample::demo_project;

#[test]
fn explicit_panel_rotations_preserve_square_orientation_and_round_trip() {
    assert_eq!(serde_json::from_str::<PanelRotation>("-90").unwrap(),PanelRotation::Deg270);
    assert!(serde_json::from_str::<PanelRotation>("45").is_err());
    let mut item=device(90000,100.,200.,40.,40.,0.,20.);
    assert_eq!(item_rotation(&item,40.),PanelRotation::Deg0);
    item.rotation=Some(PanelRotation::Deg180);
    assert_eq!(item_rotation(&item,40.),PanelRotation::Deg180);
    assert_eq!(front_point(&item,PanelRotation::Deg180,0.,40.,[0.25,0.75]),p(130.,210.));
    let original=item.clone();
    for _ in 0..4 {item.rotate();}
    assert_eq!(item,original);
    let mut prj=Project::default();
    prj.layout.enclosures.push(Enclosure {items:vec![item],..Default::default()});
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(reopened.layout.enclosures[0].items[0].rotation,Some(PanelRotation::Deg180));
}

#[test]
fn explicit_panel_orientation_maps_mesh_vertices_and_points_identically() {
    let mesh=crate::parts::Mesh3d {vertices:vec![[0.,0.,0.],[20.,40.,10.],[3.,33.,4.]],triangles:vec![[0,1,2]],..Default::default()};
    for face in [Face::Plate,Face::Bottom] {
        for angle in [0,90,180,270] {
            let rotation=PanelRotation::from_degrees(angle).unwrap();
            let natural=if face==Face::Bottom {[20.,10.]} else {[20.,40.]};
            let [w,h]=if rotation.is_quarter() {[natural[1],natural[0]]} else {natural};
            let item=PanelItem {face,rotation:Some(rotation),x:100.,y:200.,w,h,d:40.,..Default::default()};
            let tris=place_mesh(&mesh,&item);
            for (index,vertex) in mesh.vertices.iter().enumerate() {
                assert_eq!(tris[0][index],place_point(&mesh,&item,*vertex));
            }
            let expected=if face==Face::Bottom {
                match angle {0=>[103.,204.,33.],90=>[106.,203.,33.],180=>[117.,206.,33.],_=>[104.,217.,33.]}
            } else {
                match angle {0=>[103.,207.,16.],90=>[133.,203.,16.],180=>[117.,233.,16.],_=>[107.,217.,16.]}
            };
            let actual=place_point(&mesh,&item,mesh.vertices[2]);
            assert!(actual.iter().zip(expected).all(|(a,b)|(a-b).abs()<0.001),"{face:?} {angle}: {actual:?}");
        }
    }
}

fn device(id: u64, x: f32, y: f32, w: f32, h: f32, z: f32, d: f32) -> PanelItem {
    PanelItem { id, kind: ItemKind::Device, tag: format!("-D{id}"), x, y, w, h, z, d, ..Default::default() }
}

fn duct(id: u64, x: f32, y: f32, w: f32, h: f32) -> PanelItem {
    PanelItem { id, kind: ItemKind::Duct, x, y, w, h, d: 60.0, ..Default::default() }
}

#[test]
fn devices_snap_flush_on_either_side_only_when_the_space_is_free() {
    let mut e = Enclosure::default();
    e.items.push(device(1, 40., 40., 12., 100., 0., 68.));
    let mut right = device(2, 55., 42., 12., 100., 0., 68.);
    snap_device(&e, &mut right);
    assert_eq!((right.x, right.y), (52., 40.));
    let mut left = device(3, 25., 38., 12., 100., 0., 68.);
    snap_device(&e, &mut left);
    assert_eq!((left.x, left.y), (28., 40.));
    let mut far = device(4, 58., 40., 12., 100., 0., 68.);
    snap_device(&e, &mut far);
    assert_eq!(far.x, 58.);
    e.items.push(right);
    let mut blocked = device(5, 53., 40., 12., 100., 0., 68.);
    snap_device(&e, &mut blocked);
    assert_eq!(blocked.x, 53.);
    let mut another_row = device(6, 54., 60., 12., 100., 0., 68.);
    snap_device(&e, &mut another_row);
    assert_eq!((another_row.x, another_row.y), (54., 60.));
}

#[test]
fn rail_datum_controls_preview_snap_and_old_project_recovery() {
    let mut prj = Project::new_default();
    let card = crate::parts::Part {
        number: "BEC.EL3174".into(),
        width: 12.,
        height: 100.,
        depth: 68.,
        eplan_variants: vec![
            serde_json::json!({"function_type":4,"symbol":{"pins":(1..=16).map(|n| serde_json::json!({"label":n.to_string()})).collect::<Vec<_>>()}})
                .into(),
        ],
        ..Default::default()
    };
    let db = crate::parts::PartsDb { parts: vec![card] };
    for tag in ["-A1", "-A2"] {
        crate::parts::add_to_device(&mut prj, &db, tag, "BEC.EL3174", 1);
    }
    let mut e = Enclosure::default();
    e.items.push(PanelItem { kind: ItemKind::Rail, id: 100, x: 0., y: 82.5, w: 200., h: 35., d: 7.5, ..Default::default() });
    prj.layout.enclosures.push(e);
    let ghost = device_preview(&prj, &prj.layout.enclosures[0], "-A1", p(40., 50.), Face::Plate);
    let id = place_device(&mut prj, 0, "-A1", p(40., 50.)).unwrap();
    let placed = prj.layout.enclosures[0].item(id).unwrap();
    assert_eq!((placed.x, placed.y, placed.z), (ghost.x, ghost.y, ghost.z));
    assert!((placed.y - 36.2).abs() < 0.001);
    assert_eq!(placed.z, 7.5);
    let next = place_device(&mut prj, 0, "-A2", p(54., 52.)).unwrap();
    let other = prj.layout.enclosures[0].item(next).unwrap();
    assert_eq!((other.x, other.rail_anchor().y), (52., 100.));
    let legacy = prj.layout.enclosures[0].items.iter_mut().find(|s| s.id == id).unwrap();
    legacy.y = 50.;
    legacy.rail_mount = Default::default();
    let reopened = Project::from_json(&prj.to_json()).unwrap();
    assert!((reopened.layout.enclosures[0].item(id).unwrap().y - 36.2).abs() < 0.001);
    let e = &mut prj.layout.enclosures[0];
    for clip in [7.5, 10.0] {
        let item = e.items.iter_mut().find(|i| i.id == next).unwrap();
        item.rail_mount.clip_on = clip;
        assert!(placement::arrange_on_rail(e, 100, &[next], 1.));
        let item = e.item(next).unwrap();
        assert!(item.seated_on(e.item(100).unwrap()));
        assert!((item.y - 36.2).abs() < 0.001);
        assert_eq!(item.z, 7.5 - clip);
        assert_eq!(Fastening::resolve(item, None), Fastening::Clip);
    }
}

#[test]
fn device_size_from_parts() {
    let prj = demo_project();
    // Contactor 45 x 74 x 73 plus front auxiliary block (40 deep)
    let k1 = device_size(&prj, "-K1").unwrap();
    assert_eq!((k1.w, k1.h, k1.d), (45.0, 74.0, 113.0));
    assert_eq!(k1.part, "GEN.CT-9A-24DC");
    // Terminal strip: 4 terminals of 5.2 mm, 2 end plates of 2.2 mm, 2 end brackets of 9.5 mm
    let x1 = device_size(&prj, "-X1").unwrap();
    assert!((x1.w - 44.2).abs() < 0.01, "{}", x1.w);
    // Motors have no mounting data: not placeable
    assert!(device_size(&prj, "-M1").is_none());
}

#[test]
fn devices_snap_onto_rails() {
    let prj = demo_project();
    let (_, k1) = prj.layout.find_device("-K1").unwrap();
    assert_eq!(k1.z, RAIL_PROFILE.1);
    assert!((k1.center().y - 385.0).abs() < 0.01);
    // Placed devices are no longer offered for placement.
    assert!(!devices_to_place(&prj).iter().any(|d| d.tag == "-K1"));
}

#[test]
fn collisions_and_outside() {
    let mut e = Enclosure::default();
    e.items.push(device(1, 10.0, 10.0, 40.0, 40.0, 0.0, 50.0));
    e.items.push(device(2, 30.0, 30.0, 40.0, 40.0, 0.0, 50.0)); // overlaps 1
    e.items.push(device(3, 100.0, 10.0, 40.0, 40.0, 0.0, 50.0)); // free
    // Crossing ducts are joined, not colliding.
    e.items.push(duct(4, 200.0, 0.0, 300.0, 40.0));
    e.items.push(duct(5, 300.0, 0.0, 40.0, 300.0));
    // Device beyond the plate edge
    let (pw, _) = e.plate_size();
    e.items.push(device(6, pw - 10.0, 400.0, 40.0, 40.0, 0.0, 50.0));
    assert_eq!(collisions(&e, &[]), vec![(1, 2)]);
    assert_eq!(outside(&e), vec![6]);
    // Too deep for the enclosure
    e.items.push(device(7, 100.0, 400.0, 40.0, 40.0, 0.0, 400.0));
    assert!(outside(&e).contains(&7));
}

#[test]
fn routing_follows_the_ducts() {
    let mut e = Enclosure::default();
    // Without ducts: orthogonal direct connection
    let r = route(&e, p(100.0, 100.0), p(200.0, 300.0));
    assert!(!r.via_ducts);
    assert_eq!(r.plate_length, 300.0);
    // L-shaped duct network: horizontal at y=20 (centre), vertical at x=20 (centre)
    e.items.push(duct(1, 0.0, 0.0, 400.0, 40.0));
    e.items.push(duct(2, 0.0, 40.0, 40.0, 400.0));
    let a = p(300.0, 100.0);
    let b = p(100.0, 400.0);
    let r = route(&e, a, b);
    assert!(r.via_ducts);
    // a -> (300,20): 80, along the duct to (20,20): 280, down to (20,400): 380, to b: 80
    assert!((r.plate_length - 820.0).abs() < 1.0, "{}", r.plate_length);
    assert!(r.points.first().unwrap().same(a) && r.points.last().unwrap().same(b));
    // All legs are orthogonal
    for w in r.points.windows(2) {
        assert!(w[0].x == w[1].x || w[0].y == w[1].y, "{:?}", r.points);
    }
}

#[test]
fn virtual_wiring_of_demo() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    assert!(!an.wiring.is_empty());
    for w in &an.wiring {
        // Wires between door devices stay on the door.
        assert_eq!(w.route.via_ducts, !w.on_door(), "{} - {}", w.from, w.to);
        assert!(w.length > prj.layout.wire_reserve);
    }
    // Q1 feeds the main contacts of K1 inside the cabinet.
    assert!(an.wiring.iter().any(|w| w.from.starts_with("-Q1") && w.to.starts_with("-K1")));
    // The motor is in the field: its connections stay open.
    let report = &an.report(&prj, crate::model::ReportKind::PanelWiring).unwrap();
    assert!(report.rows.iter().any(|r| r[3].starts_with("-M1") && r[8] == "device not placed"));
    // Plate ends know their connection point: above the start of the route on the plate.
    let w = an.wiring.iter().find(|w| w.door.0.is_none() && !w.route.points.is_empty()).expect("plate wire");
    let (t, q) = (w.terminals.0.expect("terminal"), w.route.points[0]);
    assert!((t[0] - PLATE_MARGIN - q.x).abs() < 0.01 && (t[1] - PLATE_MARGIN - q.y).abs() < 0.01 && t[2] > PLATE_Z, "{t:?} {q:?}");
}

#[test]
fn layout_material_in_bom() {
    let prj = demo_project();
    let material = layout_material(&prj);
    let get = |n: &str| material.iter().find(|m| m.0 == n).map(|m| m.1);
    assert_eq!(get(ENCLOSURE_PART), Some(1.0));
    // Three rails of 430 mm, sold per metre
    assert_eq!(get(RAIL_PART), Some(1.29));
    // 3380 mm of duct in 2 m pieces
    assert_eq!(get(DUCT_PART), Some(2.0));
    let an = Analysis::checked(&prj);
    let bom = &an.report(&prj, crate::model::ReportKind::BillOfMaterials).unwrap();
    let rail = bom.rows.iter().find(|r| r[3] == RAIL_PART).unwrap();
    assert_eq!((rail[1].as_str(), rail[2].as_str()), ("1.29", "m"));
}

#[test]
fn rail_standoffs_use_support_height_and_respect_hardware_setting() {
    let mut prj=demo_project();
    let plate=PanelItem {id:900,kind:ItemKind::MountingPlate,z:10.,d:2.,..Default::default()};
    let rail=PanelItem {id:901,kind:ItemKind::Rail,part:RAIL_PART.into(),w:300.,h:35.,d:7.5,z:12.,mounted_on:900,mounting_explicit:true,..Default::default()};
    prj.layout.enclosures[0].items=vec![plate,rail];
    let qty=|p:&Project|layout_material(p).iter().find(|m|m.0==crate::parts::mechanics::RAIL_STANDOFF).map(|m|m.1).unwrap_or(0.);
    assert_eq!(qty(&prj),0.);
    prj.layout.enclosures[0].items[1].z=52.;assert!(qty(&prj)>0.);
    prj.layout.hardware=false;assert_eq!(qty(&prj),0.);prj.layout.hardware=true;
    prj.layout.enclosures[0].items[1].mounted_on=0;assert_eq!(qty(&prj),0.,"unknown explicit support");
    prj.layout.enclosures[0].items[1].mounting_explicit=false;assert!(qty(&prj)>0.,"legacy datum remains valid");
    prj.layout.hardware=false;assert!(qty(&prj)>0.,"rails without an explicit support do not depend on the hardware setting");
}

#[test]
fn rotate_keeps_centre() {
    let mut it = duct(1, 0.0, 0.0, 200.0, 40.0);
    let c = it.center();
    it.rotate();
    assert!(it.center().same(c));
    assert_eq!((it.w, it.h), (40.0, 200.0));
    assert!(!it.horizontal());
}

#[test]
fn mounting_page_renders() {
    let prj = demo_project();
    let page = prj.pages.iter().position(|pg| matches!(pg.kind, crate::model::PageKind::Mounting(_))).unwrap();
    let svg = crate::svg::page_svg(&prj, page);
    assert!(svg.contains("+CAB1"));
    assert!(svg.contains("Scale 1:"));
    assert!(svg.contains("-K1"));
}

#[test]
fn wire_hit_test_and_cycling() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    // Keys identify wires uniquely (selection survives re-analysis).
    let mut keys: Vec<_> = an.wiring.iter().map(|w| w.key()).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), an.wiring.len());

    // A point in the middle of a route segment finds that wire.
    let (wi, w) = an.wiring.iter().enumerate().find(|(_, w)| w.from.starts_with("-Q1")).unwrap();
    let seg = &w.route.points[0..2];
    let mid = p((seg[0].x + seg[1].x) / 2.0, (seg[0].y + seg[1].y) / 2.0);
    assert!(route_distance(&w.route.points, mid) < 0.01);
    let hits = wires_at(&an.wiring, w.enclosure, Face::Plate, mid, 2.0);
    assert!(hits.contains(&wi));
    // Nearest first, other enclosures and far points excluded.
    assert!(wires_at(&an.wiring, w.enclosure + 1, Face::Plate, mid, 2.0).is_empty());
    assert!(wires_at(&an.wiring, w.enclosure, Face::Plate, p(-500.0, -500.0), 2.0).is_empty());

    // Repeated clicks step through overlapping wires and wrap around.
    assert_eq!(next_candidate(&[4, 7, 9], None), Some(4));
    assert_eq!(next_candidate(&[4, 7, 9], Some(4)), Some(7));
    assert_eq!(next_candidate(&[4, 7, 9], Some(9)), Some(4));
    assert_eq!(next_candidate(&[4, 7, 9], Some(1)), Some(4));
    assert_eq!(next_candidate(&[], Some(1)), None);
}

fn within(tris: &[[[f32; 3]; 3]], it: &PanelItem) -> bool {
    tris.iter().flatten().all(|v| {
        v[0] >= it.x - 0.01
            && v[0] <= it.x + it.w + 0.01
            && v[1] >= it.y - 0.01
            && v[1] <= it.y + it.h + 0.01
            && v[2] >= it.z - 0.01
            && v[2] <= it.z + it.d + 0.01
    })
}

fn extent(tris: &[[[f32; 3]; 3]]) -> [f32; 3] {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for v in tris.iter().flatten() {
        for k in 0..3 {
            min[k] = min[k].min(v[k]);
            max[k] = max[k].max(v[k]);
        }
    }
    [max[0] - min[0], max[1] - min[1], max[2] - min[2]]
}

#[test]
fn part_models_are_fitted_into_devices() {
    let prj = demo_project();
    let (ei, k1) = prj.layout.find_device("-K1").unwrap();
    let part = prj.parts.iter().find(|p| p.number == k1.part).unwrap();
    let mesh = part.model_3d.as_ref().expect("catalog contactor has a 3D model");
    // Contactor + aux block: the model (73 deep) is stretched to the device depth (113).
    let tris = place_mesh(mesh, k1);
    assert_eq!(tris.len(), mesh.triangles.len());
    assert!(within(&tris, k1));
    let e = extent(&tris);
    assert!((e[0] - k1.w).abs() < 0.01 && (e[1] - k1.h).abs() < 0.01 && (e[2] - k1.d).abs() < 0.01, "{e:?}");

    // Rotated device: the model is turned, not squeezed.
    let mut rotated = k1.clone();
    rotated.rotate();
    let tris = place_mesh(mesh, &rotated);
    assert!(within(&tris, &rotated));
    assert!((extent(&tris)[0] - rotated.w).abs() < 0.01);

    // Terminal strip: every element of the strip with its own model in its cell.
    let (_, x1) = prj.layout.find_device("-X1").unwrap();
    let cells = strip_cells(&prj, x1);
    assert_eq!(cells.len(), 8, "bracket, 3 terminals, plate, PE terminal, plate, bracket");
    let model = |n: &str| prj.parts.iter().chain(crate::parts::catalog()).find(|p| p.number == n).and_then(|p| p.model_3d.as_ref()).unwrap();
    let expected: usize = cells.iter().map(|c| model(&c.part).triangles.len()).sum();
    let tris = device_mesh(&prj, x1).unwrap();
    assert_eq!(tris.len(), expected);
    assert!(within(&tris, x1));
    // A strip of equal models without accessories is still tiled.
    let tb = model("GEN.TB-2.5");
    let mut plain = x1.clone();
    plain.w = 4.0 * 5.2;
    assert_eq!(place_mesh(tb, &plain).len(), tb.triangles.len() * 4);
    assert_eq!(ei, 0);
}

#[test]
fn model_size_is_used_without_mounting_data() {
    let mut prj = demo_project();
    // Remove the mounting data of the lamp part but give it a 3D model.
    let part = prj.parts.iter_mut().find(|p| p.number == "GEN.LMP-GN-24").unwrap();
    part.width = 0.0;
    part.height = 0.0;
    part.depth = 0.0;
    let mut mesh = crate::parts::Mesh3d::default();
    mesh.add_box([0.0, 0.0, 0.0], [22.0, 22.0, 50.0]);
    part.model_3d = Some(mesh.into());
    assert_eq!(part.size(), Some([22.0, 22.0, 50.0]));
    let size = device_size(&prj, "-P1").unwrap();
    assert_eq!((size.w, size.h, size.d), (22.0, 22.0, 50.0));
    assert_eq!(tiles(20.8, 5.2), 4);
    assert_eq!(tiles(45.0, 45.0), 1);
    assert_eq!(tiles(30.0, 7.0), 1);
}

#[test]
fn mounting_page_draws_part_models() {
    let mut prj = demo_project();
    let page = prj.pages.iter().position(|pg| matches!(pg.kind, crate::model::PageKind::Mounting(_))).unwrap();
    let with = crate::svg::page_svg(&prj, page);
    for p in &mut prj.parts {
        p.model_2d = None;
    }
    let without = crate::svg::page_svg(&prj, page);
    // Terminal screws of the generated front views are circles.
    assert!(with.matches("<circle").count() > without.matches("<circle").count() + 10);
}

#[test]
fn collisions_use_the_3d_models() {
    // Two DIN devices whose boxes overlap only beside the raised fronts: the models
    // (base 60 % deep, front in the middle third) do not touch.
    let ct = crate::parts::catalog().iter().find(|p| p.number == "GEN.MPS-4-6.3A").unwrap().clone();
    let parts = vec![ct.clone()];
    let (w, h, d) = (ct.width, ct.height, ct.depth);
    let dev = |id, x: f32, y: f32, z: f32| PanelItem { id, kind: ItemKind::Device, part: ct.number.clone(), x, y, w, h, d, z, ..Default::default() };
    // B sits above A's base and terminal screws (on a higher rail), beside A's raised front.
    let a = dev(1, 100.0, 100.0, 0.0);
    let b = dev(2, 100.0, 100.0 + h * 0.75, d * 0.6 + 2.0);
    let e = Enclosure { items: vec![a.clone(), b.clone()], ..Default::default() };
    // Boxes overlap ...
    assert_eq!(collisions(&e, &[]), vec![(1, 2)]);
    // ... the models do not.
    assert!(collisions(&e, &parts).is_empty());
    // Moved onto A's front, the models collide.
    let b = dev(2, 100.0, 100.0 + h * 0.4, d * 0.6 + 2.0);
    let e = Enclosure { items: vec![a, b], ..Default::default() };
    assert_eq!(collisions(&e, &parts), vec![(1, 2)]);
}

#[test]
fn door_devices() {
    let prj = demo_project();
    let e = &prj.layout.enclosures[0];
    let s1 = e.items.iter().find(|i| i.tag == "-S1").unwrap();
    assert_eq!(s1.face, Face::Door);
    // Door devices sit behind the door, their front at the enclosure front.
    let (min, max) = item_box(e, s1);
    assert_eq!(max[2], e.depth);
    assert_eq!(min[2], e.depth - s1.d);
    assert!(collisions(e, &prj.parts).is_empty());
    assert!(outside(e).is_empty());
    // Too deep: reaches the devices of the mounting plate / the plate.
    let mut e = e.clone();
    let i = e.items.iter().position(|i| i.tag == "-S1").unwrap();
    e.items[i].d = 240.0;
    assert!(outside(&e).contains(&e.items[i].id));
    // A door device placed over a plate device in front view collides in 3D when deep enough.
    let k1 = e.items.iter().find(|i| i.tag == "-K1").unwrap().clone();
    e.items[i].x = k1.x + PLATE_MARGIN;
    e.items[i].y = k1.y + PLATE_MARGIN;
    e.items[i].d = e.depth - PLATE_Z - k1.z - k1.d + 10.0;
    assert!(collisions(&e, &prj.parts).contains(&(k1.id.min(e.items[i].id), k1.id.max(e.items[i].id))));
    // Outside the door frame.
    e.items[i].d = 45.0;
    e.items[i].x = 5.0;
    assert!(outside(&e).contains(&e.items[i].id));
}

#[test]
fn door_wires_run_over_the_hinge() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let e = &prj.layout.enclosures[0];
    let s2 = e.items.iter().find(|i| i.tag == "-S2").unwrap();
    let s3 = e.items.iter().find(|i| i.tag == "-S3").unwrap();
    let w = an.wiring.iter().find(|w| (w.items.0 == s3.id) != (w.items.1 == s3.id) && !w.on_door()).expect("door device is wired into the cabinet");
    let (start, extra, door_route) = wiring_end(e, s3, None);
    assert_eq!(start.pos.x, 0.0);
    assert!(extra > DOOR_LOOP + e.depth - PLATE_Z);
    assert!(w.length > extra);
    // On the door the wire runs through the door ducts to the hinge side.
    let door_route = door_route.expect("route on the door");
    assert!(door_route.via_ducts);
    assert!(door_route.points.last().unwrap().x < DOOR_FRAME);
    let at = if w.items.0 == s3.id { w.door.0 } else { w.door.1 }.expect("door end");
    assert!(at[2] < e.depth && at[2] > e.depth - s3.d - 1.0);
    let route = if w.items.0 == s3.id { &w.door_routes.0 } else { &w.door_routes.1 }.clone().expect("door route");
    let door_ducts: Vec<u64> = e.items.iter().filter(|i| i.kind == ItemKind::Duct && i.face == Face::Door).map(|i| i.id).collect();
    assert!(route.runs.iter().all(|r| door_ducts.contains(&r.duct)) && !route.runs.is_empty());
    // The 3D path: back into the door duct, along the door, over the hinge to the plate.
    let plate = [PLATE_MARGIN, PLATE_MARGIN + 100.0, PLATE_Z + 30.0];
    let (path, on_door) = door_path(e, at, &route, plate);
    assert_eq!((path[0], *path.last().unwrap(), on_door), (at, plate, route.points.len() + 1));
    let duct_z = e.depth - 15.0;
    assert!(path[1..on_door].iter().all(|q| (q[2] - duct_z).abs() < 0.01), "{path:?}");
    // The door ducts get fill levels; the 2D door view finds the wire on the door.
    assert!(an.duct_fill.iter().any(|f| door_ducts.contains(&f.duct) && f.total_wires > 0));
    let mid = route.points[route.points.len() / 2];
    assert!(wires_at(&an.wiring, 0, Face::Door, mid, 2.0).iter().any(|&i| an.wiring[i].key() == w.key()));
    // Between two door devices the wire stays on the door: short, no plate route.
    let jumper = an.wiring.iter().find(|w| w.items.0 == s2.id || w.items.1 == s2.id).unwrap();
    assert!(jumper.on_door() && jumper.route.points.is_empty() && jumper.route.runs.is_empty());
    assert!(!jumper.paths_in(0, Face::Door).is_empty() && jumper.paths_in(0, Face::Plate).is_empty());
    assert!(jumper.length < w.length);
    // The mounting layout page shows the door next to the plate.
    let page = prj.pages.iter().position(|p| matches!(p.kind, crate::model::PageKind::Mounting(_))).unwrap();
    assert!(crate::svg::page_svg(&prj, page).contains("Door (front view)"));
}

#[test]
fn wires_enter_the_duct_their_terminal_faces() {
    let mut e = Enclosure::default();
    // Ducts above (y 100) and below (y 300) a terminal at y 250, joined on the left.
    e.items.push(duct(1, 0.0, 80.0, 400.0, 40.0));
    e.items.push(duct(2, 0.0, 280.0, 400.0, 40.0));
    e.items.push(duct(3, 0.0, 120.0, 40.0, 160.0));
    let target = WireEnd::at(p(300.0, 300.0));
    // Without a direction the nearer duct below is used.
    let r = route_ends(&e, WireEnd::at(p(200.0, 250.0)), target);
    assert_eq!(r.runs.iter().map(|run| run.duct).collect::<Vec<_>>(), [2]);
    // A terminal at the top goes up into the upper duct, even though it is farther.
    let up = WireEnd { pos: p(200.0, 250.0), exit: Some(Exit::Up) };
    let r = route_ends(&e, up, target);
    assert_eq!(r.runs.first().unwrap().duct, 1);
    assert!(r.points[1].y < 250.0, "{:?}", r.points);
    // Nothing in that direction: the nearest duct after all.
    let right = WireEnd { pos: p(450.0, 250.0), exit: Some(Exit::Right) };
    assert!(route_ends(&e, right, target).via_ducts);
}

#[test]
fn routes_record_their_duct_runs() {
    let mut e = Enclosure::default();
    e.items.push(duct(1, 0.0, 0.0, 400.0, 40.0));
    e.items.push(duct(2, 0.0, 40.0, 40.0, 400.0));
    let r = route(&e, p(300.0, 100.0), p(100.0, 400.0));
    assert_eq!(r.runs.len(), 2, "{:?}", r.runs);
    let (h, v) = (r.runs[0], r.runs[1]);
    assert_eq!((h.duct, h.from, h.to), (1, 20.0, 300.0));
    assert_eq!((v.duct, v.to), (2, 400.0));
    assert!(v.from <= 40.0);
    assert!(route(&Enclosure::default(), p(0.0, 0.0), p(10.0, 10.0)).runs.is_empty());
}

#[test]
fn demo_wires_start_at_the_connection_points() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let centre: Vec<String> = an.wiring.iter().filter(|w| w.ends.0 == EndKind::Centre || w.ends.1 == EndKind::Centre).map(|w| format!("{} - {}", w.from, w.to)).collect();
    assert!(centre.is_empty(), "wires from device centres: {centre:?}");
    // -K1:A1 is at the top edge of the contactor and leaves upwards.
    let k1 = prj.layout.find_device("-K1").unwrap().1;
    let a1 = device_points(&prj, k1).into_iter().find(|p| p.pin == "A1").unwrap();
    let w = an.wiring.iter().find(|w| w.from == "-K1:A1" || w.to == "-K1:A1").unwrap();
    let start = if w.from == "-K1:A1" { w.route.points[0] } else { *w.route.points.last().unwrap() };
    assert!(start.same(p(a1.pos[0], a1.pos[1])));
    let next = if w.from == "-K1:A1" { w.route.points[1] } else { w.route.points[w.route.points.len() - 2] };
    assert!(next.y < start.y, "leaves upwards: {:?}", w.route.points);
}

#[test]
fn own_connection_points_make_exact_ends() {
    let mut prj = demo_project();
    let before = Analysis::checked(&prj).wiring;
    let part = prj.parts.iter_mut().find(|p| p.number == "GEN.MCB-1P-C6").unwrap();
    part.connection_points = vec![
        crate::parts::ConnectionPoint { pin: "1".into(), pos: [9.0, 0.0, 10.0], exit: Exit::Up },
        crate::parts::ConnectionPoint { pin: "2".into(), pos: [9.0, 90.0, 10.0], exit: Exit::Down },
    ];
    let after = Analysis::checked(&prj).wiring;
    let f1 = |w: &&RoutedWire| w.from.starts_with("-F1:") || w.to.starts_with("-F1:");
    let w = after.iter().find(f1).unwrap();
    let kind = if w.from.starts_with("-F1:") { w.ends.0 } else { w.ends.1 };
    assert_eq!(kind, EndKind::Exact);
    // The terminals sit low (10 mm instead of 52.5 mm above the rail): shorter wires.
    let old = before.iter().find(|o| o.key() == w.key()).unwrap();
    assert!(w.length < old.length, "{} < {}", w.length, old.length);
}

#[test]
fn weighted_routes_avoid_expensive_ducts() {
    // A ring of ducts: top (1), bottom (2), left (3), right (4).
    let mut e = Enclosure::default();
    e.items.push(duct(1, 0.0, 0.0, 400.0, 40.0));
    e.items.push(duct(2, 0.0, 360.0, 400.0, 40.0));
    e.items.push(duct(3, 0.0, 40.0, 40.0, 320.0));
    e.items.push(duct(4, 360.0, 40.0, 40.0, 320.0));
    let (a, b) = (WireEnd::at(p(100.0, 150.0)), WireEnd::at(p(300.0, 150.0)));
    let short = route_ends(&e, a, b);
    assert!(short.runs.iter().any(|r| r.duct == 1));
    assert!((short.plate_length - 780.0).abs() < 1.0, "{}", short.plate_length);
    // The top duct counts a hundred times: the way round the bottom is cheaper.
    let around = route_weighted(&e, a, b, &|duct, _, _| if duct == 1 { 100.0 } else { 1.0 });
    assert!(around.runs.iter().all(|r| r.duct != 1), "{:?}", around.runs);
    assert!(around.runs.iter().any(|r| r.duct == 2));
    assert!((around.plate_length - 980.0).abs() < 1.0, "real length: {}", around.plate_length);
    // Factors below 1 do not make ducts cheaper than their length.
    assert_eq!(route_weighted(&e, a, b, &|_, _, _| 0.1).plate_length, short.plate_length);
}


#[test]
fn wall_faces_in_enclosure_coordinates() {
    let e = Enclosure::default();
    let t = e.wall();
    let on = |face: Face| PanelItem { id: 1, kind: ItemKind::Accessory, x: 10.0, y: 20.0, w: 100.0, h: 50.0, z: 0.0, d: 30.0, face, ..Default::default() };
    // Left side seen from the left: face x runs from the rear to the front.
    assert_eq!(item_box(&e, &on(Face::Left)), ([t, 20.0, 10.0], [t + 30.0, 70.0, 110.0]));
    // Right side: face x runs from the front to the rear.
    assert_eq!(item_box(&e, &on(Face::Right)), ([e.width - t - 30.0, 20.0, e.depth - 110.0], [e.width - t, 70.0, e.depth - 10.0]));
    // Rear wall seen from behind: mirrored.
    assert_eq!(item_box(&e, &on(Face::Rear)), ([e.width - 110.0, 20.0, t], [e.width - 10.0, 70.0, t + 30.0]));
    // Roof from above: face y runs from the rear to the front.
    assert_eq!(item_box(&e, &on(Face::Roof)), ([10.0, t, 20.0], [110.0, t + 30.0, 70.0]));
    assert_eq!(item_box(&e, &on(Face::Bottom)), ([10.0, e.height - t - 30.0, e.depth - 70.0], [110.0, e.height - t, e.depth - 20.0]));
    // The plate and the door keep their coordinates.
    assert_eq!(item_box(&e, &on(Face::Plate)).0, [PLATE_MARGIN + 10.0, PLATE_MARGIN + 20.0, PLATE_Z]);
    assert_eq!(item_box(&e, &on(Face::Door)).1, [110.0, 70.0, e.depth]);
    for f in Face::ALL {
        assert_eq!(Face::from_key(f.key()), Some(f));
    }
}

#[test]
fn models_on_walls_keep_their_outside() {
    let prj = demo_project();
    let part = prj.parts.iter().find(|p| p.number == "GEN.CT-9A-24DC").unwrap();
    let [w, h, d] = part.size().unwrap();
    let e = Enclosure::default();
    // Signed volume: its sign is the orientation of the surface.
    let volume = |face: Face| {
        let it = PanelItem { id: 1, kind: ItemKind::Device, part: part.number.clone(), x: 50.0, y: 50.0, w, h, d, face, ..Default::default() };
        let tris = place_triangles(&e, &it, place_mesh(part.model_3d.as_ref().unwrap(), &it));
        tris.iter().map(|t| crate::parts::vec3::dot(t[0], crate::parts::vec3::cross(t[1], t[2])) / 6.0).sum::<f32>()
    };
    let plate = volume(Face::Plate);
    assert!((plate.abs() - w * h * d).abs() < 0.5 * w * h * d, "{plate}");
    for face in Face::ALL {
        let v = volume(face);
        assert!(v.signum() == plate.signum() && (v.abs() - plate.abs()).abs() < 0.01 * plate.abs(), "{face:?}: {v} / {plate}");
    }
}

#[test]
fn models_on_the_floor_of_a_room_stand_upright() {
    let mut prj = Project::default();
    let room = new_room(&mut prj, "+R", 4000.0, 3000.0, 2800.0);
    let mut part = crate::parts::catalog().iter().find(|p| p.number == "GEN.TANK-1000L").unwrap().clone();
    part.number = "T".into();
    let [w, h, d] = part.size().unwrap();
    // Standing on the floor its height goes up (stand_on_floor swaps height and depth).
    let mut it = PanelItem { id: 1, kind: ItemKind::Device, part: part.number.clone(), x: 1000.0, y: 1000.0, w, h, d, face: Face::Bottom, ..Default::default() };
    stand_on_floor(&room, &mut it);
    let tris = place_triangles(&room, &it, place_mesh(part.model_3d.as_ref().unwrap(), &it));
    let ys: Vec<f32> = tris.iter().flatten().map(|v| v[1]).collect();
    let (top, bottom) = (ys.iter().copied().fold(f32::INFINITY, f32::min), ys.iter().copied().fold(f32::NEG_INFINITY, f32::max));
    assert!((bottom - room.height).abs() < 1.0 && (bottom - top - h).abs() < 1.0, "from the floor up {h} mm: {top} .. {bottom}");
    // The vessel's top nozzle is above the middle of its footprint.
    let nozzle = tris.iter().flatten().min_by(|a, b| a[1].total_cmp(&b[1])).unwrap();
    assert!((nozzle[0] - (1000.0 + w / 2.0)).abs() < 0.2 * w, "{nozzle:?}");
    let volume: f32 = tris.iter().map(|t| crate::parts::vec3::dot(t[0], crate::parts::vec3::cross(t[1], t[2])) / 6.0).sum();
    let plate = PanelItem { face: Face::Plate, w, h, d, ..it.clone() };
    let flat: f32 = place_triangles(&Enclosure::default(), &plate, place_mesh(part.model_3d.as_ref().unwrap(), &plate)).iter().map(|t| crate::parts::vec3::dot(t[0], crate::parts::vec3::cross(t[1], t[2])) / 6.0).sum();
    assert_eq!(volume.signum(), flat.signum(), "outside kept");
}

#[test]
fn wall_items_inside_the_flanges() {
    let mut e = Enclosure::default();
    let f = e.flange();
    let fan = |id: u64, face: Face, x: f32| PanelItem { id, kind: ItemKind::Accessory, x, y: 100.0, w: 150.0, h: 150.0, z: -20.0, d: 100.0, face, ..Default::default() };
    e.items = vec![fan(1, Face::Left, f), fan(2, Face::Left, 0.0), fan(3, Face::Right, f)];
    assert_eq!(outside(&e), vec![2]);
    // Bayed sides have no wall to mount on.
    e.build.right_open = true;
    assert_eq!(outside(&e), vec![2, 3]);
    // Behind the mounting plate there is hardly any room on the rear wall.
    e.items = vec![fan(4, Face::Rear, f)];
    assert_eq!(outside(&e), vec![4]);
}

#[test]
fn accessories_from_parts() {
    let mut prj = demo_project();
    let id = place_accessory(&mut prj, 0, "GEN.GLAND-M25", Face::Bottom, p(100.0, 60.0)).unwrap();
    let e = &prj.layout.enclosures[0];
    let g = e.item(id).unwrap();
    assert_eq!((g.w, g.h, g.z), (36.0, 36.0, -24.0));
    let gland = crate::parts::catalog().iter().find(|p| p.number == "GEN.GLAND-M25").unwrap();
    assert_eq!(allowed_faces(gland), vec![Face::Bottom, Face::Roof, Face::Left, Face::Right, Face::Rear]);
    // Its cutout is in the NC data of the bottom, it is clamped (no holes) and in the BOM.
    let o = crate::production::openings(e, &prj.parts, Face::Bottom);
    assert!(o.iter().any(|o| o.item == id && o.round && (o.w - 25.5).abs() < 0.01));
    assert!(crate::production::holes_on(e, &prj.parts, Face::Bottom).is_empty());
    assert!(layout_material(&prj).iter().any(|m| m.0 == "GEN.GLAND-M25" && m.1 == 1.0));
}

#[test]
fn profiles_offer_generic_and_store_parts() {
    let store = crate::parts::Part { number: "ACME.DUCT-50X75".into(), subgroup: "Cable duct".into(), height: 50.0, depth: 75.0, ..Default::default() };
    let rail = crate::parts::Part { number: "ACME.RAIL".into(), subgroup: "Mounting rail".into(), ..Default::default() };
    let parts = [store, rail, crate::parts::catalog().iter().find(|p| p.number == DUCT_PART).unwrap().clone()];
    let ducts = profile_choices(ItemKind::Duct, &parts);
    assert!(ducts.iter().any(|c| c.number == DUCT_PART && c.generic) && ducts.iter().any(|c| c.number == DOOR_DUCT_PART));
    assert_eq!(ducts.iter().filter(|c| c.number == DUCT_PART).count(), 1, "each part once");
    assert_eq!(ducts.last().map(|c| (c.number.as_str(), c.generic)), Some(("ACME.DUCT-50X75", false)), "store parts after the generic ones");
    assert!(!ducts.iter().any(|c| c.number == "ACME.RAIL"));
    for kind in [ItemKind::Rail, ItemKind::Busbar] {
        assert!(profile_choices(kind, &[]).iter().filter(|c| c.generic).count() > 1, "several generic types of {kind:?}");
    }
    assert!(profile_choices(ItemKind::Device, &parts).is_empty());
}

#[test]
fn changing_the_part_of_a_profile_keeps_its_centre_line() {
    let mut prj = Project::default();
    prj.parts.push(crate::parts::Part { number: "BIG".into(), height: 80.0, depth: 80.0, ..Default::default() });
    prj.parts.push(crate::parts::Part { number: "HIGH-RAIL".into(), height: 35.0, depth: 15.0, ..Default::default() });
    let mut e = Enclosure::default();
    e.items.push(duct(1, 100.0, 80.0, 400.0, 40.0));
    e.items.push(duct(2, 50.0, 100.0, 40.0, 300.0));
    e.items.push(PanelItem { id: 3, kind: ItemKind::Rail, x: 100.0, y: 182.5, w: 400.0, h: 35.0, d: 7.5, ..Default::default() });
    e.items.push(device(4, 200.0, 163.0, 45.0, 74.0, 7.5, 80.0));
    prj.layout.enclosures.push(e);
    assert!(change_profile(&mut prj, 0, 1, "BIG") && change_profile(&mut prj, 0, 2, "BIG"));
    let e = &prj.layout.enclosures[0];
    let h = e.item(1).unwrap();
    assert_eq!((h.x, h.y, h.w, h.h, h.d, h.part.as_str()), (100.0, 60.0, 400.0, 80.0, 80.0, "BIG"));
    let v = e.item(2).unwrap();
    assert_eq!((v.x, v.y, v.w, v.h), (30.0, 100.0, 80.0, 300.0));
    assert!(change_profile(&mut prj, 0, 3, "HIGH-RAIL"));
    assert_eq!(prj.layout.enclosures[0].item(4).unwrap().z, 15.0, "the device sits on the higher rail");
    assert!(!change_profile(&mut prj, 0, 4, "BIG"), "devices are no profiles");
}

#[test]
fn free_names_skip_rooms_and_gaps() {
    let mut layout = Layout::default();
    assert_eq!(free_name(&layout, "+CAB"), "+CAB1");
    for name in ["+CAB1", "+ROOM1", "+CAB3"] {
        layout.enclosures.push(Enclosure { name: name.into(), ..Default::default() });
    }
    assert_eq!(free_name(&layout, "+CAB"), "+CAB2");
    assert_eq!(free_name(&layout, "+ROOM"), "+ROOM2");
}

#[test]
fn placed_models_follow_the_frame_of_their_face() {
    // The frame of a face is made once per item; every point lands as with `to_enclosure`.
    let prj = crate::sample::example("building_services").unwrap();
    let mut faces = std::collections::BTreeSet::new();
    for e in &prj.layout.enclosures {
        for it in &e.items {
            let Some(tris) = device_mesh(&prj, it) else { continue };
            let flip = it.face != Face::Door && mirrors(&face_frame(e, it.face));
            let each: Vec<Tri> = tris.iter().map(|t| t.map(|v| to_enclosure(e, it, v))).map(|t| if flip { [t[0], t[2], t[1]] } else { t }).collect();
            assert_eq!(place_triangles(e, it, tris), each, "{} {}", e.name, it.tag);
            faces.insert((e.is_room(), format!("{:?}", it.face)));
        }
    }
    // On the plate and the door of cabinets, on the floor and the walls of rooms.
    assert!(faces.contains(&(false, "Plate".into())) && faces.contains(&(true, "Bottom".into())), "{faces:?}");
}

#[test]
fn cable_ducts_have_slotted_walls_and_a_cover() {
    let prj = demo_project();
    let e = &prj.layout.enclosures[0];
    let mut checked = 0;
    for it in e.items.iter().filter(|i| i.kind == ItemKind::Duct) {
        let (lo, hi) = item_box(e, it);
        let boxes = duct_boxes(e, it, true);
        assert!(boxes.iter().all(|((a, b), _, _)| (0..3).all(|k| a[k] >= lo[k] - 0.01 && b[k] <= hi[k] + 0.01)), "{it:?}");
        // The cover closes the duct at the front (plate ducts face the door, door ducts
        // the inside of the enclosure).
        let (wall, cover) = DUCT_COLORS;
        let front = if it.face == Face::Door { lo[2] } else { hi[2] };
        assert!(boxes.iter().any(|((a, b), c, _)| *c == cover && (a[2] - front).abs().min((b[2] - front).abs()) < 0.01), "{it:?}");
        // Fingers with slots between them on both walls.
        let (len, along) = if it.w >= it.h { (it.w, 0) } else { (it.h, 1) };
        let fingers: Vec<_> = boxes.iter().filter(|(_, _, finger)| *finger).collect();
        assert!(fingers.iter().all(|((a, b), c, _)| *c == wall && b[along] - a[along] < len / 2.0));
        assert!(fingers.len() >= 2 * (len / 15.0) as usize, "{it:?}: {} fingers", fingers.len());
        let mut starts: Vec<f32> = fingers.iter().map(|((a, _), _, _)| a[along]).collect();
        starts.sort_by(f32::total_cmp);
        starts.dedup();
        let width = fingers[0].0 .1[along] - fingers[0].0 .0[along];
        assert!(starts.windows(2).all(|s| s[1] - s[0] > width + 1.0), "slots between the fingers");
        // Opened, the same without the cover.
        let open = duct_boxes(e, it, false);
        assert!(open.len() == boxes.len() - 1 && !open.iter().any(|(_, c, _)| *c == cover), "{it:?}");
        checked += 1;
    }
    assert!(checked > 2);
}

#[test]
fn devices_on_a_busbar_are_connected_by_its_bars() {
    // Placed over the bars, a device is mounted onto them.
    let mut e = Enclosure::default();
    e.items.push(PanelItem { id: 1, kind: ItemKind::Busbar, x: 0.0, y: 100.0, w: 600.0, h: 90.0, d: 40.0, ..Default::default() });
    let mut q = device(2, 100.0, 80.0, 50.0, 200.0, 0.0, 100.0);
    snap_to_rail(&e, &mut q);
    assert_eq!(q.z, 40.0);
    assert_eq!(busbar_of(&e, &q), Some(1));
    // Below the bars it is not.
    let mut x = device(3, 100.0, 300.0, 50.0, 50.0, 0.0, 100.0);
    snap_to_rail(&e, &mut x);
    assert_eq!((x.z, busbar_of(&e, &x)), (0.0, None));

    // The main distribution of the building: the incoming terminals and the fuse
    // switch disconnectors on the busbar are not wired to each other (the bars connect
    // them), the meters are wired to the disconnectors.
    let prj = crate::sample::example("building_services").unwrap();
    let (md, _) = prj.layout.find_device("=MD+MD-Q1").unwrap();
    let on_bars: Vec<&str> = ["=MD+MD-X0", "=MD+MD-Q1", "=MD+MD-Q2", "=MD+MD-Q3"].into();
    for tag in &on_bars {
        let (_, it) = prj.layout.find_device(tag).unwrap();
        assert!(busbar_of(&prj.layout.enclosures[md], it).is_some(), "{tag}");
    }
    let (nl, open) = (crate::netlist::Netlist::build(&prj, true), crate::netlist::Netlist::build(&prj, false));
    let conns = crate::netlist::connections(&prj, &nl, &open);
    let (wiring, _) = virtual_wiring(&prj, &conns);
    let device = |end: &str| end.rsplit_once(':').map_or(end, |(d, _)| d).to_string();
    assert!(!wiring.iter().any(|w| on_bars.contains(&device(&w.from).as_str()) && on_bars.contains(&device(&w.to).as_str())));
    let meters = wiring.iter().filter(|w| [device(&w.from), device(&w.to)] == ["=MD+MD-Q1", "=MD+MD-P1"] || [device(&w.from), device(&w.to)] == ["=MD+MD-P1", "=MD+MD-Q1"]).count();
    assert_eq!(meters, 3);
}

#[test]
fn wires_without_ducts_run_round_the_devices() {
    // Two devices side by side on a rail and a third beside them, no ducts.
    let mut e = Enclosure::default();
    let device = |id: u64, x: f32| PanelItem { id, kind: ItemKind::Device, face: Face::Plate, x, y: 100.0, w: 50.0, h: 80.0, d: 70.0, ..Default::default() };
    e.items.extend([device(1, 100.0), device(2, 150.0), device(3, 200.0)]);
    let inside = |q: P| e.items.iter().any(|i| q.x > i.x + 0.5 && q.x < i.x + i.w - 0.5 && q.y > i.y + 0.5 && q.y < i.y + i.h - 0.5);
    let through = |r: &Route| r.points.windows(2).any(|s| (0..=20).any(|k| inside(s[0] + (s[1] - s[0]) * (k as f32 / 20.0))));
    // From the top of the first to the top of the third: above all of them.
    let top = |x: f32| WireEnd { pos: p(x, 100.0), exit: Some(Exit::Up) };
    let bottom = |x: f32| WireEnd { pos: p(x, 180.0), exit: Some(Exit::Down) };
    let r = route_ends(&e, top(120.0), top(220.0));
    assert!(!r.via_ducts && !through(&r), "{:?}", r.points);
    assert!(r.points.iter().all(|q| q.y <= 100.0), "{:?}", r.points);
    // From the bottom of the first to the top of the second: round the row.
    let r = route_ends(&e, bottom(120.0), top(170.0));
    assert!(!through(&r), "{:?}", r.points);
    assert!((r.plate_length - r.points.windows(2).map(|s| (s[1].x - s[0].x).abs() + (s[1].y - s[0].y).abs()).sum::<f32>()).abs() < 0.01);
    // Without directions as before: straight across.
    assert_eq!(route_ends(&e, WireEnd::at(p(10.0, 10.0)), WireEnd::at(p(50.0, 40.0))).points, vec![p(10.0, 10.0), p(50.0, 10.0), p(50.0, 40.0)]);
}

#[test]
fn busbar_systems_of_bayed_enclosures_are_joined_through_their_side() {
    // Two enclosures 600 wide, in each a busbar system that runs up to their common
    // side, a device on each.
    let mut prj = Project::default();
    for (name, x) in [("+A", 300.0), ("+B", 0.0)] {
        let mut e = new_enclosure(&mut prj, "", name);
        e.items.push(PanelItem { id: prj.new_id(), kind: ItemKind::Busbar, x, y: 100.0, w: 250.0, h: 90.0, d: 40.0, ..Default::default() });
        let mut q = device(prj.new_id(), x + 50.0, 80.0, 50.0, 200.0, 0.0, 100.0);
        snap_to_rail(&e, &mut q);
        e.items.push(q);
        prj.layout.enclosures.push(e);
    }
    let system = |prj: &Project, ei: usize| {
        let pos = positions(&prj.layout);
        let e = &prj.layout.enclosures[ei];
        busbar_system(&prj.layout, &pos, ei, e.items.last().unwrap())
    };
    // Standing apart, each system is its own.
    assert!(system(&prj, 0).is_some() && system(&prj, 0) != system(&prj, 1));
    // Bayed, the bars run through: one system, named after its beginning on the left.
    bay_right_of(&mut prj, 1, 0);
    assert_eq!(system(&prj, 0), system(&prj, 1));
    assert_eq!(system(&prj, 1).map(|s| s.0), Some(0));
    // Not at the same height, or ending far from the side: not joined.
    prj.layout.enclosures[1].items[0].y += 50.0;
    assert_ne!(system(&prj, 0), system(&prj, 1));
    prj.layout.enclosures[1].items[0].y -= 50.0;
    prj.layout.enclosures[0].items[0].w = 100.0;
    assert_ne!(system(&prj, 0), system(&prj, 1));
}

#[test]
fn a_short_section_of_a_wide_busbar_system_lies_along_its_bars() {
    let prj = crate::sample::example("industrial_main_distribution").unwrap();
    // The section the coupling switches are fed from: 215 mm of a system 400 mm wide.
    let (ei, x30) = locate(&prj, "=NA+HV1-X30").unwrap();
    let e = &prj.layout.enclosures[ei];
    let bar = e.items.iter().find(|b| b.id == busbar_of(e, x30).unwrap()).unwrap();
    assert!(bar.w < bar.h && bars_along_x(&prj.parts, bar));
    // The terminals stand upright on its bars, one on each.
    assert!(x30.h > x30.w);
    let places = busbars::terminals_on_bars(&prj, x30).expect("across the bars");
    assert_eq!(places.len(), 3);
    assert!(places.windows(2).all(|w| w[1].0.y > w[0].0.y + 100.0 && w[1].0.x == w[0].0.x));
    // Without the part's data the longer side tells.
    let plain = PanelItem { kind: ItemKind::Busbar, w: 100.0, h: 300.0, ..Default::default() };
    assert!(!bars_along_x(&prj.parts, &plain));
}
