use crate::domain::{
    ManagedSource, ManagedSourceError,
    managed_entry::{ManagedTarget, ManagedTargetError},
};
use std::path::PathBuf;

#[test]
fn home_relative_accepts_relative_path() {
    let path = PathBuf::from(".config/ghostty/config");

    let target = ManagedTarget::home_relative(path.clone()).unwrap();

    assert_eq!(target, ManagedTarget::HomeRelative(path));
}

#[test]
fn home_relative_rejects_absolute_path() {
    let path = PathBuf::from("/etc/example");

    let result = ManagedTarget::home_relative(path.clone());

    assert!(matches!(
        result,
        Err(ManagedTargetError::AbsolutePath(error_path))
            if error_path == path
    ));
}

#[test]
fn home_relative_rejects_empty_path() {
    let result = ManagedTarget::home_relative(PathBuf::new());

    assert!(matches!(result, Err(ManagedTargetError::Empty)));
}

#[test]
fn home_relative_rejects_current_directory() {
    let result = ManagedTarget::home_relative(PathBuf::from("."));

    assert!(matches!(result, Err(ManagedTargetError::Empty)));
}

#[test]
fn home_relative_rejects_parent_traversal() {
    let path = PathBuf::from("../secret");

    let result = ManagedTarget::home_relative(path.clone());

    assert!(matches!(
        result,
        Err(ManagedTargetError::ParentTraversal(error_path))
            if error_path == path
    ));
}

#[test]
fn home_relative_rejects_nested_parent_traversal() {
    let path = PathBuf::from("ghostty/../../secret");

    let result = ManagedTarget::home_relative(path.clone());

    assert!(matches!(
        result,
        Err(ManagedTargetError::ParentTraversal(error_path))
            if error_path == path
    ));
}

#[test]
fn home_relative_accepts_nested_relative_path() {
    let path = PathBuf::from(".config/ghostty/config");

    let result = ManagedTarget::home_relative(path.clone()).unwrap();

    assert_eq!(result.path(), path.as_path());
}

#[test]
fn managed_source_accepts_relative_path() {
    let path = PathBuf::from("ghostty/config");

    let result = ManagedSource::new(path.clone()).unwrap();

    assert_eq!(result.path(), path.as_path());
}

#[test]
fn managed_source_rejects_empty_path() {
    let result = ManagedSource::new(PathBuf::new());

    assert!(matches!(result, Err(ManagedSourceError::Empty)));
}

#[test]
fn managed_source_rejects_current_directory() {
    let result = ManagedSource::new(PathBuf::from("."));

    assert!(matches!(result, Err(ManagedSourceError::Empty)));
}

#[test]
fn managed_source_rejects_absolute_path() {
    let path = PathBuf::from("/etc/example");

    let result = ManagedSource::new(path.clone());

    assert!(matches!(
        result,
        Err(ManagedSourceError::AbsolutePath(error_path))
            if error_path == path
    ));
}

#[test]
fn managed_source_rejects_parent_traversal() {
    let path = PathBuf::from("../secret");

    let result = ManagedSource::new(path.clone());

    assert!(matches!(
        result,
        Err(ManagedSourceError::ParentTraversal(error_path))
            if error_path == path
    ));
}

#[test]
fn managed_source_rejects_nested_parent_traversal() {
    let path = PathBuf::from("ghostty/../../secret");

    let result = ManagedSource::new(path.clone());

    assert!(matches!(
        result,
        Err(ManagedSourceError::ParentTraversal(error_path))
            if error_path == path
    ));
}
