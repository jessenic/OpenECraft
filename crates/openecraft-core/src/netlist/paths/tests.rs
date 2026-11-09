use super::*;
use crate::geom::p;
use crate::model::{SymbolInst, Wire};
use crate::symlib::{UserPin, UserSymbol};

#[test]
fn parallel_core_routes_reach_connections_and_regenerated_cable_reports_without_spurs() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:point".into(),pins:vec![UserPin::default()],..Default::default()});
    for (id,x) in [(1,-10.),(2,50.)] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:"user:point".into(),tag:format!("-A{id}"),pos:p(x,0.),..Default::default()});
    }
    for (id,a,b) in [(8,p(-10.,0.),p(0.,0.)),(9,p(40.,0.),p(50.,0.))] {
        prj.pages[0].wires.push(Wire {id,a,b,color:"RD".into(),explicit_junctions:Some(true),..Default::default()});
    }
    for (id,a,b,core) in [(10,p(0.,0.),p(40.,0.),"OG"),(11,p(0.,0.),p(0.,20.),""),
                         (12,p(0.,20.),p(40.,20.),"WH"),(13,p(40.,20.),p(40.,0.),""),
                         (14,p(0.,0.),p(-20.,0.),""),(15,p(-20.,0.),p(-20.,20.),"BU")] {
        prj.pages[0].wires.push(Wire {id,a,b,explicit_junctions:Some(true),
            cable:if core.is_empty() {String::new()} else {"-W1".into()},core:core.into(),color:core.into(),..Default::default()});
    }
    prj.cables.push(crate::model::Cable {id:90,tag:"-W1".into(),cores:"OG WH BU".into(),..Default::default()});
    let nl=Netlist::build(&prj,true);let open=Netlist::build(&prj,false);let connections=connections(&prj,&nl,&open);
    assert_eq!(connections.len(),2);
    let by_core:std::collections::BTreeMap<_,_>=connections.iter().map(|c|(c.core.as_str(),c)).collect();
    assert_eq!(by_core.keys().copied().collect::<Vec<_>>(),["OG","WH"]);
    assert_eq!(by_core["OG"].wires,[(0,8),(0,10),(0,9)]);
    assert_eq!(by_core["WH"].wires,[(0,8),(0,11),(0,12),(0,13),(0,9)]);
    assert_eq!(by_core["OG"].color,"OG");assert_eq!(by_core["WH"].color,"WH");
    assert!(connections.iter().all(|c|c.route_traced && c.cable=="-W1"));
    let analysis=crate::analysis::Analysis::with_builtin(&prj);
    let report=analysis.report(&prj,crate::model::ReportKind::CableConnections).unwrap();
    for core in ["OG","WH"] {
        assert!(report.rows.iter().any(|row|row[0]=="-W1" && row[4]==core && !row[3].is_empty() && !row[5].is_empty()));
    }
}

#[test]
fn local_route_keeps_the_exact_interruption_endpoint_without_named_shortcuts() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:point".into(),pins:vec![UserPin::default()],..Default::default()});
    prj.symbols.push(UserSymbol {key:"user:arrow".into(),func:Func::Interruption,pins:vec![UserPin::default()],..Default::default()});
    prj.pages[0].values.insert("page.explicit_wire_junctions".into(),"true".into());
    for (id,pos,key) in [(1,p(0.,0.),"user:point"),(2,p(10.,0.),"user:arrow"),(3,p(100.,0.),"user:arrow"),(4,p(110.,0.),"user:point")] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:key.into(),pos,label:"PE".into(),..Default::default()});
    }
    for (id,a,b) in [(10,p(0.,0.),p(10.,0.)),(11,p(0.,0.),p(0.,20.)),(12,p(0.,20.),p(100.,20.)),(13,p(100.,20.),p(100.,0.)),(14,p(100.,0.),p(110.,0.))] {
        prj.pages[0].wires.push(Wire {id,a,b,..Default::default()});
    }
    let open=Netlist::build(&prj,false);
    assert_eq!(LocalRoutes::new(&prj,&open).route((1,0),(3,0)),Some(vec![(0,11),(0,12),(0,13)]));
    prj.pages[0].wires.retain(|w|w.id!=12);
    let open=Netlist::build(&prj,false);
    assert_eq!(open.connection_point_net[&(1,0)],open.connection_point_net[&(3,0)]);
    assert_eq!(LocalRoutes::new(&prj,&open).route((1,0),(3,0)),None);
    assert_eq!(LocalRoutes::new(&prj,&open).route((1,99),(3,0)),None);
}

#[test]
fn a_loop_can_return_to_another_port_of_the_starting_junction() {
    let pin=PinRef {page:0,sym:1,pin:0};
    let sites=vec![
        Site {page:0,symbol:Some(100),points:vec![p(0.,0.);3],links:vec![(0,1),(1,2)],arms:Vec::new()},
        Site {page:0,symbol:Some(100),points:vec![p(0.,0.);2],links:vec![(0,1)],arms:Vec::new()},
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
    let sites:Vec<_>=(0..16).map(|_|Site {page:0,symbol:Some(100),points:vec![p(0.,0.);3],links:vec![(0,1),(0,2)],arms:Vec::new()}).collect();
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
    assert_eq!(connections(&prj,&nl,&open).len(),2,"implicit pages keep a connection per pin");
    prj.pages[0].values.insert("page.use_drawn_wire_targets".into(),"true".into());
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
    assert!(connections(&prj,&nl,&open).iter().all(|c|!c.route_traced),"device bridges alone do not start path tracing");
    prj.pages[0].values.insert("page.use_drawn_wire_targets".into(),"true".into());
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

#[test]
fn a_traced_pair_without_wires_keeps_the_properties_of_its_net() {
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:point".into(),pins:vec![UserPin::default()],..Default::default()});
    for (id,x) in [(1,0.),(2,0.),(3,40.)] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:"user:point".into(),tag:format!("-A{id}"),pos:p(x,0.),..Default::default()});
    }
    prj.pages[0].wires.push(Wire {id:10,a:p(0.,0.),b:p(40.,0.),color:"RD".into(),cross_section:"1.5".into(),number:"101".into(),
        cable:"-W1".into(),core:"BN".into(),explicit_junctions:Some(true),..Default::default()});
    let (nl,open)=(Netlist::build(&prj,true),Netlist::build(&prj,false));
    let all=connections(&prj,&nl,&open);
    let joined=all.iter().find(|c|c.wires.is_empty()).expect("pair without a wire");
    assert!(joined.route_traced);
    assert_eq!((joined.color.as_str(),joined.cross_section.as_str(),joined.number.as_str()),("RD","1.5","101"));
    assert_eq!((joined.cable.as_str(),joined.core.as_str()),("",""),"no conductor of its own");
    assert!(all.iter().any(|c|c.wires==[(0,10)] && c.cable=="-W1"));
}
