//! Application state and the session workflow: idle → recording → review →
//! save. The screens themselves live in [`crate::screens`], [`crate::review`],
//! [`crate::chrome`] and [`crate::dialogs`].

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use ssr_core::{export, session::Session, Lang, Progress, Recorder};

use crate::prefs::Prefs;
use crate::steps::{self, Snapshot};
use crate::textures::Textures;
use crate::viewer::Viewer;
use crate::{chrome, dialogs, review, screens, theme};

/// Where the user is in the workflow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// Nothing recorded yet (or the last session was discarded).
    Idle,
    Recording,
    /// A stopped session, to look through and save.
    Review,
}

/// The last notable event, shown in the status bar. Kept as data rather than
/// text so it follows a language switch.
#[derive(Clone, Debug)]
pub enum Status {
    Ready,
    Recording(&'static str),
    Stopped(usize),
    StartFailed(String),
    Exporting,
    Exported(PathBuf),
    ExportFailed(String),
    ExportInterrupted,
    Discarded,
}

impl Status {
    pub fn text(&self, lang: Lang) -> String {
        match self {
            Status::Ready => lang.pick("Prêt.", "Ready.").into(),
            Status::Recording(backend) => {
                format!("{} [{backend}]…", lang.pick("Enregistrement", "Recording"))
            }
            Status::Stopped(n) => format!(
                "{} {n} {}",
                lang.pick("Enregistrement arrêté —", "Recording stopped —"),
                lang.pick("étape(s).", "step(s).")
            ),
            Status::StartFailed(err) => {
                format!(
                    "{} {err}",
                    lang.pick("Échec du démarrage :", "Start failed:")
                )
            }
            Status::Exporting => lang
                .pick("Génération du rapport…", "Generating report…")
                .into(),
            Status::Exported(path) => {
                format!("{} {}", lang.pick("Exporté :", "Exported:"), path.display())
            }
            Status::ExportFailed(err) => {
                format!(
                    "{} {err}",
                    lang.pick("Échec de l'export :", "Export failed:")
                )
            }
            Status::ExportInterrupted => lang
                .pick("Export interrompu.", "Export interrupted.")
                .into(),
            Status::Discarded => lang
                .pick(
                    "Session abandonnée (non enregistrée).",
                    "Session discarded (not saved).",
                )
                .into(),
        }
    }
}

pub struct App {
    base_dir: PathBuf,
    recorder: Option<Recorder>,
    /// Last finished session, kept for review and export.
    last: Option<Arc<Mutex<Session>>>,
    /// Selected step (by step index); `None` follows the latest step.
    pub selected: Option<usize>,
    pub status: Status,
    pub textures: Textures,
    /// Long action in progress (capture, save, compression…), shared with
    /// the worker threads and shown in the status bar.
    pub progress: Progress,
    /// Export running on its own thread: receives the final result.
    export: Option<Receiver<Result<PathBuf, String>>>,
    /// Destination of the running export, shown while it runs.
    pub export_dest: Option<PathBuf>,
    /// Last rendered snapshot, kept while the session is momentarily locked
    /// (for instance during the export) to avoid any flicker.
    pub snapshot: Option<Snapshot>,
    /// Stopped session waiting for a save destination.
    pending_save: Option<Arc<Mutex<Session>>>,
    /// Shows the "session not saved" confirmation (after a cancelled dialog).
    pub confirm_discard: bool,
    /// Shows the "start over and lose the unsaved session?" confirmation.
    pub confirm_new: bool,
    /// Where the reviewed session was saved, once it was.
    pub saved_to: Option<PathBuf>,
    /// Interface language (also used for the generated reports).
    pub lang: Lang,
    pub prefs: Prefs,
    /// Screenshot viewer of the review screen.
    pub viewer: Viewer,
    /// Full-window view of the selected screenshot, when open.
    pub enlarged: Option<Viewer>,
    /// Step count seen last frame, to follow new steps while recording.
    pub seen_rows: usize,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs = Prefs::load(cc.storage);
        theme::setup(&cc.egui_ctx, prefs.theme(), prefs.zoom());
        Self {
            // Screenshots are volatile: they live in a temporary folder until
            // the user picks where to save the report.
            base_dir: std::env::temp_dir().join("support-step-recorder"),
            recorder: None,
            last: None,
            selected: None,
            status: Status::Ready,
            textures: Textures::default(),
            progress: Progress::new(),
            export: None,
            export_dest: None,
            snapshot: None,
            pending_save: None,
            confirm_discard: false,
            confirm_new: false,
            saved_to: None,
            lang: prefs.lang(),
            prefs,
            viewer: Viewer::default(),
            enlarged: None,
            seen_rows: 0,
        }
    }

    /// Translation shortcut: the string in the current language.
    pub fn t(&self, fr: &'static str, en: &'static str) -> &'static str {
        self.lang.pick(fr, en)
    }

    pub fn set_lang(&mut self, lang: Lang) {
        self.lang = lang;
        self.prefs.set_lang(lang);
    }

    pub fn stage(&self) -> Stage {
        if self.recorder.is_some() {
            Stage::Recording
        } else if self.last.is_some() {
            Stage::Review
        } else {
            Stage::Idle
        }
    }

    pub fn is_exporting(&self) -> bool {
        self.export.is_some()
    }

    pub fn backend_name(&self) -> Option<&'static str> {
        self.recorder.as_ref().map(|r| r.backend_name())
    }

    /// Active session: the one being recorded, else the last finished one.
    fn current(&self) -> Option<Arc<Mutex<Session>>> {
        self.recorder
            .as_ref()
            .map(|r| r.session.clone())
            .or_else(|| self.last.clone())
    }

    /// "New recording" from the review: ask first if the session is unsaved.
    pub fn request_start(&mut self) {
        if self.stage() == Stage::Review && self.saved_to.is_none() {
            self.confirm_new = true;
        } else {
            self.start();
        }
    }

    pub fn start(&mut self) {
        self.confirm_new = false;
        // Purge the previous session's temporary folder (screenshots only
        // matter once the user saved the report).
        self.cleanup_previous_temp();

        let name = format!("session-{}", chrono::Local::now().format("%Y%m%d-%H%M%S"));
        let dir = self.base_dir.join(name);
        match Recorder::start(dir, self.progress.clone(), self.lang) {
            Ok(rec) => {
                self.status = Status::Recording(rec.backend_name());
                self.recorder = Some(rec);
                self.reset_review();
            }
            Err(err) => self.status = Status::StartFailed(err.to_string()),
        }
    }

    pub fn stop(&mut self) {
        if let Some(rec) = self.recorder.take() {
            let session = rec.stop();
            let n = session.lock().map(|s| s.steps.len()).unwrap_or(0);
            self.status = Status::Stopped(n);
            self.last = Some(session);
            // Straight on to choosing the destination.
            self.save();
        }
    }

    /// Save the reviewed session (the Save button, or right after Stop).
    pub fn save(&mut self) {
        self.pending_save = self.last.clone();
        self.begin_save();
    }

    /// Open the "Save as" dialog, then export to the chosen place.
    /// Cancelling it brings up the confirmation.
    pub fn begin_save(&mut self) {
        let Some(session) = self.pending_save.clone() else {
            return;
        };
        let default = steps::default_zip_name(&session);
        let dest = rfd::FileDialog::new()
            .set_title(self.t(
                "Enregistrer le rapport (ZIP + HTML)",
                "Save report (ZIP + HTML)",
            ))
            .set_file_name(default)
            .add_filter(self.t("Archive ZIP", "ZIP archive"), &["zip"])
            .save_file();
        match dest {
            Some(path) => {
                self.confirm_discard = false;
                self.pending_save = None;
                self.export_to(session, path);
            }
            None => self.confirm_discard = true, // cancelled → ask to confirm
        }
    }

    /// Keep the unsaved session to look through it first (from the dialog).
    pub fn keep_reviewing(&mut self) {
        self.confirm_discard = false;
        self.pending_save = None;
    }

    /// Drop the session without saving it, back to the start screen.
    pub fn discard(&mut self) {
        self.confirm_discard = false;
        self.pending_save = None;
        self.cleanup_previous_temp();
        self.reset_review();
        self.status = Status::Discarded;
    }

    /// Write the ZIP + HTML to `path` on a dedicated thread.
    fn export_to(&mut self, session: Arc<Mutex<Session>>, mut path: PathBuf) {
        if path.extension().is_none() {
            path.set_extension("zip");
        }
        let progress = self.progress.clone();
        let lang = self.lang;
        let unreachable = self
            .t("Session inaccessible.", "Session unavailable.")
            .to_string();
        self.export_dest = Some(path.clone());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = match session.lock() {
                Ok(guard) => export::export_zip(&guard, &path, &progress, lang)
                    .map(|()| path)
                    .map_err(|err| err.to_string()),
                Err(_) => Err(unreachable),
            };
            progress.clear();
            let _ = tx.send(result);
        });
        self.status = Status::Exporting;
        self.export = Some(rx);
    }

    /// Delete the temporary folder of the last session.
    fn cleanup_previous_temp(&mut self) {
        if let Some(prev) = self.last.take() {
            if let Ok(s) = prev.lock() {
                let _ = std::fs::remove_dir_all(&s.dir);
            }
        }
    }

    /// Forget everything about the reviewed session.
    fn reset_review(&mut self) {
        self.selected = None;
        self.saved_to = None;
        self.textures.clear();
        self.viewer = Viewer::default();
        self.enlarged = None;
        self.seen_rows = 0;
    }

    /// Collect the result of a finished export, if any.
    fn poll_export(&mut self) {
        let Some(rx) = &self.export else { return };
        match rx.try_recv() {
            Ok(Ok(path)) => {
                self.saved_to = Some(path.clone());
                self.status = Status::Exported(path);
                self.export = None;
            }
            Ok(Err(err)) => {
                self.status = Status::ExportFailed(err);
                self.export = None;
            }
            Err(TryRecvError::Empty) => {} // still running
            Err(TryRecvError::Disconnected) => {
                self.status = Status::ExportInterrupted;
                self.export = None;
            }
        }
        if self.export.is_none() {
            self.export_dest = None;
        }
    }

    /// Refresh the snapshot from the session (keeps the old one if locked).
    fn refresh_snapshot(&mut self) {
        match self.current() {
            Some(s) => {
                if let Some(snap) = Snapshot::try_take(&s) {
                    self.snapshot = Some(snap);
                }
            }
            None => self.snapshot = None,
        }
    }

    /// Up/Down (and Home/End) move through the steps.
    fn keyboard_navigation(&mut self, ctx: &egui::Context) -> bool {
        if self.confirm_discard || self.confirm_new || self.is_exporting() {
            return false;
        }
        let Some(snap) = &self.snapshot else {
            return false;
        };
        if snap.rows.is_empty() {
            return false;
        }
        let last = snap.rows.len() - 1;
        let cur = self
            .selected
            .and_then(|sel| snap.position(sel))
            .unwrap_or(last);
        let (up, down, home, end) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Home),
                i.key_pressed(egui::Key::End),
            )
        });
        let target = if down && cur < last {
            Some(cur + 1)
        } else if up && cur > 0 {
            Some(cur - 1)
        } else if home {
            Some(0)
        } else if end {
            Some(last)
        } else {
            None
        };
        match target {
            Some(t) => {
                self.selected = Some(snap.rows[t].index);
                true
            }
            None => false,
        }
    }

    /// Keep the theme and zoom choices (buttons or Ctrl +/-/0) in the prefs.
    fn track_prefs(&mut self, ctx: &egui::Context) {
        let zoom = ctx.zoom_factor();
        let clamped = theme::clamp_zoom(zoom);
        if (zoom - clamped).abs() > 0.001 {
            ctx.set_zoom_factor(clamped);
        }
        self.prefs.zoom = clamped;
        self.prefs.set_theme(ctx.options(|o| o.theme_preference));
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll_export();
        self.track_prefs(&ctx);
        self.textures.begin_frame();

        // While work is going on (recording, an action, an export), data
        // changes from other threads: refresh regularly.
        let activity = self.progress.get();
        if self.recorder.is_some() || activity.is_some() || self.export.is_some() {
            ctx.request_repaint_after(Duration::from_millis(250));
        }

        self.refresh_snapshot();
        let nav = self.keyboard_navigation(&ctx);

        chrome::top_bar(self, root);
        chrome::status_bar(self, root, activity.as_deref());

        match self.stage() {
            Stage::Idle => screens::idle(self, root),
            Stage::Recording => {
                screens::recording_banner(self, root);
                review::body(self, root, nav);
            }
            Stage::Review => {
                screens::review_header(self, root);
                review::body(self, root, nav);
            }
        }

        dialogs::enlarged(self, &ctx);
        dialogs::confirm_discard(self, &ctx);
        dialogs::confirm_new(self, &ctx);
        dialogs::export_progress(self, &ctx, activity.as_deref());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.prefs.save(storage);
    }
}
