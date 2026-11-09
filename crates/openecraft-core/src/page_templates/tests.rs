use super::*;
use crate::geom::p;
use crate::model::{SymbolInst, TextItem};

#[test]
fn builtin_templates_set_type_and_frame() {
    let mut prj = Project::default();
    let t = find(&prj, "Graphics A4 portrait").unwrap();
    let at = insert(&mut prj, &t, 1);
    let pg = &prj.pages[at];
    assert_eq!((pg.kind, pg.frame.as_str(), pg.description.as_str()), (PageKind::Graphic, "A4 portrait", "Graphics"));
    assert!(pg.is_editable());
    assert_eq!(crate::frame::of_page(&prj, at).size(), (210.0, 297.0));
    assert!(find(&prj, "Nothing").is_none());
}

#[test]
fn pages_are_saved_as_templates_with_their_content() {
    let mut prj = Project::default();
    prj.pages[0].frame = "A4 landscape".into();
    prj.pages[0].title_block = "Compact".into();
    prj.pages[0].function = "A1".into();
    prj.pages[0].values.insert("page.drawing_number".into(),"DOC-7".into());
    let id = prj.new_id();
    prj.pages[0].texts.push(TextItem { id, pos: p(30.0, 40.0), text: "Note".into(), size: 3.0, ..Default::default() });
    let id = prj.new_id();
    prj.pages[0].symbols.push(SymbolInst { id, sym: "coil".into(), pos: p(60.0, 60.0), tag: "=A1-K1".into(), ..Default::default() });
    let t = from_page(&prj, 0, " Motor page ");
    save(&mut prj, t);
    assert_eq!(all(&prj)[0].name, "Motor page");

    let t = find(&prj, "Motor page").unwrap();
    let at = insert(&mut prj, &t, 1);
    let pg = &prj.pages[at];
    assert_eq!((pg.frame.as_str(), pg.title_block.as_str(), pg.function.as_str()), ("A4 landscape", "Compact", "A1"));
    assert_eq!(pg.texts[0].pos, p(30.0, 40.0));
    assert_eq!(pg.values["page.drawing_number"],"DOC-7");
    // The device exists already: the copy gets the next number.
    assert_eq!(pg.symbols[0].tag, "=A1-K2");

    let lib = Library { page_templates: prj.page_templates.clone() };
    let mut other = Project::default();
    assert_eq!(merge(&mut other, Library::from_json(&lib.to_json()).unwrap()), 1);
    assert_eq!(find(&other,"Motor page").unwrap().values["page.drawing_number"],"DOC-7");
    assert!(remove(&mut other, "Motor page") && !remove(&mut other, "Motor page"));
    assert!(Library::from_json("42").is_err());
}
