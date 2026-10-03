use std::{fs, path::PathBuf};

use crate::{
    domain::{
        BackupKind,
        EntryKind,
        ManagedEntry,
    },
    infrastructure::filesystem::{FileSystemError, copy_dir},
};

pub struct BackupStorage {
    root: PathBuf,
}

impl BackupStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn backup(
        &self,
        entry: &ManagedEntry,
        backup_kind: BackupKind,
    ) -> Result<(), FileSystemError> {
        let backup_dir = self.root.join(entry.managed_filename());

        fs::create_dir_all(&backup_dir)?;

        let suffix = match backup_kind {
            BackupKind::Link => "link",
            BackupKind::Unlink => "unlink",
        };

        let source = match backup_kind {
            BackupKind::Link => entry.points_to(),
            BackupKind::Unlink => entry.stored_at(),
        };

        let destination = backup_dir.join(format!("{}.bak.{suffix}", entry.managed_filename()));

        match entry.kind() {
            EntryKind::File => {
                fs::copy(source, &destination)?;
            }
            EntryKind::Dir => {
                copy_dir(source, &destination)?;
            }
        };

        Ok(())
    }
}
