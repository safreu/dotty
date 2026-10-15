use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
    commands::resolve_target_path,
    domain::{ConfigRootError, ManagedSource, ManagedSourceError, ManagedTarget},
    infrastructure::{
        filesystem::{DottyLayout, DottyLayoutError},
        persistence::{
            config::{ConfigRepository, ConfigRepositoryError},
            paths::{DottyPaths, DottyPathsError},
        },
    },
};

pub fn execute(target: PathBuf, source: Option<PathBuf>) -> Result<(), ManageError> {
    let paths = DottyPaths::discover()?;

    let layout = DottyLayout::new(&paths);
    layout.require_initialized()?;

    let config_repository = ConfigRepository::new(paths.config_file());
    let config = config_repository.read()?;

    let cwd = std::env::current_dir().map_err(ManageError::CurrentDir)?;

    let resolved_target = resolve_target_path(&target, &cwd);

    let managed_target = config.to_managed_target(&resolved_target)?;

    let source = source.unwrap_or_else(|| default_source(&managed_target));

    let managed_source = ManagedSource::new(source)?;

    let mut manager = DotfileManager::load(&paths)?;

    manager.manage(managed_target, managed_source)?;

    Ok(())
}

fn default_source(target: &ManagedTarget) -> PathBuf {
    let path = target.path();

    let path = path
        .strip_prefix(".config")
        .ok()
        .filter(|remaining| !remaining.as_os_str().is_empty())
        .unwrap_or(path);

    let mut components = path.components();

    let Some(first) = components.next() else {
        return PathBuf::new();
    };

    let first = first.as_os_str().to_string_lossy();

    let first = first.strip_prefix(".").unwrap_or(&first);

    let mut source = PathBuf::from(first);
    source.extend(components);

    source
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
    #[error("failed to determine current working directory: {0}")]
    CurrentDir(#[source] std::io::Error),
}

#[cfg(test)]
#[path = "tests/manage.rs"]
mod tests;
