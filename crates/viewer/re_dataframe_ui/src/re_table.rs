use std::iter;
use std::sync::Arc;

use egui::emath::GuiRounding as _;
use egui::text_selection::LabelSelectionState;
use egui::{
    Align, Direction, FontSelection, Id, Layout, NumExt as _, Rangef, RichText, TextWrapMode, Ui,
    UiBuilder, WidgetText,
};
use egui_table::{CellInfo, HeaderCellInfo, PrefetchInfo};
use re_format::format_uint;
use re_ui::egui_ext::response_ext::ResponseExt as _;
use re_ui::{TableStyle, UiExt as _};

use crate::re_table_utils::{TableConfig, apply_table_style_fixes, cell_ui, header_ui};
use crate::table_selection::TableSelectionState;

/// Wrapper around [`egui_table::TableDelegate`] that handles styling, selection, column visibility, row numbers, etc.
pub struct ReTable<'a> {
    session_id: Id,
    inner: &'a mut dyn egui_table::TableDelegate,
    config: &'a TableConfig,
    selection: TableSelectionState,
    previous_selection: TableSelectionState,
    num_rows: u64,
    table_style: TableStyle,

    /// We apply changes to [`egui::Style`] when rendering the table.
    ///
    /// We remember the original so it doesn't affect the cell contents.
    original_style: Arc<egui::Style>,
}

impl<'a> ReTable<'a> {
    pub fn new(
        egui_ctx: &egui::Context,
        session_id: Id,
        inner: &'a mut dyn egui_table::TableDelegate,
        config: &'a TableConfig,
        num_rows: u64,
    ) -> Self {
        let mut selection = TableSelectionState::load(egui_ctx, session_id);
        let previous_selection = selection.clone();
        selection.all_hovered = false;
        selection.hovered_row = None;
        Self {
            session_id,
            inner,
            config,
            selection,
            previous_selection,
            num_rows,
            table_style: TableStyle::Spacious,
            original_style: Arc::new(egui::Style::default()), // Will be set in show().
        }
    }

    fn row_number_text(row: u64) -> WidgetText {
        WidgetText::from(RichText::new(format_uint(row)).weak().monospace())
    }

    fn add_row_number_content<R>(ui: &mut Ui, content: impl FnOnce(&mut Ui) -> R) -> R {
        content(
            &mut ui.new_child(UiBuilder::new().max_rect(ui.max_rect()).layout(
                Layout::centered_and_justified(Direction::TopDown).with_cross_align(Align::Max),
            )),
        )
    }

    fn row_selection_checkbox(
        ui: &mut Ui,
        checked: &mut bool,
        intermediate: bool,
    ) -> egui::Response {
        Self::add_row_number_content(ui, |ui| {
            ui.checkbox_indeterminate(checked, (), intermediate)
        })
    }

    pub fn show(&mut self, ui: &mut Ui) {
        // Calculate the maximum width of the row number column. Since we use monospace text,
        // calculating the width of the highest row number is sufficient.
        let max_row_number_width = Self::row_number_text(self.num_rows)
            .into_galley(
                ui,
                Some(TextWrapMode::Extend),
                1000.0,
                FontSelection::Default,
            )
            .rect
            .width();

        // Ensure the checkbox fits.
        let checkbox_width = ui
            .spacing()
            .interact_size
            .y
            .at_least(ui.spacing().icon_width);

        let row_number_cell_width = (max_row_number_width.at_least(checkbox_width)
            + ui.tokens().table_cell_margin(self.table_style).sum().x)
            .ceil();

        self.original_style = ui.style().clone();
        apply_table_style_fixes(ui.style_mut());

        egui_table::Table::new()
            .id_salt(self.session_id)
            .num_sticky_cols(1) // Row number column is sticky.
            .columns(
                iter::once(
                    egui_table::Column::new(row_number_cell_width)
                        .resizable(false)
                        .range(Rangef::new(row_number_cell_width, row_number_cell_width))
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
        self.selection.clone().store(ui.ctx(), self.session_id);
    }
}

impl egui_table::TableDelegate for ReTable<'_> {
    fn prepare(&mut self, info: &PrefetchInfo) {
        self.inner.prepare(info);
    }

    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let table_style = self.table_style;

        header_ui(ui, table_style, cell.group_index != 0, |ui| {
            ui.set_truncate_style();
            ui.set_style(self.original_style.clone());

            if cell.group_index == 0 {
                let hovered = ui.rect_contains_pointer(
                    ui.max_rect().expand(ui.style().interaction.interact_radius),
                );
                if hovered {
                    self.selection.all_hovered = true;
                }
                let show_checkbox = hovered || !self.selection.selected_rows.is_empty();
                if show_checkbox {
                    // It's checked
                    let mut checked = !self.selection.selected_rows.is_empty();
                    let intermediate =
                        self.selection.selected_rows.len() as u64 != self.num_rows && checked;
                    let response = Self::row_selection_checkbox(ui, &mut checked, intermediate);
                    if response.changed() {
                        if checked {
                            self.selection.selected_rows.extend(0..self.num_rows);
                        } else {
                            self.selection.selected_rows.clear();
                        }
                    }
                } else {
                    Self::add_row_number_content(ui, |ui| {
                        ui.label("#");
                    });
                }
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
            ui.set_truncate_style();
            if cell.col_nr == 0 {
                // This is the row number column.
                let show_checkbox = self.previous_selection.all_hovered
                    || self.previous_selection.hovered_row == Some(cell.row_nr);
                if show_checkbox {
                    let mut checked = self.previous_selection.selected_rows.contains(&cell.row_nr);
                    let response = Self::row_selection_checkbox(ui, &mut checked, false);
                    if response.changed() {
                        // If the checkbox is clicked, the row will also detect the click.
                        // Since we want the checkbox to have a different click behavior,
                        // undo the row click:
                        self.selection = self.previous_selection.clone();

                        let modifiers = ui.input(|i| i.modifiers);
                        self.selection
                            .handle_row_click(cell.row_nr, modifiers, true);
                    }
                } else {
                    Self::add_row_number_content(ui, |ui| {
                        ui.label(Self::row_number_text(cell.row_nr));
                    });
                }
            } else {
                // Offset by one for the row number column.
                let col_index = cell.col_nr - 1;
                if let Some(col_index) = self.config.visible_column_indexes().nth(col_index) {
                    let mut cell_info = cell.clone();
                    cell_info.col_nr = col_index;
                    self.inner.cell_ui(ui, &cell_info);
                }
            }
        });
    }

    fn row_ui(&mut self, ui: &mut Ui, row_nr: u64) {
        ui.set_style(self.original_style.clone());
        let response = ui.response();
        if response.container_contains_pointer() {
            self.selection.hovered_row = Some(row_nr);
        }

        let mut fill = if self.previous_selection.hovered_row == Some(row_nr) {
            Some(
                ui.tokens()
                    .table_interaction_row_selection_fill
                    .gamma_multiply(0.5),
            )
        } else {
            None
        };

        ui.style_mut().interaction.selectable_labels = false;
        if self.selection.selected_rows.contains(&row_nr) {
            fill = Some(ui.tokens().table_interaction_row_selection_fill);
            let modifiers_pressed = ui.input(|i| i.modifiers.shift || i.modifiers.command);
            let any_selection = ui
                .ctx()
                .with_plugin::<LabelSelectionState, bool>(|p| p.has_selection())
                .unwrap_or(false);
            // Only enable selection if no modifiers are pressed or there is a current selection
            // (to allow cmd c).
            if !modifiers_pressed || any_selection {
                ui.style_mut().interaction.selectable_labels = true;
            }
        }

        if let Some(fill) = fill {
            let fill_rect = response
                .rect
                .round_to_pixels(ui.pixels_per_point())
                .round_ui();
            ui.painter().rect_filled(fill_rect, 0.0, fill);
        }

        if response.container_clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            self.selection.handle_row_click(row_nr, modifiers, false);
        }
        if response.container_secondary_clicked() && !self.selection.selected_rows.contains(&row_nr)
        {
            // If right-clicking a non-selected row, select it first.
            let modifiers = ui.input(|i| i.modifiers);
            self.selection.handle_row_click(row_nr, modifiers, false);
        }
        self.inner.row_ui(ui, row_nr);
    }

    fn row_top_offset(&self, egui_ctx: &egui::Context, table_id: Id, row_nr: u64) -> f32 {
        self.inner.row_top_offset(egui_ctx, table_id, row_nr)
    }

    fn default_row_height(&self) -> f32 {
        self.inner.default_row_height()
    }
}
