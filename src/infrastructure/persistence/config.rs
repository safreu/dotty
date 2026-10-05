use std::path::PathBuf;

use crate::{
    domain::ConfigRoot,
    infrastructure::persistence::{self, PreparedWrite, TomlError},
};

pub struct ConfigRepository {
    path: PathBuf,
}

impl ConfigRepository {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn read(&self) -> Result<ConfigRoot, ConfigRepositoryError> {
        persistence::read_toml(&self.path).map_err(ConfigRepositoryError::Read)
    }

    #[allow(unused)]
    pub fn write(&self, config: &ConfigRoot) -> Result<(), ConfigRepositoryError> {
        persistence::write_toml(config, &self.path).map_err(ConfigRepositoryError::Write)
    }

    pub fn prepare_write(
        &self,
        config: &ConfigRoot,
    ) -> Result<PreparedWrite, ConfigRepositoryError> {
        let contents = persistence::serialize_toml(config).map_err(ConfigRepositoryError::Write)?;

        Ok(PreparedWrite::new(self.path.clone(), contents))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigRepositoryError {
    #[error("Failed to read config.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist config.toml")]
    Write(#[source] TomlError),
}
