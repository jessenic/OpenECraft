use super::*;
use crate::geom::p;
use crate::model::{SymbolInst, Wire};
use crate::symlib::{UserPin, UserSymbol};

#[test]
fn a_loop_can_return_to_another_port_of_the_starting_junction() {
    let pin=PinRef {page:0,sym:1,pin:0};
    let sites=vec![
        Site {page:0,points:vec![p(0.,0.);3],links:vec![(0,1),(1,2)],arms:Vec::new()},
        Site {page:0,points:vec![p(0.,0.);2],links:vec![(0,1)],arms:Vec::new()},
    ];
    let context=Context {
        sites:&sites,
        through:HashMap::from([((0,0),vec![(1,0)]),((1,1),vec![(0,1)])]),
        reached:HashMap::from([((0,2),vec![pin])]),
    };
    assert_eq!(context.beyond(0,0),vec![pin]);
}

#[test]
fn physical_targets_stop_at_the_first_device_pin_on_each_branch() {
    let a=PinRef {page:0,sym:1,pin:0};let b=PinRef {page:0,sym:2,pin:0};let c=PinRef {page:0,sym:3,pin:0};
    let graph=graph_of(5,&[(0,1,1,None),(1,2,1,None),(2,3,1,None),(1,4,1,None)]);
    let pins=HashMap::from([(2,vec![a]),(3,vec![b]),(4,vec![c])]);
    assert_eq!(reachable_pins(&graph,&pins,0),vec![a,c]);
}

#[test]
fn drawn_targets_preserve_a_branch_at_a_device_connection_point() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:point".into(),pins:vec![UserPin::default()],..Default::default()});
    for (id,pos) in [(1,p(40.,40.)),(2,p(80.,40.)),(3,p(100.,20.))] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:"user:point".into(),tag:format!("-A{id}"),pos,..Default::default()});
    }
    for (i,(a,b)) in [(p(40.,40.),p(80.,40.)),(p(40.,40.),p(40.,20.)),(p(40.,20.),p(100.,20.))].into_iter().enumerate() {
        prj.pages[0].wires.push(Wire {id:10+i as u64,a,b,..Default::default()});
    }
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,2),(2,3)]));
    prj.pages[0].values.insert("page.use_drawn_wire_targets".into(),"true".into());
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,2),(1,3)]));
}

#[test]
fn a_chain_of_directed_junctions_keeps_the_common_source() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:bus".into(),func:Func::Junction,directed_junction:true,
        pins:[p(-5.,0.),p(5.,0.),p(0.,5.)].into_iter().map(|pos|UserPin {pos,..Default::default()}).collect(),
        bridges:vec![(0,1),(0,2)],..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {id:1,sym:"coil".into(),tag:"-K1".into(),pos:p(20.,100.),rot:1,..Default::default()});
    for (i,x) in [100.,180.,260.].into_iter().enumerate() {
        prj.pages[0].symbols.push(SymbolInst {id:2+i as u64,sym:"coil".into(),tag:format!("-K{}",i+2),pos:p(x,180.),..Default::default()});
        prj.pages[0].symbols.push(SymbolInst {id:10+i as u64,sym:"user:bus".into(),pos:p(x,100.),..Default::default()});
    }
    prj.pages[0].symbols.push(SymbolInst {id:5,sym:"coil".into(),tag:"-K5".into(),pos:p(340.,100.),rot:1,..Default::default()});
    let segments=[(p(24.,100.),p(95.,100.)),(p(105.,100.),p(175.,100.)),(p(185.,100.),p(255.,100.)),(p(265.,100.),p(336.,100.)),
                  (p(100.,105.),p(100.,176.)),(p(180.,105.),p(180.,176.)),(p(260.,105.),p(260.,176.))];
    for (i,(a,b)) in segments.into_iter().enumerate() {prj.pages[0].wires.push(Wire {id:100+i as u64,a,b,..Default::default()});}
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,2),(1,3),(1,4),(1,5)]));
}

#[test]
fn cyclic_target_paths_have_a_bounded_traversal() {
    let sites:Vec<_>=(0..16).map(|_|Site {page:0,points:vec![p(0.,0.);3],links:vec![(0,1),(0,2)],arms:Vec::new()}).collect();
    let all:Vec<_>=(0..sites.len()).flat_map(|site|(0..3).map(move |port|(site,port))).collect();
    let context=Context {sites:&sites,through:all.iter().map(|p|(*p,all.clone())).collect(),reached:HashMap::new()};
    assert!(context.beyond(0,0).is_empty());
}

#[test]
fn coincident_pin_records_keep_electrical_membership_without_a_phantom_wire() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:alias".into(),pins:vec![UserPin::default();2],..Default::default()});
    prj.pages[0].symbols=vec![
        SymbolInst {id:1,sym:"user:alias".into(),tag:"-A1".into(),pos:p(20.,40.),..Default::default()},
        SymbolInst {id:2,sym:"coil".into(),tag:"-K1".into(),pos:p(20.,80.),..Default::default()},
    ];
    prj.pages[0].wires.push(Wire {id:10,a:p(20.,40.),b:p(20.,76.),..Default::default()});
    let (nl,open)=(Netlist::build(&prj,true),Netlist::build(&prj,false));
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(1,1)]);
    let physical=connections(&prj,&nl,&open);
    assert_eq!(physical.len(),1);
    assert_eq!((physical[0].a.sym,physical[0].a.pin,physical[0].b.sym),(0,0,1));
}

#[test]
fn internal_component_bridges_are_not_external_wires_or_shared_cable_properties() {
    let mut prj = Project::default();
    prj.symbols.push(UserSymbol { key: "user:internal".into(),func:Func::Main {mirror:false},
        pins:[p(-10.,0.),p(10.,0.)].into_iter().map(|pos|UserPin {pos,..Default::default()}).collect(),
        bridges:vec![(0,1)],..Default::default() });
    prj.pages[0].symbols=vec![
        SymbolInst {id:1,sym:"coil".into(),tag:"-K1".into(),pos:p(20.,100.),rot:1,..Default::default()},
        SymbolInst {id:2,sym:"user:internal".into(),tag:"-A1".into(),pos:p(100.,100.),..Default::default()},
        SymbolInst {id:3,sym:"coil".into(),tag:"-K2".into(),pos:p(180.,100.),rot:1,..Default::default()},
    ];
    prj.pages[0].wires=vec![
        Wire {id:10,a:p(24.,100.),b:p(90.,100.),cable:"-W1".into(),core:"BN".into(),color:"BU".into(),number:"101".into(),..Default::default()},
        Wire {id:11,a:p(110.,100.),b:p(176.,100.),cable:"-W2".into(),core:"BK".into(),color:"RD".into(),number:"102".into(),..Default::default()},
    ];
    let (nl,open)=(Netlist::build(&prj,true),Netlist::build(&prj,false));
    assert_eq!(nl.pin_net[&(1,0)],nl.pin_net[&(3,1)]);
    let connections=connections(&prj,&nl,&open);
    assert_eq!(connections.len(),2);
    assert!(!connections.iter().any(|c|c.a.sym==1 && c.b.sym==1));
    let properties:HashSet<_>=connections.iter().map(|c|(c.cable.as_str(),c.core.as_str(),c.color.as_str(),c.number.as_str())).collect();
    assert_eq!(properties,HashSet::from([("-W1","BN","BU","101"),("-W2","BK","RD","102")]));
}

fn star_project()->Project {
    let mut prj = Project::default();
    prj.symbols.push(UserSymbol { key: "user:star".into(), func: Func::Junction, directed_junction: true,
        pins: [p(-5.,0.),p(0.,-5.),p(5.,0.)].into_iter().map(|pos| UserPin { pos,..Default::default() }).collect(),
        bridges: vec![(0,1),(0,2)],..Default::default() });
    prj.pages[0].symbols = vec![
        SymbolInst { id:1,sym:"coil".into(),tag:"-K1".into(),pos:p(20.,100.),rot:1,..Default::default() },
        SymbolInst { id:2,sym:"coil".into(),tag:"-K2".into(),pos:p(100.,20.),..Default::default() },
        SymbolInst { id:3,sym:"coil".into(),tag:"-K3".into(),pos:p(180.,100.),rot:1,..Default::default() },
        SymbolInst { id:4,sym:"user:star".into(),pos:p(100.,100.),..Default::default() },
    ];
    for (i,(a,b)) in [(p(24.,100.),p(95.,100.)),(p(100.,24.),p(100.,95.)),(p(105.,100.),p(176.,100.))].into_iter().enumerate() {
        prj.pages[0].wires.push(Wire {id:10+i as u64,a,b,..Default::default()});
    }
    prj
}

fn endpoint_pairs(prj:&Project)->HashSet<(u64,u64)> {
    let (nl,open)=(Netlist::build(prj,true),Netlist::build(prj,false));
    connections(prj,&nl,&open).iter().map(|c| {
        let a=prj.pages[c.a.page].symbols[c.a.sym].id;
        let b=prj.pages[c.b.page].symbols[c.b.sym].id;
        (a.min(b),a.max(b))
    }).collect()
}

#[test]
fn imported_paths_combine_with_new_native_t_junctions() {
    let mut prj=star_project();
    prj.pages[0].symbols.push(SymbolInst {id:5,sym:"coil".into(),tag:"-K4".into(),pos:p(60.,180.),..Default::default()});
    prj.pages[0].wires.push(Wire {id:20,a:p(60.,100.),b:p(60.,176.),..Default::default()});
    prj.pages[0].junctions.push(Junction {id:21,..Junction::new(crate::model::JunctionKind::Tee,p(60.,100.))});
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,5),(2,5),(3,5)]));
}

#[test]
fn spaced_junction_ports_preserve_the_authored_fanout_after_reopen() {
    let mut prj = Project::from_json(&star_project().to_json()).unwrap();
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,2),(1,3)]));
    let before = crate::symlib::fingerprint(&prj.symbols);
    prj.symbols[0].directed_junction = false;
    assert_ne!(before,crate::symlib::fingerprint(&prj.symbols));
    assert_eq!(endpoint_pairs(&prj),HashSet::from([(1,2),(2,3)]));
}
