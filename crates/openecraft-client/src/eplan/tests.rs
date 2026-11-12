use super::*;
use openecraft_parts::TopGroup;
use serde_json::json;

fn response(status: u16, body: &str) -> ehttp::Result<ehttp::Response> {
    Ok(ehttp::Response {
        url: "https://portal.test/api".into(),
        ok: (200..300).contains(&status),
        status,
        status_text: "Status".into(),
        headers: ehttp::Headers::default(),
        bytes: body.as_bytes().to_vec(),
    })
}

fn endpoint(token: &str) -> EplanEndpoint {
    EplanEndpoint { url: "https://portal.test/api/".into(), token: token.into() }
}

fn with_lang_en<R>(f: impl FnOnce() -> R) -> R {
    openecraft_i18n::with_lang(openecraft_i18n::Lang::En, f)
}

/// A search answer as the portal sends it (shortened): relationships to included
/// manufacturers, catalog entries and pictures.
fn document() -> Value {
    json!({
        "meta": { "facets_counts": [], "page": { "offset": 0, "limit": 2, "total": 2429086 } },
        "data": [{
            "type": "parts",
            "id": "1086107",
            "attributes": {
                "part_number": "VEL.M12P-F17S-10.0PUR_BK",
                "order_number": "M12P-F17S-10.0PUR_BK",
                "type_number": "M12P-F17S-10.0PUR_BK",
                "designation": { "de_DE": "M12-Winkelstecker", "en_US": "M12 pre cast A-code 17-hole elbow" },
                "description": { "en_US": "Rated voltage:30V\nRated current:1.5 A" },
                "lifecycle": { "value": 0, "name": "Active" },
                "update_date": "2026-10-23"
            },
            "relationships": {
                "manufacturer": { "data": { "type": "manufacturers", "id": "317" } },
                "product_top_group": { "data": { "type": "eplan_catalog_entries", "id": "200000000" } },
                "product_group": { "data": { "type": "eplan_catalog_entries", "id": "200050000" } },
                "product_sub_group": { "data": { "type": "eplan_catalog_entries", "id": "200050002" } },
                "eclass_level1": { "data": null },
                "data_types": { "data": [{ "type": "data_types", "id": "commercial_data" }, { "type": "data_types", "id": "dxf_data" }] },
                "picture_file": { "data": { "type": "picturefile", "id": "1435361" } },
                "accessories": { "data": [] }
            }
        }, {
            "type": "parts",
            "id": "1086277",
            "attributes": { "part_number": "VEL.WL-D70-004", "designation": { "en_US": "ball joint end" }, "lifecycle": { "value": 1, "name": "Phase-out" } },
            "relationships": {
                "manufacturer": { "data": { "type": "manufacturers", "id": "317" } },
                "product_top_group": { "data": { "type": "eplan_catalog_entries", "id": "400000000" } },
                "product_group": { "data": { "type": "eplan_catalog_entries", "id": "401260000" } },
                "product_sub_group": { "data": { "type": "eplan_catalog_entries", "id": "401260226" } }
            }
        }],
        "included": [
            { "type": "manufacturers", "id": "317", "attributes": { "short_name": "VEL", "long_name": "VELLE Electrical" } },
            { "type": "eplan_catalog_entries", "id": "200050000", "attributes": { "name": "Plugs" } },
            { "type": "picturefile", "id": "1435361", "relationships": { "preview": { "data": { "type": "preview", "id": "Velledq\\Precastline\\M12-F17S-L.PNG" } } } },
            { "type": "preview", "id": "Velledq\\Precastline\\M12-F17S-L.PNG", "attributes": { "512": "https://cdn.test/512/M12-F17S-L.PNG.png" } }
        ]
    })
}

#[test]
fn requests_of_the_portal() {
    let e = endpoint(" abc123 ");
    let info = e.info();
    assert_eq!(info.url, "https://portal.test/api/parts?page%5Blimit%5D=1");
    assert_eq!(info.headers.get("Authorization"), Some("Bearer PAT:abc123"));
    assert_eq!(endpoint("PAT:abc").info().headers.get("Authorization"), Some("Bearer PAT:abc"));
    assert_eq!(endpoint("eyJhbGciOi.x.y").info().headers.get("Authorization"), Some("Bearer PAT:eyJhbGciOi.x.y"));
    assert_eq!(endpoint(" Bearer abc123 ").info().headers.get("Authorization"), Some("Bearer PAT:abc123"));
    assert_eq!(endpoint(" Bearer PAT:abc123 ").info().headers.get("Authorization"), Some("Bearer PAT:abc123"));
    assert_eq!(endpoint("").info().headers.get("Authorization"), None);

    let include = "include=manufacturer%2Cproduct_top_group%2Cproduct_group%2Cproduct_sub_group%2Cpicture_file.preview";
    let q = PartQuery { q: Some("contactor 24V".into()), manufacturer: Some("SIE".into()), sort: Sort::Manufacturer, descending: true, offset: Some(50), limit: Some(25), ..Default::default() };
    assert_eq!(
        e.search(&q).url,
        format!("https://portal.test/api/parts?search=contactor%2024V&filter%5Bmanufacturer%5D=SIE&sort=-manufacturer.short_name&{include}&page%5Boffset%5D=50&page%5Blimit%5D=25")
    );
    // A text search keeps the order by relevance, a listing is ordered by number.
    assert!(!e.search(&PartQuery { q: Some("x".into()), ..Default::default() }).url.contains("sort="));
    assert!(e.search(&PartQuery::default()).url.contains("sort=part_number"));

    let req = e.parts(&["1086107".into(), "1086277".into()]);
    assert_eq!((req.method.as_str(), req.url.as_str(), req.body.as_slice()), ("POST", format!("https://portal.test/api/parts?{include}").as_str(), b"1086107,1086277".as_slice()));
    assert_eq!(req.headers.get("Content-Type"), Some("text/plain"));
    assert_eq!(e.part("VEL.WL-D70-004").url, format!("https://portal.test/api/parts?search=%22VEL.WL-D70-004%22&{include}&page%5Blimit%5D=20"));
    // Several parts by their ids, without ids the first number.
    assert_eq!(e.lookup(&["A".into(), "B".into()], &["7".into()]).body, b"7".to_vec());
    assert!(e.lookup(&["A".into(), "B".into()], &[]).url.contains("search=%22A%22"));
}

#[test]
fn files_keep_the_token_on_the_portal() {
    let e = endpoint("abc");
    let own = e.file("download/dxf_data/part/4711");
    assert_eq!(own.url, "https://portal.test/api/download/dxf_data/part");
    assert_eq!(own.method, ehttp::Method::POST);
    assert_eq!(own.body, b"4711");
    assert_eq!(own.headers.get("Content-Type"), Some("text/plain"));
    assert!(own.headers.get("Authorization").is_some());
    let other = e.file("https://cdn.test/512.jpg");
    assert_eq!(other.url, "https://cdn.test/512.jpg");
    assert_eq!(other.headers.get("Authorization"), None);
    assert!(e.needs_token("download/dxf_data/part/4711") && !e.needs_token("https://cdn.test/512.jpg"));
    // Addresses that only start like the portal's are other servers.
    for id in ["https://portal.test/api.evil.test/x", "https://portal.test/apix/y", "https://portal.test.evil.test/api/z"] {
        assert_eq!(e.file(id).headers.get("Authorization"), None, "{id}");
    }
    assert!(e.file("https://portal.test/api/download/1").headers.get("Authorization").is_some());
    assert!(e.needs_token("https://portal.test/api/download/dxf_data/part/4711"));
    assert!(!e.needs_token("https://portal.test.evil.test/api/download/dxf_data/part/4711"));
    let bare = EplanEndpoint { url: "https://portal.test".into(), ..endpoint("abc") };
    assert_eq!(bare.file("https://portal.test.evil.test/x").headers.get("Authorization"), None);
}

#[test]
fn product_groups_become_catalog_entries() {
    let id = |top: Option<TopGroup>, group: Option<&str>, sub: Option<&str>| {
        catalog_id(&PartQuery { top_group: top, group: group.map(Into::into), subgroup: sub.map(Into::into), ..Default::default() })
    };
    assert_eq!(id(None, None, None), None);
    assert_eq!(id(Some(TopGroup::Fluid), None, None).as_deref(), Some("300000000"));
    assert_eq!(id(None, Some("Plugs"), None).as_deref(), Some("200050000"));
    assert_eq!(id(Some(TopGroup::Mechanics), Some("Routing accessories"), Some("Cable / tubing clamp")).as_deref(), Some("401260226"));
    assert_eq!(id(Some(TopGroup::Electrical), None, Some("General")).as_deref(), Some("200000000"));
    assert_eq!(id(None, Some("Unknown group"), None), None);
    let q = PartQuery { group: Some("Plugs".into()), ..Default::default() };
    assert!(query_string(&q).contains("filter%5Bcatalog_eplan%5D=200050000"));
}

#[test]
fn parts_of_the_portal_become_parts() {
    let found = with_lang_en(|| parts(&document()));
    assert_eq!(found.len(), 2);
    let p = &found[0];
    assert_eq!((p.number.as_str(), p.portal_id.as_str()), ("VEL.M12P-F17S-10.0PUR_BK", "1086107"));
    assert_eq!((p.type_number.as_str(), p.order_number.as_str(), p.manufacturer.as_str()), ("M12P-F17S-10.0PUR_BK", "M12P-F17S-10.0PUR_BK", "VEL"));
    assert_eq!((p.description.as_str(), p.note.as_str()), ("M12 pre cast A-code 17-hole elbow", "Rated voltage:30V\nRated current:1.5 A"));
    assert_eq!((p.top_group, p.group.as_str(), p.subgroup.as_str()), (TopGroup::Electrical, "Plugs", "General"));
    assert_eq!((p.modified, p.revision, p.lifecycle), (1_792_713_600, 1_792_713_600, Lifecycle::Active));
    let files: Vec<(&str, &str, AttachmentKind)> = p.attachments.iter().map(|a| (a.id.as_str(), a.name.as_str(), a.kind)).collect();
    assert_eq!(
        files,
        vec![
            ("download/commercial_data/part/1086107", "VEL.M12P-F17S-10.0PUR_BK_commercial_data.csv", AttachmentKind::Other),
            ("download/dxf_data/part/1086107", "VEL.M12P-F17S-10.0PUR_BK_dxf.zip", AttachmentKind::Drawing),
            ("https://cdn.test/512/M12-F17S-L.PNG.png", "M12-F17S-L.PNG", AttachmentKind::Image),
        ]
    );
    // Groups from the ids alone, the lifecycle, no data types: commercial data only.
    let q = &found[1];
    assert_eq!((q.top_group, q.group.as_str(), q.subgroup.as_str()), (TopGroup::Mechanics, "Routing accessories", "Cable / tubing clamp"));
    assert_eq!((q.lifecycle, q.attachments.len()), (Lifecycle::PhaseOut, 1));
    // The user's language is preferred.
    let german = openecraft_i18n::with_lang(openecraft_i18n::Lang::De, || parts(&document()));
    assert_eq!(german[0].description, "M12-Winkelstecker");
}

#[test]
fn answers_become_parts_server_answers() {
    let body = document().to_string();
    let info = decode_info(response(200, &body)).unwrap();
    assert_eq!((info.name.as_str(), info.parts, info.writable, info.user.is_none()), (EPLAN_NAME, 2429086, false, true));

    let query = PartQuery { manufacturer: Some("VEL".into()), offset: Some(50), ..Default::default() };
    let r = with_lang_en(|| decode_search(&query, response(200, &body))).unwrap();
    assert_eq!((r.total, r.offset, r.parts.len()), (2429086, 50, 2));
    let names: Vec<&str> = r.facets.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(names, FACETS);
    assert!(r.facet("manufacturer").unwrap().values.iter().any(|v| v.value == "VEL" && v.count == 2));

    // Loading by number keeps only the requested parts.
    let numbers = PartQuery { numbers: vec!["VEL.WL-D70-004".into()], ..Default::default() };
    let r = decode_search(&numbers, response(200, &body)).unwrap();
    assert_eq!((r.total, r.parts[0].number.as_str()), (1, "VEL.WL-D70-004"));

    // Without a count, a next link promises one more part.
    let paged = json!({ "data": [], "links": { "next": "https://portal.test/api/parts?page[offset]=10" } }).to_string();
    assert_eq!(decode_search(&PartQuery { offset: Some(10), ..Default::default() }, response(200, &paged)).unwrap().total, 11);

    // The search for a number also finds longer numbers; the exact one is taken.
    assert_eq!(decode_part("vel.wl-d70-004", response(200, &body)).unwrap().map(|p| p.number), Some("VEL.WL-D70-004".into()));
    assert_eq!(decode_part("VEL.WL-D70", response(200, &body)).unwrap(), None);
    assert_eq!(decode_part("A", response(404, "")).unwrap(), None);
    assert_eq!(decode_file(response(200, "a;b")).unwrap(), b"a;b".to_vec());
}

#[test]
fn errors_are_readable() {
    let denied = with_lang_en(|| decode_info(response(401, "{}")).unwrap_err());
    assert_eq!(denied, "access denied – check the personal access token");
    let errors = json!({ "errors": [{ "status": "400", "code": "invalid-request-body", "title": "Invalid request body.", "detail": "Invalid content format" }, { "title": "Second" }] }).to_string();
    assert_eq!(decode_search(&PartQuery::default(), response(400, &errors)).unwrap_err(), "Invalid content format; Second");
    assert_eq!(decode_file(response(500, "oops")).unwrap_err(), "500 Status");
    assert!(decode_info(Err("refused".into())).unwrap_err().contains("refused"));
    assert!(decode_info(response(200, "<html>")).is_err());
}

#[test]
fn downloads_use_the_portals_post_protocol_for_both_attachment_forms() {
    let e = endpoint("issued-token");
    for kind in ["dxf_data", "commercial_data"] {
        for id in [format!("download/{kind}/part/577485"), format!("https://portal.test/api/download/{kind}/part/577485")] {
            let request = e.file(&id);
            assert_eq!(request.method, ehttp::Method::POST);
            assert_eq!(request.url, format!("https://portal.test/api/download/{kind}/part"));
            assert_eq!(request.body, b"577485");
            assert_eq!(request.headers.get("Authorization"), Some("Bearer PAT:issued-token"));
        }
    }
    let image = e.file("https://cdn.test/download/dxf_data/part/577485");
    assert_eq!(image.method, ehttp::Method::GET);
    assert_eq!(image.headers.get("Authorization"), None);
}

#[test]
fn public_catalog_access_does_not_pass_the_protected_check() {
    for status in [401, 403] {
        let mut requests = 0;
        let result = endpoint("invalid").connection_check().run(|request| {
            requests += 1;
            assert!(request.url.ends_with("/token/fileprovider"));
            response(status, "{}")
        });
        assert!(result.unwrap_err().contains("access denied"));
        assert_eq!(requests, 1);
    }
}

#[test]
fn connection_check_does_not_download_parts_or_require_commercial_data() {
    for catalog in [r#"{"data":[],"meta":{"total":0}}"#,
        r#"{"data":[{"id":"42","attributes":{"data_types":["picture"]}}],"meta":{"total":8}}"#] {
        let mut requests = 0;
        let info = endpoint("valid").connection_check().run(|request| {
            requests += 1;
            assert_eq!(request.headers.get("Authorization"), Some("Bearer PAT:valid"));
            assert_eq!(request.method, ehttp::Method::GET);
            assert!(request.body.is_empty());
            if request.url.ends_with("/token/fileprovider") {
                response(200, r#"{"data":{"attributes":{"token":"provider-fixture"}}}"#)
            } else {
                assert!(request.url.contains("/parts?"));
                response(200, catalog)
            }
        }).unwrap();
        assert_eq!(info.parts, if catalog.contains("42") { 8 } else { 0 });
        assert_eq!(requests, 2);
    }
}

#[test]
fn malformed_service_or_catalog_answers_are_not_reported_as_access_denied() {
    for body in ["{}", r#"{"data":{"attributes":{"token":17}}}"#, r#"{"data":{"attributes":{"token":""}}}"#] {
        let error = endpoint("valid").connection_check().run(|_| response(200, body)).unwrap_err();
        assert!(error.contains("Unexpected Data Portal response"));
        assert!(!error.contains("access denied"));
    }
    for body in ["{}", r#"{"data":null}"#, r#"{"data":{}}"#, r#"{"data":[{}]}"#, r#"{"data":[{"id":17}]}"#, r#"{"data":[{"id":""}]}"#] {
        assert!(decode_info(response(200, body)).unwrap_err().contains("Unexpected Data Portal response"));
    }
}

#[test]
fn absolute_download_urls_use_the_same_origin_rules_as_authorization() {
    for kind in ["dxf_data", "commercial_data"] {
        for host in ["portal.test", "PORTAL.test:443"] {
            let id = format!("https://{host}/api/download/{kind}/part/4711?language=en");
            let request = endpoint("fixture").file(&id);
            assert_eq!(request.method, ehttp::Method::POST);
            assert_eq!(request.url, format!("https://portal.test/api/download/{kind}/part?language=en"));
            assert_eq!(request.body, b"4711");
            assert_eq!(request.headers.get("Authorization"), Some("Bearer PAT:fixture"));
        }
    }
    for url in ["https://portal.test.evil.test/api/download/dxf_data/part/1", "https://portal.test:444/api/download/dxf_data/part/1",
        "https://user@portal.test/api/download/dxf_data/part/1", "https://portal.test/API/download/dxf_data/part/1",
        "https://portal.test/api-other/download/dxf_data/part/1", "https://portal.test/api/../outside"] {
        let request = endpoint("fixture").file(url);
        assert_eq!(request.headers.get("Authorization"), None, "{url}");
        assert!(!endpoint("fixture").needs_token(url));
    }
}
