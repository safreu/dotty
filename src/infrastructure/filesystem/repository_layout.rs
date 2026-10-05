use std::path::{Path, PathBuf};

use crate::{
    application::Plan,
    infrastructure::{
        filesystem::{self, FileSystemAction, FileSystemError, PathKind},
        persistence::RepositoryPaths,
    },
};

pub struct RepositoryLayout<'a> {
    paths: &'a RepositoryPaths,
}

impl<'a> RepositoryLayout<'a> {
    pub fn new(paths: &'a RepositoryPaths) -> Self {
        Self { paths }
    }

    pub fn paths(&self) -> &'a RepositoryPaths {
        self.paths
    }

    pub fn initialization_plan(&self) -> Result<Plan, RepositoryLayoutError> {
        let mut plan = Plan::new();

        self.plan_directory(&mut plan, self.paths().repo_dir())?;
        self.plan_directory(&mut plan, &self.paths().metadata_dir())?;

        Ok(plan)
    }

    fn plan_directory(&self, plan: &mut Plan, path: &Path) -> Result<(), RepositoryLayoutError> {
        match filesystem::path_kind(path)? {
            None => plan.push(FileSystemAction::CreateDirectory(path.to_path_buf()).into()),
            Some(PathKind::Directory) => {}
            Some(_) => return Err(RepositoryLayoutError::InvalidDirectory(path.to_path_buf())),
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryLayoutError {
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
        repo_dir: PathBuf,
        paths: RepositoryPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp_dir = tempfile::tempdir().unwrap();

            let repo_dir = temp_dir.path().join("repo");

            let paths = RepositoryPaths::new(repo_dir.clone());

            Self {
                temp_dir,
                repo_dir,
                paths,
            }
        }

        fn layout(&self) -> RepositoryLayout<'_> {
            RepositoryLayout::new(&self.paths)
        }
    }

    #[test]
    fn initialization_plan_contains_missing_repository_directories() {
        let context = TestContext::new();
        let layout = context.layout();

        let plan = layout.initialization_plan().unwrap();

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.repo_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.paths.metadata_dir()).into())
        );
    }

    #[test]
    fn initialization_plan_ignores_existing_repository_directory() {
        let context = TestContext::new();

        fs::create_dir(&context.repo_dir).unwrap();

        let layout = context.layout();

        let plan = layout.initialization_plan().unwrap();

        assert!(
            !plan
                .actions()
                .contains(&FileSystemAction::CreateDirectory(context.repo_dir.clone()).into())
        );

        assert!(
            plan.actions()
                .contains(&FileSystemAction::CreateDirectory(context.paths.metadata_dir()).into())
        );
    }

    #[test]
    fn initialization_plan_is_empty_when_repository_layout_exists() {
        let context = TestContext::new();

        fs::create_dir(&context.repo_dir).unwrap();
        fs::create_dir(context.paths.metadata_dir()).unwrap();

        let layout = context.layout();

        let plan = layout.initialization_plan().unwrap();

        assert!(plan.actions().is_empty());
    }

    #[test]
    fn initialization_plan_does_not_modify_filesystem() {
        let context = TestContext::new();
        let layout = context.layout();

        let _ = layout.initialization_plan().unwrap();

        assert!(!context.repo_dir.exists());
        assert!(!context.paths.metadata_dir().exists());
    }

    #[test]
    fn initialization_plan_rejects_file_as_repository_directory() {
        let context = TestContext::new();

        fs::write(&context.repo_dir, "not a directory").unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.repo_dir
        ));
    }

    #[test]
    fn initialization_plan_rejects_symlink_as_repository_directory() {
        let context = TestContext::new();

        let actual_directory = context.temp_dir.path().join("actual-repository");

        fs::create_dir(&actual_directory).unwrap();
        symlink(&actual_directory, &context.repo_dir).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.repo_dir
        ));
    }

    #[test]
    fn initialization_plan_rejects_broken_symlink_as_repository_directory() {
        let context = TestContext::new();

        let missing = context.temp_dir.path().join("missing-repository");

        symlink(&missing, &context.repo_dir).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.repo_dir
        ));
    }

    #[test]
    fn initialization_plan_rejects_file_as_metadata_directory() {
        let context = TestContext::new();

        fs::create_dir(&context.repo_dir).unwrap();
        fs::write(context.paths.metadata_dir(), "not a directory").unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.paths.metadata_dir()
        ));
    }

    #[test]
    fn initialization_plan_rejects_symlink_as_metadata_directory() {
        let context = TestContext::new();

        fs::create_dir(&context.repo_dir).unwrap();

        let actual_directory = context.temp_dir.path().join("actual-metadata");

        fs::create_dir(&actual_directory).unwrap();

        symlink(&actual_directory, context.paths.metadata_dir()).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.paths.metadata_dir()
        ));
    }

    #[test]
    fn initialization_plan_rejects_broken_symlink_as_metadata_directory() {
        let context = TestContext::new();

        fs::create_dir(&context.repo_dir).unwrap();

        let missing = context.temp_dir.path().join("missing-metadata");

        symlink(&missing, context.paths.metadata_dir()).unwrap();

        let layout = context.layout();

        let result = layout.initialization_plan();

        assert!(matches!(
            result,
            Err(RepositoryLayoutError::InvalidDirectory(path))
                if path == context.paths.metadata_dir()
        ));
    }
}
