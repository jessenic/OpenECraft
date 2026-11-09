use super::*;
use crate::builtin::testing::*;

#[test]
fn boxes_with_connection_points_are_drawn_by_commands() {
    let mut s = new_session();
    let b = run(&mut s, "add_device_box", json!({ "page": "1", "x": 100, "y": 100, "width": 48, "height": 24, "function_text": "Power supply" })).unwrap();
    assert_eq!((b["tag"].as_str(), b["width"].as_f64(), b["height"].as_f64()), (Some("-A1"), Some(48.0), Some(24.0)));
    let id = b["id"].as_u64().unwrap();
    // Top edge: L, N, PE pointing up; bottom edge: numbered on from 13.
    let top = run(&mut s, "add_device_connections", json!({ "page": "1", "x": 108, "y": 100, "designations": "L,N,PE" })).unwrap();
    let names: Vec<&str> = top.as_array().unwrap().iter().map(|p| p["designation"].as_str().unwrap()).collect();
    assert_eq!(names, ["L", "N", "PE"]);
    assert!(top.as_array().unwrap().iter().all(|p| p["direction"] == "up" && p["tag"] == "-A1" && p["box"] == id));
    assert_eq!((top[2]["x"].as_f64(), top[2]["pins"][0]["y"].as_f64()), (Some(124.0), Some(96.0)));
    let bottom = run(&mut s, "add_device_connections", json!({ "page": "1", "x": 108, "y": 124, "designations": "13", "count": 2 })).unwrap();
    assert_eq!((bottom[0]["designation"].as_str(), bottom[1]["designation"].as_str(), bottom[1]["direction"].as_str()), (Some("13"), Some("14"), Some("down")));
    let right = run(&mut s, "add_device_connections", json!({ "page": "1", "x": 148, "y": 108 })).unwrap();
    assert_eq!((right[0]["designation"].as_str(), right[0]["direction"].as_str()), (Some("15"), Some("right")));
    assert!(run(&mut s, "add_device_connections", json!({ "page": "1", "x": 300, "y": 200 })).unwrap_err().contains("no device box"));

    let list = run(&mut s, "list_device_boxes", json!({ "page": "1" })).unwrap();
    assert_eq!(list[0]["points"].as_array().map(Vec::len), Some(6));

    // Wider: the right edge moves with its point; a new tag takes the points along.
    let changed = run(&mut s, "update_device_box", json!({ "page": "1", "id": id, "width": 64, "tag": "-G1" })).unwrap();
    assert_eq!(changed["width"].as_f64(), Some(64.0));
    let points = changed["points"].as_array().unwrap();
    assert!(points.iter().all(|p| p["tag"] == "-G1"));
    assert!(points.iter().any(|p| p["designation"] == "15" && p["x"] == 164.0));
    // Moving the box moves its points.
    let moved = run(&mut s, "update_device_box", json!({ "page": "1", "id": id, "x": 120 })).unwrap();
    assert!(moved["points"].as_array().unwrap().iter().any(|p| p["designation"] == "L" && p["x"] == 128.0));

    let pid = points.iter().find(|p| p["designation"] == "PE").unwrap()["id"].as_u64().unwrap();
    let p = run(&mut s, "update_device_connection", json!({ "page": "1", "id": pid, "designation": "+24V", "label_dx": 2, "label_dy": -6, "label_align": "left" })).unwrap();
    assert_eq!((p["designation"].as_str(), p["designation_at"]["align"].as_str(), p["designation_at"]["dx"].as_f64()), (Some("+24V"), Some("left"), Some(2.0)));
    let p = run(&mut s, "update_device_connection", json!({ "page": "1", "id": pid, "label_auto": true, "direction": "left" })).unwrap();
    assert!(p["designation_at"].is_null());
    assert_eq!(p["direction"], "left");
    assert!(run(&mut s, "update_device_connection", json!({ "page": "1", "id": id })).is_err(), "a box is no connection point");
}

#[test]
fn copies_and_deletes_take_a_box_with_its_points() {
    let mut s = new_session();
    let id = run(&mut s, "add_device_box", json!({ "page": "1", "x": 100, "y": 100, "width": 32, "height": 16 })).unwrap()["id"].as_u64().unwrap();
    run(&mut s, "add_device_connections", json!({ "page": "1", "x": 108, "y": 100, "count": 2 })).unwrap();
    let copies = run(&mut s, "copy_items", json!({ "page": "1", "ids": [id], "dx": 0, "dy": 40 })).unwrap();
    assert_eq!(copies["ids"].as_array().map(Vec::len), Some(3));
    let list = run(&mut s, "list_device_boxes", json!({ "page": "1" })).unwrap();
    assert_eq!(list[1]["tag"], "-A2");
    assert_eq!(list[1]["points"].as_array().map(Vec::len), Some(2));
    let deleted = run(&mut s, "delete_items", json!({ "page": "1", "ids": [id] })).unwrap();
    assert_eq!(deleted["deleted"], 3);
    let symbols = run(&mut s, "list_symbols", json!({ "query": "devconn" })).unwrap();
    assert_eq!(symbols[0]["function"], "device_connection");
}
