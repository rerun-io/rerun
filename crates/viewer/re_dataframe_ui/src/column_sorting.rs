use re_sdk_types::blueprint::components::ColumnName;
use re_ui::UiExt as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

impl SortDirection {
    pub fn iter() -> impl Iterator<Item = Self> {
        [Self::Ascending, Self::Descending].into_iter()
    }

    pub fn is_ascending(&self) -> bool {
        matches!(self, Self::Ascending)
    }

    pub fn icon(&self) -> &'static re_ui::Icon {
        match self {
            Self::Ascending => &re_ui::icons::ARROW_DOWN,
            Self::Descending => &re_ui::icons::ARROW_UP,
        }
    }

    pub fn menu_item_ui(&self, ui: &mut egui::Ui) -> egui::Response {
        ui.icon_and_text_menu_item(
            self.icon(),
            match self {
                Self::Ascending => "Ascending",
                Self::Descending => "Descending",
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    pub column_name: ColumnName,
    pub direction: SortDirection,
}

impl SortBy {
    pub fn ascending(column_name: ColumnName) -> Self {
        Self {
            column_name,
            direction: SortDirection::Ascending,
        }
    }

    pub fn descending(column_name: ColumnName) -> Self {
        Self {
            column_name,
            direction: SortDirection::Descending,
        }
    }
}
