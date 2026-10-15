use std::{
    fs,
    io::Write,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

pub fn rename(from: &Path, to: &Path) -> Result<(), FileSystemError> {
    rename_noreplace(from, to).map_err(|source| FileSystemError::Rename {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(Into::into)
}

pub fn create_dir(path: &Path) -> Result<(), FileSystemError> {
    fs::create_dir(path).map_err(|source| FileSystemError::CreateDirectory {
        path: path.to_path_buf(),
        source,
    })
}
pub fn remove_dir(path: &Path) -> Result<(), FileSystemError> {
    fs::remove_dir(path).map_err(|source| FileSystemError::RemoveDirectory {
        path: path.to_path_buf(),
        source,
    })
}
pub fn remove_dir_all(path: &Path) -> Result<(), FileSystemError> {
    fs::remove_dir_all(path).map_err(|source| FileSystemError::RemoveDirectoryAll {
        path: path.to_path_buf(),
        source,
    })
}

pub fn read_file(path: &Path) -> Result<Vec<u8>, FileSystemError> {
    fs::read(path).map_err(|source| FileSystemError::ReadFile {
        path: path.to_path_buf(),
        source,
    })
}
pub fn atomic_write_file(path: &Path, content: &[u8]) -> Result<(), FileSystemError> {
    let parent = path
        .parent()
        .ok_or_else(|| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "file has no parent directory",
            ),
        })?;

    let mut tmp = tempfile::NamedTempFile::new_in(parent).map_err(|source| {
        FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source,
        }
    })?;

    tmp.write_all(content)
        .map_err(|source| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source,
        })?;

    tmp.flush()
        .map_err(|source| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source,
        })?;

    tmp.as_file()
        .sync_all()
        .map_err(|source| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source,
        })?;

    tmp.persist(path)
        .map_err(|error| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source: error.error,
        })?;

    fs::File::open(parent)
        .and_then(|dir| dir.sync_all())
        .map_err(|source| FileSystemError::AtomicWriteFile {
            path: path.to_path_buf(),
            source,
        })?;

    Ok(())
}
pub fn remove_file(path: &Path) -> Result<(), FileSystemError> {
    fs::remove_file(path).map_err(|source| FileSystemError::RemoveFile {
        path: path.to_path_buf(),
        source,
    })
}

pub fn create_symlink(target: &Path, link: &Path) -> Result<(), FileSystemError> {
    symlink(target, link).map_err(|source| FileSystemError::CreateSymlink {
        target: target.to_path_buf(),
        link: link.to_path_buf(),
        source,
    })
}
pub fn read_symlink(link: &Path) -> Result<PathBuf, FileSystemError> {
    fs::read_link(link).map_err(|source| FileSystemError::ReadSymlink {
        link: link.to_path_buf(),
        source,
    })
}
pub fn remove_symlink(link: &Path) -> Result<(), FileSystemError> {
    fs::remove_file(link).map_err(|source| FileSystemError::RemoveSymlink {
        link: link.to_path_buf(),
        source,
    })
}

pub fn copy_file(from: &Path, to: &Path) -> Result<(), FileSystemError> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|source| FileSystemError::CopyFile {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source,
        })
}

pub fn copy_dir(from: &Path, to: &Path) -> Result<(), FileSystemError> {
    copy_dir_impl(from, to).map_err(|source| FileSystemError::CopyDirectory {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
}

fn copy_dir_impl(from: &Path, to: &Path) -> std::io::Result<()> {
    let parent = to.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "destination has no parent directory",
        )
    })?;

    let staging = tempfile::Builder::new()
        .prefix(".dotty-copy-")
        .tempdir_in(parent)?;

    let staged_copy = staging.path().join("contents");

    copy_dir_inner(from, &staged_copy)?;

    rename_noreplace(&staged_copy, to)?;

    staging.close()?;

    Ok(())
}

fn copy_dir_inner(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    fs::create_dir(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;

        let source = entry.path();
        let destination = to.join(entry.file_name());

        let metadata = fs::symlink_metadata(&source)?;
        let file_type = metadata.file_type();

        if file_type.is_symlink() {
            let target = fs::read_link(&source)?;
            symlink(target, destination)?;
        } else if file_type.is_dir() {
            copy_dir_inner(&source, &destination)?;
        } else if file_type.is_file() {
            fs::copy(&source, &destination)?;
        } else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                format!("Unsupported filesystem entry: {}", source.display()),
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    File,
    Directory,
    Symlink,
    Other,
}

pub fn path_kind(path: &Path) -> Result<Option<PathKind>, FileSystemError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,

        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),

        Err(source) => {
            return Err(FileSystemError::InspectPath {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        Ok(Some(PathKind::Symlink))
    } else if file_type.is_file() {
        Ok(Some(PathKind::File))
    } else if file_type.is_dir() {
        Ok(Some(PathKind::Directory))
    } else {
        Ok(Some(PathKind::Other))
    }
}

pub fn missing_directories(root: &Path, target: &Path) -> Result<Vec<PathBuf>, FileSystemError> {
    let mut current = root.to_path_buf();
    let mut missing = Vec::new();

    match path_kind(root)? {
        Some(PathKind::Directory) => {}
        _ => {
            return Err(FileSystemError::InvalidDirectory {
                path: root.to_path_buf(),
            });
        }
    };

    let relative = target
        .strip_prefix(root)
        .map_err(|_| FileSystemError::PathOutsideRoot {
            root: root.to_path_buf(),
            target: target.to_path_buf(),
        })?;

    for component in relative.components() {
        current.push(component);

        match path_kind(&current)? {
            Some(PathKind::Directory) => {}
            None => missing.push(current.clone()),
            Some(_) => return Err(FileSystemError::InvalidDirectory { path: current }),
        }
    }

    Ok(missing)
}

#[derive(Debug, thiserror::Error)]
pub enum FileSystemError {
    #[error("failed to rename `{from}` to `{to}`")]
    Rename {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create directory: `{path}`")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove directory: `{path}`")]
    RemoveDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to recursively remove directory: `{path}`")]
    RemoveDirectoryAll {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to read file: `{path}`")]
    ReadFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove file: `{path}`")]
    RemoveFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to create symlink `{link}` pointing to `{target}`")]
    CreateSymlink {
        target: PathBuf,
        link: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read symlink `{link}`")]
    ReadSymlink {
        link: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove symlink `{link}`")]
    RemoveSymlink {
        link: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to copy file `{from}` to `{to}`")]
    CopyFile {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to copy directory `{from}` to `{to}`")]
    CopyDirectory {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to inspect path `{path}`")]
    InspectPath {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("cannot write to unsupported filesystem entry: `{path}`")]
    InvalidWriteTarget { path: PathBuf },

    #[error("failed to atomically write file `{path}`")]
    AtomicWriteFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("expected directory at `{path}`")]
    InvalidDirectory { path: PathBuf },

    #[error("path `{target}` is outside root `{root}`")]
    PathOutsideRoot { root: PathBuf, target: PathBuf },
}

#[cfg(test)]
#[path = "tests/operations.rs"]
mod tests;
