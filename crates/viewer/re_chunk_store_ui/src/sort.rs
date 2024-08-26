//! Helpers to assist with column-based sorting.

//TODO(ab): make this more generally applicable, in particular for the dataframe view?

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
    let is_sorted = &sort_column.column == column;

    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 2.0;

        if is_sorted {
            ui.label(match sort_column.direction {
                SortDirection::Ascending => "↓",
                SortDirection::Descending => "↑",
            });
        }

        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

            if ui
                .add(egui::Button::new(egui::WidgetText::from(label).strong()))
                .clicked()
            {
                if is_sorted {
                    sort_column.direction.toggle();
                } else {
                    sort_column.column = *column;
                    sort_column.direction = SortDirection::default();
                }
            }
        });
    });
}
