use std::{
    collections::hash_map,
    path::{Path, PathBuf},
};

use crate::{
    application::{Plan, PlanExecutionError, PlanExecutor},
    domain::{
        BackupEntry, BackupKind, BackupRoot, ConfigRoot, EntryKind, ManagedEntry, ManagedEntryError,
    },
    infrastructure::{
        filesystem::{
            self, BackupStorage, BackupStorageError, FileSystemAction, FileSystemError, PathKind,
        },
        persistence::{
            backup::{BackupRepository, BackupRepositoryError},
            config::{ConfigRepository, ConfigRepositoryError},
            paths::DottyPaths,
        },
    },
};

pub struct DotfileManager {
    config: ConfigRoot,
    config_repository: ConfigRepository,
    backups: BackupRoot,
    backups_repository: BackupRepository,
    backup_storage: BackupStorage,
}

impl DotfileManager {
    #[allow(unused)]
    pub fn new(
        config: ConfigRoot,
        config_repository: ConfigRepository,
        backups: BackupRoot,
        backups_repository: BackupRepository,
        backup_storage: BackupStorage,
    ) -> Self {
        Self {
            config,
            config_repository,
            backups,
            backups_repository,
            backup_storage,
        }
    }

    pub fn load(paths: &DottyPaths) -> Result<Self, DotfileManagerLoadError> {
        let config_repository = ConfigRepository::new(paths.config_file());
        let backups_repository = BackupRepository::new(paths.backup_file());

        let config = config_repository.read()?;

        let backups = backups_repository.read()?;

        let backup_storage = BackupStorage::new(paths.backup_dir());

        Ok(Self {
            config,
            config_repository,
            backups,
            backups_repository,
            backup_storage,
        })
    }

    pub fn manage(&mut self, target: PathBuf) -> Result<(), DotfileManagerError> {
        let entry_kind = match filesystem::path_kind(&target)? {
            Some(PathKind::File) => EntryKind::File,
            Some(PathKind::Directory) => EntryKind::Dir,
            Some(PathKind::Symlink | PathKind::Other) | None => {
                return Err(DotfileManagerError::UnsupportedTarget(target));
            }
        };

        let (plan, config, backups) = self.build_manage_plan(&target, entry_kind)?;

        PlanExecutor::execute(plan)?;

        self.config = config;
        self.backups = backups;

        Ok(())
    }

    fn build_manage_plan(
        &self,
        target: &Path,
        entry_kind: EntryKind,
    ) -> Result<(Plan, ConfigRoot, BackupRoot), DotfileManagerError> {
        let managed_entry =
            ManagedEntry::new(target, self.config.config().repo_dir(), &entry_kind)?;

        let mut config = self.config.clone();
        let mut backups = self.backups.clone();

        let new_backup_entry = BackupEntry::new(&entry_kind, BackupKind::Link);

        match backups.backups_mut().entry(target.to_path_buf()) {
            hash_map::Entry::Occupied(entry) => {
                entry.into_mut().backups_mut().insert(BackupKind::Link);
            }
            hash_map::Entry::Vacant(entry) => {
                entry.insert(new_backup_entry);
            }
        };

        config.manages_mut().insert(managed_entry.clone());

        let mut plan = Plan::new();

        for action in self
            .backup_storage
            .backup_actions(&managed_entry, BackupKind::Link)?
        {
            plan.push(action.into());
        }

        plan.push(
            FileSystemAction::Rename {
                from: target.to_path_buf(),
                to: managed_entry.stored_at().to_path_buf(),
            }
            .into(),
        );

        plan.push(
            FileSystemAction::CreateSymlink {
                target: managed_entry.stored_at().to_path_buf(),
                link: managed_entry.points_to().to_path_buf(),
            }
            .into(),
        );

        let config_write = self.config_repository.prepare_write(&config)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = config_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, config, backups))
    }

    pub fn forget(&mut self, target: PathBuf) -> Result<(), DotfileManagerError> {
        match filesystem::path_kind(&target)? {
            Some(PathKind::Symlink) => {}
            _ => return Err(DotfileManagerError::NotSymlink(target.to_path_buf())),
        }

        let (plan, config, backups) = self.build_forget_plan(&target)?;

        PlanExecutor::execute(plan)?;

        self.config = config;
        self.backups = backups;

        Ok(())
    }

    fn build_forget_plan(
        &self,
        target: &Path,
    ) -> Result<(Plan, ConfigRoot, BackupRoot), DotfileManagerError> {
        let mut config = self.config.clone();
        let mut backups = self.backups.clone();

        let managed_entry = config
            .manages()
            .iter()
            .find(|entry| entry.points_to() == target)
            .cloned()
            .ok_or_else(|| DotfileManagerError::Unmanaged(target.to_path_buf()))?;

        let backup_entry = backups
            .backups_mut()
            .get_mut(target)
            .ok_or_else(|| DotfileManagerError::Unmanaged(target.to_path_buf()))?;

        backup_entry.backups_mut().insert(BackupKind::Unlink);

        config.manages_mut().remove(&managed_entry);

        let mut plan = Plan::new();

        for action in self
            .backup_storage
            .backup_actions(&managed_entry, BackupKind::Unlink)?
        {
            plan.push(action.into());
        }

        plan.push(FileSystemAction::RemoveSymlink(target.to_path_buf()).into());
        plan.push(
            FileSystemAction::Rename {
                from: managed_entry.stored_at().to_path_buf(),
                to: target.to_path_buf(),
            }
            .into(),
        );

        let config_write = self.config_repository.prepare_write(&config)?;
        let backups_write = self.backups_repository.prepare_write(&backups)?;

        let (path, contents) = config_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        let (path, contents) = backups_write.into_parts();
        plan.push(FileSystemAction::WriteFile { path, contents }.into());

        Ok((plan, config, backups))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerError {
    #[error("This target is unmanaged already")]
    Unmanaged(PathBuf),
    #[error(transparent)]
    EntryCreation(#[from] ManagedEntryError),
    #[error(transparent)]
    ConfigWrite(#[from] ConfigRepositoryError),
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
}

#[derive(Debug, thiserror::Error)]
pub enum DotfileManagerLoadError {
    #[error(transparent)]
    Config(#[from] ConfigRepositoryError),
    #[error(transparent)]
    Backups(#[from] BackupRepositoryError),
}

#[cfg(test)]
mod tests {}
