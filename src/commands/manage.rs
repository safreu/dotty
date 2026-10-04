use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError, OperationError},
    domain::EntryKind,
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError},
        persistence::paths::{DottyPaths, DottyPathsError},
    },
};

// TODO: Handle different filetypes
pub fn execute(target: PathBuf) -> Result<(), ManageError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);
    layout.require_initialized()?;

    let mut manager = DotfileManager::load(&paths)?;

    if target.is_dir() {
        manager.manage(target, EntryKind::Dir)?;
    } else if target.is_file() {
        manager.manage(target, EntryKind::File)?;
    } else {
        return Err(ManageError::UnknownFileType);
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ManageError {
    #[error("The filetype is not supported")]
    UnknownFileType,

    #[error(transparent)]
    PathDiscovery(#[from] DottyPathsError),
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    Operation(#[from] OperationError<DotfileManagerError>),
    #[error(transparent)]
    Layout(#[from] DottyLayoutError),
}
