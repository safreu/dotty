use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::{ManagedSource, ManagedTarget, ManagedTargetError};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigRoot {
    repo_dir: PathBuf,
    home_dir: PathBuf,
}

impl ConfigRoot {
    pub fn new(repo_dir: PathBuf, home_dir: PathBuf) -> Self {
        Self { repo_dir, home_dir }
    }

    pub fn repo_dir(&self) -> &PathBuf {
        &self.repo_dir
    }

    pub fn home_dir(&self) -> &PathBuf {
        &self.home_dir
    }

    pub fn resolve_source(&self, source: &ManagedSource) -> PathBuf {
        self.repo_dir.join(source.path())
    }

    pub fn resolve_target(&self, target: &ManagedTarget) -> PathBuf {
        self.home_dir.join(target.path())
    }

    pub fn to_managed_target(&self, target: &Path) -> Result<ManagedTarget, ConfigRootError> {
        let relative = target
            .strip_prefix(self.home_dir())
            .map_err(|_| ConfigRootError::TargetOutsideHome(target.to_path_buf()))?;

        Ok(ManagedTarget::home_relative(relative.to_path_buf())?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigRootError {
    #[error("managed target is outside the configured home directory: {0}")]
    TargetOutsideHome(PathBuf),

    #[error(transparent)]
    InvalidManagedTarget(#[from] ManagedTargetError),
}
