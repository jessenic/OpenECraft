use super::*;

fn response(body: &str) -> ehttp::Result<ehttp::Response> {
    Ok(ehttp::Response { url: String::new(), ok: true, status: 200, status_text: "OK".into(), headers: ehttp::Headers::default(), bytes: body.as_bytes().to_vec() })
}

#[test]
fn both_kinds_answer_in_the_parts_server_types() {
    let parts = Connection::new(PortalKind::OpenECraft, "http://host:8090", "");
    assert_eq!(parts.kind(), PortalKind::OpenECraft);
    let info = parts.info().run(|req| {
        assert_eq!(req.url, "http://host:8090/api/v1/info");
        response(r#"{"name":"srv","parts":3}"#)
    });
    assert_eq!(info.unwrap().parts, 3);

    let eplan = Connection::new(PortalKind::Eplan, "https://portal.test/api", "pat");
    assert_eq!(eplan.kind(), PortalKind::Eplan);
    let info = eplan.info().run(|req| {
        if req.url.contains("/parts?") {
            assert!(req.url.starts_with("https://portal.test/api/parts?"));
            response(r#"{"data":[{"id":"1"}],"meta":{"total":7}}"#)
        } else {
            assert_eq!(req.url, "https://portal.test/api/token/fileprovider");
            response(r#"{"data":{"attributes":{"token":"provider-fixture"}}}"#)
        }
    });
    assert_eq!(info.unwrap().parts, 7);
    let found = eplan.search(&PartQuery::default()).run(|_| response(r#"{"data":[{"type":"parts","id":"1","attributes":{"part_number":"A.1"}}]}"#));
    assert_eq!(found.unwrap().parts[0].number, "A.1");
    let part = eplan.part("A.1").run(|_| response(r#"{"data":{"type":"parts","id":"1","attributes":{"part_number":"A.1"}}}"#));
    assert_eq!(part.unwrap().unwrap().number, "A.1");
}

#[test]
fn the_portal_only_reads() {
    assert!(PortalKind::Eplan.read_only() && !PortalKind::OpenECraft.read_only());
    assert_eq!(PortalKind::Eplan.default_url(), eplan::EPLAN_URL);
    assert!(Connection::new(PortalKind::Eplan, eplan::EPLAN_URL, "").parts_server().is_err());
    assert!(Connection::new(PortalKind::OpenECraft, "http://host", "").parts_server().is_ok());
    // Files of the portal need the token, pictures can be opened in the browser.
    let eplan = Connection::new(PortalKind::Eplan, eplan::EPLAN_URL, "pat");
    assert_eq!(eplan.file_link("download/dxf_data/part/1"), None);
    assert_eq!(eplan.file_link("https://cdn.test/a.jpg").as_deref(), Some("https://cdn.test/a.jpg"));
    let parts = Connection::new(PortalKind::OpenECraft, "http://host", "");
    assert_eq!(parts.file_link("f1").as_deref(), Some("http://host/api/v1/files/f1"));
}
