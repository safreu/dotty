
use std::path::PathBuf;

use super::RepositoryPaths;

#[test]
fn metadata_dir_is_inside_repository() {
    let paths = RepositoryPaths::new(PathBuf::from("/home/user/dotfiles"));

    assert_eq!(
        paths.metadata_dir(),
        PathBuf::from("/home/user/dotfiles/.dotty")
    );
}

#[test]
fn manifest_file_is_inside_metadata_directory() {
    let paths = RepositoryPaths::new(PathBuf::from("/home/user/dotfiles"));

    assert_eq!(
        paths.manifest_file(),
        PathBuf::from("/home/user/dotfiles/.dotty/manifest.toml")
    );
}
