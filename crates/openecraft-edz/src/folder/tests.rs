use super::*;

#[test]
fn references_only_select_indexed_macros_and_ambiguity_is_reported() {
    let folder = Folder { macros: vec!["catalog/a/test.ema".into(), "catalog/b/test.ema".into()], xml: Vec::new(), pictures: Vec::new() };
    assert_eq!(folder.resolve("$(MD_MACROS)\\b\\TEST.ema", Path::new("catalog/db.xml")).unwrap(), Path::new("catalog/b/test.ema"));
    assert!(folder.resolve("test.ema", Path::new("catalog/db.xml")).is_err());
    assert!(folder.resolve("../missing.ema", Path::new("catalog/db.xml")).is_err());
}

/// An unpacked EDZ file in a new temporary folder.
fn unpacked(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("openecraft-edz-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let write = |path: &str, data: &[u8]| {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, data).unwrap();
    };
    write(
        "items/partxml/SIE.HMI.part.xml",
        br#"<partsmanagement><part P_ARTICLE_PARTNR="SIE.HMI" P_ARTICLE_HEIGHT="158" P_ARTICLE_WIDTH="214"
            P_ARTICLE_PICTUREFILE="$(MD_IMG)\Siemens\HMI\hmi.png" P_ARTICLE_REF_TERMINAL_NAME="SIE.HMI" P_ARTICLE_REF_CONSTRUCTION_NAME="SIE.HMI"/></partsmanagement>"#,
    );
    write(
        "items/partxml/SIE.HMI.connectionpoints.xml",
        br#"<partsmanagement><terminal P_PART_TERMINAL_NAME="SIE.HMI"><terminalPosition devicetag="X80" name="1" maxcrosssection="1.5" maxwirecount="1" mincrosssection="0.2" xpos="147" ypos="8" ydir="-1"/></terminal></partsmanagement>"#,
    );
    write(
        "items/partxml/SIE.HMI.construction.xml",
        br#"<partsmanagement><construction P_PART_CONSTRUCTION_NAME="SIE.HMI"><drillingPosition dimension1="141" dimension2="197" type="3" xpos="107" ypos="85.5"/></construction></partsmanagement>"#,
    );
    // Another part next to it is not read with the selected one.
    write("items/partxml/SIE.HMI2.part.xml", br#"<partsmanagement><part P_ARTICLE_PARTNR="SIE.HMI2"/></partsmanagement>"#);
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(600, 300, image::Rgba([0, 90, 160, 255])).write_to(&mut png, image::ImageFormat::Png).unwrap();
    write("items/picture/Siemens/HMI/hmi.png", &png.into_inner());
    root
}

#[test]
fn a_part_file_of_an_unpacked_edz_is_read_with_its_points_drilling_and_picture() {
    let root = unpacked("read");
    let path = std::path::absolute(root.join("items/partxml/SIE.HMI.part.xml")).unwrap();
    let edz = read_file(&path, &std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(edz.parts.len(), 1);
    let p = &edz.parts[0];
    let x80 = &p.connection_points[0];
    assert_eq!((x80.pin.as_str(), x80.min_cross_section, x80.max_cross_section, x80.max_wires), ("X80.1", 0.2, 1.5, 1));
    assert_eq!(p.mounting_data.cutouts[0].shape, openecraft_parts::Cutout::Rect { w: 197.0, h: 141.0 });
    assert!(!p.picture.is_empty(), "a small copy of the picture");
    assert_eq!(edz.files.iter().map(|f| (f.name.as_str(), f.kind)).collect::<Vec<_>>(), [("hmi.png", AttachmentKind::Image)]);
    // The same when the article is picked from the indexed folder.
    let single = index(&root).unwrap().part("SIE.HMI").unwrap();
    assert_eq!(single.parts[0].connection_points, p.connection_points);
    assert_eq!(single.parts[0].mounting_data, p.mounting_data);
    assert_eq!(single.parts[0].picture, p.picture);
    std::fs::remove_dir_all(root).unwrap();
}
