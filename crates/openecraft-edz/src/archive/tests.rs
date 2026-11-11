use super::*;
use openecraft_parts::{Exit, FnKind};
use sevenz_rust2::{ArchiveEntry, ArchiveWriter};

const MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<manifest version="2.0">
 <packages>
  <package type="part" key="SIE.LOGO" name="SIE.LOGO">
   <items>
    <item type="partxml" name="part" locator="SIE.LOGO.part.xml"/>
    <item type="macro" name="graphicmacro" locator="Siemens\LOGO\LOGO_3D.ema"/>
    <item type="picture" name="picturefile" locator="Siemens\LOGO!\logo.png"/>
    <item type="partxml" name="connectionpoints" locator="SIE.LOGO.connectionpoints.xml"/>
   </items>
  </package>
  <package type="construction" key="SIE.LOGO_71.5x90" name="SIE.LOGO_71.5x90">
   <items>
    <item type="partxml" name="construction" locator="SIE.LOGO_71.5x90.construction.xml"/>
   </items>
  </package>
 </packages>
</manifest>"#;

const PART: &str = r#"<?xml version="1.0" encoding="utf-8" ?>
<partsmanagement>
 <part P_ARTICLE_PARTNR="SIE.LOGO" P_ARTICLE_TYPENR="6ED1052-1CC08-0BA3" P_ARTICLE_MANUFACTURER="SIE"
       P_ARTICLE_DESCR1="??_??@{{LOGO! 24 CE}};//??_??@LOGO! 24 CE;" P_ARTICLE_HEIGHT="90" P_ARTICLE_WIDTH="71.5" P_ARTICLE_DEPTH="58"
       P_ARTICLE_MOUNTINGSITE="0" P_ARTICLE_PRODUCTTOPGROUP="1" P_ARTICLE_PRODUCTGROUP="26" P_ARTICLE_PRODUCTSUBGROUP="1"
       P_ARTICLE_SPACING_ABOVE="50" P_ARTICLE_IDENTCODE="4034106036768" P_ARTICLE_IDENTTYPE="GTIN"
       P_ARTICLE_EXTERNAL_DOCUMENT_1="https://example.com/datasheet&#10;en_US@en@@datasheet;"
       P_ARTICLE_REF_CONSTRUCTION_NAME="SIE.LOGO_71.5x90" P_ARTICLE_REF_TERMINAL_NAME="SIE.LOGO.cp">
  <constructionPosition offsetx="0" offsety="-4" pos="1"/>
  <variant P_ARTICLE_VARIANT="1" P_ARTICLE_VOLTAGE="24 V" P_ARTICLE_POWERDISSIPATION="1.32 W">
   <functiontemplate functiondefcategory="301" functiondefgroup="1" functiondefid="1" pos="1"/>
   <functiontemplate connectionDesignation="1" connectiondescription="L+" functiondefcategory="300" functiondefgroup="2" functiondefid="5" pos="2" terminalNr="X10"/>
   <functiontemplate connectionDesignation="3" connectiondescription="I1" functiondefcategory="300" functiondefgroup="1" functiondefid="6" pos="3" terminalNr="X10"/>
   <functiontemplate connectionDesignation="5" connectiondescription="I3" functiondefcategory="300" functiondefgroup="1" functiondefid="2" pos="4" terminalNr="X10"/>
   <functiontemplate connectionDesignation="1" connectiondescription="Q1" functiondefcategory="300" functiondefgroup="1" functiondefid="3" pos="5" terminalNr="X11"/>
   <functiontemplate connectionDesignation="FE" functiondefcategory="300" functiondefgroup="2" functiondefid="4" pos="6"/>
  </variant>
 </part>
</partsmanagement>"#;

const POINTS: &str = r#"<partsmanagement>
 <terminal P_PART_TERMINAL_NAME="SIE.LOGO.cp">
  <terminalPosition devicetag="X10" name="1" xdir="0" xpos="10.35" ydir="1" ypos="85.47" zdir="0" zpos="14.9"/>
  <terminalPosition devicetag="X11" name="1" xdir="0" xpos="10.35" ydir="-1" ypos="4.53" zdir="0" zpos="14.9"/>
  <terminalPosition name="FE" xdir="-1" xpos="0" ydir="0" ypos="23.5" zdir="0" zpos="35"/>
 </terminal>
</partsmanagement>"#;

const CONSTRUCTION: &str = r#"<partsmanagement>
 <construction P_PART_CONSTRUCTION_NAME="SIE.LOGO_71.5x90">
  <drillingPosition dimension1="3.3" type="1" xpos="35.8" ypos="0"/>
  <drillingPosition dimension1="3.3" type="1" xpos="35.8" ypos="98"/>
 </construction>
</partsmanagement>"#;

/// A 7-Zip archive with these files.
fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = ArchiveWriter::new(Cursor::new(Vec::new())).unwrap();
    for (name, data) in files {
        w.push_archive_entry(ArchiveEntry::new_file(name), Some(*data)).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn logo() -> Vec<u8> {
    archive(&[
        ("manifest.xml", MANIFEST.as_bytes()),
        ("items/partxml/SIE.LOGO.part.xml", PART.as_bytes()),
        ("items/partxml/SIE.LOGO.connectionpoints.xml", POINTS.as_bytes()),
        ("items/partxml/SIE.LOGO_71.5x90.construction.xml", CONSTRUCTION.as_bytes()),
        ("items/partxml/SIE.manufacturer.xml", b"<partsmanagement><manufacturer P_MANUFACTURER_SHORTNAME=\"SIE\"/></partsmanagement>"),
        ("items/macro/Siemens/LOGO/LOGO_3D.ema", b"<EplanPxfRoot/>"),
        ("items/picture/Siemens/LOGO!/logo.png", b"PNG"),
    ])
}

#[test]
fn the_part_of_an_edz_file_is_read_with_its_data() {
    let bytes = logo();
    assert!(is_edz(&bytes) && !is_edz(b"Number;Description"));
    let edz = read(&bytes).unwrap();
    assert_eq!(edz.parts.len(), 1);
    let p = &edz.parts[0];
    assert_eq!((p.number.as_str(), p.type_number.as_str(), p.description.as_str()), ("SIE.LOGO", "6ED1052-1CC08-0BA3", "LOGO! 24 CE"), "the language neutral text without braces");
    assert_eq!((p.width, p.height, p.depth), (71.5, 90.0, 58.0));
    assert_eq!(p.voltage, "24 V");
    assert!(p.mounting.is_empty(), "mounting site 0 is none");
    assert_eq!(p.datasheet, "https://example.com/datasheet");
    assert_eq!(p.tech_value("GTIN"), Some("4034106036768"));
    assert_eq!(p.mounting_data.power_loss, 1.32);
    assert_eq!(p.mounting_data.clearance_top, 50.0);
    // Function templates: the PLC box has no connection point.
    let f: Vec<(FnKind, &str, &str)> = p.functions.iter().map(|f| (f.kind, f.pins.as_str(), f.description.as_str())).collect();
    assert_eq!(
        f,
        [
            (FnKind::Generic, "X10.1", "L+"),
            (FnKind::PlcInput, "X10.3", "I1"),
            (FnKind::PlcInput, "X10.5", "I3"),
            (FnKind::PlcOutput, "X11.1", "Q1"),
            (FnKind::Generic, "FE", ""),
        ]
    );
    // Connection points in the front view: y down from the upper edge.
    let cp: Vec<(&str, [f32; 3], Exit)> = p.connection_points.iter().map(|c| (c.pin.as_str(), c.pos, c.exit)).collect();
    assert_eq!(cp.len(), 3);
    assert_eq!(cp[0].0, "X10.1");
    assert!((cp[0].1[1] - 4.53).abs() < 1e-3 && cp[0].2 == Exit::Up);
    assert!((cp[1].1[1] - 85.47).abs() < 1e-3 && cp[1].2 == Exit::Down);
    assert_eq!((cp[2].0, cp[2].2), ("FE", Exit::Left));
    // Holes of the drilling pattern, moved by its offset: 4 mm below and above the part.
    assert_eq!(p.mounting_data.holes, vec![[35.8, 94.0], [35.8, -4.0]]);
    assert_eq!(p.mounting_data.hole_thread, "Ø3.3");
    // Picture and macro belong to the part; the parts XML files are not attached.
    let files: Vec<(&str, &str, AttachmentKind)> = edz.files.iter().map(|f| (f.part.as_str(), f.name.as_str(), f.kind)).collect();
    assert_eq!(files, [("SIE.LOGO", "LOGO_3D.ema", AttachmentKind::Other), ("SIE.LOGO", "logo.png", AttachmentKind::Image)]);
    assert_eq!(edz.files[1].data, b"PNG");
}

#[test]
fn without_manifest_the_files_belong_to_the_only_part() {
    let bytes = archive(&[("items/partxml/p.part.xml", PART.as_bytes()), ("items/picture/x/logo.png", b"PNG")]);
    let edz = read(&bytes).unwrap();
    assert_eq!(edz.files.len(), 1);
    assert_eq!((edz.files[0].part.as_str(), edz.files[0].kind), ("SIE.LOGO", AttachmentKind::Image));
    assert!(edz.parts[0].connection_points.is_empty(), "the connection point file is missing");
}

#[test]
fn archives_without_parts_and_other_files_are_refused() {
    assert!(read(&archive(&[("items/picture/logo.png", b"PNG")])).is_err());
    assert!(read(b"7z\xBC\xAF\x27\x1Cbroken").is_err());
    assert!(read(b"no archive").is_err());
}

#[test]
fn import_reads_edz_and_text_files() {
    assert_eq!(import(&logo()).unwrap()[0].number, "SIE.LOGO");
    assert_eq!(import(PART.as_bytes()).unwrap()[0].number, "SIE.LOGO");
    assert_eq!(import(b"Part number;Description\nX.1;Test\n").unwrap()[0].number, "X.1");
}

#[test]
fn archives_unpacking_to_too_much_are_refused() {
    let zeros = vec![0u8; 1 << 16];
    let bytes = archive(&[("items/a.bin", &zeros), ("items/b.bin", &zeros)]);
    assert_eq!(unpack_with_limit(&bytes, 1 << 17).unwrap().len(), 2);
    assert!(unpack_with_limit(&bytes, (1 << 17) - 1).unwrap_err().contains("more than"));
}

/// Vendor archives that may not be checked in (Siemens EDZ files of issue #131): every
/// part with a schematic macro is placed with a symbol of its macro, none falls back to
/// the function templates.
#[test]
#[ignore = "requires OPENECRAFT_EDZ_DIR pointing to a folder of EDZ files"]
fn vendor_archives_place_their_parts_with_the_macro_symbol() {
    let dir = std::env::var("OPENECRAFT_EDZ_DIR").unwrap();
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if !path.extension().is_some_and(|e| e.eq_ignore_ascii_case("edz")) {
            continue;
        }
        for part in read(&std::fs::read(&path).unwrap()).unwrap().parts {
            let schematic = part.eplan_variants.iter().any(|v| v["function_type"] == 1 && v["symbol"]["pins"].as_array().is_some_and(|p| !p.is_empty()));
            if !schematic {
                continue;
            }
            let mut prj = openecraft_core::model::Project::default();
            let (key, pins) = openecraft_core::symlib::ema::part_symbol(&mut prj, &part, None).unwrap_or_else(|| panic!("{}: no symbol", part.number));
            assert!(key.starts_with("user:ema:"), "{}: {key}", part.number);
            assert!(!pins.is_empty(), "{}", part.number);
            checked += 1;
        }
    }
    assert!(checked > 0, "no EDZ file with a schematic macro");
}
