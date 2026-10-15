use std::path::PathBuf;

use super::*;
use crate::domain::{EntryKind, ManagedEntry, ManagedSource, ManagedTarget};

fn inspection(issues: Vec<ManagedEntryIssue>) -> ManagedEntryInspection {
    let entry = ManagedEntry::new(
        ManagedSource::new(PathBuf::from("ghostty/config")).unwrap(),
        ManagedTarget::home_relative(PathBuf::from(".config/ghostty/config")).unwrap(),
        EntryKind::File,
    );

    ManagedEntryInspection::new(entry, issues)
}

#[test]
fn healthy_entry_is_skipped() {
    let result = decide_apply(&inspection(vec![]));

    assert!(matches!(result, Ok(ApplyDecision::Skip)));
}

#[test]
fn missing_target_creates_symlink_decision() {
    let result = decide_apply(&inspection(vec![ManagedEntryIssue::MissingTarget]));

    assert!(matches!(result, Ok(ApplyDecision::CreateSymlink)));
}

#[test]
fn missing_source_is_reported_separately() {
    let result = decide_apply(&inspection(vec![ManagedEntryIssue::MissingSource]));

    assert!(matches!(result, Err(ApplyDecisionError::MissingSource)));
}

#[test]
fn missing_source_takes_precedence_over_missing_target() {
    let result = decide_apply(&inspection(vec![
        ManagedEntryIssue::MissingTarget,
        ManagedEntryIssue::MissingSource,
    ]));

    assert!(matches!(result, Err(ApplyDecisionError::MissingSource)));
}

#[test]
fn missing_source_takes_precedence_over_conflicts() {
    let result = decide_apply(&inspection(vec![
        ManagedEntryIssue::MissingSource,
        ManagedEntryIssue::TargetNotSymlink,
    ]));

    assert!(matches!(result, Err(ApplyDecisionError::MissingSource)));
}

#[test]
fn wrong_source_kind_is_rejected() {
    let result = decide_apply(&inspection(vec![ManagedEntryIssue::WrongSourceKind]));

    assert!(matches!(
        result,
        Err(ApplyDecisionError::Conflict(issues))
            if issues == vec![ManagedEntryIssue::WrongSourceKind]
    ));
}

#[test]
fn existing_regular_file_is_rejected() {
    let result = decide_apply(&inspection(vec![ManagedEntryIssue::TargetNotSymlink]));

    assert!(matches!(
        result,
        Err(ApplyDecisionError::Conflict(issues))
            if issues == vec![ManagedEntryIssue::TargetNotSymlink]
    ));
}

#[test]
fn wrong_symlink_destination_is_rejected() {
    let result = decide_apply(&inspection(vec![
        ManagedEntryIssue::WrongSymlinkDestination,
    ]));

    assert!(matches!(
        result,
        Err(ApplyDecisionError::Conflict(issues))
            if issues == vec![ManagedEntryIssue::WrongSymlinkDestination]
    ));
}

#[test]
fn multiple_conflicting_issues_are_rejected() {
    let expected = vec![
        ManagedEntryIssue::WrongSourceKind,
        ManagedEntryIssue::MissingTarget,
    ];

    let result = decide_apply(&inspection(expected.clone()));

    assert!(matches!(
        result,
        Err(ApplyDecisionError::Conflict(issues))
            if issues == expected
    ));
}

#[test]
fn missing_target_with_other_issues_does_not_create_symlink() {
    let result = decide_apply(&inspection(vec![
        ManagedEntryIssue::MissingTarget,
        ManagedEntryIssue::TargetNotSymlink,
    ]));

    assert!(matches!(result, Err(ApplyDecisionError::Conflict(_))));
}
