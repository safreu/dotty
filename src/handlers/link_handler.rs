use std::{collections::hash_map, fs, os::unix::fs::symlink, path::PathBuf};

use crate::{
    backups::{BackupEntry, BackupEntryError, BackupFileError, BackupKind, BackupRoot},
    config::{ConfigFileError, ConfigRoot, EntryKind, ManagedEntry, ManagedEntryError},
    handlers::{
        backup_handler,
        path_handler::{self, PathError},
    },
};

// Creates a backup, moves it to dotfiles_dir, writes data into config about the managed file, and symlinks it
pub fn link_file(
    target: PathBuf,
    config_file: &mut ConfigRoot,
    backup_file: &mut BackupRoot,
) -> Result<(), LinkError> {
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    let entry = ManagedEntry::new(
        &target,
        config_file.config().dotfiles_dir(),
        EntryKind::File,
    )?;

    let backup_entry = match backup_file.backups_mut().entry(target.clone()) {
        hash_map::Entry::Occupied(entry) => {
            let entry = entry.into_mut();

            entry.backups_mut().insert(BackupKind::Link);

            entry
        }
        hash_map::Entry::Vacant(entry) => {
            let backup_entry = BackupEntry::new(
                &target,
                config_file.config().backups_dir(),
                EntryKind::File,
                BackupKind::Link,
            )?;

            entry.insert(backup_entry)
        }
    };

    backup_handler::backup_file(&entry, backup_entry, BackupKind::Link)
        .map_err(|_| LinkError::Backup)?;

    fs::rename(&target, entry.stored_at()).map_err(|_| LinkError::Move)?;

    symlink(entry.stored_at(), entry.points_to()).map_err(|_| LinkError::Symlink)?;

    config_file.manages_mut().insert(entry);

    config_file.write()?;
    backup_file.write()?;

    Ok(())
}

pub fn unlink_file(
    target: PathBuf,
    config_file: &mut ConfigRoot,
    backup_file: &mut BackupRoot,
) -> Result<(), LinkError> {
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    if !target.is_symlink() {
        return Err(LinkError::Unmanaged(target.clone()));
    }

    let entry = config_file
        .manages()
        .iter()
        .find(|entry| entry.points_to() == &target)
        .cloned()
        .ok_or_else(|| LinkError::Unmanaged(target.clone()))?;

    let backup_entry = backup_file
        .backups_mut()
        .get_mut(&target)
        .ok_or_else(|| LinkError::Unmanaged(target.clone()))?;

    backup_entry.backups_mut().insert(BackupKind::Unlink);

    backup_handler::backup_file(&entry, backup_entry, BackupKind::Unlink)
        .map_err(|_| LinkError::Backup)?;

    fs::remove_file(&target).map_err(|_| LinkError::DeleteSymlink)?;

    fs::rename(entry.stored_at(), &target).map_err(|_| LinkError::Move)?;

    config_file.manages_mut().remove(&entry);

    config_file.write()?;
    backup_file.write()?;

    Ok(())
}

// Creates a backup, moves it to dotfiles_dir, writes data into config about the managed directory, and symlinks it
pub fn link_dir(
    target: PathBuf,
    config_file: &mut ConfigRoot,
    backup_file: &mut BackupRoot,
) -> Result<(), LinkError> {
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    let entry = ManagedEntry::new(&target, config_file.config().dotfiles_dir(), EntryKind::Dir)?;

    let backup_entry = match backup_file.backups_mut().entry(target.clone()) {
        hash_map::Entry::Occupied(entry) => {
            let entry = entry.into_mut();

            entry.backups_mut().insert(BackupKind::Link);

            entry
        }
        hash_map::Entry::Vacant(entry) => {
            let backup_entry = BackupEntry::new(
                &target,
                config_file.config().backups_dir(),
                EntryKind::Dir,
                BackupKind::Link,
            )?;

            entry.insert(backup_entry)
        }
    };

    backup_handler::backup_dir(&entry, backup_entry, BackupKind::Link)
        .map_err(|_| LinkError::Backup)?;

    fs::rename(&target, entry.stored_at()).map_err(|_| LinkError::Move)?;

    symlink(entry.stored_at(), entry.points_to()).map_err(|_| LinkError::Symlink)?;

    config_file.manages_mut().insert(entry);

    config_file.write()?;
    backup_file.write()?;

    Ok(())
}

pub fn unlink_dir(
    target: PathBuf,
    config_file: &mut ConfigRoot,
    backup_file: &mut BackupRoot,
) -> Result<(), LinkError> {
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    if !target.is_symlink() {
        return Err(LinkError::Unmanaged(target.clone()));
    }

    let entry = config_file
        .manages()
        .iter()
        .find(|entry| entry.points_to() == &target)
        .cloned()
        .ok_or_else(|| LinkError::Unmanaged(target.clone()))?;

    let backup_entry = backup_file
        .backups_mut()
        .get_mut(&target)
        .ok_or_else(|| LinkError::Unmanaged(target.clone()))?;

    backup_entry.backups_mut().insert(BackupKind::Unlink);

    backup_handler::backup_dir(&entry, backup_entry, BackupKind::Unlink)
        .map_err(|_| LinkError::Backup)?;

    fs::remove_file(&target).map_err(|_| LinkError::DeleteSymlink)?;

    fs::rename(entry.stored_at(), &target).map_err(|_| LinkError::Move)?;

    config_file.manages_mut().remove(&entry);

    config_file.write()?;
    backup_file.write()?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error(transparent)]
    PathCreation(#[from] PathError),
    #[error(transparent)]
    BackupEntryCreation(#[from] BackupEntryError),
    #[error("Failed to create a backup")]
    Backup,
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
    ConfigWrite(#[from] ConfigFileError),
    #[error(transparent)]
    BackupWrite(#[from] BackupFileError),
}
