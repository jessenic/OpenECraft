//! Potential-transfer references from a function's S16x1062 connection-point logic.
//! Numeric references address a local pin slot. Quoted digits and plug:name
//! references address a designation at the same device. Target tracking is separate.
use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Transfer {
    pub object: String,
    pub slot: usize,
    pub label: String,
    pub target: String,
    pub status: String,
}

struct Port {
    group: usize,
    slot: usize,
    object: String,
    symbol: String,
    owner: Option<Vec<String>>,
    plug: String,
    designation: String,
    native: Option<usize>,
    static_contact: bool,
}

type Slots = BTreeMap<(usize, usize), usize>;
type Names<'a> = BTreeMap<(&'a str, &'a [String], &'a str, &'a str), Option<usize>>;

impl Port {
    fn label(&self) -> String {
        if self.plug.is_empty() {
            self.designation.clone()
        } else if self.designation.is_empty() {
            self.plug.clone()
        } else {
            format!("{}.{}", self.plug, self.designation)
        }
    }
}

#[derive(Default)]
pub(super) struct Logic {
    groups: usize,
    ports: Vec<Port>,
    references: Vec<(usize, String)>,
}

fn owner(el: &Element) -> Option<Vec<String>> {
    if let Some(dt) = device_tag(el) {
        return Some(vec![dt.to_owned()]);
    }
    let p = el.child("P150")?;
    if p.attribute("P20013").is_none_or(str::is_empty) {
        return None;
    }
    // Include structure, prefix, letter, counter and subordinate designation.
    Some(
        ["P1100", "P1200", "P1300", "P1400", "P1600", "P1700", "P20012", "P20013", "P20014", "P20015", "P20016", "P20017", "P20018"]
            .iter()
            .map(|k| el.child("P144").and_then(|f| f.attribute(k)).or_else(|| p.attribute(k)).unwrap_or("").to_owned())
            .collect(),
    )
}

fn tokens(raw: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut quoted = false;
    for (i, c) in raw.char_indices() {
        match c {
            '"' => quoted = !quoted,
            ',' if !quoted => {
                out.push(raw[start..i].trim());
                start = i + 1;
            }
            _ => (),
        }
    }
    if quoted {
        return vec![raw];
    }
    out.push(raw[start..].trim());
    out
}

impl Logic {
    pub(super) fn collect(&mut self, el: &Element, symbol: &UserSymbol, first: usize, static_contact: bool) {
        let group = self.groups;
        self.groups += 1;
        let names = fields(el, "A185");
        let mut rules: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        if let Some(logic) = el.child("S61x183") {
            for row in logic.all("S16x1062") {
                let Some(raw) = row.attribute("A163").filter(|s| !s.trim().is_empty()) else { continue };
                // Zero is retained as an unresolved source if the record lacks its slot.
                let slot = row.attribute("A161").and_then(|s| s.parse().ok()).unwrap_or(0);
                rules.entry(slot).or_default().extend(tokens(raw).into_iter().map(str::to_owned));
            }
        }
        let slots: BTreeSet<_> = (1..=names.len().max(symbol.pins.len() - first)).chain(rules.keys().copied()).collect();
        for slot in slots {
            let i = self.ports.len();
            let ord = slot.checked_sub(1);
            let native = ord.and_then(|n| first.checked_add(n)).filter(|n| *n < symbol.pins.len());
            self.ports.push(Port {
                group,
                slot,
                object: el.attribute("A1").unwrap_or("").into(),
                symbol: symbol.key.clone(),
                owner: owner(el),
                plug: plug(el).into(),
                designation: ord.and_then(|n| names.get(n)).cloned().unwrap_or_default(),
                native,
                static_contact,
            });
            if let Some(targets) = rules.remove(&slot) {
                self.references.extend(targets.into_iter().map(|t| (i, t)));
            }
        }
    }

    fn target(&self, src: usize, reference: &str, slots: &Slots, names: &Names<'_>) -> Result<usize, &'static str> {
        let p = &self.ports[src];
        if let Ok(slot) = reference.parse::<usize>() {
            return slots.get(&(p.group, slot)).copied().ok_or("unknown local slot");
        }
        let name = reference.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(reference);
        if name.is_empty() || name.contains('"') {
            return Err("invalid reference");
        }
        let (plug, designation) = name.split_once(':').unwrap_or((&p.plug, name));
        let owner = p.owner.as_deref().ok_or("device identity unavailable")?;
        names
            .get(&(p.symbol.as_str(), owner, plug, designation))
            .ok_or("target designation absent in this variant")?
            .ok_or("ambiguous target designation")
    }

    pub(super) fn finish(self, v: &mut Variant) {
        if self.references.is_empty() {
            return;
        }
        let mut slots = Slots::new();
        let mut names = Names::new();
        for (index, p) in self.ports.iter().enumerate() {
            slots.insert((p.group, p.slot), index);
            if let Some(owner) = p.owner.as_deref() {
                names.entry((&p.symbol, owner, &p.plug, &p.designation)).and_modify(|unique| *unique = None).or_insert(Some(index));
            }
        }
        let targets: Vec<_> = self.references.iter().map(|(s, t)| self.target(*s, t, &slots, &names)).collect();
        let edges: BTreeSet<_> = self.references.iter().zip(&targets).filter_map(|((s, _), t)| Some((*s, *t.as_ref().ok()?))).collect();
        for ((i, raw), target) in self.references.iter().zip(targets) {
            let p = &self.ports[*i];
            let status = match target {
                Err(reason) => reason,
                Ok(j) if *i == j => "self reference",
                Ok(j) if !edges.contains(&(j, *i)) => "one-way transfer",
                Ok(j) => match (p.native, self.ports[j].native) {
                    (Some(_), Some(_)) if !p.static_contact || !self.ports[j].static_contact => "device transfer behavior not verified",
                    (Some(a), Some(b)) => {
                        let symbol = std::iter::once(&mut v.symbol)
                            .chain(v.devices.iter_mut().map(|d| &mut d.symbol))
                            .find(|s| s.key == p.symbol)
                            .expect("transfer belongs to this variant");
                        if !symbol.bridges.contains(&(a, b)) && !symbol.bridges.contains(&(b, a)) {
                            symbol.bridges.push((a.min(b), a.max(b)));
                        }
                        "native bridge"
                    }
                    _ => "unmapped endpoint",
                },
            };
            if status != "native bridge" {
                v.warnings.insert(format!("potential transfer: {status}"));
            }
            v.transfers.push(Transfer { object: p.object.clone(), slot: p.slot, label: p.label(), target: raw.clone(), status: status.into() });
        }
    }
}
