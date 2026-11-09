use super::*;

#[test]
fn property_fields_fill_the_row_and_keep_their_least_width() {
    // A wide dialog: the fields share the row (two fifths for the name).
    let (name, value) = property_widths(600.0, 20.0, 8.0);
    assert!((name + value + 20.0 + 2.0 * 8.0 - 600.0).abs() < 0.01);
    assert!((name - 0.4 * 564.0).abs() < 0.01);
    // A narrow one (or the first frame) never squeezes them to a few characters.
    let (name, value) = property_widths(40.0, 20.0, 8.0);
    assert_eq!((name, value), (PROPERTY_NAME_WIDTH, PROPERTY_VALUE_WIDTH));
    let (name, value) = property_widths(0.0, 20.0, 8.0);
    assert_eq!((name, value), (PROPERTY_NAME_WIDTH, PROPERTY_VALUE_WIDTH));
}

#[test]
fn project_property_renames_keep_the_value() {
    let mut props: std::collections::BTreeMap<String, String> =
        [("Drawing number", "Example drawing 12345"), ("Rated voltage", "400 V")].into_iter().map(|(k, v)| (k.into(), v.into())).collect();
    assert!(rename_property(&mut props, "Rated voltage", "Voltage".into()));
    assert_eq!(props.get("Voltage").map(String::as_str), Some("400 V"));
    assert!(!props.contains_key("Rated voltage"));
    // Empty and taken names are refused, the properties stay as they were.
    let before = props.clone();
    assert!(!rename_property(&mut props, "Voltage", String::new()));
    assert!(!rename_property(&mut props, "Voltage", "Drawing number".into()));
    assert!(!rename_property(&mut props, "Missing", "Other".into()));
    assert_eq!(props, before);
}
