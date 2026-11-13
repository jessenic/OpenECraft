use super::*;
use crate::testing::*;

#[test]
fn frames_are_listed_created_and_used() {
    let mut s = new_session();
    let v = run(&mut s, "list_frames", json!({})).unwrap();
    assert_eq!(v["default"], "A3 landscape");
    let a4 = v["frames"].as_array().unwrap().iter().find(|f| f["name"] == "A4 portrait").unwrap().clone();
    assert_eq!((a4["width"].as_f64(), a4["height"].as_f64(), a4["builtin"].as_bool()), (Some(210.0), Some(297.0), Some(true)));

    assert!(run(&mut s, "set_frame", json!({ "name": "A4 portrait", "columns": 4 })).unwrap_err().contains("built-in"));
    let f = run(&mut s, "set_frame", json!({ "name": "Wide", "based_on": "A4 landscape", "columns": 12, "column_labels": "letters" })).unwrap();
    assert_eq!((f["paper"].as_str(), f["columns"].as_u64(), f["column_labels"].as_str()), (Some("A4"), Some(12), Some("letters")));
    let f = run(&mut s, "set_frame", json!({ "name": "Wide", "width": 800.0 })).unwrap();
    assert_eq!((f["paper"].as_str(), f["width"].as_f64(), f["height"].as_f64()), (Some("Custom"), Some(800.0), Some(210.0)));
    assert!(run(&mut s, "set_frame", json!({ "name": "Wide", "paper": "B5" })).unwrap_err().contains("Unknown paper"));

    let pg = run(&mut s, "add_page", json!({ "kind": "overview", "frame": "Wide" })).unwrap();
    assert_eq!((pg["kind"].as_str(), pg["frame"].as_str()), (Some("overview"), Some("Wide")));
    assert!(run(&mut s, "add_page", json!({ "kind": "schematic", "frame": "B5" })).unwrap_err().contains("No plot frame"));
    assert!(run(&mut s, "delete_frame", json!({ "name": "Wide" })).unwrap_err().contains("in use"));
    run(&mut s, "set_page", json!({ "page": "2", "frame": "" })).unwrap();
    run(&mut s, "delete_frame", json!({ "name": "Wide" })).unwrap();

    let d = run(&mut s, "set_default_frame", json!({ "name": "A4 portrait" })).unwrap();
    assert_eq!(d["width"], 210.0);
    let svg = run(&mut s, "export", json!({ "format": "svg", "page": "1" })).unwrap();
    assert!(svg.to_string().contains("210mm"));
}

#[test]
fn pages_are_made_from_templates() {
    let mut s = new_session();
    let list = run(&mut s, "list_page_templates", json!({})).unwrap();
    assert!(list.as_array().unwrap().iter().any(|t| t["name"] == "Graphics A4 portrait" && t["kind"] == "graphic"));
    let pg = run(&mut s, "add_page", json!({ "template": "Graphics A4 portrait" })).unwrap();
    assert_eq!((pg["kind"].as_str(), pg["frame"].as_str()), (Some("graphic"), Some("A4 portrait")));
    // Without kind or template the page is a schematic page.
    assert_eq!(run(&mut s, "add_page", json!({})).unwrap()["kind"], "schematic");
    assert!(run(&mut s, "add_page", json!({ "kind": "bogus" })).unwrap_err().starts_with("Unknown page type: bogus"));
    assert!(run(&mut s, "add_page", json!({ "template": "Nothing" })).unwrap_err().contains("No page template"));

    run(&mut s, "set_page", json!({ "page": "1", "function": "A1", "frame": "A4 landscape" })).unwrap();
    run(&mut s,"set_properties",json!({"object":"page:1","values":{"page.drawing_number":"DOC-7"}})).unwrap();
    let t = run(&mut s, "save_page_template", json!({ "page": "1", "name": "Motor" })).unwrap();
    assert_eq!(t["name"], "Motor");
    let list=run(&mut s,"list_page_templates",json!({})).unwrap();
    assert_eq!(list.as_array().unwrap().iter().find(|t|t["name"]=="Motor").unwrap()["values"]["page.drawing_number"],"DOC-7");
    let pg = run(&mut s, "add_page", json!({ "template": "Motor", "after": "1" })).unwrap();
    assert_eq!((pg["function"].as_str(), pg["frame"].as_str()), (Some("A1"), Some("A4 landscape")));
    assert_eq!(s.project().unwrap().pages.iter().find(|p|p.name==pg["name"].as_str().unwrap()).unwrap().values["page.drawing_number"],"DOC-7");
    run(&mut s, "delete_page_template", json!({ "name": "Motor" })).unwrap();
    assert!(run(&mut s, "delete_page_template", json!({ "name": "Motor" })).is_err());
}

#[test]
fn files_are_embedded_and_written_again() {
    let mut s = new_session();
    let dir = temp_dir("document-page");
    let file = dir.join("manual.pdf");
    std::fs::write(&file, b"%PDF-1.4 manual").unwrap();
    let pg = run(&mut s, "add_document_page", json!({ "path": file.to_str().unwrap() })).unwrap();
    assert_eq!((pg["kind"].as_str(), pg["document"].as_str(), pg["bytes"].as_u64()), (Some("document"), Some("manual.pdf"), Some(15)));
    let out = dir.join("copy.pdf");
    run(&mut s, "export_document", json!({ "page": "2", "path": out.to_str().unwrap() })).unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), b"%PDF-1.4 manual");
    assert!(run(&mut s, "export_document", json!({ "page": "1", "path": out.to_str().unwrap() })).unwrap_err().contains("no embedded document"));
}

#[test]
fn logos_are_added_to_title_blocks_and_the_project() {
    let mut s = new_session();
    let dir = temp_dir("logos");
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&120u32.to_be_bytes());
    png.extend_from_slice(&40u32.to_be_bytes());
    let file = dir.join("company.png");
    std::fs::write(&file, &png).unwrap();
    let text = dir.join("notes.txt");
    std::fs::write(&text, b"no image").unwrap();
    let path = file.to_str().unwrap();

    run(&mut s, "set_title_block", json!({ "name": "Company", "based_on": "Standard" })).unwrap();
    let place = json!({ "title_block": "Company", "x": 2.0, "y": 2.0, "width": 70.0, "height": 12.0 });
    let with = |k: &str, v: serde_json::Value| { let mut a = place.clone(); a[k] = v; a };
    let added = run(&mut s, "add_title_block_image", with("path", json!(path))).unwrap();
    assert_eq!(added["file"], "company.png");
    let added = run(&mut s, "add_title_block_image", place.clone()).unwrap();
    assert!(added["file"].is_null(), "the project logo");
    assert!(run(&mut s, "add_title_block_image", with("path", json!(text.to_str().unwrap()))).unwrap_err().contains("not a PNG or JPEG"));
    assert!(run(&mut s, "add_title_block_image", with("width", json!(0.0))).unwrap_err().contains("greater than 0"));
    assert!(run(&mut s, "add_title_block_image", with("title_block", json!("Standard"))).unwrap_err().contains("No title block of the project"));

    let info = run(&mut s, "set_project_info", json!({ "logo": path })).unwrap();
    assert_eq!((info["logo"]["name"].as_str(), info["logo"]["px"][0].as_u64()), (Some("company.png"), Some(120)));
    run(&mut s, "set_default_title_block", json!({ "name": "Company" })).unwrap();
    let svg = run(&mut s, "export", json!({ "format": "svg", "page": "1" })).unwrap().to_string();
    assert_eq!(svg.matches("data:image/png;base64,").count(), 2);
    assert!(run(&mut s, "set_project_info", json!({ "logo": "" })).unwrap()["logo"].is_null());
}
