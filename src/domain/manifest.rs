use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{EntryKind, ManagedEntry};

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
        if let Some(existing) = self.manages.iter().find(|existing| {
            Self::paths_conflict(
                existing.target().path(),
                existing.kind(),
                entry.target().path(),
                entry.kind(),
            )
        }) {
            return Err(ManifestRootError::TargetConflict {
                existing: existing.target().path().to_path_buf(),
                new: entry.target().path().to_path_buf(),
            });
        }

        if let Some(existing) = self.manages.iter().find(|existing| {
            Self::paths_conflict(
                existing.source().path(),
                existing.kind(),
                entry.source().path(),
                entry.kind(),
            )
        }) {
            return Err(ManifestRootError::SourceConflict {
                existing: existing.source().path().to_path_buf(),
                new: entry.source().path().to_path_buf(),
            });
        }

        self.manages.insert(entry);

        Ok(())
    }

    fn paths_conflict(
        existing: &Path,
        existing_kind: &EntryKind,
        new: &Path,
        new_kind: &EntryKind,
    ) -> bool {
        existing == new
            || (matches!(existing_kind, EntryKind::Dir) && new.starts_with(existing))
            || (matches!(new_kind, EntryKind::Dir) && existing.starts_with(new))
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
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use crate::{
        domain::{EntryKind, ManagedEntry, ManagedSource, ManagedTarget},
        infrastructure::persistence::{ManifestRepository, ManifestRepositoryError},
    };

    use super::{ManifestRoot, ManifestRootError};

    fn entry(source: &str, target: &str, kind: EntryKind) -> ManagedEntry {
        ManagedEntry::new(
            ManagedSource::new(source.into()).unwrap(),
            ManagedTarget::home_relative(target.into()).unwrap(),
            kind,
        )
    }

    #[test]
    fn new_manifest_is_empty() {
        let manifest = ManifestRoot::new();

        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn add_inserts_managed_entry() {
        let mut manifest = ManifestRoot::new();
        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(managed.clone()).unwrap();

        assert_eq!(manifest.manages().len(), 1);
        assert!(manifest.manages().contains(&managed));
    }

    #[test]
    fn add_allows_independent_files_in_same_directory() {
        let mut manifest = ManifestRoot::new();

        let config = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        let themes = entry(
            "ghostty/themes.toml",
            ".config/ghostty/themes.toml",
            EntryKind::File,
        );

        manifest.add(config.clone()).unwrap();
        manifest.add(themes.clone()).unwrap();

        assert_eq!(manifest.manages().len(), 2);
        assert!(manifest.manages().contains(&config));
        assert!(manifest.manages().contains(&themes));
    }

    #[test]
    fn add_rejects_duplicate_target() {
        let mut manifest = ManifestRoot::new();

        let existing = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        let duplicate = entry("other/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(existing.clone()).unwrap();

        let result = manifest.add(duplicate);

        assert!(matches!(
            result,
            Err(ManifestRootError::TargetConflict {
                existing,
                new,
            }) if existing == *".config/ghostty/config"
                && new == *".config/ghostty/config"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_rejects_duplicate_source() {
        let mut manifest = ManifestRoot::new();

        let existing = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        let duplicate = entry("ghostty/config", ".config/other/config", EntryKind::File);

        manifest.add(existing).unwrap();

        let result = manifest.add(duplicate);

        assert!(matches!(
            result,
            Err(ManifestRootError::SourceConflict {
                existing,
                new,
            }) if existing == *"ghostty/config"
                && new == *"ghostty/config"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_rejects_existing_directory_containing_new_target() {
        let mut manifest = ManifestRoot::new();

        manifest
            .add(entry("ghostty", ".config/ghostty", EntryKind::Dir))
            .unwrap();

        let result = manifest.add(entry(
            "other/config",
            ".config/ghostty/config",
            EntryKind::File,
        ));

        assert!(matches!(
            result,
            Err(ManifestRootError::TargetConflict {
                existing,
                new,
            }) if existing == *".config/ghostty"
                && new == *".config/ghostty/config"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_rejects_new_directory_containing_existing_target() {
        let mut manifest = ManifestRoot::new();

        manifest
            .add(entry(
                "other/config",
                ".config/ghostty/config",
                EntryKind::File,
            ))
            .unwrap();

        let result = manifest.add(entry("ghostty", ".config/ghostty", EntryKind::Dir));

        assert!(matches!(
            result,
            Err(ManifestRootError::TargetConflict {
                existing,
                new,
            }) if existing == *".config/ghostty/config"
                && new == *".config/ghostty"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_rejects_existing_directory_containing_new_source() {
        let mut manifest = ManifestRoot::new();

        manifest
            .add(entry("ghostty", ".config/ghostty", EntryKind::Dir))
            .unwrap();

        let result = manifest.add(entry(
            "ghostty/config",
            ".config/other/config",
            EntryKind::File,
        ));

        assert!(matches!(
            result,
            Err(ManifestRootError::SourceConflict {
                existing,
                new,
            }) if existing == *"ghostty"
                && new == *"ghostty/config"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_rejects_new_directory_containing_existing_source() {
        let mut manifest = ManifestRoot::new();

        manifest
            .add(entry(
                "ghostty/config",
                ".config/other/config",
                EntryKind::File,
            ))
            .unwrap();

        let result = manifest.add(entry("ghostty", ".config/ghostty", EntryKind::Dir));

        assert!(matches!(
            result,
            Err(ManifestRootError::SourceConflict {
                existing,
                new,
            }) if existing == *"ghostty/config"
                && new == *"ghostty"
        ));

        assert_eq!(manifest.manages().len(), 1);
    }

    #[test]
    fn add_allows_similar_names_without_path_overlap() {
        let mut manifest = ManifestRoot::new();

        let ghostty = entry("ghostty", ".config/ghostty", EntryKind::Dir);

        let ghostty_extra = entry("ghostty-extra", ".config/ghostty-extra", EntryKind::Dir);

        manifest.add(ghostty.clone()).unwrap();
        manifest.add(ghostty_extra.clone()).unwrap();

        assert_eq!(manifest.manages().len(), 2);
        assert!(manifest.manages().contains(&ghostty));
        assert!(manifest.manages().contains(&ghostty_extra));
    }

    #[test]
    fn add_allows_files_with_common_path_prefix() {
        let mut manifest = ManifestRoot::new();

        let first = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        let second = entry(
            "ghostty/config.backup",
            ".config/ghostty/config.backup",
            EntryKind::File,
        );

        manifest.add(first).unwrap();
        manifest.add(second).unwrap();

        assert_eq!(manifest.manages().len(), 2);
    }

    #[test]
    fn add_rejects_exact_duplicate_entry() {
        let mut manifest = ManifestRoot::new();

        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(managed.clone()).unwrap();

        let result = manifest.add(managed.clone());

        assert!(matches!(
            result,
            Err(ManifestRootError::TargetConflict { .. })
        ));

        assert_eq!(manifest.manages().len(), 1);
        assert!(manifest.manages().contains(&managed));
    }

    #[test]
    fn rejected_add_does_not_modify_manifest() {
        let mut manifest = ManifestRoot::new();

        let existing = entry("ghostty", ".config/ghostty", EntryKind::Dir);

        let conflicting = entry("ghostty/config", ".config/other/config", EntryKind::File);

        manifest.add(existing.clone()).unwrap();

        assert!(manifest.add(conflicting.clone()).is_err());

        assert_eq!(manifest.manages().len(), 1);
        assert!(manifest.manages().contains(&existing));
        assert!(!manifest.manages().contains(&conflicting));
    }

    #[test]
    fn remove_existing_entry_returns_true() {
        let mut manifest = ManifestRoot::new();

        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(managed.clone()).unwrap();

        let removed = manifest.remove(&managed);

        assert!(removed);
        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn remove_unknown_entry_returns_false() {
        let mut manifest = ManifestRoot::new();

        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        let removed = manifest.remove(&managed);

        assert!(!removed);
        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn removed_entry_can_be_added_again() {
        let mut manifest = ManifestRoot::new();

        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(managed.clone()).unwrap();
        assert!(manifest.remove(&managed));

        manifest.add(managed.clone()).unwrap();

        assert_eq!(manifest.manages().len(), 1);
        assert!(manifest.manages().contains(&managed));
    }

    #[test]
    fn validate_accepts_empty_manifest() {
        let manifest = ManifestRoot::new();

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_accepts_non_conflicting_entries() {
        let mut manifest = ManifestRoot::new();

        manifest
            .add(entry(
                "ghostty/config",
                ".config/ghostty/config",
                EntryKind::File,
            ))
            .unwrap();

        manifest
            .add(entry(
                "ghostty/themes.toml",
                ".config/ghostty/themes.toml",
                EntryKind::File,
            ))
            .unwrap();

        assert!(manifest.validate().is_ok());
    }

    #[test]
    fn validate_rejects_conflicting_targets() {
        let _toml = r#"
            [[manages]]
            source = "ghostty"
            target = { HomeRelative = ".config/ghostty" }
            kind = "Dir"
    
            [[manages]]
            source = "other/config"
            target = { HomeRelative = ".config/ghostty/config" }
            kind = "File"
        "#;

        let mut manifest = ManifestRoot::new();

        let first = entry("ghostty", ".config/ghostty", EntryKind::Dir);

        let second = entry("other/config", ".config/ghostty/config", EntryKind::File);

        manifest.manages.insert(first);
        manifest.manages.insert(second);

        let result = manifest.validate();

        assert!(matches!(
            result,
            Err(ManifestRootError::TargetConflict { .. })
        ));
    }

    #[test]
    fn validate_rejects_conflicting_sources() {
        let mut manifest = ManifestRoot::new();

        let first = entry("ghostty", ".config/ghostty", EntryKind::Dir);

        let second = entry("ghostty/config", ".config/other/config", EntryKind::File);

        manifest.manages.insert(first);
        manifest.manages.insert(second);

        let result = manifest.validate();

        assert!(matches!(
            result,
            Err(ManifestRootError::SourceConflict { .. })
        ));
    }

    #[test]
    fn validate_does_not_modify_manifest() {
        let mut manifest = ManifestRoot::new();

        let first = entry("ghostty", ".config/ghostty", EntryKind::Dir);

        let second = entry("ghostty/config", ".config/other/config", EntryKind::File);

        manifest.manages.insert(first);
        manifest.manages.insert(second);

        let before = manifest.manages().clone();

        assert!(manifest.validate().is_err());

        assert_eq!(manifest.manages(), &before);
    }

    #[test]
    fn read_accepts_valid_manifest() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest.toml");

        let mut manifest = ManifestRoot::new();

        let managed = entry("ghostty/config", ".config/ghostty/config", EntryKind::File);

        manifest.add(managed.clone()).unwrap();

        let repository = ManifestRepository::new(&path);

        repository.write(&manifest).unwrap();

        let loaded = repository.read().unwrap();

        assert_eq!(loaded.manages().len(), 1);
        assert!(loaded.manages().contains(&managed));
    }

    #[test]
    fn read_accepts_empty_manifest() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest.toml");

        let repository = ManifestRepository::new(&path);

        repository.write(&ManifestRoot::new()).unwrap();

        let loaded = repository.read().unwrap();

        assert!(loaded.manages().is_empty());
    }

    #[test]
    fn read_rejects_conflicting_targets() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest.toml");

        let contents = r#"
                [[manages]]
                source = "ghostty"
                target = { HomeRelative = ".config/ghostty" }
                kind = "Dir"
    
                [[manages]]
                source = "other/config"
                target = { HomeRelative = ".config/ghostty/config" }
                kind = "File"
            "#;

        fs::write(&path, contents).unwrap();

        let repository = ManifestRepository::new(&path);

        let result = repository.read();

        assert!(matches!(
            result,
            Err(ManifestRepositoryError::InvalidManifest(
                crate::domain::ManifestRootError::TargetConflict { .. }
            ))
        ));
    }

    #[test]
    fn read_rejects_conflicting_sources() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest.toml");

        let contents = r#"
                [[manages]]
                source = "ghostty"
                target = { HomeRelative = ".config/ghostty" }
                kind = "Dir"
    
                [[manages]]
                source = "ghostty/config"
                target = { HomeRelative = ".config/other/config" }
                kind = "File"
            "#;

        fs::write(&path, contents).unwrap();

        let repository = ManifestRepository::new(&path);

        let result = repository.read();

        assert!(matches!(
            result,
            Err(ManifestRepositoryError::InvalidManifest(
                crate::domain::ManifestRootError::SourceConflict { .. }
            ))
        ));
    }

    #[test]
    fn read_reports_invalid_toml() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("manifest.toml");

        fs::write(&path, "this is not valid toml = [").unwrap();

        let repository = ManifestRepository::new(&path);

        let result = repository.read();

        assert!(matches!(result, Err(ManifestRepositoryError::Read(_))));
    }
}
