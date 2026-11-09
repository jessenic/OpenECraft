use super::*;
use crate::builtin::testing::*;

#[test]
fn plugs_are_listed_with_their_pins() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "pot_24", "x": 100, "y": 76 })).unwrap();
    let pin = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "plug_pair", "x": 100, "y": 100, "tag": "-X10" })).unwrap();
    assert_eq!(pin["label"], "1");
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 120 })).unwrap();
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[100, 80], [100, 96]] })).unwrap();
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[100, 104], [100, 116]] })).unwrap();

    let list = run(&mut s, "list_plugs", json!({})).unwrap();
    assert_eq!(list, json!([{ "plug": "-X10", "pins_used": 1, "pin_count": null, "parts": [] }]));
    let plug = run(&mut s, "get_plug", json!({ "plug": "-X10" })).unwrap();
    let p = &plug["pins"][0];
    assert_eq!((p["pin"].as_str(), p["socket_side"].as_str(), p["pin_side"].as_str()), (Some("1"), Some("=+24V"), Some("-K1:A1")));
    assert_eq!(p["symbols"][0]["kind"], "pair");
    assert!(run(&mut s, "get_plug", json!({ "plug": "-X99" })).unwrap_err().contains("No plug"));
}

#[test]
fn the_pinout_and_flat_cables_are_commands() {
    let mut s = new_session();
    for (i, x) in [100, 120].into_iter().enumerate() {
        let label = (i + 1).to_string();
        run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "plug_pair", "x": x, "y": 100, "tag": "-X20", "label": label })).unwrap();
        run(&mut s, "add_wire", json!({ "page": "1", "points": [[x, 80], [x, 96]] })).unwrap();
    }
    s.project_mut().unwrap().devices.entry("-X20".into()).or_default().parts.push(openecraft_core::parts::PartRef { number: "GEN.CON-IDC-10F".into(), count: 1, total:false });
    s.project_mut().unwrap().cables.push(openecraft_core::model::Cable { id: 77, tag: "-W20".into(), cores: "1 2 3 4 5 6 7 8 9 10".into(), part: "GEN.CBL-FLAT-10".into(), ..Default::default() });

    let po = run(&mut s, "get_pinout", json!({ "plug": "-X20" })).unwrap();
    assert_eq!(po["layout"], "zigzag 10");
    assert_eq!(po["pins"].as_array().map(Vec::len), Some(10));
    assert_eq!((po["pins"][0]["spare"].as_bool(), po["pins"][2]["spare"].as_bool()), (Some(false), Some(true)));
    assert!(po["pins"][0]["at"][1].as_f64().unwrap() < po["pins"][1]["at"][1].as_f64().unwrap());

    let done = run(&mut s, "connect_flat_cable", json!({ "cable": "-W20", "plug": "-X20" })).unwrap();
    assert_eq!(done["wires"], 2);
    let po = run(&mut s, "get_pinout", json!({ "plug": "-X20" })).unwrap();
    assert_eq!((po["pins"][0]["socket_side"]["color"].as_str(), po["pins"][1]["socket_side"]["core"].as_str()), (Some("RD"), Some("-W20:2")));
    assert!(run(&mut s, "connect_flat_cable", json!({ "cable": "-W20", "plug": "-X20", "side": "top" })).unwrap_err().contains("Unknown side"));
    assert!(run(&mut s, "get_pinout", json!({ "plug": "-X99" })).unwrap_err().contains("No plug"));
}
