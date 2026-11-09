//! Buildings pane: the plot with its boundary and reference point, the buildings of the
//! plant with their levels, the rooms on them, their shafts, stairs and roofs (with
//! roof windows and PV arrays), the links between buildings and a site plan of all of
//! them ([`openecraft_core::panel::plot`], [`openecraft_core::panel::buildings`],
//! [`openecraft_core::panel::stairs`], [`openecraft_core::panel::roofs`],
//! [`openecraft_core::panel::solar`]).

use super::App;
use crate::panel_ui::site_plan::{self, SitePlan};
use crate::panel_ui::SiteTool;
use egui::{vec2, RichText, Sense};
use openecraft_core::model::Project;
use openecraft_core::panel::solar::strings;
use openecraft_core::panel::{self, buildings as bld, plot, roofs, solar, stairs, GeoReference, LinkKind, Opening, PvMounting, RoofShape, StairKind, Turn};
use openecraft_i18n::{tr, tr_fmt};
use openecraft_ui::icons::{self, Icon};

#[derive(Default)]
pub struct BuildingsUi {
    pub open: bool,
    /// Index of the building shown.
    sel: usize,
    /// Level shown in the site plan (id), 0 for all levels.
    level: u64,
    /// The building entries of a new link.
    link: (u64, u64),
    kind: LinkKind,
    status: String,
    /// Kind of a new stair and shape of a new roof.
    stair_kind: StairKind,
    roof_shape: RoofShape,
}

/// A part to choose: its number, description and size (width, height, depth).
struct Choice {
    number: String,
    description: String,
    subgroup: String,
    size: [f32; 3],
}

/// The parts of the project and of the built-in catalog of one of `subgroups`.
fn choices(prj: &Project, subgroups: &[&str]) -> Vec<Choice> {
    let mut out: Vec<Choice> = Vec::new();
    for p in prj.parts.iter().chain(openecraft_core::parts::catalog()) {
        if subgroups.contains(&p.subgroup.as_str()) && !out.iter().any(|c| c.number == p.number) {
            out.push(Choice { number: p.number.clone(), description: p.description.clone(), subgroup: p.subgroup.clone(), size: [p.width, p.height, p.depth] });
        }
    }
    out
}

/// What the PV arrays of the roofs of a building show besides their own data: the parts
/// to choose, the inverters, and per roof and array its power and the values of its
/// strings.
struct PvView {
    modules: Vec<Choice>,
    systems: Vec<Choice>,
    inverters: Vec<String>,
    powers: Vec<Vec<String>>,
    strings: Vec<Vec<Vec<String>>>,
}

impl PvView {
    fn of(prj: &Project, bi: usize) -> PvView {
        let roofs = &prj.layout.buildings[bi].roofs;
        let d = &prj.layout.pv_design;
        PvView {
            modules: choices(prj, &["PV module"]),
            systems: choices(prj, &[solar::Role::Rail.subgroup(), solar::Role::Base.subgroup()]),
            inverters: strings::inverters(prj),
            powers: roofs
                .iter()
                .map(|r| {
                    r.solar
                        .iter()
                        .map(|a| match solar::peak_power(prj, a) {
                            Some(kw) => tr_fmt!("{} modules, {} kWp", solar::module_count(a), format!("{kw:.2}")),
                            None => tr_fmt!("{} modules", solar::module_count(a)),
                        })
                        .collect()
                })
                .collect(),
            strings: roofs
                .iter()
                .map(|r| {
                    r.solar
                        .iter()
                        .map(|a| {
                            let m = strings::module_data(prj, &a.module);
                            a.strings
                                .iter()
                                .map(|s| match m {
                                    Some(m) => {
                                        let v = strings::values(&m, d, s);
                                        let f = |x: f32| format!("{x:.0}");
                                        tr_fmt!("U {} / {} / {} V, I {} A, {} kWp", f(v.voc_max), f(v.vmp_mean), f(v.vmp_min), format!("{:.1}", v.imp), format!("{:.2}", v.power / 1000.0))
                                    }
                                    None => tr("no electrical data of the module").to_string(),
                                })
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        }
    }
}

/// The strings of a PV array: modules in series and in parallel, inverter and tracker,
/// their values; whether a string is to be added.
fn pv_strings(ui: &mut egui::Ui, a: &mut panel::PvArray, id: (usize, usize), inverters: &[String], values: &[String]) -> bool {
    let mut add = false;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(28.0);
        ui.label(RichText::new(tr("Strings")).weak());
        if icons::text_button(ui, Icon::Plus, tr("String")).on_hover_text(tr("The modules no string takes yet in series, on the first inverter")).clicked() {
            add = true;
        }
    });
    let mut gone = None;
    for (s_k, s) in a.strings.iter_mut().enumerate() {
        ui.horizontal_wrapped(|ui| {
            ui.add_space(42.0);
            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(30.0));
            ui.add(egui::DragValue::new(&mut s.series).range(1..=200).suffix(" ×")).on_hover_text(tr("Modules in series"));
            ui.add(egui::DragValue::new(&mut s.parallel).range(1..=50)).on_hover_text(tr("Strings of this kind in parallel"));
            let shown = if s.inverter.is_empty() { tr("No inverter").to_string() } else { s.inverter.clone() };
            egui::ComboBox::from_id_salt(("bldpvstring", id, s_k)).selected_text(shown).width(90.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut s.inverter, String::new(), tr("No inverter"));
                for t in inverters {
                    ui.selectable_value(&mut s.inverter, t.clone(), t.as_str());
                }
            })
            .response
            .on_hover_text(tr("Inverter or charge controller (devices with PV input data)"));
            ui.add(egui::DragValue::new(&mut s.mppt).range(1..=64).prefix("MPPT ")).on_hover_text(tr("Its MPP tracker (input), from 1"));
            ui.label(values.get(s_k).map_or("", String::as_str)).on_hover_text(tr("Highest voltage (open circuit at the lowest temperature), mean and lowest voltage (at maximum power at the mean and the highest temperature), current at maximum power, peak power"));
            if icons::button(ui, Icon::Delete, tr("Delete string")).clicked() {
                gone = Some(s_k);
            }
        });
        *s = strings::checked_string(s.clone());
    }
    if let Some(s_k) = gone {
        a.strings.remove(s_k);
    }
    add
}

/// The PV arrays of a roof: place, modules, mounting, mounting system and strings of
/// each; the array to add a string to when asked.
fn pv_arrays(ui: &mut egui::Ui, r: &mut panel::Roof, k: usize, view: &PvView) -> Option<usize> {
    let (modules, systems, powers) = (&view.modules, &view.systems, &view.powers[k]);
    let mut gone = None;
    let mut add_string = None;
    for (a_k, a) in r.solar.iter_mut().enumerate() {
        let id = (k, a_k);
        ui.horizontal_wrapped(|ui| {
            ui.add_space(14.0);
            ui.add(egui::TextEdit::singleline(&mut a.name).desired_width(40.0));
            egui::ComboBox::from_id_salt(("bldpvmount", id)).selected_text(tr(a.mounting.title())).show_ui(ui, |ui| {
                for m in PvMounting::ALL {
                    ui.selectable_value(&mut a.mounting, m, tr(m.title()));
                }
            });
            egui::ComboBox::from_id_salt(("bldpvmodule", id)).selected_text(a.module.as_str()).width(170.0).show_ui(ui, |ui| {
                for c in modules {
                    if ui.selectable_label(a.module == c.number, c.number.as_str()).on_hover_text(c.description.as_str()).clicked() {
                        a.module = c.number.clone();
                        if c.size[0] > 0.0 && c.size[1] > 0.0 {
                            (a.module_width, a.module_length) = (c.size[0].min(c.size[1]), c.size[0].max(c.size[1]));
                        }
                        if c.size[2] > 0.0 {
                            a.module_height = c.size[2];
                        }
                    }
                }
            })
            .response
            .on_hover_text(tr("Module part; its size comes from the part"));
            ui.label(powers.get(a_k).map_or("", String::as_str));
            if icons::button(ui, Icon::Delete, tr("Delete PV array")).clicked() {
                gone = Some(a_k);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(28.0);
            ui.add(egui::DragValue::new(&mut a.x).speed(10.0).prefix("x ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut a.z).speed(10.0).prefix("z ").suffix(" mm")).on_hover_text(tr("Middle of the PV array in the floor plan of the building"));
            ui.add(egui::DragValue::new(&mut a.columns).range(1..=200).suffix(" ×")).on_hover_text(tr("Modules side by side in a row"));
            ui.add(egui::DragValue::new(&mut a.rows).range(1..=100)).on_hover_text(tr("Rows of modules (east-west: rows of pairs)"));
            ui.checkbox(&mut a.landscape, tr("landscape")).on_hover_text(tr("The long side of the modules along the rows"));
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(28.0);
            if a.mounting.is_tilted() {
                ui.label(tr("Tilt"));
                ui.add(egui::DragValue::new(&mut a.tilt).speed(0.5).range(0.0..=60.0).suffix(" °"));
                ui.label(tr("Facing"));
                ui.add(egui::DragValue::new(&mut a.azimuth).speed(1.0).suffix(" °")).on_hover_text(tr("Direction the modules face, counter-clockwise seen from above (0: towards the front)"));
                ui.label(tr("Row distance"));
                ui.add(egui::DragValue::new(&mut a.row_gap).speed(10.0).range(0.0..=20_000.0).suffix(" mm")).on_hover_text(tr("From row to row; 0: free of shade in winter"));
                ui.label(tr("Ballast"));
                ui.add(egui::DragValue::new(&mut a.ballast).speed(0.5).range(0.0..=500.0).suffix(" kg")).on_hover_text(tr("Ballast per module"));
            } else {
                ui.label(tr("Hook distance"));
                ui.add(egui::DragValue::new(&mut a.span).speed(10.0).range(300.0..=3000.0).suffix(" mm")).on_hover_text(tr("Greatest distance of the roof hooks along a rail"));
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.add_space(28.0);
            ui.label(tr("Mounting system"));
            let main = if a.mounting.is_tilted() { solar::Role::Base } else { solar::Role::Rail };
            let shown = if a.system.is_empty() { tr("Generic").to_string() } else { a.system.clone() };
            egui::ComboBox::from_id_salt(("bldpvsystem", id)).selected_text(shown).width(150.0).show_ui(ui, |ui| {
                ui.selectable_value(&mut a.system, String::new(), tr("Generic"));
                for c in systems.iter().filter(|c| c.subgroup == main.subgroup()) {
                    ui.selectable_value(&mut a.system, c.number.clone(), c.number.as_str()).on_hover_text(c.description.as_str());
                }
            })
            .response
            .on_hover_text(tr("The rail or mounting base; its accessories are the hooks, clamps and ballast"));
            ui.label(tr("Device"));
            ui.add(egui::TextEdit::singleline(&mut a.device).desired_width(90.0)).on_hover_text(tr("Device (PV generator) the modules belong to; its parts then count them in the bill of materials"));
        });
        let values = view.strings[k].get(a_k).map_or(&[][..], Vec::as_slice);
        if pv_strings(ui, a, id, &view.inverters, values) {
            add_string = Some(a_k);
        }
        *a = solar::checked_array(a.clone());
    }
    if let Some(a_k) = gone {
        r.solar.remove(a_k);
    }
    add_string
}

/// The building entries of all rooms: (item id, "room – wall n").
fn entry_choices(prj: &Project) -> Vec<(u64, String)> {
    let layout = &prj.layout;
    panel::rooms(layout)
        .into_iter()
        .flat_map(|ri| {
            let e = &layout.enclosures[ri];
            e.items.iter().filter(|i| i.opening == Opening::Entry).map(move |i| (i.id, format!("{}  {}", e.name, e.face_title(i.face))))
        })
        .collect()
}

fn entry_label(choices: &[(u64, String)], id: u64) -> String {
    choices.iter().find(|c| c.0 == id).map_or_else(|| tr("(none)").to_string(), |c| c.1.clone())
}

impl App {
    #[allow(clippy::too_many_lines)]
    pub(super) fn buildings_pane(&mut self, ui: &mut egui::Ui) {
        let (prj, st) = (&mut self.session.prj, &mut self.buildings_ui);
        ui.horizontal_wrapped(|ui| {
            if icons::text_button(ui, Icon::Plus, tr("New building")).on_hover_text(tr("A building with one level at ground level, beside what stands in the plant")).clicked() {
                let name = bld::free_building_name(&prj.layout);
                match bld::add_building(prj, &name) {
                    Ok(bi) => {
                        st.sel = bi;
                        st.status.clear();
                    }
                    Err(e) => st.status = e,
                }
            }
            ui.weak(tr("Rooms stand on the levels of a building; shafts join rooms on different levels, links join the building entries of different buildings."));
        });
        if !st.status.is_empty() {
            ui.colored_label(crate::style::warning(), st.status.as_str());
        }
        let problems = bld::problems(prj);
        if !problems.is_empty() {
            egui::CollapsingHeader::new(RichText::new(tr_fmt!("{} problems", problems.len())).color(crate::style::warning())).id_salt("bldproblems").show(ui, |ui| {
                for p in &problems {
                    ui.label(p.message());
                }
            });
        }
        if let Some(tool) = plot_section(ui, prj) {
            self.panel_ui.choose_site_tool(tool);
        }
        ui.separator();
        if prj.layout.buildings.is_empty() {
            ui.weak(tr("No building yet: create one with New building, then put rooms on its levels."));
            return;
        }
        st.sel = st.sel.min(prj.layout.buildings.len() - 1);
        ui.horizontal_wrapped(|ui| {
            for (bi, b) in prj.layout.buildings.iter().enumerate() {
                if ui.selectable_label(st.sel == bi, b.name.as_str()).clicked() {
                    st.sel = bi;
                    st.level = 0;
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt("bldscroll").auto_shrink([false, false]).show(ui, |ui| {
            let bi = st.sel;
            building_section(ui, prj, st, bi);
            if st.sel >= prj.layout.buildings.len() {
                return;
            }
            ui.separator();
            levels_section(ui, prj, st, bi);
            ui.separator();
            shafts_section(ui, prj, bi);
            ui.separator();
            stairs_section(ui, prj, st, bi);
            ui.separator();
            roofs_section(ui, prj, st, bi);
            ui.separator();
            links_section(ui, prj, st);
            ui.separator();
            site_plan_section(ui, prj, st);
        });
    }
}

/// A coordinate reference system shown as the example (an EPSG code, not translated).
const CRS_EXAMPLE: &str = "EPSG:25832";

/// The plot: its name, its boundary (drawn in the site plan) and the reference point
/// with its coordinates; gives the tool of the site plan to choose.
#[allow(clippy::too_many_lines)]
fn plot_section(ui: &mut egui::Ui, prj: &mut Project) -> Option<SiteTool> {
    let mut tool = None;
    let outside: Vec<String> = plot::outside(prj)
        .into_iter()
        .map(|s| match s {
            plot::Standing::Room(i) | plot::Standing::Cabinet(i) => prj.layout.enclosures[i].name.clone(),
            plot::Standing::Roof(bi, k) => format!("{} {}", prj.layout.buildings[bi].name, prj.layout.buildings[bi].roofs[k].name),
        })
        .collect();
    let p = &mut prj.layout.plot;
    egui::CollapsingHeader::new(RichText::new(tr("Plot")).strong()).id_salt("bldplot").default_open(true).show(ui, |ui| {
        egui::Grid::new("bldplotgrid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Name"));
            ui.text_edit_singleline(&mut p.name).on_hover_text(tr("Name of the plot, e.g. the cadastral parcel"));
            ui.end_row();
            ui.label(tr("Boundary"));
            ui.horizontal(|ui| {
                if p.outline.is_empty() {
                    ui.weak(tr("none"));
                } else {
                    ui.label(tr_fmt!("{} corners, {} m², {} m round", p.outline.len(), format!("{:.1}", plot::area(&p.outline)), format!("{:.2}", plot::perimeter(&p.outline))));
                }
                if icons::text_button(ui, Icon::Polygon, tr("Draw")).on_hover_text(tr("Draw the boundary of the plot in the site plan; all buildings have to stay inside it")).clicked() {
                    tool = Some(SiteTool::Boundary);
                }
                if !p.outline.is_empty() && icons::button(ui, Icon::Delete, tr("Remove the boundary")).clicked() {
                    p.outline.clear();
                }
            });
            ui.end_row();
        });
        if !outside.is_empty() {
            ui.colored_label(crate::style::error(), tr_fmt!("Outside the plot boundary: {}", outside.join(", ")));
        }
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Reference point")).strong());
            if icons::text_button(ui, Icon::Pin, tr("Place in the site plan")).on_hover_text(tr("Click where it is in the site plan")).clicked() {
                tool = Some(SiteTool::Reference);
            }
            if p.reference.is_none() {
                if icons::text_button(ui, Icon::Plus, tr("At the origin")).on_hover_text(tr("A reference point at the origin of the plant")).clicked() {
                    p.reference = Some(GeoReference::default());
                }
            } else if icons::button(ui, Icon::Delete, tr("Remove the reference point")).clicked() {
                p.reference = None;
            }
        });
        let Some(r) = &mut p.reference else {
            ui.weak(tr("Without a reference point the plant is measured from its origin, north towards the rear."));
            return;
        };
        egui::Grid::new("bldrefgrid").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Name"));
            ui.text_edit_singleline(&mut r.name).on_hover_text(tr("Name of the point, e.g. a survey point"));
            ui.end_row();
            ui.label(tr("Place in the plant"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut r.x).speed(50.0).prefix("x ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut r.z).speed(50.0).prefix("z ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut r.y).speed(10.0).prefix("y ").suffix(" mm"));
            })
            .response
            .on_hover_text(tr("Where the point is in the plant; y: its height above the ground level of the plant"));
            ui.end_row();
            ui.label(tr("Easting, northing"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut r.easting).speed(0.01).fixed_decimals(3).prefix("E ").suffix(" m"));
                ui.add(egui::DragValue::new(&mut r.northing).speed(0.01).fixed_decimals(3).prefix("N ").suffix(" m"));
            })
            .response
            .on_hover_text(tr("The point's coordinates in the coordinate reference system"));
            ui.end_row();
            ui.label(tr("Height above sea level"));
            ui.add(egui::DragValue::new(&mut r.height).speed(0.01).fixed_decimals(3).suffix(" m"));
            ui.end_row();
            ui.label(tr("Coordinate reference system"));
            ui.add(egui::TextEdit::singleline(&mut r.crs).hint_text(CRS_EXAMPLE).desired_width(140.0));
            ui.end_row();
            ui.label(tr("Grid north"));
            if ui.add(egui::DragValue::new(&mut r.north).speed(0.5).range(-360.0..=360.0).suffix(" °")).on_hover_text(tr("Direction of grid north, counter-clockwise seen from above from the rear of the plant (-z)")).changed() {
                r.north = r.north.rem_euclid(360.0);
            }
            ui.end_row();
            ui.label(tr("Latitude, longitude"));
            ui.horizontal(|ui| {
                let mut known = r.latitude.is_some() && r.longitude.is_some();
                if ui.checkbox(&mut known, tr("known")).on_hover_text(tr("WGS 84, written into the site of IFC exports")).changed() {
                    (r.latitude, r.longitude) = if known { (Some(0.0), Some(0.0)) } else { (None, None) };
                }
                if let (Some(lat), Some(lon)) = (&mut r.latitude, &mut r.longitude) {
                    ui.add(egui::DragValue::new(lat).speed(0.000_01).range(-90.0..=90.0).fixed_decimals(7).suffix(" °"));
                    ui.add(egui::DragValue::new(lon).speed(0.000_01).range(-180.0..=180.0).fixed_decimals(7).suffix(" °"));
                }
            });
            ui.end_row();
            let g = plot::to_geo(r, [0.0; 3]);
            ui.label(tr("Origin of the plant"));
            ui.label(format!("E {:.3}  N {:.3}  {:.3} m", g[0], g[1], g[2]));
            ui.end_row();
        });
    });
    tool
}

fn building_section(ui: &mut egui::Ui, prj: &mut Project, st: &mut BuildingsUi, bi: usize) {
    let mut remove = false;
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Building")).strong());
        if icons::button(ui, Icon::Delete, tr("Delete building; its rooms stay where they stand")).clicked() {
            remove = true;
        }
    });
    if remove {
        bld::remove_building(prj, bi);
        st.sel = bi.saturating_sub(1);
        return;
    }
    let b = &mut prj.layout.buildings[bi];
    egui::Grid::new("bldprops").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
        ui.label(tr("Location"));
        ui.text_edit_singleline(&mut b.name);
        ui.end_row();
        ui.label(tr("Description"));
        ui.text_edit_singleline(&mut b.description);
        ui.end_row();
        ui.label(tr("Place in the plant"));
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut b.site.x).speed(50.0).prefix("x ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut b.site.z).speed(50.0).prefix("z ").suffix(" mm"));
            ui.add(egui::DragValue::new(&mut b.site.angle).speed(1.0).range(-360.0..=360.0).suffix(" °"));
        })
        .response
        .on_hover_text(tr("Origin of the building's floor plan in the plant; its rooms move with it"));
        ui.end_row();
    });
}

#[allow(clippy::too_many_lines)]
fn levels_section(ui: &mut egui::Ui, prj: &mut Project, st: &mut BuildingsUi, bi: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Levels")).strong());
        if icons::text_button(ui, Icon::Plus, tr("Level on top")).clicked() {
            if let Err(e) = bld::add_level(prj, bi, None, None, None) {
                st.status = e;
            }
        }
        if icons::text_button(ui, Icon::Plus, tr("Basement")).on_hover_text(tr("A level below the lowest one")).clicked() {
            let low = prj.layout.buildings[bi].levels.first().map_or(0.0, |l| l.elevation);
            if let Err(e) = bld::add_level(prj, bi, None, Some(low - bld::LEVEL_HEIGHT), None) {
                st.status = e;
            }
        }
    });
    let free_rooms: Vec<(usize, String)> = panel::rooms(&prj.layout).into_iter().filter(|&r| prj.layout.enclosures[r].level == 0).map(|r| (r, prj.layout.enclosures[r].name.clone())).collect();
    let mut change: Option<(u64, Option<f32>, Option<f32>)> = None;
    let mut remove = None;
    let mut new_room = None;
    let mut put: Option<(usize, u64)> = None;
    let mut take_out = None;
    let levels = prj.layout.buildings[bi].levels.clone();
    egui::Grid::new("bldlevels").num_columns(5).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
        ui.label(RichText::new(tr("Name")).weak());
        ui.label(RichText::new(tr("Floor above ground")).weak());
        ui.label(RichText::new(tr("Height")).weak());
        ui.label(RichText::new(tr("Rooms")).weak());
        ui.label("");
        ui.end_row();
        for (k, l) in levels.iter().enumerate().rev() {
            // The name is edited in place (designations, not checked while typing).
            ui.add(egui::TextEdit::singleline(&mut prj.layout.buildings[bi].levels[k].name).desired_width(60.0));
            let mut elevation = l.elevation;
            if ui.add(egui::DragValue::new(&mut elevation).speed(50.0).suffix(" mm")).changed() {
                change = Some((l.id, Some(elevation), None));
            }
            let mut height = l.height;
            if ui.add(egui::DragValue::new(&mut height).speed(50.0).range(1000.0..=50_000.0).suffix(" mm")).changed() {
                change = Some((l.id, None, Some(height)));
            }
            ui.horizontal(|ui| {
                for r in bld::rooms_on(&prj.layout, l.id) {
                    ui.label(prj.layout.enclosures[r].name.as_str());
                    if icons::button(ui, Icon::Close, tr("Take the room out of the building; it stays where it stands")).clicked() {
                        take_out = Some(r);
                    }
                }
                if icons::button(ui, Icon::Wall, tr("New room on this level")).clicked() {
                    new_room = Some(l.id);
                }
                if !free_rooms.is_empty() {
                    egui::ComboBox::from_id_salt(("bldput", l.id)).selected_text(tr("Put a room here")).width(140.0).show_ui(ui, |ui| {
                        for (r, n) in &free_rooms {
                            if ui.selectable_label(false, n.as_str()).clicked() {
                                put = Some((*r, l.id));
                            }
                        }
                    });
                }
            });
            if icons::button(ui, Icon::Delete, tr("Delete level; its rooms stay where they stand")).clicked() {
                remove = Some(l.id);
            }
            ui.end_row();
        }
    });
    let result = if let Some((id, elevation, height)) = change {
        bld::set_level(prj, bi, id, None, elevation, height)
    } else if let Some(id) = remove {
        bld::remove_level(prj, bi, id)
    } else if let Some(level) = new_room {
        let name = panel::free_name(&prj.layout, "+ROOM");
        bld::add_room(prj, &name, Some(level)).map(|_| ())
    } else if let Some((r, level)) = put {
        bld::set_room_level(prj, r, Some(level), None)
    } else if let Some(r) = take_out {
        bld::set_room_level(prj, r, None, None)
    } else {
        Ok(())
    };
    match result {
        Ok(()) if change.is_some() || remove.is_some() || new_room.is_some() || put.is_some() || take_out.is_some() => st.status.clear(),
        Ok(()) => {}
        Err(e) => st.status = e,
    }
    ui.weak(tr("A room on a level stands at its place in the building's floor plan (Layout space › room properties); without one next to the other rooms of the level."));
}

fn shafts_section(ui: &mut egui::Ui, prj: &mut Project, bi: usize) {
    let poses = panel::site_poses(prj);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Shafts")).strong());
        if icons::text_button(ui, Icon::Plus, tr("New shaft")).on_hover_text(tr("Through all levels; move it to its place in the building's floor plan")).clicked() {
            let b = &prj.layout.buildings[bi];
            let (lo, hi) = (b.levels.first().map_or(0, |l| l.id), b.levels.last().map_or(0, |l| l.id));
            let _ = bld::add_shaft(prj, bi, None, 0.0, 0.0, bld::SHAFT_SIZE, bld::SHAFT_SIZE, lo, hi);
        }
    });
    let b = &prj.layout.buildings[bi];
    if b.shafts.is_empty() {
        ui.weak(tr("No shaft yet: cables, tubes and pipes run through shafts from level to level."));
        return;
    }
    let joins: Vec<String> = b.shafts.iter().map(|s| bld::shaft_rooms(prj, &poses, bi, s).into_iter().map(|r| prj.layout.enclosures[r].name.clone()).collect::<Vec<_>>().join(", ")).collect();
    let levels: Vec<(u64, String)> = b.levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let level_name = |id: u64| levels.iter().find(|l| l.0 == id).map_or_else(String::new, |l| l.1.clone());
    let mut remove = None;
    let b = &mut prj.layout.buildings[bi];
    egui::Grid::new("bldshafts").num_columns(6).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
        for t in [tr("Name"), tr("Place"), tr("Size"), tr("Levels"), tr("Joins"), ""] {
            ui.label(RichText::new(t).weak());
        }
        ui.end_row();
        for (k, s) in b.shafts.iter_mut().enumerate() {
            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(50.0));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.x).speed(10.0).prefix("x ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut s.z).speed(10.0).prefix("z ").suffix(" mm"));
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.width).speed(10.0).range(100.0..=20_000.0));
                ui.add(egui::DragValue::new(&mut s.depth).speed(10.0).range(100.0..=20_000.0).suffix(" mm"));
            });
            ui.horizontal(|ui| {
                for (salt, value) in [("bot", &mut s.bottom), ("top", &mut s.top)] {
                    egui::ComboBox::from_id_salt(("bldshaft", salt, k)).selected_text(level_name(*value)).width(60.0).show_ui(ui, |ui| {
                        for (id, n) in &levels {
                            ui.selectable_value(value, *id, n.as_str());
                        }
                    });
                }
            });
            if joins[k].is_empty() {
                ui.colored_label(crate::style::warning(), tr("no rooms"));
            } else {
                ui.label(joins[k].as_str());
            }
            if icons::button(ui, Icon::Delete, tr("Delete shaft")).clicked() {
                remove = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = remove {
        b.shafts.remove(k);
    }
    // Levels chosen from the top down are turned round.
    let shafts: Vec<_> = b.shafts.clone();
    for (k, s) in shafts.into_iter().enumerate() {
        if let Ok(s) = bld::checked_shaft(b, s) {
            b.shafts[k] = s;
        }
    }
}

/// A choice of a level of building `bi` by its id.
fn level_combo(ui: &mut egui::Ui, salt: impl std::hash::Hash + std::fmt::Debug, levels: &[(u64, String)], value: &mut u64) {
    let name = levels.iter().find(|l| l.0 == *value).map_or_else(String::new, |l| l.1.clone());
    egui::ComboBox::from_id_salt(salt).selected_text(name).width(60.0).show_ui(ui, |ui| {
        for (id, n) in levels {
            ui.selectable_value(value, *id, n.as_str());
        }
    });
}

fn stairs_section(ui: &mut egui::Ui, prj: &mut Project, st: &mut BuildingsUi, bi: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Stairs")).strong());
        egui::ComboBox::from_id_salt("bldstairkind").selected_text(tr(st.stair_kind.title())).show_ui(ui, |ui| {
            for kind in StairKind::ALL {
                ui.selectable_value(&mut st.stair_kind, kind, tr(kind.title()));
            }
        });
        if icons::text_button(ui, Icon::Plus, tr("New stair")).on_hover_text(tr("From the lowest level up to the next one; move it to its place in the building's floor plan")).clicked() {
            let bottom = prj.layout.buildings[bi].levels.first().map_or(0, |l| l.id);
            match stairs::add_stair(prj, bi, st.stair_kind, 0.0, 0.0, bottom, None) {
                Ok(_) => st.status.clear(),
                Err(e) => st.status = e,
            }
        }
    });
    let b = &prj.layout.buildings[bi];
    if b.stairs.is_empty() {
        ui.weak(tr("No stair yet: stairs lead from the floor of one level up to the floor of another."));
        return;
    }
    let levels: Vec<(u64, String)> = b.levels.iter().map(|l| (l.id, l.name.clone())).collect();
    let steps: Vec<String> = b.stairs.iter().map(|s| stairs::geometry(b, s).map_or_else(|| tr("no valid levels").to_string(), |g| tr_fmt!("{} × {} mm", g.risers, format!("{:.0}", g.rise)))).collect();
    let mut remove = None;
    let b = &mut prj.layout.buildings[bi];
    egui::Grid::new("bldstairs").num_columns(7).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
        for t in [tr("Name"), tr("Kind"), tr("Foot"), tr("Width / going"), tr("Levels"), tr("Risers"), ""] {
            ui.label(RichText::new(t).weak());
        }
        ui.end_row();
        for (k, s) in b.stairs.iter_mut().enumerate() {
            ui.add(egui::TextEdit::singleline(&mut s.name).desired_width(40.0));
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("bldstair", k)).selected_text(tr(s.kind.title())).width(120.0).show_ui(ui, |ui| {
                    for kind in StairKind::ALL {
                        ui.selectable_value(&mut s.kind, kind, tr(kind.title()));
                    }
                });
                if s.kind != StairKind::Straight {
                    egui::ComboBox::from_id_salt(("bldstairturn", k)).selected_text(tr(s.turn.title())).width(60.0).show_ui(ui, |ui| {
                        for turn in [Turn::Left, Turn::Right] {
                            ui.selectable_value(&mut s.turn, turn, tr(turn.title()));
                        }
                    });
                }
            });
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.x).speed(10.0).prefix("x ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut s.z).speed(10.0).prefix("z ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut s.angle).speed(1.0).range(-360.0..=360.0).suffix(" °"));
            })
            .response
            .on_hover_text(tr("Middle of the front edge of the lowest step in the building's floor plan, and the direction of the first flight"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut s.width).speed(10.0).range(500.0..=5000.0));
                ui.add(egui::DragValue::new(&mut s.tread).speed(1.0).range(150.0..=500.0).suffix(" mm"));
            });
            ui.horizontal(|ui| {
                level_combo(ui, ("bldstairbot", k), &levels, &mut s.bottom);
                level_combo(ui, ("bldstairtop", k), &levels, &mut s.top);
            });
            ui.label(steps[k].as_str());
            if icons::button(ui, Icon::Delete, tr("Delete stair")).clicked() {
                remove = Some(k);
            }
            ui.end_row();
        }
    });
    if let Some(k) = remove {
        b.stairs.remove(k);
    }
}

fn roofs_section(ui: &mut egui::Ui, prj: &mut Project, st: &mut BuildingsUi, bi: usize) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Roofs")).strong());
        egui::ComboBox::from_id_salt("bldroofshape").selected_text(tr(st.roof_shape.title())).show_ui(ui, |ui| {
            for shape in RoofShape::ALL {
                ui.selectable_value(&mut st.roof_shape, shape, tr(shape.title()));
            }
        });
        if icons::text_button(ui, Icon::Plus, tr("New roof")).on_hover_text(tr("Over the rooms of the building with its eaves on top of the highest level")).clicked() {
            match roofs::add_roof(prj, bi, st.roof_shape) {
                Ok(_) => st.status.clear(),
                Err(e) => st.status = e,
            }
        }
    });
    if prj.layout.buildings[bi].roofs.is_empty() {
        ui.weak(tr("No roof yet: a roof lower than the ceilings of the rooms below it slopes their ceilings, also through several levels."));
        return;
    }
    // The rooms of the building under sloped ceilings, with their lowest and highest ceiling.
    let poses = panel::site_poses(prj);
    let sloped: Vec<String> = bld::building_rooms(&prj.layout, bi)
        .into_iter()
        .filter_map(|ri| {
            let e = &prj.layout.enclosures[ri];
            let c = roofs::ceiling_with(prj, &poses, ri)?;
            let (lo, hi) = c.range(&roofs::floor_polys(e));
            Some(tr_fmt!("{}: {} – {} m", e.name, format!("{:.2}", lo / 1000.0), format!("{:.2}", hi / 1000.0)))
        })
        .collect();
    let view = PvView::of(prj, bi);
    let mut remove = None;
    let mut add_window = None;
    let mut add_pv = None;
    let mut add_string = None;
    let b = &mut prj.layout.buildings[bi];
    for (k, r) in b.roofs.iter_mut().enumerate() {
        ui.group(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::TextEdit::singleline(&mut r.name).desired_width(40.0));
                egui::ComboBox::from_id_salt(("bldroof", k)).selected_text(tr(r.shape.title())).show_ui(ui, |ui| {
                    for shape in RoofShape::ALL {
                        ui.selectable_value(&mut r.shape, shape, tr(shape.title()));
                    }
                });
                if matches!(r.shape, RoofShape::Gable | RoofShape::Shed) {
                    ui.checkbox(&mut r.along_z, tr("along z")).on_hover_text(tr("The ridge (or the slope of a shed roof) runs along z instead of x"));
                }
                if r.shape == RoofShape::Shed {
                    ui.checkbox(&mut r.flip, tr("low side at the rear")).on_hover_text(tr("The low eaves at the rear or left instead of the front or right"));
                }
                ui.label(tr_fmt!("ridge {} m", format!("{:.2}", roofs::ridge_height(r) / 1000.0)));
                if icons::button(ui, Icon::Delete, tr("Delete roof")).clicked() {
                    remove = Some(k);
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Covers"));
                ui.add(egui::DragValue::new(&mut r.x).speed(10.0).prefix("x ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut r.z).speed(10.0).prefix("z ").suffix(" mm"));
                ui.add(egui::DragValue::new(&mut r.width).speed(10.0).range(500.0..=200_000.0).prefix("w "));
                ui.add(egui::DragValue::new(&mut r.depth).speed(10.0).range(500.0..=200_000.0).prefix("d ").suffix(" mm"));
            })
            .response
            .on_hover_text(tr("Rear left corner and size of the rectangle it covers (the outer walls) in the building's floor plan"));
            ui.horizontal_wrapped(|ui| {
                ui.label(tr("Eaves"));
                ui.add(egui::DragValue::new(&mut r.eaves).speed(10.0).suffix(" mm")).on_hover_text(tr("Height of the underside of the roof over the walls above the ground level of the building; below the ceiling of a level it slopes the ceilings of the rooms there"));
                if r.shape != RoofShape::Flat {
                    ui.label(tr("Pitch"));
                    ui.add(egui::DragValue::new(&mut r.pitch).speed(0.5).range(1.0..=75.0).suffix(" °"));
                }
                ui.label(tr("Overhang"));
                ui.add(egui::DragValue::new(&mut r.overhang).speed(10.0).range(0.0..=3000.0).suffix(" mm"));
                ui.label(tr("Thickness"));
                ui.add(egui::DragValue::new(&mut r.thickness).speed(5.0).range(50.0..=1000.0).suffix(" mm"));
            });
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tr("Roof windows")).weak());
                if icons::text_button(ui, Icon::Plus, tr("Roof window")).on_hover_text(tr("Halfway up the front or right slope; move it to its place")).clicked() {
                    add_window = Some(k);
                }
            });
            let mut gone = None;
            for (w_k, w) in r.windows.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_space(14.0);
                    ui.add(egui::DragValue::new(&mut w.x).speed(10.0).prefix("x ").suffix(" mm"));
                    ui.add(egui::DragValue::new(&mut w.z).speed(10.0).prefix("z ").suffix(" mm"));
                    ui.add(egui::DragValue::new(&mut w.width).speed(5.0).range(200.0..=5000.0).prefix("w "));
                    ui.add(egui::DragValue::new(&mut w.length).speed(5.0).range(200.0..=5000.0).prefix("l ").suffix(" mm"))
                        .on_hover_text(tr("Width along the eaves and length up the slope"));
                    if icons::button(ui, Icon::Delete, tr("Delete roof window")).clicked() {
                        gone = Some(w_k);
                    }
                });
            }
            if let Some(w_k) = gone {
                r.windows.remove(w_k);
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tr("PV arrays")).weak());
                if icons::text_button(ui, Icon::Plus, tr("PV array")).on_hover_text(tr("In the middle of the front or right slope, on a flat roof tilted towards the front; move it to its place")).clicked() {
                    add_pv = Some(k);
                }
            });
            if let Some(a_k) = pv_arrays(ui, r, k, &view) {
                add_string = Some((k, a_k));
            }
        });
        *r = roofs::checked_roof(r.clone());
    }
    if let Some(k) = remove {
        b.roofs.remove(k);
    }
    if let Some(k) = add_window {
        let _ = roofs::add_window(prj, bi, k, None);
    }
    if let Some(k) = add_pv {
        let _ = solar::add_array(prj, bi, k, None);
    }
    if let Some((k, a_k)) = add_string {
        let _ = strings::add_string(prj, bi, k, a_k);
    }
    // The cell temperatures the strings of all arrays are designed for.
    if prj.layout.buildings.iter().flat_map(|b| &b.roofs).any(|r| !r.solar.is_empty()) {
        let d = &mut prj.layout.pv_design;
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(tr("Cell temperatures of the strings")).weak());
            ui.add(egui::DragValue::new(&mut d.t_min).speed(0.5).range(-60.0..=40.0).suffix(" °C")).on_hover_text(tr("Lowest: the highest open-circuit voltage"));
            ui.add(egui::DragValue::new(&mut d.t_mean).speed(0.5).range(-40.0..=70.0).suffix(" °C")).on_hover_text(tr("Mean: typical operation"));
            ui.add(egui::DragValue::new(&mut d.t_max).speed(0.5).range(0.0..=110.0).suffix(" °C")).on_hover_text(tr("Highest: the lowest voltage"));
        });
        d.t_mean = d.t_mean.clamp(d.t_min, d.t_max.max(d.t_min));
    }
    if !sloped.is_empty() {
        ui.label(RichText::new(tr("Rooms under sloped ceilings (lowest – highest ceiling)")).weak());
        for line in sloped {
            ui.label(line);
        }
    }
}

fn links_section(ui: &mut egui::Ui, prj: &mut Project, st: &mut BuildingsUi) {
    ui.label(RichText::new(tr("Links between buildings")).strong());
    let choices = entry_choices(prj);
    let poses = panel::site_poses(prj);
    let lengths: Vec<Option<f32>> = prj.layout.site_links.iter().map(|l| bld::site_link_length(prj, &poses, l)).collect();
    let mut remove = None;
    if !prj.layout.site_links.is_empty() {
        egui::Grid::new("bldlinks").num_columns(6).striped(true).spacing([8.0, 4.0]).show(ui, |ui| {
            for t in [tr("Name"), tr("Kind"), tr("From"), tr("To"), tr("Length"), ""] {
                ui.label(RichText::new(t).weak());
            }
            ui.end_row();
            for (k, l) in prj.layout.site_links.iter_mut().enumerate() {
                ui.add(egui::TextEdit::singleline(&mut l.name).desired_width(50.0));
                egui::ComboBox::from_id_salt(("bldlinkkind", k)).selected_text(l.kind.title()).show_ui(ui, |ui| {
                    for kind in LinkKind::ALL {
                        ui.selectable_value(&mut l.kind, kind, kind.title());
                    }
                });
                ui.label(entry_label(&choices, l.a));
                ui.label(entry_label(&choices, l.b));
                ui.horizontal(|ui| {
                    let mut given = l.length > 0.0;
                    if ui.checkbox(&mut given, tr("given")).on_hover_text(tr("Otherwise along its way from entry to entry")).changed() {
                        l.length = if given { lengths[k].unwrap_or(1000.0).round() } else { 0.0 };
                    }
                    if given {
                        ui.add(egui::DragValue::new(&mut l.length).speed(100.0).range(1.0..=10_000_000.0).suffix(" mm"));
                    } else {
                        ui.label(lengths[k].map_or_else(|| tr("unknown").to_string(), |v| format!("{:.1} m", v / 1000.0)));
                    }
                });
                if icons::button(ui, Icon::Delete, tr("Delete link")).clicked() {
                    remove = Some(k);
                }
                ui.end_row();
            }
        });
    }
    if let Some(k) = remove {
        prj.layout.site_links.remove(k);
    }
    if choices.len() < 2 {
        ui.weak(tr("A link joins two building entries: add building entries to the outer walls of rooms in different buildings (Layout space › Building entry)."));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (salt, value) in [("from", &mut st.link.0), ("to", &mut st.link.1)] {
            egui::ComboBox::from_id_salt(("bldnewlink", salt)).selected_text(entry_label(&choices, *value)).width(180.0).show_ui(ui, |ui| {
                for (id, label) in &choices {
                    ui.selectable_value(value, *id, label.as_str());
                }
            });
        }
        egui::ComboBox::from_id_salt("bldnewlinkkind").selected_text(st.kind.title()).show_ui(ui, |ui| {
            for kind in LinkKind::ALL {
                ui.selectable_value(&mut st.kind, kind, kind.title());
            }
        });
        if icons::text_button(ui, Icon::Plus, tr("Link")).on_hover_text(tr("Cables, tubes and pipes are routed through it from one building to the other")).clicked() {
            match bld::add_site_link(prj, st.link.0, st.link.1, st.kind, Vec::new(), None) {
                Ok(_) => st.status.clear(),
                Err(e) => st.status = e,
            }
        }
    });
}

/// The site plan: the rooms of all buildings seen from above (those of the level shown
/// highlighted), the shafts, the stairs reaching the level shown, the roofs with their
/// windows and the links between the buildings.
fn site_plan_section(ui: &mut egui::Ui, prj: &Project, st: &mut BuildingsUi) {
    let layout = &prj.layout;
    let Some(b) = layout.buildings.get(st.sel) else { return };
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Site plan")).strong());
        let shown = b.levels.iter().find(|l| l.id == st.level).map_or_else(|| tr("All levels").to_string(), |l| l.name.clone());
        egui::ComboBox::from_id_salt("bldplanlevel").selected_text(shown).show_ui(ui, |ui| {
            ui.selectable_value(&mut st.level, 0, tr("All levels"));
            for l in &b.levels {
                ui.selectable_value(&mut st.level, l.id, l.name.as_str());
            }
        });
    });
    let (size, _) = ui.allocate_exact_size(vec2(ui.available_width(), 320.0), Sense::hover());
    let plan = SitePlan { building: Some(st.sel), level: st.level, ..Default::default() };
    site_plan::paint(ui, size, prj, &plan, None);
}
