use super::*;

#[test]
fn analog_and_mixed_function_templates_define_all_channels_and_bytes() {
    let mut part=Part {number:"AI4".into(),functions:(0..4).map(|_|crate::parts::FunctionTemplate {kind:FnKind::PlcAnalogInput,..Default::default()}).collect(),..Default::default()};
    let desc=from_part(&part).unwrap();
    assert_eq!((desc.kind,desc.inputs,desc.outputs,desc.input_bytes),(ModuleKind::AnalogInput,4,0,8));
    part.functions.extend((0..9).map(|_|crate::parts::FunctionTemplate {kind:FnKind::PlcInput,..Default::default()}));
    part.functions.push(crate::parts::FunctionTemplate {kind:FnKind::PlcAnalogOutput,..Default::default()});
    let desc=from_part(&part).unwrap();
    assert_eq!((desc.kind,desc.inputs,desc.outputs,desc.input_bytes,desc.output_bytes),(ModuleKind::Other,13,1,10,2));
    part.tech_data="Input bytes: 24\nOutput bytes: 4".into();
    let desc=from_part(&part).unwrap();assert_eq!((desc.input_bytes,desc.output_bytes),(24,4));
}

#[test]
fn parts_of_the_catalog_give_descriptions() {
    let di = crate::parts::catalog().iter().find(|p| p.number == "GEN.PLC-DI16").unwrap();
    let d = from_part(di).unwrap();
    assert_eq!((d.kind, d.inputs, d.outputs, d.input_bytes, d.output_bytes), (ModuleKind::DigitalInput, 16, 0, 2, 0));
    let relay = crate::parts::catalog().iter().find(|p| p.group != "PLC" && p.functions.iter().all(|f| f.kind != FnKind::PlcInput)).unwrap();
    assert!(from_part(relay).is_none());

    let cpu = Part {
        number: "ACME.CPU1".into(),
        group: "PLC".into(),
        subgroup: "CPU".into(),
        tech_data: "Bus: EtherCAT\nPorts: X1, X2, X3\nSlots: 16".into(),
        ..Default::default()
    };
    let d = from_part(&cpu).unwrap();
    assert_eq!((d.kind, d.slots), (ModuleKind::Cpu, 16));
    assert_eq!(d.ports.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["X1", "X2", "X3"]);
    assert_eq!(d.bus(), Some(BusKind::EtherCat));
}

#[test]
fn bus_kinds_and_module_kinds_parse() {
    assert_eq!(BusKind::parse("PROFINET"), Some(BusKind::Profinet));
    assert_eq!(BusKind::parse("EtherNet/IP"), Some(BusKind::EthernetIp));
    assert_eq!(BusKind::parse("ethercat"), Some(BusKind::EtherCat));
    assert_eq!(BusKind::parse("x"), None);
    assert_eq!(ModuleKind::from_text("Digital input / output module"), Some(ModuleKind::DigitalMixed));
    assert_eq!(ModuleKind::from_text("Analog input module"), Some(ModuleKind::AnalogInput));
    assert_eq!(ModuleKind::from_text("Bus coupler"), Some(ModuleKind::Coupler));
    assert!(builtin().iter().any(|d| d.id == "GEN.CPU-PN" && d.ports.len() == 2));
}

#[test]
fn gsdml_files_are_imported() {
    let gsd = r#"<?xml version="1.0" encoding="iso-8859-1"?>
<ISO15745Profile xmlns="http://www.profibus.com/GSDML/2003/11/DeviceProfile">
 <ProfileBody>
  <DeviceIdentity VendorID="0x002A" DeviceID="0x0313"><VendorName Value="ACME" /></DeviceIdentity>
  <ApplicationProcess>
   <DeviceAccessPointList>
    <DeviceAccessPointItem ID="DAP1" PhysicalSlots="0..12" ModuleIdentNumber="0x1">
     <ModuleInfo><Name TextId="T_DAP" /><OrderNumber Value="ACME-IM 155" /></ModuleInfo>
     <SystemDefinedSubmoduleList>
      <InterfaceSubmoduleItem ID="IF" />
      <PortSubmoduleItem ID="P1" /><PortSubmoduleItem ID="P2" />
     </SystemDefinedSubmoduleList>
    </DeviceAccessPointItem>
   </DeviceAccessPointList>
   <ModuleList>
    <ModuleItem ID="DI16" ModuleIdentNumber="0x10">
     <ModuleInfo><Name TextId="T_DI" /><OrderNumber Value="ACME-DI16" /></ModuleInfo>
     <VirtualSubmoduleList><VirtualSubmoduleItem ID="S"><IOData><Input><DataItem DataType="Unsigned16" TextId="x" /></Input></IOData></VirtualSubmoduleItem></VirtualSubmoduleList>
    </ModuleItem>
    <ModuleItem ID="AI4">
     <ModuleInfo><Name TextId="T_AI" /><OrderNumber Value="ACME-AI4" /></ModuleInfo>
     <VirtualSubmoduleList><VirtualSubmoduleItem ID="S"><IOData><Input><DataItem DataType="OctetString" Length="8" TextId="x" /></Input></IOData></VirtualSubmoduleItem></VirtualSubmoduleList>
    </ModuleItem>
   </ModuleList>
   <ExternalTextList><PrimaryLanguage>
    <Text TextId="T_DAP" Value="IM 155 interface module" />
    <Text TextId="T_DI" Value="DI 16x24VDC" />
    <Text TextId="T_AI" Value="AI 4xU/I" />
   </PrimaryLanguage></ExternalTextList>
  </ApplicationProcess>
 </ProfileBody>
</ISO15745Profile>"#;
    let v = import(gsd, "GSDML-V2.4-ACME.xml").unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!((v[0].kind, v[0].slots, v[0].ports.len(), v[0].order_number.as_str()), (ModuleKind::Coupler, 12, 2, "ACME-IM 155"));
    assert_eq!(v[0].name, "IM 155 interface module");
    assert_eq!((v[1].kind, v[1].inputs, v[1].input_bytes), (ModuleKind::DigitalInput, 16, 2));
    assert_eq!((v[2].kind, v[2].inputs, v[2].input_bytes), (ModuleKind::AnalogInput, 4, 8));
    assert!(v.iter().all(|d| d.manufacturer == "ACME" && d.source == "GSDML-V2.4-ACME.xml"));
}

#[test]
fn esi_files_are_imported() {
    let esi = r##"<EtherCATInfo>
 <Vendor><Id>2</Id><Name>ACME</Name></Vendor>
 <Descriptions><Devices>
  <Device Physics="YY"><Type ProductCode="#x1" RevisionNo="#x1">EK1100</Type><Name LcId="1033">EK1100 EtherCAT Coupler</Name></Device>
  <Device Physics="YY"><Type>EL1008</Type><Name>EL1008 8Ch. Dig. Input</Name>
   <TxPdo><Index>#x1a00</Index><Entry><Index>#x6000</Index><SubIndex>1</SubIndex><BitLen>1</BitLen></Entry></TxPdo>
   <TxPdo><Index>#x1a01</Index><Entry><Index>#x6010</Index><SubIndex>1</SubIndex><BitLen>1</BitLen></Entry></TxPdo>
  </Device>
  <Device><Type>EL4002</Type><Name>EL4002 2Ch. Ana. Output</Name>
   <RxPdo><Index>#x1600</Index><Entry><Index>#x7000</Index><SubIndex>17</SubIndex><BitLen>16</BitLen></Entry><Entry><Index>#x7010</Index><SubIndex>17</SubIndex><BitLen>16</BitLen></Entry></RxPdo>
  </Device>
 </Devices></Descriptions>
</EtherCATInfo>"##;
    let v = import(esi, "acme.xml").unwrap();
    assert_eq!(v.len(), 3);
    assert_eq!((v[0].kind, v[0].ports.len()), (ModuleKind::Coupler, 2));
    assert_eq!(v[0].ports[0].bus, BusKind::EtherCat);
    assert_eq!((v[1].kind, v[1].inputs, v[1].input_bytes), (ModuleKind::DigitalInput, 2, 1));
    assert_eq!((v[2].kind, v[2].outputs, v[2].output_bytes), (ModuleKind::AnalogOutput, 2, 4));
    assert!(import("<Other/>", "x.xml").is_err());
}
