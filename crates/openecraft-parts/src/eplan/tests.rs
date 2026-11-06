use super::*;

#[test]
fn rail_offsets_follow_the_article_and_matching_beckhoff_housing() {
    let xml = r#"<partsmanagement><part P_ARTICLE_PARTNR="BEC.EL3174" P_ARTICLE_HEIGHT="100" P_ARTICLE_WIDTH="12" P_ARTICLE_DEPTH="68" P_ARTICLE_MIDDLEOFFSET="-4" P_ARTICLE_SNAPHEIGHT="2"/></partsmanagement>"#;
    let mut p = from_eplan_xml(xml).unwrap().remove(0);
    p.eplan_variants.push(
        serde_json::json!({"function_type":4,"symbol":{"pins":(1..=16).map(|i|serde_json::json!({"label":i.to_string()})).collect::<Vec<_>>()}})
            .into(),
    );
    assert_eq!(p.effective_rail_mount(), crate::RailMount { center_offset: -4., clip_on: 2. });
    let saved = from_eplan_xml(&to_eplan_xml(&[p.clone()])).unwrap();
    assert_eq!(saved[0].mounting_data.rail, p.mounting_data.rail);
    p.mounting_data.rail.center_offset = 0.;
    assert!((p.effective_rail_mount().center_offset - 13.8).abs() < 0.001);
    p.number = "BEC.EP3174".into();
    assert_eq!(p.effective_rail_mount().center_offset, 0.);
}

const SAMPLE: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<partsmanagement>
  <part P_ARTICLE_PARTNR="SIE.3RT2015-1BB41" P_ARTICLE_TYPENR="3RT2015-1BB41" P_ARTICLE_MANUFACTURER="SIE"
        P_ARTICLE_DESCR1="de_DE@Leistungsschütz;en_US@Power contactor;" P_ARTICLE_NOTE="de_DE@Spule 24 V DC;"
        P_ARTICLE_SALESPRICE_1="42,50" P_ARTICLE_WIDTH="45" P_ARTICLE_HEIGHT="57.5" P_ARTICLE_DEPTH="73" P_ARTICLE_WEIGHT="0,2"
        P_ARTICLE_VOLTAGE="24 V DC" P_ARTICLE_PRODUCTTOPGROUP="1" P_ARTICLE_PRODUCTGROUP="1" P_ARTICLE_PRODUCTSUBGROUP="0" />
  <part P_ARTICLE_PARTNR="PXC.UT2,5" P_ARTICLE_DESCR1="Feed-through terminal">
    <variant P_ARTICLE_VARIANT="1" P_ARTICLE_PARTNR="PXC.UT2,5-BU" P_ARTICLE_DESCR1="Feed-through terminal, blue" />
    <variant P_ARTICLE_VARIANT="2" P_ARTICLE_PARTNR="PXC.UT2,5-GY" />
  </part>
  <part P_ARTICLE_TYPENR="no number" />
</partsmanagement>"#;

#[test]
fn multi_language_texts() {
    assert_eq!(language_text("de_DE@Schütz;en_US@Contactor;"), "Contactor");
    assert_eq!(language_text("fr_FR@Contacteur;de_DE@Schütz;"), "Schütz");
    assert_eq!(language_text("fr_FR@Contacteur;"), "Contacteur");
    assert_eq!(language_text(" plain "), "plain");
    assert_eq!(language_text("mail@example.org"), "mail@example.org", "an @ without a language code is part of the text");
}

#[test]
fn parts_and_variants_are_read() {
    assert!(is_eplan_xml(SAMPLE) && !is_eplan_xml("Number;Description") && !is_eplan_xml("<BMECAT/>"));
    let parts = from_eplan_xml(SAMPLE).unwrap();
    assert_eq!(parts.len(), 3, "one part and two variants; the part without number is left out");
    let c = &parts[0];
    assert_eq!((c.number.as_str(), c.type_number.as_str(), c.manufacturer.as_str()), ("SIE.3RT2015-1BB41", "3RT2015-1BB41", "SIE"));
    assert_eq!((c.description.as_str(), c.note.as_str()), ("Power contactor", "Spule 24 V DC"));
    assert_eq!((c.price, c.width, c.height, c.depth, c.weight), (42.5, 45.0, 57.5, 73.0, 0.2));
    assert_eq!(c.voltage, "24 V DC");
    assert_eq!(c.top_group, crate::TopGroup::Electrical);
    assert_eq!(c.group, "General");
    assert_eq!(parts[1].number, "PXC.UT2,5-BU");
    assert_eq!(parts[1].description, "Feed-through terminal, blue");
    assert_eq!(parts[2].description, "Feed-through terminal", "variants take what they do not set from the part");
    assert!(from_eplan_xml("<partsmanagement/>").is_err());
    // The generic import recognises the format.
    assert_eq!(crate::import(SAMPLE).unwrap().len(), 3);
}

#[test]
fn round_trip_through_eplan_xml() {
    let mut parts = from_eplan_xml(SAMPLE).unwrap();
    parts[1].accessory = true;
    let xml = to_eplan_xml(&parts);
    assert!(xml.contains("P_ARTICLE_DESCR1=\"en_US@Power contactor;\""));
    assert!(xml.contains("P_ARTICLE_PRODUCTTOPGROUP=\"1\""));
    let back = from_eplan_xml(&xml).unwrap();
    assert_eq!(back.len(), parts.len());
    for (a, b) in parts.iter().zip(&back) {
        assert_eq!((&a.number, &a.description, a.price, a.width, &a.group, a.accessory), (&b.number, &b.description, b.price, b.width, &b.group, b.accessory));
    }
}

#[test]
fn language_neutral_texts() {
    assert_eq!(language_text("??_??@{{LOGO! 24 CE}};//??_??@LOGO! 24 CE;"), "LOGO! 24 CE");
    assert_eq!(language_text("??_??@{{Neutral}};de_DE@Deutsch;"), "Deutsch", "German comes before neutral texts");
}

const DEVICE: &str = r#"<partsmanagement>
 <part P_ARTICLE_PARTNR="SE.DRIVE" P_ARTICLE_HEIGHT="250" P_ARTICLE_WIDTH="90" P_ARTICLE_MOUNTINGSITE="0"
       P_ARTICLE_SPACING_ABOVE="50" P_ARTICLE_SPACING_BELOW="40" P_ARTICLE_SPACING_LEFT="1"
       P_ARTICLE_REF_TERMINAL_NAME="SE.DRIVE" P_ARTICLE_REF_CONSTRUCTION_NAME="SE.DRIVE">
  <variant P_ARTICLE_POWERDISSIPATION="240 W">
   <functiontemplate functiondefcategory="301" functiondefgroup="1" functiondefid="1"/>
   <functiontemplate connectionDesignation="L1C" functiondefcategory="1302" functiondefgroup="1" functiondefid="1"/>
   <functiontemplate connectionDesignation="1" connectiondescription="DO4+" functiondefcategory="300" functiondefgroup="1" functiondefid="3" terminalNr="CN1"/>
   <functiontemplate connectionDesignation="8" connectiondescription="DI4" functiondefcategory="300" functiondefgroup="1" functiondefid="2" terminalNr="CN1"/>
   <functiontemplate connectionDesignation="18" connectiondescription="AI2" functiondefcategory="300" functiondefgroup="1" functiondefid="4" terminalNr="CN1"/>
   <functiontemplate connectionDesignation="31" connectiondescription="AO1" functiondefcategory="300" functiondefgroup="1" functiondefid="5" terminalNr="CN1"/>
   <functiontemplate connectionDesignation="44" connectiondescription="OCZ" functiondefcategory="300" functiondefgroup="1" functiondefid="6" terminalNr="CN1"/>
  </variant>
 </part>
</partsmanagement>"#;

const DEVICE_POINTS: &str = r#"<partsmanagement>
 <terminal P_PART_TERMINAL_NAME="SE.DRIVE">
  <terminalPosition name="L1C" xdir="-1" xpos="18.75" ydir="0" ypos="226" zdir="0" zpos="215"/>
  <terminalPosition name="PE" xdir="0" xpos="56.75" ydir="0" ypos="9" zdir="1" zpos="232"/>
 </terminal>
 <construction P_PART_CONSTRUCTION_NAME="SE.DRIVE">
  <drillingPosition dimension1="5" xpos="6" ypos="4.75"/>
  <drillingPosition dimension1="5" xpos="84" ypos="245.25"/>
 </construction>
</partsmanagement>"#;

#[test]
fn function_templates_connection_points_and_holes() {
    let parts = from_eplan_documents(&[DEVICE, DEVICE_POINTS]).unwrap();
    let p = &parts[0];
    let f: Vec<(crate::FnKind, &str)> = p.functions.iter().map(|f| (f.kind, f.pins.as_str())).collect();
    use crate::FnKind::*;
    assert_eq!(f, [(Generic, "L1C"), (PlcOutput, "CN1.1"), (PlcInput, "CN1.8"), (PlcAnalogInput, "CN1.18"), (PlcAnalogOutput, "CN1.31"), (Generic, "CN1.44")]);
    assert_eq!(p.functions[1].description, "DO4+");
    let cp: Vec<(&str, [f32; 3], crate::Exit)> = p.connection_points.iter().map(|c| (c.pin.as_str(), c.pos, c.exit)).collect();
    assert_eq!(cp, [("L1C", [18.75, 24.0, 215.0], crate::Exit::Left), ("PE", [56.75, 241.0, 232.0], crate::Exit::Front)]);
    let md = &p.mounting_data;
    assert_eq!(md.holes, vec![[6.0, 245.25], [84.0, 4.75]], "y down from the upper edge");
    assert_eq!(md.hole_thread, "Ø5");
    assert_eq!((md.clearance_top, md.clearance_bottom, md.clearance_left), (50.0, 40.0, 1.0));
    assert!(p.mounting.is_empty());
    assert_eq!(p.tech_value("Power loss"), Some("240 W"));
    // In one file, and without the other documents.
    assert_eq!(from_eplan_xml(&format!("<root>{}{}</root>", &DEVICE[DEVICE.find("<part ").unwrap()..DEVICE.find("</partsmanagement>").unwrap()], &DEVICE_POINTS[DEVICE_POINTS.find("<terminal ").unwrap()..DEVICE_POINTS.find("</partsmanagement>").unwrap()])).unwrap()[0].connection_points.len(), 2);
    assert!(from_eplan_xml(DEVICE).unwrap()[0].connection_points.is_empty());
}

#[test]
fn semicolons_and_lines_survive_the_eplan_round_trip() {
    let p = Part { number: "X.1".into(), description: "Contactor; 3 poles".into(), tech_data: "Rated current: 9 A\nCoil: 24 V DC".into(), ..Default::default() };
    let back = from_eplan_xml(&to_eplan_xml(&[p])).unwrap();
    assert_eq!((back[0].description.as_str(), back[0].tech_data.as_str()), ("Contactor; 3 poles", "Rated current: 9 A\nCoil: 24 V DC"));
}

#[test]
fn a_semicolon_ends_an_entry_only_before_a_language_code() {
    assert_eq!(language_text("de_DE@Schütz; 3-polig;en_US@Contactor; 3 poles;"), "Contactor; 3 poles");
    assert_eq!(language_text("en_US@a;;"), "a;");
    assert_eq!(language_text("//??_??@{{LOGO!}};"), "LOGO!");
}
