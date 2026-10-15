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
#[path = "tests/manager.rs"]
mod tests;
