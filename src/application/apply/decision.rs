use crate::domain::{ManagedEntryInspection, ManagedEntryIssue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyDecision {
    Skip,
    CreateSymlink,
}

pub fn decide_apply(
    inspection: &ManagedEntryInspection,
) -> Result<ApplyDecision, ApplyDecisionError> {
    if inspection.is_healthy() {
        return Ok(ApplyDecision::Skip);
    }

    let issues = inspection.issues();

    if issues.contains(&ManagedEntryIssue::MissingSource) {
        return Err(ApplyDecisionError::MissingSource);
    }

    if let [ManagedEntryIssue::MissingTarget] = issues {
        return Ok(ApplyDecision::CreateSymlink);
    }

    Err(ApplyDecisionError::Conflict(issues.to_vec()))
}

#[derive(Debug, thiserror::Error)]
pub enum ApplyDecisionError {
    #[error("Managed source is missing")]
    MissingSource,
    #[error("Cannot apply entry because of conflicting filesystem state: {0:?}")]
    Conflict(Vec<ManagedEntryIssue>),
}

#[cfg(test)]
#[path = "tests/decision.rs"]
mod tests;
