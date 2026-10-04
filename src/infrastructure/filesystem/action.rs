use std::{fs, os::unix::fs::symlink, path::PathBuf};

use crate::infrastructure::filesystem::{self, FileSystemError};

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
                fs::remove_dir(path)?;

                Ok(ExecutedFileSystemAction::new(None))
            }

            FileSystemAction::WriteFile { path, contents } => {
                let previous = if path.exists() {
                    Some(fs::read(&path)?)
                } else {
                    None
                };

                fs::write(&path, contents)?;

                let rollback = match previous {
                    Some(contents) => FileSystemAction::WriteFile { path, contents },
                    None => FileSystemAction::RemoveFile(path),
                };

                Ok(ExecutedFileSystemAction::new(Some(rollback)))
            }
            FileSystemAction::RemoveFile(path) => {
                fs::remove_file(path)?;

                Ok(ExecutedFileSystemAction::new(None))
            }

            FileSystemAction::Rename { from, to } => {
                fs::rename(&from, &to)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::Rename { from: to, to: from },
                )))
            }

            FileSystemAction::CreateSymlink { target, link } => {
                symlink(&target, &link)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveSymlink(link),
                )))
            }
            FileSystemAction::RemoveSymlink(link) => {
                let target = fs::read_link(&link)?;

                fs::remove_file(&link)?;

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
                fs::copy(&from, &to)?;

                Ok(ExecutedFileSystemAction::new(Some(
                    FileSystemAction::RemoveFile(to),
                )))
            }
            FileSystemAction::RemoveDirectoryAll(path) => {
                fs::remove_dir_all(path)?;

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
