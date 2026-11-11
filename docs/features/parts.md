# Parts management

The parts database shared by all projects (EPLAN style): commercial, technical and
mounting data, function templates, connection points, 2D / 3D graphics and the import of
manufacturer data. Parts are shared between installations through the
[parts server](../parts-server.md).

## Parts database

A global **parts database** shared by all projects (product top group / group /
subgroup tree, filter), with general, commercial (price, price unit, packaging, quantity
unit), technical, mounting (dimensions, weight) data, accessories and a **function
template** per part (e.g. coil A1/A2, main contacts, 13/14 NO). The database is stored
in the app storage (desktop) / local storage (browser).

- **Accessories** that only work with other parts (auxiliary and signal contacts,
  contact blocks, tab *Accessories*): a part lists its accessories, an accessory can
  name the parts it fits (*Fits these parts*) – either side makes the pair, so one
  accessory serves many parts and a manufacturer's data can add one without changing
  its main parts (the Hager CZ001 signal contact names the RCCB CCA216D, the CDA440D
  names the CZ001). An accessory is flagged as such and says where it is mounted: on
  the front (stacked in depth), on the left or right side (beside the part on the rail),
  under the part as the base it stands on (the univers N component module under the
  Hager LT153 NH1 fuse switch disconnector: the device takes the module's footprint,
  the part sits centred in front of it), built into the part (the undervoltage release
  and the auxiliary switch in the slots of the h3+ circuit breaker HNT161GR: no room of
  their own, their terminals are on the breaker, and their coils drawn beside the
  breaker are functions of the accessory, not a second main function) or not at all.
  Several of one base part lie side by side along the longer side of the part (the
  Hager LVZW150 current transformers clipped in behind the three poles of an LVSG NH
  fuse switch disconnector strip), different base parts on each other. In a project the device's *Parts* tab offers the accessories that fit
  its parts (*Add accessory*) and shows the part each one sits on; its functions add to
  the device's contacts; the check `accessories` reports an accessory on a device
  without a part it fits; the panel layout and the 3D view draw it at its place with
  its own terminals, and the device gets wider or deeper. Command line / MCP:
  `list_accessories`, `add_accessory`; parts server: `GET /parts/{number}/accessories`;
  CSV columns `Accessory`, `Accessory mounting`, `Fits`; BMEcat references of the kinds
  `accessories` and `base_product`; EPLAN XML `P_ARTICLE_ISACCESSORY`.
- **Assemblies**: a part can consist of component parts with quantities (tab
  *Accessories*, CSV column `Components`: `NUM:2|NUM2`); the bill of materials lists
  them under the assembly (nested assemblies resolved, priced as included).
- **Typed mounting data** per part (tab *Mounting*): clearances (top, bottom, left,
  right, front, EMC, service space), mounting holes with thread, door and face cutouts,
  power loss, how far an accessory sticks out, fastening, faces, collision class and
  terminal type. They drive the panel checks, drilling, cutouts and the climate
  calculation; CSV has a column per value (`Power loss`, `Mounting holes`, …). Older
  parts with these values as `Key: value` lines in the technical data still work;
  imports and the built-in catalog move such lines into the typed fields, the part
  editor offers it for existing parts.
- **Connection points** per part (designation, position in the front view, the
  direction the wire leaves); parts without their own get them from the function
  templates by the IEC convention (odd designations at the top, even at the bottom, door
  devices from the rear, PLC modules at the bottom, surge arresters with the earth and
  the remote signalling contact at the bottom). The devices of the bundled catalogs
  that are not wired this way (Victron, Pylontech, Beckhoff, Phoenix Contact power
  supplies) and the 19-inch devices (power inlet at the rear) have their own; generated
  models draw their terminals at the connection points. Edited in the part editor (tab
  *Connection points*: table, click into the front view to place one); the panel layout
  wires to them; CSV column `Connection points`: `A1:22.5/0/55/up|A2:…`.

## Graphics and 3D models

2D front view (SVG) and 3D model per part:

- **STL**, **OBJ**;
- **STEP**: solids and surface models of AP203 / AP214 / AP242 files, placed by their
  assembly structure, tessellated and converted to mm;
- **IGES 5.3**: solids and trimmed surfaces;
- **Parasolid** (`.x_t` / `.x_b`): bodies of V13 and newer transmit files, planes,
  cylinders, cones, spheres, tori and B-surfaces tessellated exactly, blends and other
  procedural faces filled from their boundary;
- **IFC**: tessellated, faceted and extruded bodies of all products, placed and
  converted to mm;
- **JT**: the tri-strip meshes of JT 8 files.

Parts with dimensions but without a model get a generated one by what they are: DIN
rail devices with their terminals, toggle or display, terminals in their colours (PE
green, N blue), PLC modules, fuse switch disconnectors, wall-mounted electronics with
display, cable glands (hexagon body, neck and dome, in nickel-plated brass or polyamide
by their material and colour), 19-inch devices and rack batteries, PV modules, pumps, motors,
vessels, heat pumps, compressors, refrigeration dryers, valves with lever or actuator,
pipe fittings, manifolds and air treatment units. Devices of the bundled catalogs
are drawn after their product photos: Victron inverters with their black corner caps
and orange band, the Cerbo GX and SmartShunt, the Lynx distributor, Pylontech rack
batteries with handles and terminals, Hager fuse switch disconnectors, meters and
modular devices, DEHN arresters with their plug-in modules, Beckhoff EtherCAT
terminals, Phoenix Contact power supplies, Siemens motor protection switches and
contactors. Generated models are painted (a colour
per triangle, `colors` of the mesh); imported ones take the colour the 3D view gives
the device. Floor-standing parts stand upright on the floor of a room.

## Parts in the project

- **Project parts**: parts are copied into the project when assigned; synchronize
  project ← database, transfer project → database, remove unused project parts.
- **Part selection** dialog with *device selection* (only parts providing the device's
  main function); several parts per device with quantity (contactor + aux block).
- Function templates drive **reserve contacts** in the contact image, "Apply
  designations" (connection point numbers from the part) and the project check (more
  functions placed than the parts provide, parts missing in the project).

## Import and export

- **CSV** (`;` separated, header names) and **JSON**, import and export.
- **BMEcat** catalogs (1.2 / 2005, as published by manufacturers and wholesalers, e.g.
  from the EPLAN Data Portal or ETIM / eCl@ss data pools): identification, description,
  price, data sheet, the ETIM / eCl@ss class and every feature as technical data;
  dimensions, weight, rated current / voltage and power loss go into their fields (by
  feature name, or the ETIM codes of width, height and depth).
- **EPLAN parts XML**: the parts management export of EPLAN (`<part>` with
  `P_ARTICLE_*` attributes and variants, multi-language texts, product groups by their
  exchange numbers) is imported like the other formats, and parts export as EPLAN parts
  XML for the way back (parts management, `parts_export --format eplan_xml`). Function
  templates become function templates (PLC inputs and outputs as PLC functions),
  connection point definitions connection points (connection points on plugs as
  `X10.1`), drilling patterns mounting holes.
- **EDZ** (EPLAN Data Portal archives, 7-Zip): the parts XML inside is read as above,
  together with clearances, power loss and the datasheet link. Embedded 2D window
  macros become project symbols. Parts Manager and `parts_import` also store the
  original files as attachments. The project and server part records carry the
  converted definitions and diagnostics.

### Placing an EPLAN symbol

Open **Import parts** and select the EDZ or the vendor's article XML. On desktop, article XML can load a companion EMA from the same directory. Under **Insert center → Devices**, search for the part, select it, then select the variant before placing it. Each row has a preview and lists unresolved properties or library references. A loose EMA can also be opened using **Import symbol library**. Explicitly tagged multi-device windows can be placed as native macros.

The decoder reads PXF window macros, embedded drawing blocks, affine transforms, basic vector graphics, text and selected property fields. Its built-in SPECIAL mappings cover PLC and device contacts 13, 17 and 20, plug and bus contact forms 34, 35 and 37, two-sided form 22 with a common designation, and overview form 40. These forms use the contact drawings in the 2007 EPLAN IEC symbol overview, pp. 1–2. Physical XYZ connection points continue to belong to the panel model.

The importer reads local pin-slot and quoted plug/designation potential-transfer references. Reciprocal, unambiguous references between mapped static contacts on the same device create native bridges. One-way rules, missing targets, uncertain device identities and unsupported device behavior remain diagnostics. The importer keeps autoconnect breaks, cable/shield metadata, and connection definition points separately from wire segments. The source XML does not necessarily contain a routed source-to-target wire list. [EPLAN connection point logic](https://www.eplan.help/en-us/Infoportal/Content/Plattform/2022/Content/htm/xfctdefbrowsergui_d_anschlusslogik.htm)

Expand **Unresolved placements** for a part variant to view the master, source object, plug and terminal designations. For a file or a whole vendor folder, `cargo run -p openecraft-edz --example inspect -- PATH --references` streams JSON lines with native pins, outstanding references, connection controls and transfer status per representation. Coordinates on unresolved placements are insertion points. A verified native definition with key `user:eplan-lib:LIBRARY:NUMBER:VARIANT` supplies explicit pin anchors. Unregistered PDF artwork can be kept under `user:pdf-review:LIBRARY:NUMBER` for comparison and does not participate in mapping.

Partial variants can be inspected and placed. Missing master symbols, unsupported properties, placeholder states and other unhandled objects are reported. Embedded layer settings and explicit pen widths and predefined colors carry over. Custom palettes, exact dash patterns and fills can differ from EPLAN. Automatic reconstruction of a complete EPLAN project's wiring and 3D model conversion are outside the 2D decoder. Reimport refreshes unedited graphics with compatible endpoints. Local edits and existing pin geometries take precedence over a conflicting revision.
