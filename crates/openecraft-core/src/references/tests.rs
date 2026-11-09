use super::*;
use crate::geom::p;
use crate::model::Page;
use crate::symlib::UserSymbol;
use crate::symbols::{Anchor, BlockAlign, ContactKind, FieldKind, PropertyBlock};

fn circuit() -> Project {
    let mut prj = Project::default();
    prj.symbols.extend([
        UserSymbol { key: "user:main".into(), func: Func::Main { mirror: false }, ..Default::default() },
        UserSymbol { key: "user:aux".into(), func: Func::Aux(ContactKind::No), show_tag: false,
            prims: vec![Prim::PropertyBlock(PropertyBlock { rows: vec![BlockRow::Field(FieldKind::Tag), BlockRow::Property(PROPERTY.into())],
                pos:p(0.,0.),step:p(0.,3.),size:2.,anchor:Anchor::Left,angle:0.,align:BlockAlign::Start,color:None })], ..Default::default() },
    ]);
    prj.pages = vec![Page { name: "11".into(), symbols: vec![SymbolInst { id: 1, sym: "user:main".into(), tag: "-K1".into(), pos: p(40., 40.), ..Default::default() }], ..Default::default() },
        Page { name: "12".into(), symbols: vec![SymbolInst { id: 2, sym: "user:aux".into(), tag: "-K1".into(), pos: p(80., 40.), ..Default::default() }], ..Default::default() }];
    prj
}

#[test]
fn fields_follow_page_edits_and_missing_main_functions_after_reopen() {
    let mut prj = circuit();
    let expected = Index::build(&prj).devices["-K1"].mains[0].reference_from(&prj, 1);
    assert_eq!(property(&prj, 1, &prj.pages[1].symbols[0]), expected);
    let def = prj.symbol("user:aux").unwrap();
    assert!(has_field(&def, &prj.pages[1].symbols[0]));
    prj.pages[0].name = "37a".into();
    prj = Project::from_json(&prj.to_json()).unwrap();
    let current = property(&prj, 1, &prj.pages[1].symbols[0]);
    assert!(current.contains("37a"));assert!(!current.contains("/11."));
    prj.pages[0].symbols.clear();
    assert_eq!(property(&prj, 1, &prj.pages[1].symbols[0]), "?");
}

#[test]
fn computed_property_cannot_be_frozen_by_stored_values_or_written() {
    let mut prj = circuit();
    prj.pages[1].symbols[0].values.insert(PROPERTY.into(), "old cached sheet".into());
    let object = crate::properties::Object::Function(2);
    assert_eq!(crate::properties::get(&prj, &object, PROPERTY), property(&prj, 1, &prj.pages[1].symbols[0]));
    assert!(crate::properties::set(&mut prj, &object, PROPERTY, "stale").is_err());
}
