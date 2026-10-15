use std::path::{Path, PathBuf};

use crate::{
    application::Plan,
    domain::ManagedSource,
    infrastructure::{
        filesystem::{self, FileSystemAction, FileSystemError, PathKind},
        persistence::RepositoryPaths,
    },
};

pub struct RepositoryLayout<'a> {
    paths: &'a RepositoryPaths,
}

impl<'a> RepositoryLayout<'a> {
    pub fn new(paths: &'a RepositoryPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &'a RepositoryPaths {
        self.paths
    }

    pub fn initialization_plan(&self) -> Result<Plan, RepositoryLayoutError> {
        let mut plan = Plan::new();

        self.plan_directory(&mut plan, self.paths().repo_dir())?;
        self.plan_directory(&mut plan, &self.paths().metadata_dir())?;

        Ok(plan)
    }

    fn plan_directory(&self, plan: &mut Plan, path: &Path) -> Result<(), RepositoryLayoutError> {
        match filesystem::path_kind(path)? {
            None => plan.push(FileSystemAction::CreateDirectory(path.to_path_buf()).into()),
            Some(PathKind::Directory) => {}
            Some(_) => return Err(RepositoryLayoutError::InvalidDirectory(path.to_path_buf())),
        }

        Ok(())
    }

    pub fn managed_source_plan(
        &self,
        source: &ManagedSource,
    ) -> Result<Plan, RepositoryLayoutError> {
        let source = self.paths.repo_dir().join(source.path());

        let parent = source
            .parent()
            .ok_or_else(|| RepositoryLayoutError::MissingParent(source.clone()))?;

        let missing = filesystem::missing_directories(self.paths().repo_dir(), parent)?;

        let mut plan = Plan::new();

        for directory in missing {
            plan.push(FileSystemAction::CreateDirectory(directory).into());
        }

        Ok(plan)
    }

    pub fn managed_source_cleanup_plan(
        &self,
        source: &ManagedSource,
    ) -> Result<Plan, RepositoryLayoutError> {
        let source = self.paths.repo_dir().join(source.path());

        let parent = source
            .parent()
            .ok_or_else(|| RepositoryLayoutError::MissingParent(source.clone()))?;

        if !source.starts_with(self.paths.repo_dir()) {
            return Err(RepositoryLayoutError::FileSystem(
                FileSystemError::PathOutsideRoot {
                    root: self.paths.repo_dir().to_path_buf(),
                    target: source,
                },
            ));
        }

        let mut plan = Plan::new();

        for directory in parent
            .ancestors()
            .take_while(|path| *path != self.paths().repo_dir())
        {
            plan.push(FileSystemAction::RemoveDirectoryIfEmpty(directory.to_path_buf()).into());
        }

        Ok(plan)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryLayoutError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("expected directory at `{0}`")]
    InvalidDirectory(PathBuf),
    #[error("Missing parent for directory: `{0}`")]
    MissingParent(PathBuf),
}

#[cfg(test)]
#[path = "tests/repository_layout.rs"]
mod tests;
