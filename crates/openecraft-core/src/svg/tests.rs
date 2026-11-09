use super::*;
use crate::geom::p;
use crate::model::*;
use crate::symbols::Anchor;

#[test]
fn symbol_bitmap_preserves_affine_corners_in_svg() {
    let image = ImageItem {
        file: crate::document::Document { name: "sample.png".into(), data: "AA==".to_string().into() },
        ..Default::default()
    };
    let corners = [p(12., 20.), p(12., 26.), p(8., 26.), p(8., 20.)];
    let svg = drawing_svg(30., 30., |draw| draw.image_quad(&image, &corners));
    assert!(svg.contains("matrix(0.000000 6.000000 -4.000000 0.000000 12.000000 20.000000)"));
    assert!(svg.contains("data:image/png;base64,AA=="));
}

#[test]
fn styled_texts_special_texts_and_hyperlinks() {
    let mut prj = Project::default();
    let pg = &mut prj.pages[0];
    pg.texts.push(TextItem {
        id: 1,
        pos: p(100.0, 100.0),
        text: "Up".into(),
        angle: 90.0,
        bold: true,
        italic: true,
        underline: true,
        align: Anchor::Center,
        ..Default::default()
    });
    pg.texts.push(TextItem { id: 2, pos: p(50.0, 50.0), text: "Docs".into(), link: "https://example.org/?a=1&b=2".into(), frame: true, background: true, ..Default::default() });
    pg.texts.push(TextItem { id: 3, pos: p(50.0, 70.0), text: "{page.name} of {sheets}".into(), ..Default::default() });
    let svg = page_svg(&prj, 0);
    assert!(svg.contains("transform=\"rotate(-90.00 100.00 100.00)\""), "{svg}");
    assert!(svg.contains("font-weight=\"bold\" font-style=\"italic\" text-decoration=\"underline\""));
    assert!(svg.contains("text-anchor=\"middle\""));
    assert!(svg.contains("<a href=\"https://example.org/?a=1&amp;b=2\"><rect"));
    assert!(svg.contains(">1 of 1</text>"), "placeholders are filled in");
    // Background in the paper colour and a frame around the linked text.
    let bg = svg.lines().find(|l| l.starts_with("<polygon") && l.contains("fill=\"#fff\"")).expect("background");
    assert!(bg.contains("49.25,47.15"), "{bg}");
    assert!(svg.lines().any(|l| l.starts_with("<polygon") && l.contains("fill=\"none\"") && l.contains("49.25,47.15")), "frame");
}

#[test]
fn plain_texts_are_unchanged() {
    let mut prj = Project::default();
    prj.pages[0].texts.push(TextItem { id: 1, pos: p(40.0, 60.0), text: "Note".into(), ..Default::default() });
    let svg = page_svg(&prj, 0);
    assert!(svg.contains("<text x=\"40.00\" y=\"61.05\" font-size=\"3.75\" text-anchor=\"start\" fill=\"#000\">Note</text>"), "{svg}");
    assert!(!svg.contains("<a href"));
}
