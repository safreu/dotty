use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::domain::ManagedEntry;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ManifestRoot {
    manages: HashSet<ManagedEntry>,
}

impl ManifestRoot {
    pub fn new() -> Self {
        Self {
            manages: HashSet::new(),
        }
    }

    pub fn manages(&self) -> &HashSet<ManagedEntry> {
        &self.manages
    }

    pub fn manages_mut(&mut self) -> &mut HashSet<ManagedEntry> {
        &mut self.manages
    }
}
