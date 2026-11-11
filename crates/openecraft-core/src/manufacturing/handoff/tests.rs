use super::*;
use crate::sample::demo_project;

#[test]
fn release_and_changes_after_the_hand_off() {
    let mut prj = demo_project();
    let list = programs(&prj, 0).unwrap();
    let faces: Vec<Option<Face>> = list.iter().map(|p| p.face).collect();
    // The bottom with the hole of the motor cable's gland.
    assert_eq!(faces, vec![Some(Face::Plate), Some(Face::Door), Some(Face::Bottom)]);
    assert_eq!(list[0].name, "CAB1_plate.nc");
    let (job, files) = release(&mut prj, 0, "2026-10-24").unwrap();
    assert_eq!(job.id, "CAB1-1");
    assert_eq!(job.programs.len(), files.len());
    assert_eq!(prj.layout.manufacturing.jobs.len(), 1);
    assert!(changed(&prj).is_empty());
    // Moving a door device changes the door program.
    let it = prj.layout.enclosures[0].items.iter_mut().find(|i| i.face == Face::Door && i.kind == crate::panel::ItemKind::Device).unwrap();
    it.x += 10.0;
    assert_eq!(changed(&prj), vec![("CAB1-1".to_string(), "CAB1_door.nc".to_string())]);
    let (job2, _) = release(&mut prj, 0, "2026-10-25").unwrap();
    assert_eq!(job2.id, "CAB1-2");
    assert!(changed(&prj).is_empty());
}

#[test]
fn packages_and_bars() {
    let mut prj = demo_project();
    prj.layout.manufacturing.bars.push(super::super::CopperBar { name: "L1".into(), enclosure: "+CAB1".into(), width: 20.0, thickness: 5.0, count: 1, segments: vec![300.0], ..Default::default() });
    let list = programs(&prj, 0).unwrap();
    assert_eq!(list.last().unwrap().name, "CAB1_bars.csv");
    let v = package(&prj, None, &list);
    assert_eq!(v["programs"].as_array().unwrap().len(), 4);
    assert_eq!(v["programs"][3]["face"], "bars");
    assert!(v["programs"][0]["content"].as_str().unwrap().contains("G81"));
    assert_eq!(package_all(&prj).unwrap()["programs"], v["programs"]);
    prj.layout.manufacturing.sheet_machine = "nope".into();
    assert!(programs(&prj, 0).is_err());
    let mut empty = Project::default();
    empty.layout.enclosures.push(Default::default());
    assert!(release(&mut empty, 0, "").is_err());
}

#[test]
fn rooms_are_not_machined() {
    let prj = crate::sample::example("pump_room").unwrap();
    let r = prj.layout.enclosures.iter().position(|e| e.is_room()).unwrap();
    // Its walls, ceiling and floor carry parts, but no program is made for them.
    assert!(!prj.layout.enclosures[r].items.is_empty());
    assert!(programs(&prj, r).unwrap().is_empty());
}
