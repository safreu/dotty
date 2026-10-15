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
#[path = "tests/plan.rs"]
mod tests;
