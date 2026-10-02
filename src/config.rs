use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::handlers::{
    storage_handler::StorageHandler,
    toml_handler::{self, TomlError},
};

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigRoot {
    config: Config,
    manages: HashSet<ManagedEntry>,
}

impl ConfigRoot {
    pub fn new(dotfiles_dir: PathBuf, paths: &StorageHandler) -> Self {
        Self {
            config: Config {
                dotfiles_dir,
                config_dir: paths.config_dir().to_path_buf(),
                backups_dir: paths.backup_dir().to_path_buf(),
                storage_dir: paths.storage_dir().to_path_buf(),
            },
            manages: HashSet::new(),
        }
    }

    pub fn read(paths: &StorageHandler) -> Result<Self, ConfigFileError> {
        toml_handler::read_toml(paths.config_file()).map_err(ConfigFileError::Read)
    }

    pub fn write(&self) -> Result<(), ConfigFileError> {
        toml_handler::write_toml(self, &self.config().config_file()).map_err(ConfigFileError::Write)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn manages_mut(&mut self) -> &mut HashSet<ManagedEntry> {
        &mut self.manages
    }

    pub fn manages(&self) -> &HashSet<ManagedEntry> {
        &self.manages
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigFileError {
    #[error("Failed to read config.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist config.toml")]
    Write(#[source] TomlError),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    config_dir: PathBuf,
    storage_dir: PathBuf,
    backups_dir: PathBuf,
    dotfiles_dir: PathBuf,
}

impl Config {
    pub fn dotfiles_dir(&self) -> &PathBuf {
        &self.dotfiles_dir
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn backups_dir(&self) -> &PathBuf {
        &self.backups_dir
    }
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub struct ManagedEntry {
    filename: String,
    managed_filename: String,
    stored_at: PathBuf,
    points_to: PathBuf,
    kind: EntryKind,
}

impl ManagedEntry {
    pub fn new(
        target: &Path,
        dotfiles_dir: &Path,
        kind: EntryKind,
    ) -> Result<Self, ManagedEntryError> {
        let filename = target
            .file_name()
            .ok_or_else(|| ManagedEntryError::FilenameRequired(target.to_path_buf()))?
            .to_string_lossy()
            .into_owned();

        let managed_filename = filename.strip_prefix(".").unwrap_or(&filename).to_owned();

        let stored_at = dotfiles_dir.join(&managed_filename);

        Ok(Self {
            filename,
            managed_filename,
            stored_at,
            points_to: target.to_path_buf(),
            kind,
        })
    }

    pub fn points_to(&self) -> &PathBuf {
        &self.points_to
    }

    pub fn managed_filename(&self) -> &str {
        &self.managed_filename
    }

    pub fn stored_at(&self) -> &PathBuf {
        &self.stored_at
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManagedEntryError {
    #[error("Target file doesn't have a filename")]
    FilenameRequired(PathBuf),
}

#[derive(Debug, Serialize, Deserialize, Eq, Hash, PartialEq, Clone)]
pub enum EntryKind {
    Dir,
    File,
}
