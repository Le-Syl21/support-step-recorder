//! The editable guide project, `.ssrproj`: a ZIP holding
//!
//! ```text
//! project.json      schema version, title, introduction, language, dates, sources
//! steps.json        the steps in guide order (the enhanced `Step` model)
//! assets/<sha256>.<ext>   every image a step references, and only those
//! ```
//!
//! No HTML inside: the report is derived output, regenerated on export.
//! Saving writes a sibling temporary file and renames it over the target only
//! once it is complete, so an interrupted save leaves the previous file intact.

use std::collections::BTreeSet;
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::archive::SafeArchive;
use crate::assets::{self, AssetStore, PutError};
use crate::editor::GuideSnapshot;
use crate::i18n::Lang;
use crate::import::{read_project_metadata, read_steps, ImportError, ImportLimits};
use crate::model::Step;

/// Version of the project format written by this build.
pub const SCHEMA_VERSION: u32 = 1;
/// File extension of a project.
pub const PROJECT_EXT: &str = "ssrproj";
/// Name of the metadata member.
pub const PROJECT_FILE: &str = "project.json";

/// `project.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub schema_version: u32,
    pub title: String,
    /// Short introduction shown at the top of the report.
    #[serde(default)]
    pub description: String,
    /// Language the guide was written in ("fr" / "en").
    #[serde(default)]
    pub language: String,
    /// RFC 3339.
    pub created_at: String,
    /// RFC 3339.
    pub modified_at: String,
    #[serde(default)]
    pub generator: String,
    /// The archives steps were imported from.
    #[serde(default)]
    pub sources: Vec<SourceArchive>,
}

/// An archive some steps were imported from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceArchive {
    pub label: String,
    pub digest: String,
    pub imported_at: String,
    pub step_count: usize,
}

impl ProjectMetadata {
    pub fn new(title: impl Into<String>, lang: Lang) -> Self {
        let now = now_rfc3339();
        Self {
            schema_version: SCHEMA_VERSION,
            title: title.into(),
            description: String::new(),
            language: lang.code().into(),
            created_at: now.clone(),
            modified_at: now,
            generator: generator(),
            sources: Vec::new(),
        }
    }

    pub fn lang(&self) -> Lang {
        if self.language == "en" {
            Lang::En
        } else {
            Lang::Fr
        }
    }
}

pub(crate) fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

fn generator() -> String {
    format!("support-step-recorder {}", env!("CARGO_PKG_VERSION"))
}

/// Failure to save a project or export a guide.
#[derive(Debug)]
pub enum SaveError {
    Io(io::Error),
    /// A step points at an image that is not in the workspace any more.
    MissingAsset(String),
}

impl SaveError {
    pub fn message(&self, lang: Lang) -> String {
        match self {
            SaveError::Io(e) => e.to_string(),
            SaveError::MissingAsset(a) => match lang {
                Lang::Fr => format!("Image introuvable dans l'espace de travail : {a}. Retirez la capture de l'étape concernée puis réessayez."),
                Lang::En => format!("Image missing from the workspace: {a}. Remove the screenshot from that step and try again."),
            },
        }
    }
}

impl fmt::Display for SaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(Lang::En))
    }
}

impl std::error::Error for SaveError {}

impl From<io::Error> for SaveError {
    fn from(e: io::Error) -> Self {
        SaveError::Io(e)
    }
}

/// Steps renumbered 1..N in their current order.
pub(crate) fn numbered(steps: &[Step]) -> Vec<Step> {
    steps
        .iter()
        .enumerate()
        .map(|(i, s)| Step {
            index: i + 1,
            ..s.clone()
        })
        .collect()
}

/// The distinct images the steps reference, all checked to be present.
pub(crate) fn referenced_assets(
    steps: &[Step],
    store: &AssetStore,
) -> Result<Vec<String>, SaveError> {
    let refs: BTreeSet<String> = steps.iter().filter_map(|s| s.screenshot.clone()).collect();
    for r in &refs {
        if !store.contains(r) {
            return Err(SaveError::MissingAsset(r.clone()));
        }
    }
    Ok(refs.into_iter().collect())
}

/// Write `path` through a sibling temporary file: `fill` writes the content,
/// then the file is flushed to disk and renamed over `path`. On any failure
/// the temporary file is removed and `path` is left as it was.
pub fn write_atomically(
    path: &Path,
    fill: impl FnOnce(File) -> io::Result<File>,
) -> io::Result<()> {
    write_atomically_with(path, false, fill)
}

/// [`write_atomically`], the file being readable by its owner only (0600).
pub(crate) fn write_private_atomically(
    path: &Path,
    fill: impl FnOnce(File) -> io::Result<File>,
) -> io::Result<()> {
    write_atomically_with(path, true, fill)
}

fn write_atomically_with(
    path: &Path,
    private: bool,
    fill: impl FnOnce(File) -> io::Result<File>,
) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "output".into());
    let tmp = dir.join(format!(".{name}.{}.tmp", uuid::Uuid::new_v4().simple()));
    let result = (|| {
        let file = if private {
            crate::assets::private_file(&tmp)?
        } else {
            fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&tmp)?
        };
        let file = fill(file)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)?;
        #[cfg(unix)]
        if let Ok(d) = File::open(dir) {
            let _ = d.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Save the guide as an editable project at `path`.
pub fn save_project(guide: &GuideSnapshot, path: &Path) -> Result<(), SaveError> {
    let steps = numbered(&guide.steps);
    let assets = referenced_assets(&steps, &guide.assets)?;
    let mut meta = guide.metadata.clone();
    meta.schema_version = SCHEMA_VERSION;
    meta.modified_at = now_rfc3339();
    meta.generator = generator();
    let meta_json = serde_json::to_vec_pretty(&meta).map_err(io::Error::other)?;
    let steps_json = serde_json::to_vec_pretty(&steps).map_err(io::Error::other)?;

    write_atomically(path, |file| {
        let mut zip = ZipWriter::new(file);
        let text =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        // Images are already compressed: stored as they are.
        let raw = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        zip.start_file(PROJECT_FILE, text)
            .map_err(io::Error::other)?;
        zip.write_all(&meta_json)?;
        zip.start_file("steps.json", text)
            .map_err(io::Error::other)?;
        zip.write_all(&steps_json)?;
        for a in &assets {
            let bytes = guide.assets.read(a)?;
            zip.start_file(a.as_str(), raw).map_err(io::Error::other)?;
            zip.write_all(&bytes)?;
        }
        zip.finish().map_err(io::Error::other)
    })?;
    Ok(())
}

/// A project read back from disk.
#[derive(Debug)]
pub struct OpenedProject {
    pub metadata: ProjectMetadata,
    pub steps: Vec<Step>,
    /// Images that could not be restored (their steps come without them).
    pub warnings: Vec<String>,
}

/// Read a `.ssrproj` into `store`. Its images are checked against their
/// content hash and decoded before being accepted.
pub fn open_project(
    path: &Path,
    store: &AssetStore,
    limits: &ImportLimits,
) -> Result<OpenedProject, ImportError> {
    let mut archive = SafeArchive::open(path, limits)?;
    if !archive.contains(PROJECT_FILE) {
        return Err(ImportError::BadManifest(format!(
            "{PROJECT_FILE} missing — use “Open recording” for an exported ZIP"
        )));
    }
    let metadata = read_project_metadata(&mut archive, limits)?;
    let mut steps = read_steps(&mut archive, limits)?;
    let mut warnings = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for step in &mut steps {
        // Ids must be present and unique: the editor relies on them.
        if step.id.is_none_or(|id| !seen.insert(id)) {
            let id = uuid::Uuid::new_v4();
            seen.insert(id);
            step.id = Some(id);
        }
        let Some(name) = step.screenshot.take() else {
            continue;
        };
        match restore_asset(&mut archive, store, limits, &name) {
            Ok(r) => step.screenshot = Some(r),
            Err(w) => warnings.push(w),
        }
    }
    let steps = numbered(&steps);
    Ok(OpenedProject {
        metadata,
        steps,
        warnings,
    })
}

fn restore_asset(
    archive: &mut SafeArchive,
    store: &AssetStore,
    limits: &ImportLimits,
    name: &str,
) -> Result<String, String> {
    let Some((digest, _)) = assets::parse_asset_ref(name) else {
        return Err(format!("{name}: not a project image"));
    };
    let bytes = archive
        .read(name, limits.max_image_bytes)
        .map_err(|e| format!("{name}: {e}"))?;
    if assets::sha256_hex(&bytes) != digest {
        return Err(format!("{name}: content does not match its checksum"));
    }
    match store.put_checked(&bytes, limits.max_pixels) {
        Ok((r, _)) if r == name => Ok(r),
        Ok((r, _)) => Err(format!("{name}: stored as {r} (format mismatch)")),
        Err(PutError::Image(e)) => Err(format!("{name}: {e}")),
        Err(PutError::Io(e)) => Err(format!("{name}: {e}")),
    }
}
