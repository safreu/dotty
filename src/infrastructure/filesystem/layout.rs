use std::path::Path;

use crate::{
    application::Plan,
    infrastructure::{
        filesystem::{FileSystemAction, FileSystemError},
        persistence::paths::DottyPaths,
    },
};

pub struct DottyLayout<'a> {
    paths: &'a DottyPaths,
}

impl<'a> DottyLayout<'a> {
    pub fn new(paths: &'a DottyPaths) -> Self {
        Self { paths }
    }

    pub fn is_initialized(&self) -> bool {
        self.paths.config_file().exists() && self.paths.backup_file().exists()
    }

    pub fn require_initialized(&self) -> Result<(), DottyLayoutError> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(DottyLayoutError::NotInitialized)
        }
    }

    pub fn initialization_plan(&self, repo_dir: &Path) -> Plan {
        let mut plan = Plan::new();

        if !self.paths.config_dir().exists() {
            plan.push(
                FileSystemAction::CreateDirectory(self.paths.config_dir().to_path_buf()).into(),
            );
        }
        if !self.paths.storage_dir().exists() {
            plan.push(
                FileSystemAction::CreateDirectory(self.paths.storage_dir().to_path_buf()).into(),
            );
        }
        if !self.paths.backup_dir().exists() {
            plan.push(
                FileSystemAction::CreateDirectory(self.paths.backup_dir().to_path_buf()).into(),
            );
        }
        if !repo_dir.exists() {
            plan.push(FileSystemAction::CreateDirectory(repo_dir.to_path_buf()).into());
        }

        plan
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyLayoutError {
    #[error("Dotty is not initialized. Run `dotty init` first")]
    NotInitialized,
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use tempfile::TempDir;

    use super::*;

    struct TestContext {
        _temp_dir: TempDir,
        config_dir: PathBuf,
        storage_dir: PathBuf,
        repo_dir: PathBuf,
        paths: DottyPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().unwrap();

            let config_dir = temp_dir.path().join("config");
            let storage_dir = temp_dir.path().join("storage");
            let repo_dir = temp_dir.path().join("repo");

            let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone());

            Self {
                _temp_dir: temp_dir,
                config_dir,
                storage_dir,
                repo_dir,
                paths,
            }
        }

        fn layout(&self) -> DottyLayout<'_> {
            DottyLayout::new(&self.paths)
        }
    }

    #[test]
    fn initialization_plan_contains_missing_directories() {
        let context = TestContext::new();
        let layout = context.layout();

        let plan = layout.initialization_plan(&context.repo_dir);

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.config_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.storage_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.paths.backup_dir()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.repo_dir.clone()).into())
        );
    }

    #[test]
    fn initialization_plan_ignores_existing_directories() {
        let context = TestContext::new();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.repo_dir).unwrap();

        let layout = context.layout();

        let plan = layout.initialization_plan(&context.repo_dir);

        assert!(
            !plan
                .actions()
                .contains(&FileSystemAction::CreateDirectory(context.config_dir.clone()).into())
        );

        assert!(
            !plan
                .actions()
                .contains(&FileSystemAction::CreateDirectory(context.repo_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.storage_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.paths.backup_dir()).into())
        );
    }

    #[test]
    fn initialization_plan_is_empty_when_layout_exists() {
        let context = TestContext::new();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.storage_dir).unwrap();
        fs::create_dir_all(context.paths.backup_dir()).unwrap();
        fs::create_dir_all(&context.repo_dir).unwrap();

        let layout = context.layout();

        let plan = layout.initialization_plan(&context.repo_dir);

        assert!(plan.actions().is_empty());
    }

    #[test]
    fn initialization_plan_does_not_modify_filesystem() {
        let context = TestContext::new();
        let layout = context.layout();

        let _ = layout.initialization_plan(&context.repo_dir);

        assert!(!context.config_dir.exists());
        assert!(!context.storage_dir.exists());
        assert!(!context.paths.backup_dir().exists());
        assert!(!context.repo_dir.exists());
    }
}
