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

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ascending => " ▼".fmt(f),
            Self::Descending => " ▲".fmt(f),
        }
    }
}

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
    let label = format!(
        "{label}{}",
        if column == &sort_column.column {
            format!(" {}", sort_column.direction)
        } else {
            String::new()
        }
    );

    if ui
        .add(egui::Button::new(egui::WidgetText::from(label).strong()))
        .clicked()
    {
        if &sort_column.column == column {
            sort_column.direction.toggle();
        } else {
            sort_column.column = *column;
            sort_column.direction = SortDirection::default();
        }
    }
}
