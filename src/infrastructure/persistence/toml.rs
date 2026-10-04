use serde::{Serialize, de::DeserializeOwned};
use std::{fs, path::Path};

use crate::infrastructure::filesystem::{self, FileSystemError};

pub fn read_toml<T>(path: &Path) -> Result<T, TomlError>
where
    T: DeserializeOwned,
{
    let content = fs::read_to_string(path).map_err(|_| TomlError::Read)?;

    let value: T = toml::from_str(&content).map_err(|_| TomlError::Read)?;

    Ok(value)
}

pub fn serialize_toml<T>(value: &T) -> Result<Vec<u8>, TomlError>
where
    T: Serialize,
{
    let content = toml::to_string_pretty(value).map_err(|_| TomlError::Write)?;

    Ok(content.into_bytes())
}

#[allow(unused)]
pub fn write_toml<T>(to_write: &T, path: &Path) -> Result<(), TomlError>
where
    T: Serialize,
{
    let content = toml::to_string_pretty(to_write).map_err(|_| TomlError::Write)?;

    filesystem::create_file(path, content)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum TomlError {
    #[error("Failed to read toml")]
    Read,
    #[error("Failed to write toml")]
    Write,
    #[error(transparent)]
    PathCreationFailed(#[from] FileSystemError),
}
