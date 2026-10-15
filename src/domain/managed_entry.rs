use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub struct ManagedEntry {
    source: ManagedSource,
    target: ManagedTarget,
    kind: EntryKind,
}

impl ManagedEntry {
    pub fn new(source: ManagedSource, target: ManagedTarget, kind: EntryKind) -> Self {
        Self {
            source,
            target,
            kind,
        }
    }

    pub fn kind(&self) -> &EntryKind {
        &self.kind
    }

    pub fn target(&self) -> &ManagedTarget {
        &self.target
    }

    pub fn source(&self) -> &ManagedSource {
        &self.source
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub enum EntryKind {
    Dir,
    File,
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub enum ManagedTarget {
    HomeRelative(PathBuf),
}

impl ManagedTarget {
    pub fn home_relative(path: PathBuf) -> Result<Self, ManagedTargetError> {
        if path.is_empty() || path == Path::new(".") {
            return Err(ManagedTargetError::Empty);
        }
        if path.is_absolute() {
            return Err(ManagedTargetError::AbsolutePath(path));
        }
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(ManagedTargetError::ParentTraversal(path));
        }

        Ok(Self::HomeRelative(path))
    }

    pub fn path(&self) -> &Path {
        match self {
            Self::HomeRelative(path) => path,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedTargetError {
    #[error("home-relative target must be relative: `{0}`")]
    AbsolutePath(PathBuf),
    #[error("home-relative target must not contain parent traversal: `{0}`")]
    ParentTraversal(PathBuf),
    #[error("home-relative target must not be empty")]
    Empty,
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub struct ManagedSource(PathBuf);

impl ManagedSource {
    pub fn new(path: PathBuf) -> Result<Self, ManagedSourceError> {
        if path.is_empty() || path == Path::new(".") {
            return Err(ManagedSourceError::Empty);
        }
        if path.is_absolute() {
            return Err(ManagedSourceError::AbsolutePath(path));
        }
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return Err(ManagedSourceError::ParentTraversal(path));
        }

        Ok(Self(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedSourceError {
    #[error("home-relative source must be relative: `{0}`")]
    AbsolutePath(PathBuf),
    #[error("home-relative source must not contain parent traversal: `{0}`")]
    ParentTraversal(PathBuf),
    #[error("home-relative source must not be empty")]
    Empty,
}

#[cfg(test)]
#[path = "tests/managed_entry.rs"]
mod tests;
