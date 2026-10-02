use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::PathBuf};

use crate::handlers::path_handler::{self, PathError};

pub fn read_toml<T>(path: PathBuf) -> Result<T, TomlError>
where
    T: DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|_| TomlError::Read)?;

    let value: T = toml::from_str(&content).map_err(|_| TomlError::Read)?;

    Ok(value)
}

pub fn write_toml<T>(to_write: &T, path: PathBuf) -> Result<(), TomlError>
where
    T: Serialize,
{
    let content = toml::to_string_pretty(to_write).map_err(|_| TomlError::Write)?;

    path_handler::create_file(path, content)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum TomlError {
    #[error("Failed to read toml")]
    Read,
    #[error("Failed to write toml")]
    Write,
    #[error(transparent)]
    PathCreationFailed(#[from] PathError),
}
