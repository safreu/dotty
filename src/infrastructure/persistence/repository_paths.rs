use std::path::{Path, PathBuf};

pub struct RepositoryPaths {
    repo_dir: PathBuf,
}

impl RepositoryPaths {
    pub fn new(repo_dir: PathBuf) -> Self {
        Self { repo_dir }
    }

    pub fn repo_dir(&self) -> &Path {
        &self.repo_dir
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.repo_dir.join(".dotty")
    }

    pub fn manifest_file(&self) -> PathBuf {
        self.metadata_dir().join("manifest.toml")
    }
}

#[cfg(test)]
mod tests {
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
}
