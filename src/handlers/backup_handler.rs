use std::fs;

use crate::{
    backups::{BackupEntry, BackupKind},
    config::ManagedEntry,
    handlers::path_handler::{self, PathError},
};

pub fn backup_file(
    entry: &ManagedEntry,
    backup_entry: &BackupEntry,
    backup_kind: BackupKind,
) -> Result<(), BackupError> {
    path_handler::create_dir(backup_entry.backup_dir())?;

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

pub fn backup_dir(
    entry: &ManagedEntry,
    backup_entry: &BackupEntry,
    backup_kind: BackupKind,
) -> Result<(), BackupError> {
    path_handler::create_dir(backup_entry.backup_dir())?;

    let suffix = match backup_kind {
        BackupKind::Link => "link",
        BackupKind::Unlink => "unlink",
    };

    let source = match backup_kind {
        BackupKind::Link => entry.points_to(),
        BackupKind::Unlink => entry.stored_at(),
    };

    let name = format!("{}.bak.{suffix}", entry.managed_filename());
    path_handler::copy_dir(source, &backup_entry.backup_dir().join(name))
        .map_err(BackupError::PathCreationFailed)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error(transparent)]
    PathCreationFailed(#[from] PathError),
    #[error("Failed to create backup")]
    FileCopy(#[source] std::io::Error),
}
