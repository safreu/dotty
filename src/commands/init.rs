use std::path::PathBuf;

use crate::{
    domain::{BackupRoot, ConfigRoot},
    infrastructure::{
        filesystem::{self, FileSystemError},
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(path: PathBuf) -> Result<(), InitError> {
    let paths = DottyPaths::discover()?;

    if paths.config_dir().exists() {
        return Err(InitError::AlreadyInit);
    }

    filesystem::create_dir(paths.config_dir())?;
    filesystem::create_dir(&paths.backup_dir())?;

    let config = ConfigRoot::new(path);
    let backups = BackupRoot::new();

    let config_repository = ConfigRepository::new(paths.config_file());
    let backups_repository = BackupRepository::new(paths.backup_file());

    config_repository.write(&config)?;
    backups_repository.write(&backups)?;

    filesystem::create_dir(config.config().dotfiles_dir())?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error(transparent)]
    ConfigurationWritingFailed(#[from] ConfigRepositoryError),
    #[error(transparent)]
    BackupWritingFailed(#[from] BackupRepositoryError),
    #[error("Dotty is already initialized")]
    AlreadyInit,
    #[error(transparent)]
    PathCreationFailed(#[from] FileSystemError),

    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
}
