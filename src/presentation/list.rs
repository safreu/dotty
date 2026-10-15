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
#[path = "tests/list.rs"]
mod tests;
