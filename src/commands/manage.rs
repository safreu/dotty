use std::path::PathBuf;

use crate::{
    application::{DotfileManager, DotfileManagerError, DotfileManagerLoadError},
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

    let managed_target = config.to_managed_target(&target)?;

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
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn target(path: &str) -> ManagedTarget {
        ManagedTarget::home_relative(PathBuf::from(path)).unwrap()
    }

    #[test]
    fn default_source_removes_leading_dot_from_hidden_file() {
        let source = default_source(&target(".zshrc"));

        assert_eq!(source, PathBuf::from("zshrc"));
    }

    #[test]
    fn default_source_strips_config_directory() {
        let source = default_source(&target(".config/ghostty/config"));

        assert_eq!(source, PathBuf::from("ghostty/config"));
    }

    #[test]
    fn default_source_removes_leading_dot_after_config_directory() {
        let source = default_source(&target(".config/.nvim/init.lua"));

        assert_eq!(source, PathBuf::from("nvim/init.lua"));
    }

    #[test]
    fn default_source_preserves_nested_directories() {
        let source = default_source(&target("Documents/projects/notes.md"));

        assert_eq!(source, PathBuf::from("Documents/projects/notes.md"));
    }

    #[test]
    fn default_source_removes_dot_only_from_first_component() {
        let source = default_source(&target(".config/ghostty/.themes/config"));

        assert_eq!(source, PathBuf::from("ghostty/.themes/config"));
    }

    #[test]
    fn default_source_handles_config_directory_itself() {
        let source = default_source(&target(".config"));

        assert_eq!(source, PathBuf::from("config"));
    }

    #[test]
    fn default_source_does_not_strip_similarly_named_directory() {
        let source = default_source(&target(".configuration/ghostty/config"));

        assert_eq!(source, PathBuf::from("configuration/ghostty/config"));
    }

    #[test]
    fn default_source_removes_dot_from_other_hidden_directories() {
        let source = default_source(&target(".local/share/app"));

        assert_eq!(source, PathBuf::from("local/share/app"));
    }

    #[test]
    fn default_source_preserves_file_extensions() {
        let source = default_source(&target(".config/ghostty/settings.toml"));

        assert_eq!(source, PathBuf::from("ghostty/settings.toml"));
    }

    #[test]
    fn default_source_preserves_non_hidden_file_names() {
        let source = default_source(&target("Documents/notes.md"));

        assert_eq!(source, PathBuf::from("Documents/notes.md"));
    }
}
