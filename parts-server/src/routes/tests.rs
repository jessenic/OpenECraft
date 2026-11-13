use super::*;
use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use openecraft_parts::access::{Grant, Subject};
use openecraft_parts::default_catalog;
use openecraft_parts::portal::{ChangeKind, Lifecycle};
use tower::ServiceExt;

const TOKEN: &str = "secret-token";

fn app(token: Option<&str>) -> Router {
    router(state(token))
}

fn state(token: Option<&str>) -> AppState {
    AppState::new(Store::in_memory(default_catalog()), token.map(String::from), AccessFile::in_memory(default_access()))
}

async fn send(app: &Router, method: &str, uri: &str, token: Option<&str>, body: Vec<u8>) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut req = Request::builder().method(method).uri(uri).header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let resp = app.clone().oneshot(req.body(Body::from(body)).unwrap()).await.unwrap();
    let status = resp.status();
    let headers = resp.headers().clone();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, headers, bytes.to_vec())
}

async fn call(app: &Router, method: &str, uri: &str, token: Option<&str>, body: &str) -> (StatusCode, String) {
    let (status, _, bytes) = send(app, method, uri, token, body.as_bytes().to_vec()).await;
    (status, String::from_utf8_lossy(&bytes).to_string())
}

async fn json<T: serde::de::DeserializeOwned>(app: &Router, method: &str, uri: &str, token: Option<&str>, body: &str) -> T {
    let (status, body) = call(app, method, uri, token, body).await;
    assert!(status.is_success(), "{method} {uri}: {status} {body}");
    serde_json::from_str(&body).unwrap()
}

/// Creates a user with the administrator token and signs in. Returns the token.
async fn sign_up(app: &Router, name: &str, role: &str, manufacturer: &str) -> String {
    let body = format!(r#"{{"role": "{role}", "manufacturer": "{manufacturer}", "password": "password-{name}"}}"#);
    let (status, text) = call(app, "PUT", &format!("/api/v1/users/{name}"), Some(TOKEN), &body).await;
    assert_eq!(status, StatusCode::OK, "{text}");
    let session: Session = json(app, "POST", "/api/v1/login", None, &format!(r#"{{"name": "{name}", "password": "password-{name}"}}"#)).await;
    session.token
}

#[tokio::test]
async fn info_list_and_get() {
    let app = app(None);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert_eq!(info.parts, default_catalog().len());
    assert!(!info.writable);
    assert_eq!(info.storage, "json");
    assert!(info.user.is_none());

    let list: PartList = json(&app, "GET", "/api/v1/parts?q=contactor&limit=1", None, "").await;
    assert_eq!(list.parts.len(), 1);
    assert!(list.total >= 2);

    let part: Part = json(&app, "GET", "/api/v1/parts/GEN.CT-9A-24DC", None, "").await;
    assert!(!part.functions.is_empty());
    assert_eq!(part.revision, 1);

    let (status, body) = call(&app, "GET", "/api/v1/parts/NOPE", None, "").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(serde_json::from_str::<ApiError>(&body).unwrap().error.contains("NOPE"));
}

#[tokio::test]
async fn search_with_facets() {
    let app = app(None);
    let r: SearchResult = json(&app, "POST", "/api/v1/search", None, r#"{"q": "contactor", "brief": true, "limit": 2}"#).await;
    assert!(r.total >= 2 && r.parts.len() == 2);
    assert!(r.facet("manufacturer").is_some_and(|f| !f.values.is_empty()));
    assert!(r.parts.iter().all(|p| p.model_3d.is_none()));
    let r: SearchResult = json(&app, "POST", "/api/v1/search", None, r#"{"numbers": ["GEN.CT-9A-24DC"]}"#).await;
    assert_eq!(r.total, 1);
    // GET list with the new filters in the query string
    let list: PartList = json(&app, "GET", "/api/v1/parts?top_group=Fluid&sort=Price&descending=true&brief=true", None, "").await;
    assert!(list.total > 0 && list.parts.iter().all(|p| p.top_group == openecraft_parts::TopGroup::Fluid));
    let q = PartQuery { lifecycle: Some(Lifecycle::Discontinued), ..Default::default() };
    let list: PartList = json(&app, "GET", &format!("/api/v1/parts{}", q.to_query_string()), None, "").await;
    assert_eq!(list.total, 0);
}

#[tokio::test]
async fn writes_need_the_token() {
    let part = r#"{"number": "TEST.1", "description": "Test"}"#;
    // Read-only server
    let ro = app(None);
    assert_eq!(call(&ro, "PUT", "/api/v1/parts/TEST.1", Some(TOKEN), part).await.0, StatusCode::FORBIDDEN);
    // Writable server: missing / wrong / correct token
    let app = app(Some(TOKEN));
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", Some(TOKEN), "").await;
    assert!(info.writable && info.can_edit());
    assert_eq!(info.user.unwrap().name, ADMIN);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/TEST.1", None, part).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/TEST.1", Some("wrong"), part).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/TEST.1", Some(TOKEN), part).await.0, StatusCode::CREATED);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/TEST.1", Some(TOKEN), part).await.0, StatusCode::OK);
    // URL and body must agree, no part is its own successor
    assert_eq!(call(&app, "PUT", "/api/v1/parts/OTHER", Some(TOKEN), part).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/S", Some(TOKEN), r#"{"successor": "S"}"#).await.0, StatusCode::BAD_REQUEST);
    // Part numbers with special characters are URL encoded
    let odd = r#"{"number": "A B/1"}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/parts/A%20B%2F1", Some(TOKEN), odd).await.0, StatusCode::CREATED);
    assert_eq!(call(&app, "GET", "/api/v1/parts/A%20B%2F1", None, "").await.0, StatusCode::OK);
    assert_eq!(call(&app, "GET", "/api/v1/parts/A%20B%2F1/history", None, "").await.0, StatusCode::OK);

    assert_eq!(call(&app, "DELETE", "/api/v1/parts/TEST.1", None, "").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "DELETE", "/api/v1/parts/TEST.1", Some(TOKEN), "").await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "DELETE", "/api/v1/parts/TEST.1", Some(TOKEN), "").await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, "GET", "/api/v1/parts/TEST.1", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn history_of_changes() {
    let app = app(Some(TOKEN));
    call(&app, "PUT", "/api/v1/parts/H.1", Some(TOKEN), r#"{"description": "one"}"#).await;
    call(&app, "PUT", "/api/v1/parts/H.1", Some(TOKEN), r#"{"description": "two"}"#).await;
    let history: Vec<HistoryEntry> = json(&app, "GET", "/api/v1/parts/H.1/history", None, "").await;
    assert_eq!(history.iter().map(|h| h.change).collect::<Vec<_>>(), [ChangeKind::Changed, ChangeKind::Created]);
    assert_eq!(history[0].user, ADMIN);
    let old: Part = json(&app, "GET", &format!("/api/v1/parts/H.1/history/{}", history[1].revision), None, "").await;
    assert_eq!(old.description, "one");
    assert_eq!(call(&app, "GET", "/api/v1/parts/H.1/history/999", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn accessories_of_both_sides() {
    let app = app(Some(TOKEN));
    // The RCCB lists the signal contact, the signal contact names a second RCCB.
    call(&app, "PUT", "/api/v1/parts/HAG.CDA440D", Some(TOKEN), r#"{"accessories": ["HAG.CZ001"]}"#).await;
    call(&app, "PUT", "/api/v1/parts/HAG.CCA216D", Some(TOKEN), "{}").await;
    call(&app, "PUT", "/api/v1/parts/HAG.CZ001", Some(TOKEN), r#"{"accessory": true, "accessory_mount": "left", "fits": ["HAG.CCA216D"]}"#).await;
    let cz: Accessories = json(&app, "GET", "/api/v1/parts/HAG.CZ001/accessories", None, "").await;
    assert!(cz.accessories.is_empty());
    assert_eq!(cz.fits.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["HAG.CCA216D", "HAG.CDA440D"]);
    for main in ["HAG.CDA440D", "HAG.CCA216D"] {
        let acc: Accessories = json(&app, "GET", &format!("/api/v1/parts/{main}/accessories"), None, "").await;
        assert_eq!(acc.accessories.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["HAG.CZ001"], "{main}");
        assert_eq!(acc.accessories[0].accessory_mount, openecraft_parts::AccessoryMount::Left);
    }
    assert_eq!(call(&app, "GET", "/api/v1/parts/NONE/accessories", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn users_roles_and_sessions() {
    let app = app(Some(TOKEN));
    let editor = sign_up(&app, "eve", "Editor", "").await;
    let acme = sign_up(&app, "max", "Manufacturer", "ACME").await;
    let reader = sign_up(&app, "rita", "Reader", "").await;

    let info: ServerInfo = json(&app, "GET", "/api/v1/info", Some(&acme), "").await;
    assert_eq!(info.user.as_ref().unwrap().manufacturer, "ACME");
    assert!(info.may_edit("acme") && !info.may_edit("Other"));

    // Manufacturer: own parts only.
    assert_eq!(call(&app, "PUT", "/api/v1/parts/ACME.1", Some(&acme), r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::CREATED);
    let (status, body) = call(&app, "PUT", "/api/v1/parts/OTHER.1", Some(&acme), r#"{"manufacturer": "Other"}"#).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(call(&app, "PUT", "/api/v1/parts/GEN.CT-9A-24DC", Some(&acme), r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "DELETE", "/api/v1/parts/GEN.CT-9A-24DC", Some(&acme), "").await.0, StatusCode::FORBIDDEN);
    let csv = "Part number;Manufacturer\nACME.2;ACME\nOTHER.2;Other\n";
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(&acme), csv).await.0, StatusCode::FORBIDDEN);
    // Editor: everything; reader: nothing.
    assert_eq!(call(&app, "PUT", "/api/v1/parts/OTHER.1", Some(&editor), r#"{"manufacturer": "Other"}"#).await.0, StatusCode::CREATED);
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(&editor), csv).await.0, StatusCode::OK);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/R.1", Some(&reader), "{}").await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(&reader), csv).await.0, StatusCode::FORBIDDEN);
    // Only administrators manage users.
    assert_eq!(call(&app, "GET", "/api/v1/users", Some(&editor), "").await.0, StatusCode::FORBIDDEN);
    let users: Vec<User> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    assert_eq!(users.len(), 3);

    // Wrong passwords, logout, new password ends the sessions.
    assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "eve", "password": "nope"}"#).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "POST", "/api/v1/logout", Some(&reader), "").await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(&reader), "").await.0, StatusCode::UNAUTHORIZED);
    call(&app, "PUT", "/api/v1/users/eve", Some(TOKEN), r#"{"role": "Editor", "password": "new-password"}"#).await;
    assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(&editor), "").await.0, StatusCode::UNAUTHORIZED);
    // Changing the role keeps the password.
    call(&app, "PUT", "/api/v1/users/eve", Some(TOKEN), r#"{"role": "Reader"}"#).await;
    let s: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "EVE", "password": "new-password"}"#).await;
    assert_eq!(s.user.role, Role::Reader);

    // Invalid users
    assert_eq!(call(&app, "PUT", "/api/v1/users/new", Some(TOKEN), r#"{"role": "Reader"}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "PUT", "/api/v1/users/m", Some(TOKEN), r#"{"role": "Manufacturer", "password": "x"}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "PUT", "/api/v1/users/admin", Some(TOKEN), r#"{"role": "Reader", "password": "x"}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "DELETE", "/api/v1/users/max", Some(TOKEN), "").await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(&acme), "").await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sign_ins_at_the_same_time_keep_the_account() {
    let app = app(Some(TOKEN));
    sign_up(&app, "eve", "Editor", "").await;
    let login = r#"{"name": "eve", "password": "password-eve"}"#;
    // Both sessions stay: the second sign-in does not write back an older copy.
    let (a, b): (Session, Session) = tokio::join!(json(&app, "POST", "/api/v1/login", None, login), json(&app, "POST", "/api/v1/login", None, login));
    for token in [&a.token, &b.token] {
        assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(token), "").await.0, StatusCode::OK);
    }
    // A sign-in during which the account is deleted does not bring it back.
    let (signed_in, deleted) = tokio::join!(call(&app, "POST", "/api/v1/login", None, login), call(&app, "DELETE", "/api/v1/users/eve", Some(TOKEN), ""));
    assert_eq!(deleted.0, StatusCode::NO_CONTENT);
    assert_eq!(signed_in.0, StatusCode::UNAUTHORIZED);
    let users: Vec<User> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    assert!(users.iter().all(|u| u.name != "eve"));
}

#[tokio::test]
async fn last_administrator_stays() {
    let state = state(None);
    {
        let mut store = state.store.lock().unwrap();
        let mut account = Account { user: User { name: "root".into(), role: Role::Admin, manufacturer: String::new(), ..Default::default() }, ..Default::default() };
        account.password = hash_password("pw").unwrap();
        store.put_account(account).unwrap();
    }
    let app = router(state);
    let s: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "root", "password": "pw"}"#).await;
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert!(info.writable, "administrator accounts make the server writable");
    assert_eq!(call(&app, "PUT", "/api/v1/users/root", Some(&s.token), r#"{"role": "Editor"}"#).await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "DELETE", "/api/v1/users/root", Some(&s.token), "").await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "PUT", "/api/v1/users/second", Some(&s.token), r#"{"role": "Admin", "password": "second-password"}"#).await.0, StatusCode::OK);
    assert_eq!(call(&app, "DELETE", "/api/v1/users/root", Some(&s.token), "").await.0, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn favorites_per_user() {
    let app = app(Some(TOKEN));
    let ann = sign_up(&app, "ann", "Reader", "").await;
    let bob = sign_up(&app, "bob", "Reader", "").await;
    let catalog = default_catalog();
    let (a, b) = (catalog[0].number.as_str(), catalog[1].number.as_str());
    // Duplicates (also apart), empty and unknown numbers are dropped.
    let body = serde_json::json!([a, "", b, a, "NO-SUCH-PART"]).to_string();
    let saved: Vec<String> = json(&app, "PUT", "/api/v1/favorites", Some(&ann), &body).await;
    assert_eq!(saved, [a, b]);
    let got: Vec<String> = json(&app, "GET", "/api/v1/favorites", Some(&ann), "").await;
    assert_eq!(got, [a, b]);
    let got: Vec<String> = json(&app, "GET", "/api/v1/favorites", Some(&bob), "").await;
    assert!(got.is_empty());
    assert_eq!(call(&app, "GET", "/api/v1/favorites", None, "").await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn catalogs() {
    let app = app(Some(TOKEN));
    let acme = sign_up(&app, "max", "Manufacturer", "ACME").await;
    let body = r#"{"name": "ACME 2026", "manufacturer": "ACME", "version": "2026.1"}"#;
    let c: Catalog = json(&app, "PUT", "/api/v1/catalogs/acme_2026", Some(&acme), body).await;
    assert_eq!((c.id.as_str(), c.parts), ("acme_2026", 0));
    assert_eq!(call(&app, "PUT", "/api/v1/catalogs/other", Some(&acme), r#"{"manufacturer": "Other"}"#).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "PUT", "/api/v1/catalogs/a%20b", Some(TOKEN), "{}").await.0, StatusCode::BAD_REQUEST);
    call(&app, "PUT", "/api/v1/parts/ACME.1", Some(&acme), r#"{"manufacturer": "ACME", "catalog": "acme_2026"}"#).await;
    let list: Vec<Catalog> = json(&app, "GET", "/api/v1/catalogs", None, "").await;
    assert_eq!(list[0].parts, 1);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert_eq!(info.catalogs, 1);
    // A catalog with parts is not deleted.
    assert_eq!(call(&app, "DELETE", "/api/v1/catalogs/acme_2026", Some(&acme), "").await.0, StatusCode::CONFLICT);
    call(&app, "DELETE", "/api/v1/parts/ACME.1", Some(&acme), "").await;
    assert_eq!(call(&app, "DELETE", "/api/v1/catalogs/ACME_2026", Some(&acme), "").await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "GET", "/api/v1/catalogs/acme_2026", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn attachments_and_files() {
    let app = app(Some(TOKEN));
    let (status, _, body) = send(&app, "POST", "/api/v1/parts/GEN.CT-9A-24DC/attachments?name=C%3A%5Cdocs%5Csheet.pdf", Some(TOKEN), b"%PDF-1.4".to_vec()).await;
    assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
    let part: Part = serde_json::from_slice(&body).unwrap();
    let a = &part.attachments[0];
    assert_eq!(a.name, "sheet.pdf");
    let (status, headers, data) = send(&app, "GET", &openecraft_parts::api::file_path(&a.id), None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(data, b"%PDF-1.4");
    assert_eq!(headers[header::CONTENT_TYPE], "application/pdf");
    assert!(headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains("sheet.pdf"));
    // Explicit kind, empty files, unknown parts, no token
    let (status, _, body) = send(&app, "POST", "/api/v1/parts/GEN.CT-9A-24DC/attachments?name=m.json&kind=Macro", Some(TOKEN), b"{}".to_vec()).await;
    assert_eq!(status, StatusCode::OK);
    let part: Part = serde_json::from_slice(&body).unwrap();
    assert_eq!(part.attachments.len(), 2);
    assert_eq!(send(&app, "POST", "/api/v1/parts/GEN.CT-9A-24DC/attachments?name=e.pdf", Some(TOKEN), Vec::new()).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(send(&app, "POST", "/api/v1/parts/NOPE/attachments?name=e.pdf", Some(TOKEN), vec![1]).await.0, StatusCode::NOT_FOUND);
    assert_eq!(send(&app, "POST", "/api/v1/parts/GEN.CT-9A-24DC/attachments?name=e.pdf", None, vec![1]).await.0, StatusCode::UNAUTHORIZED);
    let uri = format!("/api/v1/parts/GEN.CT-9A-24DC/attachments/{}", a.id);
    let part: Part = json(&app, "DELETE", &uri, Some(TOKEN), "").await;
    assert_eq!(part.attachments.len(), 1);
    assert_eq!(call(&app, "DELETE", &uri, Some(TOKEN), "").await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, "GET", "/api/v1/files/unknown", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn importing_a_part_again_keeps_its_files() {
    let app = app(Some(TOKEN));
    let csv = "Part number;Manufacturer;Description\nIMP.1;ACME;Relay\n";
    json::<ImportResult>(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await;
    let (status, _, _) = send(&app, "POST", "/api/v1/parts/IMP.1/attachments?name=sheet.pdf", Some(TOKEN), b"%PDF-1.4".to_vec()).await;
    assert_eq!(status, StatusCode::OK);
    // The CSV has no files: the data sheet stays with the changed part.
    let csv = "Part number;Manufacturer;Description\nIMP.1;ACME;Relay, new text\n";
    json::<ImportResult>(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await;
    let part: Part = json(&app, "GET", "/api/v1/parts/IMP.1", None, "").await;
    assert_eq!(part.description, "Relay, new text");
    assert_eq!(part.attachments.len(), 1);
    assert_eq!(send(&app, "GET", &openecraft_parts::api::file_path(&part.attachments[0].id), None, Vec::new()).await.2, b"%PDF-1.4");
}

#[tokio::test]
async fn import_and_export() {
    let app = app(Some(TOKEN));
    let csv = "Part number;Manufacturer;Description;Functions\nIMP.1;ACME;Imported relay;Coil:A1 A2|CO:11 12 14\n";
    let result: ImportResult = json(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await;
    assert_eq!(result.imported, 1);

    let part: Part = json(&app, "GET", "/api/v1/parts/IMP.1", None, "").await;
    assert_eq!(part.manufacturer, "ACME");
    assert_eq!(part.functions.len(), 2);

    // JSON part list works as well, invalid data is rejected.
    let (status, _) = call(&app, "POST", "/api/v1/import", Some(TOKEN), r#"[{"number": "IMP.2"}]"#).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call(&app, "POST", "/api/v1/import", Some(TOKEN), "{ broken").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // A part is not its own successor, as with a single part.
    let (status, _) = call(&app, "POST", "/api/v1/import", Some(TOKEN), r#"[{"number": "IMP.3", "successor": "IMP.3"}]"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let (status, csv) = call(&app, "GET", "/api/v1/export.csv", None, "").await;
    assert_eq!(status, StatusCode::OK);
    let parts = openecraft_parts::from_csv(&csv).unwrap();
    assert!(parts.iter().any(|p| p.number == "IMP.1"));
    assert!(parts.iter().any(|p| p.number == "IMP.2"));
    // The address selects the parts exported (the filters of the web page).
    let (_, csv) = call(&app, "GET", "/api/v1/export.csv?q=IMP.1", None, "").await;
    let parts = openecraft_parts::from_csv(&csv).unwrap();
    assert_eq!(parts.iter().map(|p| p.number.as_str()).collect::<Vec<_>>(), ["IMP.1"]);
}

#[tokio::test]
async fn revision_increases_on_changes() {
    let app = app(Some(TOKEN));
    let rev = |body: String| serde_json::from_str::<ServerInfo>(&body).unwrap().revision;
    let before = rev(call(&app, "GET", "/api/v1/info", None, "").await.1);
    call(&app, "PUT", "/api/v1/parts/X", Some(TOKEN), r#"{"number": "X"}"#).await;
    let after = rev(call(&app, "GET", "/api/v1/info", None, "").await.1);
    assert_eq!(after, before + 1);
    let p: Part = json(&app, "GET", "/api/v1/parts/X", None, "").await;
    assert_eq!(p.revision, after);
}

#[tokio::test]
async fn web_page_and_texts() {
    let app = app(None);
    let (status, headers, page) = send(&app, "GET", "/", None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/html"));
    assert_eq!(headers[header::CONTENT_SECURITY_POLICY], web::PAGE_CSP);
    assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY");
    assert!(String::from_utf8(page).unwrap().contains("<script src=\"/portal.js\"></script>"));
    let (status, headers, script) = send(&app, "GET", "/portal.js", None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/javascript"));
    assert!(String::from_utf8(script).unwrap().contains("/api/v1"));
    let (status, headers, script) = send(&app, "GET", "/admin.js", None, Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/javascript"));
    assert!(String::from_utf8(script).unwrap().contains("function showAdmin"));
    let texts: web::WebTexts = json(&app, "GET", "/api/v1/texts?lang=fr-CA", None, "").await;
    assert_eq!(texts.lang, "fr");
    let texts: web::WebTexts = json(&app, "GET", "/api/v1/texts?lang=xx", None, "").await;
    assert_eq!(texts.lang, "en");
}

#[tokio::test]
async fn cors_preflight_allows_authorization_header() {
    let req = Request::builder()
        .method("OPTIONS")
        .uri("/api/v1/import")
        .header(header::ORIGIN, "http://localhost:8080")
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "authorization,content-type")
        .body(Body::empty())
        .unwrap();
    let resp = app(Some(TOKEN)).oneshot(req).await.unwrap();
    let h = |name| resp.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("").to_lowercase();
    assert_eq!(h(header::ACCESS_CONTROL_ALLOW_ORIGIN), "*");
    // The wildcard would not cover Authorization in browsers.
    assert!(h(header::ACCESS_CONTROL_ALLOW_HEADERS).contains("authorization"));
    assert!(h(header::ACCESS_CONTROL_ALLOW_METHODS).contains("post"));
}

#[tokio::test]
async fn grants_to_groups_per_manufacturer() {
    let app = app(Some(TOKEN));
    let body = r#"{"role": "Reader", "manufacturer": "", "password": "password-max", "groups": ["purchasing", "trainees"]}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/users/max", Some(TOKEN), body).await.0, StatusCode::OK);
    let max: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "max", "password": "password-max"}"#).await;
    assert_eq!(max.user.groups, ["purchasing", "trainees"]);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/ACME.1", Some(&max.token), r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::FORBIDDEN);

    // Purchasing maintains ACME; trainees never delete parts or change catalogs.
    let policy = r#"{"anonymous": "reader",
        "roles": [{"name": "cleanup", "rights": ["delete_parts", "edit_catalogs"]}],
        "grants": [{"subject": {"group": "purchasing"}, "role": "manufacturer", "scope": "ACME"},
                   {"subject": {"group": "trainees"}, "role": "cleanup", "scope": "*", "deny": true}]}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/access", Some(&max.token), policy).await.0, StatusCode::FORBIDDEN);
    let settings: Settings<PartsRight> = json(&app, "PUT", "/api/v1/access", Some(TOKEN), policy).await;
    assert_eq!(settings.policy.grants.len(), 2);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", Some(&max.token), "").await;
    assert!(info.may_edit("ACME") && !info.may(PartsRight::DeleteParts, "ACME") && !info.may_edit("Other"));
    assert_eq!(call(&app, "PUT", "/api/v1/parts/ACME.1", Some(&max.token), r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::CREATED);
    let (status, text) = call(&app, "DELETE", "/api/v1/parts/ACME.1", Some(&max.token), "").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(text.contains("delete_parts"), "{text}");
    assert_eq!(call(&app, "PUT", "/api/v1/catalogs/acme", Some(&max.token), r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::FORBIDDEN);

    let invalid = r#"{"grants": [{"subject": {"user": "max"}, "role": "nobody"}]}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/access", Some(TOKEN), invalid).await.0, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn private_servers_need_a_sign_in_to_read() {
    let state = state(Some(TOKEN));
    access_file(&state).config.policy.anonymous.clear();
    let app = router(state);
    for uri in ["/api/v1/parts", "/api/v1/catalogs", "/api/v1/parts/GEN.CT-9A-24DC", "/api/v1/export.csv"] {
        assert_eq!(call(&app, "GET", uri, None, "").await.0, StatusCode::UNAUTHORIZED, "{uri}");
    }
    assert_eq!(call(&app, "POST", "/api/v1/search", None, "{}").await.0, StatusCode::UNAUTHORIZED);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert!(info.access.is_empty(), "info tells clients to sign in");
    let reader = sign_up(&app, "rea", "Reader", "").await;
    assert_eq!(call(&app, "GET", "/api/v1/parts", Some(&reader), "").await.0, StatusCode::OK);
    let mut settings: Settings<PartsRight> = json(&app, "GET", "/api/v1/access", Some(TOKEN), "").await;
    settings.policy.grants.push(Grant { deny: true, ..Grant::new(Subject::User("rea".into()), "reader", "*") });
    json::<Settings<PartsRight>>(&app, "PUT", "/api/v1/access", Some(TOKEN), &serde_json::to_string(&settings.policy).unwrap()).await;
    assert_eq!(call(&app, "GET", "/api/v1/parts", Some(&reader), "").await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn admins_from_grants_and_directories() {
    let state = state(None);
    access_file(&state).config.directories.push(openecraft_auth::Directory {
        id: "corp".into(),
        url: "ldap://127.0.0.1:1".into(),
        bind_dn: "CN=svc".into(),
        base_dn: "DC=corp".into(),
        timeout_secs: 1,
        ..Default::default()
    });
    {
        let mut store = store(&state);
        let root = User { name: "root".into(), role: Role::Admin, ..Default::default() };
        store.put_account(Account { user: root, password: hash_password("pw-root").unwrap(), ..Default::default() }).unwrap();
        let ops = User { name: "ops".into(), groups: vec!["it".into()], ..Default::default() };
        store.put_account(Account { user: ops, password: hash_password("pw-ops").unwrap(), ..Default::default() }).unwrap();
    }
    let app = router(state.clone());
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert!(info.writable, "directory users can sign in");
    let root: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "root", "password": "pw-root"}"#).await;
    assert_eq!(call(&app, "DELETE", "/api/v1/users/root", Some(&root.token), "").await.0, StatusCode::CONFLICT);
    // An administrator by grant keeps the server manageable.
    let policy = r#"{"anonymous": "reader", "grants": [{"subject": {"group": "it"}, "role": "admin"}]}"#;
    json::<Settings<PartsRight>>(&app, "PUT", "/api/v1/access", Some(&root.token), policy).await;
    let ops: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "ops", "password": "pw-ops"}"#).await;
    assert!(ops.access.allows(SERVER, PartsRight::Administer));
    assert_eq!(call(&app, "DELETE", "/api/v1/users/root", Some(&ops.token), "").await.0, StatusCode::NO_CONTENT);
    // Unknown names go to the directory, which is down.
    assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "anna", "password": "pw"}"#).await.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "ops", "password": "wrong"}"#).await.0, StatusCode::UNAUTHORIZED);
    let settings: Settings<PartsRight> = json(&app, "GET", "/api/v1/access", Some(&ops.token), "").await;
    assert_eq!(settings.providers[0].id, "corp");
    // Now ops is the last administrator, by grant: it can neither go nor lose the grant.
    assert_eq!(call(&app, "DELETE", "/api/v1/users/ops", Some(&ops.token), "").await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "PUT", "/api/v1/users/ops", Some(&ops.token), r#"{"role": "Editor", "groups": []}"#).await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "PUT", "/api/v1/access", Some(&ops.token), r#"{"anonymous": "reader"}"#).await.0, StatusCode::CONFLICT);
    assert!(call(&app, "GET", "/api/v1/users", Some(&ops.token), "").await.0.is_success());
}

#[tokio::test]
async fn part_numbers_and_successors_are_checked() {
    let app = app(Some(TOKEN));
    let put = |n: &'static str, body: &'static str| {
        let app = app.clone();
        async move { call(&app, "PUT", &format!("/api/v1/parts/{n}"), Some(TOKEN), body).await }
    };
    assert_eq!(put("%20LEAD", r#"{"number": " LEAD"}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(put("TRAIL%20", r#"{"number": "TRAIL "}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(put("TEST-1", r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::CREATED);
    // Only the case differs from an existing part.
    let (status, text) = put("test-1", r#"{"manufacturer": "ACME"}"#).await;
    assert_eq!((status, text.as_str()), (StatusCode::CONFLICT, r#"{"error":"part 'TEST-1' already exists"}"#));
    assert_eq!(put("TEST-1", r#"{"successor": "NOPE"}"#).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(put("TEST-2", r#"{"manufacturer": "ACME"}"#).await.0, StatusCode::CREATED);
    assert_eq!(put("TEST-1", r#"{"successor": "TEST-2"}"#).await.0, StatusCode::OK);
    // An import may name a successor of the same file.
    let csv = "Part number;Successor\nIMP-1;IMP-2\nIMP-2;\n";
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await.0, StatusCode::OK);
    let csv = "Part number;Successor\nIMP-3;NOPE\n";
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await.0, StatusCode::BAD_REQUEST);
    // The history of a part that never existed is not found.
    assert_eq!(call(&app, "GET", "/api/v1/parts/NOPE/history", None, "").await.0, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn errors_are_json() {
    let app = app(Some(TOKEN));
    for (method, uri, body) in [("GET", "/api/v1/parts?limit=abc", ""), ("POST", "/api/v1/search", "{not json"), ("GET", "/api/v1/nope", ""), ("PATCH", "/api/v1/parts/X", "")] {
        let (status, text) = call(&app, method, uri, None, body).await;
        assert!(status.is_client_error(), "{uri}: {status}");
        assert!(serde_json::from_str::<ApiError>(&text).is_ok(), "{uri}: {text}");
    }
}

#[tokio::test]
async fn passwords_and_failed_sign_ins() {
    let app = app(Some(TOKEN));
    let (status, text) = call(&app, "PUT", "/api/v1/users/weak", Some(TOKEN), r#"{"role": "Reader", "password": "1"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    sign_up(&app, "anna", "Reader", "").await;
    for _ in 0..openecraft_auth::throttle::FREE_ATTEMPTS {
        assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "anna", "password": "guess"}"#).await.0, StatusCode::UNAUTHORIZED);
    }
    // Even the right password waits now.
    let right = r#"{"name": "anna", "password": "password-anna"}"#;
    assert_eq!(call(&app, "POST", "/api/v1/login", None, right).await.0, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn unknown_tokens_are_refused_for_reading() {
    let app = app(Some(TOKEN));
    // No token reads anonymously, a wrong or expired one is an error.
    assert_eq!(call(&app, "GET", "/api/v1/parts?limit=1", None, "").await.0, StatusCode::OK);
    for uri in ["/api/v1/info", "/api/v1/parts?limit=1", "/api/v1/export.csv", "/api/v1/catalogs"] {
        let (status, text) = call(&app, "GET", uri, Some("expired"), "").await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}: {text}");
    }
    // The sign-in field is read as `name` and as `user` (project server).
    let token = sign_up(&app, "anna", "Reader", "").await;
    assert_eq!(call(&app, "GET", "/api/v1/info", Some(&token), "").await.0, StatusCode::OK);
    let (status, _) = call(&app, "POST", "/api/v1/login", None, r#"{"user": "anna", "password": "password-anna"}"#).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn parts_are_checked_before_storing() {
    let app = app(Some(TOKEN));
    let long = "X".repeat(129);
    let (status, text) = call(&app, "PUT", &format!("/api/v1/parts/{long}"), Some(TOKEN), "{}").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{text}");
    let body = format!(r#"{{"description": "{}"}}"#, "y".repeat(20_001));
    assert_eq!(call(&app, "PUT", "/api/v1/parts/LONG.1", Some(TOKEN), &body).await.0, StatusCode::BAD_REQUEST);
    // Parts name existing catalogs only.
    let (status, text) = call(&app, "PUT", "/api/v1/parts/CAT.1", Some(TOKEN), r#"{"catalog": "nope"}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(text.contains("catalog 'nope'"), "{text}");
    call(&app, "PUT", "/api/v1/catalogs/acme", Some(TOKEN), r#"{"manufacturer": "ACME"}"#).await;
    assert_eq!(call(&app, "PUT", "/api/v1/parts/CAT.1", Some(TOKEN), r#"{"catalog": "ACME"}"#).await.0, StatusCode::CREATED);
    let csv = "Part number;Catalog\nCAT.2;nope\n";
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(TOKEN), csv).await.0, StatusCode::BAD_REQUEST);
    // A file naming a part twice is refused instead of counting it twice.
    let (status, text) = call(&app, "POST", "/api/v1/import", Some(TOKEN), r#"[{"number": "DUP.1"}, {"number": "dup.1"}]"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(text.contains("more than once"), "{text}");
    // Enumerations in the address are read in lower case as well.
    for uri in ["/api/v1/parts?top_group=fluid", "/api/v1/parts?lifecycle=active", "/api/v1/parts?sort=price"] {
        assert_eq!(call(&app, "GET", uri, None, "").await.0, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn files_that_can_run_scripts_are_downloaded() {
    let app = app(Some(TOKEN));
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#.to_vec();
    for (name, body, disposition) in [("evil.svg", svg, "attachment"), ("x.xml", b"<a/>".to_vec(), "attachment"), ("photo.png", b"png".to_vec(), "inline")] {
        let (status, _, part) = send(&app, "POST", &format!("/api/v1/parts/GEN.CT-9A-24DC/attachments?name={name}"), Some(TOKEN), body).await;
        assert_eq!(status, StatusCode::OK);
        let part: Part = serde_json::from_slice(&part).unwrap();
        let id = &part.attachments.iter().find(|a| a.name == name).unwrap().id;
        let (_, headers, _) = send(&app, "GET", &format!("/api/v1/files/{id}"), None, Vec::new()).await;
        assert!(headers[header::CONTENT_DISPOSITION].to_str().unwrap().starts_with(disposition), "{name}");
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(headers[header::CONTENT_SECURITY_POLICY], openecraft_auth::DEFAULT_CSP);
    }
}

#[tokio::test]
async fn changes_are_refused_before_their_body_is_read() {
    let app = app(Some(TOKEN));
    // Not even parsed: no token is a 401, not a 400 for the body.
    assert_eq!(call(&app, "POST", "/api/v1/import", None, "{not json").await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/X", Some("wrong"), "{not json").await.0, StatusCode::UNAUTHORIZED);
    // Requests anybody may send have a small limit.
    let big = format!(r#"{{"name": "x", "password": "{}"}}"#, "x".repeat(SMALL_BODY));
    assert_eq!(call(&app, "POST", "/api/v1/login", None, &big).await.0, StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn manufacturers_keep_out_of_foreign_catalogs() {
    let app = app(Some(TOKEN));
    let acme = sign_up(&app, "acme", "manufacturer", "ACME").await;
    json::<Catalog>(&app, "PUT", "/api/v1/catalogs/other", Some(TOKEN), r#"{"manufacturer": "Other"}"#).await;
    json::<Catalog>(&app, "PUT", "/api/v1/catalogs/acme", Some(TOKEN), r#"{"manufacturer": "ACME"}"#).await;
    let (status, text) = call(&app, "PUT", "/api/v1/parts/ACME-1", Some(&acme), r#"{"manufacturer": "ACME", "catalog": "other"}"#).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{text}");
    let parts = r#"[{"number": "ACME-2", "manufacturer": "ACME", "catalog": "other"}]"#;
    assert_eq!(call(&app, "POST", "/api/v1/import", Some(&acme), parts).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/ACME-1", Some(&acme), r#"{"manufacturer": "ACME", "catalog": "acme"}"#).await.0, StatusCode::CREATED);
    // A part an administrator put into another catalog stays editable.
    json::<Part>(&app, "PUT", "/api/v1/parts/ACME-3", Some(TOKEN), r#"{"manufacturer": "ACME", "catalog": "other"}"#).await;
    let (status, text) = call(&app, "PUT", "/api/v1/parts/ACME-3", Some(&acme), r#"{"manufacturer": "ACME", "catalog": "other", "description": "new"}"#).await;
    assert_eq!(status, StatusCode::OK, "{text}");
}

#[tokio::test]
async fn unknown_names_take_as_long_as_wrong_passwords() {
    let app = app(Some(TOKEN));
    sign_up(&app, "anna", "reader", "").await;
    let time = |name: &'static str| {
        let app = app.clone();
        async move {
            let start = std::time::Instant::now();
            let body = format!(r#"{{"name": "{name}", "password": "wrong-password"}}"#);
            assert_eq!(call(&app, "POST", "/api/v1/login", None, &body).await.0, StatusCode::UNAUTHORIZED);
            start.elapsed()
        }
    };
    let known = time("anna").await;
    let unknown = time("nobody").await;
    assert!(unknown * 4 > known, "unknown {unknown:?}, known {known:?}");
}

#[tokio::test]
async fn libraries_are_read_by_all_and_written_by_editors() {
    let app = app(Some(TOKEN));
    let reader = sign_up(&app, "rita", "Reader", "").await;
    let editor = sign_up(&app, "ed", "Editor", "").await;
    let acme = sign_up(&app, "max", "Manufacturer", "ACME").await;
    assert_eq!(call(&app, "GET", "/api/v1/libraries/forms", None, "").await.0, StatusCode::NOT_FOUND);
    let body = r#"{"title_blocks": [{"name": "Company", "items": []}]}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/forms", Some(&reader), body).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/forms", Some(&acme), body).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/forms", None, body).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/forms", Some(&editor), "[1, 2]").await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/a%20b", Some(&editor), body).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "PUT", "/api/v1/libraries/forms", Some(&editor), body).await.0, StatusCode::NO_CONTENT);
    let (status, headers, bytes) = send(&app, "GET", "/api/v1/libraries/forms", Some(&reader), Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "application/json");
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["title_blocks"][0]["name"], "Company");
    assert_eq!(openecraft_parts::api::library_path(openecraft_parts::api::FORMS_LIBRARY), "/api/v1/libraries/forms");
}

#[tokio::test]
async fn administrators_see_sessions_and_sign_users_out() {
    let app = app(Some(TOKEN));
    let eve = sign_up(&app, "eve", "Editor", "").await;
    sign_up(&app, "rita", "Reader", "").await;
    let second: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "eve", "password": "password-eve"}"#).await;
    let users: Vec<UserEntry> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    let of = |name: &str| users.iter().find(|u| u.user.name == name).unwrap().clone();
    assert_eq!(of("eve").sessions, 2);
    assert!(of("eve").signed_in > 0);
    assert_eq!(of("eve").user.role, Role::Editor);
    // Clients that only know users read the same list.
    let plain: Vec<User> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    assert_eq!(plain.len(), 2);

    assert_eq!(call(&app, "DELETE", "/api/v1/users/rita/sessions", Some(&eve), "").await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "DELETE", "/api/v1/users/nobody/sessions", Some(TOKEN), "").await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, "DELETE", "/api/v1/users/EVE/sessions", Some(TOKEN), "").await.0, StatusCode::NO_CONTENT);
    for token in [&eve, &second.token] {
        assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(token), "").await.0, StatusCode::UNAUTHORIZED);
    }
    let users: Vec<UserEntry> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    assert_eq!(users.iter().find(|u| u.user.name == "eve").unwrap().sessions, 0);
    // The password stays.
    assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "eve", "password": "password-eve"}"#).await.0, StatusCode::OK);
}

#[tokio::test]
async fn bundled_catalogs_are_installed_by_administrators() {
    let app = app(Some(TOKEN));
    let eve = sign_up(&app, "eve", "Editor", "").await;
    assert_eq!(call(&app, "GET", "/api/v1/bundled", Some(&eve), "").await.0, StatusCode::FORBIDDEN);
    let list: Vec<BundledCatalog> = json(&app, "GET", "/api/v1/bundled", Some(TOKEN), "").await;
    assert_eq!(list.len(), bundled::ids().len());
    let first = list[0].clone();
    assert!(first.parts > 0 && first.installed.is_empty(), "{first:?}");

    let id = format!("/api/v1/bundled/{}", first.id.to_uppercase());
    assert_eq!(call(&app, "POST", &id, Some(&eve), "").await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "POST", "/api/v1/bundled/nope", Some(TOKEN), "").await.0, StatusCode::NOT_FOUND);
    let installed: Installed = json(&app, "POST", &id, Some(TOKEN), "").await;
    assert_eq!(installed.added, first.parts);
    let list: Vec<BundledCatalog> = json(&app, "GET", "/api/v1/bundled", Some(TOKEN), "").await;
    assert_eq!(list[0].installed, first.version);
    let catalogs: Vec<Catalog> = json(&app, "GET", "/api/v1/catalogs", None, "").await;
    assert!(catalogs.iter().any(|c| c.id == first.id && c.parts == first.parts));
    // Installing again changes nothing.
    let again: Installed = json(&app, "POST", &id, Some(TOKEN), "").await;
    assert_eq!(again, Installed::default());
}

#[tokio::test]
async fn exports_and_backups() {
    let app = app(Some(TOKEN));
    let eve = sign_up(&app, "eve", "Editor", "").await;
    call(&app, "PUT", "/api/v1/parts/X.1", Some(&eve), r#"{"manufacturer": "X", "description": "First"}"#).await;
    let parts: Vec<Part> = json(&app, "GET", "/api/v1/export.json?manufacturer=X", None, "").await;
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].description, "First");
    // The export imports again.
    let all = call(&app, "GET", "/api/v1/export.json", None, "").await.1;
    let imported: ImportResult = json(&app, "POST", "/api/v1/import", Some(&eve), &all).await;
    assert_eq!(imported.imported, default_catalog().len() + 1);

    assert_eq!(call(&app, "GET", "/api/v1/backup", Some(&eve), "").await.0, StatusCode::FORBIDDEN);
    let (status, headers, data) = send(&app, "GET", "/api/v1/backup", Some(TOKEN), Vec::new()).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_DISPOSITION].to_str().unwrap().contains(".sqlite"));
    // The backup is a database the server opens: parts, accounts and all.
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let path = std::env::temp_dir().join(format!("openecraft-test-{}-{nanos}", std::process::id())).join("backup.sqlite");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, data).unwrap();
    let restored = Store::open(crate::storage::StorageKind::Sqlite, &path).unwrap();
    assert_eq!(restored.get("X.1").unwrap().unwrap().description, "First");
    assert!(restored.account("eve").is_some());
    assert_eq!(restored.count().unwrap(), default_catalog().len() + 1);
}

#[tokio::test]
async fn users_register_themselves_when_the_registration_is_open() {
    let app = app(Some(TOKEN));
    let login = |name: &str| format!(r#"{{"name": "{name}", "password": "password-{name}"}}"#);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert!(!info.registration, "closed unless switched on");
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("nina")).await.0, StatusCode::FORBIDDEN);

    let eve = sign_up(&app, "eve", "Editor", "").await;
    assert_eq!(call(&app, "PUT", "/api/v1/registration", Some(&eve), r#"{"enabled": true}"#).await.0, StatusCode::FORBIDDEN);
    let r: Registration = json(&app, "PUT", "/api/v1/registration", Some(TOKEN), r#"{"enabled": true, "approval": false}"#).await;
    assert!(r.enabled && !r.fixed && !r.approval);
    let info: ServerInfo = json(&app, "GET", "/api/v1/info", None, "").await;
    assert!(info.registration);

    let s: Session = json(&app, "POST", "/api/v1/register", None, &login("nina")).await;
    assert_eq!((s.user.name.as_str(), s.user.role), ("nina", Role::Reader));
    assert_eq!(call(&app, "GET", "/api/v1/favorites", Some(&s.token), "").await.0, StatusCode::OK);
    assert_eq!(call(&app, "PUT", "/api/v1/parts/N.1", Some(&s.token), "{}").await.0, StatusCode::FORBIDDEN, "readers only");
    let again: Session = json(&app, "POST", "/api/v1/login", None, &login("nina")).await;
    assert_eq!(again.user.name, "nina");

    // Names that are taken, refer to grants or are not names; short passwords.
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("NINA")).await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("eve")).await.0, StatusCode::CONFLICT);
    let policy = r#"{"anonymous": "reader", "grants": [{"subject": {"user": "boss*"}, "role": "admin", "scope": "*"}]}"#;
    assert_eq!(call(&app, "PUT", "/api/v1/access", Some(TOKEN), policy).await.0, StatusCode::OK);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("boss2")).await.0, StatusCode::CONFLICT);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("admin")).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("a b")).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, r#"{"name": "kim", "password": "short"}"#).await.0, StatusCode::BAD_REQUEST);

    // Many registrations from one client are slowed down.
    let mut statuses = Vec::new();
    for i in 0..4 {
        statuses.push(call(&app, "POST", "/api/v1/register", None, &login(&format!("user{i}"))).await.0);
    }
    assert!(statuses.contains(&StatusCode::TOO_MANY_REQUESTS), "{statuses:?}");

    let r: Registration = json(&app, "PUT", "/api/v1/registration", Some(TOKEN), r#"{"enabled": false, "approval": false}"#).await;
    assert!(!r.enabled);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, &login("late")).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn start_options_fix_the_registration() {
    let app = router(state(Some(TOKEN)).with_registration(Some(true)));
    let r: Registration = json(&app, "GET", "/api/v1/registration", Some(TOKEN), "").await;
    assert!(r.enabled && r.fixed && r.approval);
    assert_eq!(call(&app, "PUT", "/api/v1/registration", Some(TOKEN), r#"{"enabled": false}"#).await.0, StatusCode::CONFLICT);
    // The approval stays changeable.
    let r: Registration = json(&app, "PUT", "/api/v1/registration", Some(TOKEN), r#"{"enabled": true, "approval": false}"#).await;
    assert!(r.enabled && r.fixed && !r.approval);
    assert_eq!(call(&app, "POST", "/api/v1/register", None, r#"{"name": "nina", "password": "password-nina"}"#).await.0, StatusCode::OK);
    let app = router(state(Some(TOKEN)).with_registration(Some(false)));
    assert_eq!(call(&app, "POST", "/api/v1/register", None, r#"{"name": "nina", "password": "password-nina"}"#).await.0, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn registered_users_wait_for_approval_by_default() {
    let app = app(Some(TOKEN));
    let r: Registration = json(&app, "PUT", "/api/v1/registration", Some(TOKEN), r#"{"enabled": true}"#).await;
    assert!(r.approval, "approval unless switched off");
    let login = r#"{"name": "nina", "password": "password-nina"}"#;
    let (status, body) = call(&app, "POST", "/api/v1/register", None, login).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let s: Session = serde_json::from_str(&body).unwrap();
    assert!(s.token.is_empty() && s.user.pending);
    // Waiting: the right password is refused, a wrong one as usual.
    let (status, body) = call(&app, "POST", "/api/v1/login", None, login).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(body.contains("approval"));
    assert_eq!(call(&app, "POST", "/api/v1/login", None, r#"{"name": "nina", "password": "wrong-password"}"#).await.0, StatusCode::UNAUTHORIZED);
    let users: Vec<UserEntry> = json(&app, "GET", "/api/v1/users", Some(TOKEN), "").await;
    assert!(users[0].user.pending);

    let eve = sign_up(&app, "eve", "Editor", "").await;
    assert_eq!(call(&app, "POST", "/api/v1/users/nina/approval", Some(&eve), "").await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "POST", "/api/v1/users/nobody/approval", Some(TOKEN), "").await.0, StatusCode::NOT_FOUND);
    let approved: User = json(&app, "POST", "/api/v1/users/NINA/approval", Some(TOKEN), "").await;
    assert!(!approved.pending);
    let s: Session = json(&app, "POST", "/api/v1/login", None, login).await;
    assert_eq!(s.user.role, Role::Reader);
}

#[tokio::test]
async fn waiting_administrators_do_not_count() {
    let state = state(None);
    {
        let mut store = state.store.lock().unwrap();
        let mut root = Account { user: User { name: "root".into(), role: Role::Admin, ..Default::default() }, ..Default::default() };
        root.password = hash_password("root-password").unwrap();
        store.put_account(root).unwrap();
        let waiting = Account { user: User { name: "wait".into(), role: Role::Admin, pending: true, ..Default::default() }, ..Default::default() };
        store.put_account(waiting).unwrap();
    }
    let app = router(state);
    let s: Session = json(&app, "POST", "/api/v1/login", None, r#"{"name": "root", "password": "root-password"}"#).await;
    assert_eq!(call(&app, "DELETE", "/api/v1/users/root", Some(&s.token), "").await.0, StatusCode::CONFLICT);
}
