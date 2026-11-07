use super::*;
use crate::geom::p;
use crate::model::{Junction,JunctionKind,TeeDir,SymbolInst,Wire};
use crate::symlib::{UserPin,UserSymbol};

fn project()->Project {
    let mut prj=Project::default();
    prj.pages[0].values.insert("page.explicit_wire_junctions".into(),"true".into());
    prj.symbols.push(UserSymbol {key:"user:up".into(),pins:vec![UserPin::default()],pin_directions:[(0,1)].into(),..Default::default()});
    for (id,pos) in [(1,p(0.,10.)),(2,p(20.,0.)),(3,p(20.,10.)),(4,p(100.,0.)),(5,p(100.,20.))] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:"user:up".into(),tag:format!("-A{id}"),pos,..Default::default()});
    }
    for (id,a,b) in [(10,p(0.,10.),p(100.,10.)),(11,p(20.,0.),p(20.,10.)),(12,p(100.,0.),p(100.,20.))] {
        prj.pages[0].wires.push(Wire {id,a,b,..Default::default()});
    }
    prj
}

#[test]
fn a_wire_passing_the_side_of_a_directional_port_stays_separate() {
    let prj=project();let nl=Netlist::build(&prj,true);
    assert_eq!(nl.pin_net[&(2,0)],nl.pin_net[&(3,0)]);
    assert_eq!(nl.pin_net[&(4,0)],nl.pin_net[&(5,0)]);
    assert_ne!(nl.pin_net[&(1,0)],nl.pin_net[&(3,0)]);
    assert_ne!(nl.pin_net[&(1,0)],nl.pin_net[&(4,0)]);
    assert!(junctions(&prj,&prj.pages[0]).is_empty());
    assert!(open_wire_ends(&prj,&prj.pages[0]).iter().any(|(id,q)|*id==10 && q.same(p(100.,10.))));
}

#[test]
fn an_explicitly_drawn_endpoint_can_connect_against_a_ports_automatic_direction() {
    let prj=project();let nl=Netlist::build(&prj,true);
    // Both ports face up; the saved conductor starts at the upper port and ends at
    // the lower one. Direction restricts side attachments, not authored endpoints.
    assert_eq!(nl.pin_net[&(2,0)],nl.pin_net[&(3,0)]);
}

#[test]
fn an_explicit_wire_passes_an_intermediate_device_port_without_a_phantom_attachment() {
    let mut prj=project();
    prj.pages[0].symbols.push(SymbolInst {id:6,sym:"user:up".into(),tag:"-A6".into(),pos:p(100.,10.),..Default::default()});
    let nl=Netlist::build(&prj,false);
    // This port is at the bare crossing, in the middle of wire12. Only wire10
    // has an authored endpoint here, so the port attaches to that wire.
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(6,0)]);
    assert_ne!(nl.pin_net[&(4,0)],nl.pin_net[&(6,0)]);
}

#[test]
fn continuing_a_drawing_with_a_junction_makes_the_crossing_electrical() {
    let mut prj=project();
    prj.pages[0].junctions.push(Junction {id:30,..Junction::new(JunctionKind::Crossing,p(100.,10.))});
    let nl=Netlist::build(&prj,true);
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(4,0)]);
    assert_ne!(nl.pin_net[&(1,0)],nl.pin_net[&(3,0)]);
    assert!(junctions(&prj,&prj.pages[0]).iter().any(|q|q.same(p(100.,10.))));
    assert!(!open_wire_ends(&prj,&prj.pages[0]).iter().any(|(id,q)|*id==10 && q.same(p(100.,10.))));
}

#[test]
fn a_new_t_junction_connects_the_branch_to_an_unsplit_imported_wire() {
    let mut prj=project();
    prj.pages[0].values.insert("page.use_drawn_wire_targets".into(),"true".into());
    prj.pages[0].junctions.push(Junction {id:30,dir:TeeDir::Left,..Junction::new(JunctionKind::Tee,p(100.,10.))});
    let (nl,open)=(Netlist::build(&prj,true),Netlist::build(&prj,false));
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(4,0)]);
    let targets:std::collections::HashSet<_>=connections(&prj,&nl,&open).iter().map(|c| {
        let a=prj.pages[c.a.page].symbols[c.a.sym].id;let b=prj.pages[c.b.page].symbols[c.b.sym].id;
        (a.min(b),a.max(b))
    }).collect();
    assert!(targets.contains(&(1,4)) && targets.contains(&(1,5)));
}

#[test]
fn native_pages_keep_implicit_junctions_and_directionless_pin_contacts() {
    let mut prj=project();prj.pages[0].values.clear();prj.symbols[0].pin_directions.clear();
    let nl=Netlist::build(&prj,true);
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(3,0)]);
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(4,0)]);
}

#[test]
fn pin_directions_follow_rotation_and_mirroring_after_serialization() {
    let prj=Project::from_json(&project().to_json()).unwrap();let def=prj.symbol("user:up").unwrap();
    assert_eq!(def.pin_connection_directions(0,0,false),1);
    assert_eq!(def.pin_connection_directions(0,1,false),2);
    assert_eq!(def.pin_connection_directions(0,1,true),2);
    assert_eq!(def.pin_connection_directions(0,3,false),8);
}
