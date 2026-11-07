//! The property editor (pane and window, F2): the selected symbol in tabs (device,
//! display, parts, properties, connections), the wires, texts and graphics, the common
//! fields of a multiple selection, or the page.

use super::*;

/// The tabs of a symbol's properties.
/// What the Belongs to device field does with the text typed when it loses the focus.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum TagEdit {
    /// Nothing changes: the same tag, Esc, or an empty field.
    Keep,
    /// The function moves to the device with this complete tag.
    Move(String),
}

/// The text `typed` (relative to the page structure `here`) for a function of the device
/// `current`; `cancelled` when Esc ended the input. An empty tag is not applied.
pub(super) fn tag_edit(typed: &str, current: &str, here: &Dt, cancelled: bool) -> TagEdit {
    let typed = typed.trim();
    if cancelled || typed.is_empty() {
        return TagEdit::Keep;
    }
    let tag = here.complete(typed);
    if tag == current { TagEdit::Keep } else { TagEdit::Move(tag) }
}

/// The note below the field while typing: an empty tag (error), or the device the
/// function moves to.
fn tag_note_of(typed: &str, current: &str, here: &Dt, idx: &Index) -> Option<(String, bool)> {
    if typed.trim().is_empty() {
        return Some((tr("Enter a device tag – an empty tag is not applied.").to_string(), true));
    }
    match tag_edit(typed, current, here, false) {
        TagEdit::Keep => None,
        TagEdit::Move(tag) if idx.devices.contains_key(&tag) => Some((tr_fmt!("Moves this function to the existing device {}", tag), false)),
        TagEdit::Move(tag) => Some((tr_fmt!("Moves this function to the new device {}", tag), false)),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum PropTab {
    #[default]
    Device,
    Display,
    Parts,
    Properties,
    Connections,
}

impl PropTab {
    fn title(self) -> &'static str {
        match self {
            PropTab::Device => tr("Device"),
            PropTab::Display => tr("Display"),
            PropTab::Parts => tr("Parts"),
            PropTab::Properties => tr("Properties"),
            PropTab::Connections => tr("Connections"),
        }
    }
}

impl App {
    /// F2 and a double click: the Properties pane when it is shown in the workspace
    /// (with a frame until the next click), otherwise a window beside the pointer.
    pub(super) fn open_properties(&mut self, ctx: &egui::Context) {
        if crate::dock::visible(&self.dock).contains(&Pane::Properties) {
            self.focus_pane = Some(Pane::Properties);
            return;
        }
        if !self.props_window {
            let screen = ctx.content_rect();
            let size = egui::vec2(380.0, 520.0);
            self.props_at = match ctx.pointer_latest_pos().filter(|p| screen.contains(*p)) {
                // Beside the pointer (the double-clicked symbol), inside the window.
                Some(p) => egui::pos2((p.x + 32.0).min(screen.right() - size.x - 8.0).max(screen.left()), (p.y - 40.0).min(screen.bottom() - size.y - 8.0).max(screen.top() + 40.0)),
                None => screen.center() - size / 2.0,
            };
        }
        self.props_window = true;
    }

    /// The properties of the selection in a window beside the page (F2, double click).
    pub(super) fn properties_float_window(&mut self, ctx: &egui::Context) {
        if !self.props_window {
            return;
        }
        let idx = std::mem::take(&mut self.session.index);
        let (window, esc) = self.dialogs.window(ctx, tr("Properties"), "properties", Some(self.props_at));
        let mut open = !esc;
        window.open(&mut open).default_size([380.0, 520.0]).show(ctx, |ui| {
            egui::ScrollArea::vertical().id_salt("props_window").auto_shrink([false, false]).show(ui, |ui| self.properties(ui, &idx));
        });
        self.session.index = idx;
        self.props_window = open;
    }

    /// The tab bar of a symbol's properties; returns the tab shown.
    fn props_tab_bar(&mut self, ui: &mut egui::Ui, device: bool, pins: bool, display: bool) -> PropTab {
        let tabs: Vec<PropTab> = [
            (PropTab::Device, true),
            (PropTab::Display, display),
            (PropTab::Parts, device),
            (PropTab::Properties, true),
            (PropTab::Connections, pins || device),
        ]
        .into_iter()
        .filter(|(_, on)| *on)
        .map(|(t, _)| t)
        .collect();
        if !tabs.contains(&self.props_tab) {
            self.props_tab = PropTab::Device;
        }
        // Compact tabs in one row; a narrow pane scrolls them instead of wrapping.
        egui::ScrollArea::horizontal().id_salt("prop_tabs").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.spacing_mut().button_padding.x = 4.0;
                for t in tabs {
                    ui.selectable_value(&mut self.props_tab, t, RichText::new(t.title()).small());
                }
            });
        });
        ui.separator();
        self.props_tab
    }

    /// The fields the selected symbols share: a value that differs shows "(several)".
    fn common_symbol_fields(&mut self, ui: &mut egui::Ui, pi: usize) {
        let syms: Vec<usize> = self.session.prj.pages[pi].symbols.iter().enumerate().filter(|(_, s)| self.editor.selection.contains(&s.id)).map(|(i, _)| i).collect();
        if syms.len() < 2 {
            return;
        }
        ui.separator();
        ui.label(tr_fmt!("{} symbols", syms.len()));
        let page = &self.session.prj.pages[pi];
        let first = page.symbols[syms[0]].function_text.clone();
        let same = syms.iter().all(|&i| page.symbols[i].function_text == first);
        let key = ui.scope_id().with("common_function_text");
        let mut text: String = ui.data_mut(|d| d.get_temp(key)).unwrap_or_else(|| if same { first.clone() } else { String::new() });
        let mirror_all = syms.iter().all(|&i| page.symbols[i].mirror);
        egui::Grid::new("common_props").num_columns(2).show(ui, |ui| {
            ui.label(tr("Function text"));
            let r = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2).desired_width(160.0).hint_text(if same { "" } else { tr("(several)") }));
            if r.changed() {
                for &i in &syms {
                    self.session.prj.pages[pi].symbols[i].function_text = text.clone();
                }
            }
            ui.end_row();
            ui.label(tr("Mirror"));
            let mut m = mirror_all;
            if ui.checkbox(&mut m, if mirror_all || syms.iter().all(|&i| !self.session.prj.pages[pi].symbols[i].mirror) { "" } else { tr("(several)") }).changed() {
                for &i in &syms {
                    self.session.prj.pages[pi].symbols[i].mirror = m;
                }
            }
            ui.end_row();
        });
        ui.data_mut(|d| d.insert_temp(key, text));
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn properties(&mut self, ui: &mut egui::Ui, idx: &Index) {
        // The pane's tab and the window are titled Properties already.
        let sel: Vec<u64> = self.editor.selection.iter().copied().collect();
        let pi = self.session.page;
        let read_only = if self.session.prj.pages[pi].locked {
            Some(tr("Released page – read only").to_string())
        } else {
            self.page_locked_by().map(|user| tr_fmt!("Locked by {} – read only", user))
        };
        if let Some(text) = read_only {
            ui.horizontal(|ui| {
                icons::show(ui, Icon::Lock, 14.0);
                ui.label(RichText::new(text).color(crate::style::lock()));
            });
            ui.disable();
        }
        if sel.is_empty() {
            let pg = &mut self.session.prj.pages[pi];
            ui.label(RichText::new(tr_fmt!("Page {} – {}", structure::page_full_name(pg), pg.kind.label())).strong());
            ui.label(tr("Description"));
            ui.text_edit_singleline(&mut pg.description);
            if ui.link(tr("Structure (function / location)…")).on_hover_text(tr("Page properties")).clicked() {
                self.page_props = Some(pi);
            }
            ui.add_space(6.0);
            ui.weak(self.editor.hint());
            return;
        }
        if sel.len() > 1 {
            ui.label(tr_fmt!("{} objects selected", sel.len()));
            ui.horizontal(|ui| {
                for c in [Cmd::Rotate, Cmd::Mirror, Cmd::Delete, Cmd::Align, Cmd::Distribute] {
                    if crate::commands::def(c).kind == crate::commands::Kind::Menu {
                        let r = icons::button(ui, crate::commands::def(c).icon, tr(crate::commands::def(c).title));
                        egui::Popup::menu(&r).show(|ui| self.cmd_menu(ui, c));
                    } else {
                        self.icon_cmd(ui, c);
                    }
                }
            });
            self.common_symbol_fields(ui, pi);
            let wires: Vec<u64> = self.session.prj.pages[pi].wires.iter().filter(|w| self.editor.selection.contains(&w.id)).map(|w| w.id).collect();
            if !wires.is_empty() {
                ui.separator();
                ui.label(super::counted(wires.len(), tr("1 wire"), tr_fmt!("{} wires", wires.len())));
                let first = self.session.prj.pages[pi].wires.iter().find(|w| w.id == wires[0]).cloned().unwrap_or_default();
                let (mut color, mut cs) = (first.color.clone(), first.cross_section.clone());
                let auto = potentials::props_of_wire(&self.session.prj, &self.session.analysis.netlist, &self.session.analysis.open, first.id);
                egui::Grid::new("wprops").num_columns(2).show(ui, |ui| {
                    potentials_ui::wire_fields(ui, &mut color, &mut cs, &auto);
                });
                if color != first.color || cs != first.cross_section {
                    for w in self.session.prj.pages[pi].wires.iter_mut().filter(|w| wires.contains(&w.id)) {
                        w.color = color.clone();
                        w.cross_section = cs.clone();
                    }
                }
                let cables: Vec<String> = self.session.prj.cables.iter().map(|c| c.tag.clone()).collect();
                let mut assign = None;
                ui.horizontal(|ui| {
                    ui.menu_button(tr("Assign to cable"), |ui| {
                        for c in &cables {
                            if ui.button(c).clicked() {
                                assign = Some(c.clone());
                            }
                        }
                        if cables.is_empty() {
                            ui.weak(tr("No cables yet – Connections › Cables"));
                        }
                    })
                    .response
                    .on_hover_text(tr("The wires get the free cores of the cable in order (left to right)"));
                    if ui.button(tr("Remove from cable")).clicked() {
                        for w in self.session.prj.pages[pi].wires.iter_mut().filter(|w| wires.contains(&w.id)) {
                            w.cable.clear();
                            w.core.clear();
                        }
                    }
                });
                let harnesses: Vec<String> = self.session.prj.harnesses.iter().map(|h| h.tag.clone()).collect();
                let mut to_harness = None;
                ui.horizontal(|ui| {
                    ui.menu_button(tr("Assign to harness"), |ui| {
                        for h in &harnesses {
                            if ui.button(h).clicked() {
                                to_harness = Some(h.clone());
                            }
                        }
                        if harnesses.is_empty() {
                            ui.weak(tr("No harnesses yet – Connections › Harnesses"));
                        }
                    })
                    .response
                    .on_hover_text(tr("Connections whose two ends are both on selected wires"));
                    if ui.button(tr("Remove from harness")).clicked() {
                        to_harness = Some(String::new());
                    }
                });
                if let Some(tag) = to_harness {
                    let sel: std::collections::BTreeSet<u64> = wires.iter().copied().collect();
                    let conns = openecraft_core::harness::connections_at(&self.session.prj, &self.session.analysis, pi, &sel);
                    let ends: Vec<(String, String)> = conns
                        .iter()
                        .map(|&i| {
                            let c = &self.session.analysis.connections[i];
                            (pin_designation(&self.session.prj, c.a), pin_designation(&self.session.prj, c.b))
                        })
                        .collect();
                    for (a, b) in &ends {
                        openecraft_core::harness::assign(&mut self.session.prj, a, b, &tag);
                    }
                    let ctx = ui.ctx().clone();
                    self.notify(&ctx, if tag.is_empty() { tr_fmt!("{} connections removed from their harness", ends.len()) } else { tr_fmt!("{} connections assigned to {}", ends.len(), tag) });
                }
                if let Some(c) = assign {
                    let sel: std::collections::BTreeSet<u64> = wires.iter().copied().collect();
                    let n = ops::assign_cores(&mut self.session.prj, pi, &sel, &c);
                    let ctx = ui.ctx().clone();
                    self.notify(&ctx, tr_fmt!("{} of {} wires assigned to cores of {}", n, wires.len(), c));
                }
            }
            return;
        }
        let id = sel[0];
        let page = &self.session.prj.pages[pi];
        if let Some(si) = page.symbols.iter().position(|s| s.id == id) {
            self.symbol_properties(ui, idx, pi, si);
        } else if self.session.analysis.open.net_of_wire(id).is_some_and(|n| n.fluid && !n.mixed) && page.wires.iter().any(|w| w.id == id) {
            self.hose_properties(ui, id);
        } else if let Some(wi) = page.wires.iter().position(|w| w.id == id) {
            let net = self.session.analysis.netlist.net_of_wire(id);
            if let Some(net) = net {
                ui.label(RichText::new(tr_fmt!("Potential: {}", net.name)).strong());
                ui.label(tr("Connected to:"));
                let mut v: Vec<String> = net.pins.iter().map(|p| pin_designation(&self.session.prj, *p)).collect();
                v.sort_by(|a, b| natural_cmp(a, b));
                v.dedup();
                ui.label(RichText::new(v.join(", ")).monospace());
                ui.weak(tr("All wires of this potential are highlighted."));
            }
            ui.separator();
            let open_net = self.session.analysis.open.wire_net.get(&id).map(|&n| self.session.analysis.open.nets[n].clone());
            let number_now = open_net.as_ref().map(|n| wirenum::number_of(&self.session.prj, n)).unwrap_or_default();
            let mut number = number_now.clone();
            let here = Dt::of_page(&self.session.prj.pages[pi]);
            let cables: Vec<(String, Vec<String>)> = self.session.prj.cables.iter().map(|c| (c.tag.clone(), c.core_list())).collect();
            // Cores of the wire's cable already used by other connections
            let own: Vec<u64> = open_net.as_ref().map(|n| n.wires.iter().map(|w| w.1).collect()).unwrap_or_default();
            let cable_now = self.session.prj.pages[pi].wires[wi].cable.clone();
            let used: Vec<String> = self
                .session.prj
                .pages
                .iter()
                .flat_map(|pg| pg.wires.iter())
                .filter(|x| x.cable == cable_now && !cable_now.is_empty() && !own.contains(&x.id))
                .map(|x| x.core.clone())
                .collect();
            let harnesses: Vec<String> = self.session.prj.harnesses.iter().map(|h| h.tag.clone()).collect();
            // Connections of this wire's potential section, each with its own harness.
            let conns: Vec<(String, String, String)> = openecraft_core::harness::connections_of_wire(&self.session.analysis, id)
                .into_iter()
                .map(|i| {
                    let c = &self.session.analysis.connections[i];
                    (pin_designation(&self.session.prj, c.a), pin_designation(&self.session.prj, c.b), c.harness.clone())
                })
                .collect();
            let mut harness_change = None;
            let auto = potentials::props_of_wire(&self.session.prj, &self.session.analysis.netlist, &self.session.analysis.open, id);
            let w = &mut self.session.prj.pages[pi].wires[wi];
            egui::Grid::new("wprops1").num_columns(2).show(ui, |ui| {
                ui.label(tr("Wire number"));
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut number).hint_text(tr("not numbered")).desired_width(90.0))
                        .on_hover_text(tr("Applies to all wires of this connection; automatic numbering keeps manual numbers"));
                    if w.number_fixed {
                        ui.weak(tr("manual"));
                    }
                });
                ui.end_row();
                ui.label(tr("Labels"));
                ui.checkbox(&mut w.hide_labels, tr("Hide automatic wire and cable labels"));
                ui.end_row();
                ui.label(tr("Cable"));
                let shown = if w.cable.is_empty() { "(none)".to_string() } else { here.relative(&w.cable) };
                egui::ComboBox::from_id_salt("wcable").selected_text(shown).show_ui(ui, |ui| {
                    if ui.selectable_label(w.cable.is_empty(), "(none)").clicked() {
                        w.cable.clear();
                        w.core.clear();
                    }
                    for (tag, _) in &cables {
                        if ui.selectable_label(w.cable == *tag, tag).clicked() && w.cable != *tag {
                            w.cable = tag.clone();
                            w.core.clear();
                        }
                    }
                });
                ui.end_row();
                if let Some((_, cores)) = cables.iter().find(|(t, _)| *t == w.cable) {
                    ui.label(tr("Core"));
                    egui::ComboBox::from_id_salt("wcore").selected_text(w.core.clone()).show_ui(ui, |ui| {
                        for c in cores {
                            let text = if used.contains(c) { tr_fmt!("{}  (used)", c) } else { c.clone() };
                            ui.selectable_value(&mut w.core, c.clone(), text);
                        }
                    });
                    ui.end_row();
                }
                potentials_ui::wire_fields(ui, &mut w.color, &mut w.cross_section, &auto);
                ui.label(tr("From"));
                ui.label(format!("{:.0} / {:.0}", w.a.x, w.a.y));
                ui.end_row();
                ui.label(tr("To"));
                ui.label(format!("{:.0} / {:.0}", w.b.x, w.b.y));
                ui.end_row();
            });
            if number != number_now {
                if let Some(net) = &open_net {
                    wirenum::set(&mut self.session.prj, net, number.trim(), true);
                }
            }
            if !conns.is_empty() {
                ui.separator();
                ui.label(RichText::new(tr("Connections / harness")).strong()).on_hover_text(tr("Each point-to-point connection can belong to a harness"));
                egui::Grid::new("wharness").num_columns(2).show(ui, |ui| {
                    for (i, (a, b, h)) in conns.iter().enumerate() {
                        ui.label(RichText::new(format!("{a} – {b}")).monospace());
                        let shown = if h.is_empty() { "(none)".to_string() } else { here.relative(h) };
                        egui::ComboBox::from_id_salt(("wharness", i)).selected_text(shown).show_ui(ui, |ui| {
                            if ui.selectable_label(h.is_empty(), "(none)").clicked() {
                                harness_change = Some((a.clone(), b.clone(), String::new()));
                            }
                            for t in &harnesses {
                                if ui.selectable_label(t == h, t).clicked() {
                                    harness_change = Some((a.clone(), b.clone(), t.clone()));
                                }
                            }
                        });
                        ui.end_row();
                    }
                });
            }
            if let Some((a, b, tag)) = harness_change {
                openecraft_core::harness::assign(&mut self.session.prj, &a, &b, &tag);
            }
        } else {
            self.graphic_properties(ui, id);
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(super) fn symbol_properties(&mut self, ui: &mut egui::Ui, idx: &Index, pi: usize, si: usize) {
        let s = self.session.prj.pages[pi].symbols[si].clone();
        let Some(def) = self.session.prj.symbol(&s.sym) else {
            ui.label(tr_fmt!("Unknown symbol {}", s.sym));
            return;
        };
        ui.label(RichText::new(tr(def.name)).strong());
        let device = def.func.has_tag() && !s.tag.is_empty();
        let tab = self.props_tab_bar(ui, device, !def.pins.is_empty(), def.show_tag);
        let mut edited = s.clone();
        let here = Dt::of_page(&self.session.prj.pages[pi]);
        let mut tag_note: Option<(String, bool)> = None;
        if tab == PropTab::Device {
        egui::Grid::new("sprops").num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            if def.func.has_tag() {
                let (label, tip) = match def.func {
                    Func::Terminal => (tr("Terminal strip"), ""),
                    Func::Plc => (tr("PLC card"), ""),
                    Func::Plug(_) => (tr("Plug"), ""),
                    _ => (tr("Belongs to device"), tr("The device this function belongs to. Another tag moves the function to that device (Enter applies, Esc restores); Rename device below renames the whole device.")),
                };
                let r = ui.label(label);
                if !tip.is_empty() {
                    r.on_hover_text(tip);
                }
                ui.horizontal(|ui| {
                    // Shown relative to the page structure, stored complete; typed into a
                    // copy and applied when the field loses the focus (one undo step).
                    let key = ui.scope_id().with(("device_tag", s.id));
                    let now = here.relative(&edited.tag);
                    let mut typed: String = ui.data_mut(|d| d.get_temp(key)).unwrap_or_else(|| now.clone());
                    let r = ui
                        .add(egui::TextEdit::singleline(&mut typed).id(key.with("edit")).desired_width(90.0).font(egui::TextStyle::Monospace))
                        .on_hover_text(tr_fmt!("Complete tag: {}\nAspects missing here are taken from the page (=function +location).", edited.tag));
                    if r.lost_focus() {
                        let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if let TagEdit::Move(tag) = tag_edit(&typed, &edited.tag, &here, esc) {
                            edited.tag = tag;
                        }
                    }
                    tag_note = if r.has_focus() { tag_note_of(&typed, &edited.tag, &here, idx) } else { None };
                    if r.has_focus() {
                        ui.data_mut(|d| d.insert_temp(key, typed));
                    } else {
                        ui.data_mut(|d| d.remove::<String>(key));
                    }
                    // Assign to an existing device (e.g. contact → coil)
                    egui::ComboBox::from_id_salt("assign").selected_text("…").width(30.0).show_ui(ui, |ui| {
                        for tag in idx.sorted_tags() {
                            let dev = &idx.devices[tag];
                            let fits = match def.func {
                                Func::Aux(_) => !dev.mains.is_empty(),
                                Func::Terminal | Func::Plc | Func::Plug(_) => !dev.others.is_empty(),
                                _ => true,
                            };
                            if fits && ui.selectable_label(*tag == edited.tag, tag.as_str()).clicked() {
                                edited.tag = tag.clone();
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr("Assign to existing device"));
                });
                ui.end_row();
                if let Some((text, error)) = tag_note.take() {
                    ui.label("");
                    if error {
                        ui.colored_label(crate::style::error(), text);
                    } else {
                        ui.weak(text);
                    }
                    ui.end_row();
                }
            }
            let label_name = match def.func {
                Func::Terminal => Some(tr("Terminal number")),
                Func::Plug(_) => Some(tr("Pin")),
                Func::Plc => Some(tr("Address")),
                Func::Potential => Some(tr("Potential name")),
                Func::Interruption => Some(tr("Name")),
                _ => None,
            };
            if let Some(name) = label_name {
                ui.label(name);
                ui.add(egui::TextEdit::singleline(&mut edited.label).desired_width(120.0));
                ui.end_row();
            }
            if def.func.has_tag() {
                ui.label(tr("Function text"));
                ui.add(egui::TextEdit::multiline(&mut edited.function_text).desired_rows(2).desired_width(160.0));
                ui.end_row();
            }
            if def.show_pin_labels {
                ui.label(tr("Connection points"));
                // Fixed rows of four: a wrapping row in a grid cell would overlap the
                // row below it.
                ui.vertical(|ui| {
                    for row in edited.pins.chunks_mut(4) {
                        ui.horizontal(|ui| {
                            for pin in row {
                                ui.add(egui::TextEdit::singleline(pin).desired_width(32.0));
                            }
                        });
                    }
                });
                ui.end_row();
            }
            ui.label(tr("Orientation"));
            ui.horizontal(|ui| {
                if icons::button(ui, Icon::Undo, tr("Rotate left")).clicked() {
                    edited.rot = (edited.rot + 3) % 4;
                }
                ui.label(format!("{}°", edited.rot as u32 * 90));
                if icons::button(ui, Icon::Rotate, tr("Rotate right")).clicked() {
                    edited.rot = (edited.rot + 1) % 4;
                }
                ui.checkbox(&mut edited.mirror, tr("Mirror"));
            });
            ui.end_row();
            ui.label(tr("Position"));
            ui.label(tr_fmt!("{} / {} mm  (column {})", format!("{:.0}", s.pos.x), format!("{:.0}", s.pos.y), openecraft_core::frame::of_page(&self.session.prj, pi).column_of(s.pos)));
            ui.end_row();
        });
        if let Func::Aux(kind) = def.func {
            if def.auto_number && ui.button(tr("Renumber contact")).on_hover_text(tr("Next free contact numbers on this device")).clicked() {
                edited.pins = ops::next_contact_pins(&self.session.prj, &edited.tag, kind, Some(s.id));
            }
        }
        }
        if edited != s {
            self.session.prj.pages[pi].symbols[si] = edited.clone();
        }
        if tab == PropTab::Device {
            ui.horizontal(|ui| {
                ui.label(tr("Function definition"));
                properties_ui::function_def_combo(ui, &mut self.session.prj, s.id);
            });
        }

        // Device data
        if def.func.has_tag() && !edited.tag.is_empty() {
            let mut data = self.session.prj.devices.get(&edited.tag).cloned().unwrap_or_default();
            let before = data.clone();
            if tab == PropTab::Device {
                ui.separator();
                ui.label(RichText::new(tr_fmt!("Device {}", edited.tag)).strong());
                ui.horizontal(|ui| {
                    ui.label(tr("Description"));
                    ui.text_edit_singleline(&mut data.description);
                });
            }
            // Part assignment (several parts per device, e.g. contactor + auxiliary block)
            if tab == PropTab::Parts {
            ui.label(RichText::new(tr_fmt!("Parts of {}", edited.tag)).strong());
            let mut remove = None;
            let mut show_part = None;
            egui::Grid::new("devparts").num_columns(2).striped(true).show(ui, |ui| {
                for (i, r) in data.parts.iter_mut().enumerate() {
                    let known = self.session.prj.parts.iter().find(|p| p.number == r.number);
                    // Accessories show the part they are attached to.
                    let on = known.filter(|p| p.accessory).map(|_| parts::attached_to(&self.session.prj, Some(&self.parts_db), &edited.tag, &r.number).map(|m| m.number.clone()));
                    let link = match &on {
                        Some(Some(main)) => ui.link(RichText::new(format!("{}  ({})", r.number, main)).monospace()),
                        Some(None) => ui.link(RichText::new(&r.number).monospace().color(crate::style::warning())),
                        None => ui.link(RichText::new(&r.number).monospace()),
                    };
                    let link = match (known, &on) {
                        (Some(p), Some(None)) => link.on_hover_text(format!("{}\n{}", p.description, tr("The accessory fits none of the device's parts"))),
                        (Some(p), Some(Some(main))) => link.on_hover_text(format!("{}\n{}", p.description, tr_fmt!("Fits {}", main))),
                        (Some(p), None) => link.on_hover_text(format!("{}\n{}", p.description, p.template_summary())),
                        (None, _) => link.on_hover_text(tr("Not in the project parts – synchronize in parts management")),
                    };
                    if link.clicked() {
                        show_part = Some(r.number.clone());
                    }
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut r.count).range(1..=999).prefix("× "));
                        if icons::button(ui, Icon::Delete, tr("Remove part")).clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                data.parts.remove(i);
            }
            let mut add_accessory: Option<String> = None;
            if let Some(n) = show_part {
                self.parts_ui.show_part(&n, true);
            }
            ui.horizontal(|ui| {
                if icons::text_button(ui, Icon::Plus, tr("Add part…")).on_hover_text(tr("Part selection from the parts database")).clicked() {
                    self.parts_ui.select_for(&edited.tag);
                }
                let choices = parts::accessory_choices(&self.session.prj, Some(&self.parts_db), &edited.tag);
                if !choices.is_empty() {
                    egui::ComboBox::from_id_salt("devacc").selected_text(tr("➕ Add accessory")).width(200.0).show_ui(ui, |ui| {
                        for (acc, main) in &choices {
                            let text = format!("{}  ({})", acc.number, acc.description);
                            if ui.selectable_label(false, text).on_hover_text(tr_fmt!("Fits {}", main)).clicked() {
                                add_accessory = Some(acc.number.clone());
                            }
                        }
                    });
                }
                if !data.parts.is_empty()
                    && ui
                        .button(tr("Apply designations"))
                        .on_hover_text(tr("Takes the connection point designations of all functions of this device from the function templates of its parts"))
                        .clicked()
                {
                    parts::apply_designations(&mut self.session.prj, Some(&self.parts_db), &edited.tag);
                }
            });
            if let Some(n) = add_accessory {
                // Added to the edited copy, which is stored below.
                self.session.prj.devices.insert(edited.tag.clone(), data.clone());
                if parts::add_accessory(&mut self.session.prj, &self.parts_db, &edited.tag, &n, 1).is_ok() {
                    data = self.session.prj.devices.get(&edited.tag).cloned().unwrap_or_default();
                }
            }
            }
            if data != before {
                self.session.prj.devices.insert(edited.tag.clone(), data.clone());
            }
            if tab == PropTab::Properties {
                ui.label(RichText::new(tr("Device properties")).strong());
                properties_ui::property_grid(ui, &mut self.session.prj, &properties::Object::Device(edited.tag.clone()), &mut self.properties_ui.device_grid, "devgrid");
            }
            if tab == PropTab::Device {
                if let Some((ei, _)) = openecraft_core::panel::locate(&self.session.prj, &edited.tag) {
                    let enc = self.session.prj.layout.enclosures[ei].name.clone();
                    if icons::text_button(ui, Icon::Database, &tr_fmt!("Show in panel layout ({})", enc)).clicked() {
                        self.focus_device(&edited.tag, false);
                    }
                }
            }
            // Functions placed vs. provided by the parts' function templates
            let usage = parts::usage(&self.session.prj, None, &edited.tag);
            if tab == PropTab::Parts && usage.values().any(|(_, a)| *a > 0) {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    for (kind, (placed, avail)) in &usage {
                        let txt = format!("{} {placed}/{avail}", tr(kind.title()));
                        let color = if placed > avail && !matches!(kind, parts::FnKind::Terminal | parts::FnKind::PlugPin) {
                            crate::style::error()
                        } else if placed < avail {
                            ui.visuals().weak_text_color()
                        } else {
                            ui.visuals().text_color()
                        };
                        ui.label(RichText::new(txt).small().color(color)).on_hover_text(tr("placed / provided by the parts"));
                    }
                });
            }
            // Rename the complete device
            if tab == PropTab::Device {
            // The text typed belongs to this device; another device starts with its own tag.
            let rename_id = ui.scope_id().with(("rename", &edited.tag));
            let mut new_tag: String = ui.data_mut(|d| d.get_temp(rename_id)).unwrap_or_else(|| edited.tag.clone());
            let problem = super::drawing_ui::rename_problem(&self.session.prj, &edited.tag, new_tag.trim());
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut new_tag).desired_width(90.0));
                let ok = problem.is_none() && new_tag.trim() != edited.tag;
                if ui.add_enabled(ok, egui::Button::new(tr("Rename device"))).on_hover_text(tr("Changes the tag of all functions of this device")).clicked() {
                    ops::rename_device(&mut self.session.prj, &edited.tag, new_tag.trim());
                }
            });
            if let Some(p) = problem {
                ui.colored_label(crate::style::error(), p);
            }
            ui.data_mut(|d| d.insert_temp(rename_id, new_tag));
            }

            // Cross references
            if let (PropTab::Connections, Some(dev)) = (tab, idx.devices.get(&edited.tag)) {
                ui.separator();
                ui.label(RichText::new(tr("Cross references")).strong());
                let mut jump = None;
                for (kind, locs) in [(tr("Main"), &dev.mains), (tr("Aux"), &dev.auxs), ("", &dev.others)] {
                    for loc in locs {
                        let inst = self.session.prj.pages[loc.page].symbols.iter().find(|x| x.id == loc.sym_id);
                        let Some(inst) = inst else { continue };
                        let dname = self.session.prj.symbol(&inst.sym).map(|d| tr(d.name)).unwrap_or("?");
                        let extra = if inst.label.is_empty() { inst.pins.join("-") } else { inst.label.clone() };
                        let me = if loc.sym_id == s.id { " ◀" } else { "" };
                        if ui.link(format!("{} {kind} {dname} {extra}{me}", loc.reference(&self.session.prj))).clicked() {
                            jump = Some((loc.page, loc.sym_id));
                        }
                    }
                }
                if let Some((pg, id)) = jump {
                    self.goto(pg, id);
                }
            }
        }

        if tab == PropTab::Properties {
            ui.separator();
            ui.label(RichText::new(tr("Function properties")).strong());
            properties_ui::property_grid(ui, &mut self.session.prj, &properties::Object::Function(s.id), &mut self.properties_ui.function_grid, "fngrid");
        }

        // Potentials at the connection points
        if tab == PropTab::Connections && !def.pins.is_empty() {
            ui.separator();
            ui.label(RichText::new(tr("Connections")).strong());
            egui::Grid::new("pinnets").num_columns(2).striped(true).show(ui, |ui| {
                for i in 0..def.pins.len() {
                    let pin = PinRef { page: pi, sym: si, pin: i };
                    ui.label(RichText::new(pin_designation(&self.session.prj, pin)).monospace());
                    let net = self.session.analysis.netlist.pin_net.get(&(s.id, i)).map(|&n| self.session.analysis.netlist.nets[n].name.clone());
                    let unconnected = self.session.analysis.netlist.unconnected.iter().any(|&(_, id, p)| id == s.id && p == i);
                    if unconnected {
                        ui.label(RichText::new(tr("open")).color(crate::style::lock()));
                    } else {
                        ui.label(net.unwrap_or_default());
                    }
                    ui.end_row();
                }
            });
        }
        if tab == PropTab::Display && self.session.prj.pages[pi].symbols.get(si).is_some_and(|x| x.id == s.id) && def.show_tag {
            self.placement_properties(ui, pi, si);
        }
    }
}

#[cfg(test)]
mod tests;
