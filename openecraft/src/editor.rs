//! Interactive schematic canvas: pan/zoom, selection, moving, placing symbols,
//! drawing wires, texts, graphics, dimensions and connection / cable definitions.

use crate::commands::Cmd;
use crate::shortcuts::Action;
use openecraft_ui::part_editor::PartDrag;
use openecraft_i18n::{tr, tr_fmt};
use openecraft_core::geom::{dist_to_segment, p, P, R, GRID};
use openecraft_core::frame::PlotFrame;
use openecraft_core::index::Index;
use openecraft_core::model::*;
use openecraft_core::ops::{self, Clip, PlaceCtx};
use openecraft_core::render::{self, Draw, Ink, Opts, TextStyle, VAnchor};
use egui::emath::Rot2;
use egui::epaint::{Tessellator, TextShape};
use egui::Mesh;
use openecraft_core::analysis::Analysis;
use openecraft_core::review::{Hit, MarkupKind, Pen};
use openecraft_core::symlib::Symbols;
use openecraft_core::symbols::{Anchor, Func, SymbolDef};
use openecraft_core::device_boxes;
use openecraft_core::netlist::junctions;
use openecraft_core::structure::StructureBox;
use openecraft_core::{cabledef, conndef, graphics, routing, snap_lines, structure_boxes};
use egui::{pos2, vec2, Align2, Color32, FontId, Key, Painter, PointerButton, Pos2, Rect, Sense, Shape as EShape, Stroke, Vec2};
use std::collections::{BTreeSet, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq)]
pub enum Tool {
    Select,
    Place(&'static str),
    Wire,
    Text,
    Line,
    Rect,
    Circle,
    Ellipse,
    Arc,
    Polyline,
    Polygon,
    Dimension(DimKind),
    /// Connection definition point on a wire.
    ConnDef,
    /// Cable definition line across wires.
    CableDef,
    /// Structure box: a rectangle whose devices take other structure identifiers.
    StructureBox,
    /// Connection symbol: a directed T-junction where three wires meet, or a corner.
    Junction(JunctionKind),
    /// Place the items of `Editor::insert` (macro, image).
    Insert,
    /// Place a review comment.
    Comment,
    /// Draw redline / greenline markup.
    Markup(MarkupKind, Pen),
    /// Device box: two corners of its outline.
    DeviceBox,
}

impl Tool {
    /// Tools of the review layer: they work on every page.
    pub fn is_review(&self) -> bool {
        matches!(self, Tool::Comment | Tool::Markup(..))
    }

    /// Clicks needed by a graphic tool (None: open ended).
    fn points(&self) -> Option<usize> {
        match self {
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse | Tool::CableDef | Tool::DeviceBox | Tool::StructureBox => Some(2),
            Tool::Arc | Tool::Dimension(_) => Some(3),
            _ => None,
        }
    }

    fn shape_kind(&self) -> Option<ShapeKind> {
        Some(match self {
            Tool::Line => ShapeKind::Line,
            Tool::Rect => ShapeKind::Rect,
            Tool::Circle => ShapeKind::Circle,
            Tool::Ellipse => ShapeKind::Ellipse,
            Tool::Arc => ShapeKind::Arc,
            Tool::Polyline => ShapeKind::Polyline,
            Tool::Polygon => ShapeKind::Polygon,
            _ => return None,
        })
    }
}

/// Where the designation of pin `i` of a placed symbol is drawn.
fn pin_place(prj: &Project, idx: &Index, page_i: usize, s: &SymbolInst, i: usize) -> Option<render::PinLabelPlace> {
    prj.symbol(&s.sym).and_then(|def| render::pin_label_places(prj, idx, page_i, &def, s).into_iter().find(|l| l.pin == i))
}

/// Part of a symbol that can be dragged on its own.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Handle {
    /// The tag block.
    Tag,
    /// A placed property (index).
    Prop(usize),
    /// The designation of a connection point (pin index).
    Pin(usize),
    /// The designation of its only connection point, placed freely.
    OnlyPin,
    /// Name and cross reference of an interruption point.
    Intr(render::IntrText),
}

/// Text that can be dragged on its own: the tag block, a placed property, a
/// connection point designation or the interruption texts of a symbol, or a cable
/// core text of a wire.
#[derive(Clone, Copy, Debug, PartialEq)]
enum PropHandle {
    Symbol(u64, Handle),
    Wire(u64, render::CoreText),
}

impl PropHandle {
    /// The symbol or wire the text belongs to.
    fn id(self) -> u64 {
        match self {
            PropHandle::Symbol(id, _) | PropHandle::Wire(id, _) => id,
        }
    }
}

enum Drag {
    None,
    /// The selection with the connection points of its device boxes (`items`) moves;
    /// its connection points and wire ends (`points`) snap to the invisible grid lines
    /// of what stays (`lines`).
    Move { start: P, applied: P, moving: routing::Moving, items: BTreeSet<u64>, points: Vec<P>, lines: snap_lines::SnapLines },
    Box { start: Pos2 },
    /// `side`: side of a dragged connection point designation (symbol orientation).
    Prop { handle: PropHandle, start: P, orig: P, side: Option<LabelSide> },
}

pub struct Palette {
    pub canvas: Color32,
    pub paper: Color32,
    pub grid: Color32,
    pub line: Color32,
    pub wire: Color32,
    pub text: Color32,
    pub frame: Color32,
    pub selected: Color32,
    pub highlight: Color32,
    pub ghost: Color32,
    pub pin: Color32,
    pub crossref: Color32,
    pub error: Color32,
    pub plate: Color32,
    pub fill: Color32,
    /// Dark canvas: black items are drawn in the line colour to stay visible.
    pub dark: bool,
}

impl Palette {
    pub fn new(dark: bool) -> Palette {
        if dark {
            Palette {
                canvas: Color32::from_rgb(18, 19, 22),
                paper: Color32::from_rgb(32, 34, 38),
                grid: Color32::from_rgb(70, 72, 78),
                line: Color32::from_rgb(225, 225, 225),
                wire: Color32::from_rgb(120, 175, 255),
                text: Color32::from_rgb(232, 232, 232),
                frame: Color32::from_rgb(130, 133, 140),
                selected: Color32::from_rgb(255, 150, 40),
                highlight: Color32::from_rgb(255, 80, 220),
                ghost: Color32::from_rgba_unmultiplied(80, 170, 255, 170),
                pin: Color32::from_rgb(255, 90, 90),
                crossref: Color32::from_rgb(140, 180, 255),
                error: Color32::from_rgb(255, 90, 90),
                plate: Color32::from_rgb(52, 55, 60),
                fill: Color32::from_rgb(78, 82, 90),
                dark: true,
            }
        } else {
            Palette {
                canvas: Color32::from_rgb(190, 193, 198),
                paper: Color32::WHITE,
                grid: Color32::from_rgb(190, 190, 190),
                line: Color32::BLACK,
                wire: Color32::from_rgb(0, 40, 160),
                text: Color32::BLACK,
                frame: Color32::from_rgb(90, 90, 90),
                selected: Color32::from_rgb(235, 110, 0),
                highlight: Color32::from_rgb(210, 0, 170),
                ghost: Color32::from_rgba_unmultiplied(0, 110, 230, 170),
                pin: Color32::from_rgb(220, 0, 0),
                crossref: Color32::from_rgb(20, 60, 170),
                error: Color32::from_rgb(220, 0, 0),
                plate: Color32::from_rgb(236, 238, 241),
                fill: Color32::from_rgb(214, 218, 224),
                dark: false,
            }
        }
    }

    pub(crate) fn ink(&self, ink: Ink) -> Color32 {
        match ink {
            Ink::Line => self.line,
            Ink::Wire => self.wire,
            Ink::Text => self.text,
            Ink::Frame | Ink::FrameText => self.frame,
            Ink::Selected => self.selected,
            Ink::Highlight => self.highlight,
            Ink::Ghost => self.ghost,
            Ink::Pin => self.pin,
            Ink::CrossRef => self.crossref,
            Ink::Error => self.error,
            Ink::Plate => self.plate,
            Ink::Fill => self.fill,
            // Black (wires of colour BK, black shapes) would vanish on a dark canvas.
            Ink::Color([r, g, b]) if self.dark && r.max(g).max(b) < 64 => self.line,
            Ink::Color([r, g, b]) => Color32::from_rgb(r, g, b),
            Ink::Paper => self.paper,
        }
    }
}

/// Textures of the images on a page: image id -> (fingerprint of the file, texture;
/// None when the file cannot be decoded).
pub type Textures = HashMap<u64, (u64, Option<egui::TextureHandle>)>;

/// Renders page geometry with an egui painter.
pub struct EguiDraw<'a> {
    pub painter: &'a Painter,
    pub origin: Pos2,
    pub zoom: f32,
    pub pal: &'a Palette,
    /// Forces every element to this ink (ghost previews).
    pub force: Option<Ink>,
    /// Mirrors the drawing at `x = mirror / 2` (inside view of a door).
    pub mirror: Option<f32>,
    /// Decoded images (bitmaps are drawn as frames without).
    pub textures: Option<&'a Textures>,
}

impl EguiDraw<'_> {
    fn w2s(&self, q: P) -> Pos2 {
        let x = self.mirror.map(|m| m - q.x).unwrap_or(q.x);
        pos2(self.origin.x + x * self.zoom, self.origin.y + q.y * self.zoom)
    }
    fn color(&self, ink: Ink) -> Color32 {
        self.pal.ink(self.force.unwrap_or(ink))
    }
    fn stroke(&self, width: f32, ink: Ink) -> Stroke {
        Stroke::new((width * self.zoom).max(1.0), self.color(ink))
    }
}

impl Draw for EguiDraw<'_> {
    fn line(&mut self, a: P, b: P, width: f32, ink: Ink, dashed: bool) {
        let pts = [self.w2s(a), self.w2s(b)];
        let st = self.stroke(width, ink);
        if dashed {
            let z = self.zoom.max(0.5);
            self.painter.extend(EShape::dashed_line(&pts, st, 1.5 * z, 1.0 * z));
        } else {
            self.painter.line_segment(pts, st);
        }
    }

    fn poly(&mut self, pts: &[P], closed: bool, width: f32, ink: Ink, fill: bool) {
        let v: Vec<Pos2> = pts.iter().map(|q| self.w2s(*q)).collect();
        if fill {
            let st = if width > 0.0 { self.stroke(width, ink) } else { Stroke::NONE };
            if pts.len() > 3 && !convex(pts) {
                // Fan triangles of the even-odd filled area.
                self.painter.add(EShape::mesh(fill_mesh(pts, &v, self.color(ink))));
                if width > 0.0 {
                    self.painter.add(EShape::closed_line(v, st));
                }
            } else {
                self.painter.add(EShape::convex_polygon(v, self.color(ink), st));
            }
        } else if closed {
            self.painter.add(EShape::closed_line(v, self.stroke(width, ink)));
        } else {
            self.painter.add(EShape::line(v, self.stroke(width, ink)));
        }
    }

    fn circle(&mut self, c: P, r: f32, width: f32, ink: Ink, fill: bool) {
        let c = self.w2s(c);
        let r = (r * self.zoom).max(1.0);
        if fill {
            self.painter.circle_filled(c, r, self.color(ink));
        } else {
            self.painter.circle_stroke(c, r, self.stroke(width, ink));
        }
    }

    fn text(&mut self, pos: P, text: &str, size: f32, h: Anchor, v: VAnchor, ink: Ink) {
        self.styled_text(pos, text, &TextStyle::plain(size, h, v), ink);
    }

    fn styled_text(&mut self, pos: P, text: &str, st: &TextStyle, ink: Ink) {
        let px = st.size * self.zoom * 1.25;
        if px < 3.0 || text.is_empty() {
            return;
        }
        let color = self.color(ink);
        let galley = self.painter.layout_no_wrap(text.to_string(), FontId::proportional(px), color);
        let size = galley.size();
        // Top left corner of the text relative to its anchor, before the rotation.
        let off = vec2(
            match st.h {
                Anchor::Left => 0.0,
                Anchor::Center => -size.x / 2.0,
                Anchor::Right => -size.x,
            },
            match st.v {
                VAnchor::Top => 0.0,
                VAnchor::Middle => -size.y / 2.0,
                VAnchor::Bottom => -size.y,
            },
        );
        // egui turns clockwise; the text's angle is counter-clockwise.
        let angle = -st.angle.to_radians();
        let rot = Rot2::from_angle(angle);
        let anchor = self.w2s(pos);
        let underline = if st.underline { Stroke::new((px * 0.06).max(1.0), color) } else { Stroke::NONE };
        // Bold: a second copy a little to the right.
        let copies: &[f32] = if st.bold { &[0.0, px * 0.05] } else { &[0.0] };
        for dx in copies {
            let shape = TextShape::new(anchor + rot * (off + vec2(*dx, 0.0)), galley.clone(), color).with_angle(angle).with_underline(underline);
            if st.italic {
                self.painter.add(EShape::mesh(skewed(self.painter, &shape, 0.2)));
            } else {
                self.painter.add(EShape::Text(shape));
            }
        }
    }

    fn image_quad(&mut self, img: &ImageItem, corners: &[P; 4]) {
        let key = egui::Id::unique(("symbol-bitmap", fingerprint(&img.file)));
        let texture = self.painter.ctx().data_mut(|data| data.get_temp::<Option<egui::TextureHandle>>(key));
        let texture = texture.unwrap_or_else(|| {
            let texture = decode_image(&img.file).map(|image| self.painter.ctx().load_texture(
                format!("symbol-bitmap-{}", img.id), image, egui::TextureOptions::LINEAR));
            self.painter.ctx().data_mut(|data| data.insert_temp(key, texture.clone()));
            texture
        });
        if let Some(texture) = texture.filter(|_| self.force.is_none()) {
            let mut mesh = Mesh::with_texture(texture.id());
            for (corner, uv) in corners.iter().zip([pos2(0., 0.), pos2(1., 0.), pos2(1., 1.), pos2(0., 1.)]) {
                mesh.vertices.push(egui::epaint::Vertex { pos: self.w2s(*corner), uv, color: Color32::WHITE });
            }
            mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
            self.painter.add(EShape::mesh(mesh));
        } else {
            render::image_placeholder(self, img);
        }
    }

    fn image(&mut self, img: &ImageItem) {
        let tex = self.textures.and_then(|t| t.get(&img.id)).and_then(|t| t.1.as_ref());
        match (tex, self.force) {
            (Some(tex), None) => {
                let r = Rect::from_two_pos(self.w2s(img.a), self.w2s(img.b));
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                self.painter.image(tex.id(), r, uv, Color32::WHITE);
            }
            _ => render::image_placeholder(self, img),
        }
    }
}

/// Tessellates a text shape with its glyphs leaning by `k` (italic): a shear along
/// the baseline, applied before the shape's rotation.
fn skewed(painter: &Painter, shape: &TextShape, k: f32) -> Mesh {
    let tex = painter.fonts(|f| f.font_image_size());
    let options = painter.ctx().tessellation_options(|o| *o);
    let local = TextShape::new(Pos2::ZERO, shape.galley.clone(), shape.fallback_color).with_underline(shape.underline);
    let mut mesh = Mesh::default();
    Tessellator::new(painter.pixels_per_point(), options, tex, Vec::new()).tessellate_text(&local, &mut mesh);
    let h = shape.galley.size().y;
    let rot = Rot2::from_angle(shape.angle);
    for v in &mut mesh.vertices {
        v.pos = shape.pos + rot * vec2(v.pos.x + (h - v.pos.y) * k, v.pos.y);
    }
    mesh
}

fn convex(pts: &[P]) -> bool {
    let n = pts.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let z = (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x);
        if z.abs() > 1e-6 {
            if sign != 0.0 && z.signum() != sign {
                return false;
            }
            sign = z.signum();
        }
    }
    true
}

/// Ear clipping of a simple polygon (screen points `v`, page points `pts`).
fn fill_mesh(pts: &[P], v: &[Pos2], color: Color32) -> egui::Mesh {
    let mut mesh = egui::Mesh::default();
    for q in v {
        mesh.colored_vertex(*q, color);
    }
    let area: f32 = (0..pts.len()).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        a.x * b.y - b.x * a.y
    }).sum();
    let mut idx: Vec<usize> = (0..pts.len()).collect();
    let cross = |a: P, b: P, c: P| (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
    let mut guard = 0;
    while idx.len() > 3 && guard < 10_000 {
        guard += 1;
        let n = idx.len();
        let mut cut = None;
        for i in 0..n {
            let (ia, ib, ic) = (idx[(i + n - 1) % n], idx[i], idx[(i + 1) % n]);
            let (a, b, c) = (pts[ia], pts[ib], pts[ic]);
            if cross(a, b, c) * area <= 0.0 {
                continue;
            }
            let inside = idx.iter().filter(|&&k| k != ia && k != ib && k != ic).any(|&k| {
                let q = pts[k];
                let (d1, d2, d3) = (cross(a, b, q), cross(b, c, q), cross(c, a, q));
                (d1 * area >= 0.0) && (d2 * area >= 0.0) && (d3 * area >= 0.0)
            });
            if !inside {
                cut = Some(i);
                break;
            }
        }
        let Some(i) = cut else { break };
        let n = idx.len();
        mesh.add_triangle(idx[(i + n - 1) % n] as u32, idx[i] as u32, idx[(i + 1) % n] as u32);
        idx.remove(i);
    }
    if idx.len() == 3 {
        mesh.add_triangle(idx[0] as u32, idx[1] as u32, idx[2] as u32);
    }
    mesh
}

/// A function dragged from a navigator onto the page: symbol, device tag, label and
/// connection point designations.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacePayload {
    pub symbol: &'static str,
    pub tag: String,
    pub label: String,
    pub pins: Vec<String>,
}

/// Pixels per millimetre at 100 % (real size on a 96 dpi screen).
pub const ZOOM_ACTUAL: f32 = 3.78;

pub struct Editor {
    pub tool: Tool,
    /// Screen offset of page origin relative to the canvas rect.
    pub offset: Vec2,
    /// Pixels per millimetre.
    pub zoom: f32,
    pub selection: BTreeSet<u64>,
    pub place_rot: u8,
    pub place_mirror: bool,
    /// The connection symbol the junction tools place where the wires do not decide
    /// it (R turns it).
    pub symbol_proto: Junction,
    pub show_grid: bool,
    /// Snap and drawing grid in mm (EPLAN grid sizes; connection points are on 4 mm).
    pub grid: f32,
    /// A point typed into the coordinate field: used like a click of the current tool.
    pub typed: Option<P>,
    /// Last point clicked or typed (relative coordinates `@dx dy`).
    pub last_point: Option<P>,
    pub show_pins: bool,
    pub place_ctx: PlaceCtx,
    pub fit_pending: bool,
    pub hover: Option<P>,
    pub status: String,
    pub center_on: Option<P>,
    /// Items placed with `Tool::Insert`.
    pub insert: Option<Clip>,
    /// The window macro (with its placeholder values) `insert` comes from: recorded as
    /// a macro box when placed.
    pub insert_macro: Option<(openecraft_core::macros::Macro, std::collections::BTreeMap<String, String>)>,
    /// Macro boxes are drawn.
    pub show_macro_boxes: bool,
    pub show_revision_marks: bool,
    /// Layer of new graphics, texts, dimensions and images.
    pub active_layer: String,
    /// The next text to place (path function text, special text); None: a plain text.
    pub text_proto: Option<TextItem>,
    /// An unplaced function to place once: label (PLC address) and connection point
    /// designations of the next symbol; the tool returns to Select afterwards.
    pub place_once: Option<(String, Vec<String>)>,
    /// Connect facing connection points after placing or moving symbols.
    pub autoconnect: bool,
    /// The wire tool routes around symbols and other wires.
    pub autoroute: bool,
    /// Cable of new cable definition lines; empty = a new cable.
    pub cable_target: String,
    /// Device tag of the next symbols placed (navigator: place a function); ends with
    /// the Place tool.
    pub place_tag: Option<String>,
    /// Part of the next symbol placed (Insert center): goes to its device.
    pub place_part: Option<String>,
    /// The page is locked by another user of a shared project.
    pub read_only: bool,
    /// Why the page is read only (shown on the page instead of the hint of generated pages).
    pub read_only_reason: String,
    /// Comments and markup are drawn.
    pub review_layer: bool,
    /// Selected comment or markup.
    pub review_selected: Option<Hit>,
    wire_start: Option<P>,
    wire_hfirst: bool,
    /// Points clicked so far by a graphic tool.
    pts: Vec<P>,
    drag: Drag,
    canvas_size: Vec2,
    /// Plot frame of the page shown.
    frame: PlotFrame,
    textures: Textures,
    /// Wire routing preview: from, to and the path.
    route_cache: Option<(P, P, Vec<P>)>,
    /// The user's keyboard shortcuts.
    pub shortcuts: crate::shortcuts::Shortcuts,
    /// The page is the document worked in: it takes the keys.
    pub keys_active: bool,
    /// The items copied or cut last (Home › Paste).
    pub clip: Option<Clip>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor {
            tool: Tool::Select,
            offset: vec2(20.0, 20.0),
            zoom: 3.0,
            selection: BTreeSet::new(),
            place_rot: 0,
            place_mirror: false,
            symbol_proto: Junction::new(JunctionKind::Tee, P::default()),
            show_grid: true,
            grid: GRID,
            typed: None,
            last_point: None,
            show_pins: false,
            place_ctx: PlaceCtx::default(),
            fit_pending: true,
            hover: None,
            status: String::new(),
            center_on: None,
            insert: None,
            show_revision_marks: true,
            active_layer: String::new(),
            autoconnect: true,
            autoroute: false,
            shortcuts: Default::default(),
            cable_target: String::new(),
            place_tag: None,
            place_part: None,
            read_only: false,
            read_only_reason: String::new(),
            review_layer: true,
            insert_macro: None,
            show_macro_boxes: true,
            review_selected: None,
            wire_start: None,
            wire_hfirst: true,
            pts: Vec::new(),
            drag: Drag::None,
            canvas_size: vec2(800.0, 600.0),
            frame: PlotFrame::default(),
            textures: HashMap::new(),
            route_cache: None,
            text_proto: None,
            place_once: None,
            keys_active: true,
            clip: None,
        }
    }
}

/// What the canvas wants the application to do.
#[derive(Default)]
pub struct CanvasOutput {
    /// The user double clicked an item (show its properties).
    pub open_properties: bool,
    /// Message for the status line.
    pub message: Option<String>,
    /// A comment or markup to add (review tools).
    pub review: Option<ReviewInput>,
    /// Click with the select tool (world position), for picking comments and markup.
    pub clicked: Option<P>,
    /// Hyperlink of a text the user Ctrl-clicked.
    pub open_url: Option<String>,
    /// A symbol was placed (its id).
    pub placed: Option<u64>,
    /// The part chosen for the symbol placed.
    pub placed_part: Option<String>,
    /// A part dropped onto the page (parts management, data portal) and where.
    pub dropped_part: Option<(String, P)>,
    /// The canvas with the select tool, for the context menu.
    pub response: Option<egui::Response>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ReviewInput {
    Comment(P),
    Markup(MarkupKind, Pen, P, P),
}

/// Cheap fingerprint of an embedded file (changes when it is replaced).
fn fingerprint(doc: &openecraft_core::document::Document) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    doc.data.len().hash(&mut h);
    doc.data.get(..256.min(doc.data.len())).hash(&mut h);
    doc.data.get(doc.data.len().saturating_sub(256)..).hash(&mut h);
    h.finish()
}

fn decode_image(doc: &openecraft_core::document::Document) -> Option<egui::ColorImage> {
    let bytes = doc.bytes().ok()?;
    let img = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

fn ghost_shape(g: &mut dyn Draw, kind: ShapeKind, a: P, b: P, pts: Vec<P>, sweep_deg: f32) {
    let s = Shape { kind, a, b, pts, sweep: sweep_deg, ..Default::default() };
    render::draw_shape(g, &s, Ink::Ghost);
}

/// Counter-clockwise angle in degrees from `start` to `end` around `c`.
fn sweep(c: P, start: P, end: P) -> f32 {
    let ang = |q: P| (-(q.y - c.y)).atan2(q.x - c.x).to_degrees();
    (ang(end) - ang(start)).rem_euclid(360.0)
}

/// Whether new items may go onto `layer`: it is shown and not locked.
pub fn layer_usable(prj: &Project, layer: &str) -> bool {
    graphics::shown(prj, layer, false) && !graphics::locked(prj, layer)
}

/// Window selection: from left to right the items inside the window, from right to
/// left (crossing) also the items it touches.
pub fn box_selects(window: R, crossing: bool, item: R) -> bool {
    if crossing {
        window.min.x <= item.max.x && item.min.x <= window.max.x && window.min.y <= item.max.y && item.min.y <= window.max.y
    } else {
        window.contains_rect(&item)
    }
}

impl Editor {
    pub fn set_tool(&mut self, tool: Tool) {
        if !matches!(tool, Tool::Place(_)) {
            self.place_tag = None;
        }
        if let Tool::Junction(kind) = tool {
            if self.symbol_proto.kind != kind {
                self.symbol_proto = Junction::new(kind, P::default());
            }
        }
        self.tool = tool;
        self.wire_start = None;
        self.text_proto = None;
        self.place_once = None;
        self.place_part = None;
        self.pts.clear();
        if self.tool != Tool::Select {
            self.selection.clear();
        }
    }

    /// The keys of a binding for the hints ("–" without a key).
    fn key(&self, id: &str) -> String {
        let k = self.shortcuts.text(id);
        if k.is_empty() { "–".to_string() } else { k }
    }

    fn cmd_key(&self, cmd: Cmd) -> String {
        self.key(crate::commands::def(cmd).id)
    }

    /// What the current tool expects, with the user's keys.
    pub fn hint(&self) -> String {
        let finish = Action::FinishPolyline.id();
        match &self.tool {
            Tool::Select => tr("Click to select, drag to move, drag on empty space for window selection (from right to left: crossing). Drag the tag or a property text of a selected symbol to place it. Right/middle drag pans, wheel zooms.").to_string(),
            Tool::Place(key) if *key == openecraft_core::symbols::DEVICE_POINT => tr_fmt!(
                "Click on the outline of a device box: the point belongs to its device, leads away from the edge and is numbered on · {} turns it off the outline · Esc or right click to finish",
                self.cmd_key(Cmd::Rotate)
            ),
            Tool::DeviceBox => tr("Click two corners of the device box, then place device connection points on its outline · Esc or right click to finish").to_string(),
            Tool::Place(_) => match &self.place_tag {
                Some(tag) => tr_fmt!("Click to place for device {} · {} rotate · {} mirror · Esc or right click to finish", tag, self.cmd_key(Cmd::Rotate), self.cmd_key(Cmd::Mirror)),
                None => tr_fmt!("Click to place · {} rotate · {} mirror · Esc or right click to finish", self.cmd_key(Cmd::Rotate), self.cmd_key(Cmd::Mirror)),
            },
            Tool::Wire => tr_fmt!(
                "Click start and corner points · {} toggles corner direction · {} toggles routing around symbols · ends automatically on a connection · {} or double click ends the wire · Esc or right click to finish",
                self.key(Action::WireCorner.id()),
                self.key(Action::Autoroute.id()),
                self.key(finish)
            ),
            Tool::Text => tr("Click to place a text · Esc or right click to finish").to_string(),
            Tool::Line | Tool::Rect | Tool::Ellipse | Tool::Markup(MarkupKind::Line | MarkupKind::Rect | MarkupKind::Cloud, _) => tr("Click start point, click end point · Esc or right click to finish").to_string(),
            Tool::Circle => tr("Click the centre, then a point of the circle · Esc or right click to finish").to_string(),
            Tool::Arc => tr("Click the centre, the start point and the end point (counter-clockwise) · Esc or right click to finish").to_string(),
            Tool::Polyline | Tool::Polygon => tr_fmt!("Click the corner points · double click, {} or right click ends the shape · Esc to finish", self.key(finish)),
            Tool::Dimension(_) => tr("Click the two points to measure, then the position of the dimension line · Esc or right click to finish").to_string(),
            Tool::ConnDef => tr("Click on a wire: the point defines number, colour, cross-section and cable core of the connection · Esc or right click to finish").to_string(),
            Tool::Junction(JunctionKind::Tee) => tr_fmt!(
                "Click where three wires meet: the T-junction names the device the wires start from and the order of their targets · {} turns it where no wires meet yet · Esc or right click to finish",
                self.cmd_key(Cmd::Rotate)
            ),
            Tool::Junction(JunctionKind::Corner) => tr_fmt!(
                "Click where two wires meet at a right angle, or where connection points face each other across the corner: with Auto-connect on, wires are drawn to it · {} turns it · Esc or right click to finish",
                self.cmd_key(Cmd::Rotate)
            ),
            Tool::Junction(JunctionKind::Crossing) => tr_fmt!(
                "Click where two wires cross: the crossing connects them and leads each end of its line straight on first, then across · {} turns it · Esc or right click to finish",
                self.cmd_key(Cmd::Rotate)
            ),
            Tool::Junction(JunctionKind::Bridge) => tr_fmt!(
                "Click where four wires meet: the bridge leads the connection from one side to the three others, the opposite side first · {} turns it · Esc or right click to finish",
                self.cmd_key(Cmd::Rotate)
            ),
            Tool::CableDef if self.cable_target.is_empty() => tr("Draw a line across the wires that are cores of a new cable · Esc or right click to finish").to_string(),
            Tool::CableDef => tr_fmt!("Draw a line across the wires that are cores of cable {} · Esc or right click to finish", self.cable_target),
            Tool::StructureBox => tr("Click two corners of the structure box, then set its identifiers in the properties · Esc or right click to finish").to_string(),
            Tool::Insert => tr("Click to insert · Esc cancel").to_string(),
            Tool::Comment => tr("Click to place a comment").to_string(),
            Tool::Markup(..) => tr("Click to place the markup text").to_string(),
        }
    }

    /// Where Paste puts the items: at the cursor over the page, else at the last
    /// point clicked, else near the top left corner of the drawing area.
    pub fn paste_point(&self) -> P {
        self.hover.map(|q| q.snap(self.grid)).or(self.last_point).unwrap_or(self.frame.area().min + p(20.0, 20.0))
    }

    /// True while a multi-click operation (wire, rectangle) is in progress.
    pub fn busy(&self) -> bool {
        self.wire_start.is_some() || !self.pts.is_empty() || !matches!(self.drag, Drag::None)
    }

    pub fn cancel(&mut self) {
        self.wire_start = None;
        self.pts.clear();
        self.drag = Drag::None;
    }

    fn s2w(&self, rect: Rect, s: Pos2) -> P {
        p((s.x - rect.min.x - self.offset.x) / self.zoom, (s.y - rect.min.y - self.offset.y) / self.zoom)
    }

    fn w2s(&self, rect: Rect, q: P) -> Pos2 {
        pos2(rect.min.x + self.offset.x + q.x * self.zoom, rect.min.y + self.offset.y + q.y * self.zoom)
    }

    pub fn fit(&mut self, size: Vec2) {
        let margin = 16.0;
        let (w, h) = self.frame.size();
        self.zoom = ((size.x - 2.0 * margin) / w).min((size.y - 2.0 * margin) / h).max(0.1);
        self.offset = vec2((size.x - w * self.zoom) / 2.0, (size.y - h * self.zoom) / 2.0);
    }

    /// Zooms about the middle of the canvas (View › Zoom).
    pub fn zoom_center(&mut self, factor: f32) {
        let rect = Rect::from_min_size(Pos2::ZERO, self.canvas_size);
        self.zoom_at(rect, rect.center(), factor);
    }

    fn zoom_at(&mut self, rect: Rect, screen: Pos2, factor: f32) {
        let before = self.s2w(rect, screen);
        self.zoom = (self.zoom * factor).clamp(0.4, 80.0);
        let after = self.w2s(rect, before);
        self.offset += screen - after;
    }

    /// The selection with the connection points of the device boxes in it: what moves,
    /// turns, copies and deletes.
    pub fn items(&self, prj: &Project, page_i: usize) -> BTreeSet<u64> {
        device_boxes::with_points(prj, page_i, &self.selection)
    }

    /// Hit test: returns the id of the item under `q` (world), preferring texts and
    /// electrical symbols. Decorative symbols stay behind wires and their
    /// definitions. Items on hidden or locked layers are skipped.
    pub fn hit(&self, prj: &Project, page_i: usize, q: P) -> Option<u64> {
        let page = &prj.pages[page_i];
        let tol = (8.0 / self.zoom).max(0.4);
        let usable = |layer: &str| graphics::shown(prj, layer, false) && !graphics::locked(prj, layer);
        for t in page.texts.iter().rev().filter(|t| usable(&t.layer)) {
            if render::text_bounds(prj, page_i, t).expand(tol * 0.5).contains(q) {
                return Some(t.id);
            }
        }
        let mut best: Option<(f32, u64)> = None;
        let mut graphic: Option<(f32, u64)> = None;
        for s in page.symbols.iter().rev() {
            let Some(def) = prj.symbol(&s.sym) else { continue };
            let b = s.bounds(&def);
            // A device box is picked at its outline: what is inside stays selectable.
            if device_boxes::is_box(s) && !device_boxes::outline_hit(b, q, tol) {
                continue;
            }
            if b.expand(tol * 0.5).contains(q) {
                let d = b.center().dist(q) / (b.width() + b.height()).max(1.0);
                let candidate = if def.func == openecraft_core::symbols::Func::Graphic { &mut graphic } else { &mut best };
                if candidate.map(|(bd, _)| d < bd).unwrap_or(true) {
                    *candidate = Some((d, s.id));
                }
            }
        }
        if let Some((_, id)) = best {
            return Some(id);
        }
        if let Some(c) = page.conn_defs.iter().rev().find(|c| c.pos.dist(q) <= tol.max(1.2)) {
            return Some(c.id);
        }
        if let Some(j) = page.junctions.iter().rev().find(|j| j.pos.dist(q) <= tol.max(1.2)) {
            return Some(j.id);
        }
        for w in page.wires.iter().rev() {
            if dist_to_segment(q, w.a, w.b) <= tol {
                return Some(w.id);
            }
        }
        if let Some(c) = page.cable_defs.iter().rev().find(|c| dist_to_segment(q, c.a, c.b) <= tol) {
            return Some(c.id);
        }
        if let Some((_, id)) = graphic {
            return Some(id);
        }
        if let Some(d) = page.dimensions.iter().rev().find(|d| usable(&d.layer) && graphics::dim_hit(d, q, tol)) {
            return Some(d.id);
        }
        if let Some(s) = page.shapes.iter().rev().find(|s| usable(&s.layer) && graphics::shape_hit(s, q, tol)) {
            return Some(s.id);
        }
        // A structure box is picked at its outline, so that clicks inside reach its devices.
        if let Some(b) = page.structure_boxes.iter().rev().find(|b| structure_boxes::outline_distance(b, q) <= tol) {
            return Some(b.id);
        }
        page.images.iter().rev().find(|i| usable(&i.layer) && R::from_points(i.a, i.b).contains(q)).map(|i| i.id)
    }

    /// Tag block or placed property of a selected symbol under `q`.
    fn prop_hit(&self, prj: &Project, idx: &Index, page_i: usize, q: P) -> Option<PropHandle> {
        let page = &prj.pages[page_i];
        for s in page.symbols.iter().filter(|s| self.selection.contains(&s.id)) {
            for (i, pp) in s.props.iter().enumerate() {
                if render::prop_bounds(prj, page_i, s, pp).expand(0.3).contains(q) {
                    return Some(PropHandle::Symbol(s.id, Handle::Prop(i)));
                }
            }
            let Some(def) = prj.symbol(&s.sym) else { continue };
            if let Some(l) = render::pin_label_places(prj, idx, page_i, &def, s).iter().find(|l| l.bounds.expand(0.2).contains(q)) {
                let pin = if def.pins.len() == 1 { Handle::OnlyPin } else { Handle::Pin(l.pin) };
                return Some(PropHandle::Symbol(s.id, pin));
            }
            if render::tag_block_bounds(prj, idx, page_i, &def, s).is_some_and(|r| r.expand(0.3).contains(q)) {
                return Some(PropHandle::Symbol(s.id, Handle::Tag));
            }
            if let Some((t, _)) = render::interruption_text_bounds(prj, idx, page_i, &def, s).into_iter().find(|(_, r)| r.expand(0.3).contains(q)) {
                return Some(PropHandle::Symbol(s.id, Handle::Intr(t)));
            }
        }
        for w in page.wires.iter().filter(|w| self.selection.contains(&w.id)) {
            for which in [render::CoreText::Core, render::CoreText::Cable] {
                if render::core_text_bounds(page, w, which).is_some_and(|r| r.expand(0.3).contains(q)) {
                    return Some(PropHandle::Wire(w.id, which));
                }
            }
        }
        None
    }

    /// Does `q` touch a connection point, an existing wire or a T-junction (to end a
    /// wire there)?
    fn is_connection(symbols: &Symbols, page: &Page, q: P) -> bool {
        let pin = page.symbols.iter().any(|s| {
            symbols.get(&s.sym)
                .map(|d| (0..d.pins.len()).any(|i| d.pin_world(i, s.pos, s.rot, s.mirror).same(q)))
                .unwrap_or(false)
        });
        pin || page.wires.iter().any(|w| dist_to_segment(q, w.a, w.b) < 0.01) || page.junctions.iter().any(|j| j.pos.same(q))
    }

    /// The connection symbol the tool would place at `q`: a T-junction with its stem
    /// after the wires meeting there, a corner with the arms of the two wires there,
    /// else in the tool's direction.
    fn junction_at(&self, page: &Page, q: P, kind: JunctionKind) -> Junction {
        let arms = junctions::arms(page, q);
        let proto = if self.symbol_proto.kind == kind { self.symbol_proto.clone() } else { Junction::new(kind, q) };
        match kind {
            JunctionKind::Tee => match arms.as_deref().and_then(junctions::stem_of) {
                Some(dir) => Junction { pos: q, dir, variant: 3, ..proto },
                None => Junction { pos: q, ..proto },
            },
            JunctionKind::Corner => match arms.as_deref().and_then(junctions::corner_of) {
                Some((v, h)) => Junction::corner(q, v, h),
                None => Junction { pos: q, ..proto },
            },
            JunctionKind::Crossing | JunctionKind::Bridge => Junction { pos: q, ..proto },
        }
    }

    fn wire_path(&mut self, prj: &Project, page: &Page, a: P, b: P) -> Vec<P> {
        if self.autoroute {
            if let Some((ca, cb, path)) = &self.route_cache {
                if ca.same(a) && cb.same(b) {
                    return path.clone();
                }
            }
            let path = routing::route(&prj.symbol_table(), page, self.frame.area(), a, b);
            self.route_cache = Some((a, b, path.clone()));
            return path;
        }
        if a.x == b.x || a.y == b.y {
            return vec![a, b];
        }
        let corner = if self.wire_hfirst { p(b.x, a.y) } else { p(a.x, b.y) };
        vec![a, corner, b]
    }

    /// Point on a wire for a connection definition: the grid point if it is on a
    /// wire, otherwise the nearest point of a wire close to the pointer.
    fn wire_point(&self, page: &Page, world: P, snapped: P) -> Option<P> {
        if conndef::wire_at(page, snapped).is_some() {
            return Some(snapped);
        }
        let tol = (8.0 / self.zoom).max(0.4);
        let w = page.wires.iter().find(|w| dist_to_segment(world, w.a, w.b) <= tol)?;
        let ab = w.b - w.a;
        let l2 = ab.x * ab.x + ab.y * ab.y;
        let t = if l2 > 0.0 { (((world.x - w.a.x) * ab.x + (world.y - w.a.y) * ab.y) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let q = w.a + ab * t;
        // Whole millimetres along axis parallel wires.
        Some(if w.a.x == w.b.x { p(w.a.x, q.y.round()) } else if w.a.y == w.b.y { p(q.x.round(), w.a.y) } else { q })
    }

    /// Loads textures of the page's images that are new or changed.
    fn update_textures(&mut self, ctx: &egui::Context, page: &Page) {
        for img in &page.images {
            let fp = fingerprint(&img.file);
            if self.textures.get(&img.id).is_some_and(|t| t.0 == fp) {
                continue;
            }
            let tex = decode_image(&img.file).map(|ci| ctx.load_texture(format!("page-image-{}", img.id), ci, egui::TextureOptions::LINEAR));
            self.textures.insert(img.id, (fp, tex));
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn ui(&mut self, ui: &mut egui::Ui, prj: &mut Project, page_i: usize, idx: &Index, an: &Analysis) -> CanvasOutput {
        let mut out = CanvasOutput::default();
        let dark = ui.visuals().dark_mode;
        let pal = Palette::new(dark);
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        take_keys(ui, &resp);
        let rect = resp.rect;
        self.canvas_size = rect.size();
        let frame = openecraft_core::frame::of_page(prj, page_i);
        if frame.size() != self.frame.size() {
            self.fit_pending = true;
        }
        self.frame = frame;
        // Wait for a sensible canvas size (the first web frame can be tiny).
        if self.fit_pending && rect.width() > 200.0 && rect.height() > 150.0 {
            self.fit(rect.size());
            self.fit_pending = false;
        }
        if let Some(c) = self.center_on.take() {
            self.offset = rect.size() / 2.0 - vec2(c.x, c.y) * self.zoom;
        }

        // ------------------------------------------------------------ navigation
        if resp.hovered() {
            // Raw wheel events: CAD style zoom around the cursor, no smoothing.
            let (wheel, zoom_delta, ptr) = ui.input(|i| {
                let mut wheel = 0.0;
                for e in &i.events {
                    if let egui::Event::MouseWheel { unit, delta, modifiers, .. } = e {
                        if modifiers.ctrl || modifiers.command {
                            continue; // already contained in zoom_delta()
                        }
                        wheel += match unit {
                            egui::MouseWheelUnit::Point => delta.y,
                            egui::MouseWheelUnit::Line => delta.y * 40.0,
                            egui::MouseWheelUnit::Page => delta.y * 400.0,
                        };
                    }
                }
                (wheel, i.zoom_delta(), i.pointer.hover_pos())
            });
            if let Some(ptr) = ptr {
                let factor = zoom_delta * (wheel * 0.0025).exp();
                if (factor - 1.0).abs() > 1e-4 {
                    self.zoom_at(rect, ptr, factor);
                }
            }
        }
        if resp.dragged_by(PointerButton::Middle) || resp.dragged_by(PointerButton::Secondary) {
            self.offset += resp.drag_delta();
        }

        let locked = prj.pages[page_i].locked;
        let editable = prj.pages[page_i].is_editable() && !self.read_only;
        let ptr = ui.input(|i| i.pointer.hover_pos()).filter(|p| rect.contains(*p));
        let world = ptr.map(|s| self.s2w(rect, s));
        // Connection points and wire ends off the grid are invisible grid lines.
        let placing = match self.tool {
            Tool::Place(key) => Some((key, true)),
            _ => resp.dnd_hover_payload::<PlacePayload>().or_else(|| resp.dnd_release_payload::<PlacePayload>()).map(|pl| (pl.symbol, false)),
        };
        let snapped = world.map(|w| self.snap_pointer(prj, &prj.pages[page_i], w, placing));
        self.hover = world;
        // New items never go onto a hidden or locked layer.
        if !layer_usable(prj, &self.active_layer) {
            self.active_layer.clear();
        }
        // A click into the page takes the keys from a text field.
        if resp.hovered() && ui.input(|i| i.pointer.any_pressed()) {
            ui.memory_mut(|m| {
                if let Some(id) = m.focused() {
                    m.surrender_focus(id);
                }
            });
        }

        // ------------------------------------------------------------ keyboard
        let kb = self.keys_active && !ui.ctx().egui_wants_keyboard_input();
        if kb {
            self.keyboard(ui, prj, page_i, &mut out);
        }

        // ------------------------------------------------------------ mouse
        if self.tool == Tool::Select && resp.clicked_by(PointerButton::Primary) {
            out.clicked = world;
        }
        // Right click selects what is under the cursor; the app shows the context menu.
        if self.tool == Tool::Select {
            if resp.secondary_clicked() {
                if let Some(id) = world.and_then(|w| self.hit(prj, page_i, w)) {
                    if !self.selection.contains(&id) {
                        self.selection = BTreeSet::from([id]);
                    }
                }
            }
            out.response = Some(resp.clone());
        }
        if self.tool.is_review() {
            self.review_mouse(&resp, world, &mut out);
        } else if editable {
            self.mouse(ui, &resp, prj, page_i, rect, world, snapped, an, idx, &mut out);
        } else if resp.dnd_hover_payload::<PartDrag>().is_some() {
            // A part dragged onto a released, locked or generated page: say why it
            // cannot be dropped here instead of ignoring it.
            ui.ctx().set_cursor_icon(egui::CursorIcon::NoDrop);
            if resp.dnd_release_payload::<PartDrag>().is_some() {
                out.message = Some(tr("Parts can only be dropped onto pages that can be changed; this one is released, locked or generated").to_string());
            }
        } else if self.tool == Tool::Select {
            // Read only: select and inspect, no moves.
            if let Some(q) = self.typed.take() {
                self.select_at(prj, page_i, q, false);
            }
            self.select_mouse(ui, &resp, prj, idx, page_i, rect, world, false, &mut out);
        } else if self.typed.take().is_some() {
            out.message = Some(tr("The page is read only").to_string());
        }
        self.cursor(ui, &resp, prj, idx, page_i, world, editable);

        // ------------------------------------------------------------ drawing
        self.update_textures(ui.ctx(), &prj.pages[page_i]);
        painter.rect_filled(rect, 0.0, pal.canvas);
        let page_rect = Rect::from_min_max(self.w2s(rect, p(0.0, 0.0)), self.w2s(rect, p(self.frame.width(), self.frame.height())));
        painter.rect_filled(page_rect.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(60));
        painter.rect_filled(page_rect, 0.0, pal.paper);
        if self.show_grid && editable && self.zoom * self.grid >= 7.0 {
            self.draw_grid(&painter, rect, &pal);
        }

        let mut opts = Opts {
            selected: self.items(prj, page_i),
            show_pins: self.show_pins || self.tool == Tool::Wire,
            hide_revision_marks: !self.show_revision_marks,
            review: self.review_layer || self.tool.is_review(),
            review_selected: self.review_selected,
            screen: true,
            macro_boxes: self.show_macro_boxes,
            ..Default::default()
        };
        // Highlight the complete potential of a selected wire.
        if self.selection.len() == 1 {
            let id = *self.selection.iter().next().unwrap();
            if let Some(net) = an.netlist.net_of_wire(id) {
                opts.highlight_wires = net.wires.iter().map(|w| w.1).collect::<HashSet<u64>>();
            }
        }
        let origin = rect.min + self.offset;
        let mut d = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: None, mirror: None, textures: Some(&self.textures) };
        render::draw_page(&mut d, prj, idx, an, page_i, &opts);

        // Previews
        if let (Tool::Markup(kind, _), Some(&a), Some(b)) = (&self.tool, self.pts.first(), world) {
            let mut g = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: Some(Ink::Ghost), mirror: None, textures: None };
            match kind {
                MarkupKind::Line => g.line(a, b, 0.5, Ink::Ghost, true),
                _ => g.rect(a, b, 0.5, Ink::Ghost, false),
            }
        }
        if let (Some(w), true) = (world, self.tool == Tool::Select && matches!(self.drag, Drag::None)) {
            self.draw_hover(&painter, rect, &pal, prj, page_i, w, an);
        }
        if let (Some(sn), Some(pl), true) = (snapped, resp.dnd_hover_payload::<PlacePayload>(), editable) {
            if let Some(def) = prj.symbol(pl.symbol) {
                let mut g = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: Some(Ink::Ghost), mirror: None, textures: None };
                let inst = SymbolInst { pos: sn, ..ops::symbol_preview(prj, &def) };
                render::draw_symbol(&mut g, &def, &inst, Ink::Ghost);
            }
        }
        if let (Some(sn), Some(pl), true) = (snapped, resp.dnd_hover_payload::<PartDrag>(), editable) {
            // Where the dragged part lands, with its number.
            let c = self.w2s(rect, sn);
            let st = Stroke::new(1.5, pal.ghost);
            painter.line_segment([c - vec2(10.0, 0.0), c + vec2(10.0, 0.0)], st);
            painter.line_segment([c - vec2(0.0, 10.0), c + vec2(0.0, 10.0)], st);
            let text = painter.layout_no_wrap(pl.number.clone(), egui::FontId::proportional(12.0), pal.ghost);
            let at = c + vec2(12.0, -12.0 - text.size().y);
            painter.rect_filled(Rect::from_min_size(at, text.size()).expand(3.0), 3.0, pal.ghost.gamma_multiply(0.18));
            painter.galley(at, text, pal.ghost);
        }
        if let (Some(sn), Some(w), true) = (snapped, world, editable) {
            let mut g = EguiDraw { painter: &painter, origin, zoom: self.zoom, pal: &pal, force: Some(Ink::Ghost), mirror: None, textures: None };
            self.previews(&mut g, prj, &prj.pages[page_i], w, sn);
            if self.tool != Tool::Select {
                let c = self.w2s(rect, sn);
                let st = Stroke::new(1.0, pal.ghost);
                painter.line_segment([c - vec2(8.0, 0.0), c + vec2(8.0, 0.0)], st);
                painter.line_segment([c - vec2(0.0, 8.0), c + vec2(0.0, 8.0)], st);
            }
        }
        if let Drag::Box { start } = self.drag {
            if let Some(ptr) = ptr {
                let r = Rect::from_two_pos(start, ptr);
                let st = Stroke::new(1.0, pal.ghost);
                if ptr.x < start.x {
                    // Crossing: dashed, a lighter fill.
                    painter.rect_filled(r, 0.0, pal.ghost.gamma_multiply(0.07));
                    let c = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
                    painter.extend(EShape::dashed_line(&c, st, 5.0, 3.0));
                } else {
                    painter.rect_filled(r, 0.0, pal.ghost.gamma_multiply(0.15));
                    painter.rect_stroke(r, 0.0, st, egui::StrokeKind::Inside);
                }
            }
        }
        // Handles of the movable texts of a single selected symbol or wire.
        if self.tool == Tool::Select && self.selection.len() == 1 && editable {
            let page = &prj.pages[page_i];
            let mut boxes: Vec<R> = Vec::new();
            if let Some(s) = page.symbols.iter().find(|s| self.selection.contains(&s.id)) {
                boxes.extend(s.props.iter().map(|pp| render::prop_bounds(prj, page_i, s, pp)));
                if let Some(def) = prj.symbol(&s.sym) {
                    boxes.extend(render::tag_block_bounds(prj, idx, page_i, &def, s));
                    boxes.extend(render::pin_label_places(prj, idx, page_i, &def, s).into_iter().map(|l| l.bounds));
                }
            }
            if let Some(w) = page.wires.iter().find(|w| self.selection.contains(&w.id)) {
                boxes.extend([render::CoreText::Core, render::CoreText::Cable].into_iter().filter_map(|which| render::core_text_bounds(page, w, which)));
            }
            for r in boxes {
                let sr = Rect::from_min_max(self.w2s(rect, r.min), self.w2s(rect, r.max)).expand(1.0);
                painter.rect_stroke(sr, 0.0, Stroke::new(1.0, pal.ghost), egui::StrokeKind::Outside);
            }
        }
        if !editable {
            let msg = if locked {
                tr("Released page – unlock it in Project data › Revisions to change it")
            } else if self.read_only && !self.read_only_reason.is_empty() {
                self.read_only_reason.as_str()
            } else {
                tr("Generated page – edit via page properties")
            };
            painter.text(rect.left_top() + vec2(10.0, 10.0), Align2::LEFT_TOP, msg, FontId::proportional(13.0), pal.frame);
        }

        // Status line
        self.status = match (world, snapped) {
            (Some(w), Some(_)) => {
                let mut s = tr_fmt!("Column {}  Row {}", self.frame.column_of(w), self.frame.row_of(w));
                if let Some(id) = self.hit(prj, page_i, w) {
                    let page = &prj.pages[page_i];
                    if let Some(sym) = page.symbols.iter().find(|s| s.id == id) {
                        let name = prj.symbol(&sym.sym).map(|d| tr(d.name)).unwrap_or("?");
                        s += &format!("   |  {} {} {}", sym.tag, name, sym.label);
                    } else if let Some(net) = an.netlist.net_of_wire(id) {
                        s += &tr_fmt!("   |  Potential {}", net.name);
                    } else if let Some(dm) = page.dimensions.iter().find(|d| d.id == id) {
                        s += &format!("   |  {}", graphics::dim_text(dm));
                    } else if let Some(c) = page.cable_defs.iter().find(|c| c.id == id) {
                        s += &format!("   |  {}", c.cable);
                    } else if let Some(b) = page.structure_boxes.iter().find(|b| b.id == id) {
                        s += &format!("   |  {} {}", tr("Structure box"), b.identifiers());
                    }
                }
                if self.tool == Tool::Wire && self.autoroute {
                    s += &format!("   |  {}", tr("Routing around symbols"));
                }
                s
            }
            _ => String::new(),
        };
        out
    }

    fn previews(&mut self, g: &mut EguiDraw<'_>, prj: &Project, page: &Page, world: P, sn: P) {
        match self.tool.clone() {
            Tool::Place(key) => {
                if let Some(def) = prj.symbol(key) {
                    let inst = SymbolInst {
                        pos: sn,
                        rot: self.place_rotation(prj, page, &def, sn),
                        mirror: self.place_mirror,
                        ..ops::symbol_preview(prj, &def)
                    };
                    // A tinted box behind it: the symbol stays visible on a page zoomed out.
                    let b = inst.bounds(&def);
                    let r = Rect::from_two_pos(g.w2s(b.min), g.w2s(b.max)).expand(3.0);
                    g.painter.rect_filled(r, 2.0, g.pal.ghost.gamma_multiply(0.18));
                    render::draw_symbol(g, &def, &inst, Ink::Ghost);
                }
            }
            Tool::Wire => {
                if let Some(a) = self.wire_start {
                    let path = self.wire_path(prj, page, a, sn);
                    for w in path.windows(2) {
                        g.line(w[0], w[1], render::WIRE_W, Ink::Ghost, false);
                    }
                }
            }
            Tool::Line | Tool::Rect | Tool::Circle | Tool::Ellipse => {
                if let (Some(&a), Some(kind)) = (self.pts.first(), self.tool.shape_kind()) {
                    ghost_shape(g, kind, a, sn, Vec::new(), 0.0);
                }
            }
            Tool::DeviceBox => {
                if let Some(&a) = self.pts.first() {
                    let r = R::from_points(a, sn);
                    let c = [r.min, p(r.max.x, r.min.y), r.max, p(r.min.x, r.max.y)];
                    for i in 0..4 {
                        g.line(c[i], c[(i + 1) % 4], render::SYMBOL_W, Ink::Ghost, true);
                    }
                }
            }
            Tool::StructureBox => {
                if let Some(&a) = self.pts.first() {
                    let r = R::from_points(a, sn);
                    render::draw_structure_box(g, &StructureBox { a: r.min, b: r.max, ..Default::default() }, Ink::Ghost);
                }
            }
            Tool::Arc => match self.pts.as_slice() {
                [c] => g.line(*c, sn, render::SYMBOL_W, Ink::Ghost, true),
                [c, s] => ghost_shape(g, ShapeKind::Arc, *c, *s, Vec::new(), sweep(*c, *s, sn)),
                _ => {}
            },
            Tool::Polyline | Tool::Polygon => {
                if !self.pts.is_empty() {
                    let mut pts = self.pts.clone();
                    pts.push(sn);
                    let kind = self.tool.shape_kind().unwrap_or_default();
                    ghost_shape(g, kind, P::default(), P::default(), pts, 0.0);
                }
            }
            Tool::Dimension(kind) => match self.pts.as_slice() {
                [a] => g.line(*a, sn, render::SYMBOL_W * 0.7, Ink::Ghost, true),
                [a, b] => {
                    let mut dm = Dimension { kind, a: *a, b: *b, ..Default::default() };
                    dm.offset = graphics::dim_offset_through(&dm, sn);
                    render::draw_dimension(g, &dm, Ink::Ghost);
                }
                _ => {}
            },
            Tool::CableDef => {
                if let Some(&a) = self.pts.first() {
                    g.line(a, sn, render::SYMBOL_W, Ink::Ghost, false);
                    for id in cabledef::crossed_wires(page, a, sn) {
                        if let Some(w) = page.wires.iter().find(|w| w.id == id) {
                            g.line(w.a, w.b, render::WIRE_W * 2.0, Ink::Ghost, false);
                        }
                    }
                }
            }
            Tool::ConnDef => {
                if let Some(q) = self.wire_point(page, world, sn) {
                    let r = 0.9;
                    g.poly(&[q + p(0.0, -r), q + p(r, 0.0), q + p(0.0, r), q + p(-r, 0.0)], true, render::SYMBOL_W, Ink::Ghost, true);
                }
            }
            Tool::Junction(kind) => {
                let j = self.junction_at(page, sn, kind);
                // The arms themselves where no wires are there yet.
                if junctions::arms(page, sn).is_none_or(|a| a.is_empty()) {
                    for a in j.arms() {
                        g.line(sn, sn + a.vector() * (render::JUNCTION_R * 2.0), render::WIRE_W, Ink::Ghost, false);
                    }
                }
                render::junction_symbol(g, &j, Ink::Ghost, !j.is_corner(), true);
            }
            Tool::Comment | Tool::Markup(..) => {}
            Tool::Insert => {
                if let Some(clip) = &self.insert {
                    let d = ops::paste_offset(clip, sn);
                    for s in &clip.symbols {
                        if let Some(def) = clip.symbol(prj,&s.sym) {
                            let inst = SymbolInst { pos: s.pos + d, ..s.clone() };
                            render::draw_symbol(g, &def, &inst, Ink::Ghost);
                        }
                    }
                    for w in &clip.wires {
                        g.line(w.a + d, w.b + d, render::WIRE_W, Ink::Ghost, false);
                    }
                    for s in &clip.shapes {
                        let mut s = s.clone();
                        graphics::map_shape(&mut s, |q| q + d);
                        render::draw_shape(g, &s, Ink::Ghost);
                    }
                    for i in &clip.images {
                        g.rect(i.a + d, i.b + d, render::SYMBOL_W, Ink::Ghost, false);
                    }
                }
            }
            Tool::Select | Tool::Text => {}
        }
    }

    fn draw_grid(&self, painter: &Painter, rect: Rect, pal: &Palette) {
        let tl = self.s2w(rect, rect.min);
        let br = self.s2w(rect, rect.max);
        let g = self.grid;
        let x0 = (tl.x.max(0.0) / g).ceil() as i32;
        let x1 = (br.x.min(self.frame.width()) / g).floor() as i32;
        let y0 = (tl.y.max(0.0) / g).ceil() as i32;
        let y1 = (br.y.min(self.frame.height()) / g).floor() as i32;
        let mut mesh = egui::Mesh::default();
        let r = if self.zoom * g > 20.0 { 1.0 } else { 0.6 };
        for gx in x0..=x1 {
            for gy in y0..=y1 {
                let c = self.w2s(rect, p(gx as f32 * g, gy as f32 * g));
                mesh.add_colored_rect(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), pal.grid);
            }
        }
        painter.add(EShape::mesh(mesh));
    }

    fn keyboard(&mut self, ui: &egui::Ui, prj: &mut Project, page_i: usize, out: &mut CanvasOutput) {
        let editable = prj.pages[page_i].is_editable() && !self.read_only;
        let g = self.grid;
        let sc = &self.shortcuts;
        let (esc, enter, r, m, a, del, space, fit, all, arrows, events) = ui.input(|i| {
            (
                i.key_pressed(Key::Escape),
                sc.action_pressed(i, Action::FinishPolyline),
                sc.cmd_pressed(i, Cmd::Rotate),
                sc.cmd_pressed(i, Cmd::Mirror),
                sc.action_pressed(i, Action::Autoroute),
                sc.cmd_pressed(i, Cmd::Delete),
                sc.action_pressed(i, Action::WireCorner),
                sc.cmd_pressed(i, Cmd::FitPage),
                sc.cmd_pressed(i, Cmd::SelectAll),
                [
                    (sc.action_pressed(i, Action::NudgeLeft), p(-g, 0.0)),
                    (sc.action_pressed(i, Action::NudgeRight), p(g, 0.0)),
                    (sc.action_pressed(i, Action::NudgeUp), p(0.0, -g)),
                    (sc.action_pressed(i, Action::NudgeDown), p(0.0, g)),
                ],
                i.events.clone(),
            )
        });
        if esc {
            if self.wire_start.is_some() || !self.pts.is_empty() {
                self.wire_start = None;
                self.pts.clear();
            } else if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            } else {
                self.selection.clear();
                self.place_tag = None;
            }
        }
        if fit {
            self.fit(self.canvas_size);
        }
        if all {
            let page = &prj.pages[page_i];
            self.selection = ops::all_ids(page).into_iter().filter(|id| graphics::item_layer(page, *id).is_none_or(|l| !graphics::locked(prj, l))).collect();
        }
        // Copy works on read only pages too.
        for ev in &events {
            if matches!(ev, egui::Event::Copy) && !self.selection.is_empty() {
                let clip = ops::copy(&prj.pages[page_i], &self.items(prj, page_i));
                ui.ctx().copy_text(clip.to_text());
                self.clip = Some(clip);
            }
        }
        if !editable {
            return;
        }
        if enter {
            if self.tool == Tool::Wire {
                self.wire_start = None;
            } else {
                self.finish_poly(prj, page_i);
            }
        }
        if a && self.tool == Tool::Wire {
            self.autoroute = !self.autoroute;
            self.route_cache = None;
        }
        if r {
            if matches!(self.tool, Tool::Place(_)) {
                self.place_rot = (self.place_rot + 1) % 4;
            } else if matches!(self.tool, Tool::Junction(_)) {
                self.symbol_proto.turn();
            } else if !self.selection.is_empty() {
                routing::rotate(prj, page_i, &self.items(prj, page_i));
            }
        }
        let items = self.items(prj, page_i);
        let page = &mut prj.pages[page_i];
        if m {
            if matches!(self.tool, Tool::Place(_)) {
                self.place_mirror = !self.place_mirror;
            } else {
                ops::mirror(page, &items);
            }
        }
        if space && self.tool == Tool::Wire {
            self.wire_hfirst = !self.wire_hfirst;
        }
        if del && !self.selection.is_empty() {
            ops::delete(page, &items);
            self.selection.clear();
        }
        if !self.selection.is_empty() && arrows.iter().any(|(pressed, _)| *pressed) {
            for (pressed, d) in arrows {
                if pressed {
                    routing::move_selection(prj, page_i, &items, d);
                }
            }
            // Like a drag: facing connection points of moved symbols are connected.
            let moved_symbols = prj.pages[page_i].symbols.iter().any(|s| self.selection.contains(&s.id))
                || prj.pages[page_i].junctions.iter().any(|j| self.selection.contains(&j.id));
            if self.autoconnect && moved_symbols {
                let n = routing::apply_autoconnect(prj, page_i, &self.selection);
                if n > 0 {
                    out.message = Some(if n == 1 { tr("1 connection drawn automatically").to_string() } else { tr_fmt!("{} connections drawn automatically", n) });
                }
            }
        }
        for ev in events {
            match ev {
                egui::Event::Cut if !self.selection.is_empty() => {
                    let items = self.items(prj, page_i);
                    let page = &mut prj.pages[page_i];
                    let clip = ops::copy(page, &items);
                    ui.ctx().copy_text(clip.to_text());
                    self.clip = Some(clip);
                    ops::delete(page, &items);
                    self.selection.clear();
                }
                egui::Event::Paste(text) => {
                    if let Some(clip) = Clip::from_text(&text) {
                        if !clip.is_empty() {
                            let at = self.paste_point();
                            self.selection = ops::paste(prj, page_i, &clip, at);
                            self.set_tool(Tool::Select);
                            out.message = Some(if self.selection.len() == 1 { tr("1 object pasted").to_string() } else { tr_fmt!("{} objects pasted", self.selection.len()) });
                        }
                    }
                    return;
                }
                _ => {}
            }
        }
    }

    /// Ends a polyline / polygon (if it has enough points).
    fn finish_poly(&mut self, prj: &mut Project, page_i: usize) {
        let Some(kind) = self.tool.shape_kind().filter(|k| matches!(k, ShapeKind::Polyline | ShapeKind::Polygon)) else { return };
        let mut pts = std::mem::take(&mut self.pts);
        pts.dedup_by(|a, b| a.same(*b));
        let needed = if kind == ShapeKind::Polygon { 3 } else { 2 };
        if pts.len() >= needed {
            let id = prj.new_id();
            prj.pages[page_i].shapes.push(Shape { id, kind, pts, layer: self.active_layer.clone(), ..Default::default() });
        }
    }

    /// Adds the shape, dimension or definition of a graphic tool once all its points
    /// are clicked.
    fn complete(&mut self, prj: &mut Project, page_i: usize, out: &mut CanvasOutput) {
        let pts = std::mem::take(&mut self.pts);
        let layer = self.active_layer.clone();
        match self.tool.clone() {
            Tool::Dimension(kind) => {
                let mut dm = Dimension { kind, a: pts[0], b: pts[1], layer, ..Default::default() };
                dm.offset = graphics::dim_offset_through(&dm, pts[2]);
                if graphics::dim_length(&dm).abs() > 0.01 {
                    dm.id = prj.new_id();
                    prj.pages[page_i].dimensions.push(dm);
                }
            }
            Tool::StructureBox => {
                let r = R::from_points(pts[0], pts[1]);
                if r.width() > 0.0 && r.height() > 0.0 {
                    let b = structure_boxes::new_box(prj, r.min, r.max, Default::default(), "");
                    self.selection = BTreeSet::from([b.id]);
                    prj.pages[page_i].structure_boxes.push(b);
                    out.open_properties = true;
                }
            }
            Tool::CableDef => {
                let id = cabledef::add(prj, page_i, pts[0], pts[1], &self.cable_target);
                if let Some(c) = prj.pages[page_i].cable_defs.iter().find(|c| c.id == id) {
                    let n = prj.pages[page_i].wires.iter().filter(|w| w.cable == c.cable).count();
                    out.message = Some(tr_fmt!("Cable {}: {} cores assigned", c.cable, n));
                    self.cable_target = c.cable.clone();
                }
                self.selection = BTreeSet::from([id]);
                out.open_properties = true;
            }
            Tool::DeviceBox => match device_boxes::add_box(prj, page_i, pts[0], pts[1], "", "") {
                Ok(id) => {
                    self.selection = BTreeSet::from([id]);
                    out.open_properties = true;
                }
                Err(e) => out.message = Some(e),
            },
            tool => {
                let Some(kind) = tool.shape_kind() else { return };
                let (a, b) = (pts[0], pts[1]);
                let sweep_deg = if kind == ShapeKind::Arc { sweep(a, b, pts[2]) } else { 0.0 };
                if a.same(b) || (kind == ShapeKind::Arc && sweep_deg == 0.0) {
                    return;
                }
                let id = prj.new_id();
                prj.pages[page_i].shapes.push(Shape { id, kind, a, b, sweep: sweep_deg, layer, ..Default::default() });
            }
        }
    }

    /// Places the symbol `key` at `sn` (with the tag and label / connection points of
    /// an unplaced function when one is being placed).
    /// Places a part at `at` (a part dropped onto the page): the symbol of its main
    /// function with the connection point designations, as a new device; the output
    /// names the part to assign to it.
    pub fn place_part_at(&mut self, prj: &mut Project, page_i: usize, symbol: &'static str, pins: Vec<String>, number: &str, at: P) -> CanvasOutput {
        let mut out = CanvasOutput::default();
        self.set_tool(Tool::Place(symbol));
        self.place_rot = 0;
        self.place_mirror = false;
        self.place_tag = None;
        self.place_once = Some((String::new(), pins));
        self.place_part = Some(number.to_string());
        self.place_at(prj, page_i, symbol, at, &mut out);
        out
    }

    fn place_at(&mut self, prj: &mut Project, page_i: usize, key: &str, sn: P, out: &mut CanvasOutput) {
        let Some(def) = prj.symbol(key) else { return };
        let mut ctx = self.place_ctx.clone();
        ctx.plc_article.clone_from(&self.place_part);
        if let Some(tag) = &self.place_tag {
            ctx.last_main.insert(def.letter.to_string(), tag.clone());
            ctx.last_plc.insert(def.key.into(), tag.clone());
        }
        let rot = self.place_rotation(prj, &prj.pages[page_i], &def, sn);
        let mut s = ops::new_symbol(prj, page_i, &ctx, &def, sn, rot, self.place_mirror);
        if let (Some(tag), true) = (&self.place_tag, def.func.has_tag()) {
            if def.func == Func::Connection && s.tag != *tag {
                s.pins = vec![device_boxes::next_designation(prj, tag)];
            }
            s.tag = tag.clone();
        }
        let once = self.place_once.take();
        if let Some((label, pins)) = &once {
            if !label.is_empty() {
                s.label = label.clone();
            }
            if pins.len() == s.pins.len() {
                s.pins = pins.clone();
            }
        }
        self.place_ctx.remember(prj, &s);
        let id = s.id;
        prj.pages[page_i].symbols.push(s);
        out.placed = Some(id);
        out.placed_part = self.place_part.take();
        if self.autoconnect {
            let n = routing::apply_autoconnect(prj, page_i, &BTreeSet::from([id]));
            if n > 0 {
                out.message = Some(if n == 1 { tr("1 connection drawn automatically").to_string() } else { tr_fmt!("{} connections drawn automatically", n) });
            }
        }
        if once.is_some() {
            self.place_tag = None;
            self.set_tool(Tool::Select);
            self.selection.insert(id);
        } else if out.placed_part.is_none() && def.plc_channel.is_some() {
            if let Some(next) = prj.next_picker_symbol(key) {
                self.tool = Tool::Place(next);
            }
        }
    }

    /// Rotation of a symbol placed at `sn`: a device connection point on the outline of a
    /// device box leads away from its edge, everything else as turned by the user.
    /// Where the pointer at `w` snaps: to the grid or an invisible grid line. A symbol
    /// being `placing` (by the tool, or dragged in unturned) snaps from its connection
    /// points instead, so they land on such lines; a device connection point snaps
    /// itself, onto the outline of its box.
    fn snap_pointer(&self, prj: &Project, page: &Page, w: P, placing: Option<(&str, bool)>) -> P {
        let lines = snap_lines::SnapLines::of_page(prj, page, self.grid);
        let Some((def, by_tool)) = placing.and_then(|(key, by_tool)| Some((prj.symbol(key)?, by_tool))).filter(|(d, _)| d.func != Func::Connection) else {
            return lines.snap(w, self.grid);
        };
        let inst = match by_tool {
            true => SymbolInst { rot: self.place_rotation(prj, page, &def, w.snap(self.grid)), mirror: self.place_mirror, ..Default::default() },
            false => ops::symbol_preview(prj, &def),
        };
        match snap_lines::pins_at_origin(&def, &inst) {
            pins if pins.is_empty() => lines.snap(w, self.grid),
            pins => lines.snap_from(w, &pins, self.grid),
        }
    }

    fn place_rotation(&self, prj: &Project, page: &Page, def: &openecraft_core::symbols::SymbolDef, sn: P) -> u8 {
        match def.func {
            Func::Connection => device_boxes::direction_at(prj, page, sn).unwrap_or(self.place_rot),
            _ => self.place_rot,
        }
    }

    /// Places a connection symbol at `sn` and, with Auto-connect on, the wires to the
    /// connection points facing its arms; the message says what is still missing.
    fn place_junction(&mut self, prj: &mut Project, page_i: usize, sn: P, kind: JunctionKind, out: &mut CanvasOutput) {
        if prj.pages[page_i].junctions.iter().any(|j| j.pos.same(sn)) {
            out.message = Some(tr("There is a connection symbol at this point already").to_string());
            return;
        }
        let mut j = self.junction_at(&prj.pages[page_i], sn, kind);
        j.id = prj.new_id();
        let id = j.id;
        prj.pages[page_i].junctions.push(j);
        let drawn = if self.autoconnect { routing::apply_autoconnect(prj, page_i, &BTreeSet::from([id])) } else { 0 };
        let j = prj.pages[page_i].junctions.last().cloned().unwrap_or_default();
        out.message = Some(if drawn > 0 {
            if drawn == 1 { tr("1 connection drawn automatically").to_string() } else { tr_fmt!("{} connections drawn automatically", drawn) }
        } else if junctions::fits(prj, &prj.pages[page_i], &j) {
            String::new()
        } else {
            match kind {
                JunctionKind::Corner => tr("Draw the two wires of the corner to it, or switch on Auto-connect and place it facing free connection points").to_string(),
                JunctionKind::Tee => tr("Draw the three wires of the T-junction to it: one in its direction and two across").to_string(),
                JunctionKind::Crossing | JunctionKind::Bridge => tr("Draw wires in all four directions to it, or switch on Auto-connect and place it facing free connection points").to_string(),
            }
        })
        .filter(|m| !m.is_empty());
        self.selection = BTreeSet::from([id]);
        out.open_properties = true;
    }

    #[allow(clippy::too_many_arguments)]
    fn mouse(
        &mut self,
        ui: &egui::Ui,
        resp: &egui::Response,
        prj: &mut Project,
        page_i: usize,
        rect: Rect,
        world: Option<P>,
        snapped: Option<P>,
        an: &Analysis,
        idx: &Index,
        out: &mut CanvasOutput,
    ) {
        let shift = ui.input(|i| i.modifiers.shift);
        let secondary_click = resp.clicked_by(PointerButton::Secondary);
        if secondary_click {
            if matches!(self.tool, Tool::Polyline | Tool::Polygon) && !self.pts.is_empty() {
                self.finish_poly(prj, page_i);
            } else if self.wire_start.is_some() || !self.pts.is_empty() {
                self.wire_start = None;
                self.pts.clear();
            } else if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            }
        }
        // A part from the parts management or the data portal: the application imports
        // it when needed and places it.
        if let (Some(sn), Some(pl)) = (snapped, resp.dnd_release_payload::<PartDrag>()) {
            out.dropped_part = Some((pl.number.clone(), sn));
            return;
        }
        if let (Some(sn), Some(pl)) = (snapped, resp.dnd_release_payload::<PlacePayload>()) {
            self.set_tool(Tool::Place(pl.symbol));
            self.place_rot = 0;
            self.place_mirror = false;
            // A symbol without a device (Insert center) gets its tag as when placed by click.
            self.place_tag = (!pl.tag.is_empty()).then(|| pl.tag.clone());
            self.place_once = Some((pl.label.clone(), pl.pins.clone()));
            self.place_at(prj, page_i, pl.symbol, sn, out);
            return;
        }
        let click = resp.clicked_by(PointerButton::Primary).then_some(snapped).flatten().or_else(|| self.typed.take());
        if click.is_some() {
            self.last_point = click;
        }
        match self.tool.clone() {
            Tool::Select => {
                if let Some(q) = click.filter(|_| !resp.clicked_by(PointerButton::Primary)) {
                    self.select_at(prj, page_i, q, shift);
                }
                self.select_mouse(ui, resp, prj, idx, page_i, rect, world, true, out);
            }
            Tool::Insert => {
                if let (Some(sn), Some(clip)) = (click, click.and_then(|_| self.insert.take())) {
                    let ids = ops::paste(prj, page_i, &clip, sn);
                    if let Some((m, values)) = self.insert_macro.take() {
                        openecraft_core::macro_boxes::record(prj, page_i, &m, &values, &ids);
                    }
                    self.set_tool(Tool::Select);
                    self.selection = ids;
                }
            }
            Tool::Place(key) => {
                if let Some(sn) = click {
                    self.place_at(prj, page_i, key, sn, out);
                }
            }
            Tool::Wire => {
                let finish = resp.double_clicked();
                if let Some(sn) = click {
                    match self.wire_start {
                        None => self.wire_start = Some(sn),
                        Some(a) if a.same(sn) => self.wire_start = None,
                        Some(a) => {
                            let ends_on_connection = Self::is_connection(&prj.symbol_table(), &prj.pages[page_i], sn);
                            let path = self.wire_path(prj, &prj.pages[page_i], a, sn);
                            for seg in path.windows(2) {
                                let id = prj.new_id();
                                prj.pages[page_i].wires.push(Wire { id, a: seg[0], b: seg[1], ..Default::default() });
                            }
                            ops::cleanup_wires(&mut prj.pages[page_i]);
                            self.route_cache = None;
                            self.wire_start = if ends_on_connection { None } else { Some(sn) };
                        }
                    }
                }
                if finish {
                    self.wire_start = None;
                }
            }
            Tool::Text => {
                if let Some(sn) = click {
                    let id = prj.new_id();
                    let mut t = self.text_proto.clone().unwrap_or_else(|| TextItem { text: "Text".into(), ..Default::default() });
                    t.id = id;
                    t.pos = sn;
                    t.layer = self.active_layer.clone();
                    prj.pages[page_i].texts.push(t);
                    // The tool stays for the next text; the properties show this one.
                    self.selection = BTreeSet::from([id]);
                    out.open_properties = true;
                }
            }
            Tool::ConnDef => {
                let target = match resp.clicked_by(PointerButton::Primary) {
                    true => world.zip(snapped),
                    false => click.map(|q| (q, q)),
                };
                if let Some((w, sn)) = target {
                    match self.wire_point(&prj.pages[page_i], w, sn) {
                        Some(q) => {
                            let mut cd = conndef::adopt(prj, &an.open, page_i, q);
                            cd.id = prj.new_id();
                            let id = cd.id;
                            prj.pages[page_i].conn_defs.push(cd);
                            self.selection = BTreeSet::from([id]);
                            out.open_properties = true;
                        }
                        None => out.message = Some(tr("Click on a wire to place the connection definition point").to_string()),
                    }
                }
            }
            Tool::Polyline | Tool::Polygon => {
                if let Some(sn) = click {
                    if !self.pts.last().is_some_and(|l| l.same(sn)) {
                        self.pts.push(sn);
                    }
                }
                if resp.double_clicked() {
                    self.finish_poly(prj, page_i);
                }
            }
            Tool::Junction(kind) => {
                if let Some(sn) = click {
                    self.place_junction(prj, page_i, sn, kind, out);
                }
            }
            tool => {
                let Some(needed) = tool.points() else { return };
                if let Some(sn) = click {
                    if self.pts.last().is_some_and(|l| l.same(sn)) && tool != Tool::Arc {
                        return;
                    }
                    self.pts.push(sn);
                    if self.pts.len() >= needed {
                        self.complete(prj, page_i, out);
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Select tool: clicks select, drags move the selection or the texts of a single
    /// selected symbol, drags on empty space select by window. On a page that is not
    /// `editable` only the selection changes.
    fn select_mouse(&mut self, ui: &egui::Ui, resp: &egui::Response, prj: &mut Project, idx: &Index, page_i: usize, rect: Rect, world: Option<P>, editable: bool, out: &mut CanvasOutput) {
        let shift = ui.input(|i| i.modifiers.shift);
        if resp.drag_started_by(PointerButton::Primary) {
            let origin = ui.input(|i| i.pointer.press_origin()).unwrap_or(rect.center());
            let ow = self.s2w(rect, origin);
            let handle = self.prop_hit(prj, idx, page_i, ow).filter(|_| editable);
            // The texts of a single selected symbol are placed on their own; with several
            // selected, dragging a text moves them all.
            let group = handle.filter(|_| self.selection.len() > 1).map(PropHandle::id);
            let hit = self.hit(prj, page_i, ow).or(group).filter(|_| editable);
            if let Some(handle) = handle.filter(|_| self.selection.len() == 1) {
                let page = &prj.pages[page_i];
                let orig = match handle {
                    PropHandle::Symbol(id, h) => match (page.symbols.iter().find(|s| s.id == id), h) {
                        (Some(s), Handle::Prop(i)) => s.props[i].offset,
                        (Some(s), Handle::Tag) => s.tag_offset.unwrap_or_else(|| {
                            prj.symbol(&s.sym).and_then(|def| render::tag_block_bounds(prj, idx, page_i, &def, s)).map(|r| r.min - s.pos).unwrap_or_default()
                        }),
                        (Some(s), Handle::Pin(i)) => pin_place(prj, idx, page_i, s, i).map(|l| l.shift).unwrap_or_default(),
                        (Some(s), Handle::OnlyPin) => pin_place(prj, idx, page_i, s, 0).map(|l| l.pos - s.pos).unwrap_or_default(),
                        (Some(s), Handle::Intr(t)) => prj.symbol(&s.sym).map(|def| intr_place(&def, s, t).offset).unwrap_or_default(),
                        _ => P::default(),
                    },
                    PropHandle::Wire(id, which) => page.wires.iter().find(|w| w.id == id).and_then(|w| which.offset(w)).unwrap_or_default(),
                };
                // A designation placed automatically keeps the place it has now.
                let side = match handle {
                    PropHandle::Symbol(id, Handle::Pin(i)) => page.symbols.iter().find(|s| s.id == id).and_then(|s| pin_place(prj, idx, page_i, s, i)).map(|l| l.side),
                    _ => None,
                };
                self.drag = Drag::Prop { handle, start: ow, orig, side };
            } else if let Some(id) = hit {
                if !self.selection.contains(&id) {
                    if !shift {
                        self.selection.clear();
                    }
                    self.selection.insert(id);
                }
                let items = self.items(prj, page_i);
                let moving = routing::Moving::new(prj, page_i, &items);
                let page = &prj.pages[page_i];
                let points = snap_lines::points_of(prj, page, &items);
                let lines = snap_lines::SnapLines::without(prj, page, self.grid, &points);
                self.drag = Drag::Move { start: ow, applied: p(0.0, 0.0), moving, items, points, lines };
            } else {
                self.drag = Drag::Box { start: origin };
            }
        }
        if resp.dragged_by(PointerButton::Primary) {
            match (&mut self.drag, world) {
                (Drag::Move { start, applied, moving, items, points, lines }, Some(w)) => {
                    let total = lines.snap_from(w - *start, points, self.grid);
                    let delta = total - *applied;
                    if delta.x != 0.0 || delta.y != 0.0 {
                        moving.step(prj, page_i, items, delta);
                        *applied = total;
                    }
                }
                (Drag::Prop { handle, start, orig, side }, Some(w)) => {
                    let delta = (w - *start).snap(0.5);
                    let to = *orig + delta;
                    let symbols = prj.symbol_table();
                    let anchor = match *handle {
                        PropHandle::Symbol(id, Handle::OnlyPin) => prj.pages[page_i].symbols.iter().find(|s| s.id == id).and_then(|s| pin_place(prj, idx, page_i, s, 0)).map(|l| l.anchor),
                        _ => None,
                    };
                    let page = &mut prj.pages[page_i];
                    match *handle {
                        PropHandle::Symbol(id, part) => {
                            if let Some(s) = page.symbols.iter_mut().find(|s| s.id == id) {
                                match part {
                                    Handle::Prop(i) => s.props[i].offset = to,
                                    Handle::Tag => s.tag_offset = Some(to),
                                    Handle::Pin(i) => {
                                        let offset = *orig + delta.untransform(s.rot, s.mirror);
                                        s.set_pin_placement(i, PinPlacement { side: *side, offset });
                                    }
                                    Handle::OnlyPin => s.pin_label = Some(PinLabel { offset: to, anchor: anchor.unwrap_or(Anchor::Left) }),
                                    Handle::Intr(t) => {
                                        if let Some(def) = symbols.get(&s.sym) {
                                            let place = TextPlace { offset: to, ..intr_place(&def, s, t) };
                                            match t {
                                                render::IntrText::Label => s.label_place = Some(place),
                                                render::IntrText::Xref => s.xref_place = Some(place),
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        PropHandle::Wire(id, which) => {
                            if let Some(w) = page.wires.iter_mut().find(|w| w.id == id) {
                                *which.offset_mut(w) = Some(to);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if resp.drag_stopped() {
            match std::mem::replace(&mut self.drag, Drag::None) {
                Drag::Box { start } => {
                    if let Some(end) = resp.interact_pointer_pos().or(ui.input(|i| i.pointer.hover_pos())) {
                        let a = self.s2w(rect, start);
                        let b = self.s2w(rect, end);
                        let sel = R::from_points(a, b);
                        let crossing = end.x < start.x;
                        if !shift {
                            self.selection.clear();
                        }
                        let symbols = prj.symbol_table();
                        let page = &prj.pages[page_i];
                        for id in ops::all_ids(page) {
                            let usable = graphics::item_layer(page, id).is_none_or(|l| graphics::shown(prj, l, false) && !graphics::locked(prj, l));
                            // A crossing window inside a structure box touches its devices, not the box.
                            let within_box = crossing && page.structure_boxes.iter().any(|b| b.id == id && b.rect().contains_rect(&sel));
                            if usable && !within_box && ops::item_bounds(&symbols, page, id).is_some_and(|bb| box_selects(sel, crossing, bb)) {
                                self.selection.insert(id);
                            }
                        }
                    }
                }
                Drag::Move { applied, .. } => {
                    ops::cleanup_wires(&mut prj.pages[page_i]);
                    let pg = &prj.pages[page_i];
                    let moved_symbols = pg.symbols.iter().any(|s| self.selection.contains(&s.id)) || pg.junctions.iter().any(|j| self.selection.contains(&j.id));
                    if self.autoconnect && moved_symbols && (applied.x != 0.0 || applied.y != 0.0) {
                        let n = routing::apply_autoconnect(prj, page_i, &self.selection);
                        if n > 0 {
                            out.message = Some(if n == 1 { tr("1 connection drawn automatically").to_string() } else { tr_fmt!("{} connections drawn automatically", n) });
                        }
                    }
                }
                Drag::Prop { .. } | Drag::None => {}
            }
        }
        if resp.clicked_by(PointerButton::Primary) {
            if let Some(w) = world {
                let hit = self.hit(prj, page_i, w);
                let ctrl = ui.input(|i| i.modifiers.ctrl || i.modifiers.command);
                if let (true, Some(link)) = (ctrl, hit.and_then(|id| prj.pages[page_i].texts.iter().find(|t| t.id == id)).map(|t| t.link.trim().to_string()).filter(|l| !l.is_empty())) {
                    if openecraft_parts::api::is_web_link(&link) {
                        out.open_url = Some(link);
                    } else {
                        out.message = Some(tr("Only web and e-mail addresses (http, https, mailto) are links").to_string());
                    }
                    return;
                }
                self.select_at(prj, page_i, w, shift);
            }
        }
        if resp.double_clicked() && !self.selection.is_empty() {
            out.open_properties = true;
        }
    }
}

impl Editor {
    /// Faint highlight of the item under the pointer (select tool).
    #[allow(clippy::too_many_arguments)]
    fn draw_hover(&self, painter: &Painter, rect: Rect, pal: &Palette, prj: &Project, page_i: usize, w: P, an: &Analysis) {
        let Some(id) = self.hit(prj, page_i, w).filter(|id| !self.selection.contains(id)) else { return };
        let page = &prj.pages[page_i];
        let color = pal.selected.gamma_multiply(0.35);
        if let Some(wire) = page.wires.iter().find(|x| x.id == id) {
            // The whole potential of a wire, as the selection shows it.
            let ids: HashSet<u64> = an.netlist.net_of_wire(id).map(|n| n.wires.iter().map(|x| x.1).collect()).unwrap_or_else(|| HashSet::from([wire.id]));
            for x in page.wires.iter().filter(|x| ids.contains(&x.id)) {
                painter.line_segment([self.w2s(rect, x.a), self.w2s(rect, x.b)], Stroke::new(4.0, color));
            }
            return;
        }
        if let Some(b) = ops::item_bounds(&prj.symbol_table(), page, id) {
            let r = Rect::from_min_max(self.w2s(rect, b.min), self.w2s(rect, b.max)).expand(2.0);
            painter.rect_filled(r, 2.0, color.gamma_multiply(0.3));
            painter.rect_stroke(r, 2.0, Stroke::new(1.0, color), egui::StrokeKind::Outside);
        }
    }

    /// Pointer shape over the page: move or point over an item, grab over the texts
    /// of a selected symbol, crosshair for the drawing tools.
    #[allow(clippy::too_many_arguments)]
    fn cursor(&self, ui: &egui::Ui, resp: &egui::Response, prj: &Project, idx: &Index, page_i: usize, world: Option<P>, editable: bool) {
        let Some(w) = world.filter(|_| resp.hovered() || resp.dragged()) else { return };
        let icon = match (&self.tool, &self.drag) {
            (_, Drag::Prop { .. }) => egui::CursorIcon::Grabbing,
            (_, Drag::Move { .. }) => egui::CursorIcon::Move,
            (_, Drag::Box { .. }) => egui::CursorIcon::Crosshair,
            (Tool::Select, Drag::None) if editable && self.selection.len() == 1 && self.prop_hit(prj, idx, page_i, w).is_some() => egui::CursorIcon::Grab,
            (Tool::Select, Drag::None) if editable && self.prop_hit(prj, idx, page_i, w).is_some() => egui::CursorIcon::Move,
            (Tool::Select, Drag::None) => match self.hit(prj, page_i, w) {
                Some(_) if editable => egui::CursorIcon::Move,
                Some(_) => egui::CursorIcon::PointingHand,
                None => return,
            },
            _ if editable || self.tool.is_review() => egui::CursorIcon::Crosshair,
            _ => egui::CursorIcon::NotAllowed,
        };
        ui.ctx().set_cursor_icon(icon);
    }

    /// A click of the select tool at `q`: selects the item there (Shift toggles it).
    fn select_at(&mut self, prj: &Project, page_i: usize, q: P, shift: bool) -> Option<u64> {
        let hit = self.hit(prj, page_i, q);
        match hit {
            Some(id) => {
                if shift {
                    if !self.selection.remove(&id) {
                        self.selection.insert(id);
                    }
                } else {
                    self.selection.clear();
                    self.selection.insert(id);
                }
                if let Some(s) = prj.pages[page_i].symbols.iter().find(|s| s.id == id) {
                    self.place_ctx.remember(prj, s);
                }
            }
            None if !shift => self.selection.clear(),
            None => {}
        }
        hit
    }

    /// Comment and markup tools: two clicks for lines and rectangles, one otherwise;
    /// a typed point acts as a click.
    fn review_mouse(&mut self, resp: &egui::Response, world: Option<P>, out: &mut CanvasOutput) {
        let click = resp.clicked_by(PointerButton::Primary).then_some(world).flatten().or_else(|| self.typed.take());
        let Some(w) = click else { return };
        match self.tool {
            Tool::Comment => out.review = Some(ReviewInput::Comment(w)),
            Tool::Markup(MarkupKind::Text, pen) => out.review = Some(ReviewInput::Markup(MarkupKind::Text, pen, w, w)),
            Tool::Markup(kind, pen) => match self.pts.pop() {
                None => self.pts.push(w),
                Some(a) if a.dist(w) > 0.5 => out.review = Some(ReviewInput::Markup(kind, pen, a, w)),
                // A second click on the start point keeps it.
                Some(a) => self.pts.push(a),
            },
            _ => {}
        }
    }
}

/// Draws a symbol preview into a small rectangle (symbol library).
pub fn symbol_icon(ui: &mut egui::Ui, prj: &Project, key: &str, size: Vec2) -> egui::Response {
    let (resp, painter) = ui.allocate_painter(size, Sense::click());
    if !ui.is_rect_visible(resp.rect) {
        return resp;
    }
    let Some(def) = prj.symbol(key) else { return resp };
    let pal = Palette::new(ui.visuals().dark_mode);
    let b = def.bounds;
    let zoom = ((size.x - 6.0) / b.width()).min((size.y - 6.0) / b.height()).min(3.2);
    let center = resp.rect.center();
    let origin = center - vec2(b.center().x, b.center().y) * zoom;
    let mut d = EguiDraw { painter: &painter, origin, zoom, pal: &pal, force: None, mirror: None, textures: None };
    let inst = ops::symbol_preview(prj, &def);
    render::draw_symbol(&mut d, &def, &inst, Ink::Line);
    resp
}

/// A press in a canvas takes the keys back from a field, list or button used before
/// (their focus would keep R, Del and Enter away from the canvas).
pub fn take_keys(ui: &egui::Ui, resp: &egui::Response) {
    if resp.contains_pointer() && ui.input(|i| i.pointer.any_pressed()) {
        ui.memory_mut(|m| m.stop_text_input());
    }
}


/// Current place of the name or the cross reference of an interruption point.
fn intr_place(def: &SymbolDef, s: &SymbolInst, t: render::IntrText) -> TextPlace {
    let (label, xref) = render::interruption_places(def, s);
    if t == render::IntrText::Label { label } else { xref }
}

#[cfg(test)]
mod tests;
