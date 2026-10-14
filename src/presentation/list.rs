use std::io::Write;

use comfy_table::{Cell, Color, Table, presets};

use crate::{domain::ManagedEntryInspection, presentation::style::muted};

pub fn print_entries(
    writer: &mut impl Write,
    inspections: &[ManagedEntryInspection],
) -> std::io::Result<()> {
    if inspections.is_empty() {
        writeln!(writer, "{}", muted("No managed entries found."))?;
        return Ok(());
    }
    let mut table = Table::new();

    table.load_style(presets::NOTHING);
    table.set_header(vec!["SOURCE", "TARGET", "STATUS"]);

    for inspection in inspections {
        let entry = inspection.entry();

        let source = entry.source().path().display().to_string();

        let target = format!("~/{}", entry.target().path().display());

        let status = if inspection.is_healthy() {
            Cell::new("OK").fg(Color::Green)
        } else {
            Cell::new("UNHEALTHY").fg(Color::Red)
        };

        table.add_row(vec![Cell::new(source), Cell::new(target), status]);
    }

    writeln!(writer, "{table}")?;

    Ok(())
}

#[cfg(test)]
mod tests {
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

        print_entries(&mut output, inspections).unwrap();

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
    fn print_entries_reports_empty_manifest() {
        let output = strip_ansi(&render(&[]));

        assert_eq!(output, "No managed entries found.\n");
    }

    #[test]
    fn print_entries_reports_healthy_entry() {
        let inspections = vec![inspection("zshrc", ".zshrc", vec![])];

        let output = strip_ansi(&render(&inspections));

        assert!(output.contains("SOURCE"));
        assert!(output.contains("TARGET"));
        assert!(output.contains("STATUS"));
        assert!(output.contains("zshrc"));
        assert!(output.contains("~/.zshrc"));
        assert!(output.contains("OK"));
        assert!(!output.contains("UNHEALTHY"));
    }

    #[test]
    fn print_entries_reports_unhealthy_entry() {
        let inspections = vec![inspection(
            "nvim",
            ".config/nvim",
            vec![ManagedEntryIssue::MissingTarget],
        )];

        let output = strip_ansi(&render(&inspections));

        assert!(output.contains("nvim"));
        assert!(output.contains("~/.config/nvim"));
        assert!(output.contains("UNHEALTHY"));
    }

    #[test]
    fn print_entries_reports_mixed_health_states() {
        let inspections = vec![
            inspection("zshrc", ".zshrc", vec![]),
            inspection(
                "nvim",
                ".config/nvim",
                vec![ManagedEntryIssue::MissingSource],
            ),
            inspection("ghostty/config", ".config/ghostty/config", vec![]),
        ];

        let output = strip_ansi(&render(&inspections));

        let lines: Vec<&str> = output.lines().collect();

        let zshrc_line = lines.iter().find(|line| line.contains("~/.zshrc")).unwrap();

        let nvim_line = lines
            .iter()
            .find(|line| line.contains("~/.config/nvim"))
            .unwrap();

        let ghostty_line = lines
            .iter()
            .find(|line| line.contains("~/.config/ghostty/config"))
            .unwrap();

        assert!(zshrc_line.contains("OK"));
        assert!(nvim_line.contains("UNHEALTHY"));
        assert!(ghostty_line.contains("OK"));
    }

    #[test]
    fn print_entries_uses_portable_paths() {
        let inspections = vec![inspection(
            "apps/ghostty/config",
            ".config/ghostty/config",
            vec![],
        )];

        let output = strip_ansi(&render(&inspections));

        assert!(output.contains("apps/ghostty/config"));
        assert!(output.contains("~/.config/ghostty/config"));
    }

    #[test]
    fn print_entries_does_not_display_issue_descriptions() {
        let inspections = vec![inspection(
            "nvim",
            ".config/nvim",
            vec![
                ManagedEntryIssue::MissingSource,
                ManagedEntryIssue::WrongSymlinkDestination,
            ],
        )];

        let output = strip_ansi(&render(&inspections));

        assert!(output.contains("UNHEALTHY"));
        assert!(!output.contains("Missing source"));
        assert!(!output.contains("Wrong symlink destination"));
    }

    #[test]
    fn print_entries_does_not_print_summary() {
        let inspections = vec![
            inspection("zshrc", ".zshrc", vec![]),
            inspection(
                "nvim",
                ".config/nvim",
                vec![ManagedEntryIssue::MissingTarget],
            ),
        ];

        let output = strip_ansi(&render(&inspections));

        assert!(!output.contains("1 healthy"));
        assert!(!output.contains("1 unhealthy"));
    }

    #[test]
    fn print_entries_aligns_columns() {
        let inspections = vec![
            inspection("zshrc", ".zshrc", vec![]),
            inspection("ghostty/config", ".config/ghostty/config", vec![]),
            inspection(
                "nvim",
                ".config/nvim",
                vec![ManagedEntryIssue::MissingTarget],
            ),
        ];

        let output = strip_ansi(&render(&inspections));

        let lines: Vec<&str> = output.lines().collect();

        let header = lines
            .iter()
            .find(|line| line.contains("SOURCE") && line.contains("TARGET"))
            .unwrap();

        let target_column = header.find("TARGET").unwrap();
        let status_column = header.find("STATUS").unwrap();

        for line in lines.iter().filter(|line| {
            line.contains("~/.zshrc")
                || line.contains("~/.config/ghostty/config")
                || line.contains("~/.config/nvim")
        }) {
            assert_eq!(line.find("~/").unwrap(), target_column);

            let status_position = line.find("UNHEALTHY").or_else(|| line.find("OK")).unwrap();

            assert_eq!(status_position, status_column);
        }
    }

    #[test]
    fn print_entries_preserves_entry_order() {
        let inspections = vec![
            inspection("zshrc", ".zshrc", vec![]),
            inspection("bashrc", ".bashrc", vec![]),
            inspection("nvim", ".config/nvim", vec![]),
        ];

        let output = strip_ansi(&render(&inspections));

        let zshrc = output.find("~/.zshrc").unwrap();
        let bashrc = output.find("~/.bashrc").unwrap();
        let nvim = output.find("~/.config/nvim").unwrap();

        assert!(zshrc < bashrc);
        assert!(bashrc < nvim);
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
    fn print_entries_propagates_write_errors() {
        let inspections = vec![inspection("zshrc", ".zshrc", vec![])];

        let result = print_entries(&mut FailingWriter, &inspections);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
    }

    #[test]
    fn print_entries_propagates_write_errors_for_empty_manifest() {
        let result = print_entries(&mut FailingWriter, &[]);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::Other);
    }
}
