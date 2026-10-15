use std::path::Path;

use crate::{
    domain::{EntryKind, ManagedEntry, ManagedEntryInspection, ManagedEntryIssue},
    infrastructure::filesystem::{self, FileSystemError, PathKind},
};

pub struct ManagedEntryInspector;

impl ManagedEntryInspector {
    pub fn inspect(
        entry: &ManagedEntry,
        repo_dir: &Path,
        home_dir: &Path,
    ) -> Result<ManagedEntryInspection, InspectionError> {
        let mut issues = Vec::new();

        let source = repo_dir.join(entry.source().path());
        let target = home_dir.join(entry.target().path());

        match filesystem::path_kind(&source)? {
            Some(PathKind::File) if entry.kind() == &EntryKind::File => {}
            Some(PathKind::Directory) if entry.kind() == &EntryKind::Dir => {}
            Some(_) => issues.push(ManagedEntryIssue::WrongSourceKind),
            None => issues.push(ManagedEntryIssue::MissingSource),
        }

        match filesystem::path_kind(&target)? {
            Some(PathKind::Symlink) => {
                let actual_destination = std::fs::read_link(&target)?;

                if actual_destination != source {
                    issues.push(ManagedEntryIssue::WrongSymlinkDestination);
                }
            }
            Some(_) => issues.push(ManagedEntryIssue::TargetNotSymlink),
            None => issues.push(ManagedEntryIssue::MissingTarget),
        }

        Ok(ManagedEntryInspection::new(entry.clone(), issues))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InspectionError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
#[path = "tests/inspection.rs"]
mod tests;
