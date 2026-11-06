//! The Insert center pane (see `crate::insert_center`): search, favorites, recently
//! used, and the tree of symbols, macros, devices and unplaced functions. A click starts
//! placing, a drag of an unplaced function places it at the drop point.

use super::*;
use crate::insert_center::Entry;
use openecraft_core::macros::MacroKind;

impl Entry {
    /// The key of a symbol entry, as the editor places it.
    fn symbol_key(&self, prj: &Project) -> Option<&'static str> {
        match self {
            Entry::Symbol(k) => prj.symbol(k).map(|d| d.key),
            _ => None,
        }
    }
}

/// What a row of the Insert center asks for.
enum Pick {
    Insert(Entry),
    Favorite(Entry),
    Variant(String, String),
    Unplaced(openecraft_core::unplaced::Unplaced),
}

/// The device list stops after this many parts (the search narrows it).
const DEVICES_SHOWN: usize = 200;

impl App {
    /// Registered symbols of the project symbol library, sorted by library and name.
    pub(super) fn user_symbols(&self) -> Vec<symbols::SymbolRef> {
        let mut v: Vec<symbols::SymbolRef> = self.session.prj.symbols.iter().filter_map(|u| self.session.prj.symbol(&u.key)).collect();
        v.sort_by(|a, b| a.library.cmp(b.library).then(natural_cmp(a.name, b.name)));
        v
    }

    /// Title, further texts to search and icon of an entry.
    fn entry_texts(&self, e: &Entry) -> Option<(String, String, Icon)> {
        match e {
            Entry::Symbol(k) => self.session.prj.symbol(k).map(|d| (tr(d.name).to_string(), format!("{} {} {}", d.name, d.letter, d.category.title()), Icon::Symbol)),
            Entry::Macro(n) => self.macro_lib.get(n).map(|m| (m.name.clone(), format!("{} {}", m.category, m.description), Icon::Macro)),
            Entry::Part(n) => parts::lookup(&self.session.prj, Some(&self.parts_db), n).map(|p| (p.number.clone(), format!("{} {} {}", p.description, p.manufacturer, p.group), Icon::Parts)),
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn insert_center_pane(&mut self, ui: &mut egui::Ui) {
        let mut pick: Option<Pick> = None;
        ui.horizontal(|ui| {
            icons::show(ui, Icon::Find, 16.0);
            let r = ui.add(egui::TextEdit::singleline(&mut self.insert_center.query).hint_text(tr("Search (* and ? as placeholders)…")).desired_width(ui.available_width() - 60.0));
            if r.changed() && !self.insert_center.query.is_empty() {
                self.lib_filter.clear();
            }
            let (icon, tip) = if self.insert_center.tiles { (Icon::List, tr("Show as list")) } else { (Icon::Grid, tr("Show as tiles")) };
            if icons::button(ui, icon, tip).clicked() {
                self.insert_center.tiles = !self.insert_center.tiles;
            }
            let r = icons::button(ui, Icon::Dots, tr("Favorites"));
            egui::Popup::menu(&r).show(|ui| {
                let ctx = ui.ctx().clone();
                if ui.button(tr("Export favorites…")).clicked() {
                    let json = self.insert_center.favorites_json();
                    self.export(&ctx, "favorites.json".into(), (tr("Favorites (JSON)"), "json"), json);
                    ui.close();
                }
                if ui.button(tr("Import favorites…")).clicked() {
                    self.io.open(&ctx, Purpose::Favorites);
                    ui.close();
                }
            });
        });
        let searching = !self.insert_center.query.trim().is_empty();
        egui::ScrollArea::vertical().id_salt("insert_center").auto_shrink([false, false]).show(ui, |ui| {
            if !searching {
                for (title, list) in [(tr("Favorites"), self.insert_center.favorites()), (tr("Recently used"), self.insert_center.recent())] {
                    if list.is_empty() {
                        continue;
                    }
                    ui.label(RichText::new(title).strong());
                    self.entry_tiles(ui, &list, &mut pick);
                    ui.separator();
                }
            }
            // Symbols: the user's libraries, then the built-in categories.
            let user = self.user_symbols();
            let mut libs: Vec<&str> = user.iter().map(|d| d.library).collect();
            libs.dedup();
            let groups = libs
                .iter()
                .map(|lib| (format!("{}: {lib}", Category::User.title()), user.iter().map(|d| &**d).filter(|d| d.library == *lib).collect::<Vec<_>>()))
                .chain(Category::ALL.into_iter().map(|cat| (cat.title().to_string(), symbols::library().iter().filter(|d| d.category == cat).collect())));
            egui::CollapsingHeader::new(RichText::new(tr("Symbols")).strong()).id_salt("ic_symbols").default_open(true).open(searching.then_some(true)).show(ui, |ui| {
                for (title, defs) in groups {
                    let defs: Vec<&symbols::SymbolDef> =
                        defs.into_iter().filter(|d| !searching || self.insert_center.matches(&[d.name, tr(d.name), d.letter, &title])).collect();
                    if defs.is_empty() {
                        continue;
                    }
                    // The extra libraries and the long ones start collapsed.
                    let main = !matches!(defs[0].category, Category::Nfpa | Category::Fluid | Category::Process | Category::SingleLine | Category::BlackBox | Category::Installation | Category::Gb | Category::Gost | Category::Jis) && defs.len() <= 24;
                    egui::CollapsingHeader::new(&title).id_salt(("ic_cat", &title)).default_open(main).open(searching.then_some(true)).show(ui, |ui| {
                        let list: Vec<Entry> = defs.iter().map(|d| Entry::Symbol(d.key.to_string())).collect();
                        self.entry_rows(ui, &list, &mut pick);
                    });
                }
            });
            // Macros of the macro library.
            let macros: Vec<Entry> = self
                .macro_lib
                .macros
                .iter()
                .filter(|m| !searching || self.insert_center.matches(&[&m.name, &m.category, &m.description]))
                .map(|m| Entry::Macro(m.name.clone()))
                .collect();
            egui::CollapsingHeader::new(RichText::new(tr_fmt!("Macros ({})", macros.len())).strong()).id_salt("ic_macros").open(searching.then_some(!macros.is_empty())).show(ui, |ui| {
                self.entry_rows(ui, &macros, &mut pick);
                if macros.is_empty() {
                    ui.weak(tr("(no entries)"));
                }
            });
            // Devices: parts with function templates (the first function is placed).
            let mut devices: Vec<Entry> = Vec::new();
            let mut found = 0;
            for p in self.session.prj.parts.iter().chain(self.parts_db.parts.iter()) {
                if p.functions.is_empty() && p.eplan_variants.is_empty() {
                    continue;
                }
                if searching && !self.insert_center.matches(&[&p.number, &p.description, &p.manufacturer, &p.group]) {
                    continue;
                }
                let e = Entry::Part(p.number.clone());
                if !devices.contains(&e) {
                    found += 1;
                    if devices.len() < DEVICES_SHOWN {
                        devices.push(e);
                    }
                }
            }
            egui::CollapsingHeader::new(RichText::new(tr_fmt!("Devices ({})", found)).strong())
                .id_salt("ic_devices")
                .open(searching.then_some(!devices.is_empty()))
                .show(ui, |ui| {
                    ui.weak(tr("Parts with functions: the device gets the part, its first function is placed."));
                    self.entry_rows(ui, &devices, &mut pick);
                    if let Some(part) = self.insert_center.variant_part.as_ref().and_then(|n| parts::lookup(&self.session.prj, Some(&self.parts_db), n)) {
                        ui.separator();
                        ui.strong(&part.number);
                        for v in &part.eplan_variants {
                            let Some(key) = v["symbol"]["key"].as_str() else {
                                ui.weak(v.to_string());
                                continue;
                            };
                            ui.horizontal(|ui| {
                                let preview = symbol_icon(ui, &self.session.prj, key, egui::vec2(64.,48.));
                                let title = v["symbol"]["name"].as_str().unwrap_or(key);
                                let caption = v["symbol"]["plc_channel"]["designation"].as_str().map(|channel| format!("{title} · {channel}"));
                                let button = ui.button(caption.as_deref().unwrap_or(title)).on_hover_text(v["description"].as_str().unwrap_or(""));
                                if preview.clicked() || button.clicked() {
                                    pick = Some(Pick::Variant(part.number.clone(), key.to_owned()));
                                }
                            });
                            if let Some(warnings) = v["warnings"].as_array().filter(|list| !list.is_empty()) {
                                ui.weak(warnings.iter().filter_map(|w| w.as_str()).collect::<Vec<_>>().join(", "));
                            }
                            if let Some(missing)=v["unresolved"].as_array().filter(|refs|!refs.is_empty()){
                                egui::CollapsingHeader::new(tr_fmt!("Unresolved placements ({})",missing.len())).id_salt((&part.number,key,"unresolved")).show(ui,|ui|{
                                    for r in missing {
                                        let master=r["master"].as_str().unwrap_or("");
                                        let designations=r["designations"].as_array().into_iter().flatten().filter_map(|p|p.as_str()).collect::<Vec<_>>().join("; ");
                                        ui.horizontal(|ui|{
                                            if let Some((lib_id,_))=master.rsplit_once('/') {
                                                let review=format!("user:pdf-review:{}",lib_id.replace('/',":"));
                                                if self.session.prj.symbol(&review).is_some(){
                                                    symbol_icon(ui,&self.session.prj,&review,egui::vec2(48.,32.)).on_hover_text(tr("Reference artwork without pin anchors"));
                                                }
                                            }
                                            ui.label(format!("{}  {}  {}  {}",master,r["object"].as_str().unwrap_or(""),r["plug"].as_str().unwrap_or(""),designations)).on_hover_text(r["reason"].as_str().unwrap_or(""));
                                        });
                                    }
                                });
                            }
                        }
                    }
                    if found > devices.len() {
                        ui.colored_label(crate::style::warning(), tr_fmt!("{} of {} devices shown – refine the search", devices.len(), found));
                    }
                });
            // Unplaced functions of the project.
            let prj = &self.session.prj;
            let mut unplaced = openecraft_core::unplaced::device_functions(prj, Some(&self.parts_db));
            unplaced.extend(openecraft_core::unplaced::plc_channels(prj));
            unplaced.extend(openecraft_core::unplaced::potentials(prj));
            unplaced.retain(|u| !searching || self.insert_center.matches(&[&u.tag, &u.title, &u.label]));
            egui::CollapsingHeader::new(RichText::new(tr_fmt!("Unplaced ({})", unplaced.len())).strong())
                .id_salt("ic_unplaced")
                .open(searching.then_some(!unplaced.is_empty()))
                .show(ui, |ui| {
                    ui.weak(tr("Drag an entry into the page, or click it and then click into the page."));
                    for u in unplaced {
                        let payload = crate::editor::PlacePayload { symbol: u.symbol, tag: u.tag.clone(), label: u.label.clone(), pins: u.pins.clone() };
                        let r = super::drag_source(ui, ui.make_persistent_id(("ic_unplaced", &u.tag, &u.title)), payload, |ui| ui.link(format!("{}  {}", u.tag, u.title)));
                        if r.clicked() {
                            pick = Some(Pick::Unplaced(u));
                        }
                    }
                });
        });
        match pick {
            Some(Pick::Insert(e)) => self.insert_entry(e),
            Some(Pick::Favorite(e)) => self.insert_center.toggle_favorite(&e),
            Some(Pick::Variant(number,key)) => self.arm_part(&number,Some(&key)),
            Some(Pick::Unplaced(u)) => {
                self.set_tool(Tool::Place(u.symbol));
                self.editor.place_tag = Some(u.tag.clone());
                self.editor.place_once = Some((u.label, u.pins));
            }
            None => {}
        }
    }

    /// Entries as rows with a small preview; right click or the star adds a favorite.
    fn entry_rows(&self, ui: &mut egui::Ui, list: &[Entry], pick: &mut Option<Pick>) {
        if self.insert_center.tiles {
            self.entry_tiles(ui, list, pick);
            return;
        }
        let active = match &self.editor.tool {
            Tool::Place(k) => Some(Entry::Symbol(k.to_string())),
            _ => None,
        };
        for e in list {
            let Some((title, _, icon)) = self.entry_texts(e) else { continue };
            ui.horizontal(|ui| {
                let preview = match e {
                    Entry::Symbol(k) => symbol_icon(ui, &self.session.prj, k, egui::vec2(36.0, 26.0)),
                    _ => icons::show(ui, icon, 22.0).interact(egui::Sense::click()),
                };
                // Symbols: the identifying letter as a badge; parts and macros: the
                // description. The whole text shows on hover (the pane may clip it).
                let (letter, about) = match e {
                    Entry::Symbol(k) => (self.session.prj.symbol(k).map(|d| d.letter.to_string()).unwrap_or_default(), String::new()),
                    Entry::Part(n) => (String::new(), parts::lookup(&self.session.prj, Some(&self.parts_db), n).map(|p| p.description.clone()).unwrap_or_default()),
                    Entry::Macro(n) => (String::new(), self.macro_lib.get(n).map(|m| m.description.clone()).unwrap_or_default()),
                };
                let full = if about.is_empty() { title.clone() } else { format!("{title} – {about}") };
                let text = title.clone();
                let selected = active.as_ref() == Some(e);
                // Star and letter stay at the right edge; a long title is shortened
                // (the whole text shows on hover).
                let fav = self.insert_center.is_favorite(e);
                let (lbl, star) = ui
                    .with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let star = icons::toggle_button(ui, if fav { Icon::StarFilled } else { Icon::Star }, tr("Favorite"), fav);
                        if !letter.is_empty() {
                            ui.label(RichText::new(&letter).small().monospace().weak()).on_hover_text(tr_fmt!("Identifying letter: {}", letter));
                        }
                        let lbl = ui
                            .with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                let label = |ui: &mut egui::Ui| ui.add(egui::Button::selectable(selected, text.as_str()).truncate());
                                // Symbols can be dragged into the page as well.
                                let lbl = match e.symbol_key(&self.session.prj) {
                                    Some(symbol) => {
                                        let payload = crate::editor::PlacePayload { symbol, tag: String::new(), label: String::new(), pins: Vec::new() };
                                        super::drag_source(ui, ui.make_persistent_id(("ic_drag", symbol)), payload, label)
                                    }
                                    None => label(ui),
                                };
                                if letter.is_empty() && !about.is_empty() {
                                    ui.add(egui::Label::new(RichText::new(&about).small().weak()).truncate());
                                }
                                lbl
                            })
                            .inner;
                        (lbl.on_hover_text(&full), star)
                    })
                    .inner;
                if preview.clicked() || lbl.clicked() {
                    *pick = Some(Pick::Insert(e.clone()));
                }
                if star.clicked() {
                    *pick = Some(Pick::Favorite(e.clone()));
                }
                lbl.context_menu(|ui| {
                    if ui.button(if fav { tr("Remove from favorites") } else { tr("Add to favorites") }).clicked() {
                        *pick = Some(Pick::Favorite(e.clone()));
                        ui.close();
                    }
                });
            });
        }
    }

    /// Entries as tiles (favorites, recently used, tile view).
    fn entry_tiles(&self, ui: &mut egui::Ui, list: &[Entry], pick: &mut Option<Pick>) {
        ui.horizontal_wrapped(|ui| {
            for e in list {
                let Some((title, _, icon)) = self.entry_texts(e) else { continue };
                ui.vertical(|ui| {
                    ui.set_width(64.0);
                    let r = match e {
                        Entry::Symbol(k) => symbol_icon(ui, &self.session.prj, k, egui::vec2(56.0, 40.0)),
                        _ => icons::show(ui, icon, 40.0).interact(egui::Sense::click()),
                    };
                    // The name wraps under the tile; the whole name shows on hover.
                    ui.add(egui::Label::new(RichText::new(&title).small()).wrap()).on_hover_text(&title);
                    let r = r.on_hover_text(&title);
                    if r.clicked() {
                        *pick = Some(Pick::Insert(e.clone()));
                    }
                    let fav = self.insert_center.is_favorite(e);
                    r.context_menu(|ui| {
                        if ui.button(if fav { tr("Remove from favorites") } else { tr("Add to favorites") }).clicked() {
                            *pick = Some(Pick::Favorite(e.clone()));
                            ui.close();
                        }
                    });
                });
            }
        });
    }

    /// Starts placing an entry.
    fn insert_entry(&mut self, e: Entry) {
        match &e {
            Entry::Symbol(k) => {
                let Some(key) = self.session.prj.symbol(k).map(|d| d.key) else { return };
                self.set_tool(Tool::Place(key));
                self.editor.place_rot = 0;
                self.editor.place_mirror = false;
            }
            Entry::Macro(n) => {
                let Some(m) = self.macro_lib.get(n).cloned() else { return };
                let values = Default::default();
                if m.kind == MacroKind::Page {
                    let at = openecraft_core::macros::insert_page(&mut self.session.prj, self.session.page, &m, &values);
                    self.goto(at, 0);
                } else {
                    self.set_tool(Tool::Insert);
                    self.editor.insert = Some(m.resolve(&values));
                    self.editor.insert_macro = Some((m, values));
                }
            }
            Entry::Part(n) => {
                let Some(p) = parts::lookup(&self.session.prj, Some(&self.parts_db), n).cloned() else { return };
                if p.eplan_variants.is_empty() {
                    self.arm_part(n,None);
                } else {
                    self.insert_center.variant_part = Some(n.clone());
                    for data in p.eplan_variants {
                        if let Ok(v) = serde::Deserialize::deserialize(data.as_ref()) {
                            openecraft_core::symlib::ema::install(&mut self.session.prj.symbols,&[v]);
                        }
                    }
                }
            }
        }
        self.insert_center.used(&e);
    }

    fn arm_part(&mut self, number: &str, key: Option<&str>) {
        let Some(p) = parts::lookup(&self.session.prj, Some(&self.parts_db), number).cloned() else { return };
        if let Some(v) = p.eplan_variants.iter().find(|v| key.is_some() && v["symbol"]["key"].as_str() == key && v["devices"].is_array()) {
            if let Ok(variant) = serde::Deserialize::deserialize(v.as_ref()) {
                if let Some(m) = openecraft_core::macros::eplan_window(&variant) {
                    openecraft_core::symlib::ema::install(&mut self.session.prj.symbols, &[variant]);
                    let values = Default::default();
                    self.set_tool(Tool::Insert);
                    self.editor.insert = Some(m.resolve(&values));
                    self.editor.insert_macro = Some((m, values));
                    return;
                }
            }
        }
        let choice = openecraft_core::symlib::ema::part_symbol(&mut self.session.prj, &p, key);
        let Some((sym, pins)) = choice.or_else(|| if key.is_none() { openecraft_core::unplaced::part_symbol(&p) } else { None }) else { return };
        self.set_tool(Tool::Place(sym));
        self.editor.place_once = Some((String::new(), pins));
        self.editor.place_part = Some(number.into());
    }
}
