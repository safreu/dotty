use std::io::Write;

use crate::{
    application::{DotfileManager, DotfileManagerLoadError, InspectionError},
    infrastructure::persistence::paths::{DottyPaths, DottyPathsError},
    presentation::print_entries,
};

pub fn execute() -> Result<(), LsCommandError> {
    let paths = DottyPaths::discover()?;

    let stdout = std::io::stdout();
    let mut writer = stdout.lock();

    execute_with_paths(&paths, &mut writer)
}

fn execute_with_paths(paths: &DottyPaths, writer: &mut impl Write) -> Result<(), LsCommandError> {
    let manager = DotfileManager::load(paths)?;
    let inspections = manager.inspect()?;

    print_entries(writer, &inspections)?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LsCommandError {
    #[error(transparent)]
    ManagerLoad(#[from] DotfileManagerLoadError),
    #[error(transparent)]
    Inspection(#[from] InspectionError),
    #[error(transparent)]
    DottyPaths(#[from] DottyPathsError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{self, Write},
        os::unix::fs::symlink,
        path::PathBuf,
    };

    use tempfile::TempDir;

    use super::*;
    use crate::{
        domain::{
            BackupRoot, ConfigRoot, EntryKind, ManagedEntry, ManagedSource, ManagedTarget,
            ManifestRoot,
        },
        infrastructure::persistence::{
            ManifestRepository, RepositoryPaths, backup::BackupRepository, config::ConfigRepository,
        },
    };

    struct TestContext {
        _temp: TempDir,
        paths: DottyPaths,
        repo_dir: PathBuf,
        home_dir: PathBuf,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let repo_dir = temp.path().join("repo");
            let home_dir = temp.path().join("home");

            let paths = DottyPaths::new(config_dir, storage_dir, home_dir.clone());

            fs::create_dir_all(&repo_dir).unwrap();
            fs::create_dir_all(&home_dir).unwrap();

            fs::create_dir_all(paths.config_file().parent().unwrap()).unwrap();

            fs::create_dir_all(paths.backup_file().parent().unwrap()).unwrap();

            let repository_paths = RepositoryPaths::new(repo_dir.clone());

            fs::create_dir_all(repository_paths.metadata_dir()).unwrap();

            ConfigRepository::new(paths.config_file())
                .write(&ConfigRoot::new(repo_dir.clone(), home_dir.clone()))
                .unwrap();

            BackupRepository::new(paths.backup_file())
                .write(&BackupRoot::new())
                .unwrap();

            ManifestRepository::new(&repository_paths.manifest_file())
                .write(&ManifestRoot::new())
                .unwrap();

            Self {
                _temp: temp,
                paths,
                repo_dir,
                home_dir,
            }
        }

        fn manifest_file(&self) -> PathBuf {
            RepositoryPaths::new(self.repo_dir.clone()).manifest_file()
        }

        fn add_entry(&self, source: &str, target: &str) {
            let repository = ManifestRepository::new(&self.manifest_file());

            let mut manifest = repository.read().unwrap();

            manifest
                .add(ManagedEntry::new(
                    ManagedSource::new(PathBuf::from(source)).unwrap(),
                    ManagedTarget::home_relative(PathBuf::from(target)).unwrap(),
                    EntryKind::File,
                ))
                .unwrap();

            repository.write(&manifest).unwrap();
        }

        fn create_healthy_entry(&self, source: &str, target: &str) {
            let source_path = self.repo_dir.join(source);
            let target_path = self.home_dir.join(target);

            fs::create_dir_all(source_path.parent().unwrap()).unwrap();
            fs::create_dir_all(target_path.parent().unwrap()).unwrap();

            fs::write(&source_path, "test content").unwrap();
            symlink(&source_path, &target_path).unwrap();

            self.add_entry(source, target);
        }

        fn run(&self) -> String {
            let mut output = Vec::new();

            execute_with_paths(&self.paths, &mut output).unwrap();

            let output = String::from_utf8(output).unwrap();

            strip_ansi(&output)
        }
    }

    fn strip_ansi(text: &str) -> String {
        let mut result = String::new();
        let mut chars = text.chars().peekable();

        while let Some(character) = chars.next() {
            if character == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();

                for character in chars.by_ref() {
                    if character.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                result.push(character);
            }
        }

        result
    }

    #[test]
    fn ls_command_succeeds_with_empty_manifest() {
        let context = TestContext::new();

        let output = context.run();

        assert_eq!(output, "No managed entries found.\n");
    }

    #[test]
    fn ls_command_lists_healthy_entry() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");

        let output = context.run();

        assert!(output.contains("SOURCE"));
        assert!(output.contains("TARGET"));
        assert!(output.contains("STATUS"));
        assert!(output.contains("zshrc"));
        assert!(output.contains("~/.zshrc"));
        assert!(output.contains("OK"));
    }

    #[test]
    fn ls_command_succeeds_with_unhealthy_entry() {
        let context = TestContext::new();

        context.add_entry("nvim", ".config/nvim");

        let mut output = Vec::new();

        let result = execute_with_paths(&context.paths, &mut output);

        assert!(result.is_ok());

        let output = strip_ansi(&String::from_utf8(output).unwrap());

        assert!(output.contains("nvim"));
        assert!(output.contains("~/.config/nvim"));
        assert!(output.contains("UNHEALTHY"));
    }

    #[test]
    fn ls_command_lists_multiple_entries() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");
        context.add_entry("nvim", ".config/nvim");
        context.create_healthy_entry("ghostty/config", ".config/ghostty/config");

        let output = context.run();

        assert!(output.contains("~/.zshrc"));
        assert!(output.contains("~/.config/nvim"));
        assert!(output.contains("~/.config/ghostty/config"));

        assert!(output.contains("OK"));
        assert!(output.contains("UNHEALTHY"));
    }

    #[test]
    fn ls_command_does_not_print_detailed_issues() {
        let context = TestContext::new();

        context.add_entry("nvim", ".config/nvim");

        let output = context.run();

        assert!(output.contains("UNHEALTHY"));
        assert!(!output.contains("Missing source"));
        assert!(!output.contains("Missing target"));
    }

    #[test]
    fn ls_command_does_not_print_summary() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");
        context.add_entry("nvim", ".config/nvim");

        let output = context.run();

        assert!(!output.contains("1 healthy"));
        assert!(!output.contains("1 unhealthy"));
    }

    #[test]
    fn ls_command_propagates_manager_load_error() {
        let context = TestContext::new();

        fs::write(context.manifest_file(), "invalid toml = [").unwrap();

        let mut output = Vec::new();

        let result = execute_with_paths(&context.paths, &mut output);

        assert!(matches!(result, Err(LsCommandError::ManagerLoad(_))));

        assert!(output.is_empty());
    }

    #[test]
    fn ls_command_propagates_missing_config_error() {
        let context = TestContext::new();

        fs::remove_file(context.paths.config_file()).unwrap();

        let mut output = Vec::new();

        let result = execute_with_paths(&context.paths, &mut output);

        assert!(matches!(result, Err(LsCommandError::ManagerLoad(_))));

        assert!(output.is_empty());
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("simulated write failure"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn ls_command_propagates_output_error() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");

        let result = execute_with_paths(&context.paths, &mut FailingWriter);

        assert!(matches!(result, Err(LsCommandError::Io(_))));
    }

    #[test]
    fn ls_command_propagates_output_error_for_empty_manifest() {
        let context = TestContext::new();

        let result = execute_with_paths(&context.paths, &mut FailingWriter);

        assert!(matches!(result, Err(LsCommandError::Io(_))));
    }

    #[test]
    fn ls_command_does_not_modify_manifest() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");

        let before = fs::read(context.manifest_file()).unwrap();

        context.run();

        assert_eq!(fs::read(context.manifest_file()).unwrap(), before);
    }

    #[test]
    fn ls_command_does_not_modify_config_or_backups() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");

        let config_before = fs::read(context.paths.config_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        context.run();

        assert_eq!(
            fs::read(context.paths.config_file()).unwrap(),
            config_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );
    }

    #[test]
    fn ls_command_does_not_modify_managed_files() {
        let context = TestContext::new();

        context.create_healthy_entry("zshrc", ".zshrc");

        let source = context.repo_dir.join("zshrc");
        let target = context.home_dir.join(".zshrc");

        let source_before = fs::read(&source).unwrap();
        let target_before = fs::read_link(&target).unwrap();

        context.run();

        assert_eq!(fs::read(&source).unwrap(), source_before);

        assert_eq!(fs::read_link(&target).unwrap(), target_before);
    }
}
