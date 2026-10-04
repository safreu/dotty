use std::{
    collections::hash_map,
    path::{Path, PathBuf},
};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{
        BackupEntry, BackupKind, BackupRoot, ConfigRoot, EntryKind, ManagedEntry, ManagedEntryError,
    },
    infrastructure::{
        filesystem::{BackupStorage, BackupStorageError, FileSystemAction},
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::DottyPaths,
        },
    },
};

pub struct DotfileManager {
    config: ConfigRoot,
    config_repository: ConfigRepository,
    backups: BackupRoot,
    backups_repository: BackupRepository,
    backup_storage: BackupStorage,
}

impl DotfileManager {
    #[allow(unused)]
    pub fn new(
        config: ConfigRoot,
        config_repository: ConfigRepository,
        backups: BackupRoot,
        backups_repository: BackupRepository,
        backup_storage: BackupStorage,
    ) -> Self {
        Self {
            config,
            config_repository,
            backups,
            backups_repository,
            backup_storage,
        }
    }

    pub fn load(paths: &DottyPaths) -> Result<Self, DotfileManagerLoadError> {
        let config_repository = ConfigRepository::new(paths.config_file());
        let backups_repository = BackupRepository::new(paths.backup_file());

        let config = config_repository.read()?;

        let backups = backups_repository.read()?;

        let backup_storage = BackupStorage::new(paths.backup_dir());

        Ok(Self {
            config,
            config_repository,
            backups,
            backups_repository,
            backup_storage,
        })
    }

    pub fn manage(
        &mut self,
        target: PathBuf,
        entry_kind: EntryKind,
    ) -> Result<(), DotfileManagerError> {
        let (plan, config, backups) = self.build_manage_plan(&target, entry_kind)?;

        PlanExecutor::execute(plan)?;

        self.config = config;
        self.backups = backups;

        Ok(())
    }

    fn build_manage_plan(
        &self,
        target: &Path,
        entry_kind: EntryKind,
    ) -> Result<(Plan, ConfigRoot, BackupRoot), DotfileManagerError> {
        let managed_entry =
            ManagedEntry::new(target, self.config.config().dotfiles_dir(), &entry_kind)?;

        let mut config = self.config.clone();
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

        config.manages_mut().insert(managed_entry.clone());

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

        let config_write = self.config_repository.prepare_write(&config)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = config_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, config, backups))
    }

    pub fn forget(&mut self, target: PathBuf) -> Result<(), DotfileManagerError> {
        if !target.is_symlink() {
            return Err(DotfileManagerError::Unmanaged(target.clone()));
        }

        let (plan, config, backups) = self.build_forget_plan(&target)?;

        PlanExecutor::execute(plan)?;

        self.config = config;
        self.backups = backups;

        Ok(())
    }

    fn build_forget_plan(
        &self,
        target: &Path,
    ) -> Result<(Plan, ConfigRoot, BackupRoot), DotfileManagerError> {
        let mut config = self.config.clone();
        let mut backups = self.backups.clone();

        let managed_entry = config
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

        config.manages_mut().remove(&managed_entry);

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

        let config_write = self.config_repository.prepare_write(&config)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = config_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, config, backups))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    EntryCreation(#[from] ManagedEntryError),
    #[error(transparent)]
    ConfigWrite(#[from] ConfigRepositoryError),
    #[error(transparent)]
    BackupWrite(#[from] BackupRepositoryError),
    #[error(transparent)]
    BackupStorage(#[from] BackupStorageError),
    #[error(transparent)]
    PlanExecution(#[from] PlanExecutionError),
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerLoadError {
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::persistence::paths::DottyPaths;

    struct TestContext {
        temp: TempDir,
        paths: DottyPaths,
        dotfiles_dir: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let dotfiles_dir = temp.path().join("dotfiles");

            fs::create_dir_all(&config_dir).unwrap();
            fs::create_dir_all(&storage_dir).unwrap();
            fs::create_dir_all(&dotfiles_dir).unwrap();

            let paths = DottyPaths::new(config_dir, storage_dir);

            fs::create_dir_all(paths.backup_dir()).unwrap();

            Self {
                temp,
                paths,
                dotfiles_dir,
            }
        }

        fn manager(&self) -> DotfileManager {
            let config = ConfigRoot::new(self.dotfiles_dir.clone());
            let backups = BackupRoot::new();

            let config_repository = ConfigRepository::new(self.paths.config_file());

            let backup_repository = BackupRepository::new(self.paths.backup_file());

            config_repository.write(&config).unwrap();
            backup_repository.write(&backups).unwrap();

            DotfileManager::new(
                config,
                config_repository,
                backups,
                backup_repository,
                BackupStorage::new(self.paths.backup_dir()),
            )
        }

        fn create_file(&self, name: &str, content: &str) -> PathBuf {
            let path = self.temp.path().join(name);

            fs::write(&path, content).unwrap();

            path
        }

        fn read_config(&self) -> ConfigRoot {
            ConfigRepository::new(self.paths.config_file())
                .read()
                .unwrap()
        }

        fn read_backups(&self) -> BackupRoot {
            BackupRepository::new(self.paths.backup_file())
                .read()
                .unwrap()
        }

        fn link_backup(&self, filename: &str) -> PathBuf {
            self.paths
                .backup_dir()
                .join(filename)
                .join(format!("{filename}.bak.link"))
        }

        fn unlink_backup(&self, filename: &str) -> PathBuf {
            self.paths
                .backup_dir()
                .join(filename)
                .join(format!("{filename}.bak.unlink"))
        }
    }

    #[test]
    fn manage_file_moves_file_to_dotfiles_directory() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        let stored = ctx.dotfiles_dir.join("zshrc");

        assert!(stored.exists());
        assert_eq!(fs::read_to_string(stored).unwrap(), "hello");
    }

    #[test]
    fn manage_file_creates_symlink_at_original_location() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        assert!(target.is_symlink());

        let destination = fs::read_link(&target).unwrap();

        assert_eq!(destination, ctx.dotfiles_dir.join("zshrc"));
    }

    #[test]
    fn manage_file_persists_managed_entry() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        let config = ctx.read_config();

        assert_eq!(config.manages().len(), 1);

        let entry = config.manages().iter().next().unwrap();

        assert_eq!(entry.points_to(), &target);
        assert_eq!(entry.stored_at(), &ctx.dotfiles_dir.join("zshrc"));
        assert_eq!(entry.kind(), &EntryKind::File);
    }

    #[test]
    fn manage_file_creates_safety_backup() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target, EntryKind::File).unwrap();

        let backup = ctx.link_backup("zshrc");

        assert!(backup.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "hello");
    }

    #[test]
    fn manage_file_persists_backup_metadata() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        let backups = ctx.read_backups();

        assert!(backups.backups().contains_key(&target));

        let entry = backups.backups().get(&target).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));
    }

    #[test]
    fn manage_plan_rolls_back_when_later_action_fails() {
        let ctx = TestContext::new();
        let manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");
        let stored = ctx.dotfiles_dir.join("zshrc");

        let (mut plan, _, _) = manager.build_manage_plan(&target, EntryKind::File).unwrap();

        plan.push(FileSystemAction::RemoveFile(ctx.temp.path().join("does-not-exist")).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(target.exists());
        assert!(!target.is_symlink());
        assert!(!stored.exists());

        assert_eq!(fs::read_to_string(&target).unwrap(), "hello");

        assert!(!ctx.link_backup("zshrc").exists());

        let config = ctx.read_config();
        assert!(config.manages().is_empty());

        let backups = ctx.read_backups();
        assert!(backups.backups().is_empty());
    }

    #[test]
    fn forget_file_restores_original_file() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        manager.forget(target.clone()).unwrap();

        assert!(target.exists());
        assert!(!target.is_symlink());

        assert_eq!(fs::read_to_string(target).unwrap(), "hello");
    }

    #[test]
    fn forget_file_removes_file_from_dotfiles_directory() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        let stored = ctx.dotfiles_dir.join("zshrc");

        assert!(stored.exists());

        manager.forget(target).unwrap();

        assert!(!stored.exists());
    }

    #[test]
    fn forget_file_removes_managed_entry() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        manager.forget(target).unwrap();

        let config = ctx.read_config();

        assert!(config.manages().is_empty());
    }

    #[test]
    fn forget_file_creates_safety_backup() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        manager.forget(target).unwrap();

        let backup = ctx.unlink_backup("zshrc");

        assert!(backup.exists());
        assert_eq!(fs::read_to_string(backup).unwrap(), "hello");
    }

    #[test]
    fn forget_file_persists_unlink_backup_metadata() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        manager.forget(target.clone()).unwrap();

        let backups = ctx.read_backups();

        let entry = backups.backups().get(&target).unwrap();

        assert!(entry.backups().contains(&BackupKind::Link));
        assert!(entry.backups().contains(&BackupKind::Unlink));
    }

    #[test]
    fn forget_plan_rolls_back_when_later_action_fails() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        let stored = ctx.dotfiles_dir.join("zshrc");

        assert!(target.is_symlink());
        assert!(stored.exists());

        let (mut plan, _, _) = manager.build_forget_plan(&target).unwrap();

        plan.push(FileSystemAction::RemoveFile(ctx.temp.path().join("does-not-exist")).into());

        let result = PlanExecutor::execute(plan);

        assert!(result.is_err());

        assert!(target.is_symlink());
        assert!(stored.exists());

        assert_eq!(fs::read_to_string(&target).unwrap(), "hello");

        assert!(!ctx.unlink_backup("zshrc").exists());

        let config = ctx.read_config();

        assert_eq!(config.manages().len(), 1);

        let backups = ctx.read_backups();
        let backup_entry = backups.backups().get(&target).unwrap();

        assert!(backup_entry.backups().contains(&BackupKind::Link));
        assert!(!backup_entry.backups().contains(&BackupKind::Unlink));
    }

    #[test]
    fn manage_and_forget_keep_both_safety_backups() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        manager.manage(target.clone(), EntryKind::File).unwrap();

        manager.forget(target).unwrap();

        assert!(ctx.link_backup("zshrc").exists());
        assert!(ctx.unlink_backup("zshrc").exists());
    }

    #[test]
    fn forget_unmanaged_file_returns_unmanaged_error() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "hello");

        let result = manager.forget(target.clone());

        assert!(matches!(
            result,
            Err(DotfileManagerError::Unmanaged(path))
                if path == target
        ));
    }

    #[test]
    fn complete_file_lifecycle_preserves_content() {
        let ctx = TestContext::new();
        let mut manager = ctx.manager();

        let target = ctx.create_file(".zshrc", "export EDITOR=nvim\nalias ll='ls -la'\n");

        let original = fs::read_to_string(&target).unwrap();

        manager.manage(target.clone(), EntryKind::File).unwrap();

        assert!(target.is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), original);

        manager.forget(target.clone()).unwrap();

        assert!(!target.is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), original);
    }
}
