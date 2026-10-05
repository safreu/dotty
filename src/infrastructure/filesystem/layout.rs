use std::path::{Path, PathBuf};

use crate::{
    application::Plan,
    infrastructure::{
        filesystem::{self, FileSystemAction, FileSystemError, PathKind},
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

    pub fn is_initialized(&self) -> Result<bool, DottyLayoutError> {
        let config = filesystem::path_kind(&self.paths.config_file())?;
        let backups = filesystem::path_kind(&self.paths.config_file())?;

        Ok(matches!(config, Some(PathKind::File)) && matches!(backups, Some(PathKind::File)))
    }

    pub fn require_initialized(&self) -> Result<(), DottyLayoutError> {
        if self.is_initialized()? {
            Ok(())
        } else {
            Err(DottyLayoutError::NotInitialized)
        }
    }

    pub fn initialization_plan(&self, repo_dir: &Path) -> Result<Plan, DottyLayoutError> {
        let mut plan = Plan::new();

        self.plan_directory(&mut plan, self.paths.config_dir())?;
        self.plan_directory(&mut plan, self.paths.storage_dir())?;
        self.plan_directory(&mut plan, &self.paths.backup_dir())?;
        self.plan_directory(&mut plan, repo_dir)?;

        Ok(plan)
    }

    fn plan_directory(&self, plan: &mut Plan, path: &Path) -> Result<(), DottyLayoutError> {
        match filesystem::path_kind(path)? {
            None => plan.push(FileSystemAction::CreateDirectory(path.to_path_buf()).into()),
            Some(PathKind::Directory) => {}
            Some(_) => return Err(DottyLayoutError::InvalidDirectory(path.to_path_buf())),
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyLayoutError {
    #[error("Dotty is not initialized. Run `dotty init` first")]
    NotInitialized,
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("expected directory at `{0}`")]
    InvalidDirectory(PathBuf),
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::symlink, path::PathBuf};

    use tempfile::TempDir;

    use super::*;

    struct TestContext {
        temp_dir: TempDir,
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
                temp_dir,
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

        let plan = layout.initialization_plan(&context.repo_dir).unwrap();

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

        let plan = layout.initialization_plan(&context.repo_dir).unwrap();

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

        let plan = layout.initialization_plan(&context.repo_dir).unwrap();

        assert!(plan.actions().is_empty());
    }

    #[test]
    fn initialization_plan_does_not_modify_filesystem() {
        let context = TestContext::new();
        let layout = context.layout();

        let _ = layout.initialization_plan(&context.repo_dir).unwrap();

        assert!(!context.config_dir.exists());
        assert!(!context.storage_dir.exists());
        assert!(!context.paths.backup_dir().exists());
        assert!(!context.repo_dir.exists());
    }

    #[test]
    fn initialization_plan_rejects_file_where_directory_is_expected() {
        let context = TestContext::new();

        fs::write(&context.config_dir, "not a directory").unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan(&context.repo_dir);

        assert!(matches!(
            result,
            Err(DottyLayoutError::InvalidDirectory(path))
                if path == context.config_dir
        ));
    }

    #[test]
    fn initialization_plan_rejects_symlink_where_directory_is_expected() {
        let context = TestContext::new();

        let actual_directory = context.temp_dir.path().join("actual-directory");

        fs::create_dir(&actual_directory).unwrap();
        symlink(&actual_directory, &context.config_dir).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan(&context.repo_dir);

        assert!(matches!(
            result,
            Err(DottyLayoutError::InvalidDirectory(path))
                if path == context.config_dir
        ));
    }

    #[test]
    fn initialization_plan_rejects_broken_symlink_where_directory_is_expected() {
        let context = TestContext::new();

        let missing = context.temp_dir.path().join("missing-directory");

        symlink(&missing, &context.config_dir).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan(&context.repo_dir);

        assert!(matches!(
            result,
            Err(DottyLayoutError::InvalidDirectory(path))
                if path == context.config_dir
        ));
    }

    #[test]
    fn is_initialized_returns_true_when_metadata_files_exist() {
        let context = TestContext::new();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.storage_dir).unwrap();

        fs::write(context.paths.config_file(), "config").unwrap();
        fs::write(context.paths.backup_file(), "backups").unwrap();

        let layout = context.layout();

        assert!(layout.is_initialized().unwrap());
    }

    #[test]
    fn is_initialized_returns_false_when_metadata_files_are_missing() {
        let context = TestContext::new();
        let layout = context.layout();

        assert!(!layout.is_initialized().unwrap());
    }

    #[test]
    fn is_initialized_returns_false_when_config_path_is_directory() {
        let context = TestContext::new();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.storage_dir).unwrap();

        fs::create_dir(context.paths.config_file()).unwrap();
        fs::write(context.paths.backup_file(), "backups").unwrap();

        let layout = context.layout();

        assert!(!layout.is_initialized().unwrap());
    }

    #[test]
    fn is_initialized_returns_false_when_metadata_file_is_symlink() {
        let context = TestContext::new();

        fs::create_dir_all(&context.config_dir).unwrap();
        fs::create_dir_all(&context.storage_dir).unwrap();

        let actual_config = context.temp_dir.path().join("actual-config.toml");

        fs::write(&actual_config, "config").unwrap();
        symlink(&actual_config, context.paths.config_file()).unwrap();

        fs::write(context.paths.backup_file(), "backups").unwrap();

        let layout = context.layout();

        assert!(!layout.is_initialized().unwrap());
    }
}
