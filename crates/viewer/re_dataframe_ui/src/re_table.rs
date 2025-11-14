use crate::re_table_utils::{TableConfig, apply_table_style_fixes, cell_ui, header_ui};
use egui::{Context, FontSelection, Id, Rangef, RichText, TextWrapMode, Ui, WidgetText};
use egui_table::{CellInfo, HeaderCellInfo, PrefetchInfo};
use re_format::format_uint;
use re_ui::{TableStyle, UiExt};
use std::iter;
use std::sync::Arc;

/// Wrapper around [`egui_table`] that handles styling, selection, column visibility, row numbers, etc.
pub struct ReTable<'a> {
    session_id: Id,
    inner: &'a mut dyn egui_table::TableDelegate,
    config: &'a TableConfig,
    num_rows: u64,
    table_style: TableStyle,
    original_style: Arc<egui::Style>,
}

impl<'a> ReTable<'a> {
    pub fn new(
        session_id: Id,
        inner: &'a mut dyn egui_table::TableDelegate,
        config: &'a TableConfig,
        num_rows: u64,
    ) -> Self {
        Self {
            session_id,
            inner,
            config,
            num_rows,
            table_style: TableStyle::Spacious,
            original_style: Arc::new(egui::Style::default()), // Will be set in show().
        }
    }

    fn row_number_text(rows: u64) -> WidgetText {
        WidgetText::from(RichText::new(format_uint(rows)).weak().monospace())
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // Calculate the maximum width of the row number column. Since we use monospace text,
        // calculating the width of the highest row number is sufficient.
        let max_row_number_width = (Self::row_number_text(self.num_rows)
            .into_galley(
                ui,
                Some(TextWrapMode::Extend),
                1000.0,
                FontSelection::Default,
            )
            .rect
            .width()
            + ui.tokens().table_cell_margin(self.table_style).sum().x)
            .ceil();

        self.original_style = ui.style().clone();
        apply_table_style_fixes(ui.style_mut());

        egui_table::Table::new()
            .id_salt(self.session_id)
            .num_sticky_cols(1) // Row number column is sticky.
            .columns(
                iter::once(
                    egui_table::Column::new(max_row_number_width)
                        .resizable(false)
                        .range(Rangef::new(max_row_number_width, max_row_number_width))
                        .id(Id::new("row_number")),
                )
                .chain(
                    self.config
                        .visible_column_ids()
                        .map(|id| egui_table::Column::new(200.0).resizable(true).id(id)),
                )
                .collect::<Vec<_>>(),
            )
            .headers(vec![egui_table::HeaderRow::new(
                ui.tokens().table_header_height(),
            )])
            .num_rows(self.num_rows)
            .show(ui, self);

        ui.set_style(self.original_style.clone());
    }
}

impl<'a> egui_table::TableDelegate for ReTable<'a> {
    fn prepare(&mut self, info: &PrefetchInfo) {
        self.inner.prepare(info)
    }

    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let table_style = self.table_style;

        header_ui(ui, table_style, cell.group_index != 0, |ui| {
            ui.set_truncate_style();
            ui.set_style(self.original_style.clone());

            if cell.group_index == 0 {
                ui.weak("#");
            } else {
                // Offset by one for the row number column.
                let column_index = cell.group_index - 1;

                if let Some(col_index) = self.config.visible_column_indexes().nth(column_index) {
                    let mut header_cell_info = cell.clone();
                    header_cell_info.group_index = col_index;

                    self.inner.header_cell_ui(ui, &header_cell_info);
                }
            }
        });
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        cell_ui(ui, self.table_style, false, |ui| {
            ui.set_style(self.original_style.clone());
            ui.set_truncate_style();
            if cell.col_nr == 0 {
                // This is the row number column.
                ui.label(Self::row_number_text(cell.row_nr));
            } else {
                // Offset by one for the row number column.
                let col_index = cell.col_nr - 1;
                if let Some(col_index) = self.config.visible_column_indexes().nth(col_index) {
                    let mut cell_info = cell.clone();
                    cell_info.col_nr = col_index;
                    self.inner.cell_ui(ui, &cell_info)
                }
            }
        });
    }

    fn row_top_offset(&self, ctx: &Context, table_id: Id, row_nr: u64) -> f32 {
        self.inner.row_top_offset(ctx, table_id, row_nr)
    }

    fn default_row_height(&self) -> f32 {
        self.inner.default_row_height()
    }
}
