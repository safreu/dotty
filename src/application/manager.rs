use std::{collections::hash_map, fs, os::unix::fs::symlink, path::PathBuf};

use crate::{
    application::{OperationError, OperationTransaction, TransactionCommitError},
    domain::{
        BackupEntry, BackupKind, BackupRoot, ConfigRoot, EntryKind, ManagedEntry, ManagedEntryError,
    },
    infrastructure::{
        filesystem::{BackupStorage, FileSystemError, RollbackAction},
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
    ) -> Result<(), OperationError<DotfileManagerError>> {
        let mut transaction = OperationTransaction::new(
            &mut self.config,
            &self.config_repository,
            &mut self.backups,
            &self.backups_repository,
        );

        let result = ManagedEntry::new(
            &target,
            transaction.config().config().dotfiles_dir(),
            &entry_kind,
        )
        .map_err(DotfileManagerError::from);

        let managed_entry = transaction.handle(result)?;

        let new_backup_entry = BackupEntry::new(&entry_kind, BackupKind::Link);

        match transaction
            .backups_mut()
            .backups_mut()
            .entry(target.clone())
        {
            hash_map::Entry::Occupied(entry) => {
                entry.into_mut().backups_mut().insert(BackupKind::Link);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(new_backup_entry);
            }
        };

        let backup_result = self
            .backup_storage
            .backup(&managed_entry, BackupKind::Link)
            .map_err(DotfileManagerError::from);

        transaction.handle(backup_result)?;

        let result =
            fs::rename(&target, managed_entry.stored_at()).map_err(|_| DotfileManagerError::Move);

        transaction.handle(result)?;

        transaction.register(RollbackAction::Rename {
            from: managed_entry.stored_at().to_path_buf(),
            to: target.to_path_buf(),
        });

        let result = symlink(managed_entry.stored_at(), managed_entry.points_to())
            .map_err(|_| DotfileManagerError::Symlink);

        transaction.handle(result)?;

        transaction.register(RollbackAction::RemoveSymlink(
            managed_entry.points_to().to_path_buf(),
        ));

        transaction.config_mut().manages_mut().insert(managed_entry);

        transaction
            .commit()
            .map_err(|error| error.map_operation(DotfileManagerError::from))?;

        Ok(())
    }

    pub fn forget(&mut self, target: PathBuf) -> Result<(), OperationError<DotfileManagerError>> {
        if !target.is_symlink() {
            return Err(OperationError::Operation(DotfileManagerError::Unmanaged(
                target.clone(),
            )));
        }

        let mut transaction = OperationTransaction::new(
            &mut self.config,
            &self.config_repository,
            &mut self.backups,
            &self.backups_repository,
        );

        let result = transaction
            .config()
            .manages()
            .iter()
            .find(|entry| entry.points_to() == &target)
            .cloned()
            .ok_or_else(|| DotfileManagerError::Unmanaged(target.clone()));

        let managed_entry = transaction.handle(result)?;

        let backup_result = {
            match transaction.backups_mut().backups_mut().get_mut(&target) {
                Some(backup_entry) => {
                    backup_entry.backups_mut().insert(BackupKind::Unlink);

                    Ok(())
                }
                None => Err(DotfileManagerError::Unmanaged(target.clone())),
            }
        };

        transaction.handle(backup_result)?;

        let backup_result = self
            .backup_storage
            .backup(&managed_entry, BackupKind::Unlink)
            .map_err(DotfileManagerError::from);

        transaction.handle(backup_result)?;

        let result = fs::remove_file(&target).map_err(|_| DotfileManagerError::DeleteSymlink);

        transaction.handle(result)?;

        transaction.register(RollbackAction::CreateSymlink {
            source: managed_entry.stored_at().to_path_buf(),
            destination: target.to_path_buf(),
        });

        let result =
            fs::rename(managed_entry.stored_at(), &target).map_err(|_| DotfileManagerError::Move);

        transaction.handle(result)?;

        transaction.register(RollbackAction::Rename {
            from: target.to_path_buf(),
            to: managed_entry.stored_at().to_path_buf(),
        });

        transaction
            .config_mut()
            .manages_mut()
            .remove(&managed_entry);

        transaction
            .commit()
            .map_err(|error| error.map_operation(DotfileManagerError::from))?;

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error(transparent)]
    PathCreation(#[from] FileSystemError),
    #[error("Failed to move target")]
    Move,
    #[error("Failed to symlink target")]
    Symlink,
    #[error("Failed to remove symlink target")]
    DeleteSymlink,
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    EntryCreation(#[from] ManagedEntryError),
    #[error(transparent)]
    ConfigWrite(#[from] ConfigRepositoryError),
    #[error(transparent)]
    BackupWrite(#[from] BackupRepositoryError),
    #[error(transparent)]
    Commit(#[from] TransactionCommitError),
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
            Err(OperationError::Operation(
                DotfileManagerError::Unmanaged(path)
            )) if path == target
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
