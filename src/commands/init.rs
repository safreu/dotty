use std::path::{Path, PathBuf};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{BackupRoot, ConfigRoot, ManifestRoot},
    infrastructure::{
        filesystem::{
            DottyLayout, DottyLayoutError, FileSystemAction, RepositoryLayout,
            RepositoryLayoutError,
        },
        persistence::{
            ManifestRepository, ManifestRepositoryError, RepositoryPaths,
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(repo_dir: PathBuf) -> Result<(), InitError> {
    let paths = DottyPaths::discover()?;
    let layout = DottyLayout::new(&paths);

    if layout.is_initialized()? {
        return Err(InitError::AlreadyInit);
    }

    let plan = build_plan(&paths, &layout, &repo_dir)?;

    PlanExecutor::execute(plan)?;

    Ok(())
}

fn build_plan(
    paths: &DottyPaths,
    layout: &DottyLayout,
    repo_dir: &Path,
) -> Result<Plan, InitError> {
    let config_repository = ConfigRepository::new(paths.config_file());
    let backups_repository = BackupRepository::new(paths.backup_file());

    let repository_paths = RepositoryPaths::new(repo_dir.to_path_buf());

    let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

    let repository_layout = RepositoryLayout::new(&repository_paths);

    let mut plan = layout.initialization_plan()?;

    let repository_plan = repository_layout.initialization_plan()?;

    plan.extend(repository_plan);

    let config = ConfigRoot::new(repo_dir.to_path_buf(), paths.home_dir().to_path_buf());
    let backups = BackupRoot::new();

    let config_write = config_repository.prepare_write(&config)?;
    let backups_write = backups_repository.prepare_write(&backups)?;

    if repository_paths.manifest_file().exists() {
        manifest_repository.read()?;
    } else {
        let manifest = ManifestRoot::new();
        let manifest_write = manifest_repository.prepare_write(&manifest)?;
        let (path, contents) = manifest_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());
    }

    let (path, contents) = config_write.into_parts();
    plan.push(FileSystemAction::WriteFile { path, contents }.into());

    let (path, contents) = backups_write.into_parts();
    plan.push(FileSystemAction::WriteFile { path, contents }.into());

    Ok(plan)
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("Dotty is already initialized")]
    AlreadyInit,
    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
    #[error(transparent)]
    PlanExecution(#[from] PlanExecutionError),
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
    #[error(transparent)]
    Layout(#[from] DottyLayoutError),
    #[error(transparent)]
    RepositoryLayout(#[from] RepositoryLayoutError),
    #[error(transparent)]
    Manifest(#[from] ManifestRepositoryError),
}

#[cfg(test)]
#[path = "tests/init.rs"]
mod tests;
