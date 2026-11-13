use super::*;
use crate::document::Document;
use crate::model::{Page, PageKind};
use crate::sample::demo_project;

#[test]
fn crc_and_dates() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    assert_eq!(dos_date("2026-10-24"), (46 << 9) | (10 << 5) | 24);
    assert_eq!(dos_date("yesterday"), (1 << 5) | 1);
}

#[test]
fn zip_files_round_trip() {
    let mut z = ZipWriter::new("2026-01-02");
    z.add("a.txt", b"hello hello hello hello hello hello");
    z.add("b.bin", &[1, 2, 3]);
    z.add("dir/ü.txt", b"");
    let bytes = z.finish();
    assert!(is_archive(&bytes));
    let files = read_zip(&bytes).unwrap();
    assert_eq!(files.len(), 3);
    assert_eq!(files[0], ("a.txt".to_string(), b"hello hello hello hello hello hello".to_vec()));
    assert_eq!(files[1].1, vec![1, 2, 3]);
    assert_eq!(files[2].0, "dir/ü.txt");

    let mut broken = bytes.clone();
    broken[40] ^= 0xff;
    assert!(read_zip(&broken).is_err());
    assert!(read_zip(b"PK").is_err());
}

#[test]
fn archive_contains_project_files_and_manifest() {
    let mut prj = demo_project();
    let id = prj.new_id();
    let mut doc = Page::new(id, "8", "Data sheet", PageKind::Document);
    doc.document = Some(Document::new("sheet.pdf", b"%PDF-1.4 test"));
    prj.pages.push(doc);
    prj.baselines.push(Default::default());
    let opts = ArchiveOptions { comment: "Handover".into(), date: "2026-10-24".into(), pdf: true, compact: true };
    let bytes = archive(&prj, &opts);
    let m = manifest(&bytes).unwrap();
    assert_eq!((m.name.as_str(), m.archived.as_str(), m.comment.as_str(), m.format), ("Demo conveyor", "2026-10-24", "Handover", FORMAT));
    assert_eq!(m.version, ARCHIVE_VERSION);
    // The files of a split project, its index first, and the PDF.
    assert_eq!((m.files.first().map(String::as_str), m.files.last().map(String::as_str)), (Some(storage::INDEX), Some("project.pdf")));
    let embedded: Vec<&String> = m.files.iter().filter(|f| f.starts_with("files/")).collect();
    assert_eq!(embedded.len(), 1);
    assert!(embedded[0].ends_with(".pdf"));
    assert!(m.files.contains(&format!("pages/{}.json", prj.pages[0].id)));
    let files = read_zip(&bytes).unwrap();
    assert_eq!(files.len(), m.files.len() + 1);
    assert_eq!(files.iter().find(|f| &f.0 == embedded[0]).unwrap().1, b"%PDF-1.4 test");
    assert!(!files.iter().any(|f| f.0 == "project.oec"));

    let (back, m2, conv) = restore(&bytes).unwrap();
    assert_eq!(m2, m);
    assert!(conv.message().is_none());
    assert!(back.baselines.is_empty());
    assert_eq!(back.pages, prj.pages);
    let (again, _) = read_project(&bytes).unwrap();
    assert_eq!(again.pages.len(), prj.pages.len());
    // Without the compact option the archive holds the whole project.
    let (whole, _, _) = restore(&archive(&prj, &ArchiveOptions::default())).unwrap();
    assert!(whole == prj);
}

#[test]
fn archives_of_version_1_are_still_read() {
    let prj = demo_project();
    let mut zip = ZipWriter::new("2026-01-02");
    zip.add("project.oec", prj.to_json().as_bytes());
    zip.add("files/8 sheet.pdf", b"%PDF-1.4 test");
    zip.add("manifest.json", br#"{ "generator": "OpenECraft 0.1.1", "format": 2, "name": "Demo conveyor", "comment": "Old", "files": ["project.oec"] }"#);
    let bytes = zip.finish();
    let (back, m, conv) = restore(&bytes).unwrap();
    assert!(back == prj);
    assert_eq!((m.version, m.comment.as_str()), (0, "Old"));
    assert!(conv.message().is_none());

    let mut zip = ZipWriter::new("2026-01-02");
    zip.add("manifest.json", br#"{ "version": 99 }"#);
    zip.add("project.oecp", b"{}");
    assert!(restore(&zip.finish()).unwrap_err().contains("99"));
    let mut zip = ZipWriter::new("2026-01-02");
    zip.add("readme.txt", b"x");
    assert!(restore(&zip.finish()).is_err());
}

#[test]
fn the_same_embedded_file_is_archived_once() {
    let mut prj = demo_project();
    for _ in 0..3 {
        let id = prj.new_id();
        let mut doc = Page::new(id, "8", "Data sheet", PageKind::Document);
        doc.document = Some(Document::new("sheet.pdf", b"%PDF-1.4 test"));
        prj.pages.push(doc);
    }
    let bytes = archive(&prj, &ArchiveOptions::default());
    let m = manifest(&bytes).unwrap();
    assert_eq!(m.files.iter().filter(|f| f.starts_with("files/")).count(), 1);
    assert!(restore(&bytes).unwrap().0 == prj);
}

#[test]
fn old_files_are_converted() {
    let old = r#"{ "info": { "name": "Old" },
        "devices": { "-K1": { "part": "CT-9A-24DC" } },
        "pages": [ { "id": 1, "name": "1", "kind": "Schematic",
            "symbols": [ { "id": 2, "sym": "coil", "tag": "-K1", "props": [ { "kind": "Description" } ] } ] } ],
        "next_id": 3 }"#;
    let (prj, conv) = convert(old).unwrap();
    assert_eq!(conv.from, 0);
    assert!(conv.converted() && !conv.newer());
    assert_eq!(conv.steps.len(), 3);
    assert!(conv.message().unwrap().contains("1"));
    assert_eq!(prj.format, FORMAT);
    assert_eq!(prj.devices["-K1"].parts.len(), 1);
    assert!(prj.to_json().contains(&format!("\"format\": {FORMAT}")));

    let (_, current) = read_project(prj.to_json().as_bytes()).unwrap();
    assert_eq!(current, Conversion { from: FORMAT, steps: Vec::new() });
    let newer = prj.to_json().replace(&format!("\"format\": {FORMAT}"), "\"format\": 99");
    let (_, conv) = convert(&newer).unwrap();
    assert!(conv.newer());
    assert!(conv.message().is_some());
    assert!(convert("{ nope").is_err());
}

#[test]
fn report_names_replace_the_variant_names_of_format_1() {
    use crate::model::{PageKind, ReportKind};
    let old = r#"{ "format": 1, "info": { "name": "Old" },
        "pages": [ { "id": 1, "name": "5", "kind": { "Report": "DeviceList" } } ],
        "report_blocks": [ { "name": "b", "kind": "TerminalDiagram" } ],
        "forms": { "reports": { "BillOfMaterials": { "title": "BOM" } } },
        "next_id": 2 }"#;
    let (prj, conv) = convert(old).unwrap();
    assert_eq!(conv.from, 1);
    assert!(conv.steps.iter().any(|s| s.starts_with("3 report references")), "{:?}", conv.steps);
    assert_eq!(prj.pages[0].kind, PageKind::Report(ReportKind::DeviceList.into()));
    assert_eq!(prj.report_blocks[0].kind.as_str(), "terminal_diagram");
    assert_eq!(prj.forms.reports.keys().map(|k| k.as_str()).collect::<Vec<_>>(), ["bill_of_materials"]);
    let json = prj.to_json();
    assert!(json.contains("\"Report\": \"device_list\"") && json.contains("\"bill_of_materials\""), "{json}");
    // Reports of plugins keep their names.
    let (again, _) = convert(&json.replace("device_list", "my_plugin_report")).unwrap();
    assert_eq!(again.pages[0].kind, PageKind::Report(crate::model::ReportId::new("my_plugin_report")));
}

#[test]
fn packed_projects_are_small_and_read_back() {
    let prj = crate::sample::demo_project();
    let packed = pack(&prj);
    assert!(packed.len() * 4 < prj.to_json().len(), "{} vs {}", packed.len(), prj.to_json().len());
    assert_eq!(unpack(&packed).unwrap().pages.len(), prj.pages.len());
    assert!(unpack("{}").is_err());
    assert!(unpack("oec-deflate-base64:!!").is_err());
}

#[test]
fn zip_bombs_are_refused() {
    let mut z = ZipWriter::new("2026-01-02");
    z.add("zeros.bin", &vec![0u8; 1 << 20]);
    z.add("small.txt", b"x");
    let bytes = z.finish();
    assert!(bytes.len() < 1 << 16, "deflated");
    assert_eq!(crate::zip::read_zip_limited(&bytes, (1 << 20) + 1).unwrap().len(), 2);
    let err = crate::zip::read_zip_limited(&bytes, 1 << 20).unwrap_err();
    assert!(err.contains("more than"), "{err}");
}
