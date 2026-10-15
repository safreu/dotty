use super::*;
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use crate::{
    application::{PlanExecutionError, PlanExecutor},
    domain::{EntryKind, ManagedEntry, ManagedEntryIssue, ManagedSource, ManagedTarget},
    infrastructure::filesystem::Action,
};

fn entry(source: &str, target: &str) -> ManagedEntry {
    ManagedEntry::new(
        ManagedSource::new(PathBuf::from(source)).unwrap(),
        ManagedTarget::home_relative(PathBuf::from(target)).unwrap(),
        EntryKind::File,
    )
}

fn inspection(
    source: &str,
    target: &str,
    issues: Vec<ManagedEntryIssue>,
) -> ManagedEntryInspection {
    ManagedEntryInspection::new(entry(source, target), issues)
}

fn missing_target(source: &str, target: &str) -> ManagedEntryInspection {
    inspection(source, target, vec![ManagedEntryIssue::MissingTarget])
}

fn planner(temp: &TempDir) -> ApplyPlanner {
    let home = temp.path().join("home");
    let repo = temp.path().join("repo");

    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&repo).unwrap();

    ApplyPlanner::new(repo, home)
}

fn create_source(temp: &TempDir, source: &str) {
    let path = temp.path().join("repo").join(source);

    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, "managed content").unwrap();
}

fn create_directory_action(path: impl AsRef<Path>) -> Action {
    FileSystemAction::CreateDirectory(path.as_ref().to_path_buf()).into()
}

fn create_symlink_action(source: impl AsRef<Path>, target: impl AsRef<Path>) -> Action {
    FileSystemAction::CreateSymlink {
        target: source.as_ref().to_path_buf(),
        link: target.as_ref().to_path_buf(),
    }
    .into()
}

#[test]
fn empty_inspections_produce_empty_plan() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let plan = planner.plan(&[]).unwrap();

    assert!(plan.actions().is_empty());
}

#[test]
fn healthy_entries_produce_empty_plan() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        inspection("zsh/.zshrc", ".zshrc", vec![]),
        inspection("git/config", ".gitconfig", vec![]),
    ];

    let plan = planner.plan(&inspections).unwrap();

    assert!(plan.actions().is_empty());
}

#[test]
fn missing_target_produces_create_symlink_action() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![missing_target("zsh/.zshrc", ".zshrc")];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[create_symlink_action(
            temp.path().join("repo/zsh/.zshrc"),
            temp.path().join("home/.zshrc"),
        )]
    );
}

#[test]
fn multiple_missing_targets_produce_multiple_actions() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("zsh/.zshrc", ".zshrc"),
        missing_target("git/config", ".gitconfig"),
    ];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[
            create_symlink_action(
                temp.path().join("repo/zsh/.zshrc"),
                temp.path().join("home/.zshrc"),
            ),
            create_symlink_action(
                temp.path().join("repo/git/config"),
                temp.path().join("home/.gitconfig"),
            ),
        ]
    );
}

#[test]
fn missing_parent_directory_is_planned() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    fs::create_dir(temp.path().join("home/.config")).unwrap();

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[
            create_directory_action(temp.path().join("home/.config/ghostty"),),
            create_symlink_action(
                temp.path().join("repo/ghostty/config"),
                temp.path().join("home/.config/ghostty/config"),
            ),
        ]
    );
}

#[test]
fn nested_missing_parent_directories_are_planned_in_order() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[
            create_directory_action(temp.path().join("home/.config"),),
            create_directory_action(temp.path().join("home/.config/ghostty"),),
            create_symlink_action(
                temp.path().join("repo/ghostty/config"),
                temp.path().join("home/.config/ghostty/config"),
            ),
        ]
    );
}

#[test]
fn shared_parent_directories_are_planned_only_once() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("ghostty/config", ".config/ghostty/config"),
        missing_target("nvim/init.lua", ".config/nvim/init.lua"),
    ];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[
            create_directory_action(temp.path().join("home/.config"),),
            create_directory_action(temp.path().join("home/.config/ghostty"),),
            create_symlink_action(
                temp.path().join("repo/ghostty/config"),
                temp.path().join("home/.config/ghostty/config"),
            ),
            create_directory_action(temp.path().join("home/.config/nvim"),),
            create_symlink_action(
                temp.path().join("repo/nvim/init.lua"),
                temp.path().join("home/.config/nvim/init.lua"),
            ),
        ]
    );

    let shared_directory = create_directory_action(temp.path().join("home/.config"));

    let count = plan
        .actions()
        .iter()
        .filter(|action| **action == shared_directory)
        .count();

    assert_eq!(count, 1);
}

#[test]
fn existing_parent_directories_are_skipped() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    fs::create_dir_all(temp.path().join("home/.config/ghostty")).unwrap();

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(
        plan.actions(),
        &[create_symlink_action(
            temp.path().join("repo/ghostty/config"),
            temp.path().join("home/.config/ghostty/config"),
        )]
    );
}

#[test]
fn regular_file_as_parent_is_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    fs::write(temp.path().join("home/.config"), "not a directory").unwrap();

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::InspectParent { path, .. })
            if path == temp.path().join("home/.config/ghostty")
    ));

    assert_eq!(
        fs::read_to_string(temp.path().join("home/.config")).unwrap(),
        "not a directory"
    );
}

#[test]
fn symlink_as_parent_is_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let external = temp.path().join("external");
    fs::create_dir(&external).unwrap();

    symlink(&external, temp.path().join("home/.config")).unwrap();

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::InspectParent { path, .. })
            if path == temp.path().join("home/.config/ghostty")
    ));

    assert_eq!(
        fs::read_link(temp.path().join("home/.config")).unwrap(),
        external
    );
}

#[test]
fn missing_home_directory_is_rejected() {
    let temp = TempDir::new().unwrap();

    let planner = ApplyPlanner::new(temp.path().join("repo"), temp.path().join("missing-home"));

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::InspectParent { .. })
    ));

    assert!(!temp.path().join("missing-home").exists());
}

#[test]
fn conflicting_entry_aborts_planning() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("zsh/.zshrc", ".zshrc"),
        inspection(
            "git/config",
            ".gitconfig",
            vec![ManagedEntryIssue::TargetNotSymlink],
        ),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::Entry {
            target,
            source: ApplyDecisionError::Conflict(_),
        }) if target == temp.path().join("home/.gitconfig")
    ));
}

#[test]
fn missing_source_aborts_planning() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![inspection(
        "ghostty/config",
        ".config/ghostty/config",
        vec![
            ManagedEntryIssue::MissingSource,
            ManagedEntryIssue::MissingTarget,
        ],
    )];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::Entry {
            target,
            source: ApplyDecisionError::MissingSource,
        }) if target == temp.path().join("home/.config/ghostty/config")
    ));
}

#[test]
fn planner_does_not_modify_filesystem() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    create_source(&temp, "ghostty/config");

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(plan.actions().len(), 3);

    assert!(!temp.path().join("home/.config").exists());
    assert!(!temp.path().join("home/.config/ghostty").exists());
    assert!(!temp.path().join("home/.config/ghostty/config").exists());
}

#[test]
fn executing_plan_creates_directories_and_symlinks() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    create_source(&temp, "ghostty/config");

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let plan = planner.plan(&inspections).unwrap();

    PlanExecutor::execute(plan).unwrap();

    let home = temp.path().join("home");
    let source = temp.path().join("repo/ghostty/config");
    let target = home.join(".config/ghostty/config");

    assert!(home.join(".config").is_dir());
    assert!(home.join(".config/ghostty").is_dir());

    assert!(
        fs::symlink_metadata(&target)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(fs::read_link(&target).unwrap(), source);
}

#[test]
fn rollback_removes_created_directories_and_symlinks() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    create_source(&temp, "ghostty/config");

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let mut plan = planner.plan(&inspections).unwrap();

    // Force a failure after the planned directory and symlink creation.
    plan.push(FileSystemAction::RemoveDirectory(temp.path().join("does-not-exist")).into());

    let result = PlanExecutor::execute(plan);

    assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

    let home = temp.path().join("home");

    assert!(!home.join(".config/ghostty/config").exists());
    assert!(!home.join(".config/ghostty").exists());
    assert!(!home.join(".config").exists());

    // The source file must remain untouched.
    assert_eq!(
        fs::read_to_string(temp.path().join("repo/ghostty/config")).unwrap(),
        "managed content"
    );
}

#[test]
fn rollback_preserves_preexisting_parent_directories() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    create_source(&temp, "ghostty/config");

    let home = temp.path().join("home");
    fs::create_dir(home.join(".config")).unwrap();

    let inspections = vec![missing_target("ghostty/config", ".config/ghostty/config")];

    let mut plan = planner.plan(&inspections).unwrap();

    plan.push(FileSystemAction::RemoveDirectory(temp.path().join("does-not-exist")).into());

    let result = PlanExecutor::execute(plan);

    assert!(matches!(result, Err(PlanExecutionError::Execution { .. })));

    assert!(home.join(".config").is_dir());
    assert!(!home.join(".config/ghostty").exists());
}

#[test]
fn duplicate_targets_are_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("nvim/config", ".config/nvim"),
        missing_target("other/config", ".config/nvim"),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::DuplicateTarget { target })
            if target == PathBuf::from(".config/nvim")
    ));
}

#[test]
fn overlapping_targets_are_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("nvim", ".config/nvim"),
        missing_target("nvim/init.lua", ".config/nvim/init.lua"),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::OverlappingTargets { target, other })
            if target == PathBuf::from(".config/nvim")
                && other == PathBuf::from(".config/nvim/init.lua")
    ));
}

#[test]
fn overlapping_targets_are_rejected_in_reverse_order() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("nvim/init.lua", ".config/nvim/init.lua"),
        missing_target("nvim", ".config/nvim"),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::OverlappingTargets { target, other })
            if target == PathBuf::from(".config/nvim/init.lua")
                && other == PathBuf::from(".config/nvim")
    ));
}

#[test]
fn similarly_named_directories_do_not_overlap() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        missing_target("nvim/config", ".config/nvim"),
        missing_target("nvim-backup/config", ".config/nvim-backup"),
    ];

    let plan = planner.plan(&inspections).unwrap();

    assert_eq!(plan.actions().len(), 3);
}

#[test]
fn duplicate_healthy_targets_are_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        inspection("nvim/config", ".config/nvim", vec![]),
        inspection("other/config", ".config/nvim", vec![]),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::DuplicateTarget { .. })
    ));
}

#[test]
fn overlapping_healthy_and_missing_targets_are_rejected() {
    let temp = TempDir::new().unwrap();
    let planner = planner(&temp);

    let inspections = vec![
        inspection("nvim", ".config/nvim", vec![]),
        missing_target("nvim/init.lua", ".config/nvim/init.lua"),
    ];

    let result = planner.plan(&inspections);

    assert!(matches!(
        result,
        Err(ApplyPlanningError::OverlappingTargets { .. })
    ));
}
