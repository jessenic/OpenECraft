use super::*;

#[test]
fn replacing_a_project_discards_page_references_but_keeps_find_criteria() {
    let mut old = openecraft_core::model::Project::default();
    old.pages.resize(6, old.pages[0].clone());
    old.pages[5].description = "Previous project marker".into();
    let mut st = FindUi {
        open: true, query: Query { text: "Previous project marker".into(), case_sensitive: true, ..Default::default() },
        with: "Replacement marker".into(), searched: true, rev: 0, ..Default::default()
    };
    st.hits = search::find(&old, &st.query);
    assert!(st.hits.iter().any(|h| h.page == Some(5)));
    let next = openecraft_core::model::Project::default();
    assert!(st.hits.iter().any(|h| h.page.is_some_and(|p| p >= next.pages.len())));
    // A newly opened project's history can have the same revision as the old one.
    st.clear_results();
    assert!(st.hits.is_empty());
    assert!(!st.searched);
    assert!(st.open);
    assert!(st.query.case_sensitive);
    assert_eq!(st.query.text, "Previous project marker");
    assert_eq!(st.with, "Replacement marker");
    assert!(search::find(&next, &st.query).is_empty());
}

#[test]
fn hits_without_a_page_lead_to_their_pane() {
    assert_eq!(pane_of(Kind::Part), Some(Pane::Tool(ToolPane::Parts)));
    assert_eq!(pane_of(Kind::Cable), Some(Pane::Tool(ToolPane::Cables)));
    assert_eq!(pane_of(Kind::Device), Some(Pane::Navigator(LeftTab::Devices)));
    // Hits of these kinds are always on a page.
    assert_eq!(pane_of(Kind::Text), None);
}
