use std::path::{Path, PathBuf};

use crate::domain::ConfigRoot;

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

#[test]
fn absolute_target_remains_absolute() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new("/home/user/.config/ghostty/config");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/ghostty/config"));
}

#[test]
fn absolute_target_is_normalized() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new("/home/user/projects/../.config/nvim");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/nvim"));
}

#[test]
fn relative_target_resolves_against_current_directory() {
    let cwd = Path::new("/home/user");
    let target = Path::new(".config/ghostty/config");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/ghostty/config"));
}

#[test]
fn relative_target_resolves_from_nested_directory() {
    let cwd = Path::new("/home/user/.config");
    let target = Path::new("nvim/init.lua");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/nvim/init.lua"));
}

#[test]
fn parent_components_are_normalized() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new("../.config/nvim/init.lua");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/nvim/init.lua"));
}

#[test]
fn current_directory_components_are_normalized() {
    let cwd = Path::new("/home/user");
    let target = Path::new("./.config/./ghostty/config");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/.config/ghostty/config"));
}

#[test]
fn relative_target_can_resolve_to_current_directory() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new(".");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user/projects"));
}

#[test]
fn relative_target_can_resolve_to_parent_directory() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new("..");

    let resolved = resolve_target_path(target, cwd);

    assert_eq!(resolved, PathBuf::from("/home/user"));
}

#[test]
fn resolved_target_outside_home_is_rejected() {
    let cwd = Path::new("/home/user");
    let target = Path::new("../../etc/config");

    let resolved = resolve_target_path(target, cwd);

    let config = ConfigRoot::new(
        PathBuf::from("/home/user/dotfiles"),
        PathBuf::from("/home/user"),
    );

    let result = config.to_managed_target(&resolved);

    assert!(matches!(result, Err(ConfigRootError::TargetOutsideHome(_))));
}

#[test]
fn resolved_target_inside_home_is_accepted() {
    let cwd = Path::new("/home/user/projects");
    let target = Path::new("../.config/ghostty/config");

    let resolved = resolve_target_path(target, cwd);

    let config = ConfigRoot::new(
        PathBuf::from("/home/user/dotfiles"),
        PathBuf::from("/home/user"),
    );

    let managed_target = config.to_managed_target(&resolved).unwrap();

    assert_eq!(managed_target.path(), Path::new(".config/ghostty/config"));
}
