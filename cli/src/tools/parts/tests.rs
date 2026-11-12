use super::*;
use crate::parts_server::Server;
use openecraft_client::{ehttp, Endpoint, PortalKind};
use crate::testing::{part, run, session_with, temp_dir, FakeServer, URL};
use openecraft_parts::to_csv;

fn server() -> FakeServer {
    let server = FakeServer::default();
    server.parts.borrow_mut().extend([part("ACME.K1", "Contactors"), part("ACME.F1", "Fuses"), part("ACME.K2", "Contactors")]);
    server
}

#[test]
fn search_and_get() {
    let mut s = session_with(&server());
    assert_eq!(run(&mut s, "parts_connect", json!({})).unwrap()["info"]["parts"], 3);
    let hits = run(&mut s, "parts_search", json!({ "group": "Contactors", "limit": 1 })).unwrap();
    assert_eq!(hits["total"], 2);
    assert_eq!(hits["parts"], json!([{ "number": "ACME.K1", "manufacturer": "ACME", "type_number": "", "description": "", "group": "Contactors" }]));
    let full = run(&mut s, "parts_search", json!({ "query": "F1", "full": true })).unwrap();
    assert!(full["parts"][0].get("price").is_some());
    assert_eq!(run(&mut s, "parts_get", json!({ "number": "ACME.F1" })).unwrap()["group"], "Fuses");
    assert_eq!(run(&mut s, "parts_get", json!({ "number": "NOPE" })).unwrap_err(), "Unknown part: NOPE");
}

#[test]
fn save_rename_and_delete() {
    let srv = server();
    let mut s = session_with(&srv);
    run(&mut s, "parts_save", json!({ "part": { "number": "ACME.X1", "description": "New" } })).unwrap();
    assert!(srv.parts.borrow().iter().any(|p| p.number == "ACME.X1" && p.description == "New"));
    assert!(run(&mut s, "parts_save", json!({ "part": { "description": "no number" } })).is_err());
    assert!(run(&mut s, "parts_save", json!({ "part": { "number": "A", "price": "cheap" } })).unwrap_err().starts_with("Not a valid part"));

    assert_eq!(run(&mut s, "parts_rename", json!({ "number": "ACME.X1", "new_number": "ACME.K1" })).unwrap_err(), "Part number already exists");
    run(&mut s, "parts_rename", json!({ "number": "ACME.X1", "new_number": "ACME.X2" })).unwrap();
    let numbers: Vec<String> = srv.parts.borrow().iter().map(|p| p.number.clone()).collect();
    assert!(numbers.contains(&"ACME.X2".to_string()) && !numbers.contains(&"ACME.X1".to_string()));

    run(&mut s, "parts_delete", json!({ "number": "ACME.X2" })).unwrap();
    assert!(run(&mut s, "parts_delete", json!({ "number": "ACME.X2" })).is_err());
}

#[test]
fn changes_need_the_token() {
    let srv = server();
    let mut s = session_with(&srv);
    s.server.endpoint.token = String::new();
    assert_eq!(run(&mut s, "parts_delete", json!({ "number": "ACME.K1" })).unwrap_err(), "invalid token");
    run(&mut s, "parts_connect", json!({ "token": crate::testing::TOKEN })).unwrap();
    run(&mut s, "parts_delete", json!({ "number": "ACME.K1" })).unwrap();
}

#[test]
fn unreachable_server() {
    let mut s = Session::new(Server { endpoint: Endpoint { url: "http://elsewhere".into(), token: String::new() }, kind: PortalKind::OpenECraft, credentials: Box::new(crate::testing::NoCredentials), transport: Box::new(server()) });
    let err = run(&mut s, "parts_search", json!({})).unwrap_err();
    assert!(err.starts_with("connection failed"), "{err}");
    run(&mut s, "parts_connect", json!({ "url": URL })).unwrap();
}

#[test]
fn import_and_export_files() {
    let dir = temp_dir("parts-files");
    let srv = server();
    let mut s = session_with(&srv);
    let file = dir.join("new.csv");
    std::fs::write(&file, to_csv(&[part("ACME.N1", "New"), part("ACME.N2", "New")])).unwrap();
    assert_eq!(run(&mut s, "parts_import", json!({ "path": file.to_str().unwrap() })).unwrap()["imported"], 2);
    assert_eq!(srv.parts.borrow().len(), 5);

    let csv = run(&mut s, "parts_export", json!({})).unwrap();
    assert!(csv.as_str().unwrap().contains("ACME.N2"));
    let out = dir.join("all.csv");
    run(&mut s, "parts_export", json!({ "path": out.to_str().unwrap() })).unwrap();
    assert_eq!(openecraft_parts::from_csv(&std::fs::read_to_string(out).unwrap()).unwrap().len(), 5);
}

#[test]
fn edz_files_are_imported_with_their_pictures_and_macros() {
    let dir = temp_dir("parts-edz");
    let srv = server();
    let mut s = session_with(&srv);
    let part = r#"<partsmanagement><part P_ARTICLE_PARTNR="SIE.LOGO" P_ARTICLE_DESCR1="en_US@Logic module;" /></partsmanagement>"#;
    let manifest = r#"<manifest><packages><package type="part" key="SIE.LOGO"><items>
        <item type="partxml" locator="SIE.LOGO.part.xml"/><item type="picture" locator="Siemens\logo.png"/><item type="macro" locator="Siemens\logo.ema"/>
        </items></package></packages></manifest>"#;
    let mut w = sevenz_rust2::ArchiveWriter::new(std::io::Cursor::new(Vec::new())).unwrap();
    for (name, data) in [("manifest.xml", manifest.as_bytes()), ("items/partxml/SIE.LOGO.part.xml", part.as_bytes()), ("items/picture/Siemens/logo.png", b"PNG"), ("items/macro/Siemens/logo.ema", b"<EplanPxfRoot/>")] {
        w.push_archive_entry(sevenz_rust2::ArchiveEntry::new_file(name), Some(data)).unwrap();
    }
    let file = dir.join("logo.edz");
    std::fs::write(&file, w.finish().unwrap().into_inner()).unwrap();
    let r = run(&mut s, "parts_import", json!({ "path": file.to_str().unwrap() })).unwrap();
    assert_eq!(r["imported"], 1);
    assert_eq!(r["attached"], json!([{ "number": "SIE.LOGO", "file": "logo.png" }, { "number": "SIE.LOGO", "file": "logo.ema" }]));
    let parts = srv.parts.borrow();
    let logo = parts.iter().find(|p| p.number == "SIE.LOGO").unwrap();
    assert_eq!(logo.description, "Logic module");
    let files: Vec<(&str, AttachmentKind)> = logo.attachments.iter().map(|a| (a.name.as_str(), a.kind)).collect();
    assert_eq!(files, [("logo.png", AttachmentKind::Image), ("logo.ema", AttachmentKind::Other)]);
}

#[test]
fn search_with_portal_filters_and_facets() {
    let srv = server();
    {
        let mut parts = srv.parts.borrow_mut();
        parts[0].voltage = "24 V DC".into();
        parts[0].catalog = "acme_2026".into();
        parts[2].lifecycle = openecraft_parts::portal::Lifecycle::Discontinued;
        parts[2].successor = "ACME.K1".into();
    }
    let mut s = session_with(&srv);
    let hits = run(&mut s, "parts_search", json!({ "attributes": { "Voltage": "24 V DC" } })).unwrap();
    assert_eq!(hits["total"], 1);
    assert_eq!(hits["parts"][0]["catalog"], "acme_2026");
    let hits = run(&mut s, "parts_search", json!({ "lifecycle": "discontinued" })).unwrap();
    assert_eq!(hits["parts"][0]["successor"], "ACME.K1");
    let hits = run(&mut s, "parts_search", json!({ "sort": "Number", "descending": true, "facets": true })).unwrap();
    assert_eq!(hits["parts"][0]["number"], "ACME.K2");
    assert_eq!(hits["facets"]["group"], json!([["Contactors", 2], ["Fuses", 1]]));
    assert_eq!(hits["attributes"]["Voltage"], json!([["24 V DC", 1]]));
    let err = run(&mut s, "parts_search", json!({ "lifecycle": "old" })).unwrap_err();
    assert!(err.contains("Active, PhaseOut, Discontinued"), "{err}");
}

#[test]
fn sign_in_favorites_and_users() {
    let srv = server();
    let mut s = session_with(&srv);
    s.server.endpoint.token = String::new();
    assert_eq!(run(&mut s, "parts_connect", json!({ "user": "ann", "password": "no" })).unwrap_err(), "wrong user name or password");
    run(&mut s, "parts_connect", json!({ "user": "ann", "password": "pw" })).unwrap();
    assert_eq!(s.server.endpoint.token, crate::testing::TOKEN);
    assert_eq!(run(&mut s, "parts_favorites", json!({ "add": "ACME.K1" })).unwrap(), json!(["ACME.K1"]));
    assert_eq!(run(&mut s, "parts_favorites", json!({ "add": "ACME.K2", "remove": "ACME.K1" })).unwrap(), json!(["ACME.K2"]));
    assert_eq!(run(&mut s, "parts_favorites", json!({})).unwrap(), json!(["ACME.K2"]));

    let u = run(&mut s, "parts_user_save", json!({ "name": "max", "role": "manufacturer", "manufacturer": "ACME", "password": "x" })).unwrap();
    assert_eq!(u["role"], "Manufacturer");
    assert!(run(&mut s, "parts_user_save", json!({ "name": "max", "role": "boss" })).is_err());
    assert_eq!(run(&mut s, "parts_users", json!({})).unwrap()[0]["name"], "max");
    run(&mut s, "parts_user_delete", json!({ "name": "max" })).unwrap();
    assert!(srv.users.borrow().is_empty());
}

#[test]
fn roles_and_grants() {
    let srv = server();
    let mut s = session_with(&srv);
    let r = run(&mut s, "parts_role", json!({ "name": "buyer", "rights": "read, edit_parts" })).unwrap();
    assert_eq!(r["policy"]["roles"][0]["rights"], json!(["read", "edit_parts"]));
    assert!(run(&mut s, "parts_role", json!({ "name": "x", "rights": "fly" })).unwrap_err().contains("fly"));
    run(&mut s, "parts_grant", json!({ "subject": "group", "name": "corp:Purchasing", "role": "buyer", "scope": "ACME" })).unwrap();
    run(&mut s, "parts_grant", json!({ "subject": "group", "name": "corp:Purchasing", "role": "buyer", "scope": "acme" })).unwrap();
    run(&mut s, "parts_grant", json!({ "subject": "user", "name": "max", "role": "reader", "deny": true })).unwrap();
    assert!(run(&mut s, "parts_grant", json!({ "subject": "team", "name": "x", "role": "reader" })).is_err());
    assert!(run(&mut s, "parts_grant", json!({ "subject": "user", "name": "x", "role": "nobody" })).unwrap_err().contains("nobody"));
    let grants = srv.policy.borrow().grants.clone();
    assert_eq!(grants.len(), 2, "the same grant once");
    assert!(grants[1].deny && grants[1].scope == "*");
    assert!(run(&mut s, "parts_role", json!({ "name": "buyer", "delete": true })).is_err(), "still granted");
    run(&mut s, "parts_grant", json!({ "subject": "group", "name": "corp:Purchasing", "revoke": true })).unwrap();
    assert!(run(&mut s, "parts_grant", json!({ "subject": "group", "name": "corp:Purchasing", "revoke": true })).is_err());
    run(&mut s, "parts_role", json!({ "name": "buyer", "delete": true })).unwrap();
    assert!(run(&mut s, "parts_role", json!({ "name": "buyer", "delete": true })).is_err());
    run(&mut s, "parts_access", json!({ "anonymous": "none" })).unwrap();
    let shown = run(&mut s, "parts_access", json!({})).unwrap();
    assert_eq!(shown["policy"]["grants"].as_array().unwrap().len(), 1);
    assert!(shown["policy"].get("anonymous").is_none());
}

#[test]
fn catalogs_files_and_history() {
    let dir = temp_dir("parts-portal");
    let srv = server();
    let mut s = session_with(&srv);
    let c = run(&mut s, "parts_catalog_save", json!({ "catalog": { "name": "ACME 2026", "manufacturer": "ACME" } })).unwrap();
    assert_eq!(c["id"], "acme_2026");
    assert!(run(&mut s, "parts_catalog_save", json!({ "catalog": {} })).is_err());
    assert_eq!(run(&mut s, "parts_catalogs", json!({})).unwrap()[0]["name"], "ACME 2026");
    run(&mut s, "parts_catalog_delete", json!({ "id": "acme_2026" })).unwrap();
    assert!(srv.catalogs.borrow().is_empty());

    let sheet = dir.join("k1 sheet.pdf");
    std::fs::write(&sheet, b"%PDF").unwrap();
    let r = run(&mut s, "parts_attach", json!({ "number": "ACME.K1", "path": sheet.to_str().unwrap() })).unwrap();
    assert_eq!(r["attachments"][0]["name"], "k1 sheet.pdf");
    assert_eq!(r["attachments"][0]["kind"], "Datasheet");
    let id = r["attachments"][0]["id"].as_str().unwrap().to_string();
    let out = dir.join("copy.pdf");
    assert_eq!(run(&mut s, "parts_file", json!({ "id": id, "path": out.to_str().unwrap() })).unwrap()["bytes"], 4);
    assert_eq!(std::fs::read(&out).unwrap(), b"%PDF");
    let r = run(&mut s, "parts_attach", json!({ "number": "ACME.K1", "path": sheet.to_str().unwrap(), "kind": "manual" })).unwrap();
    assert_eq!(r["attachments"][1]["kind"], "Manual");
    let r = run(&mut s, "parts_detach", json!({ "number": "ACME.K1", "id": id })).unwrap();
    assert_eq!(r["attachments"].as_array().unwrap().len(), 1);

    assert_eq!(run(&mut s, "parts_history", json!({ "number": "ACME.K1" })).unwrap()[0]["change"], "Created");
    assert_eq!(run(&mut s, "parts_history", json!({ "number": "ACME.K1", "revision": 1 })).unwrap()["number"], "ACME.K1");
    assert!(run(&mut s, "parts_history", json!({ "number": "ACME.K1", "revision": 7 })).is_err());
}

#[test]
fn project_parts_are_checked_for_updates() {
    let srv = server();
    let mut s = session_with(&srv);
    run(&mut s, "new_project", json!({})).unwrap();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    run(&mut s, "assign_part", json!({ "tag": "-K1", "number": "ACME.K2" })).unwrap();
    assert_eq!(run(&mut s, "parts_check_updates", json!({})).unwrap()["updates"], json!([]));
    {
        let mut parts = srv.parts.borrow_mut();
        let k2 = parts.iter_mut().find(|p| p.number == "ACME.K2").unwrap();
        k2.description = "newer".into();
        k2.lifecycle = openecraft_parts::portal::Lifecycle::PhaseOut;
        k2.successor = "ACME.K1".into();
        k2.revision = 5;
    }
    let r = run(&mut s, "parts_check_updates", json!({})).unwrap();
    assert_eq!(r["updates"][0]["kind"], "PhaseOut");
    assert_eq!(r["updates"][0]["successor"], "ACME.K1");
    assert_eq!(r["applied"], 0);
    assert_eq!(run(&mut s, "parts_check_updates", json!({ "apply": true })).unwrap()["applied"], 1);
    let k2 = s.project().unwrap().parts.iter().find(|p| p.number == "ACME.K2").unwrap().clone();
    assert_eq!(k2.description, "newer");
    assert_eq!(run(&mut s, "parts_check_updates", json!({})).unwrap()["updates"], json!([]));
}

#[test]
fn devices_get_parts_from_the_server() {
    let mut s = session_with(&server());
    run(&mut s, "new_project", json!({})).unwrap();
    run(&mut s, "place_symbol", json!({ "page": "1", "symbol": "coil", "x": 100, "y": 100 })).unwrap();
    run(&mut s, "assign_part", json!({ "tag": "-K1", "number": "ACME.K2" })).unwrap();
    assert!(s.project().unwrap().parts.iter().any(|p| p.number == "ACME.K2"));
    assert_eq!(run(&mut s, "assign_part", json!({ "tag": "-K1", "number": "NOPE" })).unwrap_err(), "Unknown part: NOPE");
}

/// Public metadata accepts any credentials; the download gateway checks the PAT.
struct Portal;

impl crate::parts_server::Transport for Portal {
    fn fetch(&self, request: &ehttp::Request) -> ehttp::Result<ehttp::Response> {
        let public = request.url.starts_with("https://dataportal.eplan.com/api/parts");
        let download = request.url == "https://dataportal.eplan.com/api/token/fileprovider";
        let authorized = request.headers.get("Authorization") == Some("Bearer PAT:secret");
        let (status, body) = match (public, download, authorized) {
            (true, _, _) => (200, r#"{"data":[{"type":"parts","id":"9","attributes":{"part_number":"RIT.1","designation":"Enclosure"}}],"meta":{"total":1}}"#),
            (_, true, true) => {
                assert_eq!(request.method, ehttp::Method::GET);
                assert!(request.body.is_empty());
                (200, r#"{"data":{"attributes":{"token":"provider-fixture"}}}"#)
            }
            (_, true, false) => (401, "{}"),
            _ => (404, "{}"),
        };
        Ok(ehttp::Response { url: request.url.clone(), ok: status == 200, status, status_text: String::new(), headers: ehttp::Headers::default(), bytes: body.as_bytes().to_vec() })
    }
}

#[test]
fn eplan_data_portal() {
    let mut s = Session::new(Server { endpoint: Endpoint { url: URL.into(), token: "admin".into() }, kind: PortalKind::OpenECraft, credentials: Box::new(crate::testing::NoCredentials), transport: Box::new(Portal) });
    assert!(run(&mut s, "parts_connect", json!({ "kind": "Eplan" })).is_err(), "no token");
    let v = run(&mut s, "parts_connect", json!({ "kind": "eplan", "token": "secret" })).unwrap();
    assert_eq!((v["kind"].as_str(), v["url"].as_str(), v["info"]["parts"].as_u64()), (Some("Eplan"), Some("https://dataportal.eplan.com/api"), Some(1)));
    let v = run(&mut s, "parts_search", json!({ "query": "enclosure" })).unwrap();
    assert_eq!((v["total"].as_u64(), v["parts"][0]["number"].as_str()), (Some(1), Some("RIT.1")));
    let v = run(&mut s, "parts_get", json!({ "number": "RIT.1" })).unwrap();
    assert_eq!(v["description"].as_str(), Some("Enclosure"));
    assert!(run(&mut s, "parts_catalogs", json!({})).unwrap_err().contains("EPLAN Data Portal"));
}

#[test]
fn eplan_uses_shared_pat_without_caching_it_or_requiring_a_token_argument() {
    struct SharedCredentials(std::sync::Arc<std::sync::Mutex<String>>);
    impl openecraft_client::credentials::CredentialStore for SharedCredentials {
        fn load(&self, url: &str) -> Result<Option<String>, openecraft_client::credentials::CredentialError> {
            assert_eq!(url, "https://dataportal.eplan.com/api");
            let token = self.0.lock().unwrap().clone();
            Ok((!token.is_empty()).then_some(token))
        }
        fn save(&self, _: &str, _: &str) -> Result<bool, openecraft_client::credentials::CredentialError> { panic!("MCP must not save credentials") }
        fn forget(&self, _: &str) -> Result<bool, openecraft_client::credentials::CredentialError> { panic!("MCP must not delete credentials") }
    }
    let saved = std::sync::Arc::new(std::sync::Mutex::new("secret".to_string()));
    let mut s = Session::new(Server { endpoint: Endpoint { url: URL.into(), token: String::new() }, kind: PortalKind::OpenECraft, transport: Box::new(Portal), credentials: Box::new(SharedCredentials(saved.clone())) });
    run(&mut s, "parts_connect", json!({ "kind": "Eplan" })).unwrap();
    assert!(s.server.endpoint.token.is_empty(), "the shared PAT is not cached as an explicit override");
    *saved.lock().unwrap() = "rotated-invalid-pat".into();
    assert!(run(&mut s, "parts_connect", json!({})).is_err());
    *saved.lock().unwrap() = "secret".into();
    run(&mut s, "parts_connect", json!({})).unwrap();
    saved.lock().unwrap().clear();
    assert!(run(&mut s, "parts_connect", json!({})).is_err());
    run(&mut s, "parts_connect", json!({ "token": "secret" })).unwrap();
    assert!(saved.lock().unwrap().is_empty(), "MCP does not persist explicit session credentials");
}
