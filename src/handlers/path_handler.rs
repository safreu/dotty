use std::{fs, io::Error, path::PathBuf};

use crate::handlers::path_handler::PathError::{DirectoryCreation, FileCreation};

pub fn create_dir(dir: &PathBuf) -> Result<(), PathError> {
    if fs::create_dir_all(dir).is_err() {
        return Err(DirectoryCreation);
    }
    Ok(())
}

pub fn create_file(path: PathBuf, content: String) -> Result<(), PathError> {
    if fs::write(path, content).is_err() {
        return Err(FileCreation);
    }
    Ok(())
}

// TODO: Rework the filetype distinction, and handle them correctly
pub fn copy_dir(source: &PathBuf, destination: &PathBuf) -> Result<(), PathError> {
    create_dir(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;

        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir(&source_path, &destination_path)?;
        } else {
            fs::copy(&source_path, &destination_path)?;
        }
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("Failed to create file")]
    FileCreation,
    #[error("Failed to create directory")]
    DirectoryCreation,
    #[error("Failed while copying directory")]
    DirectoryCopy(#[from] Error),
}
