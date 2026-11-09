use super::*;
use crate::builtin::testing::*;

fn demo() -> Session {
    demo_session()
}

#[test]
fn module_channels_are_listed_and_edited_independently() {
    use openecraft_core::model::SymbolInst;
    use openecraft_core::symbols::{Func,PlcChannelDef};
    use openecraft_core::symlib::UserSymbol;
    let mut prj=Project::default();
    prj.symbols.push(UserSymbol {key:"user:module".into(),func:Func::Plc,
        plc_channels:(0..8).map(|i|PlcChannelDef {key:format!("c{i}"),index:i,input:Some(true),data_type:"Bool".into(),..Default::default()}).collect(),..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {id:123,sym:"user:module".into(),tag:"-A1".into(),..Default::default()});
    let mut session=with_project(prj);
    let points=run(&mut session,"get_plc_points",json!({})).unwrap();
    assert_eq!(points.as_array().unwrap().len(),8);
    assert_eq!(points[3]["symbolic"],"");
    run(&mut session,"set_plc_channel",json!({"symbol":123,"channel":3,"address":"I12.3","symbolic":"Ready","comment":"Drive ready"})).unwrap();
    let points=run(&mut session,"get_plc_points",json!({})).unwrap();
    let edited=points.as_array().unwrap().iter().find(|p|p["channel"]==3).unwrap();
    assert_eq!(edited["address"],"I12.3");
    assert_eq!(edited["symbolic"],"Ready");
    let untouched=points.as_array().unwrap().iter().find(|p|p["channel"]==4).unwrap();
    assert_eq!(untouched["address"],"");
    assert!(run(&mut session,"set_plc_channel",json!({"symbol":123,"channel":8,"address":"I99.0"})).is_err());
    let hardware=run(&mut session,"set_plc_card",json!({"card":"-A1","rack":2})).unwrap();
    assert_eq!(hardware["addresses_changed"],0);
    let points=run(&mut session,"get_plc_points",json!({})).unwrap();
    assert_eq!(points.as_array().unwrap().iter().find(|p|p["channel"]==3).unwrap()["address"],"I12.3");
    assert_eq!(run(&mut session,"set_plc_card",json!({"card":"-A1","start_byte":42})).unwrap()["addresses_changed"],8);
}

#[test]
fn hardware_is_configured_and_changed() {
    let mut s = demo();
    let hw = run(&mut s, "get_plc_hardware", json!({})).unwrap();
    assert_eq!(hw["unassigned_cards"], json!(["-A1", "-A2"]));
    let hw = run(&mut s, "configure_plc_hardware", json!({ "cables": true })).unwrap();
    assert_eq!(hw["created"]["stations"], 1);
    assert_eq!(hw["created"]["cables"], 0, "a single station has no connections");
    assert_eq!(hw["stations"][0]["modules"][1]["card"], "-A2");
    assert_eq!(hw["stations"][0]["modules"][1]["slot"], 3);
    assert_eq!(hw["problems"], json!([]));

    let hw = run(&mut s, "set_station", json!({ "name": "PLC1", "scheme": "iec", "new_name": "Main" })).unwrap();
    assert_eq!(hw["addresses_changed"], 5);
    assert_eq!(hw["stations"][0]["name"], "Main");
    assert!(run(&mut s, "set_station", json!({ "name": "Main", "scheme": "x" })).unwrap_err().contains("Unknown address scheme"));

    run(&mut s, "set_station", json!({ "name": "ET1", "kind": "io_device", "device": "GEN.IM-PN", "network": "BUS_1", "controller": "Main", "address": "192.168.0.5" })).unwrap();
    let hw = run(&mut s, "set_plc_card", json!({ "card": "-A2", "station": "ET1", "slot": 1, "start_byte": 4 })).unwrap();
    assert_eq!(hw["addresses_changed"], 2);
    assert!(run(&mut s, "set_plc_card", json!({ "card": "-A2", "station": "NONE" })).is_err());

    let hw = run(&mut s, "connect_ports", json!({ "a": "Main:X1 P2", "b": "ET1:X1 P1", "cable": "-W5" })).unwrap();
    assert_eq!(hw["connections"], json!([{ "network": "BUS_1", "a": "Main:X1 P2", "b": "ET1:X1 P1", "cable": "-W5" }]));
    assert!(hw["problems"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("cable -W5 does not exist")));
    let hw = run(&mut s, "create_bus_cables", json!({})).unwrap();
    assert_eq!(hw["created"], 0, "the connection names a cable");
    run(&mut s, "connect_ports", json!({ "a": "Main:X1 P2", "b": "ET1:X1 P1", "create_cable": true })).unwrap();
    let hw = run(&mut s, "get_plc_hardware", json!({})).unwrap();
    let cable = hw["connections"][0]["cable"].as_str().unwrap().to_string();
    let bus = hw["bus_cables"].as_array().unwrap().iter().find(|c| c["cable"] == cable.as_str()).unwrap();
    assert_eq!((bus["bus"].as_str(), bus["connections"].as_u64()), (Some("profinet"), Some(1)));
    run(&mut s, "disconnect_port", json!({ "port": "ET1:X1 P1" })).unwrap();
    assert!(run(&mut s, "disconnect_port", json!({ "port": "ET1:X1 P1" })).is_err());
    let hw = run(&mut s, "set_bus_system", json!({ "name": "BUS_1", "remove": true })).unwrap();
    assert_eq!(hw["bus_systems"], json!([]));
    let hw = run(&mut s, "remove_station", json!({ "name": "ET1" })).unwrap();
    assert_eq!(hw["unassigned_cards"], json!(["-A2"]));
}
