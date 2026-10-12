use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
    domain::{ConfigRootError, ManagedSource, ManagedSourceError},
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError},
        persistence::{
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(target: PathBuf, source: PathBuf) -> Result<(), ManageError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);
    layout.require_initialized()?;

    let config_repository = ConfigRepository::new(paths.config_file());
    let config = config_repository.read()?;

    let managed_source = ManagedSource::new(source)?;
    let managed_target = config.to_managed_target(&target)?;

    let mut manager = DotfileManager::load(&paths)?;

    manager.manage(managed_target, managed_source)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ManageError {
    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    DotfileManager(#[from] DotfileManagerError),
    #[error(transparent)]
    Layout(#[from] DottyLayoutError),
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
    #[error(transparent)]
    ConfigRoot(#[from] ConfigRootError),
    #[error(transparent)]
    ManagedSource(#[from] ManagedSourceError),
}
