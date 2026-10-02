use std::path::PathBuf;

use crate::{
    commands::init::InitError,
    config::{ConfigFileError, ConfigRoot},
    handlers::{
        link_handler::{self, LinkError},
        storage_handler::StorageHandler,
    },
};

pub fn execute(target: PathBuf) -> Result<(), ManageError> {
    let paths = StorageHandler::discover()?;

    let mut config_file =
        ConfigRoot::read(&paths).map_err(ManageError::ConfigurationReadingFailed)?;

    if target.is_dir() {
        link_handler::unlink_dir(target, &mut config_file)?;
    } else if target.is_file() {
        link_handler::unlink_file(target, &mut config_file)?;
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
    #[error(transparent)]
    PathDiscovery(#[from] InitError),
    #[error(transparent)]
    Linking(#[from] LinkError),
}
