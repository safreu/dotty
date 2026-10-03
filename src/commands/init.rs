use std::{env, path::PathBuf};

use crate::{
    backups::{BackupFileError, BackupRoot},
    config::{ConfigFileError, ConfigRoot},
    handlers::{
        path_handler::{self, PathError},
        storage_handler::StorageHandler,
    },
};

pub fn execute(path: PathBuf) -> Result<(), InitError> {
    let dir = match env::var("XDG_CONFIG_HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => PathBuf::from(env::var("HOME")?).join(".config"),
    };

    let config_dir = dir.join("dotty");

    if config_dir.exists() {
        return Err(InitError::AlreadyInit);
    }

    path_handler::create_dir(&config_dir)?;

    let paths = StorageHandler::discover()?;

    let config_file = ConfigRoot::new(path, &paths);

    path_handler::create_dir(&paths.backup_dir())?;
    let backup_file = BackupRoot::new(paths.backup_file());

    config_file.write()?;
    backup_file.write()?;

    path_handler::create_dir(config_file.config().dotfiles_dir())?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error(transparent)]
    MissingEnvVariable(#[from] env::VarError),
    #[error(transparent)]
    ConfigurationWritingFailed(#[from] ConfigFileError),
    #[error(transparent)]
    BackupWritingFailed(#[from] BackupFileError),
    #[error("Dotty is already initialized")]
    AlreadyInit,
    #[error(transparent)]
    PathCreationFailed(#[from] PathError),
}
