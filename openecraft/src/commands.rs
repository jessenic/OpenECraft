//! The commands of the editor: every function the ribbon, the backstage, the quick
//! access row, the context menus, the command search and the shortcut editor offer comes
//! from this table, so a function exists once and has a place in the shell (the tests
//! check that every command and every pane can be reached). No UI code:
//! `app/commands_ui.rs` runs the commands, `app/ribbon_ui.rs` draws them.

use crate::dock::{LeftTab, Pane, ToolPane};
use crate::shortcuts::Shortcut;
use openecraft_i18n::tr_const;
use openecraft_ui::icons::Icon;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Cmd {
    // File and backstage
    Backstage,
    StartPage,
    NewProject,
    NewFromTemplateFile,
    OpenDemo,
    OpenExample,
    Open,
    Save,
    SaveAs,
    SaveAsProjectTemplate,
    CheckInVault,
    ImportDrawing,
    ImportSettings,
    ExportSettings,
    ImportParts,
    ImportMacros,
    ImportSymbols,
    ImportPageTemplates,
    PrintAll,
    PrintPage,
    ProjectProperties,
    Settings,
    Quit,
    // Home
    Cut,
    Copy,
    Paste,
    MultipleCopy,
    NewSchematicPage,
    NewPage,
    NewSubPage,
    NewSection,
    NewDocumentPage,
    PageProperties,
    RenumberPages,
    MovePageUp,
    MovePageDown,
    DeletePage,
    SavePageAsTemplate,
    Undo,
    Redo,
    Delete,
    Rotate,
    Mirror,
    Align,
    Distribute,
    MoveBy,
    Find,
    CommandSearch,
    SelectAll,
    SelectTool,
    // Insert
    SpecialText,
    PathFunctionText,
    WireTool,
    TextTool,
    ConnDefTool,
    CableDefTool,
    JunctionTool,
    CornerTool,
    CrossingTool,
    BridgeTool,
    InterruptionPoint,
    PotentialPoint,
    DeviceBoxTool,
    DeviceConnectionTool,
    InsertMacro,
    SaveSelectionAsMacro,
    SavePageAsMacro,
    LineTool,
    RectTool,
    CircleTool,
    EllipseTool,
    ArcTool,
    PolylineTool,
    PolygonTool,
    InsertImage,
    DimAligned,
    DimHorizontal,
    DimVertical,
    CommentTool,
    Markup,
    // Edit
    AutoConnect,
    Route,
    AutoConnectSelection,
    JoinCollinear,
    ApplyConnDefs,
    ActiveLayer,
    Properties,
    RenameDevice,
    PartSelection,
    ShowInLayout,
    ShowIn3d,
    // View
    FitPage,
    ZoomActual,
    ZoomIn,
    ZoomOut,
    PreviousPage,
    NextPage,
    NextPane,
    PreviousPane,
    NextPageTab,
    PreviousPageTab,
    ClosePageTab,
    Grid,
    GridSize,
    ConnectionPoints,
    RevisionMarks,
    MacroBoxes,
    WireColours,
    Panes,
    WorkspaceSchematic,
    WorkspacePanel,
    WorkspaceWiring,
    WorkspaceMounting,
    WorkspaceReview,
    WorkspacePreplanning,
    SavedWorkspaces,
    SaveWorkspace,
    ResetLayout,
    Theme,
    UiScale,
    Language,
    MinimizeRibbon,
    // Devices, connections, project data, tools, master data
    DeviceNumbering,
    WireNumbering,
    UpdateReportBlocks,
    PdfInLanguage,
    CheckSettings,
    CrossReferences,
    Marking,
    Shortcuts,
    CustomizeRibbon,
    // Panes
    PagesPane,
    DevicesPane,
    ConnectionsPane,
    CablesPane,
    LayersPane,
    MessagesPane,
    ReviewPane,
    UnplacedPane,
    PlcPane,
    StripsPane,
    InsertCenter,
    PropertiesPane,
    ReportsPane,
    TablesPane,
    FindPane,
    CableEditor,
    Potentials,
    TerminalStrips,
    Plugs,
    PlcOverview,
    PlcHardware,
    Macros,
    Harnesses,
    FieldWiring,
    Buildings,
    Revisions,
    Compare,
    ChangeRequests,
    Preplanning,
    SingleLine,
    Process,
    Electrical,
    Integration,
    ProjectLanguages,
    Configurator,
    ProjectManagement,
    Structure,
    PropertyDatabase,
    SymbolEditor,
    Forms,
    ReportBlocks,
    PartsManagement,
    DataPortal,
    ProjectServer,
    Vault,
    ManagePlugins,
    PluginCommands,
    Help,
    Tutorial,
    About,
    CheckForUpdates,
    InstallUpdate,
    LayoutSpaces,
    LayoutView,
    LayoutProperties,
    WiringChecklist,
    WiringWorkbench,
    // Layout space (panel layout)
    NewEnclosure,
    NewRoom,
    LayoutSelect,
    LayoutRail,
    LayoutDuct,
    LayoutBusbar,
    LayoutCutout,
    LayoutRoutingPath,
    LayoutReservation,
    LayoutAccessory,
    Layout3dMacro,
    LayoutWallPart,
    LayoutInstallation,
    LayoutRackPart,
    LayoutFloorPart,
    LayoutPipeRun,
    LayoutFloorPlan,
    LayoutDrawWalls,
    LayoutInnerWalls,
    LayoutRoomDoor,
    LayoutRoomWindow,
    LayoutRoomPenetration,
    LayoutRoomEntry,
    LayoutFront,
    LayoutDoor,
    LayoutDoorInside,
    LayoutWalls,
    Layout3d,
    LayoutWires,
    LayoutDuctFill,
    LayoutHeat,
    LayoutLineup,
    LayoutSite,
    LayoutBuilding,
    LayoutZones,
    LayoutOpenWalls,
    LayoutHideFrontWalls,
    LayoutHideCeiling,
    LayoutOpenDucts,
    LayoutFit,
    LayoutNcData,
    LayoutReports,
    LayoutMountingPage,
    // Wiring and mounting assistants
    WiringMode,
    MountingMode,
    WiringExport,
    TabletMode,
}

/// Commands that change the items of the current page: disabled on a page that
/// cannot be edited (released, generated, locked by another user).
pub fn changes_page(cmd: Cmd) -> bool {
    matches!(
        cmd,
        Cmd::Cut
            | Cmd::Paste
            | Cmd::Delete
            | Cmd::Rotate
            | Cmd::Mirror
            | Cmd::MoveBy
            | Cmd::MultipleCopy
            | Cmd::Align
            | Cmd::Distribute
            | Cmd::AutoConnectSelection
            | Cmd::JoinCollinear
            | Cmd::SpecialText
            | Cmd::PathFunctionText
            | Cmd::WireTool
            | Cmd::TextTool
            | Cmd::ConnDefTool
            | Cmd::CableDefTool
            | Cmd::JunctionTool
            | Cmd::CornerTool
            | Cmd::CrossingTool
            | Cmd::BridgeTool
            | Cmd::InterruptionPoint
            | Cmd::PotentialPoint
            | Cmd::DeviceBoxTool
            | Cmd::DeviceConnectionTool
            | Cmd::LineTool
            | Cmd::RectTool
            | Cmd::CircleTool
            | Cmd::EllipseTool
            | Cmd::ArcTool
            | Cmd::PolylineTool
            | Cmd::PolygonTool
            | Cmd::InsertImage
            | Cmd::DimAligned
            | Cmd::DimHorizontal
            | Cmd::DimVertical
    )
}

/// Keys of a command that cannot be changed (the clipboard keys of the system).
pub fn fixed_keys(cmd: Cmd) -> Option<&'static str> {
    match cmd {
        Cmd::Cut => Some("Ctrl+X"),
        Cmd::Copy => Some("Ctrl+C"),
        Cmd::Paste => Some("Ctrl+V"),
        _ => None,
    }
}

/// How a command is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Runs when clicked.
    Button,
    /// Switches something on or off (drawn pressed while on).
    Toggle,
    /// Opens a menu whose entries the application fills (lists that change).
    Menu,
}

pub struct Def {
    pub cmd: Cmd,
    /// Stable name: shortcuts, ribbon customisation, command search.
    pub id: &'static str,
    /// English title, translated where shown.
    pub title: &'static str,
    /// English tooltip, empty if the title says enough.
    pub tip: &'static str,
    pub icon: Icon,
    pub kind: Kind,
    /// Default shortcuts: `Ctrl+Y|Ctrl+Shift+Z`.
    pub keys: &'static str,
    /// The pane the command shows.
    pub pane: Option<Pane>,
    /// The page editor handles the keys itself (rotate the symbol being placed, …).
    pub canvas_keys: bool,
}

const fn d(cmd: Cmd, id: &'static str, title: &'static str, icon: Icon) -> Def {
    Def { cmd, id, title, tip: "", icon, kind: Kind::Button, keys: "", pane: None, canvas_keys: false }
}

impl Def {
    const fn tip(mut self, tip: &'static str) -> Def {
        self.tip = tip;
        self
    }
    const fn keys(mut self, keys: &'static str) -> Def {
        self.keys = keys;
        self
    }
    const fn toggle(mut self) -> Def {
        self.kind = Kind::Toggle;
        self
    }
    const fn menu(mut self) -> Def {
        self.kind = Kind::Menu;
        self
    }
    const fn pane(mut self, pane: Pane) -> Def {
        self.pane = Some(pane);
        self
    }
    const fn nav(self, tab: LeftTab) -> Def {
        self.pane(Pane::Navigator(tab))
    }
    const fn tool(self, tool: ToolPane) -> Def {
        self.pane(Pane::Tool(tool))
    }
    const fn canvas(mut self) -> Def {
        self.canvas_keys = true;
        self
    }

    /// The default shortcuts.
    pub fn default_keys(&self) -> Vec<Shortcut> {
        self.keys.split('|').filter(|k| !k.is_empty()).filter_map(Shortcut::parse).collect()
    }
}

use Cmd as C;
use Icon as I;

pub static ALL: &[Def] = &[
    // ---------------------------------------------------------------- file
    d(C::Backstage, "file", tr_const!("File"), I::File).tip(tr_const!("Open, save, import, export, print and the settings")),
    d(C::StartPage, "start_page", tr_const!("Start page"), I::Home).tool(ToolPane::Start),
    d(C::NewProject, "new_project", tr_const!("New project"), I::FileNew).keys("Ctrl+N"),
    d(C::NewFromTemplateFile, "new_from_template_file", tr_const!("From a template file…"), I::Template)
        .tip(tr_const!("Any project file: its settings, forms, structure, potentials and pages start the new project")),
    d(C::OpenDemo, "open_demo", tr_const!("Open demo project"), I::Book),
    d(C::OpenExample, "open_example", tr_const!("Example projects"), I::Book)
        .tip(tr_const!("Projects from a first circuit to a whole plant, each showing more of the features")),
    d(C::Open, "open", tr_const!("Open…"), I::FolderOpen).keys("Ctrl+O"),
    d(C::Save, "save", tr_const!("Save"), I::Save).keys("Ctrl+S"),
    d(C::SaveAs, "save_as", tr_const!("Save as…"), I::SaveAs).keys("Ctrl+Shift+S"),
    d(C::SaveAsProjectTemplate, "save_as_project_template", tr_const!("Save as project template…"), I::Template)
        .tip(tr_const!("Saves the project without revisions, baselines, review and wiring marks as a template for new projects")),
    d(C::CheckInVault, "check_in_vault", tr_const!("Check in to Vault"), I::Upload),
    d(C::ImportDrawing, "import_drawing", tr_const!("Import DXF / DWG as page…"), I::Download)
        .tip(tr_const!("An old drawing as graphics on a new schematic page (lines, arcs, texts, blocks)")),
    d(C::ImportSettings, "import_settings", tr_const!("Import settings…"), I::Download),
    d(C::ExportSettings, "export_settings", tr_const!("Export settings…"), I::Upload)
        .tip(tr_const!("Wire numbering, potentials, wire defaults and colours, marking, hose defaults, cross references and the check scheme as one file")),
    d(C::ImportParts, "import_parts", tr_const!("Import parts…"), I::Parts),
    d(C::ImportMacros, "import_macros", tr_const!("Import macros…"), I::Macro),
    d(C::ImportSymbols, "import_symbols", tr_const!("Import symbol library…"), I::Symbol),
    d(C::ImportPageTemplates, "import_page_templates", tr_const!("Import page templates…"), I::Template),
    d(C::PrintAll, "print_all", tr_const!("Print all pages…"), I::Print).keys("Ctrl+P").tip(tr_const!("Opens the pages for printing in the browser with its print dialog")),
    d(C::PrintPage, "print_page", tr_const!("Print current page…"), I::Print),
    d(C::ProjectProperties, "project_properties", tr_const!("Project properties…"), I::Info),
    d(C::Settings, "settings", tr_const!("Settings…"), I::Settings).tip(tr_const!("Project and user settings in one dialog")),
    d(C::Quit, "quit", tr_const!("Quit"), I::Quit),
    // ---------------------------------------------------------------- home
    d(C::Cut, "cut", tr_const!("Cut"), I::Cut),
    d(C::Copy, "copy", tr_const!("Copy"), I::Copy),
    d(C::Paste, "paste", tr_const!("Paste"), I::Paste).tip(tr_const!("Copy / cut / paste (devices get new tags)")),
    d(C::MultipleCopy, "multiple_copy", tr_const!("Multiple copy…"), I::MultiCopy),
    d(C::NewSchematicPage, "new_schematic_page", tr_const!("New schematic page"), I::FileNew),
    d(C::NewPage, "new_page", tr_const!("New page"), I::FileNew).menu().tip(tr_const!("Cover sheet, overview, graphics, report, mounting layout or a page template")),
    d(C::NewSubPage, "new_sub_page", tr_const!("New sub-page"), I::FileNew)
        .tip(tr_const!("A schematic page 3.1, 3.2, … after the current page, kept as sub-page when renumbering")),
    d(C::NewSection, "new_section", tr_const!("New section"), I::FolderOpen)
        .tip(tr_const!("A numbered section that holds pages like a directory; the page names start with its number (2.3.1)")),
    d(C::NewDocumentPage, "new_document_page", tr_const!("New document page…"), I::File).tip(tr_const!("Embeds a file (PDF, Word, Excel, image …) as a page")),
    d(C::PageProperties, "page_properties", tr_const!("Page properties…"), I::PageProperties),
    d(C::RenumberPages, "renumber_pages", tr_const!("Renumber pages…"), I::Renumber),
    d(C::MovePageUp, "move_page_up", tr_const!("Move page up"), I::ChevronUp),
    d(C::MovePageDown, "move_page_down", tr_const!("Move page down"), I::ChevronDown),
    d(C::DeletePage, "delete_page", tr_const!("Delete page"), I::Delete),
    d(C::SavePageAsTemplate, "save_page_as_template", tr_const!("Save page as template…"), I::Template),
    d(C::Undo, "undo", tr_const!("Undo"), I::Undo).keys("Ctrl+Z"),
    d(C::Redo, "redo", tr_const!("Redo"), I::Redo).keys("Ctrl+Y|Ctrl+Shift+Z"),
    d(C::Delete, "delete", tr_const!("Delete"), I::Delete).keys("Delete|Backspace").canvas(),
    d(C::Rotate, "rotate", tr_const!("Rotate"), I::Rotate).keys("R").canvas().tip(tr_const!("Rotate (selection or symbol being placed)")),
    d(C::Mirror, "mirror", tr_const!("Mirror"), I::Mirror).keys("M").canvas().tip(tr_const!("Mirror (selection or symbol being placed)")),
    d(C::Align, "align", tr_const!("Align"), I::Align).menu(),
    d(C::Distribute, "distribute", tr_const!("Distribute"), I::Distribute).menu(),
    d(C::MoveBy, "move_by", tr_const!("Move selection by…"), I::Move),
    d(C::Find, "find", tr_const!("Find…"), I::Find).keys("Ctrl+F").tip(tr_const!("Find and replace in the whole project")),
    d(C::CommandSearch, "command_search", tr_const!("Find a command…"), I::Find).keys("Ctrl+Shift+P|F3").tip(tr_const!("Every command of the editor and of the plugins by name")),
    d(C::SelectAll, "select_all", tr_const!("Select all"), I::SelectAll).keys("Ctrl+A").canvas(),
    d(C::SelectTool, "select_tool", tr_const!("Select"), I::Select).tip(tr_const!("Select / move (Esc)")),
    // ---------------------------------------------------------------- insert
    d(C::SpecialText, "special_text", tr_const!("Special text"), I::SpecialText)
        .menu()
        .tip(tr_const!("Special text: project and page properties such as {page} or {prop:Name} are filled in when the page is drawn")),
    d(C::PathFunctionText, "path_function_text", tr_const!("Path function text"), I::PathText)
        .tip(tr_const!("Function text of the functions in this column that have none of their own (device list, terminal, plug and PLC diagrams)")),
    d(C::WireTool, "wire_tool", tr_const!("Wire"), I::Wire).keys("W").tip(tr_const!("Draw connections")),
    d(C::TextTool, "text_tool", tr_const!("Text"), I::Text).keys("T").tip(tr_const!("Place text")),
    d(C::ConnDefTool, "connection_definition", tr_const!("Connection definition point"), I::ConnDef)
        .tip(tr_const!("Connection definition point: defines number, colour, cross-section and cable core of a connection")),
    d(C::CableDefTool, "cable_definition", tr_const!("Cable definition"), I::CableDef)
        .tip(tr_const!("Cable definition: draw a line across the wires that are cores of a cable")),
    d(C::JunctionTool, "t_junction", tr_const!("T-junction"), I::Junction)
        .tip(tr_const!("Directed T-junction: where three wires meet, names the device the two wires of the connection start from and the order of their targets")),
    d(C::CornerTool, "corner", tr_const!("Corner"), I::Corner)
        .tip(tr_const!("Corner: two wires at a right angle; with Auto-connect on, connection points facing its arms are wired to it")),
    d(C::CrossingTool, "crossing", tr_const!("Crossing"), I::Crossing)
        .tip(tr_const!("Directed crossing: connects the two wires crossing there; each end of its line is wired straight on first and across second")),
    d(C::BridgeTool, "bridge", tr_const!("Bridge"), I::Bridge)
        .tip(tr_const!("Bridge: four wires; the connection comes in at one side and goes to the three others, the opposite side first")),
    d(C::InterruptionPoint, "interruption_point", tr_const!("Interruption point"), I::Interruption),
    d(C::PotentialPoint, "potential_point", tr_const!("Potential"), I::Potential),
    d(C::DeviceBoxTool, "device_box", tr_const!("Device box"), I::DeviceBox)
        .tip(tr_const!("Device box: draw a box of any size as a device (power supply, measuring instrument, control module)")),
    d(C::DeviceConnectionTool, "device_connection_point", tr_const!("Device connection point"), I::DeviceConnection)
        .tip(tr_const!("Device connection point: placed on a device box it is a connection of its device, numbered on")),
    d(C::InsertMacro, "insert_macro", tr_const!("Macro…"), I::Macro),
    d(C::SaveSelectionAsMacro, "save_selection_as_macro", tr_const!("Save selection as macro…"), I::Save),
    d(C::SavePageAsMacro, "save_page_as_macro", tr_const!("Save page as macro…"), I::Save),
    d(C::LineTool, "line_tool", tr_const!("Line"), I::Line),
    d(C::RectTool, "rectangle_tool", tr_const!("Rectangle"), I::Rect),
    d(C::CircleTool, "circle_tool", tr_const!("Circle"), I::Circle),
    d(C::EllipseTool, "ellipse_tool", tr_const!("Ellipse"), I::Ellipse),
    d(C::ArcTool, "arc_tool", tr_const!("Arc"), I::Arc),
    d(C::PolylineTool, "polyline_tool", tr_const!("Polyline"), I::Polyline),
    d(C::PolygonTool, "polygon_tool", tr_const!("Polygon"), I::Polygon),
    d(C::InsertImage, "image", tr_const!("Image…"), I::Image).tip(tr_const!("PNG or JPEG bitmap, e.g. a logo or a photo")),
    d(C::DimAligned, "dimension_aligned", tr_const!("Aligned"), I::Dimension),
    d(C::DimHorizontal, "dimension_horizontal", tr_const!("Horizontal"), I::Dimension),
    d(C::DimVertical, "dimension_vertical", tr_const!("Vertical"), I::Dimension),
    d(C::CommentTool, "comment_tool", tr_const!("Comment"), I::Comment).tip(tr_const!("Place a comment on the page")),
    d(C::Markup, "markup", tr_const!("Markup"), I::Markup).menu().tip(tr_const!("Redline: to be changed · Greenline: changed as built")),
    // ---------------------------------------------------------------- edit
    d(C::AutoConnect, "auto_connect", tr_const!("Auto-connect"), I::AutoConnect)
        .toggle()
        .tip(tr_const!("Draw connections between facing connection points when symbols are placed or moved")),
    d(C::Route, "route", tr_const!("Route"), I::Route).toggle().tip(tr_const!("The wire tool routes around symbols and other wires")),
    d(C::AutoConnectSelection, "auto_connect_selection", tr_const!("Auto-connect selection"), I::AutoConnect)
        .tip(tr_const!("Connects free connection points of the selected symbols with facing ones")),
    d(C::JoinCollinear, "join_collinear", tr_const!("Join collinear wires"), I::Join).tip(tr_const!("Joins wire pieces in a line where nothing else connects (current page)")),
    d(C::ApplyConnDefs, "apply_connection_definitions", tr_const!("Apply connection definitions"), I::ConnDef)
        .tip(tr_const!("All connections take the values of their connection definition points")),
    d(C::ActiveLayer, "active_layer", tr_const!("Layer"), I::Layer).menu(),
    d(C::Properties, "properties", tr_const!("Properties…"), I::Properties).keys("F2").tip(tr_const!("The properties of the selection in a window beside the page")),
    d(C::RenameDevice, "rename_device", tr_const!("Rename device…"), I::Device),
    d(C::PartSelection, "part_selection", tr_const!("Part selection…"), I::Parts),
    d(C::ShowInLayout, "show_in_layout", tr_const!("Show in panel layout"), I::Enclosure).tip(tr_const!("The selected device in the layout of its cabinet or room")),
    d(C::ShowIn3d, "show_in_3d", tr_const!("Show in 3D view"), I::Cube).tip(tr_const!("The selected device in the 3D view of its cabinet or room, seen from close by")),
    // ---------------------------------------------------------------- view
    d(C::FitPage, "fit_page", tr_const!("Fit page"), I::Fit).keys("F|Home").canvas(),
    d(C::ZoomActual, "zoom_actual", tr_const!("Zoom 100 %"), I::Scale).tip(tr_const!("Real size on a 96 dpi screen")),
    d(C::ZoomIn, "zoom_in", tr_const!("Zoom in"), I::ZoomIn),
    d(C::ZoomOut, "zoom_out", tr_const!("Zoom out"), I::ZoomOut),
    d(C::PreviousPage, "previous_page", tr_const!("Previous page"), I::Previous).keys("PageUp"),
    d(C::NextPage, "next_page", tr_const!("Next page"), I::Next).keys("PageDown"),
    d(C::NextPane, "next_pane", tr_const!("Next pane"), I::Panes).keys("F6"),
    d(C::PreviousPane, "previous_pane", tr_const!("Previous pane"), I::Panes).keys("Shift+F6"),
    d(C::NextPageTab, "next_page_tab", tr_const!("Next page tab"), I::Next).keys("Ctrl+Tab"),
    d(C::PreviousPageTab, "previous_page_tab", tr_const!("Previous page tab"), I::Previous).keys("Ctrl+Shift+Tab"),
    d(C::ClosePageTab, "close_page_tab", tr_const!("Close page tab"), I::Close).keys("Ctrl+W"),
    d(C::Grid, "grid", tr_const!("Grid"), I::Grid).toggle(),
    d(C::GridSize, "grid_size", tr_const!("Grid size"), I::Grid).menu(),
    d(C::ConnectionPoints, "connection_points", tr_const!("Connection points"), I::Pins).toggle(),
    d(C::RevisionMarks, "revision_marks", tr_const!("Revision marks"), I::RevisionMarks).toggle(),
    d(C::MacroBoxes, "macro_boxes", tr_const!("Macro boxes"), I::MacroBoxes).toggle().tip(tr_const!("Window macros placed on the pages (not printed)")),
    d(C::WireColours, "wire_colours", tr_const!("Wire colours"), I::WireColours)
        .toggle()
        .tip(tr_const!("Draw wires in their colour (also in PDF and SVG); saved with the project")),
    d(C::Panes, "panes", tr_const!("Panes"), I::Panes).menu().tip(tr_const!("Show or hide panes; drag a tab to dock it elsewhere")),
    d(C::WorkspaceSchematic, "workspace_schematic", tr_const!("Schematic"), I::File).toggle(),
    d(C::WorkspacePanel, "workspace_panel", tr_const!("Panel layout"), I::Enclosure).toggle(),
    d(C::WorkspaceWiring, "workspace_wiring", tr_const!("Wiring"), I::Wiring)
        .toggle()
        .tip(tr_const!("Wiring assistant: checklist of the wires with 3D view and progress (tablet: open the web app with #wiring)")),
    d(C::WorkspaceMounting, "workspace_mounting", tr_const!("Mounting"), I::Mounting).toggle(),
    d(C::WorkspaceReview, "workspace_review", tr_const!("Review"), I::Comment).toggle(),
    d(C::WorkspacePreplanning, "workspace_preplanning", tr_const!("Pre-planning"), I::Preplanning).toggle(),
    d(C::SavedWorkspaces, "saved_workspaces", tr_const!("Workspaces"), I::Workspace).menu(),
    d(C::SaveWorkspace, "save_workspace", tr_const!("Save workspace…"), I::Save)
        .tip(tr_const!("Saves the workspace, the arrangement of the panels and the open windows")),
    d(C::ResetLayout, "reset_layout", tr_const!("Reset pane layout"), I::Panes).tip(tr_const!("Puts the panes of this workspace back at their default places and closes its floating panes")),
    d(C::Theme, "theme", tr_const!("Theme"), I::Theme).menu(),
    d(C::UiScale, "ui_scale", tr_const!("UI scale"), I::Scale).menu(),
    d(C::Language, "language", tr_const!("Language"), I::Language).menu(),
    d(C::MinimizeRibbon, "minimize_ribbon", tr_const!("Minimize the ribbon"), I::ChevronUp).keys("Ctrl+F1"),
    // ---------------------------------------------------------------- engineering
    d(C::DeviceNumbering, "device_numbering", tr_const!("Device numbering…"), I::Numbering)
        .tip(tr_const!("Numbers the device tags per structure and identifying letter in schematic order (page, column, top to bottom)")),
    d(C::WireNumbering, "wire_numbering", tr_const!("Wire numbering…"), I::Numbering),
    d(C::UpdateReportBlocks, "update_report_blocks", tr_const!("Update report blocks"), I::Report),
    d(C::PdfInLanguage, "pdf_in_language", tr_const!("Export as PDF in language"), I::Languages).menu(),
    d(C::CheckSettings, "check_settings", tr_const!("Check settings…"), I::Settings).tip(tr_const!("Check settings: checks, message levels, accepted messages")),
    d(C::CrossReferences, "cross_references", tr_const!("Cross references…"), I::CrossRef)
        .tip(tr_const!("Format of cross references, structure of the target page, interruption points, contact images")),
    d(C::Marking, "marking", tr_const!("Marking…"), I::Marking),
    d(C::Shortcuts, "shortcuts", tr_const!("Keyboard shortcuts…"), I::Keyboard),
    d(C::CustomizeRibbon, "customize_ribbon", tr_const!("Customize the ribbon…"), I::Settings),
    // ---------------------------------------------------------------- panes
    d(C::PagesPane, "pages_pane", tr_const!("Pages"), I::File).nav(LeftTab::Pages),
    d(C::DevicesPane, "devices_pane", tr_const!("Devices"), I::Device).nav(LeftTab::Devices),
    d(C::ConnectionsPane, "connections_pane", tr_const!("Connections"), I::Wire).nav(LeftTab::Connections),
    d(C::CablesPane, "cables_pane", tr_const!("Cables"), I::Cable).nav(LeftTab::Cables),
    d(C::LayersPane, "layers_pane", tr_const!("Layers"), I::Layer).nav(LeftTab::Layers),
    d(C::MessagesPane, "messages_pane", tr_const!("Messages"), I::Warning).nav(LeftTab::Check).keys("F4").tip(tr_const!("Results of the project check")),
    d(C::ReviewPane, "review_pane", tr_const!("Review"), I::Comment).nav(LeftTab::Review).tip(tr_const!("Comments and markup")),
    d(C::UnplacedPane, "unplaced_pane", tr_const!("Unplaced"), I::Unplaced).nav(LeftTab::Unplaced).tip(tr_const!("Functions of the project data not placed yet")),
    d(C::PlcPane, "plc_pane", tr_const!("PLC"), I::Plc).nav(LeftTab::Plc),
    d(C::StripsPane, "strips_pane", tr_const!("Strips / plugs"), I::Terminal).nav(LeftTab::Strips),
    d(C::InsertCenter, "insert_center", tr_const!("Insert center"), I::InsertCenter)
        .pane(Pane::Library)
        .keys("Insert")
        .tip(tr_const!("Symbols, macros, devices and unplaced functions: search, favorites, recently used")),
    d(C::PropertiesPane, "properties_pane", tr_const!("Properties pane"), I::Properties).pane(Pane::Properties),
    d(C::ReportsPane, "reports", tr_const!("Reports"), I::Report).tool(ToolPane::Reports),
    d(C::TablesPane, "edit_in_table", tr_const!("Edit in table"), I::Table)
        .tool(ToolPane::Tables)
        .tip(tr_const!("Devices, connections and terminals: filter, fill, find and replace, CSV")),
    d(C::FindPane, "find_results", tr_const!("Find results"), I::Find).tool(ToolPane::Find),
    d(C::CableEditor, "cables", tr_const!("Cables"), I::Cable).tool(ToolPane::Cables),
    d(C::Potentials, "potentials", tr_const!("Potentials"), I::Potential).tool(ToolPane::Potentials),
    d(C::TerminalStrips, "terminal_strips", tr_const!("Terminal strips"), I::Terminal).tool(ToolPane::TerminalStrips),
    d(C::Plugs, "plugs", tr_const!("Plugs"), I::Plug).tool(ToolPane::Plugs),
    d(C::PlcOverview, "plc_overview", tr_const!("PLC overview"), I::Plc).tool(ToolPane::PlcOverview),
    d(C::PlcHardware, "plc_hardware", tr_const!("PLC hardware and bus topology"), I::Plc)
        .tool(ToolPane::PlcHardware)
        .tip(tr_const!("Stations, racks, modules, bus systems, AutomationML exchange with PLC tools")),
    d(C::Macros, "macros", tr_const!("Macros"), I::Macro).tool(ToolPane::Macros),
    d(C::Harnesses, "harnesses", tr_const!("Harnesses"), I::Harness).tool(ToolPane::Harnesses),
    d(C::FieldWiring, "field_wiring", tr_const!("Field wiring"), I::Field)
        .tool(ToolPane::FieldWiring)
        .tip(tr_const!("Plant layout, route network, field cable planning, equipotential bonding and cable accessories")),
    d(C::Revisions, "revisions", tr_const!("Revisions"), I::Clock).tool(ToolPane::Revisions).tip(tr_const!("Revisions, approval and release")),
    d(C::Compare, "compare", tr_const!("Compare and baselines"), I::Compare)
        .tool(ToolPane::Compare)
        .tip(tr_const!("Compare with a baseline, a revision or another project file and take differences over")),
    d(C::ChangeRequests, "change_requests", tr_const!("Change requests"), I::ChangeRequest).tool(ToolPane::ChangeRequests),
    d(C::Preplanning, "preplanning", tr_const!("Pre-planning"), I::Preplanning)
        .tool(ToolPane::Preplanning)
        .tip(tr_const!("Plant segments with planned devices before the schematic")),
    d(C::SingleLine, "single_line", tr_const!("Single-line / multi-line"), I::SingleLine)
        .tool(ToolPane::SingleLine)
        .tip(tr_const!("Plan devices in the single-line diagram, generate the schematic and keep both in sync")),
    d(C::Process, "fluid_and_process", tr_const!("Fluid and process"), I::Fluid).tool(ToolPane::Process).tip(tr_const!("Hose defaults and PCT loops of the pre-planning")),
    d(C::Electrical, "electrical_calculations", tr_const!("Electrical calculations"), I::Calculator)
        .tool(ToolPane::Electrical)
        .tip(tr_const!("Short circuit, selectivity, loads, EMC, functional safety and verification against rule sets")),
    d(C::Integration, "enterprise_integration", tr_const!("Enterprise integration"), I::Integration)
        .tool(ToolPane::Integration)
        .tip(tr_const!("Exchange with ERP, PDM, PLM, ECAD, MCAD, procurement and wiring machines")),
    d(C::ProjectLanguages, "project_languages", tr_const!("Project languages"), I::Languages)
        .tool(ToolPane::Languages)
        .tip(tr_const!("Translate the project texts; documentation per language")),
    d(C::Configurator, "configurator", tr_const!("Configurator"), I::Configurator).tool(ToolPane::Configurator).tip(tr_const!("Generate pages from page macros by options and rules")),
    d(C::ProjectManagement, "project_management", tr_const!("Project management"), I::Management)
        .tool(ToolPane::Projects)
        .tip(tr_const!("Project database, batch changes of project properties, subprojects, options, archives and verification")),
    d(C::Structure, "structure_management", tr_const!("Structure management"), I::Structure)
        .tool(ToolPane::Structure)
        .tip(tr_const!("Structure identifiers (IEC 81346), templates and work areas")),
    d(C::PropertyDatabase, "property_database", tr_const!("Property database"), I::Database)
        .tool(ToolPane::PropertyDatabase)
        .tip(tr_const!("Property definitions, function definitions, placement schemes and the values of all objects")),
    d(C::SymbolEditor, "symbol_editor", tr_const!("Symbol editor"), I::SymbolEditor).tool(ToolPane::SymbolEditor),
    d(C::Forms, "forms", tr_const!("Forms and frames"), I::Forms).tool(ToolPane::Forms).tip(tr_const!("Title blocks, plot frames and page templates")),
    d(C::ReportBlocks, "report_blocks", tr_const!("Report blocks"), I::Report).tool(ToolPane::ReportBlocks).tip(tr_const!("Generate report pages per strip, plug, cable or device")),
    d(C::PartsManagement, "parts_management", tr_const!("Parts management"), I::Parts).tool(ToolPane::Parts),
    d(C::DataPortal, "data_portal", tr_const!("Data portal"), I::Database)
        .tool(ToolPane::DataPortal)
        .tip(tr_const!("Search the parts server by catalog, product group and technical data, add parts and check the database for updates")),
    d(C::ProjectServer, "project_server", tr_const!("Project server"), I::Server).tool(ToolPane::ProjectServer).tip(tr_const!("Open and save shared projects, lock pages")),
    d(C::Vault, "vault", tr_const!("Autodesk Vault"), I::Vault)
        .tool(ToolPane::Vault)
        .tip(tr_const!("Open projects and files from Autodesk Vault, put the project, exports and 3D models into the vault")),
    d(C::ManagePlugins, "manage_plugins", tr_const!("Manage plugins"), I::Plugin).tool(ToolPane::Plugins),
    d(C::PluginCommands, "plugin_commands", tr_const!("Plugin commands"), I::Plugin)
        .tool(ToolPane::PluginCommands)
        .tip(tr_const!("The commands of all plugins by plugin, with a search; the star pins a command to the Plugins tab")),
    d(C::Help, "help", tr_const!("Help"), I::Help).tool(ToolPane::Help).keys("F1").tip(tr_const!("Quick guide, tutorial and shortcuts; F1 opens the help of the pane in use")),
    d(C::Tutorial, "tutorial", tr_const!("Tutorial"), I::Book).tip(tr_const!("The tutorial in 13 chapters, from the first project to the automation")),
    d(C::About, "about", tr_const!("About OpenECraft"), I::Info).tip(tr_const!("Version and project page")),
    d(C::CheckForUpdates, "check_for_updates", tr_const!("Check for updates"), I::Download)
        .tip(tr_const!("Looks for a new release of OpenECraft on GitHub and installs it in the background")),
    d(C::InstallUpdate, "install_update", tr_const!("Restart and update"), I::Download)
        .tip(tr_const!("Closes OpenECraft, installs the downloaded update and starts it again")),
    d(C::LayoutSpaces, "layout_spaces", tr_const!("Layout spaces"), I::Enclosure).pane(Pane::LayoutSpaces).tip(tr_const!("Enclosures, devices to place and placed devices")),
    d(C::LayoutView, "layout_view", tr_const!("Panel layout"), I::Front).pane(Pane::LayoutView),
    d(C::LayoutProperties, "layout_properties", tr_const!("Layout properties"), I::Properties).pane(Pane::LayoutProperties),
    d(C::WiringChecklist, "wiring_checklist", tr_const!("Checklist"), I::Wiring).pane(Pane::WiringChecklist),
    d(C::WiringWorkbench, "wiring_workbench", tr_const!("Workbench"), I::Cube).pane(Pane::WiringWorkbench),
    // ---------------------------------------------------------------- layout space
    d(C::NewEnclosure, "new_enclosure", tr_const!("New enclosure"), I::Enclosure),
    d(C::NewRoom, "new_room", tr_const!("New room"), I::Wall).tip(tr_const!("A room in which cabinets stand; rails, cable ducts and wall-mounted devices go on its walls and ceiling")),
    d(C::Buildings, "buildings", tr_const!("Buildings"), I::Building)
        .tool(ToolPane::Buildings)
        .tip(tr_const!("Buildings of the plant: levels with their rooms, shafts through the levels, links between buildings and the site plan")),
    d(C::LayoutSelect, "layout_select", tr_const!("Select"), I::Select).toggle().tip(tr_const!("Select / move (Esc)")),
    d(C::LayoutRail, "layout_rail", tr_const!("Mounting rail"), I::Rail).toggle().tip(tr_const!("Click start and end point")),
    d(C::LayoutDuct, "layout_duct", tr_const!("Cable duct"), I::Duct).toggle().tip(tr_const!("Click start and end point")),
    d(C::LayoutBusbar, "layout_busbar", tr_const!("Busbar"), I::Busbar).toggle().tip(tr_const!("Copper busbar system: click start and end point")),
    d(C::LayoutCutout, "layout_cutout", tr_const!("Cutout"), I::Cutout)
        .toggle()
        .tip(tr_const!("Opening in the mounting plate / door: click two corners (round: in the properties)")),
    d(C::LayoutRoutingPath, "layout_routing_path", tr_const!("Routing path"), I::RoutingPath)
        .toggle()
        .tip(tr_const!("Path for cables, tubes or wires over the plate: click the corners, double-click or Enter to finish")),
    d(C::LayoutReservation, "layout_reservation", tr_const!("Reservation"), I::Reservation)
        .toggle()
        .tip(tr_const!("Space kept free of routes, or for one class only: click two corners")),
    d(C::LayoutAccessory, "layout_accessory", tr_const!("Accessory"), I::Accessory)
        .menu()
        .tip(tr_const!("Fans, filters, cable glands, lights, handles: choose one, then click where it goes")),
    d(C::Layout3dMacro, "layout_3d_macro", tr_const!("3D macro"), I::Macro)
        .menu()
        .tip(tr_const!("Insert a group of items (anchored items go to their place, the others where you click)")),
    d(C::LayoutFloorPlan, "layout_floor_plan", tr_const!("Floor plan"), I::Grid)
        .toggle()
        .tip(tr_const!("Floor plan of the room: drag walls to move them, drag cabinets to their places")),
    d(C::LayoutDrawWalls, "layout_draw_walls", tr_const!("Draw walls"), I::Polyline)
        .toggle()
        .tip(tr_const!("Draw the walls of the room: click the corners, walls run at right angles; click the first corner or press Enter to close")),
    d(C::LayoutInnerWalls, "layout_inner_walls", tr_const!("Inner walls"), I::Line)
        .toggle()
        .tip(tr_const!("Walls inside the room: click their ends in the floor plan, every click starts the next one; both sides carry rails, ducts, devices, doors and windows")),
    d(C::LayoutRoomDoor, "layout_room_door", tr_const!("Door"), I::Door)
        .toggle()
        .tip(tr_const!("Door in a wall of the room: click on the wall in the floor plan or in the view of the wall")),
    d(C::LayoutRoomWindow, "layout_room_window", tr_const!("Window"), I::Rect)
        .toggle()
        .tip(tr_const!("Window in a wall of the room: click on the wall in the floor plan or in the view of the wall")),
    d(C::LayoutRoomPenetration, "layout_room_penetration", tr_const!("Wall penetration"), I::Circle)
        .toggle()
        .tip(tr_const!("Wall penetration: cables, tubes and pipes pass through it into the room behind the wall; click on an outer wall in the floor plan or in the view of the wall")),
    d(C::LayoutRoomEntry, "layout_room_entry", tr_const!("Building entry"), I::Download)
        .toggle()
        .tip(tr_const!("Building entry: the supply cable, the water main and other lines from outside come into the building through it; click on an outer wall in the floor plan or in the view of the wall")),
    d(C::LayoutRackPart, "layout_rack_part", tr_const!("19-inch part"), I::Device)
        .menu()
        .tip(tr_const!("19-inch parts (switches, patch panels, servers, blank panels, …) for the rack shown: choose one, it goes into the highest free rack units; dragged, it snaps onto whole units")),
    d(C::LayoutWallPart, "layout_wall_part", tr_const!("Wall-mounted part"), I::Device)
        .menu()
        .tip(tr_const!("Parts marked for wall mounting (inverters, charge controllers, …): choose one, then click where it goes on the wall of the room")),
    d(C::LayoutInstallation, "layout_installation", tr_const!("Outlets, switches, heating"), I::Device)
        .menu()
        .tip(tr_const!("Socket outlets, data and antenna outlets, switches, push buttons and room thermostats, heating circuit manifolds and control boxes: generic ones or from the catalogs (Gira, MDT, COSMO, …); choose one, then click on the wall: it goes at its mounting height")),
    d(C::LayoutFloorPart, "layout_floor_part", tr_const!("Floor-standing part"), I::Cube)
        .menu()
        .tip(tr_const!("Parts marked for floor mounting (pumps, tanks, compressors): choose one, then click where it stands on the floor of the room")),
    d(C::LayoutPipeRun, "layout_pipe_run", tr_const!("Pipe run"), I::RoutingPath)
        .toggle()
        .tip(tr_const!("Pipe run on a wall, the ceiling or the floor of the room, along which hoses, tubes and pipes are routed: click the corners, double-click or Enter to finish")),
    d(C::LayoutFront, "layout_plate", tr_const!("Mounting plate"), I::Front).toggle(),
    d(C::LayoutDoor, "layout_door", tr_const!("Door (front)"), I::Door).toggle().tip(tr_const!("The door seen from the front: door devices and cutouts")),
    d(C::LayoutDoorInside, "layout_door_inside", tr_const!("Door (inside)"), I::Door)
        .toggle()
        .tip(tr_const!("The door seen from the inside (hinges on the right): cable ducts and the wiring behind the door")),
    d(C::LayoutWalls, "layout_walls", tr_const!("Walls"), I::Wall)
        .menu()
        .tip(tr_const!("Side walls, rear wall, roof and bottom seen from the outside: fans, cable glands, cutouts")),
    d(C::Layout3d, "layout_3d", tr_const!("3D view"), I::Cube).toggle(),
    d(C::LayoutWires, "layout_wires", tr_const!("Wires"), I::Wire).toggle(),
    d(C::LayoutDuctFill, "layout_duct_fill", tr_const!("Duct fill"), I::Duct).toggle().tip(tr_const!("Colour the cable ducts by their fill level")),
    d(C::LayoutHeat, "layout_heat_map", tr_const!("Heat map"), I::Heat)
        .toggle()
        .tip(tr_const!("Estimated air temperature in front of the mounting plate (power loss of the devices, climate)")),
    d(C::LayoutLineup, "layout_lineup", tr_const!("Line-up"), I::Workspace).toggle().tip(tr_const!("Show the enclosures bayed to this one as well")),
    d(C::LayoutSite, "layout_all_enclosures", tr_const!("All enclosures"), I::Eye)
        .toggle()
        .tip(tr_const!("Show all enclosures side by side at their places in the room")),
    d(C::LayoutBuilding, "layout_building", tr_const!("Whole building"), I::Building)
        .toggle()
        .tip(tr_const!("Show the whole building of the room in the 3D view: all its levels and rooms, the floor slabs, stairs and roofs")),
    d(C::LayoutHideFrontWalls, "layout_hide_front_walls", tr_const!("Hide walls in front"), I::Wall)
        .toggle()
        .tip(tr_const!("Leave out the walls between the eye and the inside of the room or enclosure in the 3D view")),
    d(C::LayoutHideCeiling, "layout_hide_ceiling", tr_const!("Hide ceiling"), I::Enclosure).toggle().tip(tr_const!("Leave out the ceilings of the rooms and the roofs in the 3D view")),
    d(C::LayoutZones, "layout_zones", tr_const!("Installation zones"), I::Grid)
        .toggle()
        .tip(tr_const!("Show the installation zones of the rooms see-through: on the walls in the 3D view and the views of the walls, in the floor in the floor plan")),
    d(C::LayoutOpenWalls, "layout_open_walls", tr_const!("Open walls and floors"), I::Wall)
        .toggle()
        .tip(tr_const!("See through the walls and floors of the rooms in the 3D view: the flush-mounted lines in them are seen")),
    d(C::LayoutOpenDucts, "layout_open_ducts", tr_const!("Open cable ducts"), I::Duct).toggle().tip(tr_const!("Leave out the covers of all cable ducts in the 3D view: the wires in them are seen")),
    d(C::LayoutFit, "layout_fit", tr_const!("Fit"), I::Fit),
    d(C::LayoutNcData, "layout_nc_data", tr_const!("NC data"), I::Drill).menu().tip(tr_const!("Drilling pattern of the face shown as DXF, DWG or CSV")),
    d(C::LayoutReports, "layout_reports", tr_const!("Panel wiring report"), I::Report),
    d(C::LayoutMountingPage, "layout_mounting_page", tr_const!("New mounting layout page"), I::FileNew),
    // ---------------------------------------------------------------- wiring
    d(C::WiringMode, "wiring_mode", tr_const!("Wiring"), I::Wiring).toggle(),
    d(C::MountingMode, "mounting_mode", tr_const!("Mounting"), I::Mounting).toggle(),
    d(C::WiringExport, "wiring_export", tr_const!("Export CSV"), I::Download),
    d(C::TabletMode, "tablet_mode", tr_const!("Tablet mode"), I::Tablet).toggle().tip(tr_const!("Larger texts and buttons for a touch screen at the workbench")),
];

/// The definition of a command.
pub fn def(cmd: Cmd) -> &'static Def {
    ALL.iter().find(|d| d.cmd == cmd).expect("every command has a definition")
}

pub fn by_id(id: &str) -> Option<&'static Def> {
    ALL.iter().find(|d| d.id == id)
}

/// The command that shows a pane.
pub fn for_pane(pane: Pane) -> Option<Cmd> {
    ALL.iter().find(|d| d.pane == Some(pane)).map(|d| d.cmd)
}

/// `pattern` matches `text` ignoring case: `*` stands for any text, `?` for one
/// character; without wildcards the pattern may appear anywhere in the text.
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.trim().to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    if p.is_empty() {
        return true;
    }
    if !p.iter().any(|c| *c == '*' || *c == '?') {
        let p: String = p.into_iter().collect();
        let t: String = t.into_iter().collect();
        return t.contains(&p);
    }
    // Wildcards match the whole text, with an implicit `*` on both ends.
    let mut full = vec!['*'];
    full.extend(p);
    full.push('*');
    glob(&full, &t)
}

fn glob(p: &[char], t: &[char]) -> bool {
    // Iterative matching with backtracking to the last star.
    let (mut pi, mut ti) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some((pi, ti));
            pi += 1;
        } else if let Some((sp, st)) = star {
            pi = sp + 1;
            ti = st + 1;
            star = Some((sp, st + 1));
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// A hit of the command search: the command (or entry) and how well it matched.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit<T> {
    pub item: T,
    pub score: u32,
}

/// Ranks `items` (each with the texts it can be found by, the title first) for the
/// query: words of the query must all appear; a title starting with the query comes
/// first, then a word of the title starting with it, then any match.
pub fn rank<T: Clone>(query: &str, items: &[(T, Vec<String>)]) -> Vec<Hit<T>> {
    let q = query.trim().to_lowercase();
    let words: Vec<&str> = q.split_whitespace().collect();
    let mut out: Vec<Hit<T>> = items
        .iter()
        .filter_map(|(item, texts)| {
            let all = texts.join(" ").to_lowercase();
            if !words.iter().all(|w| all.contains(w) || wildcard_match(w, &all)) {
                return None;
            }
            let title = texts.first().map(|t| t.to_lowercase()).unwrap_or_default();
            let score = if q.is_empty() {
                0
            } else if title.starts_with(&q) {
                300
            } else if title.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(words[0])) {
                200
            } else if title.contains(&q) {
                100
            } else {
                10
            };
            Some(Hit { item: item.clone(), score })
        })
        .collect();
    out.sort_by_key(|h| std::cmp::Reverse(h.score));
    out
}

#[cfg(test)]
mod tests;
