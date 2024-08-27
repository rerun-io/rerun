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
//TODO(ab): this UI could be much improved with https://github.com/emilk/egui/issues/5015
pub(crate) fn sortable_column_header_ui<T: Default + Copy + PartialEq>(
    column: &T,
    ui: &mut egui::Ui,
    sort_column: &mut SortColumn<T>,
    label: &'static str,
) {
    let is_sorted = &sort_column.column == column;

    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);

    if ui
        .add(egui::Button::new(
            egui::WidgetText::from(format!(
                "{label}{}",
                match (is_sorted, sort_column.direction) {
                    (true, SortDirection::Ascending) => " ↓",
                    (true, SortDirection::Descending) => " ↑",
                    _ => "",
                }
            ))
            .strong(),
        ))
        .clicked()
    {
        if is_sorted {
            sort_column.direction.toggle();
        } else {
            sort_column.column = *column;
            sort_column.direction = SortDirection::default();
        }
    }
}
