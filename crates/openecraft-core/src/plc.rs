//! PLC overview: inputs / outputs per card, addressing from the card's start byte and
//! symbol table exchange with PLC programming tools (CSV: name, data type, address,
//! comment; IEC 61131-3 global variables; Siemens TIA Portal tag tables; Rockwell
//! Studio 5000 tags). The hardware (stations, racks, buses) is in `plc_hw`.

use crate::model::*;
use crate::netlist::{pin_designation, Netlist};
use crate::symbols::Func;
use std::collections::{BTreeMap, HashMap, HashSet};

/// A PLC input or output placed in the schematic.
#[derive(Clone, Debug, PartialEq)]
pub struct Point {
    pub page: usize,
    pub sym: usize,
    pub id: u64,
    pub card: String,
    pub address: String,
    pub input: bool,
    pub analog_type: Option<String>,
    pub channel: String,
    pub terminal: String,
    pub function_text: String,
    /// Devices connected to the point.
    pub connected: String,
    pub location: String,
}

impl Point {
    pub fn io_kind(&self) -> &'static str {
        match (self.analog_type.is_some(), self.input) {
            (true, true) => "AI", (true, false) => "AO",
            (false, true) => "DI", (false, false) => "DO",
        }
    }
}

fn is_plc(symbols: &crate::symlib::Symbols, s: &SymbolInst) -> bool {
    symbols.get(&s.sym).map(|d| d.func) == Some(Func::Plc)
}

/// Prefix, byte and bit of an address.
pub type AddressKey = (String, u32, u32);

/// Address sort key: prefix, byte, bit (`I1.7` → ('I', 1, 7)). Rockwell addresses
/// keep module and direction in the prefix (`Local:3:I.Data.5` → ("LOCAL:3:I", 0, 5)).
pub fn address_key(addr: &str) -> AddressKey {
    let a = addr.trim().trim_start_matches('%').to_uppercase();
    if let Some(r) = crate::plc_hw::parse_address(&a).filter(|r| r.slot.is_some()) {
        let prefix = a.split('.').next().unwrap_or_default().to_string();
        return (prefix, r.byte, r.bit);
    }
    let prefix: String = a.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
    let rest = &a[prefix.len()..];
    let (byte, bit) = rest.split_once('.').unwrap_or((rest, "0"));
    (prefix, byte.parse().unwrap_or(u32::MAX), bit.parse().unwrap_or(0))
}

/// Tags of all PLC cards.
pub fn cards(prj: &Project) -> Vec<String> {
    let symbols = prj.symbol_table();
    let mut v: Vec<String> =
        prj.pages.iter().flat_map(|p| p.symbols.iter()).filter(|s| is_plc(&symbols, s) && !s.tag.is_empty()).map(|s| s.tag.clone()).collect();
    v.extend(prj.plc_cards.keys().cloned());
    v.sort_by(|a, b| natural_cmp(a, b));
    v.dedup();
    v
}

/// Page and symbol index of a pin.
type PinAt = (usize, usize);

/// Targets listed per PLC point; further ones are counted.
const MAX_TARGETS: usize = 20;

/// All PLC points, sorted by card and address.
pub fn points(prj: &Project, nl: &Netlist) -> Vec<Point> {
    let symbols = prj.symbol_table();
    let mut v = Vec::new();
    // Designations of the pins of each net, built once (nets of potentials can have
    // thousands of pins on large projects).
    let mut by_net: HashMap<usize, Vec<(String, PinAt)>> = HashMap::new();
    for (pi, pg) in prj.pages.iter().enumerate() {
        if pg.kind != PageKind::Schematic {
            continue;
        }
        for (si, s) in pg.symbols.iter().enumerate() {
            let Some(def) = symbols.get(&s.sym).filter(|d| d.func == Func::Plc) else { continue };
            let channel = def.plc_channel.as_ref();
            let analog = channel.and_then(|c| c.analog.as_ref());
            let signal_pin = analog.and_then(|a| def.pins.iter().position(|p| p.label == a.terminal)).unwrap_or(0);
            let connected = nl
                .pin_net
                .get(&(s.id, signal_pin))
                .map(|&n| {
                    let list = by_net.entry(n).or_insert_with(|| {
                        let mut t: Vec<(String, PinAt)> = nl.nets[n].pins.iter().map(|p| (pin_designation(prj, *p), (p.page, p.sym))).collect();
                        t.sort_by(|a, b| natural_cmp(&a.0, &b.0));
                        t
                    });
                    let mut shown: Vec<&str> = Vec::new();
                    let mut more = 0;
                    let mut last = "";
                    for (d, _) in list.iter().filter(|(_, at)| *at != (pi, si)) {
                        if d == last {
                            continue;
                        }
                        last = d;
                        if shown.len() < MAX_TARGETS {
                            shown.push(d);
                        } else {
                            more += 1;
                        }
                    }
                    let mut text = shown.join(", ");
                    if more > 0 {
                        text.push_str(&format!(", … (+{more})"));
                    }
                    text
                })
                .unwrap_or_default();
            v.push(Point {
                page: pi,
                sym: si,
                id: s.id,
                card: s.tag.clone(),
                address: s.label.clone(),
                input: channel.map(|c| c.input).or_else(|| def.digital_input()).unwrap_or(s.sym == "plc_ai"),
                analog_type: analog.map(|a| a.data_type.clone()),
                channel: def.plc_channel.as_ref().map(|c| c.designation.clone()).unwrap_or_default(),
                terminal: s.pins.get(signal_pin).cloned().unwrap_or_default(),
                function_text: crate::text::function_text(prj, pi, s).replace('\n', " "),
                connected,
                location: crate::frame::reference(prj, pi, s.pos),
            });
        }
    }
    v.sort_by(|a, b| natural_cmp(&a.card, &b.card).then_with(|| address_key(&a.address).cmp(&address_key(&b.address))));
    v
}

/// Gives the digital points of a card consecutive addresses from its start byte (inputs
/// and outputs separately, in schematic order) in the address scheme of its station.
/// Returns the number of changed addresses.
pub fn readdress(prj: &mut Project, card: &str) -> usize {
    let symbols = prj.symbol_table();
    let mut changed = 0;
    for input in [true, false] {
        let mut list: Vec<(usize, usize)> = Vec::new();
        for (pi, pg) in prj.pages.iter().enumerate() {
            for (si, s) in pg.symbols.iter().enumerate() {
                if s.tag == card && symbols.get(&s.sym).and_then(|d| d.digital_input()) == Some(input) {
                    list.push((pi, si));
                }
            }
        }
        list.sort_by(|a, b| {
            let (sa, sb) = (&prj.pages[a.0].symbols[a.1], &prj.pages[b.0].symbols[b.1]);
            a.0.cmp(&b.0).then(sa.pos.x.total_cmp(&sb.pos.x)).then(sa.pos.y.total_cmp(&sb.pos.y))
        });
        let reserved: HashSet<_> = list.iter().filter_map(|&(pi, si)| {
            let def = symbols.get(&prj.pages[pi].symbols[si].sym)?;
            def.plc_channel.as_ref().map(|c| u32::from(c.index))
        }).collect();
        let mut next = (0..).filter(|n| !reserved.contains(n));
        for (pi, si) in list {
            let def = symbols.get(&prj.pages[pi].symbols[si].sym);
            let ordinal = def.as_ref().and_then(|d| d.plc_channel.as_ref()).map_or_else(|| next.next().unwrap(), |c| u32::from(c.index));
            let addr = crate::plc_hw::card_address(prj, card, input, ordinal);
            let s = &mut prj.pages[pi].symbols[si];
            if s.label != addr {
                s.label = addr;
                changed += 1;
            }
        }
    }
    changed
}

/// Tag name for a PLC tool: letters, digits and underscores.
fn tag_name(p: &Point) -> String {
    let base = if p.function_text.trim().is_empty() { format!("{}_{}", p.card, p.address) } else { p.function_text.clone() };
    let mut s: String = base.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    while s.contains("__") {
        s = s.replace("__", "_");
    }
    let s = s.trim_matches('_').to_string();
    if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s
    }
}

/// PLC points with unique tag names.
pub fn tags(prj: &Project, nl: &Netlist) -> Vec<(String, Point)> {
    let mut used = HashSet::new();
    let mut v = Vec::new();
    for p in points(prj, nl) {
        let mut name = tag_name(&p);
        let mut i = 2;
        while !used.insert(name.clone()) {
            name = format!("{}_{i}", tag_name(&p));
            i += 1;
        }
        v.push((name, p));
    }
    v
}

/// Comment of a tag: card and function text.
pub fn tag_comment(p: &Point) -> String {
    format!("{} {}", p.card, p.function_text).trim().to_string()
}

/// Symbol table for PLC programming tools: `Name;Data Type;Logical Address;Comment`.
pub fn symbol_table(prj: &Project, nl: &Netlist) -> String {
    let mut out = String::from("Name;Data Type;Logical Address;Comment\n");
    for (name, p) in tags(prj, nl) {
        let comment = tag_comment(&p).replace(';', ",");
        if let Some(ty) = &p.analog_type {
            out.push_str(&format!("{name};{ty};{};{comment}\n", p.address));
        } else {
            out.push_str(&format!("{name};Bool;%{};{comment}\n", p.address.trim_start_matches('%')));
        }
    }
    out
}

/// IEC 61131-3 global variable list (CODESYS, TwinCAT, …): one `VAR_GLOBAL` block per
/// address space with direct addresses (`%IX0.0`).
pub fn global_variables(prj: &Project, nl: &Netlist) -> String {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, p) in tags(prj, nl) {
        if let Some(ty) = &p.analog_type {
            if !ty.is_empty() {
                groups.entry(crate::plc_hw::card_domain(prj, &p.card)).or_default().push(format!("    {name} : {ty};"));
            }
            continue;
        }
        let Some((input, byte, bit)) = absolute(prj, &p) else { continue };
        let comment = tag_comment(&p).replace("*)", "* )");
        let dir = if input { 'I' } else { 'Q' };
        let line = format!("    {name} AT %{dir}X{byte}.{bit} : BOOL; (* {comment} *)");
        groups.entry(crate::plc_hw::card_domain(prj, &p.card)).or_default().push(line);
    }
    let mut out = String::new();
    for (station, lines) in groups {
        if !station.is_empty() {
            out.push_str(&format!("(* {station} *)\n"));
        }
        out.push_str("VAR_GLOBAL\n");
        for l in lines {
            out.push_str(&l);
            out.push('\n');
        }
        out.push_str("END_VAR\n\n");
    }
    out
}

/// Direction, byte and bit of a point in the process image: module relative
/// addresses (Rockwell) are placed at the card's start byte.
fn absolute(prj: &Project, p: &Point) -> Option<(bool, u32, u32)> {
    if p.analog_type.is_some() { return None; }
    let a = crate::plc_hw::parse_address(&p.address)?;
    let data = prj.plc_cards.get(&p.card).cloned().unwrap_or_default();
    let byte = match a.slot {
        Some(_) => a.byte + if a.input { data.start_byte } else { data.output_byte.unwrap_or(data.start_byte) },
        None => a.byte,
    };
    Some((a.input, byte, a.bit))
}

/// Rows of a Siemens TIA Portal PLC tag table (sheet `PLC Tags`, header first):
/// name, path (tag table), data type, logical address (`%I0.0`), comment and the HMI
/// flags.
pub fn tia_tags(prj: &Project, nl: &Netlist) -> Vec<Vec<String>> {
    let header = ["Name", "Path", "Data Type", "Logical Address", "Comment", "Hmi Visible", "Hmi Accessible", "Hmi Writeable", "Typeobject ID", "Version ID"];
    let mut rows = vec![header.iter().map(|h| h.to_string()).collect::<Vec<_>>()];
    for (name, p) in tags(prj, nl) {
        let Some((input, byte, bit)) = absolute(prj, &p) else { continue };
        let station = crate::plc_hw::card_domain(prj, &p.card);
        let path = if station.is_empty() { "Default tag table".to_string() } else { station };
        let addr = format!("%{}{byte}.{bit}", if input { 'I' } else { 'Q' });
        rows.push([name, path, "Bool".into(), addr, tag_comment(&p), "True".into(), "True".into(), "True".into(), String::new(), String::new()].to_vec());
    }
    rows
}

/// Input or output, slot of the card (otherwise 1) and channel on the card of a point
/// with a byte / bit address.
fn channel(prj: &Project, p: &Point) -> Option<(bool, u32, u32)> {
    if p.analog_type.is_some() { return None; }
    let a = crate::plc_hw::parse_address(&p.address)?;
    let data = prj.plc_cards.get(&p.card).cloned().unwrap_or_default();
    let start = if a.input { data.start_byte } else { data.output_byte.unwrap_or(data.start_byte) };
    let slot: u32 = data.slot.trim().parse().unwrap_or(1);
    Some((a.input, slot, a.byte.saturating_sub(start) * 8 + a.bit))
}

/// Rockwell Studio 5000 tag import (CSV): every point as an alias of its module
/// channel (`Local:3:I.Data.5`; the slot from the card, otherwise 1).
pub fn rockwell_tags(prj: &Project, nl: &Netlist) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = String::from("remark,\"CSV-Import-Export\"\nremark,\"Generated by OpenECraft\"\n0.3\nTYPE,SCOPE,NAME,DESCRIPTION,DATATYPE,SPECIFIER,ATTRIBUTES\n");
    for (name, p) in tags(prj, nl) {
        let Some(a) = crate::plc_hw::parse_address(&p.address) else { continue };
        let specifier = if a.slot.is_some() {
            p.address.trim().trim_start_matches('%').to_string()
        } else {
            let Some((input, slot, channel)) = channel(prj, &p) else { continue };
            format!("Local:{slot}:{}.Data.{channel}", if input { 'I' } else { 'O' })
        };
        out.push_str(&format!("ALIAS,,{},{},,{},{}\n", q(&name), q(&tag_comment(&p)), q(&specifier), q("(RADIX := Decimal, ExternalAccess := Read/Write)")));
    }
    out
}

/// Siemens STEP 7 (classic) symbol table to import into the symbol editor (`.sdf`):
/// per point the quoted, blank padded fields symbol (24), address (12), data type (10)
/// and comment (80).
pub fn step7_sdf(prj: &Project, nl: &Netlist) -> String {
    let field = |s: &str, w: usize| {
        let t: String = s.replace('"', "'").chars().take(w).collect();
        format!("\"{t:<w$}\"")
    };
    let mut out = String::new();
    for (name, p) in tags(prj, nl) {
        let Some((input, byte, bit)) = absolute(prj, &p) else { continue };
        let addr = format!("{:<4}{byte:>4}.{bit}", if input { "I" } else { "Q" });
        out.push_str(&[field(&name, 24), field(&addr, 12), field("BOOL", 10), field(&tag_comment(&p), 80)].join(","));
        out.push_str("\r\n");
    }
    out
}

/// Mitsubishi GX Works device comments (CSV): inputs `X` and outputs `Y` numbered in
/// octal (byte · 8 + bit), the comment from card and function text.
pub fn mitsubishi_comments(prj: &Project, nl: &Netlist) -> String {
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = String::from("\"Device Name\",\"Comment\"\r\n");
    for (_, p) in tags(prj, nl) {
        let Some((input, byte, bit)) = absolute(prj, &p) else { continue };
        let device = format!("{}{:o}", if input { 'X' } else { 'Y' }, byte * 8 + bit);
        let comment: String = tag_comment(&p).chars().take(32).collect();
        out.push_str(&format!("{},{}\r\n", q(&device), q(&comment)));
    }
    out
}

/// Beckhoff TwinCAT 3 global variable list (`.TcGVL`): the points as `AT %I*` /
/// `AT %Q*` variables, linked to the terminals in the I/O configuration.
pub fn twincat_gvl(prj: &Project, nl: &Netlist) -> String {
    let mut decl = String::from("{attribute 'qualified_only'}\nVAR_GLOBAL\n");
    for (name, p) in tags(prj, nl) {
        if p.analog_type.is_none() && absolute(prj, &p).is_none() { continue; }
        let comment = tag_comment(&p).replace('\n', " ");
        let ty = p.analog_type.as_deref().unwrap_or("BOOL");
        if ty.is_empty() { continue; }
        decl.push_str(&format!("    {name} AT %{}* : {ty}; // {comment}\n", if p.input { 'I' } else { 'Q' }));
    }
    decl.push_str("END_VAR\n");
    let decl = decl.replace("]]>", "]] >");
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<TcPlcObject Version=\"1.1.0.1\" ProductVersion=\"3.1.4024.0\">\n  <GVL Name=\"GVL_IO\">\n    <Declaration><![CDATA[{decl}]]></Declaration>\n  </GVL>\n</TcPlcObject>\n"
    )
}

fn xml_attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Schneider Electric Control Expert (Unity Pro) variables (`.xsy`): `EBOOL` variables
/// with the topological address `%I0.<slot>.<channel>` of their card.
pub fn schneider_xsy(prj: &Project, nl: &Netlist) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<VariablesExchangeFile>\n");
    out.push_str("<fileHeader company=\"OpenECraft\" product=\"OpenECraft\" content=\"Variable source file\" DTDVersion=\"41\"></fileHeader>\n");
    out.push_str(&format!("<contentHeader name=\"{}\" version=\"0.0.000\"></contentHeader>\n<dataBlock>\n", xml_attr(&prj.info.name)));
    for (name, p) in tags(prj, nl) {
        let Some((input, slot, ch)) = channel(prj, &p) else { continue };
        out.push_str(&format!(
            "<variables name=\"{}\" typeName=\"EBOOL\" topologicalAddress=\"%{}0.{slot}.{ch}\"><comment>{}</comment></variables>\n",
            xml_attr(&name),
            if input { 'I' } else { 'Q' },
            xml_attr(&tag_comment(&p))
        ));
    }
    out.push_str("</dataBlock>\n</VariablesExchangeFile>\n");
    out
}

/// Takes the comments (or names) of a PLC symbol table as function texts of the
/// points with the same address. Columns are found by their header (`Address` /
/// `Logical Address`, `Comment`, `Name`); `;`, `,` or tab separated. Returns the
/// number of updated points.
pub fn import_symbol_table(prj: &mut Project, text: &str) -> Result<usize, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header = lines.next().ok_or("empty file")?;
    let sep = [';', '\t', ','].into_iter().max_by_key(|c| header.matches(*c).count()).unwrap_or(';');
    let cols: Vec<String> = header.split(sep).map(|c| c.trim().trim_matches('"').to_lowercase()).collect();
    let find = |names: &[&str]| cols.iter().position(|c| names.contains(&c.as_str()));
    let addr_col = find(&["logical address", "address", "adresse"]).ok_or("no address column")?;
    let comment_col = find(&["comment", "kommentar"]);
    let name_col = find(&["name", "symbol"]);
    let mut texts: BTreeMap<AddressKey, String> = BTreeMap::new();
    for line in lines {
        let cells: Vec<&str> = line.split(sep).map(|c| c.trim().trim_matches('"')).collect();
        let Some(addr) = cells.get(addr_col).filter(|a| !a.is_empty()) else { continue };
        let comment = comment_col.and_then(|i| cells.get(i)).filter(|c| !c.is_empty());
        let text = comment.or(name_col.and_then(|i| cells.get(i))).map(|t| t.to_string()).unwrap_or_default();
        texts.insert(address_key(addr), text);
    }
    Ok(apply_texts(prj, None, &texts))
}

/// Sets the function texts of the points by address (only on the cards of address
/// space `domain` if given). A leading card tag (as in exported comments) is dropped.
/// Returns the number of changed points.
pub fn apply_texts(prj: &mut Project, domain: Option<&str>, texts: &BTreeMap<AddressKey, String>) -> usize {
    let symbols = prj.symbol_table();
    let cards: HashSet<String> = cards(prj).into_iter().filter(|c| domain.is_none_or(|d| crate::plc_hw::card_domain(prj, c) == d)).collect();
    let mut n = 0;
    for pg in &mut prj.pages {
        for s in pg.symbols.iter_mut().filter(|s| is_plc(&symbols, s) && (domain.is_none() || cards.contains(&s.tag))) {
            if let Some(t) = texts.get(&address_key(&s.label)) {
                let t = t.strip_prefix(&format!("{} ", s.tag)).unwrap_or(t).trim();
                if !t.is_empty() && s.function_text != t {
                    s.function_text = t.to_string();
                    n += 1;
                }
            }
        }
    }
    n
}

/// Addresses used by more than one point (over all cards of an address space: a
/// controller with its remote I/O).
pub fn duplicate_addresses(prj: &Project, nl: &Netlist) -> Vec<(String, Vec<Point>)> {
    let mut by: BTreeMap<(String, AddressKey), Vec<Point>> = BTreeMap::new();
    for p in points(prj, nl) {
        if p.analog_type.is_some() { continue; } // Local channel names do not allocate process-image addresses.
        by.entry((crate::plc_hw::card_domain(prj, &p.card), address_key(&p.address))).or_default().push(p);
    }
    by.into_values().filter(|v| v.len() > 1).map(|v| (v[0].address.clone(), v)).collect()
}

#[cfg(test)]
mod tests;
