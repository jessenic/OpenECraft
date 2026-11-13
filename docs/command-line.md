# Command line, MCP server and plugins

Projects and the parts server without a window, for scripts and AI agents.

`openecraft-cli` edits projects and maintains the parts server without a window. The same
commands are offered to AI agents over the [Model Context Protocol](https://modelcontextprotocol.io)
(`openecraft-cli mcp`, JSON-RPC on stdin / stdout), so an agent can create projects, place
symbols, draw wires between connection points, assign parts, run the check, generate
reports and export PDF / SVG / DXF / DWG.

```bash
cargo run --release -p openecraft-cli -- help                 # all commands
cargo run --release -p openecraft-cli -- help connect         # parameters of a command
cargo run --release -p openecraft-cli -- --version             # also -V or version
openecraft-cli new_project --name "Pump control" --path pump.oec
openecraft-cli new_project --example star_delta_starter --path star_delta.oec   # see list_examples
openecraft-cli -p pump.oec place_symbol --page 1 --symbol pb_no --x 200 --y 100   # -S1
openecraft-cli -p pump.oec place_symbol --page 1 --symbol coil --x 200 --y 160    # -K1
openecraft-cli -p pump.oec place_part --page 1 --part GEN.LMP-GN-24 --x 240 --y 160   # -P1 with its part
openecraft-cli -p pump.oec connect --page 1 --from -S1:14 --to -K1:A1
openecraft-cli -p pump.oec check
openecraft-cli -p pump.oec export --format pdf --path pump.pdf
openecraft-cli --server http://127.0.0.1:8090 --token <secret> parts_import --path parts.csv
openecraft-cli --server http://127.0.0.1:8090 --token <secret> parts_import --path 6ED1052-1CC08-0BA3.edz
openecraft-cli --project drawing.oec import_eplan_symbols --path drive.edz
openecraft-cli --project drawing.oec import_eplan_symbols --path Beckhoff --part BEC.EK1100
```

- On the command line every change is written back to `--project` at once. In an MCP
  session the project stays open in memory: `save_project` writes it, `undo` / `redo`
  revert single tool calls (`help mcp`); the command line refuses `undo` / `redo`, as it
  opens the project anew for every command. An archive given as `--project` stays
  unchanged: save the project as a project file (`save_project --path <file>.oec`) or
  convert the archive (`convert_project`).
- A **split project** keeps the project as many small files for version control (git):
  the index `<name>.oecp` with a folder `pages` (one file per page), `data` (one file
  per device, part, cable … and per area of the project-wide data) and `files` (embedded
  documents and images as plain files) next to it. It holds the same project as the
  `.oec` file: `convert_project --path plant.oec --format oecp` writes
  `plant/plant.oecp`, `--format oec` or `--format oecz` convert back, and `--project
  plant/plant.oecp` works on it like on a project file – a change rewrites only the
  files it concerns. Every file is one line of JSON and depends only on the content of
  the project, so saving an unchanged project changes nothing. A split project needs a
  folder of its own; other files in it (a README, `.gitattributes`) are left alone.
- **Git** needs two things for a split project. The `.gitattributes` that saving writes
  into the folder (unless there is one) keeps git from converting line ends, which
  would change every file and damage the embedded ones. And the merge driver merges
  the JSON files record by record, so that changes to different symbols of a page, or
  to different fields of one, do not conflict; set it up once per clone:

  ```bash
  git config merge.openecraft.name "OpenECraft split project"
  git config merge.openecraft.driver "openecraft-cli merge_project_file --base %O --ours %A --theirs %B --name %P"
  ```

  Where both sides changed the same value, the file keeps the value of the current
  branch, the command names the places and git marks the file as conflicted: open the
  project, set what is right and `git add` the file. Object ids are not coordinated
  between branches, so two branches that each added an object can give it the same
  id; after a merge run `openecraft-cli -p plant/plant.oecp check_project_data` and,
  if it reports ids used more than once, again with `--repair`.
- Parameters are `--name value` (also `--name=value`, or all at once with `--json '{…}'`);
  lists as `--ids 3,4` and points as `--points "0,0 40,0"`; lists of codes, tags or
  languages are separated by commas or blanks. Values outside their range are refused
  with the allowed range, not clamped, and a command that fails leaves the project
  unchanged. Numbers in answers are rounded to the precision they are stored with.
- Parameters renamed since earlier versions are still taken under their old names:
  `rename_device` `old` / `new` (now `tag` / `new_tag`), `rename` of `set_potential`,
  `set_fluid_group` and `set_pct_loop` (now `new_name`), `replace_text` `text` / `with`
  (now `find` / `replace`).
- Parts server: `--server` / `--token`, the `OPENECRAFT_SERVER` / `OPENECRAFT_TOKEN`
  environment variables or the `parts_connect` tool (also with `user` / `password`);
  default `http://127.0.0.1:8090`. `parts_connect` with `kind: Eplan` reads the EPLAN
  Data Portal instead (the token is the personal access token; searching, `parts_get`,
  `parts_file` and `parts_check_updates` only). The data portal tools search with facets
  (`parts_search`), manage catalogs, files, favorites and users (`parts_catalog_save`,
  `parts_attach`, `parts_favorites`, `parts_user_save`, …), compare the project parts
  with the server (`parts_check_updates`) and share the company's forms
  (`parts_company_forms` (`publish`)).
- Help and messages follow `--lang` or the system language; the MCP server answers in
  English unless `--lang` is given.

- `OPENECRAFT_MCP_ROOT` is the folder the files of the MCP tools must be in (`path`,
  `output`, `folder`, `database`, `target` and template files); default the working
  folder of the server, `*` allows every folder. An agent misled by text it reads (a
  project, a part, a web page) cannot read or overwrite files elsewhere, such as other
  projects or plugin folders.
- `OPENECRAFT_MCP_TOOLS` limits the tools the MCP server offers, for clients that take
  only a limited number: names separated by commas, `parts_*` for all tools starting
  with `parts_` (e.g. `new_project,open_project,save_project,place_symbol,connect,check,export`).
- `new_project --path` does not replace an existing file unless `overwrite` is given.
  `search_projects` searches the current folder unless a database or folder is given.

Register the MCP server with an agent, e.g. Claude Code:

```bash
claude mcp add openecraft -- /path/to/openecraft-cli mcp
```

or in the `mcpServers` configuration of other clients:

```json
{ "mcpServers": { "openecraft": { "command": "openecraft-cli", "args": ["mcp"] } } }
```

The agent can read and write any file the server process can access; start it in a
working directory (and with a parts server token) you are fine with the agent using.

## Plugins

Plugins add commands, checks, reports and exports to the editor (*Plugins* tab: *Plugin
commands* lists every command by plugin with a search, the star pins one to the tab; *Check*
tab), the command line and the MCP server at the same time. Built-in plugins are Rust
code in `crates/openecraft-api`; external plugins are programs in any language that
answer JSON requests on stdin / stdout:

```bash
OPENECRAFT_PLUGINS=examples/plugins openecraft-cli -p pump.oec plugins
openecraft-cli --plugins examples/plugins -p pump.oec report --kind page_notes
```

See [docs/plugins.md](plugins.md) for the API and the protocol.
