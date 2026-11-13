use super::*;
use crate::sample::demo_project;

#[test]
fn placeholders_are_filled() {
    let mut prj = demo_project();
    prj.info.properties.insert("Customer".into(), "ACME".into());
    let n = prj.pages.len();
    assert_eq!(fill("{project} – {sheet}/{sheets}", &prj, 1), format!("Demo conveyor – 2/{n}"));
    assert_eq!(fill("Customer: {prop:Customer}", &prj, 0), "Customer: ACME");
    assert_eq!(fill("{prop:Missing}", &prj, 0), "");
    // Unknown placeholders and escaped braces stay readable.
    assert_eq!(fill("{nothing} {{x}} {", &prj, 0), "{nothing} {x} {");
    assert_eq!(fill("{page.type}", &prj, 0), "Cover");
    assert_eq!(fill("ä {company} ö", &prj, 0), "ä OpenECraft ö");
}

#[test]
fn every_offered_placeholder_is_known() {
    let prj = demo_project();
    for (ph, _) in PLACEHOLDERS {
        let name = ph.trim_start_matches('{').trim_end_matches('}');
        if name.starts_with('{') || name.trim().is_empty() {
            continue;
        }
        let name = if name.starts_with("prop:") { "prop:Name" } else { name };
        assert!(placeholder(&prj, 0, name).is_some(), "{ph}");
    }
}

#[test]
fn title_block_choice() {
    let mut prj = demo_project();
    assert_eq!(title_block(&prj, 1).name, STANDARD);
    let mut own = find(&prj, COMPACT).into_owned();
    own.name = "Company".into();
    prj.forms.title_blocks.push(own);
    prj.forms.default_title_block = "Company".into();
    assert_eq!(title_block(&prj, 1).name, "Company");
    prj.pages[1].title_block = NONE.into();
    assert_eq!(title_block(&prj, 1).name, NONE);
    assert!(title_block(&prj, 1).items.is_empty());
    // Unknown names fall back to the standard title block.
    prj.pages[1].title_block = "Deleted".into();
    assert_eq!(title_block(&prj, 1).name, STANDARD);
    assert_eq!(names(&prj), vec!["Company", STANDARD, COMPACT, NONE]);
}

#[test]
fn title_blocks_are_drawn_with_page_values() {
    let mut prj = demo_project();
    let svg = crate::svg::page_svg(&prj, 1);
    assert!(svg.contains("Demo conveyor"));
    assert!(svg.contains(&format!("2 / {}", prj.pages.len())));
    // A custom title block with a project property.
    prj.info.properties.insert("Order".into(), "4711-XY".into());
    let item = FormItem::text(p(10.0, 10.0), "Order {prop:Order}", 3.0, Anchor::Left);
    prj.forms.title_blocks.push(Form { name: "Mine".into(), items: vec![item] });
    prj.pages[1].title_block = "Mine".into();
    let svg = crate::svg::page_svg(&prj, 1);
    assert!(svg.contains("Order 4711-XY"));
    assert!(!svg.contains("Demo conveyor"), "standard title block is not drawn");
}

#[test]
fn long_values_shrink_to_their_width() {
    struct Sizes(Vec<f32>);
    impl Draw for Sizes {
        fn line(&mut self, _: P, _: P, _: f32, _: Ink, _: bool) {}
        fn poly(&mut self, _: &[P], _: bool, _: f32, _: Ink, _: bool) {}
        fn circle(&mut self, _: P, _: f32, _: f32, _: Ink, _: bool) {}
        fn text(&mut self, _: P, _: &str, size: f32, _: Anchor, _: VAnchor, _: Ink) {
            self.0.push(size);
        }
    }
    let mut prj = demo_project();
    prj.info.name = "A very long project name".into();
    let item = FormItem::Text { pos: p(0.0, 0.0), text: "{project}".into(), size: 4.0, anchor: Anchor::Left, label: false, max_width: 20.0, bold: false, italic: false, angle: 0.0, color: None, show: Show::default() };
    let mut d = Sizes(Vec::new());
    draw(&mut d, &Form { name: "x".into(), items: vec![item] }, &prj, 0);
    assert!(d.0[0] < 4.0);
    assert!(text_width(&prj.info.name, d.0[0]) <= 20.01);
}

#[test]
fn report_columns_follow_the_form() {
    let headers: Vec<Text> = ["Tag", "Name", "Part"].map(Text::Borrowed).to_vec();
    assert_eq!(report_columns(None, &headers), vec![0, 1, 2]);
    let f = ReportForm { columns: vec!["Part".into(), "Unknown".into(), "Tag".into()], ..Default::default() };
    assert_eq!(report_columns(Some(&f), &headers), vec![2, 0]);
    let f = ReportForm { columns: vec!["Unknown".into()], ..Default::default() };
    assert_eq!(report_columns(Some(&f), &headers), vec![0, 1, 2]);
}

#[test]
fn report_pages_use_their_form() {
    let mut prj = demo_project();
    let id = prj.new_id();
    prj.pages.push(crate::model::Page::new(id, "99", "Devices", PageKind::Report(ReportKind::DeviceList.into())));
    let page = prj.pages.len() - 1;
    let form = ReportForm { title: "Our device list".into(), columns: vec!["Part number".into(), "Device tag".into()], rows: 10, ..Default::default() };
    prj.forms.reports.insert(ReportKind::DeviceList.into(), form);
    let svg = crate::svg::page_svg(&prj, page);
    assert!(svg.contains("Our device list"));
    let (part, tag) = (svg.find(">Part number<").unwrap(), svg.find(">Device tag<").unwrap());
    assert!(part < tag, "columns in the order of the form");
    assert!(!svg.contains(">Function text<"));
    // The forms are saved with the project (report kinds as keys).
    let json = serde_json::to_string(&prj).unwrap();
    let back: Project = serde_json::from_str(&json).unwrap();
    assert_eq!(back.forms, prj.forms);
}

#[test]
fn merging_title_blocks_replaces_by_name() {
    let mut prj = Project::default();
    let before = prj.forms.title_blocks.len();
    let mut own = builtin().remove(0);
    own.name = "Company".into();
    let mut lib = FormLibrary { title_blocks: vec![own.clone()], ..Default::default() };
    assert_eq!(merge(&mut prj, FormLibrary::from_json(&serde_json::to_string(&lib).unwrap()).unwrap()), (1, 0));
    own.items.clear();
    lib.title_blocks = vec![own];
    merge(&mut prj, lib);
    assert_eq!(prj.forms.title_blocks.len(), before + 1);
    assert!(prj.forms.title_blocks.iter().find(|f| f.name == "Company").unwrap().items.is_empty());
    assert!(FormLibrary::from_json("42").is_err());
}

#[test]
fn merging_report_schemes_replaces_by_report_and_name() {
    let mut prj = Project::default();
    let scheme = |kind: ReportKind, title: &str| ReportScheme { name: "Workshop".into(), kind: kind.into(), form: ReportForm { title: title.into(), ..Default::default() } };
    let lib = FormLibrary { schemes: vec![scheme(ReportKind::DeviceList, "a"), scheme(ReportKind::TerminalDiagram, "b")], ..Default::default() };
    let json = serde_json::to_string(&lib).unwrap();
    assert_eq!(merge(&mut prj, FormLibrary::from_json(&json).unwrap()), (0, 2));
    merge(&mut prj, FormLibrary { schemes: vec![scheme(ReportKind::DeviceList, "c")], ..Default::default() });
    assert_eq!(prj.forms.schemes.len(), 2);
    assert_eq!(report_form(&prj, ReportKind::DeviceList, "Workshop").unwrap().title, "c");
    // Old title block files without schemes still load.
    assert!(FormLibrary::from_json(r#"{"title_blocks":[]}"#).unwrap().schemes.is_empty());
}

#[test]
fn property_database_values_fill_placeholders() {
    use crate::properties::{set, Object};
    let mut prj = demo_project();
    set(&mut prj, &Object::Project, "project.customer", "ACME").unwrap();
    set(&mut prj, &Object::Project, "project.mains_voltage", "400").unwrap();
    let page = Object::Page(prj.pages[1].id);
    set(&mut prj, &page, "page.drawing_number", "D-17").unwrap();
    assert_eq!(fill("{prop:project.customer} {prop:project.mains_voltage}", &prj, 1), "ACME 400 V");
    assert_eq!(fill("{prop:page.drawing_number}", &prj, 1), "D-17");
    assert_eq!(fill("{prop:page.drawing_number}", &prj, 0), "");
    assert_eq!(fill("{prop:project.pages}", &prj, 0), prj.pages.len().to_string());
}

#[test]
fn row_placeholders_take_the_cells_by_key_or_title() {
    let keys = vec!["Tag".to_string(), "Part".to_string()];
    let headers = vec!["Kennz.".to_string(), "Artikel".to_string()];
    let row = vec!["-K1".to_string(), "{odd}".to_string()];
    assert_eq!(fill_row("{Tag}: {Artikel}", &keys, &headers, &row), "-K1: {{odd}}");
    assert_eq!(fill_row("{project} {Missing} {{x}}", &keys, &headers, &row), "{project} {Missing} {{x}}");
    assert_eq!(fill_row("open {", &keys, &headers, &row), "open {");
}

#[test]
fn a_drawn_form_replaces_the_table_of_a_report_page() {
    let mut prj = demo_project();
    let page = prj.pages.iter().position(|p| p.kind == PageKind::Report(ReportKind::DeviceList.into())).unwrap();
    let an = crate::analysis::Analysis::with_builtin(&prj);
    let table = an.report(&prj, ReportKind::DeviceList).unwrap();
    let keys = table.header_keys();
    let mut layout = starting_layout(&keys, &table.header_texts(), &table.widths, 380.0);
    // A mark only the drawn form has, with a column and a project placeholder.
    layout.row.push(FormItem::text(p(300.0, 3.0), &format!("#{{{}}}@{{project}}", keys[0]), 2.0, Anchor::Left));
    prj.forms.reports.insert(ReportKind::DeviceList.into(), ReportForm { layout: Some(layout.clone()), ..Default::default() });
    let t = crate::report_blocks::page_table(&prj, &an, page).unwrap();
    assert_eq!(t.layout.as_ref(), Some(&layout));
    assert_eq!(t.keys.len(), t.headers.len());
    let first = &t.rows[0][0];
    let svg = crate::svg::page_svg_with(&prj, &an, page);
    assert!(svg.contains(&format!("#{first}@Demo conveyor")), "row placeholders filled");
    assert!(svg.contains(&tr("Device list").to_string()), "{{report}} is the page heading");
    let (a, b) = items_bounds(&layout.row).unwrap();
    assert!(a.x <= 0.0 && b.x >= 300.0);
    // Without the layout the page is the table again.
    prj.forms.reports.clear();
    assert!(!crate::svg::page_svg_with(&prj, &an, page).contains(&format!("#{first}@")));
}

#[test]
fn the_starting_layout_has_a_placeholder_per_column() {
    let layout = starting_layout(&["Tag", "Part"], &["Kennz."], &[1.0, 3.0], 200.0);
    let texts: Vec<&str> = layout.row.iter().filter_map(|i| match i { FormItem::Text { text, .. } => Some(text.as_str()), _ => None }).collect();
    assert_eq!(texts, ["{Tag}", "{Part}"]);
    let labels: Vec<&str> = layout.header.iter().filter_map(|i| match i { FormItem::Text { text, label: true, .. } => Some(text.as_str()), _ => None }).collect();
    assert_eq!(labels, ["Kennz.", "Part"], "titles in the current language, keys where none is given");
    assert_eq!(layout.step, p(0.0, 6.0));
    let json = serde_json::to_string(&ReportForm { layout: Some(layout.clone()), ..Default::default() }).unwrap();
    assert_eq!(serde_json::from_str::<ReportForm>(&json).unwrap().layout, Some(layout));
    assert!(!serde_json::to_string(&ReportForm::default()).unwrap().contains("layout"));
}

/// A PNG header of `w` × `h` pixels (enough for the pixel size and the exports).
fn png(w: u32, h: u32) -> Vec<u8> {
    let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
    png.extend_from_slice(&w.to_be_bytes());
    png.extend_from_slice(&h.to_be_bytes());
    png
}

/// Records the images and styled texts drawn.
#[derive(Default)]
struct Record {
    images: Vec<crate::model::ImageItem>,
    styled: Vec<(String, crate::text::TextStyle)>,
}

impl Draw for Record {
    fn line(&mut self, _: P, _: P, _: f32, _: Ink, _: bool) {}
    fn poly(&mut self, _: &[P], _: bool, _: f32, _: Ink, _: bool) {}
    fn circle(&mut self, _: P, _: f32, _: f32, _: Ink, _: bool) {}
    fn text(&mut self, _: P, _: &str, _: f32, _: Anchor, _: VAnchor, _: Ink) {}
    fn styled_text(&mut self, _: P, text: &str, style: &crate::text::TextStyle, _: Ink) {
        self.styled.push((text.to_string(), *style));
    }
    fn image(&mut self, img: &crate::model::ImageItem) {
        self.images.push(img.clone());
    }
}

#[test]
fn images_keep_their_aspect_ratio_and_fall_back_to_the_project_logo() {
    let mut prj = demo_project();
    let company = picture("company.png", &png(200, 100)).unwrap();
    assert_eq!(company.px, [200, 100]);
    assert!(picture("logo.gif", b"GIF89a").is_none());
    let items = vec![
        FormItem::image(p(0.0, 0.0), p(40.0, 40.0), Some(company.clone())),
        FormItem::image(p(50.0, 0.0), p(70.0, 10.0), None),
    ];
    let form = Form { name: "Logos".into(), items };
    let mut d = Record::default();
    draw_at(&mut d, &form, &prj, 1, p(100.0, 100.0), 1.0);
    // Without a project logo the second image shows nothing.
    assert_eq!(d.images.len(), 1);
    assert_eq!((d.images[0].a, d.images[0].b), (p(100.0, 110.0), p(140.0, 130.0)), "fitted and centred");
    assert_eq!(d.images[0].id, picture_id(&company));
    assert!(picture_id(&company) >= 1 << 63, "apart from the ids of page images");

    let customer = picture("customer.png", &png(100, 100)).unwrap();
    prj.info.logo = Some(customer.clone());
    let mut d = Record::default();
    draw_at(&mut d, &form, &prj, 1, p(0.0, 0.0), 1.0);
    assert_eq!(d.images.len(), 2);
    assert_eq!((d.images[1].a, d.images[1].b, d.images[1].file.name.as_str()), (p(55.0, 0.0), p(65.0, 10.0), "customer.png"));
    assert_ne!(d.images[0].id, d.images[1].id);
}

#[test]
fn title_block_logos_are_exported_and_loaded_as_textures() {
    let mut prj = demo_project();
    let company = picture("company.png", &png(300, 100)).unwrap();
    let logo = picture("customer.png", &png(80, 80)).unwrap();
    let mut form = find(&prj, STANDARD).into_owned();
    form.name = "With logos".into();
    form.items.push(FormItem::image(p(2.0, 2.0), p(78.0, 14.0), Some(company.clone())));
    form.items.push(FormItem::image(p(2.0, 16.0), p(20.0, 28.0), None));
    form.items.push(FormItem::image(p(22.0, 16.0), p(40.0, 28.0), None));
    prj.forms.title_blocks.push(form);
    prj.forms.default_title_block = "With logos".into();
    assert_eq!(page_pictures(&prj, 1), vec![company.clone()]);
    prj.info.logo = Some(logo.clone());
    // The project logo once, although two elements show it.
    assert_eq!(page_pictures(&prj, 1), vec![company.clone(), logo.clone()]);
    let svg = crate::svg::page_svg(&prj, 1);
    assert_eq!(svg.matches("data:image/png;base64,").count(), 3);
    assert!(svg.contains(&*company.file.data));
    // The cover page shows the project logo too.
    assert_eq!(prj.pages[0].kind, PageKind::Cover);
    let logos = |page| crate::svg::page_svg(&prj, page).matches(&*logo.file.data).count();
    assert_eq!(logos(0), logos(1) + 1);

    // Saved and loaded with the project.
    let back: Project = serde_json::from_str(&serde_json::to_string(&prj).unwrap()).unwrap();
    assert_eq!(back.forms.title_blocks, prj.forms.title_blocks);
    assert_eq!(back.info.logo, Some(logo));
}

#[test]
fn texts_are_drawn_bold_italic_and_rotated() {
    let prj = demo_project();
    let plain = FormItem::text(p(0.0, 0.0), "{project}", 3.0, Anchor::Left);
    let styled = FormItem::Text { pos: p(10.0, 20.0), text: "A\nB".into(), size: 3.0, anchor: Anchor::Left, label: false, max_width: 0.0, bold: true, italic: true, angle: 90.0, color: None, show: Show::default() };
    let mut d = Record::default();
    draw_at(&mut d, &Form { name: "x".into(), items: vec![plain.clone(), styled.clone()] }, &prj, 0, p(0.0, 0.0), 2.0);
    // The plain text is not styled; both lines of the styled one are, scaled with the form.
    assert_eq!(d.styled.iter().map(|(t, _)| t.as_str()).collect::<Vec<_>>(), vec!["A", "B"]);
    let s = &d.styled[0].1;
    assert_eq!((s.bold, s.italic, s.angle, s.size), (true, true, 90.0, 6.0));
    // Old files without the new fields load; plain texts do not write them.
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("bold") && !json.contains("angle"), "{json}");
    let old = r#"{"type":"Text","pos":{"x":1.0,"y":2.0},"text":"T","size":2.0,"anchor":"Left"}"#;
    assert!(matches!(serde_json::from_str::<FormItem>(old).unwrap(), FormItem::Text { bold: false, angle: 0.0, .. }));
    let back: FormItem = serde_json::from_str(&serde_json::to_string(&styled).unwrap()).unwrap();
    assert_eq!(back, styled);
}

#[test]
fn neighbour_pages_and_revision_tables() {
    use crate::revision::{Revision, State};
    let mut prj = demo_project();
    let full = |i: usize| crate::structure::page_full_name(&prj.pages[i]);
    assert_eq!(fill("{page.previous}|{page.next}", &prj, 0), format!("|{}", full(1)));
    assert_eq!(fill("{page.previous}", &prj, 1), full(0));
    let last = prj.pages.len() - 1;
    assert_eq!(fill("{page.next}", &prj, last), "");

    let rev = |name: &str, state| Revision { name: name.into(), date: format!("2026-0{}", name.len()), author: format!("by {name}"), description: format!("about {name}"), state, ..Default::default() };
    prj.revisions = vec![rev("A", State::Released), rev("BB", State::Released), rev("C", State::Draft)];
    assert_eq!(fill("{rev.1.name} {rev.1.date} {rev.1.author} {rev.1.description}", &prj, 0), "BB 2026-02 by BB about BB");
    assert_eq!(fill("{rev.2.name}|{rev.3.name}", &prj, 0), "A|", "drafts are left out; missing revisions are empty");
    assert_eq!(fill("{rev.1.name}", &prj, 0), fill("{rev.name}", &prj, 0));
    assert_eq!(fill("{rev.0.name} {rev.x.name} {rev.1.colour}", &prj, 0), "{rev.0.name} {rev.x.name} {rev.1.colour}");
}

#[test]
fn image_elements_are_picked_and_moved() {
    let mut it = FormItem::image(p(10.0, 10.0), p(30.0, 20.0), None);
    assert_eq!(it.distance(p(20.0, 15.0)), 0.0);
    assert_eq!(it.distance(p(33.0, 24.0)), 5.0);
    it.translate(p(1.0, 2.0));
    assert!(matches!(it, FormItem::Image { a, b, .. } if a == p(11.0, 12.0) && b == p(31.0, 22.0)));
    assert_eq!(items_bounds(&[it]), Some((p(11.0, 12.0), p(31.0, 22.0))));
}

/// Records lines (dashed or not) and filled rectangles with their inks.
#[derive(Default)]
struct Strokes {
    lines: Vec<(Ink, bool)>,
    fills: Vec<(P, P, Ink)>,
    texts: Vec<(String, Ink)>,
}

impl Draw for Strokes {
    fn line(&mut self, _: P, _: P, _: f32, ink: Ink, dashed: bool) {
        self.lines.push((ink, dashed));
    }
    fn poly(&mut self, pts: &[P], _: bool, _: f32, ink: Ink, fill: bool) {
        if fill {
            self.fills.push((pts[0], pts[2], ink));
        } else {
            self.lines.push((ink, false));
        }
    }
    fn circle(&mut self, _: P, _: f32, _: f32, _: Ink, _: bool) {}
    fn text(&mut self, _: P, text: &str, _: f32, _: Anchor, _: VAnchor, ink: Ink) {
        self.texts.push((text.into(), ink));
    }
    fn styled_text(&mut self, _: P, text: &str, _: &crate::text::TextStyle, ink: Ink) {
        self.texts.push((text.into(), ink));
    }
}

#[test]
fn elements_are_shown_by_their_conditions() {
    use crate::revision::{Revision, State};
    let mut prj = demo_project();
    prj.revisions = vec![Revision { name: "A".into(), state: State::Released, ..Default::default() }];
    let mut row2 = FormItem::text(p(0.0, 0.0), "Row 2: {rev.2.name}", 2.0, Anchor::Left);
    row2.show_mut().filled = "{rev.2.name}".into();
    let mut cover_only = FormItem::text(p(0.0, 5.0), "Cover", 2.0, Anchor::Left);
    cover_only.show_mut().page_types = vec!["cover".into()];
    let always = FormItem::text(p(0.0, 10.0), "Always", 2.0, Anchor::Left);
    let form = Form { name: "x".into(), items: vec![row2, cover_only, always] };
    let texts = |prj: &Project, page: usize| {
        let mut d = Strokes::default();
        draw_at(&mut d, &form, prj, page, p(0.0, 0.0), 1.0);
        d.texts.into_iter().map(|t| t.0).collect::<Vec<_>>()
    };
    assert_eq!(prj.pages[0].kind, PageKind::Cover);
    assert_eq!(texts(&prj, 0), vec!["Cover", "Always"]);
    assert_eq!(texts(&prj, 1), vec!["Always"]);
    prj.revisions.push(Revision { name: "B".into(), state: State::Released, ..Default::default() });
    assert_eq!(texts(&prj, 1), vec!["Row 2: A", "Always"]);
    // The editor (no page type) draws everything.
    let mut d = Strokes::default();
    draw_items(&mut d, &form.items, p(0.0, 0.0), 1.0, &Context { logo: None, page_type: None, value: &|t| t.to_string() });
    assert_eq!(d.texts.len(), 3);
    // Saved with the element; elements without a condition do not write it.
    let json = serde_json::to_string(&form).unwrap();
    assert_eq!(json.matches("\"show\"").count(), 2, "{json}");
    assert_eq!(serde_json::from_str::<Form>(&json).unwrap(), form);
}

#[test]
fn elements_have_own_colours_and_dashes() {
    let prj = demo_project();
    let red = Some([200, 0, 0]);
    let items = vec![
        FormItem::Line { a: p(0.0, 0.0), b: p(10.0, 0.0), width: 0.5, color: red, dashed: true, show: Show::default() },
        FormItem::Rect { a: p(0.0, 0.0), b: p(10.0, 5.0), width: 0.25, fill: true, color: red, dashed: true, show: Show::default() },
        FormItem::line(p(0.0, 0.0), p(10.0, 0.0), 0.5),
    ];
    let mut text = FormItem::text(p(0.0, 0.0), "T", 2.0, Anchor::Left);
    if let FormItem::Text { color, .. } = &mut text {
        *color = red;
    }
    let items: Vec<FormItem> = items.into_iter().chain([text]).collect();
    let mut d = Strokes::default();
    draw_at(&mut d, &Form { name: "x".into(), items }, &prj, 1, p(0.0, 0.0), 1.0);
    // The dashed line, the four dashed sides of the rectangle and the plain line.
    assert_eq!(d.lines.iter().filter(|l| **l == (Ink::Color([200, 0, 0]), true)).count(), 5);
    assert_eq!(d.lines.last(), Some(&(Ink::Frame, false)));
    assert_eq!(d.fills, vec![(p(0.0, 0.0), p(10.0, 5.0), Ink::Color([200, 0, 0]))]);
    assert_eq!(d.texts, vec![("T".to_string(), Ink::Color([200, 0, 0]))]);
}

#[test]
fn qr_codes_are_drawn_from_the_filled_text() {
    let prj = demo_project();
    let mut d = Strokes::default();
    let items = vec![FormItem::qr(p(10.0, 10.0), 21.0, "{project}"), FormItem::qr(p(50.0, 10.0), 21.0, "{prop:Missing}")];
    draw_at(&mut d, &Form { name: "x".into(), items: items.clone() }, &prj, 1, p(0.0, 0.0), 1.0);
    let qr = crate::qr::Qr::encode(&prj.info.name).unwrap();
    // One rectangle per run of dark modules, all inside the code's square; nothing
    // for an empty text.
    let runs: usize = (0..qr.size).map(|y| (0..qr.size).filter(|&x| qr.dark(x, y) && (x == 0 || !qr.dark(x - 1, y))).count()).sum();
    assert_eq!(d.fills.len(), runs);
    let m = 21.0 / qr.size as f32;
    assert!(d.fills.iter().all(|(a, b, ink)| *ink == Ink::Text && a.x >= 10.0 && b.x <= 31.01 && a.y >= 10.0 && b.y <= 31.01));
    assert!(d.fills.iter().any(|(a, b, _)| *a == p(10.0, 10.0) && (b.y - 10.0 - m).abs() < 1e-4), "finder pattern in the corner");
    assert_eq!(items[0].kind_name(), "QR code");
    assert_eq!(items_bounds(&items[..1]), Some((p(10.0, 10.0), p(31.0, 31.0))));
    assert_eq!(items[0].distance(p(20.0, 20.0)), 0.0);
}

#[test]
fn title_blocks_by_page_type_and_plot_frame() {
    let mut prj = demo_project();
    let page = 1;
    assert_eq!(prj.pages[page].kind, PageKind::Schematic);
    prj.forms.default_title_block = COMPACT.into();
    assert_eq!(title_block_name(&prj, page), COMPACT);
    let mut frame = crate::frame::of_page(&prj, page);
    frame.name = "Mine".into();
    frame.title_block = NONE.into();
    crate::frame::set(&mut prj, frame);
    prj.pages[page].frame = "Mine".into();
    assert_eq!(title_block_name(&prj, page), NONE, "the frame's before the project default");
    prj.forms.page_type_title_blocks.insert("schematic".into(), STANDARD.into());
    assert_eq!(title_block_name(&prj, page), STANDARD, "the page type's before the frame's");
    prj.pages[page].title_block = COMPACT.into();
    assert_eq!(title_block_name(&prj, page), COMPACT, "the page's own first");
    assert_eq!(page_type(prj.pages[0].kind), "cover");
    assert!(PAGE_TYPES.iter().all(|(k, _)| PAGE_TYPES.iter().filter(|(x, _)| x == k).count() == 1));
}

#[test]
fn approvals_and_the_state_of_the_workflow_are_placeholders() {
    use crate::revision::{self, Revision, State};
    let mut prj = demo_project();
    assert_eq!(fill("{rev.status}", &prj, 0), "draft");
    prj.workflow.approvers = vec!["Ann".into(), "Bob".into()];
    prj.revisions.push(Revision { name: "A".into(), state: State::Draft, ..Default::default() });
    revision::submit(&mut prj, 0).unwrap();
    assert_eq!(fill("{rev.status}", &prj, 0), "in review");
    revision::decide(&mut prj, 0, "Ann", "2026-11-01", true, "").unwrap();
    revision::decide(&mut prj, 0, "Bob", "2026-11-03", true, "").unwrap();
    assert_eq!(fill("{rev.status}", &prj, 0), "approved");
    revision::release(&mut prj, 0, false).unwrap();
    assert_eq!(fill("{rev.status}|{rev.approved_by}|{rev.approved_date}", &prj, 0), "released|Ann, Bob|2026-11-03");
    assert_eq!(fill("{rev.1.approved_by}|{rev.2.approved_by}", &prj, 0), "Ann, Bob|");
}

#[test]
fn the_stamp_follows_the_revision_workflow() {
    use crate::revision::{self, Revision, State};
    let mut prj = demo_project();
    assert!(prj.forms.stamp.is_off());
    assert_eq!(stamp_text(&prj), None);
    let s = &mut prj.forms.stamp;
    (s.draft, s.review, s.approved, s.released) = ("DRAFT {rev.status}".into(), "FOR APPROVAL".into(), "APPROVED".into(), String::new());
    assert_eq!(stamp_text(&prj), Some("DRAFT {rev.status}"), "nothing released yet");
    let mut d = Strokes::default();
    draw_stamp(&mut d, &prj, 1);
    assert_eq!(d.texts, vec![("DRAFT draft".to_string(), Ink::Color(prj.forms.stamp.color))]);
    prj.workflow.approvers = vec!["Ann".into()];
    prj.revisions.push(Revision { name: "A".into(), state: State::Draft, ..Default::default() });
    revision::submit(&mut prj, 0).unwrap();
    assert_eq!(stamp_text(&prj), Some("FOR APPROVAL"));
    revision::decide(&mut prj, 0, "Ann", "2026-11-01", false, "no").unwrap();
    assert_eq!(stamp_text(&prj), Some("FOR APPROVAL"), "rejected");
    revision::decide(&mut prj, 0, "Ann", "2026-11-02", true, "").unwrap();
    assert_eq!(stamp_text(&prj), Some("APPROVED"));
    revision::release(&mut prj, 0, false).unwrap();
    assert_eq!(stamp_text(&prj), None, "released without a text");
    prj.forms.stamp.released = "AS BUILT".into();
    // On every page, also in the exports; saved with the project.
    assert!(crate::svg::page_svg(&prj, 3).contains("AS BUILT"));
    let back: Project = serde_json::from_str(&serde_json::to_string(&prj).unwrap()).unwrap();
    assert_eq!(back.forms.stamp, prj.forms.stamp);
}

#[test]
fn form_libraries_carry_plot_frames() {
    let mut prj = demo_project();
    let mut frame = crate::frame::find(&prj, crate::frame::DEFAULT);
    frame.name = "Company A3".into();
    crate::frame::set(&mut prj, frame);
    let lib = FormLibrary::of(&prj);
    assert!(!lib.is_empty() && FormLibrary::default().is_empty());
    let text = serde_json::to_string(&lib).unwrap();
    let mut other = demo_project();
    merge(&mut other, FormLibrary::from_json(&text).unwrap());
    assert!(other.forms.frames.iter().any(|f| f.name == "Company A3"));
    // Files without frames still load.
    assert!(FormLibrary::from_json(r#"{"title_blocks": []}"#).unwrap().frames.is_empty());
}

#[test]
fn the_cover_logo_goes_where_the_project_puts_it() {
    let mut prj = demo_project();
    assert_eq!(prj.pages[0].kind, PageKind::Cover);
    prj.info.logo = Some(picture("customer.png", &png(80, 80)).unwrap());
    let frame = crate::frame::of_page(&prj, 0);
    let area = frame.area();
    let images = |prj: &Project| {
        let mut d = Record::default();
        crate::render::draw_page(&mut d, prj, &crate::index::Index::build(prj), &crate::analysis::Analysis::build(prj), 0, &Default::default());
        d.images.into_iter().filter(|i| i.file.name == "customer.png").map(|i| (i.a, i.b)).collect::<Vec<_>>()
    };
    // Above the project name: centred, as high as the area allows.
    let auto = crate::render::cover_logo_place(&prj, &frame);
    assert!((auto.x + auto.width / 2.0 - area.width() / 2.0).abs() < 0.01);
    let (a, b) = images(&prj)[0];
    assert!(((a.x + b.x) / 2.0 - area.center().x).abs() < 0.01 && (b.y - a.y - auto.height).abs() < 0.01);
    // Its own place: the square logo fitted into 50 x 20 mm at (10, 30).
    prj.info.cover_logo = Some(crate::model::CoverLogo { x: 10.0, y: 30.0, width: 50.0, height: 20.0 });
    assert_eq!(images(&prj), vec![(area.min + p(25.0, 30.0), area.min + p(45.0, 50.0))]);
    let back: Project = serde_json::from_str(&serde_json::to_string(&prj).unwrap()).unwrap();
    assert_eq!(back.info.cover_logo, prj.info.cover_logo);
}
