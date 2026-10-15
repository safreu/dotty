use std::{fs, os::unix::fs::symlink};

use tempfile::TempDir;

use super::*;

#[test]
fn write_file_creates_missing_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");

    let action = FileSystemAction::WriteFile {
        path: path.clone(),
        contents: b"new content".to_vec(),
    };

    action.execute().unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new content");
}

#[test]
fn write_file_overwrites_existing_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");

    fs::write(&path, "old content").unwrap();

    let action = FileSystemAction::WriteFile {
        path: path.clone(),
        contents: b"new content".to_vec(),
    };

    action.execute().unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new content");
}

#[test]
fn write_file_rollback_removes_new_file() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");

    let action = FileSystemAction::WriteFile {
        path: path.clone(),
        contents: b"new content".to_vec(),
    };

    let executed = action.execute().unwrap();

    assert!(path.exists());

    executed.rollback().unwrap().execute().unwrap();

    assert!(!path.exists());
}

#[test]
fn write_file_rollback_restores_previous_contents() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("config.toml");

    fs::write(&path, "old content").unwrap();

    let action = FileSystemAction::WriteFile {
        path: path.clone(),
        contents: b"new content".to_vec(),
    };

    let executed = action.execute().unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new content");

    executed.rollback().unwrap().execute().unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"old content");
}

#[test]
fn write_file_rejects_directory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("directory");

    fs::create_dir(&path).unwrap();

    let action = FileSystemAction::WriteFile {
        path: path.clone(),
        contents: b"content".to_vec(),
    };

    let result = action.execute();

    assert!(matches!(
        result,
        Err(FileSystemError::InvalidWriteTarget { path: error_path })
            if error_path == path
    ));
}

#[test]
fn write_file_rejects_symlink() {
    let temp = TempDir::new().unwrap();

    let target = temp.path().join("target");
    let link = temp.path().join("link");

    fs::write(&target, "original").unwrap();
    symlink(&target, &link).unwrap();

    let action = FileSystemAction::WriteFile {
        path: link,
        contents: b"modified".to_vec(),
    };

    let result = action.execute();

    assert!(matches!(
        result,
        Err(FileSystemError::InvalidWriteTarget { .. })
    ));

    assert_eq!(fs::read(&target).unwrap(), b"original");
}

#[test]
fn write_file_rejects_broken_symlink() {
    let temp = TempDir::new().unwrap();

    let target = temp.path().join("missing");
    let link = temp.path().join("link");

    symlink(&target, &link).unwrap();

    let action = FileSystemAction::WriteFile {
        path: link.clone(),
        contents: b"content".to_vec(),
    };

    let result = action.execute();

    assert!(matches!(
        result,
        Err(FileSystemError::InvalidWriteTarget { path })
            if path == link
    ));

    assert!(!target.exists());
}
