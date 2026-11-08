use super::*;
use crate::panel::Face;
use crate::parts::ConnectionPoint;
use crate::sample::demo_project;

#[test]
fn individually_placed_terminals_use_their_own_parts_and_never_expand_the_strip() {
    let mut prj=demo_project();
    let whole=item(&prj,"-X1");
    let terminals=crate::strips::terminals(&prj,"-X1");
    let source=prj.pages[terminals[0].page].symbols[terminals[0].sym].clone();
    let mut part=super::super::part_by_number(&prj,&whole.part).unwrap().clone();
    part.number="INDIVIDUAL".into();part.width=8.;part.height=50.;part.depth=30.;
    part.connection_points=vec![ConnectionPoint {pin:"1".into(),pos:[4.,7.,20.],exit:Exit::Up}];
    prj.parts.push(part);
    let ei=prj.layout.find_device("-X1").unwrap().0;
    let single=PanelItem {id:prj.new_id(),tag:whole.tag.clone(),part:"INDIVIDUAL".into(),terminal:source.label.clone(),
        x:400.,y:200.,w:8.,h:50.,d:30.,mounting_explicit:true,..Default::default()};
    prj.layout.enclosures[ei].items.retain(|i|i.id!=whole.id);
    prj.layout.enclosures[ei].items.push(single.clone());
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    let points=device_points(&reopened,&single);
    assert_eq!(points.len(),1);assert_eq!(points[0].pos,[404.,207.,20.]);
    assert_eq!(points[0].terminal,source.label);assert!(points[0].exact);
    assert_eq!(point_of(&reopened,&points,&source,0).unwrap().pos,[404.,207.,20.]);
    assert_eq!(super::super::strip_cells(&reopened,&single).len(),1);
    assert_eq!(super::super::locate_symbol(&reopened,&source).unwrap().1.id,single.id);
    let mut absent=source.clone();absent.label="not placed".into();
    assert!(super::super::locate_symbol(&reopened,&absent).is_none());
    let mut duplicate=single.clone();duplicate.id=prj.new_id();
    prj.layout.enclosures[ei].items.push(duplicate);
    assert!(super::super::locate_symbol(&prj,&source).is_none());
}

#[test]
fn unnumbered_single_parts_generate_points_in_their_explicit_footprint() {
    let mut prj=demo_project();let mut part=super::super::part_by_number(&prj,&item(&prj,"-X1").part).unwrap().clone();
    part.number="UNSIZED".into();part.width=0.;part.height=0.;part.depth=0.;part.connection_points.clear();prj.parts.push(part);
    let it=PanelItem {id:10000,tag:"-X1".into(),part:"UNSIZED".into(),single_part:true,x:100.,y:200.,w:8.,h:50.,d:30.,..Default::default()};
    let points=device_points(&prj,&it);assert_eq!(points.len(),2);
    assert!(points.iter().all(|p|p.terminal.is_empty() && !p.exact && inside(p,&it)));
    assert_eq!(super::super::strip_cells(&prj,&it).len(),1);
    assert_eq!(prj.parts.iter().find(|p|p.number=="UNSIZED").unwrap().width,0.);
}

#[test]
fn single_part_device_views_resolve_by_selected_article_and_reject_ambiguity() {
    let mut prj=demo_project();let (ei,base)=prj.layout.find_device("-K1").map(|(ei,it)|(ei,it.clone())).unwrap();
    let mut symbol=prj.pages.iter().flat_map(|p|&p.symbols).find(|s|s.tag=="-K1").unwrap().clone();
    prj.layout.enclosures[ei].items.retain(|i|i.id!=base.id);
    let mut first=base.clone();first.id=prj.new_id();first.single_part=true;
    let mut second=first.clone();second.id=prj.new_id();second.x+=100.;
    prj.layout.enclosures[ei].items.extend([first,second.clone()]);
    assert!(super::super::locate_symbol(&prj,&symbol).is_none());
    let mut part=super::super::part_by_number(&prj,&base.part).unwrap().clone();part.number="SECOND.PART".into();prj.parts.push(part);
    prj.devices.get_mut("-K1").unwrap().parts.push(crate::parts::PartRef {number:"SECOND.PART".into(),count:1, total:false });
    prj.layout.enclosures[ei].items.last_mut().unwrap().part="SECOND.PART".into();
    symbol.values.insert("function.part_number".into(),"SECOND.PART".into());
    assert_eq!(super::super::locate_symbol(&prj,&symbol).unwrap().1.id,second.id);
}

fn item(prj: &Project, tag: &str) -> PanelItem {
    prj.layout.find_device(tag).unwrap().1.clone()
}

fn pin<'a>(points: &'a [PlacedPoint], name: &str) -> &'a PlacedPoint {
    points.iter().find(|p| p.pin == name && p.terminal.is_empty()).unwrap_or_else(|| panic!("no point {name}"))
}

fn inside(p: &PlacedPoint, it: &PanelItem) -> bool {
    let tol = 0.01;
    p.pos[0] >= it.x - tol && p.pos[0] <= it.x + it.w + tol && p.pos[1] >= it.y - tol && p.pos[1] <= it.y + it.h + tol
        && p.pos[2] >= it.z - tol && p.pos[2] <= it.z + it.d + tol
}

#[test]
fn contactor_with_front_aux_block() {
    let prj = demo_project();
    let it = item(&prj, "-K1");
    let points = device_points(&prj, &it);
    // Contactor A1 A2, 1-6, 13 14 plus the block 53 54 61 62 71 72 83 84.
    assert_eq!(points.len(), 18);
    assert!(points.iter().all(|p| inside(p, &it) && !p.exact));
    let a1 = pin(&points, "A1");
    assert_eq!((a1.pos[1], a1.exit), (it.y, Exit::Up));
    assert_eq!((pin(&points, "A2").pos[1], pin(&points, "A2").exit), (it.y + it.h, Exit::Down));
    // The block sits centred in front of the contactor (74 mm high, block 40 mm).
    let b = pin(&points, "53");
    assert_eq!(b.pos[1], it.y + 17.0);
    assert!(b.pos[2] > a1.pos[2], "in front of the contactor");
}

#[test]
fn rotated_devices_turn_their_points() {
    let mut prj = demo_project();
    let it = item(&prj, "-K1");
    let mut turned = it.clone();
    turned.rotate();
    let (ei, _) = prj.layout.find_device("-K1").unwrap();
    *prj.layout.enclosures[ei].items.iter_mut().find(|i| i.id == it.id).unwrap() = turned.clone();
    let points = device_points(&prj, &turned);
    let a1 = pin(&points, "A1");
    // The top of the contactor now points to the right.
    assert_eq!((a1.pos[0], a1.exit), (turned.x + turned.w, Exit::Right));
    assert_eq!(pin(&points, "A2").exit, Exit::Left);
    assert!(points.iter().all(|p| inside(p, &turned)));
}

#[test]
fn terminal_strips_have_points_per_terminal() {
    let prj = demo_project();
    let it = item(&prj, "-X1");
    let labels: Vec<String> = crate::strips::terminals(&prj, "-X1").into_iter().map(|t| t.label).collect();
    let points = device_points(&prj, &it);
    assert_eq!(points.len(), 2 * labels.len());
    // The terminals sit in their cells of the strip, between brackets and end plates.
    let cells = crate::panel::strip_cells(&prj, &it);
    let layout = crate::strips::strip_layout(&prj, "-X1");
    for label in &labels {
        let i = layout.iter().position(|e| e.kind == crate::strips::ElementKind::Terminal(label.clone())).unwrap();
        let centre = it.x + cells[i].at + cells[i].len / 2.0;
        let top = points.iter().find(|p| &p.terminal == label && p.pin == "1").unwrap();
        let bottom = points.iter().find(|p| &p.terminal == label && p.pin == "2").unwrap();
        assert!((top.pos[0] - centre).abs() < 0.01, "terminal {label} in its cell");
        assert_eq!((top.exit, bottom.exit), (Exit::Up, Exit::Down));
    }
}

#[test]
fn door_devices_are_wired_from_behind() {
    let prj = demo_project();
    let it = item(&prj, "-S3");
    assert_eq!(it.face, Face::Door);
    let points = device_points(&prj, &it);
    assert_eq!(points.len(), 2);
    assert!(points.iter().all(|p| p.exit == Exit::Front && (p.pos[2] - (it.z + it.d)).abs() < 0.01));
}

#[test]
fn own_points_are_placed_exactly() {
    let mut prj = demo_project();
    let part = prj.parts.iter_mut().find(|p| p.number == "GEN.MCB-1P-C6").unwrap();
    part.connection_points = vec![ConnectionPoint { pin: "1".into(), pos: [9.0, 10.0, 50.0], exit: Exit::Left }];
    let it = item(&prj, "-F1");
    let points = device_points(&prj, &it);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].pos, [it.x + 9.0, it.y + 10.0, it.z + 50.0]);
    assert_eq!((points[0].exit, points[0].exact), (Exit::Left, true));
}

#[test]
fn schematic_connection_points_find_their_place() {
    let prj = demo_project();
    let symbols: Vec<&SymbolInst> = prj.pages.iter().flat_map(|pg| pg.symbols.iter()).collect();

    let coil = symbols.iter().find(|s| s.tag == "-K1" && s.sym == "coil").unwrap();
    let points = device_points(&prj, &item(&prj, "-K1"));
    assert_eq!(point_of(&prj, &points, coil, 0).unwrap().pin, "A1");
    assert_eq!(point_of(&prj, &points, coil, 1).unwrap().pin, "A2");

    let input = symbols.iter().find(|s| s.tag == "-A1" && s.label == "I0.2").unwrap();
    let points = device_points(&prj, &item(&prj, "-A1"));
    let at = point_of(&prj, &points, input, 0).unwrap();
    assert_eq!((at.pin.as_str(), at.exit), ("I0.2", Exit::Down), "PLC points by address");

    let terminal = symbols.iter().find(|s| s.tag == "-X1").unwrap();
    let points = device_points(&prj, &item(&prj, "-X1"));
    let bottom = point_of(&prj, &points, terminal, 1).unwrap();
    assert_eq!((bottom.terminal.as_str(), bottom.pin.as_str()), (terminal.label.as_str(), "2"));
}

#[test]
fn nothing_to_place_without_parts() {
    let prj = demo_project();
    let mut it = item(&prj, "-K1");
    it.tag = "-K99".into();
    assert!(device_points(&prj, &it).is_empty());
    it.kind = ItemKind::Duct;
    assert!(device_points(&prj, &item(&prj, "-K1")).len() > 1);
    assert!(device_points(&prj, &it).is_empty());
}

#[test]
fn terminals_use_the_points_of_their_own_part() {
    let mut prj = demo_project();
    let pe = prj.parts.iter_mut().find(|p| p.number == "GEN.TB-PE-2.5").unwrap();
    pe.connection_points = vec![
        ConnectionPoint { pin: "1".into(), pos: [2.6, 4.0, 30.0], exit: Exit::Up },
        ConnectionPoint { pin: "2".into(), pos: [2.6, 44.0, 30.0], exit: Exit::Down },
    ];
    let it = item(&prj, "-X1");
    let points = device_points(&prj, &it);
    let of = |terminal: &str| points.iter().filter(|p| p.terminal == terminal).collect::<Vec<_>>();
    assert!(of("PE").iter().all(|p| p.exact), "PE terminal: points of the PE part");
    assert!(of("1").iter().all(|p| !p.exact), "grey terminal: generated points");
    let top = of("PE").into_iter().find(|p| p.pin == "1").unwrap();
    assert!((top.pos[1] - (it.y + 4.0)).abs() < 0.01);
    // In the cell of the PE terminal.
    let cells = crate::panel::strip_cells(&prj, &it);
    let layout = crate::strips::strip_layout(&prj, "-X1");
    let i = layout.iter().position(|e| e.kind == crate::strips::ElementKind::Terminal("PE".into())).unwrap();
    let centre = it.x + cells[i].at + cells[i].len / 2.0;
    assert!((top.pos[0] - centre).abs() < 0.01, "{} vs {centre}", top.pos[0]);
}


#[test]
fn the_points_of_repeated_parts_are_told_apart_by_their_number() {
    // A measured feeder strip: a current transformer behind each of its three poles,
    // their connection points numbered in the schematic (1S1 ... 3S2).
    let prj = crate::sample::example("industrial_main_distribution").unwrap();
    let it = item(&prj, "=DIST+HV2-Q21");
    let points = device_points(&prj, &it);
    assert_eq!(points.iter().filter(|p| p.pin == "S1").count(), 3);
    let symbol = |pin: &str| prj.pages.iter().flat_map(|p| &p.symbols).find(|s| s.tag == "=DIST+HV2-Q21" && s.pins.iter().any(|x| x == pin)).unwrap_or_else(|| panic!("{pin}"));
    let place = |pin: &str| {
        let s = symbol(pin);
        point_of(&prj, &points, s, s.pins.iter().position(|x| x == pin).unwrap()).unwrap_or_else(|| panic!("no point for {pin}")).pos
    };
    // Each on its own transformer, along the strip.
    let (first, second, third) = (place("1S1"), place("2S1"), place("3S1"));
    assert!(first[1] < second[1] && second[1] < third[1], "{first:?} {second:?} {third:?}");
    assert_eq!((first[0], first[2]), (second[0], second[2]));
    assert_ne!(place("1S1"), place("1S2"));
    // The connection points of the strip itself keep their own.
    assert_eq!(place("2"), pin(&points, "2").pos);
}

#[test]
fn a_strip_placed_whole_twice_resolves_to_its_first_placement() {
    let mut prj=demo_project();let whole=item(&prj,"-X1");
    let terminals=crate::strips::terminals(&prj,"-X1");
    let source=prj.pages[terminals[0].page].symbols[terminals[0].sym].clone();
    let ei=prj.layout.find_device("-X1").unwrap().0;
    let mut second=whole.clone();second.id=prj.new_id();second.x+=200.;
    prj.layout.enclosures[ei].items.push(second);
    assert_eq!(super::super::locate_symbol(&prj,&source).unwrap().1.id,whole.id);
}
