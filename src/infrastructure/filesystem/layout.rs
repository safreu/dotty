use std::path::{Path, PathBuf};

use crate::{
    application::Plan,
    infrastructure::{
        filesystem::{self, FileSystemAction, FileSystemError, PathKind},
        persistence::paths::DottyPaths,
    },
};

pub struct DottyLayout<'a> {
    paths: &'a DottyPaths,
}

impl<'a> DottyLayout<'a> {
    pub fn new(paths: &'a DottyPaths) -> Self {
        Self { paths }
    }

    pub fn is_initialized(&self) -> Result<bool, DottyLayoutError> {
        let config = filesystem::path_kind(&self.paths.config_file())?;
        let backups = filesystem::path_kind(&self.paths.backup_file())?;

        Ok(matches!(config, Some(PathKind::File)) && matches!(backups, Some(PathKind::File)))
    }

    pub fn require_initialized(&self) -> Result<(), DottyLayoutError> {
        if self.is_initialized()? {
            Ok(())
        } else {
            Err(DottyLayoutError::NotInitialized)
        }
    }

    pub fn initialization_plan(&self) -> Result<Plan, DottyLayoutError> {
        let mut plan = Plan::new();

        self.plan_directory(&mut plan, self.paths.config_dir())?;
        self.plan_directory(&mut plan, self.paths.storage_dir())?;
        self.plan_directory(&mut plan, &self.paths.backup_dir())?;

        Ok(plan)
    }

    fn plan_directory(&self, plan: &mut Plan, path: &Path) -> Result<(), DottyLayoutError> {
        match filesystem::path_kind(path)? {
            None => plan.push(FileSystemAction::CreateDirectory(path.to_path_buf()).into()),
            Some(PathKind::Directory) => {}
            Some(_) => return Err(DottyLayoutError::InvalidDirectory(path.to_path_buf())),
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyLayoutError {
    #[error("Dotty is not initialized. Run `dotty init` first")]
    NotInitialized,
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("expected directory at `{0}`")]
    InvalidDirectory(PathBuf),
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;
