use std::{fs, os::unix::fs::symlink, path::PathBuf};

use crate::{
    config::{ConfigFileError, ConfigRoot, EntryKind, ManagedEntry, ManagedEntryError},
    handlers::path_handler::{self, PathError},
};

// Creates a backup, moves it to dotfiles_dir, writes data into config about the managed file, and symlinks it
pub fn link_file(target: PathBuf, config_file: &mut ConfigRoot) -> Result<(), LinkError> {
    path_handler::create_dir(&config_file.config().backup_dir())?;
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    let entry = ManagedEntry::new(
        &target,
        &config_file.config().backup_dir(),
        config_file.config().dotfiles_dir(),
        EntryKind::File,
    )?;

    fs::copy(&target, entry.backed_up_at()).map_err(|_| LinkError::Copy)?;

    fs::rename(&target, entry.stored_at()).map_err(|_| LinkError::Move)?;

    symlink(entry.stored_at(), entry.points_to()).map_err(|_| LinkError::Symlink)?;

    config_file.manages_mut().push(entry);

    config_file.write()?;

    Ok(())
}

// Creates a backup, moves it to dotfiles_dir, writes data into config about the managed directory, and symlinks it
pub fn link_dir(target: PathBuf, config_file: &mut ConfigRoot) -> Result<(), LinkError> {
    path_handler::create_dir(&config_file.config().backup_dir())?;
    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    let entry = ManagedEntry::new(
        &target,
        &config_file.config().backup_dir(),
        config_file.config().dotfiles_dir(),
        EntryKind::Dir,
    )?;

    path_handler::copy_dir(&target, entry.backed_up_at()).map_err(|_| LinkError::Copy)?;

    fs::rename(&target, entry.stored_at()).map_err(|_| LinkError::Move)?;

    symlink(entry.stored_at(), entry.points_to()).map_err(|_| LinkError::Symlink)?;

    config_file.manages_mut().push(entry);

    config_file.write()?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error("Target file doesn't have a filename")]
    FilenameRequired(PathBuf),
    #[error(transparent)]
    PathCreationFailed(#[from] PathError),
    #[error("Failed to create backup")]
    Copy,
    #[error("Failed to move target")]
    Move,
    #[error("Failed to symlink target")]
    Symlink,
    #[error(transparent)]
    EntryCreationFailed(#[from] ManagedEntryError),
    #[error(transparent)]
    ConfigWriteFailed(#[from] ConfigFileError),
}
