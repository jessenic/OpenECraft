# Changelog

The notable changes of each OpenECraft release. The section of a version is the text of
its GitHub release ([docs/releasing.md](docs/releasing.md)).

## [0.1.2] - unreleased

- **EPLAN Data Portal downloads:** DXF and commercial-data downloads use the current
  POST API, and the connection test checks the protected file-provider service without
  downloading commercial data. On Windows, an explicit Remember token option lets
  a verified PAT be shared securely between the editor and CLI/MCP through
  Credential Manager ([parts server](docs/parts-server.md#shared-data-portal-pat-on-windows)). The first
  GUI request loads the saved PAT in the background automatically, without requiring
  a connection test. Concurrent requests share the lookup.

- **Straight ferrules and terminals:** a wire or cable core with a ferrule or a crimped
  terminal runs straight out of its connection point for the length of it (with its
  collar or insulated sleeve) and bends only behind it, in the 3D routing and in the 3D
  view, instead of bending the ferrule where it goes into the device
  ([panel layout](docs/features/panel-layout.md)).
- **Keys and gestures:** *File › Settings › Keys and gestures* configures every key,
  modifier and gesture in one place: the keyboard shortcuts (now also the keys that
  move through the 3D view, and the panel layout follows the keys of Rotate, Delete and
  Fit), the modifier keys (adding to the selection, opening links, a single item of an
  assembly, *Keep connections* reversed for a move, rotating back, moving faster in 3D,
  the wheel turned sideways or zooming),
  the mouse buttons that pan, orbit, cancel, rotate while moving and the double-clicks,
  and what the wheel does, two fingers (pan and zoom together, zoom only or pan only)
  and three fingers on a touch screen. It replaces the *Navigation* and
  *Keyboard shortcuts* settings ([user interface](docs/user-interface.md)).
- **Moved wires stay connected** (#133): a wire moved on its own no longer leaves the
  connection points it ends at; it gets a new leg from the point, or slides along the
  rail it ends on. *Edit › Keep connections* (on by default) switches this and the
  stretching of attached wires off, so the selection moves alone; Alt held at the
  start of a drag reverses it for that drag. Dragging the square handle at an end of a
  selected wire moves only that end, to disconnect it or connect it elsewhere. Commands:
  `move_items` (`keep_connections`), `move_wire_end`
  ([schematic](docs/features/schematic.md#editing)).
- **Server administrator from the environment:** `OPENECRAFT_ADMIN_USER` (parts server)
  and `OPENECRAFT_PROJECT_ADMIN_USER` (project server) name the administrator account
  created at startup, as `--admin-user` does, so a Docker container gets its first user
  from environment variables alone ([installation](docs/installation.md#docker-images-of-the-servers)).

- **Full-canvas schematic crosshair:** *File › Settings › Appearance* can extend the
  placement crosshair to the edges of the schematic canvas, following the snapped
  working point.
- **AppImage on more systems:** the AppImage carries the keyboard and cursor libraries
  of X11 (xkbcommon, Xcursor, Xi), so the editor and the parts manager start on systems
  without them instead of quitting with *Library libxkbcommon-x11.so could not be
  loaded*.
- **Invisible grid lines:** connection points and wire ends off the drawing grid, as in
  imported drawings or on device boxes of any size, add invisible grid lines through
  them. The pointer snaps to them where they are nearer than the grid, so wires can be
  drawn from such points and things connected to such wires. A symbol being placed or
  moved snaps from its own connection points, so they land on such lines and
  auto-connect joins them ([schematic](docs/features/schematic.md)).
- **Auto-connect hints:** while a symbol is placed or moved with the mouse, faint
  dotted lines show the wires auto-connect would draw
  ([schematic](docs/features/schematic.md#routing)).
- **Cut, copy and paste in the panel layout** (#153): right click an item in the front
  view for *Cut*, *Copy*, *Paste*, *Properties* and *Remove* (devices go back to the
  devices to place), or use `Ctrl+X` / `Ctrl+C` / `Ctrl+V`. Rails, ducts and the other
  items are copied with what is mounted on them, without devices; cut devices go back
  to the devices to place. The copy follows the pointer until a click places it
  ([panel layout](docs/features/panel-layout.md#enclosures-and-devices)). New command
  `copy_panel_items`.
- **Profile sizes on mounting layouts** (#152): cable ducts, mounting rails and busbar
  systems are labelled with their cross-section and cut length
  (*60 × 100 mm, L = 940 mm*) on mounting layout pages and in the panel layout, on the
  longest stretch no device covers (only the length where the whole label does not
  fit), beside the end of a full one. The panel legend lists them after the devices with part number, cross-section
  (width × height) and cut length; the bill of materials lists the pieces to cut below
  each of their parts (*2 pcs, Cut length 550 mm, +CAB1*).
- **Panel layout profile sizing** (#135): select a mounting rail or cable duct and drag
  either end to change its length while the opposite end stays put.
- **Connection symbols** (#96): Auto-connect wires the branch of a T-junction that sits
  on one continuous wire, as it does on a wire split at the junction. While a connection
  symbol tool is active, Tab (*Next routing of the connection symbol to place*) cycles
  the routing or orientation of the next symbol, Rotate turns it, and the Properties
  pane shows its routings to pick from; a placed symbol is no longer selected, so these
  changes never alter it; the tools show the Properties pane again when it was closed.
  A T-junction placed where three wires meet keeps the chosen routing. Connection
  symbols follow their wires when wires or devices move, also in fast drags, sliding
  along the rail with their drop, and wires branching off a moved wire stretch with it.
- **Trackpads and touch screens** (#134): every 2D and 3D view (schematic, front,
  floor and site plan, 3D view, field plan, symbol editor) zooms with a pinch and pans
  with two fingers, both in one movement; on a touch screen the first finger no longer
  selects, moves or orbits while two are down. The horizontal mouse wheel and
  `Shift`+wheel pan sideways. *Settings › Keys and gestures › Wheel and touch* chooses what scrolling does:
  *Automatic* (trackpads pan, mouse wheels zoom, told apart on macOS), *Zooms (mouse
  wheel)* or *Pans (trackpad)*, and can invert the direction of panning by scrolling.
  The symbol editor's canvas pans too (also with the middle mouse button).
- **Settings after an update check:** checking for updates no longer collapses Settings
  into an unusable title bar. Dialogs already stuck this way recover their contents.
- **Wire jumps** (#132): wires that cross without a connection can jump over each other
  with an arc, a square bridge or a gap, besides the plain crossing of before; the
  vertical or the horizontal wire jumps. T-connections show a dot, a large dot or no
  dot (a plain T); connections of four or more wires always keep their dot. Set in
  *Connections › Wire crossings…* (Settings › Wire crossings) with a preview, saved
  with the project and in settings schemes, drawn alike on screen and in PDF, SVG and
  DXF. Command `wire_settings`.
- **Rerouting moved connections** (#132, off by default): with *Route the connections
  of moved items anew* a wire left diagonal by a move is routed again together with
  the corners after it, up to the next connection point, junction or branch: the
  way of the least weight, its length in grid steps plus 1.2 per bend and 1.5 per
  wire crossed. Labels and connection definition points come along; without a way the
  wire is squared as before.
- **Startup and the source of the project** (#129): *File › Settings › On startup*
  chooses what opens when the editor starts: the start page with the local files and
  the projects of the project server (*Show project manager*), the last project from
  where it came – a server project is opened on the project server again, asking for a
  sign-in if needed (*Reopen last project*, the default), the project list of the
  project server without the last project (*Open project server*) or always the same
  local file or server project. A project file opened from the file manager still
  opens instead. The status bar always says where the project is saved: *Local file:
  …*, *Project server: … / Project: …* (with *connection lost* while the server does
  not answer) or *Local recovery copy of server project: …* – a project restored from
  the last session or closed on the server is never shown as the shared project, and
  the tooltip of *Save* names the destination. If the project server of the start is
  not available, a dialog offers *Retry connection*, *Sign in…*, *Choose another
  project* and *Open local recovery copy*; unsaved changes of the copy are saved or
  discarded on purpose before the shared project replaces them.
- **EDZ parts without a usable macro** (#131): a part whose first function is a
  generic one (the connection points of a drive or an HMI) is placed as a black box
  with room for its connection points instead of an earth electrode. Elliptic arcs of
  EPLAN macros keep the ratio of their axes and start and end where the macro says (the
  S7-1200 CPU 6ES7212-1HG50-0XB0 had a half circle of 400 mm across the page), and
  texts keep their justification (table labels stood on their lines, right-aligned
  texts ran into their neighbours). Open polylines of EPLAN macros stay open (each edge
  of the CPU was closed back to its start), and the schematic no longer draws spikes
  where a line turns back on itself. The part selection of *Add part…* no longer grows
  to the height of the screen.
- **Wire ends in 3D:** the 3D view of the panel layout shows the end of each wire and
  cable core at its connection point: the ferrule (metal sleeve and collar in its
  DIN 46228-4 colour) where *Wire ends with ferrules* is set, otherwise the bare copper
  conductor as long as it is stripped.
- **Terminations per wire end:** each end of a wire or cable core gets its own
  termination: bare conductor, ferrule, twin ferrule, ring, fork or pin terminal, flat
  plug or flat receptacle (insulated in red, blue or yellow by the cross-section,
  uninsulated tubular cable lugs from 10 mm²). It is chosen in the properties of a
  wire (*Wire ends*) or a cable (*Cable ends*, for all cores at one device), or with
  the command `set_termination` for a connection point, a device or one wire;
  `terminations` lists them. Ends without one keep the ferrule setting of the layout.
  The 3D view draws every termination (rings round the connection point, flat plugs,
  forks, pins and insulated sleeves in their colour, each cable core with its own
  cross-section); the production wire list shows the termination and parts of both
  ends, and the wiring assistant, the machine exports, the harness data and the bill of
  materials (with the new catalog parts) take them too.
- **Twin ferrules for two wires:** a twin ferrule belongs to the two wires ending at
  one connection point. Chosen for one of them it is chosen for the other one too
  (offered only where exactly two wires end), it is shown once in 3D, counted once in
  the bill of materials, named with the other wire in the production wire list and the
  wiring assistant (which puts the two wires one after the other, to be crimped
  together) and carried by the first of them in the machine exports (new fields *Twin
  ferrule with (from)* / *(to)*). Two cross-sections get the ferrule for the larger
  one. The check *Twin ferrules* warns about one with a single wire, at a point with
  more than two wires or for two cross-sections. New layout setting *Twin ferrules where
  two wires share a connection point* (command `set_ferrules`).
- **Workspaces show their view again:** a workspace chosen in the title bar shows its
  main view (the panel layout, the workbench, …) even where the page was left in front
  of it, and the commands of the layout view (3D view, floor plan, mounting plate,
  tools, …) bring it to the front instead of changing it unseen behind the page.
- **Rotate while moving a selection:** right-click during a drag to rotate its electrical
  components 90° clockwise; `Shift`+right-click turns them counter-clockwise. The
  selection stays attached to the pointer, in both schematic editing and panel layout.
- **3D routing keeps a route apart from itself:** a cable, wire or pipe whose ends lie
  too close for its bend radius no longer runs back across or alongside its own way
  (the supply cable `=GRID-W1` of the example `building_services` crossed itself below
  the house connection box); it takes a loop that keeps its thickness from itself, its
  bends and the nozzle it comes out of, and is reported as not routable where there is
  none. Cables and pipes through one wall penetration pass it side by side instead of
  through one point. The example `building_services` has its cable entry lower in the
  wall, the water service sleeve in front of its building entry and the couplings of
  the compressed air outlets beside their units, so that every line has room for its
  bends.
- **One cable per cable gland:** a cable gland takes one cable (the free one whose
  clamping range fits, nearest to where the cable heads); a cable that finds every
  gland of its cabinet taken is not routed and reported as an error that names the
  cabinet, instead of being drawn through a gland with another cable. Cable entry
  flanges still take several cables side by side. Cable glands are drawn with a model
  of their own in the colour of their material: outside the hexagon body, neck and
  dome on the outer surface of their wall (they floated a centimetre above it), inside
  the locknut and the end of the thread, which other routes keep clear of; and the
  generic catalog gains them in brass nickel-plated (`GEN.GLAND-M16-MS` …
  `-M63-MS`), as EMC glands with a contact spring for the shield braid
  (`GEN.GLAND-M16-EMC` … `-M40-EMC`) and two LAPP SKINTOP glands (ST-M and MS-M
  20x1.5) as manufacturer parts. The example `building_services` has a gland for every
  cable, sized to it, and the conveyor (`conveyor`, `engineering_workflow`) one for its
  motor cable, which came in through the middle of the bottom.
- **Connection point designations** (#116): each designation of a selected symbol can
  be dragged on its own, put left, right, above or below its connection point or reset
  to automatic placement (Properties › *Property placement*); the placement is stored
  with the symbol and turns and mirrors with it, without changing the connection point.
  Placed automatically, a designation follows the lead of its connection point and
  moves along it, to the other side or out beside the wire to keep clear of the
  symbol's graphics, wires, nearby symbols and its other texts (the surge protective
  device's PE no longer runs into its arrow). Command `place_pin_label`.
- **Interruption points:** the name and the cross reference no longer crowd the arrow:
  horizontal points show the name above and the cross reference below the conductor,
  inside the page, vertical ones beside the arrow. Both can be put above, below, left
  or right of the arrow for
  all selected points at once, or dragged on their own without moving the symbol, and
  reset to the automatic placement; the places are saved and exported, the cross
  reference keeps its link. Command `place_interruption_texts`
  ([schematic](docs/features/schematic.md)).
- CAD drawing imports return original geometry bounds and their page origin, so
  external importers can recover drawing coordinates without changing page placement.

- **Device boxes with device connection points:** a dashed box of any size drawn on
  the schematic is a device of its own (power supply, measuring instrument, control
  module) without a library symbol; device connection points placed on its outline
  belong to its device, lead away from its edge and are numbered on. Each point is a
  separate connection (`-A1:L`, `-A1:+24V`) in the connection reports; designations,
  directions, the box of a point and the place and alignment of its designation are
  editable, resizing a box takes the points on a moved edge and their wires along, and
  moving, copying, deleting and macros take a box with its points. Commands
  `add_device_box`, `update_device_box`, `add_device_connections`,
  `update_device_connection`, `list_device_boxes`
  ([schematic](docs/features/schematic.md)).
- **Device box roles:** a device box with the tag of a device placed elsewhere can add
  connection points to it (*Extension*) or show only selected connection points of it
  on another page (*Pin view*). Wires at a view belong to the point it shows, reports
  list that connection once, and neither role counts as a second main function. Role
  on `add_device_box` and `update_device_box`, command `show_device_pins`
  ([schematic](docs/features/schematic.md)).

- **Cable cores at the cable definition line:** the stroke, core designation and cable
  tag of each core sit where a cable definition line of its cable crosses the wire, so
  they line up on wires of different lengths (30 % of the wire without a line). The core
  designation and the cable tag of a selected wire can be dragged into place; they keep
  their offset when the line moves, *Reset* in the wire properties places them
  automatically again. `update_wire` takes `core_offset` and `cable_offset`
  ([connections](docs/features/connections.md)).
- **Plot boundary:** the boundary of the plot is drawn in the site plan corner by corner
  and its corners are dragged afterwards; all buildings have to stay inside it, the check
  reports rooms, roofs and cabinets outside it and the site plan draws them red.
  Commands `plot`, `set_plot` ([panel layout](docs/features/panel-layout.md)).
- **Georeferenced reference point:** a point of the plant with its easting, northing,
  height and coordinate reference system and the direction of grid north; the site plan
  shows the coordinates under the pointer and a north arrow, `list_buildings` the
  coordinates of the buildings, IFC exports the plot's name, latitude and longitude.
  *Site › Measure* measures distance and direction between two points. Commands
  `set_reference_point`, `locate`, `measure`.
- **A level in 3D:** a level clicked in the navigator while the 3D view is shown shows
  all rooms of that level side by side at their places, with their cabinets, floor
  slabs and stairs.
- **Project properties:** the name and value fields of the custom properties use the
  width of the dialog instead of shrinking to a few characters; the dialog can be
  resized, and a long list of properties scrolls. A renamed property is applied when
  its name field loses focus (Esc keeps the old name), so the text cursor no longer
  jumps to another property while typing.
- **Wires in 3D:** a wire starts in the opening of its connection point and leaves it
  along the direction the point opens in (up or down out of the upper or lower edge of
  a device) before it comes down to the cable duct, instead of running straight back
  along the device to the mounting plate. The generated connection points of DIN rail
  devices lie in their end faces below the screws (40 % of the depth above the rail
  instead of 75 %, in front of the screws), and the generated models of DIN rail devices
  and contactors show these wire entries. NH fuse switch disconnectors on busbars are
  wired into the clamps in their terminal compartments (57.5 % of the depth, entered
  from the end faces below the screws) instead of in front of their terminal covers,
  and the strips for 185 mm busbars are drawn with their fuses one above the other and
  the connection compartment at the bottom. Devices drawn as boxes (parts without a 3D
  model) show the openings of their connection points.
- **Structure boxes:** *Insert › Structure box* draws a rectangle whose devices take other
  function (`=`) or location (`+`) identifiers than the page (also installation site and
  product); only the aspects set on the box override the page, nested boxes resolve to
  the innermost. Devices follow when their main function moves into or out of a box or
  the box is moved, resized or changed (in one undo step); aspects set on a device stay,
  and a tag that exists already is refused instead of merging two devices. Tags on the
  page omit what the box shows; the symbol properties show where each aspect comes from;
  the structure check reports overlapping boxes and devices inside a box with another
  structure. Commands `add_structure_box`,
  `update_structure_box`, `list_structure_boxes`
  ([schematic](docs/features/schematic.md#structure-identifiers-iec-81346)).

- **Tag blocks clear of neighbouring symbols:** a tag block placed automatically that
  would run into a neighbouring symbol goes on the other side of its symbol, above or
  below, whichever is clear of symbols and wires; the tag of functions of one device
  side by side in a row is shown once, at the first of them (the surge protective
  devices `-F1` of the transformer metering examples no longer overlap). Tag blocks
  placed by hand stay where they are.

- Preserve imported plot-frame artwork and live page fields as reusable frames, separate from selectable page content, including canvas, frame preview and exports.

- PLC addresses that are not byte.bit addresses, such as imported card positions like `A1.3.1`, are compared as whole texts, so different addresses are no longer reported as duplicates.
- Keep the closing window responsive while preparing recovery data, with progress for the previous autosave, project, parts database and macro library.
- Reuse Insert center symbol groups and search results while scrolling, refreshing them when definitions, parts, search text or language change.
- Keep the Insert center responsive while scrolling large libraries by drawing only visible list rows and previews and reusing unplaced-function results until the project or catalog changes.
- Add live function cross-reference fields with native page links, multiline attribute flow and a shared reference query. Authored tag fields can retain reference placement without freezing sheet names.
- Expose native symbol definition reconciliation to importers, preserving local revisions and reusing equal definitions.
- Continue new page numbering after imported alphabetic subpages such as 20a and 20.a, within each section.
- Preserve explicit cabinet rotations through front artwork, meshes, pins, wire exits and drilling; expose orientation editing through the editor and plugin commands.
- Distinguish plugin defaults from explicit false for optional boolean form fields.
- Renaming a placed cable marker follows its explicit cable reference and keeps
  ordinary devices with a shared tag independent. Unambiguous cable material-owner
  references follow renames and are cleared when the cable is removed.
- Placed wire labels read live core, cable, wire number or colour values, retain
  their layout through routing splits, joins, rotation and portable copy/paste,
  and can be edited in wire properties or the shared `update_wire` command.
- Cable properties in symbol attribute blocks and placed annotations follow the
  linked inventory entry, including custom marker captions. Placed part properties
  show the selected article consistently with attribute blocks and exports.
- Cable tag entry commits on Enter or leaving the field; empty, duplicate and
  invalid tags leave the inventory and drawing references intact. The editor and
  table command share validation, including released drawings.
- Explicit cable markers follow the current cable article through property,
  table and catalog edits, including when their displayed caption is customized.
- Cable renames keep explicitly linked drawing markers, connection definitions,
  cable definition lines and inactive options synchronized; shared ordinary
  device tags stay separate. Portable copies retain the cable association.
- Keep frame references to the previous, next and last sheet current within their section, including schemes whose numbering starts above one.
- Keep page and project property blocks live in drawing graphics, expose local sheet numbers and section page counts, and retain editable page defaults in reusable page templates. Offer page and project fields in the symbol editor's property picker.
- Advertise parallel cable-core support to external importers so a newer verification CLI cannot mask an older running editor.
- Resolve portable macro parameters in symbol graphics, pin labels and library defaults as well as placed instance data.
- Keep imported cable symbol article references current when choosing or typing a new cable part, while preserving ordinary device selections.
- Preserve explicit function article lookup when an ordinary device and a cable share an authored tag, without duplicating cable material.
- Retain distinct parallel cable cores between the same terminals in physical connections and regenerated cable reports.
- Preserve owned cable definitions, catalog parts and core assignments when copying circuits or inserting macros, with fresh consistent tags.
- Include imported project cable parts in the cable picker, preserve local catalog overrides, and expose the same available parts through the shared registry.
- Apply declared catalog cable diameters when selecting a part, return to the routing estimate when the new part has no valid diameter, and expose cable part application through the shared plugin command.
- Keep missing lengths of cable sold by metre as unknown material quantities and prices. Retain known partial totals and identify the cables that need a length through the shared project check.
- Preserve individual wire junction policies when inserting imported macros into existing drawings. Keep existing implicit contacts, trace imported physical routes, expose editable policies through the shared API and wire properties, and preserve different conductor properties during cleanup.
- Preserve device properties, material owners and literal extra quantities when inserting portable macros. Keep local counts when extending an existing device and reserve incoming tags before assigning collision replacements.
- Keep cabinet-only devices in the device and material lists independently of their extras. Count undrawn individual cabinet terminals once and keep their quantity stable when the corresponding schematic terminal is added.
- Preserve cabinet material ownership independently of mounting geometry. Exclude duplicate physical articles or whole devices from the BOM, retain editable material-only devices, and derive optional rail spacers from actual support height.
- Preserve literal additional-article quantities, including zero, without multiplying them by terminal positions. Keep primary material growth for newly drawn terminals, avoid counting physical cabinet articles twice and expose quantity modes and physical article bindings through the shared API.
- Retain explicit cabinet mounting origins and unnumbered single-part footprints without adding implicit plates, repeated terminal strips or accessory stacks. Keep open assembly routing free of invented walls and count explicit plate articles.
- Place individual terminals with their own parts and physical routing points while retaining whole-strip placement for existing projects.
- Represent open mounting assemblies and explicit mounting plates without invented enclosure geometry. Retain instance mounting datums, support vertical DIN rails, carry mounted descendants on moves and resize operations, and preserve mounting references in portable and pattern copies through the shared panel API.
- Convert every digital channel of complete PLC modules when changing the station address scheme, synchronizing linked views while retaining custom address text and signal names.
- Preserve PLC channel addresses when changing only hardware metadata. Offer explicit readdressing through the shared API and avoid inventing PROFINET ports for imported CPUs with no network data.
- Count every logical channel of complete PLC modules in part functions and reserve-function checks. Recognize analog part functions in hardware descriptions and exclude occupied module channels from the unplaced navigator even with custom addresses.
- Preserve card-relative PLC channel numbers across split electrical symbols and sheet reordering, including continued projects imported before explicit channel indices were stored.
- Link multiple PLC channel representations into one logical I/O entry, prefer electrical pins for navigation, and synchronize channel edits across views, including property edits and configuration import. Keep copies on other device tags independent.
- Edit the selected channel's address, signal name and comment in the PLC overview without overwriting its module or other channels. Keep row identity stable while addresses change and respect released pages.
- Preserve complete PLC module symbols with independent channels, physical pin mappings, editable addresses, signal names and comments. Configure and exchange their channels through the PLC table and shared API.
- Load external plugin folders from Manage plugins and remember them for later launches. Clear previous Find results after successful plugin commands.
- Clear cached Find results when opening or creating another project.
- Let wires and connection definitions be selected inside decorative symbol bounds.
- Trace conductor attributes through valid wire contacts and expose physical wire routes to plugins.
- Preserve editable conductor types in wire properties, connection tables, API and reports.
- Add editable, flowing symbol attribute blocks that resolve current function and part properties in drawings and exports, with library defaults for new placements.
- Select an assigned part independently for each device function, retain the selection when earlier parts are removed, and report removed selected parts. Expose live function manufacturers, selection text and batch attribute inspection to plugins.
- Keep explicitly placed function text linked to editable instance data, retain user-symbol function text defaults on placement, and avoid a duplicate automatic annotation.
- Preserve locally displayed product designation levels on imported symbols, and update them when the device is renamed. The symbol editor and plugins can choose the displayed level count.
- Keep directional imported ports and explicit wire junctions from shorting circuits at side crossings or attaching wires to intermediate device ports. Preserve authored wire endpoints and internal jumper rails, and use port directions when adding new automatic connections.
- Preserve physical wire targets through imported junction paths and drawn branches, and keep internal component bridges out of manufacturing connections. Junction paths remain editable in the symbol editor and through plugins; `list_connections` exposes the resulting pin-to-pin wiring.
- Let imported conductors retain separately placed annotations while keeping their editable wire and cable properties.
- Preserve editable fluid port functions on user symbols independently of their connection point numbers.
- Keep scoped interruption points electrically separate while preserving their displayed names and scoped cross references.
- Connect intermediate terminal attachments along continuous jumper rails, and expose their net membership to plugins.
- Keep decorative imported artwork and plot frames from blocking automatic connections and wire routing.
- Preserve device assignments and catalog parts in portable macros, and render their bundled symbol definitions in previews before insertion.
- Preserve transparent and palette PNG images in PDF output, and support source plot frames and component-name fields on imported pages.
- Keep custom symbol definitions with portable native macros, preserve local edits on insertion, and distinguish internal jumper attachments from physical terminals.
- Expose macro insertion and native net membership through plugins, and accept large CLI parameter objects from JSON files.
- Let imported parts select project symbol definitions across editor, CLI and MCP placement, and retain vendor source records with projects.
- Preserve embedded bitmaps in imported symbol libraries, including rotated and mirrored instances on the canvas and in SVG and PDF output.
- Preserve the connectivity domain of imported fluid and process symbols so their ports stay distinct from electrical connections.
- Keep imported property text angles and pin designation fields editable and linked to their placed symbol.
- Support imported secondary device functions and local junction paths without adding spurious main functions or device endpoints.

- **Accessories from macros:** a macro can list in `device_part_additions` the literal
  accessory articles of its devices to add when it extends an existing device; the other
  articles and the local quantities of that device stay as they are.

- **Reusing failed route searches:** with the routing rule `reuse_failed_searches` (off by
  default), a search that found no route is not repeated when later routes only fill the
  channels.

## [0.1.1] - 2026-11-08

- **PV arrays on roofs:** rows of solar modules on every roof shape, parallel to a slope
  on rails and roof hooks or on a flat roof tilted or east-west on ballasted mounting
  bases. The mounting material (rails, connectors, roof hooks, end and middle clamps,
  mounting bases, ballast, wind deflectors) follows from the modules and goes into the
  bill of materials with them; it comes from a mounting system of the parts database
  or from generic parts. New shipped catalogs `longi` and `jinko` with PV modules; the
  check reports arrays across a ridge or off the roof, over roof windows or each other.
  Commands `add_pv_array`, `set_pv_array` ([panel layout](docs/features/panel-layout.md)).
- **PV strings:** the modules of a PV array in series and in parallel as strings on the
  MPP trackers of inverters and charge controllers, with their highest, mean and lowest
  voltage, currents and power at the design cell temperatures; the report *PV strings*
  lists them, the check reports strings above the input voltage of their inverter or the
  system voltage of their modules, outside the MPP range, beyond the trackers or their
  currents. Commands `add_pv_string`, `set_pv_string`, `pv_strings`
  ([panel layout](docs/features/panel-layout.md)).
- **Doors and windows open either way or slide:** hinges at the left or right, opening
  into or out of the room, or a sliding leaf; drawn so in the floor plan and the wall
  view, the check keeps the way of sliding doors free. Command `set_wall_opening`,
  `opens` for `add_wall_opening`.
- **Site in the navigator:** the site at the root above the buildings shows the site
  plan of all buildings or the whole plant in 3D; a click on a room keeps the 3D view or
  shows its floor plan; new buildings and levels from the context menus.
- **Flush-mounted cables, wires and tubes in rooms:** each line is laid surface-mounted,
  flush-mounted in the walls or in the floor (a project default in the routing rules,
  set per line in its properties or with `set_laying`). Flush-mounted lines run in slots
  of a configurable depth along the installation zones, preset to those of DIN 18015-3
  and editable (`installation_zones`), or down into the floor and along its zone.
  *Show › Installation zones* shows the zones see-through, *Show › Open walls and
  floors* sees through walls and floor slabs to the lines in them
  ([panel layout](docs/features/panel-layout.md)).
- **Stairs and roofs of buildings:** stairs from level to level, straight, turned once
  or twice at quarter landings, with a half landing or spiral, their steps from the
  difference in height; flat, shed, gable, hip and pyramid roofs with roof windows. A
  roof lower than the ceilings below it slopes them, also on several levels: rooms get
  different heights at different places, their walls end under the slope, and the
  check reports cabinets and wall items above it. *Show › Whole building* shows a
  building in 3D with its floor slabs, stairs and roofs. Commands `add_stair`,
  `set_stair`, `add_roof`, `set_roof`, `add_roof_window`, `set_roof_window`
  ([panel layout](docs/features/panel-layout.md)).
- **Outlets, switches and heating circuit manifolds in rooms:** *Insert › Outlets,
  switches, heating* places socket outlets, data and antenna outlets, switches, push
  buttons and room thermostats at their mounting heights, heating circuit manifolds
  (2 - 12 circuits) and single-room control boxes; generic parts and the new shipped
  catalogs `gira` and `cosmo`, more KNX push buttons in `mdt`. Example 16
  (`office_building`) shows them with a stair and a roof.
- **Rooms onto levels in the navigator:** in *Enclosures and rooms* rooms are dragged
  onto a level of a building to stand on it; a selected level takes the rooms made with
  *New room*, and the context menu of a level offers *New room on this level*
  ([panel layout](docs/features/panel-layout.md)).
- **Updates in the editor:** when it starts, the editor looks for a new release on
  GitHub in the background, downloads it, checks it against `SHA256SUMS` and installs
  it: the Windows setup runs silently when the editor is closed (*Restart and update*
  in the status bar starts it again afterwards), an AppImage and the macOS apps are
  replaced at once; packages of a distribution are only told of the new version.
  Without a connection nothing waits. *File › Help › Check for updates* looks at any
  time; *Settings › Updates* and `OPENECRAFT_NO_UPDATE_CHECK` switch the check at start
  off ([installation](docs/installation.md#updates)).
- **Docker images of the servers:** the parts server and the project server are
  published as `ghcr.io/nilres/openecraft-parts-server` and
  `ghcr.io/nilres/openecraft-project-server` with every release (`<version>`, `latest`)
  and every push to master (`edge`); `installer/docker/compose.yaml` starts both
  ([installation](docs/installation.md)).
- **Directed T-junctions (target tracing):** *Insert › Connections › T-junction*
  places a junction where three wires meet that names the device the two wires of the
  connection start from and the order of their targets, with four directions and four
  variants each as in the connection symbols of EPLAN; the connection list, diagrams,
  panel wiring and harnesses follow it, connections without one keep the schematic
  order. The junction can be drawn as a dot without losing its routing, turns and
  mirrors with the drawing, is kept in the project file and the undo history, and is
  checked against its wires (*Connection symbols* check). Commands `add_t_junction`,
  `update_connection_symbol` and `list_connection_symbols` for the command line and the
  MCP server ([#96](https://github.com/nilres/OpenECraft/issues/96)).
- **Corners for automatic wiring:** *Insert › Connections › Corner* places a connection
  symbol with two arms at a right angle; with *Auto-connect* on, connection points that
  face the free arms of a corner or a T-junction are wired to it when the symbol or the
  device is placed or moved, so a corner at the meeting point of two connections wires
  both of them in. Command `add_corner` (with `autoconnect`), and `autoconnect` takes
  connection symbols too ([#96](https://github.com/nilres/OpenECraft/issues/96)).
- **Directed crossings and bridges:** *Insert › Connections › Crossing* connects the two
  wires crossing at its point (unconnected without one) and leads each end of its
  vertical or horizontal line straight on first and across second, in two variants;
  *Bridge* leads a connection from one side to the three others, the opposite side
  first. Both follow the connection symbols of EPLAN, can be drawn as a dot and keep
  their routing, and the check reports a connection its symbols wire in a loop. Commands
  `add_crossing` and `add_bridge` ([#96](https://github.com/nilres/OpenECraft/issues/96)).
- **Editor:** wires attached to moved items stay square: they slide along their rails,
  or get a jog where they cannot. Connection definition points and cable definition
  lines stay on their wires.
- **3D routing in cabinets without cable ducts:** their wires are no longer laid
  along the mounting plate (through the devices and each other) but routed in 3D from
  connection point to connection point, like the wires of a room: clear of everything
  by half their thickness, bent with their radius only, each on a way of its own. A
  wire that finds no such way is reported as an error instead of being drawn.
- **3D routing:** a route leaves a connection point that lies inside the box of its
  device straight through that box, never through another device to the room behind
  it; its last bend may lie where the straight way into
  its end begins; the straight way out of every connection point and cable entry stays
  free until its own route is laid; a cable between two cabinets that stand in no
  common room is routed to a cable entry in each of them; every cable takes an entry of
  its own; in the 3D view a cable passing from one cable duct into the next keeps under
  the cover of the first.
- **Bend radius in the 3D view:** ways laid side by side change lanes on a slanting
  piece of a stretch instead of a step at the corner (which was rounded with a radius
  of a millimetre or two); a route is drawn straight out of its connection points, its
  first corners moved onto that line, instead of with a kink onto the routing grid; a
  jog shorter than the bends of its corners is an S-curve of the bend radius also where
  the stretches before and after it lean; bends are drawn as fine arcs, so that ways
  round one corner are not pushed apart where only their drawn arcs met; a cable ends
  on its bend where its cores part right behind a corner.
- **3D routing, crossings:** routes that come together lie side by side, a route no
  longer runs across another one (only on its own way out of a connection point, or
  where it comes up to the device the other one ends at, and there it goes round the
  other one too where that costs little); the way out of a connection point is shared
  only by a route out of a point right beside it; of the wires of a cabinet as thick
  as each other the shortest is routed first, so that between two devices the inner
  conductors lie nearest to them and the outer ones go round them. Two wires that
  still cross right at a device are lifted over each other more sharply than their
  radius allows.
- **Cable entry flanges:** the cables through one flange go through its membrane side
  by side, each at a place of its own (the cable's thickness and a centimetre apart,
  the free place nearest to where the cable heads) instead of all through its middle;
  the cores of a cable part within the box of their ends, so that a core no longer
  swings out past its terminal into the bars it sits on.
- **Busbar holders in the 3D view:** drawn at the drilling positions of the plate (a
  holder 25 mm from each end and every 250 mm) instead of every 400 mm, and moved
  beside the devices that sit in the bars (current transformers the bars pass through)
  instead of through them. The bars, their holders and the boxes of busbar terminals
  and strip elements without a model were wound inside out: the view showed their
  far faces, which flickered against the holders behind the bars and the plate.
- **Messages of the 3D routing:** a message about a cable, hose or wire (not
  routable, bends too tight, frozen route lost, hose too short) points at a connection
  point of it: a double click in the messages pane jumps to the schematic. The 3D
  view marks a cable, tube or wire that could not be routed (and so is not drawn)
  with a red note at each of its ends, with the reason.
- **Busbar terminals in the 3D routing:** each terminal of a strip across the bars is
  a solid of its own: a wire to one of them goes round the others instead of through
  them (a cable whose cores end at several of them comes up through those). Terminals
  closer than 30 mm along the bars are entered from the front: their points facing a
  neighbour leave to the front, as a wire has no way out between them.
- **Cores at busbar terminals:** a cable ends clear of the device its cores go to,
  and the cores to terminals one above the other on the bars pass the lower terminals
  in front of them instead of through them. In example 13 the supply cable comes to
  its busbar terminals from below, through a gland in line with them, instead of
  looping round the field to come down onto them.
- **3D routing, ends:** a route comes into an end against the direction it leaves it
  in also where the way out is a single cell (it could run past the connection point
  and back into it); where the way straight out of an end is barred and the route
  starts from the cell beside it, the way out must be long enough for an S-bend of the
  bend radius onto that line; cable glands of the examples lie in line with the
  connection points they serve.
- **Busbar systems:** the systems of bayed enclosures that meet at their common side
  are one system (devices on it are connected by its bars in either enclosure); a
  section shorter than the system is wide lies along its bars; the connection points
  of repeated accessories (a current transformer behind each pole of a strip) are told
  apart by their number (`2S1`).
- **Parts:** current transformer sets with their connection points (primaries through
  from the bottom to the top, secondaries on a terminal block), the leads of the
  transformers behind NH strips.
- **Examples 13 and 14** rebuilt so that every conductor can be laid: power flows
  through the fields from one device into the next, busbar cross-sections run over
  bars (transformer sets in the busbars, the distribution busbar through both cabinets,
  coupling switches fed from busbar terminals), taps to small devices have their own
  cross-sections; the string fuses of the generator junction box of example 11 stand
  above their terminals.
- **Buildings:** buildings with levels and shafts in the plant; rooms stand on the
  levels and move with their building, shafts and links between buildings (cable
  trench, conduit, cable bridge between building entries) carry routes from room to
  room; devices go on the outside of the walls and on the roof; a Buildings pane with
  a site plan, the plugin `buildings` with its commands, a check and example 15.
- **Servers:** a very long project or manufacturer name no longer overflows the stack
  of the access check (wildcards of grants and report blocks are matched without
  recursion), and failed sign-ins with an empty user name are no longer counted twice
  against the client. Two sign-ins of one user at the same time on the parts server
  both keep their session, and a sign-in no longer brings back an account deleted or a
  password changed meanwhile. Replies to review comments on the project server keep
  the name of their author only while unchanged, so nobody writes under another name
  by putting a reply before theirs.
- **MCP server:** the project files of `batch_project_properties` (`files`) and the
  macro libraries of `generate_pages`, `update_macro_boxes` and
  `generate_preplanning_pages` (`macros`) must lie in `OPENECRAFT_MCP_ROOT` like all
  other files; folder ids of the vault tools are no longer taken for paths.
- **Parts server:** importing parts that exist already (CSV, BMEcat, EPLAN XML) keeps
  their data sheets, pictures and other files instead of removing them.
- **Project archives:** documents and images of the same name are numbered inside
  the `files` folder (`files/2 …`) instead of in a folder `2 files` of their own.
- Use the DIN-rail clip position from imported mounting data when placing cabinet devices. Matching Beckhoff terminal housings and the EK1100 use the documented 63.8 mm rail datum.
- Snap cabinet devices flush to a nearby neighbor on the mounting plate while placing or moving a single module. The placement preview shows the snapped position.
- Show imported EPLAN artwork on cabinet fronts. An assigned front view takes priority, followed by a mounting representation and then a fitted overview illustration.
- Place successive imported PLC channels from a single card entry in the Insert center; when a card fills up, placement continues on a new device.
- Link PLC channels placed from the symbol library to their uniquely matching catalog article, so the physical card appears once in the cabinet's devices-to-place list. Opening an existing project or reimporting the article also recovers unassigned cards.
- Import Beckhoff analog signal and reference terminals as one PLC channel with a symbolic name such as U1, keeping both wire endpoints and the source data type.
- Import single-channel EPLAN digital I/O representations as native PLC points with editable addresses, physical terminal designations, and one parts-list entry per card.
- Reuse unchanged symbol drawings during part dragging and schematic rendering, including large imported EPLAN libraries.
- Import embedded 2D graphics from EPLAN window macros into the project symbol library. EDZ parts carry their converted variants and per-variant diagnostics through the parts server.
- Import additional SPECIAL contact forms and verified reciprocal potential transfers. Report unresolved source objects and their terminal designations per representation. Explicit reimport preserves placed wire endpoints and instance pin labels when compatible contacts are added.

## [0.1.0] - 2026-11-04

The first release: an electrical CAE editor for Windows, Linux, macOS and the browser,
with a command line, an MCP server, a parts server and a project server.

- **Schematics:** device-oriented pages with live cross references and contact images,
  structure identifiers, page sections, macros with variants and a configurator;
  potentials, cables with cores and automatic wire, device and page numbering.
- **Symbols:** about 330 IEC 60617 symbols, NFPA / JIC, GB, GOST and JIS sets, fluid
  power (ISO 1219), P&ID, installation plans and single-line; a symbol editor.
- **Terminals, plugs, PLC:** multi-level terminals with accessories, plugs, PLC tag
  tables for TIA Portal, Studio 5000, TwinCAT, CODESYS and more, marker export.
- **Connectors:** the plug window shows the pin-out of a plug on the mating faces of
  both sides, coloured by the wires and cable cores at the pins (also as SVG export);
  connector parts for M5, M8, M12 (signal, fieldbus, Ethernet and power codings),
  M16, M23, M40, 7/8", GX aviation connectors, valve connectors, D-sub, DIN 41612,
  IDC, wire-to-board housings, pluggable terminal blocks, RJ11 / RJ12 / RJ45, BNC,
  heavy duty inserts, sealed connectors for mobile machines, CEE, appliance couplers,
  mains, installation, PV and battery connectors; pin layouts with inner rings,
  column numbering and free pin positions; ribbon and flat cables with a check of the
  cores on IDC connectors; example project 12 shows them.
- **Reports and checks:** live report pages, graphical terminal, cable and plug
  diagrams, drawn forms, a project check with message management and a check for
  open wire ends.
- **Editor:** an EPLAN-style ribbon that folds groups into menus when space is short,
  dockable panes per workspace with standard workspaces to return to their default
  arrangement, an Insert center, a properties pane, key tips,
  command search; unsaved changes are kept and asked for on every way out; released
  and locked pages are read only but selectable; crossing selection, zoom commands,
  contextual help (F1) and rebindable keys for every page action; pages and sections
  are dragged in the Pages navigator with a drop marker and the resulting name shown.
- **Panel layout:** devices at real size in 2D and 3D, virtual wiring with duct fill,
  drilling and NC data, enclosure climate, line-ups, rooms and 3D routing; wiring and
  mounting assistants for the workshop. In a room's floor plan cabinets turn in place,
  cabinets in a bad place are drawn in the warning colour with the reason on hover,
  inner walls end on the room walls, and the layout checks are shown below the view;
  rooms and enclosures with content are deleted only after a question. Opening a project
  analyses it in the background with a window showing the step and the route it is
  at; the 3D routing searches several times faster and is kept while an edit changes
  none of its routes; after an edit that does, it is searched in the background (the
  status bar shows it, the 3D view keeps the earlier routes until then), so editing
  a project with rooms no longer stalls the editor for seconds. Exports, reading 3D
  model and parts files (STEP, IGES, Parasolid, EDZ, …) and the autosave run on another
  thread too; the status bar shows what is running. Pipes and cables in a room keep to
  its walls and ceiling instead of crossing it, and their bends keep the bend radius
  apart. The 3D view is drawn by the graphics card (OpenGL 3 / WebGL 2, with a depth
  buffer and anti-aliasing; Settings › Appearance switches it off), and drawn by the
  processor it keeps what it shows between frames: large rooms turn smoothly. Moving in the
  3D view: the wheel zooms at the pointer and walks on into a room, Shift+drag pans as the
  right button does, a double-click turns the camera to what was clicked and closes in,
  W A S D / the arrow keys and Q E move through it (Shift: faster), and a bar in the view
  has the standard views (front, left, right, rear, top, isometric) and *Fit* (`Home`). Devices without a 3D
  model of their own get a detailed, coloured one by what they are (pumps, vessels,
  valves, heat pumps, inverters, rack batteries, …); devices of the bundled catalogs
  (Victron, Pylontech, Hager, DEHN, Beckhoff, Phoenix Contact, Siemens) are drawn after
  their product photos and wired to their real terminals: the Pylontech batteries
  to the power terminals and ports on their front, the Victron devices from below,
  the Cerbo GX at its row of connectors, Beckhoff terminals at their spring contacts,
  surge arresters with the earth at the bottom, 19-inch devices at the rear; DIN rail
  models have a screw at every connection point; terminal blocks are drawn slice by
  slice with their stepped profile, wire entries, pushers or screws and marker fields
  (generic terminals, end covers and end brackets sized after Phoenix Contact PT, PTPOWER,
  D-ST, D-PT and CLIPFIX), between end plates and end brackets at both ends of every strip, and cable ducts with slotted walls and their
  cover; cables in the 3D routing end at the
  connection points their cores land on and part into their cores there; routes leave the devices they end at
  instead of running through them; the routes of a class are searched on several threads
  at once (with the same result), a selected cable or tube shows its properties like a
  wire with its cores and the connection points they join (picked in the 3D view and on the plate); a cabinet opened from its room is fitted into the view. The 3D view hides the
  walls in front of the camera or the
  ceiling on request; the opacity of the walls is a setting. Wires are drawn in their
  colours and, like cables and tubes, in their real thickness as round tubes bent round
  their corners with their bend radius, from the plate up to their
  connection points; cable cores are drawn with their cable. Routes keep half their
  thickness from what is around them and turn where their bends have room, cables keep
  out of cable ducts too small for them; View › Open cable ducts leaves out the covers
  of the ducts. Wires, cables and tubes that run together lie side by side, each on a lane of its own
  (in a cable duct packed in rows from its base, never over its cover), and where wires and cables pass
  through the wall of a duct its fingers are left out; where they would still run inside or through each
  other they are pushed apart, one over the other where they cross, inside the walls of their cabinet and
  out of its devices. Wires on one terminal or on terminals closer than they are thick leave them
  straight, side by side, instead of hooking round each other where they come out of a duct. The 3D
  view lays them out on another thread, several times faster, with a progress box while it does (it
  shows them as routed until then). Cables and tubes keep apart from each other in free air already when they are
  routed. A cable duct too small for its wires to lie side by side is an error of the project check.
  The fill level of a cable duct counts the cables and tubes routed through it as well as its wires,
  against the cross section inside the duct, also for the ducts on the walls, ceiling and floor of a
  room; cables and tubes keep out of the stretches of a duct they would fill above the limit.
  A cable ends a little before the device and its cores part from there in smooth curves, each coming
  into its connection point from the side its wires leave it; cables and wires run on through the cable
  glands, wall penetrations and cable passages between cabinets and rooms in one piece, bent round
  there; pipes and hoses end at the nozzles of the devices' models (suction and pressure nozzle of a
  pump, the nozzles of a vessel, both ends of a valve), coming straight out of them; a valve, meter or
  check valve in a pipe is taken as mounted with its inlet towards the pipe that feeds it. Cables
  through one gland pass it side by side; the cores of a cable are as thick as its diameter tells and
  part from their places round its axis. Wires in a cabinet without cable ducts leave their devices
  in the direction of their connection points and run round the rows of devices instead of behind or
  through them; wires leave their own devices over them, not through them. Wires, cables and pipes
  laid side by side are pushed at most a few times their thickness from their routes.
  The 3D routing measures the room around a route from the devices, walls, mounting
  plate, door and room walls themselves (thin busbars and rails between the cells of its
  grid included), so cables and pipes keep half their thickness from all of them, and
  from each other, everywhere: a route that cannot is not routed, and the project check
  reports it as an error that says to move its ends or make room for it. A route leaves
  its device, cable gland, nozzle or wall penetration straight on for its bend radius,
  runs two radii between two bends (the arcs of both fit between), turns only where its
  arc keeps clear of everything too, changes lanes with an S-bend (checked and drawn as
  one), and near its ends converges with the routes to the same end; the thickest cables
  of a class are routed first. In a cabinet a cable enters and leaves a cable duct through
  the slots of its walls, never through its base or its cover, lies in a duct over the rim
  of its walls and under its cover, and keeps half its thickness off the ducts it runs
  along outside them; in a 19-inch rack cables run along the space beside the mounting
  rails and leave it for their devices; in a room, running through the open space costs
  the more the farther from its surfaces, so cables and pipes keep close to the ceiling
  and the walls. The 3D view holds wires and cables in that band of a duct where they
  cross its walls (and leaves out the fingers they pass), cuts a duct's walls where
  another duct joins it, and brings a wire down before the next duct instead of into it
  through its cover, down beside the hinge of the door before the duct along the plate's
  edge, and straight through a cable passage for its bend radius on both sides. A cable
  takes the nearest cable gland whose clamping range fits it and that no other cable
  took, the thickest cables first, and a route whose way into its end runs beside it
  changes lanes into it with an S-bend. A connection takes the cross-section of a wire
  drawn with one at its connection point (a tap off a busbar) rather than the busbar's.
  Every wire, cable and tube has a least bend radius, a multiple of its thickness from
  the routing rules (wires and cables four times it, as laid fixed; hoses five times it
  or their own; pipes one and a half times it, a long-radius elbow), shown in the route
  and wire properties: the 3D view bends every corner with it, and corners too close for
  separate arcs as one smooth curve no tighter than it where the stretches around
  allow; wires through one cable passage pass it side by side, and two wires on one
  terminal leave it side by side. The wires of a room are routed in 3D between the
  devices on its walls (a room has no mounting plate to route them along); wires routed
  in 3D keep apart from the cables and from each other like cables do, outside the ducts
  and routing paths; cables and tubes are routed before the wires of a class; and a
  route comes up to another near its end only where that route is near one of its ends
  too (cables converging on one gland or terminal), not across its run. The 3D view
  keeps the wires and cables out of the walls, bases and covers of the cable ducts
  (crossing the walls where they are held above the rim), lays the cores of a cable
  where they part as they are and the rest clear of them, joins the legs of a route at
  a cable gland with the corner before it in line instead of a slanted run, and lets
  the cables of a rack run behind its 19-inch panels. A test of the editor checks that
  nothing drawn in the 3D views of the example projects clips anything: other wires,
  cables and pipes (beyond the overlap the layout leaves where they are packed: a
  millimetre and a half between wires, two and a half millimetres where a cable or pipe
  is involved), the ducts with their covers, the devices, the walls, plates and doors of
  the cabinets and the walls, floors and ceilings of the rooms. In the building example
  the incoming terminals of the main distribution feed the busbar they are mounted on,
  its feeder ducts are deeper, and the shunt sits between the first two Lynx
  distributors on the technical room's wall. A device standing on the floor of a room
  without connection points of its own is connected on top, where its terminal box is.
  Devices mounted on a busbar system are connected by its bars instead of wires. The title bar can show
  the frame time and the frames per second (Settings › Appearance). A placed device is shown in the
  panel layout or the 3D view from its context menu in the schematic. Pumps, tanks
  and other parts marked for floor mounting stand on the floor of a room; a pump or
  compressor without its own motor names its motor (property "Driven by") and is drawn
  coupled to it: through a shaft coupling (or a gear unit) in line, through V-belts (or a
  gear case) side by side, the motor turned towards it and raised on a riser. Pipe runs
  go on its walls, ceiling and floor, and hoses, pipes and cables are routed in 3D
  through the room and from its cabinets through their cable glands. Wall
  penetrations join neighbouring rooms: routes run from room to room through them
  (also through the rooms between), and the floor plan shows the rooms around the one
  shown, opened with a double-click. **19-inch racks** (42 U in 2000 mm) have mounting
  rails instead of a plate; switches, patch panels, servers, UPS and blank panels snap
  onto whole rack units, with a rack elevation report and the commands `add_rack`,
  `mount_in_rack` and `list_racks`. **Building entries** in outer walls bring the
  supply cable and the water main into the building: cables and pipes to devices
  outside the building are routed to them. Of several wall penetrations into the same
  room a route takes the nearest; cables and pipes ending at the same device (a heat
  pump) may meet there, also when the device is in a cabinet standing in the room:
  the cable comes in through the building entry and on through the cabinet's gland.
  Cabinets hanging on a wall are entered through the glands in their bottom too: a
  shaft below each gland, open to the front, stays free for the routes. A new enclosure system for **house connection
  boxes** (glass-fibre reinforced polyester with a hinged cover); in the building
  example the house connection box is such an enclosure above the cable entry, with
  the NH fuse base of the utility (fed from below) and the PEN terminal block inside,
  and a main earthing busbar beside it joins the PE busbar of the main distribution
  (main bonding conductor) and the foundation earth electrode, whose stainless steel
  lug comes out of the wall below it; the earth electrode is a new symbol.
- **Engineering:** conductor sizing (`size_wires` gives the wires behind protective
  devices the cross-section they need; fuse switch disconnectors protect at the rating
  of their fuse links), short circuit and selectivity, load calculation,
  EMC, ISO 13849-1 safety functions; wire harnesses with KBL (their wires stay assigned when pages or symbols move), fluid
  power, pre-planning. Fluid power and P&ID symbols for air receivers, dryers,
  lubricators, quick couplings, heat pumps and buffer tanks, the media hot water and
  heating water; symbols for energy meters, PV generators, hybrid inverters, inverter /
  chargers, valve actuators and motor-operated valves. A device drawn pole by pole (fuse
  holders, surge arresters, fuse blocks) may have as many main functions as its parts
  provide. Symbols for the motor operator of a circuit breaker and the voltage and
  frequency relay of a grid and system protection (VDE-AR-N 4105). The voltage drop of
  a cable takes the rated current of the part providing the function it feeds (the
  motor operator, not the breaker it sits on). A fuse switch disconnector with solid
  links isolates only: no protection, no selectivity. Cable entry flanges are ways in
  for the 3D routing like cable glands. The check `busbars` finds busbar systems whose
  devices join more conductors than they have bars (a short circuit); the 3D view and
  the 3D exports draw each bar of a busbar system on its own, at the part's centre
  distance, on holders reaching just across the bars (their own solids in the 3D
  exports). A busbar terminal strip across the bars puts each terminal on its own bar.
- **Revisions and review:** released pages, approval workflow, baselines with compare
  and merge, comments and redlining.
- **Parts:** a parts database with mounting data and 3D models (STEP, IGES, Parasolid,
  JT, IFC), BMEcat, EPLAN XML and EDZ import; a parts server with a data portal that
  shows the 2D front view and the 3D model of a part; the example parts, mounting
  material included, come with both; the manufacturer catalogs of the parts server
  (ABB, Beckhoff, DEHN, Hager with NH fuse switch disconnectors and energy meters,
  MDT, Phoenix Contact, Pylontech, Siemens, Victron, Weidmüller) also feed the example
  projects. Parts are dragged from parts management or the
  data portal onto a schematic page, imported into the parts database on the way when
  needed, and placed as a device with the part. Accessories that only work with other
  parts (auxiliary and signal contacts, contact blocks) know the parts they fit from
  either side, so one accessory serves many parts (the Hager CZ001 signal contact fits
  the residual current devices CDA440D and CCA216D): the data portal and the parts
  server show both directions, the device's Parts tab, the command line and the MCP
  server offer the accessories that fit its parts, the project check reports an
  accessory without such a part, and the panel layout and the 3D view mount them
  beside or in front of their part with their own terminals. CSV, BMEcat and EPLAN XML
  carry them; the building services example signals a tripped residual current
  device through its CZ001. An accessory can also be a base the part stands on (the
  univers N component module under an NH fuse switch disconnector: the device gets
  its footprint and depth) or be built into the part (the undervoltage release and
  the auxiliary switch of a circuit breaker: their terminals are on the breaker, and
  their coils count as functions of the accessory, not as a second breaker). An
  enclosure part delivered with its plinth (`Plinth: 200`) gets it without a plinth
  part of its own. The Hager catalog adds the parts of a transformer-rated metering
  with PV: the univers cabinet FR25S2 with lifting eyes, component modules for the
  connection point zone, NH1 switches and transformers, LT153, HA354, SPA811Z, L73M,
  NBN116, MCN306, the h3+ breaker HNT161GR with motor operator, undervoltage release
  and auxiliary switch, the NA protection relay EU400, HTG911H, EZM100 and the meter
  cabinet FP62ZG2; example project 13 builds the distribution cabinet of an apartment
  building with them. Several of one base part lie side by side along the part (the
  current transformers clipped in behind the poles of an NH fuse switch disconnector
  strip). The Hager catalog adds the LVSG strips NH00 - NH3 for 185 mm busbar systems
  with the LVZW150 transformers and LVZVA-V aluminium terminals, the busbar terminal
  KS30C for Al cables, h3+ P630 HNW401DR with motor operator HXW042H, the residual
  current circuit breaker CDB640E (type B), the cabinet FR24H2 and the cable entry
  flange FZ402; new catalogs `janitza` (UMG 604-PRO) and `loxone` (energy meter for
  1 / 5 A transformers, Tree); example project 14 builds the main distribution of an
  industrial plant with them.
- **Data exchange:** PDF, DXF / DWG, SVG; the panel as glTF, STEP, IGES, IFC, JT and
  Parasolid; ERP / PDM / PLM connectors and Autodesk Vault.
- **Teams and automation:** a project server with page locks, roles and Active
  Directory / LDAP sign-in; a command line, an MCP server for AI agents and plugins.
- **Security:** the servers check the token before they read a request's body, send
  security headers with every answer, end sessions 30 days after the sign-in (sessions
  of earlier versions end at once), count failed sign-ins per name and client address
  so that nobody locks others out, and do not tell which user names exist. The data
  portal page runs only its own script: files that can run scripts (SVG, XML) are
  downloaded instead of shown and only web addresses become datasheet links. A
  manufacturer user can no longer put parts into the catalog of another manufacturer.
  Importing XML nested thousands of levels deep no longer crashes the parts server or
  the editor, and archives unpacking to more than 1 GB are refused. On a project
  server without token and accounts only its own machine creates users; reviewers
  approve and comment in their own name only; managers of a project cannot lift the
  denials of the administrators; anonymous callers cannot work under the name of an
  account. Links of texts are opened and exported only for web and e-mail addresses.
  The Vault and EPLAN clients send their token to their own server only, the MCP server
  keeps its files in one folder (`OPENECRAFT_MCP_ROOT`) and plugin programs are
  stopped after 5 minutes without answer.
- **Languages:** English, German, French, Spanish and 16 more.
- **Packages:** Windows setup, deb, rpm, AppImage, Flatpak, snap, tar.gz, macOS disk
  image and the web version. Every package, the web version included, carries the
  license texts of OpenECraft and of the libraries and fonts it contains
  (`THIRD-PARTY-LICENSES.html`); release files come with `SHA256SUMS`. The EPLAN Data
  Portal is read with the personal access token of the EPLAN account only.
