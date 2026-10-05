use std::path::PathBuf;

use crate::infrastructure::filesystem::{self, FileSystemError, PathKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    FileSystem(FileSystemAction),
}

impl Action {
    pub fn execute(self) -> Result<ExecutedAction, ActionError> {
        match self {
            Action::FileSystem(action) => {
                let executed = action.execute()?;

                Ok(ExecutedAction::new(
                    executed.rollback().map(Action::FileSystem),
                ))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ActionError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileSystemAction {
    CreateDirectory(PathBuf),
    RemoveDirectory(PathBuf),

    WriteFile { path: PathBuf, contents: Vec<u8> },
    RemoveFile(PathBuf),

    Rename { from: PathBuf, to: PathBuf },

    CreateSymlink { target: PathBuf, link: PathBuf },
    RemoveSymlink(PathBuf),

    CopyFile { from: PathBuf, to: PathBuf },
    CopyDirectory { from: PathBuf, to: PathBuf },
    RemoveDirectoryAll(PathBuf),
}

impl FileSystemAction {
    pub fn execute(self) -> Result<ExecutedFileSystemAction, FileSystemError> {
        match self {
            FileSystemAction::CreateDirectory(path) => {
                filesystem::create_dir(&path)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveDirectory(path),
                )))
            }
            FileSystemAction::RemoveDirectory(path) => {
                filesystem::remove_dir(&path)?;

                Ok(ExecutedFileSystemAction::new(None))
            }

            FileSystemAction::WriteFile { path, contents } => {
                let previous = match filesystem::path_kind(&path)? {
                    Some(PathKind::File) => Some(filesystem::read_file(&path)?),
                    Some(_) => return Err(FileSystemError::InvalidWriteTarget { path }),
                    None => None,
                };

                filesystem::atomic_write_file(&path, &contents)?;

                let rollback = match previous {
                    Some(contents) => FileSystemAction::WriteFile { path, contents },
                    None => FileSystemAction::RemoveFile(path),
                };

                Ok(ExecutedFileSystemAction::new(Some(rollback)))
            }
            FileSystemAction::RemoveFile(path) => {
                filesystem::remove_file(&path)?;

                Ok(ExecutedFileSystemAction::new(None))
            }

            FileSystemAction::Rename { from, to } => {
                filesystem::rename(&from, &to)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::Rename { from: to, to: from },
                )))
            }

            FileSystemAction::CreateSymlink { target, link } => {
                filesystem::create_symlink(&target, &link)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveSymlink(link),
                )))
            }
            FileSystemAction::RemoveSymlink(link) => {
                let target = filesystem::read_symlink(&link)?;

                filesystem::remove_symlink(&link)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::CreateSymlink { target, link },
                )))
            }

            FileSystemAction::CopyDirectory { from, to } => {
                filesystem::copy_dir(&from, &to)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveDirectoryAll(to),
                )))
            }
            FileSystemAction::CopyFile { from, to } => {
                filesystem::copy_file(&from, &to)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveFile(to),
                )))
            }
            FileSystemAction::RemoveDirectoryAll(path) => {
                filesystem::remove_dir_all(&path)?;

                Ok(ExecutedFileSystemAction::new(None))
            }
        }
    }
}

impl From<FileSystemAction> for Action {
    fn from(value: FileSystemAction) -> Self {
        Self::FileSystem(value)
    }
}

pub struct ExecutedAction {
    rollback: Option<Action>,
}

impl ExecutedAction {
    pub fn new(rollback: Option<Action>) -> Self {
        Self { rollback }
    }

    pub fn rollback(self) -> Option<Action> {
        self.rollback
    }
}

pub struct ExecutedFileSystemAction {
    rollback: Option<FileSystemAction>,
}

impl ExecutedFileSystemAction {
    pub fn new(rollback: Option<FileSystemAction>) -> Self {
        Self { rollback }
    }

    pub fn rollback(self) -> Option<FileSystemAction> {
        self.rollback
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::symlink};

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn write_file_creates_missing_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");

        let action = FileSystemAction::WriteFile {
            path: path.clone(),
            contents: b"new content".to_vec(),
        };

        action.execute().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new content");
    }

    #[test]
    fn write_file_overwrites_existing_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");

        fs::write(&path, "old content").unwrap();

        let action = FileSystemAction::WriteFile {
            path: path.clone(),
            contents: b"new content".to_vec(),
        };

        action.execute().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new content");
    }

    #[test]
    fn write_file_rollback_removes_new_file() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");

        let action = FileSystemAction::WriteFile {
            path: path.clone(),
            contents: b"new content".to_vec(),
        };

        let executed = action.execute().unwrap();

        assert!(path.exists());

        executed.rollback().unwrap().execute().unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn write_file_rollback_restores_previous_contents() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("config.toml");

        fs::write(&path, "old content").unwrap();

        let action = FileSystemAction::WriteFile {
            path: path.clone(),
            contents: b"new content".to_vec(),
        };

        let executed = action.execute().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new content");

        executed.rollback().unwrap().execute().unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"old content");
    }

    #[test]
    fn write_file_rejects_directory() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("directory");

        fs::create_dir(&path).unwrap();

        let action = FileSystemAction::WriteFile {
            path: path.clone(),
            contents: b"content".to_vec(),
        };

        let result = action.execute();

        assert!(matches!(
            result,
            Err(FileSystemError::InvalidWriteTarget { path: error_path })
                if error_path == path
        ));
    }

    #[test]
    fn write_file_rejects_symlink() {
        let temp = TempDir::new().unwrap();

        let target = temp.path().join("target");
        let link = temp.path().join("link");

        fs::write(&target, "original").unwrap();
        symlink(&target, &link).unwrap();

        let action = FileSystemAction::WriteFile {
            path: link,
            contents: b"modified".to_vec(),
        };

        let result = action.execute();

        assert!(matches!(
            result,
            Err(FileSystemError::InvalidWriteTarget { .. })
        ));

        assert_eq!(fs::read(&target).unwrap(), b"original");
    }

    #[test]
    fn write_file_rejects_broken_symlink() {
        let temp = TempDir::new().unwrap();

        let target = temp.path().join("missing");
        let link = temp.path().join("link");

        symlink(&target, &link).unwrap();

        let action = FileSystemAction::WriteFile {
            path: link.clone(),
            contents: b"content".to_vec(),
        };

        let result = action.execute();

        assert!(matches!(
            result,
            Err(FileSystemError::InvalidWriteTarget { path })
                if path == link
        ));

        assert!(!target.exists());
    }
}
