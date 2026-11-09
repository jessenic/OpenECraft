//! Panel layout workspace: enclosure navigator, 2D mounting plate editor, 3D view
//! and property editor.

use openecraft_i18n::{tr, tr_const, tr_fmt};
use openecraft_ui::icons::{self, Icon};
use crate::editor::{EguiDraw, Palette};
use openecraft_core::geom::{p, P};
use openecraft_core::model::{natural_cmp, Project};
use openecraft_core::panel::{self, Align, Anchor, Enclosure, Face, Fastening, ItemKind, Opening, PanelItem, SitePlace, PLATE_MARGIN, PLATE_THICKNESS, PLATE_Z};
use openecraft_core::parts::{self, PartsDb};
use openecraft_core::parts::vec3::Orbit;
use openecraft_core::render::{self, Draw, Ink};
use openecraft_core::analysis::Analysis;
use openecraft_core::routing3d;
use egui::{vec2, Color32, Key, PointerButton, Pos2, Rect, RichText, Sense, Shape, Stroke, Vec2};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use view3d::default_cam;

mod gpu3d;
mod nav3d;
mod plan;
mod routing_ui;
mod scene3d;
pub(crate) mod site_plan;
mod site_tools;
pub use site_tools::SiteTool;
mod view3d;

#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Select,
    Rail,
    Duct,
    Busbar,
    Cutout,
    PlaceDevice(String),
    /// Accessory of this part.
    Accessory(String),
    /// 3D macro of this name.
    Macro(String),
    /// Routing path over the mounting plate: click the corners.
    RoutingPath,
    /// Routing reservation over the mounting plate: click two corners.
    Reservation,
    /// Pipe run on a wall, the ceiling or the floor of a room: click the corners.
    PipeRun,
    /// Walls of a room in its floor plan: click the corners.
    Walls,
    /// Door or window in a wall of a room: click where it goes.
    Opening(Opening),
    /// Inner walls of a room in its floor plan: click their ends.
    InnerWalls,
}

impl Tool {
    /// Item kind and default part of the profile tools.
    fn profile(&self) -> Option<(ItemKind, &'static str)> {
        match self {
            Tool::Rail => Some((ItemKind::Rail, panel::RAIL_PART)),
            Tool::Duct => Some((ItemKind::Duct, panel::DUCT_PART)),
            Tool::Busbar => Some((ItemKind::Busbar, panel::BUSBAR_PART)),
            _ => None,
        }
    }
}

/// The next corner of a line drawn at right angles: along the longer direction from
/// the last one.
fn ortho(last: Option<P>, q: P) -> P {
    match last {
        Some(l) if (q.x - l.x).abs() >= (q.y - l.y).abs() => p(q.x, l.y),
        Some(l) => p(l.x, q.y),
        None => q,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Front,
    ThreeD,
    /// Floor plan of a room.
    Plan,
}

/// File of an NC export: drawing (DXF, DWG) or drilling list (CSV).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NcFormat {
    Dxf,
    Dwg,
    Csv,
}

struct MoveDrag {
    start: P,
    originals: Vec<(u64, f32, f32)>,
}

/// Climate settings and result of an enclosure (IEC/TR 60890).
fn climate_section(ui: &mut egui::Ui, prj: &mut Project, ei: usize) {
    use openecraft_core::climate::{self, Cooling, Installation, Material};
    let result = climate::calculate(prj, &prj.layout.enclosures[ei]);
    let c = &mut prj.layout.enclosures[ei].climate;
    egui::CollapsingHeader::new(tr("Climate (IEC 60890)")).id_salt("climate").show(ui, |ui| {
        egui::Grid::new("pclimate").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Installation"));
            egui::ComboBox::from_id_salt("installation").selected_text(c.installation.title()).show_ui(ui, |ui| {
                for i in Installation::ALL {
                    ui.selectable_value(&mut c.installation, i, i.title());
                }
            });
            ui.end_row();
            ui.label(tr("Material"));
            egui::ComboBox::from_id_salt("material").selected_text(c.material.title()).show_ui(ui, |ui| {
                for m in Material::ALL {
                    ui.selectable_value(&mut c.material, m, m.title());
                }
            });
            ui.end_row();
            ui.label(tr("Ambient temperature"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut c.ambient_min).speed(0.5).range(-50.0..=80.0).suffix(" °C"));
                ui.label("…");
                ui.add(egui::DragValue::new(&mut c.ambient_max).speed(0.5).range(-50.0..=80.0).suffix(" °C"));
            });
            ui.end_row();
            ui.label(tr("Inside temperature"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut c.inside_min).speed(0.5).range(-50.0..=80.0).suffix(" °C"));
                ui.label("…");
                ui.add(egui::DragValue::new(&mut c.inside_max).speed(0.5).range(-50.0..=80.0).suffix(" °C"));
            });
            ui.end_row();
            ui.label(tr("Additional power loss"));
            ui.add(egui::DragValue::new(&mut c.extra_loss).speed(1.0).range(0.0..=100000.0).suffix(" W"))
                .on_hover_text(tr("Power loss not covered by the parts (tech data Power loss: 7.5 W)"));
            ui.end_row();
            ui.label(tr("Altitude"));
            ui.add(egui::DragValue::new(&mut c.altitude).speed(10.0).range(0.0..=5000.0).suffix(" m")).on_hover_text(tr("Thinner air needs more air flow"));
            ui.end_row();
            ui.label(tr("Relative humidity"));
            ui.add(egui::DragValue::new(&mut c.humidity).speed(1.0).range(0.0..=100.0).suffix(" %")).on_hover_text(tr("At the highest ambient temperature; the heater keeps the dew point. 0 = not considered"));
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut c.sealed, tr("Keep the degree of protection")).on_hover_text(tr("No filter fans: an air/air heat exchanger or a cooling unit"));
            ui.end_row();
            ui.label(tr("Power loss"));
            ui.label(format!("{:.0} W", result.loss));
            ui.end_row();
            ui.label(tr("Effective surface"));
            ui.label(format!("{:.2} m²", result.surface));
            ui.end_row();
            ui.label(tr("Inside without cooling"));
            let hot = result.cooling != Cooling::None;
            let text = format!("{:.1} °C", result.inside);
            if hot {
                ui.colored_label(crate::style::lock(), text);
            } else {
                ui.label(text);
            }
            ui.end_row();
            ui.label(tr("Cooling"));
            ui.label(result.cooling.text());
            ui.end_row();
            ui.label(tr("Heating"));
            ui.label(if result.heating > 0.0 { format!("{:.0} W", result.heating) } else { "–".into() });
            ui.end_row();
            if let Some(d) = result.dew_point {
                ui.label(tr("Dew point"));
                ui.label(format!("{d:.1} °C"));
                ui.end_row();
            }
        });
        if !result.without_data.is_empty() {
            ui.weak(tr_fmt!("Without power loss data: {}", result.without_data.join(", ")));
        }
    });
}

/// Collision class chooser: any class, the built-in classes and the project's own.
fn class_combo(ui: &mut egui::Ui, salt: &str, value: &mut String, custom: &[String]) {
    egui::ComboBox::from_id_salt(salt).selected_text(panel::class_title(value)).width(100.0).show_ui(ui, |ui| {
        ui.selectable_value(value, "*".to_string(), panel::class_title("*"));
        for (k, _) in panel::CLASSES {
            ui.selectable_value(value, k.to_string(), panel::class_title(k));
        }
        for k in custom {
            ui.selectable_value(value, k.clone(), k.as_str());
        }
    });
}

/// Classes named on items or in rules that are not built in.
fn custom_classes(prj: &Project) -> Vec<String> {
    let mut v: Vec<String> = prj.layout.enclosures.iter().flat_map(|e| e.items.iter().map(|i| i.class.trim().to_ascii_lowercase())).collect();
    v.extend(prj.layout.checks.classes.iter().flat_map(|r| [r.a.clone(), r.b.clone()]));
    v.retain(|k| !k.trim().is_empty() && k.trim() != "*" && !panel::CLASSES.iter().any(|c| c.0 == k));
    v.sort();
    v.dedup();
    v
}

/// Rules and results of the layout checks of an enclosure: clearances, service
/// space, heights above the floor, collision classes, thermal and manufacturing.
fn checks_section(ui: &mut egui::Ui, prj: &mut Project, ei: usize, findings: &[panel::Finding]) {
    let temps = {
        let e = &prj.layout.enclosures[ei];
        panel::thermal::device_temperatures(prj, e, &panel::thermal::heat_map(prj, e))
    };
    let custom = custom_classes(prj);
    egui::CollapsingHeader::new(tr("Layout checks")).id_salt("checks").show(ui, |ui| {
        let e = &mut prj.layout.enclosures[ei];
        ui.horizontal(|ui| {
            let mut known = e.elevation.is_some();
            let tip = tr("Bottom of the enclosure (without plinth) above the floor; heights are checked when it is known or the enclosure stands on a plinth");
            if ui.checkbox(&mut known, tr("Height above floor")).on_hover_text(tip).changed() {
                e.elevation = known.then_some(0.0);
            }
            if let Some(h) = &mut e.elevation {
                ui.add(egui::DragValue::new(h).speed(5.0).range(0.0..=5000.0).suffix(" mm"));
            }
        });
        let r = &mut prj.layout.checks;
        let row = |ui: &mut egui::Ui, label: &str, v: &mut f32, max: f32, suffix: &str, tip: &str| {
            ui.label(label);
            let resp = ui.add(egui::DragValue::new(v).speed(1.0).range(0.0..=max).suffix(suffix));
            if !tip.is_empty() {
                resp.on_hover_text(tip);
            }
            ui.end_row();
        };
        let range = |ui: &mut egui::Ui, label: &str, a: &mut f32, b: &mut f32, tip: &str| {
            ui.label(label);
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(a).speed(5.0).range(0.0..=5000.0));
                ui.label("…");
                ui.add(egui::DragValue::new(b).speed(5.0).range(0.0..=5000.0).suffix(" mm"));
            })
            .response
            .on_hover_text(tip);
            ui.end_row();
        };
        ui.label(RichText::new(tr("Clearances")).strong());
        egui::Grid::new("pchecks_clear").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            row(ui, tr("Gap between devices"), &mut r.device_gap, 200.0, " mm", tr("Mounting clearance side by side; parts add theirs (Clearance left / right)"));
            row(ui, tr("Distance to cable ducts"), &mut r.duct_gap, 200.0, " mm", tr("Wiring space between devices and ducts"));
            row(ui, tr("Distance to the plate edge"), &mut r.edge_distance, 200.0, " mm", "");
            row(ui, tr("Service space in front"), &mut r.service_space, 500.0, " mm", tr("Free space in front of every device on the plate up to the door; parts add theirs (Service space)"));
        });
        ui.label(RichText::new(tr("Heights above floor (IEC 60204-1)")).strong());
        egui::Grid::new("pchecks_height").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            range(ui, tr("Devices"), &mut r.operating_min, &mut r.operating_max, tr("All devices between these heights (11.2.1); 0 … 0 switches the check off"));
            range(ui, tr("Actuators on the door"), &mut r.actuator_min, &mut r.actuator_max, tr("Recommended height of manual actuators (10.1.2)"));
            row(ui, tr("Terminals at least"), &mut r.terminal_min, 2000.0, " mm", "");
        });
        ui.label(RichText::new(tr("Thermal")).strong());
        egui::Grid::new("pchecks_thermal").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            row(ui, tr("Air at devices at most"), &mut r.device_temperature, 150.0, " °C", tr("For parts without temperature data (Max temperature, Operating temperature); 0: not checked"));
            ui.label(tr("Stratification (factor c)"));
            ui.add(egui::DragValue::new(&mut r.stratification).speed(0.01).range(1.0..=2.0)).on_hover_text(tr("Temperature rise at the roof against the middle (IEC/TR 60890)"));
            ui.end_row();
        });
        ui.label(RichText::new(tr("Manufacturing")).strong());
        egui::Grid::new("pchecks_mfg").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            row(ui, tr("Web between holes"), &mut r.hole_web, 50.0, " mm", "");
            row(ui, tr("Hole to sheet edge"), &mut r.hole_edge, 50.0, " mm", "");
            row(ui, tr("Web at cutouts"), &mut r.cutout_web, 100.0, " mm", "");
            row(ui, tr("Shortest profile"), &mut r.min_profile, 500.0, " mm", tr("Rails, ducts and busbars shorter than this are hard to cut"));
            row(ui, tr("Stock length"), &mut r.stock_length, 10000.0, " mm", tr("For parts without Stock length; longer profiles have to be joined"));
        });
        ui.label(RichText::new(tr("Collision classes")).strong()).on_hover_text(tr("Distances between classes of items, or classes that may overlap"));
        let mut remove = None;
        egui::Grid::new("pchecks_classes").num_columns(5).spacing([6.0, 4.0]).show(ui, |ui| {
            for (i, c) in r.classes.iter_mut().enumerate() {
                class_combo(ui, &format!("cls_a{i}"), &mut c.a, &custom);
                class_combo(ui, &format!("cls_b{i}"), &mut c.b, &custom);
                ui.add_enabled(!c.ignore, egui::DragValue::new(&mut c.distance).speed(1.0).range(0.0..=1000.0).suffix(" mm"));
                ui.checkbox(&mut c.ignore, tr("may overlap"));
                if icons::button(ui, Icon::Delete, tr("Delete")).clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = remove {
            r.classes.remove(i);
        }
        if icons::text_button(ui, Icon::Plus, tr("Add rule")).clicked() {
            r.classes.push(panel::ClassRule::distance("power", "signal", 50.0));
        }
        if !temps.is_empty() {
            egui::CollapsingHeader::new(tr("Air temperature at the devices")).id_salt("ptemps").show(ui, |ui| {
                egui::Grid::new("ptemps_grid").striped(true).num_columns(3).show(ui, |ui| {
                    for t in &temps {
                        ui.label(RichText::new(&t.tag).monospace());
                        let text = format!("{:.0} °C", t.temperature);
                        if t.limit.is_some_and(|l| t.temperature > l) {
                            ui.colored_label(crate::style::error(), text);
                        } else {
                            ui.label(text);
                        }
                        ui.label(t.limit.map(|l| tr_fmt!("max. {} °C", format!("{l:.0}"))).unwrap_or_default());
                        ui.end_row();
                    }
                });
            });
        }
        if findings.is_empty() {
            ui.weak(tr("No problems found"));
        }
        for f in findings {
            let color = if f.severity == openecraft_core::issue::Severity::Info { ui.visuals().weak_text_color() } else { crate::style::lock() };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(f.kind.title()).strong().color(color));
                ui.label(&f.message);
            });
        }
    });
}

struct NewEnclosure {
    name: String,
    part: String,
    /// Enclosure system the part list is narrowed to.
    system: String,
    width: f32,
    height: f32,
    depth: f32,
}

/// Rectangular pattern of copies of the selection.
struct PatternDialog {
    nx: usize,
    ny: usize,
    px: f32,
    py: f32,
}

/// Enclosure accessories of the parts database, the project and the catalog.
fn accessory_parts(db: &PartsDb, prj: &Project) -> Vec<parts::Part> {
    let mut v: Vec<parts::Part> = Vec::new();
    for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()) {
        if parts::mechanics::ACCESSORY_SUBGROUPS.contains(&p.subgroup.as_str()) && !v.iter().any(|x| x.number == p.number) {
            v.push(p.clone());
        }
    }
    v.sort_by(|a, b| a.subgroup.cmp(&b.subgroup).then(natural_cmp(&a.number, &b.number)));
    v
}

/// Choice of the part for a rail, duct or busbar system: generic parts, then those of
/// the parts store and the project. Returns the part picked.
fn profile_part_combo(ui: &mut egui::Ui, salt: &str, choices: &[panel::ProfileChoice], current: &str) -> Option<String> {
    let mut picked = None;
    egui::ComboBox::from_id_salt(salt)
        .selected_text(current.trim_start_matches("GEN."))
        .width(150.0)
        .show_ui(ui, |ui| {
            for (generic, heading) in [(true, tr("Generic")), (false, tr("Parts store"))] {
                let list: Vec<&panel::ProfileChoice> = choices.iter().filter(|c| c.generic == generic).collect();
                if list.is_empty() {
                    continue;
                }
                ui.label(RichText::new(heading).weak());
                for c in list {
                    if ui.selectable_label(c.number == current, &c.number).on_hover_text(&c.description).clicked() {
                        picked = Some(c.number.clone());
                    }
                }
            }
        })
        .response
        .on_hover_text(tr("Part of the profile: generic types or parts from the parts store"));
    picked.filter(|p| p != current)
}

/// The item and the other members of its group (assembly).
fn with_group(e: &Enclosure, id: u64) -> Vec<u64> {
    let ids=match e.item(id).map(|i| i.group) {
        Some(g) if g != 0 => panel::group_members(e, g),
        _ => vec![id],
    };
    panel::mounted_items(e,&ids)
}

/// Placement along one axis: kind and distances.
fn align_editor(ui: &mut egui::Ui, salt: &str, a: &mut Align) {
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(salt).selected_text(a.title()).width(90.0).show_ui(ui, |ui| {
            for k in Align::KINDS {
                if ui.selectable_label(a.same_kind(k), k.title()).clicked() && !a.same_kind(k) {
                    *a = k;
                }
            }
        });
        match a {
            Align::Free => {}
            Align::Start(v) | Align::Center(v) | Align::End(v) => {
                ui.add(egui::DragValue::new(v).speed(1.0).suffix(" mm"));
            }
            Align::Stretch(s, t) => {
                ui.add(egui::DragValue::new(s).speed(1.0));
                ui.add(egui::DragValue::new(t).speed(1.0).suffix(" mm"));
            }
        }
    });
}

pub struct PanelUi {
    pub enclosure: usize,
    /// The current enclosure is a room (walls and ceiling, no plate or door).
    pub in_room: bool,
    pub tool: Tool,
    pub view: View,
    /// Face shown in the front view (mounting plate or door).
    pub face: Face,
    /// The door is seen from the inside (mirrored, hinges on the right).
    pub door_inside: bool,
    pub selection: BTreeSet<u64>,
    /// Selected wire of the virtual wiring (from, to).
    pub selected_wire: Option<(String, String)>,
    pub show_wires: bool,
    /// Wires marked as wired in the wiring assistant (drawn green in 3D).
    pub done_wires: HashSet<(String, String)>,
    /// Items marked as mounted in the mounting assistant (drawn green in 3D).
    pub done_items: HashSet<u64>,
    /// The door is (being) opened in the 3D view.
    pub door_open: bool,
    /// Current opening angle of the door (radians), animated towards `door_open`.
    door_angle: f32,
    /// Cable ducts coloured by their fill level.
    pub show_fill: bool,
    /// Estimated air temperature over the mounting plate.
    pub show_heat: bool,
    /// The 3D view shows the enclosures bayed to the current one as well.
    pub show_lineup: bool,
    /// The 3D view shows all enclosures at their places in the room.
    pub show_site: bool,
    /// The site (the root of the navigator) is shown instead of an enclosure: the site
    /// plan of the buildings, or all of the plant in the 3D view.
    pub site: bool,
    site_plan: site_plan::SitePlan,
    /// The tool of the site plan, the points it has taken, and the corner of the plot
    /// boundary dragged with the boundary before.
    pub site_tool: SiteTool,
    site_points: Vec<[f32; 2]>,
    site_drag: Option<(usize, Vec<[f32; 2]>)>,
    /// The 3D view of a room in a building shows the whole building: its rooms, floor
    /// slabs, stairs and roofs.
    pub show_building: bool,
    /// The installation zones of the rooms are shown, see-through.
    pub show_zones: bool,
    /// The walls and floors of the rooms are opened (seen through) to show the
    /// flush-mounted lines in them.
    pub open_walls: bool,
    /// The 3D view leaves out the walls between the eye and the inside of a room or
    /// enclosure.
    pub hide_front_walls: bool,
    /// The 3D view leaves out the ceilings of the rooms.
    pub hide_ceiling: bool,
    /// The 3D view leaves out the covers of the cable ducts.
    pub open_ducts: bool,
    /// Opacity of the walls seen from the outside in the 3D view, 0 … 1.
    pub wall_opacity: f32,
    /// The 3D view is drawn by the graphics card where it can (set by the application).
    pub gpu_3d: bool,
    gpu: gpu3d::Gpu,
    /// What the 3D view shows, kept while it does not change.
    scene: Option<Arc<scene3d::Scene3d>>,
    /// The wires, cables and tubes of the 3D view laid out, and the layout being worked
    /// out on another thread.
    layouts: scene3d::Layouts,
    offset: Vec2,
    zoom: f32,
    fit_pending: bool,
    start: Option<P>,
    drag: Option<MoveDrag>,
    cam: Option<Orbit>,
    /// View the 3D camera glides to, and the camera as this frame left it.
    cam_goal: Option<(Orbit, Orbit)>,
    device_filter: String,
    new_enclosure: Option<NewEnclosure>,
    pub status: String,
    /// Requests for the application.
    pub goto_schematic: Option<String>,
    pub add_page: Option<u64>,
    /// Parts management opened (to mark parts for wall mounting).
    pub open_parts: bool,
    /// NC export of an enclosure face.
    pub export: Option<(usize, Face, NcFormat)>,
    pattern: Option<PatternDialog>,
    /// Name of the 3D macro being saved from the selection.
    save_macro: Option<String>,
    /// Gap between devices lined up on a rail.
    rail_gap: f32,
    /// Parts chosen for new rails, ducts and busbars (kind, on the door).
    profile_parts: HashMap<(ItemKind, bool), String>,
    /// Diameter of new cable passages.
    passage_diameter: f32,
    /// Corners of the routing path being drawn (plate coordinates).
    path_points: Vec<P>,
    /// New routing paths: height above the plate, width and kind.
    path_height: f32,
    path_width: f32,
    path_kind: routing3d::PathKind,
    /// What new reservations keep the space for.
    reserve_mode: routing3d::ReserveMode,
    /// Selected routed wire, cable or tube (its key).
    pub selected_route: Option<String>,
    /// Selected routing path or reservation (its id).
    selected_object: Option<u64>,
    /// The layout view is the document worked in: it takes the keys.
    pub keys_active: bool,
    /// Corners of the walls being drawn in the floor plan (x, z).
    wall_points: Vec<P>,
    plan_drag: Option<plan::PlanDrag>,
    /// Line-up selected in the floor plan (its first cabinet).
    plan_cabinet: Option<usize>,
    /// Size of the 2D view in the last frame: a resized view keeps its centre.
    view_size: egui::Vec2,
    /// An enclosure or room with content waiting for the confirmation of its removal.
    confirm_remove: Option<usize>,
    /// Inner wall selected in the floor plan (its id).
    plan_inner: Option<u64>,
    /// Level selected in the navigator (its id): new rooms go onto it.
    nav_level: Option<u64>,
    /// Circuits of the generic heating circuit manifold chosen in the installation menu.
    manifold_circuits: usize,
}

/// The groups of the installation menu: title and the subgroups of their parts.
const INSTALLATION: [(&str, &[&str]); 3] = [
    (tr_const!("Socket outlets, data and antenna outlets"), &["Socket outlet", "Data outlet", "Antenna outlet"]),
    (tr_const!("Switches, push buttons and room thermostats"), &["Switch", "Push button", "Blind switch", "Dimmer", "Glass push button", "Room thermostat"]),
    (tr_const!("Heating circuit manifolds and control boxes"), &["Heating circuit manifold", "Control box"]),
];

/// The parts of the catalogs shipped with OpenECraft, read once.
fn bundled_parts() -> &'static [parts::Part] {
    static PARTS: std::sync::OnceLock<Vec<parts::Part>> = std::sync::OnceLock::new();
    PARTS.get_or_init(|| parts::bundled::all().unwrap_or_default().into_iter().flat_map(|b| b.parts).collect())
}

/// A room dragged in the navigator (the drag payload).
#[derive(Clone, Copy, PartialEq, Debug)]
struct DraggedRoom(usize);

/// What the navigator does with the levels of the buildings.
#[derive(Clone, Copy, PartialEq, Debug)]
enum NavAction {
    /// Selects the level for new rooms, or unselects it.
    Level(u64),
    NewRoomOn(u64),
    /// Puts a room onto a level.
    Put(usize, u64),
    /// Takes a room out of its building.
    TakeOut(usize),
    /// Shows a room: in the 3D view when it is shown, otherwise its floor plan.
    Open(usize),
    /// Shows the site, with a building highlighted.
    Site(Option<usize>),
    NewBuilding,
    /// A level on top of a building, or below its lowest one (`true`).
    NewLevel(usize, bool),
    /// A tool of the site plan.
    SiteTool(SiteTool),
}

/// A heading of the navigator: the site, a building (index) or a level (id).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Heading {
    Site,
    Building(usize),
    Level(u64),
}

impl Default for PanelUi {
    fn default() -> Self {
        PanelUi {
            enclosure: 0,
            in_room: false,
            tool: Tool::Select,
            view: View::Front,
            face: Face::Plate,
            door_inside: false,
            selection: BTreeSet::new(),
            selected_wire: None,
            show_wires: true,
            done_wires: HashSet::new(),
            done_items: HashSet::new(),
            door_open: true,
            door_angle: DOOR_OPEN_ANGLE,
            show_fill: true,
            show_heat: false,
            show_lineup: true,
            show_site: false,
            site: false,
            site_plan: site_plan::SitePlan::default(),
            site_tool: SiteTool::Select,
            site_points: Vec::new(),
            site_drag: None,
            show_building: false,
            show_zones: false,
            open_walls: false,
            hide_front_walls: false,
            hide_ceiling: false,
            open_ducts: false,
            wall_opacity: WALL_OPACITY,
            gpu_3d: false,
            gpu: gpu3d::Gpu::default(),
            scene: None,
            layouts: scene3d::Layouts::default(),
            offset: vec2(20.0, 20.0),
            zoom: 0.5,
            fit_pending: true,
            start: None,
            drag: None,
            cam: None,
            cam_goal: None,
            device_filter: String::new(),
            new_enclosure: None,
            status: String::new(),
            goto_schematic: None,
            add_page: None,
            open_parts: false,
            export: None,
            pattern: None,
            save_macro: None,
            rail_gap: 0.0,
            profile_parts: HashMap::new(),
            passage_diameter: panel::PASSAGE_DIAMETER,
            path_points: Vec::new(),
            path_height: 50.0,
            path_width: 50.0,
            path_kind: routing3d::PathKind::Any,
            reserve_mode: routing3d::ReserveMode::Keepout,
            selected_route: None,
            selected_object: None,
            keys_active: true,
            wall_points: Vec::new(),
            plan_drag: None,
            plan_cabinet: None,
            view_size: egui::Vec2::ZERO,
            confirm_remove: None,
            plan_inner: None,
            nav_level: None,
            manifold_circuits: 6,
        }
    }
}

/// How a wire is drawn in the 3D view: in its own colour (IEC 60757 code, the second
/// colour of a two-colour code as a stripe), in the colour of the selection or of a
/// wired wire, dimmed while another wire is selected; with a least width in pixels
/// (otherwise its outer diameter in perspective).
pub(super) fn wire_look(code: &str, selected: bool, done: bool, other_selected: bool) -> (Color32, Option<Color32>, f32) {
    let rgb = |c: [u8; 3]| Color32::from_rgb(c[0], c[1], c[2]);
    let (own, stripe) = match openecraft_core::potentials::color_parts(code).as_slice() {
        [a] => (rgb(*a), None),
        [a, b] => (rgb(*a), Some(rgb(*b))),
        _ => (WIRE_COLOR, None),
    };
    if selected {
        (WIRE_HIGHLIGHT, None, 3.5)
    } else if done {
        (if other_selected { WIRE_DONE.gamma_multiply(0.5) } else { WIRE_DONE }, None, 1.5)
    } else if other_selected {
        (own.gamma_multiply(0.3), stripe.map(|s| s.gamma_multiply(0.3)), 1.0)
    } else {
        (own, stripe, 1.5)
    }
}

/// Opacity of the walls seen from the outside in the 3D view, unless set otherwise.
pub const WALL_OPACITY: f32 = 0.6;

/// Drawing layer of enclosure walls that are seen from the outside.
const OUTER_WALLS: u8 = 6;

const WIRE_COLOR: Color32 = Color32::from_rgb(40, 110, 230);
const WIRE_DONE: Color32 = Color32::from_rgb(50, 170, 80);
/// Opening angle of the door in the 3D view.
const DOOR_OPEN_ANGLE: f32 = 1.85;
/// Selected wire and the devices it connects (same colour as potential highlighting).
const WIRE_HIGHLIGHT: Color32 = Color32::from_rgb(230, 40, 200);
/// Routing paths and reservations.
const PATH_COLOR: Color32 = Color32::from_rgb(220, 180, 40);
const RESERVE_COLOR: Color32 = Color32::from_rgb(220, 70, 60);

impl PanelUi {
    /// Findings of the layout checks of enclosure `ei`: those the analysis keeps, made
    /// again while a change of the layout is in progress (a drag).
    fn findings(&self, prj: &Project, an: &Analysis, ei: usize) -> Arc<Vec<panel::Finding>> {
        match prj.layout.enclosures.get(ei) {
            Some(e) if self.busy() => Arc::new(panel::validate(prj, e)),
            _ => an.layout_findings(prj, ei),
        }
    }

    /// The graphics card could not draw the 3D view: it is drawn by the processor.
    pub fn gpu_failed(&self) -> bool {
        self.gpu.failed()
    }

    /// The door of the 3D view is still turning to where it is to be.
    fn door_turning(&self) -> bool {
        let target = if self.door_open { DOOR_OPEN_ANGLE } else { 0.0 };
        (self.door_angle - target).abs() > 1e-3
    }

    /// True while a gesture is in progress (no undo step yet).
    pub fn busy(&self) -> bool {
        self.start.is_some() || self.drag.is_some() || !self.path_points.is_empty() || !self.wall_points.is_empty() || self.plan_drag.is_some()
    }

    fn select_item(&mut self, id: u64) {
        self.selection = [id].into_iter().collect();
        self.selected_wire = None;
        self.selected_route = None;
    }

    /// Selects an item with the other members of its assembly.
    fn select_assembly(&mut self, e: &Enclosure, id: u64) {
        self.selection = with_group(e, id).into_iter().collect();
        self.selected_wire = None;
        self.selected_route = None;
    }

    fn select_wire(&mut self, key: (String, String)) {
        self.selected_wire = Some(key);
        self.selection.clear();
        self.selected_route = None;
    }

    /// Clears the selection: items, the wire and the cable or tube.
    fn deselect(&mut self) {
        self.selection.clear();
        self.selected_wire = None;
        self.selected_route = None;
    }

    /// Index of the selected wire in the current analysis.
    fn wire_index(&self, an: &Analysis) -> Option<usize> {
        let key = self.selected_wire.as_ref()?;
        an.wiring.iter().position(|w| &w.key() == key)
    }

    /// Selects the next wire among `candidates` (cycling on repeated clicks).
    fn pick_wire(&mut self, an: &Analysis, candidates: &[usize]) {
        let current = self.wire_index(an);
        if let Some(i) = panel::next_candidate(candidates, current) {
            let w = &an.wiring[i];
            self.select_wire(w.key());
            self.status = if candidates.len() > 1 {
                let pos = candidates.iter().position(|c| *c == i).unwrap_or(0) + 1;
                tr_fmt!("Wire {} – {}  ({} of {} here, click again for the next)", w.from, w.to, pos, candidates.len())
            } else {
                tr_fmt!("Wire {} – {}  {} mm", w.from, w.to, format!("{:.0}", w.length))
            };
        }
    }

    fn hidden(&self, it: &PanelItem) -> bool {
        hidden_behind_door(self.door_inside, it)
    }

    /// Tools of the face shown: no rails or busbars on the door, cable ducts only on its
    /// inside, cutouts from the front; no busbars on the walls.
    fn tool_allowed(&self, t: &Tool) -> bool {
        if *t == Tool::PipeRun {
            return self.in_room;
        }
        // Routing paths and reservations are drawn over the mounting plate.
        if matches!(t, Tool::RoutingPath | Tool::Reservation) {
            return self.face == Face::Plate;
        }
        if matches!(t, Tool::Walls | Tool::InnerWalls | Tool::Opening(_)) {
            return self.in_room;
        }
        // Rooms: rails, ducts, devices and wall-mounted parts on the walls and the ceiling.
        if self.in_room {
            return !matches!(t, Tool::Busbar | Tool::Cutout);
        }
        if self.face.is_wall() {
            return !matches!(t, Tool::Busbar);
        }
        !matches!(
            (self.face, self.door_inside, t),
            (Face::Door, _, Tool::Rail | Tool::Busbar) | (Face::Door, false, Tool::Duct) | (Face::Door, true, Tool::Cutout)
        )
    }

    /// Item kind and part of the profile tool on the face shown: the part chosen for it,
    /// otherwise the default (the door gets the smaller door duct).
    fn profile_here(&self) -> Option<(ItemKind, String)> {
        self.profile_here_for(&self.tool)
    }

    fn profile_here_for(&self, tool: &Tool) -> Option<(ItemKind, String)> {
        let (kind, default) = match tool.profile()? {
            (ItemKind::Duct, _) if self.face == Face::Door => (ItemKind::Duct, panel::DOOR_DUCT_PART),
            x => x,
        };
        let chosen = self.profile_parts.get(&(kind, self.face == Face::Door));
        Some((kind, chosen.cloned().unwrap_or_else(|| default.to_string())))
    }

    /// Part for the profiles of this kind placed from now on (on the face shown).
    fn choose_profile_part(&mut self, kind: ItemKind, part: &str) {
        self.profile_parts.insert((kind, self.face == Face::Door), part.to_string());
    }

    fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.start = None;
        self.path_points.clear();
        self.wall_points.clear();
    }

    fn enclosure_index(&mut self, prj: &Project) -> Option<usize> {
        if prj.layout.enclosures.is_empty() {
            self.in_room = false;
            return None;
        }
        self.enclosure = self.enclosure.min(prj.layout.enclosures.len() - 1);
        let e = &prj.layout.enclosures[self.enclosure];
        self.in_room = e.is_room();
        let faces = e.faces();
        if !faces.contains(&self.face) {
            self.show_face(faces[0], false);
        }
        if !self.in_room && self.view == View::Plan {
            self.view = View::Front;
        }
        Some(self.enclosure)
    }

    pub fn reset_camera(&mut self) {
        self.cam = None;
    }

    /// Shows another enclosure (a cabinet clicked in its room), fitted into the view.
    fn switch_enclosure(&mut self, ei: usize) {
        if self.enclosure != ei {
            self.enclosure = ei;
            self.fit_pending = true;
            self.cam = None;
        }
    }

    /// Selects a wire and turns the 3D camera towards it (wiring assistant).
    pub fn focus_wire(&mut self, prj: &Project, w: &panel::RoutedWire) {
        let Some(e) = prj.layout.enclosures.get(w.enclosure) else { return };
        if self.enclosure != w.enclosure {
            self.cam = None;
        }
        self.enclosure = w.enclosure;
        self.select_wire(w.key());
        if w.door.0.is_some() || w.door.1.is_some() {
            self.door_open = true;
        }
        let pts = &w.route.points;
        if pts.is_empty() {
            return;
        }
        let n = pts.len() as f32;
        let c = p(pts.iter().map(|q| q.x).sum::<f32>() / n, pts.iter().map(|q| q.y).sum::<f32>() / n);
        self.aim_camera(prj, w.enclosure, [PLATE_MARGIN + c.x, e.height - PLATE_MARGIN - c.y, PLATE_Z]);
    }

    /// Turns a zoomed-in 3D camera to `at` (enclosure coordinates). A new camera, or one
    /// zoomed out as far as the whole view, frames everything shown instead: centred on
    /// `at` its zoom would push the rest of the line-up out of the view.
    fn aim_camera(&mut self, prj: &Project, enclosure: usize, at: [f32; 3]) {
        let shown = self.shown_3d(prj, enclosure);
        let pose = shown.iter().find(|s| s.0 == enclosure).map(|s| s.1).unwrap_or_default();
        let full = default_cam(prj, &shown);
        match &mut self.cam {
            Some(cam) if cam.dist < full.dist => cam.target = pose.apply(at),
            Some(cam) => cam.target = full.target,
            None => self.cam = Some(full),
        }
    }

    /// Selects a panel item and turns the 3D view to it (mounting assistant).
    pub fn focus_item(&mut self, prj: &Project, enclosure: usize, id: u64) {
        let Some(e) = prj.layout.enclosures.get(enclosure) else { return };
        let Some(it) = e.items.iter().find(|i| i.id == id) else { return };
        if self.enclosure != enclosure {
            self.cam = None;
        }
        self.enclosure = enclosure;
        self.face = it.face;
        self.select_item(id);
        if it.face == Face::Door {
            self.door_open = true;
        }
        let at = [PLATE_MARGIN + it.x + it.w / 2.0, e.height - PLATE_MARGIN - it.y - it.h / 2.0, PLATE_Z];
        self.aim_camera(prj, enclosure, at);
    }

    /// Shows a placed device (from the schematic): selected on its face of its
    /// cabinet or room, a device on the floor of a room in the floor plan; in the 3D
    /// view with the camera close to it.
    pub fn focus_device(&mut self, prj: &Project, tag: &str, three_d: bool) -> bool {
        let Some((ei, it)) = panel::locate(prj, tag) else { return false };
        let e = &prj.layout.enclosures[ei];
        if self.enclosure != ei {
            self.cam = None;
        }
        self.enclosure = ei;
        self.in_room = e.is_room();
        self.face = it.face;
        self.select_item(it.id);
        self.fit_pending = true;
        if !three_d {
            self.view = if self.in_room && it.face == Face::Bottom { View::Plan } else { View::Front };
            return true;
        }
        self.view = View::ThreeD;
        if it.face == Face::Door {
            self.door_open = true;
        }
        let (lo, hi) = panel::item_box(e, it);
        let at = [(lo[0] + hi[0]) / 2.0, e.height - (lo[1] + hi[1]) / 2.0, (lo[2] + hi[2]) / 2.0];
        let shown = self.shown_3d(prj, ei);
        let pose = shown.iter().find(|s| s.0 == ei).map(|s| s.1).unwrap_or_default();
        let size = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f32::max);
        let mut cam = view3d::default_cam(prj, &shown);
        cam.target = pose.apply(at);
        cam.dist = (size * 6.0).clamp(600.0, cam.dist.max(600.0));
        self.cam = Some(cam);
        true
    }

    // ------------------------------------------------------------------ navigator

    pub fn navigator(&mut self, ui: &mut egui::Ui, prj: &mut Project, db: &PartsDb) {
        ui.horizontal_wrapped(|ui| {
            ui.heading(tr("Enclosures and rooms"));
            if openecraft_ui::icons::button(ui, openecraft_ui::icons::Icon::Plus, tr("New enclosure")).clicked() {
                self.new_enclosure_dialog(prj, db);
            }
            if openecraft_ui::icons::button(ui, openecraft_ui::icons::Icon::Wall, tr("New room")).clicked() {
                self.new_room(prj);
            }
        });
        self.enclosure_tree(ui, prj);
        self.navigator_devices(ui, prj, db);
    }

    /// The buildings, rooms and enclosures of the navigator.
    fn enclosure_tree(&mut self, ui: &mut egui::Ui, prj: &mut Project) {
        let mut remove = None;
        // Buildings with their levels (the highest first) and the rooms on them, the
        // rooms with the line-ups standing in them, then the other line-ups; bayed
        // enclosures are indented.
        let lineups = panel::lineups(&prj.layout);
        let room_of = |lu: &Vec<usize>| lu.first().and_then(|&i| panel::room_of(&prj.layout, i));
        let mut rows: Vec<(usize, usize)> = Vec::new();
        let mut headings: Vec<(usize, usize, String, Heading)> = Vec::new();
        let room_rows = |rows: &mut Vec<(usize, usize)>, r: usize, indent: usize| {
            rows.push((indent, r));
            for lu in lineups.iter().filter(|lu| room_of(lu) == Some(r)) {
                rows.extend(lu.iter().enumerate().map(|(k, &i)| (indent + 1 + usize::from(k > 0), i)));
            }
        };
        // The site at the root: the buildings and the rooms outside them.
        let site = !prj.layout.buildings.is_empty() || !panel::rooms(&prj.layout).is_empty() || !prj.layout.plot.is_default();
        if site {
            headings.push((0, 0, tr("Site").to_string(), Heading::Site));
        }
        let d = usize::from(site);
        for (bi, b) in prj.layout.buildings.iter().enumerate() {
            headings.push((rows.len(), d, tr_fmt!("{}  building", b.name), Heading::Building(bi)));
            for l in b.levels.iter().rev() {
                headings.push((rows.len(), d + 1, tr_fmt!("{}  level, floor at {} m", l.name, format!("{:.2}", l.elevation / 1000.0)), Heading::Level(l.id)));
                for r in panel::buildings::rooms_on(&prj.layout, l.id) {
                    room_rows(&mut rows, r, d + 2);
                }
            }
        }
        for r in panel::rooms(&prj.layout).into_iter().filter(|&r| panel::buildings::building_of(&prj.layout, r).is_none()) {
            room_rows(&mut rows, r, d);
        }
        for lu in lineups.iter().filter(|lu| room_of(lu).is_none() && !lu.first().is_some_and(|&i| prj.layout.enclosures[i].is_room())) {
            rows.extend(lu.iter().enumerate().map(|(k, &i)| (usize::from(k > 0), i)));
        }
        let mut act = None;
        let mut headings = headings.into_iter().peekable();
        for (row, (indent, i)) in rows.into_iter().enumerate() {
            while let Some((_, depth, text, heading)) = headings.next_if(|h| h.0 <= row) {
                act = self.nav_heading(ui, prj, depth, text, heading).or(act);
            }
            let e = &prj.layout.enclosures[i];
            let level = (e.is_room() && panel::buildings::building_of(&prj.layout, i).is_some()).then_some(e.level);
            ui.horizontal(|ui| {
                ui.add_space(14.0 * indent as f32);
                let label = if e.is_room() {
                    tr_fmt!("{}  room {} × {} × {}", e.name, format!("{:.0}", e.width), format!("{:.0}", e.depth), format!("{:.0}", e.height))
                } else {
                    format!("{}  {:.0}×{:.0}×{:.0}", e.name, e.width, e.height, e.depth)
                };
                // The delete button stays visible; a long name is shortened instead.
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let tip = if e.is_room() { tr("Delete room") } else { tr("Delete enclosure") };
                    if icons::button(ui, Icon::Delete, tip).clicked() {
                        remove = Some(i);
                    }
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let mut r = ui.add(egui::Button::selectable(i == self.enclosure && !self.site, label.as_str()).truncate()).on_hover_text(label.as_str());
                        if e.is_room() {
                            // Rooms are dragged onto the levels of the buildings.
                            r = r.interact(Sense::drag());
                            r.dnd_set_drag_payload(DraggedRoom(i));
                        }
                        if r.clicked() {
                            if e.is_room() {
                                act = Some(NavAction::Open(i));
                            } else {
                                self.site = false;
                                self.enclosure = i;
                                self.nav_level = None;
                                self.selection.clear();
                                self.fit_pending = true;
                                self.cam = None;
                            }
                        }
                        if let Some(l) = level {
                            // Dropped onto a room of a level: onto its level.
                            if let Some(d) = r.dnd_release_payload::<DraggedRoom>() {
                                act = Some(NavAction::Put(d.0, l));
                            }
                            r.context_menu(|ui| {
                                if icons::text_button(ui, Icon::Wall, tr("New room on this level")).clicked() {
                                    act = Some(NavAction::NewRoomOn(l));
                                    ui.close();
                                }
                                if icons::text_button(ui, Icon::Close, tr("Take out of the building")).on_hover_text(tr("Take the room out of the building; it stays where it stands")).clicked() {
                                    act = Some(NavAction::TakeOut(i));
                                    ui.close();
                                }
                            });
                        }
                    });
                });
            });
        }
        for (_, depth, text, heading) in headings {
            act = self.nav_heading(ui, prj, depth, text, heading).or(act);
        }
        Self::room_drag_label(ui, prj);
        if let Some(a) = act {
            self.nav_action(prj, a);
        }
        if let Some(i) = remove {
            // Asked first when something would go with it.
            let e = &prj.layout.enclosures[i];
            let content = !e.items.is_empty() || !e.inner_walls.is_empty() || (e.is_room() && !panel::cabinets_in(&prj.layout, i).is_empty());
            if content {
                self.confirm_remove = Some(i);
            } else {
                self.remove_enclosure(prj, i);
            }
        }
        self.confirm_remove_ui(ui.ctx(), prj);
    }

    /// The devices of the navigator: those to place and those placed in the current
    /// enclosure.
    fn navigator_devices(&mut self, ui: &mut egui::Ui, prj: &Project, db: &PartsDb) {
        if prj.layout.enclosures.is_empty() {
            ui.weak(tr("No enclosure yet – create one with New enclosure, or a room for cabinets with New room."));
            return;
        }
        ui.separator();
        ui.heading(tr("Devices to place"));
        ui.add(egui::TextEdit::singleline(&mut self.device_filter).hint_text(tr("Filter…")));
        let todo = panel::devices_to_place(prj);
        let f = self.device_filter.to_lowercase();
        egui::ScrollArea::vertical().id_salt("todo").max_height(ui.available_height() * 0.5).auto_shrink([false, true]).show(ui, |ui| {
            for d in todo.iter().filter(|d| f.is_empty() || d.tag.to_lowercase().contains(&f)) {
                let active = self.tool == Tool::PlaceDevice(d.tag.clone());
                let text = format!("{}   {:.0}×{:.0}×{:.0}", d.tag, d.w, d.h, d.d);
                let clicked = ui
                    .horizontal(|ui| {
                        let r = ui.selectable_label(active, text).on_hover_text(&d.part).clicked();
                        let part = prj.parts.iter().chain(db.parts.iter()).find(|p| p.number == d.part);
                        if part.is_some_and(|p| p.wall_mountable()) {
                            openecraft_ui::part_editor::wall_badge(ui);
                        }
                        if part.is_some_and(|p| p.floor_standing()) {
                            openecraft_ui::part_editor::floor_badge(ui);
                        }
                        r
                    })
                    .inner;
                if clicked {
                    self.set_tool(Tool::PlaceDevice(d.tag.clone()));
                    if self.view == View::ThreeD {
                        self.view = View::Front;
                    }
                }
            }
            if todo.is_empty() {
                // Nothing placed and nothing to place: the devices have no parts with dimensions yet.
                let placed = prj.layout.enclosures.iter().any(|e| e.items.iter().any(|i| i.kind == ItemKind::Device));
                let tagged = prj.pages.iter().any(|pg| pg.symbols.iter().any(|s| !s.tag.is_empty()));
                if placed || !tagged {
                    ui.weak(tr("All devices with mounting data are placed."));
                } else {
                    ui.weak(tr("No device has mounting data yet: assign parts with dimensions to the devices (Properties › Parts)."));
                }
            }
        });
        ui.separator();
        ui.heading(tr("Placed"));
        let Some(ei) = self.enclosure_index(prj) else { return };
        let mut placed: Vec<&PanelItem> = prj.layout.enclosures[ei].items.iter().filter(|i| i.kind == ItemKind::Device).collect();
        placed.sort_by(|a, b| natural_cmp(&a.tag, &b.tag));
        let e = &prj.layout.enclosures[ei];
        egui::ScrollArea::vertical().id_salt("placed").auto_shrink([false, false]).show(ui, |ui| {
            for it in placed {
                let text = match it.face {
                    Face::Plate => it.tag.clone(),
                    Face::Door => tr_fmt!("{}  (door)", it.tag),
                    f => format!("{}  ({})", it.tag, e.face_title(f)),
                };
                if ui.selectable_label(self.selection.contains(&it.id), text).clicked() {
                    self.face = it.face;
                    self.select_item(it.id);
                }
            }
        });
    }

    // ------------------------------------------------------------------ properties

    #[allow(clippy::too_many_lines)]
    pub fn properties(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, db: &PartsDb) {
        ui.heading(tr("Properties"));
        let Some(ei) = self.enclosure_index(prj) else { return };
        if let Some(wi) = self.wire_index(an) {
            self.wire_properties(ui, prj, an, wi);
            return;
        }
        if self.selected_route.as_ref().is_some_and(|k| an.routing.item(k).is_some()) {
            self.route_properties(ui, prj, an);
            return;
        }
        let sel: Vec<u64> = self.selection.iter().copied().collect();
        if sel.len() > 1 {
            ui.label(tr_fmt!("{} items selected", sel.len()));
            ui.horizontal(|ui| {
                if icons::text_button(ui, Icon::Rotate, tr("Rotate")).clicked() {
                    rotate(&mut prj.layout.enclosures[ei], &self.selection);
                }
                if icons::text_button(ui, Icon::Delete, tr("Delete")).clicked() {
                    panel::remove_items(&mut prj.layout.enclosures[ei],&self.selection.iter().copied().collect::<Vec<_>>());
                    self.selection.clear();
                }
            });
            self.selection_tools(ui, prj, ei, &sel);
            return;
        }
        if let Some(id) = sel.first().copied() {
            let air = prj.layout.enclosures[ei].item(id).filter(|it| it.kind == ItemKind::Device && it.face == Face::Plate).and_then(|_| {
                let e = &prj.layout.enclosures[ei];
                panel::thermal::device_temperatures(prj, e, &panel::thermal::heat_map(prj, e)).into_iter().find(|t| t.id == id)
            });
            let auto_class = prj.layout.enclosures[ei].item(id).map(|it| panel::item_class(prj, &panel::device_kinds(prj), &PanelItem { class: String::new(), ..it.clone() })).unwrap_or_default();
            let custom = custom_classes(prj);
            let choices = match prj.layout.enclosures[ei].item(id).map(|it| it.kind) {
                Some(k) if k.is_profile() => panel::profile_choices(k, db.parts.iter().chain(&prj.parts)),
                _ => Vec::new(),
            };
            let mut new_part = None;
            let e = &mut prj.layout.enclosures[ei];
            let Some(idx) = e.items.iter().position(|i| i.id == id) else {
                self.selection.clear();
                return;
            };
            let mut it = e.items[idx].clone();
            let before_item = it.clone();
            let part = prj.parts.iter().chain(parts::catalog()).find(|p| p.number == it.part).cloned();
            let (face_w, face_h) = e.face_size(it.face);
            let room_wall = e.is_room() && matches!(it.face, Face::Wall(_) | Face::Inner(..));
            let room_height = e.height;
            // A door or window of a room: no part, class, anchors or turning.
            let opening = room_wall && it.kind == ItemKind::Cutout;
            // Doors and windows of a room are openings, not cutouts of a plate.
            let title = if room_wall && it.kind == ItemKind::Cutout { it.opening.title() } else { it.kind.title() };
            ui.label(RichText::new(title).strong());
            if it.kind == ItemKind::Accessory {
                if let Some(p) = &part {
                    ui.weak(&p.description);
                }
            }
            egui::Grid::new("pitem").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                if it.kind == ItemKind::Device {
                    ui.label(tr("Device"));
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&it.tag).monospace().strong());
                        if ui.small_button(tr("Show in schematic")).clicked() {
                            self.goto_schematic = Some(it.tag.clone());
                        }
                    });
                    ui.end_row();
                }
                if opening {
                    ui.label(tr("From the left end"));
                    ui.add(egui::DragValue::new(&mut it.x).speed(1.0).suffix(" mm")).on_hover_text(tr("Distance of the opening from the left end of the wall, seen from inside the room"));
                    ui.end_row();
                } else {
                    ui.label(tr("Part"));
                    if it.kind.is_profile() {
                        new_part = profile_part_combo(ui, "itempart", &choices, &it.part);
                    } else {
                        ui.label(RichText::new(if it.part.is_empty() { "–" } else { &it.part }).monospace());
                    }
                    ui.end_row();
                    ui.label("X / Y");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut it.x).speed(1.0).suffix(" mm"));
                        ui.add(egui::DragValue::new(&mut it.y).speed(1.0).suffix(" mm"));
                    });
                    ui.end_row();
                }
                if matches!(it.kind, ItemKind::Device | ItemKind::Accessory) {
                    ui.label(tr("Width × height"));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut it.w).speed(0.5).range(1.0..=5000.0));
                        ui.add(egui::DragValue::new(&mut it.h).speed(0.5).range(1.0..=5000.0));
                    });
                    ui.end_row();
                    ui.label(tr("Mounted on"));
                    let before = it.face;
                    let faces = part.as_ref().filter(|_| it.kind == ItemKind::Accessory).map(panel::allowed_faces).unwrap_or(Face::ALL.to_vec());
                    egui::ComboBox::from_id_salt("itemface").selected_text(it.face.title()).show_ui(ui, |ui| {
                        for f in faces {
                            ui.selectable_value(&mut it.face, f, f.title());
                        }
                    });
                    if it.face != before {
                        // Keep the position in front view; door devices sit directly on the door.
                        let shift = e.face_offset(it.face) - e.face_offset(before);
                        it.x -= shift.x;
                        it.y -= shift.y;
                        it.z = if it.kind == ItemKind::Accessory { before_item.z.min(0.0) } else { 0.0 };
                        it.anchor = Anchor::default();
                        self.face = it.face;
                        self.door_inside = false;
                    }
                    ui.end_row();
                    ui.label(tr("Depth / mounted at"));
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut it.d).speed(0.5).range(1.0..=2000.0));
                        ui.add(egui::DragValue::new(&mut it.z).speed(0.5).range(0.0..=2000.0).prefix("z "));
                    });
                    ui.end_row();
                } else if it.kind == ItemKind::Cutout && room_wall {
                    // Doors and windows of a room.
                    ui.label(tr("Opening"));
                    ui.horizontal(|ui| {
                        for o in Opening::ALL {
                            ui.selectable_value(&mut it.opening, o, o.title());
                        }
                    });
                    ui.end_row();
                    if matches!(it.opening, Opening::Door | Opening::Window) {
                        // Hinges left or right seen from inside, in or out, or sliding.
                        ui.label(tr("Opens"));
                        egui::ComboBox::from_id_salt("swing").selected_text(it.swing.title()).show_ui(ui, |ui| {
                            for s in panel::Swing::ALL {
                                ui.selectable_value(&mut it.swing, s, s.title());
                            }
                        })
                        .response
                        .on_hover_text(tr("How the leaf opens: hinged at its left or right edge (seen from inside the room), into or out of the room, or sliding along the wall"));
                        ui.end_row();
                    }
                    ui.label(tr("Width × height"));
                    let floor = it.y + it.h;
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut it.w).speed(5.0).range(100.0..=20_000.0));
                        ui.add(egui::DragValue::new(&mut it.h).speed(5.0).range(100.0..=room_height).suffix(" mm"));
                    });
                    ui.end_row();
                    let mut sill = room_height - floor;
                    if it.opening == Opening::Door {
                        sill = 0.0;
                    } else {
                        ui.label(tr("Above the floor"));
                        ui.add(egui::DragValue::new(&mut sill).speed(5.0).range(0.0..=room_height).suffix(" mm")).on_hover_text(tr("Height of the sill (bottom edge) above the floor"));
                        ui.end_row();
                    }
                    // Openings keep their bottom edge when they grow; doors stand on the floor.
                    it.y = room_height - sill - it.h;
                } else if it.kind == ItemKind::Cutout {
                    ui.label(tr("Shape"));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut it.round, false, tr("Rectangular"));
                        ui.selectable_value(&mut it.round, true, tr("Round"));
                    });
                    ui.end_row();
                    if it.round {
                        ui.label(tr("Diameter"));
                        if ui.add(egui::DragValue::new(&mut it.w).speed(0.5).range(1.0..=2000.0).suffix(" mm")).changed() {
                            it.h = it.w;
                        }
                    } else {
                        ui.label(tr("Width × height"));
                        ui.horizontal(|ui| {
                            ui.add(egui::DragValue::new(&mut it.w).speed(0.5).range(1.0..=5000.0));
                            ui.add(egui::DragValue::new(&mut it.h).speed(0.5).range(1.0..=5000.0));
                        });
                    }
                    ui.end_row();
                    if it.round {
                        it.h = it.w;
                    }
                } else {
                    ui.label(tr("Length"));
                    let mut len = it.length();
                    if ui.add(egui::DragValue::new(&mut len).speed(5.0).range(10.0..=5000.0).suffix(" mm")).changed() {
                        if it.horizontal() {
                            it.w = len;
                        } else {
                            it.h = len;
                        }
                    }
                    ui.end_row();
                    ui.label(tr("Orientation"));
                    ui.label(if it.horizontal() { tr("horizontal") } else { tr("vertical") });
                    ui.end_row();
                    if it.kind == ItemKind::Rail {
                        ui.label(tr("Standoff height"));
                        ui.add(egui::DragValue::new(&mut it.z).speed(1.0).range(0.0..=200.0).suffix(" mm"))
                            .on_hover_text(tr("Rail raised on standoffs above the plate (devices on it move up)"));
                        ui.end_row();
                    }
                }
                if !opening {
                ui.label(tr("Collision class"));
                ui.horizontal(|ui| {
                    let auto = tr_fmt!("Automatic ({})", panel::class_title(&auto_class));
                    let shown = if it.class.is_empty() { auto.clone() } else { panel::class_title(&it.class) };
                    egui::ComboBox::from_id_salt("itemclass").selected_text(shown).show_ui(ui, |ui| {
                        ui.selectable_value(&mut it.class, String::new(), auto);
                        for (k, _) in panel::CLASSES {
                            ui.selectable_value(&mut it.class, k.to_string(), panel::class_title(k));
                        }
                        for k in &custom {
                            ui.selectable_value(&mut it.class, k.clone(), k.as_str());
                        }
                    });
                    ui.add(egui::TextEdit::singleline(&mut it.class).desired_width(70.0).hint_text(tr("own class")));
                })
                .response
                .on_hover_text(tr("The checks keep the distances between classes and let classes overlap as set in Layout checks"));
                ui.end_row();
                }
                if let Some(t) = &air {
                    ui.label(tr("Air temperature"));
                    let text = match t.limit {
                        Some(l) => tr_fmt!("{} °C (max. {} °C)", format!("{:.0}", t.temperature), format!("{l:.0}")),
                        None => format!("{:.0} °C", t.temperature),
                    };
                    if t.limit.is_some_and(|l| t.temperature > l) {
                        ui.colored_label(crate::style::error(), text);
                    } else {
                        ui.label(text);
                    }
                    ui.end_row();
                }
                if it.kind != ItemKind::Cutout {
                    ui.label(tr("Fastening"));
                    let resolved = Fastening::resolve(&PanelItem { fastening: Fastening::Auto, ..it.clone() }, part.as_ref());
                    let text = |f: Fastening| if f == Fastening::Auto { format!("{} ({})", f.title(), resolved.title()) } else { f.title().to_string() };
                    egui::ComboBox::from_id_salt("fastening").selected_text(text(it.fastening)).show_ui(ui, |ui| {
                        for f in Fastening::ALL {
                            ui.selectable_value(&mut it.fastening, f, text(f));
                        }
                    });
                    ui.end_row();
                }
                if !opening {
                    ui.label(tr("Horizontal position"));
                    align_editor(ui, "anchorx", &mut it.anchor.x);
                    ui.end_row();
                    ui.label(tr("Vertical position"));
                    align_editor(ui, "anchory", &mut it.anchor.y);
                    ui.end_row();
                }
                if it.kind == ItemKind::Duct {
                    let limit = prj.layout.duct_fill_limit;
                    match an.duct_fill.iter().find(|f| f.enclosure == ei && f.duct == it.id) {
                        Some(f) => {
                            ui.label(tr("Fill level"));
                            ui.label(RichText::new(tr_fmt!("{} % (limit {} %)", format!("{:.0}", f.percent), format!("{limit:.0}"))).color(fill_color(f.percent, limit)));
                            ui.end_row();
                            ui.label(tr("Wires"));
                            ui.label(f.total_wires.to_string());
                            ui.end_row();
                            ui.label(tr("Fullest stretch"));
                            ui.label(tr_fmt!("{} wires, {} – {} mm", f.wires, format!("{:.0}", f.from), format!("{:.0}", f.to)));
                            ui.end_row();
                        }
                        None => {
                            ui.label(tr("Fill level"));
                            ui.label(tr("no wires"));
                            ui.end_row();
                        }
                    }
                }
            });
            ui.horizontal(|ui| {
                if !opening {
                    let label=it.rotation.map(|r|format!("{}°",r.degrees())).unwrap_or_else(||tr("Infer panel rotation from part dimensions").into());
                    egui::ComboBox::from_id_salt("panel_rotation").selected_text(label).show_ui(ui,|ui| {
                        ui.selectable_value(&mut it.rotation,None,tr("Infer panel rotation from part dimensions"));
                        for degrees in [0,90,180,270] {
                            let target=panel::PanelRotation::from_degrees(degrees).unwrap();
                            if ui.selectable_label(it.rotation==Some(target),format!("{degrees}°")).clicked() {
                                let height=part.as_ref().and_then(|p|p.size()).map(|s|s[1]).unwrap_or(it.h);
                                if panel::item_rotation(&it,height).is_quarter()!=target.is_quarter() {it.rotate();}
                                it.rotation=Some(target);
                            }
                        }
                    });
                }
                if !opening && icons::text_button(ui, Icon::Rotate, tr("Rotate")).on_hover_text("R").clicked() {
                    it.rotate();
                }
                if icons::text_button(ui, Icon::Delete, tr("Delete")).on_hover_text(tr("Del")).clicked() {
                    self.selection.clear();
                }
            });
            // A new anchor places the item; a moved anchored item keeps its anchor
            // with the new distances.
            if it.anchor != before_item.anchor {
                let a = it.anchor;
                a.apply(&mut it, (face_w, face_h));
            } else if !it.anchor.is_free() && (it.x, it.y, it.w, it.h) != (before_item.x, before_item.y, before_item.w, before_item.h) {
                it.anchor = it.anchor.capture(&it, e.face_size(it.face));
            }
            let group = it.group;
            if self.selection.is_empty() {
                e.items.remove(idx);
            } else if it != e.items[idx] {
                e.items[idx] = it;
            }
            if group != 0 {
                let name = e.group(group).map(|g| g.name.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(tr_fmt!("Assembly: {}", name));
                    if ui.small_button(tr("Select assembly")).clicked() {
                        self.selection = panel::group_members(e, group).into_iter().collect();
                    }
                    if ui.small_button(tr("Ungroup")).clicked() {
                        ungroup(e, &[id]);
                    }
                });
            }
            if let Some(part) = new_part.filter(|_| !self.selection.is_empty()) {
                parts::ensure_in_project(prj, db, &part);
                panel::change_profile(prj, ei, id, &part);
            }
            if !self.selection.is_empty() {
                self.selection_tools(ui, prj, ei, &sel);
                self.passage_tools(ui, prj, ei, id);
            }
            // Wires at this device
            if let Some(it) = prj.layout.enclosures[ei].items.iter().find(|i| i.id == id) {
                let wires: Vec<_> = an.wiring.iter().filter(|w| w.items.0 == it.id || w.items.1 == it.id).collect();
                if !wires.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(tr("Wires")).strong());
                    let mut clicked = None;
                    egui::Grid::new("pwires").striped(true).num_columns(2).show(ui, |ui| {
                        for w in wires {
                            if ui.selectable_label(false, format!("{} – {}", w.from, w.to)).on_hover_text(tr("Show this wire")).clicked() {
                                clicked = Some(w.key());
                            }
                            ui.label(format!("{:.0} mm", w.length));
                            ui.end_row();
                        }
                    });
                    if let Some(key) = clicked {
                        self.select_wire(key);
                    }
                }
            }
            return;
        }
        if prj.layout.enclosures[ei].is_room() {
            self.room_properties(ui, prj, ei, an);
            return;
        }
        // Enclosure properties
        let e = &mut prj.layout.enclosures[ei];
        let size_before = (e.width, e.height, e.depth);
        ui.label(RichText::new(tr("Enclosure")).strong());
        egui::Grid::new("penc").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Location"));
            ui.text_edit_singleline(&mut e.name);
            ui.end_row();
            ui.label(tr("Enclosure system"));
            let current = e.system().map(|s| s.title()).unwrap_or(tr("(none)"));
            let mut choice: Option<String> = None;
            egui::ComboBox::from_id_salt("encsystem").selected_text(current).width(200.0).show_ui(ui, |ui| {
                if ui.selectable_label(e.system().is_none(), tr("(none)")).clicked() {
                    choice = Some(String::new());
                }
                for s in &panel::SYSTEMS {
                    if ui.selectable_label(e.build.system == s.id(), s.title()).clicked() {
                        choice = Some(s.id().to_string());
                    }
                }
            });
            if let Some(id) = choice {
                panel::apply_system(e, &id, None);
            }
            ui.end_row();
            if let Some(s) = e.system() {
                ui.label(tr("Standard size"));
                let now = (e.width, e.height, e.depth);
                let label = |(w, h, d): (f32, f32, f32)| format!("{w:.0} × {h:.0} × {d:.0}");
                let standard = s.data.sizes.contains(&now);
                let mut size = None;
                egui::ComboBox::from_id_salt("encsize").selected_text(if standard { label(now) } else { tr("(custom size)").to_string() }).show_ui(ui, |ui| {
                    for x in s.data.sizes {
                        if ui.selectable_label(*x == now, label(*x)).clicked() {
                            size = Some(*x);
                        }
                    }
                });
                if size.is_some() {
                    let id = s.id();
                    panel::apply_system(e, id, size);
                }
                ui.end_row();
            }
            ui.label(tr("Part"));
            ui.label(RichText::new(if e.part.is_empty() { "–" } else { &e.part }).monospace());
            ui.end_row();
            ui.label(tr("Width × height"));
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut e.width).speed(5.0).range(100.0..=5000.0));
                ui.add(egui::DragValue::new(&mut e.height).speed(5.0).range(100.0..=5000.0));
            });
            ui.end_row();
            ui.label(tr("Depth"));
            ui.add(egui::DragValue::new(&mut e.depth).speed(5.0).range(60.0..=2000.0).suffix(" mm"));
            ui.end_row();
            ui.label(tr("Plinth"));
            ui.add(egui::DragValue::new(&mut e.build.plinth).speed(5.0).range(0.0..=300.0).suffix(" mm")).on_hover_text(tr("Height of the plinth, 0 without plinth"));
            ui.end_row();
            if e.system().is_some_and(|s| s.data.bayable) {
                ui.label(tr("Bayed"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut e.build.left_open, tr("left")).on_hover_text(tr("Bayed to the neighbouring enclosure: no side panel, a baying kit instead"));
                    ui.checkbox(&mut e.build.right_open, tr("right"));
                });
                ui.end_row();
            }
            ui.label(tr("Gland plate"));
            ui.checkbox(&mut e.build.gland_plate, tr("Bottom with removable gland plate"));
            ui.end_row();
            ui.label(tr("Door clearance"));
            ui.add(egui::DragValue::new(&mut e.door_clearance).speed(1.0).range(0.0..=500.0).suffix(" mm"))
                .on_hover_text(tr("Space behind the door that devices must keep free (door-mounted devices, handles)"));
            ui.end_row();
            let (pw, ph) = e.plate_size();
            ui.label(tr("Mounting plate"));
            ui.label(format!("{pw:.0} × {ph:.0} mm"));
            ui.end_row();
        });
        if (e.width, e.height, e.depth) != size_before {
            panel::sync_part(e);
            panel::apply_anchors(e);
        }
        egui::CollapsingHeader::new(tr("Components (assembly)")).id_salt("components").show(ui, |ui| {
            egui::Grid::new("pcomp").striped(true).num_columns(3).show(ui, |ui| {
                for c in panel::components(e) {
                    ui.label(c.role.title());
                    ui.label(RichText::new(if c.part.is_empty() { tr("(included)") } else { &c.part }).monospace());
                    ui.label(if c.count > 1 { format!("{} ×", c.count) } else { String::new() });
                    ui.end_row();
                }
            });
        });
        self.lineup_section(ui, prj, ei);
        self.routing_section(ui, prj, an, ei);
        ui.horizontal(|ui| {
            ui.label(tr("Wire reserve per connection"));
            ui.add(egui::DragValue::new(&mut prj.layout.wire_reserve).speed(5.0).range(0.0..=2000.0).suffix(" mm"));
        });
        ui.horizontal(|ui| {
            ui.label(tr("Duct fill limit"));
            ui.add(egui::DragValue::new(&mut prj.layout.duct_fill_limit).speed(1.0).range(5.0..=100.0).suffix(" %"))
                .on_hover_text(tr("The check warns about cable ducts filled above this level"));
        });
        ui.checkbox(&mut prj.layout.ferrules, tr("Wire ends with ferrules")).on_hover_text(tr("Production wire list and bill of materials"));
        ui.checkbox(&mut prj.layout.hardware, tr("Fastening hardware in the bill of materials")).on_hover_text(tr("Screws, nuts, washers and rivet nuts of the fixing points"));
        climate_section(ui, prj, ei);
        let findings = self.findings(prj, an, ei);
        checks_section(ui, prj, ei, &findings);
        let e = &prj.layout.enclosures[ei];
        let devices = e.items.iter().filter(|i| i.kind == ItemKind::Device).count();
        let wires: Vec<_> = an.wiring.iter().filter(|w| w.in_enclosure(ei)).collect();
        let total: f32 = wires.iter().map(|w| w.length).sum();
        ui.separator();
        ui.add(egui::Label::new(tr_fmt!("{} devices placed, {} wires routed ({} m)", devices, wires.len(), format!("{:.1}", total / 1000.0))).wrap());
        let problems = panel::problems_of(&findings).len();
        let holes = openecraft_core::production::holes(e, &prj.parts).len();
        let cutouts = e.items.iter().filter(|i| i.kind == ItemKind::Cutout).count();
        ui.add(egui::Label::new(tr_fmt!("Mounting plate: {} holes, {} cutouts", holes, cutouts)).wrap());
        ui.horizontal_wrapped(|ui| {
            if icons::text_button(ui, Icon::Download, tr("NC data (DXF)")).on_hover_text(tr("Plate outline, holes per diameter, cutouts")).clicked() {
                self.export = Some((ei, Face::Plate, NcFormat::Dxf));
            }
            if icons::text_button(ui, Icon::Download, tr("NC data (DWG)")).on_hover_text(tr("Plate outline, holes per diameter, cutouts")).clicked() {
                self.export = Some((ei, Face::Plate, NcFormat::Dwg));
            }
            if icons::text_button(ui, Icon::Download, tr("Drilling list (CSV)")).clicked() {
                self.export = Some((ei, Face::Plate, NcFormat::Csv));
            }
        });
        let door_devices = e.items.iter().filter(|i| i.face == Face::Door && i.kind == ItemKind::Device).count();
        ui.label(tr_fmt!("Door: {} devices, {} cutouts", door_devices, openecraft_core::production::openings(e, &prj.parts, Face::Door).len()));
        ui.horizontal_wrapped(|ui| {
            if icons::text_button(ui, Icon::Download, tr("Door NC data (DXF)")).clicked() {
                self.export = Some((ei, Face::Door, NcFormat::Dxf));
            }
            if icons::text_button(ui, Icon::Download, tr("Door NC data (DWG)")).clicked() {
                self.export = Some((ei, Face::Door, NcFormat::Dwg));
            }
            if icons::text_button(ui, Icon::Download, tr("Door cutouts (CSV)")).clicked() {
                self.export = Some((ei, Face::Door, NcFormat::Csv));
            }
        });
        if self.face.is_wall() {
            let face = self.face;
            let n = openecraft_core::production::holes_on(e, &prj.parts, face).len();
            ui.label(tr_fmt!("{}: {} holes, {} cutouts", face.title(), n, openecraft_core::production::openings(e, &prj.parts, face).len()));
            ui.horizontal_wrapped(|ui| {
                if icons::text_button(ui, Icon::Download, tr("NC data of this face (DXF)")).clicked() {
                    self.export = Some((ei, face, NcFormat::Dxf));
                }
                if icons::text_button(ui, Icon::Download, tr("NC data of this face (DWG)")).clicked() {
                    self.export = Some((ei, face, NcFormat::Dwg));
                }
                if icons::text_button(ui, Icon::Download, tr("Drilling list of this face (CSV)")).clicked() {
                    self.export = Some((ei, face, NcFormat::Csv));
                }
            });
        }
        if problems > 0 {
            ui.colored_label(crate::style::error(), tr_fmt!("{} placement problems (red) – see Check", problems));
        }
        if icons::text_button(ui, Icon::Plus, tr("Insert mounting layout page")).clicked() {
            self.add_page = Some(e.id);
        }
    }

    /// Properties of a room: size, place in the plant, the cabinets standing in it and
    /// what its walls and ceiling carry.
    fn room_properties(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize, an: &Analysis) {
        let pose = panel::site_poses(prj)[ei];
        let mut site: Option<Option<SitePlace>> = None;
        // The levels of the buildings ("+B1 › L0") and the one the room is on.
        let levels: Vec<(u64, String, f32)> = prj.layout.buildings.iter().flat_map(|b| b.levels.iter().map(move |l| (l.id, format!("{} › {}", b.name, l.name), l.height))).collect();
        let building = panel::buildings::find_level(&prj.layout, prj.layout.enclosures[ei].level).map(|(bi, _)| panel::buildings::building_pose(&prj.layout.buildings[bi]));
        let mut level: Option<Option<u64>> = None;
        let e = &mut prj.layout.enclosures[ei];
        let size_before = (e.width, e.depth, e.height);
        ui.label(RichText::new(tr("Room")).strong());
        egui::Grid::new("proom").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("Location"));
            ui.text_edit_singleline(&mut e.name);
            ui.end_row();
            ui.label(tr("Width × depth"));
            if e.outline.is_empty() {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut e.width).speed(10.0).range(500.0..=100_000.0));
                    ui.add(egui::DragValue::new(&mut e.depth).speed(10.0).range(500.0..=100_000.0).suffix(" mm"));
                });
            } else {
                ui.label(tr_fmt!("{} × {} mm, {} walls", format!("{:.0}", e.width), format!("{:.0}", e.depth), e.outline.len()))
                    .on_hover_text(tr("Extent of the floor plan; change it by its walls"));
            }
            ui.end_row();
            ui.label(tr("Height"));
            ui.add(egui::DragValue::new(&mut e.height).speed(10.0).range(500.0..=20_000.0).suffix(" mm"));
            ui.end_row();
            level = Self::level_row(ui, &levels, e);
            ui.label(if building.is_some() { tr("Place in the building") } else { tr("Place in the plant") });
            let current = e.site;
            // Shown where it stands: in the building's floor plan when it is on a level.
            let shown = building.map_or(pose.at, |b| b.local(pose.at));
            let angle = pose.angle - building.map_or(0.0, |b| b.angle);
            let mut place = current.unwrap_or(SitePlace { x: shown[0], z: shown[2], angle: angle.to_degrees() });
            ui.horizontal_wrapped(|ui| {
                let mut by_hand = current.is_some();
                let mut changed = ui.checkbox(&mut by_hand, tr("by hand")).on_hover_text(tr("Otherwise from the plant layout, or next to the other line-ups")).changed();
                ui.add_enabled_ui(by_hand, |ui| {
                    changed |= ui.add(egui::DragValue::new(&mut place.x).speed(10.0).prefix("x ").suffix(" mm")).changed();
                    changed |= ui.add(egui::DragValue::new(&mut place.z).speed(10.0).prefix("z ").suffix(" mm")).changed();
                    changed |= ui.add(egui::DragValue::new(&mut place.angle).speed(1.0).range(-360.0..=360.0).suffix(" °")).changed();
                });
                if changed {
                    site = Some(by_hand.then_some(place));
                }
            })
            .response
            .on_hover_text(tr("Rear left corner of the room on the floor, as in the 3D view with all enclosures; the cabinets in it move with it"));
            ui.end_row();
        });
        if (e.width, e.depth, e.height) != size_before {
            panel::apply_anchors(e);
        }
        if let Some(place) = site {
            prj.layout.enclosures[ei].site = place;
        }
        if let Some(l) = level {
            if let Err(e) = panel::buildings::set_room_level(prj, ei, l, None) {
                self.status = e;
            }
        }
        ui.separator();
        ui.label(RichText::new(tr("Cabinets in the room")).strong());
        let inside = panel::cabinets_in(&prj.layout, ei);
        for &c in &inside {
            let place = panel::room_place(prj, c).unwrap_or_default();
            let (x, z, angle) = (format!("{:.0}", place.x), format!("{:.0}", place.z), format!("{:.0}", place.angle));
            let label = tr_fmt!("{}  x {} mm, z {} mm, turned {} °", prj.layout.enclosures[c].name, x, z, angle);
            if ui.selectable_label(false, label).on_hover_text(tr("Show this cabinet; its place in the room is in its properties")).clicked() {
                self.enclosure = c;
                self.selection.clear();
                self.fit_pending = true;
                self.cam = None;
            }
        }
        if inside.is_empty() {
            ui.weak(tr("No cabinet stands in this room yet."));
        }
        let mut add = None;
        egui::ComboBox::from_id_salt("roomadd").selected_text(tr("➕ Put a cabinet into the room")).width(220.0).show_ui(ui, |ui| {
            for (i, o) in prj.layout.enclosures.iter().enumerate().filter(|(i, o)| !o.is_room() && !inside.contains(i)) {
                if ui.selectable_label(false, &o.name).clicked() {
                    add = Some(i);
                }
            }
        });
        if let Some(c) = add {
            if let Err(e) = panel::set_room(prj, c, Some(ei), None) {
                self.status = e;
            }
        }
        ui.separator();
        ui.label(RichText::new(tr("Walls and ceiling")).strong());
        ui.horizontal(|ui| {
            if ui.button(tr("Floor plan")).on_hover_text(tr("Drag walls and cabinets in the floor plan of the room")).clicked() {
                self.floor_plan();
            }
            if ui.button(tr("Draw walls")).on_hover_text(tr("Draw the walls of the room anew by clicking their corners")).clicked() {
                self.choose_tool(Tool::Walls);
            }
            if !prj.layout.enclosures[ei].outline.is_empty() && ui.button(tr("Rectangular")).on_hover_text(tr("Back to a rectangle of the room's width and depth")).clicked() {
                prj.layout.enclosures[ei].outline.clear();
            }
        });
        let corners = panel::outline(&prj.layout.enclosures[ei]);
        let mut new_outline = None;
        egui::Grid::new("roomwalls").num_columns(3).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            let e = &prj.layout.enclosures[ei];
            let walls = panel::walls(e);
            for f in e.faces() {
                let items: Vec<&PanelItem> = e.items.iter().filter(|i| i.face == f).collect();
                if ui.selectable_label(self.face == f && self.view == View::Front, panel::room_face_title(e, f)).on_hover_text(tr("Show and equip this face")).clicked() {
                    self.front_face(f, false);
                }
                match f {
                    Face::Wall(n) => {
                        let mut length = walls[n as usize].length();
                        let r = ui.add(egui::DragValue::new(&mut length).speed(10.0).range(panel::MIN_WALL..=100_000.0).suffix(" mm"));
                        if r.on_hover_text(tr("Length of the wall; its end moves, the next wall with it")).changed() {
                            match panel::wall_length_set(&corners, n as usize, (length / 10.0).round() * 10.0) {
                                Ok(o) => new_outline = Some(o),
                                Err(e) => self.status = e,
                            }
                        }
                    }
                    _ => {
                        ui.label("");
                    }
                }
                ui.label(face_summary(&items));
                ui.end_row();
            }
        });
        if let Some(o) = new_outline {
            if let Err(e) = panel::set_outline(&mut prj.layout.enclosures[ei], &o) {
                self.status = e;
            }
        }
        self.inner_walls_section(ui, prj, ei);
        ui.weak(tr("Devices on a wall sit on a mounting rail; parts marked for wall mounting go directly on the wall (Layout space › Insert › Wall-mounted part)."));
        let findings = self.findings(prj, an, ei);
        checks_section(ui, prj, ei, &findings);
    }

    /// The level of a building a room is on (rows of the room's properties): the new one
    /// when another is chosen (`None`: out of its building).
    fn level_row(ui: &mut egui::Ui, levels: &[(u64, String, f32)], e: &Enclosure) -> Option<Option<u64>> {
        let mut level = None;
        ui.label(tr("Building"));
        let shown = levels.iter().find(|l| l.0 == e.level).map_or_else(|| tr("none").to_string(), |l| l.1.clone());
        egui::ComboBox::from_id_salt("roomlevel")
            .selected_text(shown)
            .show_ui(ui, |ui| {
                if ui.selectable_label(e.level == 0, tr("none")).clicked() && e.level != 0 {
                    level = Some(None);
                }
                for (id, name, _) in levels {
                    if ui.selectable_label(e.level == *id, name.as_str()).clicked() && e.level != *id {
                        level = Some(Some(*id));
                    }
                }
            })
            .response
            .on_hover_text(tr("The level of a building the room is on (Layout space › Buildings); it moves with the building and its level"));
        ui.end_row();
        if let Some(h) = levels.iter().find(|l| l.0 == e.level).map(|l| l.2).filter(|h| e.height > *h) {
            ui.label("");
            ui.colored_label(crate::style::warning(), tr_fmt!("Higher than its level ({} mm)", format!("{h:.0}")));
            ui.end_row();
        }
        level
    }

    /// Inner walls of a room: length, thickness, removing them.
    fn inner_walls_section(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize) {
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("Inner walls")).strong());
            if ui.button(tr("Draw inner walls")).on_hover_text(tr("Click the ends of the inner walls in the floor plan")).clicked() {
                self.choose_tool(Tool::InnerWalls);
            }
        });
        let mut change = None;
        let mut remove = None;
        egui::Grid::new("roominner").num_columns(4).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for (k, w) in prj.layout.enclosures[ei].inner_walls.iter().enumerate() {
                if ui.selectable_label(self.plan_inner == Some(w.id), format!("I{}", k + 1)).on_hover_text(tr("Select it in the floor plan")).clicked() {
                    self.plan_inner = Some(w.id);
                    self.floor_plan_on();
                }
                let mut length = (w.b[0] - w.a[0]).abs() + (w.b[1] - w.a[1]).abs();
                let mut thickness = w.thickness;
                let l = ui.add(egui::DragValue::new(&mut length).speed(10.0).range(panel::MIN_WALL..=100_000.0).suffix(" mm")).on_hover_text(tr("Length; its end moves"));
                let t = ui.add(egui::DragValue::new(&mut thickness).speed(1.0).range(10.0..=1000.0).suffix(" mm")).on_hover_text(tr("Thickness"));
                if l.changed() || t.changed() {
                    change = Some((w.id, (length / 10.0).round() * 10.0, thickness));
                }
                if icons::button(ui, Icon::Delete, tr("Remove the inner wall with everything on it")).clicked() {
                    remove = Some(w.id);
                }
                ui.end_row();
            }
        });
        if let Some((id, length, thickness)) = change {
            let e = &mut prj.layout.enclosures[ei];
            match panel::moved_inner_wall(e, id, 0.0, Some(length), Some(thickness)) {
                Ok(w) => {
                    if let Some(old) = e.inner_walls.iter_mut().find(|x| x.id == id) {
                        *old = w;
                    }
                }
                Err(msg) => self.status = msg,
            }
        }
        if let Some(id) = remove {
            panel::remove_inner_wall(&mut prj.layout.enclosures[ei], id);
            self.plan_inner = None;
        }
    }

    /// Line-up of an enclosure: the neighbours it is bayed to and the cable passages
    /// the wires take into them.
    fn lineup_section(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize) {
        let (left, right) = panel::neighbours(&prj.layout, ei);
        let name = |prj: &Project, i: Option<usize>| i.map(|i| prj.layout.enclosures[i].name.clone()).unwrap_or_else(|| tr("(none)").to_string());
        let mut bay: Option<Option<usize>> = None;
        let mut site: Option<Option<SitePlace>> = None;
        let mut move_to: Option<Option<usize>> = None;
        let mut add = false;
        egui::CollapsingHeader::new(tr("Line-up and cable passages")).id_salt("lineup").default_open(left.is_some() || right.is_some()).show(ui, |ui| {
            egui::Grid::new("plineup").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
                ui.label(tr("Bayed to the right of"));
                egui::ComboBox::from_id_salt("bayedto")
                    .selected_text(name(prj, left))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(left.is_none(), tr("(none)")).clicked() {
                            bay = Some(None);
                        }
                        for (i, o) in prj.layout.enclosures.iter().enumerate().filter(|(i, _)| *i != ei) {
                            if ui.selectable_label(left == Some(i), &o.name).clicked() {
                                bay = Some(Some(i));
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr("The enclosure stands with its left side against the right side of the chosen one; wires run between them through cable passages"));
                ui.end_row();
                ui.label(tr("Right neighbour"));
                ui.label(name(prj, right));
                ui.end_row();
                ui.label(tr("Room"));
                let room = panel::room_of(&prj.layout, ei);
                egui::ComboBox::from_id_salt("encroom")
                    .selected_text(name(prj, room))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(room.is_none(), tr("(none)")).clicked() {
                            move_to = Some(None);
                        }
                        for r in panel::rooms(&prj.layout) {
                            if ui.selectable_label(room == Some(r), &prj.layout.enclosures[r].name).clicked() {
                                move_to = Some(Some(r));
                            }
                        }
                    })
                    .response
                    .on_hover_text(tr("The room the line-up stands in; its place is then measured from the rear left inside corner of the room"));
                ui.end_row();
                ui.label(tr("Place in the room"));
                let first = panel::lineup_of(&prj.layout, ei).first().copied().unwrap_or(ei);
                let current = prj.layout.enclosures[first].site;
                let mut place = current.unwrap_or_else(|| {
                    panel::room_place(prj, ei).unwrap_or_else(|| {
                        let pose = panel::site_poses(prj)[first];
                        SitePlace { x: pose.at[0], z: pose.at[2], angle: pose.angle.to_degrees() }
                    })
                });
                ui.horizontal_wrapped(|ui| {
                    let mut by_hand = current.is_some();
                    let mut changed = ui.checkbox(&mut by_hand, tr("by hand")).on_hover_text(tr("Otherwise from the plant layout, or next to the other line-ups")).changed();
                    ui.add_enabled_ui(by_hand, |ui| {
                        changed |= ui.add(egui::DragValue::new(&mut place.x).speed(10.0).prefix("x ").suffix(" mm")).changed();
                        changed |= ui.add(egui::DragValue::new(&mut place.z).speed(10.0).prefix("z ").suffix(" mm")).changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut place.angle).speed(1.0).range(-360.0..=360.0).suffix(" °"))
                            .on_hover_text(tr("Turn about the vertical, counter-clockwise seen from above (90 °: the fronts face to the right)"))
                            .changed();
                    });
                    if changed {
                        site = Some(by_hand.then_some(place));
                    }
                })
                .response
                .on_hover_text(tr("Rear left corner of the line-up on the floor, as in the 3D view with all enclosures"));
                ui.end_row();
            });
            let pos = panel::positions(&prj.layout);
            for (hop, other) in [(left, left), (right.map(|_| ei), right)] {
                let (Some(hop), Some(other)) = (hop, other) else { continue };
                let list = panel::passages(&prj.layout, &pos, hop);
                let cutouts = list.iter().filter(|x| x.cutouts != (None, None)).count();
                let other = &prj.layout.enclosures[other].name;
                if list.is_empty() {
                    ui.colored_label(crate::style::lock(), tr_fmt!("No cable passage to {}: wires to it are not routed", other));
                } else if cutouts == 0 {
                    ui.label(tr_fmt!("Open to {}: wires cross beside the mounting plates", other));
                } else {
                    ui.label(tr_fmt!("{} cable passages to {}", cutouts, other));
                }
            }
            if let Some(r) = right {
                ui.horizontal(|ui| {
                    add = icons::text_button(ui, Icon::Plus, &tr_fmt!("Cable passage to {}", prj.layout.enclosures[r].name)).on_hover_text(tr("Round cutouts facing each other in both side walls")).clicked();
                    ui.add(egui::DragValue::new(&mut self.passage_diameter).speed(1.0).range(10.0..=300.0).suffix(" mm")).on_hover_text(tr("Diameter"));
                });
            }
        });
        match bay {
            Some(Some(l)) => {
                panel::bay_right_of(prj, ei, l);
            }
            Some(None) => panel::unbay(prj, ei),
            None => {}
        }
        if let Some(place) = site {
            panel::set_site(prj, ei, place);
        }
        if let Some(room) = move_to {
            if let Err(e) = panel::set_room(prj, ei, room, None) {
                self.status = e;
            }
        }
        if add {
            let (y, z) = panel::next_passage_place(&prj.layout, ei);
            match panel::add_passage(prj, ei, y, z, self.passage_diameter).first() {
                Some(&id) if !prj.layout.enclosures[ei].build.right_open => {
                    self.show_face(Face::Right, false);
                    self.select_item(id);
                }
                Some(_) => {}
                None => self.status = tr("Both sides are open: the wires need no cutout").to_string(),
            }
        }
    }

    /// A cutout in a side wall towards a bayed neighbour: whether it is a cable
    /// passage, otherwise the facing cutout to add.
    fn passage_tools(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize, id: u64) {
        let Some(face) = prj.layout.enclosures[ei].item(id).filter(|i| i.kind == ItemKind::Cutout).map(|i| i.face) else { return };
        let (left, right) = panel::neighbours(&prj.layout, ei);
        let (hop, other) = match face {
            Face::Right => (right.map(|_| ei), right),
            Face::Left => (left, left),
            _ => return,
        };
        let (Some(hop), Some(other)) = (hop, other) else { return };
        let pos = panel::positions(&prj.layout);
        let used = panel::passages(&prj.layout, &pos, hop).iter().any(|x| x.cutouts.0 == Some(id) || x.cutouts.1 == Some(id));
        let name = prj.layout.enclosures[other].name.clone();
        ui.separator();
        if used {
            ui.label(tr_fmt!("Cable passage to {}", name));
        } else if ui
            .button(tr_fmt!("Add the facing cutout in {}", name))
            .on_hover_text(tr("A cutout of the same size at the same place in the neighbour's side wall: together they are a cable passage"))
            .clicked()
            && panel::add_facing_cutout(prj, ei, id).is_none()
        {
            self.status = tr_fmt!("The side of {} is open", name);
        }
    }

    /// Tools for the selected items: assemblies, aligning, rails, patterns and macros.
    fn selection_tools(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize, sel: &[u64]) {
        let e = &mut prj.layout.enclosures[ei];
        ui.separator();
        if sel.len() > 1 {
            ui.horizontal(|ui| {
                if ui.button(tr("Group as assembly")).on_hover_text(tr("The items are selected and moved together")).clicked() {
                    let n = e.groups.len() + 1;
                    let id = e.items.iter().map(|i| i.id).chain(e.groups.iter().map(|g| g.id)).max().unwrap_or(0) + 1;
                    ungroup(e, sel);
                    e.groups.push(panel::ItemGroup { id, name: tr_fmt!("Assembly {}", n), source: String::new() });
                    for it in e.items.iter_mut().filter(|i| sel.contains(&i.id)) {
                        it.group = id;
                    }
                }
                if e.items.iter().any(|i| sel.contains(&i.id) && i.group != 0) && ui.button(tr("Ungroup")).clicked() {
                    ungroup(e, sel);
                }
            });
            ui.horizontal_wrapped(|ui| {
                for (to, icon) in panel::AlignTo::ALL.into_iter().zip(["⏮", "↔", "⏭", "⏫", "↕", "⏬"]) {
                    if ui.small_button(icon).on_hover_text(to.title()).clicked() {
                        panel::align(e, sel, to);
                    }
                }
                if ui.small_button("|↔|").on_hover_text(tr("Distribute horizontally")).clicked() {
                    panel::distribute(e, sel, true);
                }
                if ui.small_button("|↕|").on_hover_text(tr("Distribute vertically")).clicked() {
                    panel::distribute(e, sel, false);
                }
            });
            let rail = e.items.iter().find(|i| sel.contains(&i.id) && i.kind == ItemKind::Rail).map(|i| i.id);
            if let Some(rail) = rail {
                ui.horizontal(|ui| {
                    let line_up = ui.button(tr("Line up on the rail")).on_hover_text(tr("The selected devices side by side on the selected rail, from its left end")).clicked();
                    ui.label(tr("Gap"));
                    ui.add(egui::DragValue::new(&mut self.rail_gap).speed(0.5).range(0.0..=100.0).suffix(" mm"));
                    if line_up && !panel::arrange_on_rail(e, rail, sel, self.rail_gap) {
                        self.status = tr("The devices do not fit onto the rail").to_string();
                    }
                });
            }
        }
        // Patterns and macros of doors and windows of a room make no sense.
        let openings = e.is_room() && e.items.iter().filter(|i| sel.contains(&i.id)).all(|i| i.kind == ItemKind::Cutout);
        if openings {
            return;
        }
        ui.horizontal(|ui| {
            if ui.button(tr("Pattern…")).on_hover_text(tr("Copies in rows and columns")).clicked() {
                self.pattern = Some(PatternDialog { nx: 2, ny: 1, px: 50.0, py: 50.0 });
            }
            if ui.button(tr("Save as 3D macro…")).clicked() {
                self.save_macro = Some(String::new());
            }
        });
    }

    fn wire_properties(&mut self, ui: &mut egui::Ui, prj: &Project, an: &Analysis, wi: usize) {
        let w = &an.wiring[wi];
        ui.label(RichText::new(tr("Wire")).strong());
        egui::Grid::new("pwire").num_columns(2).spacing([10.0, 5.0]).show(ui, |ui| {
            ui.label(tr("From"));
            ui.label(RichText::new(&w.from).monospace().color(WIRE_HIGHLIGHT));
            ui.end_row();
            ui.label(tr("To"));
            ui.label(RichText::new(&w.to).monospace().color(WIRE_HIGHLIGHT));
            ui.end_row();
            ui.label(tr("Potential"));
            ui.label(&w.net);
            ui.end_row();
            ui.label(tr("Length"));
            ui.label(format!("{:.0} mm", w.length));
            ui.end_row();
            ui.label(tr("Routing"));
            ui.label(w.routing());
            ui.end_row();
            if !w.legs.is_empty() {
                ui.label(tr("Enclosures"));
                ui.label(w.enclosure_names(&prj.layout));
                ui.end_row();
            }
            if let Some(ri) = an.routing.items.iter().position(|i| i.wire == Some(wi)) {
                let it = &an.routing.items[ri];
                ui.label(tr("Bundles"));
                ui.label(tr_fmt!("{} (class {})", an.routing.bundles_of(ri).len(), it.class.title()));
                ui.end_row();
                ui.label(tr("Cut length"));
                ui.label(format!("{:.0} mm", it.cut));
                ui.end_row();
            }
            ui.label(tr("Wire ends"));
            ui.label(format!("{} / {}", end_kind(w.ends.0), end_kind(w.ends.1)))
                .on_hover_text(tr("Where the wire starts and ends: a connection point from the part data, one generated from the function template, or the device centre if no connection point matches"));
            ui.end_row();
            if !w.color.is_empty() || !w.cross_section.is_empty() {
                ui.label(tr("Colour / cross-section"));
                ui.label(format!("{} {}", w.color, w.cross_section));
                ui.end_row();
            }
            if !w.cross_section.is_empty() {
                let diameter = openecraft_core::harness::outer_diameter(&w.cross_section);
                ui.label(tr("Min. bend radius"));
                ui.label(format!("{:.0} mm", prj.layout.routing.bend_radius(openecraft_core::routing3d::RouteKind::Wire, diameter)));
                ui.end_row();
            }
        });
        ui.label(
            RichText::new(tr_fmt!(
                "{} mm on the plate + {} mm reserve; the rest is the way from the connection points to the plate.",
                format!("{:.0}", w.plate_length()),
                format!("{:.0}", prj.layout.wire_reserve)
            ))
            .weak(),
        );
        ui.separator();
        ui.label(RichText::new(tr("Connected devices")).strong());
        let mut select = None;
        for (ei, id) in [(w.enclosure, w.items.0), (w.last_enclosure(), w.items.1)] {
            if let Some(it) = prj.layout.enclosures.get(ei).and_then(|e| e.item(id)) {
                ui.horizontal(|ui| {
                    if ui.link(RichText::new(&it.tag).monospace()).on_hover_text(tr("Select device")).clicked() {
                        select = Some((ei, id));
                    }
                    if ui.small_button(tr("Show in schematic")).clicked() {
                        self.goto_schematic = Some(it.tag.clone());
                    }
                });
            }
        }
        if let Some((ei, id)) = select {
            self.switch_enclosure(ei);
            self.select_item(id);
        }
    }

    // ------------------------------------------------------------------ dialogs

    pub fn dialogs(&mut self, ctx: &egui::Context, prj: &mut Project, db: &PartsDb) {
        self.pattern_dialog(ctx, prj);
        self.save_macro_dialog(ctx, prj);
        let Some(ne) = &mut self.new_enclosure else { return };
        let mut open = true;
        let mut create = false;
        let mut enclosures: Vec<&parts::Part> = Vec::new();
        for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()) {
            let system = panel::system_of_part(p).map(|s| s.id()).unwrap_or("");
            let wanted = ne.system.is_empty() || system == ne.system;
            if p.subgroup == "Enclosure" && p.width > 0.0 && wanted && !enclosures.iter().any(|x| x.number == p.number) {
                enclosures.push(p);
            }
        }
        egui::Window::new(tr("New enclosure"))
            .open(&mut open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .show(ctx, |ui| {
            egui::Grid::new("newenc").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Location"));
                ui.text_edit_singleline(&mut ne.name);
                ui.end_row();
                ui.label(tr("Enclosure system"));
                let title = panel::SYSTEMS.iter().find(|s| s.id() == ne.system).map(|s| s.title()).unwrap_or(tr("(all)"));
                egui::ComboBox::from_id_salt("newencsys").selected_text(title).width(220.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut ne.system, String::new(), tr("(all)"));
                    for s in &panel::SYSTEMS {
                        ui.selectable_value(&mut ne.system, s.id().to_string(), s.title());
                    }
                });
                ui.end_row();
                ui.label(tr("Part"));
                let before = ne.part.clone();
                egui::ComboBox::from_id_salt("encpart")
                    .selected_text(if ne.part.is_empty() { "(custom size)".to_string() } else { ne.part.clone() })
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut ne.part, String::new(), tr("(custom size)"));
                        for p in &enclosures {
                            ui.selectable_value(&mut ne.part, p.number.clone(), format!("{} – {}", p.number, p.description));
                        }
                    });
                if ne.part != before {
                    apply_enclosure_part(ne, db);
                }
                ui.end_row();
                let custom = ne.part.is_empty();
                ui.label(tr("Width × height × depth"));
                ui.add_enabled_ui(custom, |ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut ne.width).speed(5.0).range(100.0..=5000.0));
                        ui.add(egui::DragValue::new(&mut ne.height).speed(5.0).range(100.0..=5000.0));
                        ui.add(egui::DragValue::new(&mut ne.depth).speed(5.0).range(60.0..=2000.0));
                    });
                });
                ui.end_row();
            });
            if icons::text_button(ui, Icon::Check, tr("Create")).clicked() {
                create = true;
            }
        });
        if create {
            let ne = self.new_enclosure.take().unwrap();
            if !ne.part.is_empty() {
                parts::ensure_in_project(prj, db, &ne.part);
            }
            let mut e = panel::new_enclosure(prj, &ne.part, &ne.name);
            if ne.part.is_empty() {
                e.width = ne.width;
                e.height = ne.height;
                e.depth = ne.depth;
                if !ne.system.is_empty() {
                    panel::apply_system(&mut e, &ne.system, None);
                }
            }
            prj.layout.enclosures.push(e);
            self.enclosure = prj.layout.enclosures.len() - 1;
            self.selection.clear();
            self.fit_pending = true;
        } else if !open {
            self.new_enclosure = None;
        }
    }

    fn pattern_dialog(&mut self, ctx: &egui::Context, prj: &mut Project) {
        let Some(pd) = &mut self.pattern else { return };
        let mut open = true;
        let mut apply = false;
        egui::Window::new(tr("Pattern")).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            egui::Grid::new("pattern").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
                ui.label(tr("Columns × rows"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut pd.nx).range(1..=50));
                    ui.add(egui::DragValue::new(&mut pd.ny).range(1..=50));
                });
                ui.end_row();
                ui.label(tr("Pitch X / Y"));
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut pd.px).speed(1.0).suffix(" mm"));
                    ui.add(egui::DragValue::new(&mut pd.py).speed(1.0).suffix(" mm"));
                });
                ui.end_row();
            });
            ui.weak(tr("Devices are not copied: a device is placed once."));
            apply = icons::text_button(ui, Icon::Check, tr("Copy")).clicked();
        });
        if apply {
            let ids: Vec<u64> = self.selection.iter().copied().collect();
            let new = panel::pattern_copy(prj, self.enclosure, &ids, pd.nx, pd.ny, pd.px, pd.py);
            self.status = tr_fmt!("{} copies placed", new.len());
            self.pattern = None;
        } else if !open {
            self.pattern = None;
        }
    }

    fn save_macro_dialog(&mut self, ctx: &egui::Context, prj: &mut Project) {
        let Some(name) = &mut self.save_macro else { return };
        let mut open = true;
        let mut save = false;
        egui::Window::new(tr("Save as 3D macro")).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(tr("Name"));
                ui.text_edit_singleline(name);
            });
            ui.weak(tr("Items with an anchor keep it; the others are inserted relative to the click."));
            save = ui.add_enabled_ui(!name.trim().is_empty(), |ui| icons::text_button(ui, Icon::Check, tr("Save"))).inner.clicked();
        });
        if save {
            let ids: Vec<u64> = self.selection.iter().copied().collect();
            let name = name.trim().to_string();
            if let Some(m) = prj.layout.enclosures.get(self.enclosure).and_then(|e| panel::macro_from_items(e, &ids, &name)) {
                prj.layout.macros.retain(|x| x.name != name);
                prj.layout.macros.push(m);
                self.status = tr_fmt!("3D macro {} saved", name);
            }
            self.save_macro = None;
        } else if !open {
            self.save_macro = None;
        }
    }

    // ------------------------------------------------------------------ canvas

    pub fn canvas(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis, db: &PartsDb) {
        if self.site {
            self.site_canvas(ui, prj, an);
            return;
        }
        let Some(ei) = self.enclosure_index(prj) else {
            ui.centered_and_justified(|ui| {
                ui.label(tr("Create an enclosure or a room (Layout space › New enclosure, New room) to start the panel layout."));
            });
            return;
        };
        ui.horizontal(|ui| {
            // A room has no door to swing.
            if self.view == View::ThreeD && !self.in_room {
                self.door_button(ui);
            }
            if let Tool::PlaceDevice(tag) = &self.tool {
                ui.label(RichText::new(tr_fmt!("Placing {}", tag)).strong());
            }
            if let Some((kind, part)) = self.profile_here() {
                let choices = panel::profile_choices(kind, db.parts.iter().chain(&prj.parts));
                if let Some(picked) = profile_part_combo(ui, "profilepart", &choices, &part) {
                    self.choose_profile_part(kind, &picked);
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let what = match self.view {
                    View::ThreeD => "3D".to_string(),
                    View::Front if self.face == Face::Door && self.door_inside => tr("Door (inside)").to_string(),
                    View::Front if self.face == Face::Door => tr("Door (front)").to_string(),
                    View::Plan => tr("Floor plan").to_string(),
                    View::Front => prj.layout.enclosures[ei].face_title(self.face),
                };
                ui.weak(format!("{}  ·  {}", prj.layout.enclosures[ei].name, what));
            });
        });
        ui.separator();
        match self.view {
            View::Front => self.front_view(ui, prj, ei, an, db),
            View::ThreeD => self.view_3d(ui, prj, ei, an),
            View::Plan => self.plan_view(ui, prj, ei),
        }
    }

    /// Chooses a tool (Layout space tab of the ribbon); drawing tools show the front view.
    pub fn choose_tool(&mut self, tool: Tool) {
        if tool != Tool::Select {
            self.site = false;
        }
        if matches!(tool, Tool::Walls | Tool::InnerWalls) {
            self.view = View::Plan;
            self.wall_points.clear();
        } else if matches!(tool, Tool::Opening(_)) {
            // Placed in the floor plan or in the view of a wall.
            if self.view == View::ThreeD || (self.view == View::Front && !matches!(self.face, Face::Wall(_) | Face::Inner(..))) {
                self.view = View::Plan;
            }
        } else if tool != Tool::Select {
            self.view = View::Front;
        }
        if self.tool_allowed(&tool) {
            self.set_tool(tool);
        }
    }

    /// Shows a face in the front view (Layout space › View).
    pub fn front_face(&mut self, face: Face, inside: bool) {
        self.site = false;
        self.view = View::Front;
        self.show_face(face, inside);
    }

    pub fn toggle_lineup(&mut self) {
        self.show_lineup = !self.show_lineup;
        self.cam = None;
    }

    pub fn toggle_site(&mut self) {
        self.show_site = !self.show_site;
        self.cam = None;
    }

    pub fn toggle_building(&mut self) {
        self.show_building = !self.show_building;
        self.cam = None;
    }

    pub fn fit(&mut self) {
        self.fit_pending = true;
        self.cam = None;
    }

    /// The zoom of the 2D views in percent of the real size (none in the 3D view).
    pub fn zoom_percent(&self) -> Option<f32> {
        (self.view != View::ThreeD).then(|| self.zoom / crate::editor::ZOOM_ACTUAL * 100.0)
    }

    /// A resized 2D view (pane splitter, window) keeps what was in its centre there.
    fn keep_centre(&mut self, rect: Rect) {
        let size = rect.size();
        if self.view_size != egui::Vec2::ZERO && size != self.view_size && !self.fit_pending {
            self.offset += (size - self.view_size) / 2.0;
        }
        self.view_size = size;
    }

    /// Removes an enclosure or room; the cabinets standing in a room stay, without one.
    fn remove_enclosure(&mut self, prj: &mut Project, i: usize) {
        if prj.layout.enclosures[i].is_room() {
            for c in panel::cabinets_in(&prj.layout, i) {
                let _ = panel::set_room(prj, c, None, None);
            }
        }
        panel::unbay(prj, i);
        prj.layout.enclosures.remove(i);
        self.selection.clear();
    }

    /// Asks before an enclosure or room with content is removed.
    fn confirm_remove_ui(&mut self, ctx: &egui::Context, prj: &mut Project) {
        let Some(i) = self.confirm_remove.filter(|&i| i < prj.layout.enclosures.len()) else {
            self.confirm_remove = None;
            return;
        };
        let e = &prj.layout.enclosures[i];
        let question = if e.is_room() {
            tr_fmt!("Delete room {} with everything on its walls? The cabinets standing in it stay, without a room.", e.name)
        } else {
            tr_fmt!("Delete enclosure {} with everything placed in it?", e.name)
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::unique("confirm_remove_enclosure")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(question);
            ui.weak(tr("Undo brings it back."));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tr("Delete")).clicked() {
                    answer = Some(true);
                }
                if ui.button(tr("Cancel")).clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    answer = Some(false);
                }
            });
        });
        if let Some(delete) = answer {
            self.confirm_remove = None;
            if delete {
                self.remove_enclosure(prj, i);
            }
        }
    }

    /// Opens the dialog for a new enclosure.
    pub fn new_enclosure_dialog(&mut self, prj: &Project, db: &PartsDb) {
        let part = if db.get(panel::ENCLOSURE_PART).is_some() { panel::ENCLOSURE_PART } else { "" };
        let mut ne = NewEnclosure {
            name: panel::free_name(&prj.layout, "+CAB"),
            part: part.into(),
            system: String::new(),
            width: 600.0,
            height: 800.0,
            depth: 250.0,
        };
        apply_enclosure_part(&mut ne, db);
        self.new_enclosure = Some(ne);
    }

    /// The walls of the enclosure (Layout space › View › Walls); of a room its walls
    /// and the ceiling.
    pub fn walls_menu(&mut self, ui: &mut egui::Ui, prj: &Project) {
        let room = prj.layout.enclosures.get(self.enclosure).filter(|e| e.is_room());
        let faces = room.map(|e| e.faces()).unwrap_or_else(|| Face::WALLS.to_vec());
        for f in faces {
            let title = match room {
                Some(e) => match panel::side(e, f) {
                    Some(w) => format!("{}  ({:.2} m)", panel::room_face_title(e, f), w.length() / 1000.0),
                    None => panel::room_face_title(e, f),
                },
                None => f.title().to_string(),
            };
            if ui.selectable_label(self.face == f && self.view == View::Front, title).clicked() {
                self.front_face(f, false);
                ui.close();
            }
        }
    }

    /// Shows the floor plan of the room (it stays when it is shown).
    fn floor_plan_on(&mut self) {
        if self.view != View::Plan {
            self.floor_plan();
        }
    }

    /// Shows the floor plan of the room (Layout space › View › Floor plan).
    pub fn floor_plan(&mut self) {
        if self.site {
            self.view = View::Plan;
        } else if self.in_room {
            self.view = if self.view == View::Plan { View::Front } else { View::Plan };
            self.fit_pending = true;
            if self.tool != Tool::Walls {
                self.set_tool(Tool::Select);
            }
        }
    }

    /// Adds a room and shows its rear wall.
    /// A new room, on the level selected in the navigator if there is one.
    pub fn new_room(&mut self, prj: &mut Project) {
        let name = panel::free_name(&prj.layout, "+ROOM");
        self.nav_level = self.nav_level.filter(|&l| panel::buildings::level_by_id(&prj.layout, l).is_some());
        let ri = match panel::buildings::add_room(prj, &name, self.nav_level) {
            Ok(ri) => ri,
            Err(e) => {
                self.status = e;
                return;
            }
        };
        self.enclosure = ri;
        self.in_room = true;
        self.selection.clear();
        self.show_face(Face::Wall(0), false);
        self.view = View::Plan;
        self.fit_pending = true;
        self.cam = None;
        self.status = match self.nav_level.and_then(|l| panel::buildings::level_by_id(&prj.layout, l)) {
            Some(l) => tr_fmt!("Room added on level {}: drag its walls or draw new ones in the floor plan, put cabinets into it in their properties", l.name),
            None => tr("Room added: drag its walls or draw new ones in the floor plan, put cabinets into it in their properties").to_string(),
        };
    }

    /// Carries out what was done with a level or a room in the navigator.
    fn nav_action(&mut self, prj: &mut Project, act: NavAction) {
        let level_name = |prj: &Project, id: u64| panel::buildings::level_by_id(&prj.layout, id).map(|l| l.name.clone()).unwrap_or_default();
        match act {
            NavAction::Level(id) if self.nav_level == Some(id) => {
                self.nav_level = None;
                if self.level_3d() == Some(id) {
                    // Back to all of the plant, the building highlighted.
                    self.site_plan.level = 0;
                    self.cam = None;
                }
            }
            NavAction::Level(id) => {
                self.nav_level = Some(id);
                if self.view == View::ThreeD {
                    self.show_level(prj, id);
                }
            }
            NavAction::NewRoomOn(id) => {
                self.nav_level = Some(id);
                self.new_room(prj);
            }
            NavAction::Put(r, id) => {
                if prj.layout.enclosures.get(r).is_some_and(|e| e.level == id) {
                    return;
                }
                self.status = match panel::buildings::set_room_level(prj, r, Some(id), None) {
                    Ok(()) => tr_fmt!("Room {} now stands on level {}", prj.layout.enclosures[r].name, level_name(prj, id)),
                    Err(e) => e,
                };
            }
            NavAction::TakeOut(r) => {
                self.status = match panel::buildings::set_room_level(prj, r, None, None) {
                    Ok(()) => tr_fmt!("Room {} taken out of the building; it stays where it stands", prj.layout.enclosures[r].name),
                    Err(e) => e,
                };
            }
            NavAction::Open(r) => self.open_room(r),
            NavAction::Site(b) => self.show_site_plan(b),
            NavAction::SiteTool(t) => self.choose_site_tool(t),
            NavAction::NewBuilding => {
                let name = panel::buildings::free_building_name(&prj.layout);
                match panel::buildings::add_building(prj, &name) {
                    Ok(bi) => {
                        self.show_site_plan(Some(bi));
                        self.status = tr_fmt!("Building {} added with a level at ground level", name);
                    }
                    Err(e) => self.status = e,
                }
            }
            NavAction::NewLevel(bi, below) => {
                let Some(b) = prj.layout.buildings.get(bi) else { return };
                let elevation = below.then(|| b.levels.first().map_or(0.0, |l| l.elevation) - panel::buildings::LEVEL_HEIGHT);
                self.status = match panel::buildings::add_level(prj, bi, None, elevation, None) {
                    Ok(id) => {
                        self.nav_level = Some(id);
                        tr_fmt!("Level {} added to building {}; new rooms go onto it", level_name(prj, id), prj.layout.buildings[bi].name)
                    }
                    Err(e) => e,
                };
            }
        }
    }

    /// Shows a room: the 3D view stays, any other view becomes its floor plan.
    fn open_room(&mut self, ri: usize) {
        self.site = false;
        self.enclosure = ri;
        self.in_room = true;
        self.nav_level = None;
        self.selection.clear();
        self.fit_pending = true;
        self.cam = None;
        if self.view != View::ThreeD {
            self.view = View::Plan;
            if self.tool != Tool::Walls {
                self.set_tool(Tool::Select);
            }
        }
    }

    /// Shows the rooms of a level side by side at their places in the 3D view, with the
    /// cabinets standing in them; the site plan highlights them.
    fn show_level(&mut self, prj: &Project, id: u64) {
        let Some((bi, _)) = panel::buildings::find_level(&prj.layout, id) else { return };
        self.show_site_plan(Some(bi));
        self.site_plan.level = id;
    }

    /// Shows the site: its site plan, or all of the plant when the 3D view is shown;
    /// building `b` highlighted.
    fn show_site_plan(&mut self, b: Option<usize>) {
        self.site = true;
        self.site_plan = site_plan::SitePlan { building: b, ..Default::default() };
        self.site_tool = SiteTool::Select;
        self.site_points.clear();
        self.site_drag = None;
        self.selection.clear();
        self.cam = None;
        if self.view != View::ThreeD {
            self.view = View::Plan;
        }
        self.set_tool(Tool::Select);
    }

    /// The site plan of the plant in the canvas: zoomed with the wheel, moved by
    /// dragging; a click on a room shows it.
    fn site_canvas(&mut self, ui: &mut egui::Ui, prj: &mut Project, an: &Analysis) {
        if self.view == View::ThreeD {
            if let Some(ei) = self.enclosure_index(prj) {
                let level = self.level_3d().and_then(|l| panel::buildings::find_level(&prj.layout, l));
                let what = match level {
                    Some((bi, li)) => {
                        let b = &prj.layout.buildings[bi];
                        tr_fmt!("Level {} of building {}", b.levels[li].name, b.name)
                    }
                    None => tr("Site").to_string(),
                };
                ui.horizontal(|ui| {
                    if level.is_some() {
                        ui.weak(tr("The rooms of the level side by side; click the level again for all of the plant"));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.weak(format!("{what}  ·  3D"));
                    });
                });
                ui.separator();
                self.view_3d(ui, prj, ei, an);
                return;
            }
        }
        self.view = View::Plan;
        ui.horizontal(|ui| {
            let hint = match self.site_tool {
                SiteTool::Select => tr("Wheel: zoom · drag: move · click on a room: its floor plan · drag a corner of the plot boundary"),
                SiteTool::Boundary => tr("Click the corners of the plot boundary · click the first one, double-click or Enter: close · Backspace: back · Esc: stop"),
                SiteTool::Reference => tr("Click where the reference point is"),
                SiteTool::Measure => tr("Click two points to measure from one to the other · Esc: stop"),
            };
            ui.weak(hint);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let what = match self.site_plan.building.and_then(|b| prj.layout.buildings.get(b)) {
                    Some(b) => tr_fmt!("Site plan, building {}", b.name),
                    None => tr("Site plan").to_string(),
                };
                ui.weak(format!("{}  ·  {}", tr("Site"), what));
            });
        });
        ui.separator();
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        // The part of the plant in view stays while the plan is edited, until it is fitted.
        if self.site_plan.extent.is_none() || self.fit_pending {
            self.site_plan.extent = Some(site_plan::extent(prj));
        }
        if resp.hovered() {
            let (wheel, zoom_delta, pointer) = ui.input(|i| (wheel_delta(i), i.zoom_delta(), i.pointer.hover_pos()));
            let factor = zoom_delta * (wheel * 0.0025).exp();
            if let (true, Some(m)) = ((factor - 1.0).abs() > 1e-4, pointer) {
                let old = self.site_plan.zoom;
                let zoom = (old * factor).clamp(0.2, 200.0);
                // The point under the pointer stays where it is.
                let c = rect.shrink(16.0).center();
                self.site_plan.pan = (m - c) - (m - c - self.site_plan.pan) * (zoom / old);
                self.site_plan.zoom = zoom;
            }
        }
        let dragging_corner = self.site_tools(ui, rect, &resp, prj);
        if resp.dragged() && !dragging_corner {
            self.site_plan.pan += resp.drag_delta();
        }
        if self.fit_pending {
            self.site_plan.zoom = 1.0;
            self.site_plan.pan = Vec2::ZERO;
            self.fit_pending = false;
        }
        let hovered = site_plan::paint(ui, rect, prj, &self.site_plan, resp.hover_pos()).filter(|_| self.site_tool == SiteTool::Select && !dragging_corner);
        self.paint_site_tools(ui, rect, prj, resp.hover_pos());
        if let Some(ri) = hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            if resp.clicked() {
                self.open_room(ri);
                return;
            }
            resp.on_hover_text(tr_fmt!("{}: click to show its floor plan", prj.layout.enclosures[ri].name));
        }
    }

    /// A heading of the navigator: a building, or a level (`level`) that is selected for
    /// new rooms by a click, takes rooms dropped onto it and has a context menu.
    fn nav_heading(&self, ui: &mut egui::Ui, prj: &Project, depth: usize, text: String, heading: Heading) -> Option<NavAction> {
        ui.horizontal(|ui| {
            ui.add_space(14.0 * depth as f32);
            let id = match heading {
                Heading::Site => {
                    let r = ui.add(egui::Button::selectable(self.site && self.site_plan.building.is_none(), RichText::new(text).strong()).truncate()).on_hover_text(tr("Click to show the site plan of the buildings, or all of the plant in the 3D view"));
                    let mut act = r.clicked().then_some(NavAction::Site(None));
                    r.context_menu(|ui| {
                        if icons::text_button(ui, Icon::Plus, tr("New building")).on_hover_text(tr("A building with one level at ground level, beside what stands in the plant")).clicked() {
                            act = Some(NavAction::NewBuilding);
                            ui.close();
                        }
                        if icons::text_button(ui, Icon::Polygon, tr("Plot boundary")).on_hover_text(tr("Draw the boundary of the plot in the site plan; all buildings have to stay inside it")).clicked() {
                            act = Some(NavAction::SiteTool(SiteTool::Boundary));
                            ui.close();
                        }
                        if icons::text_button(ui, Icon::Pin, tr("Reference point")).on_hover_text(tr("Place the georeferenced point everything is measured from")).clicked() {
                            act = Some(NavAction::SiteTool(SiteTool::Reference));
                            ui.close();
                        }
                    });
                    return act;
                }
                Heading::Building(bi) => {
                    let r = ui.add(egui::Button::selectable(self.site && self.site_plan.building == Some(bi), RichText::new(text).strong()).truncate()).on_hover_text(tr("Click to show the building in the site plan; right-click for new levels"));
                    let mut act = r.clicked().then_some(NavAction::Site(Some(bi)));
                    r.context_menu(|ui| {
                        if icons::text_button(ui, Icon::Plus, tr("Level on top")).on_hover_text(tr("A level on top of the highest one")).clicked() {
                            act = Some(NavAction::NewLevel(bi, false));
                            ui.close();
                        }
                        if icons::text_button(ui, Icon::Plus, tr("Basement")).on_hover_text(tr("A level below the lowest one")).clicked() {
                            act = Some(NavAction::NewLevel(bi, true));
                            ui.close();
                        }
                    });
                    return act;
                }
                Heading::Level(id) => id,
            };
            let r = ui.add(egui::Button::selectable(self.nav_level == Some(id), RichText::new(text).strong()).truncate()).on_hover_text(tr("Click to put new rooms on this level; drop rooms onto it to move them here"));
            let mut act = r.clicked().then_some(NavAction::Level(id));
            r.context_menu(|ui| {
                if icons::text_button(ui, Icon::Wall, tr("New room on this level")).clicked() {
                    act = Some(NavAction::NewRoomOn(id));
                    ui.close();
                }
            });
            let elsewhere = |d: &DraggedRoom| prj.layout.enclosures.get(d.0).is_some_and(|e| e.level != id);
            if r.dnd_hover_payload::<DraggedRoom>().is_some_and(|d| elsewhere(&d)) {
                let color = ui.visuals().selection.stroke.color;
                ui.painter().rect_filled(r.rect, 2.0, ui.visuals().selection.bg_fill.gamma_multiply(0.4));
                ui.painter().rect_stroke(r.rect, 2.0, Stroke::new(1.5, color), egui::StrokeKind::Inside);
            }
            if let Some(d) = r.dnd_release_payload::<DraggedRoom>() {
                act = Some(NavAction::Put(d.0, id));
            }
            act
        })
        .inner
    }

    /// The label at the pointer while a room is dragged in the navigator.
    fn room_drag_label(ui: &egui::Ui, prj: &Project) {
        let Some(d) = egui::DragAndDrop::payload::<DraggedRoom>(ui.ctx()) else { return };
        let (Some(pos), Some(e)) = (ui.ctx().pointer_latest_pos(), prj.layout.enclosures.get(d.0)) else { return };
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        egui::Area::new(egui::Id::unique("room_drag_label")).order(egui::Order::Tooltip).fixed_pos(pos + vec2(16.0, 12.0)).interactable(false).show(ui.ctx(), |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(e.name.as_str()).strong());
                ui.weak(tr("Drop onto a level to put the room on it"));
            });
        });
    }

    /// Parts that stand on the floor of a room (pumps, tanks, …) of the parts database,
    /// the project and the catalog, placed on the floor without a device tag.
    pub fn floor_part_menu(&mut self, ui: &mut egui::Ui, prj: &Project, db: &PartsDb) {
        if !self.in_room {
            ui.weak(tr("Choose a room first: floor-standing parts go on its floor."));
            return;
        }
        let mut list: Vec<&parts::Part> = Vec::new();
        for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()) {
            if p.floor_standing() && !list.iter().any(|x| x.number == p.number) {
                list.push(p);
            }
        }
        list.sort_by(|a, b| a.manufacturer.cmp(&b.manufacturer).then(natural_cmp(&a.number, &b.number)));
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for p in &list {
                let active = self.tool == Tool::Accessory(p.number.clone()) && self.face == Face::Bottom;
                if ui.selectable_label(active, format!("{} – {}", p.number, p.description)).clicked() {
                    self.front_face(Face::Bottom, false);
                    self.set_tool(Tool::Accessory(p.number.clone()));
                    ui.close();
                }
            }
        });
        if list.is_empty() {
            ui.weak(tr("No parts marked for floor mounting yet. Mark one in the parts management: part editor › Mounting data › Floor mounting."));
            if ui.button(tr("Parts management")).clicked() {
                self.open_parts = true;
                ui.close();
            }
        }
    }

    /// Parts made for direct wall mounting (inverters, charge controllers, …) of the
    /// parts database, the project and the catalog, placed without a device tag.
    pub fn wall_part_menu(&mut self, ui: &mut egui::Ui, prj: &Project, db: &PartsDb) {
        if !self.in_room {
            ui.weak(tr("Choose a room first: wall-mounted parts go on its walls and ceiling."));
            return;
        }
        let mut list: Vec<&parts::Part> = Vec::new();
        for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()) {
            if p.wall_mountable() && !list.iter().any(|x| x.number == p.number) {
                list.push(p);
            }
        }
        list.sort_by(|a, b| a.manufacturer.cmp(&b.manufacturer).then(natural_cmp(&a.number, &b.number)));
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for p in &list {
                let active = self.tool == Tool::Accessory(p.number.clone());
                if ui.selectable_label(active, format!("{} – {}", p.number, p.description)).clicked() {
                    self.view = View::Front;
                    self.set_tool(Tool::Accessory(p.number.clone()));
                    ui.close();
                }
            }
        });
        if list.is_empty() {
            ui.weak(tr("No parts marked for wall mounting yet. Mark one in the parts management: part editor › Mounting data › Wall mounting."));
            if ui.button(tr("Parts management")).clicked() {
                self.open_parts = true;
                ui.close();
            }
        }
    }

    /// Outlets and switches of a room and heating circuit manifolds with their control
    /// boxes, from the parts database, the project, the catalog and the manufacturer
    /// catalogs shipped with OpenECraft; chosen, a part of a shipped catalog comes into
    /// the project, then a click on the wall places it at its mounting height.
    pub fn installation_menu(&mut self, ui: &mut egui::Ui, prj: &mut Project, db: &PartsDb) {
        if !self.in_room {
            ui.weak(tr("Choose a room first: outlets, switches and heating circuit manifolds go on its walls."));
            return;
        }
        let mut chosen: Option<parts::Part> = None;
        egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
            for (title, subgroups) in INSTALLATION {
                ui.label(RichText::new(tr(title)).strong());
                let mut list: Vec<&parts::Part> = Vec::new();
                for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()).chain(bundled_parts()) {
                    if p.wall_mountable() && subgroups.contains(&p.subgroup.as_str()) && !list.iter().any(|x| x.number == p.number) {
                        list.push(p);
                    }
                }
                list.sort_by(|a, b| a.manufacturer.cmp(&b.manufacturer).then(natural_cmp(&a.number, &b.number)));
                for p in list {
                    let active = self.tool == Tool::Accessory(p.number.clone());
                    let tip = p.tech_value("Mounting height").map_or_else(String::new, |h| tr_fmt!("Mounting height: {}", h));
                    if ui.selectable_label(active, format!("{} – {}", p.number, p.description)).on_hover_text(tip).clicked() {
                        chosen = Some(p.clone());
                        ui.close();
                    }
                }
                ui.add_space(4.0);
            }
            ui.horizontal(|ui| {
                ui.label(tr("Circuits of a generic manifold"));
                ui.add(egui::DragValue::new(&mut self.manifold_circuits).range(2..=12));
                if ui.button(tr("Choose")).clicked() {
                    let number = format!("GEN.HCM-{}", self.manifold_circuits);
                    chosen = parts::catalog().iter().find(|p| p.number == number).cloned();
                    ui.close();
                }
            });
        });
        let Some(p) = chosen else { return };
        let known = |prj: &Project, n: &str| prj.parts.iter().any(|x| x.number == n) || db.get(n).is_some() || parts::catalog().iter().any(|x| x.number == n);
        if !known(prj, &p.number) {
            // From a shipped catalog: into the project with its accessories.
            for a in &p.accessories {
                if !known(prj, a) {
                    if let Some(acc) = bundled_parts().iter().find(|x| &x.number == a) {
                        prj.parts.push(acc.clone());
                    }
                }
            }
            prj.parts.push(p.clone());
        }
        self.view = View::Front;
        self.set_tool(Tool::Accessory(p.number));
        self.status = tr("Click on the wall: the part goes there at its mounting height").to_string();
    }

    /// 19-inch parts (technical data `Rack units`) of the parts database, the project
    /// and the catalog, mounted in the highest free units of the rack shown.
    pub fn rack_part_menu(&mut self, ui: &mut egui::Ui, prj: &mut Project, db: &PartsDb) {
        let Some(ei) = self.enclosure_index(prj).filter(|&i| panel::is_rack(&prj.layout.enclosures[i])) else {
            ui.weak(tr("Choose a 19-inch rack first: 19-inch parts go on its rails."));
            return;
        };
        let mut list: Vec<parts::Part> = Vec::new();
        for p in db.parts.iter().chain(prj.parts.iter()).chain(parts::catalog()) {
            if p.tech_value("Rack units").is_some() && !list.iter().any(|x| x.number == p.number) {
                list.push(p.clone());
            }
        }
        list.sort_by(|a, b| a.manufacturer.cmp(&b.manufacturer).then(natural_cmp(&a.number, &b.number)));
        let e = &prj.layout.enclosures[ei];
        let used: u32 = panel::rack::occupied(e).iter().map(|o| o.1).sum();
        let total = panel::rack_units(e);
        ui.weak(tr_fmt!("{} of {} rack units free; a part goes into the highest free units", total.saturating_sub(used), total));
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for p in &list {
                let units = p.tech_value("Rack units").unwrap_or_default().to_string();
                if ui.button(format!("{} U  {} – {}", units, p.number, p.description)).clicked() {
                    if db.get(&p.number).is_some() {
                        parts::ensure_in_project(prj, db, &p.number);
                    }
                    match panel::mount_in_rack(prj, ei, &p.number, None) {
                        Ok(id) => {
                            self.front_face(Face::Plate, false);
                            self.select_item(id);
                            self.status = tr_fmt!("{} mounted in the rack", p.number);
                        }
                        Err(e) => self.status = e,
                    }
                    ui.close();
                }
            }
        });
    }

    /// Fans, filters, cable glands, lights and handles for the face shown.
    pub fn accessory_menu(&mut self, ui: &mut egui::Ui, prj: &Project, db: &PartsDb) {
        let face = self.face;
        let accessories: Vec<parts::Part> = accessory_parts(db, prj).into_iter().filter(|p| panel::allowed_faces(p).contains(&face)).collect();
        for p in &accessories {
            let active = self.tool == Tool::Accessory(p.number.clone());
            if ui.selectable_label(active, format!("{} – {}", p.subgroup, p.number)).on_hover_text(&p.description).clicked() {
                self.view = View::Front;
                self.set_tool(Tool::Accessory(p.number.clone()));
                ui.close();
            }
        }
        if accessories.is_empty() {
            ui.weak(tr("(no entries)"));
        }
    }

    /// The 3D macros: built-in and the project's own (which can be deleted).
    pub fn macro_menu(&mut self, ui: &mut egui::Ui, prj: &mut Project) {
        let macros = panel::all_macros(prj);
        let mut remove = None;
        for m in &macros {
            ui.horizontal(|ui| {
                let active = self.tool == Tool::Macro(m.name.clone());
                let text = if m.is_builtin() { m.title().to_string() } else { format!("★ {}", m.name) };
                if ui.selectable_label(active, text).on_hover_text(&m.description).clicked() {
                    self.view = View::Front;
                    self.set_tool(Tool::Macro(m.name.clone()));
                    ui.close();
                }
                if !m.is_builtin() && openecraft_ui::icons::button(ui, openecraft_ui::icons::Icon::Delete, tr("Delete macro")).clicked() {
                    remove = Some(m.name.clone());
                }
            });
        }
        if let Some(name) = remove {
            prj.layout.macros.retain(|m| m.name != name);
        }
    }

    /// Shows a face in the front view.
    fn show_face(&mut self, face: Face, inside: bool) {
        if self.face != face {
            self.selection.clear();
            self.selected_wire = None;
            self.fit_pending = true;
        }
        self.face = face;
        self.door_inside = inside;
        if !self.tool_allowed(&self.tool) {
            self.set_tool(Tool::Select);
        }
    }

    /// Mirror axis of the inside view of the door: face x ↔ `width - x`.
    fn mirror(&self, e: &Enclosure) -> Option<f32> {
        (self.face == Face::Door && self.door_inside).then_some(e.width)
    }

    fn s2w(&self, rect: Rect, s: Pos2) -> P {
        p((s.x - rect.min.x - self.offset.x) / self.zoom, (s.y - rect.min.y - self.offset.y) / self.zoom)
    }

    #[allow(clippy::too_many_lines)]
    fn front_view(&mut self, ui: &mut egui::Ui, prj: &mut Project, ei: usize, an: &Analysis, db: &PartsDb) {
        let pal = Palette::new(ui.visuals().dark_mode);
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        crate::editor::take_keys(ui, &resp);
        let rect = resp.rect;
        self.keep_centre(rect);
        let (ew, eh) = prj.layout.enclosures[ei].view_size(self.face);
        if self.fit_pending && rect.width() > 100.0 && rect.height() > 100.0 {
            self.zoom = ((rect.width() - 40.0) / ew).min((rect.height() - 40.0) / eh).max(0.05);
            self.offset = vec2((rect.width() - ew * self.zoom) / 2.0, (rect.height() - eh * self.zoom) / 2.0);
            self.fit_pending = false;
        }
        // Zoom / pan
        if resp.hovered() {
            let (wheel, zoom_delta, ptr) = ui.input(|i| (wheel_delta(i), i.zoom_delta(), i.pointer.hover_pos()));
            if let Some(ptr) = ptr {
                let factor = zoom_delta * (wheel * 0.0025).exp();
                if (factor - 1.0).abs() > 1e-4 {
                    let before = self.s2w(rect, ptr);
                    self.zoom = (self.zoom * factor).clamp(0.05, 20.0);
                    self.offset = ptr - rect.min - vec2(before.x, before.y) * self.zoom;
                }
            }
        }
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
            self.offset += resp.drag_delta();
        }
        let ptr = ui.input(|i| i.pointer.hover_pos()).filter(|q| rect.contains(*q));
        // Plate coordinates of the pointer
        let fm = prj.layout.enclosures[ei].face_offset(self.face);
        let mirror = self.mirror(&prj.layout.enclosures[ei]);
        let unmirror = move |q: P| p(mirror.map(|m| m - q.x).unwrap_or(q.x), q.y);
        let plate = ptr.map(|s| unmirror(self.s2w(rect, s)) - fm);
        let snapped = plate.map(|q| p(panel::snap(q.x), panel::snap(q.y)));

        // Keyboard
        if self.keys_active && !ui.ctx().egui_wants_keyboard_input() {
            let (esc, del, r, f) = ui.input(|i| {
                (i.key_pressed(Key::Escape), i.key_pressed(Key::Delete), i.key_pressed(Key::R) && i.modifiers.is_none(), i.key_pressed(Key::F) && i.modifiers.is_none())
            });
            let e = &mut prj.layout.enclosures[ei];
            if esc {
                if self.start.is_some() || !self.path_points.is_empty() {
                    self.start = None;
                    self.path_points.clear();
                } else if self.tool != Tool::Select {
                    self.set_tool(Tool::Select);
                } else {
                    self.deselect();
                }
            }
            if !self.tool_allowed(&self.tool) {
                self.set_tool(Tool::Select);
            }
            if del && !self.selection.is_empty() {
                panel::remove_items(e,&self.selection.iter().copied().collect::<Vec<_>>());
                self.selection.clear();
            }
            if r {
                rotate(e, &self.selection);
            }
            if f {
                self.fit_pending = true;
            }
        }
        if resp.clicked_by(PointerButton::Secondary) {
            if self.start.is_some() {
                self.start = None;
            } else {
                self.set_tool(Tool::Select);
            }
        }

        // Mouse
        match self.tool.clone() {
            Tool::Select => self.select_and_move(ui, &resp, prj, ei, rect, an),
            Tool::Rail | Tool::Duct | Tool::Busbar => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    match self.start.take() {
                        None => self.start = Some(sn),
                        Some(a) if !a.same(sn) => {
                            let (kind, part) = self.profile_here().unwrap_or((ItemKind::Duct, panel::DUCT_PART.into()));
                            parts::ensure_in_project(prj, db, &part);
                            if kind == ItemKind::Busbar {
                                parts::ensure_in_project(prj, db, openecraft_core::production::BUSBAR_HOLDER);
                            }
                            let item = PanelItem { face: self.face, ..panel::new_profile(prj, kind, &part, a, sn) };
                            prj.layout.enclosures[ei].items.push(item);
                        }
                        Some(_) => {}
                    }
                }
            }
            Tool::Cutout => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    match self.start.take() {
                        None => self.start = Some(sn),
                        Some(a) if (a.x - sn.x).abs() > 0.1 && (a.y - sn.y).abs() > 0.1 => {
                            let id = prj.new_id();
                            let (x, y) = (a.x.min(sn.x), a.y.min(sn.y));
                            let (w, h) = ((a.x - sn.x).abs(), (a.y - sn.y).abs());
                            let t = prj.layout.enclosures[ei].wall();
                            let (z, d) = match self.face {
                                Face::Plate => (-PLATE_THICKNESS, PLATE_THICKNESS),
                                Face::Door => (0.0, 2.0),
                                _ => (-t, t),
                            };
                            prj.layout.enclosures[ei].items.push(PanelItem { id, kind: ItemKind::Cutout, x, y, w, h, z, d, face: self.face, ..Default::default() });
                            self.select_item(id);
                        }
                        Some(_) => {}
                    }
                }
            }
            Tool::RoutingPath => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    if self.path_points.last().is_none_or(|l| !l.same(sn)) {
                        self.path_points.push(sn);
                    }
                }
                let enter = !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Enter));
                if (resp.double_clicked() || enter) && self.path_points.len() >= 2 {
                    let id = self.add_routing_path(prj, ei);
                    self.selected_object = Some(id);
                }
            }
            Tool::PipeRun => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    let q = ortho(self.path_points.last().copied(), sn);
                    if self.path_points.last().is_none_or(|l| !l.same(q)) {
                        self.path_points.push(q);
                    }
                }
                let enter = !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(Key::Enter));
                if (resp.double_clicked() || enter) && self.path_points.len() >= 2 {
                    let points = std::mem::take(&mut self.path_points);
                    match panel::add_pipe_run(prj, ei, self.face, &points, None, None, None) {
                        Ok(id) => self.selected_object = Some(id),
                        Err(e) => self.status = e,
                    }
                }
            }
            Tool::Reservation => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    match self.start.take() {
                        None => self.start = Some(sn),
                        Some(a) if (a.x - sn.x).abs() > 0.1 && (a.y - sn.y).abs() > 0.1 => {
                            let id = self.add_reservation(prj, ei, a, sn);
                            self.selected_object = Some(id);
                        }
                        Some(_) => {}
                    }
                }
            }
            Tool::PlaceDevice(tag) => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plate) {
                    let size = panel::device_size(prj, &tag);
                    let on_floor = self.in_room && self.face == Face::Bottom;
                    let (w, h) = size.map(|s| (s.w, if on_floor { s.d } else { s.h })).unwrap_or((panel::DEFAULT_DEVICE.0, panel::DEFAULT_DEVICE.1));
                    if let Some(id) = panel::place_device_on(prj, ei, &tag, p(q.x - w / 2.0, q.y - h / 2.0), self.face) {
                        self.select_item(id);
                        let e = &prj.layout.enclosures[ei];
                        if e.item(id).is_some_and(|it| !panel::mount_ok(prj, e, it)) {
                            self.status = if on_floor {
                                tr_fmt!("{} does not stand on the floor: choose a part marked for floor mounting", tag)
                            } else {
                                tr_fmt!("{} is not made for wall mounting: place it on a mounting rail", tag)
                            };
                        }
                    }
                    // Continue with the next unplaced device.
                    let next = panel::devices_to_place(prj).into_iter().next();
                    self.set_tool(next.map(|d| Tool::PlaceDevice(d.tag)).unwrap_or(Tool::Select));
                }
            }
            Tool::Walls | Tool::InnerWalls => {}
            Tool::Opening(opening) => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plate) {
                    let (w, _) = opening.size();
                    match panel::add_opening(prj, ei, opening, self.face, Some(q.x - w / 2.0), None, None) {
                        Ok(id) => self.select_item(id),
                        Err(e) => self.status = e,
                    }
                }
            }
            Tool::Accessory(part) => {
                if let (true, Some(q)) = (resp.clicked_by(PointerButton::Primary), plate) {
                    if db.get(&part).is_some() {
                        parts::ensure_in_project(prj, db, &part);
                    }
                    let probe = panel::new_accessory(prj, &part, self.face, p(0.0, 0.0));
                    let h = if self.in_room && self.face == Face::Bottom { probe.d } else { probe.h };
                    if let Some(id) = panel::place_accessory(prj, ei, &part, self.face, p(q.x - probe.w / 2.0, q.y - h / 2.0)) {
                        self.select_item(id);
                    }
                }
            }
            Tool::Macro(name) => {
                if let (true, Some(sn)) = (resp.clicked_by(PointerButton::Primary), snapped) {
                    if let Some(m) = panel::all_macros(prj).into_iter().find(|m| m.name == name) {
                        for it in m.items.iter().filter(|i| !i.part.is_empty()) {
                            if db.get(&it.part).is_some() {
                                parts::ensure_in_project(prj, db, &it.part);
                            }
                        }
                        let ids = panel::insert_macro(prj, ei, &m, self.face, sn);
                        self.selection = ids.into_iter().collect();
                        self.status = tr_fmt!("3D macro {} inserted", m.title());
                    }
                }
            }
        }

        // Draw
        let e = &prj.layout.enclosures[ei];
        painter.rect_filled(rect, 0.0, pal.canvas);
        let origin = rect.min + self.offset;
        let problems = panel::problems_of(&self.findings(prj, an, ei));
        let mut d = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: None, mirror, textures: None };
        render::draw_enclosure(&mut d, e, self.face, self.door_inside, p(0.0, 0.0), 1.0, &self.selection, &problems, prj);
        let to_screen = |q: P| origin + vec2(mirror.map(|m| m - (q.x + fm.x)).unwrap_or(q.x + fm.x), q.y + fm.y) * self.zoom;
        if self.show_heat && self.face == Face::Plate {
            let map = panel::thermal::heat_map(prj, e);
            let (pw, ph) = e.plate_size();
            let span = (map.max - map.min).max(1.0);
            for r in 0..map.rows {
                for c in 0..map.cols {
                    let (x, y) = (c as f32 * map.cell, r as f32 * map.cell);
                    let cell = Rect::from_two_pos(to_screen(p(x, y)), to_screen(p((x + map.cell).min(pw), (y + map.cell).min(ph))));
                    painter.rect_filled(cell, 0.0, heat_color((map.temps[r * map.cols + c] - map.min) / span).gamma_multiply(0.35));
                }
            }
            let hot = to_screen(map.hottest);
            painter.circle_stroke(hot, 6.0, Stroke::new(2.0, heat_color(1.0)));
            let label = tr_fmt!("{} °C (max.)", format!("{:.0}", map.max));
            painter.text(hot + vec2(9.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.0), heat_color(1.0));
            let range = tr_fmt!("Air temperature {} … {} °C", format!("{:.0}", map.min), format!("{:.0}", map.max));
            painter.text(rect.left_bottom() + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, range, egui::FontId::proportional(12.0), pal.line);
        }
        // Cable ducts coloured by their fill level.
        if self.show_fill {
            let limit = prj.layout.duct_fill_limit;
            for f in an.duct_fill.iter().filter(|f| f.enclosure == ei) {
                let Some(d) = e.item(f.duct).filter(|d| d.face == self.face && !self.hidden(d)) else { continue };
                let r = Rect::from_two_pos(to_screen(p(d.x, d.y)), to_screen(p(d.x + d.w, d.y + d.h)));
                let color = fill_color(f.percent, limit);
                painter.rect_filled(r, 0.0, color.gamma_multiply(0.3));
                if r.width().min(r.height()) >= 12.0 {
                    painter.text(r.center(), egui::Align2::CENTER_CENTER, format!("{:.0} %", f.percent), egui::FontId::proportional(11.0), color);
                }
            }
        }
        // Routing paths, reservations and the routed cables and tubes over the plate; the
        // pipe runs of a room's face.
        if self.face == Face::Plate {
            self.draw_routing_2d(&painter, prj, an, ei, &to_screen);
        } else if self.in_room {
            self.draw_pipe_runs(&painter, prj, ei, &to_screen);
        }
        // Wires on the face shown: the routes on the plate, or on the door from the door
        // devices to the hinge.
        let face = self.face;
        let selected_wire = self.wire_index(an).filter(|i| !an.wiring[*i].paths_in(ei, face).is_empty());
        // The wiring of the door is behind it: seen from the front only the selected wire.
        let show_wires = self.show_wires && !(face == Face::Door && !self.door_inside);
        if show_wires || selected_wire.is_some() {
            // Other wires are dimmed while one is selected.
            let normal = if selected_wire.is_some() { WIRE_COLOR.gamma_multiply(0.25) } else { WIRE_COLOR.gamma_multiply(0.7) };
            for (i, w) in an.wiring.iter().enumerate().filter(|(_, w)| w.in_enclosure(ei)) {
                if Some(i) != selected_wire && show_wires {
                    for path in w.paths_in(ei, face) {
                        let pts: Vec<Pos2> = path.iter().map(|q| to_screen(*q)).collect();
                        painter.add(Shape::line(pts, Stroke::new(1.0, normal)));
                    }
                }
            }
        }
        if let Some(i) = selected_wire {
            let w = &an.wiring[i];
            for path in w.paths_in(ei, face) {
                let pts: Vec<Pos2> = path.iter().map(|q| to_screen(*q)).collect();
                painter.add(Shape::line(pts.clone(), Stroke::new(3.0, WIRE_HIGHLIGHT)));
                for end in [pts.first(), pts.last()].into_iter().flatten() {
                    painter.circle_filled(*end, 4.5, WIRE_HIGHLIGHT);
                }
            }
            // The connected devices on this face
            for id in [w.items.0, w.items.1] {
                if let Some(it) = e.item(id).filter(|it| it.face == face) {
                    let r = Rect::from_two_pos(to_screen(p(it.x, it.y)), to_screen(p(it.x + it.w, it.y + it.h)));
                    painter.rect_filled(r, 0.0, WIRE_HIGHLIGHT.gamma_multiply(0.18));
                    painter.rect_stroke(r.expand(2.0), 2.0, Stroke::new(2.5, WIRE_HIGHLIGHT), egui::StrokeKind::Outside);
                }
            }
        }
        // The installation zones of a wall of a room, see-through.
        if self.show_zones && e.is_room() && matches!(self.face, Face::Wall(_) | Face::Inner(..)) {
            let zones = openecraft_core::routing3d::flush::wall_zones(e, self.face, &prj.layout.routing);
            let band = |x0: f32, y0: f32, x1: f32, y1: f32, kind| {
                let r = Rect::from_two_pos(to_screen(p(x0, y0)), to_screen(p(x1, y1)));
                painter.rect_filled(r, 0.0, scene3d::zone_color(kind).gamma_multiply(0.25));
            };
            for l in &zones.horizontal {
                for &(x0, x1) in &l.stretches {
                    band(x0, l.at - l.half, x1, l.at + l.half, prj.layout.routing.zones[l.zone].kind);
                }
            }
            for l in &zones.vertical {
                for &(y0, y1) in &l.stretches {
                    band(l.at - l.half, y0, l.at + l.half, y1, prj.layout.routing.zones[l.zone].kind);
                }
            }
        }
        // A wall under a sloped ceiling: the line where it ends, the part above it shaded.
        if let Some(line) = panel::roofs::ceiling(prj, ei).and_then(|c| panel::roofs::wall_line(e, &c, self.face)) {
            let pts: Vec<Pos2> = line.iter().map(|q| to_screen(p(q[0], q[1]))).collect();
            let top = to_screen(p(0.0, 0.0)).y;
            for w in pts.windows(2) {
                let quad = vec![egui::pos2(w[0].x, top), egui::pos2(w[1].x, top), w[1], w[0]];
                painter.add(egui::Shape::convex_polygon(quad, pal.ghost.gamma_multiply(0.25), Stroke::NONE));
            }
            painter.add(egui::Shape::line(pts, Stroke::new(2.0, crate::style::warning())));
        }
        // Connection points of the selected device.
        if let [id] = self.selection.iter().copied().collect::<Vec<_>>()[..] {
            if let Some(it) = e.item(id).filter(|it| it.kind == ItemKind::Device && it.face == self.face) {
                let ink = ui.visuals().text_color();
                for pt in panel::device_points(prj, it) {
                    let c = to_screen(p(pt.pos[0], pt.pos[1]));
                    let dir = match pt.exit {
                        openecraft_core::parts::Exit::Up => vec2(0.0, -1.0),
                        openecraft_core::parts::Exit::Down => vec2(0.0, 1.0),
                        openecraft_core::parts::Exit::Left => vec2(-1.0, 0.0),
                        openecraft_core::parts::Exit::Right => vec2(1.0, 0.0),
                        openecraft_core::parts::Exit::Front => vec2(0.0, 0.0),
                    };
                    painter.line_segment([c, c + dir * 9.0], Stroke::new(1.5, WIRE_COLOR));
                    if pt.exact {
                        painter.circle_filled(c, 3.0, WIRE_COLOR);
                    } else {
                        painter.circle_stroke(c, 3.0, Stroke::new(1.5, WIRE_COLOR));
                    }
                    if self.zoom >= 2.0 {
                        let name = if pt.terminal.is_empty() { pt.pin.clone() } else { format!("{}:{}", pt.terminal, pt.pin) };
                        painter.text(c - dir * 4.0, egui::Align2::CENTER_CENTER, name, egui::FontId::monospace(10.0), ink);
                    }
                }
            }
        }
        // Previews
        let mut g = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: Some(Ink::Ghost), mirror, textures: None };
        let m = fm;
        match (&self.tool, snapped, plate) {
            (Tool::RoutingPath, Some(sn), _) => {
                let mut pts: Vec<Pos2> = self.path_points.iter().map(|q| to_screen(*q)).collect();
                pts.push(to_screen(sn));
                painter.add(Shape::line(pts, Stroke::new((self.path_width * self.zoom).max(2.0), PATH_COLOR.gamma_multiply(0.4))));
                if !self.path_points.is_empty() {
                    let len: f32 = self.path_points.windows(2).map(|s| s[0].dist(s[1])).sum::<f32>() + self.path_points.last().map(|l| l.dist(sn)).unwrap_or(0.0);
                    self.status = tr_fmt!("Routing path {} mm – double-click or Enter to finish", format!("{len:.0}"));
                }
            }
            (Tool::PipeRun, Some(sn), _) => {
                let next = ortho(self.path_points.last().copied(), sn);
                let mut pts: Vec<Pos2> = self.path_points.iter().map(|q| to_screen(*q)).collect();
                pts.push(to_screen(next));
                painter.add(Shape::line(pts, Stroke::new((panel::PIPE_RUN_WIDTH * self.zoom).max(2.0), routing_ui::PIPE_COLOR.gamma_multiply(0.4))));
                if !self.path_points.is_empty() {
                    let len: f32 = self.path_points.windows(2).map(|s| s[0].dist(s[1])).sum::<f32>() + self.path_points.last().map(|l| l.dist(next)).unwrap_or(0.0);
                    self.status = tr_fmt!("Pipe run {} mm – double-click or Enter to finish", format!("{len:.0}"));
                }
            }
            (Tool::Reservation, Some(sn), _) => {
                if let Some(a) = self.start {
                    let r = Rect::from_two_pos(to_screen(a), to_screen(sn));
                    painter.rect_filled(r, 0.0, RESERVE_COLOR.gamma_multiply(0.25));
                }
            }
            (Tool::Cutout, Some(sn), _) => {
                if let Some(a) = self.start {
                    g.rect(a + m, sn + m, 0.5, Ink::Ghost, false);
                    self.status = tr_fmt!("Cutout {} × {} mm", format!("{:.0}", (sn.x - a.x).abs()), format!("{:.0}", (sn.y - a.y).abs()));
                }
                let c = to_screen(sn);
                painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], Stroke::new(1.0, pal.ghost));
                painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], Stroke::new(1.0, pal.ghost));
            }
            (Tool::Rail | Tool::Duct | Tool::Busbar, Some(sn), _) => {
                if let Some(a) = self.start {
                    let (kind, part) = self.profile_here().unwrap_or((ItemKind::Duct, panel::DUCT_PART.into()));
                    let pd = prj.parts.iter().chain(&db.parts).chain(parts::catalog()).find(|x| x.number == part);
                    let (pw, _) = panel::part_profile(kind, pd);
                    let horizontal = (sn.x - a.x).abs() >= (sn.y - a.y).abs();
                    let (r0, r1) = if horizontal {
                        (p(a.x.min(sn.x), a.y - pw / 2.0), p(a.x.max(sn.x), a.y + pw / 2.0))
                    } else {
                        (p(a.x - pw / 2.0, a.y.min(sn.y)), p(a.x + pw / 2.0, a.y.max(sn.y)))
                    };
                    g.rect(r0 + m, r1 + m, 0.5, Ink::Ghost, false);
                    let len = if horizontal { (sn.x - a.x).abs() } else { (sn.y - a.y).abs() };
                    self.status = tr_fmt!("Length {} mm", format!("{len:.0}"));
                }
                let c = to_screen(sn);
                painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], Stroke::new(1.0, pal.ghost));
                painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], Stroke::new(1.0, pal.ghost));
            }
            (Tool::Accessory(part), _, Some(q)) => {
                let pd = prj.parts.iter().chain(parts::catalog()).find(|x| &x.number == part);
                let [w, h, _] = pd.and_then(|x| x.size()).unwrap_or([panel::DEFAULT_DEVICE.0, panel::DEFAULT_DEVICE.1, 0.0]);
                let (x, y) = (panel::snap(q.x - w / 2.0), panel::snap(q.y - h / 2.0));
                g.rect(p(x, y) + m, p(x + w, y + h) + m, 0.6, Ink::Ghost, false);
            }
            (Tool::PlaceDevice(tag), _, Some(q)) => {
                let (w, h) = panel::device_size(prj, tag).map(|s| (s.w, s.h)).unwrap_or((panel::DEFAULT_DEVICE.0, panel::DEFAULT_DEVICE.1));
                let ghost = panel::device_preview(prj,e,tag,p(q.x-w/2.,q.y-h/2.),self.face);
                g.rect(p(ghost.x, ghost.y) + m, p(ghost.x + ghost.w, ghost.y + ghost.h) + m, 0.6, Ink::Ghost, false);
                g.text(ghost.center() + m, tag, (w.min(h) * 0.22).clamp(3.0, 9.0), openecraft_core::symbols::Anchor::Center, render::VAnchor::Middle, Ink::Ghost);
            }
            _ => {}
        }
        if let Some(q) = plate {
            if self.start.is_none() && !resp.clicked() {
                self.status = tr_fmt!("X {}  Y {} mm · {}", format!("{:.0}", q.x), format!("{:.0}", q.y), self.face.title());
            }
        }
    }

    fn select_and_move(&mut self, ui: &egui::Ui, resp: &egui::Response, prj: &mut Project, ei: usize, rect: Rect, an: &Analysis) {
        let shift = ui.input(|i| i.modifiers.shift);
        let tol = 6.0 / self.zoom;
        let face = self.face;
        let inside = self.door_inside;
        let hidden = move |i: &PanelItem| hidden_behind_door(inside, i);
        let hit = |e: &Enclosure, q: P| -> Option<u64> {
            // Top layer first: devices, busbars, ducts, rails, cutouts.
            let mut items: Vec<&PanelItem> = e.items.iter().filter(|i| i.face == face && !hidden(i) && i.rect().expand(tol).contains(q)).collect();
            items.sort_by_key(|i| std::cmp::Reverse(i.kind.layer()));
            items.first().map(|i| i.id)
        };
        let (offset, zoom, fm) = (self.offset, self.zoom, prj.layout.enclosures[ei].face_offset(face));
        let mirror = self.mirror(&prj.layout.enclosures[ei]);
        let to_plate = move |s: Pos2| {
            let x = (s.x - rect.min.x - offset.x) / zoom;
            p(mirror.map(|m| m - x).unwrap_or(x) - fm.x, (s.y - rect.min.y - offset.y) / zoom - fm.y)
        };
        if resp.drag_started_by(PointerButton::Primary) {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(rect.center());
            let q = to_plate(origin);
            let e = &prj.layout.enclosures[ei];
            if let Some(id) = hit(e, q) {
                self.selected_wire = None;
                if !self.selection.contains(&id) {
                    if !shift {
                        self.selection.clear();
                    }
                    self.selection.extend(with_group(e, id));
                }
                // Devices mounted on a moved rail move with it.
                let mut ids = self.selection.clone();
                for rail in e.items.iter().filter(|i| i.kind == ItemKind::Rail && self.selection.contains(&i.id)) {
                    for dev in e.items.iter().filter(|i| i.seated_on(rail)) {
                        ids.insert(dev.id);
                    }
                }
                let originals = e.items.iter().filter(|i| ids.contains(&i.id)).map(|i| (i.id, i.x, i.y)).collect();
                self.drag = Some(MoveDrag { start: q, originals });
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            if let (Some(drag), Some(ptr)) = (&self.drag, resp.interact_pointer_pos()) {
                let q = to_plate(ptr);
                let dx = panel::snap(q.x - drag.start.x);
                let dy = panel::snap(q.y - drag.start.y);
                let e = &mut prj.layout.enclosures[ei];
                for (id, x0, y0) in &drag.originals {
                    if let Some(it) = e.items.iter_mut().find(|i| i.id == *id) {
                        it.x = x0 + dx;
                        it.y = y0 + dy;
                    }
                }
                // Devices that are moved on their own snap onto rails.
                let moved: Vec<u64> = drag.originals.iter().map(|o| o.0).collect();
                let moving_rails = e.items.iter().any(|i| i.kind == ItemKind::Rail && moved.contains(&i.id));
                if !moving_rails {
                    let single = moved.len() == 1;
                    for id in moved {
                        if let Some(idx) = e.items.iter().position(|i| i.id == id) {
                            let mut it = e.items[idx].clone();
                            if single { panel::snap_device(e, &mut it); } else { panel::snap_to_rail(e, &mut it); }
                            e.items[idx] = it;
                        }
                    }
                }
                self.status = format!("ΔX {dx:.0}  ΔY {dy:.0} mm");
            }
        }
        if resp.drag_stopped() {
            if let Some(drag) = self.drag.take() {
                let moved: Vec<u64> = drag.originals.iter().map(|o| o.0).collect();
                panel::capture_anchors(&mut prj.layout.enclosures[ei], &moved);
            }
        }
        if resp.clicked_by(PointerButton::Primary) {
            if let Some(ptr) = resp.interact_pointer_pos() {
                let q = to_plate(ptr);
                let e = &prj.layout.enclosures[ei];
                let item = hit(e, q);
                let on_device = item.and_then(|id| e.item(id)).map(|i| i.kind == ItemKind::Device).unwrap_or(false);
                // Wires lie on top of ducts and rails, devices on top of wires.
                if self.show_wires && !(face == Face::Door && !inside) && !on_device && !shift {
                    let candidates = panel::wires_at(&an.wiring, ei, face, q, tol);
                    if !candidates.is_empty() {
                        self.pick_wire(an, &candidates);
                        return;
                    }
                    // Cables and tubes under the wires; clicking again steps through them.
                    let cables = if face == Face::Plate { self.cables_at(prj, an, ei, q, tol) } else { Vec::new() };
                    let current = self.selected_route.as_ref().and_then(|k| an.routing.items.iter().position(|i| &i.key == k));
                    if let Some(i) = panel::next_candidate(&cables, current) {
                        self.select_route(an, &an.routing.items[i].key);
                        return;
                    }
                }
                self.selected_wire = None;
                let alt = ui.input(|i| i.modifiers.alt);
                match item {
                    Some(id) if shift => {
                        if !self.selection.remove(&id) {
                            self.selection.insert(id);
                        }
                    }
                    // Alt: the single item of an assembly.
                    Some(id) if alt => self.select_item(id),
                    Some(id) => self.select_assembly(e, id),
                    None if !shift => self.deselect(),
                    None => {}
                }
            }
        }
    }
}

/// Items behind the door (cable ducts) are hidden in its front view: they cannot be
/// picked there.
fn hidden_behind_door(inside: bool, it: &PanelItem) -> bool {
    it.face == Face::Door && !inside && it.kind == ItemKind::Duct
}


fn apply_enclosure_part(ne: &mut NewEnclosure, db: &PartsDb) {
    if let Some(p) = db.get(&ne.part) {
        if p.width > 0.0 && p.height > 0.0 {
            ne.width = p.width;
            ne.height = p.height;
            if p.depth > 0.0 {
                ne.depth = p.depth;
            }
        }
    }
}

/// Dissolves the groups of the items (all members leave them).
fn ungroup(e: &mut Enclosure, ids: &[u64]) {
    let groups: HashSet<u64> = e.items.iter().filter(|i| ids.contains(&i.id) && i.group != 0).map(|i| i.group).collect();
    for it in e.items.iter_mut().filter(|i| groups.contains(&i.group)) {
        it.group = 0;
    }
    e.groups.retain(|g| !groups.contains(&g.id));
}

fn rotate(e: &mut Enclosure, sel: &BTreeSet<u64>) {
    panel::rotate_items(e,&sel.iter().copied().collect::<Vec<_>>());
}

/// Mouse wheel movement in points (same handling as the schematic canvas).
fn wheel_delta(i: &egui::InputState) -> f32 {
    let mut wheel = 0.0;
    for ev in &i.events {
        if let egui::Event::MouseWheel { unit, delta, modifiers, .. } = ev {
            if modifiers.ctrl || modifiers.command {
                continue;
            }
            wheel += match unit {
                egui::MouseWheelUnit::Point => delta.y,
                egui::MouseWheelUnit::Line => delta.y * 40.0,
                egui::MouseWheelUnit::Page => delta.y * 400.0,
            };
        }
    }
    wheel
}

/// Blue (cool) over yellow to red (hot) for 0 … 1.
fn heat_color(f: f32) -> Color32 {
    let f = f.clamp(0.0, 1.0);
    let lerp = |a: u8, b: u8, t: f32| (a as f32 + (b as f32 - a as f32) * t) as u8;
    if f < 0.5 {
        let t = f * 2.0;
        Color32::from_rgb(lerp(40, 240, t), lerp(110, 210, t), lerp(230, 40, t))
    } else {
        let t = (f - 0.5) * 2.0;
        Color32::from_rgb(lerp(240, 220, t), lerp(210, 40, t), lerp(40, 30, t))
    }
}

/// Green below 80 % of the limit, orange up to the limit, red above.
fn fill_color(percent: f32, limit: f32) -> Color32 {
    if percent > limit {
        crate::style::error()
    } else if percent > 0.8 * limit {
        crate::style::warning()
    } else {
        crate::style::ok()
    }
}

fn end_kind(kind: panel::EndKind) -> &'static str {
    match kind {
        panel::EndKind::Exact => tr("connection point"),
        panel::EndKind::Generated => tr("connection point (generated)"),
        panel::EndKind::Centre => tr("device centre"),
    }
}


/// What a face of a room carries: "1 rail, 2 devices, 1 door" (only what is there).
fn face_summary(items: &[&PanelItem]) -> String {
    let count = |f: &dyn Fn(&PanelItem) -> bool| items.iter().filter(|i| f(i)).count();
    let rails = count(&|i| i.kind == ItemKind::Rail);
    let ducts = count(&|i| i.kind == ItemKind::Duct);
    let doors = count(&|i| i.opening == Opening::Door);
    let windows = count(&|i| i.opening == Opening::Window);
    let devices = items.len() - rails - ducts - doors - windows;
    let parts: Vec<String> = [
        (rails, tr("1 rail"), tr_fmt!("{} rails", rails)),
        (ducts, tr("1 cable duct"), tr_fmt!("{} cable ducts", ducts)),
        (devices, tr("1 device"), tr_fmt!("{} devices", devices)),
        (doors, tr("1 door"), tr_fmt!("{} doors", doors)),
        (windows, tr("1 window"), tr_fmt!("{} windows", windows)),
    ]
    .into_iter()
    .filter(|(n, ..)| *n > 0)
    .map(|(n, one, many)| if n == 1 { one.to_string() } else { many })
    .collect();
    if parts.is_empty() { tr("empty").to_string() } else { parts.join(", ") }
}

#[cfg(test)]
mod tests;
