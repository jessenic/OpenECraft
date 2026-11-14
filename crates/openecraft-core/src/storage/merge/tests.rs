use super::*;
use crate::model::{Page, Project};
use crate::sample::demo_project;
use crate::storage::{join, split, Files, INDEX};

fn merged(path: &str, base: &str, ours: &str, theirs: &str) -> (String, Vec<String>) {
    let m = merge_file(path, base.as_bytes(), ours.as_bytes(), theirs.as_bytes()).unwrap();
    (String::from_utf8(m.content).unwrap(), m.conflicts)
}

/// Merges every file of two projects that come from `base`, like git with the merge
/// driver: files only one side changed are taken from it.
fn merge_projects(base: &Project, ours: &Project, theirs: &Project) -> (Project, Vec<String>) {
    let (b, o, t) = (split(base).unwrap(), split(ours).unwrap(), split(theirs).unwrap());
    let mut out = Files::new();
    let mut conflicts = Vec::new();
    let names: BTreeSet<&String> = o.keys().chain(t.keys()).collect();
    for name in names {
        let content = match (b.get(name), o.get(name), t.get(name)) {
            (_, Some(o), Some(t)) if o == t => Some(o.clone()),
            (Some(b), Some(o), Some(t)) if b == o => Some(t.clone()),
            (Some(b), Some(o), Some(t)) if b == t => Some(o.clone()),
            (b, Some(o), Some(t)) => {
                let m = merge_file(name, b.map(Vec::as_slice).unwrap_or_default(), o, t).unwrap();
                conflicts.extend(m.conflicts.iter().map(|c| format!("{name}: {c}")));
                Some(m.content)
            }
            (None, o, t) => o.or(t).cloned(),
            // Removed by one side.
            (Some(_), _, _) => None,
        };
        if let Some(content) = content {
            out.insert(name.clone(), content);
        }
    }
    (join(&out).unwrap().0, conflicts)
}

#[test]
fn an_unchanged_file_is_written_as_it_was_saved() {
    let mut prj = demo_project();
    prj.pages[1].symbols[0].pos.x = 0.1;
    for (path, content) in split(&prj).unwrap() {
        let m = merge_file(&path, &content, &content, &content).unwrap();
        assert!(m.content == content && m.conflicts.is_empty(), "{path}");
        // Also when both sides made the same change to an older file.
        assert!(merge_file(&path, b"{}", &content, &content).unwrap().content == content, "{path}");
    }
}

#[test]
fn fields_changed_by_one_side_are_taken() {
    let base = r#"{"id":1,"name":"1","description":"","symbols":[]}"#;
    let ours = r#"{"id":1,"name":"2","description":"","symbols":[]}"#;
    let theirs = r#"{"id":1,"name":"1","description":"Pump","symbols":[],"locked":true}"#;
    let (out, conflicts) = merged("pages/1.json", base, ours, theirs);
    assert_eq!(out, "{\"id\":1,\"name\":\"2\",\"description\":\"Pump\",\"symbols\":[],\"locked\":true}\n");
    assert!(conflicts.is_empty());
    // A field one side removed (it became the default) goes, if the other left it.
    let (out, conflicts) = merged("pages/1.json", theirs, theirs, base);
    assert_eq!(out, format!("{base}\n"));
    assert!(conflicts.is_empty());
}

#[test]
fn records_of_a_list_are_merged_by_their_id() {
    let base = r#"{"symbols":[{"id":2,"tag":"-K1","pos":[1.0,2.0]},{"id":3,"tag":"-K2","pos":[5.0,2.0]},{"id":4,"tag":"-K3","pos":[9.0,2.0]}]}"#;
    // We moved -K1 and removed -K3, they renamed -K2 and added two symbols behind it.
    let ours = r#"{"symbols":[{"id":2,"tag":"-K1","pos":[1.0,8.0]},{"id":3,"tag":"-K2","pos":[5.0,2.0]}]}"#;
    let theirs = r#"{"symbols":[{"id":2,"tag":"-K1","pos":[1.0,2.0]},{"id":3,"tag":"-Q2","pos":[5.0,2.0]},{"id":7,"tag":"-F1","pos":[0.5,0.5]},{"id":8,"tag":"-F2","pos":[0.5,0.5]},{"id":4,"tag":"-K3","pos":[9.0,2.0]}]}"#;
    let (out, conflicts) = merged("pages/1.json", base, ours, theirs);
    assert_eq!(out, "{\"symbols\":[{\"id\":2,\"tag\":\"-K1\",\"pos\":[1.0,8.0]},{\"id\":3,\"tag\":\"-Q2\",\"pos\":[5.0,2.0]},{\"id\":7,\"tag\":\"-F1\",\"pos\":[0.5,0.5]},{\"id\":8,\"tag\":\"-F2\",\"pos\":[0.5,0.5]}]}\n");
    assert!(conflicts.is_empty(), "{conflicts:?}");
}

#[test]
fn what_both_sides_changed_is_a_conflict_and_keeps_ours() {
    let base = r#"{"symbols":[{"id":2,"tag":"-K1","pos":[1.0,2.0]},{"id":3,"tag":"-K2"}]}"#;
    let ours = r#"{"symbols":[{"id":2,"tag":"-Q1","pos":[1.0,3.0]}]}"#;
    let theirs = r#"{"symbols":[{"id":2,"tag":"-M1","pos":[4.0,2.0]},{"id":3,"tag":"-K9"}]}"#;
    let (out, conflicts) = merged("pages/1.json", base, ours, theirs);
    assert_eq!(out, format!("{ours}\n"));
    // The tag, the position (a list without ids is one value) and the symbol we
    // removed and they changed.
    assert_eq!(conflicts, ["symbols[2].tag", "symbols[2].pos", "symbols[3]"]);
    // The other way round the symbol they removed stays, as we changed it.
    let (out, conflicts) = merged("pages/1.json", base, theirs, ours);
    assert_eq!(out, format!("{theirs}\n"));
    assert_eq!(conflicts.len(), 3);
    // A file of another kind on one side.
    assert_eq!(merged("data/layout.json", "{}", "[1]", r#"{"a":1}"#).1, ["/"]);
    assert!(merge_file("pages/1.json", b"", b"{", b"{}").unwrap_err().contains("pages/1.json"));
}

#[test]
fn two_new_objects_with_the_same_id_are_a_conflict() {
    let base = r#"{"symbols":[{"id":2,"tag":"-K1"}]}"#;
    let ours = r#"{"symbols":[{"id":2,"tag":"-K1"},{"id":9,"tag":"-S1"}]}"#;
    let theirs = r#"{"symbols":[{"id":2,"tag":"-K1"},{"id":9,"tag":"-P1"}]}"#;
    let (out, conflicts) = merged("pages/1.json", base, ours, theirs);
    assert_eq!(out, format!("{ours}\n"));
    assert_eq!(conflicts, ["symbols[9]"]);
    // The same object added on both sides (a merge taken over before) is none.
    assert!(merged("pages/1.json", base, ours, ours).1.is_empty());
    // Neither is a page file that is new on both sides with the same content; with
    // another content it is another page with the same id.
    assert!(merged("pages/9.json", "", ours, ours).1.is_empty());
    assert_eq!(merged("pages/9.json", "", ours, theirs), (format!("{ours}
"), vec!["/".to_string()]));
    // The order of a list that is new on both sides holds the records of both.
    assert_eq!(merged("data/cables/order.json", "", r#"["1"]"#, r#"["2"]"#), ("[\"1\",\"2\"]
".to_string(), Vec::new()));
}

#[test]
fn orders_and_the_id_counter_merge_without_conflicts() {
    // Both sides added a page: the order of the pages in the index.
    let base = r#"{"layout":1,"pages":["1","2","3"],"project":{"format":2,"info":{"name":"A"}}}"#;
    let ours = r#"{"layout":1,"pages":["1","10","2","3"],"project":{"format":2,"info":{"name":"A"}}}"#;
    let theirs = r#"{"layout":1,"pages":["1","2","11","3"],"project":{"format":2,"info":{"name":"B"}}}"#;
    let (out, conflicts) = merged("plant.oecp", base, ours, theirs);
    assert_eq!(out, "{\"layout\":1,\"pages\":[\"1\",\"10\",\"2\",\"11\",\"3\"],\"project\":{\"format\":2,\"info\":{\"name\":\"B\"}}}\n");
    assert!(conflicts.is_empty());
    // One removed a part, the other added two.
    let (out, conflicts) = merged("data/parts/order.json", r#"["a","b","c"]"#, r#"["a","c"]"#, r#"["x","a","b","c","y"]"#);
    assert_eq!((out.as_str(), conflicts.len()), ("[\"x\",\"a\",\"c\",\"y\"]\n", 0));
    // Other lists of texts are one value: the pins of a symbol keep their places.
    assert_eq!(merged("pages/1.json", r#"{"pins":["1","2"]}"#, r#"{"pins":["3","2"]}"#, r#"{"pins":["1","4"]}"#).1, ["pins"]);
    // Both sides created objects: the larger counter.
    assert_eq!(merged("data/next_id.json", "100", "140", "112").0, "140\n");
    assert_eq!(merged("data/next_id.json", "100", "105", "112"), ("112\n".to_string(), Vec::new()));
}

#[test]
fn an_index_of_a_newer_layout_is_not_merged() {
    let index = |layout: u32, name: &str| format!(r#"{{"layout":{layout},"pages":["1"],"project":{{"format":2,"info":{{"name":"{name}"}}}}}}"#);
    let (base, ours) = (index(crate::storage::LAYOUT, "A"), index(crate::storage::LAYOUT, "B"));
    assert!(merge_file("p.oecp", base.as_bytes(), ours.as_bytes(), ours.as_bytes()).is_ok());
    let newer = index(crate::storage::LAYOUT + 1, "C");
    for (o, t) in [(&ours, &newer), (&newer, &ours)] {
        let err = merge_file("p.oecp", base.as_bytes(), o.as_bytes(), t.as_bytes()).unwrap_err();
        assert!(err.contains("newer version"), "{err}");
    }
    // The other files do not name a layout.
    assert!(merge_file("pages/1.json", b"{}", br#"{"layout":9}"#, br#"{"layout":9,"a":1}"#).is_ok());
}

#[test]
fn projects_edited_on_two_branches_merge_file_by_file() {
    let base = demo_project();
    let (mut ours, mut theirs) = (base.clone(), base.clone());
    // We: a page renamed, a symbol moved, a device changed, a page added.
    ours.pages[0].description = "Ours".into();
    ours.pages[1].symbols[0].pos.x += 10.0;
    let tag = ours.devices.keys().next().unwrap().clone();
    ours.devices.get_mut(&tag).unwrap().description = "Ours".into();
    let id = ours.new_id();
    ours.pages.push(Page::new(id, "90", "Ours", crate::model::PageKind::Schematic));
    // They: another symbol of the same page moved, another page changed, the project
    // renamed, a part removed.
    theirs.pages[1].symbols[1].pos.y += 10.0;
    theirs.pages[2].description = "Theirs".into();
    theirs.info.name = "Theirs".into();
    let part = theirs.parts.pop().unwrap();

    let (merged, conflicts) = merge_projects(&base, &ours, &theirs);
    assert!(conflicts.is_empty(), "{conflicts:?}");
    assert_eq!((merged.pages[0].description.as_str(), merged.pages[2].description.as_str()), ("Ours", "Theirs"));
    assert_eq!(merged.pages[1].symbols[0].pos, ours.pages[1].symbols[0].pos);
    assert_eq!(merged.pages[1].symbols[1].pos, theirs.pages[1].symbols[1].pos);
    assert_eq!(merged.devices[&tag].description, "Ours");
    assert_eq!(merged.info.name, "Theirs");
    assert_eq!(merged.pages.len(), base.pages.len() + 1);
    assert!(!merged.parts.iter().any(|p| p.number == part.number));
    assert_eq!(merged.next_id, ours.next_id);
    assert!(crate::verify::verify(&merged).findings.iter().all(|f| f.severity != crate::issue::Severity::Error));

    // Both sides create an object: the same id on two pages. The files merge, the
    // verification finds the id and repairs it.
    let id = theirs.new_id();
    theirs.pages.push(Page::new(id, "91", "Theirs", crate::model::PageKind::Schematic));
    let (mut merged, conflicts) = merge_projects(&base, &ours, &theirs);
    assert_eq!(conflicts.len(), 1, "{conflicts:?}");
    assert!(conflicts[0].starts_with(&format!("pages/{id}.json")));
    assert_eq!(merged.pages.len(), ours.pages.len());
    assert!(split(&merged).unwrap().contains_key(INDEX));
    let _ = crate::verify::repair(&mut merged);
    assert!(crate::verify::verify(&merged).findings.iter().all(|f| !f.repairable));
}
