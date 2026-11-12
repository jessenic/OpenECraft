//! Parts management in the application: re-exports the shared part model of the
//! `openecraft-parts` library and adds everything that needs a project (project
//! parts, device functions, function template usage).

use crate::model::Project;
use crate::symbols::{self, Func};
pub use openecraft_parts::*;
use std::collections::BTreeMap;
use std::sync::OnceLock;

/// The built-in catalog, built once (reports look up wiring material in it).
pub fn catalog() -> &'static [Part] {
    static CATALOG: OnceLock<Vec<Part>> = OnceLock::new();
    CATALOG.get_or_init(default_catalog)
}

/// Function kind represented by a library symbol (the symbols of the national sets
/// as the IEC symbols they are drawn like).
pub fn kind_of_symbol(key: &str) -> Option<FnKind> {
    let key = symbols::iec_equivalent(key).unwrap_or(key);
    Some(match key {
        "coil" | "coil_on" | "coil_off" | "nfpa_coil" | "nfpa_tr_coil" => FnKind::Coil,
        "valve" | "nfpa_sol" => FnKind::Valve,
        "no" | "nfpa_no" => FnKind::No,
        "nc" | "ol_nc" | "nfpa_nc" | "nfpa_ol_nc" => FnKind::Nc,
        "co" => FnKind::Changeover,
        "no_on" | "nfpa_notc" => FnKind::NoOnDelay,
        "nc_on" | "nfpa_ncto" => FnKind::NcOnDelay,
        "no_off" => FnKind::NoOffDelay,
        "nc_off" => FnKind::NcOffDelay,
        "main3" | "nfpa_main3" => FnKind::MainContacts3,
        "main4" => FnKind::MainContacts4,
        "main1" => FnKind::MainContact,
        "pb_no" | "nfpa_pb_no" => FnKind::PushbuttonNo,
        "pb_nc" | "estop" | "nfpa_pb_nc" => FnKind::PushbuttonNc,
        "sel" | "key_no" | "nfpa_ss" => FnKind::SwitchNo,
        "sel_nc" => FnKind::SwitchNc,
        "disc3" | "nfpa_disc3" => FnKind::Disconnector3,
        "prox" | "limit" | "pressure_no" | "level_no" | "nfpa_ls_no" => FnKind::Sensor,
        "limit_nc" | "temp_nc" | "nfpa_ls_nc" => FnKind::SensorNc,
        "transmitter" => FnKind::Transmitter,
        "ptc" => FnKind::Thermistor,
        "fuse" | "nfpa_fuse" => FnKind::Fuse,
        "fuse3" => FnKind::Fuse3,
        "cb1" => FnKind::Breaker1,
        "cb2" => FnKind::Breaker2,
        "cb3" | "nfpa_cb3" => FnKind::Breaker3,
        "cb4" => FnKind::Breaker4,
        "rcd4" => FnKind::ResidualCurrent,
        "spd" => FnKind::SurgeProtection,
        "mps3" => FnKind::MotorProtection,
        "ol3" | "nfpa_ol" => FnKind::Overload,
        "motor3" | "nfpa_motor" => FnKind::Motor3,
        "motor1" => FnKind::Motor1,
        "motor_dc" => FnKind::MotorDc,
        "brake" => FnKind::Brake,
        "vfd" => FnKind::FrequencyConverter,
        "softstarter" => FnKind::SoftStarter,
        "heater" => FnKind::Heater,
        "socket" => FnKind::SocketOutlet,
        "lamp" | "horn" | "nfpa_lamp" => FnKind::Signal,
        "ammeter" | "voltmeter" => FnKind::Meter,
        "psu" => FnKind::PowerSupply,
        "trafo" => FnKind::Transformer,
        "ct" => FnKind::CurrentTransformer,
        "battery" => FnKind::Battery,
        "pv" => FnKind::PvGenerator,
        "hybrid_inverter" | "mppt" | "inverter_charger" => FnKind::PowerSupply,
        "kwh3" | "kwh3_ct" => FnKind::Meter,
        "actuator" => FnKind::Actuator,
        "term" | "term_pe" | "term_n" | "term_disc" | "term_fuse" => FnKind::Terminal,
        "plug_pair" | "plug_m" | "plug_f" => FnKind::PlugPin,
        "plc_in" => FnKind::PlcInput,
        "plc_out" => FnKind::PlcOutput,
        "plc_ai" => FnKind::PlcAnalogInput,
        "plc_ao" => FnKind::PlcAnalogOutput,
        "fl_valve42" | "fl_valve52" => FnKind::DirectionalValve,
        "fl_cylinder" | "fl_cylinder_sa" => FnKind::Cylinder,
        "fl_pump" | "fl_compressor" | "pid_pump" => FnKind::Pump,
        "fl_filter" | "fl_gauge" | "fl_tank" | "fl_source" | "fl_silencer" | "fl_throttle" | "pid_vessel" | "pid_valve"
        | "pid_control_valve" | "pid_check_valve" | "pid_heat_exchanger" | "pid_motor_valve" | "pid_flow_meter" | "pid_connection" | "pid_heat_pump" | "pid_buffer"
        | "fl_shutoff" | "fl_regulator" | "fl_receiver" | "fl_dryer" | "fl_lubricator" | "fl_coupling" => FnKind::FluidComponent,
        "res" | "cap" | "diode" | "led" | "inductor" | "varistor" | "potentiometer" | "cap_pol" | "zener" | "photodiode" | "npn" | "pnp" | "nmos" | "thyristor" | "triac" | "ntc" | "ldr"
        | "optocoupler" | "bridge_rect" | "inst_switch" | "inst_switch2" | "inst_dimmer" | "inst_push" | "inst_lamp" | "inst_luminaire" | "inst_socket2" | "inst_junction" | "inst_motion"
        | "inst_board" | "earth_electrode" | "box1" | "box2" | "box4" => FnKind::Generic,
        _ => return symbols::iec_kind(key),
    })
}

// ---------------------------------------------------------------------- project

/// Copies a part from the database (or the built-in catalog) into the project parts
/// (if not there yet).
pub fn ensure_in_project(prj: &mut Project, db: &PartsDb, number: &str) {
    if prj.parts.iter().any(|p| p.number == number) {
        return;
    }
    if let Some(p) = db.get(number).or_else(|| catalog().iter().find(|p| p.number == number)) {
        prj.parts.push(p.clone());
        // Accessories come along so the BOM can resolve them later.
        for acc in p.accessories.clone() {
            if !prj.parts.iter().any(|x| x.number == acc) {
                if let Some(a) = db.get(&acc) {
                    prj.parts.push(a.clone());
                }
            }
        }
    }
}

/// Adds `count` units of a part to a device; the part is copied from the database
/// into the project parts.
pub fn add_to_device(prj: &mut Project, db: &PartsDb, tag: &str, number: &str, count: u32) {
    ensure_in_project(prj, db, number);
    let dev = prj.devices.entry(tag.to_string()).or_default();
    match dev.parts.iter_mut().find(|r| r.number == number && !r.total) {
        Some(r) => r.count += count,
        None => dev.parts.push(PartRef { number: number.into(), count, total:false }),
    }
}

/// Keep the primary assignment and its function bindings with a changed physical part.
pub fn replace_device_part(prj:&mut Project,tag:&str,old:&str,new:&str) {
    if tag.is_empty() || old.is_empty() || old==new {return;}
    let Some(device)=prj.devices.get_mut(tag) else {return;};
    let mut changed=false;
    for part in device.parts.iter_mut().filter(|p|p.number==old && !p.total) {part.number=new.into();changed=true;}
    if !changed {return;}
    for symbol in prj.pages.iter_mut().flat_map(|p|&mut p.symbols).filter(|s|s.tag==tag) {
        if symbol.values.get("function.part_number").is_some_and(|n|n==old) {symbol.values.insert("function.part_number".into(),new.into());}
    }
}

pub fn excluded_from_bom(prj:&Project,tag:&str)->bool {
    prj.devices.get(tag).and_then(|d|d.values.get("device.exclude_from_bom")).is_some_and(|v|v.eq_ignore_ascii_case("true"))
}

pub fn is_material_device(data:&crate::model::DeviceData)->bool {
    data.parts.iter().any(|p|p.total)
        || data.values.get("device.material_owner").is_some_and(|v|!v.is_empty())
        || data.values.get("device.exclude_from_bom").is_some_and(|v|v.eq_ignore_ascii_case("true"))
}

/// Every channel of a physical card shares its one BOM entry.
pub fn assign_placed_part(prj: &mut Project, db: &PartsDb, sym: &str, tag: &str, number: &str) {
    if prj.symbol(sym).is_some_and(|d| d.plc_channel.is_some())
        && prj.devices.get(tag).is_some_and(|d| d.parts.iter().any(|r| r.number == number)) {
        return;
    }
    add_to_device(prj, db, tag, number, 1);
}

/// Assigns a picked part and retains the placed function's explicit selection.
pub fn assign_placed_function_part(prj: &mut Project, db: &PartsDb, page: usize, id: u64, number: &str) -> bool {
    let Some(s) = prj.pages.get(page).filter(|p| p.is_editable()).and_then(|p| p.symbols.iter().find(|s| s.id == id && !s.tag.is_empty())) else { return false };
    let (key, tag) = (s.sym.clone(), s.tag.clone());
    assign_placed_part(prj, db, &key, &tag, number);
    let s = prj.pages[page].symbols.iter_mut().find(|s| s.id == id).unwrap();
    s.values.insert("function.part_number".into(), number.into());
    true
}

/// A channel can be placed directly from the symbol library. Recover its physical
/// article only if one catalog part owns all the imported PLC variants on the tag.
pub fn link_unassigned_plc_parts(prj: &mut Project, db: &PartsDb, only_tag: Option<&str>) {
    use std::collections::BTreeSet;
    let mut channels: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for s in prj.pages.iter().flat_map(|p| &p.symbols) {
        if s.tag.is_empty()
            || only_tag.is_some_and(|tag| tag != s.tag)
            || !s.sym.starts_with("user:ema:")
            || prj.devices.get(&s.tag).is_some_and(|d| !d.parts.is_empty())
        {
            continue;
        }
        if prj.symbol(&s.sym).is_some_and(|d| d.plc_channel.is_some()) {
            channels.entry(&s.tag).or_default().insert(s.library_key());
        }
    }
    if channels.is_empty() {
        return;
    }
    let mut owners: BTreeMap<&str, BTreeSet<&str>> = channels.values().flatten().map(|key| (*key, BTreeSet::new())).collect();
    for p in prj.parts.iter().chain(&db.parts) {
        for variant in &p.eplan_variants {
            if let Some(set) = variant["symbol"]["key"].as_str().and_then(|key| owners.get_mut(key)) {
                set.insert(&p.number);
            }
        }
    }
    let mut links = Vec::new();
    for (tag, keys) in channels {
        let mut keys = keys.into_iter();
        let Some(parts) = keys.next().and_then(|k| owners.get(k)) else { continue };
        let mut candidates = parts.clone();
        for key in keys {
            candidates.retain(|p| owners.get(key).is_some_and(|set| set.contains(p)));
        }
        if let Some(number) = candidates.first().filter(|_| candidates.len() == 1) {
            links.push((tag.to_owned(), (*number).to_owned()));
        }
    }
    for (tag, number) in links {
        add_to_device(prj, db, &tag, &number, 1);
    }
}

/// Resolves a part: project parts first, then the database.
pub fn lookup<'a>(prj: &'a Project, db: Option<&'a PartsDb>, number: &str) -> Option<&'a Part> {
    prj.parts.iter().find(|p| p.number == number).or_else(|| db.and_then(|d| d.get(number)))
}

/// Group choices with project entries taking precedence over the supplied database,
/// or the built-in catalog when no database is supplied.
pub fn group_choices<'a>(project_parts: &'a [Part], db: Option<&'a PartsDb>, group: &str) -> Vec<&'a Part> {
    let fallback=match db {Some(db)=>db.parts.as_slice(),None=>catalog()};
    let mut choices=BTreeMap::new();
    for part in project_parts.iter().chain(fallback.iter()) {
        choices.entry(part.number.as_str()).or_insert(part);
    }
    choices.into_values().filter(|p|p.group==group).collect()
}

/// Updates all project parts from the database. Returns the number of updated parts.
pub fn sync_project_from_db(prj: &mut Project, db: &PartsDb) -> usize {
    let mut n = 0;
    // Make sure every referenced part is present.
    let refs: Vec<String> =
        prj.devices.values().flat_map(|d| d.parts.iter().map(|r| r.number.clone())).collect();
    for r in refs {
        ensure_in_project(prj, db, &r);
    }
    for p in prj.parts.iter_mut() {
        if let Some(src) = db.get(&p.number) {
            if p != src {
                *p = src.clone();
                n += 1;
            }
        }
    }
    n
}

/// Adds project parts that are unknown in the database. Returns the number added.
pub fn transfer_project_to_db(prj: &Project, db: &mut PartsDb) -> usize {
    let mut n = 0;
    for p in &prj.parts {
        if db.get(&p.number).is_none() {
            db.parts.push(p.clone());
            n += 1;
        }
    }
    n
}

/// Removes project parts that no device uses any more.
pub fn purge_unused(prj: &mut Project) -> usize {
    let mut used: Vec<String> = prj.devices.values().flat_map(|d| d.parts.iter().map(|r| r.number.clone())).collect();
    let acc: Vec<String> = prj
        .parts
        .iter()
        .filter(|p| used.contains(&p.number))
        .flat_map(|p| p.accessories.clone())
        .collect();
    used.extend(acc);
    let before = prj.parts.len();
    prj.parts.retain(|p| used.contains(&p.number));
    before - prj.parts.len()
}

// ---------------------------------------------------------------------- accessories

/// A part from the project, the database or the built-in catalog.
fn find_part<'a>(prj: &'a Project, db: Option<&'a PartsDb>, number: &str) -> Option<&'a Part> {
    lookup(prj, db, number).or_else(|| catalog().iter().find(|p| p.number == number))
}

/// The parts a device uses, resolved (unknown numbers are left out).
pub fn device_parts<'a>(prj: &'a Project, db: Option<&'a PartsDb>, tag: &str) -> Vec<&'a Part> {
    prj.devices.get(tag).map(|d| d.parts.iter().filter_map(|r| find_part(prj, db, &r.number)).collect()).unwrap_or_default()
}

/// The part of the device that accessory `number` is attached to: the first of its
/// other parts it fits.
pub fn attached_to<'a>(prj: &'a Project, db: Option<&'a PartsDb>, tag: &str, number: &str) -> Option<&'a Part> {
    let acc = find_part(prj, db, number)?;
    device_parts(prj, db, tag).into_iter().find(|main| accessory::fits(acc, main))
}

/// Accessories of a device's parts that can still be added: from the database, the
/// project parts and the built-in catalog, each with the part it fits, in the order of
/// the device's parts.
pub fn accessory_choices(prj: &Project, db: Option<&PartsDb>, tag: &str) -> Vec<(Part, String)> {
    let mains = device_parts(prj, db, tag);
    let pool = prj.parts.iter().chain(db.map(|d| d.parts.as_slice()).unwrap_or_default()).chain(catalog());
    let pool: Vec<&Part> = pool.collect();
    let mut out: Vec<(Part, String)> = Vec::new();
    for main in &mains {
        for acc in accessory::accessories_of(pool.iter().copied(), main) {
            if !out.iter().any(|(p, _)| p.number == acc.number) && !mains.iter().any(|m| m.number == acc.number) {
                out.push((acc.clone(), main.number.clone()));
            }
        }
    }
    out
}

/// Adds `count` units of accessory `number` to a device; it must fit one of the
/// device's parts. Returns the part it is attached to.
pub fn add_accessory(prj: &mut Project, db: &PartsDb, tag: &str, number: &str, count: u32) -> Result<String, String> {
    if !prj.devices.contains_key(tag) {
        return Err(openecraft_i18n::tr_fmt!("Unknown device: {}", tag));
    }
    if find_part(prj, Some(db), number).is_none() {
        return Err(openecraft_i18n::tr_fmt!("Unknown part: {}", number));
    }
    let main = match attached_to(prj, Some(db), tag, number) {
        Some(m) => m.number.clone(),
        None => return Err(openecraft_i18n::tr_fmt!("{} fits none of the parts of {}", number, tag)),
    };
    if !prj.parts.iter().any(|p| p.number == number) {
        if let Some(p) = find_part(prj, Some(db), number).cloned() {
            prj.parts.push(p);
        }
    }
    add_to_device(prj, db, tag, number, count);
    Ok(main)
}

/// Accessories of devices that fit none of the device's other parts: (device tag,
/// accessory part number).
pub fn loose_accessories(prj: &Project) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (tag, data) in &prj.devices {
        for r in &data.parts {
            let Some(acc) = prj.parts.iter().find(|p| p.number == r.number && p.accessory) else { continue };
            if attached_to(prj, None, tag, &acc.number).is_none() {
                out.push((tag.clone(), acc.number.clone()));
            }
        }
    }
    out.sort_by(|a, b| crate::model::natural_cmp(&a.0, &b.0).then(a.1.cmp(&b.1)));
    out
}

// ---------------------------------------------------------------------- devices

/// Functions placed for a device: (kind, connection designations, symbol id).
pub fn placed_functions(prj: &Project, tag: &str) -> Vec<(FnKind, Vec<String>, u64)> {
    all_placed_functions(prj,Some(tag)).into_iter().map(|(_,k,pins,id)|(k,pins,id)).collect()
}

fn all_placed_functions<'a>(prj:&'a Project,tag:Option<&str>)->Vec<(&'a str,FnKind,Vec<String>,u64)> {
    let mut v=Vec::new();
    for pg in &prj.pages {
        for s in pg.symbols.iter().filter(|s|tag.is_none_or(|tag|s.tag==tag)) {
            if prj.symbol(&s.sym).is_some_and(|d|!d.plc_channels.is_empty()) {continue;}
            if let Some(k) = placed_kind(prj, &s.sym) {
                v.push((s.tag.as_str(),k, s.pins.clone(), s.id));
            }
        }
    }
    for (pi,si,index) in crate::model::plc_hw::bindings::placed_for(prj,tag) {
        let s=&prj.pages[pi].symbols[si];let Some(def)=prj.symbol(&s.sym) else {continue;};let c=&def.plc_channels[index];
        let kind=match (c.input,c.analog) {
            (Some(true),false)=>FnKind::PlcInput,(Some(false),false)=>FnKind::PlcOutput,
            (Some(true),true)=>FnKind::PlcAnalogInput,(Some(false),true)=>FnKind::PlcAnalogOutput,
            (None,_)=>FnKind::Generic,
        };
        let pins=c.pins.iter().filter_map(|i|s.pins.get(*i).cloned().or_else(||def.pins.get(*i).map(|p|p.label.to_owned()))).collect();
        v.push((s.tag.as_str(),kind,pins,s.id));
    }
    v
}

/// Functions available from the parts of a device (templates × count).
pub fn available_functions(prj: &Project, db: Option<&PartsDb>, tag: &str) -> Vec<FunctionTemplate> {
    let Some(dev) = prj.devices.get(tag) else { return Vec::new() };
    let mut v = Vec::new();
    for r in &dev.parts {
        if let Some(p) = lookup(prj, db, &r.number) {
            for _ in 0..r.quantity() {
                v.extend(p.functions.iter().cloned());
            }
        }
    }
    v
}

/// Template functions not used by any placed function (shown as reserve contacts).
pub fn reserve_functions(prj: &Project, db: Option<&PartsDb>, tag: &str) -> Vec<FunctionTemplate> {
    let mut avail = available_functions(prj, db, tag);
    let placed = placed_functions(prj, tag);
    // First match by kind and designations, then by kind only.
    let mut rest = Vec::new();
    for (k, pins, _) in &placed {
        if let Some(i) = avail.iter().position(|f| f.kind == *k && &f.pin_list() == pins) {
            avail.remove(i);
        } else {
            rest.push(*k);
        }
    }
    for k in rest {
        if let Some(i) = avail.iter().position(|f| f.kind == k) {
            avail.remove(i);
        }
    }
    avail
}

/// Per function kind: (placed, available) for devices that have parts assigned.
pub fn usage(prj: &Project, db: Option<&PartsDb>, tag: &str) -> BTreeMap<FnKind, (usize, usize)> {
    usage_with(prj, db, tag, &placed_functions(prj, tag))
}

/// The kinds of the placed functions of every device (one pass over the project).
pub fn placed_kinds(prj: &Project) -> std::collections::HashMap<&str, Vec<FnKind>> {
    let mut m: std::collections::HashMap<&str, Vec<FnKind>> = std::collections::HashMap::new();
    for (tag,k,_,_) in all_placed_functions(prj,None) {
        m.entry(tag).or_default().push(k);
    }
    m
}

fn placed_kind(prj: &Project, key: &str) -> Option<FnKind> {
    kind_of_symbol(key).or_else(|| {
        let def = prj.symbol(key)?;
        if let Some(c) = def.plc_channel.as_ref().filter(|c| c.analog.is_some()) {
            return Some(if c.input { FnKind::PlcAnalogInput } else { FnKind::PlcAnalogOutput });
        }
        let input = def.digital_input()?;
        Some(if input { FnKind::PlcInput } else { FnKind::PlcOutput })
    })
}

/// [`usage`] with the kinds of the placed functions given (see [`placed_kinds`]).
pub fn usage_of(prj: &Project, db: Option<&PartsDb>, tag: &str, placed: &[FnKind]) -> BTreeMap<FnKind, (usize, usize)> {
    let mut m: BTreeMap<FnKind, (usize, usize)> = BTreeMap::new();
    for k in placed {
        m.entry(*k).or_default().0 += 1;
    }
    for f in available_functions(prj, db, tag) {
        m.entry(f.kind).or_default().1 += 1;
    }
    m
}

fn usage_with(prj: &Project, db: Option<&PartsDb>, tag: &str, placed: &[(FnKind, Vec<String>, u64)]) -> BTreeMap<FnKind, (usize, usize)> {
    let mut m: BTreeMap<FnKind, (usize, usize)> = BTreeMap::new();
    for (k, _, _) in placed {
        m.entry(*k).or_default().0 += 1;
    }
    for f in available_functions(prj, db, tag) {
        m.entry(f.kind).or_default().1 += 1;
    }
    m
}

/// Assigns connection designations from the function templates to the placed
/// functions of a device (in page / position order). Returns how many changed.
pub fn apply_designations(prj: &mut Project, db: Option<&PartsDb>, tag: &str) -> usize {
    let mut avail = available_functions(prj, db, tag);
    let mut placed: Vec<(usize, usize)> = Vec::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        for (si, s) in pg.symbols.iter().enumerate() {
            if s.tag == tag {
                placed.push((pi, si));
            }
        }
    }
    placed.sort_by(|a, b| {
        let (sa, sb) = (&prj.pages[a.0].symbols[a.1], &prj.pages[b.0].symbols[b.1]);
        a.0.cmp(&b.0).then(sa.pos.x.total_cmp(&sb.pos.x)).then(sa.pos.y.total_cmp(&sb.pos.y))
    });
    let mut changed = 0;
    let symbols = prj.symbol_table();
    for (pi, si) in placed {
        let s = &mut prj.pages[pi].symbols[si];
        let Some(kind) = kind_of_symbol(&s.sym) else { continue };
        let n_pins = symbols.get(&s.sym).map(|d| d.pins.len()).unwrap_or(0);
        if let Some(i) = avail.iter().position(|f| f.kind == kind && f.pin_list().len() == n_pins) {
            let pins = avail.remove(i).pin_list();
            if s.pins != pins {
                s.pins = pins;
                changed += 1;
            }
        }
    }
    changed
}

/// Device selection: does the part fit a device with these placed functions?
/// The part must provide the main function (or, without one, any placed function).
pub fn fits_device(part: &Part, placed: &[(FnKind, Vec<String>, u64)], main: Option<FnKind>) -> bool {
    if part.functions.is_empty() {
        return false;
    }
    match main {
        Some(m) => part.functions.iter().any(|f| f.kind == m),
        None => placed.iter().any(|(k, _, _)| part.functions.iter().any(|f| f.kind == *k)),
    }
}

/// Main function kind of a device, if one is placed.
pub fn main_kind(prj: &Project, tag: &str) -> Option<FnKind> {
    prj.pages
        .iter()
        .flat_map(|pg| pg.symbols.iter())
        .filter(|s| s.tag == tag)
        .find(|s| prj.symbol(&s.sym).map(|d| matches!(d.func, Func::Main { .. })).unwrap_or(false))
        .and_then(|s| kind_of_symbol(&s.sym))
}

/// The components of an assembly for `qty` assemblies, nested assemblies resolved
/// (quantities multiplied), in the order of first use.
pub fn assembly_components(prj: &Project, number: &str, qty: f32) -> Vec<(String, f32)> {
    fn walk(prj: &Project, number: &str, qty: f32, depth: usize, path: &mut Vec<String>, out: &mut Vec<(String, f32)>) {
        if depth > 8 || path.iter().any(|p| p == number) {
            return;
        }
        let Some(part) = prj.parts.iter().chain(catalog()).find(|p| p.number == number) else { return };
        path.push(number.to_string());
        for c in &part.components {
            if path.contains(&c.number) {
                continue;
            }
            let n = qty * c.quantity;
            match out.iter_mut().find(|x| x.0 == c.number) {
                Some(x) => x.1 += n,
                None => out.push((c.number.clone(), n)),
            }
            walk(prj, &c.number, n, depth + 1, path, out);
        }
        path.pop();
    }
    let mut out = Vec::new();
    walk(prj, number, qty, 0, &mut Vec::new(), &mut out);
    out
}

#[cfg(test)]
mod tests;
