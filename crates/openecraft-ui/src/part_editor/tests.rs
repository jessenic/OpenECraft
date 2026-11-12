use super::*;

#[test]
fn models_are_assigned_to_the_part() {
    let mut part = Part { number: "NEW.PART".into(), ..Default::default() };
    let svg = r#"<svg width="45mm" height="74mm" viewBox="0 0 45 74"><rect x="0" y="0" width="45" height="74"/></svg>"#;
    let msg = apply_model(&mut part, ModelKind::Front2d, "C:\\drawings\\front.svg", svg.as_bytes()).unwrap();
    assert!(msg.starts_with("front.svg"), "{msg}");
    assert_eq!(part.model_2d.as_ref().unwrap().prims.len(), 1);

    // Without mounting data the part takes the model size.
    let obj = "v 0 0 0\nv 30 0 0\nv 30 20 0\nv 0 20 10\nf 1 2 3\nf 1 3 4\n";
    let msg = apply_model(&mut part, ModelKind::Mesh3d, "/models/part.obj", obj.as_bytes()).unwrap();
    assert!(msg.contains("part.obj") && msg.contains("NEW.PART"), "{msg}");
    assert_eq!((part.width, part.height, part.depth), (30.0, 20.0, 10.0));
    assert_eq!(part.model_3d.as_ref().unwrap().triangles.len(), 2);

    // Existing mounting data is kept.
    part.width = 45.0;
    apply_model(&mut part, ModelKind::Mesh3d, "part.obj", obj.as_bytes()).unwrap();
    assert_eq!(part.width, 45.0);

    // Broken files are reported, the part keeps its model.
    assert!(apply_model(&mut part, ModelKind::Mesh3d, "bad.stl", b"garbage").is_err());
    assert!(part.model_3d.is_some());
}

#[test]
fn model_file_filters() {
    assert_eq!(ModelKind::Front2d.filter().1, &["svg"]);
    assert_eq!(ModelKind::Mesh3d.filter().1, &["stl", "obj", "step", "stp", "igs", "iges", "x_t", "x_b", "xmt_txt", "xmt_bin", "ifc", "jt"]);
}

#[test]
fn files_go_to_the_reader_of_their_format() {
    let mut part = Part::default();
    for (name, format) in [("Model.STEP", "STEP"), ("model.stp", "STEP"), ("model.IGS", "IGES"), ("model.iges", "IGES"), ("model.x_t", "Parasolid"), ("model.X_B", "Parasolid"), ("model.ifc", "IFC"), ("model.jt", "JT")] {
        let err = apply_model(&mut part, ModelKind::Mesh3d, name, b"garbage").unwrap_err();
        assert!(err.contains(format), "{name}: {err}");
    }
    assert!(part.model_3d.is_none());
}

#[test]
fn jt_and_ifc_models_are_assigned() {
    let tris = [[[0.0, 0.0, 0.0], [12.0, 0.0, 0.0], [0.0, 8.0, 3.0]]];
    let jt = openecraft_jt::write_jt("part", &[openecraft_jt::Group { name: "g", bodies: vec![openecraft_jt::Body { name: "b", color: [0; 3], transparent: false, triangles: &tris }] }]);
    let mut part = Part::default();
    apply_model(&mut part, ModelKind::Mesh3d, "part.jt", &jt).unwrap();
    assert_eq!((part.width, part.height, part.depth), (12.0, 8.0, 3.0));
    let ifc = "ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n#1=IFCCARTESIANPOINT((0.,0.,0.));\n#2=IFCAXIS2PLACEMENT3D(#1,$,$);\n#3=IFCBLOCK(#2,5.,6.,7.);\n#4=IFCSHAPEREPRESENTATION($,'Body','CSG',(#3));\n#5=IFCPRODUCTDEFINITIONSHAPE($,$,(#4));\n#6=IFCBUILDINGELEMENTPROXY('x',$,$,$,$,$,#5,$,$);\nENDSEC;\nEND-ISO-10303-21;\n";
    apply_model(&mut part, ModelKind::Mesh3d, "part.ifc", ifc.as_bytes()).unwrap();
    assert_eq!(part.model_3d.as_ref().unwrap().size(), [5.0, 6.0, 7.0]);
}

#[test]
fn the_3d_preview_finds_the_edges_and_splits_large_faces() {
    let mut mesh = Mesh3d::default();
    mesh.add_box([0.0; 3], [100.0, 50.0, 20.0]);
    let normals: Vec<[f32; 3]> = mesh.triangles.iter().map(|t| {
        let v = t.map(|i| mesh.vertices[i as usize]);
        norm(cross(sub(v[1], v[0]), sub(v[2], v[0])))
    }).collect();
    // The 12 edges of the box, not the diagonals of its faces.
    assert_eq!(sharp_edges(&mesh, &normals).len(), 12);
    let mut pieces = Vec::new();
    split_triangle([[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 50.0, 0.0]], 30.0, &mut pieces);
    let area = |v: &[[f32; 3]; 3]| {
        let c = cross(sub(v[1], v[0]), sub(v[2], v[0]));
        dot(c, c).sqrt() / 2.0
    };
    assert!(pieces.len() > 4);
    assert!((pieces.iter().map(area).sum::<f32>() - 2500.0).abs() < 0.1);
    // Each piece keeps the turning direction of the face.
    assert!(pieces.iter().all(|v| cross(sub(v[1], v[0]), sub(v[2], v[0]))[2] > 0.0));
}

#[test]
fn wire_and_clearance_summaries() {
    openecraft_i18n::with_lang(openecraft_i18n::Lang::En, || {
        let point = |min, max, wires| ConnectionPoint { pin: "1".into(), min_cross_section: min, max_cross_section: max, max_wires: wires, ..Default::default() };
        let mut p = Part::default();
        assert_eq!(wire_summary(&p), "");
        p.connection_points = vec![point(0.2, 1.5, 1), point(0.5, 4.0, 2), ConnectionPoint::default()];
        assert_eq!(wire_summary(&p), "0.2 – 4 mm², 2 wires per connection point");
        p.connection_points = vec![point(0.0, 2.5, 0)];
        assert_eq!(wire_summary(&p), "up to 2.5 mm²");
        p.connection_points = vec![point(0.0, 0.0, 3)];
        assert_eq!(wire_summary(&p), "3 wires per connection point");
        let md = MountingData { clearance_top: 25.0, clearance_bottom: 25.0, clearance_front: 12.5, ..Default::default() };
        assert_eq!(clearance_summary(&md), "25 mm above, 25 mm below, 12.5 mm in front");
        assert_eq!(clearance_summary(&MountingData::default()), "");
    });
}
