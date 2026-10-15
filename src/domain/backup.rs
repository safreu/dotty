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
#[path = "tests/backup.rs"]
mod tests;
