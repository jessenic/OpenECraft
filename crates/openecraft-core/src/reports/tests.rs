use super::*;
use crate::sample::demo_project;

#[test]
fn metered_cable_unknown_lengths_keep_partial_material_totals_uncertain() {
    use crate::parts::Part;
    let mut prj=Project::default();
    prj.marking.cable_material.clear();
    prj.parts.push(Part {number:"BULK".into(),quantity_unit:"m".into(),price:2.,..Default::default()});
    prj.parts.push(Part {number:"UNKNOWN".into(),quantity_unit:"m".into(),price:4.,..Default::default()});
    prj.parts.push(Part {number:"PIECE".into(),quantity_unit:"pcs".into(),price:3.,..Default::default()});
    prj.cables=vec![Cable {id:1,tag:"-W1".into(),part:"BULK".into(),length:2.5,..Default::default()},
        Cable {id:2,tag:"-W2".into(),part:"BULK".into(),..Default::default()},
        Cable {id:3,tag:"-W3".into(),part:"UNKNOWN".into(),..Default::default()},
        Cable {id:4,tag:"-W4".into(),part:"PIECE".into(),..Default::default()}];
    let table=bom(&prj,&Index::build(&prj),&[],&[]);
    let bulk=table.rows.iter().find(|r|r[3]=="BULK").unwrap();
    assert_eq!((bulk[1].as_str(),bulk[2].as_str(),bulk[9].as_str()),("2.50 + ?","m","5.00 + ?"));
    assert!(bulk[7].contains("-W1") && bulk[7].contains("-W2"));
    let unknown=table.rows.iter().find(|r|r[3]=="UNKNOWN").unwrap();
    assert_eq!((unknown[1].as_str(),unknown[9].as_str()),("?","?"));
    let piece=table.rows.iter().find(|r|r[3]=="PIECE").unwrap();
    assert_eq!((piece[1].as_str(),piece[9].as_str()),("1","3.00"));
    assert_eq!(table.rows.last().unwrap()[9],"8.00 + ?");
    prj.cables[1].length=1.5;prj.cables[2].length=3.;
    let table=bom(&prj,&Index::build(&prj),&[],&[]);
    assert_eq!(table.rows.iter().find(|r|r[3]=="BULK").unwrap()[1],"4");
    assert_eq!(table.rows.last().unwrap()[9],"23.00");
}

#[test]
fn metered_cable_length_check_names_unknown_cables_and_accepts_configured_lengths() {
    let mut prj=Project::default();
    prj.cables.push(Cable {id:1,tag:"-W1".into(),part:"GEN.CBL-4G1.5".into(),..Default::default()});
    let check=crate::checks::builtin().into_iter().find(|c|c.name=="wires").unwrap();
    let issues=(check.run)(&prj,&Analysis::build(&prj)).unwrap();
    assert!(issues.iter().any(|i|i.message().contains("-W1: cable length is missing") && i.severity==Severity::Warning));
    let table=bom(&prj,&Index::build(&prj),&[],&[]);
    assert_eq!(table.rows.iter().find(|r|r[3]=="GEN.CBL-4G1.5").unwrap()[1],"?");
    prj.cables[0].length=25.;
    assert!(!(check.run)(&prj,&Analysis::build(&prj)).unwrap().iter().any(|i|i.message().contains("length is missing")));
    let reopened:Project=serde_json::from_str(&serde_json::to_string(&prj).unwrap()).unwrap();
    assert_eq!(material_quantity(&reopened,"GEN.CBL-4G1.5"),25.);
}

fn material_quantity(prj:&Project,number:&str)->f32 {
    bom(prj,&Index::build(prj),&[],&[]).rows.iter().find(|r|r[3]==number).map(|r|r[1].parse().unwrap()).unwrap_or(0.)
}

#[test]
fn cabinet_only_device_material_is_independent_of_additional_articles() {
    use crate::parts::{Part,PartRef};use crate::panel::{Enclosure,PanelItem,ItemKind};
    let mut prj=Project::default();prj.layout.hardware=false;
    prj.parts.push(Part {number:"GLAND".into(),..Default::default()});
    prj.devices.entry("-S1".into()).or_default().parts.push(PartRef {number:"GLAND".into(),count:1,total:false});
    prj.layout.enclosures.push(Enclosure {no_implicit_plate:true,items:vec![PanelItem {id:1,kind:ItemKind::Device,
        tag:"-S1".into(),part:"GLAND".into(),single_part:true,w:20.,h:30.,..Default::default()}],..Default::default()});
    assert_eq!(material_quantity(&prj,"GLAND"),1.);
    prj.devices.get_mut("-S1").unwrap().parts.push(PartRef {number:"GLAND".into(),count:0,total:true});
    assert_eq!(material_quantity(&prj,"GLAND"),1.);
    prj.devices.get_mut("-S1").unwrap().parts.pop();assert_eq!(material_quantity(&prj,"GLAND"),1.);
    let mut duplicate=prj.layout.enclosures[0].items[0].clone();duplicate.id=2;
    prj.layout.enclosures[0].items.push(duplicate);assert_eq!(material_quantity(&prj,"GLAND"),1.,"duplicate views are one logical device");
}

#[test]
fn cabinet_terminals_and_new_schematic_terminals_share_one_primary_quantity() {
    let mut prj=demo_project();prj.layout.hardware=false;
    let number=crate::strips::terminal_part(&prj,"-X1",false).unwrap().number.clone();
    let terminal=crate::strips::terminals(&prj,"-X1").into_iter().find(|t|t.label=="1").unwrap();
    let mut symbol=prj.pages[terminal.page].symbols[terminal.sym].clone();
    let before=material_quantity(&prj,&number);
    let mut item=prj.layout.find_device("-X1").unwrap().1.clone();item.single_part=true;item.terminal="1".into();
    prj.layout.enclosures[0].items.retain(|i|i.tag!="-X1");prj.layout.enclosures[0].items.push(item.clone());
    assert_eq!(material_quantity(&prj,&number),before,"schematic and cabinet terminal must collapse");
    item.id=prj.new_id();item.terminal="99".into();prj.layout.enclosures[0].items.push(item.clone());
    item.id=prj.new_id();prj.layout.enclosures[0].items.push(item);
    assert_eq!(material_quantity(&prj,&number),before+1.,"one undrawn terminal despite duplicate physical views");
    symbol.id=prj.new_id();symbol.label="99".into();prj.pages[terminal.page].symbols.push(symbol);
    assert_eq!(material_quantity(&prj,&number),before+1.,"drawing the cabinet terminal must not add a second piece");
}

#[test]
fn excluded_physical_duplicates_keep_geometry_and_material_owner_counts() {
    use crate::parts::{Part,PartRef};use crate::panel::{Enclosure,PanelItem,ItemKind};
    let mut prj=Project::default();prj.layout.hardware=false;
    prj.parts.push(Part {number:"END".into(),..Default::default()});
    prj.devices.entry("-X1".into()).or_default().parts.push(PartRef {number:"END".into(),count:2,total:true});
    let child=prj.devices.entry("-X1_1".into()).or_default();
    child.parts.push(PartRef {number:"END".into(),count:1,total:false});
    child.values.insert("device.material_owner".into(),"-X1".into());
    child.values.insert("device.exclude_from_bom".into(),"true".into());
    prj.layout.enclosures.push(Enclosure {no_implicit_plate:true,items:vec![PanelItem {id:1,kind:ItemKind::Accessory,
        tag:"-X1_1".into(),owner_device:"-X1".into(),bom_excluded:true,part:"END".into(),w:20.,h:30.,..Default::default()}],..Default::default()});
    assert_eq!(material_quantity(&prj,"END"),2.);
    prj.devices.get_mut("-X1_1").unwrap().values.remove("device.exclude_from_bom");
    assert_eq!(material_quantity(&prj,"END"),3.,"placement exclusion must not suppress the logical assignment");
    prj.layout.enclosures[0].items[0].bom_excluded=false;
    assert_eq!(material_quantity(&prj,"END"),3.,"one physical primary must replace its logical primary");
    prj.devices.get_mut("-X1").unwrap().values.insert("device.exclude_from_bom".into(),"true".into());
    assert_eq!(material_quantity(&prj,"END"),1.);
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(reopened.layout.enclosures[0].items[0].owner_device,"-X1");
    assert_eq!(reopened.layout.enclosures[0].items[0].w,20.);
    assert_eq!(material_quantity(&reopened,"END"),1.);
}

#[test]
fn literal_terminal_extras_do_not_multiply_with_the_strip_and_new_terminals_still_count() {
    let mut prj=demo_project();prj.layout.hardware=false;
    let number=crate::strips::terminal_part(&prj,"-X1",false).unwrap().number.clone();
    let before=material_quantity(&prj,&number);
    prj.devices.get_mut("-X1").unwrap().parts.push(crate::parts::PartRef {number:number.clone(),count:2,total:true});
    assert_eq!(material_quantity(&prj,&number),before+2.);
    let terminal=crate::strips::terminals(&prj,"-X1").into_iter().find(|t|t.label=="1").unwrap();
    let mut symbol=prj.pages[terminal.page].symbols[terminal.sym].clone();symbol.id=prj.new_id();symbol.label="99".into();
    prj.pages[terminal.page].symbols.push(symbol);
    assert_eq!(material_quantity(&prj,&number),before+3.);
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    assert!(reopened.devices["-X1"].parts.last().unwrap().total);
    assert_eq!(material_quantity(&reopened,&number),before+3.);
}

#[test]
fn physical_primary_articles_count_once_and_literal_only_devices_and_zero_quantities_are_retained() {
    use crate::parts::{Part,PartRef};use crate::panel::{Enclosure,EnclosureKind,PanelItem,ItemKind};
    let mut prj=Project::default();prj.layout.hardware=false;
    prj.parts.extend([Part {number:"BODY".into(),..Default::default()},Part {number:"PLATE".into(),..Default::default()},Part {number:"EXTRA".into(),..Default::default()},Part {number:"ZERO".into(),..Default::default()}]);
    for (tag,part) in [("-U1","BODY"),("-U2","PLATE")] {
        prj.devices.entry(tag.into()).or_default().parts.extend([PartRef {number:part.into(),count:1,total:false},PartRef {number:part.into(),count:3,total:true}]);
    }
    prj.devices.entry("-OnlyMaterial".into()).or_default().parts.push(PartRef {number:"EXTRA".into(),count:4,total:true});
    prj.devices.entry("-OnlyMaterial".into()).or_default().parts.push(PartRef {number:"ZERO".into(),count:0,total:true});
    prj.layout.enclosures.push(Enclosure {device_tag:"-U1".into(),part:"BODY".into(),no_implicit_plate:true,
        items:vec![PanelItem {id:1,kind:ItemKind::MountingPlate,tag:"-U2".into(),part:"PLATE".into(),w:300.,h:400.,d:2.,..Default::default()}],..Default::default()});
    assert_eq!(material_quantity(&prj,"BODY"),4.);assert_eq!(material_quantity(&prj,"PLATE"),4.);
    assert_eq!(material_quantity(&prj,"EXTRA"),4.);assert_eq!(material_quantity(&prj,"ZERO"),0.);
    prj.layout.enclosures[0].kind=EnclosureKind::Assembly;prj.layout.enclosures[0].part.clear();
    assert_eq!(material_quantity(&prj,"BODY"),4.,"unplaced primary plus literal extras remain counted when no body owns the article");
}

fn row<'a>(t: &'a Table, first: &str) -> Vec<&'a Vec<String>> {
    t.rows.iter().filter(|r| r[0] == first).collect()
}

#[test]
fn cable_reports_of_the_demo() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let overview = &an.report(&prj, ReportKind::CableOverview).unwrap();
    let w1 = row(overview, "-W1");
    assert_eq!(w1.len(), 1);
    assert_eq!(w1[0][2], "4/4");
    assert_eq!((w1[0][4].as_str(), w1[0][5].as_str(), w1[0][6].as_str()), ("15", "-X1", "-M1"));

    let diagram = &an.report(&prj, ReportKind::CableDiagram).unwrap();
    let cores: Vec<(&str, &str, &str)> = row(diagram, "-W1").iter().map(|r| (r[1].as_str(), r[2].as_str(), r[3].as_str())).collect();
    assert_eq!(cores, vec![("BN", "-X1:1", "-M1:U1"), ("BK", "-X1:2", "-M1:V1"), ("GY", "-X1:3", "-M1:W1"), ("GNYE", "-X1:PE", "-M1:PE")]);
    assert!(row(diagram, "-W1").iter().all(|r| !r[5].is_empty()), "wire numbers");

    // Connection list: wire numbers and cable cores, terminal sides apart.
    let conns = &an.report(&prj, ReportKind::ConnectionList).unwrap();
    let motor = conns.rows.iter().find(|r| r[3] == "-M1:U1").unwrap();
    assert_eq!((motor[2].as_str(), motor[7].as_str(), motor[8].as_str()), ("-X1:1", "-W1", "BN"));
    assert!(conns.rows.iter().all(|r| !r[0].is_empty()), "every connection is numbered");
    assert!(conns.rows.iter().all(|r| r[2] != r[3]), "no connection from a terminal to itself");

    // The cable is in the bill of materials by length.
    let bom = &an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    let cable = bom.rows.iter().find(|r| r[3] == "GEN.CBL-4G1.5").unwrap();
    assert_eq!((cable[1].as_str(), cable[2].as_str(), cable[7].as_str()), ("15", "m", "-W1"));
}

fn messages(prj: &Project) -> Vec<String> {
    Analysis::checked(prj).issues.into_iter().map(|i| i.message()).collect()
}

fn find<'a>(msgs: &'a [String], part: &str) -> Option<&'a String> {
    msgs.iter().find(|m| m.contains(part))
}

#[test]
fn cable_assignments_are_checked() {
    let mut prj = demo_project();
    let page = prj.pages.iter().position(|p| p.name == "2").unwrap();
    let cabled: Vec<usize> = prj.pages[page].wires.iter().enumerate().filter(|(_, w)| w.cable == "-W1").map(|(i, _)| i).collect();
    // Same core on two connections, a core the cable does not have, an unknown cable.
    prj.pages[page].wires[cabled[1]].core = "BN".into();
    prj.pages[page].wires[cabled[2]].core = "RD".into();
    prj.pages[page].wires[cabled[3]].cable = "-W7".into();
    let msgs = messages(&prj);
    assert!(find(&msgs, "-W1: core BN used for 2 connections").is_some(), "{msgs:#?}");
    assert!(find(&msgs, "core RD is not a core of the cable").is_some(), "{msgs:#?}");
    assert!(find(&msgs, "unknown cable -W7").is_some(), "{msgs:#?}");
    prj.cables.push(prj.cables[0].clone());
    assert!(find(&messages(&prj), "Cable -W1 defined 2 times").is_some());
}

#[test]
fn wire_numbers_and_tags_are_checked() {
    let mut prj = demo_project();
    let open = Netlist::build(&prj, false);
    let bridged = Netlist::build(&prj, true);
    let numbered: Vec<&crate::netlist::Net> = open.nets.iter().filter(|n| n.wires.len() > 1 && !n.pins.is_empty()).collect();
    // Two numbers in one connection
    let (pi, id) = numbered[0].wires[0];
    prj.pages[pi].wires.iter_mut().find(|w| w.id == id).unwrap().number = "X99".into();
    assert!(find(&messages(&prj), "Different wire numbers in one connection").is_some());
    // The same number on two potentials
    let pot = |n: &crate::netlist::Net| {
        let p = n.pins[0];
        bridged.pin_net[&(prj.pages[p.page].symbols[p.sym].id, p.pin)]
    };
    let other = open.nets.iter().find(|n| !n.wires.is_empty() && !n.pins.is_empty() && pot(n) != pot(numbered[1])).unwrap().clone();
    let number = crate::wirenum::number_of(&prj, numbered[1]);
    crate::wirenum::set(&mut prj, &other, &number, true);
    assert!(find(&messages(&prj), &format!("Wire number {number} is used for 2 different potentials")).is_some());
    // A tag without device name
    prj.pages[2].symbols[0].tag = "=A1".into();
    assert!(find(&messages(&prj), "=A1: device tag without device name").is_some());
}

#[test]
fn mounting_pages_without_enclosure_are_reported() {
    let mut prj = demo_project();
    assert!(find(&messages(&prj), "no longer exists").is_none());
    prj.layout.enclosures.clear();
    let msgs = messages(&prj);
    let page = prj.pages.iter().find(|p| matches!(p.kind, PageKind::Mounting(_))).unwrap();
    assert_eq!(find(&msgs, "no longer exists").unwrap(), &format!("Page {}: its enclosure no longer exists", page.name));
}


#[test]
fn assemblies_list_their_components_in_the_bom() {
    let mut prj = demo_project();
    let number = prj.devices.values().find_map(|d| d.parts.first().map(|r| r.number.clone())).unwrap();
    let comp = |n: &str, q: f32| crate::parts::Component { number: n.into(), quantity: q };
    let part = prj.parts.iter_mut().find(|p| p.number == number).unwrap();
    part.components = vec![comp("GEN.SCREW-M4", 4.0)];
    let an = crate::analysis::Analysis::checked(&prj);
    let bom = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    let main = bom.rows.iter().position(|r| r[3] == number).unwrap();
    let sub = &bom.rows[main + 1];
    assert_eq!(sub[0], format!("{}.1", bom.rows[main][0]));
    assert_eq!(sub[3], "GEN.SCREW-M4");
    let qty: f32 = bom.rows[main][1].parse().unwrap();
    assert_eq!(sub[1].parse::<f32>().unwrap(), qty * 4.0);
    assert_eq!(sub[7], format!("in {number}"));
}
