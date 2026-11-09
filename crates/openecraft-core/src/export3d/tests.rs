use super::*;
use crate::sample::demo_project;
use std::collections::HashSet;

#[test]
fn the_scene_has_walls_door_plate_and_items() {
    let prj = demo_project();
    let s = scene(&prj);
    assert_eq!(s.len(), prj.layout.enclosures.len());
    let e = &prj.layout.enclosures[0];
    let solids = &s[0].solids;
    let parts = panel::components(e).iter().filter(|c| !c.boxes.is_empty()).count();
    assert_eq!(parts, 7, "door, five walls, mounting plate");
    assert_eq!(solids.len(), parts + e.items.iter().filter(|i| i.kind != ItemKind::Cutout).count());
    assert!(solids.iter().any(|x| x.transparent && x.name.ends_with("door")));
    assert!(solids.iter().any(|x| x.name == "-K1"));
    // Everything lies inside the enclosure (the door in front of it).
    for x in solids {
        for v in x.triangles.iter().flatten() {
            assert!(v[0] >= -0.01 && v[0] <= e.width + 0.01 && v[1] >= -0.01 && v[1] <= e.height + 0.01, "{} {v:?}", x.name);
            assert!(v[2] >= -0.01 && v[2] <= e.depth + crate::panel::WALL + 0.01, "{} {v:?}", x.name);
        }
    }
}

#[test]
fn base64_and_step_reals() {
    assert_eq!(crate::document::base64_encode(b"Man"), "TWFu");
    assert_eq!(crate::document::base64_encode(b"Ma"), "TWE=");
    assert_eq!(crate::document::base64_encode(b"M"), "TQ==");
    assert_eq!(real(12.0), "12.");
    assert_eq!(real(-0.5), "-0.5");
    assert_eq!(real(1e-9), "0.");
}

#[test]
fn gltf_is_consistent() {
    let prj = demo_project();
    let text = gltf(&prj);
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["asset"]["version"], "2.0");
    let buffer = &v["buffers"][0];
    let len = buffer["byteLength"].as_u64().unwrap() as usize;
    let uri = buffer["uri"].as_str().unwrap();
    let data = uri.strip_prefix("data:application/octet-stream;base64,").unwrap();
    assert_eq!(data.len(), len.div_ceil(3) * 4);
    let views = v["bufferViews"].as_array().unwrap();
    let total: u64 = views.iter().map(|b| b["byteLength"].as_u64().unwrap()).sum();
    assert_eq!(total as usize, len);
    for a in v["accessors"].as_array().unwrap() {
        let view = &views[a["bufferView"].as_u64().unwrap() as usize];
        assert_eq!(a["count"].as_u64().unwrap() * 12, view["byteLength"].as_u64().unwrap());
    }
    // A root node per enclosure, a mesh node per solid.
    let solids: usize = scene(&prj).iter().map(|e| e.solids.len()).sum();
    assert_eq!(v["meshes"].as_array().unwrap().len(), solids);
    assert_eq!(v["scenes"][0]["nodes"].as_array().unwrap().len(), prj.layout.enclosures.len());
    assert!(v["materials"].as_array().unwrap().iter().any(|m| m["alphaMode"] == "BLEND"));
}

/// Every `#n` used in the data section of a STEP physical file (STEP, IFC) is defined once.
fn assert_references_defined(text: &str) {
    assert!(text.starts_with("ISO-10303-21;\nHEADER;") && text.ends_with("END-ISO-10303-21;\n"));
    let mut defined = HashSet::new();
    let mut used = HashSet::new();
    for line in text.lines().filter(|l| l.starts_with('#')) {
        let (id, body) = line.split_once('=').unwrap();
        assert!(defined.insert(id.to_string()), "{id} twice");
        let mut rest = body;
        while let Some(i) = rest.find('#') {
            let digits: String = rest[i + 1..].chars().take_while(|c| c.is_ascii_digit()).collect();
            used.insert(format!("#{digits}"));
            rest = &rest[i + 1 + digits.len()..];
        }
        assert!(line.ends_with(';'));
    }
    let missing: Vec<_> = used.difference(&defined).collect();
    assert!(missing.is_empty(), "{missing:?}");
}

fn solid_count(prj: &Project) -> usize {
    scene(prj).iter().map(|e| e.solids.iter().filter(|s| !s.triangles.is_empty()).count()).sum()
}

#[test]
fn step_references_only_defined_entities() {
    let prj = demo_project();
    let text = step(&prj, "panel.stp");
    assert!(text.contains("FILE_SCHEMA(('AUTOMOTIVE_DESIGN"));
    assert_references_defined(&text);
    let solids: usize = scene(&prj).iter().map(|e| e.solids.iter().filter(|s| !s.triangles.is_empty()).count()).sum();
    assert_eq!(text.matches("=FACETED_BREP(").count(), solids);
    assert_eq!(text.matches("=STYLED_ITEM(").count(), solids);
    assert!(text.contains("FACETED_BREP('-K1'"));
}

#[test]
fn ifc_has_a_board_per_enclosure_and_an_element_per_solid() {
    let prj = demo_project();
    let text = ifc(&prj, "panel.ifc");
    assert!(text.contains("FILE_SCHEMA(('IFC4'))"));
    assert_references_defined(&text);
    assert_eq!(text.matches("=IFCELECTRICDISTRIBUTIONBOARD(").count(), prj.layout.enclosures.len());
    assert_eq!(text.matches("=IFCBUILDINGELEMENTPROXY(").count(), solid_count(&prj));
    assert_eq!(text.matches("=IFCTRIANGULATEDFACESET(").count(), solid_count(&prj));
    assert!(text.contains("IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.)"));
    // GlobalIds: 22 characters, unique, the same for every export.
    let ids: Vec<&str> = text.lines().filter(|l| l.contains("=IFC")).filter_map(|l| l.split('\'').nth(1)).filter(|s| s.len() == 22).collect();
    assert!(ids.len() > solid_count(&prj));
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    assert_eq!(ifc(&prj, "panel.ifc"), text);
    // The IFC import reads it back: z up, all enclosures side by side.
    let mesh = crate::parts::parse_ifc(text.as_bytes()).unwrap();
    let height = prj.layout.enclosures.iter().map(|e| e.height).fold(0.0, f32::max);
    assert!((mesh.size()[2] - height).abs() < 0.01, "{:?}", mesh.size());
}

#[test]
fn ifc_strings_and_guids() {
    assert_eq!(ifc::guid(0), "0000000000000000000000");
    assert_eq!(ifc::guid(u128::MAX), "3$$$$$$$$$$$$$$$$$$$$$");
    assert_eq!(ifc::text("It's -K1"), "'It''s -K1'");
    assert_eq!(ifc::text("Größe"), "'Gr\\X2\\00F6\\X0\\\\X2\\00DF\\X0\\e'");
}

/// Directory entry: type, form, parameter pointer.
type DirectoryEntry = (u32, u32, usize);
/// Parameter record: DE pointer, text.
type ParameterRecord = (usize, String);

/// Directory entries and parameter records of an IGES file, after checking the
/// fixed-column layout.
fn iges_sections(text: &str) -> (Vec<DirectoryEntry>, Vec<ParameterRecord>) {
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines.iter().all(|l| l.len() == 80), "80 columns");
    let count = |c: char| lines.iter().filter(|l| l.as_bytes()[72] == c as u8).count();
    let terminate = lines.last().unwrap();
    assert_eq!(&terminate[72..73], "T");
    let field = |s: &str| s[1..8].trim().parse::<usize>().unwrap();
    assert_eq!([field(&terminate[0..8]), field(&terminate[8..16]), field(&terminate[16..24]), field(&terminate[24..32])], [count('S'), count('G'), count('D'), count('P')]);
    let d: Vec<&str> = lines.iter().filter(|l| &l[72..73] == "D").copied().collect();
    let num = |s: &str| s.trim().parse::<usize>().unwrap();
    let entries = d.chunks(2).map(|e| (num(&e[0][0..8]) as u32, num(&e[1][32..40]) as u32, num(&e[0][8..16]))).collect();
    let params = lines.iter().filter(|l| &l[72..73] == "P").map(|l| (num(&l[64..72]), l[..64].trim_end().to_string())).collect();
    (entries, params)
}

#[test]
fn iges_solids_reference_existing_entities() {
    let prj = demo_project();
    let text = iges(&prj, "panel.igs");
    let (entries, params) = iges_sections(&text);
    // Closed meshes as solids, the others (the walls: overlapping boxes) as open shells.
    let solids = entries.iter().filter(|e| e.0 == 186).count();
    assert_eq!(solids + entries.iter().filter(|e| (e.0, e.1) == (514, 2)).count(), solid_count(&prj));
    assert!(solids > prj.layout.enclosures.len());
    assert_eq!(entries.iter().filter(|e| (e.0, e.1) == (402, 7)).count(), prj.layout.enclosures.len());
    // The parameters of each entity start at its pointer, with its type, and end with `;`.
    for (i, &(kind, _, first)) in entries.iter().enumerate() {
        let de = 2 * i + 1;
        assert_eq!(params[first - 1].0, de);
        assert!(params[first - 1].1.starts_with(&format!("{kind},")), "{kind}: {}", params[first - 1].1);
        let all: String = params.iter().filter(|p| p.0 == de).map(|p| p.1.as_str()).collect();
        assert!(all.ends_with(';'));
    }
    // An MSBO points to a closed shell, a face to a plane and a loop.
    let kind_at = |de: usize| entries[(de - 1) / 2].0;
    let fields = |de: usize| -> Vec<String> {
        let all: String = params.iter().filter(|p| p.0 == de).map(|p| p.1.as_str()).collect();
        all.trim_end_matches(';').split(',').map(str::to_string).collect()
    };
    let msbo = 2 * entries.iter().position(|e| e.0 == 186).unwrap() + 1;
    let shell: usize = fields(msbo)[1].parse().unwrap();
    assert_eq!((kind_at(shell), entries[(shell - 1) / 2].1), (514, 1));
    let face: usize = fields(shell)[2].parse().unwrap();
    let f = fields(face);
    assert_eq!((kind_at(f[1].parse().unwrap()), kind_at(f[4].parse().unwrap())), (190, 508));
    assert!(text.contains("2HMM"));
}

#[test]
fn iges_packs_parameters_and_dates() {
    assert_eq!(iges::pack(&["110".into(), "1.".into()], 64), ["110,1.;"]);
    let long: Vec<String> = (0..30).map(|i| format!("{i}.5")).collect();
    let lines = iges::pack(&long, 64);
    assert!(lines.len() > 1 && lines.iter().all(|l| l.len() <= 64));
    assert_eq!(lines.concat().split(',').count(), 30);
    let tetrahedron = [[1, 3, 2], [1, 2, 4], [2, 3, 4], [3, 1, 4]];
    assert!(iges::closed(&tetrahedron));
    assert!(!iges::closed(&tetrahedron[..3]));
    assert!(!iges::closed(&[[1, 2, 3], [1, 2, 4], [2, 3, 4], [3, 1, 4]]), "inconsistent orientation");
    assert_eq!(iges::timestamp("2026-10-23"), "20261023.000000");
    assert_eq!(iges::timestamp("23.10.26"), "20000101.000000");
}

#[test]
fn jt_holds_every_solid() {
    let prj = demo_project();
    let mesh = openecraft_jt::parse_jt(&jt(&prj)).unwrap();
    let triangles: usize = scene(&prj).iter().flat_map(|e| &e.solids).map(|s| s.triangles.len()).sum();
    assert!(mesh.triangles.len() > triangles / 2, "{} of {triangles}", mesh.triangles.len());
    // z up: the enclosure height is the extent in z.
    let height = prj.layout.enclosures.iter().map(|e| e.height).fold(0.0, f32::max);
    assert!((mesh.size()[2] - height).abs() < 0.01, "{:?}", mesh.size());
}

#[test]
fn parasolid_has_a_body_per_closed_part() {
    let prj = demo_project();
    let text = parasolid(&prj);
    assert!(text.starts_with("**ABCDEFGHIJKLMNOPQRSTUVWXYZ") && text.contains("SCH=SCH_1300000_13006;"));
    assert!(text.lines().all(|l| l.len() <= 80));
    // The stream (newlines are not significant) starts with the root list of bodies.
    let stream: String = text.lines().skip_while(|l| !l.starts_with("T51")).collect();
    let tokens: Vec<&str> = stream.split_once("SCH_1300000_13006").unwrap().1.split(' ').collect();
    assert_eq!((tokens[0], tokens[1]), ("0", "74"));
    let bodies: usize = tokens[2].parse().unwrap();
    // Every box of a wall, plate, rail or device is a solid of its own.
    let solids = scene(&prj).iter().flat_map(|e| &e.solids).count();
    assert!(bodies >= solids, "{bodies} bodies for {solids} solids");
}

#[test]
fn empty_layouts_export_empty_files() {
    let prj = Project::default();
    let v: serde_json::Value = serde_json::from_str(&gltf(&prj)).unwrap();
    assert_eq!(v["meshes"].as_array().unwrap().len(), 0);
    assert!(step(&prj, "x.stp").contains("FACETED_BREP_SHAPE_REPRESENTATION"));
    assert!(!ifc(&prj, "x.ifc").contains("IFCRELCONTAINEDINSPATIALSTRUCTURE"));
    assert_eq!(iges_sections(&iges(&prj, "x.igs")).0.len(), 0);
    assert!(openecraft_jt::parse_jt(&jt(&prj)).is_err());
    assert!(parasolid(&prj).trim_end().ends_with("1 0"));
}

#[test]
fn floor_standing_system_with_frame_and_plinth() {
    let mut prj = demo_project();
    panel::apply_system(&mut prj.layout.enclosures[0], "baying", Some((800.0, 2000.0, 600.0)));
    let s = scene(&prj);
    let solids = &s[0].solids;
    let plinth = solids.iter().find(|x| x.name.contains("plinth")).unwrap();
    // y up: the plinth is below the enclosure.
    assert!(plinth.triangles.iter().flatten().all(|v| v[1] <= 0.01 && v[1] >= -100.01));
    assert!(solids.iter().any(|x| x.name.contains("GEN.TS-800X2000X600")));
    assert!(solids.iter().any(|x| x.name.contains("GEN.TS-SP-2000X600")));
}

#[test]
fn bayed_enclosures_touch_and_stand_on_the_floor() {
    let mut prj = demo_project();
    panel::apply_system(&mut prj.layout.enclosures[0], "baying", Some((800.0, 2000.0, 600.0)));
    let mut e = panel::new_enclosure(&mut prj, "", "+CAB2");
    e.build.plinth = 0.0;
    prj.layout.enclosures.push(e);
    let mut lone = panel::new_enclosure(&mut prj, "", "+CAB3");
    lone.build.plinth = 0.0;
    prj.layout.enclosures.push(lone);
    panel::bay_right_of(&mut prj, 1, 0);
    let s = scene(&prj);
    assert_eq!(s[1].offset, [800.0, 0.0, 0.0]);
    assert_eq!(s[0].offset[1], 100.0, "lifted by its plinth");
    assert_eq!(s[2].offset[0], 800.0 + 600.0 + panel::LINEUP_GAP);
}

#[test]
fn the_ifc_site_has_the_name_and_place_of_the_plot() {
    let mut prj = demo_project();
    assert!(ifc(&prj, "x.ifc").contains(",'Site',$,$,#"), "no plot: a site without a place");
    prj.layout.plot.name = "Flur 3, 112/4".into();
    prj.layout.plot.reference = Some(crate::panel::GeoReference { height: 54.25, latitude: Some(52.520_008), longitude: Some(-13.404_954), ..Default::default() });
    let text = ifc(&prj, "x.ifc");
    assert!(text.contains(",'Flur 3, 112/4',$,$,#"), "{text}");
    assert!(text.contains(".ELEMENT.,(52,31,12,28800),(-13,-24,-17,-834400),54250.0,$,$)"), "{text}");
    assert_references_defined(&text);
}
