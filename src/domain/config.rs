use std::{collections::HashSet, path::PathBuf};

use serde::{Deserialize, Serialize};

use crate::domain::managed_entry::ManagedEntry;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConfigRoot {
    config: Config,
    manages: HashSet<ManagedEntry>,
}

impl ConfigRoot {
    pub fn new(repo_dir: PathBuf) -> Self {
        Self {
            config: Config { repo_dir },
            manages: HashSet::new(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn manages_mut(&mut self) -> &mut HashSet<ManagedEntry> {
        &mut self.manages
    }

    pub fn manages(&self) -> &HashSet<ManagedEntry> {
        &self.manages
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    repo_dir: PathBuf,
}

impl Config {
    pub fn repo_dir(&self) -> &PathBuf {
        &self.repo_dir
    }
}
