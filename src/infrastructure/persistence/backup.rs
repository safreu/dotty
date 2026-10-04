use std::path::PathBuf;

use crate::{
    domain::BackupRoot,
    infrastructure::persistence::{self, PreparedWrite, TomlError},
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

    #[allow(unused)]
    pub fn write(&self, backups: &BackupRoot) -> Result<(), BackupRepositoryError> {
        persistence::write_toml(backups, &self.path).map_err(BackupRepositoryError::Write)
    }

    pub fn prepare_write(
        &self,
        backups: &BackupRoot,
    ) -> Result<PreparedWrite, BackupRepositoryError> {
        let contents =
            persistence::serialize_toml(backups).map_err(BackupRepositoryError::Write)?;

        Ok(PreparedWrite::new(self.path.clone(), contents))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackupRepositoryError {
    #[error("Failed to read backups.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist backups.toml")]
    Write(#[source] TomlError),
}
