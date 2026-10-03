use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError, OperationError},
    infrastructure::persistence::paths::{DottyPaths, DottyPathsError},
};

pub fn execute(target: PathBuf) -> Result<(), ForgetError> {
    let paths = DottyPaths::discover()?;

    let mut manager = DotfileManager::load(&paths)?;

    if target.is_dir() || target.is_file() {
        manager.forget(target)?;
    } else {
        return Err(ForgetError::UnknownFileType);
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ForgetError {
    #[error("The filetype is not supported")]
    UnknownFileType,

    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    Operation(#[from] OperationError<DotfileManagerError>),
}
