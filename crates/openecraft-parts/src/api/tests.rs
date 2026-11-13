use super::*;
use crate::{default_catalog, natural_cmp};

#[test]
fn query_filters_sorts_and_pages() {
    let parts = default_catalog();
    let all = PartQuery::default().apply(&parts);
    assert_eq!(all.0, parts.len());
    assert_eq!(all.1.len(), parts.len());

    let q = PartQuery { q: Some("contactor".into()), ..Default::default() };
    let (total, hits) = q.apply(&parts);
    assert!(total >= 2);
    assert!(hits.iter().all(|p| p.matches_text("contactor")));

    let q = PartQuery { group: Some("relays, CONTACTORS".into()), ..Default::default() };
    let (_, hits) = q.apply(&parts);
    assert!(!hits.is_empty() && hits.iter().all(|p| p.group == "Relays, contactors"));
    // Sorted by part number
    assert!(hits.windows(2).all(|w| natural_cmp(&w[0].number, &w[1].number).is_le()));

    let q = PartQuery { offset: Some(2), limit: Some(3), ..Default::default() };
    let (total, page) = q.apply(&parts);
    assert_eq!(total, parts.len());
    assert_eq!(page.len(), 3);
    assert_eq!(page[0].number, all.1[2].number);
}

#[test]
fn query_string_and_urls() {
    assert_eq!(PartQuery::default().to_query_string(), "");
    let q = PartQuery { q: Some("24 V/DC".into()), limit: Some(10), ..Default::default() };
    assert_eq!(q.to_query_string(), "?q=24%20V%2FDC&limit=10");
    assert_eq!(part_path("GEN.CT-9A-24DC"), "/api/v1/parts/GEN.CT-9A-24DC");
    assert_eq!(part_path("A B/1"), "/api/v1/parts/A%20B%2F1");
    assert_eq!(url("http://host:8090/", "/api/v1/info"), "http://host:8090/api/v1/info");
    assert_eq!(url(" http://host:8090 ", "/api/v1/info"), "http://host:8090/api/v1/info");
}

#[test]
fn error_text_prefers_the_server_message() {
    assert_eq!(error_text(401, "Unauthorized", br#"{"error":"invalid token"}"#), "invalid token");
    assert_eq!(error_text(502, "Bad Gateway", b"<html>"), "502 Bad Gateway");
}

#[test]
fn bearer_tokens() {
    assert_eq!(bearer_token("Bearer abc"), Some("abc"));
    assert_eq!(bearer_token("Bearer  abc "), Some("abc"));
    assert_eq!(bearer_token("Basic abc"), None);
    assert!(token_matches("abc", "abc"));
    assert!(!token_matches("abc", "abd"));
    assert!(!token_matches("abc", "abcd"));
}

#[test]
fn only_web_addresses_are_links() {
    for ok in ["https://example.com/a.pdf", "HTTP://host:8080/x?y", " mailto:anna@example.com "] {
        assert!(is_web_link(ok), "{ok}");
    }
    for bad in ["javascript:alert(1)", "JavaScript://%0aalert(1)", "data:text/html,x", "file:///C:/Windows/System32/calc.exe", r"\\server\share\x.exe", r"C:\docs\a.pdf", "https:evil", "https://", "https://a b", "mailto:", "", "ms-settings:"] {
        assert!(!is_web_link(bad), "{bad}");
    }
}

#[test]
fn exports_carry_the_server_version() {
    let info = ExportInfo::new("OpenECraft parts server", "0.1.2", 42, 1_794_268_800);
    let line = info.csv_comment();
    assert!(line.starts_with("# format=openecraft-parts;format_version=1;"), "{line}");
    let csv = format!("{line}\n{}", crate::to_csv(&[Part { number: "A".into(), ..Default::default() }]));
    assert_eq!(ExportInfo::from_csv(&csv), Some(info.clone()));
    assert_eq!(ExportInfo::from_csv("Part number;Description\nA;x\n"), None);
    assert_eq!(crate::import(&csv).unwrap().len(), 1);
    // The JSON export is read as a parts database.
    let json = serde_json::to_string(&PartsExport { export: info.clone(), parts: vec![Part { number: "B".into(), ..Default::default() }] }).unwrap();
    assert_eq!(crate::import(&json).unwrap()[0].number, "B");
    let back: PartsExport = serde_json::from_str(&json).unwrap();
    assert_eq!(back.export, info);
}

#[test]
fn rights_on_a_part_come_from_its_manufacturer_or_its_catalog() {
    use crate::access::{Entry, SERVER};
    let entry = |scope: &str, deny: bool| Entry { scope: scope.into(), rights: [PartsRight::EditParts].into_iter().collect(), deny };
    let read = Entry { scope: SERVER.into(), rights: [PartsRight::Read].into_iter().collect(), deny: false };
    let info = |entries: Vec<Entry<PartsRight>>| ServerInfo { access: Access { entries }, ..Default::default() };
    let part = Part { number: "A".into(), manufacturer: "Siemens".into(), catalog: "sirius".into(), ..Default::default() };
    let other = Part { catalog: "sentron".into(), manufacturer: "ABB".into(), ..part.clone() };
    let by_catalog = info(vec![read.clone(), entry(&crate::portal::catalog_scope("sirius"), false)]);
    assert!(by_catalog.may_on_part(PartsRight::EditParts, &part));
    assert!(!by_catalog.may_on_part(PartsRight::EditParts, &other));
    assert!(!by_catalog.may_edit("Siemens"));
    let denied = info(vec![read, entry("Siemens", false), entry("catalog:sir*", true)]);
    assert!(!denied.may_on_part(PartsRight::EditParts, &part));
    assert!(denied.may_on_part(PartsRight::EditParts, &Part { catalog: String::new(), ..part }));
}
