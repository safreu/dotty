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
mod tests {
    use crate::domain::{
        ManagedSource, ManagedSourceError,
        managed_entry::{ManagedTarget, ManagedTargetError},
    };
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

    #[test]
    fn home_relative_rejects_empty_path() {
        let result = ManagedTarget::home_relative(PathBuf::new());

        assert!(matches!(result, Err(ManagedTargetError::Empty)));
    }

    #[test]
    fn home_relative_rejects_current_directory() {
        let result = ManagedTarget::home_relative(PathBuf::from("."));

        assert!(matches!(result, Err(ManagedTargetError::Empty)));
    }

    #[test]
    fn home_relative_rejects_parent_traversal() {
        let path = PathBuf::from("../secret");

        let result = ManagedTarget::home_relative(path.clone());

        assert!(matches!(
            result,
            Err(ManagedTargetError::ParentTraversal(error_path))
                if error_path == path
        ));
    }

    #[test]
    fn home_relative_rejects_nested_parent_traversal() {
        let path = PathBuf::from("ghostty/../../secret");

        let result = ManagedTarget::home_relative(path.clone());

        assert!(matches!(
            result,
            Err(ManagedTargetError::ParentTraversal(error_path))
                if error_path == path
        ));
    }

    #[test]
    fn home_relative_accepts_nested_relative_path() {
        let path = PathBuf::from(".config/ghostty/config");

        let result = ManagedTarget::home_relative(path.clone()).unwrap();

        assert_eq!(result.path(), path.as_path());
    }

    #[test]
    fn managed_source_accepts_relative_path() {
        let path = PathBuf::from("ghostty/config");

        let result = ManagedSource::new(path.clone()).unwrap();

        assert_eq!(result.path(), path.as_path());
    }

    #[test]
    fn managed_source_rejects_empty_path() {
        let result = ManagedSource::new(PathBuf::new());

        assert!(matches!(result, Err(ManagedSourceError::Empty)));
    }

    #[test]
    fn managed_source_rejects_current_directory() {
        let result = ManagedSource::new(PathBuf::from("."));

        assert!(matches!(result, Err(ManagedSourceError::Empty)));
    }

    #[test]
    fn managed_source_rejects_absolute_path() {
        let path = PathBuf::from("/etc/example");

        let result = ManagedSource::new(path.clone());

        assert!(matches!(
            result,
            Err(ManagedSourceError::AbsolutePath(error_path))
                if error_path == path
        ));
    }

    #[test]
    fn managed_source_rejects_parent_traversal() {
        let path = PathBuf::from("../secret");

        let result = ManagedSource::new(path.clone());

        assert!(matches!(
            result,
            Err(ManagedSourceError::ParentTraversal(error_path))
                if error_path == path
        ));
    }

    #[test]
    fn managed_source_rejects_nested_parent_traversal() {
        let path = PathBuf::from("ghostty/../../secret");

        let result = ManagedSource::new(path.clone());

        assert!(matches!(
            result,
            Err(ManagedSourceError::ParentTraversal(error_path))
                if error_path == path
        ));
    }
}
