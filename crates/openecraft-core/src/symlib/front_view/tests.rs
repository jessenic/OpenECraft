use super::*;

#[test]
fn front_artwork_is_fitted_to_the_part_and_invalidated_when_the_source_changes() {
    let variant = |kind, x| {
        let symbol = UserSymbol {
            key: format!("user:ema:test:{kind}:0"),
            prims: vec![Prim::Line { a: p(-8., 20.), b: p(x, 120.), dashed: false }],
            ..Default::default()
        };
        Shared::new(serde_json::json!({"function_type":kind,"symbol":symbol}))
    };
    let mut part = Part { number: "Card".into(), width: 12., height: 100., eplan_variants: vec![variant(4, 4.)], ..Default::default() };
    let cache = crate::symlib::SymbolCache::default();
    let art = cache.front_view(&part).unwrap();
    assert_eq!(art.prims, [Prim2d::Line { a: [0., 0.], b: [12., 100.] }]);
    assert!(art.ptr_eq(&cache.front_view(&part).unwrap()));
    part.eplan_variants.push(variant(6, -2.));
    let mounting = cache.front_view(&part).unwrap();
    assert_eq!(mounting.prims, [Prim2d::Line { a: [3., 0.], b: [9., 100.] }]);
    part.eplan_variants[1] = variant(6, 4.);
    assert!(!mounting.ptr_eq(&cache.front_view(&part).unwrap()));
    part.model_2d = Some(mounting.clone());
    assert!(mounting.ptr_eq(&cache.front_view(&part).unwrap()));
}
