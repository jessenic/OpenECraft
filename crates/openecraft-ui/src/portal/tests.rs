use super::*;
use openecraft_parts::api::Session;
use openecraft_parts::portal::{FacetValue, UpdateKind, User};
use openecraft_parts::TopGroup;

fn result(total: usize, numbers: &[&str]) -> SearchResult {
    let parts = numbers.iter().map(|n| Part { number: n.to_string(), ..Default::default() }).collect();
    SearchResult { total, parts, facets: vec![Facet { name: "group".into(), values: vec![FacetValue { value: "Relays, contactors".into(), count: 2 }] }], ..Default::default() }
}

#[test]
fn searches_once_until_something_changes() {
    let mut b = Browser::new();
    b.text = "  contactor ".into();
    let q = b.take_search().unwrap();
    assert_eq!((q.q.as_deref(), q.brief, q.limit), (Some("contactor"), true, Some(PAGE)));
    assert!(b.take_search().is_none());
    b.toggle_facet("manufacturer", "ACME");
    assert_eq!(b.take_search().unwrap().manufacturer.as_deref(), Some("ACME"));
}

#[test]
fn facets_toggle_and_clear() {
    let mut b = Browser::new();
    b.query.offset = Some(100);
    b.toggle_facet("top_group", "Fluid");
    assert_eq!(b.query.top_group, Some(TopGroup::Fluid));
    assert_eq!(b.query.offset, None, "a new filter starts on the first page");
    assert_eq!(b.facet_filter("top_group").as_deref(), Some("Fluid"));
    b.toggle_facet("top_group", "Fluid");
    assert_eq!(b.query.top_group, None);
    b.toggle_facet("lifecycle", "Discontinued");
    assert_eq!(b.query.lifecycle, Some(Lifecycle::Discontinued));
    b.toggle_attribute("Poles", "3");
    b.toggle_attribute("poles", "4");
    assert_eq!(b.query.attributes, [("poles".to_string(), "4".to_string())]);
    assert_eq!(b.attribute_filter("POLES"), Some("4"));
    b.toggle_attribute("Poles", "4");
    assert!(b.query.attributes.is_empty());
    b.query.sort = Sort::Price;
    assert!(b.has_filters());
    b.clear_filters();
    assert!(!b.has_filters());
    assert_eq!(b.query.sort, Sort::Price, "the order is kept");
}

#[test]
fn pages() {
    let mut b = Browser::new();
    b.result = Some(result(120, &[]));
    assert_eq!(b.page(), (1, 3));
    b.go_to_page(3);
    assert_eq!(b.query.offset, Some(2 * PAGE));
    assert_eq!(b.page(), (3, 3));
    b.go_to_page(9);
    assert_eq!(b.page(), (3, 3));
    b.result = Some(result(0, &[]));
    assert_eq!(b.page().1, 1);
}

#[test]
fn favorites() {
    let mut b = Browser::new();
    b.take_search();
    b.apply(&Reply::Favorites(Ok(vec!["A".into()])), &[]);
    assert!(b.is_favorite("A"));
    assert_eq!(b.toggled_favorite("B"), ["A", "B"]);
    assert!(b.toggled_favorite("A").is_empty());
    b.only_favorites = true;
    assert_eq!(b.search_query().numbers, ["A"]);
    b.apply(&Reply::Favorites(Ok(Vec::new())), &[]);
    assert_eq!(b.search_query().numbers, [""], "no favorites: no parts");
    assert!(b.take_search().is_some(), "changed favorites refresh the list");
    b.apply(&Reply::SignedOut, &[]);
    assert!(!b.only_favorites && b.favorites.is_empty());
}

#[test]
fn replies_update_results_history_and_updates() {
    let mut b = Browser::new();
    b.selected = Some("GONE".into());
    b.apply(&Reply::Search(Ok(result(2, &["A", "B"]))), &[]);
    assert_eq!(b.selected, None);
    b.selected = Some("B".into());
    assert_eq!(b.selected_part().unwrap().number, "B");
    b.apply(&Reply::History("B".into(), Ok(vec![HistoryEntry::default()])), &[]);
    assert_eq!(b.history.as_ref().unwrap().0, "B");

    let local = vec![Part { number: "A".into(), revision: 1, ..Default::default() }];
    let remote = vec![Part { number: "A".into(), revision: 2, description: "new".into(), ..Default::default() }];
    b.apply(&Reply::Versions(Ok(SearchResult { parts: remote.clone(), ..Default::default() })), &local);
    assert_eq!(b.updates.as_ref().unwrap()[0].kind, UpdateKind::Changed);
    assert_eq!(b.remote, remote);
    b.updates_applied();
    assert!(b.updates.is_none() && b.remote.is_empty());

    b.password = "secret".into();
    b.apply(&Reply::SignedIn(Ok(Session { token: "t".into(), user: User::default(), ..Default::default() })), &[]);
    assert!(b.password.is_empty(), "the password is not kept");
}

#[test]
fn values_are_shown_translated() {
    assert_eq!(value_text("top_group", "Fluid"), tr("Fluid power"));
    assert_eq!(value_text("lifecycle", "PhaseOut"), tr("Phase-out"));
    assert_eq!(value_text("group", "relays, contactors"), tr("Relays, contactors"));
    assert_eq!(value_text("group", "own group"), "own group");
    assert_eq!(value_text("manufacturer", "ACME"), "ACME");
}

#[test]
fn the_details_load_the_whole_part_once() {
    let mut b = Browser::new();
    b.apply(&Reply::Search(Ok(result(2, &["A", "B"]))), &[]);
    assert_eq!(b.detail_due(), None, "nothing selected");
    b.selected = Some("A".into());
    assert_eq!(b.detail_due().as_deref(), Some("A"));
    assert_eq!(b.detail_due(), None, "asked for once");
    assert!(b.selected_detail().is_none());
    let mesh = openecraft_parts::Mesh3d { vertices: vec![[0.0; 3]], ..Default::default() };
    let full = Part { number: "A".into(), model_3d: Some(mesh.into()), ..Default::default() };
    // A late answer for another part is dropped.
    b.apply(&Reply::Detail("B".into(), Ok(Some(Box::new(full.clone())))), &[]);
    assert!(b.selected_detail().is_none());
    b.apply(&Reply::Detail("A".into(), Ok(Some(Box::new(full)))), &[]);
    assert!(b.selected_detail().is_some_and(|p| p.model_3d.is_some()));
    b.selected = Some("B".into());
    assert!(b.selected_detail().is_none());
    assert_eq!(b.detail_due().as_deref(), Some("B"));
}

#[test]
fn accessories_belong_to_the_selected_part() {
    let mut b = Browser::new();
    b.apply(&Reply::Search(Ok(result(2, &["HAG.CDA440D", "HAG.CZ001"]))), &[]);
    b.selected = Some("HAG.CDA440D".into());
    let acc = Accessories { accessories: vec![Part { number: "HAG.CZ001".into(), ..Default::default() }], fits: Vec::new() };
    b.apply(&Reply::Accessories("HAG.CDA440D".into(), Ok(acc)), &[]);
    assert_eq!(b.selected_accessories().map(|a| a.accessories.len()), Some(1));
    // Following an accessory searches for it and shows it.
    b.show_part("HAG.CZ001");
    assert_eq!((b.text.as_str(), b.selected.as_deref()), ("HAG.CZ001", Some("HAG.CZ001")));
    assert!(b.selected_accessories().is_none());
}

#[test]
fn the_browser_fits_its_window_instead_of_growing_it() {
    // A window of the size the user left it, with the update list under the buttons:
    // the columns take the height left, so the window keeps its size frame by frame.
    let ctx = egui::Context::default();
    let mut b = Browser { result: Some(result(2, &["A.1", "A.2"])), ..Default::default() };
    b.updates = Some((0..8).map(|i| PartUpdate { number: format!("U.{i}"), kind: UpdateKind::Changed, revision: 2, successor: String::new() }).collect());
    let mut client = ServerClient::default();
    let mut heights = Vec::new();
    for _ in 0..6 {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1200.0))), ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| {
            let r = egui::Window::new("portal").default_size([900.0, 420.0]).max_size([1500.0, 1100.0]).show(ui.ctx(), |ui| {
                browser(ui, &mut b, &mut client, &[]);
            });
            heights.push(r.map(|r| r.response.rect.height()).unwrap_or_default());
        });
        out.textures_delta.clear();
    }
    let (first, last) = (heights[1], *heights.last().unwrap());
    assert!(last <= first + 1.0, "the window grew: {heights:?}");
}

#[test]
fn catalogs_and_logos_load_while_logos_are_shown() {
    use openecraft_parts::portal::{Attachment, Catalog};
    let mut b = Browser::new();
    assert!(b.show_logos, "shown by default");
    assert!(b.take_catalogs());
    assert!(!b.take_catalogs(), "once per server");
    let logo = Attachment { id: "f1".into(), name: "acme.svg".into(), ..Default::default() };
    b.apply(&Reply::Catalogs(Ok(vec![Catalog { id: "acme".into(), name: "ACME".into(), logo: Some(logo), ..Default::default() }])), &[]);
    assert_eq!(b.catalog("ACME ").map(|c| c.name.as_str()), Some("ACME"));
    b.apply(&Reply::Logo("f1".into(), "acme.svg".into(), Ok(b"<svg/>".to_vec())), &[]);
    assert!(b.logos.loaded("f1"));
    // Connecting again loads them again (another server, or changed catalogs).
    b.apply(&Reply::Info(Ok(Default::default())), &[]);
    assert!(b.catalogs.is_empty() && !b.logos.loaded("f1"));
    b.show_logos = false;
    assert!(!b.take_catalogs(), "nothing is loaded while logos are hidden");
    b.show_logos = true;
    assert!(b.take_catalogs());
}
