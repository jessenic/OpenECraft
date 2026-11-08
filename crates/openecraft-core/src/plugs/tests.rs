use super::*;
use crate::geom::p;
use crate::harness::Harness;
use crate::ops::{self, PlaceCtx};
use crate::parts::{FunctionTemplate, Part, PartRef};
use crate::analysis::Analysis;

fn sym(prj: &mut Project, key: &str, x: f32, y: f32, tag: &str, label: &str) -> u64 {
    let def = prj.symbol(key).unwrap();
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst {
        id,
        sym: key.into(),
        pos: p(x, y),
        tag: tag.into(),
        label: label.into(),
        pins: def.pins.iter().map(|x| x.label.to_string()).collect(),
        ..Default::default()
    });
    id
}

fn wire(prj: &mut Project, x1: f32, y1: f32, x2: f32, y2: f32) -> u64 {
    let id = prj.new_id();
    prj.pages[0].wires.push(Wire { id, a: p(x1, y1), b: p(x2, y2), ..Default::default() });
    id
}

/// +24V → pin pair -X10:1 → coil -K1 at x = 100.
fn pair_circuit(prj: &mut Project) -> (u64, u64) {
    sym(prj, "pot_24", 100.0, 76.0, "", "+24V");
    let w1 = wire(prj, 100.0, 80.0, 100.0, 96.0);
    sym(prj, "plug_pair", 100.0, 100.0, "-X10", "1");
    let w2 = wire(prj, 100.0, 104.0, 100.0, 116.0);
    sym(prj, "coil", 100.0, 120.0, "-K1", "");
    (w1, w2)
}

fn plug_part(number: &str, pins: usize) -> Part {
    Part { number: number.into(), functions: (1..=pins).map(|i| FunctionTemplate::new(FnKind::PlugPin, &i.to_string())).collect(), ..Default::default() }
}

fn assign(prj: &mut Project, tag: &str, part: Part) {
    prj.devices.entry(tag.into()).or_default().parts.push(PartRef { number: part.number.clone(), count: 1, total:false });
    prj.parts.push(part);
}

#[test]
fn pin_pairs_pass_the_potential_and_separate_the_connections() {
    let mut prj = Project::default();
    let (w1, w2) = pair_circuit(&mut prj);
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    assert_eq!(nl.net_of_wire(w2).unwrap().name, "+24V");
    assert_ne!(open.wire_net[&w1], open.wire_net[&w2]);
    let an = Analysis::checked(&prj);
    let ends: Vec<(String, String)> = an.connections.iter().map(|c| (pin_designation(&prj, c.a), pin_designation(&prj, c.b))).collect();
    assert_eq!(ends, vec![("-X10:1".to_string(), "-K1:A1".to_string())]);
}

#[test]
fn male_and_female_pins_of_one_designation_are_mated() {
    let mut prj = Project::default();
    sym(&mut prj, "pot_24", 100.0, 76.0, "", "+24V");
    wire(&mut prj, 100.0, 80.0, 100.0, 96.0);
    sym(&mut prj, "plug_f", 100.0, 100.0, "-X10", "1");
    sym(&mut prj, "plug_m", 200.0, 100.0, "-X10", "1");
    let w = wire(&mut prj, 200.0, 104.0, 200.0, 116.0);
    sym(&mut prj, "coil", 200.0, 120.0, "-K1", "");
    let nl = Netlist::build(&prj, true);
    assert_eq!(nl.net_of_wire(w).unwrap().name, "+24V");
    // Mated, not placed twice.
    assert!(problems(&prj).is_empty());
    // A second male pin 1 is.
    sym(&mut prj, "plug_m", 300.0, 100.0, "-X10", "1");
    let msgs: Vec<String> = problems(&prj).into_iter().map(|x| x.0).collect();
    assert_eq!(msgs, vec!["-X10: pin 1 placed 2 times".to_string()]);
    let an = Analysis::checked(&prj);
    assert!(!an.issues.iter().any(|i| i.message().contains("used more than once")));
}

#[test]
fn rows_show_both_sides_order_and_spare_pins() {
    let mut prj = Project::default();
    pair_circuit(&mut prj);
    sym(&mut prj, "plug_pair", 140.0, 100.0, "-X10", "2");
    let nl = Netlist::build(&prj, true);
    let open = Netlist::build(&prj, false);
    let r = rows(&prj, "-X10", &nl, &open);
    assert_eq!(r.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["1", "2"]);
    assert_eq!((r[0].socket.as_str(), r[0].pin.as_str(), r[0].potential.as_str()), ("=+24V", "-K1:A1", "+24V"));
    move_pin(&mut prj, "-X10", 1, true);
    assert_eq!(designations(&prj, "-X10"), ["2", "1"]);
    sort(&mut prj, "-X10");
    assert_eq!(designations(&prj, "-X10"), ["1", "2"]);

    assign(&mut prj, "-X10", plug_part("ACME.M12-4F", 4));
    assign(&mut prj, "-X10", plug_part("ACME.M12-4M", 4));
    assert_eq!(capacity(&prj, "-X10"), Some(4));
    let r = rows(&prj, "-X10", &nl, &open);
    assert_eq!(r.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["1", "2", "3", "4"]);
    assert!(r[2].pins.is_empty() && r[3].function_text == "spare");
}

#[test]
fn more_pins_than_the_parts_have_are_reported() {
    let mut prj = Project::default();
    pair_circuit(&mut prj);
    sym(&mut prj, "plug_pair", 140.0, 100.0, "-X10", "2");
    assign(&mut prj, "-X10", plug_part("ACME.PLUG1", 1));
    let an = Analysis::checked(&prj);
    assert!(an.issues.iter().any(|i| i.message() == "-X10: 2 pins used, the plug parts have 1"));
    // The generic function template check leaves plug pins to this one.
    assert!(!an.issues.iter().any(|i| i.message().contains("Plug pin")));
}

#[test]
fn placement_numbers_pins_per_plug() {
    let mut prj = Project::default();
    let mut ctx = PlaceCtx::default();
    let pair = prj.symbol("plug_pair").unwrap();
    let a = ops::new_symbol(&mut prj, 0, &ctx, &pair, p(100.0, 100.0), 0, false);
    assert_eq!((a.tag.as_str(), a.label.as_str()), ("-X1", "1"));
    ctx.remember(&prj, &a);
    prj.pages[0].symbols.push(a);
    let b = ops::new_symbol(&mut prj, 0, &ctx, &pair, p(120.0, 100.0), 0, false);
    assert_eq!((b.tag.as_str(), b.label.as_str()), ("-X1", "2"));
    prj.pages[0].symbols.push(b);
    // Single sided pins continue after the pairs.
    let male = prj.symbol("plug_m").unwrap();
    let m = ops::new_symbol(&mut prj, 0, &ctx, &male, p(140.0, 100.0), 0, false);
    assert_eq!(m.label, "3");
}

#[test]
fn plug_diagram_report() {
    let mut prj = Project::default();
    pair_circuit(&mut prj);
    let an = Analysis::checked(&prj);
    let t = &an.report(&prj, ReportKind::PlugDiagram).unwrap();
    assert_eq!(t.rows, vec![["-X10", "1", "=+24V", "-K1:A1", "+24V", "/1.3", ""].iter().map(|s| s.to_string()).collect::<Vec<_>>()]);
}

#[test]
fn kbl_names_the_plug_housing() {
    let mut prj = Project::default();
    pair_circuit(&mut prj);
    assign(&mut prj, "-X10", plug_part("ACME.M12-4F", 4));
    prj.harnesses.push(Harness { id: 99, tag: "-WH1".into(), length: 1.0, ..Default::default() });
    crate::harness::assign(&mut prj, "-X10:1", "-K1:A1", "-WH1");
    let kbl = crate::kbl::export(&prj, &Analysis::checked(&prj), None).unwrap();
    let housing = kbl.split("<Connector_housing ").find(|h| h.contains("<Part_number>ACME.M12-4F</Part_number>")).expect(&kbl);
    // All four pins of the part are cavities, also the unused ones.
    assert!(housing.contains("<Housing_type>connector</Housing_type>"));
    assert!(housing.contains("<Number_of_cavities>4</Number_of_cavities>"), "{housing}");
    let id = housing.split('"').nth(1).unwrap();
    let occ = kbl.split("<Connector_occurrence ").find(|o| o.contains("<Id>-X10</Id>")).unwrap();
    assert!(occ.contains(&format!("<Part>{id}</Part>")), "{occ}");
}
