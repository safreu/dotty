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

    let config = ConfigRoot::new(repo_dir.to_path_buf());
    let backups = BackupRoot::new();
    let manifest = ManifestRoot::new();

    let config_write = config_repository.prepare_write(&config)?;
    let backups_write = backups_repository.prepare_write(&backups)?;
    let manifest_write = manifest_repository.prepare_write(&manifest)?;

    let (path, contents) = config_write.into_parts();
    plan.push(FileSystemAction::WriteFile { path, contents }.into());

    let (path, contents) = backups_write.into_parts();
    plan.push(FileSystemAction::WriteFile { path, contents }.into());

    let (path, contents) = manifest_write.into_parts();
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
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::{filesystem::FileSystemAction, persistence::RepositoryPaths};

    struct TestContext {
        _temp: TempDir,
        config_dir: PathBuf,
        storage_dir: PathBuf,
        repo_dir: PathBuf,
        paths: DottyPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let repo_dir = temp.path().join("repo");

            let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

            Self {
                _temp: temp,
                config_dir,
                storage_dir,
                repo_dir,
                paths,
            }
        }

        fn layout(&self) -> DottyLayout<'_> {
            DottyLayout::new(&self.paths)
        }

        fn repository_paths(&self) -> RepositoryPaths {
            RepositoryPaths::new(self.repo_dir.clone())
        }

        fn build_plan(&self) -> Plan {
            let layout = self.layout();

            super::build_plan(&self.paths, &layout, &self.repo_dir).unwrap()
        }

        fn initialize(&self) {
            PlanExecutor::execute(self.build_plan()).unwrap();
        }
    }

    #[test]
    fn init_creates_config_file() {
        let context = TestContext::new();

        context.initialize();

        assert!(context.paths.config_file().is_file());
    }

    #[test]
    fn init_creates_backup_file() {
        let context = TestContext::new();

        context.initialize();

        assert!(context.paths.backup_file().is_file());
    }

    #[test]
    fn init_creates_manifest_file() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        context.initialize();

        assert!(repository_paths.manifest_file().is_file());
    }

    #[test]
    fn init_creates_required_directories() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        context.initialize();

        assert!(context.config_dir.is_dir());
        assert!(context.storage_dir.is_dir());
        assert!(context.paths.backup_dir().is_dir());

        assert!(context.repo_dir.is_dir());
        assert!(repository_paths.metadata_dir().is_dir());
    }

    #[test]
    fn init_persists_valid_config() {
        let context = TestContext::new();

        context.initialize();

        let repository = ConfigRepository::new(context.paths.config_file());

        let config = repository.read().unwrap();

        assert_eq!(config.repo_dir(), &context.repo_dir);
    }

    #[test]
    fn init_persists_valid_backup_root() {
        let context = TestContext::new();

        context.initialize();

        let repository = BackupRepository::new(context.paths.backup_file());

        let backups = repository.read().unwrap();

        assert!(backups.backups().is_empty());
    }

    #[test]
    fn init_persists_valid_manifest_root() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        context.initialize();

        let repository = ManifestRepository::new(&repository_paths.manifest_file());

        let manifest = repository.read().unwrap();

        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn init_rolls_back_complete_initialization_when_later_action_fails() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        let mut plan = context.build_plan();

        let missing = context._temp.path().join("missing");

        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(!context.paths.config_file().exists());
        assert!(!context.paths.backup_file().exists());
        assert!(!repository_paths.manifest_file().exists());

        assert!(!repository_paths.metadata_dir().exists());
        assert!(!context.paths.backup_dir().exists());
        assert!(!context.storage_dir.exists());
        assert!(!context.config_dir.exists());
        assert!(!context.repo_dir.exists());
    }

    #[test]
    fn init_rollback_preserves_preexisting_directories() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.repo_dir).unwrap();

        let mut plan = context.build_plan();

        let missing = context._temp.path().join("missing");

        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(context.config_dir.exists());
        assert!(context.repo_dir.exists());

        assert!(!context.storage_dir.exists());
        assert!(!context.paths.backup_dir().exists());
        assert!(!repository_paths.metadata_dir().exists());

        assert!(!context.paths.config_file().exists());
        assert!(!context.paths.backup_file().exists());
        assert!(!repository_paths.manifest_file().exists());
    }
}
