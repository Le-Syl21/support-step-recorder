# Master Guide Editor — implementation notes

This records how the *SSR Master Guide Editor* handoff was implemented on top
of v0.2.0 (`3e785468`), what was verified and how, and where the
implementation deviates from the handoff.

## Workflow now

```
Record ──Stop──▶ Editor ──Save project──▶ guide.ssrproj ──Open project──▶ Editor
                   ▲  │
   Open recording ─┘  └──Export guide──▶ guide.zip (report.html + steps.json + images)
   Import steps… (several ZIP / .ssrproj, preview + checkboxes)
```

Stopping no longer opens “Save as”. The guide stays in a private workspace
until it is saved or exported.

## Code map

### `ssr-core`

| File | Role |
|---|---|
| `model.rs` | `Step` gains `id`, `title_override`, `notes`, `source: StepSource`, `kind: StepKind` — all `serde(default)`, so v0.2.0 `steps.json` loads unchanged and exports stay readable by v0.2.0 (optional fields only). `Step::manual`, `auto_title`, `display_title`. |
| `session.rs` | Assigns an id to each recorded step. Capture loop untouched. |
| `editor.rs` | **New.** `EditorDocument`: steps in guide order, metadata, workspace, asset store, revisions (`is_dirty` vs `is_exported`), undo/redo (50 entries), recovery copy. Commands: `set_title`, `set_notes`, `detach_screenshot`, `attach_screenshot`, `delete_step`, `move_step`, `insert_steps`, `import_steps`/`import_many`, `add_manual_step`, `redact_text`, `set_guide_info`. Every command renumbers 1..N. |
| `workspace.rs` | **New.** Private workspaces under `$XDG_CACHE_HOME/ssr/workspaces/<uuid>` (0700, lock file held while open). `scan_abandoned` finds crashed sessions with unsaved work and removes the others. |
| `assets.rs` | **New.** Content-addressed store `assets/<sha256>.<png|jpg|webp>` (files 0600, atomic). Format sniffed from magic bytes; dimensions checked from the header before decoding; full decode under an allocation limit. |
| `archive.rs` | **New.** `SafeArchive`: refuses unsafe member names (absolute, `..`, `\`, `:`, NUL), symlinks, duplicate names, too many entries, too large (declared and actual decompressed bytes). Members are read into memory only on demand; nothing is extracted by name. |
| `import.rs` | **New.** `import_recording` (v0.2.0 ZIP or `.ssrproj` as a source; `report.html` never read), `import_image_file`, `ImportLimits` (3000 entries, 1000 steps, 8 MiB JSON, 25 MiB/image, 256 MiB total, 100 MP). Transactional: a fatal error removes the images already stored. A broken or missing image is a per-step warning — the step can still be imported as text. |
| `project.rs` | **New.** `.ssrproj` v1 (`project.json`, `steps.json`, referenced `assets/` only; no HTML). `save_project` and `write_atomically` (sibling temp file, fsync, rename, fsync dir). `open_project` refuses newer `schema_version`, verifies each image against its hash, guarantees unique ids. |
| `export.rs` | `export_guide`: custom title else generated description (in the report language), notes escaped with `pre-wrap`, numbering 1..N, only referenced images, error on a missing image, no “elapsed since start” on a guide, empty-guide replay guard, `'` escaped, print CSS, atomic write. `export_zip(Session)` kept (now atomic too). |

### `ssr-gui`

| File | Role |
|---|---|
| `app.rs` | Stop → `EditorDocument::from_session` → editor. Background jobs (save, export, open, read for import) with a progress modal; editing is disabled meanwhile. `Intent` + unsaved-changes prompt (new recording, open, close, quit — window close is intercepted). Draft buffer for title/notes. Keyboard: Ctrl+Z, Ctrl+Shift+Z/Ctrl+Y, Ctrl+S, Ctrl+Shift+S, Ctrl+E, Alt+↑/↓, Delete. Autosave of the recovery copy (3 s throttle). `ssr <file>` opens a file at start. |
| `review.rs` | Timeline with drag and drop (insertion line, edge scrolling), step properties panel (title, instructions, move, delete, add/replace/remove screenshot, redact typed text), source metadata. |
| `screens.rs` | Start screen: Open project / Open recording, recovery banner. Editor header: title (click to edit), saved/exported pills, Undo/Redo, Import…, Step, Image…, Save project, Export guide…, New recording, More… (compact on narrow windows). |
| `editor_sources.rs` | **New.** Import dialog: recordings with step thumbnails/titles/checkboxes, All/None, insert at end / after selection / at start, per-step image warnings. |
| `dialogs.rs` | Unsaved changes, privacy notice (before the first recording), guide details, “replace or insert?” for an added image, warnings, job progress. |
| `steps.rs`, `prefs.rs`, `icons.rs`, `chrome.rs`, `main.rs` | Id-based rows and snapshots; `privacy_ack` and last folder in prefs; new icons; error colour in the status bar. |

### Packaging

`packaging/rhel/`: `build-rhel10.sh` (deps check/install, MSRV check, build,
tarball with the same `./ssr` + `./packaging/install-linux.sh` layout,
`--vendor`/`--offline` for air-gapped builds), `build-in-container.sh`,
`README-RHEL10.md`. `install-linux.sh` gained only a `restorecon` call for
SELinux. CI: `rhel10` job (x86_64 + aarch64) in a CentOS Stream 10 container;
its tarballs join the release.

## Acceptance status

Automated: `cargo test --workspace` (65 tests). Manual: GUI driven with
xdotool on Xvfb (X11) on Ubuntu 24.04; see “Not verified”.

| ID | How verified | Result |
|---|---|---|
| A01 Stop → editor, no Save As | `tests/editor.rs::a01…`; manual (X11, 3 clicks recorded) | ✅ |
| A02 Rename + notes | `a02_rename_and_notes`; manual (typing, Enter, blur) | ✅ |
| A03 Detach → orphan excluded | `a12_export_contains_only_what_is_referenced` | ✅ |
| A04 Delete renumbers | `a04_a05…`; manual (Delete key) | ✅ |
| A05 Reorder keeps ids | `a04_a05…`; manual (drag and drop) | ✅ |
| A06 v0.2.0 steps.json | `a06_legacy_steps_load_with_defaults` | ✅ |
| A07 Alpha + Beta, no collision | `a07_merge_two_recordings_without_collisions` | ✅ |
| A08 Subset at position | `a08…`; manual (import dialog, after selected) | ✅ |
| A09 Same source twice | `a09_same_source_twice` | ✅ |
| A10 External PNG | `a10_attach_an_external_png` | ✅ (GUI path needs the file portal: not exercised headless) |
| A11 Save + reopen | `a11_project_round_trip`; manual (Ctrl+S, reopen) | ✅ |
| A12 Export contents | `a12…`; report opened offline in Chromium, all images load, no network | ✅ |
| A13 Import while dirty | Import is non-destructive (no prompt); open/new/close prompt — manual | ✅ |
| A14 Cancel Save As / Export | Code path: dialog `None` → nothing happens, not a discard | ⚠️ not exercised (no portal headless) |
| A15 Failed export keeps previous | `a15_failed_export_keeps_previous_file`, `failed_save_keeps_previous_project` | ✅ |
| A16 Undo/redo | `a16_undo_redo_and_dirty_state`; manual (import undone in one step) | ✅ |
| A17 Empty report | `a17_empty_guide_exports` | ✅ |
| A18 EN/FR titles | `a18_titles_and_languages`; manual (FR UI) | ✅ |
| A19 HTML escaping | `a12…` (`<script>`, `<img onerror>`, quotes, `'`) | ✅ |
| A20 Large project | Lazy thumbnails with a per-frame decode budget (existing); 8-step merge tested | ⚠️ not load-tested at 1000 steps |
| A21 Save/reopen, private files | `a11…` checks 0700; manual check of 0700/0600 in `~/.cache/ssr` | ✅ (on Ubuntu, not RHEL) |
| A22 Timestamps preserved | `a11…`, `a12…` (no ⏱ on a guide) | ✅ |
| Archive security table | `tests/archive_security.rs` (traversal, absolute, backslash, symlink, duplicates, malformed, budgets, PNG bomb, broken WebP, report.html ignored, read-only source) | ✅ |
| Crash recovery | `recovers_after_a_crash`; manual (`kill -9`, Recover) | ✅ |

### Not verified

- **RHEL 10 GNOME Wayland end-to-end** (portal consent, evdev capture, file
  chooser portal): no RHEL 10 desktop was available. The RHEL 10 build was done
  in a CentOS Stream 10 container. Run the checklist in
  `packaging/rhel/README-RHEL10.md` / handoff §05 on a real workstation.
- **File dialogs** (`rfd` → xdg-desktop-portal FileChooser) were not exercised
  headless; Open/Import were driven through `ssr <file>` and a local test hook
  that is not part of the code.
- macOS/Windows builds were not run locally (CI covers them).

## Deviations from the handoff

- **Sources panel** lives in the Import dialog (preview, checkboxes, position)
  rather than a permanent third column; the editor keeps two panes plus the
  properties column. The guide's source archives are listed in
  `project.json` and per step in the metadata.
- **Undo** stores the previous step records and metadata (no image bytes) per
  command instead of hand-written inverse commands — same behaviour, fewer
  ways to get an inverse wrong. Limit 50.
- **Delete** has no confirmation dialog: it is undoable (Ctrl+Z) and the
  status bar names what was undone.
- **External images** keep their format (PNG/JPEG/WebP); no conversion to WebP.
- **Opening a project with a broken image** opens it with that image removed,
  a warning, and the guide marked unsaved, instead of refusing the project.
- **`.ssrproj` as an import source** is accepted (same safe reader). A step
  that already carries provenance keeps its original source.
- **Recorder temp folder** moved from the shared `/tmp/support-step-recorder`
  to the private workspace.
- The handoff's `fixtures/*.zip`, `contracts/example-project.json` and starter
  patch were not provided; fixtures are generated by `tests/common` and
  `examples/sample_guide.rs` (output committed in `docs/samples/`).
- **Rust ≥ 1.95** is required (egui/eframe 0.37), not 1.92 as the handoff's
  Rust Toolset link suggests. CentOS Stream 10 ships 1.99; check
  `rustc --version` on your RHEL minor release.

## Commands

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p support-step-recorder-core --example sample_guide -- docs/samples
packaging/rhel/build-in-container.sh        # → dist/ssr-<ver>-rhel10-<arch>.tar.gz
```
