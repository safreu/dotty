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
        home_dir: PathBuf,
        paths: DottyPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let repo_dir = temp.path().join("repo");
            let home_dir = temp.path().join("home");

            let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone(), home_dir.clone());

            Self {
                _temp: temp,
                config_dir,
                storage_dir,
                repo_dir,
                home_dir,
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

        assert_eq!(config.repo_dir(), context.repo_dir.as_path());
        assert_eq!(config.home_dir(), context.home_dir.as_path());
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

    #[test]
    fn init_preserves_existing_manifest() {
        use crate::domain::{EntryKind, ManagedEntry, ManagedSource, ManagedTarget};

        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

        let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

        let source = ManagedSource::new(PathBuf::from("ghostty/config")).unwrap();
        let target = ManagedTarget::home_relative(PathBuf::from(".config/ghostty/config")).unwrap();

        let mut manifest = ManifestRoot::new();
        manifest
            .add(ManagedEntry::new(source, target, EntryKind::File))
            .unwrap();

        manifest_repository.write(&manifest).unwrap();

        let contents_before = fs::read(repository_paths.manifest_file()).unwrap();

        context.initialize();

        let contents_after = fs::read(repository_paths.manifest_file()).unwrap();
        let loaded_manifest = manifest_repository.read().unwrap();

        assert_eq!(contents_before, contents_after);
        assert_eq!(loaded_manifest.manages().len(), 1);

        assert!(context.paths.config_file().is_file());
        assert!(context.paths.backup_file().is_file());
    }

    #[test]
    fn init_rejects_invalid_existing_manifest() {
        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

        fs::write(
            repository_paths.manifest_file(),
            "this is not valid TOML = [",
        )
        .unwrap();

        let layout = context.layout();

        let result = super::build_plan(&context.paths, &layout, &context.repo_dir);

        assert!(matches!(result, Err(InitError::Manifest(_))));

        assert!(!context.paths.config_file().exists());
        assert!(!context.paths.backup_file().exists());

        assert!(repository_paths.manifest_file().is_file());
    }

    #[test]
    fn init_rollback_preserves_existing_manifest() {
        use crate::domain::{EntryKind, ManagedEntry, ManagedSource, ManagedTarget};

        let context = TestContext::new();
        let repository_paths = context.repository_paths();

        fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

        let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

        let source = ManagedSource::new(PathBuf::from("nvim/init.lua")).unwrap();
        let target = ManagedTarget::home_relative(PathBuf::from(".config/nvim/init.lua")).unwrap();

        let mut manifest = ManifestRoot::new();
        manifest
            .add(ManagedEntry::new(source, target, EntryKind::File))
            .unwrap();

        manifest_repository.write(&manifest).unwrap();

        let contents_before = fs::read(repository_paths.manifest_file()).unwrap();

        let mut plan = context.build_plan();

        let missing = context._temp.path().join("nonexistent-file");
        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert_eq!(
            fs::read(repository_paths.manifest_file()).unwrap(),
            contents_before
        );

        assert_eq!(manifest_repository.read().unwrap().manages().len(), 1);

        assert!(!context.paths.config_file().exists());
        assert!(!context.paths.backup_file().exists());
        assert!(!context.paths.backup_dir().exists());
        assert!(!context.config_dir.exists());
        assert!(!context.storage_dir.exists());

        assert!(context.repo_dir.is_dir());
        assert!(repository_paths.metadata_dir().is_dir());
    }
}
