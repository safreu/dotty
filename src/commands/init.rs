use std::path::PathBuf;

use crate::{
    domain::{BackupRoot, ConfigRoot},
    infrastructure::{
        filesystem::{
            DottyLayout, DottyLayoutError, RollbackAction, RollbackOperationError, RollbackStack,
        },
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
    let mut rollback = RollbackStack::new();

    if layout.is_initialized() {
        return Err(InitError::AlreadyInit);
    }

    if let Err(e) = layout.initialize(&repo_dir, &mut rollback) {
        return Err(rollback.fail(InitOperationError::Layout(e)).into());
    }

    let config = ConfigRoot::new(repo_dir);
    let backups = BackupRoot::new();

    let config_repository = ConfigRepository::new(paths.config_file());
    let backups_repository = BackupRepository::new(paths.backup_file());

    if let Err(e) = config_repository.write(&config) {
        return Err(rollback.fail(InitOperationError::Config(e)).into());
    }
    rollback.register(RollbackAction::RemoveFile(
        paths.config_file().to_path_buf(),
    ));

    if let Err(e) = backups_repository.write(&backups) {
        return Err(rollback.fail(InitOperationError::Backups(e)).into());
    };
    rollback.register(RollbackAction::RemoveFile(
        paths.backup_file().to_path_buf(),
    ));

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("Dotty is already initialized")]
    AlreadyInit,
    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
    #[error(transparent)]
    Operation(#[from] RollbackOperationError<InitOperationError>),
}

#[derive(Debug, thiserror::Error)]
pub enum InitOperationError {
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
    #[error(transparent)]
    Layout(#[from] DottyLayoutError),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn init_creates_config_file() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir);

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        let config = ConfigRoot::new(repo_dir);

        let repository = ConfigRepository::new(paths.config_file());
        repository.write(&config).unwrap();

        assert!(paths.config_file().exists());
    }

    #[test]
    fn init_creates_backup_file() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        let backups = BackupRoot::new();

        let repository = BackupRepository::new(paths.backup_file());
        repository.write(&backups).unwrap();

        assert!(paths.backup_file().exists());
    }

    #[test]
    fn init_creates_required_directories() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        assert!(config_dir.is_dir());
        assert!(storage_dir.is_dir());
        assert!(paths.backup_dir().is_dir());
        assert!(repo_dir.is_dir());
    }

    #[test]
    fn rollback_removes_directories_created_during_initialization() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        assert!(config_dir.exists());
        assert!(storage_dir.exists());
        assert!(repo_dir.exists());

        rollback.rollback().unwrap();

        assert!(!config_dir.exists());
        assert!(!storage_dir.exists());
        assert!(!repo_dir.exists());
    }

    #[test]
    fn rollback_preserves_preexisting_directories() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&repo_dir).unwrap();

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        assert!(config_dir.exists());
        assert!(storage_dir.exists());
        assert!(repo_dir.exists());

        rollback.rollback().unwrap();

        // These existed before initialization, so Dotty must preserve them.
        assert!(config_dir.exists());
        assert!(repo_dir.exists());

        // This was created by initialization, so rollback removes it.
        assert!(!storage_dir.exists());
    }

    #[test]
    fn rollback_removes_files_registered_during_initialization() {
        let temp = TempDir::new().unwrap();

        let config_dir = temp.path().join("config");
        let storage_dir = temp.path().join("storage");
        let repo_dir = temp.path().join("repo");

        let paths = DottyPaths::new(config_dir, storage_dir);

        let layout = DottyLayout::new(&paths);
        let mut rollback = RollbackStack::new();

        layout.initialize(&repo_dir, &mut rollback).unwrap();

        let config = ConfigRoot::new(repo_dir);
        let config_repository = ConfigRepository::new(paths.config_file());

        config_repository.write(&config).unwrap();

        rollback.register(RollbackAction::RemoveFile(
            paths.config_file().to_path_buf(),
        ));

        assert!(paths.config_file().exists());

        rollback.rollback().unwrap();

        assert!(!paths.config_file().exists());
    }
}
