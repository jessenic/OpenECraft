use super::*;
use crate::model::forms::{Form, FormItem, Picture};
use crate::model::{ImageItem, PageKind};
use crate::sample::{demo_project, EXAMPLES};

fn text(files: &Files, path: &str) -> String {
    String::from_utf8(files[path].clone()).unwrap()
}

/// The demo with a document page, an image, pictures and a baseline of all of it.
fn project_with_files() -> Project {
    let mut prj = demo_project();
    let id = prj.new_id();
    let mut doc = Page::new(id, "8", "Data sheet", PageKind::Document);
    doc.document = Some(Document::new("sheet.pdf", b"%PDF-1.4 test"));
    prj.pages.push(doc);
    let id = prj.new_id();
    prj.pages[0].images.push(ImageItem { id, file: Document::new("photo.PNG", b"png bytes"), ..Default::default() });
    let picture = Picture { file: Document::new("logo.png", b"logo bytes"), px: [4, 2] };
    prj.info.logo = Some(picture.clone());
    let item = FormItem::Image { a: Default::default(), b: Default::default(), picture: Some(picture), show: Default::default() };
    prj.forms.title_blocks.push(Form { items: vec![item], ..Default::default() });
    let stored = prj.clone();
    prj.baselines.push(crate::model::compare::Baseline { name: "As built".into(), project: crate::model::compare::Stored(stored.into()), ..Default::default() });
    prj
}

#[test]
fn sample_projects_round_trip() {
    let projects = [demo_project(), project_with_files()].into_iter().chain(EXAMPLES.iter().map(|e| (e.build)()));
    for prj in projects {
        let files = split(&prj).unwrap();
        let (back, conv) = join(&files).unwrap();
        assert!(back == prj, "{}", prj.info.name);
        assert!(!conv.converted() && !conv.newer());
        // The same project as its project file.
        assert!(back == Project::from_json(&prj.to_json()).unwrap(), "{}", prj.info.name);
        // The files depend only on the content.
        assert!(split(&back).unwrap() == files, "{}", prj.info.name);
    }
}

#[test]
fn pages_and_records_are_files_of_one_line() {
    let prj = demo_project();
    let files = split(&prj).unwrap();
    for pg in &prj.pages {
        let page: Page = serde_json::from_slice(&files[&format!("pages/{}.json", pg.id)]).unwrap();
        assert_eq!(&page, pg);
    }
    let index: serde_json::Value = serde_json::from_slice(&files[INDEX]).unwrap();
    assert_eq!(index["layout"], LAYOUT);
    assert_eq!(index["pages"].as_array().unwrap().len(), prj.pages.len());
    assert_eq!(index["project"]["format"], FORMAT);
    assert_eq!(index["project"]["info"]["name"], prj.info.name.as_str());
    for field in ["pages", "devices", "parts", "layout", "next_id"] {
        assert!(index["project"].get(field).is_none(), "{field}");
    }
    assert_eq!(text(&files, "data/next_id.json"), format!("{}\n", prj.next_id));
    assert!(files.contains_key("data/layout.json"));

    // One file per device and part; the order of the parts is a file of its own.
    assert_eq!(records(&files, "data/devices").count(), prj.devices.len());
    let tag = prj.devices.keys().next().unwrap();
    let device = text(&files, &record_path("data/devices", &file_stem(tag), true));
    assert!(device.starts_with(&format!("{{{}:", serde_json::to_string(tag).unwrap())), "{device}");
    assert_eq!(records(&files, "data/parts").count(), prj.parts.len());
    let order: Vec<String> = serde_json::from_slice(&files["data/parts/order.json"]).unwrap();
    assert_eq!(order, prj.parts.iter().map(|p| file_stem(&p.number)).collect::<Vec<_>>());

    for (path, content) in &files {
        let content = String::from_utf8(content.clone()).unwrap();
        assert!(content.ends_with('\n') && !content.trim_end_matches('\n').contains('\n'), "{path}");
    }
}

#[test]
fn records_are_spread_over_folders() {
    // By the first digit of the hash in the name, or the last hex digit of an id.
    let stem = file_stem("-K1");
    let digit = stem.rsplit_once('~').unwrap().1.chars().next().unwrap();
    assert_eq!(record_path("data/devices", &stem, true), format!("data/devices/{digit}/{stem}.json"));
    assert_eq!(record_path("data/cables", "31", true), "data/cables/f/31.json");
    assert_eq!(record_path("pages", "31", false), "pages/31.json");
    let mut prj = demo_project();
    for i in 0..400 {
        prj.devices.insert(format!("-K{i}"), Default::default());
    }
    let files = split(&prj).unwrap();
    let folders: BTreeSet<&str> = files.keys().filter_map(|p| p.strip_prefix("data/devices/")).filter_map(|p| p.split_once('/')).map(|(folder, _)| folder).collect();
    assert_eq!(folders.len(), 16);
    assert_eq!(records(&files, "data/devices").count(), prj.devices.len());
    assert!(join(&files).unwrap().0 == prj);
    // The pages stay in one folder.
    assert!(files.keys().filter(|p| p.starts_with("pages/")).all(|p| p.matches('/').count() == 1));
}

#[test]
fn only_what_changed_since_the_last_project_is_split() {
    let last = project_with_files();
    assert_eq!(same_fields(&last, &last), RULES.iter().map(|(field, _)| *field).collect::<Vec<_>>());
    let whole = split(&last).unwrap();
    // Nothing changed: the index alone (it is small and always written), with the
    // logo it holds.
    let (files, kept) = split_since(&last, Some(&last)).unwrap();
    assert_eq!(files.keys().filter(|name| !name.starts_with("files/")).collect::<Vec<_>>(), [INDEX]);
    assert_eq!(files.len(), 2);
    assert_eq!(files[INDEX], whole[INDEX]);
    assert!(whole.keys().all(|name| name == INDEX || kept.covers(name)), "{:?}", whole.keys().filter(|n| !kept.covers(n)).collect::<Vec<_>>());
    assert!(!kept.covers("data/devices.txt") && !kept.covers("data/devicesx/a.json") && !kept.covers("pages/0.json"));

    // A page, a device and the logo changed: their files, as a whole split has them.
    let mut prj = last.clone();
    prj.pages[1].description = "Changed".into();
    let tag = prj.devices.keys().next().unwrap().clone();
    prj.devices.get_mut(&tag).unwrap().description = "Changed".into();
    prj.info.logo = Some(Picture { file: Document::new("new.png", b"new logo"), px: [1, 1] });
    let whole = split(&prj).unwrap();
    let (files, kept) = split_since(&prj, Some(&last)).unwrap();
    let page = format!("pages/{}.json", prj.pages[1].id);
    assert!(files.contains_key(&page) && !kept.covers(&page));
    assert!(kept.covers(&format!("pages/{}.json", prj.pages[0].id)) && kept.covers("data/parts/order.json") && kept.covers("data/layout.json"));
    assert_eq!(files.keys().filter(|name| name.starts_with("data/devices/")).count(), prj.devices.len());
    // The new embedded file is there; those of what was left out are not looked at.
    assert_eq!(files.keys().filter(|name| name.starts_with("files/")).count(), 1);
    assert!(kept.covers("files/any"));
    for (name, content) in &files {
        assert!(whole.get(name) == Some(content), "{name}");
    }
    assert!(whole.keys().all(|name| files.contains_key(name) || kept.covers(name)));
    // Without a project before, nothing is left out.
    assert_eq!(split_since(&prj, None).unwrap(), (whole, Kept::default()));
}

#[test]
fn embedded_files_are_stored_apart_wherever_the_project_holds_them() {
    // Also in a revision and in a page template, which no list of places names.
    let mut prj = demo_project();
    let mut page = Page::new(999, "T", "Template", PageKind::Document);
    page.document = Some(Document::new("template.pdf", b"template bytes"));
    prj.revisions.push(crate::model::revision::Revision { pages: crate::model::revision::Snapshot(vec![page].into()), ..Default::default() });
    let files = split(&prj).unwrap();
    let embedded: Vec<&String> = files.keys().filter(|p| p.starts_with("files/")).collect();
    assert_eq!(embedded.len(), 1);
    assert!(!text(&files, "data/revisions.json").contains(&base64_encode(b"template bytes")));
    let (back, _) = join(&files).unwrap();
    assert!(back == prj);
    assert_eq!(back.revisions[0].pages.0[0].document.as_ref().unwrap().bytes().unwrap(), b"template bytes");
    // A project file holds the content, as before.
    assert!(prj.to_json().contains(&base64_encode(b"template bytes")));
    assert!(Project::from_json(&prj.to_json()).unwrap() == prj);
}

#[test]
fn file_names_are_safe_and_distinct() {
    let stem = file_stem("=A1+S1-K1:2/3");
    assert!(stem.starts_with("=A1+S1-K1_2_3~"), "{stem}");
    assert!(stem.chars().all(|c| c.is_ascii_alphanumeric() || "-_+=.~".contains(c)));
    // Names that differ only in case or in replaced characters stay apart.
    assert_ne!(file_stem("-K1").to_lowercase(), file_stem("-k1").to_lowercase());
    assert_ne!(file_stem("A/1"), file_stem("A:1"));
    assert_ne!(file_stem("CON"), "CON");
    assert!(file_stem(&"x".repeat(300)).len() < 64);
    assert_eq!(file_stem("Ä"), file_stem("Ä"));
}

#[test]
fn embedded_files_are_plain_files_stored_once() {
    let prj = project_with_files();
    let files = split(&prj).unwrap();
    let embedded: Vec<(&str, &[u8])> = files.iter().filter(|(p, _)| p.starts_with("files/")).map(|(p, b)| (p.as_str(), b.as_slice())).collect();
    // The logo of the project and of the title block and all files of the baseline
    // are the same files.
    assert_eq!(embedded.len(), 3, "{embedded:?}");
    let pdf = embedded.iter().find(|(p, _)| p.ends_with(".pdf")).unwrap();
    assert_eq!(pdf.1, b"%PDF-1.4 test");
    assert_eq!(pdf.0, format!("files/{}.pdf", sha256(b"%PDF-1.4 test")));
    assert!(embedded.iter().any(|(p, b)| p.ends_with(".png") && *b == b"png bytes"));
    for data in [base64_encode(b"%PDF-1.4 test"), base64_encode(b"png bytes"), base64_encode(b"logo bytes")] {
        assert!(files.iter().all(|(_, content)| !String::from_utf8_lossy(content).contains(&data)));
    }
    assert!(text(&files, &format!("pages/{}.json", prj.pages.last().unwrap().id)).contains(&format!("\"data\":\"file:{}\"", pdf.0)));

    let (back, _) = join(&files).unwrap();
    assert_eq!(back.pages.last().unwrap().document.as_ref().unwrap().bytes().unwrap(), b"%PDF-1.4 test");
    assert_eq!(back.baselines[0].project.0.info.logo.as_ref().unwrap().file.bytes().unwrap(), b"logo bytes");

    let mut broken = files.clone();
    broken.remove(pdf.0);
    assert!(join(&broken).unwrap_err().contains(pdf.0));
}

#[test]
fn damaged_documents_stay_in_the_page() {
    let mut prj = demo_project();
    prj.pages[0].document = Some(Document { name: "broken.pdf".into(), data: String::from("not base64 !").into() });
    let files = split(&prj).unwrap();
    assert!(!files.keys().any(|p| p.starts_with("files/")));
    assert!(join(&files).unwrap().0 == prj);
}

#[test]
fn records_without_unique_keys_stay_in_one_file() {
    let mut prj = demo_project();
    let part = prj.parts[0].clone();
    prj.parts.push(part);
    let id = prj.pages[0].id;
    prj.pages[1].id = id;
    let files = split(&prj).unwrap();
    assert!(files.contains_key("data/parts.json"));
    assert!(!files.keys().any(|p| p.starts_with("data/parts/") || p.starts_with("pages/")));
    assert!(join(&files).unwrap().0 == prj);
}

#[test]
fn empty_data_has_no_files() {
    let mut prj = demo_project();
    prj.devices.clear();
    prj.parts.clear();
    prj.cables.clear();
    let files = split(&prj).unwrap();
    assert!(!files.keys().any(|p| p.starts_with("data/devices") || p.starts_with("data/parts") || p.starts_with("data/cables")), "{:?}", files.keys());
    assert!(join(&files).unwrap().0 == prj);
}

#[test]
fn records_missing_in_the_order_are_appended() {
    let prj = demo_project();
    let mut files = split(&prj).unwrap();
    // As after a merge of two branches that each added a page and a part.
    let mut index: serde_json::Value = serde_json::from_slice(&files[INDEX]).unwrap();
    let last = index["pages"].as_array_mut().unwrap().pop().unwrap();
    files.insert(INDEX.into(), line(&index).unwrap());
    let mut order: Vec<String> = serde_json::from_slice(&files["data/parts/order.json"]).unwrap();
    let first = order.remove(0);
    files.insert("data/parts/order.json".into(), line(&order).unwrap());
    let (back, _) = join(&files).unwrap();
    assert_eq!(back.pages.len(), prj.pages.len());
    assert_eq!(back.pages.last().unwrap().id.to_string(), last.as_str().unwrap());
    assert_eq!(back.parts.len(), prj.parts.len());
    assert_eq!(file_stem(&back.parts.last().unwrap().number), first);
}

#[test]
fn broken_folders_are_refused() {
    let files = split(&demo_project()).unwrap();
    assert!(join(&Files::new()).unwrap_err().contains(INDEX));

    let mut missing = files.clone();
    let page = missing.keys().find(|p| p.starts_with("pages/")).unwrap().clone();
    missing.remove(&page);
    assert!(join(&missing).unwrap_err().contains(&page));

    let mut invalid = files.clone();
    invalid.insert(page.clone(), b"{".to_vec());
    assert!(join(&invalid).unwrap_err().contains(&page));

    let mut newer = files.clone();
    newer.insert(INDEX.into(), line(&serde_json::json!({ "layout": LAYOUT + 1 })).unwrap());
    assert!(join(&newer).unwrap_err().contains(&(LAYOUT + 1).to_string()));
}

#[test]
fn files_merged_as_text_are_named() {
    let mut files = split(&demo_project()).unwrap();
    let page = files.keys().find(|p| p.starts_with("pages/")).unwrap().clone();
    let content = String::from_utf8(files[&page].clone()).unwrap();
    files.insert(page.clone(), format!("<<<<<<< HEAD\n{content}=======\n{content}>>>>>>> theirs\n").into_bytes());
    let err = join(&files).unwrap_err();
    assert!(err.contains(&page) && err.contains("merge conflict"), "{err}");
    // Other broken files are reported as before.
    files.insert(page.clone(), b"{ <<<<<<< nope".to_vec());
    assert!(join(&files).unwrap_err().starts_with("Not a valid project file"));
}

#[test]
fn the_rules_belong_to_the_layout() {
    // Changing the rules changes which files a field has: a version without the
    // rule would read the project without the field and remove its files. Raise
    // LAYOUT with every change of RULES, and add the rules of the new layout here.
    let rules: Vec<String> = RULES.iter().map(|(field, rule)| format!("{field}:{rule:?}")).collect();
    let layouts = [(
        1,
        "baselines:List(\"name\") cables:List(\"id\") configuration:Whole devices:Map electrical:Whole field:Whole forms:Whole harnesses:List(\"id\") import_sources:Map languages:Whole layout:Whole \
         mounting_marks:Whole next_id:Whole page_templates:Whole parts:List(\"number\") pct_loops:Whole plc:Whole plc_cards:Map plugs:Map preplanning:Whole property_db:Whole report_blocks:Whole review:Whole \
         revisions:Whole strips:Map structures:Whole symbols:List(\"key\") wiring_marks:Whole",
    )];
    let (layout, expected) = layouts.last().unwrap();
    assert_eq!(*layout, LAYOUT, "the rules of layout {LAYOUT} are not listed");
    assert_eq!(rules.join(" "), expected.split_whitespace().collect::<Vec<_>>().join(" "), "RULES changed: raise LAYOUT");
    // The names sort as the files do.
    assert!(RULES.windows(2).all(|w| w[0].0 < w[1].0));
}

#[test]
fn an_index_names_its_layout() {
    let mut files = split(&demo_project()).unwrap();
    let mut index: serde_json::Value = serde_json::from_slice(&files[INDEX]).unwrap();
    index.as_object_mut().unwrap().remove("layout");
    files.insert(INDEX.into(), line(&index).unwrap());
    let err = join(&files).unwrap_err();
    assert!(err.contains("no file layout"), "{err}");
    index["layout"] = 0.into();
    files.insert(INDEX.into(), line(&index).unwrap());
    assert!(join(&files).is_err());
}

#[test]
fn only_a_project_is_split() {
    // Anything the capture does not take is an error, never an empty project.
    assert!("text".serialize(Capture { skip: &|_| false }).is_err());
    assert!(vec![1].serialize(Capture { skip: &|_| false }).is_err());
    assert!(BTreeMap::from([("a", 1)]).serialize(Capture { skip: &|_| false }).is_err());
    assert!(demo_project().serialize(Capture { skip: &|_| false }).is_ok());
}

#[test]
fn changes_of_files_are_taken_over_into_a_project() {
    let base = project_with_files();
    // They: a page changed, a page added, a page removed, a device changed, the name.
    let mut theirs = base.clone();
    theirs.pages[2].description = "Theirs".into();
    let new = theirs.new_id();
    theirs.pages.insert(1, Page::new(new, "T", "Theirs new", PageKind::Schematic));
    let removed = theirs.pages.remove(5);
    let tag = theirs.devices.keys().next().unwrap().clone();
    theirs.devices.get_mut(&tag).unwrap().description = "Theirs".into();
    theirs.info.name = "Theirs".into();
    // We: another page and the parts.
    let mut ours = base.clone();
    ours.pages[0].description = "Ours".into();
    ours.parts.pop();
    let (b, t) = (split(&base).unwrap(), split(&theirs).unwrap());
    let names: BTreeSet<String> = b.keys().chain(t.keys()).filter(|name| b.get(*name) != t.get(*name)).cloned().collect();
    assert!(names.contains(INDEX) && names.contains(&format!("pages/{new}.json")) && names.contains(&format!("pages/{}.json", removed.id)));

    let merged = take_over(&ours, &theirs, &names);
    assert_eq!(merged.info.name, "Theirs");
    assert_eq!(merged.pages.iter().map(|p| p.id).collect::<Vec<_>>(), theirs.pages.iter().map(|p| p.id).collect::<Vec<_>>());
    assert_eq!((merged.pages[0].description.as_str(), merged.pages[3].description.as_str()), ("Ours", "Theirs"));
    assert_eq!(merged.devices[&tag].description, "Theirs");
    assert_eq!(merged.parts, ours.parts);
    // It is the project of the files of both.
    let mut files = split(&ours).unwrap();
    for name in &names {
        match t.get(name) {
            Some(content) => files.insert(name.clone(), content.clone()),
            None => files.remove(name),
        };
    }
    assert!(join(&files).unwrap().0 == merged);
    // Nothing of theirs: the project as it is; all of theirs: their project.
    assert!(take_over(&ours, &theirs, &BTreeSet::new()) == ours);
    let all: BTreeSet<String> = b.keys().chain(t.keys()).cloned().collect();
    assert!(take_over(&ours, &theirs, &all) == theirs);
    // Without their index (merged by hand): their new page comes last, the removed one goes.
    let mut partly = names.clone();
    partly.remove(INDEX);
    let merged = take_over(&ours, &theirs, &partly);
    assert_eq!(merged.info.name, base.info.name);
    assert_eq!(merged.pages.last().unwrap().id, new);
    assert!(!merged.pages.iter().any(|p| p.id == removed.id));
}

#[test]
fn older_project_formats_are_converted() {
    let mut files = split(&demo_project()).unwrap();
    let mut index: serde_json::Value = serde_json::from_slice(&files[INDEX]).unwrap();
    index["project"]["format"] = 1.into();
    files.insert(INDEX.into(), line(&index).unwrap());
    let (prj, conv) = join(&files).unwrap();
    assert_eq!((conv.from, prj.format), (1, FORMAT));
    assert!(conv.converted());
}
