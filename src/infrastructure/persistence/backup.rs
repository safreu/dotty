use std::path::PathBuf;

use crate::{
    domain::BackupRoot,
    infrastructure::persistence::{self, TomlError},
};

#[derive(Debug)]
pub struct BackupRepository {
    path: PathBuf,
}

impl BackupRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn read(&self) -> Result<BackupRoot, BackupRepositoryError> {
        persistence::read_toml(&self.path).map_err(BackupRepositoryError::Read)
    }

    pub fn write(&self, backups: &BackupRoot) -> Result<(), BackupRepositoryError> {
        persistence::write_toml(backups, &self.path).map_err(BackupRepositoryError::Write)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackupRepositoryError {
    #[error("Failed to read backups.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist backups.toml")]
    Write(#[source] TomlError),
}
