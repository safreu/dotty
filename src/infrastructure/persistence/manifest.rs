use std::path::{Path, PathBuf};

use crate::{
    domain::ManifestRoot,
    infrastructure::persistence::{self, PreparedWrite, TomlError},
};

pub struct ManifestRepository {
    path: PathBuf,
}

impl ManifestRepository {
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
        }
    }

    pub fn read(&self) -> Result<ManifestRoot, ManifestRepositoryError> {
        persistence::read_toml(&self.path).map_err(ManifestRepositoryError::Read)
    }

    #[allow(unused)]
    pub fn write(&self, manifest: &ManifestRoot) -> Result<(), ManifestRepositoryError> {
        persistence::write_toml(manifest, &self.path).map_err(ManifestRepositoryError::Write)
    }

    pub fn prepare_write(
        &self,
        manifest: &ManifestRoot,
    ) -> Result<PreparedWrite, ManifestRepositoryError> {
        let contents =
            persistence::serialize_toml(manifest).map_err(ManifestRepositoryError::Write)?;

        Ok(PreparedWrite::new(self.path.clone(), contents))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestRepositoryError {
    #[error("Failed to read manifest.toml")]
    Read(#[source] TomlError),
    #[error("Failed to persist manifest.toml")]
    Write(#[source] TomlError),
}
