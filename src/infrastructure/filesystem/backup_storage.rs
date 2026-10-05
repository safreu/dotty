use std::path::PathBuf;

use crate::{
    domain::{BackupKind, EntryKind, ManagedEntry},
    infrastructure::filesystem::{self, FileSystemAction, FileSystemError, PathKind},
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
        entry: &ManagedEntry,
        backup_kind: BackupKind,
    ) -> Result<Vec<FileSystemAction>, BackupStorageError> {
        let backup_dir = self.root.join(entry.managed_filename());

        let suffix = match backup_kind {
            BackupKind::Link => "link",
            BackupKind::Unlink => "unlink",
        };

        let source = match backup_kind {
            BackupKind::Link => entry.points_to(),
            BackupKind::Unlink => entry.stored_at(),
        };

        let destination = backup_dir.join(format!("{}.bak.{suffix}", entry.managed_filename()));

        let mut actions = Vec::new();

        match filesystem::path_kind(&backup_dir)? {
            None => actions.push(FileSystemAction::CreateDirectory(backup_dir.clone())),
            Some(PathKind::Directory) => {}
            Some(_) => return Err(BackupStorageError::InvalidBackupDirectory(backup_dir)),
        }

        if filesystem::path_kind(&destination)?.is_some() {
            return Err(BackupStorageError::AlreadyExists(destination));
        }

        match entry.kind() {
            EntryKind::File => {
                actions.push(FileSystemAction::CopyFile {
                    from: source.to_path_buf(),
                    to: destination,
                });
            }
            EntryKind::Dir => {
                actions.push(FileSystemAction::CopyDirectory {
                    from: source.to_path_buf(),
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
    #[error("backup directory is not a directory: `{0}`")]
    InvalidBackupDirectory(PathBuf),
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
        dotfiles_dir: PathBuf,
        target: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let backup_root = temp.path().join("backups");
            let dotfiles_dir = temp.path().join("dotfiles");
            let target = temp.path().join(".zshrc");

            fs::create_dir(&backup_root).unwrap();
            fs::create_dir(&dotfiles_dir).unwrap();
            fs::write(&target, "hello").unwrap();

            Self {
                _temp: temp,
                backup_root,
                dotfiles_dir,
                target,
            }
        }

        fn storage(&self) -> BackupStorage {
            BackupStorage::new(self.backup_root.clone())
        }

        fn entry(&self) -> ManagedEntry {
            ManagedEntry::new(&self.target, &self.dotfiles_dir, &EntryKind::File).unwrap()
        }

        fn entry_backup_dir(&self) -> PathBuf {
            self.backup_root.join("zshrc")
        }

        fn link_backup(&self) -> PathBuf {
            self.entry_backup_dir().join("zshrc.bak.link")
        }
    }

    #[test]
    fn backup_actions_creates_missing_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();
        let entry = ctx.entry();

        let actions = storage.backup_actions(&entry, BackupKind::Link).unwrap();

        assert!(actions.contains(&FileSystemAction::CreateDirectory(ctx.entry_backup_dir())));
    }

    #[test]
    fn backup_actions_reuses_existing_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();
        let entry = ctx.entry();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();

        let actions = storage.backup_actions(&entry, BackupKind::Link).unwrap();

        assert!(!actions.contains(&FileSystemAction::CreateDirectory(ctx.entry_backup_dir())));
    }

    #[test]
    fn backup_actions_rejects_file_as_backup_directory() {
        let ctx = TestContext::new();
        let storage = ctx.storage();
        let entry = ctx.entry();

        fs::write(ctx.entry_backup_dir(), "not a directory").unwrap();

        let result = storage.backup_actions(&entry, BackupKind::Link);

        assert!(matches!(
            result,
            Err(BackupStorageError::InvalidBackupDirectory(path))
                if path == ctx.entry_backup_dir()
        ));
    }

    #[test]
    fn backup_actions_rejects_existing_destination() {
        let ctx = TestContext::new();
        let storage = ctx.storage();
        let entry = ctx.entry();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();
        fs::write(ctx.link_backup(), "existing backup").unwrap();

        let result = storage.backup_actions(&entry, BackupKind::Link);

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
        let entry = ctx.entry();

        fs::create_dir(ctx.entry_backup_dir()).unwrap();

        let missing_target = ctx.backup_root.join("does-not-exist");
        symlink(missing_target, ctx.link_backup()).unwrap();

        let result = storage.backup_actions(&entry, BackupKind::Link);

        assert!(matches!(
            result,
            Err(BackupStorageError::AlreadyExists(path))
                if path == ctx.link_backup()
        ));
    }
}
