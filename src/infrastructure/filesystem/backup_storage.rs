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
#[path = "tests/backup_storage.rs"]
mod tests;
