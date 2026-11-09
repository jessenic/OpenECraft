use super::*;

#[test]
fn explicit_geometry_uses_authored_frames_without_a_default_plate_or_shell() {
    use super::super::{face_point,Face,EnclosureKind};
    let mut e=Enclosure {kind:EnclosureKind::Assembly,width:300.,height:400.,depth:100.,..Default::default()};
    assert_eq!(e.plate_size(),(300.,400.));assert_eq!(face_point(&e,Face::Plate,[10.,20.,3.]),[10.,20.,3.]);
    e.kind=EnclosureKind::Cabinet;e.no_implicit_plate=true;e.mounting_origin=Some([0.,0.,0.]);
    assert_eq!(face_point(&e,Face::Plate,[10.,20.,3.]),[10.,20.,3.]);
    assert!(!super::super::components(&e).iter().any(|c|c.role==super::super::Role::MountingPlate));
    e.no_implicit_plate=false;e.mounting_origin=Some([30.,40.,25.]);
    assert_eq!(face_point(&e,Face::Plate,[10.,20.,3.]),[40.,60.,28.]);
    let plate=super::super::components(&e).into_iter().find(|c|c.role==super::super::Role::MountingPlate).unwrap();
    assert_eq!(plate.boxes[0].1,[270.,360.,25.]);
    let mut prj=crate::model::Project::default();e.items.push(PanelItem {kind:ItemKind::MountingPlate,part:"EXPLICIT.PLATE".into(),w:200.,h:300.,d:2.,..Default::default()});
    prj.layout.enclosures.push(e);prj.layout.hardware=false;
    assert_eq!(super::super::layout_material(&prj).iter().find(|m|m.0=="EXPLICIT.PLATE").unwrap().1,1.);
    assert_eq!(crate::model::Project::from_json(&prj.to_json()).unwrap().layout.enclosures[0].mounting_origin,Some([30.,40.,25.]));
}

#[test]
fn nested_mounts_follow_moves_once_and_reject_cycles() {
    let mut e=Enclosure {items:vec![
        PanelItem {id:1,kind:ItemKind::MountingPlate,..Default::default()},
        PanelItem {id:2,kind:ItemKind::Rail,mounted_on:1,x:10.,y:20.,z:2.,..Default::default()},
        PanelItem {id:3,mounted_on:2,x:20.,y:10.,z:10.,..Default::default()},
        PanelItem {id:4,mounted_on:3,x:25.,y:15.,z:15.,..Default::default()},
    ],..Default::default()};
    assert_eq!(mounted_items(&e,&[1]),vec![1,2,3,4]);
    assert!(!can_mount(&e,1,4));assert!(can_mount(&e,4,1));
    let old=e.items.clone();e.items[0].x=30.;e.items[2].x=60.;
    carry_mounts(&mut e,&old,&[1,3]);
    assert_eq!((e.items[1].x,e.items[2].x,e.items[3].x),(40.,60.,65.));
    let old=e.items.clone();e.items[0].z=5.;carry_mounts(&mut e,&old,&[1]);
    assert_eq!((e.items[1].z,e.items[2].z,e.items[3].z),(7.,15.,20.));
    let old=e.items.clone();e.items[0].d=3.;carry_mounts(&mut e,&old,&[1]);
    assert_eq!((e.items[1].z,e.items[2].z,e.items[3].z),(10.,18.,23.));
}

#[test]
fn source_instance_mounting_datum_survives_part_refresh_and_native_reopening() {
    use crate::model::Project;
    let mut prj=Project::default();
    prj.parts.push(crate::parts::Part {number:"SourcePart".into(),width:12.,height:100.,depth:68.,..Default::default()});
    prj.layout.enclosures.push(Enclosure {items:vec![
        PanelItem {id:1,kind:ItemKind::Rail,x:0.,y:100.,w:200.,h:35.,z:2.,d:7.5,..Default::default()},
        PanelItem {id:2,part:"SourcePart".into(),x:50.,y:54.,w:12.,h:100.,z:9.5,d:68.,mounted_on:1,
            rail_mount:openecraft_parts::RailMount {center_offset:13.5,clip_on:0.},rail_mount_override:true,..Default::default()},
    ],..Default::default()});
    let mut reopened=Project::from_json(&prj.to_json()).unwrap();
    let item=&reopened.layout.enclosures[0].items[1];
    assert_eq!((item.y,item.z,item.rail_mount.center_offset),(54.,9.5,13.5));assert!(item.seated_on_rail);
    reopened.layout.enclosures[0].items[1].rail_mount_override=false;
    reopened.refresh_rail_mounts();
    assert_eq!(reopened.layout.enclosures[0].items[1].y,67.5);
}

#[test]
fn open_assemblies_and_explicit_plates_do_not_gain_implicit_geometry() {
    let mut e=Enclosure {kind:crate::model::panel::EnclosureKind::Assembly,..Default::default()};
    assert!(super::super::components(&e).is_empty());assert_eq!(e.faces(),vec![super::super::Face::Plate]);
    e.kind=crate::model::panel::EnclosureKind::Cabinet;
    e.items.push(PanelItem {kind:ItemKind::MountingPlate,..Default::default()});
    assert!(!super::super::components(&e).iter().any(|c|c.role==super::super::Role::MountingPlate));
}

#[test]
fn vertical_rails_snap_and_carry_their_devices_and_detach_survivors() {
    let mut e=Enclosure {items:vec![
        PanelItem {id:1,kind:ItemKind::Rail,x:100.,y:0.,w:35.,h:300.,z:2.,d:7.5,..Default::default()},
        PanelItem {id:2,x:62.,y:40.,w:100.,h:12.,rail_mount:openecraft_parts::RailMount {center_offset:13.5,clip_on:0.},..Default::default()},
    ],..Default::default()};
    let mut item=e.items[1].clone();super::super::snap_to_rail(&e,&mut item);e.items[1]=item;
    assert!(e.items[1].rail_vertical);assert!(e.items[1].seated_on(&e.items[0]));
    assert_eq!((e.items[1].x,e.items[1].z),(54.,9.5));
    let old=e.items.clone();e.items[0].x+=20.;carry_mounts(&mut e,&old,&[1]);
    assert_eq!(e.items[1].x,74.);
    assert!(super::super::arrange_on_rail(&mut e,1,&[2],5.));
    assert_eq!(e.items[1].y,5.);
    assert_eq!(remove_items(&mut e,&[1]),1);assert_eq!(e.items[0].mounted_on,0);
    assert!(e.items[0].mounting_explicit);
}

#[test]
fn portable_and_pattern_copies_remap_internal_mounting_parents() {
    let mut prj=crate::model::Project::default();
    let plate=prj.new_id();let rail=prj.new_id();
    prj.layout.enclosures.push(Enclosure {items:vec![
        PanelItem {id:plate,kind:ItemKind::MountingPlate,w:300.,h:400.,d:2.,..Default::default()},
        PanelItem {id:rail,kind:ItemKind::Rail,mounted_on:plate,mounting_explicit:true,x:20.,y:30.,z:2.,w:200.,h:35.,d:7.5,..Default::default()},
    ],..Default::default()});
    let template=super::super::macro_from_items(&prj.layout.enclosures[0],&[plate,rail],"Mounted").unwrap();
    let ids=super::super::insert_macro(&mut prj,0,&template,super::super::Face::Plate,crate::geom::p(400.,0.));
    assert_eq!(prj.layout.enclosures[0].item(ids[1]).unwrap().mounted_on,ids[0]);
    let ids=super::super::pattern_copy(&mut prj,0,&[plate],2,1,800.,0.);
    assert_eq!(ids.len(),2);
    assert_eq!(prj.layout.enclosures[0].item(ids[1]).unwrap().mounted_on,ids[0]);
}

#[test]
fn a_device_moved_off_a_vertical_rail_loses_its_rail_orientation() {
    let rail=PanelItem {id:1,kind:ItemKind::Rail,x:100.,y:0.,w:35.,h:300.,z:2.,d:7.5,..Default::default()};
    let e=Enclosure {items:vec![rail],..Default::default()};
    let mut item=PanelItem {id:2,x:500.,y:40.,w:100.,h:12.,rail_vertical:true,..Default::default()};
    super::super::snap_to_rail(&e,&mut item);
    assert!(!item.rail_vertical);
}
