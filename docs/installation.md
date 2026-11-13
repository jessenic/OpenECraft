# Installation and building

Packages for Windows, Linux and macOS, the web version, and building from source.

Released versions are on the [releases page](https://github.com/nilres/OpenECraft/releases)
with a `SHA256SUMS` file; how they are made is in [releasing](releasing.md).

Building from source requires a recent stable Rust toolchain (`rustup update stable`);
releases are built with the version pinned in `.github/workflows/ci.yml`. The installer
scripts below and `trunk build --release` also need
[cargo-about](https://github.com/EmbarkStudios/cargo-about)
(`cargo install cargo-about@0.9.3 --locked --features cli`) for
`THIRD-PARTY-LICENSES.html`, which every package contains next to `LICENSE-MIT` and
`LICENSE-APACHE`.

## Desktop (Windows / Linux / macOS)

```bash
cargo run --release -p openecraft
```

On Linux you need the usual windowing / dialog libraries, e.g. on Debian/Ubuntu:

```bash
sudo apt install libxkbcommon-dev libgtk-3-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev
```

A project file given on the command line is opened at start (`openecraft plant.oec`).

## Windows installer

`OpenECraft-<version>-setup.exe` installs the editor with a Start menu entry, without
administrator rights (or for all users in Program Files), in English, German, French or
Spanish after the Windows language. Optional: the parts manager, the command line / MCP
server (added to the `PATH` on request), the parts and project servers, a desktop
shortcut, and opening `.oec` / `.oecz` / `.oecp` files by double-click. It is uninstalled in
*Settings › Apps*; projects and settings are kept. CI builds it on every push (artifact
`openecraft-windows-installer`); locally it needs
[Inno Setup 6](https://jrsoftware.org/isinfo.php) (`winget install JRSoftware.InnoSetup`):

```bash
powershell -ExecutionPolicy Bypass -File installer/windows/build.ps1   # → target/installer/
```

The binaries link the C runtime statically, so no Visual C++ redistributable is needed.
For unattended installation (software distribution) run the setup with
`/VERYSILENT /SUPPRESSMSGBOXES`, add `/ALLUSERS` for all users and select parts with
`/COMPONENTS="main,partsmanager,cli,servers"` and `/TASKS="fileassoc,addtopath"`.
With `WINDOWS_SIGN_CERT=<.pfx file>` and `WINDOWS_SIGN_PASSWORD` the script signs the
programs, the setup and the uninstaller (signtool of the Windows SDK); otherwise they are
unsigned and SmartScreen asks for confirmation on first run (see [releasing](releasing.md)).
`installer/icon.py` draws the application icon (`assets/icon.png`, `assets/icon.ico`,
`assets/icon.icns`, `assets/icon.svg`).

## Linux packages

`installer/linux/build.sh` builds the packages for the architecture of the build machine
into `target/installer/`. Each contains the editor and the parts manager (with menu
entries in English, German, French and Spanish), the command line / MCP server and the
parts and project servers. With deb, rpm, Flatpak and tar.gz `.oec` / `.oecz` / `.oecp` files open
by double-click.

```bash
bash installer/linux/build.sh                        # tar.gz, deb, rpm, AppImage
bash installer/linux/build.sh flatpak snap           # sandboxed packages
```

| Package | File | Install | Build tool |
| --- | --- | --- | --- |
| Debian, Ubuntu | `openecraft_<version>_amd64.deb` | `sudo apt install ./openecraft_*.deb` | `dpkg-dev` |
| Fedora, RHEL, openSUSE | `openecraft-<version>-1.x86_64.rpm` | `sudo dnf install ./openecraft-*.rpm` | `rpmbuild` |
| Any distribution | `OpenECraft-<version>-x86_64.AppImage` | `chmod +x`, start it | Debian or Ubuntu, `patchelf`, `appimagetool` and its runtime (downloaded, checksums verified) |
| Any distribution | `OpenECraft-<version>-x86_64.flatpak` | `flatpak install --user OpenECraft-*.flatpak` | `flatpak-builder` |
| Any distribution | `openecraft_<version>_amd64.snap` | `sudo snap install --dangerous openecraft_*.snap` | `snapcraft` |
| Any distribution | `OpenECraft-<version>-linux-x86_64.tar.gz` | `./install.sh` (to `~/.local`) | `tar` |

The programs are `openecraft`, `openecraft-parts-manager`, `openecraft-cli`,
`openecraft-parts-server` and `openecraft-project-server`. In the AppImage they are
started with `OpenECraft-*.AppImage parts-manager|cli|parts-server|project-server …`, in
the Flatpak with `flatpak run --command=openecraft-cli io.github.nilres.OpenECraft …`
and in the snap as `openecraft.cli`, `openecraft.parts-manager`, … CI builds them on every
push (artifacts `openecraft-linux-packages`, `openecraft-flatpak`, `openecraft-snap`);
deb, rpm, AppImage and tar.gz are built on Debian 12 and run with glibc 2.36 or newer.
The AppImage carries the window libraries that are not on every system (xkbcommon,
Xcursor, Xi: `appimage_libs` in `installer/linux/build.sh`); the build refuses one that
needs another library or a newer glibc than the programs.
The desktop entries, MIME types and AppStream data (`io.github.nilres.OpenECraft`) are
in `installer/linux/`; a new version needs a `<release>` in the metainfo file.

## macOS disk image

`installer/macos/build.sh` builds `OpenECraft-<version>-macos.dmg` (macOS 11 or newer,
universal binaries for Apple silicon and Intel) with the apps *OpenECraft* and
*OpenECraft Parts Manager*; drag them to *Applications*. The command line and the servers
are in the app bundle:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
bash installer/macos/build.sh                        # → target/installer/
/Applications/OpenECraft.app/Contents/MacOS/openecraft-cli help
```

Without `MACOS_SIGN_IDENTITY` the apps are signed ad hoc, so Gatekeeper asks for
confirmation on first start (*System Settings › Privacy & Security › Open Anyway*). With
a Developer ID certificate (`MACOS_SIGN_IDENTITY="Developer ID Application: …"`) and a
`notarytool` keychain profile (`MACOS_NOTARY_PROFILE`, `MACOS_NOTARY_KEYCHAIN` when it is
not in the login keychain) the image is signed and notarized.
Projects are opened with *File › Open project*; opening them by double-click in the Finder is not
supported yet. CI builds the image on every push (artifact `openecraft-macos-dmg`).

## Updates

When it starts, the editor asks GitHub in the background for the latest release
(`https://api.github.com/repos/nilres/OpenECraft/releases/latest`); without a connection
it carries on without a message. A newer release is downloaded, checked against the
`SHA256SUMS` of the release and installed:

| Installation | Update |
| --- | --- |
| Windows setup | `OpenECraft-<version>-setup.exe` runs silently (`/SILENT`) when the editor is closed, in the folder and with the components of the installation; *Restart and update* starts the editor again afterwards |
| AppImage | the AppImage file is replaced at once; the new version starts next time |
| macOS app | `OpenECraft.app` (and `OpenECraft Parts Manager.app` next to it) are replaced from the disk image at once |
| deb, rpm, Flatpak, Snap, tar.gz, own build | the status bar only tells of the new version and links its release page |

The status bar shows the download and *Restart and update*; *File › Help › Check for
updates* looks again and reports every outcome. *Settings › Updates* switches the check
at start off, the environment variable `OPENECRAFT_NO_UPDATE_CHECK` (any value) does so
for all users of a computer. The web version has no update of its own.

## Docker images of the servers

The parts server and the project server are also published as Docker images on
every release and on every push to `master`, built by `installer/docker/Dockerfile`
(Debian 12, `linux/amd64`, running as the user `openecraft`, data in the volume
`/data`, licenses in `/usr/share/doc/openecraft/`):

| Image | Port | Tags |
| --- | --- | --- |
| `ghcr.io/nilres/openecraft-parts-server` | 8090 | `<version>`, `<major>.<minor>`, `latest` (releases), `edge` (master), `sha-<commit>` |
| `ghcr.io/nilres/openecraft-project-server` | 8091 | the same |

```bash
docker run -d --name parts -p 127.0.0.1:8090:8090 -e OPENECRAFT_TOKEN=<secret> -v parts:/data ghcr.io/nilres/openecraft-parts-server
docker run -d --name projects -p 127.0.0.1:8091:8091 -e OPENECRAFT_PROJECT_TOKEN=<secret> -v projects:/data ghcr.io/nilres/openecraft-project-server
```

The settings of each image are in `/etc/openecraft/parts-server.toml` and
`project-server.toml` (`bind = "0.0.0.0:<port>"`, `data` in `/data`); mount your own
file there or add command line options after the image name
(`… openecraft-parts-server --install-catalogs all`). The first administrator comes
from the environment too: `-e OPENECRAFT_ADMIN_USER=admin -e
OPENECRAFT_ADMIN_PASSWORD=<password>` (`OPENECRAFT_PROJECT_ADMIN_USER` /
`OPENECRAFT_PROJECT_ADMIN_PASSWORD` for the project server) creates the account, or
resets its password, at every start.
A bind mount instead of a volume must be writable by the user of the image (uid 1000,
or start the container with `--user`). `installer/docker/compose.yaml` starts both
servers with Docker Compose. The servers speak plain HTTP: keep them on `127.0.0.1`
behind a TLS reverse proxy when other machines reach them
([project server](project-server.md)). Building the images yourself:

```bash
docker build -f installer/docker/Dockerfile --target parts-server -t openecraft-parts-server .
docker build -f installer/docker/Dockerfile --target project-server -t openecraft-project-server .
```

CI builds both images on every push, starts them once and checks that they answer;
on `master` and for a release tag it pushes them to the GitHub Container Registry
(job `docker`).

## Web (WebAssembly)

```bash
rustup target add wasm32-unknown-unknown
cargo install trunk@0.22.1 --locked
cd openecraft && trunk serve            # development server on http://127.0.0.1:8080
cd openecraft && trunk build --release  # static files in openecraft/dist/ – host them on any web server
```

Both web builds put `LICENSE-MIT`, `LICENSE-APACHE` and `OFL-NotoSans.txt` next to the
page, the release build also `THIRD-PARTY-LICENSES.html` (`installer/web/licenses.sh`,
on Windows `licenses.ps1`, run by trunk after the build); keep these files when hosting
the page.

In the browser "Save" downloads the project file and "Open" uses the file picker.

## Tests

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo clippy -p openecraft -p openecraft-parts-manager --target wasm32-unknown-unknown
```

All three must pass without warnings (CI runs them with `-D warnings`).
