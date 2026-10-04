use std::path::Path;

use crate::infrastructure::{
    filesystem::{self, FileSystemError},
    persistence::paths::DottyPaths,
};

pub struct DottyLayout<'a> {
    paths: &'a DottyPaths,
}

impl<'a> DottyLayout<'a> {
    pub fn new(paths: &'a DottyPaths) -> Self {
        Self { paths }
    }

    pub fn is_initialized(&self) -> bool {
        self.paths.config_file().exists() && self.paths.backup_file().exists()
    }

    pub fn require_initialized(&self) -> Result<(), DottyLayoutError> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(DottyLayoutError::NotInitialized)
        }
    }

    pub fn initialize(&self, repo_dir: &Path) -> Result<(), DottyLayoutError> {
        filesystem::create_dir(self.paths.config_dir())?;
        filesystem::create_dir(self.paths.storage_dir())?;
        filesystem::create_dir(&self.paths.backup_dir())?;
        filesystem::create_dir(repo_dir)?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyLayoutError {
    #[error("Dotty is not initialized. Run `dotty init` first")]
    NotInitialized,
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}
