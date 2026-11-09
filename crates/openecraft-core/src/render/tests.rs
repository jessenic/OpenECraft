use super::*;

#[test]
fn frame_artwork_uses_live_page_properties_without_page_objects() {
    use crate::symbols::{PropertyBlock,BlockAlign};
    let mut project=Project::default();
    project.pages[0].frame="Source frame".into();
    project.pages[0].description="Initial caption".into();
    project.forms.frames.push(crate::frame::PlotFrame {name:"Source frame".into(),artwork:vec![
        Prim::Line {a:p(1.,2.),b:p(10.,2.),dashed:false},
        Prim::PropertyBlock(PropertyBlock {rows:vec![BlockRow::Property("page.description".into())],pos:p(10.,20.),
            step:p(0.,3.),size:2.,anchor:Anchor::Left,angle:0.,align:BlockAlign::Start,color:None})],..Default::default()});
    let mut first=Rec::default();draw_frame(&mut first,&project,0);
    assert_eq!(first.line_positions,vec![(p(1.,2.),p(10.,2.))]);
    assert!(first.rects.is_empty());
    assert!(first.texts.iter().any(|(_,text,_,_)|text=="Initial caption"));
    project.pages[0].description="Updated caption".into();
    let saved=serde_json::to_string(&project).unwrap();
    let reopened:Project=serde_json::from_str(&saved).unwrap();
    let mut second=Rec::default();draw_frame(&mut second,&reopened,0);
    assert!(second.texts.iter().any(|(_,text,_,_)|text=="Updated caption"));
    assert!(reopened.pages[0].symbols.is_empty() && reopened.pages[0].shapes.is_empty());
    project.forms.frames[0].visible=false;
    let mut hidden=Rec::default();draw_frame(&mut hidden,&project,0);
    assert!(hidden.line_positions.is_empty() && hidden.texts.is_empty());
}

#[test]
fn live_reference_rows_flow_and_keep_page_links_under_every_symbol_transform() {
    use crate::symbols::{BlockAlign,PropertyBlock};
    let block = PropertyBlock { rows:vec![BlockRow::Field(FieldKind::FunctionText),BlockRow::Property(references::PROPERTY.into()),BlockRow::Text("Tail".into())],
        pos:p(2.,3.),step:p(0.,3.),size:2.,anchor:Anchor::Left,angle:90.,align:BlockAlign::End,color:None };
    let refs = vec![references::Reference {text:"/11.4".into(),page:Some(0)},references::Reference {text:"/12.2".into(),page:Some(1)}];
    for rot in 0..4 { for mirror in [false,true] {
        let mut s = SymbolInst {pos:p(40.,40.),rot,mirror,function_text:"Header".into(),..Default::default()};
        let mut draw = Rec::default();
        draw_property_block(&mut draw,&block,&s,Ink::Line,&|_|String::new(),&refs);
        assert_eq!(draw.styled.iter().map(|(_,t,_)|t.as_str()).collect::<Vec<_>>(),["Header","/11.4","/12.2","Tail"]);
        assert_eq!(draw.page_links.iter().map(|(_,_,page)|*page).collect::<Vec<_>>(),[0,1]);
        for ((min,max,_),(pos,text,style)) in draw.page_links.iter().zip(draw.styled.iter().skip(1)) {
            let actual = text::bounds(&TextItem {pos:*pos,size:style.size,align:style.h,valign:style.v,angle:style.angle,..Default::default()},text);
            assert_eq!((*min,*max),(actual.min,actual.max));
        }
        s.function_text.clear();
        let mut short = Rec::default();
        draw_property_block(&mut short,&block,&s,Ink::Line,&|_|String::new(),&refs);
        assert_eq!(short.styled.len(),3);
        assert_eq!(short.styled[0].0,s.pos+block.row_pos(0,3).transform(rot,mirror));
    }}
}

#[test]
fn an_explicit_reference_field_suppresses_only_duplicate_automatic_references() {
    use crate::symbols::{BlockAlign,PropertyBlock};
    let mut prj = Project::default();
    prj.pages[0].symbols=vec![SymbolInst {id:1,sym:"coil".into(),tag:"-K1".into(),pos:p(40.,40.),..Default::default()},
        SymbolInst {id:2,sym:"no".into(),tag:"-K1".into(),pos:p(80.,40.),..Default::default()}];
    let symbol=&prj.pages[0].symbols[1];let idx=Index::build(&prj);
    let mut def=(*prj.symbol("no").unwrap()).clone();
    let mut automatic=Rec::default();draw_annotations(&mut automatic,&prj,&idx,0,&def,symbol,Ink::Line);
    assert_eq!(automatic.page_links.len(),1);
    def.prims.push(Prim::PropertyBlock(PropertyBlock {rows:vec![BlockRow::Property(references::PROPERTY.into())],
        pos:p(0.,0.),step:p(0.,3.),size:2.,anchor:Anchor::Left,angle:0.,align:BlockAlign::Start,color:None}));
    let mut explicit=Rec::default();draw_annotations(&mut explicit,&prj,&idx,0,&def,symbol,Ink::Line);
    assert!(explicit.page_links.is_empty());
    assert!(explicit.texts.iter().any(|(_,t,_,_)|t=="-K1"));
}

#[test]
fn cabinet_front_artwork_and_text_follow_explicit_half_turn() {
    let model=Model2d {width:20.,height:40.,prims:vec![
        Prim2d::Line {a:[3.,7.],b:[5.,11.]},
        Prim2d::Text {pos:[3.,7.],text:"A1".into(),size:2.},
    ]};
    let item=PanelItem {x:100.,y:200.,w:20.,h:40.,rotation:Some(panel::PanelRotation::Deg180),..Default::default()};
    let mut draw=Rec::default();
    draw_model_2d(&mut draw,&model,&item,&p,1.,Ink::Line);
    assert_eq!(draw.line_positions[0],(p(117.,233.),p(115.,229.)));
    assert_eq!(draw.styled[0].0,p(117.,233.));assert_eq!(draw.styled[0].2.angle,-180.);
}

#[test]
fn placed_wire_labels_render_live_values_despite_hidden_automatic_labels_after_reopen() {
    use crate::model::wiring::WireLabel;
    let mut prj=Project::default();
    prj.pages[0].wires.push(Wire {id:10,a:p(40.,40.),b:p(80.,40.),core:"BN".into(),hide_labels:true,
        labels:vec![WireLabel {along:0.25,offset:p(-0.5,-0.25),angle:-90.,..Default::default()}],..Default::default()});
    for value in ["BN","BU",""] {
        prj.pages[0].wires[0].core=value.into();
        prj=Project::from_json(&prj.to_json()).unwrap();
        let an=Analysis::with_builtin(&prj);let mut draw=Rec::default();
        draw_wire_labels(&mut draw,&prj,&an,0);
        if value.is_empty() {assert!(draw.styled.is_empty());}
        else {
            assert_eq!(draw.styled.len(),1);
            assert_eq!(draw.styled[0].0,p(49.5,39.75));assert_eq!(draw.styled[0].1,value);
            assert_eq!(draw.styled[0].2.angle,-90.);
        }
    }
}

#[test]
fn attribute_blocks_flow_live_values_and_transform_every_line() {
    use crate::symbols::{BlockAlign, PropertyBlock};
    let block = PropertyBlock {
        rows: vec![BlockRow::Field(FieldKind::FunctionText), BlockRow::Property("function.selection_text".into()), BlockRow::Text("Last".into())],
        pos: p(2., 3.), step: p(3., 0.), size: 2., anchor: Anchor::Left, angle: 90., align: BlockAlign::End, color: None,
    };
    for rot in 0..4 {
        for mirror in [false, true] {
            let mut s = SymbolInst { pos: p(40., 40.), rot, mirror, ..Default::default() };
            s.values.insert("function.selection_text".into(), "Choice".into());
            let mut draw = Rec::default();
            draw_prims(&mut draw, &[Prim::PropertyBlock(block.clone())], &s, Ink::Line);
            assert_eq!(draw.styled.len(), 2);
            assert_eq!(draw.styled[0].0, s.pos + p(-1., 3.).transform(rot, mirror));
            assert_eq!(draw.styled[1].0, s.pos + p(2., 3.).transform(rot, mirror));
            s.function_text = "First\n\nThird".into();
            let mut draw = Rec::default();
            draw_prims(&mut draw, &[Prim::PropertyBlock(block.clone())], &s, Ink::Line);
            assert_eq!(draw.styled.iter().map(|(_, text, _)| text.as_str()).collect::<Vec<_>>(), ["First", "Third", "Choice", "Last"]);
            assert_eq!(draw.styled[0].0, s.pos + p(-10., 3.).transform(rot, mirror));
            assert_eq!(draw.styled[3].0, s.pos + p(2., 3.).transform(rot, mirror));
            assert!(draw.styled.iter().all(|(_, _, style)| style.angle > -90. && style.angle <= 90.));
        }
    }
}

#[test]
fn attribute_block_part_values_use_full_identity_on_structured_pages() {
    use crate::symbols::{BlockAlign, PropertyBlock};
    let mut prj = Project::default();
    prj.pages[0].name = "=F1+L1/1".into();
    prj.symbols.push(crate::symlib::UserSymbol {
        key: "user:block".into(), show_tag: true,
        prims: vec![Prim::PropertyBlock(PropertyBlock {
            rows: vec![BlockRow::Field(FieldKind::FunctionText), BlockRow::Property("part.number".into()), BlockRow::Property("function.manufacturer".into())],
            pos: p(0., 0.), step: p(0., 3.), size: 2., anchor: Anchor::Left, angle: 0., align: BlockAlign::Start, color: None,
        })], ..Default::default()
    });
    prj.parts.push(crate::model::Part { number: "SECOND".into(), manufacturer: "Maker".into(), ..Default::default() });
    prj.devices.insert("=F1+L1-K1".into(), crate::model::DeviceData {
        parts: vec![crate::model::PartRef { number: "FIRST".into(), count: 1, total:false }, crate::model::PartRef { number: "SECOND".into(), count: 1, total:false }], ..Default::default()
    });
    prj.pages[0].symbols.push(SymbolInst {
        id: 10, sym: "user:block".into(), tag: "=F1+L1-K1".into(), function_text: "Live".into(), pos: p(40., 40.),
        values: [("function.part_number".into(), "SECOND".into())].into_iter().collect(), ..Default::default()
    });
    let prj = Project::from_json(&prj.to_json()).unwrap();
    let an = Analysis::with_builtin(&prj);
    let mut draw = Rec::default();
    draw_page(&mut draw, &prj, &an.index, &an, 0, &Opts::default());
    assert!(draw.texts.iter().any(|(_, text, _, _)| text == "SECOND"));
    assert!(draw.texts.iter().any(|(_, text, _, _)| text == "Maker"));
    assert_eq!(draw.texts.iter().filter(|(_, text, _, _)| text == "Live").count(), 1);
}

#[test]
fn separately_annotated_wires_keep_properties_without_duplicate_labels() {
    let mut prj = Project::default();
    prj.pages[0].wires.push(Wire { id: 10, a: p(20., 20.), b: p(80., 20.), number: "101".into(), cable: "-W1".into(), core: "BN".into(), hide_labels: true, ..Default::default() });
    let prj = Project::from_json(&prj.to_json()).unwrap();
    let analysis = Analysis::with_builtin(&prj);
    let mut hidden = Rec::default();
    draw_wire_labels(&mut hidden, &prj, &analysis, 0);
    assert!(hidden.texts.is_empty() && hidden.lines.is_empty());
    assert_eq!(prj.pages[0].wires[0].number, "101");
    let mut visible = prj.clone();
    visible.pages[0].wires[0].hide_labels = false;
    let mut draw = Rec::default();
    draw_wire_labels(&mut draw, &visible, &analysis, 0);
    assert!(draw.texts.iter().any(|(_,text,_,_)| text == "101"));
    assert!(draw.texts.iter().any(|(_,text,_,_)| text == "-W1"));
    assert!(draw.texts.iter().any(|(_,text,_,_)| text == "BN"));
    assert!(!serde_json::from_str::<Wire>("{}").unwrap().hide_labels);
}

#[test]
fn imported_pin_fields_follow_instance_designations_and_explicit_angles() {
    let mut prj = Project::default();
    prj.symbols.push(crate::symlib::UserSymbol {
        key: "user:pin_field".into(), show_tag: false,
        pins: vec![crate::symlib::UserPin { pos: p(0., 0.), label: "13".into(), internal: false }],
        prims: vec![Prim::Field { kind: FieldKind::Pin(0), pos: p(0., 2.), size: 2., anchor: Anchor::Left, angle: Some(90.) }],
        ..Default::default()
    });
    prj.pages[0].symbols.push(SymbolInst { id: 2, sym: "user:pin_field".into(), pos: p(40., 40.), rot: 1, pins: vec!["99".into()], ..Default::default() });
    let analysis = Analysis::with_builtin(&prj);
    let mut draw = Rec::default();
    draw_page(&mut draw, &prj, &analysis.index, &analysis, 0, &Opts::default());
    let field = draw.styled.iter().find(|(_, text, _)| text == "99").unwrap();
    assert_eq!(field.2.angle, 0.);
}

#[derive(Default)]
struct Rec {
    rects: Vec<(P, P)>,
    texts: Vec<(P, String, f32, Anchor)>,
    styled: Vec<(P, String, TextStyle)>,
    polys: Vec<(Vec<P>, Ink, bool)>,
    urls: Vec<(P, P, String)>,
    lines: Vec<Ink>,
    line_positions: Vec<(P,P)>,
    page_links: Vec<(P,P,usize)>,
}

impl Draw for Rec {
    fn line(&mut self, a: P, b: P, _: f32, ink: Ink, _: bool) {
        self.lines.push(ink);
        self.line_positions.push((a,b));
    }
    fn poly(&mut self, pts: &[P], _: bool, _: f32, ink: Ink, fill: bool) {
        self.polys.push((pts.to_vec(), ink, fill));
    }
    fn circle(&mut self, _: P, _: f32, _: f32, _: Ink, _: bool) {}
    fn text(&mut self, pos: P, text: &str, size: f32, h: Anchor, _: VAnchor, _: Ink) {
        self.texts.push((pos, text.into(), size, h));
    }
    fn styled_text(&mut self, pos: P, text: &str, style: &TextStyle, ink: Ink) {
        self.styled.push((pos, text.into(), *style));
        self.text(pos, text, style.size, style.h, style.v, ink);
    }
    fn rect(&mut self, a: P, b: P, _: f32, _: Ink, _: bool) {
        self.rects.push((a, b));
    }
    fn link_url(&mut self, min: P, max: P, url: &str) {
        self.urls.push((min, max, url.into()));
    }
    fn link(&mut self, min:P, max:P, page:usize) { self.page_links.push((min,max,page)); }
}

#[test]
fn imported_page_can_hide_generated_frame_while_retaining_paper_geometry() {
    let mut project=Project::default();
    let frame=crate::frame::PlotFrame { name:"Imported".into(), visible:false, ..Default::default() };
    project.pages[0].frame=frame.name.clone();
    project.forms.frames.push(frame);
    let mut drawing=Rec::default();
    draw_frame(&mut drawing,&project,0);
    assert!(drawing.rects.is_empty() && drawing.lines.is_empty() && drawing.texts.is_empty());
    assert_eq!(crate::frame::of_page(&project,0).size(),(420.,297.));
    let old:crate::frame::PlotFrame=serde_json::from_str("{}").unwrap();
    assert!(old.visible);
    assert_eq!(crate::frame::Labels::ZeroBasedNumbers.label(0),"0");
}

#[test]
fn component_tag_field_omits_inherited_structure_and_stays_dynamic() {
    let project=Project::default();
    let mut definition=(*project.symbol("coil").unwrap()).clone();
    definition.show_tag=false;
    definition.prims=vec![Prim::Field {kind:FieldKind::TagName,pos:p(0.,0.),size:2.,anchor:Anchor::Left,angle:None}];
    let mut drawing=Rec::default();
    draw_symbol(&mut drawing,&definition,&SymbolInst {tag:"=F1+L1-K2".into(),..Default::default()},Ink::Line);
    assert!(drawing.texts.iter().any(|(_,text,_,_)| text=="-K2"));
    assert!(drawing.texts.iter().all(|(_,text,_,_)| !text.contains("L1")));
}

#[test]
fn product_fields_follow_renames_and_preserve_only_requested_levels() {
    let project = Project::default();
    let mut definition = (*project.symbol("coil").unwrap()).clone();
    definition.show_tag = false;
    for (levels, tag, expected) in [
        (2, "=FIELD+M350-CAB-350-WE1", "-350-WE1"),
        (2, "=FIELD+M350-CAB-351-WE2", "-351-WE2"),
        (1, "=FIELD+M350-CAB-351-WE2", "-WE2"),
        (20, "=FIELD+M350-350-WE1", "-350-WE1"),
        (0, "=FIELD+M350-350-WE1", "-WE1"),
        (2, "=FIELD+M350", ""),
        (2, "-Ä1-K2", "-Ä1-K2"),
    ] {
        definition.prims = vec![Prim::Field { kind: FieldKind::Product(levels), pos: p(0., 0.), size: 2., anchor: Anchor::Left, angle: None }];
        let serialized = serde_json::to_string(&definition.prims).unwrap();
        let prims: Vec<Prim> = serde_json::from_str(&serialized).unwrap();
        let mut drawing = Rec::default();
        draw_prims(&mut drawing, &prims, &SymbolInst { tag: tag.into(), ..Default::default() }, Ink::Line);
        let texts: Vec<_> = drawing.texts.iter().map(|(_, text, _, _)| text.as_str()).collect();
        if expected.is_empty() { assert!(texts.is_empty()); } else { assert_eq!(texts, vec![expected]); }
    }
}

#[test]
fn live_function_fields_match_source_text_layout_and_follow_edits() {
    for (rot, mirror) in [(0, false), (1, false), (0, true), (2, true)] {
        let mut s = SymbolInst { pos: p(40., 40.), rot, mirror, function_text: "First\nSecond".into(), ..Default::default() };
        let field = Prim::Field { kind: FieldKind::FunctionText, pos: p(2., 3.), size: 2., anchor: Anchor::Left, angle: Some(0.) };
        let text = Prim::Text { pos: p(2., 3.), text: s.function_text.clone(), size: 2., anchor: Anchor::Left, angle: Some(0.), color: None };
        let mut live = Rec::default();
        let mut source = Rec::default();
        draw_prims(&mut live, std::slice::from_ref(&field), &s, Ink::Line);
        draw_prims(&mut source, &[text], &s, Ink::Line);
        assert_eq!(live.texts, source.texts);
        assert_eq!(live.styled.len(), 2);
        for (a, b) in live.styled.iter().zip(&source.styled) { assert_eq!((a.2.angle, a.2.h), (b.2.angle, b.2.h)); }
        s.function_text = "Changed".into();
        let mut updated = Rec::default();
        draw_prims(&mut updated, &[field], &s, Ink::Line);
        assert_eq!(updated.texts.len(), 1);
        assert_eq!(updated.texts[0].1, "Changed");
    }
}

#[test]
fn explicit_function_field_avoids_a_second_automatic_annotation() {
    let mut prj = Project::default();
    prj.symbols.push(crate::symlib::UserSymbol {
        key: "user:live_function".into(),
        prims: vec![Prim::Field { kind: FieldKind::FunctionText, pos: p(0., 0.), size: 2., anchor: Anchor::Left, angle: Some(0.) }],
        ..Default::default()
    });
    prj.pages[0].symbols.push(SymbolInst { id: 50, sym: "user:live_function".into(), pos: p(40., 40.), tag: "-K1".into(), function_text: "Live note".into(), ..Default::default() });
    let analysis = Analysis::with_builtin(&prj);
    let mut drawing = Rec::default();
    draw_page(&mut drawing, &prj, &analysis.index, &analysis, 0, &Opts::default());
    assert_eq!(drawing.texts.iter().filter(|(_, t, _, _)| t == "Live note").count(), 1);
    let def = prj.symbol("user:live_function").unwrap();
    let mut instance = prj.pages[0].symbols[0].clone();
    instance.tag.clear();
    assert!(tag_block_bounds(&prj, 0, &def, &instance).is_none());
}

#[test]
fn transformed_imported_labels_stay_upright_at_their_pin() {
    let label = Prim::Text { pos: p(2., 0.), text: "U1".into(), size: 2., anchor: Anchor::Left, angle: Some(0.), color: None };
    for (rot, mirror, anchor) in [(0, true, Anchor::Right), (2, false, Anchor::Right), (2, true, Anchor::Left)] {
        let s = SymbolInst { rot, mirror, ..Default::default() };
        let mut draw = Rec::default();
        draw_prims(&mut draw, std::slice::from_ref(&label), &s, Ink::Line);
        let (pos, _, style) = &draw.styled[0];
        assert_eq!(*pos, p(2., 0.).transform(rot, mirror));
        assert_eq!((style.angle, style.h), (0., anchor));
    }
}

#[test]
fn free_texts_are_drawn_with_style_background_and_link() {
    let mut prj = Project::default();
    prj.info.name = "Pump station".into();
    prj.pages[0].texts.push(TextItem {
        id: 1,
        pos: p(100.0, 100.0),
        text: "{project}\nline 2".into(),
        angle: 90.0,
        bold: true,
        background: true,
        frame: true,
        link: "https://example.org".into(),
        ..Default::default()
    });
    let an = Analysis::with_builtin(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, 0, &Opts::default());
    let mine: Vec<_> = d.styled.iter().filter(|(_, _, st)| st.angle == 90.0).collect();
    assert_eq!(mine.len(), 2);
    assert_eq!(mine[0].1, "Pump station", "placeholders are filled in");
    assert!(mine[0].2.bold && mine[0].2.v == VAnchor::Middle);
    assert!((mine[1].0.x - 104.2).abs() < 0.01 && (mine[1].0.y - 100.0).abs() < 0.01, "second line to the right: {:?}", mine[1].0);
    assert!(d.polys.iter().any(|(pts, ink, fill)| pts.len() == 4 && *ink == Ink::Paper && *fill), "background");
    assert!(d.polys.iter().any(|(pts, ink, fill)| pts.len() == 4 && *ink == Ink::Line && !*fill), "frame");
    assert_eq!(d.urls.len(), 1);
    assert_eq!(d.urls[0].2, "https://example.org");
    let b = text_bounds(&prj, 0, &prj.pages[0].texts[0]);
    assert!((b.min.x - 97.9).abs() < 0.01 && (b.max.x - 106.3).abs() < 0.01, "{b:?}");
    assert!((b.min.y - 77.68).abs() < 0.01 && (b.max.y - 100.0).abs() < 0.01, "{b:?}");
}

#[test]
fn cover_rows_sit_centred_inside_their_box() {
    let mut prj = crate::sample::demo_project();
    prj.info.company = "Schaltanlagenbau Musterstadt GmbH & Co. KG, Werk Nord".into();
    let cover = prj.pages.iter().position(|pg| pg.kind == PageKind::Cover).unwrap();
    let mut d = Rec::default();
    draw_cover(&mut d, &prj, &crate::frame::of_page(&prj, cover));
    let (a, b) = *d.rects.last().unwrap();
    let rows: Vec<_> = d.texts.iter().filter(|t| t.0.y > a.y).collect();
    assert_eq!(rows.len(), 10);
    for (pos, text, size, h) in &rows {
        let w = text_width(text, *size);
        let (x0, x1) = match h {
            Anchor::Left => (pos.x, pos.x + w),
            Anchor::Right => (pos.x - w, pos.x),
            Anchor::Center => (pos.x - w / 2.0, pos.x + w / 2.0),
        };
        assert!(x0 > a.x && x1 < b.x, "{text} sticks out of the box");
        assert!(pos.y - size / 2.0 > a.y && pos.y + size / 2.0 < b.y, "{text} overlaps the box");
    }
    let first = rows.iter().map(|t| t.0.y).fold(f32::INFINITY, f32::min);
    let last = rows.iter().map(|t| t.0.y).fold(f32::NEG_INFINITY, f32::max);
    assert!(((first - a.y) - (b.y - last)).abs() < 0.01);
}

#[test]
fn graphical_terminal_and_cable_diagrams() {
    use crate::forms::ReportForm;
    use crate::model::ReportKind;
    let mut prj = crate::sample::demo_project();
    let data = prj.strips.entry("-X1".into()).or_default();
    data.jumpers.insert("1".into(), 1);
    data.jumpers.insert("2".into(), 1);
    for kind in [ReportKind::TerminalDiagram, ReportKind::CableDiagram] {
        prj.forms.reports.insert(kind.into(), ReportForm { graphic: true, ..Default::default() });
    }
    let at = prj.pages.len();
    crate::ops::add_page(&mut prj, PageKind::Report(ReportKind::TerminalDiagram.into()), at);
    crate::ops::add_page(&mut prj, PageKind::Report(ReportKind::CableDiagram.into()), at + 1);
    let an = Analysis::with_builtin(&prj);
    let table = crate::report_blocks::page_table(&prj, &an, at).unwrap();
    assert!(table.graphic);
    assert_eq!(table.per_page, crate::report_blocks::GRAPHIC_ROWS_PER_PAGE);

    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, at, &Opts::default());
    let texts: Vec<&str> = d.texts.iter().map(|t| t.1.as_str()).collect();
    assert!(texts.contains(&"-X1"), "strip heading");
    for label in ["1", "2", "3", "PE"] {
        assert!(texts.contains(&label), "terminal {label}");
    }
    assert!(texts.iter().any(|t| t.starts_with("Plug-in jumper")), "jumper listed below: {texts:?}");
    // A box per terminal (4) and a bar per end bracket / end plate (4), and the frame.
    assert!(d.rects.len() >= 8, "{} boxes", d.rects.len());
    assert!(!texts.contains(&"External (top)"), "no table header");

    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, at + 1, &Opts::default());
    let texts: Vec<&str> = d.texts.iter().map(|t| t.1.as_str()).collect();
    assert!(texts.iter().any(|t| t.starts_with("-W1")), "cable band heading: {texts:?}");

    // Without the switch the table is drawn.
    prj.forms.reports.clear();
    let an = Analysis::with_builtin(&prj);
    assert!(!crate::report_blocks::page_table(&prj, &an, at).unwrap().graphic);
    assert!(crate::report_blocks::has_graphic(ReportKind::CableDiagram) && !crate::report_blocks::has_graphic(ReportKind::DeviceList));
}

#[test]
fn graphical_plug_diagram_and_plc_assignment() {
    use crate::forms::ReportForm;
    use crate::model::ReportKind;
    let mut prj = crate::sample::demo_project();
    for kind in [ReportKind::PlugDiagram, ReportKind::PlcAssignment] {
        prj.forms.reports.insert(kind.into(), ReportForm { graphic: true, ..Default::default() });
    }
    let at = prj.pages.len();
    crate::ops::add_page(&mut prj, PageKind::Report(ReportKind::PlugDiagram.into()), at);
    crate::ops::add_page(&mut prj, PageKind::Report(ReportKind::PlcAssignment.into()), at + 1);
    let an = Analysis::with_builtin(&prj);
    for (page, heading) in [(at, "Socket side"), (at + 1, "Inputs from")] {
        let table = crate::report_blocks::page_table(&prj, &an, page).unwrap();
        assert!(table.graphic);
        let mut d = Rec::default();
        draw_page(&mut d, &prj, &an.index, &an, page, &Opts::default());
        let texts: Vec<&str> = d.texts.iter().map(|t| t.1.as_str()).collect();
        assert!(texts.contains(&heading), "{texts:?}");
        if let Some(first) = table.rows.first() {
            assert!(texts.contains(&first[0].as_str()), "group heading {}", first[0]);
        }
    }
    let plc = crate::report_blocks::page_table(&prj, &an, at + 1).unwrap();
    assert!(!plc.rows.is_empty(), "the demo has PLC points");
    assert!(crate::report_blocks::has_graphic(ReportKind::PlugDiagram) && crate::report_blocks::has_graphic(ReportKind::PlcAssignment));
}

#[test]
fn mounting_pages_show_every_face_with_items() {
    let mut prj = crate::sample::demo_project();
    let pi = prj.pages.iter().position(|p| matches!(p.kind, PageKind::Mounting(_))).unwrap();
    let PageKind::Mounting(id) = prj.pages[pi].kind else { unreachable!() };
    let e = prj.layout.enclosures.iter_mut().find(|e| e.id == id).unwrap();
    let mut fan = e.items.iter().find(|i| i.face == crate::panel::Face::Plate).cloned().unwrap();
    fan.id = 987_654;
    fan.face = crate::panel::Face::Roof;
    fan.x = 10.0;
    fan.y = 10.0;
    e.items.push(fan);
    let an = Analysis::checked(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, pi, &Opts::default());
    let texts: Vec<&str> = d.texts.iter().map(|t| t.1.as_str()).collect();
    assert!(texts.contains(&"Mounting plate") && texts.contains(&"Roof"), "{texts:?}");
}

#[test]
fn a_row_of_terminals_shows_the_strip_tag_once() {
    let mut prj = Project::default();
    for (i, x) in [100.0, 112.0, 124.0, 200.0].into_iter().enumerate() {
        let label = (i + 1).to_string();
        prj.pages[0].symbols.push(SymbolInst { id: 10 + i as u64, sym: "term".into(), pos: p(x, 100.0), tag: "-X1".into(), label, ..Default::default() });
    }
    let an = Analysis::with_builtin(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, 0, &Opts::default());
    let tags: Vec<f32> = d.texts.iter().filter(|t| t.1 == "-X1").map(|t| t.0.x).collect();
    assert_eq!(tags.len(), 2, "first terminal of each row: {tags:?}");
    assert!(tags.iter().all(|x| *x < 100.0 || (*x > 180.0 && *x < 200.0)), "{tags:?}");
}

#[test]
fn the_contact_image_of_a_turned_coil_stays_clear_of_its_pins() {
    let mut prj = Project::default();
    let sym = |id: u64, key: &str, pos: P, rot: u8, pins: &[&str]| SymbolInst {
        id,
        sym: key.into(),
        pos,
        rot,
        tag: "-K1".into(),
        pins: pins.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    prj.pages[0].symbols.push(sym(1, "coil", p(200.0, 100.0), 1, &["A1", "A2"]));
    prj.pages[0].symbols.push(sym(2, "no", p(100.0, 60.0), 0, &["13", "14"]));
    let an = Analysis::with_builtin(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, 0, &Opts::default());
    let def = prj.symbol("coil").unwrap();
    let b = def.world_bounds(p(200.0, 100.0), 1, false);
    // The rows of the contact image ("13", "14") lie below the coil, nothing of it
    // beside the coil where A1 and A2 are.
    let near = |t: &&(P, String, f32, Anchor)| (t.0.x - b.center().x).abs() < 20.0;
    let image: Vec<_> = d.texts.iter().filter(near).filter(|t| t.1 == "13" || t.1 == "14").collect();
    assert_eq!(image.len(), 2, "{:?}", d.texts);
    assert!(image.iter().all(|t| t.0.y > b.max.y), "{image:?} {b:?}");
}

#[test]
fn black_wires_are_drawn_black_not_in_the_wire_ink() {
    let mut prj = Project { wire_colors: true, ..Default::default() };
    let black = crate::potentials::color_rgb("BK").unwrap();
    let red = crate::potentials::color_rgb("RD").unwrap();
    prj.pages[0].wires.push(Wire { id: 1, a: p(100.0, 100.0), b: p(200.0, 100.0), color: "BK".into(), ..Default::default() });
    prj.pages[0].wires.push(Wire { id: 2, a: p(100.0, 150.0), b: p(200.0, 150.0), color: "RD".into(), ..Default::default() });
    let an = Analysis::with_builtin(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, 0, &Opts::default());
    assert!(d.lines.contains(&Ink::Color(black)), "{:?}", d.lines);
    assert!(d.lines.contains(&Ink::Color(red)));
    assert!(!d.lines.contains(&Ink::Wire), "no wire falls back to the wire ink");
}

#[test]
fn only_web_links_are_exported() {
    let mut prj = Project::default();
    for (id, link) in [(1, "javascript:alert(1)"), (2, "file:///C:/Windows/System32/calc.exe"), (3, "mailto:anna@example.org")] {
        prj.pages[0].texts.push(TextItem { id, pos: p(100.0, 100.0 + id as f32 * 10.0), text: "x".into(), link: link.into(), ..Default::default() });
    }
    let an = Analysis::with_builtin(&prj);
    let mut d = Rec::default();
    draw_page(&mut d, &prj, &an.index, &an, 0, &Opts::default());
    let urls: Vec<&str> = d.urls.iter().map(|u| u.2.as_str()).collect();
    assert_eq!(urls, ["mailto:anna@example.org"]);
}
