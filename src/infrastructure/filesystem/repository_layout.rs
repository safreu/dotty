use std::path::{Path, PathBuf};

use crate::{
    application::Plan,
    domain::ManagedSource,
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

    pub fn managed_source_plan(
        &self,
        source: &ManagedSource,
    ) -> Result<Plan, RepositoryLayoutError> {
        let source = self.paths.repo_dir().join(source.path());

        let parent = source
            .parent()
            .ok_or_else(|| RepositoryLayoutError::MissingParent(source.clone()))?;

        let missing = filesystem::missing_directories(self.paths().repo_dir(), parent)?;

        let mut plan = Plan::new();

        for directory in missing {
            plan.push(FileSystemAction::CreateDirectory(directory).into());
        }

        Ok(plan)
    }

    pub fn managed_source_cleanup_plan(
        &self,
        source: &ManagedSource,
    ) -> Result<Plan, RepositoryLayoutError> {
        let source = self.paths.repo_dir().join(source.path());

        let parent = source
            .parent()
            .ok_or_else(|| RepositoryLayoutError::MissingParent(source.clone()))?;

        if !source.starts_with(self.paths.repo_dir()) {
            return Err(RepositoryLayoutError::FileSystem(
                FileSystemError::PathOutsideRoot {
                    root: self.paths.repo_dir().to_path_buf(),
                    target: source,
                },
            ));
        }

        let mut plan = Plan::new();

        for directory in parent
            .ancestors()
            .take_while(|path| *path != self.paths().repo_dir())
        {
            plan.push(FileSystemAction::RemoveDirectoryIfEmpty(directory.to_path_buf()).into());
        }

        Ok(plan)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RepositoryLayoutError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("expected directory at `{0}`")]
    InvalidDirectory(PathBuf),
    #[error("Missing parent for directory: `{0}`")]
    MissingParent(PathBuf),
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

    #[test]
    fn managed_source_plan_creates_missing_parent_directory() {
        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let source = ManagedSource::new(PathBuf::from("ghostty/config")).unwrap();

        let plan = context.layout().managed_source_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[FileSystemAction::CreateDirectory(context.repo_dir.join("ghostty")).into(),]
        );
    }

    #[test]
    fn managed_source_plan_creates_nested_directories_in_order() {
        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let source = ManagedSource::new(PathBuf::from("editors/nvim/init.lua")).unwrap();

        let plan = context.layout().managed_source_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[
                FileSystemAction::CreateDirectory(context.repo_dir.join("editors")).into(),
                FileSystemAction::CreateDirectory(context.repo_dir.join("editors/nvim")).into(),
            ]
        );
    }

    #[test]
    fn managed_source_plan_reuses_existing_directories() {
        let context = TestContext::new();

        fs::create_dir_all(context.repo_dir.join("editors")).unwrap();

        let source = ManagedSource::new(PathBuf::from("editors/nvim/init.lua")).unwrap();

        let plan = context.layout().managed_source_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[FileSystemAction::CreateDirectory(context.repo_dir.join("editors/nvim")).into(),]
        );
    }

    #[test]
    fn managed_source_plan_is_empty_for_root_level_source() {
        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let source = ManagedSource::new(PathBuf::from("zshrc")).unwrap();

        let plan = context.layout().managed_source_plan(&source).unwrap();

        assert!(plan.actions().is_empty());
    }

    #[test]
    fn managed_source_plan_rejects_file_in_directory_hierarchy() {
        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let blocking_file = context.repo_dir.join("editors");
        fs::write(&blocking_file, "not a directory").unwrap();

        let source = ManagedSource::new(PathBuf::from("editors/nvim/init.lua")).unwrap();

        let result = context.layout().managed_source_plan(&source);

        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn managed_source_plan_rejects_symlink_in_directory_hierarchy() {
        use std::os::unix::fs::symlink;

        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let actual_directory = context.repo_dir.join("actual");
        fs::create_dir(&actual_directory).unwrap();

        symlink(&actual_directory, context.repo_dir.join("editors")).unwrap();

        let source = ManagedSource::new(PathBuf::from("editors/nvim/init.lua")).unwrap();

        let result = context.layout().managed_source_plan(&source);

        assert!(result.is_err());
    }

    #[test]
    fn managed_source_plan_does_not_modify_filesystem() {
        let context = TestContext::new();
        fs::create_dir(&context.repo_dir).unwrap();

        let source = ManagedSource::new(PathBuf::from("editors/nvim/init.lua")).unwrap();

        let plan = context.layout().managed_source_plan(&source).unwrap();

        assert_eq!(plan.actions().len(), 2);

        assert!(!context.repo_dir.join("editors").exists());
        assert!(!context.repo_dir.join("editors/nvim").exists());
    }

    #[test]
    fn managed_source_cleanup_plan_removes_nested_parents_deepest_first() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("repo");

        std::fs::create_dir_all(&repo_dir).unwrap();

        let paths = RepositoryPaths::new(repo_dir.clone());
        let layout = RepositoryLayout::new(&paths);

        let source = ManagedSource::new("editors/nvim/init.lua".into()).unwrap();

        let plan = layout.managed_source_cleanup_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[
                FileSystemAction::RemoveDirectoryIfEmpty(repo_dir.join("editors/nvim"),).into(),
                FileSystemAction::RemoveDirectoryIfEmpty(repo_dir.join("editors"),).into(),
            ]
        );
    }

    #[test]
    fn managed_source_cleanup_plan_does_not_remove_repository_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("repo");

        std::fs::create_dir_all(&repo_dir).unwrap();

        let paths = RepositoryPaths::new(repo_dir.clone());
        let layout = RepositoryLayout::new(&paths);

        let source = ManagedSource::new("ghostty/config".into()).unwrap();

        let plan = layout.managed_source_cleanup_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[FileSystemAction::RemoveDirectoryIfEmpty(repo_dir.join("ghostty"),).into(),]
        );
    }

    #[test]
    fn managed_source_cleanup_plan_is_empty_for_root_level_source() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("repo");

        std::fs::create_dir_all(&repo_dir).unwrap();

        let paths = RepositoryPaths::new(repo_dir);
        let layout = RepositoryLayout::new(&paths);

        let source = ManagedSource::new("bashrc".into()).unwrap();

        let plan = layout.managed_source_cleanup_plan(&source).unwrap();

        assert!(plan.actions().is_empty());
    }

    #[test]
    fn managed_source_cleanup_plan_does_not_remove_source_itself() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("repo");

        std::fs::create_dir_all(&repo_dir).unwrap();

        let paths = RepositoryPaths::new(repo_dir.clone());
        let layout = RepositoryLayout::new(&paths);

        let source = ManagedSource::new("ghostty/config".into()).unwrap();

        let plan = layout.managed_source_cleanup_plan(&source).unwrap();

        assert!(!plan.actions().contains(
            &FileSystemAction::RemoveDirectoryIfEmpty(repo_dir.join("ghostty/config"),).into()
        ));
    }

    #[test]
    fn managed_source_cleanup_plan_does_not_depend_on_directory_emptiness() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("repo");
        let ghostty_dir = repo_dir.join("ghostty");

        std::fs::create_dir_all(&ghostty_dir).unwrap();
        std::fs::write(ghostty_dir.join("themes.toml"), "theme = 'dark'").unwrap();

        let paths = RepositoryPaths::new(repo_dir);
        let layout = RepositoryLayout::new(&paths);

        let source = ManagedSource::new("ghostty/config".into()).unwrap();

        let plan = layout.managed_source_cleanup_plan(&source).unwrap();

        assert_eq!(
            plan.actions(),
            &[FileSystemAction::RemoveDirectoryIfEmpty(ghostty_dir).into(),]
        );
    }
}
