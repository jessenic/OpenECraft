# Plugins

OpenECraft is extended through the `openecraft-api` crate. A plugin adds

| Extension | What it is | Editor | Command line / MCP |
| --- | --- | --- | --- |
| Command | function on the project with typed parameters, returns JSON | *Plugin commands* pane (*Plugins* tab: every command by plugin with a search; favorites, recently used commands and a menu per external plugin on the tab; a form for the parameters) and the command search | a command / tool of its own |
| Check | adds issues to the project check | *Messages* pane | `check` |
| Report | table generated from the project | *Reports* pane, report pages and report blocks | `report --kind <name>` |
| Exporter | file generated from the project | *File › Export* | `export --format <name>` |

Names are snake_case and unique among all plugins; commands must not take the name of
a built-in tool. Changes a command makes to the project are one undo step.
`openecraft-cli plugins` lists the loaded plugins, *Tools › Manage plugins* in the
editor enables and disables them (remembered by the editor).

In *Plugins › Manage plugins*, enter a plugin folder containing `plugin.json`, or
a parent folder whose subfolders contain plugins, and click *Load plugins*. The
editor loads them for the current session and remembers usable folders for later
launches. A failed sibling plugin does not prevent remembering plugins that loaded.
`OPENECRAFT_PLUGINS` remains available for environment-based configuration.

## Rust plugins

Built-in plugins live in `crates/openecraft-api/src/builtin.rs` and its submodules and
are compiled into every front end (also the web version): the project checks, reports
and exports of `openecraft-core`, project statistics, panel layout checks, panel
manufacturing, part lifecycle, and one plugin per area of project commands (pages,
sheets, drawing, devices, plugs, PLC, potentials, process engineering, single-line
diagrams, tables, engineering, electrical engineering, panel layout, rooms, buildings, 19-inch racks, routing, field
wiring, wiring, harnesses, integration, languages, structure, properties, configurator, report
blocks, project management). An area lists its commands in a `COMMANDS` table of
`Spec`s (`builtin/<area>.rs`); the command line and the MCP server offer them as tools
next to the session tools of `cli/`. The project checks themselves are registry
entries of `openecraft_core::checks` (one `Check` per area, with a snake_case name),
and the registry kinds, parameters and tables are defined in
`openecraft_core::{ext, params, table}`. A plugin implements `Plugin`:

```rust
use openecraft_api::{param, Info, Kind, Plugin, Registry, Table, Text};
use openecraft_core::issue::{Issue, Severity};
use openecraft_i18n::tr_const;

pub struct Spares;

impl Plugin for Spares {
    fn info(&self) -> Info {
        Info { id: "spares".into(), name: Text::Borrowed(tr_const!("Spare parts")), version: "1.0".into(), description: Text::Borrowed("") }
    }

    fn register(&self, reg: &mut Registry) -> Result<(), String> {
        reg.command("set_spares", tr_const!("Sets the spare quantity of a part"), vec![param("number", Kind::Str, tr_const!("Part number"))], |prj, args| {
            // change `prj`, return JSON
            Ok(serde_json::json!({ "part": args.req("number") }))
        })
        .check("spares_missing", tr_const!("Spare parts"), |prj, analysis| Ok(vec![Issue::plain(Severity::Info, "nothing missing", None)]))
        .report("spares", tr_const!("Spare parts"), |prj, analysis| Ok(Table::new([tr_const!("Part")])))
        .exporter("spares_csv", tr_const!("Spare parts (CSV)"), "csv", |prj| Ok(String::new()));
        Ok(())
    }
}
```

Texts given as `&'static str` are English catalog entries (`tr_const!`, see AGENTS.md)
and are shown translated. Issues of a check carry either a finished text
(`Issue::plain`) or, like the built-in checks, a catalog entry with its arguments
(`openecraft_core::issue!(Severity::Warning, target, "Cable {} defined {} times", tag, n)`),
which `Issue::message()` renders in the current language; the `check` tool reports the
template as `key` next to the message.

## External plugins

An external plugin is a program in any language in a folder of its own:

```
my_plugin/
  plugin.json
  main.py
```

```json
{
  "id": "my_plugin",
  "name": "My plugin",
  "version": "1.0",
  "description": "What it does",
  "api": 1,
  "run": ["python3", "main.py"]
}
```

The folders containing plugin folders are given in the environment variable
`OPENECRAFT_PLUGINS` (separated like `PATH`) for the editor and the command line, or
with `openecraft-cli --plugins <folder>`. External plugins need processes and are not
available in the browser.

### Protocol

For every request OpenECraft starts the program (`run`, in the plugin folder), writes
one JSON object to its stdin and reads one JSON object from its stdout, both in UTF-8
(Python is started with `PYTHONUTF8=1` and `PYTHONIOENCODING=utf-8`; where `python3` is
not installed, as on Windows, `python` is started instead). Every request
has `"api": 1` and a `method`. An answer `{"error": "message"}` (or a non-zero exit
code, with the message on stderr) is an error. `project` is the project as in a
`.oec` file. A program that has not answered after 5 minutes is stopped and the request
fails. Plugins run with the rights of the user: load only plugin folders you trust.

Requests also include `project_features`, an array advertising model support in the
running host. Current names are `directional_pin_contacts`, `explicit_wire_junctions`,
`directed_junction_paths`, `fluid_port_roles`, `wire_hide_labels` and
`scoped_connections`. Importers can require features before returning a changed
project; using a separate updated CLI does not give an older editor these features.

| Request | Answer |
| --- | --- |
| `{"method": "describe"}` | `{"commands": [{"name", "help", "params": [{"name", "kind", "required", "help"}]}], "checks": [{"name", "title"}], "reports": [{"name", "title"}], "exporters": [{"name", "title", "extension", "params"}]}` (all lists optional, `params` of exporters too) |
| `{"method": "command", "name", "params": {…}, "project": {…}}` | `{"result": <any JSON>, "project": {…}}` – `project` only if the command changed it |
| `{"method": "check", "name", "project"}` | `{"issues": [{"severity": "error" \| "warning" \| "info", "message", "page": <page index>, "item": <item id>}]}` – `page` / `item` optional; `page` alone points at the page, a page the project does not have at nothing |
| `{"method": "report", "name", "project"}` | `{"headers": ["…"], "rows": [["…"]]}` |
| `{"method": "export", "name", "params": {…}, "project"}` | `{"content": "file content"}` (text; `params` holds what the front end passed: by convention `page`, the name of one page, and `file`, the file name) |

Parameter kinds: `str`, `int`, `num`, `bool`, `ids` (list of item ids), `points`
(list of `[x, y]` in mm), `object` (JSON object). `required` defaults to `true`.

A command that answers with an error leaves the project unchanged, also in the editor
and the MCP server.

`describe` is asked once when the plugin is loaded; checks run after every change of
the project in the editor, so they should answer quickly. The project's check settings
apply to the issues of plugins as well: a check can be switched off by its name, and
its plain messages share the check's name as code (level changes then apply to all of
them) and can be accepted one by one.

Parts may set `symbol` to the key of a project symbol definition. The shared part
placement logic uses that definition in the editor, CLI and MCP. An empty key keeps
the existing template-based selection. `source_data` on parts and symbol
definitions can retain importer identifiers and vendor properties as JSON.
`Project.import_sources` maps stable source IDs to JSON datasets, including records
that have no native editing surface. These fields are optional for older files.

Portable native macro clips may carry `symbol_defs`, an array of project symbol
definitions. Inserting a clip installs missing definitions and renames conflicting
incoming keys, preserving local edits and updating the incoming instances. The
optional `devices` map and `parts` array carry device assignments and catalog records.
Insertion renames device assignments with their instances and installs missing parts
without overwriting local catalog records. `Part.symbol` follows renamed definitions.
Macro previews resolve bundled definitions before consulting the destination project.
The
`insert_macro` command accepts a native macro as a JSON string plus `page`, `x`
and `y`. The editor, CLI and MCP expose the same command. Large CLI parameter
objects can be supplied as `--json @parameters.json` to avoid shell argument limits.
User symbol pins may set `internal: true` for jumper attachment nodes. They still
participate in local bridges but do not appear as physical device endpoints.
`list_nets` exposes native membership by symbol ID and zero-based pin index;
its `pins` array contains physical device endpoints. The additional
`connection_points` array includes junction and internal bridge attachment points
that belong to a wired or physical net, without adding device endpoints or empty
nets. Importers can verify paths ending at these points through the same command
in the editor, CLI and MCP.
`bridge_terminals` defaults to true and can be disabled for individual wire runs.
`list_connections` returns physical pin-to-pin wire targets as `from` and `to`
objects containing a symbol ID and zero-based pin index, together with the
potential, conductor number, color, cross section, cable and core. Internal device
bridges retain electrical continuity without creating external manufacturing wires.
User junction symbols may set `directed_junction: true`: their `bridges` then also
describe physical target paths between spaced ports. `set_symbol_junction_paths`
accepts a symbol key and `paths: {"links": [[0, 1], [0, 2]], "directed": true}`;
the symbol editor exposes the same paths. The page property
`page.use_drawn_wire_targets` preserves explicit branches at device connection points.
User symbols can supply `pin_directions`, a map from zero-based pin indices to local
direction masks (up 1, right 2, down 4, left 8). Omitted entries allow every side;
zero disables automatic side contacts. Directions follow instance rotation and
mirroring. Saved wire endpoints always connect. `set_pin_connection_directions`
accepts `symbol`, `pin` and `directions` (`"up,right"`, `"all"` or `"none"`); the
symbol editor exposes the same directions. New automatic wires use these directions.
The page property `page.explicit_wire_junctions` makes endpoint-to-interior crossings
nonconductive unless a junction connects them, and device ports connect only to
authored wire endpoints. Internal rail attachments and junction ports retain side
contacts. Junction dots and open-end checks use the same rules.
An interruption instance can store `function.connection_scope` in its `values`.
Points connect and cross-reference only others with the same scope and label;
an empty scope retains the existing project-wide behavior. The label stays the
displayed name. This property is editable through the property database in the
editor and the shared CLI/MCP property commands.
User symbols can supply `fluid_ports`, a map from zero-based pin indices to
`supply`, `pressure`, `working`, `exhaust`, `return`, `pilot`, `suction`, `leakage`,
`measuring` or `passage`. An omitted entry keeps designation-based inference.
These roles survive library copies, portable macros and project serialization;
`list_symbols` exposes them and `set_fluid_port_role` edits or clears an entry
(`role: "automatic"`) through the editor's Plugins pane, CLI and MCP. The symbol
editor also offers the role on the selected fluid connection point.
`FieldKind::TagName` displays the device name without inherited structure while
remaining linked to the instance tag. Plot frames can set `visible: false` when
imported artwork includes its own border and title block; paper geometry remains
available. `ZeroBasedNumbers` is supported for source drawing column labels.
PNG drawing images, including palette images and transparency, export to PDF with
an image soft mask.
`Func::Graphic` keeps decorative artwork outside device identification and netlist
evaluation while retaining editable symbol primitives.

Project symbol definitions can contain an `Image` drawing primitive. It carries a
file `name`, base64 encoded `data`, and four `corners` in
local symbol coordinates, ordered top left, top right, bottom right, bottom left.
The editor and SVG/PDF output apply the symbol's
rotation and mirror to these corners. Existing drawing primitive representations
are supported alongside images.

User symbols can specify a `domain` of `Electrical`, `Fluid`, or `Process`.
Older symbol files default to `Electrical`. Fluid and process connection points
participate in fluid nets, and connecting them to an electrical pin is reported as
a mixed net.

`Field` primitives accept an optional `angle` in degrees using the same convention
as symbol text. Omitting it keeps the historical horizontal-label behavior. Their
`kind` can also be `{"Pin": 0}` (zero-based pin index), which follows the placed
instance's connection designation. A terminal's displayed designation uses `Label`.

The `Auxiliary` function represents an additional device function with electrical
pins and a cross-reference to its main function, without inventing a contact type.
`Junction` has no device tag and connects only the paths listed in its bridges.
Its connection points are local routing nodes and are excluded from device
endpoints in the netlist.

Wires can set `hide_labels: true` when their wire numbers and cable/core labels
are already drawn as separate annotations. Electrical properties and reports stay
available. The wire properties panel and `update_wire` command can restore the
automatic labels; new wires show labels by default.

A complete example is in [`examples/plugins/page_notes`](../examples/plugins/page_notes).

### EPLAN window macros

The drawing plugin accepts PXF window macro XML with `import_ema` (parameter `xml`). Its result lists native symbol definitions, function types, variant IDs, multi-device macros, and unresolved references. Reimport preserves locally edited definitions. The `place_part` command accepts an optional `variant` symbol key. The CLI and MCP file tool `import_eplan_symbols` reads an EMA, an EDZ, a companion article XML, or a vendor folder with a selected part number.

`product_designation_fields` advertises `Prim::Field` kind `{"Product":N}`. It displays the final N product designation levels of the current instance tag, excluding function and location aspects; zero behaves as one. The field updates after renaming and survives symbol-library and project serialization. The symbol editor exposes the level count; plugins use the same serialized primitive through symbol definitions. Hosts without this feature cannot load these field variants.

`function_text_fields` advertises `Prim::Field` kind `"FunctionText"` and the optional user-symbol `default_function_text`. Shared placement initializes the native instance function text from the library default. Fields render the current instance value, including transformed multiline text; the automatic tag block omits a duplicate function text when an explicit field exists. `list_symbols` returns the library default.

`function_part_selection` advertises function-specific assigned-part references. The `function.part_index` property selects a one-based position in the device parts and records its part number in `SymbolInst.values["function.part_number"]`. Removing earlier parts does not change this reference. A removed selected part yields an empty selection, position zero, and a project issue; setting an existing position repairs it, while clearing the position resets automatic first-part selection. `part.*` properties on a function read only its selected part; device properties retain their aggregate behavior. `function.manufacturer` supports an explicit editable override with selected-part fallback, and `function.selection_text` retains authored selection text independently of catalog type numbers. The registered `list_function_parts` command returns every function’s selected part and editable attribute values for batch inspection.

`property_blocks` advertises `Prim::PropertyBlock` with ordered `Property(key)`, `Field(FieldKind)` and `Text(value)` rows, local `pos` and `step` vectors, font `size`, horizontal `anchor`, text `angle`, `align` (`Start`, `Middle`, `End`) and optional RGB `color`. The shared canvas/export renderer resolves properties on the original full-tagged function, skips empty values, and flows every line before applying symbol transforms. User-symbol `default_values` initializes editable instance properties during shared placement; `list_symbols` exposes defaults and blocks. The symbol editor supports creation and editing of blocks and default properties.

The `wire_type` project feature preserves conductor construction on `Wire.wire_type`.
`update_wire` accepts `wire_type`; `list_connections` returns it for each physical
connection. The editable connections table appends Wire type after its original
nine columns; connection reports include the same value. Empty means unspecified.

The `physical_wire_routes` feature advertises contact-aware conductor tracing.
`list_connections` includes `route_traced` and `wires` (native page ID and wire ID).
When `route_traced` is false, the wire list is the existing net-wide property scope,
not a demonstrated point-to-point path. Explicit-junction pages keep wire-local
vertices separate until a valid endpoint, junction or directional pin contact joins
them. These rules also apply when a crossing belongs to one net by another route.

`trace_wire_routes` accepts `routes: {"pairs": [{"from": {"symbol": 12, "pin": 0},
"to": {"symbol": 34, "pin": 0}}]}` and returns an ordered result for each pair.
It traces exact local drawn endpoints, including virtual interruption points,
without jumping between equal potential or interruption names. Each result has
`route_traced` and `wires` with native page/segment IDs and their conductor properties.
This verifies a particular branch even when the physical wiring list chooses another
valid path through a named net. An invalid or disconnected endpoint has no traced route.

### Complete PLC module channels

The `multi_channel_plc` project feature advertises `UserSymbol.plc_channels`, also exposed by `list_symbols`. Each channel definition contains a stable `key`, zero-based directional `index`, optional `input` direction, `analog`, `data_type`, `designation`, zero-based physical `pins`, and an optional `signal_pin`. Existing single-channel `plc_channel` metadata remains supported.

Channel instance values use one-based property keys `function.plc_address[1]`, `function.plc_symbol[1]`, `function.plc_comment[1]`, and `function.plc_data_type[1]`. Library definitions should leave configured addresses and signal names empty. Property blocks can reference these keys to display current configuration. `get_plc_points` returns a row per channel with its physical pin indices; `set_plc_channel` edits one channel using a symbol ID and a zero-based channel index. PLC table row keys use `symbol_id:channel_index` for complete modules and retain numeric symbol IDs for legacy single points. Blank and directionless channels are excluded from PLC tag exports.

### Linked PLC channel representations

The `linked_plc_channels` feature adds `SymbolInst.plc_bindings`, a map from zero-based channel indices to `{group, primary}`. A nonempty group identifies the same logical channel across representations with the same full device tag. Set `primary` on the electrical representation whose physical pins and location should be used for I/O navigation. Different full device tags keep copied devices independent.

The PLC overview, hardware counts, tables and tag exports use logical channels. Channel edits, indexed channel property edits, card readdressing and symbol-table import update all linked views; a released view rejects the whole channel edit. Existing authored values remain stored on each view. Empty displayed fields can fall back to another linked view. `get_plc_points` returns logical I/O rows by default; `representations:true` returns all schematic representations with their own stored values, physical pin indices and binding metadata for lossless import verification.

The `plc_channel_indices` feature adds optional `ChannelBinding.index`, the card-relative hardware channel number. Store the module overview's hardware number on every linked view, even when an electrical view contains only one local channel. `get_plc_points.hardware_index` exposes this number separately from the symbol-local `channel` index used by `set_plc_channel`. Readdressing and the logical channel designation use the hardware number. Older bindings without an explicit number fall back to the most complete linked module definition, keeping continued projects usable after sheet reordering.

Parts representing complete PLC modules use one `FunctionTemplate` per channel, with `PlcInput`, `PlcOutput`, `PlcAnalogInput` or `PlcAnalogOutput` and that channel's physical pin designations. Leave reusable signal descriptions empty. Native function usage and reserve calculations count linked schematic channels once and exclude cabinet graphics. `list_device_descriptions` includes analog function capacities; `list_unplaced` recognizes occupied module channels by hardware identity independently of configured address text.

`set_plc_card` preserves authored channel addresses for metadata-only changes. Supplying `start_byte`, `output_byte`, `readdress:true`, or a Rockwell `slot` rebuilds addresses from configuration. An imported head description with empty ports and no configured bus keeps an empty port list; it does not acquire an inferred PROFINET interface.

Station scheme conversion includes complete-module digital channels and their linked representations. It retains each configured address offset and signal name. Custom address text that is not a supported digital address remains unchanged.

The `explicit_panel_mounts` feature supports `EnclosureKind::Assembly`, an open mounting canvas without implicit shell or plate geometry, and `ItemKind::MountingPlate`, an explicit item with authored position, size and thickness. Explicit plate items also suppress the default plate of a cabinet. Coordinates remain in the enclosure's common frame. `PanelItem.mounted_on` references another item in the same enclosure; `mounting_explicit:true` distinguishes an explicitly free item from legacy inferred rail placement. `rail_mount_override:true` retains the instance datum through native loading and part refresh. `rail_vertical:true` measures the cross-rail datum along x instead of y.

Use `add_enclosure` / `set_enclosure` with `assembly:true`, `place_mounting_plate`, and `set_panel_item` with `mounted_on`, dimensions, `rail_offset`, `clip_on`, `rail_vertical` and `rail_mount_override`. Parent moves and depth changes carry descendants once, including nested supports. Explicitly detached items keep their position. Item lists expose the mounting fields. Portable and pattern copies remap internal mounting references; deleting a support detaches surviving children. Both horizontal and vertical rails support snapping and arrangement. Original project data without these optional fields retains legacy behavior.

The `individual_panel_terminals` feature adds `PanelItem.terminal`: a nonempty designation binds a footprint and its own part to one terminal of a strip. Empty retains whole-strip layout. Physical points, meshes, virtual wiring and 3D cable endpoints resolve each terminal separately; ambiguous repeated placements do not select an arbitrary item. Function-specific selected parts also drive terminal strip layout. `place_panel_device` creates an explicitly dimensioned device or terminal; `set_panel_item` can edit its terminal designation. `get_panel_points` exposes generated or exact physical points through the shared API.

The `explicit_panel_geometry` feature retains `PanelItem.single_part`: the footprint represents only its own catalog part even with no terminal designation. Missing catalog dimensions can generate non-exact connection points using the instance footprint, without rewriting the catalog. Multiple physical views resolve by selected article only when unique. `Enclosure.no_implicit_plate` suppresses the default plate, while `mounting_origin: [x,y,z]` gives an explicit mounting-face datum in enclosure coordinates. Open assemblies default to the zero datum and full canvas extent; their routing spaces have no cabinet walls. The add/set enclosure commands expose `implicit_plate` and `mounting_origin` (object with x/y/z; empty resets), and device placement/edit commands expose `single_part`. Explicit plates contribute their own part to the material list.


The `literal_part_quantities` feature adds `PartRef.total`. A true value makes `count`
the literal quantity for the whole device/strip (including an explicit zero), instead
of multiplying terminal articles by strip positions. The default false preserves
legacy counting, including the legacy zero-as-one default. Primary automatic and
additional literal references may use the same article number; their quantities
add, so newly drawn terminals still increase primary material without multiplying
spares. Literal material-only devices also appear in `list_devices` and the BOM.
The shared `add_device_part` and `set_device_part_quantity` commands expose both
count and mode; the latter uses a 1-based assignment index. The device parts editor
also exposes the total quantity flag. Zero literal references contribute no function
capacity, pieces, weight, heat loss or material quantity.

`Enclosure.device_tag` binds a physical body to its primary device assignment, like
the `tag` on a structural panel item. Their primary automatic article is counted by
the physical layout, with additional literal articles counted separately. Changing
the physical article through `set_enclosure`, `set_panel_item` or profile replacement
updates its primary assignment and function part bindings while retaining literal
spares. Add/set enclosure commands expose `device_tag`; item edits expose `part`.

The `material_ownership` feature adds `owner_device` and `bom_excluded` to panel items
and enclosures, exposed by the panel query and editing commands. Ownership records
the device accounting for an article and never changes `mounted_on`. A placement
exclusion suppresses only its physical material entry; the editable device properties
`device.material_owner` and `device.exclude_from_bom` retain material-only devices
and can suppress all material assigned to a device. Ownership references follow
device renaming. Reusable panel macros clear external owner tags and retain explicit
placement exclusion. Panel queries expose `mounting_height` above the explicit
support (null if unknown); generated rail standoffs use that height and require
`layout.hardware`, rather than treating the support thickness as a spacer.

Cabinet device bindings also participate in `list_devices` and the material list
when no schematic function is drawn, independently of any additional articles.
An individually placed terminal not yet represented in the schematic contributes
its primary part once per terminal designation. Repeated physical views and newly
drawing the same terminal do not add another piece. Existing whole-strip placement
keeps its schematic terminal expansion.

The `portable_macro_materials` feature guarantees that portable macro insertion
preserves device property values and distinct automatic
and literal references to the same article, including zero. When extending an
existing device, missing reference modes and properties are filled while local
quantities and property values remain unchanged. Material owners within a clip
follow target page structure and collision renaming; external owners are cleared.
Incoming device tags are reserved before allocating replacements, including
material-only existing tags. Conductive auxiliary views follow their master tag.
Device property placeholders resolve together with macro tags and part numbers.

The `per_wire_junctions` feature adds optional `Wire.explicit_junctions`: omitted
inherits `page.explicit_wire_junctions`, true requires authored junction contacts,
and false permits implicit T contacts. Implicit wire-to-interior joins require
both conductors to permit them; explicit junctions and saved endpoints still
connect. Pin-side attachments use the individual conductor policy. An explicit
wire also enables drawn physical targets on its electrical net.
The shared add_wire, connect, route and update_wire commands accept
`junction_policy: inherit|explicit|implicit`. update_wire returns that policy;
page queries and portable clips retain the optional native field. The editor
offers the same choices in wire properties. Inserting an explicit macro never
changes the page setting or other wires. Scan caching tracks policy edits, and
wire cleanup and collinear merging preserve different policies and properties.

`apply_cable_part` (`cable`, `part`) applies a project or built-in catalog part to
an existing cable. It uses the same operation as the editor, including type,
supplied core names and cross-section, and `Outer diameter` from technical data
in millimetres. Missing or invalid diameters reset the override to zero so routing
uses its estimate. It returns the configured cable fields and `diameter_mm` (zero
means estimated), and rejects unknown cable or part names without changing state.

`list_cable_parts` takes no parameters and returns the available cable articles
from project parts and the built-in catalog, sorted by number and without
duplicates. Project definitions take precedence, including a local change of
product group. Rows include number, type, description, manufacturer, supplied
cores and cross-section, and a positive declared `diameter_mm` or null. The
editor uses the same precedence with its configured parts database.

The `portable_cables` feature carries cable inventories and their referenced local
parts in `Clip.cables`. Copy/paste and macro insertion allocate fresh cable IDs
and collision-free tags in the destination structure, consistently remapping
marker symbols, wires, connection definitions and cable definition lines. Core
assignments are retained only for cables owned by the clip. Legacy clips without
cable definitions continue to clear wire/core references. Cable text fields also
support native macro placeholders. This behavior is shared by all frontends.

An explicit function article reference can resolve the matching cable article
when the same tag also has an ordinary device material record. The selected
article must match an actual assignment; unknown references remain invalid.
Cable material stays in the cable inventory rather than being duplicated in
the ordinary device bill of materials.

Physical connection results can contain the same endpoint pair more than once
when separate cable cores have distinct simple drawn routes. Each connection
retains its own wire IDs and cable/core properties for reports and routing. A
dangling spur is not treated as an additional complete connection. The existing
connection commands and report registry expose this behavior in every frontend.

The shared `apply_cable_part` command updates explicit placed-function references
that belonged only to the previous cable article. References to an ordinary
device article under the same tag, and invalid or unrelated explicit references,
are preserved. The cable editor uses the same core reference operation for
catalog selection and direct part-number edits. Length and conductor assignments
are retained when applying catalog cable data.

The `macro_definition_parameters` host feature extends native macro placeholders
to portable symbol definition letters, default labels/function texts/properties,
pin labels, text primitives and literal attribute-block rows. Symbol keys, property
keys, encoded images and raw source metadata retain their identity. Resolution
operates on a clip copy before installation and is shared by desktop, CLI and MCP.
Importers converting source template parameters should require this feature so
older hosts do not leave static graphics inconsistent with resolved instances.

`insert_macro` accepts optional `values`, an object mapping placeholder names to
strings. It resolves instance and portable definition fields together before paste;
unknown placeholders stay literal. Invalid non-string values fail before any edit.
Omitting the parameter preserves existing unresolved template text. This option is
available through the shared registry in every frontend.

The `parallel_cable_cores` host feature confirms that physical connection consumers
retain distinct authored parallel core routes. Importers must check the running
host as well as any separate verification CLI before accepting circuits containing
assigned cable cores; a newer verifier cannot update an older editor's report engine.

The `page_property_blocks` feature resolves project/page properties in symbol
attribute blocks using their placed drawing context. Other fields retain function
selection, including the selected part article. Read-only `page.local_name` returns
the page number without its section prefix; `page.section_pages` counts pages in
the same section (top-level pages form section zero). Both update after edits.

The `page_template_values` feature retains a page template's `values` map when
saving, exchanging and inserting it. Empty legacy maps remain compatible. The
shared list_page_templates command returns these defaults, and every frontend
applies them through the same insertion operation. Importers should check both
features before supplying reusable source frames with editable context fields.

The `page_frame_fields` feature adds computed `page.section_last_number`,
`page.previous_local_name` and `page.next_local_name` properties. The last number
uses natural ordering; adjacent pages follow the editable order of their section,
with empty values at either end. For sheets 11 through 23, the page count is 13
while the last number is 23. Insertions, deletions and renumbering update these
fields. `page.function_designation` and `page.location_designation` include the
`=` and `+` aspect signs, including bare signs for an unconfigured aspect.
These read-only properties are available through the shared property registry,
symbol attribute blocks, free-text placeholders and title-block forms.


The `function_cable_references` host feature provides the editable Function
property `function.cable`, containing the full cable tag for a placed marker.
Renaming the cable through the cable table or editor updates these references,
matching marker tags, wire assignments, connection points and definition lines,
including inactive model options. Ordinary device symbols sharing a tag remain
separate. Deleting the inventory entry clears its explicit marker association and
core assignments, removes its owned route and accessory definitions, and retains
ordinary device endpoint places. Portable macro/clipboard content carries the referenced cable
and maps the association to the fresh pasted cable tag. Use `set_properties` on
`function:<symbol id>` to edit the association; existing registry operations apply
equally to desktop, CLI and MCP.


Cable tag cells (`table_set`, table `cables`, column `Cable`) and the cable manager
use the same atomic `ops::rename_project_cable` operation. It trims the entered
tag, validates its designation, refuses duplicates and released drawing changes,
and updates references only after validation. The cable manager keeps draft text
outside the project until Enter or focus loss; Escape cancels the draft.

A function with an explicit `function.cable` reference resolves its selected and
assigned primary article through that cable inventory entry. A changed, cleared or
missing cable article is reflected immediately, including direct cable property
and table edits. Cached function part selections do not shadow this association.
Ordinary function article selection keeps its existing explicit-reference rules.


The `cable_property_blocks` feature resolves Cable-class rows in symbol attribute
blocks against the symbol's explicit `function.cable` reference. A missing explicit
target stays unresolved; symbols without that reference retain the legacy
same-tag cable context. This context also applies to placed cable properties and
is shared by the canvas and exports. The symbol editor offers Cable properties.
Placed Part-class properties resolve the function's selected catalog article,
including the current linked cable article and non-primary device assignments.

Hosts advertising `placed_wire_labels` retain `Wire.labels` even when
`hide_labels` suppresses automatic labels. Each layout contains `kind` (`Core`,
`Cable`, `Number` or `Color`), attachment fraction `along` (0..1 from a to b),
`offset` in drawing mm, `size`, `anchor`, `angle` and optional RGB `color`.
Text comes from the current owning wire. `update_wire` accepts
`labels: {"items": [...]}` to replace the layouts, or an empty items array to
remove them. Invalid layouts reject the entire command. Labels follow their
wire during editing and are redistributed once when routing creates new legs.

Unattached core markers can retain independent editable text in the indexed
`function.cable_core[1]` through `[64]` properties. Importers should use placed
wire labels for markers that reference an actual physical wire, so wire core
edits change the displayed text automatically.

`rename_symbol_device` accepts a placed symbol `id` and `new_tag`. An explicit
`function.cable` reference matching its displayed tag selects the cable inventory;
ordinary shared-tag devices remain independent. Missing cable targets, duplicate
cable tags and released drawing references reject the operation. The editor's
selected-function rename uses the same operation. `rename_device` follows the
cable inventory when no ordinary device identity shares that tag. Material owner
references follow cable edits only while the old tag has no ordinary device identity.
Renaming a device or cable can retain an unchanged imported prefix containing
spaces; the new final device name still follows the normal tag validation rules.

### Explicit cabinet orientation

Hosts advertising `explicit_panel_rotations` preserve optional numeric
`PanelItem.rotation` values 0, 90, 180 and 270 (clockwise in the face plane).
Negative multiples of 90 are accepted and normalized. Missing values preserve
legacy dimension inference; explicit zero suppresses that inference. The saved
`x`, `y`, `w`, `h` remain the already oriented footprint. Importers must not rotate
those dimensions a second time. Graphics, meshes, connection point positions and
exit directions, assembly accessories and exact drilling patterns follow the angle.

`set_panel_item(rotation: ...)` changes orientation without moving or resizing the
footprint. `infer_rotation: true` clears the explicit angle; supplying both is an
error. Item queries return `rotation` and `effective_rotation`.
`arrange_panel_items(action: "rotate")` turns selected items 90 degrees around their
centres and swaps width and height, matching the editor Rotate action.

Optional boolean parameters in the editor offer Use plugin default, Enabled and
Disabled. The first omits the argument so external plugins retain their documented
defaults; explicit false remains distinct from omission in every frontend.

### Symbol definition revisions

`merge_symbol_definitions` accepts `definitions: {"items": [<UserSymbol>, ...]}`.
The complete batch is deserialized and validated before any change. It returns
`keys`, one resolved key per input in source order, and `added`, the number of new
definitions. Identical typed definitions reuse their existing key. Conflicts try
`<original>_import_1`, `_import_2`, and so on, reusing an equal existing revision.
Existing definitions and page instances remain unchanged. Two inputs sharing an
original key can return different revision keys, so importers should map references
within each incoming clip rather than build one global map of original keys.

The editor, CLI, MCP and external plugins all use the same registered command.
Native typing normalizes default fields and numeric precision before comparison;
importers should use this operation instead of comparing raw saved JSON objects.
This is the same core operation used when pasting portable macro definitions.
