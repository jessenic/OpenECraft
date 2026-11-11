//! The ribbon (see `crate::ribbon`): the quick access row, the tabs with the File tab
//! that opens the backstage, the command groups of the active tab, the favorite and
//! recent plugin commands and the external plugins on the Plugins tab, the user's own
//! groups and the key tips (Alt or F10).

use super::*;
use crate::commands::{self, Kind};
use crate::plugin_commands;
use crate::ribbon::{self, Item, TabId, TABS};

/// Key tips shown: on the tabs, or on the commands of a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tips {
    Tabs,
    Commands(TabId),
}

pub(super) struct RibbonUi {
    pub tab: TabId,
    pub custom: ribbon::Custom,
    pub tips: Option<Tips>,
    alt_down: bool,
    alt_used: bool,
}

impl Default for RibbonUi {
    fn default() -> Self {
        RibbonUi { tab: TabId::Home, custom: Default::default(), tips: None, alt_down: false, alt_used: false }
    }
}

impl RibbonUi {
    /// The context tab of a workspace comes to the front, a hidden one goes.
    pub fn follow_workspace(&mut self, w: Workspace) {
        match w {
            Workspace::Panel => self.tab = TabId::LayoutSpace,
            Workspace::Wiring | Workspace::Mounting => self.tab = TabId::Wiring,
            _ if ribbon::tab(self.tab).context => self.tab = TabId::Home,
            _ => {}
        }
    }
}

/// Height of a small button; a group is three of them high.
const ROW: f32 = 22.0;
const LARGE_W: f32 = 54.0;

impl App {
    /// Context tabs shown: the layout space while the panel layout is open, the wiring
    /// tab while the assistants are open.
    fn tab_visible(&self, id: TabId) -> bool {
        match id {
            TabId::LayoutSpace => self.workspace == Workspace::Panel || [Pane::LayoutView, Pane::LayoutSpaces].iter().any(|p| self.pane_shown(*p)),
            TabId::Wiring => self.workspace.is_workshop() || [Pane::WiringChecklist, Pane::WiringWorkbench].iter().any(|p| self.pane_shown(*p)),
            _ => true,
        }
    }

    /// The quick access row, the tabs and the groups of the active tab.
    pub(super) fn ribbon_bar(&mut self, ui: &mut egui::Ui) {
        if !self.tab_visible(self.ribbon.tab) {
            self.ribbon.tab = TabId::Home;
        }
        self.quick_access(ui);
        self.tab_row(ui);
        let peek = matches!(self.ribbon.tips, Some(Tips::Commands(_)));
        if (!self.shell.ribbon_minimized || peek) && self.backstage.is_none() {
            ui.add_space(2.0);
            // Every tab is as high as three small buttons and the group titles; groups
            // that do not fit collapse into one button each (no scroll bar).
            let height = ROW * 3.0 + 14.0 + ui.spacing().item_spacing.y;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());
            let mut groups = ui.new_child(egui::UiBuilder::new().max_rect(rect).layout(egui::Layout::left_to_right(egui::Align::Min)));
            groups.set_clip_rect(rect.intersect(ui.clip_rect()));
            self.ribbon_groups(&mut groups, rect.width());
        }
        ui.add_space(2.0);
    }

    fn quick_access(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for &cmd in ribbon::QUICK_ACCESS.iter().filter(|c| **c != Cmd::Backstage) {
                self.icon_cmd(ui, cmd);
                if cmd == Cmd::Undo && self.session.history.undo_steps() > 1 {
                    let r = icons::button(ui, Icon::ChevronDown, tr("Undo list"));
                    egui::Popup::menu(&r).show(|ui| {
                        let n = self.session.history.undo_steps().min(15);
                        for k in 1..=n {
                            if ui.button(tr_fmt!("Undo {} steps", k)).clicked() {
                                self.editor.cancel();
                                for _ in 0..k {
                                    self.session.undo();
                                }
                                ui.close();
                            }
                        }
                    });
                }
                if cmd == Cmd::PreviousPage {
                    let name = self.session.prj.pages.get(self.session.page).map(structure::page_full_name).unwrap_or_default();
                    let r = ui.add(egui::Button::new(RichText::new(name).monospace()).frame(false)).on_hover_text(tr("Pages"));
                    egui::Popup::menu(&r).show(|ui| {
                        egui::ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
                            for i in 0..self.session.prj.pages.len() {
                                let pg = &self.session.prj.pages[i];
                                let label = format!("{}  {}", structure::page_full_name(pg), pg.description);
                                if ui.selectable_label(i == self.session.page, label).clicked() {
                                    self.goto(i, 0);
                                    ui.close();
                                }
                            }
                        });
                    });
                }
            }
            ui.separator();
            let current = self.workspace;
            egui::ComboBox::from_id_salt("workspace").selected_text(tr(current.title())).width(130.0).show_ui(ui, |ui| {
                for w in Workspace::ALL {
                    if ui.selectable_label(w == current, tr(w.title())).clicked() {
                        self.set_workspace(w);
                    }
                }
            })
            .response
            .on_hover_text(tr("Workspaces"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // On a narrow window the project name goes first, then the search text
                // (the icon stays): nothing is drawn over the commands on the left.
                let room = ui.available_width();
                if room > 420.0 {
                    let dirty = if self.session.dirty() { " *" } else { "" };
                    ui.add(egui::Label::new(RichText::new(format!("{}{}", self.session.prj.info.name, dirty)).weak()).truncate());
                    ui.separator();
                }
                let hint = tr_fmt!("Find a command ({})", self.editor.shortcuts.cmd_text(Cmd::CommandSearch));
                let r = if room > 240.0 { icons::text_button(ui, Icon::Find, &hint) } else { icons::button(ui, Icon::Find, &hint) };
                if r.clicked() {
                    self.search.open();
                }
            });
        });
    }

    fn tab_row(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let tips_on_tabs = self.ribbon.tips == Some(Tips::Tabs);
            let file = ui.add(egui::Button::new(RichText::new(tr("File")).strong().color(ui.visuals().selection.stroke.color)).selected(self.backstage.is_some()));
            if tips_on_tabs {
                key_tip(ui, file.rect.center_bottom(), egui::Align2::CENTER_TOP, 'F');
            }
            if file.on_hover_text(tr(commands::def(Cmd::Backstage).tip)).clicked() {
                self.run(&ui.ctx().clone(), Cmd::Backstage);
            }
            for t in TABS {
                if !self.tab_visible(t.id) {
                    continue;
                }
                if t.id == TabId::Plugins && self.plugin_groups().is_empty() {
                    continue;
                }
                let selected = self.ribbon.tab == t.id && self.backstage.is_none();
                let mut text = RichText::new(tr(t.title));
                if t.context {
                    text = text.color(ui.visuals().warn_fg_color).strong();
                }
                let r = ui.selectable_label(selected, text);
                if tips_on_tabs {
                    key_tip(ui, r.rect.center_bottom(), egui::Align2::CENTER_TOP, t.key_tip);
                }
                if r.clicked() {
                    self.ribbon.tab = t.id;
                    self.backstage = None;
                    if self.shell.ribbon_minimized {
                        self.ribbon.tips = Some(Tips::Commands(t.id));
                    }
                }
                if r.double_clicked() {
                    self.shell.ribbon_minimized = !self.shell.ribbon_minimized;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (icon, tip) = if self.shell.ribbon_minimized { (Icon::ChevronDown, tr("Show the ribbon")) } else { (Icon::ChevronUp, tr("Minimize the ribbon")) };
                if icons::button(ui, icon, tip).clicked() {
                    self.shell.ribbon_minimized = !self.shell.ribbon_minimized;
                }
                if icons::button(ui, Icon::Help, tr("Help")).clicked() {
                    self.run(&ui.ctx().clone(), Cmd::Help);
                }
            });
        });
    }

    /// The commands of the active tab: built-in groups, the user's groups, plugin groups.
    /// Groups wider than `available` collapse from the right (`ribbon::collapsed`); the
    /// widths are measured when a group is drawn and kept for the next frames.
    fn ribbon_groups(&mut self, ui: &mut egui::Ui, available: f32) {
        let tab = ribbon::tab(self.ribbon.tab);
        let mut groups: Vec<(String, Vec<Item>)> = tab
            .groups
            .iter()
            .filter(|g| !self.ribbon.custom.is_hidden(tab, g))
            .map(|g| (tr(g.title).to_string(), g.items.to_vec()))
            .collect();
        for (_, g, cmds) in self.ribbon.custom.groups_of(tab) {
            groups.push((g.title.clone(), cmds.into_iter().map(Item::Small).collect()));
        }
        let tips: Vec<char> = if self.ribbon.tips == Some(Tips::Commands(tab.id)) {
            let titles: Vec<String> = groups.iter().flat_map(|(_, items)| items.iter().map(|i| tr(commands::def(i.cmd()).title).to_string())).collect();
            ribbon::key_tips(&titles, &[])
        } else {
            Vec::new()
        };
        let size = egui::TextStyle::Button.resolve(ui.style()).size.to_bits();
        let width_key = |title: &str, short: bool| egui::Id::unique(("ribbon_group_width", tab.key, title.to_string(), short, size));
        let measured = |ui: &egui::Ui, key: egui::Id| ui.data(|d| d.get_temp::<f32>(key));
        let plugins_key = egui::Id::unique(("ribbon_plugin_width", size));
        let extra = if tab.id == TabId::Plugins { measured(ui, plugins_key).unwrap_or(0.0) } else { 0.0 };
        // A group not measured yet is drawn whole once.
        let full: Vec<f32> = groups.iter().map(|(t, _)| measured(ui, width_key(t, false)).unwrap_or(0.0)).collect();
        let short: Vec<f32> = groups.iter().map(|(t, _)| measured(ui, width_key(t, true)).unwrap_or(LARGE_W + 12.0)).collect();
        if full.contains(&0.0) {
            ui.ctx().request_repaint();
        }
        let collapsed = ribbon::collapsed(&full, &short, available - extra);
        let mut k = 0;
        for ((title, items), short) in groups.into_iter().zip(collapsed) {
            let n = items.len();
            let tip_slice: Vec<char> = tips.get(k..k + n).map(|s| s.to_vec()).unwrap_or_default();
            k += n;
            let x = ui.cursor().min.x;
            if short {
                self.collapsed_group(ui, &title, &items);
            } else {
                self.ribbon_group(ui, &title, &items, &tip_slice);
            }
            let w = ui.cursor().min.x - x;
            ui.data_mut(|d| d.insert_temp(width_key(&title, short), w));
        }
        if tab.id == TabId::Plugins {
            let x = ui.cursor().min.x;
            self.plugin_tab(ui);
            let w = ui.cursor().min.x - x;
            ui.data_mut(|d| d.insert_temp(plugins_key, w));
        }
    }

    /// A group that does not fit: one button with the commands of the group in its menu.
    fn collapsed_group(&mut self, ui: &mut egui::Ui, title: &str, items: &[Item]) {
        ui.vertical(|ui| {
            // A folded group shows the tool of it that is on (Draw › Door), so that the
            // active tool stays visible.
            let active = items.iter().map(|i| i.cmd()).find(|&c| self.cmd_checked(c) == Some(true));
            let icon = active.or(items.first().map(|i| i.cmd())).map(|c| commands::def(c).icon).unwrap_or(Icon::Dots);
            let tip = match active {
                Some(c) => tr_fmt!("{}: {} is on", title, tr(commands::def(c).title)),
                None => title.to_string(),
            };
            let r = self.large_widget(ui, icon, title, active.is_some(), true, true).on_hover_text(tip);
            egui::Popup::menu(&r).show(|ui| {
                for item in items {
                    let c = item.cmd();
                    let def = commands::def(c);
                    if def.kind == Kind::Menu {
                        let enabled = self.cmd_enabled(c);
                        ui.add_enabled_ui(enabled, |ui| ui.menu_button(tr(def.title), |ui| self.cmd_menu(ui, c)));
                    } else {
                        self.cmd_button(ui, c);
                    }
                }
            });
            group_title(ui, "", r.rect.width());
        });
        ui.separator();
    }

    fn ribbon_group(&mut self, ui: &mut egui::Ui, title: &str, items: &[Item], tips: &[char]) {
        ui.vertical(|ui| {
            let row = ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                let mut i = 0;
                while i < items.len() {
                    match items[i] {
                        Item::Large(c) => {
                            self.large_button(ui, c, tips.get(i).copied());
                            i += 1;
                        }
                        Item::Small(_) => {
                            let start = i;
                            while i < items.len() && i - start < 3 && matches!(items[i], Item::Small(_)) {
                                i += 1;
                            }
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                                for (j, item) in items.iter().enumerate().take(i).skip(start) {
                                    self.small_button(ui, item.cmd(), tips.get(j).copied());
                                }
                            });
                        }
                    }
                }
            });
            group_title(ui, title, row.response.rect.width());
        });
        ui.separator();
    }

    fn large_button(&mut self, ui: &mut egui::Ui, cmd: Cmd, tip: Option<char>) {
        let def = commands::def(cmd);
        let title = tr(def.title).trim_end_matches('…').to_string();
        let enabled = self.cmd_enabled(cmd);
        let on = self.cmd_checked(cmd).unwrap_or(false);
        let resp = self.large_widget(ui, def.icon, &title, on, enabled, def.kind == Kind::Menu);
        if let Some(c) = tip {
            key_tip(ui, resp.rect.left_top() + egui::vec2(2.0, 2.0), egui::Align2::LEFT_TOP, c);
        }
        self.cmd_response(ui, cmd, resp);
    }

    /// A large button: the icon above the title.
    fn large_widget(&self, ui: &mut egui::Ui, icon: Icon, title: &str, on: bool, enabled: bool, menu: bool) -> egui::Response {
        let font = egui::TextStyle::Small.resolve(ui.style());
        // Lines break between words only: a long word widens the button.
        let longest = title.split_whitespace().map(|w| ui.painter().layout_no_wrap(w.to_string(), font.clone(), ui.visuals().text_color()).size().x).fold(0.0, f32::max);
        let galley = ui.painter().layout(title.to_string(), font, ui.visuals().text_color(), (LARGE_W + 16.0).max(longest + 1.0));
        let w = (galley.size().x + 8.0).max(LARGE_W);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ROW * 3.0), egui::Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, title));
        self.paint_cmd_frame(ui, &resp, rect, on, enabled);
        let color = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
        let icon_rect = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 15.0), egui::Vec2::splat(24.0));
        icons::paint(ui.painter(), icon_rect, icon, color);
        let text_pos = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.min.y + 31.0);
        ui.painter().galley_with_override_text_color(text_pos, galley, color);
        if menu {
            icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.max.x - 6.0, rect.min.y + 6.0), egui::Vec2::splat(8.0)), Icon::ChevronDown, color);
        }
        resp
    }

    fn small_button(&mut self, ui: &mut egui::Ui, cmd: Cmd, tip: Option<char>) {
        let def = commands::def(cmd);
        let title = tr(def.title).to_string();
        let font = egui::TextStyle::Button.resolve(ui.style());
        let galley = ui.painter().layout_no_wrap(title.clone(), font, ui.visuals().text_color());
        let menu = def.kind == Kind::Menu;
        let w = ROW + galley.size().x + 8.0 + if menu { 12.0 } else { 0.0 };
        let enabled = self.cmd_enabled(cmd);
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(w, ROW), egui::Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, &title));
        let on = self.cmd_checked(cmd).unwrap_or(false);
        self.paint_cmd_frame(ui, &resp, rect, on, enabled);
        let color = if enabled { ui.visuals().text_color() } else { ui.visuals().weak_text_color() };
        icons::paint(ui.painter(), egui::Rect::from_min_size(rect.min, egui::Vec2::splat(ROW)).shrink(3.5), def.icon, color);
        let text_pos = egui::pos2(rect.min.x + ROW + 2.0, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley_with_override_text_color(text_pos, galley, color);
        if menu {
            icons::paint(ui.painter(), egui::Rect::from_center_size(egui::pos2(rect.max.x - 7.0, rect.center().y), egui::Vec2::splat(10.0)), Icon::ChevronDown, color);
        }
        if let Some(c) = tip {
            // Over the icon: the title stays readable.
            key_tip(ui, rect.min + egui::Vec2::splat(ROW / 2.0), egui::Align2::CENTER_CENTER, c);
        }
        self.cmd_response(ui, cmd, resp);
    }

    fn paint_cmd_frame(&self, ui: &egui::Ui, resp: &egui::Response, rect: egui::Rect, on: bool, enabled: bool) {
        let v = ui.visuals();
        if on {
            ui.painter().rect_filled(rect, 3.0, v.selection.bg_fill.gamma_multiply(0.6));
        }
        if enabled && (resp.hovered() || resp.has_focus()) {
            ui.painter().rect_stroke(rect, 3.0, v.widgets.hovered.bg_stroke, egui::StrokeKind::Inside);
        }
    }

    /// Tooltip with shortcut, click runs the command or opens its menu.
    fn cmd_response(&mut self, ui: &mut egui::Ui, cmd: Cmd, resp: egui::Response) {
        let def = commands::def(cmd);
        let keys = self.editor.shortcuts.cmd_text(cmd);
        let tip = self.cmd_tip(cmd);
        let resp = resp.on_hover_ui(|ui| {
            ui.label(RichText::new(tr(def.title)).strong());
            if !tip.is_empty() {
                ui.label(tip.clone());
            }
            if !keys.is_empty() {
                ui.weak(keys.clone());
            }
        });
        if def.kind == Kind::Menu {
            if self.cmd_enabled(cmd) {
                egui::Popup::menu(&resp).show(|ui| self.cmd_menu(ui, cmd));
            }
        } else if resp.clicked() {
            let ctx = ui.ctx().clone();
            self.run(&ctx, cmd);
        }
    }

    /// A command as an icon button (quick access row, pane headers).
    pub(super) fn icon_cmd(&mut self, ui: &mut egui::Ui, cmd: Cmd) {
        let def = commands::def(cmd);
        let keys = self.editor.shortcuts.cmd_text(cmd);
        let label = if keys.is_empty() { tr(def.title).to_string() } else { format!("{} ({keys})", tr(def.title)) };
        let enabled = self.cmd_enabled(cmd);
        let on = self.cmd_checked(cmd).unwrap_or(false);
        let r = ui.add_enabled_ui(enabled, |ui| icons::toggle_button(ui, def.icon, &label, on)).inner;
        if r.clicked() {
            let ctx = ui.ctx().clone();
            self.run(&ctx, cmd);
        }
    }

    // ------------------------------------------------------------------ plugins

    /// The plugins with commands: name and commands (built-in areas first).
    pub(super) fn plugin_groups(&self) -> Vec<(String, Vec<openecraft_api::Command>)> {
        self.plugins.list().iter().filter(|l| l.enabled && !l.registry.commands.is_empty()).map(|l| (text(&l.info.name).to_string(), l.registry.commands.clone())).collect()
    }

    /// The external plugins with commands; the built-in ones are in the Plugin commands pane.
    fn external_plugin_groups(&self) -> Vec<(String, Vec<openecraft_api::Command>)> {
        self.plugins
            .list()
            .iter()
            .filter(|l| l.enabled && !l.registry.commands.is_empty() && matches!(l.origin, openecraft_api::Origin::External(_)))
            .map(|l| (text(&l.info.name).to_string(), l.registry.commands.clone()))
            .collect()
    }

    /// After the fixed group: the favorite and the recently used commands, and a menu per
    /// external plugin. Every command is in the Plugin commands pane.
    fn plugin_tab(&mut self, ui: &mut egui::Ui) {
        let all: Vec<openecraft_api::Command> = self.plugin_groups().into_iter().flat_map(|(_, c)| c).collect();
        let mut start = None;
        let favorites = plugin_commands::pick(&self.plugin_cmds.favorites, &all, |c| &c.name);
        let recent = plugin_commands::pick(&self.plugin_cmds.recent_only(), &all, |c| &c.name);
        let mut groups = vec![(tr("Favorites"), favorites)];
        if !recent.is_empty() {
            groups.push((tr("Recently used"), recent));
        }
        for (title, cmds) in groups {
            ui.vertical(|ui| {
                let row = ui.horizontal_top(|ui| {
                    if cmds.is_empty() {
                        ui.allocate_ui_with_layout(egui::vec2(170.0, ROW * 3.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                            ui.set_min_size(egui::vec2(170.0, ROW * 3.0));
                            ui.add(egui::Label::new(RichText::new(tr("Star a command in Plugin commands to pin it here")).weak()).wrap());
                        });
                    }
                    for chunk in cmds.chunks(3) {
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                            for c in chunk {
                                if icons::text_button(ui, Icon::Plugin, &humanize(&c.name)).on_hover_text(text(&c.help)).clicked() {
                                    start = Some((*c).clone());
                                }
                            }
                        });
                    }
                });
                group_title(ui, title, row.response.rect.width());
            });
            ui.separator();
        }
        let external = self.external_plugin_groups();
        if !external.is_empty() {
            ui.vertical(|ui| {
                let row = ui.horizontal_top(|ui| {
                    for (name, cmds) in &external {
                        let r = self.large_widget(ui, Icon::Plugin, name, false, true, true);
                        egui::Popup::menu(&r).show(|ui| {
                            for c in cmds {
                                if icons::text_button(ui, Icon::Plugin, &humanize(&c.name)).on_hover_text(text(&c.help)).clicked() {
                                    start = Some(c.clone());
                                    ui.close();
                                }
                            }
                        });
                    }
                });
                group_title(ui, tr("Installed plugins"), row.response.rect.width());
            });
            ui.separator();
        }
        if let Some(c) = start {
            self.start_plugin_command(c);
        }
    }

    // ------------------------------------------------------------------ key tips

    /// Alt (released alone) or F10 shows the key tips; a letter picks a tab, a second
    /// letter runs a command of it; Esc goes back.
    pub(super) fn key_tips(&mut self, ctx: &egui::Context) {
        let (alt, keys, f10, esc, clicked) = ctx.input(|i| {
            let keys: Vec<Key> = i.events.iter().filter_map(|e| if let egui::Event::Key { key, pressed: true, .. } = e { Some(*key) } else { None }).collect();
            (i.modifiers.alt, keys, i.key_pressed(Key::F10), i.key_pressed(Key::Escape), i.pointer.any_pressed())
        });
        if alt && !self.ribbon.alt_down {
            self.ribbon.alt_used = false;
        }
        if alt && (!keys.is_empty() || clicked) {
            self.ribbon.alt_used = true;
        }
        let toggle = (!alt && self.ribbon.alt_down && !self.ribbon.alt_used) || f10;
        self.ribbon.alt_down = alt;
        if clicked && !alt {
            self.ribbon.tips = None;
        }
        if toggle && !ctx.egui_wants_keyboard_input() {
            self.ribbon.tips = if self.ribbon.tips.is_some() { None } else { Some(Tips::Tabs) };
            return;
        }
        let Some(tips) = self.ribbon.tips else { return };
        self.editor.keys_active = false;
        self.panel_ui.keys_active = false;
        if esc {
            self.ribbon.tips = match tips {
                Tips::Commands(_) => Some(Tips::Tabs),
                Tips::Tabs => None,
            };
            return;
        }
        let Some(letter) = keys.iter().filter_map(|k| key_char(*k)).next() else { return };
        ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Text(_))));
        match tips {
            Tips::Tabs => {
                if letter == 'F' {
                    self.backstage = Some(crate::ribbon::Page::Home);
                    self.ribbon.tips = None;
                } else if let Some(t) = TABS.iter().find(|t| t.key_tip == letter && self.tab_visible(t.id)) {
                    self.ribbon.tab = t.id;
                    self.backstage = None;
                    self.ribbon.tips = Some(Tips::Commands(t.id));
                }
            }
            Tips::Commands(id) => {
                let tab = ribbon::tab(id);
                let mut cmds: Vec<Cmd> = tab.groups.iter().filter(|g| !self.ribbon.custom.is_hidden(tab, g)).flat_map(|g| g.items.iter().map(|i| i.cmd())).collect();
                for (_, _, c) in self.ribbon.custom.groups_of(tab) {
                    cmds.extend(c);
                }
                let titles: Vec<String> = cmds.iter().map(|c| tr(commands::def(*c).title).to_string()).collect();
                let tips = ribbon::key_tips(&titles, &[]);
                if let Some(at) = tips.iter().position(|c| *c == letter) {
                    self.ribbon.tips = None;
                    self.run(ctx, cmds[at]);
                }
            }
        }
    }
}

/// The title below a group, as wide as the group.
fn group_title(ui: &mut egui::Ui, title: &str, width: f32) {
    // A title longer than its group widens the group instead of being cut off.
    let text = RichText::new(title).small().weak();
    let width = width.max(32.0).max(egui::WidgetText::from(text.clone()).into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Small).size().x);
    ui.allocate_ui_with_layout(egui::vec2(width, 14.0), egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.set_width(width);
        ui.add(egui::Label::new(text).extend());
    });
}

/// The letter or digit of a key.
fn key_char(k: Key) -> Option<char> {
    let name = k.name();
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c.to_ascii_uppercase()),
        _ => None,
    }
}

/// A small key tip badge placed at `pos` (below a tab, in a corner of a button, over
/// the icon of a small button), so that the title stays readable.
fn key_tip(ui: &egui::Ui, pos: egui::Pos2, align: egui::Align2, c: char) {
    let font = egui::FontId::monospace(10.0);
    let galley = ui.painter().layout_no_wrap(c.to_string(), font, ui.visuals().strong_text_color());
    let r = align.anchor_size(pos, galley.size() + egui::vec2(6.0, 2.0));
    let layer = ui.painter().clone().with_layer_id(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("key_tip")));
    layer.rect(r, 2.0, ui.visuals().extreme_bg_color, ui.visuals().widgets.active.bg_stroke, egui::StrokeKind::Inside);
    layer.galley(r.min + egui::vec2(3.0, 1.0), galley, ui.visuals().strong_text_color());
}

/// `set_wire_defaults` → `Set wire defaults`.
pub(super) fn humanize(name: &str) -> String {
    let s = name.replace('_', " ");
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => s,
    }
}
