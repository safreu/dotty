use std::io::Write;

use crate::{
    application::{DotfileManager, DotfileManagerLoadError, InspectionError},
    infrastructure::persistence::paths::{DottyPaths, DottyPathsError},
    presentation::print_entries,
};

pub fn execute() -> Result<(), LsCommandError> {
    let paths = DottyPaths::discover()?;

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    execute_with_paths(&paths, &mut writer)
}

fn execute_with_paths(paths: &DottyPaths, writer: &mut impl Write) -> Result<(), LsCommandError> {
    let manager = DotfileManager::load(paths)?;
    let inspections = manager.inspect()?;

    print_entries(writer, &inspections)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LsCommandError {
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    Inspection(#[from] InspectionError),
    #[error(transparent)]
    DottyPaths(#[from] DottyPathsError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
#[path = "tests/ls.rs"]
mod tests;
