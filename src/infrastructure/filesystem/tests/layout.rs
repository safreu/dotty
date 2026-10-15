use std::{fs, os::unix::fs::symlink, path::PathBuf};

use tempfile::TempDir;

use super::*;

struct TestContext {
    temp_dir: TempDir,
    config_dir: PathBuf,
    storage_dir: PathBuf,
    paths: DottyPaths,
}

impl TestContext {
    fn new() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();

        let config_dir = temp_dir.path().join("config");
        let storage_dir = temp_dir.path().join("storage");
        let home_dir = temp_dir.path().join("home");

        let paths = DottyPaths::new(config_dir.clone(), storage_dir.clone(), home_dir.clone());

        Self {
            temp_dir,
            config_dir,
            storage_dir,
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

    let plan = layout.initialization_plan().unwrap();

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
}

#[test]
fn initialization_plan_ignores_existing_directories() {
    let context = TestContext::new();

    fs::create_dir_all(&context.config_dir).unwrap();

    let layout = context.layout();

    let plan = layout.initialization_plan().unwrap();

    assert!(
        !plan
            .actions()
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
}

#[test]
fn initialization_plan_is_empty_when_layout_exists() {
    let context = TestContext::new();

    fs::create_dir_all(&context.config_dir).unwrap();
    fs::create_dir_all(&context.storage_dir).unwrap();
    fs::create_dir_all(context.paths.backup_dir()).unwrap();

    let layout = context.layout();

    let plan = layout.initialization_plan().unwrap();

    assert!(plan.actions().is_empty());
}

#[test]
fn initialization_plan_does_not_modify_filesystem() {
    let context = TestContext::new();
    let layout = context.layout();

    let _ = layout.initialization_plan().unwrap();

    assert!(!context.config_dir.exists());
    assert!(!context.storage_dir.exists());
    assert!(!context.paths.backup_dir().exists());
}

#[test]
fn initialization_plan_rejects_file_where_directory_is_expected() {
    let context = TestContext::new();

    fs::write(&context.config_dir, "not a directory").unwrap();

    let layout = context.layout();

    let result = layout.initialization_plan();

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

    let result = layout.initialization_plan();

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

    let result = layout.initialization_plan();

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
