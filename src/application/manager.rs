use std::path::{Path, PathBuf};

use crate::{
    application::{
        InspectionError, ManagedEntryInspector, Plan, PlanExecutionError, PlanExecutor,
        apply::{ApplyPlanner, ApplyPlanningError},
    },
    domain::{
        BackupEntry, BackupId, BackupKind, BackupRoot, BackupRootError, ConfigRoot,
        ConfigRootError, EntryKind, ManagedEntry, ManagedEntryInspection, ManagedSource,
        ManagedTarget, ManifestRoot, ManifestRootError,
    },
    infrastructure::{
        filesystem::{
            self, BackupStorage, BackupStorageError, FileSystemAction, FileSystemError, PathKind,
            RepositoryLayout, RepositoryLayoutError,
        },
        persistence::{
            ManifestRepository, ManifestRepositoryError, RepositoryPaths,
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::DottyPaths,
        },
    },
};

pub struct DotfileManager {
    config: ConfigRoot,

    manifest: ManifestRoot,
    manifest_repository: ManifestRepository,

    backups: BackupRoot,
    backups_repository: BackupRepository,
    backup_storage: BackupStorage,
}

impl DotfileManager {
    #[allow(unused)]
    pub fn new(
        config: ConfigRoot,
        manifest: ManifestRoot,
        manifest_repository: ManifestRepository,
        backups: BackupRoot,
        backups_repository: BackupRepository,
        backup_storage: BackupStorage,
    ) -> Self {
        Self {
            config,
            manifest,
            manifest_repository,
            backups,
            backups_repository,
            backup_storage,
        }
    }

    pub fn load(paths: &DottyPaths) -> Result<Self, DotfileManagerLoadError> {
        let config_repository = ConfigRepository::new(paths.config_file());
        let backups_repository = BackupRepository::new(paths.backup_file());

        let config = config_repository.read()?;

        let repository_paths = RepositoryPaths::new(config.repo_dir().clone());

        let manifest_repository = ManifestRepository::new(&repository_paths.manifest_file());

        let manifest = manifest_repository.read()?;

        let backups = backups_repository.read()?;

        let backup_storage = BackupStorage::new(paths.backup_dir());

        Ok(Self {
            config,
            manifest,
            manifest_repository,
            backups,
            backups_repository,
            backup_storage,
        })
    }

    pub fn manage(
        &mut self,
        target: ManagedTarget,
        source: ManagedSource,
    ) -> Result<(), DotfileManagerError> {
        let physical_target = self.config.resolve_target(&target);

        let entry_kind = match filesystem::path_kind(&physical_target)? {
            Some(PathKind::File) => EntryKind::File,
            Some(PathKind::Directory) => EntryKind::Dir,
            Some(PathKind::Symlink | PathKind::Other) | None => {
                return Err(DotfileManagerError::UnsupportedTarget(
                    physical_target.to_path_buf(),
                ));
            }
        };

        let managed_entry = ManagedEntry::new(source, target, entry_kind);

        let (plan, manifest, backups) = self.build_manage_plan(managed_entry)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_manage_plan(
        &self,
        managed_entry: ManagedEntry,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let source = self.config.resolve_source(managed_entry.source());
        let target = self.config.resolve_target(managed_entry.target());

        let backup_id = BackupId::new();

        let backup_entry = BackupEntry::new(
            managed_entry.source().clone(),
            managed_entry.target().clone(),
            managed_entry.kind().clone(),
            BackupKind::Link,
        );

        backups.add(backup_id, backup_entry)?;

        manifest.add(managed_entry.clone())?;

        let mut plan = Plan::new();

        let repository_paths = RepositoryPaths::new(self.config.repo_dir().to_path_buf());

        let repository_layout = RepositoryLayout::new(&repository_paths);

        let repository_plan = repository_layout.managed_source_plan(managed_entry.source())?;

        plan.extend(repository_plan);

        for action in
            self.backup_storage
                .backup_actions(&target, backup_id, managed_entry.kind().clone())?
        {
            plan.push(action.into());
        }

        plan.push(
            FileSystemAction::Rename {
                from: target.clone(),
                to: source.clone(),
            }
            .into(),
        );

        plan.push(
            FileSystemAction::CreateSymlink {
                target: source,
                link: target.clone(),
            }
            .into(),
        );

        let manifest_write = self.manifest_repository.prepare_write(&manifest)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = manifest_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, manifest, backups))
    }

    pub fn forget(&mut self, target: ManagedTarget) -> Result<(), DotfileManagerError> {
        let physical_target = self.config.resolve_target(&target);

        match filesystem::path_kind(&physical_target)? {
            Some(PathKind::Symlink) => {}
            _ => return Err(DotfileManagerError::NotSymlink(physical_target)),
        }

        let (plan, manifest, backups) = self.build_forget_plan(&target)?;

        PlanExecutor::execute(plan)?;

        self.manifest = manifest;
        self.backups = backups;

        Ok(())
    }

    fn build_forget_plan(
        &self,
        target: &ManagedTarget,
    ) -> Result<(Plan, ManifestRoot, BackupRoot), DotfileManagerError> {
        let mut manifest = self.manifest.clone();
        let mut backups = self.backups.clone();

        let physical_target = self.config.resolve_target(target);

        let managed_entry = manifest
            .manages()
            .iter()
            .find(|entry| entry.target() == target)
            .cloned()
            .ok_or_else(|| DotfileManagerError::Unmanaged(physical_target.to_path_buf()))?;

        let backup_id = BackupId::new();

        let backup_entry = BackupEntry::new(
            managed_entry.source().clone(),
            managed_entry.target().clone(),
            managed_entry.kind().clone(),
            BackupKind::Unlink,
        );

        backups.add(backup_id, backup_entry)?;

        manifest.remove(&managed_entry);

        let source = self.config.resolve_source(managed_entry.source());
        let target = self.config.resolve_target(managed_entry.target());

        let mut plan = Plan::new();

        let repository_paths = RepositoryPaths::new(self.config.repo_dir().to_path_buf());

        let repository_layout = RepositoryLayout::new(&repository_paths);

        let repository_plan =
            repository_layout.managed_source_cleanup_plan(managed_entry.source())?;

        for action in
            self.backup_storage
                .backup_actions(&source, backup_id, managed_entry.kind().clone())?
        {
            plan.push(action.into());
        }

        plan.push(FileSystemAction::RemoveSymlink(target.clone()).into());
        plan.push(
            FileSystemAction::Rename {
                from: source,
                to: target.clone(),
            }
            .into(),
        );

        plan.extend(repository_plan);

        let manifest_write = self.manifest_repository.prepare_write(&manifest)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = manifest_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, manifest, backups))
    }

    pub fn inspect(&self) -> Result<Vec<ManagedEntryInspection>, InspectionError> {
        self.manifest
            .manages()
            .iter()
            .map(|entry| {
                ManagedEntryInspector::inspect(
                    entry,
                    self.config.repo_dir(),
                    self.config.home_dir(),
                )
            })
            .collect::<Result<Vec<_>, _>>()
    }

    pub fn inspect_target(
        &self,
        target: &Path,
    ) -> Result<Option<ManagedEntryInspection>, InspectionError> {
        self.manifest
            .manages()
            .iter()
            .find(|entry| entry.target().path() == target)
            .map(|entry| {
                ManagedEntryInspector::inspect(
                    entry,
                    self.config.repo_dir(),
                    self.config.home_dir(),
                )
            })
            .transpose()
    }

    pub fn apply(&self) -> Result<(), DotfileManagerError> {
        let plan = self.plan_apply()?;

        PlanExecutor::execute(plan)?;

        Ok(())
    }

    pub fn plan_apply(&self) -> Result<Plan, DotfileManagerError> {
        let inspections = self.inspect()?;

        let planner = ApplyPlanner::new(
            self.config.repo_dir().to_path_buf(),
            self.config.home_dir().to_path_buf(),
        );

        Ok(planner.plan(&inspections)?)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    ManifestWrite(#[from] ManifestRepositoryError),
    #[error(transparent)]
    BackupWrite(#[from] BackupRepositoryError),
    #[error(transparent)]
    BackupStorage(#[from] BackupStorageError),
    #[error(transparent)]
    PlanExecution(#[from] PlanExecutionError),
    #[error("This target is not a symlink: `{0}`")]
    NotSymlink(PathBuf),
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("target cannot be managed: `{0}`")]
    UnsupportedTarget(PathBuf),
    #[error(transparent)]
    ConfigRoot(#[from] ConfigRootError),
    #[error(transparent)]
    RepositoryLayout(#[from] RepositoryLayoutError),
    #[error(transparent)]
    ManifestRoot(#[from] ManifestRootError),
    #[error(transparent)]
    BackupRoot(#[from] BackupRootError),
    #[error(transparent)]
    Inspection(#[from] InspectionError),
    #[error(transparent)]
    ApplyPlanning(#[from] ApplyPlanningError),
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerLoadError {
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
    #[error(transparent)]
    Manifest(#[from] ManifestRepositoryError),
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, fs, path::PathBuf};

    use tempfile::TempDir;

    use super::*;
    use crate::{domain::ManagedEntryIssue, infrastructure::persistence::RepositoryPaths};

    struct TestContext {
        _temp: TempDir,
        home_dir: PathBuf,
        repo_dir: PathBuf,
        paths: DottyPaths,
        repository_paths: RepositoryPaths,
    }

    impl TestContext {
        fn new() -> Self {
            let temp = TempDir::new().unwrap();

            let config_dir = temp.path().join("config");
            let storage_dir = temp.path().join("storage");
            let home_dir = temp.path().join("home");
            let repo_dir = temp.path().join("repo");

            fs::create_dir_all(&config_dir).unwrap();
            fs::create_dir_all(&storage_dir).unwrap();
            fs::create_dir_all(&home_dir).unwrap();
            fs::create_dir_all(&repo_dir).unwrap();

            let paths = DottyPaths::new(config_dir, storage_dir, home_dir.clone());

            fs::create_dir_all(paths.backup_dir()).unwrap();

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
                home_dir,
                repo_dir,
                paths,
                repository_paths,
            }
        }

        fn manager(&self) -> DotfileManager {
            DotfileManager::load(&self.paths).unwrap()
        }

        fn target(&self, path: &str) -> PathBuf {
            self.home_dir.join(path)
        }

        fn managed_target(&self, path: &str) -> ManagedTarget {
            ManagedTarget::home_relative(PathBuf::from(path)).unwrap()
        }

        fn managed_source(&self, path: &str) -> ManagedSource {
            ManagedSource::new(PathBuf::from(path)).unwrap()
        }

        fn create_file(&self, path: &str, contents: &str) {
            let target = self.target(path);

            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, contents).unwrap();
        }

        fn read_manifest(&self) -> ManifestRoot {
            ManifestRepository::new(&self.repository_paths.manifest_file())
                .read()
                .unwrap()
        }

        fn read_backups(&self) -> BackupRoot {
            BackupRepository::new(self.paths.backup_file())
                .read()
                .unwrap()
        }

        fn operations_dir(&self) -> PathBuf {
            self.paths.backup_dir().join("operations")
        }

        fn snapshot(&self, id: BackupId) -> PathBuf {
            self.operations_dir().join(id.to_string()).join("snapshot")
        }

        fn snapshot_count(&self) -> usize {
            if !self.operations_dir().exists() {
                return 0;
            }

            fs::read_dir(self.operations_dir()).unwrap().count()
        }

        fn manage(&self, manager: &mut DotfileManager, target: &str, source: &str) {
            manager
                .manage(self.managed_target(target), self.managed_source(source))
                .unwrap();
        }

        fn forget(&self, manager: &mut DotfileManager, target: &str) {
            manager.forget(self.managed_target(target)).unwrap();
        }
    }

    #[test]
    fn load_uses_repository_from_config_to_find_manifest() {
        let context = TestContext::new();

        let result = DotfileManager::load(&context.paths);

        assert!(result.is_ok());
    }

    #[test]
    fn load_fails_when_manifest_is_missing() {
        let context = TestContext::new();

        fs::remove_file(context.repository_paths.manifest_file()).unwrap();

        let result = DotfileManager::load(&context.paths);

        assert!(matches!(result, Err(DotfileManagerLoadError::Manifest(_))));
    }

    #[test]
    fn manage_persists_entry_in_manifest() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source.clone())
            .unwrap();

        let manifest = context.read_manifest();

        assert_eq!(manifest.manages().len(), 1);

        let entry = manifest.manages().iter().next().unwrap();

        assert_eq!(entry.target(), &managed_target);
        assert_eq!(entry.source(), &managed_source);
        assert_eq!(entry.kind(), &EntryKind::File);
    }

    #[test]
    fn manage_does_not_modify_config() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let before = fs::read(context.paths.config_file()).unwrap();

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn manage_persists_backup_metadata() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        manager
            .manage(managed_target.clone(), managed_source.clone())
            .unwrap();

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 1);

        let (id, entry) = backups.backups().iter().next().unwrap();

        assert_eq!(entry.source(), &managed_source);
        assert_eq!(entry.target(), &managed_target);
        assert_eq!(entry.entry_kind(), &EntryKind::File);
        assert_eq!(entry.backup_kind(), &BackupKind::Link);

        let snapshot = context.snapshot(*id);

        assert!(snapshot.is_file());
        assert_eq!(fs::read_to_string(snapshot).unwrap(), "hello");
    }

    #[test]
    fn manage_moves_target_into_repository() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let stored = context.repo_dir.join("zshrc");
        let target = context.target(".zshrc");

        assert!(stored.is_file());

        assert_eq!(fs::read_to_string(&stored).unwrap(), "hello");

        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(fs::read_link(target).unwrap(), stored);
    }

    #[test]
    fn forget_removes_entry_from_manifest() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        assert_eq!(context.read_manifest().manages().len(), 1);

        context.forget(&mut manager, ".zshrc");

        assert!(context.read_manifest().manages().is_empty());
    }

    #[test]
    fn forget_does_not_modify_config() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let before = fs::read(context.paths.config_file()).unwrap();

        context.forget(&mut manager, ".zshrc");

        let after = fs::read(context.paths.config_file()).unwrap();

        assert_eq!(before, after);
    }

    #[test]
    fn forget_restores_original_target() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.forget(&mut manager, ".zshrc");

        let target = context.target(".zshrc");

        assert!(target.is_file());

        assert_eq!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::File)
        );

        assert_eq!(fs::read_to_string(target).unwrap(), "hello");

        assert!(!context.repo_dir.join("zshrc").exists());
    }

    #[test]
    fn forget_persists_unlink_backup_metadata() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let managed_target = context.managed_target(".zshrc");
        let managed_source = context.managed_source("zshrc");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.forget(&mut manager, ".zshrc");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 2);

        let mut kinds = HashSet::new();

        for (id, entry) in backups.backups() {
            assert_eq!(entry.source(), &managed_source);
            assert_eq!(entry.target(), &managed_target);
            assert_eq!(entry.entry_kind(), &EntryKind::File);

            kinds.insert(entry.backup_kind().clone());

            assert_eq!(fs::read_to_string(context.snapshot(*id)).unwrap(), "hello");
        }

        assert_eq!(
            kinds,
            HashSet::from([BackupKind::Link, BackupKind::Unlink,])
        );
    }

    #[test]
    fn manage_supports_nested_source_path() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        let stored = context.repo_dir.join("ghostty/config");
        let target = context.target(".config/ghostty/config");

        assert!(stored.is_file());

        assert_eq!(fs::read_to_string(&stored).unwrap(), "hello");

        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(fs::read_link(&target).unwrap(), stored);

        let manifest = context.read_manifest();

        let entry = manifest
            .manages()
            .iter()
            .find(|entry| entry.source() == &context.managed_source("ghostty/config"))
            .unwrap();

        assert_eq!(
            entry.target(),
            &context.managed_target(".config/ghostty/config")
        );
    }

    #[test]
    fn manage_rolls_back_nested_directories_when_later_action_fails() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "original content");

        let target = context.target(".config/ghostty/config");

        let entry = ManagedEntry::new(
            context.managed_source("ghostty/config"),
            context.managed_target(".config/ghostty/config"),
            EntryKind::File,
        );

        let manager = context.manager();

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let (mut plan, _, _) = manager.build_manage_plan(entry).unwrap();

        plan.push(FileSystemAction::RemoveFile(context.repo_dir.join("nonexistent-file")).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(target.is_file());

        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");

        assert!(!context.repo_dir.join("ghostty/config").exists());
        assert!(!context.repo_dir.join("ghostty").exists());

        assert_eq!(context.snapshot_count(), 0);

        assert!(!context.operations_dir().exists());

        assert!(context.repo_dir.is_dir());
        assert!(context.paths.backup_dir().is_dir());

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );
    }

    #[test]
    fn forget_removes_empty_nested_source_directories() {
        let context = TestContext::new();

        context.create_file(".config/nvim/init.lua", "vim.opt.number = true");

        let mut manager = context.manager();

        context.manage(
            &mut manager,
            ".config/nvim/init.lua",
            "editors/nvim/init.lua",
        );

        assert!(context.repo_dir.join("editors/nvim/init.lua").is_file());

        context.forget(&mut manager, ".config/nvim/init.lua");

        let target = context.target(".config/nvim/init.lua");

        assert!(target.is_file());

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "vim.opt.number = true"
        );

        assert!(!context.repo_dir.join("editors/nvim").exists());
        assert!(!context.repo_dir.join("editors").exists());

        assert!(context.repo_dir.is_dir());
        assert!(context.repository_paths.metadata_dir().is_dir());

        assert!(context.read_manifest().manages().is_empty());
        assert_eq!(context.read_backups().backups().len(), 2);
    }

    #[test]
    fn forget_preserves_directory_containing_other_managed_sources() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "font-size = 14");

        context.create_file(".config/ghostty/themes.toml", "theme = 'dark'");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        context.manage(
            &mut manager,
            ".config/ghostty/themes.toml",
            "ghostty/themes.toml",
        );

        context.forget(&mut manager, ".config/ghostty/config");

        assert!(context.repo_dir.join("ghostty").is_dir());

        assert!(context.repo_dir.join("ghostty/themes.toml").is_file());

        assert!(!context.repo_dir.join("ghostty/config").exists());

        assert_eq!(
            fs::read_to_string(context.target(".config/ghostty/config")).unwrap(),
            "font-size = 14"
        );

        assert!(matches!(
            filesystem::path_kind(&context.target(".config/ghostty/themes.toml")).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(context.read_manifest().manages().len(), 1);
        assert_eq!(context.read_backups().backups().len(), 3);
    }

    #[test]
    fn forget_removes_shared_directory_after_last_source() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "config content");

        context.create_file(".config/ghostty/themes.toml", "theme content");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        context.manage(
            &mut manager,
            ".config/ghostty/themes.toml",
            "ghostty/themes.toml",
        );

        context.forget(&mut manager, ".config/ghostty/config");

        assert!(context.repo_dir.join("ghostty").is_dir());

        context.forget(&mut manager, ".config/ghostty/themes.toml");

        assert!(!context.repo_dir.join("ghostty").exists());

        assert_eq!(
            fs::read_to_string(context.target(".config/ghostty/config")).unwrap(),
            "config content"
        );

        assert_eq!(
            fs::read_to_string(context.target(".config/ghostty/themes.toml")).unwrap(),
            "theme content"
        );

        assert!(context.repo_dir.is_dir());
        assert!(context.read_manifest().manages().is_empty());

        assert_eq!(context.read_backups().backups().len(), 4);
    }

    #[test]
    fn forget_rolls_back_nested_directory_cleanup_when_later_action_fails() {
        let context = TestContext::new();

        context.create_file(".config/nvim/init.lua", "original content");

        let managed_target = context.managed_target(".config/nvim/init.lua");

        let mut manager = context.manager();

        context.manage(
            &mut manager,
            ".config/nvim/init.lua",
            "editors/nvim/init.lua",
        );

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let snapshots_before = context.snapshot_count();

        let (mut plan, _, _) = manager.build_forget_plan(&managed_target).unwrap();

        plan.push(FileSystemAction::RemoveFile(context.repo_dir.join("nonexistent-file")).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        let source = context.repo_dir.join("editors/nvim/init.lua");

        assert!(source.is_file());

        assert_eq!(fs::read_to_string(&source).unwrap(), "original content");

        assert!(context.repo_dir.join("editors/nvim").is_dir());
        assert!(context.repo_dir.join("editors").is_dir());

        let target = context.target(".config/nvim/init.lua");

        assert!(matches!(
            filesystem::path_kind(&target).unwrap(),
            Some(PathKind::Symlink)
        ));

        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );

        assert_eq!(context.snapshot_count(), snapshots_before);

        assert_eq!(context.read_backups().backups().len(), 1);
    }

    #[test]
    fn manage_forget_manage_preserves_all_backup_snapshots() {
        let context = TestContext::new();

        context.create_file(".zshrc", "version 1");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        fs::write(context.repo_dir.join("zshrc"), "version 2").unwrap();

        context.forget(&mut manager, ".zshrc");

        context.manage(&mut manager, ".zshrc", "zshrc");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 3);
        assert_eq!(context.snapshot_count(), 3);

        let mut link_contents = Vec::new();
        let mut unlink_contents = Vec::new();

        for (id, entry) in backups.backups() {
            assert_eq!(entry.source(), &context.managed_source("zshrc"));

            assert_eq!(entry.target(), &context.managed_target(".zshrc"));

            let contents = fs::read_to_string(context.snapshot(*id)).unwrap();

            match entry.backup_kind() {
                BackupKind::Link => link_contents.push(contents),
                BackupKind::Unlink => unlink_contents.push(contents),
            }
        }

        link_contents.sort();

        assert_eq!(
            link_contents,
            vec!["version 1".to_string(), "version 2".to_string(),]
        );

        assert_eq!(unlink_contents, vec!["version 2".to_string()]);

        assert_eq!(context.read_manifest().manages().len(), 1);

        assert!(matches!(
            filesystem::path_kind(&context.target(".zshrc")).unwrap(),
            Some(PathKind::Symlink)
        ));
    }

    #[test]
    fn backup_history_survives_manager_reload() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        {
            let mut manager = context.manager();

            context.manage(&mut manager, ".zshrc", "zshrc");
            context.forget(&mut manager, ".zshrc");
        }

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 3);
        assert_eq!(context.snapshot_count(), 3);

        let ids: HashSet<_> = backups.backups().keys().copied().collect();

        assert_eq!(ids.len(), 3);

        for id in ids {
            assert!(context.snapshot(id).is_file());
        }
    }

    #[test]
    fn manage_directory_creates_directory_snapshot() {
        let context = TestContext::new();

        context.create_file(".config/nvim/init.lua", "vim.opt.number = true");

        context.create_file(".config/nvim/lua/settings.lua", "return {}");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/nvim", "nvim");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 1);

        let (id, entry) = backups.backups().iter().next().unwrap();

        assert_eq!(entry.entry_kind(), &EntryKind::Dir);
        assert_eq!(entry.backup_kind(), &BackupKind::Link);

        assert_eq!(entry.source(), &context.managed_source("nvim"));

        assert_eq!(entry.target(), &context.managed_target(".config/nvim"));

        let snapshot = context.snapshot(*id);

        assert!(snapshot.is_dir());

        assert_eq!(
            fs::read_to_string(snapshot.join("init.lua")).unwrap(),
            "vim.opt.number = true"
        );

        assert_eq!(
            fs::read_to_string(snapshot.join("lua/settings.lua")).unwrap(),
            "return {}"
        );

        assert!(context.repo_dir.join("nvim").is_dir());

        assert!(matches!(
            filesystem::path_kind(&context.target(".config/nvim")).unwrap(),
            Some(PathKind::Symlink)
        ));
    }

    #[test]
    fn forget_directory_preserves_backup_history() {
        let context = TestContext::new();

        context.create_file(".config/nvim/init.lua", "original");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/nvim", "nvim");

        fs::write(context.repo_dir.join("nvim/init.lua"), "modified").unwrap();

        context.forget(&mut manager, ".config/nvim");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 2);

        for (id, entry) in backups.backups() {
            assert_eq!(entry.entry_kind(), &EntryKind::Dir);

            let snapshot = context.snapshot(*id);

            assert!(snapshot.is_dir());

            let contents = fs::read_to_string(snapshot.join("init.lua")).unwrap();

            match entry.backup_kind() {
                BackupKind::Link => {
                    assert_eq!(contents, "original");
                }
                BackupKind::Unlink => {
                    assert_eq!(contents, "modified");
                }
            }
        }

        assert_eq!(
            fs::read_to_string(context.target(".config/nvim/init.lua")).unwrap(),
            "modified"
        );

        assert!(context.read_manifest().manages().is_empty());
    }

    #[test]
    fn separate_managed_entries_create_independent_backup_histories() {
        let context = TestContext::new();

        context.create_file(".zshrc", "zsh");
        context.create_file(".bashrc", "bash");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.manage(&mut manager, ".bashrc", "bashrc");

        let backups = context.read_backups();

        assert_eq!(backups.backups().len(), 2);

        let sources: HashSet<_> = backups
            .backups()
            .values()
            .map(|entry| entry.source().clone())
            .collect();

        assert!(sources.contains(&context.managed_source("zshrc")));
        assert!(sources.contains(&context.managed_source("bashrc")));

        for id in backups.backups().keys() {
            assert!(context.snapshot(*id).is_file());
        }

        assert_eq!(context.read_manifest().manages().len(), 2);
    }

    #[test]
    fn inspect_returns_empty_results_for_empty_manifest() {
        let context = TestContext::new();
        let manager = context.manager();

        let inspections = manager.inspect().unwrap();

        assert!(inspections.is_empty());
    }

    #[test]
    fn inspect_returns_healthy_result_for_managed_file() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);

        let inspection = &inspections[0];

        assert!(inspection.is_healthy());
        assert!(inspection.issues().is_empty());

        assert_eq!(
            inspection.entry().source(),
            &context.managed_source("zshrc")
        );

        assert_eq!(
            inspection.entry().target(),
            &context.managed_target(".zshrc")
        );
    }

    #[test]
    fn inspect_returns_healthy_result_for_managed_directory() {
        let context = TestContext::new();

        context.create_file(".config/nvim/init.lua", "vim.opt.number = true");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/nvim", "nvim");

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);
        assert!(inspections[0].is_healthy());

        assert_eq!(inspections[0].entry().kind(), &EntryKind::Dir);
    }

    #[test]
    fn inspect_reports_missing_repository_source() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        fs::remove_file(context.repo_dir.join("zshrc")).unwrap();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);

        assert!(!inspections[0].is_healthy());

        assert_eq!(inspections[0].issues(), &[ManagedEntryIssue::MissingSource]);
    }

    #[test]
    fn inspect_reports_missing_target() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        fs::remove_file(context.target(".zshrc")).unwrap();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);

        assert_eq!(inspections[0].issues(), &[ManagedEntryIssue::MissingTarget]);
    }

    #[test]
    fn inspect_reports_modified_target_symlink() {
        use std::os::unix::fs::symlink;

        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");

        fs::remove_file(&target).unwrap();

        symlink(context.repo_dir.join("other"), &target).unwrap();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);

        assert_eq!(
            inspections[0].issues(),
            &[ManagedEntryIssue::WrongSymlinkDestination]
        );
    }

    #[test]
    fn inspect_reports_target_replaced_with_regular_file() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");

        fs::remove_file(&target).unwrap();
        fs::write(&target, "replacement").unwrap();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);

        assert_eq!(
            inspections[0].issues(),
            &[ManagedEntryIssue::TargetNotSymlink]
        );
    }

    #[test]
    fn inspect_returns_results_for_multiple_entries() {
        let context = TestContext::new();

        context.create_file(".zshrc", "zsh");
        context.create_file(".bashrc", "bash");
        context.create_file(".config/ghostty/config", "ghostty");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.manage(&mut manager, ".bashrc", "bashrc");
        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        fs::remove_file(context.repo_dir.join("bashrc")).unwrap();

        fs::remove_file(context.target(".config/ghostty/config")).unwrap();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 3);

        let zsh = inspections
            .iter()
            .find(|inspection| inspection.entry().source() == &context.managed_source("zshrc"))
            .unwrap();

        let bash = inspections
            .iter()
            .find(|inspection| inspection.entry().source() == &context.managed_source("bashrc"))
            .unwrap();

        let ghostty = inspections
            .iter()
            .find(|inspection| {
                inspection.entry().source() == &context.managed_source("ghostty/config")
            })
            .unwrap();

        assert!(zsh.is_healthy());

        assert_eq!(bash.issues(), &[ManagedEntryIssue::MissingSource]);

        assert_eq!(ghostty.issues(), &[ManagedEntryIssue::MissingTarget]);

        assert_eq!(
            inspections
                .iter()
                .filter(|inspection| inspection.is_healthy())
                .count(),
            1
        );
    }

    #[test]
    fn inspect_does_not_modify_persistent_state() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let config_before = fs::read(context.paths.config_file()).unwrap();

        let snapshots_before = context.snapshot_count();

        let inspections = manager.inspect().unwrap();

        assert_eq!(inspections.len(), 1);
        assert!(inspections[0].is_healthy());

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );

        assert_eq!(
            fs::read(context.paths.config_file()).unwrap(),
            config_before
        );

        assert_eq!(context.snapshot_count(), snapshots_before);
    }

    #[test]
    fn inspect_does_not_modify_manager_state() {
        let context = TestContext::new();

        context.create_file(".zshrc", "hello");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");

        let first = manager.inspect().unwrap();
        let second = manager.inspect().unwrap();

        assert_eq!(first.len(), second.len());
        assert_eq!(first.len(), 1);

        assert_eq!(first[0].issues(), second[0].issues());

        assert_eq!(first[0].entry(), second[0].entry());
    }

    #[test]
    fn manage_rejects_overlapping_targets_without_modifying_state() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "original");
        context.create_file(".config/other", "other content");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let snapshots_before = context.snapshot_count();

        let result = manager.manage(
            context.managed_target(".config/ghostty"),
            context.managed_source("other/config"),
        );

        assert!(matches!(
            result,
            Err(DotfileManagerError::ManifestRoot(
                ManifestRootError::TargetConflict { .. }
            ))
        ));

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );

        assert_eq!(context.snapshot_count(), snapshots_before);

        assert_eq!(
            fs::read_to_string(context.target(".config/other")).unwrap(),
            "other content"
        );

        assert_eq!(
            fs::read_to_string(context.repo_dir.join("ghostty/config")).unwrap(),
            "original"
        );

        assert!(!context.repo_dir.join("other/config").exists());
        assert_eq!(context.read_manifest().manages().len(), 1);
    }

    #[test]
    fn apply_recreates_missing_symlink() {
        let context = TestContext::new();

        context.create_file(".zshrc", "original content");

        let mut manager = context.manager();
        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");
        let source = context.repo_dir.join("zshrc");

        fs::remove_file(&target).unwrap();

        manager.apply().unwrap();

        assert!(
            fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(fs::read_link(&target).unwrap(), source);
        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");
    }

    #[test]
    fn apply_recreates_missing_parent_directories() {
        let context = TestContext::new();

        context.create_file(".config/ghostty/config", "ghostty config");

        let mut manager = context.manager();

        context.manage(&mut manager, ".config/ghostty/config", "ghostty/config");

        let target = context.target(".config/ghostty/config");
        let parent = context.target(".config/ghostty");

        fs::remove_file(&target).unwrap();
        fs::remove_dir(&parent).unwrap();

        manager.apply().unwrap();

        assert!(parent.is_dir());

        assert!(
            fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(
            fs::read_link(&target).unwrap(),
            context.repo_dir.join("ghostty/config")
        );
    }

    #[test]
    fn apply_is_idempotent_for_healthy_entries() {
        let context = TestContext::new();

        context.create_file(".zshrc", "original content");

        let mut manager = context.manager();
        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");
        let source = context.repo_dir.join("zshrc");

        let manifest_before = fs::read(context.repository_paths.manifest_file()).unwrap();

        let backups_before = fs::read(context.paths.backup_file()).unwrap();

        let snapshots_before = context.snapshot_count();

        let plan = manager.plan_apply().unwrap();

        assert!(plan.into_actions().is_empty());

        manager.apply().unwrap();
        manager.apply().unwrap();

        assert_eq!(fs::read_link(&target).unwrap(), source);

        assert_eq!(fs::read_to_string(&target).unwrap(), "original content");

        assert_eq!(
            fs::read(context.repository_paths.manifest_file()).unwrap(),
            manifest_before
        );

        assert_eq!(
            fs::read(context.paths.backup_file()).unwrap(),
            backups_before
        );

        assert_eq!(context.snapshot_count(), snapshots_before);
    }

    #[test]
    fn apply_rejects_conflicting_regular_file() {
        let context = TestContext::new();

        context.create_file(".zshrc", "original content");

        let mut manager = context.manager();
        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");

        fs::remove_file(&target).unwrap();
        fs::write(&target, "user modified content").unwrap();

        let result = manager.apply();

        assert!(matches!(
            result,
            Err(DotfileManagerError::ApplyPlanning(
                ApplyPlanningError::Entry { .. }
            ))
        ));

        assert!(target.is_file());

        assert!(
            !fs::symlink_metadata(&target)
                .unwrap()
                .file_type()
                .is_symlink()
        );

        assert_eq!(
            fs::read_to_string(&target).unwrap(),
            "user modified content"
        );

        assert_eq!(
            fs::read_to_string(context.repo_dir.join("zshrc")).unwrap(),
            "original content"
        );
    }

    #[test]
    fn apply_rejects_missing_source() {
        let context = TestContext::new();

        context.create_file(".zshrc", "original content");

        let mut manager = context.manager();
        context.manage(&mut manager, ".zshrc", "zshrc");

        let target = context.target(".zshrc");
        let source = context.repo_dir.join("zshrc");

        fs::remove_file(&target).unwrap();
        fs::remove_file(&source).unwrap();

        let result = manager.apply();

        assert!(matches!(
            result,
            Err(DotfileManagerError::ApplyPlanning(
                ApplyPlanningError::Entry { .. }
            ))
        ));

        assert!(!target.exists());
        assert!(!source.exists());

        assert_eq!(context.read_manifest().manages().len(), 1);
    }

    #[test]
    fn apply_restores_multiple_missing_symlinks() {
        let context = TestContext::new();

        context.create_file(".zshrc", "zsh content");
        context.create_file(".gitconfig", "git content");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.manage(&mut manager, ".gitconfig", "gitconfig");

        let zsh_target = context.target(".zshrc");
        let git_target = context.target(".gitconfig");

        fs::remove_file(&zsh_target).unwrap();
        fs::remove_file(&git_target).unwrap();

        manager.apply().unwrap();

        assert_eq!(
            fs::read_link(&zsh_target).unwrap(),
            context.repo_dir.join("zshrc")
        );

        assert_eq!(
            fs::read_link(&git_target).unwrap(),
            context.repo_dir.join("gitconfig")
        );

        assert_eq!(fs::read_to_string(&zsh_target).unwrap(), "zsh content");

        assert_eq!(fs::read_to_string(&git_target).unwrap(), "git content");

        assert_eq!(context.read_manifest().manages().len(), 2);
    }

    #[test]
    fn apply_planning_failure_does_not_modify_other_entries() {
        let context = TestContext::new();

        context.create_file(".zshrc", "zsh content");
        context.create_file(".gitconfig", "git content");

        let mut manager = context.manager();

        context.manage(&mut manager, ".zshrc", "zshrc");
        context.manage(&mut manager, ".gitconfig", "gitconfig");

        let zsh_target = context.target(".zshrc");
        let git_target = context.target(".gitconfig");

        fs::remove_file(&zsh_target).unwrap();
        fs::remove_file(&git_target).unwrap();

        fs::write(&git_target, "conflicting content").unwrap();

        let result = manager.apply();

        assert!(matches!(
            result,
            Err(DotfileManagerError::ApplyPlanning(
                ApplyPlanningError::Entry { .. }
            ))
        ));

        assert!(!zsh_target.exists());

        assert_eq!(
            fs::read_to_string(&git_target).unwrap(),
            "conflicting content"
        );

        assert_eq!(
            fs::read_to_string(context.repo_dir.join("zshrc")).unwrap(),
            "zsh content"
        );

        assert_eq!(
            fs::read_to_string(context.repo_dir.join("gitconfig")).unwrap(),
            "git content"
        );
    }
}
