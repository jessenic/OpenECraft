use super::*;

#[test]
fn impossible_route_caching_is_explicit_and_round_trips() {
    let mut rules=RoutingRules::default();
    assert!(!rules.reuse_failed_searches);
    assert!(serde_json::to_value(&rules).unwrap().get("reuse_failed_searches").is_none());
    rules.reuse_failed_searches=true;
    let saved=serde_json::to_string(&rules).unwrap();
    assert!(serde_json::from_str::<RoutingRules>(&saved).unwrap().reuse_failed_searches);
    assert!(!serde_json::from_str::<RoutingRules>("{}").unwrap().reuse_failed_searches);
}


#[test]
fn an_impossible_ahead_search_is_not_repeated_after_fill_changes() {
    let prj=Project::default();let rules=RoutingRules::default();
    let mut space=grid::Space::build(&prj,&[],&[],&rules);
    space.paths.push(grid::PathCells {enclosure:0,id:1,kind:PathKind::Any,classes:0,capacity:1.,used:2.,items:1,implied:false});
    let loads=vec![0.];
    assert!(!space.full_as(&loads,1.));
    let req=grid::Request {kind:RouteKind::Cable,class:Class::Signal,ignore:&[],at:[[0.;3];2],leave:[[0.;3];2],out:[0.;2],area:1.,turn:25.,half:1.};
    assert!(finish_ahead(&space,&req,1.,true,Some((None,loads.clone(),None)),|_|panic!("an impossible route must not be searched again")).is_none());
    let mut searched=false;
    assert!(finish_ahead(&space,&req,1.,true,None,|_|{searched=true;None}).is_none());
    assert!(searched,"a missing ahead result still needs a search");
    searched=false;
    assert!(finish_ahead(&space,&req,1.,false,Some((None,loads,None)),|_|{searched=true;None}).is_none());
    assert!(searched,"legacy spaces retry failed ahead searches when channel fill changes");
}

#[test]
fn cable_endpoints_resolve_individually_placed_terminal_numbers() {
    let mut prj=crate::sample::demo_project();
    let terms=crate::strips::terminals(&prj,"-X1");
    let pins:Vec<_>=terms.iter().take(2).map(|t|PinRef {page:t.page,sym:t.sym,pin:0}).collect();
    let (ei,whole)=prj.layout.find_device("-X1").map(|(ei,it)|(ei,it.clone())).unwrap();
    prj.layout.enclosures[ei].items.retain(|i|i.id!=whole.id);
    let mut ids=Vec::new();
    for (k,t) in terms.iter().take(2).enumerate() {
        let mut it=whole.clone();it.id=prj.new_id();it.terminal=t.label.clone();it.x=100.+k as f32*200.;it.w=8.;
        ids.push(it.id);prj.layout.enclosures[ei].items.push(it);
    }
    let pos=panel::positions(&prj.layout);
    let first=device_end(&prj,&pos,"-X1",&pins[..1]);let second=device_end(&prj,&pos,"-X1",&pins[1..]);
    assert_eq!(first.item,Some(ids[0]));assert_eq!(second.item,Some(ids[1]));
    assert!(second.point.unwrap()[0]-first.point.unwrap()[0]>100.);
    let connection=crate::netlist::Connection {net:"terminal jumper".into(),a:pins[0],b:pins[1],color:String::new(),cross_section:String::new(),
        wire_type:String::new(),number:String::new(),cable:String::new(),core:String::new(),harness:String::new(),wires:Vec::new(),route_traced:false};
    let (wires,open)=panel::virtual_wiring(&prj,&[connection]);
    assert!(open.is_empty());assert_eq!(wires.len(),1);assert!(wires[0].length>100.);
    let combined=device_end(&prj,&pos,"-X1",&pins);
    assert_eq!(combined.parts,ids);assert_eq!(combined.cores.len(),2);
    let mut other=prj.layout.enclosures[ei].clone();other.id=prj.new_id();other.name="Other".into();
    other.items.retain(|i|i.id==ids[1]);prj.layout.enclosures[ei].items.retain(|i|i.id!=ids[1]);
    prj.layout.enclosures.push(other);
    assert!(device_end(&prj,&panel::positions(&prj.layout),"-X1",&pins).point.is_none());
}
use crate::model::ReportKind;
use crate::analysis::Analysis;
use crate::issue::Severity;
use crate::sample::demo_project;

#[test]
fn classes_diameters_and_kinds() {
    let prj = demo_project();
    assert_eq!(class_of_net(&prj, "L1"), Class::Power);
    assert_eq!(class_of_net(&prj, "PE"), Class::Power);
    assert_eq!(class_of_net(&prj, "+24V"), Class::Control);
    assert_eq!(class_of_net(&prj, "unknown"), Class::Control);
    assert!(Class::Power.keeps_apart(Class::Signal) && !Class::Control.keeps_apart(Class::Signal));
    assert_eq!(Class::Signal.apart_bits(), Class::Power.bit());
    assert_eq!(tube_diameter("8 mm"), 8.0);
    assert_eq!(tube_diameter("DN25"), 25.0);
    assert_eq!(tube_diameter(""), 8.0);
    let w1 = prj.cable("-W1").unwrap();
    assert!(cable_diameter(w1) > 8.0 && cable_diameter(w1) < 15.0, "{}", cable_diameter(w1));
    assert!(PathKind::Cables.accepts(RouteKind::Cable) && !PathKind::Cables.accepts(RouteKind::Wire) && PathKind::Any.accepts(RouteKind::Tube));
    assert!(ShieldEnds::First.connects(0) && !ShieldEnds::First.connects(1) && !ShieldEnds::None.connects(0));
    let p = RoutingPath { width: 20.0, reserve: 25.0, points: vec![[0.0; 3], [0.0, 300.0, 0.0], [400.0, 300.0, 0.0]], ..Default::default() };
    assert!((p.capacity() - std::f32::consts::PI * 75.0).abs() < 0.01);
    assert_eq!(p.length(), 700.0);
}

#[test]
fn field_cable_runs_to_its_gland() {
    // Without the gland in the bottom of +CAB1 the motor cable enters through the
    // middle of the bottom, which is reported; with it, through the gland.
    let mut bare = demo_project();
    let glands: Vec<u64> = entries(&bare, 0).iter().map(|g| g.id).collect();
    assert_eq!(glands.len(), 1);
    bare.layout.enclosures[0].items.retain(|i| !glands.contains(&i.id));
    let an = Analysis::checked(&bare);
    let w1 = an.routing.item("-W1").expect("motor cable");
    assert_eq!((w1.kind, w1.class), (RouteKind::Cable, Class::Power));
    assert!(w1.routed(), "{:?}", w1.status);
    assert_eq!(w1.ends[0].tag, "-X1");
    assert!(an.issues.iter().any(|i| i.message().contains("no cable gland")));
    let an = Analysis::checked(&demo_project());
    let w1 = an.routing.item("-W1").unwrap();
    assert_eq!(w1.ends[1].tag, "-M1");
    assert!(w1.ends[1].item.is_some());
    assert!(!an.issues.iter().any(|i| i.message().contains("no cable gland")));
    // Length with both cable end reserves, cut length in steps of 100 mm.
    let rules = RoutingRules::default();
    assert!(w1.length > 2.0 * rules.cable_reserve);
    assert_eq!(w1.cut % rules.cut_cable, 0.0);
    assert!(w1.cut >= w1.length && w1.cut < w1.length + rules.cut_cable);
    assert_eq!(w1.enclosures, vec![0]);
}

#[test]
fn wires_in_ducts_form_bundles_with_branches() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let wires: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Wire).collect();
    assert!(!wires.is_empty());
    // Cores of -W1 are routed with their cable, not as wires.
    assert!(!wires.iter().any(|w| w.label.contains("-M1")));
    for w in &wires {
        let rw = &an.wiring[w.wire.unwrap()];
        assert_eq!(w.length, rw.length);
        assert_eq!(w.free, 0.0);
    }
    assert!(an.routing.bundles.iter().any(|b| b.items.len() >= 3));
    assert!(!an.routing.branches.is_empty());
    let report = &an.report(&prj, ReportKind::RoutingList).unwrap();
    assert_eq!(report.rows.len(), an.routing.items.len());
    assert_eq!(an.report(&prj, ReportKind::BundleList).unwrap().rows.len(), an.routing.bundles.len());
}

#[test]
fn wires_routed_in_3d_feed_the_duct_fill() {
    let mut prj = demo_project();
    prj.layout.routing.wires_3d = true;
    let ducts = Analysis::checked(&demo_project());
    let an = Analysis::checked(&prj);
    let free: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Wire && an.wiring[i.wire.unwrap()].door == (None, None)).collect();
    assert!(!free.is_empty());
    for it in &free {
        assert!(it.routed(), "{} {:?}", it.label, it.status);
        let w = &an.wiring[it.wire.unwrap()];
        assert_eq!(it.length, w.length);
        assert!(w.route.points.len() >= 2);
    }
    // The 3D routes use the ducts: they still carry wires, and most of the way is in them.
    assert!(!an.duct_fill.is_empty());
    let (in_air, total): (f32, f32) = free.iter().fold((0.0, 0.0), |(a, t), i| (a + i.free, t + i.length));
    assert!(in_air < total * 0.5, "{in_air} of {total}");
    // Lengths are of the same order as through the ducts on the plate.
    let sum = |an: &Analysis| an.wiring.iter().map(|w| w.length).sum::<f32>();
    assert!(sum(&an) < sum(&ducts) * 1.5 && sum(&an) > sum(&ducts) * 0.5, "{} {}", sum(&an), sum(&ducts));
}

#[test]
fn frozen_routes_survive_recalculation_until_invalid() {
    let mut prj = demo_project();
    let an = Analysis::checked(&prj);
    let n = freeze(&mut prj, &an.routing, &[]);
    // Wires that stay on the door have no route in the enclosure to freeze.
    assert_eq!(n, an.routing.items.iter().filter(|i| i.routed() && !i.enclosures.is_empty()).count());
    let an = Analysis::checked(&prj);
    assert_eq!(an.routing.item("-W1").unwrap().status, Status::Frozen);
    assert!(an.routing.changes.is_empty(), "{:?}", an.routing.changes);
    // A keep-out across the frozen cable route: routed again, and reported.
    let w1 = an.routing.item("-W1").unwrap().clone();
    let mid = w1.points[w1.points.len() / 2];
    prj.layout.enclosures[0].reservations.push(Reservation { id: 1, min: [mid[0] - 30.0, mid[1] - 30.0, 0.0], max: [mid[0] + 30.0, mid[1] + 30.0, 500.0], ..Default::default() });
    let an = Analysis::checked(&prj);
    let again = an.routing.item("-W1").unwrap();
    assert!(matches!(again.status, Status::Rerouted | Status::Unrouted(_)), "{:?}", again.status);
    assert!(an.issues.iter().any(|i| i.message().contains("Frozen route of -W1")));
    // Moving the terminal strip changes the length of its wires: a change against the
    // frozen length.
    prj.layout.enclosures[0].reservations.clear();
    let x1 = prj.layout.enclosures[0].items.iter().position(|i| i.tag == "-X1").unwrap();
    prj.layout.enclosures[0].items[x1].x += 200.0;
    let an = Analysis::checked(&prj);
    assert!(!an.routing.changes.is_empty());
    assert!(an.issues.iter().any(|i| i.message().contains("frozen routes changed")));
    // Released routes are calculated afresh.
    assert_eq!(release(&mut prj, &["-W1".to_string()]), 1);
    assert!(prj.layout.frozen.iter().all(|f| f.key != "-W1"));
    let all = prj.layout.frozen.len();
    assert_eq!(release(&mut prj, &[]), all);
    assert!(prj.layout.frozen.is_empty());
}

#[test]
fn bayed_enclosures_route_cables_through_passages() {
    let mut prj = demo_project();
    let cab2 = crate::panel::new_enclosure(&mut prj, crate::panel::ENCLOSURE_PART, "+CAB2");
    prj.layout.enclosures.push(cab2);
    let i = prj.layout.enclosures[0].items.iter().position(|i| i.tag == "-X1").unwrap();
    let x1 = prj.layout.enclosures[0].items.remove(i);
    prj.layout.enclosures[1].items.push(x1);
    // The gland now in +CAB2 as well.
    crate::panel::place_accessory(&mut prj, 1, "GEN.GLAND-M25", Face::Bottom, crate::geom::p(80.0, 100.0)).unwrap();
    let an = Analysis::checked(&prj);
    let k1 = an.routing.items.iter().find(|i| i.kind == RouteKind::Wire && i.label.contains("-X1")).map(|i| i.status.clone());
    assert!(k1.is_none(), "not bayed: the wires are not routed at all");
    crate::panel::bay_right_of(&mut prj, 1, 0);
    crate::panel::add_passage(&mut prj, 0, 600.0, 80.0, 60.0);
    prj.layout.routing.wires_3d = true;
    let an = Analysis::checked(&prj);
    let across = an.routing.items.iter().find(|i| i.kind == RouteKind::Wire && i.enclosures.len() == 2).expect("a wire across");
    assert!(across.routed());
    let w = &an.wiring[across.wire.unwrap()];
    assert_eq!(w.legs.len(), 1);
    assert!(w.in_enclosure(0) && w.in_enclosure(1));
}

#[test]
fn rules_and_frozen_routes_round_trip() {
    let mut prj = demo_project();
    prj.layout.routing.segregation = 150.0;
    prj.layout.enclosures[0].paths.push(RoutingPath { id: 5, name: "Tray".into(), kind: PathKind::Cables, points: vec![[50.0, 50.0, 100.0], [50.0, 700.0, 100.0]], width: 40.0, classes: vec![Class::Signal], reserve: 20.0, face: None });
    prj.layout.enclosures[0].reservations.push(Reservation { id: 6, name: "Spare".into(), min: [300.0, 600.0, 20.0], max: [500.0, 750.0, 200.0], mode: ReserveMode::For(Class::Power) });
    let an = Analysis::checked(&prj);
    freeze(&mut prj, &an.routing, &["-W1".to_string()]);
    let json = serde_json::to_string(&prj.layout).unwrap();
    let back: crate::panel::Layout = serde_json::from_str(&json).unwrap();
    assert_eq!(back, prj.layout);
    assert!(!serde_json::to_string(&crate::panel::Layout::default()).unwrap().contains("routing"));
    // The cable path is reported with its use.
    assert!(an.routing.paths.iter().any(|p| p.path == 5 && p.capacity > 0.0));
}

#[test]
fn the_routing_is_reused_while_nothing_it_depends_on_changes() {
    let mut prj = demo_project();
    let mut cache = crate::analysis::Cache::default();
    // Routes searched by a build (progress is told before each one).
    let searched = |prj: &Project, cache: &mut crate::analysis::Cache| {
        let routes = std::sync::atomic::AtomicUsize::new(0);
        let an = Analysis::build_progress(prj, cache, &|p| {
            if p.step == crate::analysis::Step::Routing && p.total > 0 {
                routes.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        });
        (routes.into_inner(), an)
    };
    let (first, an) = searched(&prj, &mut cache);
    assert!(first > 0);
    // A text on a page changes no route: the same routing and wiring, nothing searched.
    let page = prj.pages.iter().position(|p| !p.texts.is_empty()).unwrap();
    prj.pages[page].texts[0].text.push_str(" (new)");
    let (again, same) = searched(&prj, &mut cache);
    assert_eq!(again, 0);
    assert!(same.routing == an.routing && same.wiring == an.wiring);
    assert_eq!(same.routing, Analysis::build(&prj).routing, "as built without the cache");
    // A device moved in the layout and the language are searched again.
    let it = prj.layout.enclosures[0].items.iter_mut().find(|i| i.tag == "-K1").unwrap();
    it.x += 25.0;
    assert_eq!(searched(&prj, &mut cache).0, first);
    openecraft_i18n::with_lang(openecraft_i18n::Lang::De, || assert_eq!(searched(&prj, &mut cache).0, first));
}

#[test]
fn an_interim_routing_keeps_what_still_fits() {
    let an = Analysis::checked(&demo_project());
    let r = &an.routing;
    assert!(r.items.iter().any(|i| i.kind == RouteKind::Wire) && r.items.iter().any(|i| i.kind == RouteKind::Cable));
    let interim = r.interim(1);
    assert!(interim.items.iter().all(|i| i.kind != RouteKind::Wire), "wire indices may have changed");
    assert_eq!(interim.items.len(), r.items.iter().filter(|i| i.kind != RouteKind::Wire).count());
    assert!(interim.bundles.is_empty() && interim.branches.is_empty());
    assert_eq!(interim.problems, r.problems);
    // Without the enclosure the routes run in, nothing is left of them.
    assert!(r.interim(0).items.iter().all(|i| i.enclosures.is_empty() && i.ends.iter().all(|e| e.enclosure.is_none())));
}

#[test]
fn a_kept_routing_is_only_taken_from_the_cache() {
    let mut prj = demo_project();
    let mut cache = crate::analysis::Cache::default();
    let full = Analysis::build_with(&prj, &mut cache);
    let it = prj.layout.enclosures[0].items.iter_mut().find(|i| i.tag == "-K1").unwrap();
    it.x += 25.0;
    let kept = Analysis::build_kept(&prj, &mut cache, &full.routing);
    assert!(kept.routes_pending);
    assert_eq!(kept.routing, full.routing.interim(prj.layout.enclosures.len()));
    // Once a build elsewhere routed the project, its routing is taken.
    let mut elsewhere = cache.clone();
    let routed = Analysis::build_with(&prj, &mut elsewhere);
    cache.keep_routing(elsewhere);
    let kept = Analysis::build_kept(&prj, &mut cache, &full.routing);
    assert!(!kept.routes_pending);
    assert!(kept.routing == routed.routing && kept.wiring == routed.wiring);
}

#[test]
fn searches_at_the_same_time_give_the_same_routes() {
    // The pump room: cables of several classes, pipes along its pipe runs (routing
    // paths that fill up), legs through the room into the cabinets.
    let prj = crate::sample::example("pump_room").unwrap();
    let an = Analysis::build(&prj);
    let route_on = |workers: usize| {
        let mut wiring = crate::panel::virtual_wiring(&prj, &an.connections).0;
        let jobs = jobs(&prj, &wiring, &an.connections, &an.open);
        let routing = route(&prj, &mut wiring, jobs, Glands::default(), &|_, _| {}, workers);
        (routing, wiring)
    };
    let (one, four) = (route_on(1), route_on(4));
    assert!(one.0.items.iter().filter(|i| i.kind != RouteKind::Wire && i.routed()).count() > 5);
    assert!(one.0.paths.iter().any(|u| u.items > 1), "{:?}", one.0.paths);
    assert!(one == four);
}

#[test]
fn pipes_end_at_the_nozzles_of_their_devices() {
    let prj = crate::sample::example("pump_room").unwrap();
    let an = Analysis::build(&prj);
    let pipes: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Tube && i.routed()).collect();
    assert!(!pipes.is_empty());
    for it in &pipes {
        // Into each nozzle straight from the end of the route out of it (devices without
        // a model with nozzles, like the gauge, as before).
        let n = it.points.len();
        for (end, tip, out) in [(0, it.points[0], it.points[1]), (1, it.points[n - 1], it.points[n - 2])] {
            let Some(nozzle) = it.ends[end].nozzle else { continue };
            assert_eq!(tip, nozzle, "{}", it.label);
            assert!(crate::parts::vec3::dist(out, it.ends[end].point.unwrap()) < 0.01, "{}", it.label);
            assert!(crate::parts::vec3::dist(out, tip) >= NOZZLE_OUT, "{}", it.label);
        }
    }
    // The suction and the pressure pipe of the pump, the two pipes of the vessel: each at
    // a nozzle of its own.
    for tag in ["=P1+ER1-GP1", "=P1+ER1-CM1"] {
        let mut at: Vec<V3> = pipes.iter().flat_map(|i| i.ends.iter()).filter(|e| e.tag == tag).filter_map(|e| e.nozzle).collect();
        assert_eq!(at.len(), 2, "{tag}");
        at.dedup();
        assert_eq!(at.len(), 2, "{tag}: {at:?}");
    }
}

#[test]
fn fittings_in_a_pipe_are_turned_to_the_pipes_they_join() {
    // The fittings on the left wall of the house connection room are mounted against
    // their connection points: each pipe still joins the ends facing each other.
    let prj = crate::sample::example("building_services").unwrap();
    let an = Analysis::build(&prj);
    let centre = |tag: &str| {
        let (k, it) = crate::panel::locate(&prj, tag).unwrap();
        let e = &prj.layout.enclosures[k];
        add(crate::panel::face_point(e, it.face, [it.x + it.w / 2.0, it.y + it.h / 2.0, it.z + it.d / 2.0]), crate::panel::positions(&prj.layout)[k])
    };
    let mut checked = 0;
    for key in ["hose:=WAT+HR-QM1:2|=WAT+HR-BF1:1", "hose:=WAT+HR-BF1:2|=WAT+HR-RM1:1"] {
        let it = an.routing.item(key).unwrap_or_else(|| panic!("{key}"));
        let tips = [it.ends[0].nozzle.unwrap(), it.ends[1].nozzle.unwrap()];
        let (a, b) = (centre(&it.ends[0].tag), centre(&it.ends[1].tag));
        assert!(crate::parts::vec3::dist(tips[0], b) < crate::parts::vec3::dist(a, b), "{key}: {tips:?}");
        assert!(crate::parts::vec3::dist(tips[1], a) < crate::parts::vec3::dist(a, b), "{key}: {tips:?}");
        // Straight from one to the other: not much longer than the way between them.
        assert!(crate::parts::vec3::length(sub(tips[0], tips[1])) * 1.5 + 200.0 > it.points.windows(2).map(|s| crate::parts::vec3::dist(s[0], s[1])).sum::<f32>(), "{key}: {:?}", it.points);
        checked += 1;
    }
    assert_eq!(checked, 2);
}

#[test]
fn cables_take_the_glands_that_fit_them() {
    let prj = Project::default();
    let gland = |id: u64, part: &str| PanelItem { id, kind: ItemKind::Accessory, part: part.into(), ..Default::default() };
    let (m20, m32, m32b, m25) = (gland(1, "GEN.GLAND-M20"), gland(2, "GEN.GLAND-M32"), gland(3, "GEN.GLAND-M32"), gland(4, "GEN.GLAND-M25"));
    let mut glands = Glands::default();
    let through = |e: Entry| match e {
        Entry::Through(g) => Some(g.id),
        Entry::None => None,
        Entry::Taken => Some(0),
    };
    // The nearest that fits the cable, even farther off than one that does not.
    assert_eq!(through(glands.pick(&prj, vec![(&m32, 10.0), (&m20, 100.0)], 10.0)), Some(1));
    // Each thick cable its own gland; a gland takes one cable, so a third finds none.
    assert_eq!(through(glands.pick(&prj, vec![(&m32, 10.0), (&m32b, 50.0)], 20.0)), Some(2));
    assert_eq!(through(glands.pick(&prj, vec![(&m32, 10.0), (&m32b, 50.0)], 20.0)), Some(3));
    assert_eq!(through(glands.pick(&prj, vec![(&m32, 10.0), (&m32b, 50.0)], 20.0)), Some(0));
    // None fits: the nearest free one; nothing without glands.
    assert_eq!(through(glands.pick(&prj, vec![(&m32, 10.0), (&m25, 50.0)], 30.0)), Some(4));
    assert_eq!(through(glands.pick(&prj, vec![], 30.0)), None);
    // A cable entry flange takes cable after cable (each at a place of its own).
    let mut prj = Project::default();
    prj.parts.push(crate::parts::Part { number: "FLANGE".into(), subgroup: "Cable entry".into(), tech_data: "Cutout: 220 x 90".into(), ..Default::default() });
    let flange = gland(5, "FLANGE");
    for _ in 0..3 {
        assert_eq!(through(glands.pick(&prj, vec![(&flange, 10.0)], 20.0)), Some(5));
    }
}

#[test]
fn a_cable_without_a_free_gland_is_reported() {
    // Two of the four cores of the demo's motor cable as a cable -W2 of its own: two
    // cables out of +CAB1 through its one gland. A gland takes one cable, so one of
    // them is not routed, with an error that names the cabinet; a second gland takes
    // the other.
    let mut prj = demo_project();
    let part = prj.parts.iter().find(|p| p.number == "GEN.CBL-4G1.5").unwrap().clone();
    let id = prj.new_id();
    prj.cables.push(Cable { id, tag: "-W2".into(), cable_type: part.type_number.clone(), cores: part.tech_value("Cores").unwrap_or_default().into(), cross_section: part.tech_value("Cross-section").unwrap_or_default().into(), length: 15.0, part: part.number.clone(), ..Default::default() });
    let page = prj.pages.iter().position(|p| p.name == "2").unwrap();
    let wires: std::collections::BTreeSet<u64> = prj.pages[page].wires.iter().filter(|w| w.cable == "-W1").map(|w| w.id).take(2).collect();
    assert_eq!(crate::ops::assign_cores(&mut prj, page, &wires, "-W2"), 2);
    let an = Analysis::checked(&prj);
    let (w1, w2) = (an.routing.item("-W1").unwrap(), an.routing.item("-W2").unwrap());
    assert!(w1.routed() != w2.routed(), "{:?} {:?}", w1.status, w2.status);
    let left = if w1.routed() { w2 } else { w1 };
    assert_eq!(left.status, Status::Unrouted(tr("no free cable gland").to_string()));
    let p = an.routing.problems.iter().find(|p| p.level == Level::Error && p.message.contains(&left.label)).unwrap_or_else(|| panic!("{:?}", an.routing.problems));
    assert!(p.message.contains("every cable gland of +CAB1") && p.target.is_some(), "{}", p.message);
    assert!(an.issues.iter().any(|i| i.severity == Severity::Error && i.message() == p.message));
    crate::panel::place_accessory(&mut prj, 0, "GEN.GLAND-M25", Face::Bottom, crate::geom::p(160.0, 100.0)).unwrap();
    let an = Analysis::checked(&prj);
    assert!(["-W1", "-W2"].iter().all(|k| an.routing.item(k).unwrap().routed()), "{:?}", an.routing.problems);
    assert!(an.routing.problems.iter().all(|p| !p.message.contains("every cable gland")), "{:?}", an.routing.problems);
}

#[test]
fn a_wire_to_a_busbar_terminal_passes_the_strips_other_terminals_outside() {
    // The wire from the coupling switch to terminal 2 of =NA+HV1-X31 comes up past
    // terminal 3 (the lowest on the bars): round it, not through it; the cable of the
    // supply ends at all three terminals of X01 and may come up through them.
    let prj = crate::sample::example("industrial_main_distribution").unwrap();
    let an = Analysis::checked(&prj);
    let pos = panel::positions(&prj.layout);
    let (ei, x31) = panel::locate(&prj, "=NA+HV1-X31").unwrap();
    let e = &prj.layout.enclosures[ei];
    let places = panel::busbars::terminals_on_bars(&prj, x31).unwrap();
    let third = places.iter().find(|p| p.2 == "3").map(|p| panel::item_box(e, &p.0)).unwrap();
    let (lo, hi) = (add(third.0, pos[ei]), add(third.1, pos[ei]));
    let w = an.routing.items.iter().find(|i| i.kind == RouteKind::Wire && i.label == "=NA+HV1-Q31:3 – =NA+HV1-X31:2").unwrap();
    assert!(w.routed(), "{:?}", w.status);
    assert_eq!(w.ends[1].parts, vec![part_id(x31.id, 1)]);
    let r = w.diameter / 2.0;
    let gap = |p: V3| (0..3).map(|k| (lo[k] - p[k]).max(p[k] - hi[k]).max(0.0).powi(2)).sum::<f32>().sqrt();
    for s in w.points.windows(2) {
        let n = (dist(s[0], s[1]) / 5.0).ceil().max(1.0) as usize;
        for i in 0..=n {
            let p = add(s[0], scale(sub(s[1], s[0]), i as f32 / n as f32));
            assert!(gap(p) >= r - 0.5, "{:?} - {:?} through terminal 3 at {lo:?}..{hi:?}", s[0], s[1]);
        }
    }
    let w1 = an.routing.item("=GRID-W1").unwrap();
    let (_, x01) = panel::locate(&prj, "=GRID+HV1-X01").unwrap();
    assert_eq!(w1.ends[0].parts, (0..3).map(|k| part_id(x01.id, k)).collect::<Vec<_>>());
}

#[test]
fn an_unroutable_cable_is_reported_at_its_connection_point() {
    // With half a metre of clearance nothing fits into the cabinet: the error names
    // the cable and points at a connection point of it, and so does the issue of the
    // project check.
    let mut prj = demo_project();
    prj.layout.routing.clearance = 500.0;
    let an = Analysis::checked(&prj);
    assert!(!an.routing.item("-W1").unwrap().routed());
    let p = an.routing.problems.iter().find(|p| p.level == Level::Error && p.message.contains("-W1")).unwrap_or_else(|| panic!("{:?}", an.routing.problems));
    let (page, id) = p.target.unwrap();
    let sym = prj.pages[page].symbols.iter().find(|s| s.id == id).unwrap();
    assert!(sym.tag == "-M1" || sym.tag == "-X1", "{}", sym.tag);
    assert!(an.issues.iter().any(|i| i.severity == Severity::Error && i.message() == p.message && i.target == p.target));
}

#[test]
fn cables_through_one_entry_flange_go_through_it_side_by_side() {
    let mut prj = Project::default();
    prj.parts.push(crate::parts::Part { number: "FLANGE".into(), subgroup: "Cable entry".into(), tech_data: "Cutout: 220 x 90".into(), ..Default::default() });
    let flange = PanelItem { id: 1, kind: ItemKind::Accessory, part: "FLANGE".into(), w: 240.0, h: 110.0, ..Default::default() };
    let gland = PanelItem { id: 2, kind: ItemKind::Accessory, part: "GEN.GLAND-M32".into(), ..Default::default() };
    let at = |off: [f32; 2]| [off[0], off[1], 0.0];
    let mut glands = Glands::default();
    // Places 35 mm apart along the membrane for 25 mm cables: the first takes the one
    // nearest to where it heads, the second heading the same way the next one; the
    // ones beyond the cutout are none. A cable gland takes its middle.
    let heading = [-200.0, 0.0, 0.0];
    assert_eq!(glands.slot(&prj, &flange, 25.0, at, heading), [-70.0, 0.0, 0.0]);
    assert_eq!(glands.slot(&prj, &flange, 25.0, at, heading), [-35.0, 0.0, 0.0]);
    assert_eq!(glands.slot(&prj, &flange, 25.0, at, [200.0, 0.0, 0.0]), [70.0, 0.0, 0.0]);
    assert_eq!(glands.slot(&prj, &gland, 20.0, at, heading), [0.0; 3]);
    assert_eq!(glands.slot(&prj, &gland, 20.0, at, heading), [0.0; 3]);
    // The two supply cables of example 14 (56 mm thick) come up through one flange side
    // by side, each below its terminals.
    let an = Analysis::checked(&crate::sample::example("industrial_main_distribution").unwrap());
    let through = |key: &str| an.routing.item(key).unwrap().ends.iter().find(|e| e.tag.contains("+HAK")).and_then(|e| e.point).unwrap();
    assert!(through("=GRID-W1")[0] < 100.0 && through("=GRID-W2")[0] > 200.0, "{:?} {:?}", through("=GRID-W1"), through("=GRID-W2"));
}

#[test]
fn duct_runs_take_the_part_of_a_segment_inside_the_duct() {
    let mut e = crate::panel::Enclosure::default();
    e.items.push(PanelItem { id: 3, kind: ItemKind::Duct, x: 100.0, y: 100.0, w: 40.0, h: 400.0, d: 60.0, ..Default::default() });
    let (lo, hi) = panel::item_box(&e, &e.items[0]);
    let mid = |y: f32| [(lo[0] + hi[0]) / 2.0, y, (lo[2] + hi[2]) / 2.0];
    // Down the duct and on past its end (into a crossing duct, to a terminal).
    let runs = duct_runs(&e, &[mid(lo[1] + 50.0), mid(hi[1] + 200.0)]);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].duct, 3);
    assert!((runs[0].from - 150.0).abs() < 1e-3 && (runs[0].to - 501.0).abs() < 1e-3, "{runs:?}");
    // Into the duct from the side and along it.
    let side = [lo[0] - 100.0, lo[1] + 50.0, (lo[2] + hi[2]) / 2.0];
    let runs = duct_runs(&e, &[side, mid(lo[1] + 50.0), mid(lo[1] + 150.0)]);
    assert_eq!(runs.iter().map(|r| (r.from, r.to)).collect::<Vec<_>>(), vec![(150.0, 250.0)]);
    // Beside the duct: nothing.
    assert!(duct_runs(&e, &[[lo[0] - 10.0, lo[1], lo[2]], [lo[0] - 10.0, hi[1], lo[2]]]).is_empty());
}

#[test]
fn cables_in_the_ducts_count_in_their_fill() {
    // The cabinets of the line-up: cables enter their ducts through the slots and run
    // along them (the demo's motor cable goes over the bottom duct to its gland).
    let prj = (crate::sample::EXAMPLES.iter().find(|x| x.key == "cabinet_lineup").unwrap().build)();
    let an = Analysis::checked(&prj);
    let contents = duct_contents(&prj, &an.routing);
    assert!(!contents.is_empty(), "the cables run through the ducts");
    for c in &contents {
        for r in &c.runs {
            let f = an.duct_fill.iter().find(|f| f.enclosure == c.enclosure && f.duct == r.duct).expect("fill of the duct");
            assert!(f.diameters.contains(&c.diameter), "{f:?}");
        }
    }
}

#[test]
fn cables_in_the_ducts_of_a_room_count_in_their_fill() {
    let prj = (crate::sample::EXAMPLES.iter().find(|x| x.key == "pump_room").unwrap().build)();
    let an = Analysis::checked(&prj);
    let in_room: Vec<&crate::panel::DuctFill> = an.duct_fill.iter().filter(|f| prj.layout.enclosures[f.enclosure].is_room()).collect();
    assert!(!in_room.is_empty(), "the cables along the ceiling of the room");
    for f in in_room {
        let d = prj.layout.enclosures[f.enclosure].item(f.duct).unwrap();
        assert!(f.total_wires > 0 && f.percent > 0.0, "{f:?}");
        // The fullest stretch along the duct, in the coordinates of its face.
        let (lo, hi) = if d.horizontal() { (d.x, d.x + d.w) } else { (d.y, d.y + d.h) };
        assert!(f.from >= lo - 1.0 && f.to <= hi + 1.0 && f.to > f.from, "{f:?} on {lo}..{hi}");
    }
}

/// Where the segments `a` and `b` come nearest to each other: the points on both.
fn nearest(a: (V3, V3), b: (V3, V3)) -> (V3, V3) {
    use crate::parts::vec3::{add, dot, scale, sub};
    let (u, v, w) = (sub(a.1, a.0), sub(b.1, b.0), sub(a.0, b.0));
    let (uu, uv, vv, uw, vw) = (dot(u, u), dot(u, v), dot(v, v), dot(u, w), dot(v, w));
    let denom = uu * vv - uv * uv;
    let mut s = if denom > 1e-6 { ((uv * vw - vv * uw) / denom).clamp(0.0, 1.0) } else { 0.0 };
    let t = if vv > 1e-9 { ((uv * s + vw) / vv).clamp(0.0, 1.0) } else { 0.0 };
    if uu > 1e-9 {
        s = ((uv * t - uw) / uu).clamp(0.0, 1.0);
    }
    (add(a.0, scale(u, s)), add(b.0, scale(v, t)))
}

#[test]
fn wires_of_cabinets_without_cable_ducts_run_from_point_to_point() {
    for key in ["pv_transformer_metering", "industrial_main_distribution"] {
        let prj = crate::sample::example(key).unwrap();
        let an = Analysis::checked(&prj);
        assert!(an.routing.items.iter().all(|i| i.routed()), "{key}: {:?}", an.routing.items.iter().filter(|i| !i.routed()).map(|i| &i.key).collect::<Vec<_>>());
        let wires: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Wire && an.wiring[i.wire.unwrap()].door == (None, None)).collect();
        assert!(wires.len() > 50, "{key}");
        for it in &wires {
            // Searched through the space like a cable: from the connection point, out of
            // it the way it leaves, to the connection point at the other end.
            let w = &an.wiring[it.wire.unwrap()];
            assert!(between_points(&prj.layout, w) && wire_in_3d(&prj.layout, w), "{key}: {}", it.label);
            assert_eq!((it.points.first().copied(), it.points.last().copied()), (it.ends[0].point, it.ends[1].point), "{key}: {}", it.label);
            assert!(it.ends.iter().all(|e| e.leave.is_some()), "{key}: {}", it.label);
            assert!(it.free > 0.0 && it.length == w.length, "{key}: {}", it.label);
            // As long as its way and the reserve: nothing added for a way down to the plate.
            let way = grid::length(&it.points);
            assert!(it.length <= way + prj.layout.wire_reserve + 0.5 && it.length > way * 0.8, "{key}: {} {} of {way}", it.label, it.length);
            // Bent with its radius: its corners far enough apart for the arcs of both.
            assert_eq!(grid::close_bends(&it.points, 2.0 * it.bend_radius), 0, "{key}: {} {:?}", it.label, it.points);
        }
        // None runs inside another: only where both come up to one of their ends (the
        // terminals of one device, a cable parting into its cores) they converge.
        let rules = &prj.layout.routing;
        let cell = rules.grid.max(5.0);
        let zone = |i: &RoutedItem| (3.0 * cell).max(2.0 * i.bend_radius.max(cell / 2.0) + cell) + i.diameter / 2.0 + 2.0 * cell;
        let ends = |i: &RoutedItem| -> Vec<V3> { i.ends.iter().filter_map(|e| e.point).chain(i.legs.iter().flat_map(|l| [l.points[0], l.points[l.points.len() - 1]])).collect() };
        let near_end = |i: &RoutedItem, q: V3| ends(i).into_iter().any(|e| dist(e, q) <= zone(i));
        let routed: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind != RouteKind::Wire || wires.iter().any(|w| w.key == i.key)).collect();
        // The stretches of a route: no segment joins two of its legs.
        let segments = |i: &RoutedItem| -> Vec<(V3, V3)> { i.stretches().into_iter().flat_map(|(_, points)| points.windows(2).map(|s| (s[0], s[1])).collect::<Vec<_>>()).collect() };
        for (n, a) in routed.iter().enumerate() {
            for b in &routed[n + 1..] {
                for sa in segments(a) {
                    for sb in segments(b) {
                        let (p, q) = nearest(sa, sb);
                        let inside = dist(p, q) < (a.diameter + b.diameter) / 2.0 - 0.5;
                        assert!(!inside || (near_end(a, p) && near_end(b, q)), "{key}: {} and {} run inside each other at {p:?}", a.label, b.label);
                    }
                }
            }
        }
    }
    // With cable ducts on the mounting plate the wires run through them.
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    assert!(an.wiring.iter().all(|w| !between_points(&prj.layout, w) && !wire_in_3d(&prj.layout, w)));
}

#[test]
fn cables_between_cabinets_in_no_room_run_to_their_entries() {
    let prj = crate::sample::example("pv_transformer_metering").unwrap();
    let an = Analysis::checked(&prj);
    // The cable from the transformers to the test terminal block of the meter cabinet.
    let it = an.routing.item("=MS1-W2").unwrap();
    assert!(it.routed(), "{:?}", it.status);
    assert_eq!(it.legs.len(), 2);
    let hv = prj.layout.enclosures.iter().position(|e| e.name == "+HV").unwrap();
    let zs = prj.layout.enclosures.iter().position(|e| e.name == "+ZS1").unwrap();
    assert_eq!((it.legs[0].enclosure, it.legs[1].enclosure), (hv, zs));
    // Each stretch ends at a cable entry of its cabinet, in its bottom.
    let pos = crate::panel::positions(&prj.layout);
    for (leg, k, end) in [(&it.legs[0], hv, leg_end(&it.legs[0], true)), (&it.legs[1], zs, leg_end(&it.legs[1], false))] {
        let e = &prj.layout.enclosures[k];
        assert!((end[1] - pos[k][1] - e.height).abs() < 10.0, "{:?} of {}", leg.points, e.name);
    }
    assert!(an.issues.iter().any(|i| i.severity == Severity::Info && i.message().contains("=MS1-W2") && i.message().contains("cable entries")));
    assert!(!an.issues.iter().any(|i| i.severity <= Severity::Warning));
    // Every cable through a cable entry of its own.
    let cables: Vec<&RoutedItem> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Cable).collect();
    let through = |i: &RoutedItem| -> Vec<[i32; 3]> { i.ends.iter().filter(|e| e.tag.contains("GLAND")).filter_map(|e| e.point).chain(i.legs.iter().flat_map(|l| [l.points[0], l.points[l.points.len() - 1]])).map(|q| q.map(|v| v.round() as i32)).collect() };
    for (n, a) in cables.iter().enumerate() {
        for b in &cables[n + 1..] {
            assert!(!through(a).iter().any(|q| through(b).contains(q)), "{} and {} through one cable entry", a.label, b.label);
        }
    }
}

/// The end of a stretch of a route at the side of its cabinet: its last point, or its
/// first.
fn leg_end(leg: &RouteLeg, last: bool) -> V3 {
    if last {
        leg.points[leg.points.len() - 1]
    } else {
        leg.points[0]
    }
}

#[test]
fn routes_through_one_wall_penetration_pass_it_side_by_side() {
    // A penetration of 300 x 200 mm in the right wall of a room, into the room behind.
    let mut prj = Project::default();
    let room = panel::new_room(&mut prj, "+R1", 4000.0, 3000.0, 2800.0);
    prj.layout.enclosures.push(room);
    let item = panel::add_opening(&mut prj, 0, panel::Opening::Penetration, Face::Wall(1), Some(1000.0), None, None).unwrap();
    let link = panel::WallLink { from: 0, item, to: 1, via: panel::Via::Wall, a: [3940.0, 500.0, 1150.0], b: [60.0, 500.0, 1150.0], length: 360.0, leave: [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0]] };
    let mut glands = Glands::default();
    // The first cable of 25 mm through its middle; the second, which comes from the
    // front of the room like it (z 300), 35 mm beside it on that side: at the same
    // place on both sides of the wall.
    let (front, rear) = ([1000.0, 500.0, 300.0], [1000.0, 500.0, 2000.0]);
    assert_eq!(glands.through_wall(&prj, link, 25.0, front, front), link);
    let second = glands.through_wall(&prj, link, 25.0, front, front);
    assert_eq!((sub(second.a, link.a), sub(second.b, link.b)), ([0.0, 0.0, -35.0], [0.0, 0.0, -35.0]));
    // One from the rear of the room behind takes the place on the other side.
    let third = glands.through_wall(&prj, link.reversed(), 25.0, rear, rear);
    assert_eq!((sub(third.a, link.b), sub(third.b, link.a)), ([0.0, 0.0, 35.0], [0.0, 0.0, 35.0]));
    // Seven of them fill the row, the next ones go into the rows above and below it.
    let more: Vec<panel::WallLink> = (0..6).map(|_| glands.through_wall(&prj, link, 25.0, front, front)).collect();
    assert!(more[..4].iter().all(|l| l.a[1] == 500.0) && more[0].a[2] == 1080.0 && more[4].a == [3940.0, 465.0, 1150.0] && more[5].a == [3940.0, 535.0, 1150.0], "{more:?}");
    let places: Vec<V3> = [link, second, third.reversed()].into_iter().chain(more).map(|l| l.a).collect();
    for (i, p) in places.iter().enumerate() {
        assert!(places[..i].iter().all(|q| dist(*p, *q) >= 34.0), "{places:?}");
    }
    // A shaft or a link between buildings is passed as it is.
    let shaft = panel::WallLink { via: panel::Via::Shaft, ..link };
    assert_eq!(glands.through_wall(&prj, shaft, 25.0, front, front), shaft);
}
