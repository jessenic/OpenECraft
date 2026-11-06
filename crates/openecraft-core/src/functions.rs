//! Function definitions: what a placed symbol represents (coil, NO contact, motor,
//! PLC input …). A definition gives the identifying letter, the default symbol and
//! connection point designations, default property values, the placement scheme
//! and which property groups apply. Built-in definitions cover the symbol library;
//! projects add their own reusable ones (also through property library files).

use openecraft_i18n::{tr, tr_const, tr_fmt};
use crate::model::*;
use crate::parts::FnKind;
use crate::properties::Group;
use crate::symbols::{self, Func};
use std::sync::OnceLock;
pub use crate::model::functions::{FnCategory, FunctionDef};

impl FnCategory {
    pub const ALL: [FnCategory; 20] = [
        FnCategory::Relays,
        FnCategory::Contacts,
        FnCategory::Switches,
        FnCategory::Sensors,
        FnCategory::Protection,
        FnCategory::Motors,
        FnCategory::Drives,
        FnCategory::Signal,
        FnCategory::Measuring,
        FnCategory::PowerSupply,
        FnCategory::Installation,
        FnCategory::Terminals,
        FnCategory::Plugs,
        FnCategory::Plc,
        FnCategory::Passive,
        FnCategory::BlackBox,
        FnCategory::Potentials,
        FnCategory::Fluid,
        FnCategory::Process,
        FnCategory::General,
    ];

    pub fn title(self) -> &'static str {
        match self {
            FnCategory::Relays => tr("Relays, contactors"),
            FnCategory::Contacts => tr("Contacts"),
            FnCategory::Switches => tr("Switches, pushbuttons"),
            FnCategory::Sensors => tr("Sensors"),
            FnCategory::Protection => tr("Protective devices"),
            FnCategory::Motors => tr("Motors"),
            FnCategory::Drives => tr("Drives"),
            FnCategory::Signal => tr("Signal devices"),
            FnCategory::Measuring => tr("Measuring instruments"),
            FnCategory::PowerSupply => tr("Power supplies, transformers"),
            FnCategory::Installation => tr("Installation devices"),
            FnCategory::Terminals => tr("Terminals"),
            FnCategory::Plugs => tr("Plugs"),
            FnCategory::Plc => tr("PLC"),
            FnCategory::Passive => tr("Passive components"),
            FnCategory::BlackBox => tr("Black boxes"),
            FnCategory::Potentials => tr("Potentials, interruption points"),
            FnCategory::Fluid => tr("Fluid power"),
            FnCategory::Process => tr("Process engineering"),
            FnCategory::General => tr("General"),
        }
    }

    /// Property groups that apply besides the common ones ([`Group::is_common`]).
    pub fn groups(self) -> &'static [Group] {
        use Group::*;
        match self {
            FnCategory::Relays => &[Electrical, Control, Contacts, Safety],
            FnCategory::Contacts => &[Electrical, Contacts],
            FnCategory::Switches => &[Electrical, Contacts, Signal, Safety],
            FnCategory::Sensors => &[Electrical, Sensor, Communication, Safety, Process],
            FnCategory::Protection => &[Electrical, Protection, Contacts],
            FnCategory::Motors => &[Electrical, Motor],
            FnCategory::Drives => &[Electrical, Drive, Communication, Safety],
            FnCategory::Signal => &[Electrical, Signal],
            FnCategory::Measuring => &[Electrical, Sensor, PowerSupply],
            FnCategory::PowerSupply => &[Electrical, PowerSupply],
            FnCategory::Installation | FnCategory::Terminals | FnCategory::Plugs | FnCategory::Passive | FnCategory::Potentials => &[Electrical],
            FnCategory::Plc => &[Electrical, Plc, Communication, Safety],
            FnCategory::Fluid => &[Electrical, Control, Fluid],
            FnCategory::Process => &[Process, Fluid, Sensor],
            FnCategory::BlackBox | FnCategory::General => &Group::ALL,
        }
    }
}

impl FunctionDef {
    pub fn title(&self) -> String {
        match self.builtin.then(|| ROWS.iter().find(|r| r.0 == self.key)).flatten() {
            Some(r) => tr(r.1).to_string(),
            None => self.name.clone(),
        }
    }

    pub fn pin_list(&self) -> Vec<String> {
        self.pins.split_whitespace().map(String::from).collect()
    }
}

type Row = (&'static str, &'static str, FnCategory, FnKind, &'static str, &'static [&'static str]);

/// Key, name, category, kind, letter and the symbols the definition stands for
/// (the first is its default symbol).
const ROWS: &[Row] = {
    use FnCategory as C;
    use FnKind as K;
    &[
        ("fn.coil", tr_const!("Relay / contactor coil"), C::Relays, K::Coil, "K", &["coil", "nfpa_coil", "sl_contactor"]),
        ("fn.timer_on", tr_const!("Timer relay coil, on-delay"), C::Relays, K::Coil, "K", &["coil_on"]),
        ("fn.timer_off", tr_const!("Timer relay coil, off-delay"), C::Relays, K::Coil, "K", &["coil_off"]),
        ("fn.safety_relay", tr_const!("Safety relay"), C::Relays, K::Generic, "K", &[]),
        ("fn.no", tr_const!("NO contact"), C::Contacts, K::No, "K", &["no", "nfpa_no"]),
        ("fn.nc", tr_const!("NC contact"), C::Contacts, K::Nc, "K", &["nc", "nfpa_nc"]),
        ("fn.co", tr_const!("Changeover contact"), C::Contacts, K::Changeover, "K", &["co"]),
        ("fn.no_on", tr_const!("NO contact, on-delay"), C::Contacts, K::NoOnDelay, "K", &["no_on", "nfpa_notc"]),
        ("fn.nc_on", tr_const!("NC contact, on-delay"), C::Contacts, K::NcOnDelay, "K", &["nc_on", "nfpa_ncto"]),
        ("fn.no_off", tr_const!("NO contact, off-delay"), C::Contacts, K::NoOffDelay, "K", &["no_off"]),
        ("fn.nc_off", tr_const!("NC contact, off-delay"), C::Contacts, K::NcOffDelay, "K", &["nc_off"]),
        ("fn.main3", tr_const!("Main contacts 3-pole"), C::Contacts, K::MainContacts3, "K", &["main3"]),
        ("fn.main4", tr_const!("Main contacts 4-pole"), C::Contacts, K::MainContacts4, "K", &["main4"]),
        ("fn.main1", tr_const!("Main contact 1-pole"), C::Contacts, K::MainContact, "K", &["main1"]),
        ("fn.overload_aux", tr_const!("Overload auxiliary contact"), C::Contacts, K::Nc, "F", &["ol_nc"]),
        ("fn.pb_no", tr_const!("Pushbutton NO"), C::Switches, K::PushbuttonNo, "S", &["pb_no", "nfpa_pb_no"]),
        ("fn.pb_nc", tr_const!("Pushbutton NC"), C::Switches, K::PushbuttonNc, "S", &["pb_nc", "nfpa_pb_nc"]),
        ("fn.estop", tr_const!("Emergency stop"), C::Switches, K::PushbuttonNc, "S", &["estop"]),
        ("fn.selector", tr_const!("Selector switch NO"), C::Switches, K::SwitchNo, "S", &["sel", "nfpa_ss"]),
        ("fn.selector_nc", tr_const!("Selector switch NC"), C::Switches, K::SwitchNc, "S", &["sel_nc"]),
        ("fn.key_switch", tr_const!("Key switch NO"), C::Switches, K::SwitchNo, "S", &["key_no"]),
        ("fn.main_switch", tr_const!("Main switch 3-pole"), C::Switches, K::Disconnector3, "Q", &["disc3", "nfpa_disc3"]),
        ("fn.proximity", tr_const!("Proximity sensor"), C::Sensors, K::Sensor, "B", &["prox"]),
        ("fn.position", tr_const!("Position switch NO"), C::Sensors, K::Sensor, "B", &["limit", "nfpa_ls_no"]),
        ("fn.position_nc", tr_const!("Position switch NC"), C::Sensors, K::SensorNc, "B", &["limit_nc", "nfpa_ls_nc"]),
        ("fn.pressure_switch", tr_const!("Pressure switch"), C::Sensors, K::Sensor, "B", &["pressure_no"]),
        ("fn.temperature_switch", tr_const!("Temperature switch"), C::Sensors, K::SensorNc, "B", &["temp_nc"]),
        ("fn.level_switch", tr_const!("Level switch"), C::Sensors, K::Sensor, "B", &["level_no"]),
        ("fn.transmitter", tr_const!("Measuring transducer"), C::Sensors, K::Transmitter, "B", &["transmitter"]),
        ("fn.thermistor", tr_const!("Thermistor (PTC)"), C::Sensors, K::Thermistor, "B", &["ptc"]),
        ("fn.instrument", tr_const!("Instrument (PCT loop)"), C::Sensors, K::Transmitter, "B", &["pid_instrument"]),
        ("fn.fuse", tr_const!("Fuse"), C::Protection, K::Fuse, "F", &["fuse", "nfpa_fuse", "sl_fuse"]),
        ("fn.fuse3", tr_const!("Fuse 3-pole"), C::Protection, K::Fuse3, "F", &["fuse3"]),
        ("fn.fuse_switch3", tr_const!("Fuse switch-disconnector 3-pole"), C::Protection, K::Fuse3, "Q", &["fuse_swdisc3", "sl_fuse_switch"]),
        ("fn.cb1", tr_const!("Circuit breaker 1-pole"), C::Protection, K::Breaker1, "F", &["cb1"]),
        ("fn.cb2", tr_const!("Circuit breaker 2-pole"), C::Protection, K::Breaker2, "F", &["cb2"]),
        ("fn.cb3", tr_const!("Circuit breaker 3-pole"), C::Protection, K::Breaker3, "Q", &["cb3", "nfpa_cb3", "sl_cb"]),
        ("fn.cb4", tr_const!("Circuit breaker 4-pole"), C::Protection, K::Breaker4, "Q", &["cb4"]),
        ("fn.rcd", tr_const!("Residual current device"), C::Protection, K::ResidualCurrent, "F", &["rcd4"]),
        ("fn.motor_protection", tr_const!("Motor protection switch"), C::Protection, K::MotorProtection, "Q", &["mps3"]),
        ("fn.overload", tr_const!("Overload relay"), C::Protection, K::Overload, "F", &["ol3", "nfpa_ol"]),
        ("fn.spd", tr_const!("Surge protective device"), C::Protection, K::SurgeProtection, "F", &["spd"]),
        ("fn.earth_electrode", tr_const!("Earth electrode"), C::Protection, K::Generic, "E", &["earth_electrode"]),
        ("fn.motor3", tr_const!("Motor 3-phase"), C::Motors, K::Motor3, "M", &["motor3", "nfpa_motor", "sl_motor"]),
        ("fn.motor1", tr_const!("Motor 1-phase"), C::Motors, K::Motor1, "M", &["motor1"]),
        ("fn.motor_dc", tr_const!("Motor DC"), C::Motors, K::MotorDc, "M", &["motor_dc"]),
        ("fn.brake", tr_const!("Brake"), C::Motors, K::Brake, "Y", &["brake"]),
        ("fn.actuator", tr_const!("Valve actuator"), C::Motors, K::Actuator, "M", &["actuator"]),
        ("fn.vfd", tr_const!("Frequency converter"), C::Drives, K::FrequencyConverter, "T", &["vfd"]),
        ("fn.soft_starter", tr_const!("Soft starter"), C::Drives, K::SoftStarter, "Q", &["softstarter"]),
        ("fn.lamp", tr_const!("Indicator lamp"), C::Signal, K::Signal, "P", &["lamp", "nfpa_lamp"]),
        ("fn.horn", tr_const!("Horn / buzzer"), C::Signal, K::Signal, "P", &["horn"]),
        ("fn.meter", tr_const!("Measuring instrument"), C::Measuring, K::Meter, "P", &["ammeter", "voltmeter"]),
        ("fn.ct", tr_const!("Current transformer"), C::Measuring, K::CurrentTransformer, "T", &["ct"]),
        ("fn.energy_meter", tr_const!("Energy meter"), C::Measuring, K::Meter, "P", &["kwh3", "kwh3_ct", "sl_meter"]),
        ("fn.psu", tr_const!("Power supply"), C::PowerSupply, K::PowerSupply, "T", &["psu"]),
        ("fn.transformer", tr_const!("Transformer"), C::PowerSupply, K::Transformer, "T", &["trafo", "sl_trafo"]),
        ("fn.battery", tr_const!("Battery"), C::PowerSupply, K::Battery, "G", &["battery", "sl_battery"]),
        ("fn.pv_generator", tr_const!("PV generator"), C::PowerSupply, K::PvGenerator, "G", &["pv", "sl_pv"]),
        ("fn.inverter", tr_const!("Inverter"), C::PowerSupply, K::PowerSupply, "T", &["psu_dcac", "hybrid_inverter", "inverter_charger", "sl_inverter"]),
        ("fn.solar_charger", tr_const!("Solar charge controller"), C::PowerSupply, K::PowerSupply, "T", &["mppt"]),
        ("fn.heater", tr_const!("Heating element"), C::Installation, K::Heater, "E", &["heater"]),
        ("fn.socket", tr_const!("Socket outlet"), C::Installation, K::SocketOutlet, "X", &["socket"]),
        ("fn.installation_switch", tr_const!("Installation switch"), C::Installation, K::Generic, "S", &["inst_switch", "inst_switch2", "inst_dimmer", "inst_push"]),
        ("fn.lighting_outlet", tr_const!("Lighting outlet"), C::Installation, K::Generic, "E", &["inst_lamp", "inst_luminaire"]),
        ("fn.installation_socket", tr_const!("Double socket outlet with PE"), C::Installation, K::Generic, "X", &["inst_socket2"]),
        ("fn.junction_box", tr_const!("Junction box"), C::Installation, K::Generic, "X", &["inst_junction"]),
        ("fn.motion_detector", tr_const!("Motion detector"), C::Installation, K::Generic, "B", &["inst_motion"]),
        ("fn.distribution_board", tr_const!("Distribution board"), C::Installation, K::Generic, "A", &["inst_board"]),
        ("fn.terminal", tr_const!("Terminal"), C::Terminals, K::Terminal, "X", &["term"]),
        ("fn.terminal_pe", tr_const!("PE terminal"), C::Terminals, K::Terminal, "X", &["term_pe"]),
        ("fn.terminal_n", tr_const!("N terminal"), C::Terminals, K::Terminal, "X", &["term_n"]),
        ("fn.terminal_disconnect", tr_const!("Disconnect terminal"), C::Terminals, K::Terminal, "X", &["term_disc"]),
        ("fn.terminal_fuse", tr_const!("Fuse terminal"), C::Terminals, K::Terminal, "X", &["term_fuse"]),
        ("fn.plug_pair", tr_const!("Plug pin and socket"), C::Plugs, K::PlugPin, "X", &["plug_pair"]),
        ("fn.plug_male", tr_const!("Plug male pin"), C::Plugs, K::PlugPin, "X", &["plug_m"]),
        ("fn.plug_female", tr_const!("Plug female socket"), C::Plugs, K::PlugPin, "X", &["plug_f"]),
        ("fn.plc_di", tr_const!("PLC digital input"), C::Plc, K::PlcInput, "A", &["plc_in"]),
        ("fn.plc_do", tr_const!("PLC digital output"), C::Plc, K::PlcOutput, "A", &["plc_out"]),
        ("fn.plc_ai", tr_const!("PLC analog input"), C::Plc, K::PlcAnalogInput, "A", &["plc_ai"]),
        ("fn.plc_ao", tr_const!("PLC analog output"), C::Plc, K::PlcAnalogOutput, "A", &["plc_ao"]),
        ("fn.plc_card", tr_const!("PLC card"), C::Plc, K::Generic, "A", &["plcbox_di8", "plcbox_do8"]),
        ("fn.resistor", tr_const!("Resistor"), C::Passive, K::Generic, "R", &["res"]),
        ("fn.potentiometer", tr_const!("Potentiometer"), C::Passive, K::Generic, "R", &["potentiometer"]),
        ("fn.capacitor", tr_const!("Capacitor"), C::Passive, K::Generic, "C", &["cap"]),
        ("fn.diode", tr_const!("Diode"), C::Passive, K::Generic, "V", &["diode"]),
        ("fn.led", tr_const!("LED"), C::Passive, K::Generic, "P", &["led"]),
        ("fn.inductor", tr_const!("Inductor"), C::Passive, K::Generic, "L", &["inductor"]),
        ("fn.varistor", tr_const!("Varistor"), C::Passive, K::Generic, "R", &["varistor"]),
        ("fn.capacitor_polarized", tr_const!("Capacitor, polarized"), C::Passive, K::Generic, "C", &["cap_pol"]),
        ("fn.zener_diode", tr_const!("Zener diode"), C::Passive, K::Generic, "V", &["zener"]),
        ("fn.photodiode", tr_const!("Photodiode"), C::Passive, K::Generic, "B", &["photodiode"]),
        ("fn.transistor", tr_const!("Transistor"), C::Passive, K::Generic, "V", &["npn", "pnp", "nmos"]),
        ("fn.thyristor", tr_const!("Thyristor"), C::Passive, K::Generic, "V", &["thyristor", "triac"]),
        ("fn.ntc", tr_const!("Thermistor NTC"), C::Passive, K::Generic, "R", &["ntc"]),
        ("fn.photoresistor", tr_const!("Photoresistor (LDR)"), C::Passive, K::Generic, "R", &["ldr"]),
        ("fn.optocoupler", tr_const!("Optocoupler"), C::Passive, K::Generic, "U", &["optocoupler"]),
        ("fn.bridge_rectifier", tr_const!("Bridge rectifier"), C::Passive, K::Generic, "T", &["bridge_rect"]),
        ("fn.black_box", tr_const!("Black box"), C::BlackBox, K::Generic, "A", &["box1", "box2", "box4"]),
        ("fn.potential", tr_const!("Potential"), C::Potentials, K::Generic, "", &["ground"]),
        ("fn.interruption", tr_const!("Interruption point"), C::Potentials, K::Generic, "", &["intr"]),
        ("fn.valve_coil", tr_const!("Solenoid valve coil"), C::Fluid, K::Valve, "Y", &["valve"]),
        ("fn.directional_valve", tr_const!("Directional valve"), C::Fluid, K::DirectionalValve, "QM", &["fl_valve42", "fl_valve52"]),
        ("fn.cylinder", tr_const!("Cylinder"), C::Fluid, K::Cylinder, "MM", &["fl_cylinder", "fl_cylinder_sa"]),
        ("fn.pump", tr_const!("Pump"), C::Fluid, K::Pump, "GP", &["fl_pump", "fl_compressor"]),
        ("fn.fluid_component", tr_const!("Fluid component"), C::Fluid, K::FluidComponent, "", &["fl_filter", "fl_gauge", "fl_tank", "fl_source", "fl_silencer", "fl_throttle", "fl_shutoff", "fl_regulator", "fl_receiver", "fl_dryer", "fl_lubricator", "fl_coupling"]),
        ("fn.process_pump", tr_const!("Process pump"), C::Process, K::Pump, "GP", &["pid_pump"]),
        ("fn.process_component", tr_const!("Process component"), C::Process, K::FluidComponent, "", &["pid_vessel", "pid_valve", "pid_control_valve", "pid_check_valve", "pid_heat_exchanger", "pid_motor_valve", "pid_flow_meter", "pid_connection", "pid_heat_pump", "pid_buffer"]),
        ("fn.generic", tr_const!("Generic function"), C::General, K::Generic, "", &[]),
    ]
};

/// The built-in function definitions.
pub fn builtin() -> &'static [FunctionDef] {
    static DEFS: OnceLock<Vec<FunctionDef>> = OnceLock::new();
    DEFS.get_or_init(|| {
        ROWS.iter()
            .map(|&(key, name, category, kind, letter, syms)| FunctionDef {
                key: key.into(),
                name: name.into(),
                category,
                kind,
                letter: letter.into(),
                symbol: syms.first().map(|s| s.to_string()).unwrap_or_default(),
                builtin: true,
                ..Default::default()
            })
            .collect()
    })
}

pub fn builtin_def(key: &str) -> Option<&'static FunctionDef> {
    builtin().iter().find(|d| d.key == key)
}

/// A definition by key: the project's own first (they may refine built-in ones
/// under a new key), then the built-in ones.
pub fn get<'a>(prj: &'a Project, key: &str) -> Option<&'a FunctionDef> {
    prj.property_db.functions.iter().find(|d| d.key == key).or_else(|| builtin_def(key))
}

/// All definitions: built-in, then the project's own.
pub fn all(prj: &Project) -> impl Iterator<Item = &FunctionDef> {
    builtin().iter().chain(prj.property_db.functions.iter())
}

/// Built-in definition a library symbol stands for.
pub fn default_for_symbol(prj: &Project, sym: &str) -> &'static FunctionDef {
    let by_list = ROWS.iter().position(|r| r.5.contains(&sym));
    let i = by_list.or_else(|| {
        let def = prj.symbol(sym)?;
        let key = match def.func {
            Func::Potential => "fn.potential",
            Func::Interruption => "fn.interruption",
            Func::Terminal => "fn.terminal",
            Func::Plug(_) => "fn.plug_pair",
            Func::Plc if def.digital_input() == Some(false) => "fn.plc_do",
            Func::Plc => "fn.plc_di",
            _ => match crate::parts::kind_of_symbol(sym) {
                Some(kind) => return ROWS.iter().position(|r| r.3 == kind),
                None if def.category == symbols::Category::BlackBox => "fn.black_box",
                None => return None,
            },
        };
        ROWS.iter().position(|r| r.0 == key)
    });
    let generic = || builtin().iter().position(|d| d.key == "fn.generic").unwrap_or(0);
    &builtin()[i.unwrap_or_else(generic)]
}

/// The definition of a placed symbol: the one assigned to it, otherwise the
/// symbol's built-in one.
pub fn of_symbol<'a>(prj: &'a Project, s: &SymbolInst) -> Option<&'a FunctionDef> {
    if !s.function_def.is_empty() {
        if let Some(d) = get(prj, &s.function_def) {
            return Some(d);
        }
    }
    Some(default_for_symbol(prj, &s.sym))
}

/// Definitions a symbol can take: those for the same kind of function (built-in
/// ones of its symbol family and the project's ones with the same kind or symbol).
pub fn fitting<'a>(prj: &'a Project, s: &SymbolInst) -> Vec<&'a FunctionDef> {
    let own = default_for_symbol(prj, &s.sym);
    all(prj).filter(|d| d.key == own.key || d.symbol == s.sym || (d.kind == own.kind && (d.kind != FnKind::Generic || !d.builtin))).collect()
}

/// Adds or replaces a project definition.
pub fn define(prj: &mut Project, d: FunctionDef) -> Result<(), String> {
    if d.key.trim().is_empty() || d.name.trim().is_empty() {
        return Err(tr("The function definition needs a key and a name").into());
    }
    if builtin_def(&d.key).is_some() {
        return Err(tr_fmt!("{} is a built-in function definition", d.key));
    }
    if !d.symbol.is_empty() && prj.symbol(&d.symbol).is_none() {
        return Err(tr_fmt!("Unknown symbol {}", d.symbol));
    }
    for k in d.defaults.keys() {
        if crate::properties::valid_key(prj, k).is_none() {
            return Err(tr_fmt!("Unknown property: {}", k));
        }
    }
    let d = FunctionDef { builtin: false, ..d };
    match prj.property_db.functions.iter_mut().find(|x| x.key == d.key) {
        Some(x) => *x = d,
        None => prj.property_db.functions.push(d),
    }
    Ok(())
}

/// A new project definition derived from another one, with a free key.
pub fn derive(prj: &Project, from: &FunctionDef) -> FunctionDef {
    let base = format!("user.{}", from.key.trim_start_matches("fn.").trim_start_matches("user."));
    let mut key = base.clone();
    let mut n = 1;
    while get(prj, &key).is_some() {
        n += 1;
        key = format!("{base}_{n}");
    }
    FunctionDef { key, name: from.title(), builtin: false, ..from.clone() }
}

/// Removes a project definition; symbols using it go back to their default one.
/// Returns the number of symbols changed.
pub fn remove(prj: &mut Project, key: &str) -> usize {
    prj.property_db.functions.retain(|d| d.key != key);
    let mut n = 0;
    for s in prj.pages.iter_mut().flat_map(|pg| pg.symbols.iter_mut()).filter(|s| s.function_def == key) {
        s.function_def.clear();
        n += 1;
    }
    n
}

/// Assigns a definition to a placed symbol: pin designations and placement scheme
/// are taken over when the definition has them. The symbol's built-in definition
/// is stored as "none".
pub fn assign(prj: &mut Project, symbol_id: u64, key: &str) -> Result<(), String> {
    let symbols = prj.symbol_table();
    let d = get(prj, key).ok_or_else(|| tr_fmt!("Unknown function definition: {}", key))?.clone();
    let (pi, si) = prj
        .pages
        .iter()
        .enumerate()
        .find_map(|(pi, pg)| pg.symbols.iter().position(|s| s.id == symbol_id).map(|si| (pi, si)))
        .ok_or_else(|| tr_fmt!("No symbol with id {}", symbol_id))?;
    let s = &prj.pages[pi].symbols[si];
    let builtin_own = default_for_symbol(prj, &s.sym).key == d.key;
    let scheme = (!d.scheme.is_empty()).then(|| crate::props::scheme(prj, &d.scheme).cloned()).flatten();
    let s = &mut prj.pages[pi].symbols[si];
    s.function_def = if builtin_own { String::new() } else { d.key.clone() };
    let pins = d.pin_list();
    if !pins.is_empty() {
        for (i, p) in pins.into_iter().enumerate().take(s.pins.len()) {
            s.pins[i] = p;
        }
    }
    if let Some(scheme) = scheme {
        crate::props::apply_scheme(&symbols, s, &scheme);
    }
    Ok(())
}

/// Number of placed symbols per definition key.
pub fn usage(prj: &Project) -> std::collections::BTreeMap<String, usize> {
    let mut out = std::collections::BTreeMap::new();
    for s in prj.pages.iter().flat_map(|pg| pg.symbols.iter()) {
        if let Some(d) = of_symbol(prj, s) {
            *out.entry(d.key.clone()).or_insert(0) += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests;
