use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigRoot {
    repo_dir: PathBuf,
}

impl ConfigRoot {
    pub fn new(repo_dir: PathBuf) -> Self {
        Self { repo_dir }
    }

    pub fn repo_dir(&self) -> &PathBuf {
        &self.repo_dir
    }
}
