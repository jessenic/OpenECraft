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
