use super::*;

#[test]
fn analog_pair_is_one_symbolic_channel_even_when_the_reference_comes_first() {
    use crate::{model::Project, netlist::Netlist, ops, plc};
    let source = r#"<EplanPxfRoot Name="Analog" Type="WindowMacro"><MacroVariant MacroFuncType="1" VariantId="0" ReferencePoint="0/0/0">
    <ESymLib ID="0" Name="SPECIAL"/><O4><O26>
    <O17 A1261="0" A1262="13" A1263="0" A1381="300" A1382="3" A1383="7" A185="9" A186="U1-"><S61x183 A1061="1"/><P11 P20407="AI Standard Channel 1.Value"/><S40x1201 A762="8/0"/></O17>
    <O17 A1261="0" A1262="13" A1263="0" A1381="300" A1382="1" A1383="4" A185="1" A186="U1+"><S61x183 A1061="1"/><P11 P20405="INT" P20407="AI Standard Channel 1.Value"/></O17>
    </O26></O4></MacroVariant></EplanPxfRoot>"#;
    for (code, kind) in [("4", "AI"), ("5", "AO")] {
        let vs = decode(&source.replace("A1383=\"4\"", &format!("A1383=\"{code}\""))).unwrap();
        let mut prj = Project::new_default();
        import_project(&mut prj, &vs);
        let def = prj.symbol(&vs[0].symbol.key).unwrap();
        let s = ops::new_symbol(&mut prj, 0, &Default::default(), &def, p(40., 40.), 0, false);
        assert_eq!(s.pins, ["9", "1"]);
        assert_eq!(s.label, "U1");
        let tag = s.tag.clone();
        prj.pages[0].symbols.push(s);
        assert_eq!(plc::readdress(&mut prj, &tag), 0);
        let nl = Netlist::build(&prj, true);
        let points = plc::points(&prj, &nl);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].io_kind(), kind);
        assert_eq!(points[0].terminal, "1");
        assert!(plc::twincat_gvl(&prj, &nl).contains("* : INT;"));
        assert!(def.bridges.is_empty());
    }
    let different = source.replacen("Channel 1.Value", "Channel 2.Value", 1);
    assert!(decode(&different).unwrap()[0].symbol.plc_channel.is_none());
    let untyped = source.replace("P20405=\"INT\"", "").replace("AI Standard Channel 1.Value", "Ch1 Sample 0.Ch1 Value");
    let vs = decode(&untyped).unwrap();
    let channel = vs[0].symbol.plc_channel.as_ref().unwrap();
    assert_eq!(channel.designation, "U1");
    assert!(channel.analog.as_ref().unwrap().data_type.is_empty());
    let unnumbered = source.replace("AI Standard Channel 1.Value", "AI Standard");
    let single = decode(&unnumbered).unwrap();
    assert_eq!(single[0].symbol.plc_channel.as_ref().unwrap().index, 0);
    let second = unnumbered.split_once("<MacroVariant").unwrap().1.split_once("</MacroVariant>").unwrap().0;
    let ambiguous = unnumbered.replace(
        "</EplanPxfRoot>",
        &format!("<MacroVariant{}</MacroVariant></EplanPxfRoot>", second.replace("VariantId=\"0\"", "VariantId=\"1\"").replace("U1", "U2")),
    );
    assert!(decode(&ambiguous).unwrap().iter().all(|v| v.symbol.plc_channel.is_none()));
    let numbered = source.replace("AI Standard Channel 1.Value", "Ch. 2.Ch. 2 Al Inputs.Value").replace("U1+", "+I2");
    let numbered = decode(&numbered).unwrap();
    let channel = numbered[0].symbol.plc_channel.as_ref().unwrap();
    assert_eq!((&*channel.designation, channel.index), ("I2", 1));
}

#[test]
fn digital_channels_keep_their_card_bit_and_terminal_when_placed_out_of_order() {
    use crate::{model::Project, netlist::Netlist, ops, plc, unplaced};
    let source = |direction, index, pin| {
        format!(
            r#"<MacroVariant MacroFuncType="1" VariantId="{index}" ReferencePoint="0/0/0">
        <ESymLib ID="0" Name="SPECIAL"/><O4><O26><O17 A1261="0" A1262="13" A1263="0" A1381="300" A1382="1" A1383="{direction}" A185="{pin}">
        <S61x183 A1061="1"/><P11 P20405="BOOL" P20407="Channel {index}" P20011="??_??@Sensor;"/><S53x5 A501="0/0" A771="20400"/>
        </O17></O26></O4></MacroVariant>"#
        )
    };
    for (direction, input, prefix) in [(2, true, "I"), (3, false, "Q")] {
        let text = format!(r#"<EplanPxfRoot Name="Card" Type="WindowMacro">{}{}</EplanPxfRoot>"#, source(direction, 1, 1), source(direction, 2, 5));
        let variants = decode(&text).unwrap();
        let mut prj = Project::new_default();
        import_project(&mut prj, &variants);
        let place = |p: &mut Project, v: &Variant, x| {
            let def = p.symbol(&v.symbol.key).unwrap();
            let s = ops::new_symbol(p, 0, &Default::default(), &def, super::p(x, 40.), 0, false);
            p.pages[0].symbols.push(s);
        };
        place(&mut prj, &variants[1], 8.);
        let card = prj.pages[0].symbols[0].tag.clone();
        assert_eq!(prj.pages[0].symbols[0].pins, ["5"]);
        assert_eq!(prj.pages[0].symbols[0].label, format!("{prefix}0.1"));
        assert_eq!(unplaced::plc_channels(&prj)[0].pins, ["1"]);
        place(&mut prj, &variants[0], 80.);
        assert_eq!(prj.pages[0].symbols[1].tag, card);
        assert!(unplaced::plc_channels(&prj).is_empty());
        prj.plc_cards.entry(card.clone()).or_default().start_byte = 4;
        assert_eq!(plc::readdress(&mut prj, &card), 2);
        let back: Project = serde_json::from_value(serde_json::to_value(&prj).unwrap()).unwrap();
        let points = plc::points(&back, &Netlist::build(&back, true));
        assert_eq!(points.len(), 2);
        assert!(points.iter().all(|p| p.input == input && p.function_text == "Sensor"));
        assert_eq!(points[1].address, format!("{prefix}4.1"));
        assert_eq!((&*points[1].channel, &*points[1].terminal), ("Channel 2", "5"));
        let mut legacy = Project::new_default();
        let old = decode(&text.replace("P20405=\"BOOL\"", "")).unwrap();
        import_project(&mut legacy, &old);
        place(&mut legacy, &old[1], 8.);
        let previous_tag = legacy.pages[0].symbols[0].tag.clone();
        let authored = decode(&text.replace("P20405=", "P20400=\"remote_address\" P20405=")).unwrap();
        import_project(&mut legacy, &authored);
        assert_eq!(legacy.pages[0].symbols[0].tag, previous_tag);
        assert_eq!(legacy.pages[0].symbols[0].label, "remote_address");
        legacy.pages[0].symbols[0].label = "OperatorAddress".into();
        import_project(&mut legacy, &variants);
        assert_eq!(legacy.pages[0].symbols[0].label, "OperatorAddress");
        place(&mut prj, &variants[0], 100.);
        assert_ne!(prj.pages[0].symbols[2].tag, card);
        let supply = decode(&text.replace("A1382=\"1\"", "A1382=\"2\"")).unwrap();
        assert!(supply.iter().all(|v| v.symbol.plc_channel.is_none()));
        assert!(decode(&text.replace("MacroFuncType=\"1\"", "MacroFuncType=\"4\"")).unwrap().iter().all(|v| v.symbol.plc_channel.is_none()));
    }
}

#[test]
fn placeholder_preserves_selected_set_and_escaped_values() {
    let control = r#"<O106 A1261="0" A1262="323" A1892="mode" A1896="X;Y"><P11 P19301="active"/><S114x1894 A2011="active" A2012="left\;value;."/><S109x1895 R1906="17/2"><S115x1904 A1911="511" A1913="X"/></S109x1895></O106>"#;
    let v = decode(&MACRO.replace("<O4><O26>", &format!("<O4><O26>{control}")).replace("<S31x5 ", "<S31x5 A406=\"1\" ")).unwrap();
    let state = &v[0].placeholders[0];
    assert_eq!(state.selected, "active");
    assert_eq!(state.variables, ["X", "Y"]);
    assert_eq!(state.sets[0].values, ["left;value", "."]);
    assert_eq!(state.bindings[0].object, "17/2");
    assert!(!v[0].symbol.prims.contains(&Prim::Line { a: p(2., 2.), b: p(4., -1.), dashed: false }));
}

#[test]
fn empty_multilingual_properties_stay_blank() {
    assert_eq!(display_text("??_??@;"), "");
    assert_eq!(display_text("de_DE@;en_US@;"), "");
    assert_eq!(display_text("de_DE@Eingang;??_??@Input;"), "Input");
}

const MACRO: &str = r#"<EplanPxfRoot Name="EK1100" Type="WindowMacro">
<MacroVariant MacroFuncType="1" VariantId="0" ReferencePoint="72/92/0">
<ESymLib ID="0" Name="SPECIAL"/><O4><O26>
<O5 A1="5/1"><S31x5 A531="0/0" A532="2/3"/></O5>
<O144 R9="5/1"><S40x1201 A762="74/90"/></O144>
<O17 A185="1" A186="24V" A1261="0" A1262="13" A1263="0"><S61x183 A1061="1"/><S40x1201 A762="80/84"/></O17>
<O17 A185="9" A1261="0" A1262="999" A1263="1"><S40x1201 A762="96/84"/></O17>
</O26></O4></MacroVariant>
<MacroVariant MacroFuncType="4" VariantId="0" ReferencePoint="0/0/0"/></EplanPxfRoot>"#;

#[test]
fn macro_graphics_and_known_contacts_keep_their_relative_positions() {
    let v = decode(MACRO).unwrap();
    assert_eq!(v.len(), 2);
    assert_ne!(v[0].symbol.key, v[1].symbol.key);
    assert_eq!(v[0].symbol.pins, [UserPin { pos: p(8., 4.), label: "1".into() }]);
    assert_eq!(v[0].symbol.prims[0], Prim::Line { a: p(2., 2.), b: p(4., -1.), dashed: false });
    assert!(v[0].warnings.contains("library SPECIAL/999/1"));
    let mut symbols = Vec::new();
    assert_eq!(install(&mut symbols, &v), 1);
    symbols[0].name = "Local edit".into();
    assert_eq!(install(&mut symbols, &v), 0);
    assert_eq!(symbols[0].name, "Local edit");
}

#[test]
fn invalid_geometry_and_recursive_drawings_are_bounded() {
    assert!(decode(&MACRO.replace("72/92/0", "NaN/92/0")).is_err());
    assert!(decode(&MACRO.replace("<S31x5 A531=\"0/0\" A532=\"2/3\"/>", "<S144x5 R9=\"5/1\"/>")).is_err());
}

#[test]
fn explicit_and_inherited_drawing_strokes_survive_conversion() {
    let text = MACRO
        .replace("<ESymLib", "<O76 A1426=\"1000\" A1428=\"1\" A1427=\"1\" A1429=\"0.35\"/><ESymLib")
        .replace("A531=\"0/0\"", "A411=\"1000\" A412=\"L\" A531=\"0/0\"");
    let variant = decode(&text).unwrap();
    let Prim::Path { color, width, dashed, .. } = &variant[0].symbol.prims[0] else { panic!("Expected a styled path") };
    assert_eq!(*color, Some([255, 0, 0]));
    assert_eq!(*width, 0.35);
    assert!(*dashed);
}

#[test]
fn referenced_devices_stay_separate_and_keep_the_macro_origin() {
    let text = MACRO
        .replace("<S61x183", "<P11 P20006=\"-A1\"/><S61x183")
        .replace("<S40x1201 A762=\"96/84\"/>", "<P11 P20006=\"-A2\"/><S40x1201 A762=\"96/84\"/>");
    let list = decode(&text).unwrap();
    assert_eq!(list[0].devices.len(), 2);
    assert_eq!(list[0].devices[0].tag, "-A1");
    assert_eq!(list[0].devices[0].symbol.pins[0].pos, p(0., -4.));
    assert_eq!(list[0].devices[0].position, p(8., 8.));
}

#[test]
fn native_master_mapping_keeps_internal_bridges_and_contact_positions() {
    let mapping = UserSymbol {
        key: "user:eplan-lib:SPECIAL:999:1".into(),
        pins: vec![UserPin { pos: p(-4., 0.), label: "a".into() }, UserPin { pos: p(4., 0.), label: "b".into() }],
        bridges: vec![(0, 1)],
        ..Default::default()
    };
    let v = decode_with_library(&MACRO.replace("A185=\"9\"", "A185=\"9;10\""), &[mapping]).unwrap();
    assert_eq!(v[0].symbol.bridges, [(1, 2)]);
    assert_eq!(v[0].symbol.pins[1].label, "9");
    assert_eq!(v[0].symbol.pins[2].pos, p(28., 8.));
    assert!(!v[0].warnings.contains("library SPECIAL/999/1"));
}

#[test]
fn explicit_logic_and_mirrored_contacts_use_the_master_direction() {
    let text = MACRO.replace("A1263=\"0\"><S61x183 A1061=\"1\"/>", "A1263=\"7\"><S61x183 A1061=\"0\" A1063=\"1\"><S16x1062 A161=\"1\"/></S61x183>");
    let v = decode(&text).unwrap();
    assert_eq!(v[0].symbol.pins[0], UserPin { pos: p(4., 8.), label: "1".into() });
    let unknown = &v[0].unresolved[0];
    assert_eq!(unknown.master, "SPECIAL/999/1");
    assert_eq!(unknown.designations, ["9"]);
    assert_eq!(unknown.placement, p(24., 8.));
}

#[test]
fn repeated_two_sided_designations_remain_two_independent_endpoints() {
    let text = MACRO.replace("A185=\"1\"", "A185=\"L1;L1\"").replace("A1262=\"13\"", "A1262=\"22\"");
    let v = decode(&text).unwrap();
    assert_eq!(v[0].symbol.pins, [UserPin { pos: p(8., 4.), label: "L1".into() }, UserPin { pos: p(8., 10.), label: "L1".into() }]);
    assert!(v[0].symbol.bridges.is_empty());
    let v = decode(&text.replace("L1;L1", "A;B")).unwrap();
    assert!(v[0].symbol.pins.is_empty());
    assert_eq!(v[0].unresolved[0].designations, ["A", "B"]);
}

#[test]
fn connection_controls_are_preserved_without_becoming_device_terminals() {
    let text =
        MACRO.replace("<O17 A185=\"9\"", "<O42 A1=\"42/11\" A1261=\"0\" A1262=\"255\" A1263=\"0\"><S40x1201 A762=\"84/80\"/></O42><O17 A185=\"9\"");
    let v = decode(&text).unwrap();
    let marker = &v[0].connection_markers[0];
    assert_eq!(marker.object, "42/11");
    assert_eq!(marker.position, p(12., 12.));
    assert_eq!(v[0].symbol.pins.len(), 1);
    assert!(!v[0].warnings.contains("library SPECIAL/255/0"));
}

#[test]
fn omitted_zero_coordinates_are_drawn_inside_embedded_blocks() {
    let v = decode(&MACRO.replace("A531=\"0/0\" ", "").replace("<S31x5", "<S34x5 A621=\"1/1;;2/3\"/><S31x5")).unwrap();
    assert!(matches!(&v[0].symbol.prims[0],Prim::Poly{pts,..} if pts==&[p(3.,1.),p(2.,2.),p(4.,-1.)]));
    assert_eq!(v[0].symbol.prims[1], Prim::Line { a: p(2., 2.), b: p(4., -1.), dashed: false });
}

#[test]
fn local_slots_and_named_potentials_resolve_only_reciprocal_unambiguous_pairs() {
    let device = r#"<P150 P20013="Q" P20014="1"/>"#;
    let gnd = |plug: &str, pin: &str, target: &str| {
        format!(
            r#"<O17 A1="17/{pin}" A1261="0" A1262="13" A1263="0" A185="{pin}">
        {device}<P11 P20406="{plug}"/><S61x183 A1061="0" A1063="0"><S16x1062 A161="1" A163="{target}"/></S61x183></O17>"#
        )
    };
    let a = gnd("X137", "13", "&quot;X136:2&quot;");
    let b = gnd("X136", "2", "&quot;X137:13&quot;");
    let dual = r#"<O17 A1="17/3" A1261="0" A1262="22" A1263="0" A185="L1;L1"><S61x183 A1061="1">
        <S16x1062 A161="1" A163="2"/><S16x1062 A161="2" A163="1"/></S61x183></O17>"#;
    let source = MACRO.replace("<O4><O26>", &format!("<O4><O26>{a}{b}{dual}"));
    let v = decode(&source).unwrap();
    assert_eq!(v[0].symbol.bridges, [(0, 1), (2, 3)]);
    assert!(v[0].transfers.iter().all(|r| r.status == "native bridge"));
    let missing = decode(&source.replace("&quot;X137:13&quot;", "&quot;X137:99&quot;")).unwrap();
    assert_eq!(missing[0].symbol.bridges, [(2, 3)]);
    assert!(missing[0].transfers.iter().any(|r| r.status == "one-way transfer"));
    let duplicate = decode(&source.replace(&b, &format!("{b}{b}"))).unwrap();
    assert!(duplicate[0].transfers.iter().any(|r| r.status == "ambiguous target designation"));
    assert_eq!(duplicate[0].symbol.bridges, [(3, 4)]);
}

#[test]
fn reimport_preserves_placed_pin_overrides_when_new_contacts_precede_existing_ones() {
    let mut project = crate::model::Project::new_default();
    let old = decode(MACRO).unwrap();
    import_project(&mut project, &old);
    project.pages[0].symbols.push(crate::model::SymbolInst { sym: old[0].symbol.key.clone(), pins: vec!["custom".into()], ..Default::default() });
    let extra = r#"<O17 A1261="0" A1262="17" A1263="2" A185="2"><S61x183 A1061="1"/><S40x1201 A762="72/76"/></O17>"#;
    let expanded = decode(&MACRO.replace("<O4><O26>", &format!("<O4><O26>{extra}"))).unwrap();
    assert_eq!(import_project(&mut project, &expanded), 0);
    assert_eq!(project.pages[0].symbols[0].pins, ["2", "custom"]);
    assert_eq!(project.symbols[0].pins[1], old[0].symbol.pins[0]);
    let moved = decode(&MACRO.replace("80/84", "84/84")).unwrap();
    import_project(&mut project, &moved);
    assert_eq!(project.symbols[0], expanded[0].symbol);
}

#[test]
fn reimport_updates_source_graphics_and_preserves_changed_endpoints() {
    let v = decode(MACRO).unwrap();
    let mut symbols = Vec::new();
    install(&mut symbols, &v);
    let newer = decode(&MACRO.replace("A532=\"2/3\"", "A532=\"5/3\"")).unwrap();
    install(&mut symbols, &newer);
    assert_eq!(symbols[0], newer[0].symbol);
    let moved = decode(&MACRO.replace("80/84", "84/84")).unwrap();
    install(&mut symbols, &moved);
    assert_eq!(symbols[0], newer[0].symbol);
}
