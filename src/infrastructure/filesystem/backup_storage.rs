use std::path::{Path, PathBuf};

use crate::{
    domain::{BackupId, EntryKind},
    infrastructure::filesystem::{self, FileSystemAction, FileSystemError},
};

pub struct BackupStorage {
    root: PathBuf,
}

impl BackupStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn backup_actions(
        &self,
        from: &Path,
        backup_id: BackupId,
        entry_kind: EntryKind,
    ) -> Result<Vec<FileSystemAction>, BackupStorageError> {
        let backup_dir = self.root.join("operations").join(backup_id.to_string());

        let destination = backup_dir.join("snapshot");

        let mut actions = Vec::new();

        let missing = filesystem::missing_directories(&self.root, &backup_dir)?;

        for directory in missing {
            actions.push(FileSystemAction::CreateDirectory(directory));
        }
        if filesystem::path_kind(&destination)?.is_some() {
            return Err(BackupStorageError::AlreadyExists(destination));
        }

        match entry_kind {
            EntryKind::File => {
                actions.push(FileSystemAction::CopyFile {
                    from: from.to_path_buf(),
                    to: destination,
                });
            }
            EntryKind::Dir => {
                actions.push(FileSystemAction::CopyDirectory {
                    from: from.to_path_buf(),
                    to: destination,
                });
            }
        }

        Ok(actions)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BackupStorageError {
    #[error("backup destination already exists: {0}")]
    AlreadyExists(PathBuf),
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::symlink};

    use tempfile::TempDir;

    use super::*;
    use crate::application::{Plan, PlanExecutionError, PlanExecutor};

    struct TestContext {
        _temp: TempDir,
        backup_root: PathBuf,
        target: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let backup_root = temp.path().join("backups");
            let target = temp.path().join(".zshrc");

            fs::create_dir(&backup_root).unwrap();
            fs::write(&target, "hello").unwrap();

            Self {
                _temp: temp,
                backup_root,
                target,
            }
        }

        fn storage(&self) -> BackupStorage {
            BackupStorage::new(self.backup_root.clone())
        }

        fn operations_dir(&self) -> PathBuf {
            self.backup_root.join("operations")
        }

        fn operation_dir(&self, id: BackupId) -> PathBuf {
            self.operations_dir().join(id.to_string())
        }

        fn snapshot(&self, id: BackupId) -> PathBuf {
            self.operation_dir(id).join("snapshot")
        }

        fn backup_actions(
            &self,
            id: BackupId,
        ) -> Result<Vec<FileSystemAction>, BackupStorageError> {
            self.storage()
                .backup_actions(&self.target, id, EntryKind::File)
        }

        fn execute_backup(&self, id: BackupId) {
            let actions = self.backup_actions(id).unwrap();

            let mut plan = Plan::new();

            for action in actions {
                plan.push(action.into());
            }

            PlanExecutor::execute(plan).unwrap();
        }
    }

    #[test]
    fn backup_actions_creates_missing_operations_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        let actions = ctx.backup_actions(id).unwrap();

        assert!(actions.contains(&FileSystemAction::CreateDirectory(ctx.operations_dir())));
    }

    #[test]
    fn backup_actions_creates_missing_operation_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        let actions = ctx.backup_actions(id).unwrap();

        assert!(actions.contains(&FileSystemAction::CreateDirectory(ctx.operation_dir(id))));
    }

    #[test]
    fn backup_actions_reuses_existing_operations_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir(ctx.operations_dir()).unwrap();

        let actions = ctx.backup_actions(id).unwrap();

        assert!(!actions.contains(&FileSystemAction::CreateDirectory(ctx.operations_dir())));

        assert!(actions.contains(&FileSystemAction::CreateDirectory(ctx.operation_dir(id))));
    }

    #[test]
    fn backup_actions_reuses_existing_operation_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir_all(ctx.operation_dir(id)).unwrap();

        let actions = ctx.backup_actions(id).unwrap();

        assert!(!actions.contains(&FileSystemAction::CreateDirectory(ctx.operation_dir(id))));

        assert!(actions.contains(&FileSystemAction::CopyFile {
            from: ctx.target.clone(),
            to: ctx.snapshot(id),
        }));
    }

    #[test]
    fn backup_actions_rejects_file_as_operations_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::write(ctx.operations_dir(), "not a directory").unwrap();

        let result = ctx.backup_actions(id);

        assert!(matches!(
            result,
            Err(BackupStorageError::FileSystem(
                FileSystemError::InvalidDirectory { path }
            )) if path == ctx.operations_dir()
        ));
    }

    #[test]
    fn backup_actions_rejects_file_as_operation_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir(ctx.operations_dir()).unwrap();
        fs::write(ctx.operation_dir(id), "not a directory").unwrap();

        let result = ctx.backup_actions(id);

        assert!(matches!(
            result,
            Err(BackupStorageError::FileSystem(
                FileSystemError::InvalidDirectory { path }
            )) if path == ctx.operation_dir(id)
        ));
    }

    #[test]
    fn backup_actions_rejects_existing_snapshot() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir_all(ctx.operation_dir(id)).unwrap();
        fs::write(ctx.snapshot(id), "existing backup").unwrap();

        let result = ctx.backup_actions(id);

        assert!(matches!(
            result,
            Err(BackupStorageError::AlreadyExists(path))
                if path == ctx.snapshot(id)
        ));
    }

    #[test]
    fn backup_actions_rejects_broken_symlink_as_destination() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir_all(ctx.operation_dir(id)).unwrap();

        let missing_target = ctx.backup_root.join("does-not-exist");

        symlink(missing_target, ctx.snapshot(id)).unwrap();

        let result = ctx.backup_actions(id);

        assert!(matches!(
            result,
            Err(BackupStorageError::AlreadyExists(path))
                if path == ctx.snapshot(id)
        ));
    }

    #[test]
    fn backup_actions_copies_file_to_operation_snapshot() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        ctx.execute_backup(id);

        assert!(ctx.snapshot(id).is_file());
        assert_eq!(fs::read_to_string(ctx.snapshot(id)).unwrap(), "hello");

        assert!(ctx.target.is_file());
        assert_eq!(fs::read_to_string(&ctx.target).unwrap(), "hello");
    }

    #[test]
    fn backup_actions_supports_multiple_operations_for_same_source() {
        let ctx = TestContext::new();

        let first_id = BackupId::new();
        let second_id = BackupId::new();

        ctx.execute_backup(first_id);

        fs::write(&ctx.target, "updated content").unwrap();

        ctx.execute_backup(second_id);

        assert_ne!(ctx.operation_dir(first_id), ctx.operation_dir(second_id));

        assert_eq!(fs::read_to_string(ctx.snapshot(first_id)).unwrap(), "hello");

        assert_eq!(
            fs::read_to_string(ctx.snapshot(second_id)).unwrap(),
            "updated content"
        );
    }

    #[test]
    fn backup_actions_copies_directory_to_operation_snapshot() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        let directory = ctx._temp.path().join("nvim");
        let nested = directory.join("lua");

        fs::create_dir_all(&nested).unwrap();
        fs::write(directory.join("init.lua"), "vim.opt.number = true").unwrap();
        fs::write(nested.join("config.lua"), "return {}").unwrap();

        let actions = ctx
            .storage()
            .backup_actions(&directory, id, EntryKind::Dir)
            .unwrap();

        let mut plan = Plan::new();

        for action in actions {
            plan.push(action.into());
        }

        PlanExecutor::execute(plan).unwrap();

        let snapshot = ctx.snapshot(id);

        assert!(snapshot.is_dir());

        assert_eq!(
            fs::read_to_string(snapshot.join("init.lua")).unwrap(),
            "vim.opt.number = true"
        );

        assert_eq!(
            fs::read_to_string(snapshot.join("lua/config.lua")).unwrap(),
            "return {}"
        );

        assert!(directory.is_dir());
    }

    #[test]
    fn backup_actions_rolls_back_new_directories_when_later_action_fails() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        let actions = ctx.backup_actions(id).unwrap();

        let mut plan = Plan::new();

        for action in actions {
            plan.push(action.into());
        }

        let missing = ctx._temp.path().join("nonexistent-file");

        plan.push(FileSystemAction::RemoveFile(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(!ctx.snapshot(id).exists());
        assert!(!ctx.operation_dir(id).exists());
        assert!(!ctx.operations_dir().exists());

        assert!(ctx.backup_root.is_dir());
        assert!(ctx.target.is_file());

        assert_eq!(fs::read_to_string(&ctx.target).unwrap(), "hello");
    }

    #[test]
    fn backup_actions_rollback_preserves_existing_operations_directory() {
        let ctx = TestContext::new();
        let id = BackupId::new();

        fs::create_dir(ctx.operations_dir()).unwrap();

        let actions = ctx.backup_actions(id).unwrap();

        let mut plan = Plan::new();

        for action in actions {
            plan.push(action.into());
        }

        plan.push(FileSystemAction::RemoveFile(ctx._temp.path().join("nonexistent-file")).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(ctx.operations_dir().is_dir());
        assert!(!ctx.operation_dir(id).exists());
        assert!(!ctx.snapshot(id).exists());
    }
}
