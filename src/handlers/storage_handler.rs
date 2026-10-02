use std::{env, path::PathBuf};

use crate::commands::init::InitError;

pub struct StorageHandler {
    config_dir: PathBuf,
    storage_dir: PathBuf,
}

impl StorageHandler {
    fn discover_config_dir() -> Result<PathBuf, InitError> {
        let config_dir = match env::var("XDG_CONFIG_HOME") {
            Ok(dir) => PathBuf::from(dir),
            _ => PathBuf::from(env::var("HOME")?).join(".config"),
        }
        .join("dotty");

        Ok(config_dir)
    }
    fn discover_storage_dir() -> Result<PathBuf, InitError> {
        let storage_dir = match env::var("XDG_DATA_HOME") {
            Ok(dir) => PathBuf::from(dir),
            _ => PathBuf::from(env::var("HOME")?)
                .join(".local")
                .join("share"),
        }
        .join("dotty");

        Ok(storage_dir)
    }
    pub fn discover() -> Result<Self, InitError> {
        let config_dir = StorageHandler::discover_config_dir()?;
        let storage_dir = StorageHandler::discover_storage_dir()?;

        Ok(Self {
            config_dir,
            storage_dir,
        })
    }

    pub fn config_dir(&self) -> &PathBuf {
        &self.config_dir
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join("config.toml")
    }

    pub fn storage_dir(&self) -> &PathBuf {
        &self.storage_dir
    }
}
