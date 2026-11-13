use super::*;
use openecraft_parts::portal::{Lifecycle, Role, User};

pub(crate) fn temp_file(name: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    std::env::temp_dir().join(format!("openecraft-test-{}-{nanos}", std::process::id())).join(name)
}

fn part(number: &str) -> Part {
    Part { number: number.into(), description: format!("Test {number}"), ..Default::default() }
}

/// The same store rules on both backends.
fn stores() -> Vec<Store> {
    vec![
        Store::with_backend(Box::new(JsonBackend::in_memory()), default_catalog()).unwrap(),
        Store::with_backend(Box::new(SqliteBackend::in_memory().unwrap()), default_catalog()).unwrap(),
    ]
}

#[test]
fn new_stores_are_seeded() {
    for store in stores() {
        assert_eq!(store.revision, 1);
        assert_eq!(store.count().unwrap(), default_catalog().len());
        assert!(store.all().unwrap().iter().all(|p| p.revision == 1 && p.modified > 0));
    }
}

#[test]
fn changes_get_revisions_and_history() {
    for mut store in stores() {
        let stored = store.put_parts("alice", vec![part("TEST.1")]).unwrap();
        assert_eq!((stored[0].0.revision, stored[0].1), (2, true));
        assert_eq!(store.revision, 2);
        // Unchanged parts are not stored again.
        let same = store.put_parts("bob", vec![part("TEST.1")]).unwrap();
        assert_eq!((same[0].0.revision, same[0].1), (2, false));
        assert_eq!(store.revision, 2);
        let changed = Part { description: "changed".into(), ..part("TEST.1") };
        store.put_parts("bob", vec![changed]).unwrap();
        assert!(store.delete("carol", "TEST.1").unwrap());
        assert!(!store.delete("carol", "TEST.1").unwrap());
        assert_eq!(store.revision, 4);

        let history = store.history("TEST.1").unwrap();
        let summary: Vec<(u64, &str, ChangeKind)> = history.iter().map(|h| (h.revision, h.user.as_str(), h.change)).collect();
        assert_eq!(summary, [(4, "carol", ChangeKind::Deleted), (3, "bob", ChangeKind::Changed), (2, "alice", ChangeKind::Created)]);
        assert_eq!(store.history_part("TEST.1", 2).unwrap().unwrap().description, "Test TEST.1");
        assert_eq!(store.history_part("TEST.1", 4).unwrap().unwrap().description, "changed");
        assert!(store.history_part("TEST.1", 9).unwrap().is_none());
    }
}

#[test]
fn history_is_limited() {
    for mut store in stores() {
        for i in 0..HISTORY_LIMIT + 5 {
            store.put_parts("u", vec![Part { note: i.to_string(), ..part("TEST.1") }]).unwrap();
        }
        let history = store.history("TEST.1").unwrap();
        assert_eq!(history.len(), HISTORY_LIMIT);
        assert_eq!(history[0].revision, store.revision);
    }
}

#[test]
fn import_with_duplicates_is_one_revision() {
    for mut store in stores() {
        let parts = vec![part("A"), part("B"), Part { note: "second".into(), ..part("A") }];
        let out = store.put_parts("u", parts).unwrap();
        assert_eq!(out.len(), 3);
        assert_eq!(store.revision, 2);
        assert_eq!(store.get("A").unwrap().unwrap().note, "second");
        assert_eq!(store.count().unwrap(), default_catalog().len() + 2);
    }
}

#[test]
fn search_uses_the_shared_query() {
    for mut store in stores() {
        let p = Part { manufacturer: "ACME".into(), voltage: "24 V".into(), catalog: "acme_2026".into(), ..part("ACME.1") };
        store.put_parts("u", vec![p, Part { lifecycle: Lifecycle::Discontinued, ..part("ACME.2") }]).unwrap();
        let q = PartQuery { q: Some("acme_2026".into()), ..Default::default() };
        let r = store.search(&q).unwrap();
        assert_eq!(r.total, 1);
        assert_eq!(r.facet("Voltage").unwrap().values[0].value, "24 V");
        let q = PartQuery { numbers: vec!["ACME.2".into(), "GEN.CT-9A-24DC".into()], brief: true, ..Default::default() };
        let r = store.search(&q).unwrap();
        assert_eq!(r.total, 2);
        assert!(r.parts.iter().all(|p| p.model_3d.is_none()));
        // Words with LIKE wildcards are looked up literally.
        assert_eq!(store.list(&PartQuery { q: Some("%".into()), ..Default::default() }).unwrap().0, 0);
        assert_eq!(store.list(&PartQuery { q: Some("acme_".into()), ..Default::default() }).unwrap().0, 1);
    }
}

#[test]
fn models_are_kept_with_the_part() {
    for mut store in stores() {
        let full = default_catalog().into_iter().find(|p| p.model_3d.is_some() || p.model_2d.is_some()).expect("catalog part with a model");
        let got = store.get(&full.number).unwrap().unwrap();
        assert_eq!(content(&got), content(&full));
        let listed = store.list(&PartQuery { numbers: vec![full.number.clone()], ..Default::default() }).unwrap().1;
        assert_eq!(content(&listed[0]), content(&full));
        store.put_parts("u", vec![Part { model_3d: None, model_2d: None, ..full.clone() }]).unwrap();
        assert!(store.get(&full.number).unwrap().unwrap().model_3d.is_none());
    }
}

#[test]
fn eplan_variants_round_trip_without_other_models() {
    for mut store in stores() {
        let mut full = part("BEC.EK1100");
        full.eplan_variants.push(serde_json::json!({"symbol":{"key":"user:ema:EK1100:1:0","coordinate":3.713_567_972_183_227_5_f64}}).into());
        store.put_parts("u",vec![full.clone()]).unwrap();
        assert_eq!(store.get(&full.number).unwrap().unwrap().eplan_variants,full.eplan_variants);
        assert!(brief(&full).eplan_variants.is_empty());
    }
}

#[test]
fn catalogs_count_their_parts() {
    for mut store in stores() {
        let c = Catalog { id: "acme".into(), name: "ACME".into(), manufacturer: "ACME".into(), version: "1".into(), ..Default::default() };
        let stored = store.put_catalog(c).unwrap();
        assert!(stored.modified > 0);
        store.put_parts("u", vec![Part { catalog: "ACME".into(), ..part("A") }, Part { catalog: "acme".into(), ..part("B") }]).unwrap();
        assert_eq!(store.catalog("Acme").unwrap().unwrap().parts, 2);
        let rev = store.revision;
        store.delete_catalog("ACME").unwrap();
        assert!(store.catalogs().unwrap().is_empty());
        assert_eq!(store.revision, rev + 1);
    }
}

#[test]
fn accounts_sessions_and_favorites() {
    for mut store in stores() {
        let mut account = Account { user: User { name: "ann".into(), role: Role::Editor, manufacturer: String::new(), ..Default::default() }, ..Default::default() };
        let token = account.start_session();
        let rev = store.revision;
        store.put_account(account).unwrap();
        assert_eq!(store.revision, rev, "accounts do not change the parts revision");
        assert_eq!(store.session(&token).unwrap().user.name, "ann");
        assert!(store.account("ANN").is_some());
        store.set_favorites("ann", vec!["A".into(), "B".into()]).unwrap();
        assert_eq!(store.favorites("ann").unwrap(), ["A", "B"]);
        assert!(store.favorites("bob").unwrap().is_empty());
        store.delete_account("ann").unwrap();
        assert!(store.session(&token).is_none());
        assert!(store.favorites("ann").unwrap().is_empty());
    }
}

#[test]
fn attachments_store_files() {
    for mut store in stores() {
        store.put_parts("u", vec![part("A")]).unwrap();
        let p = store.attach("u", "A", "sheet.pdf", None, b"%PDF-1".to_vec()).unwrap().unwrap();
        assert_eq!(p.attachments.len(), 1);
        let a = p.attachments[0].clone();
        assert_eq!((a.name.as_str(), a.kind, a.size), ("sheet.pdf", AttachmentKind::Datasheet, 6));
        assert_eq!(store.file(&a.id).unwrap().unwrap().data, b"%PDF-1");
        // Same name: replaced.
        let p = store.attach("u", "A", "sheet.pdf", Some(AttachmentKind::Manual), b"%PDF-2".to_vec()).unwrap().unwrap();
        assert_eq!(p.attachments.len(), 1);
        assert_eq!(p.attachments[0].kind, AttachmentKind::Manual);
        assert!(store.attach("u", "NOPE", "x.pdf", None, vec![1]).unwrap().is_none());
        let p = store.detach("u", "A", &p.attachments[0].id).unwrap().unwrap();
        assert!(p.attachments.is_empty());
        assert!(store.detach("u", "A", "unknown").unwrap().is_none());
        // Files of older versions stay available.
        assert!(store.file(&a.id).unwrap().is_some());
        assert!(store.file("unknown").unwrap().is_none());
        assert_eq!(store.history("A").unwrap().len(), 4);
    }
}

#[test]
fn files_are_removed_when_nothing_refers_to_them() {
    for mut store in stores() {
        store.put_parts("u", vec![part("A"), part("B")]).unwrap();
        let a = store.attach("u", "A", "a.pdf", None, b"only A".to_vec()).unwrap().unwrap().attachments[0].clone();
        // Same content with two parts: one file.
        let shared = store.attach("u", "A", "s.pdf", None, b"shared".to_vec()).unwrap().unwrap().attachments[1].id.clone();
        store.attach("u", "B", "s.pdf", None, b"shared".to_vec()).unwrap();
        store.detach("u", "A", &a.id).unwrap();
        // Older versions of A in the history still need the file.
        assert!(store.file(&a.id).unwrap().is_some());
        for i in 0..HISTORY_LIMIT {
            let mut p = store.get("A").unwrap().unwrap();
            p.note = i.to_string();
            store.put_parts("u", vec![p]).unwrap();
        }
        assert!(store.file(&a.id).unwrap().is_none(), "no version of A uses the file any more");
        // B still uses the shared file after A drops it.
        let mut p = store.get("A").unwrap().unwrap();
        p.attachments.clear();
        store.put_parts("u", vec![p]).unwrap();
        assert!(store.file(&shared).unwrap().is_some());
        assert_eq!(store.collect_garbage().unwrap(), 0);
    }
}

#[test]
fn garbage_collection_removes_left_over_files() {
    for mut store in stores() {
        store.put_parts("u", vec![part("A")]).unwrap();
        let used = store.attach("u", "A", "a.pdf", None, b"used".to_vec()).unwrap().unwrap().attachments[0].id.clone();
        let orphan = StoredFile { id: sha256_hex(b"orphan"), name: "o.pdf".into(), data: b"orphan".to_vec() };
        store.backend.commit(Change { revision: store.revision, files: vec![orphan.clone()], ..Default::default() }).unwrap();
        assert!(store.file(&orphan.id).unwrap().is_some());
        assert_eq!(store.collect_garbage().unwrap(), 1);
        assert!(store.file(&orphan.id).unwrap().is_none());
        assert!(store.file(&used).unwrap().is_some());
    }
}

#[test]
fn migrate_copies_everything() {
    let mut store = Store::with_backend(Box::new(JsonBackend::in_memory()), vec![part("A")]).unwrap();
    store.put_catalog(Catalog { id: "c".into(), ..Default::default() }).unwrap();
    store.attach("u", "A", "a.pdf", None, b"data".to_vec()).unwrap();
    let mut account = Account { user: User { name: "ann".into(), ..Default::default() }, ..Default::default() };
    let token = account.start_session();
    store.put_account(account).unwrap();
    store.set_favorites("ann", vec!["A".into()]).unwrap();
    let from = store.into_backend();
    let mut to = SqliteBackend::in_memory().unwrap();
    assert_eq!(migrate(from.as_ref(), &mut to).unwrap(), 1);
    let copy = Store::with_backend(Box::new(to), Vec::new()).unwrap();
    assert_eq!(copy.revision, from.revision());
    assert_eq!(copy.get("A").unwrap(), from.part("A").unwrap());
    assert_eq!(copy.history("A").unwrap().len(), 1);
    assert_eq!(copy.catalogs().unwrap().len(), 1);
    assert!(copy.session(&token).is_some());
    assert_eq!(copy.favorites("ann").unwrap(), ["A"]);
    let id = &copy.get("A").unwrap().unwrap().attachments[0].id;
    assert_eq!(copy.file(id).unwrap().unwrap().data, b"data");
}

#[test]
fn storage_kind_from_the_file_name() {
    let p = std::path::Path::new;
    assert_eq!(StorageKind::resolve(None, p("db/parts-db.json")), StorageKind::Json);
    assert_eq!(StorageKind::resolve(None, p("parts.JSON")), StorageKind::Json);
    assert_eq!(StorageKind::resolve(None, p("parts-db.sqlite")), StorageKind::Sqlite);
    assert_eq!(StorageKind::resolve(Some(StorageKind::Json), p("parts.db")), StorageKind::Json);
}

fn bundle(version: &str, parts: Vec<Part>) -> Bundle {
    let catalog = Catalog { id: "acme".into(), name: "ACME".into(), manufacturer: "ACME".into(), version: version.into(), ..Default::default() };
    Bundle { parts: parts.into_iter().map(|p| Part { catalog: "acme".into(), ..p }).collect(), catalog, logo: None }
}

#[test]
fn catalogs_keep_their_logos() {
    for mut store in stores() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"/>"#;
        let with_logo = Bundle { logo: Some(svg), ..bundle("1", vec![part("ACME.1")]) };
        store.install(with_logo.clone()).unwrap();
        let logo = store.catalog("acme").unwrap().unwrap().logo.unwrap();
        assert_eq!((logo.name.as_str(), logo.size), ("acme.svg", svg.len() as u64));
        assert_eq!(store.file(&logo.id).unwrap().unwrap().data, svg.as_bytes());
        // Saving the catalog keeps the logo, the clean-up of files too.
        let c = store.put_catalog(Catalog { description: "changed".into(), logo: None, ..store.catalog("acme").unwrap().unwrap() }).unwrap();
        assert_eq!(c.logo.as_ref(), Some(&logo));
        assert_eq!(store.collect_garbage().unwrap(), 0);
        assert!(store.file(&logo.id).unwrap().is_some());

        // An own logo stays when the bundle is installed again; a removed one comes back.
        let png = b"\x89PNG own".to_vec();
        let own = store.set_logo("ACME", Some(("own.png", png.clone()))).unwrap().unwrap().logo.unwrap();
        assert!(store.file(&logo.id).unwrap().is_none(), "the old file is removed");
        store.install(Bundle { catalog: Catalog { version: "2".into(), ..with_logo.catalog.clone() }, ..with_logo.clone() }).unwrap();
        assert_eq!(store.catalog("acme").unwrap().unwrap().logo, Some(own.clone()));
        assert!(store.set_logo("acme", None).unwrap().unwrap().logo.is_none());
        assert!(store.file(&own.id).unwrap().is_none());
        store.install(with_logo).unwrap();
        assert_eq!(store.catalog("acme").unwrap().unwrap().logo.unwrap().id, logo.id);
        assert!(store.set_logo("nope", None).unwrap().is_none());

        // Migrations and backups take the logo along.
        let mut target = SqliteBackend::in_memory().unwrap();
        migrate(store.backend.as_ref(), &mut target).unwrap();
        assert_eq!(target.file(&logo.id).unwrap().unwrap().data, svg.as_bytes());
        // Deleting the catalog deletes its logo.
        store.delete("x", "ACME.1").unwrap();
        store.delete_catalog("acme").unwrap();
        assert!(store.file(&logo.id).unwrap().is_none());
    }
}

#[test]
fn bundled_catalogs_install_and_update() {
    for mut store in stores() {
        let first = bundle("1", vec![part("ACME.1"), part("ACME.2"), part("ACME.3")]);
        assert_eq!(store.install(first.clone()).unwrap(), Installed { added: 3, ..Default::default() });
        assert_eq!(store.catalog("acme").unwrap().unwrap().parts, 3);
        assert_eq!(store.history("ACME.1").unwrap()[0].user, bundled::INSTALLER);
        // Installing again changes nothing.
        let rev = store.revision;
        assert_eq!(store.install(first).unwrap(), Installed::default());
        assert_eq!(store.revision, rev);

        // A user changes one part and deletes another; a part of the example catalog
        // has the number of a bundled part.
        store.put_parts("alice", vec![Part { note: "own".into(), ..store.get("ACME.1").unwrap().unwrap() }]).unwrap();
        store.delete("alice", "ACME.2").unwrap();
        let newer = |p: Part| Part { description: "new".into(), ..p };
        let next = bundle("2", vec![newer(part("ACME.1")), newer(part("ACME.2")), newer(part("ACME.3")), part("ACME.4"), newer(part("GEN.CT-9A-24DC"))]);
        let result = store.install(next).unwrap();
        assert_eq!((result.added, result.updated), (1, 1));
        assert_eq!(result.kept, ["ACME.1", "ACME.2", "GEN.CT-9A-24DC"]);
        assert_eq!(store.get("ACME.1").unwrap().unwrap().note, "own");
        assert!(store.get("ACME.2").unwrap().is_none());
        assert_eq!(store.get("ACME.3").unwrap().unwrap().description, "new");
        assert_ne!(store.get("GEN.CT-9A-24DC").unwrap().unwrap().description, "new");
        assert_eq!(store.catalog("acme").unwrap().unwrap().version, "2");
    }
}

#[test]
fn new_databases_get_the_bundled_catalogs() {
    let path = temp_file("bundled.sqlite");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let store = Store::open(StorageKind::Sqlite, &path).unwrap();
    let bundles = bundled::all().unwrap();
    let count = default_catalog().len() + bundles.iter().map(|b| b.parts.len()).sum::<usize>();
    assert_eq!(store.count().unwrap(), count);
    assert!(bundles.iter().all(|b| store.catalog(&b.catalog.id).unwrap().is_some_and(|c| c.parts == b.parts.len())));
    assert!(bundles.iter().all(|b| store.catalog(&b.catalog.id).unwrap().is_some_and(|c| c.logo.is_some())), "with their logos");
    let rev = store.revision;
    drop(store);
    // Opening it again installs nothing.
    assert_eq!(Store::open(StorageKind::Sqlite, &path).unwrap().revision, rev);
    std::fs::remove_dir_all(path.parent().unwrap()).ok();
}

#[test]
fn libraries_are_stored_by_name() {
    for mut store in stores() {
        let revision = store.revision;
        assert_eq!(store.library("forms").unwrap(), None);
        store.put_library("forms", r#"{"title_blocks":[{"name":"Company"}]}"#.into()).unwrap();
        store.put_library("forms", r#"{"title_blocks":[{"name":"Company 2"}]}"#.into()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&store.library("forms").unwrap().unwrap()).unwrap();
        assert_eq!(v["title_blocks"][0]["name"], "Company 2");
        assert_eq!(store.library("other").unwrap(), None);
        assert_eq!(store.revision, revision, "libraries do not change the parts revision");
    }
    // The JSON file keeps them.
    let path = temp_file("libraries.json");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut store = Store::open(StorageKind::Json, &path).unwrap();
    store.put_library("forms", r#"{"frames":[]}"#.into()).unwrap();
    drop(store);
    let store = Store::open(StorageKind::Json, &path).unwrap();
    assert_eq!(store.library("forms").unwrap().as_deref(), Some(r#"{"frames":[]}"#));
}
