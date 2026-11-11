use super::*;
use crate::index::Index;
use crate::analysis::Analysis;
use crate::issue::Severity;

#[test]
fn demo_is_consistent() {
    let prj = demo_project();
    let an = Analysis::checked(&prj);
    let errors: Vec<_> = an.issues.iter().filter(|i| i.severity == Severity::Error).collect();
    assert!(errors.is_empty(), "{errors:#?}");
    let idx = Index::build(&prj);
    assert_eq!(idx.devices["-K1"].mains.len(), 1);
    assert_eq!(idx.devices["-K1"].auxs.len(), 4);
    // Potentials propagate through the interruption points.
    let lamp = prj.pages[2].symbols.iter().find(|s| s.tag == "-P1").unwrap();
    let net = an.netlist.pin_net[&(lamp.id, 1)];
    assert_eq!(an.netlist.nets[net].name, "0V");
    let warnings: Vec<_> = an.issues.iter().filter(|i| i.severity == Severity::Warning).map(|i| i.message()).collect();
    assert!(warnings.is_empty(), "{warnings:#?}");
    // Unused contacts of contactor + auxiliary block are reserves (2 NC of the block).
    let reserve = crate::parts::reserve_functions(&prj, None, "-K1");
    let pins: Vec<_> = reserve.iter().map(|f| f.pins.as_str()).collect();
    assert_eq!(pins, vec!["61 62", "71 72"]);
    // BOM: -X1 has three feed-through terminals and one PE terminal.
    let bom = &an.report(&prj, crate::model::ReportKind::BillOfMaterials).unwrap();
    let qty = |part: &str| bom.rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap_or_default();
    assert_eq!((qty("GEN.TB-2.5").as_str(), qty("GEN.TB-PE-2.5").as_str()), ("3", "1"));
}

fn checked(key: &str) -> (Project, Analysis) {
    let prj = example(key).unwrap_or_else(|| panic!("example {key}"));
    let an = Analysis::checked(&prj);
    (prj, an)
}

#[test]
fn examples_are_consistent() {
    let mut keys: Vec<&str> = EXAMPLES.iter().map(|e| e.key).collect();
    keys.dedup();
    assert_eq!(keys.len(), EXAMPLES.len());
    assert!(EXAMPLES.windows(2).all(|w| w[0].level < w[1].level), "levels rise");
    for e in &EXAMPLES {
        let (prj, an) = checked(e.key);
        let problems: Vec<String> =
            an.issues.iter().filter(|i| i.severity <= Severity::Warning).map(|i| format!("{:?}: {}", i.severity, i.message())).collect();
        assert!(problems.is_empty(), "{}: {problems:#?}", e.key);
        // No wire, cable or tube routed in 3D runs through itself or back alongside
        // itself closer than it is thick.
        for it in an.routing.items.iter().filter(|i| i.routed()) {
            let searched = it.wire.and_then(|wi| an.wiring.get(wi)).is_none_or(|w| crate::routing3d::wire_in_3d(&prj.layout, w));
            for (_, points) in it.stretches().into_iter().filter(|_| searched) {
                let at = crate::routing3d::grid::meets_itself(points, it.bend_radius, it.diameter + 0.5, 0);
                assert_eq!(at, None, "{}: {} comes back to itself: {points:?}", e.key, it.label);
            }
        }
        // Every device with a function in the schematic has a part (fluid ports and
        // the solenoid coils of the valves aside).
        let idx = Index::build(&prj);
        let missing: Vec<&String> = idx
            .devices
            .keys()
            .filter(|t| !prj.devices.get(*t).is_some_and(|d| !d.parts.is_empty()))
            .filter(|t| !["-BP1", "-GQ1", "-MB1", "-MB2"].contains(&t.as_str()))
            .collect();
        assert!(missing.is_empty(), "{}: devices without part {missing:?}", e.key);
        // Saved and opened again unchanged.
        let json = serde_json::to_string(&prj).unwrap();
        let back: Project = serde_json::from_str(&json).unwrap();
        assert_eq!(serde_json::to_string(&back).unwrap(), json, "{}", e.key);
    }
}

#[test]
fn lamp_circuit_takes_the_potential_colours() {
    let (prj, an) = checked("lamp_circuit");
    assert!(prj.wire_colors);
    let lamp = prj.pages[1].symbols.iter().find(|s| s.tag == "-P1").unwrap();
    assert_eq!(an.netlist.nets[an.netlist.pin_net[&(lamp.id, 1)]].name, "0V");
    let bom = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    for part in ["GEN.MCB-1P-C6", "GEN.PB-GN-1NO", "GEN.SEL-2POS-1NO", "GEN.LMP-GN-24", "GEN.LMP-RD-24"] {
        assert!(bom.rows.iter().any(|r| r[3] == part), "{part}");
    }
}

#[test]
fn relay_holds_itself_through_the_strip() {
    let (prj, an) = checked("self_holding_relay");
    let idx = Index::build(&prj);
    assert_eq!((idx.devices["-K1"].mains.len(), idx.devices["-K1"].auxs.len()), (1, 3));
    let strip = an.report(&prj, ReportKind::TerminalDiagram).unwrap();
    assert_eq!(strip.rows.iter().filter(|r| r[0] == "-X1" && r[1].parse::<u32>().is_ok()).count(), 4);
    // Wire numbers after page and column.
    assert!(prj.pages[1].wires.iter().any(|w| w.number.starts_with("2.")), "page / column numbers");
}

#[test]
fn star_delta_cable_uses_all_cores() {
    let (prj, an) = checked("star_delta_starter");
    let mut cores: Vec<&str> = prj.pages.iter().flat_map(|p| &p.wires).filter(|w| w.cable == "-W1").map(|w| w.core.as_str()).collect();
    cores.sort();
    assert_eq!(cores, ["1", "2", "3", "4", "5", "6", "GNYE"]);
    let idx = Index::build(&prj);
    for k in ["-K1", "-K2", "-K3", "-K4"] {
        assert_eq!(idx.devices[k].mains.len(), 1, "{k}");
    }
    assert!(an.report(&prj, ReportKind::CableOverview).unwrap().rows.iter().any(|r| r.contains(&"-W1".to_string())));
}

#[test]
fn press_links_fluid_and_electrical_devices() {
    let (prj, an) = checked("pneumatic_press");
    assert_eq!(crate::electrofluid::linked(&prj, "-QM1"), ["-MB1".to_string()]);
    assert_eq!(crate::electrofluid::linked(&prj, "-QM2"), ["-MB2".to_string()]);
    assert_eq!(prj.fluid.groups[0].members, ["-QM1", "-QM2"]);
    assert!(!an.report(&prj, ReportKind::HoseList).unwrap().rows.is_empty());
}

#[test]
fn pump_station_is_structured() {
    let (prj, an) = checked("pump_station");
    let idx = Index::build(&prj);
    for tag in ["=SUP+CAB1-T1", "=P1+CAB1-K1", "=P1+FLD-M1", "=P2+CAB1-T1", "=PLC+CAB1-A3", "=SUP+FLD-X10"] {
        assert!(idx.devices.contains_key(tag), "{tag}");
    }
    assert_eq!(prj.sections.len(), 6);
    assert!(prj.pages.iter().all(|p| p.name.contains('.')), "page names carry the section numbers");
    let plc = an.report(&prj, ReportKind::PlcAssignment).unwrap();
    assert!(plc.rows.iter().any(|r| r.contains(&"IW64".to_string())), "{plc:?}");
    // Planned in the single-line diagram and engineered in the schematic.
    let devices = crate::singleline::devices(&prj, &idx);
    assert!(devices.iter().filter(|d| d.status == crate::singleline::Status::Engineered).count() >= 5);
}

#[test]
fn workflow_marks_the_change_since_the_release() {
    use crate::revision::{pending, status, Status};
    let (prj, _) = checked("engineering_workflow");
    assert_eq!(status(&prj, 0), Status::Released);
    assert_eq!(status(&prj, 1), Status::InReview);
    let changed: Vec<String> = pending(&prj).into_iter().map(|c| c.name).collect();
    assert_eq!(changed, ["3"]);
    assert!(prj.pages.iter().filter(|p| p.name != "3").all(|p| p.locked));
    assert_eq!(prj.review.requests[0].number, "CR-001");
    assert_eq!(prj.baselines.len(), 1);
}

#[test]
fn cabinets_stand_in_a_line_up() {
    use crate::panel::{lineups, locate, ItemKind};
    let (prj, an) = checked("cabinet_lineup");
    let l = &prj.layout;
    assert_eq!(lineups(l), vec![vec![0, 1]]);
    // Every device of the cabinets is placed, the converter in +CAB2.
    let idx = Index::build(&prj);
    for tag in idx.devices.keys().filter(|t| t.contains("+CAB")) {
        assert!(locate(&prj, tag).is_some(), "{tag} not placed");
    }
    assert_eq!(locate(&prj, "=P2+CAB2-T1").map(|(e, _)| e), Some(1));
    let kinds = |e: usize, k: ItemKind| l.enclosures[e].items.iter().filter(|i| i.kind == k).count();
    assert_eq!(kinds(0, ItemKind::Busbar), 1);
    assert_eq!(kinds(0, ItemKind::Cutout) + kinds(1, ItemKind::Cutout), 2, "cable passage");
    assert!(crate::routing3d::accessories(&prj, 0, "Cable gland").len() == 1);
    assert!(!an.report(&prj, ReportKind::DuctFill).unwrap().rows.is_empty());
    assert_eq!(an.report(&prj, ReportKind::Climate).unwrap().rows.len(), 2);
    assert!(prj.pages.iter().filter(|p| matches!(p.kind, PageKind::Mounting(_))).count() == 2);
}

#[test]
fn electrical_room_holds_the_line_up() {
    use crate::model::panel::Opening;
    use crate::panel::{cabinets_in, locate, mount_ok, Face};
    let (prj, _) = checked("electrical_room");
    let l = &prj.layout;
    let r = l.enclosures.iter().position(|e| e.is_room()).expect("room");
    let room = &l.enclosures[r];
    assert_eq!((room.outline.len(), room.inner_walls.len()), (6, 1));
    let openings: Vec<Opening> = room.items.iter().map(|i| i.opening).filter(|o| !o.is_cutout()).collect();
    assert_eq!(openings, [Opening::Door, Opening::Window]);
    assert_eq!(cabinets_in(l, r), [0, 1]);
    assert!(room.items.iter().all(|i| mount_ok(&prj, room, i)));
    assert!(matches!(locate(&prj, "=SUP+ER1-X2"), Some((e, it)) if e == r && it.face == Face::Wall(5)));
    assert!(room.items.iter().any(|i| i.part == room::LIGHT && i.tag.is_empty()));
    assert!(room.items.iter().any(|i| i.part == room::TRAY && i.face == Face::Roof));
}

#[test]
fn pump_room_routes_pipes_and_motor_cables() {
    use crate::panel::{mount_ok, Face};
    use crate::routing3d::RouteKind;
    let (prj, an) = checked("pump_room");
    let l = &prj.layout;
    let r = l.enclosures.iter().position(|e| e.is_room()).expect("room");
    let room = &l.enclosures[r];
    // Vessel, pump and both motors stand on the floor; valves and gauge on the wall.
    let on = |face: Face| room.items.iter().filter(|i| i.face == face && !i.tag.is_empty()).map(|i| i.tag.as_str()).collect::<Vec<_>>();
    assert_eq!(on(Face::Bottom), ["=P1+ER1-CM1", "=P1+ER1-GP1", "=P1+FLD-M1", "=P2+FLD-M1"]);
    assert!(["=P1+ER1-QM1", "=P1+ER1-QM2", "=P1+ER1-BP1"].iter().all(|t| on(Face::Wall(2)).contains(t)));
    assert!(room.items.iter().all(|i| mount_ok(&prj, room, i)));
    assert_eq!(room.paths.iter().filter(|p| p.face.is_some()).count(), 1);
    // The pipes run through the room only, along the pipe runs (the pressure side to
    // the gauge and on to the vessel are two).
    let pipes: Vec<_> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Tube).collect();
    assert_eq!(pipes.len(), 5);
    assert!(pipes.iter().all(|i| i.routed() && i.legs.is_empty() && i.enclosures == [r] && i.diameter == 25.0), "{pipes:#?}");
    assert!(an.routing.paths.iter().any(|u| u.enclosure == r && u.items >= 2), "{:?}", an.routing.paths);
    // The motor cables leave their cabinets through the roof.
    for tag in ["=P1-W1", "=P2-W1"] {
        let c = an.routing.item(tag).expect(tag);
        assert!(c.routed() && c.legs.len() == 2 && c.enclosures.contains(&r), "{tag}: {:?} {:?}", c.status, c.enclosures);
    }
    assert!(!an.issues.iter().any(|i| i.message().contains("enters through the roof")));
}

#[test]
fn plant_buildings_route_through_the_trench() {
    use crate::panel::{self, buildings as bld, Face, Via};
    let (prj, an) = checked("plant_buildings");
    let l = &prj.layout;
    let room = |name: &str| l.enclosures.iter().position(|e| e.name == name).unwrap_or_else(|| panic!("{name}"));
    let (er, cr, ph) = (room("+ER1"), room("+CR1"), room("+PH1"));
    assert_eq!(l.buildings.len(), 2);
    assert_eq!(bld::building_rooms(l, 0), [er, cr]);
    assert_eq!(bld::building_rooms(l, 1), [ph]);
    // The control room stands on the level above the electrical room.
    let poses = panel::site_poses(&prj);
    assert_eq!((poses[er].at[1], poses[cr].at[1]), (0.0, 3500.0));
    let links = panel::wall_links(&prj);
    assert!(links.iter().any(|w| w.via == Via::Shaft && (w.from, w.to) == (er, cr)), "{links:?}");
    assert!(links.iter().any(|w| w.via == Via::Site && (w.from, w.to) == (er, ph)), "{links:?}");
    // The motor cable of pump 2 runs from its cabinet through the electrical room, the
    // trench and the pump house to the motor.
    let c = an.routing.item("=P2-W1").expect("=P2-W1");
    assert!(c.routed() && c.enclosures.contains(&er) && c.enclosures.contains(&ph), "{:?} {:?}", c.status, c.enclosures);
    let trench = bld::site_link_length(&prj, &poses, &l.site_links[0]).unwrap();
    assert!(c.length > trench, "{} {trench}", c.length);
    // On the outside of the buildings.
    let outside = |ri: usize, face: Face| l.enclosures[ri].items.iter().filter(|i| i.face == face).map(|i| i.part.as_str()).collect::<Vec<_>>();
    assert_eq!(outside(er, Face::Outside(0)), [site::FACADE_LIGHT]);
    assert_eq!(outside(ph, Face::Outside(2)), [site::FACADE_LIGHT]);
    assert_eq!(outside(cr, Face::RoofTop), [site::WEATHER]);
}

#[test]
fn building_services_route_from_room_to_room() {
    use crate::model::panel::Opening;
    use crate::routing3d::RouteKind;
    let (prj, an) = checked("building_services");
    let l = &prj.layout;
    let room = |name: &str| l.enclosures.iter().position(|e| e.is_room() && e.name == name).expect(name);
    let (hr, tr) = (room("+HR"), room("+TR"));
    // Manufacturer parts from the catalogs of the parts server.
    for part in ["HAG.LT0057", "HAG.LT056", "HAG.LT253", "HAG.ECR300C", "VIC.PMP482505010", "VIC.PIN482601000", "VIC.BPP900450100", "PYL.US5000", "DEH.952515"] {
        assert!(prj.parts.iter().any(|p| p.number == part), "{part}");
    }
    // The residual current device carries its signal contact beside it on the rail;
    // its NC contact switches the trip lamp.
    let f10 = "=ESS+ACD-F10";
    assert_eq!(crate::parts::attached_to(&prj, None, f10, "HAG.CZ001").map(|p| p.number.as_str()), Some("HAG.CDA440D"));
    let (ei, it) = crate::panel::locate(&prj, f10).expect("-F10 placed");
    assert_eq!((l.enclosures[ei].name.as_str(), it.w), ("+ACD", 79.0));
    assert!(crate::panel::device_points(&prj, it).iter().any(|p| p.pin == "21" && p.pos[0] < it.x + 9.0));
    let f10s: Vec<_> = prj.pages.iter().flat_map(|p| &p.symbols).filter(|s| s.tag.ends_with("-F10")).map(|s| (s.tag.clone(), s.sym.clone(), s.pins.clone())).collect();
    assert!(f10s.iter().any(|(_, sym, pins)| sym == "nc" && pins == &["21", "22"]), "{f10s:?}");
    // Sections in two levels, the pages numbered after them.
    assert!(prj.sections.iter().any(|s| s.parent != 0));
    assert!(prj.pages.iter().any(|p| p.name == "2.1.1") && prj.pages.iter().any(|p| p.name == "7.2.4"), "{:?}", prj.pages.iter().map(|p| &p.name).collect::<Vec<_>>());
    // Two building entries lead into the house connection room, wall penetrations on.
    let entries = l.enclosures[hr].items.iter().filter(|i| i.opening == Opening::Entry).count();
    assert_eq!(entries, 2);
    assert!(crate::panel::wall_links(&prj).iter().filter(|w| (w.from, w.to) == (hr, tr)).count() >= 2);
    // The house connection box is an enclosure with a hinged cover on the left wall of
    // the room, above the cable entry, the NH fuse base and the PEN terminal block in it.
    let hak = l.enclosures.iter().position(|e| e.name == "+HAK").expect("+HAK");
    assert_eq!(crate::panel::room_of(l, hak), Some(hr));
    assert_eq!(l.enclosures[hak].build.system, crate::parts::mechanics::HOUSE_CONNECTION_SYSTEM);
    assert!(crate::panel::components(&l.enclosures[hak]).iter().any(|c| c.role == crate::panel::Role::Door));
    for tag in ["=GRID+HAK-F1", "=GRID+HAK-X0"] {
        assert_eq!(crate::panel::locate(&prj, tag).map(|(k, _)| k), Some(hak), "{tag}");
    }
    // The supply cable comes in through its building entry, through the room and up
    // through the bottom of the box; the main cable leaves it through its top.
    let w1 = an.routing.item("=GRID-W1").expect("W1 routed");
    assert!(w1.routed() && w1.enclosures.contains(&hr) && w1.enclosures.contains(&hak), "{:?} {:?}", w1.status, w1.enclosures);
    assert!(w1.ends.iter().any(|e| e.item.is_some_and(|id| l.enclosures[hr].item(id).is_some_and(|i| i.opening == Opening::Entry))));
    let w2 = an.routing.item("=GRID-W2").expect("W2 routed");
    assert!(w2.routed() && w2.enclosures.contains(&hak), "{:?}", w2.status);
    let leaves = |c: &crate::routing3d::RoutedItem| c.legs.iter().find(|leg| leg.enclosure == hak).map(|leg| (leg.points[0][1], leg.points[leg.points.len() - 1][1]));
    let (bottom, top) = (l.enclosures[hak].height, 0.0);
    let pos = crate::panel::positions(l);
    let (a, b) = leaves(w1).expect("W1 in the box");
    assert!((a - pos[hak][1] - bottom).abs() < 20.0 || (b - pos[hak][1] - bottom).abs() < 20.0, "W1 through the bottom: {a} {b}");
    let (a, b) = leaves(w2).expect("W2 in the box");
    assert!((a - pos[hak][1] - top).abs() < 20.0 || (b - pos[hak][1] - top).abs() < 20.0, "W2 through the top: {a} {b}");
    // The main earthing busbar beside it: joined to the PE busbar of the main
    // distribution by the main bonding conductor, and to the lug of the foundation earth
    // electrode coming out of the wall right below it.
    let md = l.enclosures.iter().position(|e| e.name == "+MD").expect("+MD");
    let w3 = an.routing.item("=GRID-W3").expect("W3 routed");
    assert!(w3.routed() && w3.enclosures.contains(&md) && w3.enclosures.contains(&hr), "{:?} {:?}", w3.status, w3.enclosures);
    let (bar, lug) = (crate::panel::locate(&prj, "=GRID+HR-X1").expect("busbar"), crate::panel::locate(&prj, "=GRID+HR-E1").expect("lug"));
    assert!(bar.0 == hr && lug.0 == hr);
    let (bar, lug) = (bar.1, lug.1);
    assert!(lug.x >= bar.x && lug.x + lug.w <= bar.x + bar.w && lug.y >= bar.y + bar.h, "{lug:?} {bar:?}");
    assert_eq!(lug.y + lug.h, l.enclosures[hr].height, "out of the wall at the floor");
    assert!(an.routing.items.iter().any(|i| i.key == "=GRID+HR-X1:1|=GRID+HR-E1:1" && i.routed()));
    // The feeders of the main distribution and the water pipe pass into the technical room.
    for tag in ["=MD-W11", "=MD-W12", "=MD-W13"] {
        let c = an.routing.item(tag).expect(tag);
        assert!(c.routed() && c.enclosures.contains(&hr) && c.enclosures.contains(&tr), "{tag}: {:?} {:?}", c.status, c.enclosures);
    }
    // They pass their wall penetration side by side, a centimetre between them.
    let passes: Vec<(crate::parts::vec3::V3, f32)> = ["=MD-W11", "=MD-W12", "=MD-W13"].iter().map(|tag| an.routing.item(tag).expect(tag)).map(|c| (*c.legs[1].points.last().expect("through the room"), c.diameter)).collect();
    for (i, (q, d)) in passes.iter().enumerate() {
        assert!(passes[..i].iter().all(|(o, e)| crate::parts::vec3::dist(*q, *o) >= (d + e) / 2.0 + 9.0), "{passes:?}");
    }
    let pipes: Vec<_> = an.routing.items.iter().filter(|i| i.kind == RouteKind::Tube).collect();
    assert!(pipes.iter().all(|i| i.routed()), "{pipes:#?}");
    assert!(pipes.iter().any(|i| i.enclosures == [hr, tr]), "the pipe into the storage tank");
    // Compressed air, heating water and hot water in their own pipes, the compressor
    // and the heat pump fed from the pump cabinet.
    let media: std::collections::BTreeSet<_> = prj.pages.iter().flat_map(|p| &p.wires).filter_map(|w| w.medium).collect();
    use crate::model::fluid::Medium;
    assert!([Medium::Water, Medium::CompressedAir, Medium::HeatingWater, Medium::HotWater].iter().all(|m| media.contains(m)), "{media:?}");
    for tag in ["=AIR-W1", "=HEAT-W1", "=HEAT-W2", "=HEAT-W3"] {
        assert!(an.routing.item(tag).is_some_and(|c| c.routed() && c.enclosures.contains(&tr)), "{tag}");
    }
    assert!(pipes.len() > 30, "{}", pipes.len());
    // Batteries and network in the 19-inch rack.
    let rack = l.enclosures.iter().position(crate::panel::is_rack).expect("rack");
    assert_eq!(crate::panel::rack::occupied(&l.enclosures[rack]).len(), 9);
    // The BMS cable ends at the CAN port of the battery (a connection point of the
    // part, not the middle of the front); the battery cable carries + and - and parts at
    // both ends: to + and - of the battery, to the fuse F1 and the stud N1 of the
    // negative busbar of the Lynx distributor.
    let pos = crate::panel::positions(l);
    let point = |tag: &str, pin: &str| {
        let (ei, it) = crate::panel::locate(&prj, tag).expect(tag);
        let q = crate::panel::device_points(&prj, it).into_iter().find(|p| p.pin == pin).expect(pin).pos;
        crate::parts::vec3::add(crate::panel::face_point(&l.enclosures[ei], it.face, q), pos[ei])
    };
    let bms = an.routing.item("=ESS-W96").expect("=ESS-W96");
    assert!(bms.routed() && bms.ends.iter().any(|e| e.tag == "=ESS+RACK-G1" && e.point == Some(point("=ESS+RACK-G1", "CAN")) && e.cores.is_empty()));
    // So do the DC cables of the inverters.
    for (cable, tag, pins) in [
        ("=ESS-W71", "=ESS+RACK-G1", ["+", "-"]),
        ("=ESS-W71", "=ESS+TR-F1", ["F1", "N1"]),
        ("=ESS-W81", "=ESS+TR-F2", ["F1", "N1"]),
        ("=ESS-W81", "=ESS+TR-T1", ["DC+", "DC-"]),
        ("=ESS-W85", "=ESS+TR-F3", ["F1", "N1"]),
    ] {
        let c = an.routing.item(cable).expect(cable);
        assert!(c.routed(), "{cable}: {:?}", c.status);
        let end = c.ends.iter().find(|e| e.tag == tag).unwrap_or_else(|| panic!("{cable} {tag}: {:?}", c.ends));
        let cores: Vec<_> = pins.iter().map(|p| point(tag, p)).collect();
        assert_eq!(end.cores, cores, "{cable} {tag}");
        let middle = crate::parts::vec3::add(cores[0], cores[1]).map(|v| v / 2.0);
        assert!(end.point.is_some_and(|q| (0..3).all(|k| (q[k] - middle[k]).abs() < 0.01)), "{tag}: {:?}", end.point);
    }
    // The battery cables leave their battery at the front and run past the others, not
    // through them (a device is passable only where a route leaves it).
    let racked = &l.enclosures[rack];
    let batteries: Vec<_> = racked.items.iter().filter(|i| i.tag.contains("RACK-G")).map(|i| crate::panel::item_box(racked, i)).collect();
    for cable in ["=ESS-W71", "=ESS-W72", "=ESS-W73", "=ESS-W74"] {
        let c = an.routing.item(cable).expect(cable);
        let (_, points) = c.stretches().into_iter().find(|(leg, _)| *leg == Some(rack)).expect("rack leg");
        for seg in points[1..].windows(2) {
            let mid = [0, 1, 2].map(|k| (seg[0][k] + seg[1][k]) / 2.0 - pos[rack][k]);
            assert!(!batteries.iter().any(|(lo, hi)| (0..3).all(|k| mid[k] > lo[k] && mid[k] < hi[k])), "{cable}: {seg:?}");
        }
    }
    // Each of them through a cable gland of its own in the roof of the rack.
    let mut tops: Vec<[i32; 3]> = ["=ESS-W71", "=ESS-W72", "=ESS-W73", "=ESS-W74"]
        .iter()
        .map(|cable| {
            let (_, points) = an.routing.item(cable).unwrap().stretches().into_iter().find(|(leg, _)| *leg == Some(rack)).unwrap();
            let q = points.iter().min_by(|a, b| a[1].total_cmp(&b[1])).unwrap();
            [0, 1, 2].map(|k| q[k].round() as i32)
        })
        .collect();
    tops.sort_unstable();
    tops.dedup();
    assert_eq!(tops.len(), 4, "{tops:?}");
    // The plate view of a cabinet shows the legs of its line-up only: the cable from
    // the main distribution to the PLC cabinet stays on the plate of each.
    let w11 = an.routing.item("=MD-W11").expect("=MD-W11");
    assert_eq!(w11.legs.len(), 4, "{:?}", w11.enclosures);
    for name in ["+MD", "+CC1"] {
        let ei = l.enclosures.iter().position(|e| e.name == name).expect(name);
        let lineup = crate::panel::lineup_of(l, ei);
        let x = (pos[lineup[0]][0], lineup.iter().map(|&k| pos[k][0] + l.enclosures[k].width).fold(0.0, f32::max));
        let points: Vec<_> = w11.stretches_in_lineup(l, ei).concat();
        assert!(!points.is_empty() && points.iter().all(|q| q[0] >= x.0 && q[0] <= x.1), "{name}: {points:?}");
    }
    // PLC cards and EtherCAT terminals, NH fuse switches of three sizes.
    let plc = an.report(&prj, ReportKind::PlcAssignment).unwrap();
    for addr in ["I0.0", "Q0.1", "IW66", "DI4", "DO2"] {
        assert!(plc.rows.iter().any(|r| r.contains(&addr.to_string())), "{addr}");
    }
    let idx = Index::build(&prj);
    let devices = crate::singleline::devices(&prj, &idx);
    assert!(devices.iter().filter(|d| d.status == crate::singleline::Status::Engineered).count() >= 12);
}

#[test]
fn terminal_strips_are_drawn_with_their_models() {
    use crate::panel::{part_by_number, strip_cells, ItemKind};
    let mut strips = 0;
    for ex in &EXAMPLES {
        let prj = example(ex.key).unwrap();
        for it in prj.layout.enclosures.iter().flat_map(|e| &e.items).filter(|i| i.kind == ItemKind::Device) {
            let cells = strip_cells(&prj, it);
            if cells.is_empty() {
                continue;
            }
            strips += 1;
            // Every terminal, end plate and end bracket has its 3D model (no plain box).
            for c in &cells {
                assert!(part_by_number(&prj, &c.part).is_some_and(|p| p.model_3d.is_some()), "{}: {} {:?}", ex.key, it.tag, c.part);
            }
            // Side by side on the rail: the strip is as deep as its deepest part.
            let deepest = cells.iter().filter_map(|c| part_by_number(&prj, &c.part).and_then(|p| p.size())).map(|s| s[2]).fold(0.0, f32::max);
            assert!((it.d - deepest).abs() < 0.01, "{}: {} {} deep, parts {}", ex.key, it.tag, it.d, deepest);
            // Strips on a rail end in end brackets.
            if part_by_number(&prj, &it.part).is_some_and(|p| p.mounting.starts_with("Mounting rail")) {
                let bracket = |c: &crate::panel::Cell| part_by_number(&prj, &c.part).is_some_and(|p| p.subgroup == "End bracket");
                assert!(bracket(&cells[0]) && bracket(&cells[cells.len() - 1]), "{}: {}", ex.key, it.tag);
            }
        }
    }
    assert!(strips > 20, "{strips}");
}

#[test]
fn pumps_and_the_compressor_are_driven_by_their_motors() {
    use crate::panel::{drives, DriveKind};
    let kinds = |prj: &Project| -> Vec<DriveKind> { prj.layout.enclosures.iter().flat_map(|e| drives(prj, e)).map(|d| d.kind).collect() };
    assert_eq!(kinds(&example("pump_room").unwrap()), [DriveKind::Coupling]);
    let building = kinds(&example("building_services").unwrap());
    let count = |k: DriveKind| building.iter().filter(|b| **b == k).count();
    assert_eq!((count(DriveKind::Coupling), count(DriveKind::Belt)), (2, 1), "{building:?}");
}

#[test]
fn connectors_show_their_pin_outs_and_the_ribbon_cable() {
    let (prj, an) = checked("connector_handling");
    assert_eq!(crate::plugs::plug_tags(&prj), ["-X11", "-X12", "-X13", "-X14", "-X15", "-X20", "-X21"]);
    let po = |plug: &str| crate::pinout::pinout(&prj, plug, &an.netlist, &an.open);
    // M12: the field cable's core colours on the socket side, pins 2 and 3 spare.
    let m12 = po("-X11");
    assert_eq!(m12.layout.as_ref().map(|l| l.text()), Some("circle 4".to_string()));
    let v: Vec<(&str, bool, &str)> = m12.pins.iter().map(|p| (p.label.as_str(), p.used(), p.socket.color.as_str())).collect();
    assert_eq!(v, [("1", true, "BN"), ("2", false, ""), ("3", false, ""), ("4", true, "BK")]);
    assert_eq!(m12.pins[0].socket.core, "-W11:BN");
    // M8 3-pin: designations 1, 3, 4 on a face of four places.
    assert_eq!(po("-X12").pins.iter().map(|p| p.slot).collect::<Vec<_>>(), [Some(0), Some(2), Some(3)]);
    for (plug, layout) in [("-X13", "circle 4"), ("-X14", "circle 3"), ("-X15", "dsub 9"), ("-X20", "zigzag 10")] {
        assert_eq!(po(plug).layout.as_ref().map(|l| l.text()).as_deref(), Some(layout), "{plug}");
    }
    // The ribbon cable: core n on pin n at both IDC sockets, core 1 red, the others grey.
    for plug in ["-X20", "-X21"] {
        let idc = po(plug);
        assert_eq!(idc.pins.len(), 10);
        for p in idc.pins.iter().take(5) {
            assert_eq!(p.socket.core, format!("-W20:{}", p.label), "{plug}");
            assert_eq!(p.socket.color, if p.label == "1" { "RD" } else { "GY" }, "{plug}");
        }
        assert!(idc.pins.iter().skip(5).all(|p| !p.used()));
    }
    let bom = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    for part in ["GEN.CBL-FLAT-10", "GEN.CON-IDC-10F", "GEN.CON-BOX-10M", "GEN.CON-GX16-4F", "GEN.CON-DSUB-9M", "GEN.CON-M8-3F"] {
        assert!(bom.rows.iter().any(|r| r[3] == part), "{part}");
    }
    let svg = crate::pinout::svg(&prj, &an.netlist, &an.open, &[]);
    assert!(svg.contains("-X21") && svg.contains("zigzag 10"));
}

#[test]
fn transformer_metering_builds_the_univers_cabinet() {
    let (prj, an) = checked("pv_transformer_metering");
    let hv = &prj.layout.enclosures[0];
    assert_eq!((hv.name.as_str(), hv.part.as_str()), ("+HV", "HAG.FR25S2"));
    assert!(hv.build.plinth_included && hv.build.plinth == 200.0, "the cabinet comes with its plinth");
    let eyes = hv.items.iter().filter(|i| i.part == "HAG.FZ831" && i.face == crate::panel::Face::Roof).count();
    assert_eq!(eyes, 2);
    // The NH1 fuse switch disconnectors and the transformers stand on their modules.
    for (tag, module) in [("=MS1+HV-F10", "HAG.UE31E1B"), ("=MS2+HV-F40", "HAG.UE31E1B"), ("=MS1+HV-T1", "HAG.UE31SS400")] {
        let a = crate::panel::assembly(&prj, tag).unwrap_or_else(|| panic!("{tag}"));
        assert_eq!(a.behind.iter().map(|b| b.part.number.as_str()).collect::<Vec<_>>(), [module], "{tag}");
        assert_eq!((a.w, a.h), (250.0, 450.0), "{tag}");
    }
    // The coupling switch: motor operator in front, release and auxiliary switch built in.
    let q = crate::panel::assembly(&prj, "=NA+HV-Q30").unwrap();
    assert_eq!(q.front.iter().map(|f| f.part.number.as_str()).collect::<Vec<_>>(), ["HAG.HXT041H"]);
    assert_eq!(q.inside.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["HAG.HXA011H", "HAG.HXA021H"]);
    let idx = Index::build(&prj);
    assert_eq!(idx.devices["=NA+HV-Q30"].mains.len(), 3, "breaker, release and motor operator");
    let bom = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    let qty = |part: &str| bom.rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap_or_default();
    assert_eq!((qty("HAG.LT153").as_str(), qty("GEN.NH1-GG-125A").as_str(), qty("HAG.UE31E1B").as_str()), ("2", "6", "2"));
    for part in ["HAG.FR25S2", "HAG.SPA811Z", "HAG.HA354", "HAG.EU400", "HAG.HTG911H", "HAG.EZM100", "HAG.L73M", "HAG.UD21APZ", "HAG.FP62ZG2"] {
        assert!(!qty(part).is_empty(), "{part} in the bill of materials");
    }
}

#[test]
fn industrial_main_distribution_measures_every_feeder() {
    let (prj, an) = checked("industrial_main_distribution");
    let (hv1, hv2) = (&prj.layout.enclosures[0], &prj.layout.enclosures[1]);
    assert_eq!((hv1.part.as_str(), hv2.part.as_str()), ("HAG.FR24H2", "HAG.FR24H2"));
    assert_eq!(hv2.bayed_to, hv1.id, "+HV2 stands right of +HV1");
    // Behind each measured strip a current transformer per pole, along the strip.
    for tag in ["=DIST+HV2-Q21", "=DIST+HV2-Q22", "=DIST+HV2-Q23", "=DIST+HV2-Q24"] {
        let a = crate::panel::assembly(&prj, tag).unwrap_or_else(|| panic!("{tag}"));
        assert_eq!(a.behind.iter().map(|b| b.part.number.as_str()).collect::<Vec<_>>(), ["HAG.LVZW150"; 3], "{tag}");
        assert!(a.behind.windows(2).all(|w| w[1].across > w[0].across), "{tag}");
    }
    // The isolation of the metering protects nothing: solid links instead of fuses.
    for tag in ["=MS+HV1-F10", "=MS+HV1-Q10"] {
        assert!(crate::sizing::isolates_only(&prj, tag), "{tag}");
    }
    // The large cables come in through the membrane of a cable entry flange.
    assert!(hv1.items.iter().any(|i| i.part == "HAG.FZ402" && i.face == crate::panel::Face::Bottom));
    let bom = an.report(&prj, ReportKind::BillOfMaterials).unwrap();
    let qty = |part: &str| bom.rows.iter().find(|r| r[3] == part).map(|r| r[1].clone()).unwrap_or_default();
    // Busbar terminals: three per supply cable, two for their PEN, two for N and PE of
    // the charging station, three per coupling switch.
    assert_eq!((qty("HAG.LVZW150").as_str(), qty("LOX.100652").as_str(), qty("HAG.KS30C").as_str()), ("12", "4", "16"));
    for part in ["HAG.LVSG1CPX", "HAG.LVSG3CPX", "HAG.LVSG00RPX", "HAG.HNT161GR", "HAG.HXT041H", "HAG.CDB640E", "JAN.52.16.202", "HAG.LVZVA-V"] {
        assert!(!qty(part).is_empty(), "{part} in the bill of materials");
    }
    // The distribution busbar runs through both cabinets: one system, the strip that
    // isolates it in +HV1 and the feeders in +HV2 are connected by its bars.
    let pos = crate::panel::positions(&prj.layout);
    let system = |tag: &str| crate::panel::locate(&prj, tag).and_then(|(ei, it)| crate::panel::busbar_system(&prj.layout, &pos, ei, it));
    assert!(system("=MS+HV1-Q10").is_some() && system("=MS+HV1-Q10") == system("=DIST+HV2-Q21"));
    assert_ne!(system("=MS+HV1-F10"), system("=MS+HV1-Q10"));
    // The only conductors as thick as that that are no bars: between the two strips.
    let thick: Vec<&crate::panel::RoutedWire> = an.wiring.iter().filter(|w| crate::harness::outer_diameter(&w.cross_section) > 15.0).collect();
    assert_eq!(thick.len(), 3, "{:?}", thick.iter().map(|w| (&w.from, &w.to)).collect::<Vec<_>>());
    assert!(thick.iter().all(|w| w.from.starts_with("=MS+HV1-F10") && w.to.starts_with("=MS+HV1-Q10")));
}

#[test]
fn office_building_has_its_roof_stair_and_installation() {
    let (prj, _) = checked("office_building");
    let b = prj.layout.buildings.iter().find(|b| b.name == "+B3").unwrap();
    assert_eq!((b.levels.len(), b.stairs.len(), b.roofs.len(), b.roofs[0].windows.len()), (2, 1, 1, 2));
    let g = crate::panel::stairs::geometry(b, &b.stairs[0]).unwrap();
    assert!(g.pieces.iter().any(|p| p.landing) && (160.0..=190.0).contains(&g.rise));
    // The attic's ceiling slopes down to the knee wall, the office's stays flat.
    let room = |n: &str| prj.layout.enclosures.iter().position(|e| e.name == n).unwrap();
    let (office, attic) = (room("+OF1"), room("+AT1"));
    assert!(crate::panel::roofs::ceiling(&prj, office).is_none());
    let c = crate::panel::roofs::ceiling(&prj, attic).unwrap();
    let (lo, hi) = c.range(&crate::panel::roofs::floor_polys(&prj.layout.enclosures[attic]));
    assert!((1000.0..1400.0).contains(&lo) && (hi - 2700.0).abs() < 1.0, "{lo} {hi}");
    // Outlets and switches at their mounting heights, from the shipped catalogs.
    let e = &prj.layout.enclosures[office];
    let middle = |part: &str| e.items.iter().find(|i| i.part == part).map(|i| e.height - i.y - i.h / 2.0).unwrap();
    assert_eq!((middle("GIRA.418803").round(), middle("GIRA.310600").round()), (300.0, 1050.0));
    for n in ["GIRA.418803", "GIRA.021103", "COSMO.CVS6", "MDT.BE-TA55P4.01"] {
        assert!(prj.parts.iter().any(|p| p.number == n), "{n} in the project");
    }
    assert!(prj.layout.enclosures[attic].items.iter().any(|i| i.part == "COSMO.CBRTLTH"));
    // The PV array in the rear slope: ten LONGi modules, 4.3 kWp, with its rails and hooks
    // in the bill of materials.
    let a = &b.roofs[0].solar[0];
    assert_eq!((crate::panel::solar::module_count(a), crate::panel::solar::peak_power(&prj, a)), (10, Some(4.3)));
    let material = crate::panel::layout_material(&prj);
    for (part, qty) in [("LONGI.LR5-54HTH-430M", 10.0), ("GEN.PV-RAIL-40", 8.0), ("GEN.PV-CLAMP-MID", 16.0)] {
        assert!(material.iter().any(|(p, q, _)| p == part && *q == qty), "{part} {qty}: {material:?}");
    }
    // One string of ten on the inverter: 423 V at -10 °C, within its 1000 V.
    let strings = crate::panel::solar::strings::all_strings(&prj);
    let v = strings[0].values.unwrap();
    assert_eq!((strings.len(), strings[0].string.series, strings[0].string.inverter.as_str()), (1, 10, "=PV-T1"));
    assert!((v.voc_max - 422.8).abs() < 0.1 && v.vmp_min > 160.0, "{v:?}");
}
