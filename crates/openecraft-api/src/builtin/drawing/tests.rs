use super::*;

#[test]
fn per_wire_junction_policy_is_created_edited_and_reset_through_the_registry() {
    let mut s=crate::builtin::testing::new_session();
    let wire=run(&mut s,"add_wire",json!({"page":"1","points":[[40,40],[80,40]],"junction_policy":"explicit"})).unwrap();
    let id=wire["ids"][0].as_u64().unwrap();
    assert_eq!(s.project().unwrap().pages[0].wires[0].explicit_junctions,Some(true));
    assert!(!s.project().unwrap().pages[0].values.contains_key("page.explicit_wire_junctions"));
    let changed=run(&mut s,"update_wire",json!({"page":"1","id":id,"junction_policy":"implicit"})).unwrap();
    assert_eq!(changed["junction_policy"],"implicit");
    assert_eq!(Project::from_json(&s.project().unwrap().to_json()).unwrap().pages[0].wires[0].explicit_junctions,Some(false));
    let before=s.project().unwrap().to_json();
    assert!(run(&mut s,"update_wire",json!({"page":"1","id":id,"number":"Bad edit","junction_policy":"unknown"})).is_err());
    assert_eq!(s.project().unwrap().to_json(),before);
    run(&mut s,"update_wire",json!({"page":"1","id":id,"junction_policy":"inherit"})).unwrap();
    let reopened=Project::from_json(&s.project().unwrap().to_json()).unwrap();
    assert_eq!(reopened.pages[0].wires[0].explicit_junctions,None);
}

#[test]
fn queried_local_routes_return_native_wire_properties_and_reject_invalid_queries() {
    use openecraft_core::symlib::{UserPin,UserSymbol};
    let mut s=new_session();
    let prj=s.project_mut().unwrap();
    prj.symbols.push(UserSymbol {key:"user:route_point".into(),pins:vec![UserPin::default()],..Default::default()});
    for (id,x) in [(100,20.),(101,40.)] {
        prj.pages[0].symbols.push(SymbolInst {id,sym:"user:route_point".into(),pos:p(x,20.),..Default::default()});
    }
    prj.pages[0].wires.push(Wire {id:102,a:p(20.,20.),b:p(40.,20.),wire_type:"MKEM".into(),color:"GNYE".into(),cross_section:"10".into(),..Default::default()});
    let page=prj.pages[0].id;
    let result=run(&mut s,"trace_wire_routes",json!({"routes":{"pairs":[{"from":{"symbol":100,"pin":0},"to":{"symbol":101,"pin":0}},{"from":{"symbol":100,"pin":99},"to":{"symbol":101,"pin":0}}]}})).unwrap();
    assert_eq!(result[0]["route_traced"],true);
    assert_eq!(result[0]["wires"],json!([{"page":page,"id":102,"wire_type":"MKEM","color":"GNYE","cross_section":"10","number":""}]));
    assert_eq!(result[1]["route_traced"],false);
    assert!(run(&mut s,"trace_wire_routes",json!({"routes":{"pairs":[{}]}})).is_err());
    assert!(run(&mut s,"trace_wire_routes",json!({"routes":{}})).is_err());
}

#[test]
fn attribute_block_defaults_are_exposed_placed_and_editable_after_reopen() {
    use openecraft_core::symbols::{Anchor, BlockAlign, BlockRow, Prim, PropertyBlock};
    use openecraft_core::symlib::UserSymbol;
    let mut s = new_session();
    s.project_mut().unwrap().symbols.push(UserSymbol {
        key: "user:block_defaults".into(), show_tag: false,
        default_values: [("function.supplementary[2]".into(), "Library detail".into())].into_iter().collect(),
        prims: vec![Prim::PropertyBlock(PropertyBlock {
            rows: vec![BlockRow::Property("function.supplementary[2]".into())], pos: p(0., 0.), step: p(0., 3.), size: 2.5,
            anchor: Anchor::Left, angle: 0., align: BlockAlign::Start, color: None,
        })], ..Default::default()
    });
    let defs = run(&mut s, "list_symbols", json!({"query":"user:block_defaults"})).unwrap();
    assert_eq!(defs[0]["default_values"]["function.supplementary[2]"], "Library detail");
    assert_eq!(defs[0]["property_blocks"].as_array().unwrap().len(), 1);
    let placed = run(&mut s, "place_symbol", json!({"page":"1","symbol":"user:block_defaults","x":40,"y":40})).unwrap();
    assert!(openecraft_core::svg::page_svg(s.project().unwrap(), 0).contains("Library detail"));
    run(&mut s, "set_properties", json!({"object":format!("function:{}", placed["id"]),"values":{"function.supplementary[2]":"Edited detail"}})).unwrap();
    let prj = Project::from_json(&s.project().unwrap().to_json()).unwrap();
    let svg = openecraft_core::svg::page_svg(&prj, 0);
    assert!(svg.contains("Edited detail") && !svg.contains("Library detail"));
    let copy = openecraft_core::symlib::from_def(&prj.symbol("user:block_defaults").unwrap(), &prj.symbols);
    assert_eq!(copy.default_values, prj.symbols[0].default_values);
    assert_eq!(copy.prims, prj.symbols[0].prims);
}

#[test]
fn user_symbol_function_text_defaults_are_live_after_shared_placement_and_reopen() {
    use openecraft_core::symbols::{Anchor, FieldKind, Prim};
    use openecraft_core::symlib::UserSymbol;
    let mut s = new_session();
    s.project_mut().unwrap().symbols.push(UserSymbol {
        key: "user:function_default".into(), default_function_text: "Library default".into(), show_tag: false,
        prims: vec![Prim::Field { kind: FieldKind::FunctionText, pos: p(0., 0.), size: 2., anchor: Anchor::Left, angle: Some(0.) }],
        ..Default::default()
    });
    let defs = run(&mut s, "list_symbols", json!({"query":"user:function_default"})).unwrap();
    assert_eq!(defs[0]["default_function_text"], "Library default");
    let placed = run(&mut s, "place_symbol", json!({"page":"1","symbol":"user:function_default","x":40,"y":40})).unwrap();
    let prj = s.project().unwrap();
    assert_eq!(prj.pages[0].symbols[0].function_text, "Library default");
    assert!(openecraft_core::svg::page_svg(prj, 0).contains("Library default"));
    run(&mut s, "update_symbol", json!({"page":"1","id":placed["id"],"function_text":"Edited in project"})).unwrap();
    let reopened = Project::from_json(&s.project().unwrap().to_json()).unwrap();
    let svg = openecraft_core::svg::page_svg(&reopened, 0);
    assert!(svg.contains("Edited in project") && !svg.contains("Library default"));
    assert_eq!(reopened.symbols[0].default_function_text, "Library default");
    let copied = openecraft_core::symlib::from_def(&reopened.symbol("user:function_default").unwrap(), &reopened.symbols);
    assert_eq!(copied.default_function_text, "Library default");
}
use crate::builtin::testing::*;
use openecraft_core::analysis::Analysis;

#[test]
fn wire_annotation_visibility_is_editable_through_the_shared_command() {
    let mut s = new_session();
    let added = run(&mut s,"add_wire",json!({"page":"1","points":[[20,20],[80,20]]})).unwrap();
    let id = &added["ids"][0];
    let hidden = run(&mut s,"update_wire",json!({"page":"1","id":id,"number":"101","hide_labels":true})).unwrap();
    assert_eq!(hidden["hide_labels"],true);
    assert_eq!(run(&mut s,"update_wire",json!({"page":"1","id":id,"color":"BU"})).unwrap()["hide_labels"],true);
    let shown = run(&mut s,"update_wire",json!({"page":"1","id":id,"hide_labels":false})).unwrap();
    assert_eq!(shown["hide_labels"],false);
    assert_eq!(shown["number"],"101");
}

#[test]
fn native_wiring_connections_expose_stable_pin_identities() {
    let mut s = new_session();
    let a = run(&mut s,"place_symbol",json!({"page":"1","symbol":"coil","x":40,"y":40,"tag":"-K1"})).unwrap();
    let b = run(&mut s,"place_symbol",json!({"page":"1","symbol":"coil","x":40,"y":80,"tag":"-K2"})).unwrap();
    run(&mut s,"connect",json!({"page":"1","from":"-K1:A2","to":"-K2:A1"})).unwrap();
    let connections = run(&mut s,"list_connections",json!({})).unwrap();
    assert!(connections.as_array().unwrap().iter().any(|c| c["from"]["symbol"] == a["id"] && c["from"]["pin"] == 1 && c["to"]["symbol"] == b["id"] && c["to"]["pin"] == 0));
    s.project_mut().unwrap().pages[0].values.insert("page.use_drawn_wire_targets".into(),"true".into());
    let connections = run(&mut s,"list_connections",json!({})).unwrap();
    let connection=&connections[0];
    assert_eq!(connection["route_traced"],true);
    let project=s.project().unwrap();
    let wires:Vec<_>=project.pages[0].wires.iter().map(|w|json!({"page":project.pages[0].id,"id":w.id})).collect();
    assert_eq!(connection["wires"],json!(wires));
}

#[test]
fn user_junction_paths_are_editable_and_invalid_changes_are_atomic() {
    use openecraft_core::symlib::{UserSymbol,UserPin};
    let mut s=new_session();
    s.project_mut().unwrap().symbols.push(UserSymbol {key:"user:paths".into(),func:Func::Junction,pins:vec![UserPin::default();3],..Default::default()});
    run(&mut s,"set_symbol_junction_paths",json!({"symbol":"user:paths","paths":{"links":[[0,1],[0,2]]}})).unwrap();
    let definitions=run(&mut s,"list_symbols",json!({"query":"user:paths"})).unwrap();
    assert_eq!(definitions[0]["directed_junction"],true);
    assert_eq!(definitions[0]["bridges"],json!([[0,1],[0,2]]));
    let before=s.project().unwrap().symbols[0].clone();
    for links in [json!([[0,3]]),json!([[1,1]]),json!([[-1,0]])] {
        assert!(run(&mut s,"set_symbol_junction_paths",json!({"symbol":"user:paths","paths":{"links":links}})).is_err());
        assert_eq!(s.project().unwrap().symbols[0],before);
    }
}

#[test]
fn user_pin_directions_are_editable_and_invalid_changes_are_atomic() {
    use openecraft_core::symlib::{UserSymbol,UserPin};
    let mut s=new_session();
    s.project_mut().unwrap().symbols.push(UserSymbol {key:"user:port".into(),pins:vec![UserPin::default()],..Default::default()});
    run(&mut s,"set_pin_connection_directions",json!({"symbol":"user:port","pin":0,"directions":"up,right"})).unwrap();
    let definitions=run(&mut s,"list_symbols",json!({"query":"user:port"})).unwrap();
    assert_eq!(definitions[0]["pin_directions"]["0"],3);
    let before=s.project().unwrap().symbols[0].clone();
    for (pin,directions) in [(1,"down"),(0,"unknown"),(-1,"up")] {
        assert!(run(&mut s,"set_pin_connection_directions",json!({"symbol":"user:port","pin":pin,"directions":directions})).is_err());
        assert_eq!(s.project().unwrap().symbols[0],before);
    }
    run(&mut s,"set_pin_connection_directions",json!({"symbol":"user:port","pin":0,"directions":"all"})).unwrap();
    assert!(s.project().unwrap().symbols[0].pin_directions.is_empty());
}

#[test]
fn reciprocal_ground_transfers_form_one_net_after_project_reopen() {
    let mut s=new_session();
    let contact=|number:&str,pos:&str,rule:&str|format!(r#"<O17 A1="17/{number}" A1261="0" A1262="13" A1263="0" A185="{number}"><P150 P20013="Q"/><P11 P20406="X1"/><S40x1201 A762="{pos}"/><S61x183 A1061="0" A1063="0"><S16x1062 A161="1" A163="{rule}"/></S61x183></O17>"#);
    let xml=format!(r#"<EplanPxfRoot Name="drive" Type="WindowMacro"><MacroVariant MacroFuncType="1" VariantId="0" ReferencePoint="0/0/0"><ESymLib ID="0" Name="SPECIAL"/>{}{}{}</MacroVariant></EplanPxfRoot>"#,contact("1","0/0","&quot;X1:2&quot;"),contact("2","8/0","&quot;X1:1&quot;"),contact("3","16/0",""));
    let imported=run(&mut s,"import_ema",json!({"xml":xml})).unwrap();
    run(&mut s,"place_symbol",json!({"page":"1","symbol":imported["variants"][0]["symbol"]["key"],"x":80,"y":80,"rotation":90,"tag":"-Q1"})).unwrap();
    run(&mut s,"place_symbol",json!({"page":"1","symbol":"coil","x":144,"y":80,"tag":"-K1"})).unwrap();
    run(&mut s,"connect",json!({"page":"1","from":"-Q1:X1.1","to":"-K1:A1"})).unwrap();
    let prj=Project::from_json(&s.project().unwrap().to_json()).unwrap();
    let net=openecraft_core::netlist::Netlist::build(&prj,false);
    let drive=prj.pages[0].symbols[0].id;let coil=prj.pages[0].symbols[1].id;
    assert_eq!(net.pin_net[&(drive,1)],net.pin_net[&(coil,0)]);
    assert_ne!(net.pin_net[&(drive,2)],net.pin_net[&(coil,0)]);
}

#[test]
fn imported_contact_can_be_placed_and_wired() {
    let mut s = new_session();
    let xml = r#"<EplanPxfRoot Type="WindowMacro" Name="input"><MacroVariant MacroFuncType="1" VariantId="0" ReferencePoint="0/0/0"><ESymLib ID="0" Name="SPECIAL"/><O17 A185="1" A1261="0" A1262="13" A1263="0"><S61x183 A1061="1"/><S40x1201 A762="8/-8"/></O17></MacroVariant></EplanPxfRoot>"#;
    let report = run(&mut s, "import_ema", json!({"xml":xml})).unwrap();
    assert_eq!(report["added"], 1);
    let key = &report["variants"][0]["symbol"]["key"];
    run(&mut s, "place_symbol", json!({"page":"1","x":80,"y":80,"tag":"-A1","symbol":key,"rotation":90})).unwrap();
    run(&mut s, "place_symbol", json!({"page":"1","x":140,"y":80,"tag":"-K1","symbol":"coil"})).unwrap();
    assert!(run(&mut s, "connect", json!({"page":"1","from":"-A1:1","to":"-K1:A1"})).is_ok());
    let project = Project::from_json(&s.project().unwrap().to_json()).unwrap();
    let net = openecraft_core::netlist::Netlist::build(&project, false);
    let symbols = &project.pages[0].symbols;
    assert_eq!(net.pin_net[&(symbols[0].id, 0)], net.pin_net[&(symbols[1].id, 0)]);
    assert_eq!(symbols[0].pins, ["1"]);
    assert!(openecraft_core::svg::page_svg(&project, 0).contains("<circle"));
}

/// Messages of the project checks (like the `check` tool of the command line).
fn issues(s: &mut Session) -> Vec<String> {
    let prj = s.project().unwrap();
    s.plugins.check(prj, &Analysis::build(prj)).into_iter().map(|i| i.message()).collect()
}

#[test]
fn symbols_take_the_device_of_a_given_tag() {
    let mut s = new_session();
    let coil = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 101, "y": 99 })).unwrap();
    assert_eq!(coil["tag"], "-K1");
    assert_eq!((coil["x"].as_f64(), coil["y"].as_f64()), (Some(100.0), Some(100.0)), "snapped to the grid");
    let first = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "no", "x": 60, "y": 60, "tag": "-K1" })).unwrap();
    let second = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "no", "x": 80, "y": 60, "tag": "-K1" })).unwrap();
    let pins = |v: &Value| v["pins"].as_array().unwrap().iter().map(|p| p["pin"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(pins(&first), ["13", "14"]);
    assert_eq!(pins(&second), ["23", "24"]);
    let next = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 140, "y": 100, "rotation": 90 })).unwrap();
    assert_eq!(next["tag"], "-K2");
    assert_eq!(next["rotation"], 90);

    assert!(run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 0, "y": 0, "rotation": 45 })).is_err());
    assert!(run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "nonsense", "x": 0, "y": 0 })).is_err());
    let listed = run(&mut s, "list_symbols", json!({ "category": "coils" })).unwrap();
    assert!(listed.as_array().unwrap().iter().any(|d| d["key"] == "coil"));
}

#[test]
fn connect_wires_connection_points() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "no", "x": 60, "y": 60, "tag": "-K1" })).unwrap();
    assert!(issues(&mut s).iter().any(|m| m.contains("-K1:A1")));
    let wire = run(&mut s, "connect", json!({ "page": "1", "from": "-K1:A1", "to": "-K1:13" })).unwrap();
    assert_eq!(wire["ids"].as_array().unwrap().len(), 2, "one corner");
    assert!(!issues(&mut s).iter().any(|m| m.contains("-K1:A1") || m.contains("-K1:13")));

    assert!(run(&mut s, "connect", json!({ "page": "1", "from": "-K1:X9", "to": "-K1:14" })).unwrap_err().contains("No connection point"));
    assert!(run(&mut s, "connect", json!({ "page": "1", "from": "K1", "to": "-K1:14" })).is_err());
}

#[test]
fn ambiguous_connection_points_need_an_id() {
    let mut s = new_session();
    let a = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "term", "x": 40, "y": 40, "tag": "-X1" })).unwrap();
    let b = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "term", "x": 80, "y": 40, "tag": "-X1" })).unwrap();
    assert_eq!((a["label"].as_str(), b["label"].as_str()), (Some("1"), Some("2")), "terminal numbers");
    assert_eq!(a["pins"][1]["pin"], "2", "points without designation are numbered");
    let pin = "1";
    let err = run(&mut s, "connect", json!({ "page": "1", "from": format!("-X1:{pin}"), "to": format!("-X1:{pin}") })).unwrap_err();
    assert!(err.contains("ambiguous"), "{err}");
    let by_id = format!("#{}:{pin}", a["id"]);
    let prj = s.project().unwrap();
    assert!(pin_position(&prj.symbol_table(), &prj.pages[0], &by_id).is_ok());
}

#[test]
fn items_move_with_their_wires_and_can_be_deleted() {
    let mut s = new_session();
    let coil = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    let a1 = (coil["pins"][0]["x"].as_f64().unwrap(), coil["pins"][0]["y"].as_f64().unwrap());
    let wire = run(&mut s, "add_wire", json!({ "page": "1", "points": [[a1.0, a1.1], [a1.0, 40]] })).unwrap();
    let id = coil["id"].as_u64().unwrap();
    run(&mut s, "move_items", json!({ "page": "1", "ids": [id], "dx": 0, "dy": 8 })).unwrap();
    let page = run(&mut s, "get_page", json!({ "page": "1" })).unwrap();
    assert_eq!(page["symbols"][0]["y"], 108.0);
    assert_eq!(page["wires"][0]["from"], json!([a1.0, a1.1 + 8.0]), "wire end follows");

    run(&mut s, "update_wire", json!({ "page": "1", "id": wire["ids"][0], "number": "W1", "color": "BU" })).unwrap();
    assert_eq!(run(&mut s, "get_page", json!({ "page": "1" })).unwrap()["wires"][0]["number"], "W1");
    run(&mut s, "add_text", json!({ "page": "1", "x": 20, "y": 20, "text": "Hello" })).unwrap();

    assert!(run(&mut s, "delete_items", json!({ "page": "1", "ids": [999] })).is_err());
    assert_eq!(run(&mut s, "delete_items", json!({ "page": "1", "ids": [id] })).unwrap()["deleted"], 1);
    assert_eq!(run(&mut s, "list_pages", json!({})).unwrap()[0]["symbols"], 0);
}

#[test]
fn locked_pages_are_not_changed() {
    let mut s = new_session();
    s.project_mut().unwrap().pages[0].locked = true;
    let err = run(&mut s, "add_text", json!({ "page": "1", "x": 0, "y": 0, "text": "x" })).unwrap_err();
    assert_eq!(err, "Page 1 is locked");
}

#[test]
fn routing_autoconnect_and_definitions() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "no", "x": 40, "y": 40 })).unwrap();
    let coil = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 40, "y": 80 })).unwrap();
    let auto = run(&mut s, "autoconnect", json!({ "page": "1" })).unwrap();
    assert_eq!(auto["ids"].as_array().unwrap().len(), 1);
    // A route around the coil to another coil beside it.
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 120, "y": 80, "tag": "-K9" })).unwrap();
    let from = format!("#{}:A2", coil["id"]);
    let routed = run(&mut s, "route", json!({ "page": "1", "from": from, "to": "-K9:A2" })).unwrap();
    assert!(!routed["ids"].as_array().unwrap().is_empty());
    let wire = routed["ids"][0].as_u64().unwrap();
    let pts = routed["points"].as_array().unwrap();
    let (x, y) = (pts[0][0].as_f64().unwrap() as f32, pts[0][1].as_f64().unwrap() as f32);
    let (x1, y1) = (pts[1][0].as_f64().unwrap() as f32, pts[1][1].as_f64().unwrap() as f32);
    let cd = run(&mut s, "add_connection_definition", json!({ "page": "1", "x": (x + x1) / 2.0, "y": (y + y1) / 2.0, "number": "W5", "color": "BU" })).unwrap();
    assert!(cd["wires_changed"].as_u64().unwrap() >= 1);
    let prj = s.project().unwrap();
    let w = prj.pages[0].wires.iter().find(|w| w.id == wire).unwrap();
    assert_eq!((w.number.as_str(), w.color.as_str()), ("W5", "BU"));
    assert!(run(&mut s, "add_connection_definition", json!({ "page": "1", "x": 300, "y": 10 })).is_err());

    let cable = run(&mut s, "add_cable_definition", json!({ "page": "1", "points": [[30, 60], [50, 60]] })).unwrap();
    assert_eq!(cable["cores"].as_array().unwrap().len(), 1);
    assert!(run(&mut s, "add_cable_definition", json!({ "page": "1", "points": [[30, 60], [50, 60]], "cable": "-W99" })).is_err());
}

#[test]
fn texts_with_style_placeholders_and_links() {
    let mut s = new_session();
    let t = run(
        &mut s,
        "add_text",
        json!({ "page": "1", "x": 100, "y": 100, "text": "Pump {page.name}", "angle": 90, "align": "center", "valign": "middle", "bold": true, "width": 40, "link": "https://example.org", "path_function": true }),
    )
    .unwrap();
    assert_eq!((t["angle"].as_f64(), t["align"].as_str(), t["valign"].as_str(), t["bold"].as_bool()), (Some(90.0), Some("center"), Some("middle"), Some(true)));
    let id = t["id"].as_u64().unwrap();
    assert!(run(&mut s, "update_text", json!({ "page": "1", "id": id, "align": "sideways" })).is_err());
    let t = run(&mut s, "update_text", json!({ "page": "1", "id": id, "italic": true, "angle": -90, "text": "Motor" })).unwrap();
    assert_eq!((t["italic"].as_bool(), t["angle"].as_f64(), t["text"].as_str(), t["bold"].as_bool()), (Some(true), Some(270.0), Some("Motor"), Some(true)));
    let item = s.project().unwrap().pages[0].texts.iter().find(|t| t.id == id).cloned().unwrap();
    assert!(item.path_function && item.link == "https://example.org" && item.width == 40.0);
    assert_eq!(run(&mut s, "update_text", json!({ "page": "1", "id": 999 })).unwrap_err(), "No text with id 999");
}

#[test]
fn align_distribute_and_copy() {
    let mut s = new_session();
    let ids: Vec<u64> = [(40, 40), (100, 60), (160, 48)]
        .iter()
        .map(|(x, y)| run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": x, "y": y })).unwrap()["id"].as_u64().unwrap())
        .collect();
    assert_eq!(run(&mut s, "align_items", json!({ "page": "1", "ids": ids, "align": "top" })).unwrap()["moved"], 2);
    assert!(run(&mut s, "align_items", json!({ "page": "1", "ids": ids, "align": "diagonal" })).is_err());
    let ys: Vec<f32> = s.project().unwrap().pages[0].symbols.iter().map(|s| s.pos.y).collect();
    assert_eq!(ys, [40.0, 40.0, 40.0]);
    assert_eq!(run(&mut s, "distribute_items", json!({ "page": "1", "ids": ids })).unwrap()["moved"], 0, "already even");
    let copied = run(&mut s, "copy_items", json!({ "page": "1", "ids": [ids[0]], "count": 2, "dx": 0, "dy": 40 })).unwrap();
    assert_eq!(copied["ids"].as_array().unwrap().len(), 2);
    assert_eq!(s.project().unwrap().pages[0].symbols.len(), 5);
}

#[test]
fn positions_tags_and_wires_are_checked() {
    let mut s = new_session();
    let err = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": -5000, "y": -5000 })).unwrap_err();
    assert!(err.contains("outside the page"), "{err}");
    assert!(run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 1e12, "y": 100 })).is_err());
    assert!(run(&mut s, "add_text", json!({ "page": "1", "x": 500, "y": 10, "text": "x" })).is_err());
    assert!(run(&mut s, "add_wire", json!({ "page": "1", "points": [[10, 10], [900, 10]] })).is_err());
    // An empty tag is the next free one; tags need their prefix and no separators.
    let coil = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100, "tag": "" })).unwrap();
    assert_eq!(coil["tag"], "-K1");
    for tag in ["K 1;<x>", "S7", "-K;5"] {
        assert!(run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 120, "y": 100, "tag": tag })).is_err(), "{tag}");
        assert!(run(&mut s, "rename_device", json!({ "old": "-K1", "new": tag })).is_err(), "{tag}");
    }
    let err = run(&mut s, "rename_device", json!({ "old": "-K1", "new": "" })).unwrap_err();
    assert!(err.contains("must not be empty"), "{err}");
    assert!(run(&mut s, "update_symbol", json!({ "page": "1", "id": coil["id"], "x": -40 })).is_err());
    // Wires without length and a point connected to itself are refused.
    assert!(run(&mut s, "add_wire", json!({ "page": "1", "points": [[10, 10], [10, 10]] })).unwrap_err().contains("no length"));
    assert!(run(&mut s, "connect", json!({ "page": "1", "from": "-K1:A1", "to": "-K1:A1" })).is_err());
    assert!(run(&mut s, "set_page", json!({ "page": "1", "name": " " })).is_err());
}

#[test]
fn symbols_outside_the_page_are_reported() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    s.project_mut().unwrap().pages[0].symbols[0].pos = p(-500.0, 40.0);
    assert!(issues(&mut s).iter().any(|m| m == "-K1 lies outside page 1"), "{:?}", issues(&mut s));
}

#[test]
fn positions_are_named_as_given_and_items_stay_on_the_page() {
    let mut s = new_session();
    let err = run(&mut s, "add_wire", json!({ "page": "1", "points": [[10, 10], [900, 10]] })).unwrap_err();
    assert!(err.contains("900 / 10 mm"), "{err}");
    let err = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 1e308, "y": 100 })).unwrap_err();
    assert!(err.contains("1e308") && !err.contains("inf"), "{err}");
    let coil = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    let ids = json!([coil["id"]]);
    let before = s.project().unwrap().clone();
    assert!(run(&mut s, "move_items", json!({ "page": "1", "ids": ids, "dx": 1000, "dy": 0 })).unwrap_err().contains("leave the page"));
    assert!(run(&mut s, "copy_items", json!({ "page": "1", "ids": ids, "count": 1000, "dx": 40, "dy": 0 })).is_err());
    assert!(run(&mut s, "copy_items", json!({ "page": "1", "ids": ids, "count": 0, "dx": 40, "dy": 0 })).is_err());
    assert!(run(&mut s, "copy_items", json!({ "page": "1", "ids": ids, "count": 20, "dx": 40, "dy": 0 })).unwrap_err().contains("leave the page"));
    assert_eq!(s.project().unwrap(), &before, "refused commands change nothing");
    // Text heights and wire lengths are refused out of range, not corrected.
    assert!(run(&mut s, "add_text", json!({ "page": "1", "x": 20, "y": 20, "text": "x", "size": -5 })).is_err());
    assert!(run(&mut s, "add_text", json!({ "page": "1", "x": 20, "y": 20, "text": "x", "size": 0 })).is_err());
    assert_eq!(run(&mut s, "add_text", json!({ "page": "1", "x": 20, "y": 20, "text": "x", "size": 2.5 })).unwrap()["size"], 2.5);
}

#[test]
fn wires_do_not_short_circuit_a_function() {
    let mut s = new_session();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    for tool in ["connect", "route"] {
        let err = run(&mut s, tool, json!({ "page": "1", "from": "-K1:A1", "to": "-K1:A2" })).unwrap_err();
        assert!(err.contains("short-circuit"), "{tool}: {err}");
    }
    // Poles of a multi-pole symbol may be bridged (star point of a star contactor).
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "main3", "x": 200, "y": 100, "tag": "-Q1" })).unwrap();
    assert!(run(&mut s, "connect", json!({ "page": "1", "from": "-Q1:2", "to": "-Q1:4" })).is_ok());
}

#[test]
fn definitions_need_wires_and_known_values() {
    let mut s = new_session();
    let w = run(&mut s, "add_wire", json!({ "page": "1", "points": [[40, 40], [40, 120]] })).unwrap();
    assert!(run(&mut s, "add_cable_definition", json!({ "page": "1", "points": [[100, 100], [100, 100]] })).unwrap_err().contains("no length"));
    assert!(run(&mut s, "add_cable_definition", json!({ "page": "1", "points": [[100, 80], [200, 80]] })).unwrap_err().contains("crosses no wire"));
    assert!(run(&mut s, "add_cable_definition", json!({ "page": "1", "points": [[20, 80], [60, 80]] })).is_ok());
    assert_eq!(run(&mut s, "add_connection_definition", json!({ "page": "1", "x": 200, "y": 200 })).unwrap_err(), "No wire at 200 / 200 mm");
    let id = w["ids"][0].clone();
    assert!(run(&mut s, "update_wire", json!({ "page": "1", "id": id, "color": "notacolor" })).unwrap_err().starts_with("Unknown wire colour notacolor"));
    assert!(run(&mut s, "update_wire", json!({ "page": "1", "id": id, "cross_section": "abc" })).unwrap_err().starts_with("Not a cross-section"));
    assert_eq!(run(&mut s, "update_wire", json!({ "page": "1", "id": id, "color": "gn/ye", "cross_section": "1,5 mm²" })).unwrap()["color"], "gn/ye");
    assert!(run(&mut s, "update_wire", json!({ "page": "1", "id": id, "length": -1 })).is_err());
    let changed = run(&mut s, "update_wire", json!({ "page": "1", "id": id, "wire_type": "MKEM" })).unwrap();
    assert_eq!(changed["wire_type"], "MKEM");
    assert_eq!(s.project().unwrap().pages[0].wires[0].wire_type, "MKEM");
    assert_eq!(run(&mut s, "update_wire", json!({ "page": "1", "id": id, "wire_type": "" })).unwrap()["wire_type"], "");
}

#[test]
fn parts_are_placed_as_new_devices() {
    let mut s = new_session();
    let lamp = run(&mut s, "place_part", json!({ "page": "1", "part": "GEN.LMP-GN-24", "x": 100, "y": 100 })).unwrap();
    assert_eq!(lamp["part"], "GEN.LMP-GN-24");
    let tag = lamp["tag"].as_str().unwrap().to_string();
    assert!(tag.starts_with("-P"), "{tag}");
    let pins: Vec<&str> = lamp["pins"].as_array().unwrap().iter().map(|p| p["pin"].as_str().unwrap()).collect();
    assert_eq!(pins, ["X1", "X2"]);
    let prj = s.project().unwrap();
    assert!(prj.parts.iter().any(|p| p.number == "GEN.LMP-GN-24"), "copied into the project parts");
    assert_eq!(prj.devices[&tag].parts[0].number, "GEN.LMP-GN-24");
    assert!(run(&mut s, "place_part", json!({ "page": "1", "part": "NO.SUCH-PART", "x": 0, "y": 0 })).is_err());
}
#[test]
fn insert_macro_installs_portable_custom_symbols() {
    let mut session = new_session();
    let m = json!({"name":"Portable", "clip":{
        "symbol_defs":[{"key":"user:portable", "name":"Portable drawing"}],
        "symbols":[{"id":1,"sym":"user:portable","pos":{"x":0,"y":0}}]
    }});
    let result = run(&mut session,"insert_macro",json!({"page":"1","macro":m.to_string(),"x":40,"y":40})).unwrap();
    assert_eq!(result["symbol_definitions"],1);
    let project = session.project().unwrap();
    assert_eq!(project.symbols[0].key,"user:portable");
    assert_eq!(project.pages[0].symbols[0].pos,p(40.,40.));
}

#[test]
fn native_net_membership_is_available_through_plugins() {
    let mut session = new_session();
    let first = run(&mut session,"place_symbol",json!({"page":"1","symbol":"coil","x":40,"y":40,"tag":"-K1"})).unwrap();
    let second = run(&mut session,"place_symbol",json!({"page":"1","symbol":"coil","x":80,"y":40,"tag":"-K2"})).unwrap();
    run(&mut session,"connect",json!({"page":"1","from":"-K1:A1","to":"-K2:A1"})).unwrap();
    let nets = run(&mut session,"list_nets",json!({"bridge_terminals":false})).unwrap();
    let pins = nets["pins"].as_array().unwrap();
    let net = |id: &Value| pins.iter().find(|p| p["symbol"]==*id && p["pin"]==0).unwrap()["net"].clone();
    assert_eq!(net(&first["id"]),net(&second["id"]));
    let points=nets["connection_points"].as_array().unwrap();
    assert!(points.iter().any(|point|point["symbol"]==first["id"] && point["pin"]==0 && point["net"]==net(&first["id"])));
}

#[test]
fn t_junctions_route_the_wires_of_a_connection() {
    let mut s = new_session();
    for (tag, x, y) in [("-K1", 60, 70), ("-K2", 140, 70), ("-K3", 100, 90)] {
        run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": x, "y": y, "tag": tag })).unwrap();
    }
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[60, 66], [60, 60], [140, 60], [140, 66]] })).unwrap();
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[100, 60], [100, 86]] })).unwrap();
    let err = run(&mut s, "add_t_junction", json!({ "page": "1", "x": 80, "y": 60 })).unwrap_err();
    assert!(err.contains("No three wires"), "{err}");
    assert!(run(&mut s, "add_t_junction", json!({ "page": "1", "x": 100, "y": 60, "direction": "sideways" })).is_err());
    assert!(run(&mut s, "add_t_junction", json!({ "page": "1", "x": 100, "y": 60, "variant": 5 })).is_err());
    let j = run(&mut s, "add_t_junction", json!({ "page": "1", "x": 100, "y": 60 })).unwrap();
    assert_eq!((j["direction"].as_str(), j["variant"].as_i64(), j["fits"].as_bool()), (Some("down"), Some(3), Some(true)), "{j}");
    let wires = |v: &Value| v["wires"].as_array().unwrap().iter().map(|w| format!("{}>{}", w["from"].as_str().unwrap(), w["to"].as_str().unwrap())).collect::<Vec<_>>();
    assert_eq!(wires(&j), ["-K3:A1>-K1:A1", "-K3:A1>-K2:A1"]);
    let id = j["id"].clone();
    let j = run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "variant": 1, "dot": true })).unwrap();
    assert_eq!(wires(&j), ["-K1:A1>-K2:A1", "-K1:A1>-K3:A1"]);
    assert_eq!(j["dot"], true);
    let list = run(&mut s, "list_connection_symbols", json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["routing"], "Source on the left, first target on the right, second target below");
    assert!(run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": 999_999, "variant": 2 })).unwrap_err().contains("No connection symbol"));
    assert!(run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "arms": "down left" })).is_err(), "a T-junction has no arms parameter");
    let wrong = run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "direction": "up" })).unwrap();
    assert_eq!(wrong["fits"], false);
    assert!(issues(&mut s).iter().any(|m| m.contains("does not fit")));
    run(&mut s, "delete_items", json!({ "page": "1", "ids": [id] })).unwrap();
    assert!(run(&mut s, "list_connection_symbols", json!({ "page": "1" })).unwrap().as_array().unwrap().is_empty());
}

#[test]
fn corners_are_wired_in_by_the_automatic_connections() {
    let mut s = new_session();
    // A contact above the corner, a coil to its right: their pins face the corner's arms.
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "no", "x": 60, "y": 40, "tag": "-S1" })).unwrap();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 120, "y": 80, "rotation": 90, "tag": "-K1" })).unwrap();
    assert!(run(&mut s, "add_corner", json!({ "page": "1", "x": 60, "y": 80, "arms": "down sideways" })).is_err());
    assert!(run(&mut s, "add_corner", json!({ "page": "1", "x": 60, "y": 80, "arms": "left right" })).unwrap_err().contains("vertical and one horizontal"));
    let c = run(&mut s, "add_corner", json!({ "page": "1", "x": 60, "y": 80, "arms": "up right", "autoconnect": true })).unwrap();
    assert_eq!(c["kind"], "corner");
    assert_eq!(c["arms"], json!(["up", "right"]));
    assert_eq!(c["wires_drawn"], 2, "{c}");
    assert_eq!(c["fits"], true);
    let wires = c["wires"].as_array().unwrap();
    assert_eq!(wires.len(), 1, "one connection -S1 – -K1 through the corner: {wires:?}");
    let id = c["id"].clone();
    assert!(run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "variant": 2 })).unwrap_err().contains("has no variant"));
    let turned = run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "arms": "down left" })).unwrap();
    assert_eq!((turned["arms"].clone(), turned["fits"].as_bool()), (json!(["down", "left"]), Some(false)));
    assert!(run(&mut s, "add_corner", json!({ "page": "1", "x": 60, "y": 80 })).unwrap_err().contains("already"));
    assert!(issues(&mut s).iter().any(|m| m.contains("Corner on page 1 does not fit")));
    // Without arms given the corner takes the two wires at its point.
    let again = run(&mut s, "add_corner", json!({ "page": "1", "x": 100, "y": 120 })).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(again["fits"], false, "no wires at that point, the default arms are used: {again}");
    assert_eq!(again["arms"], json!(["down", "right"]));
}

#[test]
fn crossings_and_bridges_connect_four_wires() {
    let mut s = new_session();
    for (tag, x, y, rot) in [("-K1", 100, 36, 0), ("-K2", 100, 84, 0), ("-K3", 76, 60, 90), ("-K4", 124, 60, 90)] {
        run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": x, "y": y, "tag": tag, "rotation": rot })).unwrap();
    }
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[100, 40], [100, 80]] })).unwrap();
    run(&mut s, "add_wire", json!({ "page": "1", "points": [[80, 60], [120, 60]] })).unwrap();
    let wires = |v: &Value| v["wires"].as_array().unwrap().iter().map(|w| format!("{}>{}", w["from"].as_str().unwrap(), w["to"].as_str().unwrap())).collect::<Vec<_>>();
    assert!(run(&mut s, "add_crossing", json!({ "page": "1", "x": 100, "y": 60, "line": "diagonal" })).is_err());
    assert!(run(&mut s, "add_crossing", json!({ "page": "1", "x": 100, "y": 60, "variant": 3 })).is_err());
    let c = run(&mut s, "add_crossing", json!({ "page": "1", "x": 100, "y": 60 })).unwrap();
    assert_eq!((c["kind"].as_str(), c["line"].as_str(), c["variant"].as_i64(), c["fits"].as_bool()), (Some("crossing"), Some("vertical"), Some(2), Some(true)), "{c}");
    assert_eq!(wires(&c), ["-K1:A2>-K2:A1", "-K1:A2>-K3:A1", "-K2:A1>-K4:A2"]);
    assert_eq!(c["links"], json!([["up", "down"], ["up", "left"], ["down", "right"]]));
    let id = c["id"].clone();
    let c = run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "line": "horizontal", "variant": 1, "dot": true })).unwrap();
    assert_eq!(wires(&c), ["-K3:A1>-K4:A2", "-K3:A1>-K2:A1", "-K4:A2>-K1:A2"]);
    assert!(run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": id, "direction": "up" })).unwrap_err().contains("has no direction"));
    run(&mut s, "delete_items", json!({ "page": "1", "ids": [id] })).unwrap();
    assert!(run(&mut s, "add_bridge", json!({ "page": "1", "x": 100, "y": 60, "direction": "sideways" })).is_err());
    let b = run(&mut s, "add_bridge", json!({ "page": "1", "x": 100, "y": 60, "direction": "left" })).unwrap();
    assert_eq!((b["kind"].as_str(), b["direction"].as_str(), b["fits"].as_bool()), (Some("bridge"), Some("left"), Some(true)), "{b}");
    assert_eq!(wires(&b), ["-K3:A1>-K4:A2", "-K3:A1>-K1:A2", "-K3:A1>-K2:A1"]);
    assert!(run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": b["id"], "variant": 1 })).unwrap_err().contains("has no variant"));
    let b = run(&mut s, "update_connection_symbol", json!({ "page": "1", "id": b["id"], "direction": "down", "dot": true })).unwrap();
    assert_eq!(wires(&b)[0], "-K2:A1>-K1:A2");
    assert_eq!(run(&mut s, "list_connection_symbols", json!({})).unwrap().as_array().unwrap().len(), 1);
}
