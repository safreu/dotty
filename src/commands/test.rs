use std::{
    io::Write,
    path::{Component, PathBuf},
};

use crate::{
    application::{DotfileManager, DotfileManagerLoadError, InspectionError},
    domain::ManagedEntryInspection,
    infrastructure::persistence::paths::{DottyPaths, DottyPathsError},
    presentation::print_inspections,
};

pub fn execute(path: Option<PathBuf>) -> Result<bool, TestCommandError> {
    let paths = DottyPaths::discover()?;

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    execute_with_paths(path, &paths, &mut writer)
}

fn execute_with_paths(
    path: Option<PathBuf>,
    paths: &DottyPaths,
    writer: &mut impl Write,
) -> Result<bool, TestCommandError> {
    let manager = DotfileManager::load(paths)?;
    let inspections = match path {
        Some(target) => {
            let relative = target
                .strip_prefix(paths.home_dir())
                .map_err(|_| TestCommandError::InvalidTarget)?;

            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|component| matches!(component, Component::ParentDir))
            {
                return Err(TestCommandError::InvalidTarget);
            };

            let inspection = manager
                .inspect_target(relative)?
                .ok_or(TestCommandError::TargetNotManaged)?;

            vec![inspection]
        }
        None => manager.inspect()?,
    };

    print_inspections(writer, &inspections)?;

    Ok(inspections.iter().all(ManagedEntryInspection::is_healthy))
}

#[derive(Debug, thiserror::Error)]
pub enum TestCommandError {
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    Inspection(#[from] InspectionError),
    #[error(transparent)]
    DottyPaths(#[from] DottyPathsError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Target must be inside the home directory")]
    InvalidTarget,
    #[error("Target is not managed by Dotty")]
    TargetNotManaged,
}

#[cfg(test)]
#[path = "tests/test.rs"]
mod tests;
