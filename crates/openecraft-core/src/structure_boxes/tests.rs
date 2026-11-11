use super::*;
use crate::model::SymbolInst;
use crate::structure::Source;

/// Page `=SYS+CAB1` with a structure box `+FIELD` from (100, 100) to (200, 200).
fn project() -> Project {
    let mut prj = Project::default();
    let pg = &mut prj.pages[0];
    pg.function = "SYS".into();
    pg.location = "CAB1".into();
    let b = new_box(&mut prj, p(200.0, 200.0), p(100.0, 100.0), Dt::parse_structure("+FIELD"), "Field devices");
    prj.pages[0].structure_boxes.push(b);
    prj
}

fn sym(prj: &mut Project, key: &str, x: f32, y: f32, tag: &str) -> u64 {
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst { id, sym: key.into(), pos: p(x, y), tag: tag.into(), ..Default::default() });
    id
}

fn tag(prj: &Project, id: u64) -> String {
    prj.pages[0].symbols.iter().find(|s| s.id == id).unwrap().tag.clone()
}

fn move_to(prj: &mut Project, id: u64, x: f32, y: f32) {
    prj.pages[0].symbols.iter_mut().find(|s| s.id == id).unwrap().pos = p(x, y);
}

#[test]
fn a_box_sets_only_its_own_aspects() {
    let mut prj = project();
    let b = prj.pages[0].structure_boxes[0].clone();
    assert_eq!((b.a, b.b), (p(100.0, 100.0), p(200.0, 200.0)));
    assert_eq!(b.identifiers(), "+FIELD");
    let pg = &prj.pages[0];
    assert_eq!(structure::scope(pg, p(150.0, 150.0)).structure(), "=SYS+FIELD");
    assert_eq!(structure::scope(pg, p(50.0, 50.0)).structure(), "=SYS+CAB1");
    // Edges belong to the box.
    assert_eq!(structure::scope(pg, p(100.0, 200.0)).structure(), "=SYS+FIELD");
    assert_eq!(structure::complete_at(pg, p(150.0, 150.0), "-S1"), "=SYS+FIELD-S1");
    assert_eq!(structure::relative_at(pg, p(150.0, 150.0), "=SYS+FIELD-S1"), "-S1");
    assert_eq!(structure::relative_at(pg, p(50.0, 50.0), "=SYS+FIELD-S1"), "+FIELD-S1");
    // A nested box sets the function; the location still comes from the outer box.
    let inner = new_box(&mut prj, p(120.0, 120.0), p(160.0, 160.0), Dt::parse_structure("=PUMP"), "");
    let inner_id = inner.id;
    prj.pages[0].structure_boxes.push(inner);
    let pg = &prj.pages[0];
    assert_eq!(structure::scope(pg, p(130.0, 130.0)).structure(), "=PUMP+FIELD");
    assert!(structure::overlapping(pg).is_empty());
    let outer = pg.structure_boxes[0].id;
    // The structure of a box is its own and that of the boxes around it, not of those in it.
    let (o, i) = (&pg.structure_boxes[0], &pg.structure_boxes[1]);
    assert_eq!(structure::box_scope(pg, o).structure(), "=SYS+FIELD");
    assert_eq!(structure::box_scope(pg, i).structure(), "=PUMP+FIELD");
    assert_eq!(structure::around_box(pg, i).structure(), "=SYS+FIELD");
    assert_eq!(structure::around_box(pg, o).structure(), "=SYS+CAB1");
    assert_eq!(
        structure::sources(pg, p(130.0, 130.0), "=PUMP+X-S1"),
        vec![(Aspect::Function, "PUMP".to_string(), Source::Box(inner_id)), (Aspect::Location, "X".to_string(), Source::Device)]
    );
    assert_eq!(structure::sources(pg, p(150.0, 190.0), "=SYS+FIELD-S1")[1].2, Source::Box(outer));
    assert_eq!(structure::sources(pg, p(150.0, 190.0), "=SYS+FIELD-S1")[0].2, Source::Page);
}

#[test]
fn devices_follow_their_main_function_into_and_out_of_a_box() {
    let mut prj = project();
    let coil = sym(&mut prj, "coil", 20.0, 20.0, "=SYS+CAB1-K1");
    let contact = sym(&mut prj, "no", 40.0, 20.0, "=SYS+CAB1-K1");
    prj.devices.entry("=SYS+CAB1-K1".into()).or_default().description = "pump".into();
    let before = prj.clone();
    move_to(&mut prj, coil, 150.0, 150.0);
    assert!(follow(&mut prj, &before).is_empty());
    // The whole device moves: its contact outside the box keeps the device identity.
    assert_eq!((tag(&prj, coil), tag(&prj, contact)), ("=SYS+FIELD-K1".to_string(), "=SYS+FIELD-K1".to_string()));
    assert_eq!(prj.devices["=SYS+FIELD-K1"].description, "pump");
    // The contact outside shows the location it differs in.
    assert_eq!(structure::relative_at(&prj.pages[0], p(40.0, 20.0), &tag(&prj, contact)), "+FIELD-K1");
    // Moving an auxiliary function changes nothing.
    let before = prj.clone();
    move_to(&mut prj, contact, 160.0, 150.0);
    assert!(follow(&mut prj, &before).is_empty());
    assert_eq!(tag(&prj, contact), "=SYS+FIELD-K1");
    // Out again.
    let before = prj.clone();
    move_to(&mut prj, coil, 20.0, 20.0);
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, coil), "=SYS+CAB1-K1");
    // Nothing moved: nothing to do.
    let same = prj.clone();
    assert!(follow(&mut prj, &same).is_empty());
    assert_eq!(prj, same);
}

#[test]
fn explicit_aspects_are_kept() {
    let mut prj = project();
    let own_location = sym(&mut prj, "coil", 20.0, 20.0, "=SYS+DESK-K2");
    let own_function = sym(&mut prj, "coil", 30.0, 20.0, "=OTHER+CAB1-K3");
    let before = prj.clone();
    move_to(&mut prj, own_location, 150.0, 150.0);
    move_to(&mut prj, own_function, 150.0, 170.0);
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, own_location), "=SYS+DESK-K2");
    assert_eq!(tag(&prj, own_function), "=OTHER+FIELD-K3");
    // A tag changed in the same step is the user's choice.
    let before = prj.clone();
    let s = prj.pages[0].symbols.iter_mut().find(|s| s.id == own_function).unwrap();
    s.pos = p(20.0, 20.0);
    s.tag = "=OTHER+FIELD-K9".into();
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, own_function), "=OTHER+FIELD-K9");
}

#[test]
fn a_device_tag_that_exists_already_is_refused() {
    let mut prj = project();
    sym(&mut prj, "coil", 150.0, 150.0, "=SYS+FIELD-K1");
    let other = sym(&mut prj, "coil", 20.0, 20.0, "=SYS+CAB1-K1");
    let before = prj.clone();
    move_to(&mut prj, other, 150.0, 180.0);
    let problems = follow(&mut prj, &before);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("=SYS+FIELD-K1"), "{problems:?}");
    // The device keeps its tag, which now shows the location it differs in.
    assert_eq!(tag(&prj, other), "=SYS+CAB1-K1");
    assert_eq!(structure::relative_at(&prj.pages[0], p(150.0, 180.0), "=SYS+CAB1-K1"), "+CAB1-K1");
}

#[test]
fn changing_resizing_and_deleting_a_box_moves_its_devices() {
    let mut prj = project();
    let coil = sym(&mut prj, "pb_no", 220.0, 150.0, "=SYS+CAB1-S1");
    // Resized over the device.
    let before = prj.clone();
    prj.pages[0].structure_boxes[0].b = p(240.0, 200.0);
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, coil), "=SYS+FIELD-S1");
    // Another identifier.
    let before = prj.clone();
    prj.pages[0].structure_boxes[0].structure.location = "FIELD2".into();
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, coil), "=SYS+FIELD2-S1");
    // The box moved together with its devices: nothing changes.
    let before = prj.clone();
    let sel = std::collections::BTreeSet::from([prj.pages[0].structure_boxes[0].id, coil]);
    crate::ops::translate(&mut prj.pages[0], &sel, &[], p(0.0, 40.0));
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, coil), "=SYS+FIELD2-S1");
    // Deleted: back to the page structure.
    let before = prj.clone();
    prj.pages[0].structure_boxes.clear();
    follow(&mut prj, &before);
    assert_eq!(tag(&prj, coil), "=SYS+CAB1-S1");
}

#[test]
fn undo_restores_geometry_and_assignment_together() {
    let mut prj = project();
    let coil = sym(&mut prj, "coil", 20.0, 20.0, "=SYS+CAB1-K1");
    let mut history = crate::history::History::new(&prj);
    let start = prj.clone();
    move_to(&mut prj, coil, 150.0, 150.0);
    follow(&mut prj, &start);
    assert!(history.track(&prj, true));
    assert_eq!(tag(&prj, coil), "=SYS+FIELD-K1");
    history.undo(&mut prj);
    assert_eq!(prj, start);
}

#[test]
fn new_functions_and_pasted_devices_take_the_structure_of_their_place() {
    let mut prj = project();
    let def = prj.symbol("coil").unwrap();
    let s = crate::ops::new_symbol(&mut prj, 0, &Default::default(), &def, p(150.0, 150.0), 0, false);
    assert_eq!(s.tag, "=SYS+FIELD-K1");
    prj.pages[0].symbols.push(s);
    // Copied with its box and pasted elsewhere: a new box, the device in it.
    let all: std::collections::BTreeSet<u64> = crate::ops::all_ids(&prj.pages[0]).into_iter().collect();
    let clip = crate::ops::copy(&prj.pages[0], &all);
    assert_eq!(clip.symbols[0].tag, "=SYS+CAB1-K1", "stored in the page structure");
    let ids = crate::ops::paste(&mut prj, 0, &clip, p(300.0, 100.0));
    assert_eq!(prj.pages[0].structure_boxes.len(), 2);
    let pasted = prj.pages[0].symbols.iter().find(|s| ids.contains(&s.id)).unwrap();
    assert_eq!(pasted.tag, "=SYS+FIELD-K2", "{:?}", prj.all_tags());
    // Copied without the box and pasted outside: the page structure.
    let one = std::collections::BTreeSet::from([prj.pages[0].symbols[0].id]);
    let clip = crate::ops::copy(&prj.pages[0], &one);
    let ids = crate::ops::paste(&mut prj, 0, &clip, p(20.0, 300.0));
    let pasted = prj.pages[0].symbols.iter().find(|s| ids.contains(&s.id)).unwrap();
    assert_eq!(pasted.tag, "=SYS+CAB1-K1");
}

#[test]
fn overlapping_and_empty_boxes_are_reported() {
    let mut prj = project();
    assert!(problems(&prj).is_empty());
    let b = new_box(&mut prj, p(150.0, 150.0), p(250.0, 250.0), Dt::parse_structure("=A2"), "");
    prj.pages[0].structure_boxes.push(b);
    let b = new_box(&mut prj, p(300.0, 300.0), p(320.0, 320.0), Dt::default(), "");
    prj.pages[0].structure_boxes.push(b);
    let found = problems(&prj);
    assert_eq!(found.len(), 2, "{found:?}");
    assert_eq!(found[0].severity, Severity::Warning);
    assert_eq!(found[1].severity, Severity::Info);
    // The smaller box wins in the common area, whatever the order.
    assert_eq!(structure::scope(&prj.pages[0], p(180.0, 180.0)).structure(), "=A2+FIELD");
}

#[test]
fn identifiers_of_boxes_are_used_and_renamed() {
    let mut prj = project();
    let u = structure::overview(&prj);
    assert!(u.iter().any(|u| u.aspect == Aspect::Location && u.id == "FIELD" && u.pages == 1), "{u:?}");
    let coil = sym(&mut prj, "coil", 150.0, 150.0, "=SYS+FIELD-K1");
    structure::rename(&mut prj, Aspect::Location, "FIELD", "OUT").unwrap();
    assert_eq!(prj.pages[0].structure_boxes[0].identifiers(), "+OUT");
    assert_eq!(tag(&prj, coil), "=SYS+OUT-K1");
}

#[test]
fn the_label_sits_in_the_chosen_corner() {
    let mut prj = project();
    let b = &mut prj.pages[0].structure_boxes[0];
    assert_eq!(label_lines(b), vec!["+FIELD".to_string(), "Field devices".to_string()]);
    assert_eq!(label_pos(b), p(101.0, 101.0));
    b.label = structure::BoxCorner::BottomRight;
    b.label_offset = p(-2.0, 0.0);
    assert_eq!(label_pos(b), p(197.0, 199.0));
    assert!((outline_distance(b, p(150.0, 102.0)) - 2.0).abs() < 1e-4);
    // Saved and read back.
    let text = prj.to_json();
    assert_eq!(Project::from_json(&text).unwrap().pages[0].structure_boxes, prj.pages[0].structure_boxes);
}
#[test]
fn a_device_inside_a_box_with_another_structure_is_reported() {
    let mut prj = project();
    sym(&mut prj, "no", 150.0, 150.0, "=SYS+CAB1-K1");
    sym(&mut prj, "no", 160.0, 150.0, "=SYS+FIELD-K2");
    sym(&mut prj, "no", 50.0, 50.0, "=SYS+CAB1-K3");
    let found: Vec<String> = problems(&prj).iter().map(|i| i.message().to_string()).collect();
    assert_eq!(found, ["=SYS+CAB1-K1 is inside the structure box +FIELD but has another structure"]);
}

#[test]
fn boxes_continuing_a_device_placed_elsewhere_are_not_reported() {
    use crate::device_boxes::{add_box, place_point, set_role};
    use crate::model::DeviceBoxRole;
    let mut prj = project();
    let b = add_box(&mut prj, 0, p(120.0, 120.0), p(160.0, 144.0), "=SYS+CAB1-A1", "").unwrap();
    place_point(&mut prj, 0, p(128.0, 120.0), 0, "", "L").unwrap();
    assert_eq!(problems(&prj).len(), 2, "the box and its point are this device");
    set_role(&mut prj, 0, b, DeviceBoxRole::View).unwrap();
    assert!(problems(&prj).is_empty());
}


#[test]
fn device_box_tags_typed_inside_a_box_take_its_structure() {
    use crate::device_boxes::{add_box, place_point};
    let mut prj = project();
    let b = add_box(&mut prj, 0, p(120.0, 120.0), p(160.0, 144.0), "-A1", "").unwrap();
    let point = place_point(&mut prj, 0, p(128.0, 120.0), 0, "-A1", "L").unwrap();
    assert_eq!([tag(&prj, b), tag(&prj, point)], ["=SYS+FIELD-A1", "=SYS+FIELD-A1"]);
    assert!(problems(&prj).is_empty());
    let outside = add_box(&mut prj, 0, p(20.0, 20.0), p(60.0, 44.0), "-A2", "").unwrap();
    assert_eq!(tag(&prj, outside), "=SYS+CAB1-A2");
}
