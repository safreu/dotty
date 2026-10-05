use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::domain::EntryKind;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupRoot {
    backups: HashMap<PathBuf, BackupEntry>,
}

impl BackupRoot {
    pub fn new() -> Self {
        Self {
            backups: HashMap::new(),
        }
    }

    pub fn backups_mut(&mut self) -> &mut HashMap<PathBuf, BackupEntry> {
        &mut self.backups
    }

    #[allow(unused)]
    pub fn backups(&self) -> &HashMap<PathBuf, BackupEntry> {
        &self.backups
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct BackupEntry {
    kind: EntryKind,
    backups: HashSet<BackupKind>,
}

impl BackupEntry {
    pub fn new(entry_kind: &EntryKind, backup_kind: BackupKind) -> Self {
        let mut backups = HashSet::new();
        backups.insert(backup_kind);

        Self {
            kind: entry_kind.clone(),
            backups,
        }
    }

    pub fn backups_mut(&mut self) -> &mut HashSet<BackupKind> {
        &mut self.backups
    }

    #[allow(unused)]
    pub fn backups(&self) -> &HashSet<BackupKind> {
        &self.backups
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Hash)]
pub enum BackupKind {
    Link,
    Unlink,
}
