use super::*;

#[test]
fn literal_part_counts_preserve_zero_and_default_legacy_counts() {
    let old:PartRef=serde_json::from_str(r#"{"number":"LEGACY","count":0}"#).unwrap();
    assert_eq!(old.quantity(),1);assert!(!old.total);
    let literal=PartRef {number:"ZERO".into(),count:0,total:true};
    assert_eq!(literal.quantity(),0);
    assert_eq!(serde_json::from_str::<PartRef>(&serde_json::to_string(&literal).unwrap()).unwrap(),literal);
    assert!(!serde_json::to_string(&old).unwrap().contains("total"));
}

#[test]
fn merge_adds_and_updates() {
    let mut db = PartsDb { parts: vec![Part { number: "A".into(), description: "old".into(), ..Default::default() }] };
    let incoming = vec![
        Part { number: "A".into(), description: "new".into(), ..Default::default() },
        Part { number: "B".into(), ..Default::default() },
    ];
    assert_eq!(db.merge(incoming.clone()), (1, 1));
    assert_eq!(db.get("A").unwrap().description, "new");
    // Same data again: nothing changes.
    assert_eq!(db.merge(incoming), (0, 0));
}

#[test]
fn unique_numbers() {
    let list = vec![Part { number: "X".into(), ..Default::default() }, Part { number: "X_1".into(), ..Default::default() }];
    assert_eq!(unique_number(&list, "Y"), "Y");
    assert_eq!(unique_number(&list, "X"), "X_2");
}

#[test]
fn technical_data_values() {
    let p = crate::default_catalog().into_iter().find(|p| p.number == "GEN.CBL-4G1.5").unwrap();
    assert_eq!(p.tech_value("cores"), Some("BN BK GY GNYE"));
    assert_eq!(p.tech_value("Cross-section"), Some("1.5 mm²"));
    assert_eq!(p.tech_value("Voltage"), None);
    assert_eq!(p.quantity_unit, "m");
}

#[test]
fn function_kinds_have_unique_titles_and_short_names() {
    let titles: std::collections::BTreeSet<_> = FnKind::ALL.iter().map(|k| k.title()).collect();
    assert_eq!(titles.len(), FnKind::ALL.len());
    for k in FnKind::ALL {
        assert_eq!(FnKind::from_short(k.short()), Some(k), "{}", k.title());
    }
}

#[test]
fn catalog_uses_the_function_kinds() {
    // Every kind except the generic one appears in a part of the example catalog.
    let parts = crate::default_catalog();
    for k in FnKind::ALL.into_iter().filter(|k| *k != FnKind::Generic) {
        assert!(parts.iter().any(|p| p.functions.iter().any(|f| f.kind == k)), "no part with {}", k.title());
    }
}
