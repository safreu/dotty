use std::{fs, os::unix::fs::symlink, path::PathBuf, process::Command};

use tempfile::tempdir;

use crate::infrastructure::filesystem::{atomic_write_file, rename};

use super::{FileSystemError, PathKind, copy_dir, path_kind};

#[test]
fn path_kind_identifies_regular_file() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("file");

    fs::write(&path, "content").unwrap();

    let kind = path_kind(&path).unwrap();

    assert_eq!(kind, Some(PathKind::File));
}

#[test]
fn path_kind_identifies_directory() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("directory");

    fs::create_dir(&path).unwrap();

    let kind = path_kind(&path).unwrap();

    assert_eq!(kind, Some(PathKind::Directory));
}

#[test]
fn path_kind_identifies_symlink() {
    let temp = tempdir().unwrap();

    let target = temp.path().join("target");
    let link = temp.path().join("link");

    fs::write(&target, "content").unwrap();
    symlink(&target, &link).unwrap();

    let kind = path_kind(&link).unwrap();

    assert_eq!(kind, Some(PathKind::Symlink));
}

#[test]
fn path_kind_identifies_broken_symlink() {
    let temp = tempdir().unwrap();

    let target = temp.path().join("missing-target");
    let link = temp.path().join("link");

    symlink(&target, &link).unwrap();

    let kind = path_kind(&link).unwrap();

    assert_eq!(kind, Some(PathKind::Symlink));
}

#[test]
fn path_kind_returns_none_for_missing_path() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("missing");

    let kind = path_kind(&path).unwrap();

    assert_eq!(kind, None);
}

#[test]
fn copy_dir_preserves_relative_symlink() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::write(source.join("target.txt"), "content").unwrap();

    symlink("target.txt", source.join("link.txt")).unwrap();

    copy_dir(&source, &destination).unwrap();

    let copied_link = destination.join("link.txt");

    assert!(
        fs::symlink_metadata(&copied_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    assert_eq!(
        fs::read_link(&copied_link).unwrap(),
        PathBuf::from("target.txt")
    );

    assert_eq!(fs::read_to_string(&copied_link).unwrap(), "content");
}

#[test]
fn copy_dir_preserves_absolute_symlink() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");
    let target = temp.path().join("target.txt");

    fs::create_dir(&source).unwrap();
    fs::write(&target, "content").unwrap();

    symlink(&target, source.join("link.txt")).unwrap();

    copy_dir(&source, &destination).unwrap();

    let copied_link = destination.join("link.txt");

    assert!(
        fs::symlink_metadata(&copied_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    assert_eq!(fs::read_link(&copied_link).unwrap(), target);
}

#[test]
fn copy_dir_preserves_symlink_to_directory() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::create_dir(source.join("actual")).unwrap();
    fs::write(source.join("actual").join("file.txt"), "content").unwrap();

    symlink("actual", source.join("linked")).unwrap();

    copy_dir(&source, &destination).unwrap();

    let copied_link = destination.join("linked");

    assert!(
        fs::symlink_metadata(&copied_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    assert_eq!(
        fs::read_link(&copied_link).unwrap(),
        PathBuf::from("actual")
    );

    assert_eq!(
        fs::read_to_string(copied_link.join("file.txt")).unwrap(),
        "content"
    );
}

#[test]
fn copy_dir_preserves_broken_symlink() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();

    symlink("does-not-exist", source.join("broken")).unwrap();

    copy_dir(&source, &destination).unwrap();

    let copied_link = destination.join("broken");

    assert!(
        fs::symlink_metadata(&copied_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );

    assert_eq!(
        fs::read_link(&copied_link).unwrap(),
        PathBuf::from("does-not-exist")
    );

    assert!(!copied_link.exists());
}

#[test]
fn copy_dir_removes_partial_destination_when_copy_fails() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::write(source.join("file.txt"), "content").unwrap();

    let unsupported = source.join("unsupported");

    let status = Command::new("mkfifo").arg(&unsupported).status().unwrap();

    assert!(status.success());

    let result = copy_dir(&source, &destination);

    assert!(matches!(result, Err(FileSystemError::CopyDirectory { .. })));

    assert!(
        !destination.exists(),
        "partial destination should be removed after copy failure"
    );
}

#[test]
fn atomic_write_file_creates_missing_file() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");

    atomic_write_file(&path, b"content").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"content");
}

#[test]
fn atomic_write_file_replaces_existing_file() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");

    fs::write(&path, b"old content").unwrap();

    atomic_write_file(&path, b"new content").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"new content");
}

#[test]
fn atomic_write_file_does_not_leave_temporary_file() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");

    atomic_write_file(&path, b"content").unwrap();

    let entries: Vec<_> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();

    assert_eq!(entries, vec![path]);
}

#[test]
fn atomic_write_file_rejects_missing_parent_directory() {
    let temp = tempdir().unwrap();

    let path = temp.path().join("missing").join("config.toml");

    let result = atomic_write_file(&path, b"content");

    assert!(matches!(
        result,
        Err(FileSystemError::AtomicWriteFile {
            path: error_path,
            ..
        }) if error_path == path
    ));

    assert!(!path.exists());
}

#[test]
fn atomic_write_file_rejects_directory_as_destination() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");

    fs::create_dir(&path).unwrap();

    let result = atomic_write_file(&path, b"content");

    assert!(matches!(
        result,
        Err(FileSystemError::AtomicWriteFile {
            path: error_path,
            ..
        }) if error_path == path
    ));

    assert!(path.is_dir());
}

#[test]
fn atomic_write_file_preserves_existing_file_when_replacement_fails() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("config.toml");

    fs::write(&path, b"original").unwrap();

    let original_permissions = fs::metadata(temp.path()).unwrap().permissions();

    let mut readonly_permissions = original_permissions.clone();
    readonly_permissions.set_readonly(true);
    fs::set_permissions(temp.path(), readonly_permissions).unwrap();

    let result = atomic_write_file(&path, b"replacement");

    fs::set_permissions(temp.path(), original_permissions).unwrap();

    assert!(result.is_err());
    assert_eq!(fs::read(&path).unwrap(), b"original");
}

#[test]
fn rename_moves_file_when_destination_is_missing() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source.txt");
    let destination = temp.path().join("destination.txt");

    fs::write(&source, "content").unwrap();

    rename(&source, &destination).unwrap();

    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&destination).unwrap(), "content");
}

#[test]
fn rename_rejects_existing_file_destination() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source.txt");
    let destination = temp.path().join("destination.txt");

    fs::write(&source, "original source").unwrap();
    fs::write(&destination, "original destination").unwrap();

    let result = rename(&source, &destination);

    assert!(matches!(
        result,
        Err(FileSystemError::Rename {
            from,
            to,
            source: error,
        }) if from == source
            && to == destination
            && error.kind() == std::io::ErrorKind::AlreadyExists
    ));

    assert_eq!(fs::read_to_string(&source).unwrap(), "original source");
    assert_eq!(
        fs::read_to_string(&destination).unwrap(),
        "original destination"
    );
}

#[test]
fn rename_rejects_existing_directory_destination() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();

    fs::write(source.join("source.txt"), "source content").unwrap();
    fs::write(destination.join("destination.txt"), "destination content").unwrap();

    let result = rename(&source, &destination);

    assert!(matches!(result, Err(FileSystemError::Rename { .. })));

    assert!(source.is_dir());
    assert!(destination.is_dir());

    assert_eq!(
        fs::read_to_string(source.join("source.txt")).unwrap(),
        "source content"
    );
    assert_eq!(
        fs::read_to_string(destination.join("destination.txt")).unwrap(),
        "destination content"
    );
}

#[test]
fn rename_rejects_existing_broken_symlink_destination() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source.txt");
    let destination = temp.path().join("destination.txt");
    let missing_target = temp.path().join("missing.txt");

    fs::write(&source, "source content").unwrap();
    symlink(&missing_target, &destination).unwrap();

    let result = rename(&source, &destination);

    assert!(matches!(result, Err(FileSystemError::Rename { .. })));

    assert_eq!(fs::read_to_string(&source).unwrap(), "source content");

    assert_eq!(fs::read_link(&destination).unwrap(), missing_target);

    assert_eq!(path_kind(&destination).unwrap(), Some(PathKind::Symlink));
}

#[test]
fn rename_preserves_source_when_destination_conflicts() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source.txt");
    let destination = temp.path().join("destination.txt");

    fs::write(&source, b"important user data").unwrap();
    fs::write(&destination, b"existing configuration").unwrap();

    let source_before = fs::read(&source).unwrap();
    let destination_before = fs::read(&destination).unwrap();

    assert!(rename(&source, &destination).is_err());

    assert_eq!(fs::read(&source).unwrap(), source_before);
    assert_eq!(fs::read(&destination).unwrap(), destination_before);
}

#[test]
fn copy_dir_preserves_existing_destination_when_copy_fails() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();

    fs::write(source.join("source.txt"), "source").unwrap();
    fs::write(destination.join("important.txt"), "important").unwrap();

    let result = copy_dir(&source, &destination);

    assert!(result.is_err());

    assert_eq!(
        fs::read_to_string(destination.join("important.txt")).unwrap(),
        "important"
    );
    assert_eq!(
        fs::read_to_string(source.join("source.txt")).unwrap(),
        "source"
    );
}

#[test]
fn copy_dir_does_not_replace_existing_empty_directory() {
    let temp = tempdir().unwrap();

    let source = temp.path().join("source");
    let destination = temp.path().join("destination");

    fs::create_dir(&source).unwrap();
    fs::write(source.join("file.txt"), "content").unwrap();

    fs::create_dir(&destination).unwrap();

    let result = copy_dir(&source, &destination);

    assert!(matches!(result, Err(FileSystemError::CopyDirectory { .. })));

    assert!(destination.is_dir());
    assert!(!destination.join("file.txt").exists());
    assert_eq!(
        fs::read_to_string(source.join("file.txt")).unwrap(),
        "content"
    );
}
