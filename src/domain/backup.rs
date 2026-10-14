use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use uuid::Uuid;

use crate::domain::{EntryKind, ManagedSource, ManagedTarget};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupRoot {
    backups: HashMap<BackupId, BackupEntry>,
}

impl BackupRoot {
    pub fn new() -> Self {
        Self {
            backups: HashMap::new(),
        }
    }

    pub fn add(&mut self, id: BackupId, entry: BackupEntry) -> Result<(), BackupRootError> {
        match self.backups.entry(id) {
            Entry::Vacant(slot) => {
                slot.insert(entry);
                Ok(())
            }
            Entry::Occupied(_) => Err(BackupRootError::DuplicateId(id)),
        }
    }

    #[allow(unused)]
    pub fn backups(&self) -> &HashMap<BackupId, BackupEntry> {
        &self.backups
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackupRootError {
    #[error("backup with ID {0:?} already exists")]
    DuplicateId(BackupId),
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct BackupEntry {
    source: ManagedSource,
    target: ManagedTarget,
    entry_kind: EntryKind,
    backup_kind: BackupKind,
}

impl BackupEntry {
    pub fn new(
        source: ManagedSource,
        target: ManagedTarget,
        entry_kind: EntryKind,
        backup_kind: BackupKind,
    ) -> Self {
        Self {
            source,
            target,
            entry_kind,
            backup_kind,
        }
    }

    #[allow(unused)]
    pub fn source(&self) -> &ManagedSource {
        &self.source
    }

    #[allow(unused)]
    pub fn target(&self) -> &ManagedTarget {
        &self.target
    }

    #[allow(unused)]
    pub fn entry_kind(&self) -> &EntryKind {
        &self.entry_kind
    }

    #[allow(unused)]
    pub fn backup_kind(&self) -> &BackupKind {
        &self.backup_kind
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Hash)]
pub enum BackupKind {
    Link,
    Unlink,
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Hash, Copy)]
#[serde(transparent)]
pub struct BackupId(Uuid);

impl BackupId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl std::fmt::Display for BackupId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn backup_entry(source: &str, target: &str, kind: BackupKind) -> BackupEntry {
        BackupEntry::new(
            ManagedSource::new(PathBuf::from(source)).unwrap(),
            ManagedTarget::home_relative(PathBuf::from(target)).unwrap(),
            EntryKind::File,
            kind,
        )
    }

    #[test]
    fn backup_id_is_unique() {
        let first = BackupId::new();
        let second = BackupId::new();

        assert_ne!(first, second);
    }

    #[test]
    fn backup_entry_preserves_operation_metadata() {
        let source = ManagedSource::new(PathBuf::from("ghostty/config")).unwrap();
        let target = ManagedTarget::home_relative(PathBuf::from(".config/ghostty/config")).unwrap();

        let entry = BackupEntry::new(
            source.clone(),
            target.clone(),
            EntryKind::File,
            BackupKind::Link,
        );

        assert_eq!(entry.source(), &source);
        assert_eq!(entry.target(), &target);
        assert_eq!(entry.entry_kind(), &EntryKind::File);
        assert_eq!(entry.backup_kind(), &BackupKind::Link);
    }

    #[test]
    fn backup_root_accepts_new_backup() {
        let mut root = BackupRoot::new();
        let id = BackupId::new();

        let entry = backup_entry("zshrc", ".zshrc", BackupKind::Link);

        root.add(id, entry.clone()).unwrap();

        assert_eq!(root.backups().len(), 1);
        assert_eq!(root.backups().get(&id), Some(&entry));
    }

    #[test]
    fn backup_root_rejects_duplicate_backup_id() {
        let mut root = BackupRoot::new();
        let id = BackupId::new();

        let first = backup_entry("zshrc", ".zshrc", BackupKind::Link);
        let second = backup_entry(
            "ghostty/config",
            ".config/ghostty/config",
            BackupKind::Unlink,
        );

        root.add(id, first.clone()).unwrap();

        let result = root.add(id, second);

        assert!(matches!(
            result,
            Err(BackupRootError::DuplicateId(duplicate)) if duplicate == id
        ));

        assert_eq!(root.backups().len(), 1);
        assert_eq!(root.backups().get(&id), Some(&first));
    }

    #[test]
    fn backup_root_accepts_multiple_backups_for_same_source() {
        let mut root = BackupRoot::new();

        let first_id = BackupId::new();
        let second_id = BackupId::new();

        let first = backup_entry("zshrc", ".zshrc", BackupKind::Link);
        let second = backup_entry("zshrc", ".zshrc", BackupKind::Unlink);

        root.add(first_id, first.clone()).unwrap();
        root.add(second_id, second.clone()).unwrap();

        assert_eq!(root.backups().len(), 2);
        assert_eq!(root.backups().get(&first_id), Some(&first));
        assert_eq!(root.backups().get(&second_id), Some(&second));
    }

    #[test]
    fn backup_root_accepts_same_source_with_different_targets() {
        let mut root = BackupRoot::new();

        let first_id = BackupId::new();
        let second_id = BackupId::new();

        let first = backup_entry("shared/config", ".config/app-a/config", BackupKind::Link);
        let second = backup_entry("shared/config", ".config/app-b/config", BackupKind::Link);

        root.add(first_id, first.clone()).unwrap();
        root.add(second_id, second.clone()).unwrap();

        assert_eq!(root.backups().len(), 2);
        assert_eq!(root.backups().get(&first_id), Some(&first));
        assert_eq!(root.backups().get(&second_id), Some(&second));
    }
}
