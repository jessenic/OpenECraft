# Access control of the servers

The parts server and the project server share one access model: users sign in with a
local account or with a directory (Active Directory, OpenLDAP or another LDAP server),
and what they may do comes from **roles** given to them or their **groups** by
**grants**, per project (project server) or per manufacturer (parts server).

- Rules and their evaluation: `openecraft_parts::access` (shared with the clients)
- Directories and the settings file: `openecraft-auth` (`directory.rs`, `config.rs`)
- Rights and built-in roles: `openecraft_core::collab::{ProjectRight, ProjectRole}` and
  `openecraft_parts::portal::{PartsRight, Role}`

## Rights and built-in roles

Project server (the scope of a grant is a project name):

| Right | Allows |
| --- | --- |
| `view` | see the project in the list, open it read-only |
| `review` | save review records (comments, markup, approvals) without locks |
| `edit_pages` | lock and change pages |
| `edit_project` | lock and change the project data (page list, devices, parts, settings) |
| `create` | create projects (the scope is the new name) |
| `delete` | delete a project nobody has open |
| `manage_access` | change the grants on the project |
| `administer` | users, roles and all grants (only from grants on `*`) |

| Role | Rights |
| --- | --- |
| `viewer` | view |
| `reviewer` | view, review |
| `editor` | view, review, edit_pages, edit_project, create |
| `manager` | editor + delete, manage_access |
| `admin` | everything |

A signed-in user who creates a project becomes its `manager` (a grant on the new
project), so project owners manage who works on their projects.

Parts server (the scope of a grant is a manufacturer, or one catalog as `catalog:<id>`):

| Right | Allows |
| --- | --- |
| `read` | browse, search, download files, favorites (whole server) |
| `edit_parts` | create and change parts and their files, import |
| `delete_parts` | delete parts |
| `edit_catalogs` | create, change and delete catalogs |
| `administer` | users, roles and grants (whole server) |

Built-in roles: `reader` (read), `manufacturer` and `editor` (read, edit_parts,
delete_parts, edit_catalogs), `admin` (everything). An account's own role still works
as before: a manufacturer user has the manufacturer role on their manufacturer only.

The rights on a part or a catalog are those on its manufacturer together with those on
its catalog (`catalog:siemens`): a grant on a catalog lets users and groups change the
catalog and its parts whatever their manufacturer, and a deny grant on either takes
the right away. Administrators set the grants of a catalog in *Administration ›
Catalogs › Permissions* (`GET` / `PUT /api/v1/catalogs/{id}/grants`); reading stays a
right of the whole server.

## How rights are resolved

1. The account's built-in role (project server: on its projects; none for users that
   came from a directory).
2. Every grant of the policy whose subject matches the user (`{"user": "anna"}`) or one
   of their groups (`{"group": "corp:Engineering"}` of a directory, `{"group":
   "Wiring"}` defined on the parts server). Names and scopes take `*` and `?` wildcards
   and are case insensitive; `{"user": "*"}` is every signed-in user.
3. The role of anonymous requests (`anonymous`), which signed-in users have as well.
4. **Deny grants win:** rights of a grant with `"deny": true` are taken away on its
   scope, whatever the other grants give.

Server-wide rights (`administer`, and `read` on the parts server) only come from
grants on `*`. The server token (`--token`) keeps full access.

## The settings file

`access.json` in the project server's data directory, `<data>.access.json` next to the
parts database (both changeable with `--access <file>`). A missing file means: no
directories, no defined roles or grants; on the parts server anonymous requests may
read (`"anonymous": "reader"`). Administrators change roles, grants and `anonymous`
through the API (`GET|PUT /api/v1/access`), which rewrites the file; directories are
only configured in the file. On the parts server the web page does so too
(*Administration › Access rights*, which also exports and restores them).

```json
{
  "directories": [
    {
      "id": "corp",
      "title": "Corporate AD",
      "kind": "active_directory",
      "url": "ldaps://dc1.corp.example",
      "base_dn": "DC=corp,DC=example",
      "bind_dn": "CN=svc-openecraft,OU=Service,DC=corp,DC=example",
      "bind_password_env": "OPENECRAFT_LDAP_PASSWORD",
      "nested_groups": true,
      "allowed_groups": ["OpenECraft *"]
    },
    {
      "id": "lab",
      "kind": "ldap",
      "url": "ldap://ldap.lab.example",
      "starttls": true,
      "user_bind": "uid={user},ou=people,dc=lab,dc=example",
      "base_dn": "ou=people,dc=lab,dc=example",
      "group_filter": "(&(objectClass=groupOfNames)(member={dn}))",
      "group_base_dn": "ou=groups,dc=lab,dc=example"
    }
  ],
  "anonymous": "",
  "roles": [
    { "name": "checker", "description": "Checks drawings", "rights": ["view", "review"] }
  ],
  "grants": [
    { "subject": { "group": "corp:Engineering" }, "role": "editor", "scope": "Plant*" },
    { "subject": { "group": "corp:Quality" }, "role": "checker", "scope": "*" },
    { "subject": { "group": "corp:Contractors" }, "role": "viewer", "scope": "Secret*", "deny": true },
    { "subject": { "group": "corp:OpenECraft Admins" }, "role": "admin", "scope": "*" }
  ]
}
```

On the parts server `"registration": true` lets users register themselves as readers
(switched in *Administration › Access rights*; the start option `--registration`
overrides it, see [docs/parts-server.md](parts-server.md)); their accounts wait for the
approval of an administrator unless `"registration_approval": false`.
`"dashboard": false` hides the dashboard of the web page (*Administration ›
Overview*; the start option `--dashboard` overrides it).

### Groups

The parts server keeps groups of users in `groups` (*Administration › Groups*, `GET
/api/v1/groups`, `PUT` / `DELETE /api/v1/groups/{name}`):

```json
"groups": [{ "name": "Wiring", "description": "Panel shop", "members": ["anna", "bob"] }]
```

Members are user names, local users and users of a directory alike (by the name they
sign in with). Grants to `{"group": "Wiring"}` apply to the members next to the groups
of their directory; group names cannot hold `:`, which separates the directory groups.
The groups of a user (*Users*, `groups` of `PUT /api/v1/users/{name}`) are memberships:
saving them adds the user to the named groups (new names become groups) and removes
them from the others. Groups that local accounts named before are turned into groups of
the server at startup. Deleting a group deletes the grants to it, deleting a user their
memberships; names of members cannot be registered, and a change that would leave
nobody who manages users is refused.

### Directories

Administrators set the directories up in *Administration › Directories* too (`GET
/api/v1/directories`, `PUT` / `DELETE /api/v1/directories/{id}`), which shows no
password (an empty password keeps the stored one) and tests the settings: *Test*
connects and binds with the service account, with a user name and password it signs the
user in and shows their groups (`POST /api/v1/directories/{id}/test`).

| Key | Meaning |
| --- | --- |
| `id` | short name; groups appear as `id:group`, users of the directory show it |
| `kind` | `active_directory` (default) or `ldap` |
| `url` | `ldap://…` or `ldaps://…`; `starttls` upgrades `ldap://` |
| `insecure_tls` | accept any certificate (tests only); otherwise the system's CA store is used |
| `bind_dn`, `bind_password` / `bind_password_env` | service account to look users up |
| `user_bind` | without service account: bind name of the user, `{user}@corp.example` or `uid={user},ou=…` |
| `base_dn` | where users are searched |
| `user_filter` | default `(&(objectClass=user)(sAMAccountName={user}))` / `(&(objectClass=person)(uid={user}))` |
| `name_attribute` | account name, default `sAMAccountName` / `uid` |
| `nested_groups` | Active Directory: also groups of groups (`LDAP_MATCHING_RULE_IN_CHAIN`) |
| `group_filter`, `group_base_dn` | search the groups (`{dn}`, `{user}`) instead of reading `memberOf` |
| `allowed_groups` | only members of these groups may sign in (empty: everybody) |
| `timeout_secs` | connection and request timeout (default 10) |

Sign-in tries the local accounts first; a name without local account goes to the
directories in order (or only to the one named in the request's `provider`). The first
directory that knows the user checks the password (a bind as the user; empty passwords
are refused before the directory is asked) and returns the groups. The server keeps an
account for the directory user (no password; groups refreshed at every sign-in), so
administrators see them in the user list and session tokens work as for local users.
A name that already belongs to a local account cannot sign in through a directory.

Passwords of local accounts need at least 8 characters. After 5 failed sign-ins of a
user name (local or directory) from one client address every further attempt of that
name from there waits, 1 s at first and twice as long after every further failure, up
to 5 minutes; after 30 failed sign-ins of a client with any names its attempts wait
the same way. The servers answer `429 Too Many Requests` meanwhile. Others cannot lock
a user out this way; a successful sign-in forgets the failures of the name (not those
of the client), and failures are forgotten after an hour. A name without account takes
as long to refuse as a wrong password, so the answers do not tell which names exist.
Errors of both servers come as `{"error": "…"}`, also for unknown routes and requests
that do not parse.

A session (the token of a sign-in) ends at sign-out, when the password is changed and
30 days after the sign-in; a user keeps at most 20. Sessions stored by earlier versions
have no start and count as ended: their users sign in once more. Requests that change
something are checked before their body is read, so unknown callers cannot make a
server read large bodies; sign-in (and the parts server's search) take at most 64 kB.

Both servers send `X-Content-Type-Options: nosniff`, `X-Frame-Options: DENY`,
`Referrer-Policy: no-referrer` and a content security policy with every answer. The
user and access files (`users.json`, `access.json`, a JSON parts database) are written
readable for their owner only (Unix). Prefer `bind_password_env` to `bind_password`
(the servers warn about a password in the file) and `OPENECRAFT_TOKEN` /
`OPENECRAFT_PROJECT_TOKEN` or the configuration file to `--token`, which other users of
the machine see in the list of processes. Neither server speaks TLS: put a reverse proxy
with HTTPS in front of a server that is reached over the network.

Values are escaped for LDAP filters and DNs. Keep the service account read-only.

## Clients

- Editor, *Tools › Project server…*: shows the rights on the open project; viewers and
  reviewers cannot change it, project managers edit the grants on it in the *Project
  access* section.
- Parts manager: the user editor shows the directory and groups of a user and edits the
  groups of the server the user is a member of.
- Command line / MCP: `parts_access` (also sets `anonymous`), `parts_grant` (with
  `deny`, or `revoke` to remove grants), `parts_role` (define, change, `delete`),
  `parts_registration` (`enabled`, `approval`), `parts_user_approve` and
  `parts_user_save` with `groups`.
