use std::{fs, path::Path};

pub fn create_dir(dir: &Path) -> Result<(), FileSystemError> {
    if fs::create_dir_all(dir).is_err() {
        return Err(FileSystemError::DirectoryCreation);
    }
    Ok(())
}

pub fn create_file(path: &Path, content: String) -> Result<(), FileSystemError> {
    if fs::write(path, content).is_err() {
        return Err(FileSystemError::FileCreation);
    }
    Ok(())
}

// TODO: Rework the filetype distinction, and handle them correctly
pub fn copy_dir(source: &Path, destination: &Path) -> Result<(), FileSystemError> {
    if destination.exists() {
        fs::remove_dir_all(destination)?;
    }

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
pub enum FileSystemError {
    #[error("Failed to create file")]
    FileCreation,
    #[error("Failed to create directory")]
    DirectoryCreation,
    #[error("Failed while performing filesystem operation")]
    Io(#[from] std::io::Error),
}
