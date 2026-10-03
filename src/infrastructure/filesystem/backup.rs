use std::fs;

use crate::{
    domain::{BackupEntry, BackupKind, EntryKind, ManagedEntry},
    infrastructure::filesystem::{self, FileSystemError},
};

pub fn backup(
    entry: &ManagedEntry,
    backup_entry: &BackupEntry,
    backup_kind: BackupKind,
) -> Result<(), BackupError> {
    match entry.kind() {
        EntryKind::Dir => backup_dir(entry, backup_entry, backup_kind),
        EntryKind::File => backup_file(entry, backup_entry, backup_kind),
    }
}

fn backup_file(
    entry: &ManagedEntry,
    backup_entry: &BackupEntry,
    backup_kind: BackupKind,
) -> Result<(), BackupError> {
    filesystem::create_dir(backup_entry.backup_dir())?;

    let suffix = match backup_kind {
        BackupKind::Link => "link",
        BackupKind::Unlink => "unlink",
    };

    let source = match backup_kind {
        BackupKind::Link => entry.points_to(),
        BackupKind::Unlink => entry.stored_at(),
    };

    let name = format!("{}.bak.{suffix}", entry.managed_filename());

    fs::copy(source, backup_entry.backup_dir().join(name)).map_err(BackupError::FileCopy)?;

    Ok(())
}

fn backup_dir(
    entry: &ManagedEntry,
    backup_entry: &BackupEntry,
    backup_kind: BackupKind,
) -> Result<(), BackupError> {
    filesystem::create_dir(backup_entry.backup_dir())?;

    let suffix = match backup_kind {
        BackupKind::Link => "link",
        BackupKind::Unlink => "unlink",
    };

    let source = match backup_kind {
        BackupKind::Link => entry.points_to(),
        BackupKind::Unlink => entry.stored_at(),
    };

    let name = format!("{}.bak.{suffix}", entry.managed_filename());
    filesystem::copy_dir(source, &backup_entry.backup_dir().join(name))
        .map_err(BackupError::PathCreationFailed)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error(transparent)]
    PathCreationFailed(#[from] FileSystemError),
    #[error("Failed to create backup")]
    FileCopy(#[source] std::io::Error),
}
