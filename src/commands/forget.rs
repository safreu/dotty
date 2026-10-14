use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
    domain::ConfigRootError,
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError},
        persistence::{
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(target: PathBuf) -> Result<(), ForgetError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);
    layout.require_initialized()?;

    let config_repository = ConfigRepository::new(paths.config_file());
    let config = config_repository.read()?;

    let managed_target = config.to_managed_target(&target)?;

    let mut manager = DotfileManager::load(&paths)?;

    manager.forget(managed_target)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ForgetError {
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
}
