use std::{fs, os::unix::fs::symlink, path::PathBuf};

use crate::infrastructure::filesystem::{self, FileSystemError};

pub enum RollbackAction {
    #[allow(unused)]
    RemoveFile(PathBuf),
    #[allow(unused)]
    RemoveDir(PathBuf),
    RemoveSymlink(PathBuf),

    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    #[allow(unused)]
    RestoreFile {
        backup: PathBuf,
        destination: PathBuf,
    },
    #[allow(unused)]
    RestoreDir {
        backup: PathBuf,
        destination: PathBuf,
    },

    CreateSymlink {
        source: PathBuf,
        destination: PathBuf,
    },
}
pub struct RollbackStack {
    actions: Vec<RollbackAction>,
}

impl RollbackStack {
    pub fn new() -> Self {
        Self { actions: vec![] }
    }

    pub fn register(&mut self, action: RollbackAction) {
        self.actions.push(action)
    }

    pub fn rollback(&mut self) -> Result<(), RollbackError> {
        let mut errors = Vec::new();
        while let Some(action) = self.actions.pop() {
            match action {
                RollbackAction::RemoveDir(path) => {
                    if let Err(e) = fs::remove_dir_all(&path) {
                        errors.push(RollbackActionError::RemoveDir { path, source: e });
                    };
                }
                RollbackAction::RemoveFile(path) => {
                    if let Err(e) = fs::remove_file(&path) {
                        errors.push(RollbackActionError::RemoveFile { path, source: e });
                    };
                }
                RollbackAction::RemoveSymlink(path) => {
                    if let Err(e) = fs::remove_file(&path) {
                        errors.push(RollbackActionError::RemoveSymlink { path, source: e });
                    };
                }
                RollbackAction::Rename { from, to } => {
                    if let Err(e) = fs::rename(&from, &to) {
                        errors.push(RollbackActionError::Rename {
                            from,
                            to,
                            source: e,
                        });
                    };
                }
                RollbackAction::RestoreDir {
                    backup,
                    destination,
                } => {
                    if let Err(e) = filesystem::copy_dir(&backup, &destination) {
                        errors.push(RollbackActionError::RestoreDir {
                            backup,
                            destination,
                            source: e,
                        });
                    };
                }
                RollbackAction::RestoreFile {
                    backup,
                    destination,
                } => {
                    if let Err(e) = fs::copy(&backup, &destination) {
                        errors.push(RollbackActionError::RestoreFile {
                            backup,
                            destination,
                            source: e,
                        });
                    };
                }
                RollbackAction::CreateSymlink {
                    source,
                    destination,
                } => {
                    if let Err(e) = symlink(&source, &destination) {
                        errors.push(RollbackActionError::CreateSymlink {
                            source,
                            destination,
                            source_error: e,
                        })
                    };
                }
            };
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(RollbackError { errors })
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("One or more rollback actions failed")]
pub struct RollbackError {
    errors: Vec<RollbackActionError>,
}

#[derive(Debug, thiserror::Error)]
pub enum RollbackActionError {
    #[error("Failed to remove file {path}")]
    RemoveFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to remove dir {path}")]
    RemoveDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to remove symlink {path}")]
    RemoveSymlink {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to rename {from} to {to}")]
    Rename {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to restore dir {backup} to {destination}")]
    RestoreDir {
        backup: PathBuf,
        destination: PathBuf,
        #[source]
        source: FileSystemError,
    },
    #[error("Failed to restore file {backup} to {destination}")]
    RestoreFile {
        backup: PathBuf,
        destination: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to recreate symlink {destination} to {source}")]
    CreateSymlink {
        source: PathBuf,
        destination: PathBuf,
        #[source]
        source_error: std::io::Error,
    },
}
