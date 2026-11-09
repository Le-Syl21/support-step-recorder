//! Private working folders: where a recording writes its screenshots and
//! where an open guide keeps its images and its crash-recovery copy.
//!
//! They live under the user's cache directory (`$XDG_CACHE_HOME/ssr` on
//! Linux), never in a shared, predictable `/tmp` path, with mode 0700. Each
//! open workspace holds an exclusive lock on its `lock` file: a workspace
//! whose lock can be taken at startup belongs to no running instance, which
//! is how a crash is told apart from another window.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::Step;
use crate::project::ProjectMetadata;

const LOCK_FILE: &str = "lock";
/// The crash-recovery copy of the guide being edited.
pub const RECOVERY_FILE: &str = "recovery.json";

/// Base folder of all workspaces (`<cache>/ssr`).
pub fn cache_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("SSR_CACHE_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(dir);
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(d) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(d).join("ssr");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join("Library/Caches/ssr");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(d) = std::env::var_os("XDG_CACHE_HOME").filter(|d| !d.is_empty()) {
            return PathBuf::from(d).join("ssr");
        }
        if let Some(h) = std::env::var_os("HOME") {
            return PathBuf::from(h).join(".cache/ssr");
        }
    }
    // Last resort: still a per-user, unpredictable name inside the temp dir.
    std::env::temp_dir().join(format!("ssr-{}", uuid::Uuid::new_v4().simple()))
}

/// Folder holding one workspace per open guide or recording.
pub fn workspaces_root() -> PathBuf {
    cache_root().join("workspaces")
}

/// `create_dir_all`, then restrict the leaf to its owner (0700 on Unix).
pub fn create_private_dir(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// A private, locked working folder.
#[derive(Debug)]
pub struct Workspace {
    dir: PathBuf,
    _lock: fs::File,
}

impl Workspace {
    /// A new workspace under [`workspaces_root`].
    pub fn create() -> io::Result<Self> {
        Self::create_in(&workspaces_root())
    }

    /// A new workspace under `base` (tests use a temporary base).
    pub fn create_in(base: &Path) -> io::Result<Self> {
        create_private_dir(base)?;
        let dir = base.join(uuid::Uuid::new_v4().simple().to_string());
        create_private_dir(&dir)?;
        Self::adopt(&dir)
    }

    /// Take ownership of an existing workspace folder (recovery).
    pub fn adopt(dir: &Path) -> io::Result<Self> {
        let lock = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join(LOCK_FILE))?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "workspace already in use",
                ))
            }
            Err(fs::TryLockError::Error(e)) => return Err(e),
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            _lock: lock,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// A private sub-folder (the recorder's capture folder, for instance).
    pub fn subdir(&self, name: &str) -> io::Result<PathBuf> {
        let d = self.dir.join(name);
        create_private_dir(&d)?;
        Ok(d)
    }

    /// Delete the workspace and everything in it.
    pub fn discard(self) {
        let dir = self.dir.clone();
        drop(self);
        let _ = fs::remove_dir_all(dir);
    }
}

/// What the recovery file holds: enough to rebuild the editor state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecoveryData {
    pub metadata: ProjectMetadata,
    pub steps: Vec<Step>,
    #[serde(default)]
    pub project_path: Option<PathBuf>,
    /// `true` when the guide had changes not saved to its project.
    pub dirty: bool,
    /// When the copy was written (RFC 3339).
    pub written_at: String,
}

/// A workspace left behind by an instance that did not close cleanly.
#[derive(Debug, Clone)]
pub struct Recoverable {
    pub dir: PathBuf,
    pub title: String,
    pub step_count: usize,
    pub written_at: String,
}

/// Look for abandoned workspaces under `base`. Those with unsaved changes are
/// returned for the user to recover or discard; the others (nothing to lose:
/// a clean guide, an interrupted recording without steps on disk) are deleted.
pub fn scan_abandoned(base: &Path) -> Vec<Recoverable> {
    let Ok(entries) = fs::read_dir(base) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.is_dir() {
            continue;
        }
        // In use by a running instance: leave it alone.
        let Ok(ws) = Workspace::adopt(&dir) else {
            continue;
        };
        let data = fs::read(dir.join(RECOVERY_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice::<RecoveryData>(&b).ok());
        match data {
            Some(d) if d.dirty => {
                found.push(Recoverable {
                    dir: dir.clone(),
                    title: d.metadata.title.clone(),
                    step_count: d.steps.len(),
                    written_at: d.written_at.clone(),
                });
                drop(ws);
            }
            _ => ws.discard(),
        }
    }
    found.sort_by(|a, b| b.written_at.cmp(&a.written_at));
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_live_workspace_is_not_scanned_and_a_clean_one_is_removed() {
        let base = std::env::temp_dir().join(format!("ssr-ws-{}", uuid::Uuid::new_v4()));
        let live = Workspace::create_in(&base).unwrap();
        let abandoned = Workspace::create_in(&base).unwrap();
        let abandoned_dir = abandoned.dir().to_path_buf();
        drop(abandoned); // lock released, no recovery file
        assert!(scan_abandoned(&base).is_empty());
        assert!(live.dir().exists(), "a locked workspace is left alone");
        assert!(!abandoned_dir.exists(), "nothing to recover: removed");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(live.dir()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700);
        }
        live.discard();
        let _ = fs::remove_dir_all(&base);
    }
}
