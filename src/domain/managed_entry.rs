use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub struct ManagedEntry {
    filename: String,
    managed_filename: String,
    stored_at: PathBuf,
    points_to: PathBuf,
    kind: EntryKind,
}

impl ManagedEntry {
    pub fn new(
        target: &Path,
        dotfiles_dir: &Path,
        kind: &EntryKind,
    ) -> Result<Self, ManagedEntryError> {
        let filename = target
            .file_name()
            .ok_or_else(|| ManagedEntryError::FilenameRequired(target.to_path_buf()))?
            .to_string_lossy()
            .into_owned();

        let managed_filename = filename.strip_prefix(".").unwrap_or(&filename).to_owned();

        let stored_at = dotfiles_dir.join(&managed_filename);

        Ok(Self {
            filename,
            managed_filename,
            stored_at,
            points_to: target.to_path_buf(),
            kind: kind.clone(),
        })
    }

    pub fn points_to(&self) -> &PathBuf {
        &self.points_to
    }

    pub fn managed_filename(&self) -> &str {
        &self.managed_filename
    }

    pub fn stored_at(&self) -> &PathBuf {
        &self.stored_at
    }

    pub fn kind(&self) -> &EntryKind {
        &self.kind
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedEntryError {
    #[error("Target file doesn't have a filename")]
    FilenameRequired(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub enum EntryKind {
    Dir,
    File,
}
