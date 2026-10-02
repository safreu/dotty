use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    config::EntryKind,
    handlers::{
        storage_handler::StorageHandler,
        toml_handler::{self, TomlError},
    },
};

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupRoot {
    config: BackupConfig,
    backups: HashMap<PathBuf, BackupEntry>,
}

impl BackupRoot {
    pub fn read(paths: &StorageHandler) -> Result<Self, BackupFileError> {
        toml_handler::read_toml(paths.backup_dir()).map_err(BackupFileError::Read)
    }

    pub fn write(&self) -> Result<(), BackupFileError> {
        toml_handler::write_toml(self, &self.config.backup_file).map_err(BackupFileError::Write)
    }

    pub fn backups_mut(&mut self) -> &mut HashMap<PathBuf, BackupEntry> {
        &mut self.backups
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    backup_file: PathBuf,
}

#[derive(Debug, thiserror::Error)]
pub enum BackupFileError {
    #[error("Failed to read backups.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist backups.toml")]
    Write(#[source] TomlError),
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
        entry_kind: EntryKind,
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
            kind: entry_kind,
            backups,
        })
    }

    pub fn backups_mut(&mut self) -> &mut HashSet<BackupKind> {
        &mut self.backups
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
