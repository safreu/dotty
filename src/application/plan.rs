use crate::infrastructure::filesystem::{Action, ActionError};

pub struct Plan {
    actions: Vec<Action>,
}

impl Plan {
    pub fn new() -> Self {
        Self {
            actions: Vec::new(),
        }
    }

    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn into_actions(self) -> Vec<Action> {
        self.actions
    }

    pub fn extend(&mut self, other: Plan) {
        self.actions.extend(other.actions);
    }

    #[cfg(test)]
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }
}

pub struct PlanExecutor;

impl PlanExecutor {
    pub fn execute(plan: Plan) -> Result<(), PlanExecutionError> {
        let mut rollback = Vec::new();

        for action in plan.into_actions() {
            match action.execute() {
                Ok(executed) => {
                    if let Some(action) = executed.rollback() {
                        rollback.push(action);
                    }
                }
                Err(source) => {
                    let rollback_errors = Self::rollback(&mut rollback);

                    if rollback_errors.is_empty() {
                        return Err(PlanExecutionError::Execution { source });
                    }

                    return Err(PlanExecutionError::Rollback {
                        execution: source,
                        rollback: rollback_errors,
                    });
                }
            }
        }

        Ok(())
    }

    fn rollback(actions: &mut Vec<Action>) -> Vec<ActionError> {
        let mut errors = Vec::new();

        while let Some(action) = actions.pop() {
            if let Err(e) = action.execute() {
                errors.push(e);
            }
        }

        errors
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlanExecutionError {
    #[error("failed to execute plan")]
    Execution {
        #[source]
        source: ActionError,
    },

    #[error("failed to execute plan and rollback also failed")]
    Rollback {
        execution: ActionError,
        rollback: Vec<ActionError>,
    },
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;
    use crate::infrastructure::filesystem::FileSystemAction;

    #[test]
    fn executor_executes_all_actions() {
        let temp = TempDir::new().unwrap();

        let first = temp.path().join("first");
        let second = temp.path().join("second");

        let mut plan = Plan::new();
        plan.push(FileSystemAction::CreateDirectory(first.clone()).into());
        plan.push(FileSystemAction::CreateDirectory(second.clone()).into());

        PlanExecutor::execute(plan).unwrap();

        assert!(first.is_dir());
        assert!(second.is_dir());
    }

    #[test]
    fn executor_rolls_back_previous_actions_when_action_fails() {
        let temp = TempDir::new().unwrap();

        let created = temp.path().join("created");
        let missing = temp.path().join("missing");

        let mut plan = Plan::new();
        plan.push(FileSystemAction::CreateDirectory(created.clone()).into());
        plan.push(FileSystemAction::RemoveDirectory(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(!created.exists());
    }

    #[test]
    fn executor_rolls_back_actions_in_reverse_order() {
        let temp = TempDir::new().unwrap();

        let parent = temp.path().join("parent");
        let child = parent.join("child");
        let missing = temp.path().join("missing");

        let mut plan = Plan::new();
        plan.push(FileSystemAction::CreateDirectory(parent.clone()).into());
        plan.push(FileSystemAction::CreateDirectory(child.clone()).into());
        plan.push(FileSystemAction::RemoveDirectory(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(!child.exists());
        assert!(!parent.exists());
    }

    #[test]
    fn rollback_restores_removed_directory() {
        let temp = TempDir::new().unwrap();

        let directory = temp.path().join("existing");
        let missing = temp.path().join("missing");

        std::fs::create_dir(&directory).unwrap();

        let mut plan = Plan::new();

        plan.push(FileSystemAction::RemoveDirectory(directory.clone()).into());

        plan.push(FileSystemAction::RemoveDirectory(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(directory.is_dir());

        assert!(temp.path().is_dir());
    }

    #[test]
    fn remove_directory_if_empty_removes_empty_directory() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("ghostty");

        std::fs::create_dir(&directory).unwrap();

        let mut plan = Plan::new();
        plan.push(FileSystemAction::RemoveDirectoryIfEmpty(directory.clone()).into());

        PlanExecutor::execute(plan).unwrap();

        assert!(!directory.exists());
        assert!(temp.path().is_dir());
    }

    #[test]
    fn remove_directory_if_empty_preserves_non_empty_directory() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("ghostty");
        let file = directory.join("themes.toml");

        std::fs::create_dir(&directory).unwrap();
        std::fs::write(&file, "theme = 'dark'").unwrap();

        let mut plan = Plan::new();
        plan.push(FileSystemAction::RemoveDirectoryIfEmpty(directory.clone()).into());

        PlanExecutor::execute(plan).unwrap();

        assert!(directory.is_dir());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "theme = 'dark'");
    }

    #[test]
    fn remove_directory_if_empty_restores_directory_on_rollback() {
        let temp = TempDir::new().unwrap();
        let directory = temp.path().join("ghostty");
        let missing = temp.path().join("missing");

        std::fs::create_dir(&directory).unwrap();

        let mut plan = Plan::new();

        plan.push(FileSystemAction::RemoveDirectoryIfEmpty(directory.clone()).into());

        plan.push(FileSystemAction::RemoveDirectory(missing).into());

        let result = PlanExecutor::execute(plan);

        assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

        assert!(directory.is_dir());
    }
}
