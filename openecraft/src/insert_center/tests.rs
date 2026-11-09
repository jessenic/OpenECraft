use super::*;

#[test]
fn catalog_scrolling_reuses_groups_and_edits_refresh_search_results() {
    use openecraft_core::{parts::Part, symlib::UserSymbol};
    let mut prj = Project::default();
    let mut db = PartsDb { parts: vec![] };
    prj.symbols.push(UserSymbol { key: "user:scroll".into(), name: "Scroll relay".into(), library: "Scroll library".into(), ..Default::default() });
    let mut center = InsertCenter { query: "Scroll".into(), ..Default::default() };
    let mut cache = CatalogCache::default();
    let initial = cache.get(&prj, &db, &center);
    assert_eq!(initial.len(), 1);
    assert_eq!(initial[0].entries, [Entry::Symbol("user:scroll".into())]);
    assert!(Arc::ptr_eq(&initial, &cache.get(&prj, &db, &center)));
    prj.info.name = "Drawing edit does not change the symbol catalog".into();
    assert!(Arc::ptr_eq(&initial, &cache.get(&prj, &db, &center)));
    // A same-length definition edit must invalidate the cache too.
    prj.symbols[0].name = "Renamed contact".into();
    center.query = "relay".into();
    assert!(!cache.get(&prj, &db, &center).iter().any(|g| g.entries.contains(&Entry::Symbol("user:scroll".into()))));
    center.query = "Renamed".into();
    let renamed = cache.get(&prj, &db, &center);
    assert_eq!(renamed[0].entries, [Entry::Symbol("user:scroll".into())]);
    prj.parts.push(Part { number: "local".into(), ..Default::default() });
    let local = cache.get(&prj, &db, &center);
    assert!(!Arc::ptr_eq(&local, &renamed));
    db.parts.push(Part { number: "global".into(), ..Default::default() });
    let catalog = cache.get(&prj, &db, &center);
    assert!(!Arc::ptr_eq(&catalog, &local));
    openecraft_i18n::with_lang(openecraft_i18n::Lang::De, || {
        assert!(!Arc::ptr_eq(&catalog, &cache.get(&prj, &db, &center)));
    });
    let replacement = Project::default();
    assert!(cache.get(&replacement, &db, &center).is_empty());
}

#[test]
fn unplaced_results_are_reused_and_invalidated_by_edits_undo_replacement_and_catalog() {
    use openecraft_core::{history::History, model::potentials::PotentialDef};
    let mut prj = Project::default();
    let mut history = History::new(&prj);
    let mut db = PartsDb { parts: vec![] };
    let mut cache = UnplacedCache::default();
    let original = cache.get(&prj, &db, history.rev).to_vec();
    let allocation = cache.entries.as_ptr();
    assert_eq!(cache.get(&prj, &db, history.rev).as_ptr(), allocation);
    prj.potentials.push(PotentialDef { name: "+48V".into(), ..Default::default() });
    history.track(&prj, true);
    assert!(cache.get(&prj, &db, history.rev).iter().any(|u| u.label == "+48V"));
    history.undo(&mut prj);
    assert_eq!(cache.get(&prj, &db, history.rev), original);
    history.redo(&mut prj);
    assert!(cache.get(&prj, &db, history.rev).iter().any(|u| u.label == "+48V"));
    // Session::replace advances the history revision even for a new project.
    assert_eq!(cache.get(&Project::default(), &db, history.rev + 1), original);
    prj.devices.insert("-K99".into(), openecraft_core::model::DeviceData {
        parts: vec![openecraft_core::parts::PartRef { number: "catalog-relay".into(), ..Default::default() }],
        ..Default::default()
    });
    history.track(&prj, true);
    assert!(!cache.get(&prj, &db, history.rev).iter().any(|u| u.tag == "-K99"));
    db.parts.push(openecraft_core::parts::Part {
        number: "catalog-relay".into(),
        functions: vec![openecraft_core::parts::FunctionTemplate::new(openecraft_core::parts::FnKind::No, "13 14")],
        ..Default::default()
    });
    assert!(cache.get(&prj, &db, history.rev).iter().any(|u| u.tag == "-K99" && u.pins == ["13", "14"]));
    assert_eq!(cache.key.as_ref().unwrap().2, db);
    openecraft_i18n::with_lang(openecraft_i18n::Lang::De, || {
        cache.get(&prj, &db, history.rev);
        assert_eq!(cache.key.as_ref().unwrap().1, openecraft_i18n::Lang::De);
    });
}

#[test]
fn card_variants_form_one_loose_entry_or_one_catalog_device() {
    use openecraft_core::{
        parts::Part,
        symbols::{Func, PlcChannel},
        symlib::UserSymbol,
    };
    let mut p = Project::default();
    let mut part = Part { number: "BEC.EL1008".into(), ..Default::default() };
    for i in 0..8 {
        let key = format!("user:ema:EL1008:1:{i}");
        let channel = PlcChannel {
            family: "EL1008".into(),
            input: true,
            index: i,
            designation: format!("Channel {}", i + 1),
            address: String::new(),
            function_text: String::new(),
            analog: None,
            digital_terminal: None,
        };
        p.symbols.push(UserSymbol { key: key.clone(), func: Func::Plc, plc_channel: Some(channel), ..Default::default() });
        part.eplan_variants.push(serde_json::json!({"symbol":{"key":key,"plc_channel":{}}}).into());
    }
    p.symbols.push(UserSymbol { key: "user:ema:EL1008:4:0".into(), ..Default::default() });
    part.eplan_variants.push(serde_json::json!({"symbol":{"key":"user:ema:EL1008:4:0"}}).into());
    let mut db = PartsDb { parts: vec![] };
    let loose = listed_user_symbols(&p, &db);
    assert_eq!(loose.len(), 1);
    assert_eq!(loose[0].key, "user:ema:EL1008:1:0");
    db.parts.push(part);
    assert!(listed_user_symbols(&p, &db).is_empty());
}

#[test]
fn keys_round_trip() {
    for e in [Entry::Symbol("coil".into()), Entry::Macro("Motor star-delta".into()), Entry::Part("3RT2016:1BB41".into())] {
        assert_eq!(Entry::from_key(&e.key()), Some(e));
    }
    assert_eq!(Entry::from_key("symbol:"), None);
    assert_eq!(Entry::from_key("rocket:x"), None);
    assert_eq!(Entry::from_key("nonsense"), None);
}

#[test]
fn recent_entries_come_first_and_are_limited() {
    let mut ic = InsertCenter::default();
    for i in 0..25 {
        ic.used(&Entry::Symbol(format!("s{i}")));
    }
    assert_eq!(ic.recent.len(), RECENT);
    assert_eq!(ic.recent[0], "symbol:s24");
    ic.used(&Entry::Symbol("s10".into()));
    assert_eq!(ic.recent[0], "symbol:s10");
    assert_eq!(ic.recent.iter().filter(|k| *k == "symbol:s10").count(), 1);
}

#[test]
fn favorites_toggle_and_are_exchanged() {
    let mut ic = InsertCenter::default();
    let coil = Entry::Symbol("coil".into());
    ic.toggle_favorite(&coil);
    assert!(ic.is_favorite(&coil));
    ic.toggle_favorite(&Entry::Macro("M1".into()));
    let json = ic.favorites_json();
    ic.toggle_favorite(&coil);
    assert!(!ic.is_favorite(&coil));
    assert_eq!(ic.favorites(), vec![Entry::Macro("M1".into())]);
    assert_eq!(ic.import_favorites(&json), Ok(1), "only the coil is new");
    assert!(ic.import_favorites("{").is_err());
}

#[test]
fn search_uses_all_words_and_placeholders() {
    let mut ic = InsertCenter { query: "con no".into(), ..Default::default() };
    assert!(ic.matches(&["Contact NO", "K"]));
    assert!(!ic.matches(&["Contact NC", "K"]));
    ic.query = "c?il".into();
    assert!(ic.matches(&["Relay coil"]));
    ic.query = String::new();
    assert!(ic.matches(&["anything"]));
}
