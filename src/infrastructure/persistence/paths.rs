use std::{
    env,
    path::{Path, PathBuf},
};

pub struct DottyPaths {
    config_dir: PathBuf,
    storage_dir: PathBuf,
}

impl DottyPaths {
    pub fn new(config_dir: PathBuf, storage_dir: PathBuf) -> Self {
        Self {
            config_dir,
            storage_dir,
        }
    }

    fn discover_config_dir() -> Result<PathBuf, DottyPathsError> {
        let config_dir = match env::var("XDG_CONFIG_HOME") {
            Ok(dir) => PathBuf::from(dir),
            _ => PathBuf::from(env::var("HOME")?).join(".config"),
        }
        .join("dotty");

        Ok(config_dir)
    }
    fn discover_storage_dir() -> Result<PathBuf, DottyPathsError> {
        let storage_dir = match env::var("XDG_DATA_HOME") {
            Ok(dir) => PathBuf::from(dir),
            _ => PathBuf::from(env::var("HOME")?)
                .join(".local")
                .join("share"),
        }
        .join("dotty");

        Ok(storage_dir)
    }
    pub fn discover() -> Result<Self, DottyPathsError> {
        let config_dir = DottyPaths::discover_config_dir()?;
        let storage_dir = DottyPaths::discover_storage_dir()?;

        Ok(Self::new(config_dir, storage_dir))
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.storage_dir.join("backups")
    }

    pub fn backup_file(&self) -> PathBuf {
        self.storage_dir.join("backups.toml")
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DottyPathsError {
    #[error("Required environment variable is missing")]
    MissingEnvironmentVariable(#[from] env::VarError),
}
