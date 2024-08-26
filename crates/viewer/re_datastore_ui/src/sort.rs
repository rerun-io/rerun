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

/// Private wrapper to make formatting [`SortDirection`] easier in [`sortable_column_header_ui`].
struct SortDirectionHeaderPrinter(Option<SortDirection>);

impl std::fmt::Display for SortDirectionHeaderPrinter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(direction) = self.0 {
            match direction {
                SortDirection::Ascending => " ▼".fmt(f),
                SortDirection::Descending => " ▲".fmt(f),
            }
        } else {
            Ok(())
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
    let label = format!(
        "{label}{}",
        SortDirectionHeaderPrinter(is_sorted.then_some(sort_column.direction)),
    );

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
}
