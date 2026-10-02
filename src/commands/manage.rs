use std::path::PathBuf;

use crate::{
    backups::{BackupFileError, BackupRoot},
    commands::init::InitError,
    config::{ConfigFileError, ConfigRoot},
    handlers::{
        link_handler::{self, LinkError},
        storage_handler::StorageHandler,
    },
};

// TODO: Handle different filetypes
pub fn execute(target: PathBuf) -> Result<(), ManageError> {
    let paths = StorageHandler::discover()?;

    let mut config_file =
        ConfigRoot::read(&paths).map_err(ManageError::ConfigurationReadingFailed)?;

    let mut backup_file = BackupRoot::read(&paths).map_err(ManageError::BackupReadingFailed)?;

    if target.is_dir() {
        link_handler::link_dir(target, &mut config_file, &mut backup_file)?;
    } else if target.is_file() {
        link_handler::link_file(target, &mut config_file, &mut backup_file)?;
    } else {
        return Err(ManageError::UnknownFileType);
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ManageError {
    #[error("The filetype is not supported")]
    UnknownFileType,
    #[error("Configuration File couldn't be read")]
    ConfigurationReadingFailed(#[source] ConfigFileError),
    #[error("Backup File couldn't be read")]
    BackupReadingFailed(#[source] BackupFileError),
    #[error(transparent)]
    PathDiscovery(#[from] InitError),
    #[error(transparent)]
    Linking(#[from] LinkError),
}
