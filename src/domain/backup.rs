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
    backup_dir: PathBuf,
    kind: EntryKind,
    backups: HashSet<BackupKind>,
}

impl BackupEntry {
    pub fn backup_dir(&self) -> &PathBuf {
        &self.backup_dir
    }

    pub fn new(
        target: &Path,
        backup_dir: &Path,
        entry_kind: &EntryKind,
        backup_kind: BackupKind,
    ) -> Result<Self, BackupEntryError> {
        let filename = target
            .file_name()
            .ok_or_else(|| BackupEntryError::FilenameRequired(target.to_path_buf()))?
            .to_string_lossy()
            .into_owned();

        let managed_filename = filename.strip_prefix(".").unwrap_or(&filename).to_owned();

        let stored_dir = backup_dir.join(managed_filename);

        let mut backups = HashSet::new();
        backups.insert(backup_kind);
        Ok(Self {
            original_path: target.to_path_buf(),
            backup_dir: stored_dir,
            kind: entry_kind.clone(),
            backups,
        })
    }

    pub fn backups_mut(&mut self) -> &mut HashSet<BackupKind> {
        &mut self.backups
    }

    #[allow(unused)]
    pub fn backups(&self) -> &HashSet<BackupKind> {
        &self.backups
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackupEntryError {
    #[error("Target file doesn't have a filename")]
    FilenameRequired(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone, Hash)]
pub enum BackupKind {
    Link,
    Unlink,
}
