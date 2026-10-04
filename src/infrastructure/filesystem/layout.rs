use crate::infrastructure::persistence::paths::DottyPaths;

pub struct DottyLayout<'a> {
    paths: &'a DottyPaths,
}

impl<'a> DottyLayout<'a> {
    pub fn new(paths: &'a DottyPaths) -> Self {
        Self { paths }
    }

    pub fn is_initialized(&self) -> bool {
        self.paths.config_file().exists() && self.paths.backup_file().exists()
    }

    pub fn require_initialized(&self) -> Result<(), DottyLayoutError> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(DottyLayoutError::NotInitialized)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyLayoutError {
    #[error("Dotty is not initialized. Run `dotty init` first")]
    NotInitialized,
}
