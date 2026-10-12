use std::path::{Path, PathBuf};

use crate::{
    domain::{BackupKind, EntryKind, ManagedSource},
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
        managed_source: &ManagedSource,
        entry_kind: &EntryKind,
        backup_kind: BackupKind,
    ) -> Result<Vec<FileSystemAction>, BackupStorageError> {
        let backup_dir = self.root.join(managed_source.path());

        let destination = match backup_kind {
            BackupKind::Link => backup_dir.join("link"),
            BackupKind::Unlink => backup_dir.join("unlink"),
        };

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

    struct TestContext {
        _temp: TempDir,
        backup_root: PathBuf,
        target: PathBuf,
        managed_source: ManagedSource,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let backup_root = temp.path().join("backups");
            let target = temp.path().join(".zshrc");

            fs::create_dir(&backup_root).unwrap();
            fs::write(&target, "hello").unwrap();

            let managed_source = ManagedSource::new(PathBuf::from("zshrc")).unwrap();

            Self {
                _temp: temp,
                backup_root,
                target,
                managed_source,
            }
        }

        fn storage(&self) -> BackupStorage {
            BackupStorage::new(self.backup_root.clone())
        }

        fn entry_backup_dir(&self) -> PathBuf {
            self.backup_root.join("zshrc")
        }

        fn link_backup(&self) -> PathBuf {
            self.entry_backup_dir().join("link")
        }

        fn backup_actions(
            &self,
            storage: &BackupStorage,
        ) -> Result<Vec<FileSystemAction>, BackupStorageError> {
            storage.backup_actions(
                &self.target,
                &self.managed_source,
                &EntryKind::File,
                BackupKind::Link,
            )
        }
    }

    #[test]
    fn backup_actions_creates_missing_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();

        let actions = ctx.backup_actions(&storage).unwrap();

        assert!(actions.contains(&FileSystemAction::CreateDirectory(ctx.entry_backup_dir())));
    }

    #[test]
    fn backup_actions_reuses_existing_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();

        let actions = ctx.backup_actions(&storage).unwrap();

        assert!(!actions.contains(&FileSystemAction::CreateDirectory(ctx.entry_backup_dir())));
    }

    #[test]
    fn backup_actions_rejects_file_as_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();

        fs::write(ctx.entry_backup_dir(), "not a directory").unwrap();

        let result = ctx.backup_actions(&storage);

        assert!(matches!(
            result,
            Err(BackupStorageError::FileSystem(
                FileSystemError::InvalidDirectory { path }
            )) if path == ctx.entry_backup_dir()
        ));
    }

    #[test]
    fn backup_actions_rejects_existing_destination() {
        let ctx = TestContext::new();
        let storage = ctx.storage();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();
        fs::write(ctx.link_backup(), "existing backup").unwrap();

        let result = ctx.backup_actions(&storage);

        assert!(matches!(
            result,
            Err(BackupStorageError::AlreadyExists(path))
                if path == ctx.link_backup()
        ));
    }

    #[test]
    fn backup_actions_rejects_broken_symlink_as_destination() {
        let ctx = TestContext::new();
        let storage = ctx.storage();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();

        let missing_target = ctx.backup_root.join("does-not-exist");

        symlink(missing_target, ctx.link_backup()).unwrap();

        let result = ctx.backup_actions(&storage);

        assert!(matches!(
            result,
            Err(BackupStorageError::AlreadyExists(path))
                if path == ctx.link_backup()
        ));
    }
}
