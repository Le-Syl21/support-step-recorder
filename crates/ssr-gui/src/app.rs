//! Application state and the workflow: idle → recording → editor. Stopping
//! a recording lands in the editor (no "Save as" dialog); from there the
//! guide is saved as an editable `.ssrproj` and exported as a shareable ZIP,
//! two separate actions. The screens themselves live in [`crate::screens`],
//! [`crate::review`], [`crate::chrome`], [`crate::dialogs`] and
//! [`crate::editor_sources`].

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use eframe::egui;
use ssr_core::editor::{EditError, EditKind};
use ssr_core::import::{self, ImportLimits, ImportedRecording};
use ssr_core::project::PROJECT_EXT;
use ssr_core::workspace::{self, Recoverable, Workspace};
use ssr_core::{export, EditorDocument, Lang, Progress, Recorder};
use uuid::Uuid;

use crate::editor_sources::ImportDialog;
use crate::prefs::Prefs;
use crate::steps::{self, Snapshot};
use crate::textures::Textures;
use crate::viewer::Viewer;
use crate::{chrome, dialogs, editor_sources, review, screens, theme};

/// How often the crash-recovery copy may be rewritten.
const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(3);

/// Where the user is in the workflow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stage {
    /// No guide open.
    Idle,
    Recording,
    /// A guide to edit, save and export.
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
    Working,
    Exported(PathBuf),
    ExportFailed(String),
    Saved(PathBuf),
    SaveFailed(String),
    Opened(String),
    OpenFailed(String),
    Imported(usize),
    Undone(EditKind),
    Redone(EditKind),
    Interrupted,
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
                lang.pick(
                    "étape(s). Relisez et modifiez le guide.",
                    "step(s). Review and edit the guide."
                )
            ),
            Status::StartFailed(err) => {
                format!(
                    "{} {err}",
                    lang.pick("Échec du démarrage :", "Start failed:")
                )
            }
            Status::Working => lang.pick("Traitement…", "Working…").into(),
            Status::Exported(path) => {
                format!(
                    "{} {}",
                    lang.pick("Guide exporté :", "Guide exported:"),
                    path.display()
                )
            }
            Status::ExportFailed(err) => {
                format!(
                    "{} {err}",
                    lang.pick("Échec de l'export :", "Export failed:")
                )
            }
            Status::Saved(path) => {
                format!(
                    "{} {}",
                    lang.pick("Projet enregistré :", "Project saved:"),
                    path.display()
                )
            }
            Status::SaveFailed(err) => format!(
                "{} {err}",
                lang.pick("Échec de l'enregistrement :", "Save failed:")
            ),
            Status::Opened(title) => format!("{} {title}", lang.pick("Ouvert :", "Opened:")),
            Status::OpenFailed(err) => {
                format!(
                    "{} {err}",
                    lang.pick("Ouverture impossible :", "Could not open:")
                )
            }
            Status::Imported(n) => format!(
                "{n} {}",
                lang.pick("étape(s) importée(s).", "step(s) imported.")
            ),
            Status::Undone(k) => format!("{} {}", lang.pick("Annulé :", "Undone:"), k.label(lang)),
            Status::Redone(k) => format!("{} {}", lang.pick("Rétabli :", "Redone:"), k.label(lang)),
            Status::Interrupted => lang
                .pick("Opération interrompue.", "Operation interrupted.")
                .into(),
            Status::Discarded => lang
                .pick(
                    "Guide fermé sans enregistrer.",
                    "Guide closed without saving.",
                )
                .into(),
        }
    }

    pub fn is_error(&self) -> bool {
        matches!(
            self,
            Status::StartFailed(_)
                | Status::ExportFailed(_)
                | Status::SaveFailed(_)
                | Status::OpenFailed(_)
                | Status::Interrupted
        )
    }
}

/// Something that would replace or close the open guide: asked first when
/// the guide has unsaved changes.
#[derive(Clone, Debug, PartialEq)]
pub enum Intent {
    NewRecording,
    OpenProject,
    OpenRecording,
    CloseGuide,
    Recover(PathBuf),
    Quit,
}

/// Work running on its own thread; the editor is read-only meanwhile.
enum JobResult {
    Saved {
        path: PathBuf,
        revision: u64,
    },
    Exported {
        path: PathBuf,
        revision: u64,
    },
    /// A new guide to replace the current one (opened project or recording).
    Opened {
        doc: Box<EditorDocument>,
        warnings: Vec<String>,
    },
    /// Recordings read for the import dialog.
    ForImport {
        recordings: Vec<ImportedRecording>,
        errors: Vec<String>,
    },
    Failed(Status),
}

struct Job {
    rx: Receiver<JobResult>,
    /// Shown in the progress dialog.
    pub title: (&'static str, &'static str),
    pub dest: Option<PathBuf>,
}

/// The title and instructions being typed for the selected step. Kept apart
/// from the guide so a redraw never overwrites what is being typed; written
/// back on Enter / focus loss, dropped on Escape.
#[derive(Clone, Debug)]
pub struct Draft {
    pub id: Uuid,
    pub title: String,
    pub notes: String,
}

/// A screenshot added from a file, waiting for "replace or insert?".
#[derive(Clone, Debug)]
pub struct PendingImage {
    pub asset: String,
    pub target: Uuid,
}

pub struct App {
    recorder: Option<Recorder>,
    /// Workspace of the recording in progress (its capture folder).
    rec_workspace: Option<Workspace>,
    /// The guide being edited.
    pub doc: Option<EditorDocument>,
    /// Bumped whenever `doc` is replaced, to rebuild the snapshot.
    doc_generation: u64,
    snapshot_key: Option<(u64, u64)>,
    /// Selected step (by id); `None` follows the latest step.
    pub selected: Option<Uuid>,
    pub status: Status,
    pub textures: Textures,
    /// Long action in progress, shared with the worker threads.
    pub progress: Progress,
    job: Option<Job>,
    /// Last rendered snapshot.
    pub snapshot: Option<Snapshot>,
    /// The unsaved-changes question, with what to do afterwards.
    pub pending: Option<Intent>,
    /// What to do once the save under way succeeds.
    after_save: Option<Intent>,
    /// The import dialog, once its recordings are read.
    pub import: Option<ImportDialog>,
    pub pending_image: Option<PendingImage>,
    /// Guide title / introduction being edited.
    pub guide_info: Option<(String, String)>,
    pub draft: Option<Draft>,
    /// Step being dragged in the list.
    pub dragging: Option<Uuid>,
    /// Abandoned guides found at startup.
    pub recoverable: Vec<Recoverable>,
    /// Shows the keyboard-capture notice before the first recording.
    pub privacy_prompt: bool,
    /// Warnings to show (images that could not be imported…).
    pub notice: Option<(String, Vec<String>)>,
    /// The window may close (unsaved work was dealt with).
    allow_close: bool,
    /// Interface language (also used for the generated reports).
    pub lang: Lang,
    pub prefs: Prefs,
    pub viewer: Viewer,
    pub enlarged: Option<Viewer>,
    /// Step count seen last frame, to follow new steps while recording.
    pub seen_rows: usize,
    limits: ImportLimits,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, open: Option<PathBuf>) -> Self {
        let prefs = Prefs::load(cc.storage);
        theme::setup(&cc.egui_ctx, prefs.theme(), prefs.zoom());
        let mut app = Self {
            recorder: None,
            rec_workspace: None,
            doc: None,
            doc_generation: 0,
            snapshot_key: None,
            selected: None,
            status: Status::Ready,
            textures: Textures::default(),
            progress: Progress::new(),
            job: None,
            snapshot: None,
            pending: None,
            after_save: None,
            import: None,
            pending_image: None,
            guide_info: None,
            draft: None,
            dragging: None,
            recoverable: workspace::scan_abandoned(&workspace::workspaces_root()),
            privacy_prompt: false,
            notice: None,
            allow_close: false,
            lang: prefs.lang(),
            prefs,
            viewer: Viewer::default(),
            enlarged: None,
            seen_rows: 0,
            limits: ImportLimits::default(),
        };
        if let Some(path) = open {
            app.open_path(path);
        }
        app
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
        } else if self.doc.is_some() {
            Stage::Review
        } else {
            Stage::Idle
        }
    }

    /// A save, export, open or import is running.
    pub fn is_busy(&self) -> bool {
        self.job.is_some()
    }

    pub fn job_title(&self) -> Option<(&'static str, &'static str)> {
        self.job.as_ref().map(|j| j.title)
    }

    pub fn job_dest(&self) -> Option<&Path> {
        self.job.as_ref().and_then(|j| j.dest.as_deref())
    }

    /// The guide can be changed right now.
    pub fn can_edit(&self) -> bool {
        self.doc.is_some() && !self.is_busy() && self.recorder.is_none()
    }

    pub fn is_dirty(&self) -> bool {
        self.doc.as_ref().is_some_and(|d| d.is_dirty())
    }

    pub fn backend_name(&self) -> Option<&'static str> {
        self.recorder.as_ref().map(|r| r.backend_name())
    }

    // ── Guarded transitions ─────────────────────────────────────────

    /// Do `intent`, asking first if the open guide has unsaved changes.
    pub fn request(&mut self, intent: Intent) {
        if self.is_busy() {
            return;
        }
        if self.is_dirty() {
            self.pending = Some(intent);
        } else {
            self.perform(intent);
        }
    }

    /// Do `intent` now (unsaved changes were saved or discarded).
    pub fn perform(&mut self, intent: Intent) {
        self.pending = None;
        match intent {
            Intent::NewRecording => self.start(),
            Intent::OpenProject => self.open_project(),
            Intent::OpenRecording => self.open_recording(),
            Intent::CloseGuide => {
                self.close_doc();
                self.status = Status::Discarded;
            }
            Intent::Recover(dir) => self.recover(&dir),
            Intent::Quit => {
                self.close_doc();
                self.allow_close = true;
            }
        }
    }

    /// Save first, then carry on with `intent` (from the unsaved dialog).
    pub fn save_then(&mut self, intent: Intent) {
        self.pending = None;
        self.after_save = Some(intent);
        self.save_project(false);
        if !self.is_busy() {
            // Dialog cancelled: nothing saved, nothing done (not a discard).
            self.after_save = None;
        }
    }

    /// Close the guide and delete its workspace.
    fn close_doc(&mut self) {
        if let Some(doc) = self.doc.take() {
            doc.close();
        }
        self.reset_review();
    }

    fn set_doc(&mut self, doc: EditorDocument) {
        self.close_doc();
        self.selected = doc.steps().first().and_then(|s| s.id);
        self.doc = Some(doc);
        self.doc_generation += 1;
    }

    // ── Recording ───────────────────────────────────────────────────

    pub fn start(&mut self) {
        if !self.prefs.privacy_ack {
            self.privacy_prompt = true;
            return;
        }
        self.close_doc();
        let ws = match Workspace::create() {
            Ok(ws) => ws,
            Err(e) => {
                self.status = Status::StartFailed(e.to_string());
                return;
            }
        };
        let dir = match ws.subdir("capture") {
            Ok(d) => d,
            Err(e) => {
                self.status = Status::StartFailed(e.to_string());
                ws.discard();
                return;
            }
        };
        match Recorder::start(dir, self.progress.clone(), self.lang) {
            Ok(rec) => {
                self.status = Status::Recording(rec.backend_name());
                self.recorder = Some(rec);
                self.rec_workspace = Some(ws);
                self.reset_review();
            }
            Err(err) => {
                ws.discard();
                self.status = Status::StartFailed(err.to_string());
            }
        }
    }

    /// The privacy notice was accepted: remember it and record.
    pub fn accept_privacy(&mut self) {
        self.privacy_prompt = false;
        self.prefs.privacy_ack = true;
        self.start();
    }

    /// Stop: straight into the editor, nothing written outside the workspace.
    pub fn stop(&mut self) {
        let Some(rec) = self.recorder.take() else {
            return;
        };
        let session = rec.stop();
        let Some(ws) = self.rec_workspace.take() else {
            return;
        };
        let title = format!(
            "{} {}",
            self.t("Enregistrement du", "Recording of"),
            chrono::Local::now().format(match self.lang {
                Lang::Fr => "%d/%m/%Y %H:%M",
                Lang::En => "%Y-%m-%d %H:%M",
            })
        );
        let result = match session.lock() {
            Ok(s) => EditorDocument::from_session(&s, ws, title, self.lang),
            Err(_) => Err(std::io::Error::other("session unavailable")),
        };
        match result {
            Ok(doc) => {
                self.status = Status::Stopped(doc.steps().len());
                self.set_doc(doc);
            }
            Err(e) => self.status = Status::StartFailed(e.to_string()),
        }
    }

    // ── Background work ─────────────────────────────────────────────

    fn spawn(
        &mut self,
        title: (&'static str, &'static str),
        dest: Option<PathBuf>,
        work: impl FnOnce(&Progress) -> JobResult + Send + 'static,
    ) {
        let progress = self.progress.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = work(&progress);
            progress.clear();
            let _ = tx.send(r);
        });
        self.status = Status::Working;
        self.job = Some(Job { rx, title, dest });
    }

    fn poll_job(&mut self) {
        let Some(job) = &self.job else { return };
        let result = match job.rx.try_recv() {
            Ok(r) => r,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => JobResult::Failed(Status::Interrupted),
        };
        self.job = None;
        match result {
            JobResult::Saved { path, revision } => {
                if let Some(doc) = &mut self.doc {
                    doc.mark_saved(&path, revision);
                    let _ = doc.write_recovery();
                }
                self.prefs.remember_dir(&path);
                self.status = Status::Saved(path);
                if let Some(next) = self.after_save.take() {
                    self.perform(next);
                }
            }
            JobResult::Exported { path, revision } => {
                if let Some(doc) = &mut self.doc {
                    doc.mark_exported(&path, revision);
                }
                self.prefs.remember_dir(&path);
                self.status = Status::Exported(path);
            }
            JobResult::Opened { doc, warnings } => {
                let title = doc.metadata().title.clone();
                self.set_doc(*doc);
                self.status = Status::Opened(title);
                if !warnings.is_empty() {
                    self.notice = Some((
                        self.t(
                            "Certaines images n'ont pas pu être reprises",
                            "Some images could not be brought in",
                        )
                        .into(),
                        warnings,
                    ));
                }
            }
            JobResult::ForImport { recordings, errors } => {
                if !errors.is_empty() {
                    self.notice =
                        Some((self.t("Import impossible", "Import failed").into(), errors));
                }
                if !recordings.is_empty() {
                    self.import = Some(ImportDialog::new(recordings));
                }
                self.status = Status::Ready;
            }
            JobResult::Failed(status) => {
                self.after_save = None;
                self.status = status;
            }
        }
    }

    // ── File actions ────────────────────────────────────────────────

    fn dialog(&self) -> rfd::FileDialog {
        let d = rfd::FileDialog::new();
        match self.prefs.last_dir() {
            Some(dir) => d.set_directory(dir),
            None => d,
        }
    }

    /// Save the guide as an editable project (`as_new`: always ask where).
    pub fn save_project(&mut self, as_new: bool) {
        if self.is_busy() {
            return;
        }
        let Some(doc) = &self.doc else { return };
        let target = match doc.project_path().filter(|_| !as_new) {
            Some(p) => p.to_path_buf(),
            None => {
                let name = format!("{}.{PROJECT_EXT}", steps::file_stem(&doc.metadata().title));
                let mut d = self
                    .dialog()
                    .set_title(self.t("Enregistrer le projet", "Save project"))
                    .set_file_name(name)
                    .add_filter(self.t("Projet SSR", "SSR project"), &[PROJECT_EXT]);
                if let Some(dir) = doc.project_path().and_then(|p| p.parent()) {
                    d = d.set_directory(dir);
                }
                let Some(mut path) = d.save_file() else {
                    return; // cancelled: nothing happens
                };
                if path.extension().and_then(|e| e.to_str()) != Some(PROJECT_EXT) {
                    // Never turn e.g. a recording's `.zip` into a project:
                    // append the extension and refuse to clobber silently.
                    let mut s = path.into_os_string();
                    s.push(format!(".{PROJECT_EXT}"));
                    path = PathBuf::from(s);
                    if path.exists() {
                        self.status = Status::SaveFailed(format!(
                            "{} {}",
                            self.t("existe déjà :", "already exists:"),
                            path.display()
                        ));
                        return;
                    }
                }
                path
            }
        };
        let snap = doc.snapshot();
        let lang = self.lang;
        self.spawn(
            ("Enregistrement du projet…", "Saving project…"),
            Some(target.clone()),
            move |_| match ssr_core::project::save_project(&snap, &target) {
                Ok(()) => JobResult::Saved {
                    path: target,
                    revision: snap.revision,
                },
                Err(e) => JobResult::Failed(Status::SaveFailed(e.message(lang))),
            },
        );
    }

    /// Export the shareable ZIP (HTML report + JSON + images).
    pub fn export_guide(&mut self) {
        if self.is_busy() {
            return;
        }
        let Some(doc) = &self.doc else { return };
        let name = format!("{}.zip", steps::file_stem(&doc.metadata().title));
        let mut d = self
            .dialog()
            .set_title(self.t(
                "Exporter le guide (ZIP + HTML)",
                "Export guide (ZIP + HTML)",
            ))
            .set_file_name(name)
            .add_filter(self.t("Archive ZIP", "ZIP archive"), &["zip"]);
        if let Some(dir) = doc.export_path().and_then(|p| p.parent()) {
            d = d.set_directory(dir);
        }
        let Some(mut path) = d.save_file() else {
            return;
        };
        if path.extension().is_none() {
            path.set_extension("zip");
        }
        if doc.project_path() == Some(path.as_path()) {
            self.status = Status::ExportFailed(
                self.t(
                    "c'est le fichier du projet ; choisissez un autre nom.",
                    "that is the project file; choose another name.",
                )
                .into(),
            );
            return;
        }
        let snap = doc.snapshot();
        let lang = self.lang;
        self.spawn(
            ("Export du guide…", "Exporting guide…"),
            Some(path.clone()),
            move |progress| match export::export_guide(&snap, &path, progress, lang) {
                Ok(()) => JobResult::Exported {
                    path,
                    revision: snap.revision,
                },
                Err(e) => JobResult::Failed(Status::ExportFailed(e.message(lang))),
            },
        );
    }

    fn open_project(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title(self.t("Ouvrir un projet", "Open project"))
            .add_filter(self.t("Projet SSR", "SSR project"), &[PROJECT_EXT])
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    /// Open an exported recording (v0.2.0 ZIP) as a new, untitled guide.
    fn open_recording(&mut self) {
        let Some(path) = self
            .dialog()
            .set_title(self.t("Ouvrir un enregistrement", "Open recording"))
            .add_filter(
                self.t("Enregistrement ou projet", "Recording or project"),
                &["zip", PROJECT_EXT],
            )
            .pick_file()
        else {
            return;
        };
        self.open_path(path);
    }

    /// Open a `.ssrproj` as a project, anything else as a recording to edit.
    /// The current guide is only replaced once the file is read.
    pub fn open_path(&mut self, path: PathBuf) {
        if self.is_busy() {
            return;
        }
        self.prefs.remember_dir(&path);
        let limits = self.limits.clone();
        let lang = self.lang;
        if path.extension().and_then(|e| e.to_str()) == Some(PROJECT_EXT) {
            self.spawn(
                ("Ouverture du projet…", "Opening project…"),
                Some(path.clone()),
                move |_| {
                    let ws = match Workspace::create() {
                        Ok(ws) => ws,
                        Err(e) => return JobResult::Failed(Status::OpenFailed(e.to_string())),
                    };
                    match EditorDocument::open_project(&path, ws, &limits) {
                        Ok((doc, warnings)) => JobResult::Opened {
                            doc: Box::new(doc),
                            warnings,
                        },
                        Err((e, ws)) => {
                            ws.discard();
                            JobResult::Failed(Status::OpenFailed(e.message(lang)))
                        }
                    }
                },
            );
            return;
        }
        self.spawn(
            ("Ouverture de l'enregistrement…", "Opening recording…"),
            Some(path.clone()),
            move |_| {
                let fail = |e: String| JobResult::Failed(Status::OpenFailed(e));
                let ws = match Workspace::create() {
                    Ok(ws) => ws,
                    Err(e) => return fail(e.to_string()),
                };
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "guide".into());
                let mut doc = match EditorDocument::new(ws, title, lang) {
                    Ok(d) => d,
                    Err(e) => return fail(e.to_string()),
                };
                let rec = match import::import_recording(&path, doc.assets(), &limits) {
                    Ok(r) => r,
                    Err(e) => {
                        doc.close();
                        return fail(e.message(lang));
                    }
                };
                if let Some(t) = &rec.title {
                    let _ = doc.set_guide_info(t.clone(), String::new());
                }
                let warnings: Vec<String> = rec
                    .steps
                    .iter()
                    .filter_map(|s| s.image_warning.clone())
                    .collect();
                let all: Vec<usize> = (0..rec.steps.len()).collect();
                let _ = doc.import_steps(0, &rec, &all);
                doc.clear_history();
                JobResult::Opened {
                    doc: Box::new(doc),
                    warnings,
                }
            },
        );
    }

    /// Pick recordings to take steps from (the guide stays as it is).
    pub fn import_recordings(&mut self) {
        if !self.can_edit() {
            return;
        }
        let Some(paths) = self
            .dialog()
            .set_title(self.t(
                "Importer des étapes d'enregistrements",
                "Import steps from recordings",
            ))
            .add_filter(
                self.t("Enregistrement ou projet", "Recording or project"),
                &["zip", PROJECT_EXT],
            )
            .pick_files()
        else {
            return;
        };
        if let Some(p) = paths.first() {
            self.prefs.remember_dir(p);
        }
        let Some(doc) = &self.doc else { return };
        let store = doc.assets().clone();
        let limits = self.limits.clone();
        let lang = self.lang;
        self.spawn(
            ("Lecture des enregistrements…", "Reading recordings…"),
            None,
            move |_| {
                let mut recordings = Vec::new();
                let mut errors = Vec::new();
                for p in paths {
                    match import::import_recording(&p, &store, &limits) {
                        Ok(r) => recordings.push(r),
                        Err(e) => errors.push(format!(
                            "{}: {}",
                            p.file_name()
                                .map(|n| n.to_string_lossy())
                                .unwrap_or_default(),
                            e.message(lang)
                        )),
                    }
                }
                JobResult::ForImport { recordings, errors }
            },
        );
    }

    /// Add a PNG/JPEG/WebP: attach to the selected step or make a new step.
    pub fn add_image(&mut self) {
        if !self.can_edit() {
            return;
        }
        let Some(path) = self
            .dialog()
            .set_title(self.t("Ajouter une image", "Add image"))
            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        self.prefs.remember_dir(&path);
        let Some(doc) = &mut self.doc else { return };
        let asset = match import::import_image_file(&path, doc.assets(), &self.limits) {
            Ok((a, _)) => a,
            Err(e) => {
                self.status = Status::OpenFailed(e.message(self.lang));
                return;
            }
        };
        let selected = self.selected.filter(|id| doc.position(*id).is_some());
        match selected {
            // Ambiguous: let the user say which.
            Some(target) => self.pending_image = Some(PendingImage { asset, target }),
            None => {
                let pos = doc.steps().len();
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if let Ok(id) = doc.add_manual_step(pos, title, Some(asset)) {
                    self.select(id);
                }
            }
        }
    }

    fn recover(&mut self, dir: &Path) {
        self.recoverable.retain(|r| r.dir != dir);
        let result = Workspace::adopt(dir).and_then(EditorDocument::recover);
        match result {
            Ok(doc) => {
                let title = doc.metadata().title.clone();
                self.set_doc(doc);
                self.status = Status::Opened(title);
            }
            Err(e) => self.status = Status::OpenFailed(e.to_string()),
        }
    }

    /// Delete an abandoned guide without opening it.
    pub fn discard_recoverable(&mut self, dir: &Path) {
        self.recoverable.retain(|r| r.dir != dir);
        if let Ok(ws) = Workspace::adopt(dir) {
            ws.discard();
        }
    }

    // ── Editing ─────────────────────────────────────────────────────

    pub fn select(&mut self, id: Uuid) {
        self.commit_draft();
        self.selected = Some(id);
    }

    /// Run an edit command on the guide; the typing buffer is re-read after.
    pub fn edit<T>(
        &mut self,
        f: impl FnOnce(&mut EditorDocument) -> Result<T, EditError>,
    ) -> Option<T> {
        if !self.can_edit() {
            return None;
        }
        let doc = self.doc.as_mut()?;
        let r = f(doc).ok();
        if r.is_some() {
            self.draft = None;
        }
        r
    }

    /// Write the typed title / instructions back to the guide.
    pub fn commit_draft(&mut self) {
        let Some(d) = self.draft.take() else { return };
        if !self.can_edit() {
            return;
        }
        let Some(doc) = self.doc.as_mut() else { return };
        let Some(step) = doc.step(d.id) else { return };
        let title_changed = step.title_override.as_deref().unwrap_or("") != d.title.trim();
        let notes_changed = step.notes != d.notes.trim_end();
        if title_changed {
            let _ = doc.set_title(d.id, Some(d.title.clone()));
        }
        if notes_changed {
            let _ = doc.set_notes(d.id, d.notes.clone());
        }
    }

    pub fn undo(&mut self) {
        self.commit_draft();
        if let Some(k) = self.edit(|d| d.undo()) {
            self.status = Status::Undone(k);
        }
    }

    pub fn redo(&mut self) {
        self.commit_draft();
        if let Some(k) = self.edit(|d| d.redo()) {
            self.status = Status::Redone(k);
        }
    }

    pub fn move_selected(&mut self, delta: isize) {
        let Some(id) = self.current_id() else { return };
        let Some(doc) = &self.doc else { return };
        let Some(pos) = doc.position(id) else { return };
        let target = pos as isize + delta;
        if target < 0 || target as usize >= doc.steps().len() {
            return;
        }
        self.edit(|d| d.move_step(id, target as usize));
    }

    /// Delete a step; the nearest remaining one gets the selection.
    pub fn delete_step(&mut self, id: Uuid) {
        let Some(doc) = &self.doc else { return };
        let Some(pos) = doc.position(id) else { return };
        if self.edit(|d| d.delete_step(id)).is_some() {
            let steps = self.doc.as_ref().map(|d| d.steps()).unwrap_or(&[]);
            self.selected = steps
                .get(pos)
                .or_else(|| pos.checked_sub(1).and_then(|p| steps.get(p)))
                .and_then(|s| s.id);
        }
    }

    /// Insert a new written step after the selected one.
    pub fn add_written_step(&mut self) {
        let Some(doc) = &self.doc else { return };
        let pos = self
            .current_id()
            .and_then(|id| doc.position(id))
            .map(|p| p + 1)
            .unwrap_or(doc.steps().len());
        let title = self.t("Nouvelle étape", "New step");
        if let Some(id) = self.edit(|d| d.add_manual_step(pos, title, None)) {
            self.select(id);
        }
    }

    /// Insert the chosen steps of the import dialog.
    pub fn confirm_import(&mut self, dialog: ImportDialog) {
        let Some(doc) = &self.doc else { return };
        let position = match dialog.position {
            editor_sources::Position::Append => doc.steps().len(),
            editor_sources::Position::AfterSelected => self
                .current_id()
                .and_then(|id| doc.position(id))
                .map(|p| p + 1)
                .unwrap_or(doc.steps().len()),
            editor_sources::Position::Start => 0,
        };
        let picks = dialog.picks();
        let selections: Vec<(&ImportedRecording, &[usize])> = dialog
            .recordings
            .iter()
            .zip(&picks)
            .map(|(r, p)| (r, p.as_slice()))
            .collect();
        let count: usize = picks.iter().map(Vec::len).sum();
        if let Some(ids) = self.edit(|d| d.import_many(position, &selections)) {
            self.status = Status::Imported(count);
            if let Some(first) = ids.first() {
                self.selected = Some(*first);
            }
        }
        // Images of steps left out were stored for the preview only.
        if let Some(doc) = &self.doc {
            let used: std::collections::HashSet<&str> = doc
                .steps()
                .iter()
                .filter_map(|s| s.screenshot.as_deref())
                .collect();
            for r in &dialog.recordings {
                for a in &r.created_assets {
                    if !used.contains(a.as_str()) {
                        doc.assets().remove(a);
                    }
                }
            }
        }
    }

    /// The import dialog was cancelled: forget its images.
    pub fn cancel_import(&mut self, dialog: ImportDialog) {
        if let Some(doc) = &self.doc {
            for r in &dialog.recordings {
                r.rollback(doc.assets());
            }
        }
    }

    /// The step shown on the right: the selected one, else the last.
    pub fn current_id(&self) -> Option<Uuid> {
        let snap = self.snapshot.as_ref()?;
        snap.current(self.selected).map(|i| snap.rows[i].id)
    }

    /// Forget everything about the reviewed guide's display.
    fn reset_review(&mut self) {
        self.selected = None;
        self.textures.clear();
        self.viewer = Viewer::default();
        self.enlarged = None;
        self.seen_rows = 0;
        self.draft = None;
        self.dragging = None;
        self.pending_image = None;
        self.guide_info = None;
        self.snapshot_key = None;
    }

    /// Refresh the snapshot (only when the guide changed).
    fn refresh_snapshot(&mut self) {
        if let Some(rec) = &self.recorder {
            if let Some(snap) = Snapshot::try_take(&rec.session) {
                self.snapshot = Some(snap);
            }
            self.snapshot_key = None;
            return;
        }
        match &self.doc {
            Some(doc) => {
                let key = (self.doc_generation, doc.revision());
                if self.snapshot_key != Some(key) {
                    self.snapshot = Some(Snapshot::of_document(doc));
                    self.snapshot_key = Some(key);
                }
            }
            None => {
                self.snapshot = None;
                self.snapshot_key = None;
            }
        }
    }

    fn modal_open(&self) -> bool {
        self.pending.is_some()
            || self.import.is_some()
            || self.pending_image.is_some()
            || self.guide_info.is_some()
            || self.privacy_prompt
            || self.notice.is_some()
            || self.is_busy()
            || self.enlarged.is_some()
    }

    /// Up/Down (and Home/End) move through the steps; the editing shortcuts.
    fn keyboard(&mut self, ctx: &egui::Context) -> bool {
        use egui::{Key, KeyboardShortcut, Modifiers};
        if self.modal_open() || ctx.egui_wants_keyboard_input() {
            return false;
        }
        if self.stage() == Stage::Review {
            let shortcut = |m, k| KeyboardShortcut::new(m, k);
            let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
            let (redo, undo, save_as, save, export, up, down, del) = ctx.input_mut(|i| {
                (
                    i.consume_shortcut(&shortcut(cmd_shift, Key::Z))
                        || i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Y)),
                    i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::Z)),
                    i.consume_shortcut(&shortcut(cmd_shift, Key::S)),
                    i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::S)),
                    i.consume_shortcut(&shortcut(Modifiers::COMMAND, Key::E)),
                    i.consume_shortcut(&shortcut(Modifiers::ALT, Key::ArrowUp)),
                    i.consume_shortcut(&shortcut(Modifiers::ALT, Key::ArrowDown)),
                    i.consume_key(Modifiers::NONE, Key::Delete),
                )
            });
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            }
            if save_as {
                self.save_project(true);
            } else if save {
                self.save_project(false);
            }
            if export {
                self.export_guide();
            }
            if up {
                self.move_selected(-1);
                return true;
            }
            if down {
                self.move_selected(1);
                return true;
            }
            if del {
                if let Some(id) = self.current_id() {
                    self.delete_step(id);
                }
            }
        }

        let Some(snap) = &self.snapshot else {
            return false;
        };
        if snap.rows.is_empty() {
            return false;
        }
        let last = snap.rows.len() - 1;
        let cur = snap.current(self.selected).unwrap_or(last);
        let (up, down, home, end) = ctx.input(|i| {
            (
                i.key_pressed(Key::ArrowUp) && i.modifiers.is_none(),
                i.key_pressed(Key::ArrowDown) && i.modifiers.is_none(),
                i.key_pressed(Key::Home),
                i.key_pressed(Key::End),
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
                let id = snap.rows[t].id;
                self.select(id);
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

    /// Closing the window with unsaved changes asks first.
    fn guard_close(&mut self, ctx: &egui::Context) {
        if self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }
        if !ctx.input(|i| i.viewport().close_requested()) {
            return;
        }
        if self.is_dirty() || self.is_busy() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if !self.is_busy() {
                self.commit_draft();
                self.pending = Some(Intent::Quit);
            }
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root.ctx().clone();
        self.poll_job();
        self.guard_close(&ctx);
        self.track_prefs(&ctx);
        self.textures.begin_frame();

        let activity = self.progress.get();
        if activity.is_some() || self.is_busy() {
            ctx.request_repaint_after(Duration::from_millis(250));
        } else if self.recorder.is_some() {
            ctx.request_repaint_after(Duration::from_secs(1));
        }

        // Crash-recovery copy of the guide, throttled.
        if !self.is_busy() {
            if let Some(doc) = &mut self.doc {
                doc.autosave(AUTOSAVE_INTERVAL);
                if doc.needs_autosave() {
                    ctx.request_repaint_after(AUTOSAVE_INTERVAL);
                }
            }
        }

        self.refresh_snapshot();
        let nav = self.keyboard(&ctx);
        // An edit by shortcut changes the guide: show it this frame.
        self.refresh_snapshot();

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
        dialogs::unsaved_changes(self, &ctx);
        dialogs::privacy(self, &ctx);
        dialogs::guide_info(self, &ctx);
        dialogs::image_choice(self, &ctx);
        editor_sources::import_dialog(self, &ctx);
        dialogs::notice(self, &ctx);
        dialogs::job_progress(self, &ctx, activity.as_deref());
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.prefs.save(storage);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // A clean guide has nothing to recover: remove its workspace. A dirty
        // one (the window was closed some other way) keeps its recovery copy.
        if let Some(mut doc) = self.doc.take() {
            if doc.is_dirty() {
                let _ = doc.write_recovery();
            } else {
                doc.close();
            }
        }
        if let Some(rec) = self.recorder.take() {
            drop(rec.stop());
        }
        if let Some(ws) = self.rec_workspace.take() {
            ws.discard();
        }
    }
}
