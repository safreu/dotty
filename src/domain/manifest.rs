use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::ManagedEntry;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestRoot {
    manages: HashSet<ManagedEntry>,
}

impl ManifestRoot {
    pub fn new() -> Self {
        Self {
            manages: HashSet::new(),
        }
    }

    pub fn manages(&self) -> &HashSet<ManagedEntry> {
        &self.manages
    }

    pub fn add(&mut self, entry: ManagedEntry) -> Result<(), ManifestRootError> {
        if let Some(existing) = self
            .manages
            .iter()
            .find(|existing| Self::paths_conflict(existing.target().path(), entry.target().path()))
        {
            return Err(ManifestRootError::TargetConflict {
                existing: existing.target().path().to_path_buf(),
                new: entry.target().path().to_path_buf(),
            });
        }

        if let Some(existing) = self
            .manages
            .iter()
            .find(|existing| Self::paths_conflict(existing.source().path(), entry.source().path()))
        {
            return Err(ManifestRootError::SourceConflict {
                existing: existing.source().path().to_path_buf(),
                new: entry.source().path().to_path_buf(),
            });
        }

        self.manages.insert(entry);

        Ok(())
    }

    fn paths_conflict(existing: &Path, new: &Path) -> bool {
        existing.starts_with(new) || new.starts_with(existing)
    }

    pub fn remove(&mut self, entry: &ManagedEntry) -> bool {
        self.manages.remove(entry)
    }

    pub fn validate(&self) -> Result<(), ManifestRootError> {
        let mut validated = ManifestRoot::new();

        for entry in self.manages() {
            validated.add(entry.clone())?;
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestRootError {
    #[error("managed target `{new}` conflicts with existing target `{existing}`")]
    TargetConflict { existing: PathBuf, new: PathBuf },
    #[error("managed source `{new}` conflicts with existing source `{existing}`")]
    SourceConflict { existing: PathBuf, new: PathBuf },
}

#[cfg(test)]
#[path = "tests/manifest.rs"]
mod tests;
