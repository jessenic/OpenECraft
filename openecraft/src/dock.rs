//! Docking panes of the editor: the page, the navigators, the Insert center, the
//! properties, the panel layout, the wiring assistant and every tool (reports, cables,
//! revisions, parts, …) are panes of a tile tree that the user rearranges by dragging
//! tabs (side by side, stacked, as tabs of one another). Each workspace has its own
//! tree; the trees are saved by the app and in workspaces. Tools open as floating
//! windows unless the user docked them (`Placement`). No UI code:
//! `app/dock_ui.rs` draws the panes.

use crate::workspaces::Workspace;
use openecraft_i18n::tr_const;
use egui_tiles::{Container, Linear, LinearDir, Tile, TileId, Tiles, Tree};
use serde::{Deserialize, Serialize};

/// Storage key of the panel arrangement.
pub const STORAGE_KEY: &str = "dock";

/// Tabs of the navigator.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LeftTab {
    Pages,
    Devices,
    Connections,
    Cables,
    Layers,
    /// The messages of the project check.
    Check,
    Review,
    /// Functions of the project data not placed yet.
    Unplaced,
    /// PLC cards with their channels.
    Plc,
    /// Terminal strips and plugs with their terminals and pins.
    Strips,
}

impl LeftTab {
    pub const ALL: [LeftTab; 10] = [
        LeftTab::Pages,
        LeftTab::Devices,
        LeftTab::Connections,
        LeftTab::Cables,
        LeftTab::Layers,
        LeftTab::Check,
        LeftTab::Review,
        LeftTab::Unplaced,
        LeftTab::Plc,
        LeftTab::Strips,
    ];

    /// Name in saved workspaces.
    pub fn key(self) -> &'static str {
        match self {
            LeftTab::Pages => "pages",
            LeftTab::Devices => "devices",
            LeftTab::Connections => "connections",
            LeftTab::Cables => "cables",
            LeftTab::Layers => "layers",
            LeftTab::Check => "check",
            LeftTab::Review => "review",
            LeftTab::Unplaced => "unplaced",
            LeftTab::Plc => "plc",
            LeftTab::Strips => "strips",
        }
    }

    pub fn from_key(key: &str) -> Option<LeftTab> {
        LeftTab::ALL.into_iter().find(|t| t.key() == key)
    }
}

/// The tools of the editor that are panes (they were windows before).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolPane {
    Start,
    Help,
    Reports,
    Tables,
    Find,
    Cables,
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
    Languages,
    Configurator,
    Projects,
    Structure,
    PropertyDatabase,
    SymbolEditor,
    Forms,
    ReportBlocks,
    Parts,
    DataPortal,
    ProjectServer,
    Vault,
    Plugins,
    PluginCommands,
}

impl ToolPane {
    #[cfg(test)]
    pub const ALL: [ToolPane; 37] = [
        ToolPane::Start,
        ToolPane::Help,
        ToolPane::Reports,
        ToolPane::Tables,
        ToolPane::Find,
        ToolPane::Cables,
        ToolPane::Potentials,
        ToolPane::TerminalStrips,
        ToolPane::Plugs,
        ToolPane::PlcOverview,
        ToolPane::PlcHardware,
        ToolPane::Macros,
        ToolPane::Harnesses,
        ToolPane::FieldWiring,
        ToolPane::Buildings,
        ToolPane::Revisions,
        ToolPane::Compare,
        ToolPane::ChangeRequests,
        ToolPane::Preplanning,
        ToolPane::SingleLine,
        ToolPane::Process,
        ToolPane::Electrical,
        ToolPane::Integration,
        ToolPane::Languages,
        ToolPane::Configurator,
        ToolPane::Projects,
        ToolPane::Structure,
        ToolPane::PropertyDatabase,
        ToolPane::SymbolEditor,
        ToolPane::Forms,
        ToolPane::ReportBlocks,
        ToolPane::Parts,
        ToolPane::DataPortal,
        ToolPane::ProjectServer,
        ToolPane::Vault,
        ToolPane::Plugins,
        ToolPane::PluginCommands,
    ];

    /// Narrow tools open beside the navigators and the properties; the others are wide
    /// and open as tabs beside the page.
    pub fn is_side(self) -> bool {
        matches!(self, ToolPane::Find | ToolPane::ProjectServer | ToolPane::Vault | ToolPane::Plugins)
    }

    /// Tools open as floating windows, so that it is clear what a button opened; the
    /// start page and the find results open docked.
    pub fn floats(self) -> bool {
        !matches!(self, ToolPane::Start | ToolPane::Find)
    }

    /// Size of the tool's window when it opens.
    pub fn window_size(self) -> [f32; 2] {
        use ToolPane as T;
        match self {
            T::Reports | T::Tables | T::PlcHardware | T::Harnesses | T::FieldWiring | T::Buildings | T::Compare | T::Preplanning | T::SingleLine | T::Process | T::Electrical | T::Integration | T::Projects | T::SymbolEditor | T::Forms | T::Parts | T::DataPortal => [960.0, 640.0],
            T::Help | T::Potentials | T::Plugs | T::ProjectServer | T::Vault | T::Plugins => [480.0, 560.0],
            _ => [720.0, 500.0],
        }
    }
}

/// What a pane shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Pane {
    /// The page being edited.
    Page,
    Navigator(LeftTab),
    /// The Insert center (symbols, macros, devices, unplaced functions).
    Library,
    Properties,
    Tool(ToolPane),
    /// Panel layout: enclosures, devices to place and placed devices.
    LayoutSpaces,
    /// Panel layout: the front view or the 3D view.
    LayoutView,
    /// Panel layout: the properties of the selection.
    LayoutProperties,
    /// Wiring and mounting assistant: the list of steps.
    WiringChecklist,
    /// Wiring and mounting assistant: the 3D view with the current step.
    WiringWorkbench,
}

/// Where a pane opens when it is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Area {
    Left,
    Center,
    Right,
    Bottom,
}

impl Pane {
    /// Every pane (View › Panes groups them: `menu_groups`).
    #[cfg(test)]
    pub fn all() -> Vec<Pane> {
        let mut v = vec![Pane::Page];
        v.extend(LeftTab::ALL.map(Pane::Navigator));
        v.extend([Pane::Library, Pane::Properties]);
        v.extend([Pane::LayoutSpaces, Pane::LayoutView, Pane::LayoutProperties, Pane::WiringChecklist, Pane::WiringWorkbench]);
        v.extend(ToolPane::ALL.map(Pane::Tool));
        v
    }

    pub fn area(self) -> Area {
        match self {
            Pane::Navigator(LeftTab::Check) => Area::Bottom,
            Pane::Navigator(_) | Pane::LayoutSpaces | Pane::WiringChecklist => Area::Left,
            Pane::Library | Pane::Properties | Pane::LayoutProperties => Area::Right,
            Pane::Tool(t) if t.is_side() => Area::Right,
            _ => Area::Center,
        }
    }

    /// The pane is a document (the page, the layout view, a wide tool): the last one of
    /// a workspace cannot be closed.
    pub fn is_document(self) -> bool {
        self.area() == Area::Center
    }

    /// The page and the navigators have no close button on their tab: the page is never
    /// closed, the navigators are hidden from View › Panes or the pane menu.
    pub fn has_close_button(self) -> bool {
        !matches!(self, Pane::Page | Pane::Navigator(_))
    }

    /// The pane opens as a window unless the user docked it.
    pub fn floats(self) -> bool {
        matches!(self, Pane::Tool(t) if t.floats())
    }
}

/// The sections of View › Panes (title, panes): every pane is in one of them, so the menu
/// stays short; the tools are grouped like their ribbon tabs.
pub fn menu_groups() -> Vec<(&'static str, Vec<Pane>)> {
    use ToolPane as T;
    let tools = |t: &[ToolPane]| t.iter().map(|t| Pane::Tool(*t)).collect::<Vec<_>>();
    vec![
        (tr_const!("Navigators"), LeftTab::ALL.map(Pane::Navigator).to_vec()),
        (tr_const!("Page, Insert center and properties"), vec![Pane::Page, Pane::Library, Pane::Properties, Pane::Tool(T::Find)]),
        (tr_const!("Panel layout"), vec![Pane::LayoutSpaces, Pane::LayoutView, Pane::LayoutProperties, Pane::Tool(T::Buildings)]),
        (tr_const!("Wiring and mounting"), vec![Pane::WiringChecklist, Pane::WiringWorkbench]),
        (tr_const!("Devices"), tools(&[T::TerminalStrips, T::Plugs, T::PlcOverview, T::PlcHardware, T::Structure, T::Preplanning, T::SingleLine])),
        (tr_const!("Connections"), tools(&[T::Cables, T::Potentials, T::Harnesses, T::FieldWiring])),
        (
            tr_const!("Project data"),
            tools(&[T::Reports, T::ReportBlocks, T::Tables, T::Revisions, T::Compare, T::ChangeRequests, T::Languages, T::Configurator, T::Projects]),
        ),
        (tr_const!("Tools"), tools(&[T::Electrical, T::Process, T::Integration, T::ProjectServer, T::Vault])),
        (tr_const!("Master data"), tools(&[T::SymbolEditor, T::Forms, T::Macros, T::Parts, T::DataPortal, T::PropertyDatabase])),
        (tr_const!("Plugins"), tools(&[T::PluginCommands, T::Plugins])),
        (tr_const!("Help"), tools(&[T::Start, T::Help])),
    ]
}

/// Where panes open that are not in the workspace: as a window or docked. Panes follow
/// `Pane::floats` until the user floats or docks one (pane menu › Float, Dock).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Placement {
    docked: Vec<Pane>,
    floated: Vec<Pane>,
}

impl Placement {
    pub fn floats(&self, pane: Pane) -> bool {
        if self.docked.contains(&pane) {
            false
        } else {
            self.floated.contains(&pane) || pane.floats()
        }
    }

    /// Remembers that the user floated (`true`) or docked a pane.
    pub fn set(&mut self, pane: Pane, floats: bool) {
        self.docked.retain(|p| *p != pane);
        self.floated.retain(|p| *p != pane);
        if floats != pane.floats() {
            if floats { &mut self.floated } else { &mut self.docked }.push(pane);
        }
    }
}

/// What a workspace keeps besides its tree: where its panes open and which of them float
/// as windows. Floating panes belong to their workspace and are hidden in the others.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Side {
    pub placement: Placement,
    #[serde(skip)]
    pub floating: Vec<Pane>,
}

/// The `Side` of every workspace not shown (workspace key → side).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Sides(Vec<(String, Side)>);

impl Sides {
    /// Switching from workspace `from` to `to`: `current` (the side of `from`) is kept
    /// and becomes the side of `to`.
    pub fn switch(&mut self, from: Workspace, to: Workspace, current: &mut Side) {
        let old = std::mem::take(current);
        self.0.retain(|(k, _)| k != from.key());
        self.0.push((from.key().to_string(), old));
        if let Some(i) = self.0.iter().position(|(k, _)| k == to.key()) {
            *current = self.0.remove(i).1;
        }
    }
}

fn tabs(tiles: &mut Tiles<Pane>, panes: &[Pane]) -> TileId {
    let ids: Vec<TileId> = panes.iter().map(|p| tiles.insert_pane(*p)).collect();
    tiles.insert_tab_tile(ids)
}

fn linear(tiles: &mut Tiles<Pane>, dir: LinearDir, children: &[(TileId, f32)]) -> TileId {
    let mut l = Linear::new(dir, children.iter().map(|c| c.0).collect());
    for (id, share) in children {
        l.shares.set_share(*id, *share);
    }
    tiles.insert_container(l)
}

const NAVIGATORS: [Pane; 9] = [
    Pane::Navigator(LeftTab::Pages),
    Pane::Navigator(LeftTab::Devices),
    Pane::Navigator(LeftTab::Connections),
    Pane::Navigator(LeftTab::Cables),
    Pane::Navigator(LeftTab::Strips),
    Pane::Navigator(LeftTab::Plc),
    Pane::Navigator(LeftTab::Unplaced),
    Pane::Navigator(LeftTab::Layers),
    Pane::Navigator(LeftTab::Review),
];

/// The arrangement of a workspace before any change.
pub fn default_tree_for(workspace: Workspace) -> Tree<Pane> {
    let mut tiles = Tiles::default();
    let root = match workspace {
        Workspace::Schematic => {
            let nav = tabs(&mut tiles, &NAVIGATORS);
            let page = tabs(&mut tiles, &[Pane::Page]);
            let messages = tabs(&mut tiles, &[Pane::Navigator(LeftTab::Check)]);
            let center = linear(&mut tiles, LinearDir::Vertical, &[(page, 4.0), (messages, 1.0)]);
            let library = tabs(&mut tiles, &[Pane::Library]);
            let props = tabs(&mut tiles, &[Pane::Properties]);
            let side = linear(&mut tiles, LinearDir::Vertical, &[(library, 1.3), (props, 1.0)]);
            linear(&mut tiles, LinearDir::Horizontal, &[(nav, 230.0), (center, 910.0), (side, 300.0)])
        }
        Workspace::Panel => {
            let nav = tabs(&mut tiles, &[Pane::LayoutSpaces, Pane::Navigator(LeftTab::Devices)]);
            let view = tabs(&mut tiles, &[Pane::LayoutView, Pane::Page]);
            // The layout checks (cabinets outside a room, collisions, …) stay in sight.
            let messages = tabs(&mut tiles, &[Pane::Navigator(LeftTab::Check)]);
            let center = linear(&mut tiles, LinearDir::Vertical, &[(view, 4.0), (messages, 1.0)]);
            let props = tabs(&mut tiles, &[Pane::LayoutProperties]);
            linear(&mut tiles, LinearDir::Horizontal, &[(nav, 250.0), (center, 900.0), (props, 300.0)])
        }
        Workspace::Wiring | Workspace::Mounting => {
            let list = tabs(&mut tiles, &[Pane::WiringChecklist]);
            let bench = tabs(&mut tiles, &[Pane::WiringWorkbench]);
            linear(&mut tiles, LinearDir::Horizontal, &[(list, 380.0), (bench, 1000.0)])
        }
        Workspace::Review => {
            let nav = tabs(&mut tiles, &[Pane::Navigator(LeftTab::Review), Pane::Navigator(LeftTab::Pages), Pane::Navigator(LeftTab::Check)]);
            let page = tabs(&mut tiles, &[Pane::Page, Pane::Tool(ToolPane::Revisions), Pane::Tool(ToolPane::ChangeRequests)]);
            let props = tabs(&mut tiles, &[Pane::Properties]);
            linear(&mut tiles, LinearDir::Horizontal, &[(nav, 300.0), (page, 900.0), (props, 280.0)])
        }
        Workspace::Preplanning => {
            let nav = tabs(&mut tiles, &[Pane::Navigator(LeftTab::Pages), Pane::Navigator(LeftTab::Devices)]);
            let page = tabs(&mut tiles, &[Pane::Tool(ToolPane::Preplanning), Pane::Tool(ToolPane::SingleLine), Pane::Page]);
            let props = tabs(&mut tiles, &[Pane::Properties]);
            linear(&mut tiles, LinearDir::Horizontal, &[(nav, 230.0), (page, 950.0), (props, 280.0)])
        }
    };
    Tree::new(egui::Id::unique(("dock", workspace.key())), root, tiles)
}

/// The document a workspace is for, shown when it is chosen: the first one of its
/// default arrangement (the page, the panel layout, the workbench, …).
pub fn main_document(workspace: Workspace) -> Pane {
    panes(&default_tree_for(workspace)).into_iter().find(|p| p.is_document()).unwrap_or(Pane::Page)
}

/// Puts a workspace back to its default: the arrangement before any change, no floating
/// panes, every pane opening where it opens by default.
pub fn reset(workspace: Workspace, tree: &mut Tree<Pane>, side: &mut Side) {
    *tree = default_tree_for(workspace);
    *side = Side::default();
}

/// The arrangement of the schematic workspace before any change.
#[cfg(test)]
pub fn default_tree() -> Tree<Pane> {
    default_tree_for(Workspace::Schematic)
}

/// The tile of a pane, if it is in the tree.
pub fn find(tree: &Tree<Pane>, pane: Pane) -> Option<TileId> {
    tree.tiles.find_pane(&pane)
}

/// The panes of the tree in their order.
pub fn panes(tree: &Tree<Pane>) -> Vec<Pane> {
    let mut out = Vec::new();
    let mut open: Vec<TileId> = tree.root().into_iter().collect();
    while let Some(id) = open.pop() {
        match tree.tiles.get(id) {
            Some(Tile::Pane(p)) => out.push(*p),
            Some(Tile::Container(c)) => open.extend(c.children_vec().into_iter().rev()),
            None => {}
        }
    }
    out
}

/// The panes that are shown (active tabs), in their order.
pub fn visible(tree: &Tree<Pane>) -> Vec<Pane> {
    let active = tree.active_tiles();
    panes(tree).into_iter().filter(|p| find(tree, *p).is_some_and(|id| active.contains(&id))).collect()
}

/// The document worked in after the user switched tabs: `current` while it is shown,
/// otherwise the first shown document (the page for a tree without one).
pub fn shown_document(tree: &Tree<Pane>, current: Pane) -> Pane {
    let shown = visible(tree);
    if shown.contains(&current) {
        return current;
    }
    shown.into_iter().find(|p| p.is_document()).unwrap_or(current)
}

/// The tab container a pane of `area` joins: the parent of a pane of the same area.
fn container_for(tree: &Tree<Pane>, pane: Pane) -> Option<TileId> {
    let area = pane.area();
    let order: Vec<Pane> = panes(tree);
    // Prefer a pane of the same kind (navigator beside navigator, tool beside the page).
    let candidates = order
        .iter()
        .filter(|p| p.area() == area)
        .chain(order.iter().filter(|p| area == Area::Bottom && p.area() == Area::Center))
        .chain(order.iter().filter(|p| area == Area::Left && p.area() == Area::Center));
    for p in candidates {
        let Some(id) = find(tree, *p) else { continue };
        if let Some(parent) = tree.tiles.parent_of(id) {
            if matches!(tree.tiles.get(parent), Some(Tile::Container(Container::Tabs(_)))) {
                return Some(parent);
            }
        }
    }
    None
}

/// Shows a pane: makes it the active tab where it is, or adds it as a tab of the area it
/// belongs to (navigators on the left, tools beside the page, …) if it was closed.
pub fn show(tree: &mut Tree<Pane>, pane: Pane) {
    if find(tree, pane).is_none() {
        let parent = container_for(tree, pane);
        let id = tree.tiles.insert_pane(pane);
        match parent {
            Some(parent) => {
                if let Some(Tile::Container(c)) = tree.tiles.get_mut(parent) {
                    c.add_child(id);
                }
            }
            None => {
                let tabs = tree.tiles.insert_tab_tile(vec![id]);
                let root = match tree.root() {
                    Some(old) if pane.area() == Area::Right => tree.tiles.insert_horizontal_tile(vec![old, tabs]),
                    Some(old) if pane.area() == Area::Bottom => tree.tiles.insert_vertical_tile(vec![old, tabs]),
                    Some(old) => tree.tiles.insert_horizontal_tile(vec![tabs, old]),
                    None => tabs,
                };
                tree.root = Some(root);
            }
        }
    }
    tree.make_active(|_, tile| matches!(tile, Tile::Pane(p) if *p == pane));
}

/// Closes a pane; the page and the last document of a tree stay.
pub fn close(tree: &mut Tree<Pane>, pane: Pane) {
    if !can_close(tree, pane) {
        return;
    }
    if let Some(id) = find(tree, pane) {
        tree.remove_recursively(id);
    }
}

/// A pane can be closed unless it is the page or the only document left.
pub fn can_close(tree: &Tree<Pane>, pane: Pane) -> bool {
    pane != Pane::Page && (!pane.is_document() || panes(tree).iter().filter(|p| p.is_document()).count() > 1)
}

/// The navigator tab shown first: the active one of the navigators on the left.
pub fn active_tab(tree: &Tree<Pane>) -> Option<LeftTab> {
    let active = tree.active_tiles();
    LeftTab::ALL.into_iter().filter(|t| Pane::Navigator(*t).area() == Area::Left).find(|t| find(tree, Pane::Navigator(*t)).is_some_and(|id| active.contains(&id)))
}

/// A tab label of at most `max` characters, cut with an ellipsis.
pub fn truncate_label(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let cut: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{}…", cut.trim_end())
    }
}

pub fn to_json(tree: &Tree<Pane>) -> String {
    serde_json::to_string(tree).unwrap_or_default()
}

/// A saved arrangement; `None` if it cannot be read, is empty or has a pane twice.
pub fn from_json(text: &str) -> Option<Tree<Pane>> {
    let tree: Tree<Pane> = serde_json::from_str(text).ok()?;
    let root = tree.root()?;
    tree.tiles.get(root)?;
    let all: Vec<Pane> = tree.tiles.tiles().filter_map(|t| if let Tile::Pane(p) = t { Some(*p) } else { None }).collect();
    let unique = all.iter().all(|p| all.iter().filter(|q| *q == p).count() == 1);
    (unique && all.iter().any(|p| p.is_document())).then_some(tree)
}

#[cfg(test)]
mod tests;
