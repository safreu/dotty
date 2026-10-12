use std::{collections::hash_map, path::PathBuf};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{
        BackupEntry, BackupKind, BackupRoot, ConfigRoot, ConfigRootError, EntryKind, ManagedEntry,
        ManagedSource, ManagedTarget, ManifestRoot,
    },
    infrastructure::{
        filesystem::{
            self, BackupStorage, BackupStorageError, FileSystemAction, FileSystemError, PathKind,
            RepositoryLayout, RepositoryLayoutError,
        },
        persistence::{
            ManifestRepository, ManifestRepositoryError, RepositoryPaths,
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::DottyPaths,
        },
    },
};

pub struct DotfileManager {
    config: ConfigRoot,

    manifest: ManifestRoot,
    manifest_repository: ManifestRepository,

    backups: BackupRoot,
    backups_repository: BackupRepository,
    backup_storage: BackupStorage,
}

impl DotfileManager {
    #[allow(unused)]
    pub fn new(
        config: ConfigRoot,
        manifest: ManifestRoot,
        manifest_repository: ManifestRepository,
        backups: BackupRoot,
        backups_repository: BackupRepository,
        backup_storage: BackupStorage,
    ) -> Self {
        Self {
            config,
            manifest,
            manifest_repository,
            backups,
            backups_repository,
            backup_storage,
        }
    }

    pub fn load(paths: &DottyPaths) -> Result<Self, DotfileManagerLoadError> {
        let config_repository = ConfigRepository::new(paths.config_file());
        let backups_repository = BackupRepository::new(paths.backup_file());

        let config = config_repository.read()?;

        let repository_paths = RepositoryPaths::new(config.repo_dir().clone());

        let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

        let manifest = manifest_repository.read()?;

        let backups = backups_repository.read()?;

        let backup_storage = BackupStorage::new(paths.backup_dir());

        Ok(Self {
            config,
            manifest,
            manifest_repository,
            backups,
            backups_repository,
            backup_storage,
        })
    }

    pub fn manage(
        &mut self,
        target: ManagedTarget,
        source: ManagedSource,
    ) -> Result<(), DotfileManagerError> {
        let physical_target = self.config.resolve_target(&target);

        let entry_kind = match filesystem::path_kind(&physical_target)? {
            Some(PathKind::File) => EntryKind::File,
            Some(PathKind::Directory) => EntryKind::Dir,
            Some(PathKind::Symlink | PathKind::Other) | None => {
                return Err(DotfileManagerError::UnsupportedTarget(
                    physical_target.to_path_buf(),
                ));
            }
        };

        let managed_entry = ManagedEntry::new(source, target, entry_kind);

        let (plan, manifest, backups) = self.build_manage_plan(managed_entry)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_manage_plan(
        &self,
        managed_entry: ManagedEntry,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let source = self.config.resolve_source(managed_entry.source());
        let target = self.config.resolve_target(managed_entry.target());

        let new_backup_entry = BackupEntry::new(managed_entry.kind(), BackupKind::Link);

        match backups.backups_mut().entry(managed_entry.source().clone()) {
            hash_map::Entry::Occupied(entry) => {
                entry.into_mut().backups_mut().insert(BackupKind::Link);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(new_backup_entry);
            }
        };

        manifest.manages_mut().insert(managed_entry.clone());

        let mut plan = Plan::new();

        let repository_paths = RepositoryPaths::new(self.config.repo_dir().to_path_buf());

        let repository_layout = RepositoryLayout::new(&repository_paths);

        let repository_plan = repository_layout.managed_source_plan(managed_entry.source())?;

        plan.extend(repository_plan);

        for action in self.backup_storage.backup_actions(
            &target,
            managed_entry.source(),
            managed_entry.kind(),
            BackupKind::Link,
        )? {
            plan.push(action.into());
        }

        plan.push(
            FileSystemAction::Rename {
                from: target.clone(),
                to: source.clone(),
            }
            .into(),
        );

        plan.push(
            FileSystemAction::CreateSymlink {
                target: source,
                link: target.clone(),
            }
            .into(),
        );

        let manifest_write = self.manifest_repository.prepare_write(&manifest)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = manifest_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, manifest, backups))
    }

    pub fn forget(&mut self, target: ManagedTarget) -> Result<(), DotfileManagerError> {
        let physical_target = self.config.resolve_target(&target);

        match filesystem::path_kind(&physical_target)? {
            Some(PathKind::Symlink) => {}
            _ => return Err(DotfileManagerError::NotSymlink(physical_target)),
        }

        let (plan, manifest, backups) = self.build_forget_plan(&target)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_forget_plan(
        &self,
        target: &ManagedTarget,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let physical_target = self.config.resolve_target(target);

        let managed_entry = manifest
            .manages()
            .iter()
            .find(|entry| entry.target() == target)
            .cloned()
            .ok_or_else(|| DotfileManagerError::Unmanaged(physical_target.to_path_buf()))?;

        let backup_entry = backups
            .backups_mut()
            .get_mut(managed_entry.source())
            .ok_or(DotfileManagerError::Unmanaged(physical_target))?;

        backup_entry.backups_mut().insert(BackupKind::Unlink);

        manifest.manages_mut().remove(&managed_entry);

        let source = self.config.resolve_source(managed_entry.source());
        let target = self.config.resolve_target(managed_entry.target());

        let mut plan = Plan::new();

        let repository_paths = RepositoryPaths::new(self.config.repo_dir().to_path_buf());

        let repository_layout = RepositoryLayout::new(&repository_paths);

        let repository_plan =
            repository_layout.managed_source_cleanup_plan(managed_entry.source())?;

        for action in self.backup_storage.backup_actions(
            &source,
            managed_entry.source(),
            managed_entry.kind(),
            BackupKind::Unlink,
        )? {
            plan.push(action.into());
        }

        plan.push(FileSystemAction::RemoveSymlink(target.clone()).into());
        plan.push(
            FileSystemAction::Rename {
                from: source,
                to: target.clone(),
            }
            .into(),
        );

        plan.extend(repository_plan);

        let manifest_write = self.manifest_repository.prepare_write(&manifest)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = manifest_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, manifest, backups))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    ManifestWrite(#[from] ManifestRepositoryError),
    #[error(transparent)]
    BackupWrite(#[from] BackupRepositoryError),
    #[error(transparent)]
    BackupStorage(#[from] BackupStorageError),
    #[error(transparent)]
    PlanExecution(#[from] PlanExecutionError),
    #[error("This target is not a symlink: `{0}`")]
    NotSymlink(PathBuf),
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("target cannot be managed: `{0}`")]
    UnsupportedTarget(PathBuf),
    #[error(transparent)]
    ConfigRoot(#[from] ConfigRootError),
    #[error(transparent)]
    RepositoryLayout(#[from] RepositoryLayoutError),
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerLoadError {
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
    #[error(transparent)]
    Manifest(#[from] ManifestRepositoryError),
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::persistence::RepositoryPaths;

    struct TestContext {
        _temp: TempDir,

        #[allow(unused)]
        config_dir: PathBuf,
        #[allow(unused)]
        storage_dir: PathBuf,

        home_dir: PathBuf,
        repo_dir: PathBuf,

        paths: DottyPaths,
        repository_paths: RepositoryPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let home_dir = temp.path().join("home");
            let repo_dir = temp.path().join("repo");

            fs::create_dir_all(&config_dir).unwrap();
            fs::create_dir_all(&storage_dir).unwrap();
            fs::create_dir_all(&home_dir).unwrap();
            fs::create_dir_all(&repo_dir).unwrap();

            let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone(), home_dir.clone());

            fs::create_dir_all(paths.backup_dir()).unwrap();

            let repository_paths = RepositoryPaths::new(repo_dir.clone());

            fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

            let config_repository = ConfigRepository::new(paths.config_file());

            let backup_repository = BackupRepository::new(paths.backup_file());

            let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

            config_repository
                .write(&ConfigRoot::new(repo_dir.clone(), home_dir.clone()))
                .unwrap();

            backup_repository.write(&BackupRoot::new()).unwrap();

            manifest_repository.write(&ManifestRoot::new()).unwrap();

            Self {
                _temp: temp,
                config_dir,
                storage_dir,
                home_dir,
                repo_dir,
                paths,
                repository_paths,
            }
        }

        fn manager(&self) -> DotfileManager {
            DotfileManager::load(&self.paths).unwrap()
        }

        fn target(&self, path: &str) -> PathBuf {
            self.home_dir.join(path)
        }

        fn managed_target(&self, path: &str) -> ManagedTarget {
            ManagedTarget::home_relative(PathBuf::from(path)).unwrap()
        }

        fn managed_source(&self, path: &str) -> ManagedSource {
            ManagedSource::new(PathBuf::from(path)).unwrap()
        }

        #[allow(unused)]
        fn read_config(&self) -> ConfigRoot {
            ConfigRepository::new(self.paths.config_file())
                .read()
                .unwrap()
        }

        fn read_manifest(&self) -> ManifestRoot {
            ManifestRepository::new(&self.repository_paths.manifest_file())
                .read()
                .unwrap()
        }

        fn read_backups(&self) -> BackupRoot {
            BackupRepository::new(self.paths.backup_file())
                .read()
                .unwrap()
        }
    }

    #[test]
    fn load_uses_repository_from_config_to_find_manifest() {
        let context = TestContext::new();

        let result = DotfileManager::load(&context.paths);

        assert!(result.is_ok());
    }

    #[test]
    fn load_fails_when_manifest_is_missing() {
        let context = TestContext::new();

        fs::remove_file(context.repository_paths.manifest_file()).unwrap();

        let result = DotfileManager::load(&context.paths);

        assert!(matches!(result, Err(DotfileManagerLoadError::Manifest(_))));
    }

    #[test]
    fn manage_persists_entry_in_manifest() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source.clone())
            .unwrap();

        let manifest = context.read_manifest();

        assert_eq!(manifest.manages().len(), 1);

        let entry = manifest.manages().iter().next().unwrap();

        assert_eq!(entry.target(), &managed_target);
        assert_eq!(entry.source(), &managed_source);
        assert_eq!(entry.kind(), &EntryKind::File);
    }

    #[test]
    fn manage_does_not_modify_config() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let config_before = fs::read(context.paths.config_file()).unwrap();

        let mut manager = context.manager();

        manager.manage(managed_target, managed_source).unwrap();

        let config_after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(config_before, config_after);
    }

    #[test]
    fn manage_persists_backup_metadata() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target, managed_source.clone())
            .unwrap();

        let backups = context.read_backups();

        let entry = backups.backups().get(&managed_source).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));
    }

    #[test]
    fn manage_moves_target_into_repository() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager.manage(managed_target, managed_source).unwrap();

        let stored = context.repo_dir.join("zshrc");

        assert!(stored.is_file());

        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));
    }

    #[test]
    fn forget_removes_entry_from_manifest() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source)
            .unwrap();

        assert_eq!(context.read_manifest().manages().len(), 1);

        manager.forget(managed_target).unwrap();

        let manifest = context.read_manifest();

        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn forget_does_not_modify_config() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source)
            .unwrap();

        let config_before = fs::read(context.paths.config_file()).unwrap();

        manager.forget(managed_target).unwrap();

        let config_after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(config_before, config_after);
    }

    #[test]
    fn forget_restores_original_target() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source)
            .unwrap();

        manager.forget(managed_target).unwrap();

        assert!(target.is_file());
        assert_eq!(fs::read_to_string(target).unwrap(), "hello");
    }

    #[test]
    fn forget_persists_unlink_backup_metadata() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source.clone())
            .unwrap();

        manager.forget(managed_target).unwrap();

        let backups = context.read_backups();

        let entry = backups.backups().get(&managed_source).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));
        assert!(entry.backups().contains(&BackupKind::Unlink));
    }

    #[test]
    fn manage_supports_nested_source_path() {
        let context = TestContext::new();

        let target = context.target(".config/ghostty/config");

        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "hello").unwrap();

        let managed_target = context.managed_target(".config/ghostty/config");

        let managed_source = context.managed_source("ghostty/config");

        let mut manager = context.manager();

        manager
            .manage(managed_target, managed_source.clone())
            .unwrap();

        let stored = context.repo_dir.join("ghostty/config");

        assert!(stored.is_file());
        assert_eq!(fs::read_to_string(&stored).unwrap(), "hello");

        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(fs::read_link(&target).unwrap(), stored);

        let manifest = context.read_manifest();

        let entry = manifest
            .manages()
            .iter()
            .find(|entry| entry.source() == &managed_source)
            .unwrap();

        assert_eq!(
            entry.target(),
            &context.managed_target(".config/ghostty/config")
        );
    }

    #[test]
    fn manage_rolls_back_nested_directories_when_later_action_fails() {
        let context = TestContext::new();

        let target = context.target(".config/ghostty/config");

        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "original content").unwrap();

        let managed_target = context.managed_target(".config/ghostty/config");
        let managed_source = context.managed_source("ghostty/config");

        let manager = context.manager();

        let entry = ManagedEntry::new(managed_source.clone(), managed_target, EntryKind::File);

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();
        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let (mut plan, _, _) = manager.build_manage_plan(entry).unwrap();

        plan.push(FileSystemAction::RemoveFile(context.repo_dir.join("nonexistent-file")).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(target.is_file());
        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");

        assert!(!context.repo_dir.join("ghostty/config").exists());
        assert!(!context.repo_dir.join("ghostty").exists());

        assert!(!context.paths.backup_dir().join("ghostty/config").exists());
        assert!(!context.paths.backup_dir().join("ghostty").exists());

        assert!(context.repo_dir.is_dir());
        assert!(context.paths.backup_dir().is_dir());

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );
        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );
    }

    #[test]
    fn forget_removes_empty_nested_source_directories() {
        let context = TestContext::new();

        let target = context.target(".config/nvim/init.lua");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "vim.opt.number = true").unwrap();

        let managed_target = context.managed_target(".config/nvim/init.lua");
        let managed_source = context.managed_source("editors/nvim/init.lua");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source)
            .unwrap();

        assert!(context.repo_dir.join("editors/nvim/init.lua").is_file());

        manager.forget(managed_target).unwrap();

        assert!(target.is_file());
        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "vim.opt.number = true"
        );

        assert!(!context.repo_dir.join("editors/nvim").exists());
        assert!(!context.repo_dir.join("editors").exists());

        assert!(context.repo_dir.is_dir());
        assert!(context.repository_paths.metadata_dir().is_dir());
        assert!(context.read_manifest().manages().is_empty());
    }

    #[test]
    fn forget_preserves_directory_containing_other_managed_sources() {
        let context = TestContext::new();

        let config_target = context.target(".config/ghostty/config");
        let theme_target = context.target(".config/ghostty/themes.toml");

        fs::create_dir_all(config_target.parent().unwrap()).unwrap();
        fs::write(&config_target, "font-size = 14").unwrap();
        fs::write(&theme_target, "theme = 'dark'").unwrap();

        let config_managed_target = context.managed_target(".config/ghostty/config");
        let theme_managed_target = context.managed_target(".config/ghostty/themes.toml");

        let mut manager = context.manager();

        manager
            .manage(
                config_managed_target.clone(),
                context.managed_source("ghostty/config"),
            )
            .unwrap();

        manager
            .manage(
                theme_managed_target.clone(),
                context.managed_source("ghostty/themes.toml"),
            )
            .unwrap();

        manager.forget(config_managed_target).unwrap();

        assert!(context.repo_dir.join("ghostty").is_dir());
        assert!(context.repo_dir.join("ghostty/themes.toml").is_file());
        assert!(!context.repo_dir.join("ghostty/config").exists());

        assert_eq!(
            fs::read_to_string(&config_target).unwrap(),
            "font-size = 14"
        );

        assert!(matches!(
            filesystem::path_kind(&theme_target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(context.read_manifest().manages().len(), 1);
    }

    #[test]
    fn forget_removes_shared_directory_after_last_source() {
        let context = TestContext::new();

        let first_target = context.target(".config/ghostty/config");
        let second_target = context.target(".config/ghostty/themes.toml");

        fs::create_dir_all(first_target.parent().unwrap()).unwrap();
        fs::write(&first_target, "config content").unwrap();
        fs::write(&second_target, "theme content").unwrap();

        let first_managed_target = context.managed_target(".config/ghostty/config");
        let second_managed_target = context.managed_target(".config/ghostty/themes.toml");

        let mut manager = context.manager();

        manager
            .manage(
                first_managed_target.clone(),
                context.managed_source("ghostty/config"),
            )
            .unwrap();

        manager
            .manage(
                second_managed_target.clone(),
                context.managed_source("ghostty/themes.toml"),
            )
            .unwrap();

        manager.forget(first_managed_target).unwrap();

        assert!(context.repo_dir.join("ghostty").is_dir());

        manager.forget(second_managed_target).unwrap();

        assert!(!context.repo_dir.join("ghostty").exists());

        assert_eq!(fs::read_to_string(&first_target).unwrap(), "config content");
        assert_eq!(fs::read_to_string(&second_target).unwrap(), "theme content");

        assert!(context.repo_dir.is_dir());
        assert!(context.read_manifest().manages().is_empty());
    }

    #[test]
    fn forget_rolls_back_nested_directory_cleanup_when_later_action_fails() {
        let context = TestContext::new();

        let target = context.target(".config/nvim/init.lua");
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(&target, "original content").unwrap();

        let managed_target = context.managed_target(".config/nvim/init.lua");
        let managed_source = context.managed_source("editors/nvim/init.lua");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source.clone())
            .unwrap();

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();
        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let (mut plan, _, _) = manager.build_forget_plan(&managed_target).unwrap();

        // Force failure after the normal forget actions.
        plan.push(FileSystemAction::RemoveFile(context.repo_dir.join("nonexistent-file")).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        // The managed source must be restored.
        let source = context.repo_dir.join("editors/nvim/init.lua");

        assert!(source.is_file());
        assert_eq!(fs::read_to_string(&source).unwrap(), "original content");

        // Removed parent directories must be recreated.
        assert!(context.repo_dir.join("editors/nvim").is_dir());
        assert!(context.repo_dir.join("editors").is_dir());

        // The original target must again be a symlink.
        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");

        // Persistent metadata must be unchanged.
        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );
        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );

        // Any backup created by the failed forget must be removed.
        assert!(
            !context
                .paths
                .backup_dir()
                .join("editors/nvim/init.lua/unlink")
                .exists()
        );
    }
}
