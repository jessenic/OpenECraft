use super::*;

#[test]
fn wire_labels_read_live_attributes_and_keep_their_position_when_rebased() {
    let mut wire = Wire {a:p(10.,20.),b:p(50.,20.),core:"BN".into(),..Default::default()};
    let mut label = WireLabel {along:0.25,offset:p(-0.5,-0.25),..Default::default()};
    assert_eq!(label.text(&wire),"BN");
    wire.core="BU".into();
    assert_eq!(label.text(&wire),"BU");
    let position = label.position(&wire);
    label.rebase(&wire,p(80.,20.),p(10.,20.));
    wire.a=p(80.,20.);wire.b=p(10.,20.);
    assert_eq!(label.position(&wire),position);
    assert!(label.is_valid());
    label.along=f32::NAN;
    assert!(!label.is_valid());
}
