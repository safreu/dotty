use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::{
    application::{
        Plan,
        apply::decision::{ApplyDecision, ApplyDecisionError, decide_apply},
    },
    domain::ManagedEntryInspection,
    infrastructure::filesystem::{self, FileSystemAction, FileSystemError},
};

pub struct ApplyPlanner {
    repo_dir: PathBuf,
    home_dir: PathBuf,
}

impl ApplyPlanner {
    pub fn new(repo_dir: PathBuf, home_dir: PathBuf) -> Self {
        Self { repo_dir, home_dir }
    }

    pub fn plan(&self, inspections: &[ManagedEntryInspection]) -> Result<Plan, ApplyPlanningError> {
        Self::validate_targets(inspections)?;

        let mut plan = Plan::new();
        let mut planned_directories = HashSet::new();

        for inspection in inspections {
            let entry = inspection.entry();

            let source = self.repo_dir.join(entry.source().path());
            let target = self.home_dir.join(entry.target().path());

            match decide_apply(inspection) {
                Ok(ApplyDecision::Skip) => {}
                Ok(ApplyDecision::CreateSymlink) => {
                    self.plan_parent_directories(&target, &mut plan, &mut planned_directories)?;

                    plan.push(
                        FileSystemAction::CreateSymlink {
                            target: source,
                            link: target,
                        }
                        .into(),
                    );
                }
                Err(error) => {
                    return Err(ApplyPlanningError::Entry {
                        target,
                        source: error,
                    });
                }
            }
        }

        Ok(plan)
    }

    fn plan_parent_directories(
        &self,
        target: &Path,
        plan: &mut Plan,
        planned_directories: &mut HashSet<PathBuf>,
    ) -> Result<(), ApplyPlanningError> {
        let parent = target
            .parent()
            .ok_or_else(|| ApplyPlanningError::InvalidParent {
                path: target.to_path_buf(),
            })?;

        let missing =
            filesystem::missing_directories(&self.home_dir, parent).map_err(|source| {
                ApplyPlanningError::InspectParent {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;

        for directory in missing {
            if planned_directories.insert(directory.clone()) {
                plan.push(FileSystemAction::CreateDirectory(directory).into());
            }
        }

        Ok(())
    }

    fn validate_targets(inspections: &[ManagedEntryInspection]) -> Result<(), ApplyPlanningError> {
        for (index, current) in inspections.iter().enumerate() {
            for other in inspections.iter().skip(index + 1) {
                let current_path = current.entry().target().path();
                let other_path = other.entry().target().path();

                if current_path == other_path {
                    return Err(ApplyPlanningError::DuplicateTarget {
                        target: current_path.to_path_buf(),
                    });
                }

                if current_path.starts_with(other_path) || other_path.starts_with(current_path) {
                    return Err(ApplyPlanningError::OverlappingTargets {
                        target: current_path.to_path_buf(),
                        other: other_path.to_path_buf(),
                    });
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyPlanningError {
    #[error("cannot apply managed entry {target}: {source}")]
    Entry {
        target: PathBuf,
        #[source]
        source: ApplyDecisionError,
    },
    #[error("cannot create symlink at `{path}`: unable to determine its parent directory")]
    InvalidParent { path: PathBuf },
    #[error("cannot prepare parent directories for `{path}`: {source}")]
    InspectParent {
        path: PathBuf,
        #[source]
        source: FileSystemError,
    },
    #[error("multiple managed entries use the same target: `{target}`")]
    DuplicateTarget { target: PathBuf },
    #[error("managed target `{target}` overlaps with `{other}`")]
    OverlappingTargets { target: PathBuf, other: PathBuf },
}

#[cfg(test)]
#[path = "tests/planner.rs"]
mod tests;
