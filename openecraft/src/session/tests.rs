use super::*;
use openecraft_core::model::Page;
use openecraft_core::sample::demo_project;

#[test]
fn a_new_session_starts_clean_on_the_first_schematic_page() {
    let s = Session::new(demo_project(), Some("demo.oec".into()));
    assert!(!s.dirty());
    assert_eq!(s.prj.pages[s.page].kind, PageKind::Schematic);
    assert_eq!(s.file_name.as_deref(), Some("demo.oec"));
}

#[test]
fn edits_make_the_session_dirty_until_saved_or_undone() {
    let mut s = Session::new(demo_project(), None);
    s.prj.info.name = "Changed".into();
    s.commit(false);
    assert!(!s.dirty(), "no commit while an edit is in progress");
    s.commit(true);
    assert!(s.dirty());
    // Undo back to the saved state leaves no changes, redo brings them back.
    s.undo();
    assert!(!s.dirty() && s.history.can_redo());
    s.redo();
    assert!(s.dirty());
    assert_eq!(s.prj.info.name, "Changed");
    s.mark_saved();
    assert!(!s.dirty());
    s.undo();
    assert!(s.dirty(), "the saved file holds the change undone");
    s.redo();
    assert!(!s.dirty());
}

#[test]
fn a_commit_is_reported_so_the_shell_repaints() {
    let mut s = Session::new(demo_project(), None);
    assert!(!s.commit(true), "nothing changed");
    s.prj.info.name = "Changed".into();
    assert!(!s.commit(false));
    assert!(s.commit(true));
    assert!(!s.commit(true));
}

#[test]
fn a_project_restored_with_unsaved_changes_stays_dirty_until_saved() {
    let mut s = Session::new(demo_project(), Some("plant.oec".into()));
    assert!(s.untouched());
    s.mark_unsaved();
    assert!(s.dirty() && s.untouched());
    s.prj.info.name = "Changed".into();
    s.commit(true);
    s.undo();
    assert!(s.dirty(), "no state of the history is the file's");
    assert!(!s.untouched());
    s.mark_saved();
    assert!(!s.dirty());
}

#[test]
fn pages_with_contents_ask_and_released_or_locked_ones_cannot_be_deleted() {
    let mut s = Session::new(demo_project(), None);
    let full = s.prj.pages.iter().position(|p| !p.symbols.is_empty()).unwrap();
    assert!(s.page_items(full) > 0);
    assert!(s.may_delete_page(full, false));
    assert!(!s.may_delete_page(full, true), "locked by another user");
    s.prj.pages[full].locked = true;
    assert!(!s.may_delete_page(full, false), "released");
    let empty = Project { pages: vec![Page::new(1, "1", "", PageKind::Schematic), Page::new(2, "2", "", PageKind::Schematic)], ..Default::default() };
    let s = Session::new(empty, None);
    assert_eq!(s.page_items(1), 0, "an empty page is deleted without asking");
    assert!(s.may_delete_page(1, false));
    let mut one = s.prj.clone();
    one.pages.truncate(1);
    assert!(!Session::new(one, None).may_delete_page(0, false), "the last page stays");
}

#[test]
fn the_guard_asks_only_with_unsaved_changes() {
    let mut g = Guard::default();
    assert_eq!(g.request(false, "open"), Some("open"));
    assert_eq!(g.pending(), None);
    assert_eq!(g.request(true, "open"), None);
    assert_eq!(g.pending(), Some(&"open"));
    g.cancel();
    assert_eq!(g.pending(), None);
    g.request(true, "close");
    assert_eq!(g.discard(), Some("close"));
    assert_eq!(g.pending(), None);
}

#[test]
fn save_in_the_guard_runs_the_action_after_a_successful_save() {
    let mut g = Guard::default();
    g.request(true, "new");
    g.save();
    assert!(g.saving());
    // A save on a server: nothing happens while it runs.
    assert_eq!(g.after_save(true, true), None);
    assert!(g.saving() && g.pending().is_some());
    assert_eq!(g.after_save(false, false), Some("new"));
    assert!(!g.saving() && g.pending().is_none());
    assert_eq!(g.after_save(false, false), None, "runs once");
}

#[test]
fn a_cancelled_or_failed_save_keeps_the_question() {
    let mut g = Guard::default();
    g.request(true, "open");
    g.save();
    assert_eq!(g.after_save(true, false), None);
    assert!(g.not_saved && !g.saving());
    assert_eq!(g.pending(), Some(&"open"), "the dialog stays open");
    // Without Save chosen a clean project runs nothing on its own.
    assert_eq!(g.after_save(false, false), None);
    assert_eq!(g.discard(), Some("open"));
}

#[test]
fn replacing_the_project_starts_a_new_history() {
    let mut s = Session::new(demo_project(), Some("a.oec".into()));
    s.page = 3;
    s.prj.info.name = "Changed, then discarded".into();
    s.commit(true);
    let rev = s.history.rev;
    s.replace(Project::default(), None, None);
    assert_eq!((s.page, s.file_name.as_deref(), s.dirty()), (0, None, false));
    // Undo would put the discarded project under the new name (and saving would
    // overwrite the new file with it).
    assert!(!s.history.can_undo());
    s.undo();
    assert_eq!(s.prj.info.name, Project::default().info.name);
    assert!(s.history.rev > rev, "the analysis is built again");
}

#[test]
fn refresh_follows_the_history_and_the_plugins() {
    let plugins = Plugins::new(Vec::<String>::new());
    let mut s = Session::new(demo_project(), None);
    s.refresh(&plugins, false);
    opened(&mut s, &plugins);
    let devices = s.index.devices.len();
    assert!(devices > 0);
    assert!(s.analysis.reports().len() > 40, "the analysis knows the registered reports");
    let issues = s.analysis.issues.len();
    let page = s.page;
    s.prj.pages[page].symbols.clear();
    s.refresh(&plugins, false);
    assert_eq!((s.analysis.issues.len(), s.index.devices.len()), (issues, devices), "nothing is rebuilt before the change is committed");
    s.refresh(&plugins, true);
    assert!(s.index.devices.len() < devices, "the index follows every frame while editing");
    assert_eq!(s.analysis.issues.len(), issues);
    s.commit(true);
    s.refresh(&plugins, false);
    assert!(s.analysis.index.devices.len() < devices, "the analysis follows the committed change");
    let rev = s.history.rev;
    s.invalidate();
    s.refresh(&plugins, false);
    assert_eq!(s.history.rev, rev, "a rebuild is no change");
}

#[test]
fn released_generated_and_locked_pages_cannot_be_edited() {
    let mut s = Session::new(demo_project(), None);
    let page = s.prj.pages.iter().position(|p| p.kind == PageKind::Schematic).unwrap();
    assert!(s.may_edit_page(page, false));
    assert!(!s.may_edit_page(page, true), "locked by another user or a viewer");
    s.prj.pages[page].locked = true;
    assert!(!s.may_edit_page(page, false), "released");
    s.prj.pages.push(Page::new(999, "99", "", PageKind::Mounting(1)));
    assert!(!s.may_edit_page(s.prj.pages.len() - 1, false), "generated");
    assert!(!s.may_edit_page(s.prj.pages.len(), false), "no such page");
}

/// Refreshes until the analysis built in the background is taken.
fn opened(s: &mut Session, plugins: &Plugins) {
    let start = std::time::Instant::now();
    while s.opening().is_some() {
        assert!(start.elapsed().as_secs() < 120, "the analysis is not ready");
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.refresh(plugins, false);
    }
}

#[test]
fn an_opened_project_is_analysed_in_the_background() {
    let plugins = Plugins::new(Vec::<String>::new());
    // The project the app starts with too (restored from the last session).
    let mut s = Session::new(demo_project(), None);
    s.refresh(&plugins, false);
    if openecraft_core::analysis::threads() > 1 {
        assert!(s.opening().is_some(), "the start project is analysed in the background");
    }
    opened(&mut s, &plugins);
    assert!(!s.analysis.index.devices.is_empty());
    s.replace(demo_project(), Some("demo.oec".into()), None);
    s.refresh(&plugins, false);
    if openecraft_core::analysis::threads() > 1 {
        assert!(s.opening().is_some());
    }
    opened(&mut s, &plugins);
    let devices = s.analysis.index.devices.len();
    assert!(devices > 0 && devices == Index::build(&s.prj).devices.len());
    assert!(!s.dirty());
    // Edits are analysed at once again.
    s.prj.info.name = "Changed".into();
    s.commit(true);
    s.refresh(&plugins, false);
    assert!(s.opening().is_none());
}

#[test]
fn a_project_replaced_while_opening_gets_its_own_analysis() {
    let plugins = Plugins::new(Vec::<String>::new());
    let mut s = Session::new(Project::new_default(), None);
    s.replace(demo_project(), None, None);
    s.refresh(&plugins, false);
    s.replace(Project::new_default(), None, None);
    s.refresh(&plugins, false);
    opened(&mut s, &plugins);
    assert_eq!(s.analysis.index.devices.len(), Index::build(&s.prj).devices.len());
}

#[test]
fn edits_of_the_layout_are_routed_in_the_background() {
    if analysis::threads() < 2 {
        return;
    }
    let plugins = Plugins::new(Vec::<String>::new());
    let mut s = Session::new(demo_project(), None);
    s.refresh(&plugins, false);
    opened(&mut s, &plugins);
    let before = s.analysis.routing.clone();
    assert!(!before.items.is_empty() && !s.routing());
    // A text changes no route: the routing is taken at once.
    let page = s.prj.pages.iter().position(|p| !p.texts.is_empty()).unwrap();
    s.prj.pages[page].texts[0].text.push_str(" (new)");
    s.commit(true);
    s.refresh(&plugins, false);
    assert!(!s.analysis.routes_pending && !s.routing());
    assert_eq!(s.analysis.routing, before);
    // A device moved in the layout: the analysis follows at once, the routes when
    // they are found.
    let ei = s.prj.layout.enclosures.iter().position(|e| e.items.iter().any(|i| i.tag == "-K1")).unwrap();
    s.prj.layout.enclosures[ei].items.iter_mut().find(|i| i.tag == "-K1").unwrap().x += 25.0;
    s.commit(true);
    s.refresh(&plugins, false);
    assert!(s.analysis.routes_pending && s.routing());
    assert!(s.analysis.routing.items.iter().all(|i| i.kind != openecraft_core::routing3d::RouteKind::Wire), "wires wait for their routes");
    let start = std::time::Instant::now();
    while s.routing() || s.analysis.routes_pending {
        assert!(start.elapsed().as_secs() < 120, "the routing is not ready");
        std::thread::sleep(std::time::Duration::from_millis(5));
        s.refresh(&plugins, false);
    }
    assert_eq!(s.analysis.routing, Analysis::build(&s.prj).routing, "as routed at once");
    assert_ne!(s.analysis.routing, before);
}

#[test]
fn a_snapshot_analyses_like_the_session() {
    let plugins = Plugins::new(Vec::<String>::new());
    let mut s = Session::new(demo_project(), None);
    s.refresh(&plugins, false);
    opened(&mut s, &plugins);
    let mut snapshot = s.snapshot(&plugins);
    // On another thread, as an export does.
    let an = std::thread::spawn(move || {
        let an = snapshot.analyse();
        (an.issues.len(), an.reports().len(), an.routing)
    })
    .join()
    .unwrap();
    assert_eq!(an, (s.analysis.issues.len(), s.analysis.reports().len(), s.analysis.routing.clone()));
}

#[test]
fn a_device_moved_into_a_structure_box_takes_its_structure_in_the_same_undo_step() {
    use openecraft_core::geom::p;
    use openecraft_core::structure::Dt;
    let mut prj = Project::default();
    prj.pages[0].location = "CAB1".into();
    let b = openecraft_core::structure_boxes::new_box(&mut prj, p(100.0, 100.0), p(200.0, 200.0), Dt::parse_structure("+FIELD"), "");
    prj.pages[0].structure_boxes.push(b);
    let id = prj.new_id();
    prj.pages[0].symbols.push(openecraft_core::model::SymbolInst { id, sym: "coil".into(), pos: p(20.0, 20.0), tag: "+CAB1-K1".into(), ..Default::default() });
    prj.pages[0].symbols.push(openecraft_core::model::SymbolInst { id: id + 100, sym: "coil".into(), pos: p(150.0, 150.0), tag: "+FIELD-K2".into(), ..Default::default() });
    let mut s = Session::new(prj, None);
    let start = s.prj.clone();
    // Dragging: nothing follows before the gesture ends.
    s.prj.pages[0].symbols[0].pos = p(150.0, 120.0);
    s.commit(false);
    assert_eq!(s.prj.pages[0].symbols[0].tag, "+CAB1-K1");
    assert!(s.commit(true));
    assert_eq!(s.prj.pages[0].symbols[0].tag, "+FIELD-K1");
    s.undo();
    assert_eq!(s.prj, start);
    // A tag in use is kept and reported once.
    s.prj.pages[0].symbols[0].tag = "+CAB1-K2".into();
    s.commit(true);
    s.prj.pages[0].symbols[0].pos = p(150.0, 120.0);
    s.commit(true);
    assert_eq!(s.prj.pages[0].symbols[0].tag, "+CAB1-K2");
    assert_eq!(std::mem::take(&mut s.structure_kept).len(), 1);
    s.commit(true);
    assert!(s.structure_kept.is_empty());
}
