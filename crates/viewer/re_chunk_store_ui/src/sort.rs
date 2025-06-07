//! Helpers to assist with column-based sorting.

//TODO(ab): make this more generally applicable, in particular for the dataframe view?

use re_ui::UiExt as _;

/// Sort direction.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) enum SortDirection {
    #[default]
    Ascending,
    Descending,
}

impl SortDirection {
    pub(crate) fn toggle(&mut self) {
        match self {
            Self::Ascending => *self = Self::Descending,
            Self::Descending => *self = Self::Ascending,
        }
    }
}

/// Defines which column is currently sorted and in which direction.
#[derive(Default, Clone, Copy)]
pub(crate) struct SortColumn<T> {
    pub(crate) column: T,
    pub(crate) direction: SortDirection,
}

/// UI for a column header that is sortable.
pub(crate) fn sortable_column_header_ui<T: Default + Copy + PartialEq>(
    column: &T,
    ui: &mut egui::Ui,
    sort_column: &mut SortColumn<T>,
    label: &'static str,
) {
    let tokens = ui.tokens();
    let is_sorted = &sort_column.column == column;
    let direction = sort_column.direction;

    let (left_clicked, right_clicked) = egui::Sides::new()
        .height(tokens.deprecated_table_line_height())
        .show(
            ui,
            |ui| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

                ui.button(egui::WidgetText::from(label).strong()).clicked()
            },
            |ui| {
                ui.button(match (is_sorted, direction) {
                    (true, SortDirection::Ascending) => "↓",
                    (true, SortDirection::Descending) => "↑",
                    _ => "",
                })
                .clicked()
            },
        );

    if left_clicked || right_clicked {
        if is_sorted {
            sort_column.direction.toggle();
        } else {
            sort_column.column = *column;
            sort_column.direction = SortDirection::default();
        }
    }
}
