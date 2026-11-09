//! The ribbon of the editor: its tabs with their command groups, the pages of the
//! backstage (File tab), the key tips and the user's customisation. No UI code:
//! `app/ribbon_ui.rs` and `app/backstage.rs` draw it. The tab set follows EPLAN's
//! order and vocabulary; the groups hold the commands of `crate::commands`.

use crate::commands::{self, Cmd};
use openecraft_i18n::tr_const;
use serde::{Deserialize, Serialize};

/// Storage key of the ribbon customisation.
pub const STORAGE_KEY: &str = "ribbon";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TabId {
    Home,
    Insert,
    Edit,
    View,
    Devices,
    Connections,
    ProjectData,
    Tools,
    MasterData,
    Plugins,
    Help,
    /// Context tab of the panel layout.
    LayoutSpace,
    /// Context tab of the wiring and mounting assistants.
    Wiring,
}

/// An entry of a group.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// Large button: icon above the title.
    Large(Cmd),
    /// Small button: icon beside the title, three in a column.
    Small(Cmd),
}

impl Item {
    pub fn cmd(self) -> Cmd {
        match self {
            Item::Large(c) | Item::Small(c) => c,
        }
    }
}

pub struct Group {
    pub id: &'static str,
    pub title: &'static str,
    pub items: &'static [Item],
}

pub struct Tab {
    pub id: TabId,
    pub key: &'static str,
    pub title: &'static str,
    /// Letter of the key tip (Alt, then the letter).
    pub key_tip: char,
    /// Context tabs are drawn highlighted and shown first while their workspace is on.
    pub context: bool,
    pub groups: &'static [Group],
}

use Cmd as C;
use Item::{Large as L, Small as S};

const fn g(id: &'static str, title: &'static str, items: &'static [Item]) -> Group {
    Group { id, title, items }
}

pub static TABS: &[Tab] = &[
    Tab {
        id: TabId::Home,
        key: "home",
        title: tr_const!("Home"),
        key_tip: 'H',
        context: false,
        groups: &[
            g("clipboard", tr_const!("Clipboard"), &[L(C::Paste), S(C::Cut), S(C::Copy), S(C::MultipleCopy)]),
            g(
                "page",
                tr_const!("Page"),
                &[L(C::NewSchematicPage), S(C::NewPage), S(C::NewSubPage), S(C::NewSection), S(C::PageProperties), S(C::RenumberPages), S(C::DeletePage), S(C::MovePageUp), S(C::MovePageDown)],
            ),
            g("edit", tr_const!("Edit"), &[L(C::SelectTool), S(C::Undo), S(C::Redo), S(C::Delete), S(C::Rotate), S(C::Mirror), S(C::Align), S(C::Distribute), S(C::MoveBy)]),
            g("find", tr_const!("Find"), &[L(C::Find), S(C::CommandSearch), S(C::SelectAll)]),
            g("navigators", tr_const!("Navigators"), &[S(C::PagesPane), S(C::DevicesPane), S(C::MessagesPane)]),
        ],
    },
    Tab {
        id: TabId::Insert,
        key: "insert",
        title: tr_const!("Insert"),
        key_tip: 'I',
        context: false,
        groups: &[
            g("symbols", tr_const!("Symbols"), &[L(C::InsertCenter), S(C::UnplacedPane), S(C::DeviceBoxTool), S(C::DeviceConnectionTool)]),
            g(
                "connections",
                tr_const!("Connections"),
                &[L(C::WireTool), S(C::JunctionTool), S(C::CornerTool), S(C::CrossingTool), S(C::BridgeTool), S(C::ConnDefTool), S(C::CableDefTool), S(C::InterruptionPoint), S(C::PotentialPoint)],
            ),
            g("structure", tr_const!("Structure"), &[L(C::StructureBoxTool)]),
            g("macros", tr_const!("Macros"), &[L(C::InsertMacro), S(C::SaveSelectionAsMacro), S(C::SavePageAsMacro)]),
            g("text", tr_const!("Text"), &[L(C::TextTool), S(C::PathFunctionText), S(C::SpecialText)]),
            g(
                "graphics",
                tr_const!("Graphics"),
                &[S(C::LineTool), S(C::RectTool), S(C::CircleTool), S(C::EllipseTool), S(C::ArcTool), S(C::PolylineTool), S(C::PolygonTool), S(C::InsertImage)],
            ),
            g("dimensions", tr_const!("Dimension"), &[S(C::DimAligned), S(C::DimHorizontal), S(C::DimVertical)]),
            g("review", tr_const!("Review"), &[L(C::CommentTool), S(C::Markup)]),
            g("page", tr_const!("Page"), &[S(C::NewDocumentPage), S(C::ImportDrawing)]),
        ],
    },
    Tab {
        id: TabId::Edit,
        key: "edit",
        title: tr_const!("Edit"),
        key_tip: 'E',
        context: false,
        groups: &[
            g(
                "routing",
                tr_const!("Routing"),
                &[S(C::AutoConnect), S(C::Route), S(C::AutoConnectSelection), S(C::JoinCollinear), S(C::ApplyConnDefs)],
            ),
            g("layer", tr_const!("Layer"), &[L(C::ActiveLayer), S(C::LayersPane)]),
            g("properties", tr_const!("Properties"), &[L(C::Properties), S(C::PropertiesPane)]),
        ],
    },
    Tab {
        id: TabId::View,
        key: "view",
        title: tr_const!("View"),
        key_tip: 'V',
        context: false,
        groups: &[
            g("zoom", tr_const!("Zoom"), &[L(C::FitPage), S(C::ZoomActual), S(C::ZoomIn), S(C::ZoomOut)]),
            g("pages", tr_const!("Pages"), &[S(C::PreviousPage), S(C::NextPage), S(C::NextPageTab), S(C::ClosePageTab)]),
            g("show", tr_const!("Show"), &[S(C::Grid), S(C::GridSize), S(C::ConnectionPoints), S(C::RevisionMarks), S(C::MacroBoxes), S(C::WireColours)]),
            g("panes", tr_const!("Panes"), &[L(C::Panes), S(C::NextPane), S(C::PreviousPane), S(C::ResetLayout)]),
            g(
                "workspaces",
                tr_const!("Workspaces"),
                &[
                    S(C::WorkspaceSchematic),
                    S(C::WorkspacePanel),
                    S(C::WorkspaceWiring),
                    S(C::WorkspaceMounting),
                    S(C::WorkspaceReview),
                    S(C::WorkspacePreplanning),
                    S(C::SavedWorkspaces),
                    S(C::SaveWorkspace),
                ],
            ),
            g("appearance", tr_const!("Appearance"), &[S(C::Theme), S(C::UiScale), S(C::Language), S(C::MinimizeRibbon)]),
        ],
    },
    Tab {
        id: TabId::Devices,
        key: "devices",
        title: tr_const!("Devices"),
        key_tip: 'D',
        context: false,
        groups: &[
            g("navigators", tr_const!("Navigators"), &[S(C::DevicesPane), S(C::StripsPane), S(C::PlcPane)]),
            g("numbering", tr_const!("Numbering"), &[L(C::DeviceNumbering), S(C::RenameDevice)]),
            g("terminals", tr_const!("Terminals"), &[L(C::TerminalStrips)]),
            g("plugs", tr_const!("Plugs"), &[L(C::Plugs)]),
            g("plc", tr_const!("PLC"), &[L(C::PlcOverview), S(C::PlcHardware)]),
            g("parts", tr_const!("Parts"), &[L(C::PartSelection)]),
            g("layout", tr_const!("Panel layout"), &[S(C::ShowInLayout), S(C::ShowIn3d)]),
            g("structure", tr_const!("Structure"), &[L(C::Structure)]),
            g("preplanning", tr_const!("Pre-planning"), &[L(C::Preplanning), S(C::SingleLine)]),
        ],
    },
    Tab {
        id: TabId::Connections,
        key: "connections",
        title: tr_const!("Connections"),
        key_tip: 'C',
        context: false,
        groups: &[
            g("navigators", tr_const!("Navigators"), &[S(C::ConnectionsPane), S(C::CablesPane)]),
            g("numbering", tr_const!("Numbering"), &[L(C::WireNumbering)]),
            g("cables", tr_const!("Cables"), &[L(C::CableEditor)]),
            g("potentials", tr_const!("Potentials"), &[L(C::Potentials)]),
            g("harnesses", tr_const!("Harnesses"), &[L(C::Harnesses)]),
            g("field", tr_const!("Field wiring"), &[L(C::FieldWiring)]),
            g("marking", tr_const!("Marking"), &[L(C::Marking)]),
            g("xref", tr_const!("Cross references"), &[L(C::CrossReferences)]),
        ],
    },
    Tab {
        id: TabId::ProjectData,
        key: "project_data",
        title: tr_const!("Project data"),
        key_tip: 'P',
        context: false,
        groups: &[
            g("check", tr_const!("Check"), &[L(C::MessagesPane), S(C::CheckSettings)]),
            g("reports", tr_const!("Reports"), &[L(C::ReportsPane), S(C::ReportBlocks), S(C::UpdateReportBlocks)]),
            g("revisions", tr_const!("Revisions"), &[L(C::Revisions), S(C::Compare), S(C::ChangeRequests)]),
            g("review", tr_const!("Review"), &[L(C::ReviewPane)]),
            g("languages", tr_const!("Languages"), &[L(C::ProjectLanguages), S(C::PdfInLanguage)]),
            g("options", tr_const!("Options"), &[L(C::Configurator)]),
            g("tables", tr_const!("Tables"), &[L(C::TablesPane)]),
            g(
                "management",
                tr_const!("Management"),
                &[L(C::ProjectManagement), S(C::ProjectProperties), S(C::ExportSettings), S(C::ImportSettings)],
            ),
        ],
    },
    Tab {
        id: TabId::Tools,
        key: "tools",
        title: tr_const!("Tools"),
        key_tip: 'T',
        context: false,
        groups: &[
            g("calculations", tr_const!("Calculations"), &[L(C::Electrical)]),
            g("fluid", tr_const!("Fluid and process"), &[L(C::Process)]),
            g("integration", tr_const!("Integration"), &[L(C::Integration)]),
            g("collaboration", tr_const!("Collaboration"), &[L(C::ProjectServer), S(C::Vault), S(C::CheckInVault)]),
        ],
    },
    Tab {
        id: TabId::MasterData,
        key: "master_data",
        title: tr_const!("Master data"),
        key_tip: 'M',
        context: false,
        groups: &[
            g("symbols", tr_const!("Symbols"), &[L(C::SymbolEditor), S(C::ImportSymbols)]),
            g("forms", tr_const!("Forms"), &[L(C::Forms), S(C::ImportPageTemplates)]),
            g("macros", tr_const!("Macros"), &[L(C::Macros), S(C::ImportMacros)]),
            g("parts", tr_const!("Parts"), &[L(C::PartsManagement), S(C::DataPortal), S(C::ImportParts)]),
            g("properties", tr_const!("Properties"), &[L(C::PropertyDatabase)]),
            g("templates", tr_const!("Templates"), &[L(C::SaveAsProjectTemplate), S(C::SavePageAsTemplate)]),
        ],
    },
    Tab {
        id: TabId::Plugins,
        key: "plugins",
        title: tr_const!("Plugins"),
        key_tip: 'G',
        context: false,
        groups: &[g("plugin_commands", tr_const!("Plugins"), &[L(C::PluginCommands), L(C::ManagePlugins)])],
    },
    Tab {
        id: TabId::Help,
        key: "help",
        title: tr_const!("Help"),
        key_tip: 'Y',
        context: false,
        groups: &[g("help", tr_const!("Help"), &[L(C::Help), L(C::Tutorial), S(C::StartPage), S(C::Shortcuts), S(C::About)])],
    },
    Tab {
        id: TabId::LayoutSpace,
        key: "layout_space",
        title: tr_const!("Layout space"),
        key_tip: 'L',
        context: true,
        groups: &[
            g("enclosures", tr_const!("Enclosures"), &[L(C::NewEnclosure), S(C::NewRoom), S(C::Buildings), S(C::LayoutSpaces), S(C::LayoutProperties)]),
            g("place", tr_const!("Insert"), &[L(C::LayoutSelect), S(C::LayoutAccessory), S(C::LayoutRackPart), S(C::LayoutWallPart), S(C::LayoutInstallation), S(C::LayoutFloorPart), S(C::Layout3dMacro)]),
            g(
                "draw",
                tr_const!("Draw"),
                &[S(C::LayoutRail), S(C::LayoutDuct), S(C::LayoutBusbar), S(C::LayoutCutout), S(C::LayoutRoutingPath), S(C::LayoutReservation), S(C::LayoutDrawWalls), S(C::LayoutInnerWalls), S(C::LayoutRoomDoor), S(C::LayoutRoomWindow), S(C::LayoutRoomPenetration), S(C::LayoutRoomEntry), S(C::LayoutPipeRun)],
            ),
            g(
                "view",
                tr_const!("View"),
                &[L(C::Layout3d), S(C::LayoutFloorPlan), S(C::LayoutFront), S(C::LayoutDoor), S(C::LayoutDoorInside), S(C::LayoutWalls), S(C::LayoutFit)],
            ),
            g("show", tr_const!("Show"), &[S(C::LayoutWires), S(C::LayoutDuctFill), S(C::LayoutHeat), S(C::LayoutLineup), S(C::LayoutSite), S(C::LayoutBuilding), S(C::LayoutHideFrontWalls), S(C::LayoutHideCeiling), S(C::LayoutOpenDucts), S(C::LayoutZones), S(C::LayoutOpenWalls)]),
            g("production", tr_const!("Production"), &[L(C::LayoutNcData), S(C::LayoutReports), S(C::LayoutMountingPage)]),
        ],
    },
    Tab {
        id: TabId::Wiring,
        key: "wiring",
        title: tr_const!("Wiring"),
        key_tip: 'W',
        context: true,
        groups: &[
            g("mode", tr_const!("Mode"), &[L(C::WiringMode), L(C::MountingMode)]),
            g("panes", tr_const!("Panes"), &[S(C::WiringChecklist), S(C::WiringWorkbench)]),
            g("output", tr_const!("Export"), &[L(C::WiringExport), S(C::TabletMode)]),
        ],
    },
];

pub fn tab(id: TabId) -> &'static Tab {
    TABS.iter().find(|t| t.id == id).expect("every tab is defined")
}

/// Which groups of a tab are drawn collapsed (one dropdown button per group) so that the
/// groups fit into `available`: the rightmost groups collapse first, a group that would
/// not get narrower stays, and no more groups collapse than needed. `full` and `short` are the widths of each group drawn whole
/// and collapsed.
pub fn collapsed(full: &[f32], short: &[f32], available: f32) -> Vec<bool> {
    let mut out = vec![false; full.len()];
    let mut total: f32 = full.iter().sum();
    for i in (0..full.len()).rev() {
        if total <= available {
            break;
        }
        let s = short.get(i).copied().unwrap_or(full[i]);
        if s < full[i] {
            out[i] = true;
            total -= full[i] - s;
        }
    }
    // Room left after the last group that had to collapse: groups collapsed before it
    // come back, the left ones first.
    for i in 0..full.len() {
        let s = short.get(i).copied().unwrap_or(full[i]);
        if out[i] && total + full[i] - s <= available {
            out[i] = false;
            total += full[i] - s;
        }
    }
    out
}

// ------------------------------------------------------------------ backstage

/// A page of the backstage view (File tab).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Home,
    New,
    Open,
    Import,
    Export,
    Print,
    Info,
    Help,
}

/// An entry of the backstage's left column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Page(Page),
    Command(Cmd),
    Separator,
}

pub static BACKSTAGE: &[Entry] = &[
    Entry::Page(Page::Home),
    Entry::Page(Page::New),
    Entry::Page(Page::Open),
    Entry::Command(C::Save),
    Entry::Command(C::SaveAs),
    Entry::Command(C::SaveAsProjectTemplate),
    Entry::Page(Page::Import),
    Entry::Page(Page::Export),
    Entry::Page(Page::Print),
    Entry::Page(Page::Info),
    Entry::Separator,
    Entry::Command(C::Settings),
    Entry::Page(Page::Help),
    Entry::Command(C::Quit),
];

impl Page {
    pub fn title(self) -> &'static str {
        match self {
            Page::Home => tr_const!("Home"),
            Page::New => tr_const!("New"),
            Page::Open => tr_const!("Open project"),
            Page::Import => tr_const!("Import"),
            Page::Export => tr_const!("Export"),
            Page::Print => tr_const!("Print"),
            Page::Info => tr_const!("Project information"),
            Page::Help => tr_const!("Help"),
        }
    }

    pub fn icon(self) -> openecraft_ui::icons::Icon {
        use openecraft_ui::icons::Icon as I;
        match self {
            Page::Home => I::Home,
            Page::New => I::FileNew,
            Page::Open => I::FolderOpen,
            Page::Import => I::Download,
            Page::Export => I::Upload,
            Page::Print => I::Print,
            Page::Info => I::Info,
            Page::Help => I::Help,
        }
    }

    /// The commands a page offers (besides the lists it shows: recent projects,
    /// templates, exporters).
    pub fn commands(self) -> &'static [Cmd] {
        match self {
            Page::Home => &[C::NewProject, C::Open, C::OpenDemo, C::StartPage],
            Page::New => &[C::NewProject, C::NewFromTemplateFile, C::OpenDemo, C::OpenExample],
            Page::Open => &[C::Open, C::ProjectServer, C::Vault, C::ProjectManagement],
            Page::Import => &[C::ImportDrawing, C::NewDocumentPage, C::ImportSettings, C::ImportParts, C::ImportMacros, C::ImportSymbols, C::ImportPageTemplates],
            Page::Export => &[C::ExportSettings, C::PdfInLanguage, C::CheckInVault],
            Page::Print => &[C::PrintAll, C::PrintPage],
            Page::Info => &[C::ProjectProperties, C::ProjectManagement, C::MessagesPane, C::Revisions],
            Page::Help => &[C::Help, C::Tutorial, C::Shortcuts, C::CommandSearch, C::CustomizeRibbon, C::About, C::CheckForUpdates, C::InstallUpdate],
        }
    }
}

/// The quick access row above the ribbon.
pub static QUICK_ACCESS: &[Cmd] = &[C::Backstage, C::Save, C::Undo, C::Redo, C::PreviousPage, C::NextPage];

/// Every place a command is offered at: tab and group ids, `backstage/<page>`, `quick`.
#[cfg(test)]
pub fn places(cmd: Cmd) -> Vec<String> {
    let mut out = Vec::new();
    for t in TABS {
        for g in t.groups {
            if g.items.iter().any(|i| i.cmd() == cmd) {
                out.push(format!("{}/{}", t.key, g.id));
            }
        }
    }
    for e in BACKSTAGE {
        if *e == Entry::Command(cmd) {
            out.push("backstage".into());
        }
        if let Entry::Page(p) = e {
            if p.commands().contains(&cmd) {
                out.push(format!("backstage/{}", p.title()));
            }
        }
    }
    if QUICK_ACCESS.contains(&cmd) {
        out.push("quick".into());
    }
    if commands::def(cmd).pane.is_some() {
        out.push("view/panes".into());
    }
    out
}

// ------------------------------------------------------------------ key tips

/// Key tips for a list of titles: the first letter of a title not used yet, else
/// another letter of it, else a digit; the same title list gives the same tips.
pub fn key_tips(titles: &[String], taken: &[char]) -> Vec<char> {
    let mut used: Vec<char> = taken.iter().map(|c| c.to_ascii_uppercase()).collect();
    let mut out = Vec::with_capacity(titles.len());
    for t in titles {
        let letters: Vec<char> = t.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
        let pick = letters
            .iter()
            .copied()
            .find(|c| !used.contains(c))
            .or_else(|| ('A'..='Z').chain('1'..='9').find(|c| !used.contains(c)))
            .unwrap_or('0');
        used.push(pick);
        out.push(pick);
    }
    out
}

// ------------------------------------------------------------------ customisation

/// The user's changes of the ribbon: hidden groups and own groups of commands.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Custom {
    /// `tab/group` of the groups not shown.
    pub hidden: Vec<String>,
    pub groups: Vec<CustomGroup>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomGroup {
    /// Key of the tab it is shown in.
    pub tab: String,
    pub title: String,
    /// Command ids.
    pub commands: Vec<String>,
}

impl Custom {
    pub fn is_hidden(&self, tab: &Tab, group: &Group) -> bool {
        self.hidden.iter().any(|h| *h == format!("{}/{}", tab.key, group.id))
    }

    pub fn set_hidden(&mut self, tab: &Tab, group: &Group, hidden: bool) {
        let key = format!("{}/{}", tab.key, group.id);
        self.hidden.retain(|h| *h != key);
        if hidden {
            self.hidden.push(key);
        }
    }

    /// The own groups of a tab with the commands that exist (unknown ids are skipped).
    pub fn groups_of(&self, tab: &Tab) -> Vec<(usize, &CustomGroup, Vec<Cmd>)> {
        self.groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.tab == tab.key)
            .map(|(i, g)| (i, g, g.commands.iter().filter_map(|id| commands::by_id(id)).map(|d| d.cmd).collect()))
            .collect()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// A customisation exported before; unknown tabs and commands are dropped.
    pub fn from_json(text: &str) -> Result<Custom, String> {
        let mut c: Custom = serde_json::from_str(text).map_err(|e| e.to_string())?;
        c.groups.retain(|g| TABS.iter().any(|t| t.key == g.tab));
        for g in &mut c.groups {
            g.commands.retain(|id| commands::by_id(id).is_some());
        }
        c.hidden.retain(|h| TABS.iter().any(|t| t.groups.iter().any(|g| *h == format!("{}/{}", t.key, g.id))));
        Ok(c)
    }
}

#[cfg(test)]
mod tests;
