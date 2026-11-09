use super::*;
use crate::sample::demo_project;

fn nl(prj: &Project) -> Netlist {
    Netlist::build(prj, true)
}

#[test]
fn points_of_the_demo() {
    let prj = demo_project();
    assert_eq!(cards(&prj), vec!["-A1".to_string(), "-A2".to_string()]);
    let pts = points(&prj, &nl(&prj));
    let addrs: Vec<&str> = pts.iter().map(|p| p.address.as_str()).collect();
    assert_eq!(addrs, ["I0.0", "I0.1", "I0.2", "Q0.0", "Q0.1"]);
    assert!(pts[0].input && !pts[3].input);
    assert!(pts[0].connected.contains("-B1"), "{}", pts[0].connected);
    assert_eq!(address_key("%I10.7"), ("I".into(), 10, 7));
}

#[test]
fn addresses_that_are_not_byte_and_bit_compare_as_text() {
    assert_eq!(address_key("E1"), ("E".into(), 1, 0));
    for (a, b) in [("A1.2.1", "A1.3.1"), ("A1_3.1", "A1_4.1"), ("1.11.1", "1.12.1"), ("Pump on", "Pump off")] {
        assert_ne!(address_key(a), address_key(b), "{a} and {b}");
    }
    assert_eq!(address_key("%a1_3.1"), address_key("A1_3.1"));
}

#[test]
fn readdressing_uses_the_card_start_byte() {
    let mut prj = demo_project();
    prj.plc_cards.insert("-A2".into(), PlcCard { start_byte: 4, ..Default::default() });
    assert_eq!(readdress(&mut prj, "-A2"), 2);
    let pts = points(&prj, &nl(&prj));
    let a2: Vec<&str> = pts.iter().filter(|p| p.card == "-A2").map(|p| p.address.as_str()).collect();
    assert_eq!(a2, ["Q4.0", "Q4.1"]);
    assert_eq!(readdress(&mut prj, "-A2"), 0, "nothing left to change");
}

#[test]
fn symbol_table_round_trip() {
    let mut prj = demo_project();
    let table = symbol_table(&prj, &nl(&prj));
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines[0], "Name;Data Type;Logical Address;Comment");
    assert_eq!(lines.len(), 6);
    assert!(lines.contains(&"Part_present;Bool;%I0.0;-A1 Part present"), "{table}");
    // Names stay unique ("Motor running" is not used twice here, but the rule holds).
    let names: HashSet<&str> = lines[1..].iter().map(|l| l.split(';').next().unwrap()).collect();
    assert_eq!(names.len(), 5);

    let import = "\"Name\",\"Data Type\",\"Logical Address\",\"Comment\"\nX,Bool,%I0.1,Reset button\nY,Bool,%Q0.1,\n";
    assert_eq!(import_symbol_table(&mut prj, import).unwrap(), 2);
    let pts = points(&prj, &nl(&prj));
    let text = |a: &str| pts.iter().find(|p| p.address == a).unwrap().function_text.clone();
    assert_eq!(text("I0.1"), "Reset button");
    assert_eq!(text("Q0.1"), "Y", "name when there is no comment");
    assert!(import_symbol_table(&mut prj, "Name;Comment\nx;y\n").is_err());
}

#[test]
fn addresses_used_on_two_cards_are_reported() {
    let mut prj = demo_project();
    let s = prj.pages.iter_mut().flat_map(|p| p.symbols.iter_mut()).find(|s| s.label == "I0.2").unwrap();
    s.tag = "-A3".into();
    s.label = "I0.0".into();
    let msgs: Vec<String> = crate::analysis::Analysis::checked(&prj).issues.into_iter().map(|i| i.message()).collect();
    assert!(msgs.iter().any(|m| m == "PLC address I0.0 used on several cards: -A1, -A3"), "{msgs:#?}");
}

#[test]
fn iec_global_variables_per_address_space() {
    let mut prj = demo_project();
    let gvl = global_variables(&prj, &nl(&prj));
    assert!(gvl.starts_with("VAR_GLOBAL\n"), "{gvl}");
    assert!(gvl.contains("    Part_present AT %IX0.0 : BOOL; (* -A1 Part present *)\n"), "{gvl}");
    assert!(gvl.contains("    Valve AT %QX0.0 : BOOL;"), "{gvl}");
    crate::plc_hw::auto_configure(&mut prj, false);
    let gvl = global_variables(&prj, &nl(&prj));
    assert!(gvl.starts_with("(* PLC1 *)\nVAR_GLOBAL\n") && gvl.ends_with("END_VAR\n\n"), "{gvl}");
    // Exported comments carry the card; importing them gives the plain text again.
    let table = symbol_table(&prj, &nl(&prj));
    assert_eq!(import_symbol_table(&mut prj, &table).unwrap(), 0);
}

#[test]
fn vendor_tag_formats() {
    let prj = crate::sample::demo_project();
    let nl = Netlist::build(&prj, true);
    let n = tags(&prj, &nl).len();
    assert!(n > 0);
    let tia = tia_tags(&prj, &nl);
    assert_eq!(tia[0][..5], ["Name", "Path", "Data Type", "Logical Address", "Comment"]);
    assert_eq!(tia.len(), n + 1);
    assert!(tia[1..].iter().all(|r| r[2] == "Bool" && (r[3].starts_with("%I") || r[3].starts_with("%Q"))), "{tia:?}");
    let rw = rockwell_tags(&prj, &nl);
    let lines: Vec<&str> = rw.lines().collect();
    assert_eq!(lines[3], "TYPE,SCOPE,NAME,DESCRIPTION,DATATYPE,SPECIFIER,ATTRIBUTES");
    assert_eq!(lines.len(), 4 + n);
    assert!(lines[4].starts_with("ALIAS,,\"") && lines[4].contains("\"Local:"), "{}", lines[4]);
    // The workbook for TIA Portal reads back.
    let ex = crate::exporters::builtin().into_iter().find(|e| e.name == "plc_tia").unwrap();
    let an = crate::analysis::Analysis::build(&prj);
    let bytes = (ex.run)(&prj, &an, &crate::params::Args(&serde_json::Map::new())).unwrap().into_bytes();
    let sheets = crate::xlsx::read(&bytes).unwrap();
    assert_eq!(sheets[0].0, "PLC Tags");
    assert_eq!(sheets[0].1.len(), n + 1);
}

#[test]
fn large_nets_list_twenty_targets() {
    use crate::geom::p;
    use crate::model::{SymbolInst, Wire};
    let mut prj = Project::default();
    // A PLC input on a potential with many other connection points.
    let add = |prj: &mut Project, sym: &str, tag: &str, label: &str, x: f32| {
        let id = prj.new_id();
        prj.pages[0].symbols.push(SymbolInst { id, sym: sym.into(), pos: p(x, 100.0), tag: tag.into(), label: label.into(), ..Default::default() });
    };
    add(&mut prj, "plc_in", "-A1", "I0.0", 20.0);
    for i in 0..30 {
        add(&mut prj, "term", "-X1", &(i + 1).to_string(), 40.0 + 8.0 * i as f32);
    }
    let symbols = prj.symbol_table();
    let pins: Vec<crate::geom::P> = prj.pages[0].symbols.iter().map(|s| symbols.get(&s.sym).unwrap().pin_world(0, s.pos, 0, false)).collect();
    for w in pins.windows(2) {
        let id = prj.new_id();
        prj.pages[0].wires.push(Wire { id, a: w[0], b: p(w[0].x, w[0].y - 8.0), ..Default::default() });
        let id = prj.new_id();
        prj.pages[0].wires.push(Wire { id, a: p(w[0].x, w[0].y - 8.0), b: p(w[1].x, w[1].y - 8.0), ..Default::default() });
        let id = prj.new_id();
        prj.pages[0].wires.push(Wire { id, a: p(w[1].x, w[1].y - 8.0), b: w[1], ..Default::default() });
    }
    let point = points(&prj, &nl(&prj)).into_iter().next().unwrap();
    assert!(point.connected.ends_with(", … (+10)"), "{}", point.connected);
    assert_eq!(point.connected.matches("-X1:").count(), 20);
}

#[test]
fn step7_twincat_and_schneider_exports() {
    let prj = crate::sample::demo_project();
    let an = crate::analysis::Analysis::checked(&prj);
    let nl = &an.netlist;
    let n = tags(&prj, nl).iter().filter(|(_, p)| absolute(&prj, p).is_some()).count();
    assert!(n > 0);
    let sdf = step7_sdf(&prj, nl);
    assert_eq!(sdf.lines().count(), n);
    let first = sdf.lines().next().unwrap();
    let fields: Vec<&str> = first.split("\",\"").collect();
    assert_eq!(fields.len(), 4, "{first}");
    assert_eq!(fields[0].trim_start_matches('"').len(), 24);
    assert_eq!(fields[1].len(), 12);
    assert!(fields[1].starts_with('I') || fields[1].starts_with('Q'));
    assert_eq!(fields[2], "BOOL      ");
    let gvl = twincat_gvl(&prj, nl);
    assert!(gvl.contains("<TcPlcObject") && gvl.contains("VAR_GLOBAL") && gvl.contains("END_VAR"));
    assert_eq!(gvl.matches(" AT %").count(), n);
    let xsy = schneider_xsy(&prj, nl);
    assert!(xsy.starts_with("<?xml") && xsy.contains("<VariablesExchangeFile>"));
    assert!(xsy.matches("<variables ").count() >= 1);
    assert!(xsy.contains("typeName=\"EBOOL\" topologicalAddress=\"%"));
}

#[test]
fn mitsubishi_devices_are_octal() {
    let prj = crate::sample::demo_project();
    let an = crate::analysis::Analysis::checked(&prj);
    let csv = mitsubishi_comments(&prj, &an.netlist);
    let mut lines = csv.lines();
    assert_eq!(lines.next(), Some("\"Device Name\",\"Comment\""));
    let devices: Vec<&str> = lines.map(|l| l.split(',').next().unwrap().trim_matches('"')).collect();
    assert!(!devices.is_empty());
    assert!(devices.iter().all(|d| (d.starts_with('X') || d.starts_with('Y')) && d[1..].chars().all(|c| ('0'..='7').contains(&c))), "{devices:?}");
}

#[test]
fn readdressing_keeps_the_addresses_of_analog_points() {
    let mut prj = Project::default();
    for (i, (sym, label)) in [("plc_in", "I9.9"), ("plc_ai", "IW64"), ("plc_out", "Q9.9"), ("plc_ao", "QW80"), ("plc_out", "Q9.8")].into_iter().enumerate() {
        let id = prj.new_id();
        prj.pages[0].symbols.push(SymbolInst { id, sym: sym.into(), tag: "-A1".into(), label: label.into(), pos: crate::geom::p(40.0 + 40.0 * i as f32, 100.0), ..Default::default() });
    }
    assert_eq!(readdress(&mut prj, "-A1"), 3);
    let labels: Vec<&str> = prj.pages[0].symbols.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["I0.0", "IW64", "Q0.0", "QW80", "Q0.1"]);
}
