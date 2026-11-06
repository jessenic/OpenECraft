# Changelog

The notable changes of each OpenECraft release. The section of a version is the text of
its GitHub release ([docs/releasing.md](docs/releasing.md)).

## [0.1.1] - unreleased

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
