use super::*;
use crate::{Args, Origin, Plugins};
use openecraft_core::analysis::Analysis;
use openecraft_core::sample::demo_project;
use serde_json::Map;

fn run(p: &Plugins, prj: &mut Project, name: &str, args: Value) -> Result<Value, String> {
    let args: Map<String, Value> = args.as_object().cloned().unwrap_or_default();
    p.run(p.command(name).ok_or("no command")?, prj, &args)
}

#[test]
fn machines_strategy_and_programs() {
    let p = Plugins::new(Vec::<String>::new());
    assert!(p.list().iter().any(|l| l.info.id == "panel_manufacturing" && l.origin == Origin::BuiltIn));
    let mut prj = demo_project();
    assert!(p.check(&prj, &Analysis::build(&prj)).iter().all(|i| i.severity != Severity::Warning));
    let m = run(&p, &mut prj, "cnc_machines", json!({})).unwrap();
    assert_eq!(m["faces"]["door"], "drill_mill_iso");
    let plan = run(&p, &mut prj, "machining_plan", json!({ "enclosure": "+CAB1", "face": "plate" })).unwrap();
    assert!(plan["by_kind"]["drill"].as_u64().unwrap() > 0);
    assert_eq!(plan["steps"].as_array().unwrap().len() as u64, plan["operations"].as_u64().unwrap());
    // The door on the punching machine.
    run(&p, &mut prj, "machining_strategy", json!({ "face": "door", "machine": "punch_turret", "tap": false })).unwrap();
    let prog = run(&p, &mut prj, "cnc_program", json!({ "enclosure": "+CAB1", "face": "door" })).unwrap();
    assert_eq!(prog["machine"], "punch_turret");
    assert_eq!(prog["file"], "CAB1_door.pun");
    assert!(prog["content"].as_str().unwrap().contains("G92"));
    assert!(!prj.layout.manufacturing.strategy.tap);
    assert!(run(&p, &mut prj, "machining_strategy", json!({ "face": "door", "machine": "nope" })).is_err());
    assert!(run(&p, &mut prj, "cnc_program", json!({ "enclosure": "+CAB1", "face": "front" })).is_err());
    // An own machine.
    let own = run(&p, &mut prj, "set_cnc_machine", json!({ "id": "my_mill", "dialect": "heidenhain", "tools": "D4.2 T1; D3.3 T2; M5 T3; M4 T4; F8 T5" })).unwrap();
    assert_eq!(own["tools"], "D4.2 T1; D3.3 T2; M5 T3; M4 T4; F8 T5");
    run(&p, &mut prj, "machining_strategy", json!({ "sheet_machine": "my_mill" })).unwrap();
    let prog = run(&p, &mut prj, "cnc_program", json!({ "enclosure": "+CAB1", "face": "plate" })).unwrap();
    assert!(prog["content"].as_str().unwrap().starts_with("0 BEGIN PGM"));
    assert!(run(&p, &mut prj, "set_cnc_machine", json!({ "id": "My Mill" })).is_err());
    assert!(run(&p, &mut prj, "set_cnc_machine", json!({ "id": "my_mill", "remove": true })).is_err());
    run(&p, &mut prj, "machining_strategy", json!({ "sheet_machine": "drill_mill_iso" })).unwrap();
    run(&p, &mut prj, "set_cnc_machine", json!({ "id": "my_mill", "remove": true })).unwrap();
    assert!(prj.layout.manufacturing.machines.is_empty());
    // The plate, the door and the bottom with the hole of the motor cable's gland.
    let table = (p.report("cnc_operations").unwrap().run)(&prj, &Analysis::build(&prj)).unwrap();
    assert_eq!(table.rows.len(), 3);
}

#[test]
fn copper_bars_and_configurator() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let b = run(&p, &mut prj, "add_copper_bar", json!({ "enclosure": "+CAB1", "name": "L1", "segments": "100, 200, 100", "bends": "90; 90", "end_holes": 9, "count": 3 })).unwrap();
    assert_eq!(b["bends"].as_array().unwrap().len(), 2);
    assert_eq!(b["holes"].as_array().unwrap().len(), 2);
    assert!(run(&p, &mut prj, "add_copper_bar", json!({ "enclosure": "+CAB1", "name": "L2", "segments": "100, 200", "bends": "" })).is_err());
    let list = run(&p, &mut prj, "copper_bars", json!({})).unwrap();
    assert_eq!(list["stock_bars"].as_array().unwrap().len(), 1);
    let prog = run(&p, &mut prj, "copper_bar_program", json!({ "enclosure": "+CAB1" })).unwrap();
    assert_eq!(prog["content"].as_str().unwrap().lines().filter(|l| l.contains(";bend;")).count(), 2);
    let rules = run(&p, &mut prj, "copper_bar_rules", json!({ "stock_length": 300 })).unwrap();
    assert_eq!(rules["stock_length"], 300.0);
    assert!(p.check(&prj, &Analysis::build(&prj)).iter().any(|i| i.message().contains("stock length")));
    let id = prj.layout.manufacturing.bars[0].id;
    run(&p, &mut prj, "remove_copper_bar", json!({ "id": id })).unwrap();
    assert!(prj.layout.manufacturing.bars.is_empty());
    // Configurator: export, dry run, import, smallest size.
    let cfg = run(&p, &mut prj, "export_enclosure_configuration", json!({ "enclosure": "+CAB1" })).unwrap();
    let text = cfg.to_string().replace("+CAB1", "+CAB7");
    let dry = run(&p, &mut prj, "import_enclosure_configuration", json!({ "text": text, "dry_run": true })).unwrap();
    assert_eq!(dry["created"], true);
    assert_eq!(prj.layout.enclosures.len(), 1);
    run(&p, &mut prj, "import_enclosure_configuration", json!({ "text": text })).unwrap();
    assert_eq!(prj.layout.enclosures.len(), 2);
    assert!(run(&p, &mut prj, "configure_enclosure", json!({ "enclosure": "+CAB1" })).is_err());
    let c = run(&p, &mut prj, "configure_enclosure", json!({ "enclosure": "+CAB1", "system": "compact", "dry_run": true })).unwrap();
    assert_eq!(prj.layout.enclosures[0].build.system, "");
    run(&p, &mut prj, "configure_enclosure", json!({ "enclosure": "+CAB1", "system": "compact" })).unwrap();
    assert_eq!(prj.layout.enclosures[0].width as f64, c["size"][0].as_f64().unwrap());
    assert_eq!(prj.layout.enclosures[0].build.system, "compact");
}

#[test]
fn handoff_feedback_and_status() {
    let p = Plugins::new(Vec::<String>::new());
    let mut prj = demo_project();
    let dry = run(&p, &mut prj, "manufacturing_handoff", json!({ "enclosure": "+CAB1", "dry_run": true })).unwrap();
    assert!(dry["job"].is_null() && prj.layout.manufacturing.jobs.is_empty());
    let pkg = run(&p, &mut prj, "manufacturing_handoff", json!({ "enclosure": "+CAB1", "date": "2026-10-24" })).unwrap();
    assert_eq!(pkg["job"], "CAB1-1");
    // Plate, door and bottom (the hole of the gland).
    assert_eq!(pkg["programs"].as_array().unwrap().len(), 3);
    let fb = run(&p, &mut prj, "production_feedback", json!({ "text": "job;program;status;time\nCAB1-1;CAB1_plate.nc;done;600\nCAB1-1;CAB1_door.nc;failed;\n" })).unwrap();
    assert_eq!(fb["imported"], 2);
    assert_eq!(fb["jobs"][0]["state"], "Problem");
    let st = run(&p, &mut prj, "production_status", json!({})).unwrap();
    assert!(st["changed"].as_array().unwrap().is_empty());
    assert!(p.check(&prj, &Analysis::build(&prj)).iter().any(|i| i.severity == Severity::Warning && i.message().contains("failed")));
    let cal = run(&p, &mut prj, "calibrate_machines", json!({ "apply": true })).unwrap();
    assert_eq!(cal[0]["machine"], "drill_mill_iso");
    let table = (p.report("production_status").unwrap().run)(&prj, &Analysis::build(&prj)).unwrap();
    assert_eq!(table.rows[0][4], "1");
    // Changes after the hand-off are found by the check.
    prj.layout.enclosures[0].items.retain(|i| i.face != Face::Door);
    assert!(p.check(&prj, &Analysis::build(&prj)).iter().any(|i| i.message().contains("CAB1_door.nc")));
    let an = Analysis::build(&prj);
    let pkg = (p.exporter("manufacturing_package").unwrap().run)(&prj, &an, &Args(&Map::new())).unwrap().text().unwrap();
    assert!(pkg.contains("CAB1_plate.nc"));
    let csv = (p.exporter("copper_bar_program").unwrap().run)(&prj, &an, &Args(&Map::new())).unwrap().text().unwrap();
    assert!(csv.starts_with("Bar;"));
    let cut = (p.report("copper_bar_cutting").unwrap().run)(&prj, &Analysis::build(&prj)).unwrap();
    assert!(cut.rows.is_empty());
}
