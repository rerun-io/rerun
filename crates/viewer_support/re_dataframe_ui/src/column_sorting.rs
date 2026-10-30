use re_sdk_types::blueprint::components::{ColumnName, SortBy as BlueprintSortBy};
use re_sdk_types::blueprint::encodings::SortBy as BlueprintSortByEncoding;
pub use re_sdk_types::blueprint::encodings::SortDirection;
use re_ui::UiExt as _;

pub trait SortDirectionUiExt {
    fn icon(&self) -> &'static re_ui::Icon;

    fn menu_item_ui(&self, ui: &mut egui::Ui) -> egui::Response;
}

impl SortDirectionUiExt for SortDirection {
    fn icon(&self) -> &'static re_ui::Icon {
        match self {
            Self::Ascending => &re_ui::icons::ARROW_DOWN,
            Self::Descending => &re_ui::icons::ARROW_UP,
        }
    }

    fn menu_item_ui(&self, ui: &mut egui::Ui) -> egui::Response {
        ui.icon_and_text_menu_item(self.icon(), self.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    pub column_name: ColumnName,
    pub direction: SortDirection,
}

impl From<BlueprintSortBy> for SortBy {
    fn from(sort_by: BlueprintSortBy) -> Self {
        let BlueprintSortByEncoding { column, direction } = sort_by.0;
        Self {
            column_name: column.as_str().into(),
            direction: direction.unwrap_or_default(),
        }
    }
}

impl From<&SortBy> for BlueprintSortByEncoding {
    fn from(sort_by: &SortBy) -> Self {
        Self {
            column: sort_by.column_name.as_str().into(),
            direction: Some(sort_by.direction),
        }
    }
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
