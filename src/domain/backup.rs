use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
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
    original_path: PathBuf,
    kind: EntryKind,
    backups: HashSet<BackupKind>,
}

impl BackupEntry {
    pub fn new(target: &Path, entry_kind: &EntryKind, backup_kind: BackupKind) -> Self {
        let mut backups = HashSet::new();
        backups.insert(backup_kind);

        Self {
            original_path: target.to_path_buf(),
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
