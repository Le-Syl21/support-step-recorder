//! Import of earlier recordings (v0.2.0 exported ZIPs, or `.ssrproj`
//! projects) and of standalone images into a guide's workspace.
//!
//! Everything read here is untrusted. Only `steps.json` (and `project.json`
//! for a project) and the images those manifests name are ever read;
//! `report.html` is derived output and is ignored. The import is
//! transactional: either every step comes back staged and checked, or the
//! images it had already stored are removed again and nothing is returned.

use std::fmt;
use std::io;
use std::path::Path;

use uuid::Uuid;

use crate::archive::{safe_member_name, SafeArchive};
use crate::assets::{AssetStore, PutError};
use crate::i18n::Lang;
use crate::model::{Step, StepSource};
use crate::project::{self, ProjectMetadata};

/// Budgets applied to anything imported. They are generous for real
/// recordings and exist so that a hostile archive cannot exhaust memory.
#[derive(Debug, Clone)]
pub struct ImportLimits {
    pub max_entries: usize,
    pub max_steps: usize,
    pub max_json_bytes: u64,
    pub max_image_bytes: u64,
    pub max_total_uncompressed: u64,
    pub max_pixels: u64,
}

impl Default for ImportLimits {
    fn default() -> Self {
        Self {
            max_entries: 3000,
            max_steps: 1000,
            max_json_bytes: 8 * 1024 * 1024,
            max_image_bytes: 25 * 1024 * 1024,
            max_total_uncompressed: 256 * 1024 * 1024,
            max_pixels: 100_000_000,
        }
    }
}

/// Why an archive (or an image) could not be imported.
#[derive(Debug)]
pub enum ImportError {
    Io(io::Error),
    NotAnArchive(String),
    UnsafePath(String),
    Symlink(String),
    DuplicateEntry(String),
    TooManyEntries(usize),
    TooManySteps(usize),
    TooLarge(String),
    MissingManifest,
    MissingMember(String),
    BadManifest(String),
    UnsupportedVersion(u32),
    BadImage(String),
}

impl ImportError {
    /// The message shown to the user.
    pub fn message(&self, lang: Lang) -> String {
        let fr = lang == Lang::Fr;
        match self {
            ImportError::Io(e) => format!(
                "{} {e}",
                if fr {
                    "Erreur de lecture :"
                } else {
                    "Read error:"
                }
            ),
            ImportError::NotAnArchive(e) => {
                if fr {
                    format!("Ce fichier n'est pas une archive ZIP lisible ({e}).")
                } else {
                    format!("This file is not a readable ZIP archive ({e}).")
                }
            }
            ImportError::UnsafePath(n) => {
                if fr {
                    format!("Archive refusée : chemin dangereux « {n} ».")
                } else {
                    format!("Archive refused: unsafe path “{n}”.")
                }
            }
            ImportError::Symlink(n) => {
                if fr {
                    format!("Archive refusée : lien symbolique « {n} ».")
                } else {
                    format!("Archive refused: symbolic link “{n}”.")
                }
            }
            ImportError::DuplicateEntry(n) => {
                if fr {
                    format!("Archive refusée : entrées en double ({n}).")
                } else {
                    format!("Archive refused: duplicate entries ({n}).")
                }
            }
            ImportError::TooManyEntries(n) => {
                if fr {
                    format!("Archive refusée : trop de fichiers ({n}).")
                } else {
                    format!("Archive refused: too many files ({n}).")
                }
            }
            ImportError::TooManySteps(n) => {
                if fr {
                    format!("Trop d'étapes dans cet enregistrement ({n}).")
                } else {
                    format!("Too many steps in this recording ({n}).")
                }
            }
            ImportError::TooLarge(n) => {
                if fr {
                    format!("Contenu trop volumineux : {n}.")
                } else {
                    format!("Content too large: {n}.")
                }
            }
            ImportError::MissingManifest => lang
                .pick(
                    "Ce n'est pas un enregistrement Support Step Recorder (steps.json absent).",
                    "This is not a Support Step Recorder recording (no steps.json).",
                )
                .into(),
            ImportError::MissingMember(n) => {
                if fr {
                    format!("Fichier manquant dans l'archive : {n}.")
                } else {
                    format!("File missing from the archive: {n}.")
                }
            }
            ImportError::BadManifest(e) => {
                if fr {
                    format!("Fichier de description illisible : {e}.")
                } else {
                    format!("Unreadable description file: {e}.")
                }
            }
            ImportError::UnsupportedVersion(v) => {
                if fr {
                    format!("Ce projet a été créé par une version plus récente (format {v}). Mettez Support Step Recorder à jour pour l'ouvrir ; il n'a pas été modifié.")
                } else {
                    format!("This project was made by a newer version (format {v}). Update Support Step Recorder to open it; it was not modified.")
                }
            }
            ImportError::BadImage(e) => {
                if fr {
                    format!("Image refusée : {e}.")
                } else {
                    format!("Image refused: {e}.")
                }
            }
        }
    }
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message(Lang::En))
    }
}

impl std::error::Error for ImportError {}

/// One step staged for import.
#[derive(Debug, Clone)]
pub struct ImportedStep {
    /// A copy with a fresh id and its provenance; `screenshot` points into
    /// the destination store (or is `None`).
    pub step: Step,
    /// Set when the step had a screenshot that could not be imported (missing,
    /// broken, too large): the step comes without it.
    pub image_warning: Option<String>,
}

/// A recording read from an archive, ready to be inserted in a guide.
#[derive(Debug, Clone)]
pub struct ImportedRecording {
    /// File name of the archive.
    pub label: String,
    /// SHA-256 of the archive.
    pub source_digest: String,
    /// Project title, when the archive is a `.ssrproj`.
    pub title: Option<String>,
    pub steps: Vec<ImportedStep>,
    /// Assets this import added to the store (for a rollback).
    pub created_assets: Vec<String>,
}

impl ImportedRecording {
    /// Remove the assets this import created (the user cancelled it).
    pub fn rollback(&self, store: &AssetStore) {
        for a in &self.created_assets {
            store.remove(a);
        }
    }
}

/// Read an exported recording (or a project) as a source of steps.
///
/// Each step becomes a *copy*: fresh id, provenance pointing at the archive,
/// original action and timestamps kept. The archive itself is only read.
pub fn import_recording(
    path: &Path,
    store: &AssetStore,
    limits: &ImportLimits,
) -> Result<ImportedRecording, ImportError> {
    let mut archive = SafeArchive::open(path, limits)?;
    let title = if archive.contains(project::PROJECT_FILE) {
        Some(read_project_metadata(&mut archive, limits)?.title)
    } else {
        None
    };
    let steps = read_steps(&mut archive, limits)?;

    let label = archive.label.clone();
    let digest = archive.digest.clone();
    let mut created = Vec::new();
    let mut out = Vec::with_capacity(steps.len());
    for original in steps {
        let mut step = original.clone();
        step.id = Some(Uuid::new_v4());
        if step.source.is_none() {
            step.source = Some(StepSource {
                source_label: label.clone(),
                source_digest: digest.clone(),
                original_index: original.index,
                original_step_id: original.id.map(|i| i.to_string()),
            });
        }
        let (screenshot, warning) = match stage_image(
            &mut archive,
            store,
            limits,
            original.screenshot.as_deref(),
            &mut created,
        ) {
            Ok(v) => v,
            Err(fatal) => {
                for a in &created {
                    store.remove(a);
                }
                return Err(fatal);
            }
        };
        step.screenshot = screenshot;
        out.push(ImportedStep {
            step,
            image_warning: warning,
        });
    }
    Ok(ImportedRecording {
        label,
        source_digest: digest,
        title,
        steps: out,
        created_assets: created,
    })
}

/// Copy a standalone PNG/JPEG/WebP into the store. The file is only read.
pub fn import_image_file(
    path: &Path,
    store: &AssetStore,
    limits: &ImportLimits,
) -> Result<(String, bool), ImportError> {
    let meta = std::fs::metadata(path).map_err(ImportError::Io)?;
    if !meta.is_file() {
        return Err(ImportError::BadImage(path.display().to_string()));
    }
    if meta.len() > limits.max_image_bytes {
        return Err(ImportError::TooLarge(path.display().to_string()));
    }
    let bytes = std::fs::read(path).map_err(ImportError::Io)?;
    store
        .put_checked(&bytes, limits.max_pixels)
        .map_err(|e| match e {
            PutError::Image(e) => ImportError::BadImage(e.to_string()),
            PutError::Io(e) => ImportError::Io(e),
        })
}

/// `project.json`, refusing a format newer than this build understands.
pub(crate) fn read_project_metadata(
    archive: &mut SafeArchive,
    limits: &ImportLimits,
) -> Result<ProjectMetadata, ImportError> {
    let bytes = archive.read(project::PROJECT_FILE, limits.max_json_bytes)?;
    let value: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| ImportError::BadManifest(e.to_string()))?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ImportError::BadManifest("schema_version".into()))?;
    if version > u64::from(project::SCHEMA_VERSION) {
        return Err(ImportError::UnsupportedVersion(
            u32::try_from(version).unwrap_or(u32::MAX),
        ));
    }
    if version == 0 {
        return Err(ImportError::BadManifest("schema_version".into()));
    }
    serde_json::from_value(value).map_err(|e| ImportError::BadManifest(e.to_string()))
}

/// `steps.json`, bounded, with every screenshot name checked before any
/// image is touched.
pub(crate) fn read_steps(
    archive: &mut SafeArchive,
    limits: &ImportLimits,
) -> Result<Vec<Step>, ImportError> {
    if !archive.contains("steps.json") {
        return Err(ImportError::MissingManifest);
    }
    let bytes = archive.read("steps.json", limits.max_json_bytes)?;
    // Count first: a huge array is refused before being materialised.
    let raw: Vec<serde_json::Value> =
        serde_json::from_slice(&bytes).map_err(|e| ImportError::BadManifest(e.to_string()))?;
    if raw.len() > limits.max_steps {
        return Err(ImportError::TooManySteps(raw.len()));
    }
    let steps: Vec<Step> = raw
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .map_err(|e| ImportError::BadManifest(e.to_string()))?;
    for s in &steps {
        if let Some(name) = &s.screenshot {
            if !safe_member_name(name) {
                return Err(ImportError::UnsafePath(name.clone()));
            }
        }
    }
    Ok(steps)
}

/// Read, check and store one screenshot. A missing or broken image is a
/// per-step warning (the step can still come as text); an I/O failure of the
/// destination is fatal.
pub(crate) fn stage_image(
    archive: &mut SafeArchive,
    store: &AssetStore,
    limits: &ImportLimits,
    name: Option<&str>,
    created: &mut Vec<String>,
) -> Result<(Option<String>, Option<String>), ImportError> {
    let Some(name) = name else {
        return Ok((None, None));
    };
    if !archive.contains(name) {
        return Ok((None, Some(format!("{name}: missing"))));
    }
    let bytes = match archive.read(name, limits.max_image_bytes) {
        Ok(b) => b,
        Err(ImportError::TooLarge(n)) => return Ok((None, Some(format!("{n}: too large")))),
        Err(ImportError::NotAnArchive(e)) => return Ok((None, Some(format!("{name}: {e}")))),
        Err(e) => return Err(e),
    };
    match store.put_checked(&bytes, limits.max_pixels) {
        Ok((asset, new)) => {
            if new {
                created.push(asset.clone());
            }
            Ok((Some(asset), None))
        }
        Err(PutError::Image(e)) => Ok((None, Some(format!("{name}: {e}")))),
        Err(PutError::Io(e)) => Err(ImportError::Io(e)),
    }
}
