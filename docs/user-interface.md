# User interface

The editor's layout and the commands and keys of everyday work. New users start with the [tutorial](tutorial.md).

The editor is laid out like EPLAN: a **ribbon** with the File tab (backstage: new,
open, save, import, export, print, settings) and the tabs *Home*, *Insert*, *Edit*,
*View*, *Devices*, *Connections*, *Project data*, *Tools*, *Master data*, *Plugins* and
*Help*, plus the context tabs *Layout space* (panel layout) and *Wiring* (assistants);
a **quick access row** above it (save, undo list, page switch, workspace, command
search); dockable **panes** for the page, the navigators, the Insert center, the
properties, the messages and every tool; a **status bar** with position, grid, layer,
zoom, language, check results and the messages of the session.

| Action | How |
| --- | --- |
| Find any command | `Ctrl+Shift+P` / `F3` or the search box of the quick access row: every command of the editor and of the plugins by name, tooltip or place, and the entries of the fixed menus (*Align: Left*, *New cover sheet*, …); a menu command (*Align*, *Panes*, *Grid size*, …) opens its menu in a small window |
| Key tips | `Alt` (or `F10`) shows a letter below each tab, a second letter (at the corner of a button) runs a command; `Esc` goes back |
| Place symbol | *Insert › Insert center* (`Ins`): search (`*` and `?` as placeholders), favorites, recently used, symbols, macros, devices from parts, unplaced functions; click, then click into the page; `R` rotate, `M` mirror, `Esc` stop |
| Draw wires | `W` (*Insert › Wire*), click start and corner points, ends automatically on a connection point; `Space` flips the corner, `Enter` or a double click ends the wire |
| Drawing tools | wire, text, graphics, dimensions, connection and cable definitions stay active for the next item; `Esc` or a right click ends the tool. The status bar shows the hint of the tool with the current keys |
| Select / move | click, `Shift`+click, drag on empty space for window selection (from left to right: the items inside; from right to left, dashed: the items it touches), drag items to move; while dragging in schematic editing or panel layout, right-click rotates 90° clockwise and `Shift`+right-click rotates 90° counter-clockwise. Arrow keys move by one grid step; right click for the commands of the selection. Released, generated and locked pages can be selected and copied, not changed |
| Edit properties | *Properties* pane with the tabs *Device*, *Display*, *Parts*, *Properties*, *Connections*; `F2` or a double click highlights the pane, or shows the properties in a window beside the pointer when the pane is hidden; several symbols: their common fields. *Belongs to device* moves the function to another device when `Enter` is pressed or the field is left (`Esc` restores); *Rename device* renames the whole device |
| Page structure | *Home › Page properties…* (or right click on a page): function (`=`) and location (`+`) |
| Structure box | *Insert › Structure box*: click two corners, then set the function (`=`) and / or location (`+`) of the area in the *Properties* pane (empty = inherited), the corner of the identifiers, the text offset and the outline; pick the box at its outline to move it |
| Unsaved changes | a new or opened project (also from the recent list, a template, the project database, the vault or the project server) and closing the window ask first: *Save* saves and then goes on, *Discard changes*, *Cancel*; unsaved changes stay unsaved (`*`) when the app is started again |
| Delete page | *Home › Delete page* (or right click on a page) asks first when the page is not empty; released pages and pages locked by another user cannot be deleted; undo brings a page back |
| Page tabs | the pages opened lately are tabs above the page (long names end in `…`, the whole name and description on hover); `Ctrl+Tab` / `Ctrl+Shift+Tab` switch, `Ctrl+W`, the cross of the current tab or a middle click closes a tab |
| Cables | *Connections › Cables*; select wires › *Assign to cable*; single wire: cable and core |
| Wire numbers | *Connections › Wire numbering…*; single wire: type a number (kept by automatic numbering) |
| Macros | select items, right click › *Save selection as macro…*; place them from the Insert center or *Insert › Macro…* |
| Configurator | *Project data › Configurator*: options, rules with page macros, *Generate pages* |
| Bulk changes | *Project data › Edit in table* (or the table button of a navigator): devices, connections, terminals, cables, plug pins, PLC points, pages and parts; filter, edit, fill, find and replace, CSV |
| Project check | *Messages* pane (`F4`): sortable table of the messages; click shows the object, right click accepts, changes the level or switches the check off |
| Enclosure climate | panel layout: select the enclosure › *Climate (IEC 60890)* |
| Terminal strips / PLC | *Devices › Terminal strips* / *Devices › PLC overview* |
| Plugs | place pins from the symbol group *Plugs*; *Devices › Plugs* for pin order and both sides |
| Revisions | *Project data › Revisions*: create (and release) revisions, approval workflow, lock / unlock pages, see changes |
| Compare | *Project data › Compare and baselines*: create baselines, compare, take differences over |
| Review | *Review* workspace or navigator, *Insert › Comment* and *Insert › Markup* (redline / greenline) |
| Settings | *File › Settings*: project settings (wire numbering, cross references, checks, marking, settings scheme) and user settings (appearance, UI scale, language, 3D view drawn by the graphics card, wall opacity of the 3D view, frame time and frames per second in the title bar, what opens on startup, keys and gestures, ribbon, servers) in one dialog; *Appearance › Schematic crosshair* extends the placement crosshair to the canvas edges |
| Settings scheme | *Project data › Export settings… / Import settings…*: the project's wire numbering, potentials, wire defaults and colours, marking, hose defaults, cross references and check scheme as one file for other projects (tools `export_settings`, `import_settings`) |
| Panes | drag the tab of any pane (page, navigators, Insert center, properties, messages, tools) beside, above or below another one or among its tabs; tools (reports, cables, revisions, parts, …) open as floating windows, *Dock* in a window puts it into the workspace and the pane menu (`⋯`) floats a docked pane again or closes it (the choice is remembered); the page cannot be closed (a click on a page brings it back), navigators have no cross on their tab; a tab bar too narrow for its titles shows icons and a list of all its tabs (the arrow beside the pane menu); *View › Panes* shows or hides each, by area, and *Reset pane layout* puts the panes of the current workspace back at their places; floating panes and docking choices belong to their workspace; `F6` moves between the panes |
| Workspaces | *View › Workspaces* or the chooser in the quick access row: *Schematic*, *Panel layout*, *Wiring*, *Mounting*, *Review*, *Pre-planning*, each with its own arrangement of the panes; *Save workspace…* keeps the arrangement and the ribbon under a name (kept by the app) |
| Ribbon | groups that do not fit the window collapse from the right into one button with their commands; double click a tab or the arrow at the right minimizes it; *File › Settings › Ribbon* hides groups, adds own groups of commands and exports / imports them |
| Keys and gestures | *File › Settings › Keys and gestures*, four sections. *Keyboard shortcuts*: record, clear or reset the keys of every command (also Delete and the arrow keys of the page, the keys that move through the 3D view); grouped by ribbon tab, a filter searches all groups; conflicts are shown where both keys act (the page, the panel layout), the keys are kept by the app and shown in the tooltips, the hints and the help; `Esc` and the clipboard keys stay fixed. *Modifier keys*: Shift, Ctrl, Alt or off for adding to the selection, opening a text's link, picking a single item of an assembly, reversing *Keep connections* for a move, rotating back while moving, moving faster in 3D, scrolling sideways and zooming by scrolling. *Mouse*: record or add the button (with a modifier) that pans the flat views, orbits and pans the 3D view, cancels a drawing, rotates while moving and the double-clicks that finish a drawing, open the selection and turn the 3D view; the left button alone stays the one that selects and draws. *Wheel and touch*: what scrolling does and its direction, what two fingers do (pan and zoom together, zoom only, pan only; on touch screens and trackpads) and three fingers on a touch screen (like two, or any of those). Conflicts are shown in each section, each has its reset |
| Change requests | *Project data › Change requests*: requests, CSV / JSON import and export |
| Export | *File › Export*: PDF, DXF / DWG (page / all), SVG, HTML, production wire list, panel layout in 3D (glTF / STEP / IGES / IFC / JT / Parasolid) |
| Import old drawings | *File › Import › Import DXF / DWG as page…* |
| Own symbols | *Master data › Symbol editor*; they appear under *User symbols* in the Insert center |
| Title blocks / report forms | *Master data › Forms and frames*; per page in the page properties |
| Panel production | *Layout space* tab: *Busbar*, *Cutout*, *NC data* (DXF / DWG / CSV) |
| Door devices | *Layout space › Door (front)* for door devices and cutouts, *Door (inside)* (mirrored) for cable ducts and the door wiring; 3D: *Open door* / *Close door* |
| Wiring the panel | *Wiring* workspace: pick a wire in the list or in 3D, *Wired* / *Problem*; *Wiring › Tablet mode* for touch screens; `--wiring` (web: `#wiring`) starts in the Wiring workspace with tablet mode for that session |
| Project texts in several languages | *Project data › Project languages*: target languages, *Collect texts from the project*, translate (or CSV for translators); *Project data › Export as PDF in language* |
| Hoses / PCT loops | draw wires between fluid / P&ID ports; hose properties and configuration in the wire properties; *Tools › Fluid and process* (hoses, device groups, electro-fluid links and *Synchronize*, PCT loops) |
| Harnesses | *Connections › Harnesses*; select the wires between two devices › *Assign to harness*, or per connection in the wire properties; 3D topology, protections, fixings, end treatment, KBL, harness interface and 3D export in the pane |
| Help | `F1` opens the tutorial chapter of the pane in use; the Help pane has the quick guide (the first hour, with links into the tutorial), the tutorial, keyboard shortcuts; *Help › Tutorial*, *Help › Start page* (all 13 chapters), *Help › About OpenECraft* |
| Dialogs | open centred over the editor and keep the place and size they were given; `Esc` closes the one on top, *Cancel* drops what was typed; the command search closes on a click outside |
| Pan / zoom | right or middle mouse drag / mouse wheel (horizontal wheel and `Shift`+wheel pan sideways; buttons and modifiers in *Settings › Keys and gestures*), `F` fits the page; *View › Zoom*: fit page, zoom 100 %, zoom in, zoom out |
| Trackpad / touch screen | in every 2D and 3D view: pinch zooms, two fingers pan (also together with the pinch); on a touch screen one finger works like the left mouse button. *Settings › Keys and gestures › Wheel and touch*: scrolling *Automatic* (identified trackpads pan, identified mouse wheels zoom); *Wheel source*: *Assume Windows trackpad* (default) treats unknown point-based or fractional-line scrolling as trackpad input, while whole-line and page scrolling zoom; *Strict* treats every unknown source as a mouse wheel; *Zooms (mouse wheel)* or *Pans (trackpad)*, the direction of panning by scrolling inverted, two fingers set to zoom only or pan only, three fingers to their own choice |
| Moving in 3D | drag: orbit; right / middle or Shift+drag: pan; wheel: zoom at the pointer (on into a room); double-click: look at what was clicked; `W` `A` `S` `D` / arrow keys, `Q` `E`: move (Shift: faster); bar in the view: front, left, right, rear, top, isometric, *Fit* (`Home`) |
| Pages | `Page Up` / `Page Down`, *Pages* navigator (its context menu also adds a schematic page after the page, renumbers and deletes pages), page switch of the quick access row |
| Undo / redo | `Ctrl+Z` / `Ctrl+Y`; the undo list of the quick access row undoes several steps |
| Copy / paste | `Ctrl+C` / `Ctrl+X` / `Ctrl+V`, *Home › Cut* / *Copy* / *Paste* |
