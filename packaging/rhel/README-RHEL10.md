# Support Step Recorder on RHEL 10

## Install (same flow as every Linux build)

```bash
tar xzf ssr-<version>-rhel10-x86_64.tar.gz
cd ssr-<version>-rhel10-x86_64
sudo ./packaging/install-linux.sh
```

The script installs the app (`/usr/local/bin/ssr`), its entry in the
applications menu, and the input-capture rule. **Log out and back in**, then
open **Activities**, type **Support Step Recorder** and click it (or run `ssr`
in a terminal). Right-click the icon → *Pin to Dash* to keep it in the dash.

`ssr guide.ssrproj` or `ssr recording.zip` opens that file directly; in Files,
*Open With → Support Step Recorder* does the same.

Run it as your normal user, never with `sudo`. To try it without installing
anything, `./ssr` from the extracted folder works too (in a terminal; Files
does not run programs on double-click).

### Runtime packages

A RHEL 10 Workstation (GNOME) install already has them. On a minimal system:

```bash
sudo dnf install pipewire-libs libX11 libxcb \
    libxkbcommon libxkbcommon-x11 mesa-libGL mesa-libEGL libwayland-client \
    libXcursor libXi libXrandr \
    xdg-desktop-portal xdg-desktop-portal-gnome
```

The binary links directly only against glibc, `libX11`, `libxcb`, `libXau` and
`libpipewire-0.3` (`packaging/linked-libraries.txt` in this archive, from `ldd`
on the build machine); the windowing and OpenGL libraries are loaded at start,
and D-Bus (portals) is spoken natively, without `libdbus`.

## What `install-linux.sh` does — and when you need it

On **Wayland** (the RHEL 10 default), recording reads keyboard and mouse
events from `/dev/input`. The script installs
`/etc/udev/rules.d/60-ssr-input.rules` (`TAG+="uaccess"`), which gives the
user of the active local session read access to input devices, then reloads
udev. Log out and back in if capture does not start right away.

> ⚠️ This lets applications run by the active user read all keyboard input.
> On managed or regulated workstations, get approval first. Never run `ssr`
> as root to work around missing access.

The rule is **not** needed to edit guides: opening projects (`.ssrproj`),
opening or importing earlier recordings (`.zip`), editing, saving and
exporting all work without it. A workstation where input capture is not
allowed can still be used to prepare guides from recordings made elsewhere.

Screen capture on Wayland goes through `xdg-desktop-portal` + PipeWire: GNOME
asks once which window or screen to share.

Uninstall (app, menu entry and rule):

```bash
sudo rm -f /usr/local/bin/ssr /usr/local/share/applications/support-step-recorder.desktop \
    /usr/local/share/icons/hicolor/scalable/apps/support-step-recorder.svg \
    /etc/udev/rules.d/60-ssr-input.rules
sudo udevadm control --reload-rules && sudo udevadm trigger --subsystem-match=input
```

## Where files go

| What | Where |
|---|---|
| Screenshots of a recording, open guides, crash-recovery copies | `~/.cache/ssr/workspaces/<id>/` (mode 0700, files 0600) |
| Preferences (theme, language, last folder) | `~/.local/share/support-step-recorder/` |
| Projects and exported guides | wherever you save them |

Nothing is sent over the network.

## Build it yourself

On RHEL 10 (subscribed, with the CodeReady Builder repository) or an EL10
rebuild, from a checkout of the sources:

```bash
sudo packaging/rhel/build-rhel10.sh --install-deps   # once: dnf install the build deps
packaging/rhel/build-rhel10.sh                        # → dist/ssr-<version>-rhel10-<arch>.tar.gz
```

From any Linux machine with podman or docker:

```bash
packaging/rhel/build-in-container.sh                  # CentOS Stream 10 image by default
```

Rust 1.95 or newer is required (egui 0.37). If the `rust-toolset` of your
RHEL minor release is older, install a newer toolchain with rustup on the
build machine only; the resulting binary does not depend on it.

### Air-gapped build

On a connected machine with the same `Cargo.lock`:

```bash
packaging/rhel/build-rhel10.sh --vendor     # → ssr-vendored-source.tar.gz + .sha256
```

Transfer the archive and its checksum, then on the offline RHEL 10 machine
(build RPMs from an approved local repository):

```bash
sha256sum -c ssr-vendored-source.tar.gz.sha256
mkdir ssr && tar xzf ssr-vendored-source.tar.gz -C ssr && cd ssr
packaging/rhel/build-rhel10.sh --offline
```
