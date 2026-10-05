use std::{
    fs,
    io::Write,
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
}

#[cfg(test)]
mod tests {
    use std::{fs, os::unix::fs::symlink, path::PathBuf, process::Command};

    use tempfile::tempdir;

    use crate::infrastructure::filesystem::atomic_write_file;

    use super::{FileSystemError, PathKind, copy_dir, path_kind};

    #[test]
    fn path_kind_identifies_regular_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("file");

        fs::write(&path, "content").unwrap();

        let kind = path_kind(&path).unwrap();

        assert_eq!(kind, Some(PathKind::File));
    }

    #[test]
    fn path_kind_identifies_directory() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("directory");

        fs::create_dir(&path).unwrap();

        let kind = path_kind(&path).unwrap();

        assert_eq!(kind, Some(PathKind::Directory));
    }

    #[test]
    fn path_kind_identifies_symlink() {
        let temp = tempdir().unwrap();

        let target = temp.path().join("target");
        let link = temp.path().join("link");

        fs::write(&target, "content").unwrap();
        symlink(&target, &link).unwrap();

        let kind = path_kind(&link).unwrap();

        assert_eq!(kind, Some(PathKind::Symlink));
    }

    #[test]
    fn path_kind_identifies_broken_symlink() {
        let temp = tempdir().unwrap();

        let target = temp.path().join("missing-target");
        let link = temp.path().join("link");

        symlink(&target, &link).unwrap();

        let kind = path_kind(&link).unwrap();

        assert_eq!(kind, Some(PathKind::Symlink));
    }

    #[test]
    fn path_kind_returns_none_for_missing_path() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("missing");

        let kind = path_kind(&path).unwrap();

        assert_eq!(kind, None);
    }

    #[test]
    fn copy_dir_preserves_relative_symlink() {
        let temp = tempdir().unwrap();

        let source = temp.path().join("source");
        let destination = temp.path().join("destination");

        fs::create_dir(&source).unwrap();
        fs::write(source.join("target.txt"), "content").unwrap();

        symlink("target.txt", source.join("link.txt")).unwrap();

        copy_dir(&source, &destination).unwrap();

        let copied_link = destination.join("link.txt");

        assert!(
            fs::symlink_metadata(&copied_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(
            fs::read_link(&copied_link).unwrap(),
            PathBuf::from("target.txt")
        );

        assert_eq!(fs::read_to_string(&copied_link).unwrap(), "content");
    }

    #[test]
    fn copy_dir_preserves_absolute_symlink() {
        let temp = tempdir().unwrap();

        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        let target = temp.path().join("target.txt");

        fs::create_dir(&source).unwrap();
        fs::write(&target, "content").unwrap();

        symlink(&target, source.join("link.txt")).unwrap();

        copy_dir(&source, &destination).unwrap();

        let copied_link = destination.join("link.txt");

        assert!(
            fs::symlink_metadata(&copied_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(fs::read_link(&copied_link).unwrap(), target);
    }

    #[test]
    fn copy_dir_preserves_symlink_to_directory() {
        let temp = tempdir().unwrap();

        let source = temp.path().join("source");
        let destination = temp.path().join("destination");

        fs::create_dir(&source).unwrap();
        fs::create_dir(source.join("actual")).unwrap();
        fs::write(source.join("actual").join("file.txt"), "content").unwrap();

        symlink("actual", source.join("linked")).unwrap();

        copy_dir(&source, &destination).unwrap();

        let copied_link = destination.join("linked");

        assert!(
            fs::symlink_metadata(&copied_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(
            fs::read_link(&copied_link).unwrap(),
            PathBuf::from("actual")
        );

        assert_eq!(
            fs::read_to_string(copied_link.join("file.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn copy_dir_preserves_broken_symlink() {
        let temp = tempdir().unwrap();

        let source = temp.path().join("source");
        let destination = temp.path().join("destination");

        fs::create_dir(&source).unwrap();

        symlink("does-not-exist", source.join("broken")).unwrap();

        copy_dir(&source, &destination).unwrap();

        let copied_link = destination.join("broken");

        assert!(
            fs::symlink_metadata(&copied_link)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(
            fs::read_link(&copied_link).unwrap(),
            PathBuf::from("does-not-exist")
        );

        assert!(!copied_link.exists());
    }

    #[test]
    fn copy_dir_removes_partial_destination_when_copy_fails() {
        let temp = tempdir().unwrap();

        let source = temp.path().join("source");
        let destination = temp.path().join("destination");

        fs::create_dir(&source).unwrap();
        fs::write(source.join("file.txt"), "content").unwrap();

        let unsupported = source.join("unsupported");

        let status = Command::new("mkfifo").arg(&unsupported).status().unwrap();

        assert!(status.success());

        let result = copy_dir(&source, &destination);

        assert!(matches!(result, Err(FileSystemError::CopyDirectory { .. })));

        assert!(
            !destination.exists(),
            "partial destination should be removed after copy failure"
        );
    }

    #[test]
    fn atomic_write_file_creates_missing_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");

        atomic_write_file(&path, b"content").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"content");
    }

    #[test]
    fn atomic_write_file_replaces_existing_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");

        fs::write(&path, b"old content").unwrap();

        atomic_write_file(&path, b"new content").unwrap();

        assert_eq!(fs::read(&path).unwrap(), b"new content");
    }

    #[test]
    fn atomic_write_file_does_not_leave_temporary_file() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");

        atomic_write_file(&path, b"content").unwrap();

        let entries: Vec<_> = fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();

        assert_eq!(entries, vec![path]);
    }

    #[test]
    fn atomic_write_file_rejects_missing_parent_directory() {
        let temp = tempdir().unwrap();

        let path = temp.path().join("missing").join("config.toml");

        let result = atomic_write_file(&path, b"content");

        assert!(matches!(
            result,
            Err(FileSystemError::AtomicWriteFile {
                path: error_path,
                ..
            }) if error_path == path
        ));

        assert!(!path.exists());
    }

    #[test]
    fn atomic_write_file_rejects_directory_as_destination() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");

        fs::create_dir(&path).unwrap();

        let result = atomic_write_file(&path, b"content");

        assert!(matches!(
            result,
            Err(FileSystemError::AtomicWriteFile {
                path: error_path,
                ..
            }) if error_path == path
        ));

        assert!(path.is_dir());
    }

    #[test]
    fn atomic_write_file_preserves_existing_file_when_replacement_fails() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("config.toml");

        fs::write(&path, b"original").unwrap();

        let mut permissions = fs::metadata(temp.path()).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(temp.path(), permissions).unwrap();

        let result = atomic_write_file(&path, b"replacement");

        let mut permissions = fs::metadata(temp.path()).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(temp.path(), permissions).unwrap();

        assert!(result.is_err());

        assert_eq!(fs::read(&path).unwrap(), b"original");
    }
}
