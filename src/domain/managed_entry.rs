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
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub struct ManagedSource(PathBuf);

impl ManagedSource {
    pub fn new(path: PathBuf) -> Result<Self, ManagedSourceError> {
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
    #[error("home-relative target must be relative: `{0}`")]
    AbsolutePath(PathBuf),
    #[error("home-relative target must not contain parent traversal: `{0}`")]
    ParentTraversal(PathBuf),
}

#[cfg(test)]
mod tests {
    use crate::domain::managed_entry::{ManagedTarget, ManagedTargetError};
    use std::path::PathBuf;

    #[test]
    fn home_relative_accepts_relative_path() {
        let path = PathBuf::from(".config/ghostty/config");

        let target = ManagedTarget::home_relative(path.clone()).unwrap();

        assert_eq!(target, ManagedTarget::HomeRelative(path));
    }

    #[test]
    fn home_relative_rejects_absolute_path() {
        let path = PathBuf::from("/etc/example");

        let result = ManagedTarget::home_relative(path.clone());

        assert!(matches!(
            result,
            Err(ManagedTargetError::AbsolutePath(error_path))
                if error_path == path
        ));
    }
}
