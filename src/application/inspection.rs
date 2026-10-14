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
mod tests {
    use std::{
        fs,
        os::unix::fs::symlink,
        path::{Path, PathBuf},
    };

    use tempfile::TempDir;

    use super::*;
    use crate::domain::{EntryKind, ManagedSource, ManagedTarget};

    struct TestContext {
        _temp: TempDir,
        repo_dir: PathBuf,
        home_dir: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let repo_dir = temp.path().join("repo");
            let home_dir = temp.path().join("home");

            fs::create_dir(&repo_dir).unwrap();
            fs::create_dir(&home_dir).unwrap();

            Self {
                _temp: temp,
                repo_dir,
                home_dir,
            }
        }

        fn source(&self, path: &str) -> PathBuf {
            self.repo_dir.join(path)
        }

        fn target(&self, path: &str) -> PathBuf {
            self.home_dir.join(path)
        }

        fn entry(&self, source: &str, target: &str, kind: EntryKind) -> ManagedEntry {
            ManagedEntry::new(
                ManagedSource::new(PathBuf::from(source)).unwrap(),
                ManagedTarget::home_relative(PathBuf::from(target)).unwrap(),
                kind,
            )
        }

        fn create_source_file(&self, path: &str) {
            let source = self.source(path);

            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(source, "hello").unwrap();
        }

        fn create_source_directory(&self, path: &str) {
            fs::create_dir_all(self.source(path)).unwrap();
        }

        fn create_target_file(&self, path: &str) {
            let target = self.target(path);

            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, "hello").unwrap();
        }

        fn create_symlink(&self, source: &Path, target: &str) {
            let target = self.target(target);

            fs::create_dir_all(target.parent().unwrap()).unwrap();
            symlink(source, target).unwrap();
        }

        fn inspect(&self, entry: &ManagedEntry) -> ManagedEntryInspection {
            ManagedEntryInspector::inspect(entry, &self.repo_dir, &self.home_dir).unwrap()
        }
    }

    #[test]
    fn inspection_is_healthy_when_no_issues_exist() {
        let context = TestContext::new();

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = ManagedEntryInspection::new(entry.clone(), Vec::new());

        assert!(inspection.is_healthy());
        assert!(inspection.issues().is_empty());
        assert_eq!(inspection.entry(), &entry);
    }

    #[test]
    fn inspection_is_unhealthy_when_issues_exist() {
        let context = TestContext::new();

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection =
            ManagedEntryInspection::new(entry.clone(), vec![ManagedEntryIssue::MissingSource]);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::MissingSource]);

        assert_eq!(inspection.entry(), &entry);
    }

    #[test]
    fn inspector_reports_healthy_file_entry() {
        let context = TestContext::new();

        context.create_source_file("zshrc");

        let source = context.source("zshrc");
        context.create_symlink(&source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(inspection.is_healthy());
        assert!(inspection.issues().is_empty());
        assert_eq!(inspection.entry(), &entry);
    }

    #[test]
    fn inspector_reports_healthy_directory_entry() {
        let context = TestContext::new();

        context.create_source_directory("nvim");

        let source = context.source("nvim");
        context.create_symlink(&source, ".config/nvim");

        let entry = context.entry("nvim", ".config/nvim", EntryKind::Dir);

        let inspection = context.inspect(&entry);

        assert!(inspection.is_healthy());
        assert!(inspection.issues().is_empty());
    }

    #[test]
    fn inspector_detects_missing_source() {
        let context = TestContext::new();

        let source = context.source("zshrc");
        context.create_symlink(&source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::MissingSource]);
    }

    #[test]
    fn inspector_detects_missing_target() {
        let context = TestContext::new();

        context.create_source_file("zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::MissingTarget]);
    }

    #[test]
    fn inspector_reports_both_missing_source_and_target() {
        let context = TestContext::new();

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(
            inspection.issues(),
            &[
                ManagedEntryIssue::MissingSource,
                ManagedEntryIssue::MissingTarget,
            ]
        );
    }

    #[test]
    fn inspector_detects_regular_file_instead_of_symlink() {
        let context = TestContext::new();

        context.create_source_file("zshrc");
        context.create_target_file(".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::TargetNotSymlink]);
    }

    #[test]
    fn inspector_detects_directory_instead_of_symlink() {
        let context = TestContext::new();

        context.create_source_directory("nvim");

        fs::create_dir_all(context.target(".config/nvim")).unwrap();

        let entry = context.entry("nvim", ".config/nvim", EntryKind::Dir);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::TargetNotSymlink]);
    }

    #[test]
    fn inspector_detects_wrong_symlink_destination() {
        let context = TestContext::new();

        context.create_source_file("zshrc");
        context.create_source_file("other");

        let wrong_source = context.source("other");
        context.create_symlink(&wrong_source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(
            inspection.issues(),
            &[ManagedEntryIssue::WrongSymlinkDestination]
        );
    }

    #[test]
    fn inspector_detects_wrong_source_kind_when_file_expected() {
        let context = TestContext::new();

        context.create_source_directory("zshrc");

        let source = context.source("zshrc");
        context.create_symlink(&source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::WrongSourceKind]);
    }

    #[test]
    fn inspector_detects_wrong_source_kind_when_directory_expected() {
        let context = TestContext::new();

        context.create_source_file("nvim");

        let source = context.source("nvim");
        context.create_symlink(&source, ".config/nvim");

        let entry = context.entry("nvim", ".config/nvim", EntryKind::Dir);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::WrongSourceKind]);
    }

    #[test]
    fn inspector_detects_symlink_as_source() {
        let context = TestContext::new();

        context.create_source_file("actual");

        let source = context.source("zshrc");
        let actual = context.source("actual");

        symlink(&actual, &source).unwrap();

        context.create_symlink(&source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(inspection.issues(), &[ManagedEntryIssue::WrongSourceKind]);
    }

    #[test]
    fn inspector_detects_broken_symlink_with_wrong_destination() {
        let context = TestContext::new();

        context.create_source_file("zshrc");

        let nonexistent = context.source("nonexistent");
        context.create_symlink(&nonexistent, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(
            inspection.issues(),
            &[ManagedEntryIssue::WrongSymlinkDestination]
        );
    }

    #[test]
    fn inspector_reports_multiple_issues() {
        let context = TestContext::new();

        context.create_target_file(".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(
            inspection.issues(),
            &[
                ManagedEntryIssue::MissingSource,
                ManagedEntryIssue::TargetNotSymlink,
            ]
        );
    }

    #[test]
    fn inspector_reports_wrong_source_kind_and_wrong_destination() {
        let context = TestContext::new();

        context.create_source_directory("zshrc");
        context.create_source_file("other");

        let wrong_source = context.source("other");
        context.create_symlink(&wrong_source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let inspection = context.inspect(&entry);

        assert!(!inspection.is_healthy());

        assert_eq!(
            inspection.issues(),
            &[
                ManagedEntryIssue::WrongSourceKind,
                ManagedEntryIssue::WrongSymlinkDestination,
            ]
        );
    }

    #[test]
    fn inspector_does_not_modify_filesystem() {
        let context = TestContext::new();

        context.create_source_file("zshrc");

        let source = context.source("zshrc");
        context.create_symlink(&source, ".zshrc");

        let entry = context.entry("zshrc", ".zshrc", EntryKind::File);

        let source_before = fs::read(&source).unwrap();
        let target_before = fs::read_link(context.target(".zshrc")).unwrap();

        let inspection = context.inspect(&entry);

        assert!(inspection.is_healthy());

        assert_eq!(fs::read(&source).unwrap(), source_before);

        assert_eq!(
            fs::read_link(context.target(".zshrc")).unwrap(),
            target_before
        );
    }
}
