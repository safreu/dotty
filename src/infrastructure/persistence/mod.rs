pub mod backup;
pub mod config;
pub mod paths;

mod manifest;
pub use manifest::{ManifestRepository, ManifestRepositoryError};

mod repository_paths;
pub use repository_paths::RepositoryPaths;

mod toml;
use std::path::PathBuf;

pub use toml::{TomlError, read_toml, serialize_toml, write_toml};

pub struct PreparedWrite {
    path: PathBuf,
    contents: Vec<u8>,
}

impl PreparedWrite {
    pub fn new(path: PathBuf, contents: Vec<u8>) -> Self {
        Self { path, contents }
    }

    pub fn into_parts(self) -> (PathBuf, Vec<u8>) {
        (self.path, self.contents)
    }
}
