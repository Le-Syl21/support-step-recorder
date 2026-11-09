//! The guide being edited once a recording has stopped (or a project or an
//! old recording has been opened).
//!
//! [`EditorDocument`] owns the steps in guide order, the project metadata,
//! the private workspace holding the images, the undo history and the
//! "saved / exported" bookkeeping. Every change goes through a command method
//! that keeps the invariants:
//!
//! - `steps[i].index == i + 1` after every change (the number is editorial);
//! - step ids are stable through reorders and unique within the guide;
//! - timestamps and provenance are never rewritten;
//! - every change bumps the revision and can be undone.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use uuid::Uuid;

use crate::assets::{self, AssetStore};
use crate::i18n::Lang;
use crate::import::{ImportError, ImportLimits, ImportedRecording};
use crate::model::{Action, Step};
use crate::project::{self, now_rfc3339, OpenedProject, ProjectMetadata, SourceArchive};
use crate::session::Session;
use crate::workspace::{RecoveryData, Workspace, RECOVERY_FILE};

/// How many edits can be undone.
pub const HISTORY_LIMIT: usize = 50;
/// What replaces typed text that was redacted.
pub const REDACTED: &str = "•••";

/// A rejected edit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// No step with this id (it was deleted meanwhile).
    UnknownStep(Uuid),
    /// A position past the end of the guide.
    OutOfRange { index: usize, len: usize },
    /// Not an image of this guide's workspace.
    InvalidAsset(String),
    /// The edit would change nothing.
    NoChange,
    /// Nothing to undo or redo.
    EmptyHistory,
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EditError::UnknownStep(id) => write!(f, "unknown step {id}"),
            EditError::OutOfRange { index, len } => {
                write!(f, "position {index} out of range (guide has {len} steps)")
            }
            EditError::InvalidAsset(a) => write!(f, "invalid image reference {a}"),
            EditError::NoChange => write!(f, "nothing changed"),
            EditError::EmptyHistory => write!(f, "nothing to undo or redo"),
        }
    }
}

impl std::error::Error for EditError {}

/// The kinds of edit, as named in the Undo/Redo tooltips.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditKind {
    Rename,
    Notes,
    AddStep,
    AttachImage,
    DetachImage,
    Delete,
    Move,
    Import,
    Redact,
    GuideInfo,
}

impl EditKind {
    pub fn label(self, lang: Lang) -> &'static str {
        match self {
            EditKind::Rename => lang.pick("renommer l'étape", "rename step"),
            EditKind::Notes => lang.pick("modifier la consigne", "edit instructions"),
            EditKind::AddStep => lang.pick("ajouter une étape", "add step"),
            EditKind::AttachImage => lang.pick("ajouter la capture", "attach screenshot"),
            EditKind::DetachImage => lang.pick("retirer la capture", "remove screenshot"),
            EditKind::Delete => lang.pick("supprimer l'étape", "delete step"),
            EditKind::Move => lang.pick("déplacer l'étape", "move step"),
            EditKind::Import => lang.pick("importer des étapes", "import steps"),
            EditKind::Redact => lang.pick("masquer le texte saisi", "redact typed text"),
            EditKind::GuideInfo => lang.pick("modifier le guide", "edit guide details"),
        }
    }
}

/// A state the history can go back to. Only metadata and step records are
/// kept (image bytes stay in the workspace, which never deletes during a
/// session), so the history stays small.
#[derive(Clone)]
struct HistoryEntry {
    kind: EditKind,
    metadata: ProjectMetadata,
    steps: Vec<Step>,
    revision: u64,
}

/// Everything an export or a save needs, detached from the editor so it can
/// run on another thread.
#[derive(Clone, Debug)]
pub struct GuideSnapshot {
    pub metadata: ProjectMetadata,
    pub steps: Vec<Step>,
    pub assets: AssetStore,
    pub revision: u64,
}

/// The guide under edition.
pub struct EditorDocument {
    metadata: ProjectMetadata,
    steps: Vec<Step>,
    workspace: Workspace,
    assets: AssetStore,
    revision: u64,
    next_revision: u64,
    saved_revision: Option<u64>,
    last_export_revision: Option<u64>,
    project_path: Option<PathBuf>,
    export_path: Option<PathBuf>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    autosaved: Option<(u64, bool, Instant)>,
}

impl fmt::Debug for EditorDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EditorDocument")
            .field("title", &self.metadata.title)
            .field("steps", &self.steps.len())
            .field("revision", &self.revision)
            .field("dirty", &self.is_dirty())
            .finish()
    }
}

impl EditorDocument {
    /// An empty, untitled (hence unsaved) guide.
    pub fn new(workspace: Workspace, title: impl Into<String>, lang: Lang) -> io::Result<Self> {
        let assets = AssetStore::new(workspace.dir())?;
        Ok(Self::with_store(
            workspace,
            assets,
            ProjectMetadata::new(title, lang),
        ))
    }

    fn with_store(workspace: Workspace, assets: AssetStore, metadata: ProjectMetadata) -> Self {
        Self {
            metadata,
            steps: Vec::new(),
            workspace,
            assets,
            revision: 0,
            next_revision: 1,
            saved_revision: None,
            last_export_revision: None,
            project_path: None,
            export_path: None,
            undo: Vec::new(),
            redo: Vec::new(),
            autosaved: None,
        }
    }

    /// The guide of a stopped recording: its screenshots move into the
    /// workspace's asset store, and its capture folder is removed.
    pub fn from_session(
        session: &Session,
        workspace: Workspace,
        title: impl Into<String>,
        lang: Lang,
    ) -> io::Result<Self> {
        let mut doc = Self::new(workspace, title, lang)?;
        let mut steps = session.steps.clone();
        for step in &mut steps {
            if step.id.is_none() {
                step.id = Some(Uuid::new_v4());
            }
            let Some(name) = step.screenshot.take() else {
                continue;
            };
            let file = session.dir.join(&name);
            let stored = fs::read(&file).ok().and_then(|bytes| {
                let kind = assets::ImageKind::sniff(&bytes)?;
                doc.assets.put(&bytes, kind).ok().map(|(r, _)| r)
            });
            step.screenshot = stored;
        }
        doc.steps = project::numbered(&steps);
        if session.dir.starts_with(doc.workspace.dir()) {
            let _ = fs::remove_dir_all(&session.dir);
        }
        Ok(doc)
    }

    /// Open a `.ssrproj`: the guide starts clean, bound to that file.
    pub fn open_project(
        path: &Path,
        workspace: Workspace,
        limits: &ImportLimits,
    ) -> Result<(Self, Vec<String>), (ImportError, Workspace)> {
        let assets = match AssetStore::new(workspace.dir()) {
            Ok(a) => a,
            Err(e) => return Err((ImportError::Io(e), workspace)),
        };
        let OpenedProject {
            metadata,
            steps,
            warnings,
        } = match project::open_project(path, &assets, limits) {
            Ok(p) => p,
            Err(e) => return Err((e, workspace)),
        };
        let mut doc = Self::with_store(workspace, assets, metadata);
        doc.steps = steps;
        doc.project_path = Some(path.to_path_buf());
        // A project whose images could not all be restored differs from the
        // file: it is not clean.
        if warnings.is_empty() {
            doc.saved_revision = Some(doc.revision);
        }
        Ok((doc, warnings))
    }

    /// Rebuild a guide from a workspace left behind by a crash.
    pub fn recover(workspace: Workspace) -> io::Result<Self> {
        let bytes = fs::read(workspace.dir().join(RECOVERY_FILE))?;
        let data: RecoveryData = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        let lang = data.metadata.lang();
        let mut doc = Self::new(workspace, data.metadata.title.clone(), lang)?;
        doc.metadata = data.metadata;
        let mut steps = data.steps;
        for s in &mut steps {
            if s.id.is_none() {
                s.id = Some(Uuid::new_v4());
            }
            if s.screenshot
                .as_deref()
                .is_some_and(|r| !doc.assets.contains(r))
            {
                s.screenshot = None;
            }
        }
        doc.steps = project::numbered(&steps);
        doc.project_path = data.project_path;
        Ok(doc)
    }

    // ── Reading ─────────────────────────────────────────────────────

    pub fn steps(&self) -> &[Step] {
        &self.steps
    }

    pub fn metadata(&self) -> &ProjectMetadata {
        &self.metadata
    }

    pub fn assets(&self) -> &AssetStore {
        &self.assets
    }

    /// Folder asset references are relative to.
    pub fn asset_root(&self) -> &Path {
        self.assets.root()
    }

    pub fn workspace_dir(&self) -> &Path {
        self.workspace.dir()
    }

    pub fn step(&self, id: Uuid) -> Option<&Step> {
        self.steps.iter().find(|s| s.id == Some(id))
    }

    /// Zero-based position of step `id`.
    pub fn position(&self, id: Uuid) -> Option<usize> {
        self.steps.iter().position(|s| s.id == Some(id))
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Changes since the last project save (an export does not count).
    pub fn is_dirty(&self) -> bool {
        self.saved_revision != Some(self.revision)
    }

    /// Whether the current state was exported to a ZIP.
    pub fn is_exported(&self) -> bool {
        self.last_export_revision == Some(self.revision)
    }

    pub fn project_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    pub fn export_path(&self) -> Option<&Path> {
        self.export_path.as_deref()
    }

    pub fn can_undo(&self) -> Option<EditKind> {
        self.undo.last().map(|e| e.kind)
    }

    pub fn can_redo(&self) -> Option<EditKind> {
        self.redo.last().map(|e| e.kind)
    }

    /// A copy for a save or an export on another thread.
    pub fn snapshot(&self) -> GuideSnapshot {
        GuideSnapshot {
            metadata: self.metadata.clone(),
            steps: self.steps.clone(),
            assets: self.assets.clone(),
            revision: self.revision,
        }
    }

    // ── Save / export bookkeeping ───────────────────────────────────

    /// Save to `path` (on the calling thread).
    pub fn save_project(&mut self, path: &Path) -> Result<(), project::SaveError> {
        let snap = self.snapshot();
        project::save_project(&snap, path)?;
        self.mark_saved(path, snap.revision);
        Ok(())
    }

    /// Record that `revision` was saved to `path` (a save done elsewhere).
    pub fn mark_saved(&mut self, path: &Path, revision: u64) {
        self.project_path = Some(path.to_path_buf());
        if revision == self.revision {
            self.saved_revision = Some(revision);
        }
    }

    /// Record that `revision` was exported to `path`. The project stays dirty.
    pub fn mark_exported(&mut self, path: &Path, revision: u64) {
        self.export_path = Some(path.to_path_buf());
        self.last_export_revision = Some(revision);
    }

    // ── Commands ────────────────────────────────────────────────────

    fn index_of(&self, id: Uuid) -> Result<usize, EditError> {
        self.position(id).ok_or(EditError::UnknownStep(id))
    }

    /// Apply `change` to a copy of the state; commit it (with an undo entry)
    /// only if it succeeded.
    fn edit<T>(
        &mut self,
        kind: EditKind,
        change: impl FnOnce(&mut ProjectMetadata, &mut Vec<Step>) -> Result<T, EditError>,
    ) -> Result<T, EditError> {
        let mut metadata = self.metadata.clone();
        let mut steps = self.steps.clone();
        let out = change(&mut metadata, &mut steps)?;
        self.undo.push(HistoryEntry {
            kind,
            metadata: std::mem::replace(&mut self.metadata, metadata),
            steps: std::mem::replace(&mut self.steps, project::numbered(&steps)),
            revision: self.revision,
        });
        if self.undo.len() > HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.revision = self.next_revision;
        self.next_revision += 1;
        Ok(out)
    }

    /// Set (or clear, with `None` or blank text) the custom title of a step.
    pub fn set_title(&mut self, id: Uuid, title: Option<String>) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        let title = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
        if self.steps[i].title_override == title {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::Rename, |_, steps| {
            steps[i].title_override = title;
            Ok(())
        })
    }

    pub fn set_notes(&mut self, id: Uuid, notes: String) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        let notes = notes.trim_end().to_string();
        if self.steps[i].notes == notes {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::Notes, |_, steps| {
            steps[i].notes = notes;
            Ok(())
        })
    }

    /// Unlink the screenshot of a step; the step, its text and its place stay.
    pub fn detach_screenshot(&mut self, id: Uuid) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        if self.steps[i].screenshot.is_none() {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::DetachImage, |_, steps| {
            steps[i].screenshot = None;
            Ok(())
        })
    }

    /// Attach (or replace) the screenshot of a step with an image of the
    /// workspace.
    pub fn attach_screenshot(&mut self, id: Uuid, asset_ref: String) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        if !self.assets.contains(&asset_ref) {
            return Err(EditError::InvalidAsset(asset_ref));
        }
        if self.steps[i].screenshot.as_deref() == Some(asset_ref.as_str()) {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::AttachImage, |_, steps| {
            steps[i].screenshot = Some(asset_ref);
            Ok(())
        })
    }

    pub fn delete_step(&mut self, id: Uuid) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        self.edit(EditKind::Delete, |_, steps| {
            steps.remove(i);
            Ok(())
        })
    }

    /// Move step `id` to zero-based position `new_index`.
    pub fn move_step(&mut self, id: Uuid, new_index: usize) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        let len = self.steps.len();
        if new_index >= len {
            return Err(EditError::OutOfRange {
                index: new_index,
                len,
            });
        }
        if new_index == i {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::Move, |_, steps| {
            let s = steps.remove(i);
            steps.insert(new_index, s);
            Ok(())
        })
    }

    /// Insert `new_steps` at zero-based `position` (`len` appends), each with
    /// a fresh id. One undo entry for the whole batch.
    pub fn insert_steps(
        &mut self,
        position: usize,
        new_steps: Vec<Step>,
    ) -> Result<Vec<Uuid>, EditError> {
        self.insert_batch(EditKind::AddStep, position, new_steps, Vec::new())
    }

    /// Insert the selected steps of an imported recording and remember the
    /// archive among the guide's sources.
    pub fn import_steps(
        &mut self,
        position: usize,
        recording: &ImportedRecording,
        selected: &[usize],
    ) -> Result<Vec<Uuid>, EditError> {
        self.import_many(position, &[(recording, selected)])
    }

    /// Insert steps picked from several recordings, in the order given, as
    /// one undoable import.
    pub fn import_many(
        &mut self,
        position: usize,
        selections: &[(&ImportedRecording, &[usize])],
    ) -> Result<Vec<Uuid>, EditError> {
        let mut steps = Vec::new();
        let mut sources = Vec::new();
        for (recording, selected) in selections {
            let picked: Vec<Step> = selected
                .iter()
                .filter_map(|&i| recording.steps.get(i))
                .map(|s| s.step.clone())
                .collect();
            if picked.is_empty() {
                continue;
            }
            sources.push(SourceArchive {
                label: recording.label.clone(),
                digest: recording.source_digest.clone(),
                imported_at: now_rfc3339(),
                step_count: picked.len(),
            });
            steps.extend(picked);
        }
        if steps.is_empty() {
            return Err(EditError::NoChange);
        }
        self.insert_batch(EditKind::Import, position, steps, sources)
    }

    fn insert_batch(
        &mut self,
        kind: EditKind,
        position: usize,
        new_steps: Vec<Step>,
        sources: Vec<SourceArchive>,
    ) -> Result<Vec<Uuid>, EditError> {
        let len = self.steps.len();
        if position > len {
            return Err(EditError::OutOfRange {
                index: position,
                len,
            });
        }
        if new_steps.is_empty() {
            return Err(EditError::NoChange);
        }
        for s in &new_steps {
            if let Some(r) = &s.screenshot {
                if !self.assets.contains(r) {
                    return Err(EditError::InvalidAsset(r.clone()));
                }
            }
        }
        self.edit(kind, |meta, steps| {
            let mut ids = Vec::with_capacity(new_steps.len());
            for (k, mut s) in new_steps.into_iter().enumerate() {
                let id = Uuid::new_v4();
                s.id = Some(id);
                ids.push(id);
                steps.insert(position + k, s);
            }
            meta.sources.extend(sources);
            Ok(ids)
        })
    }

    /// A new hand-written step at `position`, optionally with an image.
    pub fn add_manual_step(
        &mut self,
        position: usize,
        title: impl Into<String>,
        screenshot: Option<String>,
    ) -> Result<Uuid, EditError> {
        let step = Step::manual(title, screenshot);
        self.insert_steps(position, vec![step]).map(|ids| ids[0])
    }

    /// Replace the typed text of a keyboard step (it may hold a password).
    pub fn redact_text(&mut self, id: Uuid) -> Result<(), EditError> {
        let i = self.index_of(id)?;
        match &self.steps[i].action {
            Action::Text { content } if content != REDACTED && !content.is_empty() => {}
            _ => return Err(EditError::NoChange),
        }
        let lang = self.metadata.lang();
        self.edit(EditKind::Redact, |_, steps| {
            let s = &mut steps[i];
            s.action = Action::Text {
                content: REDACTED.into(),
            };
            s.description = Step::build_description(&s.action, s.window.as_ref(), lang);
            Ok(())
        })
    }

    /// Change the guide title and introduction.
    pub fn set_guide_info(&mut self, title: String, description: String) -> Result<(), EditError> {
        let title = title.trim().to_string();
        let description = description.trim_end().to_string();
        if self.metadata.title == title && self.metadata.description == description {
            return Err(EditError::NoChange);
        }
        self.edit(EditKind::GuideInfo, |meta, _| {
            meta.title = title;
            meta.description = description;
            Ok(())
        })
    }

    // ── History ─────────────────────────────────────────────────────

    pub fn undo(&mut self) -> Result<EditKind, EditError> {
        let entry = self.undo.pop().ok_or(EditError::EmptyHistory)?;
        let kind = entry.kind;
        let current = HistoryEntry {
            kind,
            metadata: std::mem::replace(&mut self.metadata, entry.metadata),
            steps: std::mem::replace(&mut self.steps, entry.steps),
            revision: std::mem::replace(&mut self.revision, entry.revision),
        };
        self.redo.push(current);
        Ok(kind)
    }

    pub fn redo(&mut self) -> Result<EditKind, EditError> {
        let entry = self.redo.pop().ok_or(EditError::EmptyHistory)?;
        let kind = entry.kind;
        let current = HistoryEntry {
            kind,
            metadata: std::mem::replace(&mut self.metadata, entry.metadata),
            steps: std::mem::replace(&mut self.steps, entry.steps),
            revision: std::mem::replace(&mut self.revision, entry.revision),
        };
        self.undo.push(current);
        Ok(kind)
    }

    /// Forget the undo/redo history (after building a freshly opened guide).
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    // ── Recovery ────────────────────────────────────────────────────

    /// Whether the recovery copy is behind the current state.
    pub fn needs_autosave(&self) -> bool {
        match self.autosaved {
            None => true,
            Some((rev, dirty, _)) => rev != self.revision || dirty != self.is_dirty(),
        }
    }

    /// Write the crash-recovery copy now.
    pub fn write_recovery(&mut self) -> io::Result<()> {
        let data = RecoveryData {
            metadata: self.metadata.clone(),
            steps: self.steps.clone(),
            project_path: self.project_path.clone(),
            dirty: self.is_dirty(),
            written_at: now_rfc3339(),
        };
        let json = serde_json::to_vec(&data).map_err(io::Error::other)?;
        let path = self.workspace.dir().join(RECOVERY_FILE);
        project::write_private_atomically(&path, |mut f| {
            io::Write::write_all(&mut f, &json)?;
            Ok(f)
        })?;
        self.autosaved = Some((self.revision, data.dirty, Instant::now()));
        Ok(())
    }

    /// Write the recovery copy if the guide changed and `min_interval` has
    /// passed since the last one. Returns whether it wrote.
    pub fn autosave(&mut self, min_interval: Duration) -> bool {
        let due = self.needs_autosave()
            && self
                .autosaved
                .is_none_or(|(_, _, at)| at.elapsed() >= min_interval);
        due && self.write_recovery().is_ok()
    }

    /// Close the guide and delete its workspace (images, recovery copy).
    pub fn close(self) {
        self.workspace.discard();
    }
}
