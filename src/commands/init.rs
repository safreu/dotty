use std::path::{Path, PathBuf};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{BackupRoot, ConfigRoot},
    infrastructure::{
        filesystem::{DottyLayout, FileSystemAction},
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(repo_dir: PathBuf) -> Result<(), InitError> {
    let paths = DottyPaths::discover()?;
    let layout = DottyLayout::new(&paths);

    if layout.is_initialized() {
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

    let mut plan = layout.initialization_plan(repo_dir);

    let config = ConfigRoot::new(repo_dir.to_path_buf());
    let backups = BackupRoot::new();

    let config_write = config_repository.prepare_write(&config)?;
    let backups_write = backups_repository.prepare_write(&backups)?;

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
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::filesystem::FileSystemAction;

    #[test]
    fn init_creates_config_file() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);
        let layout = DottyLayout::new(&paths);

        let plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        PlanExecutor::execute(plan).unwrap();

        assert!(paths.config_file().is_file());
    }

    #[test]
    fn init_creates_backup_file() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);
        let layout = DottyLayout::new(&paths);

        let plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        PlanExecutor::execute(plan).unwrap();

        assert!(paths.backup_file().is_file());
    }

    #[test]
    fn init_creates_required_directories() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());
        let layout = DottyLayout::new(&paths);

        let plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        PlanExecutor::execute(plan).unwrap();

        assert!(config_dir.is_dir());
        assert!(storage_dir.is_dir());
        assert!(paths.backup_dir().is_dir());
        assert!(repo_dir.is_dir());
    }

    #[test]
    fn init_persists_valid_config() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);
        let layout = DottyLayout::new(&paths);

        let plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        PlanExecutor::execute(plan).unwrap();

        let repository = ConfigRepository::new(paths.config_file());
        let config = repository.read().unwrap();

        assert_eq!(config.config().dotfiles_dir(), &repo_dir,);
    }

    #[test]
    fn init_persists_valid_backup_root() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);
        let layout = DottyLayout::new(&paths);

        let plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        PlanExecutor::execute(plan).unwrap();

        let repository = BackupRepository::new(paths.backup_file());
        let backups = repository.read().unwrap();

        assert!(backups.backups().is_empty());
    }

    #[test]
    fn init_rolls_back_complete_initialization_when_later_action_fails() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());
        let layout = DottyLayout::new(&paths);

        let mut plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        let missing = temp.path().join("missing");

        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(!paths.config_file().exists());
        assert!(!paths.backup_file().exists());

        assert!(!paths.backup_dir().exists());
        assert!(!storage_dir.exists());
        assert!(!config_dir.exists());
        assert!(!repo_dir.exists());
    }

    #[test]
    fn init_rollback_preserves_preexisting_directories() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&repo_dir).unwrap();

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());
        let layout = DottyLayout::new(&paths);

        let mut plan = build_plan(&paths, &layout, &repo_dir).unwrap();

        let missing = temp.path().join("missing");

        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(config_dir.exists());
        assert!(repo_dir.exists());

        assert!(!storage_dir.exists());
        assert!(!paths.backup_dir().exists());

        assert!(!paths.config_file().exists());
        assert!(!paths.backup_file().exists());
    }
}
