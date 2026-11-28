use egui_table::{AutoSizeMode, CellInfo, Column, HeaderCellInfo, HeaderRow, Table, TableDelegate};

use crate::{TableStyle, UiExt as _};

/// Show a table with resizable columns and a fixed height for each row.
///
/// The table takes its full content height so that it can scroll with its parent.
/// Each column specifies its title and initial width.
/// The cell callback receives the zero-based row and column indices.
pub fn embedded_table_ui(
    ui: &mut egui::Ui,
    id_salt: impl egui::AsIdSalt,
    style: TableStyle,
    columns: &[(&str, f32)],
    num_rows: usize,
    cell_ui: impl FnMut(&mut egui::Ui, usize, usize),
) {
    let tokens = ui.tokens();
    let header_height = tokens.table_header_height();
    let row_height = tokens.table_row_height(style);

    // Bound the child UI to the table's content height. This lets the parent scroll vertically
    // while egui_table still uses the parent's clip rect to render only visible rows.
    let mut rect = ui.available_rect_before_wrap();
    rect.set_height(header_height + row_height * num_rows as f32);
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        Table::new()
            .id_salt(id_salt)
            .columns(
                columns
                    .iter()
                    .map(|&(_, width)| Column::new(width))
                    .collect::<Vec<_>>(),
            )
            .auto_size_mode(AutoSizeMode::Never)
            .headers(vec![HeaderRow::new(header_height)])
            .num_rows(num_rows as u64)
            .show(
                ui,
                &mut EmbeddedTable {
                    style,
                    columns,
                    row_height,
                    cell_ui,
                },
            );
    });
}

struct EmbeddedTable<'a, F> {
    style: TableStyle,
    columns: &'a [(&'a str, f32)],
    row_height: f32,
    cell_ui: F,
}

impl<F: FnMut(&mut egui::Ui, usize, usize)> TableDelegate for EmbeddedTable<'_, F> {
    fn default_row_height(&self) -> f32 {
        self.row_height
    }

    fn header_cell_ui(&mut self, ui: &mut egui::Ui, cell: &HeaderCellInfo) {
        if let Some(&(title, _)) = self.columns.get(cell.col_range.start) {
            let tokens = ui.tokens();
            ui.painter()
                .rect_filled(ui.max_rect(), 0.0, tokens.table_header_bg_fill);
            egui::Frame::new()
                .inner_margin(tokens.header_cell_margin(self.style))
                .show(ui, |ui| ui.strong(title));
        }
    }

    fn cell_ui(&mut self, ui: &mut egui::Ui, cell: &CellInfo) {
        let tokens = ui.tokens();
        if !ui.is_sizing_pass() {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        }
        ui.spacing_mut().interact_size.y = tokens.table_content_height(self.style);
        ui.spacing_mut().item_spacing.y = 0.0;
        egui::Frame::new()
            .inner_margin(tokens.table_cell_margin(self.style))
            .show(ui, |ui| {
                (self.cell_ui)(ui, cell.row_nr as usize, cell.col_nr);
            });
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::{Harness, kittest::Queryable as _};

    use super::*;

    #[test]
    fn embedded_table_scrolls_with_parent() {
        let mut harness = Harness::builder()
            .with_context_setup(crate::apply_style_and_install_loaders)
            .with_size(egui::vec2(220.0, 140.0))
            .with_max_steps(12)
            .build_ui(|ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    embedded_table_ui(
                        ui,
                        "table",
                        TableStyle::Dense,
                        &[("Coordinates", 120.0)],
                        30,
                        |ui, row, _| {
                            ui.label(format!("Row {row}"));
                        },
                    );
                    ui.label("After table");
                });
            });
        harness.run();

        let first_row = harness.get_by_label("Row 0").rect();
        assert!(harness.query_by_label("Row 29").is_none());

        harness.hover_at(first_row.center());
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -2_000.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
            source: egui::MouseWheelSource::Unknown,
        });
        harness.run();

        assert!(harness.query_by_label("Row 0").is_none());
        let last_row = harness.get_by_label("Row 29").rect();
        let after_table = harness.get_by_label("After table").rect();
        assert!(
            last_row.bottom() <= after_table.top() && after_table.bottom() <= 140.0,
            "The last row and following label should both be visible: {last_row:?}, {after_table:?}",
        );
    }
}
