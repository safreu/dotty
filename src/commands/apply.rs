use std::io::Write;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
    infrastructure::persistence::paths::{DottyPaths, DottyPathsError},
};

pub fn execute() -> Result<(), ApplyCommandError> {
    let paths = DottyPaths::discover()?;

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    execute_with_paths(&paths, &mut writer)
}

fn execute_with_paths(
    paths: &DottyPaths,
    writer: &mut impl Write,
) -> Result<(), ApplyCommandError> {
    let manager = DotfileManager::load(paths)?;

    manager.apply()?;

    writeln!(writer, "Managed dotfiles applied successfully.")?;

    Ok(())
}
#[derive(Debug, thiserror::Error)]
pub enum ApplyCommandError {
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    DottyPaths(#[from] DottyPathsError),
    #[error(transparent)]
    DotfileManager(#[from] DotfileManagerError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
