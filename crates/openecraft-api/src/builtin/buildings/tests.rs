use super::*;
use crate::builtin::testing::*;

#[test]
fn buildings_with_levels_rooms_and_shafts() {
    let mut s = demo_session();
    let b = run(&mut s, "add_building", json!({ "name": "+B1", "x": 20000, "z": 0 })).unwrap();
    assert_eq!(b["levels"][0]["name"], "L0");
    assert!(run(&mut s, "add_building", json!({ "name": "+B1" })).unwrap_err().contains("exists"));
    run(&mut s, "add_level", json!({ "building": "+B1" })).unwrap();
    let b = run(&mut s, "add_level", json!({ "building": "B1", "elevation": -3000, "height": 3000 })).unwrap();
    let names: Vec<&str> = b["levels"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["B1", "L0", "L1"]);
    assert!(run(&mut s, "add_level", json!({ "building": "+B1", "elevation": 1000 })).unwrap_err().contains("overlap"));
    // Rooms on the levels: as high as the level less its slab, side by side.
    let r = run(&mut s, "add_room", json!({ "name": "+R0", "building": "+B1" })).unwrap();
    assert_eq!((r["building"].as_str(), r["level"].as_str()), (Some("+B1"), Some("L0")));
    assert_eq!(r["size"]["height"], 2700.0);
    assert_eq!(r["at"], json!([20000.0, 0.0, 0.0]));
    let r = run(&mut s, "add_room", json!({ "name": "+R1", "building": "+B1", "level": "L1", "x": 0, "z": 0 })).unwrap();
    assert_eq!(r["at"], json!([20000.0, 3000.0, 0.0]));
    let r = run(&mut s, "add_room", json!({ "name": "+R2", "building": "+B1", "level": "L0", "width": 2000 })).unwrap();
    assert_eq!(r["at"], json!([24000.0, 0.0, 0.0]), "right of +R0");
    // A shaft through all levels joins the rooms it passes.
    let b = run(&mut s, "add_shaft", json!({ "building": "+B1", "x": 3000, "z": 2000 })).unwrap();
    assert_eq!(b["shafts"][0]["name"], "S1");
    assert_eq!((b["shafts"][0]["bottom"].as_str(), b["shafts"][0]["top"].as_str()), (Some("B1"), Some("L1")));
    assert_eq!(b["shafts"][0]["rooms"], json!(["+R0", "+R1"]));
    let b = run(&mut s, "set_shaft", json!({ "building": "+B1", "shaft": "s1", "bottom": "L0", "width": 400 })).unwrap();
    assert_eq!((b["shafts"][0]["bottom"].as_str(), b["shafts"][0]["width"].as_f64()), (Some("L0"), Some(400.0)));
    assert!(run(&mut s, "set_shaft", json!({ "building": "+B1", "shaft": "S1", "width": 10 })).is_err());
    let list = run(&mut s, "list_buildings", json!({})).unwrap();
    assert_eq!(list["problems"], json!([]));
    assert_eq!(list["buildings"][0]["levels"][1]["rooms"].as_array().unwrap().len(), 2);
    // The building moves, its rooms with it.
    run(&mut s, "set_building", json!({ "building": "+B1", "x": 0 })).unwrap();
    let r = run(&mut s, "list_rooms", json!({ "room": "+R1" })).unwrap();
    assert_eq!(r[0]["at"], json!([0.0, 3000.0, 0.0]));
    // Out of the building: it stays where it is.
    let r = run(&mut s, "place_on_level", json!({ "room": "+R1" })).unwrap();
    assert_eq!(r["building"], Value::Null);
    let r = run(&mut s, "list_rooms", json!({ "room": "+R1" })).unwrap();
    assert_eq!((r[0]["at"].clone(), r[0]["building"].clone()), (json!([0.0, 3000.0, 0.0]), Value::Null));
    let b = run(&mut s, "place_on_level", json!({ "room": "+R1", "building": "+B1", "level": "3", "x": 0, "z": 0 })).unwrap();
    assert_eq!(b["levels"][2]["rooms"][0]["room"], "+R1");
    run(&mut s, "set_level", json!({ "building": "+B1", "level": "B1", "remove": true })).unwrap();
    run(&mut s, "set_shaft", json!({ "building": "+B1", "shaft": "S1", "remove": true })).unwrap();
    let r = run(&mut s, "set_building", json!({ "building": "+B1", "remove": true })).unwrap();
    assert_eq!(r["removed"], "+B1");
}

#[test]
fn links_between_buildings() {
    let mut s = demo_session();
    for k in 1..=2 {
        run(&mut s, "add_building", json!({ "name": format!("+B{k}") })).unwrap();
        run(&mut s, "add_room", json!({ "name": format!("+R{k}"), "building": format!("+B{k}") })).unwrap();
    }
    assert!(run(&mut s, "add_site_link", json!({ "from": "+R1", "to": "+R2" })).unwrap_err().contains("building entry"));
    for k in 1..=2 {
        run(&mut s, "add_wall_opening", json!({ "room": format!("+R{k}"), "kind": "entry", "wall": "wall3" })).unwrap();
    }
    let l = run(&mut s, "add_site_link", json!({ "from": "+R1", "to": "+R2", "kind": "conduit", "points": [[2000, 6000]] })).unwrap();
    assert_eq!((l["name"].as_str(), l["kind"].as_str(), l["to"].as_str()), (Some("SL1"), Some("conduit"), Some("+R2")));
    assert!(l["length"].as_f64().unwrap() > 10_000.0, "{l}");
    let l = run(&mut s, "set_site_link", json!({ "link": "sl1", "kind": "bridge", "length": 25000 })).unwrap();
    assert_eq!((l["kind"].as_str(), l["length"].as_f64()), (Some("bridge"), Some(25000.0)));
    assert!(run(&mut s, "set_site_link", json!({ "link": "SL1", "kind": "rope" })).is_err());
    let list = run(&mut s, "list_buildings", json!({})).unwrap();
    assert_eq!(list["links"].as_array().unwrap().len(), 1);
    assert_eq!(list["problems"], json!([]));
    run(&mut s, "set_site_link", json!({ "link": "SL1", "remove": true })).unwrap();
}

#[test]
fn devices_on_the_facade_and_the_roof() {
    let mut s = demo_session();
    run(&mut s, "add_building", json!({ "name": "+B1" })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+R1", "building": "+B1" })).unwrap();
    let r = run(&mut s, "list_rooms", json!({ "room": "+R1" })).unwrap();
    assert_eq!(r[0]["problems"], json!([]));
    // A contactor needs a rail outside too.
    assert!(run(&mut s, "mount_on_wall", json!({ "room": "+R1", "device": "-K1", "face": "outside3", "x": 1000, "y": 500 })).unwrap_err().contains("wall mounting"));
    run(&mut s, "add_wall_profile", json!({ "room": "+R1", "kind": "rail", "face": "outside3", "x1": 900, "y1": 540, "x2": 1500, "y2": 540 })).unwrap();
    let m = run(&mut s, "mount_on_wall", json!({ "room": "+R1", "device": "-K1", "face": "outside3", "x": 1000, "y": 500 })).unwrap();
    assert_eq!(m["on_rail"], true);
    let r = run(&mut s, "list_rooms", json!({ "room": "+R1" })).unwrap();
    let faces: Vec<&str> = r[0]["faces"].as_array().unwrap().iter().map(|f| f["face"].as_str().unwrap()).collect();
    assert_eq!(faces, ["outside3"]);
    assert_eq!(r[0]["problems"], json!([]));
    assert!(run(&mut s, "mount_on_wall", json!({ "room": "+R1", "device": "-K2", "face": "outside9", "x": 0, "y": 0 })).unwrap_err().contains("Unknown face"));
}

#[test]
fn stairs_roofs_and_roof_windows() {
    let mut s = demo_session();
    run(&mut s, "add_building", json!({ "name": "+H1", "x": 30000, "z": 0 })).unwrap();
    run(&mut s, "add_level", json!({ "building": "+H1" })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+EG", "building": "+H1", "level": "L0", "x": 0, "z": 0, "width": 8000, "depth": 6000 })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+DG", "building": "+H1", "level": "L1", "x": 0, "z": 0, "width": 8000, "depth": 6000 })).unwrap();
    // A stair turned once from the ground floor up.
    let b = run(&mut s, "add_stair", json!({ "building": "+H1", "x": 1000, "z": 5000, "kind": "quarter_landing", "turn": "right" })).unwrap();
    let st = &b["stairs"][0];
    assert_eq!((st["name"].as_str(), st["kind"].as_str(), st["turn"].as_str()), (Some("ST1"), Some("quarter_landing"), Some("right")));
    assert_eq!((st["bottom"].as_str(), st["top"].as_str(), st["risers"].as_u64()), (Some("L0"), Some("L1"), Some(17)));
    assert!(run(&mut s, "add_stair", json!({ "building": "+H1", "x": 0, "z": 0, "kind": "ladder" })).unwrap_err().contains("ladder"));
    assert!(run(&mut s, "add_stair", json!({ "building": "+H1", "x": 0, "z": 0, "bottom": "L1" })).is_err(), "nothing above L1");
    let b = run(&mut s, "set_stair", json!({ "building": "+H1", "stair": "st1", "kind": "spiral", "width": 800 })).unwrap();
    assert_eq!((b["stairs"][0]["kind"].as_str(), b["stairs"][0]["width"].as_f64()), (Some("spiral"), Some(800.0)));
    // A gable roof low over the attic slopes its ceiling, not that of the ground floor.
    let b = run(&mut s, "add_roof", json!({ "building": "+H1", "shape": "gable", "eaves": 4000, "pitch": 40 })).unwrap();
    let r = &b["roofs"][0];
    assert_eq!((r["name"].as_str(), r["shape"].as_str(), r["eaves"].as_f64()), (Some("R1"), Some("gable"), Some(4000.0)));
    assert!(r["ridge"].as_f64().unwrap() > 6000.0);
    assert!(b["levels"][0]["rooms"][0].get("ceiling").is_none());
    let attic = &b["levels"][1]["rooms"][0]["ceiling"];
    assert!((attic["lowest"].as_f64().unwrap() - 1000.0).abs() < 300.0 && (attic["highest"].as_f64().unwrap() - 2700.0).abs() < 0.5, "{attic}");
    assert!(run(&mut s, "add_roof", json!({ "building": "+H1", "shape": "dome" })).unwrap_err().contains("dome"));
    // Roof windows in its slope.
    let b = run(&mut s, "add_roof_window", json!({ "building": "+H1", "roof": "R1" })).unwrap();
    let id = b["roofs"][0]["windows"][0]["id"].as_u64().unwrap();
    let b = run(&mut s, "set_roof_window", json!({ "building": "+H1", "roof": "R1", "window": id, "width": 1140 })).unwrap();
    assert_eq!(b["roofs"][0]["windows"][0]["width"], 1140.0);
    let list = run(&mut s, "list_buildings", json!({ "building": "+H1" })).unwrap();
    assert_eq!(list["problems"], json!([]), "{}", list["problems"]);
    let b = run(&mut s, "set_roof_window", json!({ "building": "+H1", "roof": "R1", "window": id, "remove": true })).unwrap();
    assert_eq!(b["roofs"][0]["windows"], json!([]));
    let b = run(&mut s, "set_roof", json!({ "building": "+H1", "roof": "R1", "shape": "flat", "eaves": 6000 })).unwrap();
    assert!(b["levels"][1]["rooms"][0].get("ceiling").is_none(), "a high flat roof leaves the ceiling flat");
    let b = run(&mut s, "set_roof", json!({ "building": "+H1", "roof": "R1", "remove": true })).unwrap();
    assert_eq!(b["roofs"], json!([]));
    let b = run(&mut s, "set_stair", json!({ "building": "+H1", "stair": "ST1", "remove": true })).unwrap();
    assert_eq!(b["stairs"], json!([]));
}

#[test]
fn pv_arrays_on_pitched_and_flat_roofs() {
    let mut s = demo_session();
    run(&mut s, "add_building", json!({ "name": "+H1", "x": 30000, "z": 0 })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+EG", "building": "+H1", "level": "L0", "x": 0, "z": 0, "width": 8000, "depth": 6000 })).unwrap();
    run(&mut s, "add_roof", json!({ "building": "+H1", "shape": "gable", "pitch": 40 })).unwrap();
    // On rails and roof hooks in the middle of the front slope.
    let b = run(&mut s, "add_pv_array", json!({ "building": "+H1", "roof": "R1", "module": "GEN.PV-400W-FB" })).unwrap();
    let a = &b["roofs"][0]["solar"][0];
    assert_eq!((a["name"].as_str(), a["mounting"].as_str(), a["modules"].as_u64()), (Some("PV1"), Some("parallel"), Some(8)));
    assert!((a["peak_kw"].as_f64().unwrap() - 3.2).abs() < 1e-4);
    let parts: Vec<&str> = a["material"].as_array().unwrap().iter().map(|m| m["part"].as_str().unwrap()).collect();
    assert_eq!(parts, ["GEN.PV-400W-FB", "GEN.PV-RAIL-40", "GEN.PV-RAIL-CONN", "GEN.PV-HOOK", "GEN.PV-CLAMP-END", "GEN.PV-CLAMP-MID"]);
    let list = run(&mut s, "list_buildings", json!({ "building": "+H1" })).unwrap();
    assert_eq!(list["problems"], json!([]), "{}", list["problems"]);
    let b = run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "pv1", "columns": 5, "rows": 1, "landscape": true })).unwrap();
    assert_eq!(b["roofs"][0]["solar"][0]["modules"], 5);
    assert!(run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "mounting": "wall" })).unwrap_err().contains("wall"));
    assert!(run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "module": "NOPE" })).unwrap_err().contains("NOPE"));
    assert!(run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "PV9" })).unwrap_err().contains("PV9"));
    // On a flat roof: east-west pairs on ballasted mounting bases.
    run(&mut s, "set_roof", json!({ "building": "+H1", "roof": "R1", "shape": "flat" })).unwrap();
    let b = run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "mounting": "east-west", "columns": 2, "tilt": 10, "azimuth": 90 })).unwrap();
    let a = &b["roofs"][0]["solar"][0];
    assert_eq!((a["mounting"].as_str(), a["modules"].as_u64()), (Some("east_west"), Some(4)));
    assert!(a["material"].as_array().unwrap().iter().any(|m| m["part"] == "GEN.PV-BASE-EW10" && m["qty"] == 3.0));
    let list = run(&mut s, "list_buildings", json!({ "building": "+H1" })).unwrap();
    assert_eq!(list["problems"], json!([]), "{}", list["problems"]);
    let b = run(&mut s, "set_pv_array", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "remove": true })).unwrap();
    assert!(b["roofs"][0].get("solar").is_some_and(|v| v == &json!([])));
}

#[test]
fn pv_strings_on_the_trackers_of_an_inverter() {
    let mut s = new_session();
    run(&mut s, "add_building", json!({ "name": "+H1", "x": 30000, "z": 0 })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+EG", "building": "+H1", "level": "L0", "x": 0, "z": 0, "width": 8000, "depth": 6000 })).unwrap();
    run(&mut s, "add_roof", json!({ "building": "+H1", "shape": "gable", "pitch": 40 })).unwrap();
    run(&mut s, "add_pv_array", json!({ "building": "+H1", "roof": "R1", "columns": 5 })).unwrap();
    let inv = run(&mut s, "place_part", json!({ "page": "1", "part": "GEN.INV-PV-10KW", "x": 200, "y": 100 })).unwrap();
    let tag = inv["tag"].as_str().unwrap().to_string();
    // All ten modules in series on the inverter's first tracker.
    let a = run(&mut s, "add_pv_string", json!({ "building": "+H1", "roof": "R1", "array": "PV1" })).unwrap();
    let st = &a["strings"][0];
    assert_eq!((st["name"].as_str(), st["series"].as_u64(), st["inverter"].as_str(), st["mppt"].as_u64()), (Some("S1"), Some(10), Some(tag.as_str()), Some(1)));
    assert!((st["values"]["voltage_max"].as_f64().unwrap() - 426.3).abs() < 0.1, "{st}");
    assert!((st["values"]["power"].as_f64().unwrap() - 4300.0).abs() < 0.1);
    let list = run(&mut s, "pv_strings", json!({})).unwrap();
    assert_eq!((list["strings"].as_array().unwrap().len(), list["problems"].clone()), (1, json!([])));
    // Two strings of five in parallel; at -30 °C still below the input voltage.
    let a = run(&mut s, "set_pv_string", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "string": "s1", "series": 5, "parallel": 2 })).unwrap();
    assert!((a["strings"][0]["values"]["current"].as_f64().unwrap() - 26.4).abs() < 0.01);
    let list = run(&mut s, "pv_strings", json!({ "t_min": -30 })).unwrap();
    let problems: Vec<&str> = list["problems"].as_array().unwrap().iter().map(|p| p.as_str().unwrap()).collect();
    assert!(problems.iter().any(|p| p.contains("short-circuit current of 28.4 A")), "{problems:?}");
    assert!(run(&mut s, "pv_strings", json!({ "t_min": 30, "t_mean": 25 })).is_err());
    assert!(run(&mut s, "set_pv_string", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "string": "S9" })).unwrap_err().contains("S9"));
    let a = run(&mut s, "set_pv_string", json!({ "building": "+H1", "roof": "R1", "array": "PV1", "string": "S1", "remove": true })).unwrap();
    assert_eq!(a["strings"], json!([]));
}

#[test]
fn the_plot_boundary_and_the_reference_point() {
    let mut s = demo_session();
    run(&mut s, "add_building", json!({ "name": "+B1", "x": 100000, "z": 0 })).unwrap();
    run(&mut s, "add_room", json!({ "name": "+R1", "building": "+B1" })).unwrap();
    let corners = json!([[90000, -10000], [130000, -10000], [130000, 20000], [90000, 20000]]);
    let p = run(&mut s, "set_plot", json!({ "outline": corners, "name": "Flur 3, 112/4" })).unwrap();
    assert_eq!((p["area"].as_f64(), p["perimeter"].as_f64(), p["name"].as_str()), (Some(1200.0), Some(140.0), Some("Flur 3, 112/4")));
    assert!(run(&mut s, "set_plot", json!({ "outline": [[0, 0], [1000, 1000]] })).is_err(), "two corners");
    // The demo's cabinets stand outside the plot; a room moved out of it is an error.
    let outside = |s: &mut Session| run(s, "plot", json!({})).unwrap()["outside"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert!(!outside(&mut s).contains(&"+R1".to_string()));
    run(&mut s, "set_building", json!({ "building": "+B1", "x": 128000 })).unwrap();
    assert!(outside(&mut s).contains(&"+R1".to_string()));
    let list = run(&mut s, "list_buildings", json!({})).unwrap();
    assert!(list["problems"].as_array().unwrap().iter().any(|p| p.as_str().unwrap().contains("+R1")), "{}", list["problems"]);
    // The reference point: coordinates of the building, from geographic ones back.
    let p = run(&mut s, "set_reference_point", json!({ "x": 100000, "z": 0, "easting": 32512000.5, "northing": 5812000.25, "height": 50, "crs": "EPSG:25832" })).unwrap();
    assert_eq!(p["reference"]["crs"], "EPSG:25832");
    let list = run(&mut s, "list_buildings", json!({})).unwrap();
    assert_eq!(list["buildings"][0]["geo"], json!({ "easting": 32512028.5, "northing": 5812000.25, "height": 50.0 }));
    let l = run(&mut s, "locate", json!({ "easting": 32512010.5, "northing": 5812005.25 })).unwrap();
    assert_eq!((l["x"].as_f64(), l["z"].as_f64(), l["on_plot"].as_bool()), (Some(110000.0), Some(-5000.0), Some(true)));
    assert!(run(&mut s, "locate", json!({ "easting": 1 })).is_err());
    let m = run(&mut s, "measure", json!({ "points": [[0, 0], [0, -10000]] })).unwrap();
    assert_eq!((m["distance"].as_f64(), m["north"].as_f64(), m["bearing"].as_f64()), (Some(10000.0), Some(10.0), Some(0.0)));
    run(&mut s, "set_reference_point", json!({ "remove": true })).unwrap();
    let p = run(&mut s, "set_plot", json!({ "remove": true })).unwrap();
    assert_eq!(p["outline"], json!([]));
}
