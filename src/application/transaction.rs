use crate::{
    domain::{BackupRoot, ConfigRoot},
    infrastructure::{
        filesystem::{RollbackAction, RollbackError, RollbackStack},
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
        },
    },
};

pub struct OperationTransaction<'a> {
    config: &'a mut ConfigRoot,
    config_repository: &'a ConfigRepository,

    backups: &'a mut BackupRoot,
    backups_repository: &'a BackupRepository,

    original_config: ConfigRoot,
    original_backups: BackupRoot,

    rollback: RollbackStack,
}

impl<'a> OperationTransaction<'a> {
    pub fn new(
        config: &'a mut ConfigRoot,
        config_repository: &'a ConfigRepository,
        backups: &'a mut BackupRoot,
        backups_repository: &'a BackupRepository,
    ) -> Self {
        let original_config = config.clone();
        let original_backups = backups.clone();
        Self {
            config,
            config_repository,
            backups,
            backups_repository,
            original_config,
            original_backups,
            rollback: RollbackStack::new(),
        }
    }

    pub fn register(&mut self, action: RollbackAction) {
        self.rollback.register(action);
    }

    pub fn restore_metadata(&mut self) -> Result<(), TransactionRollbackError> {
        *self.config = self.original_config.clone();
        *self.backups = self.original_backups.clone();

        let config_result = self.config_repository.write(self.config());
        let backups_result = self.backups_repository.write(self.backups());

        let config_error = config_result.err();
        let backups_error = backups_result.err();

        if config_error.is_none() && backups_error.is_none() {
            Ok(())
        } else {
            Err(TransactionRollbackError {
                filesystem: None,
                config: config_error,
                backups: backups_error,
            })
        }
    }

    pub fn fail<E>(&mut self, operation: E) -> OperationError<E> {
        let metadata_result = self.restore_metadata();
        let filesystem_result = self.rollback.rollback();

        let metadata_error = metadata_result.err();
        let filesystem_error = filesystem_result.err();

        let (config_error, backups_error) = match metadata_error {
            Some(error) => (error.config, error.backups),
            None => (None, None),
        };

        if filesystem_error.is_none() && config_error.is_none() && backups_error.is_none() {
            OperationError::Operation(operation)
        } else {
            OperationError::Rollback {
                operation,
                rollback: TransactionRollbackError {
                    filesystem: filesystem_error,
                    config: config_error,
                    backups: backups_error,
                },
            }
        }
    }

    pub fn handle<T, E>(&mut self, result: Result<T, E>) -> Result<T, OperationError<E>> {
        match result {
            Ok(value) => Ok(value),
            Err(e) => Err(self.fail(e)),
        }
    }

    pub fn config(&self) -> &ConfigRoot {
        self.config
    }

    pub fn backups(&self) -> &BackupRoot {
        self.backups
    }

    pub fn config_mut(&mut self) -> &mut ConfigRoot {
        self.config
    }

    pub fn backups_mut(&mut self) -> &mut BackupRoot {
        self.backups
    }

    pub fn persist_config(&self) -> Result<(), ConfigRepositoryError> {
        self.config_repository.write(self.config)
    }

    pub fn persist_backups(&self) -> Result<(), BackupRepositoryError> {
        self.backups_repository.write(self.backups())
    }
    pub fn commit(&mut self) -> Result<(), OperationError<TransactionCommitError>> {
        if let Err(e) = self.persist_config() {
            return Err(self.fail(TransactionCommitError::Config(e)));
        }

        if let Err(e) = self.persist_backups() {
            return Err(self.fail(TransactionCommitError::Backups(e)));
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum TransactionCommitError {
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),

    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
}

#[derive(Debug, thiserror::Error)]
#[error("transaction rollback failed")]
pub struct TransactionRollbackError {
    pub filesystem: Option<RollbackError>,
    pub config: Option<ConfigRepositoryError>,
    pub backups: Option<BackupRepositoryError>,
}

#[derive(Debug, thiserror::Error)]
pub enum OperationError<E> {
    #[error(transparent)]
    Operation(E),
    #[error("Operation failed and rollback also failed")]
    Rollback {
        operation: E,
        rollback: TransactionRollbackError,
    },
}

impl<E> From<E> for OperationError<E> {
    fn from(value: E) -> Self {
        Self::Operation(value)
    }
}

impl<E> OperationError<E> {
    pub fn map_operation<T, F>(self, f: F) -> OperationError<T>
    where
        F: FnOnce(E) -> T,
    {
        match self {
            OperationError::Operation(operation) => OperationError::Operation(f(operation)),
            OperationError::Rollback {
                operation,
                rollback,
            } => OperationError::Rollback {
                operation: f(operation),
                rollback,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::*;
    use crate::{
        domain::{BackupEntry, BackupKind, EntryKind, ManagedEntry},
        infrastructure::persistence::paths::DottyPaths,
    };

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

        fn roots(&self) -> (ConfigRoot, BackupRoot) {
            (
                ConfigRoot::new(self.dotfiles_dir.clone()),
                BackupRoot::new(),
            )
        }

        fn repositories(&self) -> (ConfigRepository, BackupRepository) {
            (
                ConfigRepository::new(self.paths.config_file()),
                BackupRepository::new(self.paths.backup_file()),
            )
        }

        fn persist_initial_state(
            &self,
            config: &ConfigRoot,
            backups: &BackupRoot,
            config_repository: &ConfigRepository,
            backup_repository: &BackupRepository,
        ) {
            config_repository.write(config).unwrap();
            backup_repository.write(backups).unwrap();
        }

        fn managed_entry(&self, target: &std::path::Path) -> ManagedEntry {
            ManagedEntry::new(target, &self.dotfiles_dir, &EntryKind::File).unwrap()
        }

        fn backup_entry(&self, target: &std::path::Path) -> BackupEntry {
            BackupEntry::new(target, &EntryKind::File, BackupKind::Link)
        }
    }

    #[test]
    fn restore_metadata_restores_original_config_in_memory() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .config_mut()
                .manages_mut()
                .insert(ctx.managed_entry(&target));

            assert_eq!(transaction.config().manages().len(), 1);

            transaction.restore_metadata().unwrap();

            assert!(transaction.config().manages().is_empty());
        }

        assert!(config.manages().is_empty());
    }

    #[test]
    fn restore_metadata_restores_original_config_on_disk() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = fs::read_to_string(ctx.paths.config_file()).unwrap();
        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .config_mut()
                .manages_mut()
                .insert(ctx.managed_entry(&target));

            transaction.restore_metadata().unwrap();
        }

        let restored = fs::read_to_string(ctx.paths.config_file()).unwrap();

        assert_eq!(restored, original);
    }

    #[test]
    fn restore_metadata_restores_original_backups_on_disk() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = fs::read_to_string(ctx.paths.backup_file()).unwrap();
        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .backups_mut()
                .backups_mut()
                .insert(target.clone(), ctx.backup_entry(&target));

            transaction.restore_metadata().unwrap();
        }

        let restored = fs::read_to_string(ctx.paths.backup_file()).unwrap();

        assert_eq!(restored, original);
    }

    #[test]
    fn fail_returns_operation_error_when_rollback_succeeds() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        let error = transaction.fail("operation failed");

        assert!(matches!(
            error,
            OperationError::Operation("operation failed")
        ));
    }

    #[test]
    fn fail_restores_metadata() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original_config = fs::read_to_string(ctx.paths.config_file()).unwrap();
        let original_backups = fs::read_to_string(ctx.paths.backup_file()).unwrap();

        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .config_mut()
                .manages_mut()
                .insert(ctx.managed_entry(&target));

            transaction
                .backups_mut()
                .backups_mut()
                .insert(target.clone(), ctx.backup_entry(&target));

            let error = transaction.fail("operation failed");

            assert!(matches!(
                error,
                OperationError::Operation("operation failed")
            ));

            assert!(transaction.config().manages().is_empty());
            assert!(transaction.backups().backups().is_empty());
        }

        assert_eq!(
            fs::read_to_string(ctx.paths.config_file()).unwrap(),
            original_config
        );

        assert_eq!(
            fs::read_to_string(ctx.paths.backup_file()).unwrap(),
            original_backups
        );
    }

    #[test]
    fn fail_executes_registered_filesystem_rollback() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = ctx.temp.path().join("original");
        let moved = ctx.temp.path().join("moved");

        fs::write(&original, "hello").unwrap();
        fs::rename(&original, &moved).unwrap();

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction.register(RollbackAction::Rename {
                from: moved.clone(),
                to: original.clone(),
            });

            let error = transaction.fail("operation failed");

            assert!(matches!(
                error,
                OperationError::Operation("operation failed")
            ));
        }

        assert!(original.exists());
        assert!(!moved.exists());
        assert_eq!(fs::read_to_string(original).unwrap(), "hello");
    }

    #[test]
    fn fail_returns_rollback_error_when_filesystem_rollback_fails() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let missing = ctx.temp.path().join("missing");
        let destination = ctx.temp.path().join("destination");

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        transaction.register(RollbackAction::Rename {
            from: missing,
            to: destination,
        });

        let error = transaction.fail("operation failed");

        match error {
            OperationError::Rollback {
                operation,
                rollback,
            } => {
                assert_eq!(operation, "operation failed");
                assert!(rollback.filesystem.is_some());
                assert!(rollback.config.is_none());
                assert!(rollback.backups.is_none());
            }
            OperationError::Operation(_) => {
                panic!("expected rollback failure");
            }
        }
    }

    #[test]
    fn filesystem_rollback_continues_after_one_action_fails() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = ctx.temp.path().join("original");
        let moved = ctx.temp.path().join("moved");

        fs::write(&original, "hello").unwrap();
        fs::rename(&original, &moved).unwrap();

        let missing = ctx.temp.path().join("missing");
        let impossible_destination = ctx.temp.path().join("impossible");

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        transaction.register(RollbackAction::Rename {
            from: moved.clone(),
            to: original.clone(),
        });

        transaction.register(RollbackAction::Rename {
            from: missing,
            to: impossible_destination,
        });

        let error = transaction.fail("operation failed");

        assert!(matches!(error, OperationError::Rollback { .. }));
        assert!(original.exists());
        assert!(!moved.exists());
    }

    #[test]
    fn rollback_actions_execute_in_reverse_order() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = ctx.temp.path().join("original");
        let first = ctx.temp.path().join("first");
        let second = ctx.temp.path().join("second");

        fs::write(&original, "hello").unwrap();

        fs::rename(&original, &first).unwrap();
        fs::rename(&first, &second).unwrap();

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        transaction.register(RollbackAction::Rename {
            from: first.clone(),
            to: original.clone(),
        });

        transaction.register(RollbackAction::Rename {
            from: second.clone(),
            to: first.clone(),
        });

        let error = transaction.fail("operation failed");

        assert!(matches!(
            error,
            OperationError::Operation("operation failed")
        ));

        assert!(original.exists());
        assert!(!first.exists());
        assert!(!second.exists());

        assert_eq!(fs::read_to_string(original).unwrap(), "hello");
    }

    #[test]
    fn handle_returns_success_without_rollback() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        let result: Result<u32, OperationError<&str>> = transaction.handle(Ok(42));

        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn handle_rolls_back_when_result_is_error() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let original = ctx.temp.path().join("original");
        let moved = ctx.temp.path().join("moved");

        fs::write(&original, "hello").unwrap();
        fs::rename(&original, &moved).unwrap();

        let mut transaction = OperationTransaction::new(
            &mut config,
            &config_repository,
            &mut backups,
            &backup_repository,
        );

        transaction.register(RollbackAction::Rename {
            from: moved.clone(),
            to: original.clone(),
        });

        let result: Result<(), OperationError<&str>> = transaction.handle(Err("operation failed"));

        assert!(matches!(
            result,
            Err(OperationError::Operation("operation failed"))
        ));

        assert!(original.exists());
        assert!(!moved.exists());
    }

    #[test]
    fn commit_persists_config() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .config_mut()
                .manages_mut()
                .insert(ctx.managed_entry(&target));

            transaction.commit().unwrap();
        }

        let persisted = config_repository.read().unwrap();

        assert_eq!(persisted.manages().len(), 1);
        assert!(
            persisted
                .manages()
                .iter()
                .any(|entry| { entry.points_to() == target.as_path() })
        );
    }

    #[test]
    fn commit_persists_backups() {
        let ctx = TestContext::new();
        let (mut config, mut backups) = ctx.roots();
        let (config_repository, backup_repository) = ctx.repositories();

        ctx.persist_initial_state(&config, &backups, &config_repository, &backup_repository);

        let target = ctx.temp.path().join(".zshrc");

        {
            let mut transaction = OperationTransaction::new(
                &mut config,
                &config_repository,
                &mut backups,
                &backup_repository,
            );

            transaction
                .backups_mut()
                .backups_mut()
                .insert(target.clone(), ctx.backup_entry(&target));

            transaction.commit().unwrap();
        }

        let persisted = backup_repository.read().unwrap();

        assert!(persisted.backups().contains_key(&target));
    }

    #[test]
    fn map_operation_maps_operation_error() {
        let error = OperationError::Operation("original");

        let mapped = error.map_operation(|_| 42);

        assert!(matches!(mapped, OperationError::Operation(42)));
    }

    #[test]
    fn map_operation_preserves_rollback_error() {
        let rollback = TransactionRollbackError {
            filesystem: None,
            config: None,
            backups: None,
        };

        let error = OperationError::Rollback {
            operation: "original",
            rollback,
        };

        let mapped = error.map_operation(|_| 42);

        match mapped {
            OperationError::Rollback {
                operation,
                rollback,
            } => {
                assert_eq!(operation, 42);
                assert!(rollback.filesystem.is_none());
                assert!(rollback.config.is_none());
                assert!(rollback.backups.is_none());
            }
            OperationError::Operation(_) => {
                panic!("expected rollback error");
            }
        }
    }
}
