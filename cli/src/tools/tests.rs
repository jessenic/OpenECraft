use super::*;
use crate::testing::session;
use serde_json::json;

fn args(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[test]
fn tool_and_parameter_names_are_unique_snake_case() {
    let snake = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    let mut names = std::collections::HashSet::new();
    let s = session();
    for tool in entries(&s) {
        assert!(snake(tool.name()), "{}", tool.name());
        assert!(names.insert(tool.name().to_string()), "duplicate tool {}", tool.name());
        assert!(!tool.help().is_empty());
        let mut params = std::collections::HashSet::new();
        for p in tool.params() {
            assert!(snake(&p.name), "{}.{}", tool.name(), p.name);
            assert!(params.insert(p.name.to_string()), "duplicate parameter {}.{}", tool.name(), p.name);
            assert!(!p.help.is_empty());
        }
    }
    assert!(names.len() > 150, "{}", names.len());
    // Built-in tools are the ones that need the session or the machine (files, parts
    // server, Autodesk Vault, undo); everything else is a plugin command.
    assert!(all().count() < 90, "the project commands are plugin commands");
}

#[test]
fn schema_lists_types_and_required_parameters() {
    let session = session();
    let s = schema(lookup(&session, "add_wire").unwrap());
    assert_eq!(s["type"], "object");
    assert_eq!(s["additionalProperties"], false);
    assert_eq!(s["required"], json!(["page", "points"]));
    assert_eq!(s["properties"]["page"]["type"], "string");
    assert_eq!(s["properties"]["points"]["items"]["maxItems"], 2);
    assert!(s["properties"]["points"]["description"].is_string());
}

#[test]
fn call_checks_the_parameters() {
    let mut s = session();
    let tool = find("open_project").unwrap();
    assert_eq!(call(&mut s, tool, &args(json!({}))).unwrap_err(), "Missing parameter: path");
    assert_eq!(call(&mut s, tool, &args(json!({ "path": 3 }))).unwrap_err(), "Parameter path has the wrong type");
    assert_eq!(call(&mut s, tool, &args(json!({ "path": "a", "x": 1 }))).unwrap_err(), "Unknown parameter: x");
    let list_pages = lookup(&s, "list_pages").unwrap();
    assert!(matches!(list_pages, Entry::Plugin(_)), "project operations are plugin commands");
    let err = call(&mut s, list_pages, &Map::new()).unwrap_err();
    assert!(err.starts_with("No project is open"), "{err}");
}

#[test]
fn command_line_values_follow_the_parameter_type() {
    let pair = |n: &str, v: Option<&str>| (n.to_string(), v.map(String::from));
    let s = session();
    let place = lookup(&s, "place_symbol").unwrap();
    let map = from_cli(place.clone(), None, &[pair("x", Some("12.5")), pair("rotation", Some("90")), pair("mirror", None), pair("tag", Some("-K1"))]).unwrap();
    assert_eq!(Value::Object(map), json!({ "x": 12.5, "rotation": 90, "mirror": true, "tag": "-K1" }));
    assert!(from_cli(place.clone(), None, &[pair("x", Some("left"))]).is_err());
    assert!(from_cli(place.clone(), None, &[pair("tag", None)]).is_err());
    assert!(from_cli(place, None, &[pair("colour", Some("red"))]).is_err());

    let del = lookup(&s, "delete_items").unwrap();
    assert_eq!(from_cli(del.clone(), None, &[pair("ids", Some("3, 4,5"))]).unwrap()["ids"], json!([3, 4, 5]));
    assert_eq!(from_cli(del, None, &[pair("ids", Some("[6]"))]).unwrap()["ids"], json!([6]));

    let wire = lookup(&s, "add_wire").unwrap();
    assert_eq!(from_cli(wire.clone(), None, &[pair("points", Some("0,0 8,-4"))]).unwrap()["points"], json!([[0.0, 0.0], [8.0, -4.0]]));
    let map = from_cli(wire.clone(), Some(r#"{"page": "1", "points": [[0, 0], [4, 0]]}"#), &[pair("page", Some("2"))]).unwrap();
    assert_eq!(map["page"], "2", "--name overrides --json");
    assert!(from_cli(wire, Some("[1]"), &[]).is_err());
}

#[test]
fn help_shows_all_parameters() {
    let s = session();
    let connect = lookup(&s, "connect").unwrap();
    let text = help(connect.clone());
    assert!(text.starts_with("openecraft-cli connect --project <file>"));
    for p in connect.params() {
        assert!(text.contains(&format!("--{}", p.name)));
    }
    assert!(text.contains("(optional)"));
}

#[test]
fn tidy_removes_the_noise_of_f32_numbers() {
    let mut v = json!({ "length": 41.8f32 as f64, "list": [0.1f32 as f64, 2], "exact": 0.1, "count": 3 });
    tidy(&mut v);
    assert_eq!(v, json!({ "length": 41.8, "list": [0.1, 2], "exact": 0.1, "count": 3 }));
    assert_eq!(serde_json::to_string(&v["length"]).unwrap(), "41.8");
}

#[test]
fn devices_moved_by_a_command_follow_structure_boxes() {
    let mut s = crate::testing::new_session();
    let run = |s: &mut Session, tool: &str, v: Value| crate::testing::run(s, tool, v);
    run(&mut s, "set_page", json!({ "page": "1", "function": "SYS", "location": "CAB1" })).unwrap();
    let k1 = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 40, "y": 40 })).unwrap();
    let k2 = run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 60, "y": 40 })).unwrap();
    run(&mut s, "add_structure_box", json!({ "page": "1", "x1": 100, "y1": 100, "x2": 200, "y2": 200, "location": "FIELD" })).unwrap();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 120, "y": 120, "tag": "=SYS+FIELD-K2" })).unwrap();
    // Moved in: the device takes the location of the box.
    let moved = run(&mut s, "update_symbol", json!({ "page": "1", "id": k1["id"], "x": 140, "y": 140 })).unwrap();
    assert!(moved.get("structure_kept").is_none(), "{moved}");
    assert!(s.project().unwrap().all_tags().contains(&"=SYS+FIELD-K1".to_string()));
    // The tag exists in the box already: kept and reported.
    let kept = run(&mut s, "update_symbol", json!({ "page": "1", "id": k2["id"], "x": 160, "y": 160 })).unwrap();
    assert!(kept["structure_kept"][0].as_str().unwrap().contains("=SYS+FIELD-K2"), "{kept}");
    assert!(s.project().unwrap().all_tags().contains(&"=SYS+CAB1-K2".to_string()));
    // Undo takes back the move and the new tag in one step.
    run(&mut s, "undo", json!({})).unwrap();
    run(&mut s, "undo", json!({})).unwrap();
    assert!(s.project().unwrap().all_tags().contains(&"=SYS+CAB1-K1".to_string()));
}
