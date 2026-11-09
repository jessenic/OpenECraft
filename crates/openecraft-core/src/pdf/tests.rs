use super::*;
use crate::sample::demo_project;

#[test]
fn document_structure_is_valid() {
    let prj = demo_project();
    let pdf = document(&prj);
    assert!(pdf.starts_with("%PDF-1.4\n") && pdf.ends_with("%%EOF\n"));
    assert!(pdf.is_ascii());
    // startxref points at the cross reference table, whose offsets point at the objects.
    let start: usize = pdf.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
    assert!(pdf[start..].starts_with("xref\n"));
    let table: Vec<&str> = pdf[start..].lines().skip(3).take_while(|l| !l.starts_with("trailer")).collect();
    for (i, entry) in table.iter().enumerate() {
        let off: usize = entry[..10].parse().unwrap();
        assert!(pdf[off..].starts_with(&format!("{} 0 obj", i + 1)), "object {}", i + 1);
    }
    // Stream lengths match.
    for part in pdf.split("<< /Length ").skip(1) {
        let len: usize = part[..part.find(' ').unwrap()].parse().unwrap();
        let body = &part[part.find("stream\n").unwrap() + 7..];
        assert!(body[len..].starts_with("endstream"));
    }
    assert_eq!(pdf.matches("/Type /Page ").count(), prj.pages.len());
    assert_eq!(pdf.matches("/Parent 4 0 R").count(), prj.pages.len(), "one bookmark per page");
}

#[test]
fn cross_references_are_links() {
    let prj = demo_project();
    let pdf = document(&prj);
    // Contacts on page 3 refer to the coil, the coil's contact image to page 2 and 3,
    // report rows to their pages.
    assert!(pdf.matches("/Subtype /Link").count() > 10);
    assert!(pdf.contains("(-K1) Tj"));
}

#[test]
fn text_encoding_and_metrics() {
    assert_eq!(pdf_string("a(b)\\"), "a\\(b\\)\\\\");
    assert_eq!(pdf_string("1.5 mm² – x"), "1.5 mm\\262 \\226 x");
    assert_eq!(pdf_string("Ω"), "?");
    assert!((text_width("A", 10.0) - 6.67).abs() < 0.01);
}

fn png(color_type: u8) -> Vec<u8> {
    let mut bytes=Vec::new();
    {
        let mut encoder=png::Encoder::new(&mut bytes,2,1);
        let color=if color_type==6 {png::ColorType::Rgba} else {png::ColorType::Rgb};
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer=encoder.write_header().unwrap();
        writer.write_image_data(&vec![128; if color_type==6 {8} else {6}]).unwrap();
    }
    bytes
}

#[test]
fn images_become_xobjects() {
    let mut prj = crate::model::Project::default();
    crate::graphics::add_image(&mut prj, 0, crate::geom::p(20.0, 20.0), "logo.png", &png(2), 40.0, "");
    let jpeg = [0xFF, 0xD8, 0xFF, 0xC0, 0, 11, 8, 0, 1, 0, 2, 3, 0, 0, 0, 0xFF, 0xD9];
    crate::graphics::add_image(&mut prj, 0, crate::geom::p(80.0, 20.0), "photo.jpg", &jpeg, 40.0, "");
    crate::graphics::add_image(&mut prj, 0, crate::geom::p(140.0, 20.0), "alpha.png", &png(6), 40.0, "");
    let pdf = document(&prj);
    assert!(pdf.is_ascii());
    assert_eq!(pdf.matches("/Subtype /Image").count(), 4);
    assert!(pdf.contains("/FlateDecode"));
    assert!(pdf.contains("/DCTDecode"));
    assert!(pdf.contains("/XObject << /Im0 "));
    assert!(pdf.contains("/SMask "));
    assert!(!pdf.contains("(alpha.png) Tj"));
}

#[test]
fn palette_png_transparency_is_preserved_as_a_soft_mask() {
    let mut bytes=Vec::new();
    {
        let mut encoder=png::Encoder::new(&mut bytes,2,1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![255,0,0,0,255,0]);
        encoder.set_trns(vec![255,0]);
        encoder.write_header().unwrap().write_image_data(&[0,1]).unwrap();
    }
    let image=pdf_image(&bytes).unwrap();
    assert!(image.dict.contains("/DeviceRGB"));
    assert!(image.mask.unwrap().dict.contains("/DeviceGray"));
    assert!(pdf_image(&bytes[..20]).is_none());
}

#[test]
fn styled_texts_fonts_rotation_and_hyperlinks() {
    use crate::geom::p;
    use crate::model::*;
    let mut prj = Project::default();
    let pg = &mut prj.pages[0];
    pg.texts.push(TextItem { id: 1, pos: p(100.0, 100.0), text: "Up".into(), angle: 90.0, bold: true, ..Default::default() });
    pg.texts.push(TextItem { id: 2, pos: p(50.0, 50.0), text: "Docs".into(), italic: true, underline: true, link: "https://example.org/a(b)".into(), ..Default::default() });
    let pdf = document(&prj);
    for name in ["Helvetica-Bold", "Helvetica-Oblique", "Helvetica-BoldOblique"] {
        assert!(pdf.contains(&format!("/BaseFont /{name}")), "{name}");
    }
    assert!(pdf.contains("/F2 ") && pdf.contains("/F3 ") && pdf.contains("/F4 "), "the page lists the four fonts");
    let up = pdf.lines().find(|l| l.ends_with("(Up) Tj ET")).expect("rotated text");
    assert!(up.contains("rg BT /F2 ") && up.contains(" 1.0000 -1.0000 ") && up.contains(" Tm "), "{up}");
    let docs = pdf.lines().find(|l| l.ends_with("(Docs) Tj ET")).expect("italic text");
    assert!(docs.contains("rg BT /F3 ") && docs.contains(" Td "), "{docs}");
    assert!(pdf.contains("/A << /S /URI /URI (https://example.org/a\\(b\\)) >>"), "{pdf}");
}

#[test]
fn texts_beyond_winansi_use_the_embedded_font() {
    use crate::geom::p;
    use crate::model::*;
    assert!(needs_unicode("Łódź") && needs_unicode("Ωμ") && !needs_unicode("Grüße 1.5 mm² – €") && !needs_unicode("a?"));
    assert_eq!(ascii85(b"Man "), "9jqo^~>");
    assert_eq!(ascii85(&[0, 0, 0, 0]), "z~>");
    assert_eq!(ascii85(b"M"), "9`~>");

    let plain = document(&Project::default());
    assert!(!plain.contains("/F5 "), "no font without such texts");

    let mut prj = Project::default();
    prj.pages[0].texts.push(TextItem { id: 1, pos: p(50.0, 50.0), text: "Łódź Ж".into(), bold: true, ..Default::default() });
    prj.pages[0].texts.push(TextItem { id: 2, pos: p(50.0, 70.0), text: "Wire 1.5 mm²".into(), ..Default::default() });
    let pdf = document(&prj);
    assert!(pdf.is_ascii(), "the file stays ASCII");
    assert!(pdf.contains("/Subtype /Type0 /BaseFont /OECUBU+Ubuntu-Light /Encoding /Identity-H"));
    assert!(pdf.contains("/FontFile2 ") && pdf.contains("/Filter [/ASCII85Decode /FlateDecode]"));
    assert!(pdf.contains("/F5 ") && pdf.contains(" 2 Tr "), "bold by stroking");
    let f = unicode_font().unwrap();
    let l = format!("<{:04X}> <0141>", f.glyph('Ł'));
    assert!(pdf.contains(&l), "ToUnicode maps the glyph back to the character");
    assert!(pdf.contains("(Wire 1.5 mm\\262) Tj"), "WinAnsi texts keep Helvetica");
}

#[test]
fn selected_pages_only_with_links_among_them() {
    let prj = crate::sample::demo_project();
    let an = Analysis::checked(&prj);
    let all = document_with(&prj, &an);
    let two = document_pages(&prj, &an, &[1, 2]);
    assert!(two.contains("/Count 2"));
    assert!(two.len() < all.len());
    let pages = |s: &str| s.matches("/Type /Page ").count();
    assert_eq!(pages(&two), 2);
    assert_eq!(pages(&all), prj.pages.len());
}

#[test]
fn characters_the_bundled_font_lacks_use_the_fallback_font() {
    // The Hack font of egui stands in for a system CJK font: it has glyphs the
    // Ubuntu font lacks.
    let fb = Font::parse(epaint_default_fonts::HACK_REGULAR).unwrap();
    let primary = unicode_font().unwrap();
    let c = ('\u{2000}'..='\u{2BFF}').find(|c| primary.glyph(*c) == 0 && fb.glyph(*c) != 0).unwrap();
    assert!(set_fallback_font(epaint_default_fonts::HACK_REGULAR.to_vec()) || fallback_font().is_some());
    let mut prj = demo_project();
    let id = prj.new_id();
    prj.pages[1].texts.push(crate::model::TextItem { id, pos: crate::geom::p(50.0, 50.0), text: format!("A{c}b"), size: 3.0, ..Default::default() });
    let an = Analysis::checked(&prj);
    let pdf = document_with(&prj, &an);
    assert!(pdf.contains("/F6 "), "fallback font used");
    assert!(pdf.contains("OECFBK+Fallback"));
}

#[test]
fn chinese_japanese_and_korean_texts_use_the_bundled_fonts() {
    let mut prj = demo_project();
    let id = prj.new_id();
    prj.pages[1].texts.push(crate::model::TextItem { id, pos: crate::geom::p(50.0, 50.0), text: "日本語 한국어".into(), size: 3.0, ..Default::default() });
    let an = Analysis::checked(&prj);
    let pdf = openecraft_i18n::with_lang(openecraft_i18n::Lang::Ja, || document_with(&prj, &an));
    assert!(pdf.contains("OECCJK+NotoSansJP"), "the font of the language comes first");
    assert!(pdf.contains("OECCJK+NotoSansKR"));
    assert!(pdf.contains("/F9 "));
}
