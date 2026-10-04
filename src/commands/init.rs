use std::path::PathBuf;

use crate::{
    domain::{BackupRoot, ConfigRoot},
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError, FileSystemError},
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(repo_dir: PathBuf) -> Result<(), InitError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);

    if layout.is_initialized() {
        return Err(InitError::AlreadyInit);
    }

    layout.initialize(&repo_dir)?;

    let config = ConfigRoot::new(repo_dir);
    let backups = BackupRoot::new();

    let config_repository = ConfigRepository::new(paths.config_file());
    let backups_repository = BackupRepository::new(paths.backup_file());

    config_repository.write(&config)?;
    backups_repository.write(&backups)?;

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
    #[error(transparent)]
    Layout(#[from] DottyLayoutError),
}
