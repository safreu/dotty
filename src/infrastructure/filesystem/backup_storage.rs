use std::path::PathBuf;

use crate::{
    domain::{BackupKind, EntryKind, ManagedEntry},
    infrastructure::filesystem::FileSystemAction,
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

        if !backup_dir.exists() {
            actions.push(FileSystemAction::CreateDirectory(backup_dir.clone()));
        }

        if destination.exists() {
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
}
