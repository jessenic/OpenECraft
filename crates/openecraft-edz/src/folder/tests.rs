use super::*;

#[test]
fn references_only_select_indexed_macros_and_ambiguity_is_reported() {
    let folder = Folder { macros: vec!["catalog/a/test.ema".into(), "catalog/b/test.ema".into()], xml: Vec::new() };
    assert_eq!(folder.resolve("$(MD_MACROS)\\b\\TEST.ema", Path::new("catalog/db.xml")).unwrap(), Path::new("catalog/b/test.ema"));
    assert!(folder.resolve("test.ema", Path::new("catalog/db.xml")).is_err());
    assert!(folder.resolve("../missing.ema", Path::new("catalog/db.xml")).is_err());
}
