use super::*;

/// Pixels that are clearly drawn (not transparent, not white).
fn ink(image: &ColorImage, x: std::ops::Range<usize>) -> usize {
    let w = image.size[0];
    image.pixels.iter().enumerate().filter(|(i, p)| x.contains(&(i % w)) && p.a() > 200 && (p.r() < 200 || p.g() < 200 || p.b() < 200)).count()
}

#[test]
fn bundled_logos_are_drawn() {
    for id in openecraft_parts::bundled::ids() {
        let svg = openecraft_parts::bundled::logo(id).unwrap();
        let image = decode("logo.svg", svg.as_bytes()).unwrap_or_else(|| panic!("{id}: not decoded"));
        let [w, h] = image.size;
        // Full height, or full width for very wide logos: never cut off.
        assert!(h <= PIXELS as usize && w <= (PIXELS as f32 * MAX_ASPECT) as usize + 1, "{id}: {:?}", image.size);
        assert!(h == PIXELS as usize || w >= (PIXELS as f32 * MAX_ASPECT) as usize, "{id}: {:?}", image.size);
        assert!(ink(&image, 0..image.size[0]) > 200, "{id}: nothing drawn");
    }
}

#[test]
fn texts_of_svg_logos_use_the_bundled_font() {
    // A badge: the name is a text right of the initial's tile.
    let svg = openecraft_parts::bundled::logo("janitza").unwrap();
    assert!(svg.contains("<text"), "the test needs a badge with text");
    let image = decode("janitza.svg", svg.as_bytes()).unwrap();
    let text_from = image.size[0] * 80 / 240;
    assert!(ink(&image, text_from..image.size[0]) > 100, "the name is drawn");
}

#[test]
fn raster_logos_and_broken_files() {
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(40, 20, image::Rgba([200, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let image = decode("logo.PNG", &png).unwrap();
    assert_eq!(image.size, [40, 20]);
    assert!(decode("logo.svg", b"not svg").is_none());
    assert!(decode("logo.png", b"not png").is_none());
}

#[test]
fn received_logos_are_kept_by_their_file() {
    let mut logos = Logos::default();
    assert!(!logos.loaded("f1"));
    logos.receive(&Reply::Logo("f1".into(), "a.svg".into(), Ok(b"<svg/>".to_vec())));
    logos.receive(&Reply::Logo("f2".into(), "b.png".into(), Err("404".into())));
    logos.receive(&Reply::Catalogs(Ok(Vec::new())));
    assert!(logos.loaded("f1") && logos.loaded("f2"));
    logos.clear();
    assert!(!logos.loaded("f1"));
}

#[test]
fn wide_logos_are_drawn_whole() {
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 100"><rect width="1000" height="100" fill="#000"/></svg>"##;
    let image = decode("wide.svg", svg).unwrap();
    assert_eq!(image.size, [(PIXELS as f32 * MAX_ASPECT) as usize, (PIXELS as f32 * MAX_ASPECT / 10.0) as usize]);
    // The right end is drawn too.
    let [w, h] = image.size;
    assert!(image.pixels[(h / 2) * w + w - 1].a() > 200);
    assert_eq!(fit([200, 100], egui::vec2(100.0, 100.0)), egui::vec2(100.0, 50.0));
}
