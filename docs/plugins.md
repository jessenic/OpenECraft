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
