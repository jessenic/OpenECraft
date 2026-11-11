use super::*;

#[test]
fn the_default_trees_have_their_panes_once() {
    let tree = default_tree();
    for pane in [Pane::Page, Pane::Library, Pane::Properties, Pane::Navigator(LeftTab::Check)].into_iter().chain(NAVIGATORS) {
        assert!(find(&tree, pane).is_some(), "{pane:?}");
    }
    assert_eq!(active_tab(&default_tree()), Some(LeftTab::Pages));
    for w in Workspace::ALL {
        let tree = default_tree_for(w);
        let all = panes(&tree);
        assert!(!all.is_empty(), "{w:?}");
        for p in &all {
            assert_eq!(all.iter().filter(|q| *q == p).count(), 1, "{p:?} twice in {w:?}");
        }
        assert!(from_json(&to_json(&tree)).is_some(), "{w:?} can be saved and read");
    }
    assert!(find(&default_tree_for(Workspace::Panel), Pane::LayoutView).is_some());
    assert!(find(&default_tree_for(Workspace::Wiring), Pane::WiringWorkbench).is_some());
}

#[test]
fn every_pane_has_an_area_and_the_lists_are_complete() {
    let all = Pane::all();
    assert_eq!(all.len(), 1 + LeftTab::ALL.len() + 2 + 5 + ToolPane::ALL.len());
    for p in &all {
        assert_eq!(all.iter().filter(|q| *q == p).count(), 1, "{p:?}");
    }
    assert_eq!(Pane::Tool(ToolPane::Reports).area(), Area::Center);
    assert_eq!(Pane::Tool(ToolPane::Find).area(), Area::Right);
    assert_eq!(Pane::Navigator(LeftTab::Check).area(), Area::Bottom);
}

#[test]
fn showing_a_tab_activates_it_and_brings_closed_panes_back() {
    let mut tree = default_tree();
    show(&mut tree, Pane::Navigator(LeftTab::Devices));
    assert_eq!(active_tab(&tree), Some(LeftTab::Devices));

    close(&mut tree, Pane::Navigator(LeftTab::Review));
    close(&mut tree, Pane::Library);
    close(&mut tree, Pane::Page);
    assert!(find(&tree, Pane::Navigator(LeftTab::Review)).is_none());
    assert!(find(&tree, Pane::Library).is_none());
    assert!(find(&tree, Pane::Page).is_some(), "the only document cannot be closed");

    show(&mut tree, Pane::Navigator(LeftTab::Review));
    let review = find(&tree, Pane::Navigator(LeftTab::Review)).unwrap();
    let pages = find(&tree, Pane::Navigator(LeftTab::Pages)).unwrap();
    assert_eq!(tree.tiles.parent_of(review), tree.tiles.parent_of(pages), "back among the navigator tabs");
    assert_eq!(active_tab(&tree), Some(LeftTab::Review));
    show(&mut tree, Pane::Library);
    assert!(find(&tree, Pane::Library).is_some());
}

#[test]
fn tools_open_beside_the_page_which_never_closes() {
    let mut tree = default_tree();
    show(&mut tree, Pane::Tool(ToolPane::Reports));
    let reports = find(&tree, Pane::Tool(ToolPane::Reports)).unwrap();
    let page = find(&tree, Pane::Page).unwrap();
    assert_eq!(tree.tiles.parent_of(reports), tree.tiles.parent_of(page));
    assert!(visible(&tree).contains(&Pane::Tool(ToolPane::Reports)));
    assert!(!can_close(&tree, Pane::Page), "the page stays though another document is open");
    close(&mut tree, Pane::Page);
    assert!(find(&tree, Pane::Page).is_some());
    assert!(can_close(&tree, Pane::Tool(ToolPane::Reports)));
    assert!(!Pane::Page.has_close_button() && !Pane::Navigator(LeftTab::Cables).has_close_button());
    assert!(Pane::Library.has_close_button() && Pane::Tool(ToolPane::Reports).has_close_button());
    // The wiring workspace has no page: its last document stays.
    let wiring = default_tree_for(Workspace::Wiring);
    assert!(!can_close(&wiring, Pane::WiringWorkbench));
}

#[test]
fn every_pane_is_in_one_section_of_the_panes_menu() {
    let groups = menu_groups();
    let all: Vec<Pane> = groups.iter().flat_map(|(_, p)| p.iter().copied()).collect();
    for pane in Pane::all() {
        assert_eq!(all.iter().filter(|p| **p == pane).count(), 1, "{pane:?}");
    }
    assert_eq!(all.len(), Pane::all().len());
    // Short enough for a window 700 pixels high (about 20 pixels per entry).
    assert!(groups.len() <= 14);
    for (title, panes) in &groups {
        assert!(!panes.is_empty() && panes.len() <= 12, "{title}");
    }
}

#[test]
fn placement_and_floating_panes_belong_to_their_workspace() {
    let mut sides = Sides::default();
    let mut current = Side::default();
    current.placement.set(Pane::Tool(ToolPane::Reports), false);
    current.floating.push(Pane::Tool(ToolPane::Cables));
    sides.switch(Workspace::Schematic, Workspace::Panel, &mut current);
    assert_eq!(current, Side::default(), "nothing of the schematic workspace in the panel layout");
    current.floating.push(Pane::Tool(ToolPane::Parts));
    sides.switch(Workspace::Panel, Workspace::Schematic, &mut current);
    assert!(!current.placement.floats(Pane::Tool(ToolPane::Reports)));
    assert_eq!(current.floating, vec![Pane::Tool(ToolPane::Cables)]);
    // Reset layout resets the current workspace only.
    current = Side::default();
    sides.switch(Workspace::Schematic, Workspace::Panel, &mut current);
    assert_eq!(current.floating, vec![Pane::Tool(ToolPane::Parts)]);
    let json = serde_json::to_string(&sides).unwrap();
    let back: Sides = serde_json::from_str(&json).unwrap();
    assert_eq!(back.0.len(), sides.0.len());
}

#[test]
fn a_closed_navigator_comes_back_beside_the_page() {
    let mut tree = default_tree();
    for t in LeftTab::ALL {
        close(&mut tree, Pane::Navigator(t));
    }
    tree.simplify(&egui_tiles::SimplificationOptions { all_panes_must_have_tabs: true, ..Default::default() });
    show(&mut tree, Pane::Navigator(LeftTab::Devices));
    let devices = find(&tree, Pane::Navigator(LeftTab::Devices)).unwrap();
    let page = find(&tree, Pane::Page).unwrap();
    assert_eq!(tree.tiles.parent_of(devices), tree.tiles.parent_of(page));
}

#[test]
fn arrangements_are_saved_and_checked_when_read() {
    let mut tree = default_tree();
    close(&mut tree, Pane::Properties);
    let json = to_json(&tree);
    let back = from_json(&json).unwrap();
    assert!(find(&back, Pane::Properties).is_none());
    assert!(find(&back, Pane::Library).is_some());
    assert!(from_json("{").is_none());
    let mut no_document = default_tree();
    let page = find(&no_document, Pane::Page).unwrap();
    no_document.remove_recursively(page);
    assert!(from_json(&to_json(&no_document)).is_none());
    assert_eq!(LeftTab::from_key("plc"), Some(LeftTab::Plc));
}

#[test]
fn arrangements_saved_before_tools_were_panes_are_read() {
    // A tree of the earlier editor: navigator, page, library and properties.
    let mut tiles = Tiles::default();
    let nav = tabs(&mut tiles, &[Pane::Navigator(LeftTab::Pages)]);
    let page = tabs(&mut tiles, &[Pane::Page]);
    let side = tabs(&mut tiles, &[Pane::Library, Pane::Properties]);
    let root = linear(&mut tiles, LinearDir::Horizontal, &[(nav, 1.0), (page, 3.0), (side, 1.0)]);
    let old = Tree::new(egui::Id::unique("old"), root, tiles);
    assert!(from_json(&to_json(&old)).is_some());
}

#[test]
fn tools_float_until_the_user_docks_them() {
    let mut placement = Placement::default();
    assert!(placement.floats(Pane::Tool(ToolPane::Reports)));
    assert!(placement.floats(Pane::Tool(ToolPane::Plugins)));
    for p in [Pane::Page, Pane::Library, Pane::Properties, Pane::Navigator(LeftTab::Devices), Pane::Tool(ToolPane::Start), Pane::Tool(ToolPane::Find)] {
        assert!(!placement.floats(p), "{p:?} opens docked");
    }

    placement.set(Pane::Tool(ToolPane::Reports), false);
    placement.set(Pane::Navigator(LeftTab::Devices), true);
    assert!(!placement.floats(Pane::Tool(ToolPane::Reports)));
    assert!(placement.floats(Pane::Navigator(LeftTab::Devices)));

    placement.set(Pane::Tool(ToolPane::Reports), true);
    placement.set(Pane::Navigator(LeftTab::Devices), false);
    assert_eq!(placement, Placement::default(), "back to the defaults, nothing remembered");
}

#[test]
fn every_floating_tool_has_a_window_size() {
    for t in ToolPane::ALL.into_iter().filter(|t| t.floats()) {
        let [w, h] = t.window_size();
        assert!(w >= 400.0 && h >= 300.0, "{t:?}");
    }
}

#[test]
fn long_page_tab_labels_end_with_an_ellipsis() {
    assert_eq!(truncate_label("3  Motors", 28), "3  Motors");
    let cut = truncate_label("2  Power supply conveyor motors", 28);
    assert_eq!(cut, "2  Power supply conveyor mo…");
    assert_eq!(cut.chars().count(), 28);
    assert_eq!(truncate_label("2  Power supply conveyor motor", 26), "2  Power supply conveyor…", "no space before the ellipsis");
}

#[test]
fn reports_open_large_enough_for_their_tables() {
    let [w, h] = ToolPane::Reports.window_size();
    assert!(w >= 900.0 && h >= 600.0, "report list and table side by side");
}

#[test]
fn the_document_worked_in_follows_the_tab_shown() {
    let mut tree = default_tree();
    let start = Pane::Tool(ToolPane::Start);
    show(&mut tree, start);
    assert_eq!(shown_document(&tree, start), start);
    show(&mut tree, Pane::Page);
    assert_eq!(shown_document(&tree, start), Pane::Page, "the start page tab is behind the page now");
    assert_eq!(shown_document(&tree, Pane::Page), Pane::Page);
}

#[test]
fn a_standard_workspace_comes_back_in_its_default_arrangement() {
    for w in Workspace::ALL {
        let mut tree = default_tree_for(w);
        let before = panes(&tree);
        close(&mut tree, before[0]);
        show(&mut tree, Pane::Tool(ToolPane::Reports));
        let mut side = Side::default();
        side.placement.set(Pane::Tool(ToolPane::Cables), !Pane::Tool(ToolPane::Cables).floats());
        side.floating.push(Pane::Tool(ToolPane::Parts));
        assert_ne!(panes(&tree), before);
        reset(w, &mut tree, &mut side);
        assert_eq!(panes(&tree), before, "{}", w.key());
        assert_eq!(side, Side::default());
    }
}

#[test]
fn a_workspace_shows_its_main_document_again() {
    assert_eq!(main_document(Workspace::Schematic), Pane::Page);
    assert_eq!(main_document(Workspace::Panel), Pane::LayoutView);
    assert_eq!(main_document(Workspace::Wiring), Pane::WiringWorkbench);
    for w in Workspace::ALL {
        assert!(main_document(w).is_document(), "{w:?}");
    }
    // The page was left in front of the panel layout: choosing the workspace shows the layout.
    let mut tree = default_tree_for(Workspace::Panel);
    assert!(visible(&tree).contains(&Pane::LayoutView));
    show(&mut tree, Pane::Page);
    assert!(!visible(&tree).contains(&Pane::LayoutView));
    show(&mut tree, main_document(Workspace::Panel));
    assert!(visible(&tree).contains(&Pane::LayoutView) && !visible(&tree).contains(&Pane::Page));
}
