//! Defensive reading of user-supplied ZIP archives (exported recordings and
//! `.ssrproj` projects).
//!
//! Nothing is ever extracted to disk by name: members are read into memory,
//! one at a time, under size budgets, and only when the manifest asks for
//! them. The whole archive is refused when any member has a dangerous name
//! (absolute, `..`, backslash, NUL, drive letter), is a symbolic link, or is
//! listed twice — an ambiguous archive is not worth guessing about.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use zip::ZipArchive;

use crate::import::{ImportError, ImportLimits};

/// Whether `name` is a plain relative member path, safe to look up.
pub fn safe_member_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 512 {
        return false;
    }
    if name.contains('\0') || name.contains('\\') || name.contains(':') {
        return false;
    }
    if name.starts_with('/') {
        return false;
    }
    let trimmed = name.strip_suffix('/').unwrap_or(name);
    trimmed
        .split('/')
        .all(|c| !c.is_empty() && c != "." && c != "..")
}

/// An opened archive whose directory has been checked.
pub struct SafeArchive {
    zip: ZipArchive<File>,
    index: HashMap<String, usize>,
    /// File name of the archive (for messages and provenance).
    pub label: String,
    /// SHA-256 of the archive file.
    pub digest: String,
    budget_left: u64,
}

impl SafeArchive {
    pub fn open(path: &Path, limits: &ImportLimits) -> Result<Self, ImportError> {
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "archive".into());
        let digest = crate::assets::sha256_file(path).map_err(ImportError::Io)?;
        let file = File::open(path).map_err(ImportError::Io)?;
        let mut zip =
            ZipArchive::new(file).map_err(|e| ImportError::NotAnArchive(e.to_string()))?;
        if zip.len() > limits.max_entries {
            return Err(ImportError::TooManyEntries(zip.len()));
        }
        // The reader keys members by name: a second `steps.json` would be
        // silently shadowed. Refuse the ambiguity instead.
        if zip.has_duplicate_names() {
            return Err(ImportError::DuplicateEntry(label));
        }

        let mut index = HashMap::new();
        let mut declared: u64 = 0;
        for i in 0..zip.len() {
            let entry = zip
                .by_index_raw(i)
                .map_err(|e| ImportError::NotAnArchive(e.to_string()))?;
            let name = std::str::from_utf8(entry.name_raw())
                .map_err(|_| {
                    ImportError::UnsafePath(String::from_utf8_lossy(entry.name_raw()).into())
                })?
                .to_string();
            if !safe_member_name(&name) {
                return Err(ImportError::UnsafePath(name));
            }
            if entry.is_symlink() {
                return Err(ImportError::Symlink(name));
            }
            declared = declared.saturating_add(entry.size());
            if entry.is_dir() {
                continue;
            }
            if index.insert(name.clone(), i).is_some() {
                return Err(ImportError::DuplicateEntry(name));
            }
        }
        if declared > limits.max_total_uncompressed {
            return Err(ImportError::TooLarge(label.clone()));
        }
        Ok(Self {
            zip,
            index,
            label,
            digest,
            budget_left: limits.max_total_uncompressed,
        })
    }

    pub fn contains(&self, name: &str) -> bool {
        self.index.contains_key(name)
    }

    /// Read member `name`, at most `max` bytes (its declared size is not
    /// trusted: the actual decompressed stream is bounded too).
    pub fn read(&mut self, name: &str, max: u64) -> Result<Vec<u8>, ImportError> {
        let &i = self
            .index
            .get(name)
            .ok_or_else(|| ImportError::MissingMember(name.to_string()))?;
        let entry = self
            .zip
            .by_index(i)
            .map_err(|e| ImportError::NotAnArchive(e.to_string()))?;
        let cap = max.min(self.budget_left);
        if entry.size() > cap {
            return Err(ImportError::TooLarge(name.to_string()));
        }
        let mut buf = Vec::with_capacity(entry.size().min(cap) as usize);
        entry
            .take(cap + 1)
            .read_to_end(&mut buf)
            .map_err(|e| match e.kind() {
                io::ErrorKind::InvalidData => ImportError::NotAnArchive(e.to_string()),
                _ => ImportError::Io(e),
            })?;
        if buf.len() as u64 > cap {
            return Err(ImportError::TooLarge(name.to_string()));
        }
        self.budget_left -= buf.len() as u64;
        Ok(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::safe_member_name;

    #[test]
    fn member_names() {
        for ok in ["steps.json", "step-0001.webp", "assets/ab.webp", "dir/"] {
            assert!(safe_member_name(ok), "{ok}");
        }
        for bad in [
            "",
            "/etc/shadow",
            "../../etc/passwd",
            "a/../b",
            "..\\evil.webp",
            "C:evil",
            "a//b",
            "./x",
            "nul\0byte",
        ] {
            assert!(!safe_member_name(bad), "{bad:?}");
        }
    }
}
