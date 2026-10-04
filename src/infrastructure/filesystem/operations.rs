use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

pub fn rename(from: &Path, to: &Path) -> Result<(), FileSystemError> {
    fs::rename(from, to).map_err(|source| FileSystemError::Rename {
        from: from.to_path_buf(),
        to: to.to_path_buf(),
        source,
    })
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
pub fn write_file(path: &Path, content: &[u8]) -> Result<(), FileSystemError> {
    fs::write(path, content).map_err(|source| FileSystemError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
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

// TODO: Rework the filetype distinction, and handle them correctly
pub fn copy_dir(from: &Path, to: &Path) -> Result<(), FileSystemError> {
    if let Err(source) = copy_dir_inner(from, to) {
        let _ = fs::remove_dir_all(to);

        return Err(FileSystemError::CopyDirectory {
            from: from.to_path_buf(),
            to: to.to_path_buf(),
            source,
        });
    }

    Ok(())
}

fn copy_dir_inner(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    fs::create_dir(to)?;

    for entry in fs::read_dir(from)? {
        let entry = entry?;

        let source = entry.path();
        let destination = to.join(entry.file_name());

        if source.is_dir() {
            copy_dir_inner(&source, &destination)?;
        } else {
            fs::copy(&source, &destination)?;
        }
    }
    Ok(())
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
    #[error("failed to write file: `{path}`")]
    WriteFile {
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
}
