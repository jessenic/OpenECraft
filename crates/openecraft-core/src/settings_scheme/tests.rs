use super::*;
use crate::sample::demo_project;

#[test]
fn settings_go_from_one_project_to_another() {
    let mut a = demo_project();
    a.wire_colors = true;
    a.wire_settings.jump = crate::model::JumpStyle::Bridge;
    a.wire_settings.reroute = true;
    a.marking.wire_material = "SHOP.WIRE".into();
    a.check_settings.disabled.push("cables".into());
    let json = export(&a);
    let mut b = crate::model::Project::default();
    let taken = apply(&mut b, &json).unwrap();
    assert_eq!(taken.len(), 10);
    assert_eq!(b.potentials, a.potentials);
    assert_eq!(b.wire_numbering, a.wire_numbering);
    assert!(b.wire_colors);
    assert_eq!(b.wire_settings, a.wire_settings);
    assert_eq!(b.marking.wire_material, "SHOP.WIRE");
    assert_eq!(b.check_settings.disabled, vec!["cables".to_string()]);
}

#[test]
fn partial_schemes_change_only_their_parts() {
    let mut b = demo_project();
    let potentials = b.potentials.clone();
    let taken = apply(&mut b, r#"{ "wire_colors": true }"#).unwrap();
    assert_eq!(taken, vec!["Wire colours"]);
    assert!(b.wire_colors);
    assert_eq!(b.potentials, potentials);
    assert!(apply(&mut b, "[]").is_err());
}
