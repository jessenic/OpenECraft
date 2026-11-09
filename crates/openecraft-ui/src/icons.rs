//! Vector icons of the ribbon, the panes and the menus, drawn with the painter instead of
//! glyphs of a font: they scale with the UI, follow the text colour and need no
//! characters in the bundled fonts. Each icon is a short path on a 16 × 16 grid:
//!
//! - `M x y x y …` open polyline, `P x y x y …` closed outline, `F x y x y …` filled polygon
//! - `C cx cy r` circle, `D cx cy r` filled disc, `E cx cy rx ry` ellipse
//! - `R x y w h` rectangle, `A cx cy r a0 a1` arc, `B cx cy rx ry a0 a1` elliptic arc
//!   (degrees, 0 = right, increasing clockwise on screen)
//!
//! Shapes are separated by `;`.

use egui::{pos2, vec2, Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, WidgetInfo, Role};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    File,
    FileNew,
    FolderOpen,
    Save,
    SaveAs,
    Download,
    Upload,
    Print,
    Template,
    Info,
    Settings,
    Help,
    Quit,
    Home,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Delete,
    Rotate,
    Mirror,
    Align,
    Distribute,
    Move,
    MultiCopy,
    Find,
    Select,
    SelectAll,
    PageProperties,
    Renumber,
    Previous,
    Next,
    Report,
    Table,
    Symbol,
    Device,
    InsertCenter,
    Wire,
    ConnDef,
    Junction,
    Corner,
    Crossing,
    Bridge,
    CableDef,
    Interruption,
    Potential,
    Macro,
    Text,
    PathText,
    SpecialText,
    Line,
    Rect,
    Circle,
    Ellipse,
    Arc,
    Polyline,
    Polygon,
    Image,
    Dimension,
    Comment,
    Markup,
    AutoConnect,
    Route,
    Join,
    Layer,
    Properties,
    Fit,
    Grid,
    Pins,
    RevisionMarks,
    MacroBoxes,
    WireColours,
    Panes,
    Workspace,
    Theme,
    Scale,
    Language,
    Numbering,
    Terminal,
    Plug,
    Plc,
    Parts,
    Database,
    Server,
    Vault,
    Structure,
    Preplanning,
    SingleLine,
    Cable,
    Harness,
    Field,
    Marking,
    CrossRef,
    Check,
    Warning,
    Error,
    Clock,
    Compare,
    ChangeRequest,
    Languages,
    Configurator,
    Management,
    Calculator,
    Fluid,
    Integration,
    Plugin,
    SymbolEditor,
    Forms,
    Enclosure,
    Rail,
    Duct,
    Busbar,
    Cutout,
    RoutingPath,
    Reservation,
    Cube,
    Front,
    Door,
    Wall,
    Accessory,
    Drill,
    Wiring,
    Mounting,
    Tablet,
    Star,
    StarFilled,
    Pin,
    Close,
    Filter,
    Tree,
    List,
    Lock,
    Unlock,
    Plus,
    ChevronDown,
    ChevronRight,
    ChevronUp,
    ChevronLeft,
    Float,
    Dot,
    Dots,
    Keyboard,
    Book,
    Unplaced,
    Heat,
    Eye,
    ZoomIn,
    ZoomOut,
    Building,
    DeviceBox,
    DeviceConnection,
}

impl Icon {
    /// Every icon, for the tests.
    pub const ALL: [Icon; 152] = [
        Icon::File, Icon::FileNew, Icon::FolderOpen, Icon::Save, Icon::SaveAs, Icon::Download, Icon::Upload, Icon::Print, Icon::Template,
        Icon::Info, Icon::Settings, Icon::Help, Icon::Quit, Icon::Home, Icon::Undo, Icon::Redo, Icon::Cut, Icon::Copy, Icon::Paste,
        Icon::Delete, Icon::Rotate, Icon::Mirror, Icon::Align, Icon::Distribute, Icon::Move, Icon::MultiCopy, Icon::Find, Icon::Select,
        Icon::SelectAll, Icon::PageProperties, Icon::Renumber, Icon::Previous, Icon::Next, Icon::Report, Icon::Table, Icon::Symbol,
        Icon::Device, Icon::InsertCenter, Icon::Wire, Icon::ConnDef, Icon::Junction, Icon::Corner, Icon::Crossing, Icon::Bridge, Icon::CableDef, Icon::Interruption, Icon::Potential, Icon::Macro,
        Icon::Text, Icon::PathText, Icon::SpecialText, Icon::Line, Icon::Rect, Icon::Circle, Icon::Ellipse, Icon::Arc, Icon::Polyline,
        Icon::Polygon, Icon::Image, Icon::Dimension, Icon::Comment, Icon::Markup, Icon::AutoConnect, Icon::Route, Icon::Join, Icon::Layer,
        Icon::Properties, Icon::Fit, Icon::Grid, Icon::Pins, Icon::RevisionMarks, Icon::MacroBoxes, Icon::WireColours, Icon::Panes,
        Icon::Workspace, Icon::Theme, Icon::Scale, Icon::Language, Icon::Numbering, Icon::Terminal, Icon::Plug, Icon::Plc, Icon::Parts,
        Icon::Database, Icon::Server, Icon::Vault, Icon::Structure, Icon::Preplanning, Icon::SingleLine, Icon::Cable, Icon::Harness,
        Icon::Field, Icon::Marking, Icon::CrossRef, Icon::Check, Icon::Warning, Icon::Error, Icon::Clock, Icon::Compare,
        Icon::ChangeRequest, Icon::Languages, Icon::Configurator, Icon::Management, Icon::Calculator, Icon::Fluid, Icon::Integration,
        Icon::Plugin, Icon::SymbolEditor, Icon::Forms, Icon::Enclosure, Icon::Rail, Icon::Duct, Icon::Busbar, Icon::Cutout,
        Icon::RoutingPath, Icon::Reservation, Icon::Cube, Icon::Front, Icon::Door, Icon::Wall, Icon::Accessory, Icon::Drill, Icon::Wiring,
        Icon::Mounting, Icon::Tablet, Icon::Star, Icon::StarFilled, Icon::Pin, Icon::Close, Icon::Filter, Icon::Tree, Icon::List,
        Icon::Lock, Icon::Unlock, Icon::Plus, Icon::ChevronDown, Icon::ChevronRight, Icon::ChevronUp, Icon::ChevronLeft, Icon::Float,
        Icon::Dot, Icon::Dots, Icon::Keyboard, Icon::Book, Icon::Unplaced, Icon::Heat, Icon::Eye, Icon::ZoomIn,
        Icon::ZoomOut,
        Icon::Building,
        Icon::DeviceBox,
        Icon::DeviceConnection,
    ];

    /// The path of the icon (see the module documentation).
    pub fn path(self) -> &'static str {
        match self {
            Icon::File => "P 4 1.5 10 1.5 13 4.5 13 14.5 4 14.5 ; M 10 1.5 10 4.5 13 4.5",
            Icon::FileNew => "P 3 1.5 9 1.5 12 4.5 12 9 ; M 12 9 12 14.5 3 14.5 3 1.5 ; M 9 1.5 9 4.5 12 4.5 ; M 7.5 7 7.5 12 ; M 5 9.5 10 9.5",
            Icon::FolderOpen => "P 1.5 3 6 3 7.5 4.5 14.5 4.5 14.5 13.5 1.5 13.5 ; M 1.5 7 14.5 7",
            Icon::Save => "P 2 2 12 2 14 4 14 14 2 14 ; R 5 2 6 4 ; R 4.5 9 7 5",
            Icon::SaveAs => "M 9 14 2 14 2 2 11 2 13 4 13 7 ; R 4.5 2 5 3.5 ; M 4.5 14 4.5 9 9 9 ; P 9.5 15 10 12.5 14 8.5 15.5 10 11.5 14",
            Icon::Download => "M 8 2 8 11 ; M 4.5 7.5 8 11 11.5 7.5 ; M 2.5 14 13.5 14",
            Icon::Upload => "M 8 11 8 2 ; M 4.5 5.5 8 2 11.5 5.5 ; M 2.5 14 13.5 14",
            Icon::Print => "R 4 1.5 8 4.5 ; R 2 6 12 6 ; R 4.5 10 7 4.5",
            Icon::Template => "R 2 2 12 12 ; M 2 5.5 14 5.5 ; M 6 5.5 6 14",
            Icon::Info => "C 8 8 6.5 ; M 8 7 8 11.5 ; D 8 4.8 0.9",
            Icon::Settings => "C 8 8 2.3 ; C 8 8 4.8 ; M 8 1.5 8 3.2 ; M 8 12.8 8 14.5 ; M 1.5 8 3.2 8 ; M 12.8 8 14.5 8 ; M 3.4 3.4 4.6 4.6 ; M 11.4 11.4 12.6 12.6 ; M 3.4 12.6 4.6 11.4 ; M 11.4 4.6 12.6 3.4",
            Icon::Help => "C 8 8 6.5 ; M 6 6.2 6 5.2 7 4.2 9 4.2 10 5.2 10 6.5 8 8 8 9.5 ; D 8 11.8 0.9",
            Icon::Quit => "M 9 2 3 2 3 14 9 14 ; M 7 8 14.5 8 ; M 12 5.5 14.5 8 12 10.5",
            Icon::Home => "M 1.5 8 8 2 14.5 8 ; M 3.5 6.5 3.5 14 12.5 14 12.5 6.5 ; R 6.5 9.5 3 4.5",
            Icon::Undo => "M 3.5 7 10 7 12 8 13 10 13 13 ; M 6.5 4 3.5 7 6.5 10",
            Icon::Redo => "M 12.5 7 6 7 4 8 3 10 3 13 ; M 9.5 4 12.5 7 9.5 10",
            Icon::Cut => "C 4.5 12 2 ; C 11.5 12 2 ; M 5.8 10.4 11.5 2 ; M 10.2 10.4 4.5 2",
            Icon::Copy => "R 5 5 9 9 ; M 2 11 2 2 11 2",
            Icon::Paste => "R 3 3 10 11.5 ; R 5.5 1.5 5 3",
            Icon::Delete => "M 2.5 4 13.5 4 ; M 6 4 6 2 10 2 10 4 ; P 4 4 12 4 11 14.5 5 14.5 ; M 7 7 7 12 ; M 9 7 9 12",
            Icon::Rotate => "A 8 8 5.5 -60 200 ; M 10.6 0.8 10.75 3.24 13.2 3.6",
            Icon::Mirror => "M 8 1.5 8 3.5 ; M 8 5.5 8 7.5 ; M 8 9.5 8 11.5 ; M 8 13 8 14.5 ; P 6 4 2 12 6 12 ; P 10 4 14 12 10 12",
            Icon::Align => "M 2 2 2 14 ; R 4 3.5 9 3 ; R 4 9.5 6 3",
            Icon::Distribute => "M 2 2 14 2 ; M 2 14 14 14 ; R 3 5.5 3 5 ; R 10 5.5 3 5",
            Icon::Move => "M 8 1.5 8 14.5 ; M 1.5 8 14.5 8 ; M 6 3.5 8 1.5 10 3.5 ; M 6 12.5 8 14.5 10 12.5 ; M 3.5 6 1.5 8 3.5 10 ; M 12.5 6 14.5 8 12.5 10",
            Icon::MultiCopy => "R 1.5 1.5 6 6 ; R 5 5 6 6 ; R 8.5 8.5 6 6",
            Icon::Find => "C 6.5 6.5 4.5 ; M 10 10 14.5 14.5",
            Icon::Select => "F 3 1.5 3 13 6 10 8.5 14.5 10.5 13.5 8 9.5 12 9.5",
            Icon::SelectAll => "M 1.5 4 1.5 1.5 4 1.5 ; M 12 1.5 14.5 1.5 14.5 4 ; M 14.5 12 14.5 14.5 12 14.5 ; M 4 14.5 1.5 14.5 1.5 12 ; M 6.5 1.5 9.5 1.5 ; M 6.5 14.5 9.5 14.5 ; M 1.5 6.5 1.5 9.5 ; M 14.5 6.5 14.5 9.5 ; F 5 5 11 5 11 11 5 11",
            Icon::PageProperties => "P 3 1.5 9 1.5 12 4.5 12 14.5 3 14.5 ; M 9 1.5 9 4.5 12 4.5 ; M 5 7.5 10 7.5 ; M 5 10 10 10 ; M 5 12.5 8 12.5",
            Icon::Renumber => "M 1.5 3.5 3 2 3 7.5 ; M 5.5 2.5 6.5 2 8 2 8.5 3 5.5 7.5 8.5 7.5 ; M 10.5 2 13 2 11.5 4.5 13 5.5 12 7.5 10.5 7 ; M 1.5 10.5 14.5 10.5 ; M 1.5 13.5 14.5 13.5",
            Icon::Previous => "M 10 3 5 8 10 13",
            Icon::Next => "M 6 3 11 8 6 13",
            Icon::Report => "R 2.5 1.5 11 13 ; M 4.5 5 11.5 5 ; M 4.5 8 11.5 8 ; M 4.5 11 9 11",
            Icon::Table => "R 1.5 2.5 13 11 ; M 1.5 6 14.5 6 ; M 1.5 9.5 14.5 9.5 ; M 6 2.5 6 13.5",
            Icon::Symbol => "R 4.5 4.5 7 7 ; M 8 1 8 4.5 ; M 8 11.5 8 15",
            Icon::Device => "R 3 1.5 10 13 ; M 5 4.5 11 4.5 ; M 5 7.5 11 7.5 ; D 8 11.5 1.1",
            Icon::InsertCenter => "R 1.5 1.5 5 5 ; R 9.5 1.5 5 5 ; R 1.5 9.5 5 5 ; M 12 9.5 12 14.5 ; M 9.5 12 14.5 12",
            Icon::Wire => "M 2 13 2 8 14 8 14 3 ; D 2 13 1.3 ; D 14 3 1.3",
            Icon::ConnDef => "M 1.5 8 14.5 8 ; P 8 4.5 11.5 8 8 11.5 4.5 8",
            Icon::Junction => "M 1.5 5 14.5 5 ; M 8 5 8 14.5 ; M 11.5 5 8 8.5",
            Icon::Corner => "M 4 1.5 4 12 14.5 12",
            Icon::Crossing => "M 8 1.5 8 14.5 ; M 1.5 8 14.5 8 ; M 8 4.5 4.5 8 ; M 8 11.5 11.5 8",
            Icon::Bridge => "M 8 1.5 8 14.5 ; M 1.5 8 14.5 8 ; M 8 4.5 4.5 8 ; M 8 4.5 11.5 8",
            Icon::CableDef => "M 4 2 4 14 ; M 8 2 8 14 ; M 12 2 12 14 ; M 1.5 12.5 14.5 3.5",
            Icon::Interruption => "M 1.5 8 9 8 ; F 9 4.5 14.5 8 9 11.5",
            Icon::Potential => "M 2 4 14 4 ; M 2 8 14 8 ; M 2 12 14 12 ; D 5 4 1 ; D 8 8 1 ; D 11 12 1",
            Icon::Macro => "R 1.5 4.5 9 9 ; M 4.5 4.5 4.5 1.5 14.5 1.5 14.5 11.5 10.5 11.5",
            Icon::Text => "M 3 3 13 3 ; M 8 3 8 14 ; M 3 3 3 5 ; M 13 3 13 5 ; M 6 14 10 14",
            Icon::PathText => "M 2 3 9 3 ; M 5.5 3 5.5 11 ; M 11.5 5 11.5 14.5 ; M 9.5 12.5 11.5 14.5 13.5 12.5",
            Icon::SpecialText => "M 6 2 4.5 2 4 3 4 7 2.5 8 4 9 4 13 4.5 14 6 14 ; M 10 2 11.5 2 12 3 12 7 13.5 8 12 9 12 13 11.5 14 10 14",
            Icon::Line => "M 2.5 13.5 13.5 2.5",
            Icon::Rect => "R 2 3.5 12 9",
            Icon::Circle => "C 8 8 6",
            Icon::Ellipse => "E 8 8 6.5 4",
            Icon::Arc => "A 8 11 6 180 360",
            Icon::Polyline => "M 2 13 5 4 10 11 14 3",
            Icon::Polygon => "P 3 13 2 6 8 2 14 6 12 13",
            Icon::Image => "R 1.5 2.5 13 11 ; M 1.5 12 6 7 9 10 11 8 14.5 11.5 ; C 11 5.5 1.3",
            Icon::Dimension => "M 2 4 2 12 ; M 14 4 14 12 ; M 2 8 14 8 ; M 4.5 6 2 8 4.5 10 ; M 11.5 6 14 8 11.5 10",
            Icon::Comment => "P 2 2.5 14 2.5 14 11 8 11 4.5 14 4.5 11 2 11",
            Icon::Markup => "P 2.5 13.5 3.5 10.5 11.5 2.5 13.5 4.5 5.5 12.5 ; M 10 4 12 6",
            Icon::AutoConnect => "C 3 8 1.5 ; C 13 8 1.5 ; M 4.5 8 11.5 8 ; M 8 2 8 5 ; M 8 11 8 14",
            Icon::Route => "M 2 14 2 3 7 3 7 13 14 13 14 2",
            Icon::Join => "M 1.5 8 6 8 ; M 10 8 14.5 8 ; D 8 8 1.6",
            Icon::Layer => "P 8 2 14.5 5.5 8 9 1.5 5.5 ; M 1.5 8.5 8 12 14.5 8.5 ; M 1.5 11.5 8 15 14.5 11.5",
            Icon::Properties => "R 2 2 12 12 ; M 4.5 5.5 6 5.5 ; M 7.5 5.5 11.5 5.5 ; M 4.5 8 6 8 ; M 7.5 8 11.5 8 ; M 4.5 10.5 6 10.5 ; M 7.5 10.5 11.5 10.5",
            Icon::Fit => "M 2 6 2 2 6 2 ; M 10 2 14 2 14 6 ; M 14 10 14 14 10 14 ; M 6 14 2 14 2 10 ; R 5.5 5.5 5 5",
            Icon::Grid => "D 3 3 0.9 ; D 8 3 0.9 ; D 13 3 0.9 ; D 3 8 0.9 ; D 8 8 0.9 ; D 13 8 0.9 ; D 3 13 0.9 ; D 8 13 0.9 ; D 13 13 0.9",
            Icon::Pins => "R 5 5 6 6 ; D 8 2.5 1.1 ; D 8 13.5 1.1 ; M 8 3.5 8 5 ; M 8 11 8 12.5",
            Icon::RevisionMarks => "P 8 2.5 14 13.5 2 13.5 ; M 6 10.5 10 10.5",
            Icon::MacroBoxes => "M 2 5 2 2 5 2 ; M 11 2 14 2 14 5 ; M 14 11 14 14 11 14 ; M 5 14 2 14 2 11 ; M 7 2 9 2 ; M 7 14 9 14 ; M 2 7 2 9 ; M 14 7 14 9",
            Icon::WireColours => "C 8 8 6.5 ; D 5.5 6 1.1 ; D 9 4.5 1.1 ; D 11.5 7.5 1.1 ; D 6 10.5 1.1",
            Icon::Panes => "R 1.5 2.5 13 11 ; M 6 2.5 6 13.5 ; M 6 7.5 14.5 7.5",
            Icon::Workspace => "R 1.5 2 6 5 ; R 9 2 5.5 5 ; R 1.5 9 6 5 ; R 9 9 5.5 5",
            Icon::Theme => "C 8 8 6 ; F 8 2 10.3 2.5 12.2 3.8 13.5 5.7 14 8 13.5 10.3 12.2 12.2 10.3 13.5 8 14",
            Icon::Scale => "M 2.5 13.5 13.5 2.5 ; M 8.5 2.5 13.5 2.5 13.5 7.5 ; M 2.5 8.5 2.5 13.5 7.5 13.5",
            Icon::Language => "C 8 8 6.5 ; E 8 8 3 6.5 ; M 1.5 8 14.5 8",
            Icon::Numbering => "M 6 2 5 14 ; M 11 2 10 14 ; M 2.5 5.5 14 5.5 ; M 2 10.5 13.5 10.5",
            Icon::Terminal => "R 4 1.5 8 13 ; C 8 5 1.5 ; C 8 11 1.5 ; M 1 5 4 5 ; M 12 11 15 11",
            Icon::Plug => "R 3 6 10 8.5 ; M 6 1.5 6 6 ; M 10 1.5 10 6",
            Icon::Plc => "R 4 4 8 8 ; M 6 1.5 6 4 ; M 10 1.5 10 4 ; M 6 12 6 14.5 ; M 10 12 10 14.5 ; M 1.5 6 4 6 ; M 1.5 10 4 10 ; M 12 6 14.5 6 ; M 12 10 14.5 10",
            Icon::Parts => "P 4.5 2 11.5 2 15 8 11.5 14 4.5 14 1 8 ; C 8 8 2.5",
            Icon::Database => "E 8 4 5.5 2 ; M 2.5 4 2.5 12 ; M 13.5 4 13.5 12 ; B 8 12 5.5 2 0 180 ; B 8 8 5.5 2 0 180",
            Icon::Server => "R 2 2 12 5 ; R 2 9 12 5 ; D 4.5 4.5 0.9 ; D 4.5 11.5 0.9",
            Icon::Vault => "R 2 2 12 12 ; C 8 8 3 ; M 8 5 8 6.5 ; M 8 9.5 8 11 ; M 5 8 6.5 8 ; M 9.5 8 11 8",
            Icon::Structure => "R 6 1.5 4 3 ; R 1.5 11 4 3.5 ; R 10.5 11 4 3.5 ; M 8 4.5 8 8 3.5 8 3.5 11 ; M 8 8 12.5 8 12.5 11",
            Icon::Preplanning => "R 2 2 4 4 ; M 8 4 14 4 ; R 2 10 4 4 ; M 8 12 14 12 ; M 3 4 3.8 5 5.2 2.8",
            Icon::SingleLine => "M 8 1.5 8 14.5 ; M 6 5 10 3 ; M 6 8 10 6 ; M 6 11 10 9",
            Icon::Cable => "M 1.5 5 14.5 5 ; M 1.5 8 14.5 8 ; M 1.5 11 14.5 11 ; E 8 8 2.2 5",
            Icon::Harness => "M 1.5 8 6 8 ; M 6 8 14.5 3 ; M 6 8 14.5 8 ; M 6 8 14.5 13 ; D 6 8 1.3",
            Icon::Field => "R 1.5 6 4 4 ; R 10.5 2 4 4 ; R 10.5 10.5 4 4 ; M 5.5 8 8 8 8 4 10.5 4 ; M 8 8 8 12.5 10.5 12.5",
            Icon::Marking => "P 2 5 10 5 14 8 10 11 2 11 ; D 5 8 1",
            Icon::CrossRef => "R 1.5 5.5 7 5 ; R 7.5 5.5 7 5",
            Icon::Check => "M 2.5 8.5 6 12 13.5 3.5",
            Icon::Warning => "P 8 2 14.5 13.5 1.5 13.5 ; M 8 6 8 9.5 ; D 8 11.5 0.9",
            Icon::Error => "C 8 8 6.5 ; M 5.5 5.5 10.5 10.5 ; M 10.5 5.5 5.5 10.5",
            Icon::Clock => "C 8 8 6.5 ; M 8 4 8 8 11 10",
            Icon::Compare => "R 1.5 2 5.5 12 ; R 9 2 5.5 12 ; M 3 5 5.5 5 ; M 10.5 5 13 5 ; M 3 8 5.5 8 ; M 10.5 9 13 9",
            Icon::ChangeRequest => "P 3 1.5 13 1.5 13 14.5 3 14.5 ; M 5.5 5 10.5 5 ; M 8 7.5 8 12.5 ; M 5.5 10 10.5 10",
            Icon::Languages => "M 2 3 8 3 ; M 5 1.5 5 3 ; M 3 3 4 6.5 7 9 ; M 7 3 6 6 2.5 9 ; M 8.5 14.5 11 7.5 13.5 14.5 ; M 9.3 12 12.7 12",
            Icon::Configurator => "M 2 4 14 4 ; M 2 8 14 8 ; M 2 12 14 12 ; R 4 2.5 2 3 ; R 9.5 6.5 2 3 ; R 6 10.5 2 3",
            Icon::Management => "R 1.5 4.5 13 9.5 ; M 5.5 4.5 5.5 2 10.5 2 10.5 4.5 ; M 1.5 8.5 14.5 8.5",
            Icon::Calculator => "R 3 1.5 10 13 ; R 5 3.5 6 2.5 ; D 6 8.5 0.8 ; D 8 8.5 0.8 ; D 10 8.5 0.8 ; D 6 11.5 0.8 ; D 8 11.5 0.8 ; D 10 11.5 0.8",
            Icon::Fluid => "P 8 1.5 12.5 8.5 12.5 10.5 11.3 13 8 14.5 4.7 13 3.5 10.5 3.5 8.5",
            Icon::Integration => "M 2 5 13 5 ; M 10 2 13 5 10 8 ; M 14 11 3 11 ; M 6 8 3 11 6 14",
            Icon::Plugin => "P 2 3 7 3 7 1.5 9.5 1.5 9.5 3 14 3 14 8 12.5 8 12.5 10.5 14 10.5 14 14.5 2 14.5",
            Icon::SymbolEditor => "R 2.5 4.5 6 7 ; M 5.5 1.5 5.5 4.5 ; M 5.5 11.5 5.5 14.5 ; P 9 13.5 9.5 11 14 6.5 15 7.5 10.5 12",
            Icon::Forms => "R 1.5 2 13 12 ; M 1.5 10 14.5 10 ; M 8 10 8 14 ; M 8 12 14.5 12",
            Icon::Enclosure => "R 3 1.5 10 12.5 ; M 10.5 6.5 10.5 9 ; M 4 14 4 15.5 ; M 12 14 12 15.5",
            Icon::Rail => "M 1.5 6 14.5 6 ; M 1.5 10 14.5 10 ; M 1.5 6 1.5 10 ; M 14.5 6 14.5 10 ; M 4 8 6 8 ; M 9 8 11 8",
            Icon::Duct => "R 1.5 4 13 8 ; M 4 4 4 7 ; M 7 4 7 7 ; M 10 4 10 7 ; M 13 4 13 7",
            Icon::Busbar => "R 1.5 3 13 2 ; R 1.5 7 13 2 ; R 1.5 11 13 2",
            Icon::Cutout => "R 1.5 1.5 13 13 ; C 8 8 3.5",
            Icon::RoutingPath => "M 2 14 2 9 8 9 8 4 14 4 ; D 2 14 1.1 ; D 14 4 1.1",
            Icon::Reservation => "R 2 2 12 12 ; M 2 7 7 2 ; M 2 12 12 2 ; M 7 14 14 7 ; M 12 14 14 12",
            Icon::Cube => "P 8 1.5 14 4.5 14 11.5 8 14.5 2 11.5 2 4.5 ; M 2 4.5 8 7.5 14 4.5 ; M 8 7.5 8 14.5",
            Icon::Front => "R 2 2 12 12 ; R 4.5 4.5 3 3 ; R 9 4.5 3 3 ; M 4 11 12 11",
            Icon::Door => "R 3 1.5 10 13 ; M 13 1.5 8 3 8 15 13 14.5 ; D 9.5 8.5 0.8",
            Icon::Wall => "P 2 4 10 2 10 12 2 14 ; P 10 2 14 4 14 14 10 12",
            Icon::Accessory => "R 2 2 12 12 ; C 8 8 4 ; M 8 4 8 12 ; M 4 8 12 8",
            Icon::Drill => "M 8 1.5 8 10 ; M 5 4 11 4 ; P 6 10 10 10 8 14.5",
            Icon::Wiring => "M 2 4 3.5 5.5 6 2.5 ; M 8 4 14 4 ; M 2 9 3.5 10.5 6 7.5 ; M 8 9 14 9 ; R 2 12 3 3 ; M 8 13.5 14 13.5",
            Icon::Mounting => "M 2.5 13.5 8.5 7.5 ; P 8.5 7.5 8 5 10 2.5 12 2.5 10.5 4.5 11.5 5.5 13.5 4 13.5 6 11 8",
            Icon::Tablet => "R 3 1.5 10 13 ; D 8 12.5 0.8",
            Icon::Star => "P 8 1.5 9.9 6 14.5 6.2 10.9 9.1 12.1 13.9 8 11.3 3.9 13.9 5.1 9.1 1.5 6.2 6.1 6",
            Icon::StarFilled => "F 8 1.5 9.9 6 14.5 6.2 10.9 9.1 12.1 13.9 8 11.3 3.9 13.9 5.1 9.1 1.5 6.2 6.1 6",
            Icon::Pin => "M 8 10 8 14.5 ; P 5 10 11 10 10 7 10 2 6 2 6 7",
            Icon::Close => "M 4 4 12 12 ; M 12 4 4 12",
            Icon::Filter => "P 1.5 2.5 14.5 2.5 9.5 8.5 9.5 14 6.5 12.5 6.5 8.5",
            Icon::Tree => "M 3 2 3 12 ; M 3 5.5 7 5.5 ; M 3 12 7 12 ; R 7.5 4 6.5 3 ; R 7.5 10.5 6.5 3",
            Icon::List => "M 5 4 14 4 ; M 5 8 14 8 ; M 5 12 14 12 ; D 2.5 4 0.9 ; D 2.5 8 0.9 ; D 2.5 12 0.9",
            Icon::Lock => "R 3 7 10 7.5 ; M 5 7 5 4.5 6.5 2.5 9.5 2.5 11 4.5 11 7",
            Icon::Unlock => "R 3 7 10 7.5 ; M 5 7 5 4.5 6.5 2.5 9.5 2.5 11 4",
            Icon::Plus => "M 8 3 8 13 ; M 3 8 13 8",
            Icon::ChevronDown => "M 4 6 8 10 12 6",
            Icon::ChevronRight => "M 6 4 10 8 6 12",
            Icon::ChevronUp => "M 4 10 8 6 12 10",
            Icon::ChevronLeft => "M 10 4 6 8 10 12",
            Icon::Float => "R 2 4.5 9.5 9.5 ; M 8.5 1.5 14.5 1.5 14.5 7.5 ; M 14.5 1.5 7.5 8.5",
            Icon::Dot => "D 8 8 3.5",
            Icon::Dots => "D 3.5 8 1.1 ; D 8 8 1.1 ; D 12.5 8 1.1",
            Icon::Keyboard => "R 1.5 4 13 8 ; M 4 10 12 10 ; D 4 7 0.7 ; D 6.5 7 0.7 ; D 9 7 0.7 ; D 11.5 7 0.7",
            Icon::Book => "P 8 4 2 2.5 2 13 8 14.5 ; P 8 4 14 2.5 14 13 8 14.5",
            Icon::Unplaced => "M 1.5 4 1.5 1.5 4 1.5 ; M 6 1.5 8 1.5 9.5 1.5 9.5 4 ; M 9.5 6 9.5 9.5 6 9.5 ; M 4 9.5 1.5 9.5 1.5 6 ; M 12 8 12 14.5 ; M 8.5 11.5 15 11.5",
            Icon::Heat => "M 7 2.5 7 10 ; C 7 12 2.2 ; M 9 2.5 9 10 ; M 7 2.5 9 2.5 ; M 11 4 13 4 ; M 11 6.5 13 6.5",
            Icon::Eye => "B 8 8 6.5 4 180 360 ; B 8 8 6.5 4 0 180 ; C 8 8 2",
            Icon::ZoomIn => "C 6.5 6.5 4.5 ; M 10 10 14.5 14.5 ; M 6.5 4.5 6.5 8.5 ; M 4.5 6.5 8.5 6.5",
            Icon::ZoomOut => "C 6.5 6.5 4.5 ; M 10 10 14.5 14.5 ; M 4.5 6.5 8.5 6.5",
            Icon::Building => "R 1.5 6 6 8.5 ; R 7.5 2 7 12.5 ; M 3.5 9 5.5 9 ; M 3.5 11.5 5.5 11.5 ; M 9.5 5 12.5 5 ; M 9.5 8 12.5 8 ; M 9.5 11 12.5 11",
            Icon::DeviceBox => "M 2 5 4 5 ; M 6 5 10 5 ; M 12 5 14 5 ; M 14 7 14 9 ; M 2 7 2 9 ; M 2 11 4 11 ; M 6 11 10 11 ; M 12 11 14 11 ; M 5 1.5 5 5 ; M 11 1.5 11 5 ; M 8 11 8 14.5",
            Icon::DeviceConnection => "M 1.5 11 4 11 ; M 6.5 11 9.5 11 ; M 12 11 14.5 11 ; M 8 11 8 3.5 ; D 8 3.5 1.3",
        }
    }
}

/// A shape of an icon on the 16 × 16 grid.
#[derive(Clone, Debug, PartialEq)]
pub enum Prim {
    Lines(Vec<Pos2>, bool),
    Fill(Vec<Pos2>),
    Circle(Pos2, f32, bool),
}

fn arc(c: Pos2, rx: f32, ry: f32, a0: f32, a1: f32) -> Vec<Pos2> {
    let n = (((a1 - a0).abs() / 12.0).ceil() as usize).max(2);
    (0..=n)
        .map(|i| {
            let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
            pos2(c.x + rx * a.cos(), c.y + ry * a.sin())
        })
        .collect()
}

/// The shapes of a path; `None` if it cannot be read.
pub fn parse(path: &str) -> Option<Vec<Prim>> {
    let mut out = Vec::new();
    for part in path.split(';') {
        let mut it = part.split_whitespace();
        let Some(op) = it.next() else { continue };
        let nums: Vec<f32> = it.map(|n| n.parse::<f32>().ok()).collect::<Option<_>>()?;
        let pts = || nums.chunks(2).map(|c| pos2(c[0], c[1])).collect::<Vec<_>>();
        let prim = match op {
            "M" | "P" | "F" if nums.len() >= 4 && nums.len().is_multiple_of(2) => match op {
                "M" => Prim::Lines(pts(), false),
                "P" => Prim::Lines(pts(), true),
                _ => Prim::Fill(pts()),
            },
            "C" | "D" if nums.len() == 3 => Prim::Circle(pos2(nums[0], nums[1]), nums[2], op == "D"),
            "E" if nums.len() == 4 => Prim::Lines(arc(pos2(nums[0], nums[1]), nums[2], nums[3], 0.0, 360.0), true),
            "R" if nums.len() == 4 => {
                let (x, y, w, h) = (nums[0], nums[1], nums[2], nums[3]);
                Prim::Lines(vec![pos2(x, y), pos2(x + w, y), pos2(x + w, y + h), pos2(x, y + h)], true)
            }
            "A" if nums.len() == 5 => Prim::Lines(arc(pos2(nums[0], nums[1]), nums[2], nums[2], nums[3], nums[4]), false),
            "B" if nums.len() == 6 => Prim::Lines(arc(pos2(nums[0], nums[1]), nums[2], nums[3], nums[4], nums[5]), false),
            _ => return None,
        };
        out.push(prim);
    }
    (!out.is_empty()).then_some(out)
}

/// Paints an icon into `rect` (square) in `color`.
pub fn paint(painter: &Painter, rect: Rect, icon: Icon, color: Color32) {
    let Some(prims) = parse(icon.path()) else { return };
    let s = rect.width().min(rect.height()) / 16.0;
    let o = rect.center() - vec2(8.0, 8.0) * s;
    let at = |p: Pos2| o + p.to_vec2() * s;
    let stroke = Stroke::new((1.25 * s).max(1.0), color);
    for prim in prims {
        match prim {
            Prim::Lines(pts, closed) => {
                let pts: Vec<Pos2> = pts.into_iter().map(at).collect();
                if closed {
                    painter.add(Shape::closed_line(pts, stroke));
                } else {
                    painter.add(Shape::line(pts, stroke));
                }
            }
            Prim::Fill(pts) => {
                let pts: Vec<Pos2> = pts.into_iter().map(at).collect();
                // Concave outlines (arrow, star) are drawn as a fan from the first point.
                for i in 1..pts.len().saturating_sub(1) {
                    painter.add(Shape::convex_polygon(vec![pts[0], pts[i], pts[i + 1]], color, Stroke::NONE));
                }
                painter.add(Shape::closed_line(pts, Stroke::new(0.5 * s, color)));
            }
            Prim::Circle(c, r, filled) => {
                if filled {
                    painter.circle_filled(at(c), r * s, color);
                } else {
                    painter.circle_stroke(at(c), r * s, stroke);
                }
            }
        }
    }
}

/// An icon of `size` points as a widget (no interaction).
pub fn show(ui: &mut Ui, icon: Icon, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let color = ui.visuals().text_color();
    paint(ui.painter(), rect, icon, color);
    resp
}

/// A button showing only an icon; `label` is its tooltip and the name a screen reader reads.
pub fn button(ui: &mut Ui, icon: Icon, label: &str) -> Response {
    toggle_button(ui, icon, label, false)
}

/// An icon button drawn as pressed when `on`.
pub fn toggle_button(ui: &mut Ui, icon: Icon, label: &str, on: bool) -> Response {
    let size = ui.spacing().interact_size.y;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let enabled = ui.is_enabled();
    resp.widget_info(|| WidgetInfo::selected(Role::Button, enabled, on, label));
    let visuals = ui.style().interact_selectable(&resp, on);
    if on || resp.hovered() || resp.has_focus() {
        ui.painter().rect(rect, visuals.corner_radius, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    }
    let color = if enabled { visuals.fg_stroke.color } else { ui.visuals().weak_text_color() };
    paint(ui.painter(), rect.shrink(3.0), icon, color);
    if label.is_empty() {
        resp
    } else {
        resp.on_hover_text(label)
    }
}

/// A button with an icon and a text beside it.
pub fn text_button(ui: &mut Ui, icon: Icon, text: &str) -> Response {
    let h = ui.spacing().interact_size.y;
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::TextStyle::Button.resolve(ui.style()), ui.visuals().text_color());
    let w = h + galley.size().x + ui.spacing().button_padding.x * 2.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let enabled = ui.is_enabled();
    resp.widget_info(|| WidgetInfo::labeled(Role::Button, enabled, text));
    let visuals = ui.style().interact(&resp);
    if resp.hovered() || resp.has_focus() {
        ui.painter().rect(rect, visuals.corner_radius, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
    }
    let color = if enabled { visuals.fg_stroke.color } else { ui.visuals().weak_text_color() };
    let icon_rect = Rect::from_min_size(rect.min, Vec2::splat(h)).shrink(3.0);
    paint(ui.painter(), icon_rect, icon, color);
    let text_pos = pos2(rect.min.x + h + ui.spacing().button_padding.x * 0.5, rect.center().y - galley.size().y / 2.0);
    ui.painter().galley_with_override_text_color(text_pos, galley, color);
    resp
}

#[cfg(test)]
mod tests;
