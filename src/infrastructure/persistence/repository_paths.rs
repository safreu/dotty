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
#[path = "tests/repository_paths.rs"]
mod tests;
