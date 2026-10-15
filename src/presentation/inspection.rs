use std::io::Write;

use crate::{
    domain::ManagedEntryInspection,
    presentation::style::{error, muted, success},
};

const OK_PREFIX: &str = "[OK]";
const ERR_PREFIX: &str = "[ERR]";
const PREFIX_WIDTH: usize = 5;

fn print_inspection(
    writer: &mut impl Write,
    inspection: &ManagedEntryInspection,
    width: usize,
) -> std::io::Result<()> {
    let target = inspection.entry().target().path().display();

    if inspection.is_healthy() {
        let prefix = format!("{OK_PREFIX:<PREFIX_WIDTH$}");

        writeln!(
            writer,
            "{} {target:<width$} {}",
            success(&prefix),
            muted("Healthy"),
        )?;
    } else {
        let prefix = format!("{ERR_PREFIX:<PREFIX_WIDTH$}");

        writeln!(
            writer,
            "{} {target:<width$} {}",
            error(&prefix),
            error("Unhealthy")
        )?;

        let indent = PREFIX_WIDTH + 1;

        for issue in inspection.issues() {
            writeln!(writer, "{:indent$}{}", "", error(&issue.to_string()))?;
        }
    }

    Ok(())
}

fn print_summary(
    writer: &mut impl Write,
    inspections: &[ManagedEntryInspection],
) -> std::io::Result<()> {
    let healthy = inspections
        .iter()
        .filter(|insection| insection.is_healthy())
        .count();

    let unhealthy = inspections.len() - healthy;

    writeln!(writer)?;

    writeln!(
        writer,
        "{} healthy, {} unhealthy",
        success(&healthy.to_string()),
        error(&unhealthy.to_string())
    )?;

    Ok(())
}

pub fn print_inspections(
    writer: &mut impl Write,
    inspections: &[ManagedEntryInspection],
) -> std::io::Result<()> {
    if inspections.is_empty() {
        writeln!(writer, "{}", muted("No managed entries found."))?;
        return Ok(());
    }

    let width = inspections
        .iter()
        .map(|inspection| {
            inspection
                .entry()
                .target()
                .path()
                .to_string_lossy()
                .chars()
                .count()
        })
        .max()
        .unwrap_or(0);

    for inspection in inspections {
        print_inspection(writer, inspection, width)?;
    }

    print_summary(writer, inspections)?;

    Ok(())
}

#[cfg(test)]
#[path = "tests/inspection.rs"]
mod tests;
