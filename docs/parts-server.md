# Parts server and parts manager

The data portal shared by all OpenECraft installations and the app that maintains it.

`openecraft-parts-server` is the data portal of all OpenECraft installations (desktop
and web), in the spirit of the EPLAN Data Portal: manufacturer catalogs with a faceted
search, part history and lifecycle, files of the parts (datasheets, images, models,
macros), user accounts with roles and a web page for browsing. The details of a part
show its 2D front view and its 3D model (turned by dragging) where it has them; the
parts of the example catalog all have both when their dimensions are known, generated
from the dimensions (devices, terminals, PLC modules, rails, ducts, busbar systems,
enclosures and their components, accessories). It is a separate executable and runs
independently of the editor.

```bash
cargo run --release -p openecraft-parts-server -- --bind 0.0.0.0:8090 --token <secret>
cargo run --release -p openecraft-parts-server -- --storage json --data parts-db.json
cargo run --release -p openecraft-parts-server -- --data parts-db.sqlite --migrate-from parts-db.json
cargo run --release -p openecraft-parts-server -- --install-catalogs all
cargo run --release -p openecraft-parts-server -- --config parts-server.toml
```

As a container: `docker run -d -p 127.0.0.1:8090:8090 -e OPENECRAFT_TOKEN=<secret> -v parts:/data
ghcr.io/nilres/openecraft-parts-server` (see [installation](installation.md)).

- Configuration file (`--config <file>` or `OPENECRAFT_PARTS_SERVER_CONFIG`): a TOML
  file with the settings of the command line options under their names (`bind`,
  `storage`, `data`, `token`, `admin_user`, `install_catalogs`, `access`,
  `registration`, `dashboard`; see
  [`parts-server/parts-server.example.toml`](../parts-server/parts-server.example.toml)).
  Options on the command line or in the environment win; relative paths are relative to
  the file; unknown keys stop the server with an error. `--migrate-from` is a command
  line option only.
- Storage (`--storage sqlite|json`): SQLite by default (`--data parts-db.sqlite`, one
  file with indexed parts, history, accounts and files); JSON keeps everything in one
  file written atomically on every change, with the files in `<name>.files/` next to
  it. A `.json` data file selects JSON storage. New databases are filled with the
  example catalog and the bundled manufacturer catalogs; `--migrate-from` copies an
  existing database into a new one.
- Bundled catalogs: `crates/openecraft-parts/catalogs/<id>.json` (a `catalog` and its
  `parts`, listed in `openecraft_parts::bundled`, which the example projects use too)
  are curated starter sets of real
  manufacturer parts: `abb` (circuit breakers, RCCB, contactors, manual motor
  starters), `beckhoff` (EtherCAT coupler and terminals), `cosmo` (stainless steel
  heating circuit manifolds for 2 - 14 circuits and single-room control boxes RTL / TH
  for underfloor heating), `dehn` (surge protective devices type 1 - 3, for PV systems and for data lines), `gira` (System 55 socket
  outlets, switch and push button inserts, network outlet Cat.6A with rockers, covers
  and frames), `hager` (circuit breakers,
  RCCBs, installation contactors, switch disconnectors, NH fuse switch disconnectors
  for 60 mm busbars and fuse switch disconnector strips for 185 mm with clip-in current
  transformers, circuit breakers h3+ with motor operators and releases, the NA
  protection relay EU400, univers cabinets with their component modules, energy
  meters), `janitza` (power analyser UMG 604-PRO), `jinko` (PV modules Tiger Neo
  54HL4-B all black 405 - 415 Wp), `longi` (PV modules Hi-MO X6 Explorer LR5-54HTH
  425 - 435 Wp), `loxone` (energy meter for
  1 / 5 A current transformers on the Tree bus), `mdt` (KNX actuators, IP
  interface / router, power supply, KNX push buttons, glass push button and room
  temperature controller for flush mounting), `phoenix_contact` (terminal blocks, relay
  modules, power supplies), `pylontech` (19-inch LiFePO4 battery modules), `siemens` (SIRIUS contactors, overload relays, motor
  starter protectors, SENTRON circuit breakers), `victron` (MultiPlus-II inverter /
  charger, Inverter RS Smart Solar, SmartSolar MPPT, Orion-Tr, Cerbo GX, SmartShunt, BMV-712, Lynx
  Distributor; mostly parts for direct wall mounting) and `weidmueller` (terminals,
  relay module, power supplies, CD wiring ducts). `--install-catalogs all` (or ids such as
  `siemens`) installs or updates them in an existing database: new parts are added,
  parts nobody changed are updated, parts changed or deleted by users are kept. Parts
  without models get generated ones from their mounting dimensions. Only add data
  that may be redistributed (facts from public data sheets, no copied texts, images
  or models). Each bundled catalog has a logo, `catalogs/logos/<id>.svg`, which it
  gets when it is installed while it has no logo: the company's logo from Wikimedia
  Commons (public domain or CC BY-SA 4.0, sources and licences in
  `catalogs/logos/README.md`; the logos are trademarks of their owners), or a neutral
  badge with the name where Commons has no freely licensed one (COSMO, Janitza, MDT,
  Pylontech).
- Logos: every catalog can have the logo of its company (PNG, JPEG, GIF, WebP or SVG,
  at most 512 kB), shown on the catalog cards, in the catalog list of the
  administration and on the parts of the catalog. Who may change a catalog changes
  its logo in *Administration › Catalogs › Edit* (`PUT` / `DELETE
  /api/v1/catalogs/{id}/logo?name=`, `GET` serves it with a sandboxing content
  security policy, so that an SVG runs no scripts). Saving a catalog keeps its logo;
  the logo file is stored with the parts' files and is part of backups.
- Libraries: JSON documents shared by all projects (`GET` / `PUT /api/v1/libraries/{name}`),
  so far `forms`, the company's title blocks, report schemes and plot frames that the
  editor (*Forms › Title blocks › Company library*) and the command line publish and
  load. Everybody who may read parts reads them; changing them needs the right to edit
  catalogs for the whole server (editors, administrators).
- Users: readers keep favorites, manufacturers change the parts and catalogs of their
  own manufacturer, editors all parts, administrators also manage the users. The
  `--token` (or `OPENECRAFT_TOKEN`) is the administrator token; `--admin-user <name>`
  (or `OPENECRAFT_ADMIN_USER`) creates an administrator account with the password from
  `OPENECRAFT_ADMIN_PASSWORD`.
  Users sign in with `POST /login` (`name` and `password`; `user` is taken for `name`, as
  on the project server) and send the session token as `Authorization: Bearer <token>`.
  A token the server does not know (an expired session) is refused with `401`, for
  reading too, so that clients notice it instead of reading anonymously. Passwords of
  local accounts (also the one of `--admin-user`) have at least 8 characters; after 5 failed sign-ins of a user name every further attempt waits
  (1 s, doubling up to 5 minutes, answer `429`). Without token, accounts and directories the server
  is read-only.
- Registration: when it is open, anybody creates a local reader account on the web
  page (*Register* next to the sign-in, `POST /register` with `name` and `password`);
  administrators give more rights. By default the new account waits for the approval
  of an administrator (`202`, no session): it cannot sign in until an administrator
  approves it in *Administration › Users* (`POST /users/{name}/approval`) or rejects
  (deletes) it; waiting accounts are listed first and counted in the overview. Without
  approval (switched off in *Administration › Access rights*, stored as
  `"registration_approval": false`) the new user is signed in at once. Approving stays
  an administrator's choice even when the start options fix the registration. Accounts
  that wait when approval is switched off keep waiting until approved. It is closed unless
  switched on: by administrators in *Administration › Access rights* (stored as
  `"registration": true` in the access settings file), or fixed by the start options
  `--registration true|false`, `OPENECRAFT_REGISTRATION` or `registration` in the
  configuration file, which win over the switch. Registered users read the parts even
  where reading needs a sign-in. A client may register 5 accounts, then every further
  attempt waits (1 s, doubling up to 5 minutes); names that grants refer to (also with
  wildcards) and `admin` are not available, so that nobody registers into the rights
  of a grant. A registered name also stands before a directory user of the same name
  who has not signed in yet: on servers with directories, grant by groups.
- Access control (`<data>.access.json`, `--access`): sign-in with Active Directory or
  LDAP directories next to the local accounts, groups of users, roles defined by
  administrators and grants of roles to users and groups per manufacturer or per
  catalog, with deny grants; anonymous reading can be switched off. See
  [docs/access-control.md](access-control.md).
- Dashboard: the web page's *Dashboard* shows the number of parts, catalogs,
  manufacturers and files and graphs of the parts per manufacturer, product group,
  catalog, top group and lifecycle, the last changes per month, how many parts have a
  description, price, dimensions, models, files and data sheets, the users per role
  (administrators) and the parts changed last; its bars open the parts they count
  (`GET /api/v1/dashboard`). Administrators hide it in *Administration › Overview*
  (`"dashboard": false` in the access settings, `PUT /api/v1/dashboard/settings`); the
  start option `--dashboard true|false`, `OPENECRAFT_DASHBOARD` or `dashboard` in the
  configuration file fix it.
- Exports carry the version of the server, so that a later version can convert them
  when they are imported: `GET /export.json` is an object with `export` (`format`,
  `format_version`, `server`, `server_version`, `revision`, `exported`) and `parts`
  (`/import` and the editors read it as before), `GET /export.csv` starts with a line
  `# format=openecraft-parts;format_version=1;server_version=…` (the CSV import skips
  such lines before the header), backups keep the fields as `export.<name>` in their
  `meta` table, the catalogs and access settings exported on the web page have an
  `export` object and the users CSV the same first line. The answers also carry the
  header `X-OpenECraft-Server-Version`.
- Parts are checked before they are stored: part numbers have at most 128 characters,
  the other fields at most 256 (description, note, technical data, certificates and
  data sheet 20000), the catalog a part names must exist, and an import refuses a file
  that has a part number twice.
- Every change is a new revision; each part keeps its revision, time of change and the
  last 20 versions (history). Parts have a lifecycle (active, phase-out, discontinued)
  and a successor. Files are stored once per content; a file is deleted as soon as
  neither a part nor a version in the history uses it (and at startup).
- Product groups: the EPLAN generic product groups, product groups and subgroups with
  their exchange numbers (`openecraft_parts::productgroups`) are offered in the part
  editor next to OpenECraft's own groups.
- Search (`POST /search`): text (every word must occur), top group, group, subgroup,
  manufacturer, catalog, lifecycle and technical data (voltage, current, power and the
  `Name: value` lines of the technical data), sorted and paged, with facets that count
  the matching parts per value.
- Web page: `http://<server>:8090/` – dashboard, search with facets, part details with
  files and history, catalogs, sign-in and favorites, in English, German, French and
  Spanish. The logo and the title at the top left lead back to the start page (all
  parts, no filters). The list shows 10, 25, 50, 100 or 200 parts per page (*Per page*,
  kept by the browser). The address keeps the search and the open part (bookmarks, the
  browser's back button); `?lang=de` chooses the language.
- New parts on the web page: users who may change parts of some manufacturer or catalog
  get *New part* above the list and *Edit* / *Delete* on the parts they may change; the
  form holds the general, technical, commercial and mounting data, offers the catalogs
  the user may add parts to and never replaces an existing part (`PUT /parts/{number}`
  with `If-None-Match: *` answers `409`).
- Administration on the web page (*Administration*, shown to administrators and to
  users who may change catalogs or parts; sign in with a user or, on a new server,
  with the administrator token): an overview of the server (storage, revision, users
  per role, access settings, the caller's rights); *Users* – create, change and delete
  local users (role, manufacturer, groups, new password), see their last sign-in and
  open sessions and sign them out everywhere; *Access rights* – the role of requests
  without sign-in, defined roles with their rights and the grants to users and groups
  per manufacturer or catalog; *Groups* – groups of users with their members;
  *Directories* – the Active Directory and LDAP connections with a test of the
  connection and of a sign-in; *Catalogs* – create, change and delete catalogs, the
  permissions of users and groups on each catalog, and install or update the bundled
  catalogs; *Import parts* (CSV,
  JSON, BMEcat, EPLAN XML); *Export and backup* – the parts as CSV or JSON, the
  catalogs, the users (CSV, without passwords) and the access settings (JSON, which
  restore again), and a backup of the whole database. The page only offers what the
  user may do; the server checks every change.
- Backup (`GET /backup`, administrators): one SQLite file with the parts, their
  history and files, the catalogs, the users with their password hashes and the
  favorites, whatever the storage of the server. Restore it by starting a server with
  `--data <backup>.sqlite`, or copy it into a new database with `--migrate-from`. The
  access settings file (directories, roles, grants) is not part of it: keep a copy of
  `<data>.access.json` too.
- REST API (`/api/v1`, see `openecraft_parts::api`): `GET /info`, `GET /parts`,
  `POST /search`, `GET|PUT|DELETE /parts/{number}`, `GET /parts/{number}/accessories` (its accessories
  and, for an accessory, the parts it fits), `GET /parts/{number}/history[/{revision}]`,
  `POST|DELETE /parts/{number}/attachments`, `GET /files/{id}` (PDF, raster images and
  text are shown in the browser, other files such as SVG and XML are downloaded, so
  that no file runs a script on the server's address), `POST /import` (CSV,
  JSON or BMEcat), `GET /export.csv` (all parts, or those of the query in the address:
  `q`, groups, manufacturer, catalog, lifecycle, sort), `GET /export.json` (the same as a
  JSON part list that `/import` reads), `GET /backup`, `GET|PUT|DELETE /catalogs/{id}`,
  `GET /bundled`, `POST /bundled/{id}` (install a bundled catalog), `POST /login`,
  `POST /register`, `GET|PUT /registration`, `POST /users/{name}/approval`,
  `POST /logout`, `GET|PUT /favorites`, `GET /users` (with their open sessions and last
  sign-in), `PUT|DELETE /users/{name}`, `DELETE /users/{name}/sessions` (sign out
  everywhere), `GET|PUT /access`, `GET|PUT|DELETE /catalogs/{id}/logo`, `GET|PUT /catalogs/{id}/grants`, `GET /groups`,
  `PUT|DELETE /groups/{name}`, `GET /directories`, `PUT|DELETE /directories/{id}`,
  `POST /directories/{id}/test`, `GET /dashboard`, `GET|PUT /dashboard/settings`.
- In the editor: *Master data › Data portal* – search the server, see the details with
  the front view and the 3D model, add parts to the local database, save their files, add their macros to the macro library and check the
  database for newer versions, phased out and discontinued parts. The details show the
  part's catalog with its logo (hidden with *Settings › Appearance › Data portal*). Logos
  are drawn on a white plate; SVG logos are rendered with `resvg` and the editor's own
  font, so they look the same on the desktop and in the browser. *Devices › Parts
  management › 🌐 Parts server* downloads all parts or uploads parts. The project check
  warns about devices with discontinued parts; `use_successor_parts` changes them over.
- The server speaks plain HTTP, so tokens and passwords cross the network unencrypted.
  Keep it on `127.0.0.1` behind a TLS reverse proxy when it is reachable from other
  machines, e.g. with [Caddy](https://caddyserver.com/), which fetches the certificate:
  `caddy reverse-proxy --from parts.example.com --to 127.0.0.1:8090`.

## EPLAN Data Portal

The parts handling also connects to the EPLAN Data Portal: choose *EPLAN Data Portal*
as the server type in the data portal window or in *Parts management › 🌐 Parts
server*, keep the address `https://dataportal.eplan.com/api` and enter a personal
access token (created for the Data Portal in the profile of your EPLAN account,
ePulse). The portal then works like a parts server: search by text, manufacturer and
product group, part details with the commercial data (CSV), DXF drawings and
pictures, adding parts to the local database and the update check of the database.

The PAT is sent as `Authorization: Bearer PAT:<token>` (pasted `PAT:` and `Bearer `
prefixes are accepted too). The connection test reads the public catalog and then
downloads the commercial data of its first returned part to check authenticated
download access. A successful public search alone does not prove authentication;
`/authorization` is not used because it rejects PATs accepted by the download gateway. DXF and
commercial-data downloads POST the part id to `/download/{type}/part`, matching the
portal's web client; existing attachment ids in saved projects remain supported. The use of
the portal and of the part data taken from it is governed by EPLAN's terms of use and
the account's license: check them before putting portal parts on a shared parts
server. OpenECraft is not affiliated with EPLAN.

- `openecraft_client::eplan` wraps the portal's REST API (JSON:API): it builds the
  portal's requests from the parts server's queries and turns the answers into
  `ServerInfo`, `SearchResult` and `Part`. `openecraft_client::Connection` offers the
  reading functions of both kinds of server; the editor and the command line use it.
- The connection is read-only: no uploads, catalogs, users, sign-in, favorites,
  history or download of the whole database. The facets count the parts of the shown
  page, as the portal does not count them; catalogs, lifecycle and technical data
  cannot be filtered.
- The portal finds parts by its own ids, not by number: a part loaded from it keeps
  the id (`portal_id`), so adding marked parts and the update check load them in one
  request. The update check compares only the parts that came from the portal; a
  single part number is found by an exact search.
- Manufacturers are filtered by their short name, product groups by the id of the
  EPLAN catalog entry: (generic product group + 1) · 10⁸ + (product group + 1) · 10⁴ +
  (subgroup + 1), e.g. `200050000` for electrical engineering / plugs.
- Parts in EPLAN format (macros, connection points) come as EDZ files from the portal
  web page; import them like other parts files (see [parts management](features/parts.md)).
- Only the desktop version and the command line reach the portal: it allows no
  cross-origin requests from browsers, so the web version would need a proxy.

### Shared Data Portal PAT on Windows

The editor and CLI/MCP can use the same PAT through Windows Credential Manager.
Enter the PAT in the editor's Data Portal, select **Remember token on this computer**,
and use **Test connection**. Remembering is off by default: without this opt-in the
PAT stays in the session. Only a successful protected-service check saves a replacement.
A storage error is shown separately and does not invalidate the connection. The PAT is stored for
the current Windows user on this computer, not in a project, JSON settings,
environment file or MCP response.

Leave the PAT field empty to use the saved value automatically. On the first
request, the GUI loads it in the background before constructing that request;
concurrent requests share the lookup. The UI thread never waits for the credential
read. The result, including a missing token or read error, is cached for the session.
**Test connection** forces a fresh lookup after an external replacement/removal or
a read error. **Forget saved token** clears the cache as well. A different endpoint
or explicit token uses a separate cache, so an older lookup cannot replace it.

MCP uses the saved value when
`parts_connect` selects `Eplan` without an explicit token. Each request resolves the
credential again, so replacements and removals take effect without restarting MCP.
An explicit token supplied for the current EPLAN connection takes precedence;
MCP does not save explicit session tokens to the shared store.

Credentials are scoped to the HTTPS origin, port and API path. Changing the server
does not reuse another endpoint's saved PAT. **Forget saved token** removes only
the current portal's entry and clears its GUI field. Other platforms retain
session-only credentials; no plaintext persistence is used as a fallback.

When upgrading from a version that kept PATs only in memory, enter the PAT once in
the updated editor, select **Remember token on this computer**, and test the connection.
It does not need to be entered in MCP. Connection tests query the protected
file-provider token service and public catalog metadata; they do not download a
commercial-data file or depend on a particular catalog part having one.

## Parts manager

`openecraft-parts-manager` is a standalone application that maintains the parts of a
parts server: create new parts, edit existing ones (including 2D / 3D models), rename
and delete parts, upload CSV / JSON parts files and export all parts. It uses the same
part editor as the editor's parts management and runs on the desktop and in the browser.

```bash
cargo run --release -p openecraft-parts-manager        # desktop
cd parts-manager && trunk serve                        # web, http://127.0.0.1:8082
cd parts-manager && trunk build --release              # static files in parts-manager/dist/
```

Enter the server URL and the access token (kept in memory only) and click *Connect*, or
sign in with a user name and password. Changes are saved to the server with *Save*
(`Ctrl+S`); the editor asks before unsaved changes are discarded and before a part is
deleted. *Files and history* above the part editor uploads and removes the files of a
part and shows its changes; *Catalogs* maintains the manufacturer catalogs (with their
logos: *Choose logo…*, *Remove logo*, for a catalog without unsaved changes) and *Users*
(administrators) the accounts and roles. *Logos* beside the language menu shows or hides
the logos in the list of catalogs (remembered).
