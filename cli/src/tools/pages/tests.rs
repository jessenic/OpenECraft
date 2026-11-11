use super::*;
use crate::testing::*;

#[test]
fn pages_are_added_renamed_and_deleted() {
    let mut s = new_session();
    assert_eq!(run(&mut s, "add_page", json!({ "kind": "schematic" })).unwrap()["name"], "2");
    let bom = run(&mut s, "add_page", json!({ "kind": "bill-of-materials", "after": "1", "name": "BOM" })).unwrap();
    assert_eq!(bom["kind"], "bill_of_materials");
    let names: Vec<Value> = run(&mut s, "list_pages", json!({})).unwrap().as_array().unwrap().iter().map(|p| p["name"].clone()).collect();
    assert_eq!(names, vec![json!("1"), json!("BOM"), json!("2")]);

    assert!(run(&mut s, "add_page", json!({ "kind": "drawing" })).is_err());
    assert!(run(&mut s, "set_page", json!({ "page": "2", "name": "BOM" })).unwrap_err().contains("already exists"));
    let pg = run(&mut s, "set_page", json!({ "page": "2", "name": "10", "function": "=A1" })).unwrap();
    assert_eq!(pg["function"], "A1");

    run(&mut s, "delete_page", json!({ "page": "BOM" })).unwrap();
    run(&mut s, "delete_page", json!({ "page": "10" })).unwrap();
    assert!(run(&mut s, "delete_page", json!({ "page": "1" })).unwrap_err().contains("last page"));
}

#[test]
fn released_pages_are_not_deleted() {
    let mut s = new_session();
    run(&mut s, "add_page", json!({ "kind": "schematic" })).unwrap();
    s.project_mut().unwrap().pages[1].locked = true;
    assert_eq!(run(&mut s, "delete_page", json!({ "page": "2" })).unwrap_err(), "Released pages cannot be deleted");
    assert!(run(&mut s, "set_page", json!({ "page": "2", "name": "9" })).unwrap_err().contains("locked"));
}

#[test]
fn dxf_drawings_are_imported_as_pages() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let file = temp_dir("dxf-import").join("old drawing.dxf");
    run(&mut s, "export", json!({ "format": "dxf", "page": "2", "path": file.to_str().unwrap() })).unwrap();
    let pages = s.project().unwrap().pages.len();
    let r = run(&mut s, "import_dxf", json!({ "path": file.to_str().unwrap(), "after": "1" })).unwrap();
    assert_eq!((r["name"].as_str(), r["description"].as_str()), (Some(s.project().unwrap().pages[1].name.as_str()), Some("old drawing")));
    assert!(r["lines"].as_u64().unwrap() > 50 && r["texts"].as_u64().unwrap() > 5);
    assert_eq!(s.project().unwrap().pages.len(), pages + 1);
    std::fs::write(&file, "not a drawing").unwrap();
    assert!(run(&mut s, "import_dxf", json!({ "path": file.to_str().unwrap() })).unwrap_err().contains("Not a DXF file"));
}

#[test]
fn dwg_drawings_are_imported_as_pages() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let dir = temp_dir("dwg-import");
    let (dwg, dxf) = (dir.join("old drawing.dwg"), dir.join("old drawing.dxf"));
    run(&mut s, "export", json!({ "format": "dwg", "page": "2", "path": dwg.to_str().unwrap() })).unwrap();
    run(&mut s, "export", json!({ "format": "dxf", "page": "2", "path": dxf.to_str().unwrap() })).unwrap();
    let pages = s.project().unwrap().pages.len();
    let r = run(&mut s, "import_dwg", json!({ "path": dwg.to_str().unwrap(), "after": "1", "description": "Plant" })).unwrap();
    assert_eq!(r["description"], "Plant");
    let from_dxf = run(&mut s, "import_dxf", json!({ "path": dxf.to_str().unwrap() })).unwrap();
    assert_eq!((&r["lines"], &r["texts"]), (&from_dxf["lines"], &from_dxf["texts"]));
    assert_eq!(r["source_bounds"], from_dxf["source_bounds"]);
    assert_eq!(r["page_origin"], from_dxf["page_origin"]);
    assert!(r["source_bounds"]["min"]["x"].is_number());
    assert_eq!(s.project().unwrap().pages.len(), pages + 2);
    std::fs::write(&dwg, b"AC1018 damaged").unwrap();
    assert!(run(&mut s, "import_dwg", json!({ "path": dwg.to_str().unwrap() })).unwrap_err().contains("DWG"));
}

#[test]
fn pdf_comments_are_imported_into_the_review() {
    let mut s = session();
    run(&mut s, "new_project", json!({ "demo": true })).unwrap();
    let file = temp_dir("pdf-comments").join("reviewed.pdf");
    let body = "%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n\
3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 841.89 595.28] /Annots [4 0 R] >>\nendobj\n\
4 0 obj\n<< /Type /Annot /Subtype /Text /Rect [72 500 92 520] /Contents (Check fuse) /T (Anna) >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n";
    std::fs::write(&file, body).unwrap();
    let r = run(&mut s, "import_pdf_comments", json!({ "path": file.to_str().unwrap(), "author": "me" })).unwrap();
    assert_eq!((r["comments"].as_u64(), r["skipped"].as_u64()), (Some(1), Some(0)));
    let again = run(&mut s, "import_pdf_comments", json!({ "path": file.to_str().unwrap() })).unwrap();
    assert_eq!(again["skipped"], 1);
    std::fs::write(&file, "no pdf").unwrap();
    assert!(run(&mut s, "import_pdf_comments", json!({ "path": file.to_str().unwrap() })).unwrap_err().contains("Not a PDF file"));
}

#[test]
fn pages_are_structured_in_numbered_sections() {
    let mut s = new_session();
    assert_eq!(run(&mut s, "add_section", json!({ "description": "Supply" })).unwrap()["number"], "1");
    assert_eq!(run(&mut s, "add_section", json!({ "parent": "1", "description": "Main feed" })).unwrap()["number"], "1.1");
    assert!(run(&mut s, "add_section", json!({ "parent": "1", "number": "1" })).is_err());
    assert_eq!(run(&mut s, "add_page", json!({ "kind": "schematic", "section": "1.1" })).unwrap()["name"], "1.1.1");
    assert_eq!(run(&mut s, "add_page", json!({ "kind": "schematic" })).unwrap()["name"], "1.1.2");
    let pg = run(&mut s, "add_page", json!({ "kind": "schematic", "section": "" })).unwrap();
    assert_eq!(pg["name"], "2");
    let pg = run(&mut s, "set_page", json!({ "page": "2", "section": "1" })).unwrap();
    assert_eq!((pg["name"].as_str(), pg["section"].as_str()), (Some("1.2"), Some("1")));
    let names: Vec<Value> = run(&mut s, "list_pages", json!({})).unwrap().as_array().unwrap().iter().map(|p| p["name"].clone()).collect();
    assert_eq!(names, vec![json!("1"), json!("1.2"), json!("1.1.1"), json!("1.1.2")]);

    run(&mut s, "set_section", json!({ "section": "1", "number": "4", "description": "Power" })).unwrap();
    let list = run(&mut s, "list_sections", json!({})).unwrap();
    assert_eq!(list[1]["number"], "4.1");
    assert_eq!(list[0]["pages"], json!(["4.2"]));
    assert_eq!(run(&mut s, "renumber_pages", json!({ "sections": true })).unwrap()["pages"], json!(["1", "1.1", "1.1.1", "1.1.2"]));
    assert!(run(&mut s, "set_section", json!({ "section": "9" })).unwrap_err().contains("No section"));
    run(&mut s, "delete_section", json!({ "section": "1.1" })).unwrap();
    assert_eq!(run(&mut s, "list_sections", json!({})).unwrap()[0]["pages"], json!(["1.1", "1.2", "1.3"]));
}

#[test]
fn pages_and_sections_move_before_others() {
    let mut s = new_session();
    run(&mut s, "add_section", json!({ "description": "Supply" })).unwrap();
    run(&mut s, "add_section", json!({ "description": "Controls" })).unwrap();
    for section in ["1", "1", "2"] {
        run(&mut s, "add_page", json!({ "kind": "schematic", "section": section })).unwrap();
    }
    let names = |s: &mut _| -> Vec<Value> { run(s, "list_pages", json!({})).unwrap().as_array().unwrap().iter().map(|p| p["name"].clone()).collect() };
    // Page 1.2 before page 1.1: the order changes, the names stay.
    run(&mut s, "set_page", json!({ "page": "1.2", "before": "1.1" })).unwrap();
    assert_eq!(names(&mut s), vec![json!("1"), json!("1.2"), json!("1.1"), json!("2.1")]);
    // Page 2.1 into section 1 before page 1.1: number 1 is taken there.
    assert_eq!(run(&mut s, "set_page", json!({ "page": "2.1", "before": "1.1" })).unwrap()["name"], "1.3");
    assert!(run(&mut s, "set_page", json!({ "page": "1", "section": "2", "before": "1.1" })).unwrap_err().contains("not in that section"));
    // Section 2 before section 1: they swap their numbers, the pages follow.
    run(&mut s, "set_section", json!({ "section": "2", "before": "1" })).unwrap();
    assert_eq!(run(&mut s, "list_sections", json!({})).unwrap()[1]["pages"], json!(["2.2", "2.3", "2.1"]));
}

#[test]
fn adding_a_page_continues_imported_numeric_subpage_names() {
    let mut s=new_session();s.project_mut().unwrap().pages[0].name="20a".into();
    assert_eq!(run(&mut s,"add_page",json!({"kind":"schematic"})).unwrap()["name"],"21");
    assert_eq!(run(&mut s,"list_pages",json!({})).unwrap()[0]["name"],"20a");
}
