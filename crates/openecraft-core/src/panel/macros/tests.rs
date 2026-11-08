use super::*;
use crate::geom::p;
use crate::panel::{collisions, outside};
use crate::sample::demo_project;

fn find(prj: &Project, name: &str) -> PanelMacro {
    builtin_macros(prj).into_iter().find(|m| m.name == name).unwrap()
}

#[test]
fn reusable_panel_macros_clear_external_material_owners_and_keep_exclusion() {
    let mut prj=demo_project();
    let item=&mut prj.layout.enclosures[0].items[0];item.owner_device="-X1".into();item.bom_excluded=true;
    let id=item.id;let saved=macro_from_items(&prj.layout.enclosures[0],&[id],"Reusable").unwrap();
    assert!(saved.items[0].owner_device.is_empty());assert!(saved.items[0].tag.is_empty());
    assert!(saved.items[0].bom_excluded);
}

#[test]
fn builtin_macros_are_named_and_fit() {
    let prj = demo_project();
    let all = builtin_macros(&prj);
    assert_eq!(all.len(), BUILTIN.len());
    for m in &all {
        assert!(m.is_builtin() && !m.items.is_empty(), "{}", m.name);
        for it in m.items.iter().filter(|i| !i.part.is_empty()) {
            assert!(part_by_number(&prj, &it.part).is_some(), "{}: {}", m.name, it.part);
        }
    }
}

#[test]
fn duct_frame_adapts_to_the_plate() {
    let mut prj = demo_project();
    prj.layout.enclosures[0].items.clear();
    let m = find(&prj, "Duct frame with rails");
    let ids = insert_macro(&mut prj, 0, &m, Face::Plate, p(0.0, 0.0));
    let e = &prj.layout.enclosures[0];
    assert_eq!(ids.len(), 7);
    let (pw, ph) = e.plate_size();
    let top = e.item(ids[0]).unwrap();
    assert_eq!((top.x, top.y, top.w), (0.0, 0.0, pw));
    let right = e.item(ids[3]).unwrap();
    assert_eq!((right.x + right.w, right.h), (pw, ph - 2.0 * DUCT_PROFILE.0));
    // One group named after the macro, nothing outside or colliding.
    assert_eq!(e.groups.len(), 1);
    assert!(e.items.iter().all(|i| i.group == e.groups[0].id));
    assert!(outside(e).is_empty(), "{:?}", outside(e));
    assert!(collisions(e, &prj.parts).is_empty(), "{:?}", collisions(e, &prj.parts));
}

#[test]
fn macros_on_several_faces() {
    let mut prj = demo_project();
    let m = find(&prj, "Filter fan with outlet filter");
    let ids = insert_macro(&mut prj, 0, &m, Face::Left, p(0.0, 0.0));
    let e = &prj.layout.enclosures[0];
    let faces: Vec<Face> = ids.iter().map(|id| e.item(*id).unwrap().face).collect();
    assert_eq!(faces, vec![Face::Left, Face::Right]);
    let fan = e.item(ids[0]).unwrap();
    assert_eq!(fan.y + fan.h, e.height - 150.0);
    assert!(fan.z < 0.0, "the fan grille sticks out");
    // Their cutouts are part of the NC data of the sides.
    assert_eq!(crate::production::openings(e, &prj.parts, Face::Left).len(), 1);
}

#[test]
fn relative_items_follow_the_insertion_point() {
    let mut prj = demo_project();
    let m = find(&prj, "Cable entry 5 x M25");
    let ids = insert_macro(&mut prj, 0, &m, Face::Bottom, p(100.0, 80.0));
    let e = &prj.layout.enclosures[0];
    let xs: Vec<f32> = ids.iter().map(|id| e.item(*id).unwrap().x).collect();
    assert_eq!(xs, vec![100.0, 150.0, 200.0, 250.0, 300.0]);
    assert!(ids.iter().all(|id| e.item(*id).unwrap().y == 80.0));
}

#[test]
fn macro_from_selection_and_back() {
    let mut prj = demo_project();
    let e = &prj.layout.enclosures[0];
    // The rail with K1 on it.
    let k1 = prj.layout.find_device("-K1").unwrap().1.clone();
    let rail = e.items.iter().find(|i| i.kind == ItemKind::Rail && i.rect().contains(k1.center())).unwrap();
    let ids = [rail.id, k1.id];
    let m = macro_from_items(e, &ids, "Rail with contactor").unwrap();
    assert_eq!(m.face, Face::Plate);
    assert!(m.items.iter().all(|i| i.tag.is_empty()));
    assert_eq!(m.items.iter().map(|i|i.id).collect::<Vec<_>>(),(1..=m.items.len() as u64).collect::<Vec<_>>());
    assert!(m.items.iter().filter(|i|i.mounted_on!=0).all(|i|m.items.iter().any(|p|p.id==i.mounted_on)));
    assert_eq!(m.items.iter().map(|i| i.x).fold(f32::INFINITY, f32::min), 0.0);
    // K1 is placed already: the device of the copy becomes an accessory with its part.
    let new = insert_macro(&mut prj, 0, &m, Face::Plate, p(50.0, 650.0));
    let e = &prj.layout.enclosures[0];
    let dev = new.iter().filter_map(|id| e.item(*id)).find(|i| i.part == k1.part).unwrap();
    assert_eq!(dev.kind, ItemKind::Accessory);
    // It sits on the rail of the copy.
    let rail = new.iter().filter_map(|id| e.item(*id)).find(|i| i.kind == ItemKind::Rail).unwrap();
    assert!((dev.center().y - rail.center().y).abs() < 0.01);
    assert_eq!(dev.mounted_on,rail.id);
    assert_eq!(group_members(e, dev.group).len(), 2);
}
