//! The navigator tabs: pages, devices, connections, cables, layers, check, review,
//! unplaced functions, strips / plugs and PLC (each a docked pane).

use super::*;

impl App {
    #[allow(clippy::too_many_lines)]
    /// One navigator tab (a docked pane), with the work area chooser above it.
    pub(super) fn navigator(&mut self, ui: &mut egui::Ui, idx: &Index, tab: LeftTab) {
        let area = self.session.prj.structures.work_area(&self.work_area).cloned();
        self.navigator_header(ui, tab);
        match tab {
            LeftTab::Pages => {
                ui.horizontal(|ui| {
                    self.icon_cmd(ui, Cmd::NewSchematicPage);
                    self.icon_cmd(ui, Cmd::NewSection);
                    self.icon_cmd(ui, Cmd::PageProperties);
                    let tree = self.page_tree;
                    if icons::toggle_button(ui, Icon::Tree, tr("Group the pages by structure"), tree).clicked() {
                        self.page_tree = !tree;
                    }
                    self.icon_cmd(ui, Cmd::MovePageUp);
                    self.icon_cmd(ui, Cmd::MovePageDown);
                    self.icon_cmd(ui, Cmd::RenumberPages);
                });
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut action = PageAction::None;
                    // Rows are dragged to move pages and sections, not in the list by structure.
                    let mut probe = if self.page_tree { page_dnd::Probe::off() } else { page_dnd::Probe::new(ui, &self.session.prj) };
                    page_dnd::Probe::scroll_at_edges(ui);
                    let shown: Vec<usize> = (0..self.session.prj.pages.len())
                        .filter(|&i| i == self.session.page || area.as_ref().is_none_or(|w| w.shows_page(&self.session.prj.pages[i])))
                        .collect();
                    if self.page_tree {
                        // Groups by structure, in the order they first occur.
                        let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
                        for &i in &shown {
                            let st = structure::page_structure(&self.session.prj.pages[i]);
                            match groups.iter_mut().find(|g| g.0 == st) {
                                Some(g) => g.1.push(i),
                                None => groups.push((st, vec![i])),
                            }
                        }
                        for (st, pages) in groups {
                            let title = if st.is_empty() { tr("(no structure)").to_string() } else { self.structure_title(&st) };
                            egui::CollapsingHeader::new(title).id_salt(("ptree", st.as_str())).default_open(true).show(ui, |ui| {
                                for i in pages {
                                    action = std::mem::replace(&mut action, PageAction::None).or(self.page_row(ui, i, &mut probe));
                                }
                            });
                        }
                    } else if !self.session.prj.sections.is_empty() {
                        action = self.section_tree(ui, &shown, &mut probe);
                        // While dragging: the row for the top level, after the sections.
                        let strip = probe.dragged.is_some().then(|| {
                            ui.add_space(4.0);
                            let r = ui.add(egui::Label::new(RichText::new(tr("(top level)")).weak()).selectable(false));
                            r.rect.expand2(egui::vec2(0.0, 2.0)).with_max_x(ui.clip_rect().right() - 2.0)
                        });
                        probe.below(strip);
                    } else {
                        for i in shown {
                            action = std::mem::replace(&mut action, PageAction::None).or(self.page_row(ui, i, &mut probe));
                        }
                        probe.below(None);
                    }
                    if let Some(a) = self.page_drop(ui, &probe) {
                        action = action.or(PageAction::Section(a));
                    }
                    let (goto, lock_action) = match action {
                        PageAction::Goto(i) => (Some(i), None),
                        PageAction::Properties(i) => {
                            self.page_props = Some(i);
                            (None, None)
                        }
                        PageAction::Lock(t, r) => (None, Some((t, r))),
                        PageAction::Table => {
                            self.open_table(openecraft_core::tables::TableKind::Pages, "");
                            (None, None)
                        }
                        PageAction::Section(a) => {
                            self.section_action(ui.ctx(), a);
                            (None, None)
                        }
                        PageAction::Run(i, cmd) => {
                            self.goto(i, 0);
                            self.run(&ui.ctx().clone(), cmd);
                            (None, None)
                        }
                        PageAction::None => (None, None),
                    };
                    if let Some(i) = goto {
                        self.goto(i, 0);
                    }
                    if let Some((target, release)) = lock_action {
                        if release {
                            self.save_remote(vec![target]);
                        } else {
                            self.remote.lock(vec![target]);
                        }
                    }
                });
            }
            LeftTab::Devices => {
                ui.add(egui::TextEdit::singleline(&mut self.device_filter).hint_text(tr("Filter devices…")));
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    let mut jump = None;
                    let f = self.device_filter.to_lowercase();
                    // Devices grouped by structure (=function+location).
                    let mut groups: Vec<(String, Vec<&String>)> = Vec::new();
                    let tags = idx.sorted_tags().into_iter().filter(|t| area.as_ref().is_none_or(|w| w.shows_device(t)));
                    for tag in tags.filter(|t| f.is_empty() || t.to_lowercase().contains(&f)) {
                        let st = Dt::parse(tag).structure();
                        match groups.iter_mut().find(|g| g.0 == st) {
                            Some(g) => g.1.push(tag),
                            None => groups.push((st, vec![tag])),
                        }
                    }
                    groups.sort_by(|a, b| natural_cmp(&a.0, &b.0));
                    let structured = groups.iter().any(|g| !g.0.is_empty());
                    let prj = &self.session.prj;
                    let placing = self.editor.place_tag.clone();
                    let selected = self.selected_tag();
                    let scroll = self.scroll_nav;
                    let mut menu: Option<(String, drawing_ui::DeviceAction)> = None;
                    let mut device = |ui: &mut egui::Ui, tag: &String| {
                        let dev = &idx.devices[tag];
                        let title = if structured { Dt::parse(tag).name } else { tag.clone() };
                        let title = if placing.as_deref() == Some(tag.as_str()) { format!("{title}  ✏") } else { title };
                        let is_sel = selected.as_deref() == Some(tag.as_str());
                        let mut text = RichText::new(title).monospace().strong();
                        if is_sel {
                            text = text.background_color(ui.visuals().selection.bg_fill);
                        }
                        let header = egui::CollapsingHeader::new(text).id_salt(tag).show(ui, |ui| {
                            for (kind, locs) in [(tr("Main"), &dev.mains), (tr("Aux"), &dev.auxs), ("", &dev.others)] {
                                for loc in locs {
                                    let inst = prj.pages[loc.page].symbols.iter().find(|s| s.id == loc.sym_id);
                                    let Some(inst) = inst else { continue };
                                    let name = self.session.prj.symbol(&inst.sym).map(|d| tr(d.name)).unwrap_or("?");
                                    let extra = if inst.label.is_empty() { inst.pins.join("-") } else { inst.label.clone() };
                                    let text = format!("{} {kind} {name} {extra}", loc.reference(prj));
                                    if ui.link(text.trim()).clicked() {
                                        jump = Some((loc.page, loc.sym_id));
                                    }
                                }
                            }
                        });
                        if is_sel && scroll {
                            header.header_response.scroll_to_me(None);
                        }
                        header.header_response.context_menu(|ui| {
                            if let Some(a) = drawing_ui::device_menu(ui, placing.as_deref() == Some(tag.as_str())) {
                                menu = Some((tag.clone(), a));
                            }
                        });
                    };
                    for (st, tags) in &groups {
                        if structured {
                            let title = if st.is_empty() { "(no structure)" } else { st.as_str() };
                            egui::CollapsingHeader::new(RichText::new(title).strong()).id_salt(("structure", st)).default_open(true).show(ui, |ui| {
                                for tag in tags {
                                    device(ui, tag);
                                }
                            });
                        } else {
                            for tag in tags {
                                device(ui, tag);
                            }
                        }
                    }
                    if idx.devices.is_empty() {
                        ui.weak(tr("No devices yet. Pick a symbol in the Insert center and click into the page."));
                    }
                    if let Some((tag, a)) = menu {
                        self.device_action(&tag, a);
                    }
                    if let Some((pg, id)) = jump {
                        self.goto(pg, id);
                    }
                });
            }
            LeftTab::Review => {
                egui::ScrollArea::vertical().id_salt("review_tab").auto_shrink([false, false]).show(ui, |ui| self.review_tab(ui));
            }
            LeftTab::Connections => self.connections_tab(ui),
            LeftTab::Cables => self.cables_tab(ui),
            LeftTab::Layers => self.layers_tab(ui),
            LeftTab::Check => self.check_tab(ui),
            LeftTab::Unplaced => self.unplaced_tab(ui),
            LeftTab::Plc => self.plc_tab(ui),
            LeftTab::Strips => self.strips_tab(ui),
        }
    }
}

impl App {
    /// Title of a navigator with its Edit in table and help buttons.
    fn navigator_header(&mut self, ui: &mut egui::Ui, tab: LeftTab) {
        use openecraft_core::tables::TableKind;
        let table = match tab {
            LeftTab::Pages => Some(TableKind::Pages),
            LeftTab::Devices => Some(TableKind::Devices),
            LeftTab::Connections => Some(TableKind::Connections),
            LeftTab::Cables => Some(TableKind::Cables),
            LeftTab::Plc => Some(TableKind::Plc),
            LeftTab::Strips => Some(TableKind::Terminals),
            _ => None,
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(self.pane_title(Pane::Navigator(tab)).text()).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(ui, Icon::Help, tr("Help")).clicked() {
                    self.help_for(Pane::Navigator(tab));
                }
                if let Some(k) = table {
                    if icons::button(ui, Icon::Table, tr("Edit in table…")).clicked() {
                        self.open_table(k, "");
                    }
                }
            });
        });
        ui.separator();
    }

    /// Opens Edit in table with a kind of rows (and a filter).
    pub(super) fn open_table(&mut self, kind: openecraft_core::tables::TableKind, filter: &str) {
        self.tables_ui.show(kind, filter);
        self.show_pane(Pane::Tool(ToolPane::Tables));
    }
}

/// What a row of the page list asks for.
pub(super) enum PageAction {
    None,
    Goto(usize),
    Properties(usize),
    Lock(openecraft_core::collab::Target, bool),
    Table,
    Section(sections_ui::SectionAction),
    /// A page command on this page (new page after it, delete, renumber).
    Run(usize, crate::commands::Cmd),
}

impl PageAction {
    pub(super) fn or(self, other: PageAction) -> PageAction {
        match self {
            PageAction::None => other,
            a => a,
        }
    }
}

impl App {
    /// `=A1+CAB1  Pump station` (descriptions from the structure management).
    fn structure_title(&self, st: &str) -> String {
        let dt = Dt::parse_structure(st);
        let s = &self.session.prj.structures;
        let desc: Vec<String> = openecraft_core::structure::Aspect::ALL
            .into_iter()
            .filter_map(|a| s.entry(a, dt.get(a)).map(|e| e.description.clone()))
            .filter(|d| !d.is_empty())
            .collect();
        if desc.is_empty() {
            st.to_string()
        } else {
            format!("{st}  {}", desc.join(", "))
        }
    }

    /// One page of the page list; it is dragged to move it between pages or onto a
    /// section.
    pub(super) fn page_row(&self, ui: &mut egui::Ui, i: usize, probe: &mut page_dnd::Probe) -> PageAction {
        let pg = &self.session.prj.pages[i];
        let icon = Self::page_icon(pg.kind);
        let mut text = egui::text::LayoutJob::default();
        let dragged = probe.dragged == Some(page_dnd::Dragged::Page(i));
        let color = if dragged { ui.visuals().weak_text_color() } else { ui.visuals().text_color() };
        let style = egui::TextFormat { color, ..Default::default() };
        let name = if self.page_tree { pg.name.clone() } else { structure::page_full_name(pg) };
        text.append(&format!("{icon} {name}   {}", pg.description), 0.0, style.clone());
        if let Some((mark, color)) = self.page_lock_mark(pg.id) {
            text.append(&mark, 8.0, egui::TextFormat { color, ..style });
        }
        let sectioned = !self.session.prj.sections.is_empty();
        let mut r = ui.selectable_label(i == self.session.page, text);
        if probe.enabled {
            r = r.interact(egui::Sense::drag());
            r.dnd_set_drag_payload(page_dnd::Dragged::Page(i));
            probe.page(&self.session.prj, r.rect, i);
        }
        if i == self.session.page && self.scroll_nav {
            r.scroll_to_me(None);
        }
        let mut action = PageAction::None;
        if r.clicked() {
            action = PageAction::Goto(i);
        }
        r.context_menu(|ui| {
            if ui.button(tr("Page properties…")).clicked() {
                action = PageAction::Properties(i);
                ui.close();
            }
            if ui.button(tr("Open in a new page tab")).clicked() {
                action = PageAction::Goto(i);
                ui.close();
            }
            if ui.button(tr("Edit in table…")).clicked() {
                action = PageAction::Table;
                ui.close();
            }
            ui.separator();
            use crate::commands::{def, Cmd};
            for (cmd, enabled) in [(Cmd::NewSchematicPage, true), (Cmd::RenumberPages, true), (Cmd::DeletePage, self.session.prj.pages.len() > 1 && !pg.locked)] {
                if ui.add_enabled(enabled, egui::Button::new(tr(def(cmd).title))).clicked() {
                    action = PageAction::Run(i, cmd);
                    ui.close();
                }
            }
            if sectioned {
                ui.menu_button(tr("Move to section"), |ui| {
                    let current = openecraft_core::sections::of_page(&self.session.prj, i);
                    for (id, title) in self.section_choices() {
                        if ui.add_enabled(id != current, egui::Button::new(title)).clicked() {
                            action = PageAction::Section(sections_ui::SectionAction::MovePage(i, id));
                            ui.close();
                        }
                    }
                });
            }
            if self.remote.shared.is_some() {
                ui.separator();
                if let Some((t, release)) = self.page_lock_menu(ui, pg.id) {
                    action = PageAction::Lock(t, release);
                }
            }
        });
        if r.double_clicked() {
            action = PageAction::Properties(i);
        }
        action
    }
}

impl App {
    /// Unplaced PLC channels and part functions, grouped by device; a click places one.
    fn unplaced_tab(&mut self, ui: &mut egui::Ui) {
        let prj = &self.session.prj;
        let channels = openecraft_core::unplaced::plc_channels(prj);
        let functions = openecraft_core::unplaced::device_functions(prj, Some(&self.parts_db));
        let potentials = openecraft_core::unplaced::potentials(prj);
        let mut start = None;
        ui.weak(tr("Drag an entry into the page, or click it and then click into the page."));
        egui::ScrollArea::vertical().id_salt("unplaced").auto_shrink([false, false]).show(ui, |ui| {
            for (title, list) in [(tr("PLC channels"), &channels), (tr("Functions of the parts"), &functions), (tr("Defined potentials"), &potentials)] {
                egui::CollapsingHeader::new(RichText::new(tr_fmt!("{} ({})", title, list.len())).strong()).id_salt(title).default_open(true).show(ui, |ui| {
                    let mut tags: Vec<&String> = list.iter().map(|u| &u.tag).collect();
                    tags.dedup();
                    for tag in tags {
                        let items: Vec<&openecraft_core::unplaced::Unplaced> = list.iter().filter(|u| &u.tag == tag).collect();
                        egui::CollapsingHeader::new(RichText::new(format!("{tag}  ({})", items.len())).monospace()).id_salt((title, tag)).show(ui, |ui| {
                            for u in items {
                                let payload = crate::editor::PlacePayload { symbol: u.symbol, tag: u.tag.clone(), label: u.label.clone(), pins: u.pins.clone() };
                                let r = super::drag_source(ui, ui.make_persistent_id(("unplaced", &u.tag, &u.title)), payload, |ui| ui.link(&u.title).on_hover_text(tr("Click, or drag it into the page")));
                                if r.clicked() {
                                    start = Some(u.clone());
                                }
                            }
                        });
                    }
                    if list.is_empty() {
                        ui.weak(tr("(no entries)"));
                    }
                });
            }
        });
        if let Some(u) = start {
            self.editor.set_tool(Tool::Place(u.symbol));
            self.editor.place_tag = Some(u.tag.clone());
            self.editor.place_once = Some((u.label, u.pins));
        }
    }

    /// PLC cards with their channels in address order: placed ones with function text
    /// and connected devices (a click shows the symbol), unplaced ones of cards with a
    /// device description to drag into the page.
    fn plc_tab(&mut self, ui: &mut egui::Ui) {
        let prj = &self.session.prj;
        let points = openecraft_core::plc::points(prj, &self.session.analysis.netlist);
        let unplaced = openecraft_core::unplaced::plc_channels(prj);
        let mut cards: Vec<String> = points.iter().map(|p| p.card.clone()).chain(unplaced.iter().map(|u| u.tag.clone())).collect();
        cards.sort_by(|a, b| openecraft_core::model::natural_cmp(a, b));
        cards.dedup();
        let mut jump = None;
        let mut start = None;
        egui::ScrollArea::vertical().id_salt("plc_nav").auto_shrink([false, false]).show(ui, |ui| {
            if cards.is_empty() {
                ui.weak(tr("No PLC cards in the project."));
            }
            for card in &cards {
                let placed: Vec<&openecraft_core::plc::Point> = points.iter().filter(|p| &p.card == card).collect();
                let open: Vec<&openecraft_core::unplaced::Unplaced> = unplaced.iter().filter(|u| &u.tag == card).collect();
                let mut rows: Vec<(openecraft_core::plc::AddressKey, Option<&openecraft_core::plc::Point>, Option<&openecraft_core::unplaced::Unplaced>)> = placed
                    .iter()
                    .map(|p| (openecraft_core::plc::address_key(&p.address), Some(*p), None))
                    .chain(open.iter().map(|u| (openecraft_core::plc::address_key(&u.label), None, Some(*u))))
                    .collect();
                rows.sort_by(|a, b| a.0.cmp(&b.0));
                let title = if open.is_empty() { format!("{card}  ({})", placed.len()) } else { tr_fmt!("{}  ({} placed, {} free)", card, placed.len(), open.len()) };
                egui::CollapsingHeader::new(RichText::new(title).monospace()).id_salt(("plc_card", card)).show(ui, |ui| {
                    for (_, p, u) in rows {
                        if let Some(p) = p {
                            let kind = p.io_kind();
                            let mut text = format!("{}  {kind}  {}", p.address, p.function_text);
                            if p.channel == p.address && !p.channel.is_empty() {
                                text = format!("{} ({})  {kind}  {}", p.channel, p.terminal, p.function_text);
                            } else if !p.channel.is_empty() {
                                text = format!("{} ({})  {text}", p.channel, p.terminal);
                            }
                            if !p.connected.is_empty() {
                                text = format!("{text}  › {}", p.connected);
                            }
                            if ui.selectable_label(false, text).on_hover_text(&p.location).clicked() {
                                jump = Some((p.page, p.id));
                            }
                        } else if let Some(u) = u {
                            let payload = crate::editor::PlacePayload { symbol: u.symbol, tag: u.tag.clone(), label: u.label.clone(), pins: u.pins.clone() };
                            let r = super::drag_source(ui, ui.make_persistent_id(("plc_free", &u.tag, &u.label)), payload, |ui| {
                                ui.add(egui::Label::new(RichText::new(format!("{}  –", u.label)).weak()).sense(egui::Sense::click())).on_hover_text(tr("Click, or drag it into the page"))
                            });
                            if r.clicked() {
                                start = Some(u.clone());
                            }
                        }
                    }
                });
            }
        });
        if let Some((page, id)) = jump {
            self.goto(page, id);
        }
        if let Some(u) = start {
            self.editor.set_tool(Tool::Place(u.symbol));
            self.editor.place_tag = Some(u.tag.clone());
            self.editor.place_once = Some((u.label, u.pins));
        }
    }

    /// Terminal strips (terminals in rail order, spares) and plugs (pins, unused pins of
    /// the plug parts): a click shows a placed one, spare ones are dragged into the page.
    fn strips_tab(&mut self, ui: &mut egui::Ui) {
        use openecraft_core::strips::{self, ElementKind};
        let prj = &self.session.prj;
        let mut jump = None;
        let mut start: Option<crate::editor::PlacePayload> = None;
        // Placed terminal symbols by strip and designation.
        let placed = |tag: &str, label: &str| {
            prj.pages.iter().enumerate().find_map(|(pi, pg)| {
                pg.symbols.iter().find(|s| s.tag == tag && s.label == label && prj.symbol(&s.sym).is_some_and(|d| d.func == openecraft_core::symbols::Func::Terminal)).map(|s| (pi, s.id))
            })
        };
        let free = |ui: &mut egui::Ui, id: (&str, &str, &str), payload: crate::editor::PlacePayload, text: String| -> bool {
            super::drag_source(ui, ui.make_persistent_id(id), payload, |ui| {
                ui.add(egui::Label::new(RichText::new(text).weak()).sense(egui::Sense::click())).on_hover_text(tr("Click, or drag it into the page"))
            })
            .clicked()
        };
        egui::ScrollArea::vertical().id_salt("strips_nav").auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new(tr("Terminal strips")).strong());
            let tags = strips::strip_tags(prj);
            if tags.is_empty() {
                ui.weak(tr("(no entries)"));
            }
            for tag in &tags {
                let layout = strips::strip_layout(prj, tag);
                let n = layout.iter().filter(|e| matches!(e.kind, ElementKind::Terminal(_) | ElementKind::Spare(_))).count();
                egui::CollapsingHeader::new(RichText::new(format!("{tag}  ({n})")).monospace()).id_salt(("strip_nav", tag)).show(ui, |ui| {
                    for e in &layout {
                        match &e.kind {
                            ElementKind::Terminal(label) => {
                                let levels = e.designations().join(" / ");
                                let text = if e.levels.is_empty() { label.clone() } else { levels };
                                match placed(tag, label) {
                                    Some(at) => {
                                        if ui.selectable_label(false, text).clicked() {
                                            jump = Some(at);
                                        }
                                    }
                                    None => {
                                        ui.weak(text);
                                    }
                                }
                            }
                            ElementKind::Spare(label) => {
                                let payload = crate::editor::PlacePayload { symbol: "term", tag: tag.clone(), label: label.clone(), pins: Vec::new() };
                                if free(ui, ("strip_free", tag, label), payload.clone(), tr_fmt!("{} (spare)", label)) {
                                    start = Some(payload);
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
            ui.add_space(6.0);
            ui.label(RichText::new(tr("Plugs")).strong());
            let plugs = openecraft_core::plugs::plug_tags(prj);
            if plugs.is_empty() {
                ui.weak(tr("(no entries)"));
            }
            for plug in &plugs {
                let rows = openecraft_core::plugs::rows(prj, plug, &self.session.analysis.netlist, &self.session.analysis.open);
                egui::CollapsingHeader::new(RichText::new(format!("{plug}  ({})", rows.len())).monospace()).id_salt(("plug_nav", plug)).show(ui, |ui| {
                    for r in &rows {
                        match r.pins.first() {
                            Some(p) => {
                                let targets = [r.socket.as_str(), r.pin.as_str()].iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join(" | ");
                                let text = if targets.is_empty() { r.label.clone() } else { format!("{}  › {targets}", r.label) };
                                if ui.selectable_label(false, text).clicked() {
                                    jump = Some((p.page, p.id));
                                }
                            }
                            None => {
                                let payload = crate::editor::PlacePayload { symbol: "plug_pair", tag: plug.clone(), label: r.label.clone(), pins: Vec::new() };
                                if free(ui, ("plug_free", plug, &r.label), payload.clone(), tr_fmt!("{} (unused)", r.label)) {
                                    start = Some(payload);
                                }
                            }
                        }
                    }
                });
            }
        });
        if let Some((page, id)) = jump {
            self.goto(page, id);
        }
        if let Some(pl) = start {
            self.editor.set_tool(Tool::Place(pl.symbol));
            self.editor.place_tag = Some(pl.tag);
            self.editor.place_once = Some((pl.label, pl.pins));
        }
    }
}
