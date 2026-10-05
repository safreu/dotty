use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError},
        persistence::paths::{DottyPaths, DottyPathsError},
    },
};

pub fn execute(target: PathBuf) -> Result<(), ForgetError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);
    layout.require_initialized()?;

    let mut manager = DotfileManager::load(&paths)?;

    manager.forget(target)?;

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
}
