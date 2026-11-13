# Getting started: from the first page to the bill of materials

This tutorial walks through a small control circuit in OpenECraft: pages, symbols,
wires, devices with parts, terminals, a PLC, the project check, reports, the panel
layout and the output. Commands are written as *Tab › Command*: the tabs of the
ribbon at the top hold the commands by topic, *File* opens the backstage view with
new, open, save, import, export and print. Every command is also found by its name
with *Find a command* (Ctrl+Shift+P), Alt shows the key tips of the ribbon and F1
opens the help of the pane in use. The user interface is available in English,
German, French, Spanish and 16 further languages (*View › Language*); the names below
are the English ones.

The fastest way to see everything at once is *File › New › Open demo project*: a conveyor
control with power and control circuits, terminals, a cable, a PLC, a harness and a
panel layout. Most steps below can be tried on it.

## 1. A new project

- *File › New › New project* starts an empty project with one schematic page; the
  templates below it start from a built-in template (IEC or NFPA style frames,
  structure, potentials) and *From a template file…* from any saved project. The
  start page shows the same choices and the projects opened last.
- *Project data › Project properties…* holds the project data shown in the title
  blocks (name, customer, creator) and the default plot frame and title block.
- The project is saved as one `.oec` file (*File › Save*, Ctrl+S). In the browser the
  project is kept in the browser storage and downloaded with *File › Download project*.
  For version control (git) the desktop version also saves it as a split project
  (*File › Save as split project…*): a folder with the index `<name>.oecp` and one
  small file per page, device and part, so a change touches only the files it concerns.

## 2. Pages and structure

- The *Pages* navigator on the left lists the pages; *Home › New schematic page* adds
  one after the current page, *Home › New page* the other kinds: cover sheet, overview,
  graphics, document (an embedded PDF or image), report and mounting layout pages.
  The pages opened lately are tabs above the page (Ctrl+Tab switches).
- *Home › Page properties…* (or a right click on a page) gives a page its description
  and its structure: the function (`=A1`) and location (`+CAB1`). Devices placed on
  the page take this structure (`=A1+CAB1-K1`); on the page only the part that
  differs is shown.
- The tree button of the *Pages* navigator groups the pages by structure; *Home ›
  Renumber pages…* renumbers them. *Home › New sub-page* inserts `3.1`, `3.2`, …
  after the current page; renumbering keeps them sub-pages of their page.
- *Home › New section* structures the pages like directories: a section holds pages
  and subsections, and the page names start with its number (`2.3.1` is page 1 of
  section 2.3). In the *Pages* navigator, drag a page between two pages or onto a
  section (or choose its *Section* in the page properties) to move it there, and drag
  a section between sections or onto another one; the line or frame shows where it
  lands, the label at the pointer the name it gets, a collapsed section opens when the
  drag is held over it, and the empty space below the list is the top level. A right
  click on a section adds pages and subsections, moves, renumbers or deletes it, and
  the pages follow.
  *Renumber pages…* counts again in every section and can number the sections
  1, 2, 3 … first. Once a project has sections, sub-pages are lettered (`2.3.1.a`).

## 3. Placing symbols

- The *Insert center* on the right (*Insert › Insert center*, Ins) finds symbols of all
  libraries (IEC 60617, NFPA, fluid power, P&ID, installation plan, …), macros,
  devices from parts and unplaced functions; `*` and `?` are placeholders. Click an
  entry, then click into the page to place it. Placing continues until Esc or a right
  click. The star keeps an entry among the favorites; the ones used last come first.
- R rotates and M mirrors the symbol being placed (or the selection). Keys can be
  changed in *File › Settings › Keys and gestures*, like the mouse buttons and modifiers.
- Device tags are assigned automatically from the identifying letter (`-K1`, `-S2`).
  To place contacts of a contactor, select its coil first: contacts placed then
  belong to it and get the next free numbers (13/14, 21/22, …). Contacts and coils
  show their cross references (`/3.4`) live.
- Select a symbol to edit it in the *Properties* pane on the right: its tabs hold the
  tag, function text and connection point designations (*Device*), the placement of
  its texts (*Display*), the parts of the device (*Parts*), further properties and the
  connections with the cross references. F2 or a double click shows the same in a
  window; a right click on the page offers the commands for the selection.

## 4. Wires and potentials

- W (*Insert › Wire*) starts the wire tool: click the start, intermediate corners and
  the end; Space switches the corner direction, Enter or a double click ends the wire,
  Esc or a right click ends the tool. A wire ending in empty space is reported by the
  check *Open wire ends*. With *Edit › Auto-connect* on, symbols placed in line with
  others are connected automatically.
- Potentials (`L1`, `+24V`, `0V`, …) are potential symbols (*Insert › Potential* or the
  *Potentials* library); *Connections › Potentials* defines their kind, voltage, wire
  colour and cross-section. The *Unplaced* navigator lists defined potentials not on
  any page yet.
- Interruption points (*Insert › Interruption point*) carry a net across pages; their
  cross references are set in *Connections › Cross references…* (star or chain).
- *Connections › Wire numbering…* numbers the connections (per potential, per page,
  formats such as `{page}.{n}`); a number typed into a wire's properties is kept.

## 5. Parts

- *Devices › Parts management* opens the parts database (product groups, filter,
  part editor with commercial, technical and typed mounting data, function templates,
  connection points, 2D / 3D models, assemblies). Parts are imported from CSV,
  BMEcat, EPLAN parts XML, EDZ files of the EPLAN Data Portal (*File › Import*) or the
  data portal (*Master data › Data portal*).
- Assign parts in the *Parts* tab of a device's properties (*Add part…*); the device
  selection offers only parts that provide the device's main function. *Apply
  designations* takes the connection point designations from the part. The *Devices*
  section of the Insert center places a new device from a part with functions.
- A part can also be dragged from the tree of *Parts management* or from the results
  of the *Data portal* onto a schematic page: the symbol of its main function is placed
  there as a new device with the part assigned. A part that is not in the parts
  database yet is imported first (from the example catalog, or downloaded from the
  parts server). The command line does the same with `place_part`.
- The *Unplaced* navigator then lists the functions of the assigned parts that are not
  in the schematic yet (for example the reserve contacts of a contactor); drag one
  into the page to place it with its tag and designations.

## 6. Terminals and plugs

- Terminals are placed like other symbols (*Terminals* library); terminals with the
  same tag form a strip. *Devices › Terminal strips* orders them, adds spares,
  jumpers (plug-in, saddle, wire), end brackets and plates and multi-level blocks.
- The *Strips / plugs* navigator shows each strip in rail order and each plug with its
  pins; a click shows a placed terminal or pin, spares and unused pins are dragged
  into the page.
- *Connections › Cables* defines cables; select wires and use *Assign to cable*, or
  draw a cable definition line across them (*Insert › Cable definition*).

## 7. PLC

- PLC inputs and outputs are placed from the *PLC* library with their card tag and
  address. *Devices › PLC hardware and bus topology* adds stations, racks and modules
  from device descriptions (GSDML, ESI); *Devices › PLC overview* lists all points
  with their connected devices and readdresses cards.
- The *PLC* navigator shows the cards with their channels; free channels of cards with
  a device description are dragged into the page.
- *File › Export* writes the symbol table for the PLC programming tool: TIA Portal,
  Studio 5000, STEP 7 classic, TwinCAT 3, Control Expert, GX Works or IEC 61131-3.

## 8. The project check

- The *Messages* pane below the page runs the checks continuously (devices,
  connections, terminals, cables, PLC, panel layout, …) and lists the messages as a
  table; a click on a column sorts, a click on a message jumps to its place (F4 shows
  the pane).
- Right click a message to accept it as checked, to change the level of its kind or to
  switch its check off; *Project data › Check settings* lists them and exchanges check
  schemes with other projects.

## 9. Reports

- *Project data › Reports* shows every report (device list, bill of materials, terminal
  diagram, cable diagram, plug diagram, PLC assignment, connection list, …) as a
  table with export to CSV and Excel.
- *Insert as report page* (or *Home › New page › New report page*) inserts a report as
  pages of the project; they update when the project changes. *Master data › Forms*
  chooses the columns of a report form or switches the terminal, cable and plug
  diagrams and the PLC assignment to their graphical form.
- The bill of materials sums all parts of the devices, terminals, cables, harnesses,
  panel layout material, ferrules and marker material; assemblies list their
  components.
- *Project data › Edit in table* changes devices, connections, terminals, cables, plug
  pins, PLC points, pages and parts in bulk (filter, fill, find and replace, CSV); the
  navigators open it for their rows.

## 10. Panel layout

- The *Panel layout* workspace (*View › Workspaces* or the chooser in the quick access
  row) shows the *Layout space* tab of the ribbon: enclosures, mounting plates, rails,
  ducts, busbars, cutouts and the devices of the schematic; the 3D view shows the
  enclosure with the door, routed wires and duct fill.
- The *Wiring* workspace is the wiring assistant for the workshop (wires in order with
  colour, cross-section, length and end treatment); the *Mounting* workspace is the
  mounting assistant (items in mounting order with place, size and part). *Wiring ›
  Tablet mode* enlarges texts and buttons for a touch screen.
- Drilling patterns (*Layout space › NC data*), and the panel layout in 3D (STEP, glTF,
  JT, IFC, *File › Export*) are exported for production.

## 11. Output

- *File › Export* lists all exports: PDF (all pages or a page selection such as
  `1-3, 5`, with bookmarks and clickable cross references), HTML for printing, DXF /
  DWG, SVG, the production wire list, marking (with labeling schemes for marking
  programs), harness KBL and nailboard drawings, PLC tables, AutomationML and 3D
  formats. Exports with options ask for them first.
- *File › Print* opens all pages or the current page in the browser with its print
  dialog.
- *Project data › Export as PDF in language* writes the PDF in one of the project
  languages (*Project data › Project languages*).

## 12. Changes and review

- *Project data › Revisions* creates revisions; changes since the last released
  revision are marked on the pages and listed with old and new values in the revision
  overview. Released pages are locked.
- The *Review* workspace and *Insert › Comment* / *Markup* hold comments and redline /
  greenline markup; *Import PDF comments…* takes the comments a reviewer made in a
  PDF export.

## 13. Automation

Everything above is also available without the user interface: the command line
(`cargo run -p openecraft-cli -- help`) and the MCP server for AI agents
(`openecraft-cli mcp`) offer the same commands, checks, reports and exports, and
plugins add their own (see [plugins.md](plugins.md)); in the editor they are in
*Plugins › Plugin commands*, where the star pins a command to the *Plugins* tab.
