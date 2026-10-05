use std::{
    collections::hash_map,
    path::{Path, PathBuf},
};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{
        BackupEntry, BackupKind, BackupRoot, ConfigRoot, EntryKind, ManagedEntry,
        ManagedEntryError, ManifestRoot,
    },
    infrastructure::{
        filesystem::{
            self, BackupStorage, BackupStorageError, FileSystemAction, FileSystemError, PathKind,
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

    pub fn manage(&mut self, target: PathBuf) -> Result<(), DotfileManagerError> {
        let entry_kind = match filesystem::path_kind(&target)? {
            Some(PathKind::File) => EntryKind::File,
            Some(PathKind::Directory) => EntryKind::Dir,
            Some(PathKind::Symlink | PathKind::Other) | None => {
                return Err(DotfileManagerError::UnsupportedTarget(target));
            }
        };

        let (plan, manifest, backups) = self.build_manage_plan(&target, entry_kind)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_manage_plan(
        &self,
        target: &Path,
        entry_kind: EntryKind,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let managed_entry = ManagedEntry::new(target, self.config.repo_dir(), &entry_kind)?;

        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let new_backup_entry = BackupEntry::new(&entry_kind, BackupKind::Link);

        match backups.backups_mut().entry(target.to_path_buf()) {
            hash_map::Entry::Occupied(entry) => {
                entry.into_mut().backups_mut().insert(BackupKind::Link);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(new_backup_entry);
            }
        };

        manifest.manages_mut().insert(managed_entry.clone());

        let mut plan = Plan::new();

        for action in self
            .backup_storage
            .backup_actions(&managed_entry, BackupKind::Link)?
        {
            plan.push(action.into());
        }

        plan.push(
            FileSystemAction::Rename {
                from: target.to_path_buf(),
                to: managed_entry.stored_at().to_path_buf(),
            }
            .into(),
        );

        plan.push(
            FileSystemAction::CreateSymlink {
                target: managed_entry.stored_at().to_path_buf(),
                link: managed_entry.points_to().to_path_buf(),
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

    pub fn forget(&mut self, target: PathBuf) -> Result<(), DotfileManagerError> {
        match filesystem::path_kind(&target)? {
            Some(PathKind::Symlink) => {}
            _ => return Err(DotfileManagerError::NotSymlink(target.to_path_buf())),
        }

        let (plan, manifest, backups) = self.build_forget_plan(&target)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_forget_plan(
        &self,
        target: &Path,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let managed_entry = manifest
            .manages()
            .iter()
            .find(|entry| entry.points_to() == target)
            .cloned()
            .ok_or_else(|| DotfileManagerError::Unmanaged(target.to_path_buf()))?;

        let backup_entry = backups
            .backups_mut()
            .get_mut(target)
            .ok_or_else(|| DotfileManagerError::Unmanaged(target.to_path_buf()))?;

        backup_entry.backups_mut().insert(BackupKind::Unlink);

        manifest.manages_mut().remove(&managed_entry);

        let mut plan = Plan::new();

        for action in self
            .backup_storage
            .backup_actions(&managed_entry, BackupKind::Unlink)?
        {
            plan.push(action.into());
        }

        plan.push(FileSystemAction::RemoveSymlink(target.to_path_buf()).into());
        plan.push(
            FileSystemAction::Rename {
                from: managed_entry.stored_at().to_path_buf(),
                to: target.to_path_buf(),
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
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    EntryCreation(#[from] ManagedEntryError),
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
        repo_dir: PathBuf,

        paths: DottyPaths,
        repository_paths: RepositoryPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let repo_dir = temp.path().join("repo");

            fs::create_dir_all(&config_dir).unwrap();
            fs::create_dir_all(&storage_dir).unwrap();
            fs::create_dir_all(&repo_dir).unwrap();

            let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

            fs::create_dir_all(paths.backup_dir()).unwrap();

            let repository_paths = RepositoryPaths::new(repo_dir.clone());

            fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

            let config_repository = ConfigRepository::new(paths.config_file());

            let backup_repository = BackupRepository::new(paths.backup_file());

            let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

            config_repository
                .write(&ConfigRoot::new(repo_dir.clone()))
                .unwrap();

            backup_repository.write(&BackupRoot::new()).unwrap();

            manifest_repository.write(&ManifestRoot::new()).unwrap();

            Self {
                _temp: temp,
                config_dir,
                storage_dir,
                repo_dir,
                paths,
                repository_paths,
            }
        }

        fn manager(&self) -> DotfileManager {
            DotfileManager::load(&self.paths).unwrap()
        }

        fn target(&self, name: &str) -> PathBuf {
            self._temp.path().join(name)
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

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();

        let manifest = context.read_manifest();

        assert_eq!(manifest.manages().len(), 1);

        let entry = manifest.manages().iter().next().unwrap();

        assert_eq!(entry.points_to(), &target);
    }

    #[test]
    fn manage_does_not_modify_config() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let config_before = fs::read(context.paths.config_file()).unwrap();

        let mut manager = context.manager();

        manager.manage(target).unwrap();

        let config_after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(config_before, config_after);
    }

    #[test]
    fn manage_persists_backup_metadata() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();

        let backups = context.read_backups();

        let entry = backups.backups().get(&target).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));
    }

    #[test]
    fn manage_moves_target_into_repository() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();

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

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();

        assert_eq!(context.read_manifest().manages().len(), 1);

        manager.forget(target).unwrap();

        let manifest = context.read_manifest();

        assert!(manifest.manages().is_empty());
    }

    #[test]
    fn forget_does_not_modify_config() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();

        let config_before = fs::read(context.paths.config_file()).unwrap();

        manager.forget(target).unwrap();

        let config_after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(config_before, config_after);
    }

    #[test]
    fn forget_restores_original_target() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();
        manager.forget(target.clone()).unwrap();

        assert!(target.is_file());
        assert_eq!(fs::read_to_string(target).unwrap(), "hello");
    }

    #[test]
    fn forget_persists_unlink_backup_metadata() {
        let context = TestContext::new();

        let target = context.target(".zshrc");
        fs::write(&target, "hello").unwrap();

        let mut manager = context.manager();

        manager.manage(target.clone()).unwrap();
        manager.forget(target.clone()).unwrap();

        let backups = context.read_backups();

        let entry = backups.backups().get(&target).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));

        assert!(entry.backups().contains(&BackupKind::Unlink));
    }
}
