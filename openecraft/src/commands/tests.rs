use super::*;
use crate::ribbon;

#[test]
fn ids_are_unique_snake_case_and_every_command_is_defined_once() {
    let mut ids = std::collections::BTreeSet::new();
    let mut cmds = std::collections::BTreeSet::new();
    for d in ALL {
        assert!(ids.insert(d.id), "id {} twice", d.id);
        assert!(cmds.insert(d.cmd), "{:?} defined twice", d.cmd);
        assert!(d.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'), "{} is not snake_case", d.id);
        assert!(!d.title.is_empty(), "{:?} has no title", d.cmd);
        assert_eq!(def(d.cmd).id, d.id);
        assert_eq!(by_id(d.id).map(|x| x.cmd), Some(d.cmd));
    }
}

#[test]
fn every_command_has_a_place_in_the_shell() {
    let mut missing = Vec::new();
    for d in ALL {
        if ribbon::places(d.cmd).is_empty() {
            missing.push(d.id);
        }
    }
    // Commands only reachable by key or from the page tabs are listed here on purpose.
    let by_key_only = ["previous_page_tab"];
    missing.retain(|id| !by_key_only.contains(id));
    assert!(missing.is_empty(), "commands without a place in the ribbon, the backstage or View › Panes: {missing:?}");
}

#[test]
fn every_pane_is_shown_by_exactly_one_command() {
    for pane in Pane::all() {
        if pane == Pane::Page {
            continue;
        }
        let n = ALL.iter().filter(|d| d.pane == Some(pane)).count();
        assert_eq!(n, 1, "{pane:?} is shown by {n} commands");
        assert!(for_pane(pane).is_some());
    }
}

#[test]
fn toggles_and_menus_have_no_pane() {
    for d in ALL {
        if d.pane.is_some() {
            assert_eq!(d.kind, Kind::Button, "{}", d.id);
        }
    }
}

#[test]
fn wildcards_match_like_in_eplan() {
    assert!(wildcard_match("coil", "Relay coil"));
    assert!(wildcard_match("", "anything"));
    assert!(wildcard_match("c*l", "Relay coil"));
    assert!(wildcard_match("co?l", "COIL"));
    assert!(!wildcard_match("co?l", "cot"));
    assert!(wildcard_match("*contact*NO", "Contact, NO"));
    assert!(!wildcard_match("x*z", "abc"));
    assert!(wildcard_match("a*", "abc"));
}

#[test]
fn search_puts_title_matches_first() {
    let items = vec![
        ("wire", vec!["Wire numbering…".to_string()]),
        ("pages", vec!["Pages".to_string(), "navigator".to_string()]),
        ("new", vec!["New schematic page".to_string()]),
        ("renumber", vec!["Renumber pages…".to_string()]),
    ];
    let hits = rank("page", &items);
    let order: Vec<&str> = hits.iter().map(|h| h.item).collect();
    assert_eq!(order[0], "pages", "title starts with the query");
    assert!(order.contains(&"new") && order.contains(&"renumber"));
    assert!(!order.contains(&"wire"));
    assert_eq!(rank("navigator", &items).len(), 1, "found by its further texts");
    assert_eq!(rank("", &items).len(), 4);
    assert!(rank("page renumber", &items).iter().all(|h| h.item == "renumber"));
}

#[test]
fn commands_that_change_the_page_are_known() {
    for c in [Cmd::Cut, Cmd::Paste, Cmd::Delete, Cmd::Rotate, Cmd::Mirror, Cmd::MoveBy, Cmd::MultipleCopy, Cmd::Align, Cmd::Distribute, Cmd::AutoConnectSelection, Cmd::JoinCollinear, Cmd::WireTool, Cmd::TextTool, Cmd::LineTool, Cmd::DimAligned] {
        assert!(changes_page(c), "{c:?}");
    }
    // Looking, copying and the review tools stay on read only pages.
    for c in [Cmd::Copy, Cmd::SelectAll, Cmd::SaveSelectionAsMacro, Cmd::Properties, Cmd::FitPage, Cmd::CommentTool, Cmd::Markup, Cmd::PageProperties] {
        assert!(!changes_page(c), "{c:?}");
    }
    // The clipboard keys are shown though they cannot be changed.
    assert_eq!(fixed_keys(Cmd::Copy), Some("Ctrl+C"));
    assert_eq!(crate::shortcuts::Shortcuts::default().cmd_text(Cmd::Paste), "Ctrl+V");
    assert_eq!(crate::shortcuts::Shortcuts::default().cmd_text(Cmd::Delete), "Delete / Backspace");
}

#[test]
fn only_commands_that_ask_for_input_end_with_an_ellipsis() {
    for d in ALL {
        if d.pane.is_some() || d.kind != Kind::Button {
            assert!(!d.title.ends_with('…'), "{} shows a pane, switches or opens a menu: no …", d.id);
        }
    }
    for c in [Cmd::Open, Cmd::SaveAs, Cmd::CheckSettings, Cmd::MoveBy, Cmd::Properties] {
        assert!(def(c).title.ends_with('…'), "{c:?} opens a dialog");
    }
}

#[test]
fn the_layout_view_commands_bring_the_view_to_the_front() {
    for cmd in [Cmd::Layout3d, Cmd::LayoutFloorPlan, Cmd::LayoutFront, Cmd::LayoutDoor, Cmd::LayoutSelect, Cmd::LayoutFit] {
        assert!(acts_in_layout_view(cmd), "{cmd:?}");
    }
    for cmd in [Cmd::Save, Cmd::LayoutView, Cmd::LayoutReports, Cmd::LayoutNcData, Cmd::WorkspaceSchematic] {
        assert!(!acts_in_layout_view(cmd), "{cmd:?}");
    }
    for d in ALL.iter().filter(|d| acts_in_layout_view(d.cmd)) {
        assert!(d.pane.is_none() && d.kind != Kind::Menu, "{:?} is a plain command", d.cmd);
    }
}
