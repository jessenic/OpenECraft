use super::*;
use crate::geom::p;
use crate::symlib::{UserPin,UserSymbol};

fn module()->Project {
    let mut prj=Project::default();
    let channels=(0..8).map(|i|PlcChannelDef {key:format!("ch{i}"),index:i as u16,input:Some(true),
        data_type:"Bool".into(),designation:i.to_string(),pins:vec![i],signal_pin:Some(i),..Default::default()}).collect();
    prj.symbols.push(UserSymbol {key:"user:module8".into(),func:Func::Plc,plc_channels:channels,
        pins:(0..8).map(|i|UserPin {pos:p(0.,i as f32*4.),label:(i+1).to_string(),internal:false}).collect(),..Default::default()});
    prj.pages[0].symbols.push(SymbolInst {id:100,sym:"user:module8".into(),tag:"-A1".into(),pos:p(40.,40.),
        pins:(1..=8).map(|i|i.to_string()).collect(),..Default::default()});
    prj
}

#[test]
fn complete_module_functions_and_unplaced_navigator_count_logical_channels_once() {
    let mut prj=linked_module();
    let part=crate::parts::Part {number:"Card8".into(),functions:(1..=8).map(|i|crate::parts::FunctionTemplate {
        kind:crate::parts::FnKind::PlcInput,pins:i.to_string(),..Default::default()}).collect(),..Default::default()};
    prj.parts.push(part);
    prj.devices.entry("-A1".into()).or_default().parts.push(crate::parts::PartRef {number:"Card8".into(),..Default::default()});
    for i in 0..8 {set(&mut prj,100,i,"address",&format!("SourceCustom{i}")).unwrap();}
    assert_eq!(crate::parts::placed_functions(&prj,"-A1").len(),8);
    assert!(crate::parts::reserve_functions(&prj,None,"-A1").is_empty());
    assert_eq!(crate::parts::usage(&prj,None,"-A1")[&crate::parts::FnKind::PlcInput],(8,8));
    assert_eq!(crate::parts::placed_kinds(&prj)["-A1"].len(),8);
    assert!(crate::unplaced::plc_channels(&prj).is_empty());
    prj.pages[0].symbols[0].tag="-A2".into();
    assert_eq!(crate::parts::placed_functions(&prj,"-A2").len(),8);
}

#[test]
fn missing_module_channel_keeps_its_hardware_vacancy_despite_custom_addresses() {
    let mut prj=module();
    prj.symbols[0].plc_channels.retain(|c|c.index!=3);
    prj.plc_cards.insert("-A1".into(),PlcCard {device:"Card8".into(),..Default::default()});
    prj.plc.descriptions.push(crate::model::devdesc::DeviceDescription {id:"Card8".into(),inputs:8,..Default::default()});
    for i in 0..7 {set(&mut prj,100,i,"address",&format!("SourceCustom{i}")).unwrap();}
    let unplaced=crate::unplaced::plc_channels(&prj);
    assert_eq!(unplaced.len(),1);assert_eq!(unplaced[0].label,"I0.3");
}

#[test]
fn eight_channels_remain_independent_and_configure_from_the_card() {
    let mut prj=module();
    let points=super::super::points(&prj,&Netlist::build(&prj,true));
    assert_eq!(points.len(),8);
    assert!(points.iter().all(|p|p.address.is_empty() && p.symbolic.is_empty()));
    assert!(super::super::duplicate_addresses(&prj,&Netlist::build(&prj,true)).is_empty());
    assert_eq!(crate::plc_hw::used(&prj,"-A1"),(8,0));
    prj.plc_cards.insert("-A1".into(),PlcCard {start_byte:12,..Default::default()});
    assert_eq!(super::super::readdress(&mut prj,"-A1"),8);
    set(&mut prj,100,3,"symbol","MotorReady").unwrap();
    set(&mut prj,100,3,"comment","Drive ready").unwrap();
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    let points=super::super::points(&reopened,&Netlist::build(&reopened,true));
    assert_eq!(points[3].address,"I12.3");
    assert_eq!(points[3].symbolic,"MotorReady");
    assert_eq!(points[3].function_text,"Drive ready");
    assert_eq!(points[3].terminal,"4");
    assert_eq!(points[4].address,"I12.4");
    assert!(points[4].symbolic.is_empty());
    let table=crate::tables::rows(&reopened,&Netlist::build(&reopened,true),&Netlist::build(&reopened,false),crate::tables::TableKind::Plc);
    assert_eq!(table.len(),8);
    assert_eq!(table[3].key,"100:3");
}

#[test]
fn channel_table_edits_target_one_channel_and_respect_released_pages() {
    let mut prj=module();let nl=Netlist::build(&prj,false);
    crate::tables::set_cell(&mut prj,&nl,crate::tables::TableKind::Plc,"100:5",1,"I42.7").unwrap();
    assert_eq!(value(&prj.pages[0].symbols[0],"address",5),"I42.7");
    assert!(value(&prj.pages[0].symbols[0],"address",4).is_empty());
    prj.pages[0].locked=true;
    assert!(set(&mut prj,100,5,"address","I1.0").is_err());
    assert!(set(&mut prj,100,8,"address","I1.0").is_err());
    assert_eq!(value(&prj.pages[0].symbols[0],"address",5),"I42.7");
}

#[test]
fn plc_configuration_import_fills_names_without_baking_library_defaults() {
    let mut prj=module();
    prj.plc_cards.insert("-A1".into(),PlcCard {start_byte:12,..Default::default()});
    super::super::readdress(&mut prj,"-A1");
    assert_eq!(super::super::import_symbol_table(&mut prj,"Name;Data Type;Logical Address;Comment\nReady;Bool;%I12.3;Drive ready\n").unwrap(),1);
    let points=super::super::points(&prj,&Netlist::build(&prj,true));
    assert_eq!(points[3].symbolic,"Ready");
    assert_eq!(points[3].function_text,"Drive ready");
    assert!(points[4].symbolic.is_empty());
    assert!(prj.symbols[0].default_values.is_empty());
    assert!(super::super::symbol_table(&prj,&Netlist::build(&prj,true)).contains("Ready;Bool;%I12.3;-A1 Drive ready"));
}

#[test]
fn cabinet_representations_do_not_allocate_additional_io_channels() {
    let mut prj=module();
    let mut cabinet=prj.pages[0].clone();
    cabinet.kind=PageKind::Graphic;
    cabinet.id=101;
    cabinet.symbols[0].id=102;
    prj.pages.push(cabinet);
    assert_eq!(crate::plc_hw::used(&prj,"-A1"),(8,0));
    assert_eq!(super::super::points(&prj,&Netlist::build(&prj,true)).len(),8);
}

#[test]
fn overview_edits_address_name_and_comment_on_the_selected_channel_only() {
    let mut prj=module();
    let row=super::super::points(&prj,&Netlist::build(&prj,true))[3].clone();
    assert!(super::super::edit_point(&mut prj,&row,"I12.3","Ready","Drive ready").unwrap());
    assert_eq!(value(&prj.pages[0].symbols[0],"symbol",3),"Ready");
    assert_eq!(value(&prj.pages[0].symbols[0],"address",3),"I12.3");
    assert_eq!(value(&prj.pages[0].symbols[0],"comment",3),"Drive ready");
    assert!(value(&prj.pages[0].symbols[0],"symbol",4).is_empty());
    assert!(prj.pages[0].symbols[0].label.is_empty());
    assert!(prj.pages[0].symbols[0].function_text.is_empty());
    prj.pages[0].locked=true;
    assert!(super::super::edit_point(&mut prj,&row,"I99.0","Changed","Changed").is_err());
    assert_eq!(value(&prj.pages[0].symbols[0],"symbol",3),"Ready");
}

fn linked_module()->Project {
    let mut prj=module();
    let mut physical=prj.pages[0].clone();physical.id=200;physical.symbols[0].id=201;
    for (page,primary) in [(&mut prj.pages[0],false),(&mut physical,true)] {
        page.symbols[0].plc_bindings=(0..8).map(|i|(i,crate::model::plc_hw::ChannelBinding {group:format!("ch{i}"),primary,..Default::default()})).collect();
    }
    prj.pages.push(physical);prj
}

#[test]
fn address_scheme_changes_convert_all_linked_channels_and_preserve_custom_names() {
    use crate::model::plc_hw::{Scheme,Station};
    let mut prj=linked_module();
    prj.plc_cards.insert("-A1".into(),PlcCard {station:"CPU".into(),start_byte:12,..Default::default()});
    prj.plc.stations.push(Station {name:"CPU".into(),..Default::default()});
    assert_eq!(super::super::readdress(&mut prj,"-A1"),8);
    set(&mut prj,100,3,"symbol","Ready").unwrap();
    prj.plc.stations[0].scheme=Scheme::Iec;
    assert_eq!(crate::plc_hw::convert_addresses(&mut prj),8);
    for page in &prj.pages {for i in 0..8 {assert_eq!(value(&page.symbols[0],"address",i),format!("%IX12.{i}"));}}
    assert_eq!(value(&prj.pages[1].symbols[0],"symbol",3),"Ready");
    set(&mut prj,100,2,"address","SourceCustom").unwrap();
    prj.plc.stations[0].scheme=Scheme::Siemens;
    assert_eq!(crate::plc_hw::convert_addresses(&mut prj),7);
    assert_eq!(value(&prj.pages[1].symbols[0],"address",2),"SourceCustom");
}

#[test]
fn linked_views_allocate_one_logical_channel_and_share_configuration_edits() {
    let mut prj=linked_module();
    assert_eq!(super::super::representations(&prj,&Netlist::build(&prj,true)).len(),16);
    let points=super::super::points(&prj,&Netlist::build(&prj,true));
    assert_eq!(points.len(),8);assert!(points.iter().all(|p|p.id==201));
    assert_eq!(crate::plc_hw::used(&prj,"-A1"),(8,0));
    set(&mut prj,100,3,"symbol","Ready").unwrap();
    assert_eq!(value(&prj.pages[1].symbols[0],"symbol",3),"Ready");
    crate::properties::set(&mut prj,&crate::properties::Object::Function(201),"function.plc_address[4]","I12.3").unwrap();
    assert_eq!(value(&prj.pages[0].symbols[0],"address",3),"I12.3");
    assert!(super::super::duplicate_addresses(&prj,&Netlist::build(&prj,true)).is_empty());
    let reopened=Project::from_json(&prj.to_json()).unwrap();
    assert_eq!(super::super::points(&reopened,&Netlist::build(&reopened,true)).len(),8);
    assert_eq!(crate::properties::get(&reopened,&crate::properties::Object::Function(100),"function.plc_symbol[4]"),"Ready");
}

#[test]
fn linked_edits_are_atomic_across_released_views_and_new_device_tags_detach_copies() {
    let mut prj=linked_module();
    prj.pages[0].locked=true;
    assert!(set(&mut prj,201,3,"address","I99.0").is_err());
    assert!(value(&prj.pages[1].symbols[0],"address",3).is_empty());
    prj.pages[0].locked=false;
    let mut copy=prj.pages[1].symbols[0].clone();copy.id=301;copy.tag="-A2".into();prj.pages[1].symbols.push(copy);
    set(&mut prj,100,3,"symbol","Ready").unwrap();
    assert!(value(&prj.pages[1].symbols[1],"symbol",3).is_empty());
    assert_eq!(super::super::points(&prj,&Netlist::build(&prj,true)).len(),16);
}

#[test]
fn split_electrical_views_keep_card_channel_numbers_when_sheets_are_reordered() {
    for explicit_index in [false,true] {
        let mut prj=module();
        let point=UserSymbol {key:"user:point".into(),func:Func::Plc,
            plc_channels:vec![PlcChannelDef {key:"point".into(),input:Some(true),data_type:"Bool".into(),designation:"0".into(),pins:vec![0],signal_pin:Some(0),..Default::default()}],
            pins:vec![UserPin {pos:p(0.,0.),label:"1".into(),internal:false}],..Default::default()};
        prj.symbols.push(point);
        let mut physical=Page {id:200,name:"2".into(),..Default::default()};
        for i in 0..8 {
            let binding=|primary|crate::model::plc_hw::ChannelBinding {group:format!("ch{i}"),primary,index:explicit_index.then_some(i as u16)};
            prj.pages[0].symbols[0].plc_bindings.insert(i,binding(false));
            physical.symbols.push(SymbolInst {id:201+i as u64,sym:"user:point".into(),tag:"-A1".into(),
                pos:p(40.,20.+i as f32*4.),pins:vec![(i+1).to_string()],plc_bindings:[(0,binding(true))].into_iter().collect(),..Default::default()});
        }
        prj.pages.insert(0,physical);
        prj.plc_cards.insert("-A1".into(),PlcCard {start_byte:12,..Default::default()});
        assert_eq!(super::super::readdress(&mut prj,"-A1"),8);
        let reopened=Project::from_json(&prj.to_json()).unwrap();
        let points=super::super::points(&reopened,&Netlist::build(&reopened,true));
        assert_eq!(points.len(),8);
        for (i,point) in points.iter().enumerate() {
            assert_eq!(point.address,format!("I12.{i}"));
            assert_eq!(point.channel,i.to_string());
            assert_eq!(point.channel_index,Some(0));
            assert_eq!(point.id,201+i as u64);
        }
    }
}

#[test]
fn an_unstored_channel_number_comes_from_the_primary_view_of_equal_modules() {
    let mut prj=Project::default();
    for (key,index) in [("user:a",3),("user:b",5)] {
        let channel=PlcChannelDef {key:"point".into(),index,input:Some(true),data_type:"Bool".into(),designation:"0".into(),pins:vec![0],signal_pin:Some(0),..Default::default()};
        prj.symbols.push(UserSymbol {key:key.into(),func:Func::Plc,plc_channels:vec![channel],
            pins:vec![UserPin {pos:p(0.,0.),label:"1".into(),internal:false}],..Default::default()});
    }
    for (id,key,primary) in [(1,"user:a",false),(2,"user:b",true)] {
        let binding=crate::model::plc_hw::ChannelBinding {group:"ch".into(),primary,index:None};
        prj.pages[0].symbols.push(SymbolInst {id,sym:key.into(),tag:"-A1".into(),pos:p(id as f32*20.,0.),
            pins:vec!["1".into()],plc_bindings:[(0,binding)].into_iter().collect(),..Default::default()});
    }
    assert_eq!(crate::model::plc_hw::bindings::index(&prj,1,0),Some(5));
}
