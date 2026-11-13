# Project server

Shared projects for a team: several users work on the same project, each page is locked by one of them.

`openecraft-project-server` stores projects for a team. Several users open the same
project at the same time and work on different pages; a page is edited by one user at a
time.

```bash
cargo run --release -p openecraft-project-server -- --bind 0.0.0.0:8091 --data projects --token <secret>
cargo run --release -p openecraft-project-server -- --config project-server.toml
```

As a container: `docker run -d -p 127.0.0.1:8091:8091 -e OPENECRAFT_PROJECT_TOKEN=<secret>
-v projects:/data ghcr.io/nilres/openecraft-project-server` (see [installation](installation.md)).

- Configuration file (`--config <file>` or `OPENECRAFT_PROJECT_SERVER_CONFIG`): the
  settings of the command line options in TOML (`bind`, `data`, `token`, `admin_user`,
  `access`; see
  [`project-server/project-server.example.toml`](../project-server/project-server.example.toml)),
  as for the parts server.
- Project names differing only in case are one name (Windows and macOS do not tell
  their files apart): requests find a project under any case of its name; Windows device names (`CON`, `NUL`, `COM1` …) and names ending
  with a dot are refused.
- Each project is stored as a split project in a folder of its own
  (`<name>/<name>.oecp` with one small file per page, device and part; it opens in the
  desktop editor and can be kept under version control) next to `<name>.state.json`
  (revision, id counter) in the data directory. A save writes only the files that
  changed. Project files of earlier versions (`<name>.oec`) are converted at the
  first start and kept as `<name>.oec.bak`.
- With `--token` (or `OPENECRAFT_PROJECT_TOKEN`) every request needs
  `Authorization: Bearer <token>`; without it, user accounts and directories everyone
  who reaches the server can change its projects, but only clients on the server's own
  machine may create users and change the access settings (so nobody else takes the
  server over by creating the first administrator).
- The server speaks plain HTTP, so tokens and passwords cross the network unencrypted.
  Keep it on `127.0.0.1` behind a TLS reverse proxy when it is reachable from other
  machines, e.g. with [Caddy](https://caddyserver.com/), which fetches the certificate:
  `caddy reverse-proxy --from projects.example.com --to 127.0.0.1:8091`. The editor
  then uses `https://projects.example.com`.
- **User accounts** (`users.json` in the data directory, Argon2 password hashes):
  viewers open projects read-only, reviewers also save comments, markup and approvals,
  editors lock, change, save and create projects, project managers also delete
  projects and manage who may use them, administrators also manage the users; a user's
  role can be limited to some projects (names with `*` / `?`). `--admin-user <name>`
  (or `OPENECRAFT_PROJECT_ADMIN_USER`) creates the first administrator with the password from
  `OPENECRAFT_PROJECT_ADMIN_PASSWORD` (at least 8 characters, as every password). Users
  sign in from *Tools › Project server…* (user name, password, *Sign in*); the server uses
  the account's name on the locks, whatever the request says, and for the comments,
  replies, markup and approvals a signed-in user adds (approvals of other users stay as
  they are). Anonymous callers cannot use the name of an account. Managers of a project
  may add denials on it but only administrators lift them. User names have at most 64
  characters and no control characters; a new user needs a role, directory groups or a
  grant in the access settings. Signing out closes the projects opened with that
  sign-in and releases their locks; deleting a user closes all of theirs. A failed
  sign-in answers `wrong user name or password` whichever of the two was wrong.
- **Access control** (`access.json` in the data directory, `--access`): users sign in
  with a local account or with Active Directory / LDAP directories; roles (built-in or
  defined by administrators) are granted to users and directory groups per project,
  deny grants take rights away. Whoever creates a project manages it; project managers
  edit its grants in the *Project access* section of the project server window. See
  [docs/access-control.md](access-control.md).
- Locks: every page and the project data (page list and order, devices, parts, panel
  layout, settings) are locked separately. Editing something you have not locked
  requests the lock automatically; if another user holds it, the change is taken back.
  Pages locked by others are read-only and marked with 🔒 and the user's name in the
  page list. Locks end when they are released, when the project is closed, or 90 s after
  the editor stopped sending its status (every 15 s).
- Saving merges only what you have locked into the server's project; everything else
  comes from the server. Changes of other users are loaded automatically.
  A save names the revision it was edited from (`base_revision`; the editor does):
  the server then also takes over changed pages and project data nobody has locked
  (it locks them for you first) and answers with the changes it left out
  (`ignored`): locked by another user, changed by another user since that revision or
  not allowed. With `strict` nothing is saved in that case (`409`). The server keeps
  the last 16 revisions per project in memory; a save from an older revision (or
  without `base_revision`) saves only what is locked.
- Each session gets its own range of object ids, so users never create the same id.
- In the editor: *Tools › Project server…* – enter the server URL, your user name and the
  token, list the projects and open one, or upload the current project. While a shared
  project is open, *Save* (`Ctrl+S`) saves to the server; the window and the page list's
  context menu lock and release pages. The status bar shows *Project server: … /
  Project: …* (*connection lost* while the server does not answer its status); a
  project restored from the last session or closed on the server shows as *Local
  recovery copy of server project: …* and is saved to a local file.
- *File › Settings › On startup* decides what the editor opens when it starts: the
  start page with local files and server projects, the last project (a server project
  is opened on the server again), the project list of the server or a chosen file or
  server project. If the server cannot open the project, a dialog offers to retry, to
  sign in, to choose another project or to go on with the local copy; unsaved changes
  of the copy are saved or discarded first, never overwritten silently.
- REST API (`/api/v1`): `GET /info` (name, version and the number of projects the
  caller may see; answered without token too), `GET|POST /projects` (`POST` needs the
  `project`), `GET|PUT|DELETE /projects/{name}`,
  `POST /projects/{name}/open|status|lock|release|close`, `GET|PUT /projects/{name}/access`,
  `POST /login` (`user` and `password`; `name` is taken for `user`, as on the parts
  server), `POST /logout`, `GET /users`, `PUT|DELETE /users/{name}`, `GET|PUT /access`
  (see `openecraft_core::collab`).
