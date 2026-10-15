use std::{
    io::{self, Write},
    path::PathBuf,
};

use super::*;
use crate::domain::{EntryKind, ManagedEntry, ManagedEntryIssue, ManagedSource, ManagedTarget};

fn inspection(
    source: &str,
    target: &str,
    issues: Vec<ManagedEntryIssue>,
) -> ManagedEntryInspection {
    let entry = ManagedEntry::new(
        ManagedSource::new(PathBuf::from(source)).unwrap(),
        ManagedTarget::home_relative(PathBuf::from(target)).unwrap(),
        EntryKind::File,
    );

    ManagedEntryInspection::new(entry, issues)
}

fn render(inspections: &[ManagedEntryInspection]) -> String {
    let mut output = Vec::new();

    print_inspections(&mut output, inspections).unwrap();

    String::from_utf8(output).unwrap()
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
fn print_inspections_reports_empty_manifest() {
    let output = strip_ansi(&render(&[]));

    assert_eq!(output, "No managed entries found.\n");
}

#[test]
fn print_inspections_reports_healthy_entry() {
    let inspections = vec![inspection("zshrc", ".zshrc", vec![])];

    let output = strip_ansi(&render(&inspections));

    assert!(output.contains("[OK]"));
    assert!(output.contains(".zshrc"));
    assert!(output.contains("Healthy"));
    assert!(output.contains("1 healthy, 0 unhealthy"));
}

#[test]
fn print_inspections_reports_unhealthy_entry() {
    let inspections = vec![inspection(
        "zshrc",
        ".zshrc",
        vec![ManagedEntryIssue::MissingSource],
    )];

    let output = strip_ansi(&render(&inspections));

    assert!(output.contains("[ERR]"));
    assert!(output.contains(".zshrc"));
    assert!(output.contains("Unhealthy"));
    assert!(output.contains("Missing source"));
    assert!(output.contains("0 healthy, 1 unhealthy"));
}

#[test]
fn print_inspections_reports_all_issues() {
    let inspections = vec![inspection(
        "nvim",
        ".config/nvim",
        vec![
            ManagedEntryIssue::MissingSource,
            ManagedEntryIssue::WrongSymlinkDestination,
        ],
    )];

    let output = strip_ansi(&render(&inspections));

    assert!(output.contains("Missing source"));
    assert!(output.contains("Wrong symlink destination"));
}

#[test]
fn print_inspections_reports_correct_summary_counts() {
    let inspections = vec![
        inspection("zshrc", ".zshrc", vec![]),
        inspection("bashrc", ".bashrc", vec![]),
        inspection(
            "nvim",
            ".config/nvim",
            vec![ManagedEntryIssue::MissingTarget],
        ),
    ];

    let output = strip_ansi(&render(&inspections));

    assert!(output.contains("2 healthy, 1 unhealthy"));
}

#[test]
fn print_inspections_aligns_status_columns() {
    let inspections = vec![
        inspection("zshrc", ".zshrc", vec![]),
        inspection("ghostty", ".config/ghostty/config", vec![]),
        inspection(
            "nvim",
            ".config/nvim",
            vec![ManagedEntryIssue::MissingTarget],
        ),
    ];

    let output = strip_ansi(&render(&inspections));

    let lines: Vec<&str> = output.lines().collect();

    let status_positions: Vec<usize> = lines
        .iter()
        .filter(|line| line.starts_with("[OK]") || line.starts_with("[ERR]"))
        .map(|line| {
            line.find("Healthy")
                .or_else(|| line.find("Unhealthy"))
                .unwrap()
        })
        .collect();

    assert_eq!(status_positions.len(), 3);

    assert!(
        status_positions
            .iter()
            .all(|position| *position == status_positions[0])
    );
}

#[test]
fn print_inspections_indents_issues_under_entry() {
    let inspections = vec![inspection(
        "nvim",
        ".config/nvim",
        vec![ManagedEntryIssue::MissingSource],
    )];

    let output = strip_ansi(&render(&inspections));

    let issue_line = output
        .lines()
        .find(|line| line.contains("Missing source"))
        .unwrap();

    assert_eq!(issue_line, format!("{:6}Missing source", ""));
}

#[test]
fn print_inspections_does_not_modify_inspections() {
    let inspections = vec![inspection(
        "zshrc",
        ".zshrc",
        vec![ManagedEntryIssue::MissingSource],
    )];

    let issues_before = inspections[0].issues().to_vec();

    render(&inspections);

    assert_eq!(inspections[0].issues(), issues_before);
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
fn print_inspections_propagates_write_errors() {
    let inspections = vec![inspection("zshrc", ".zshrc", vec![])];

    let result = print_inspections(&mut FailingWriter, &inspections);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
}

#[test]
fn print_inspections_propagates_write_errors_for_empty_manifest() {
    let result = print_inspections(&mut FailingWriter, &[]);

    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
}
