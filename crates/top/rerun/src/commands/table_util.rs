use comfy_table::{CellAlignment, ContentArrangement, Table, TableComponent, presets};
use re_error::try_println;

/// Prints a plain, aligned text table with an underlined header row.
pub fn print_table(
    headers: &[&str],
    rows: &[Vec<String>],
    right_aligned: &[usize],
) -> std::io::Result<()> {
    if headers.is_empty()
        || rows.iter().any(|row| row.len() != headers.len())
        || right_aligned.iter().any(|&column| column >= headers.len())
    {
        return Ok(());
    }

    let mut table = Table::new();
    table
        .load_preset(presets::NOTHING)
        .set_content_arrangement(ContentArrangement::Disabled)
        .set_style(TableComponent::HeaderLines, '─')
        .set_header(headers);

    for row in rows {
        table.add_row(row);
    }

    for &column in right_aligned {
        let Some(column) = table.column_mut(column) else {
            return Ok(());
        };
        column.set_cell_alignment(CellAlignment::Right);
    }

    for (index, column) in table.column_iter_mut().enumerate() {
        let right_padding = if index + 1 == headers.len() { 0 } else { 2 };
        column.set_padding((0, right_padding));
    }

    try_println!("{}", table.trim_fmt())
}
