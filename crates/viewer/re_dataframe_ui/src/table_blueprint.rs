use re_log_types::EntryId;

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

    pub fn menu_button(&self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(
            egui::Button::image_and_text(
                self.icon()
                    .as_image()
                    .fit_to_exact_size(re_ui::DesignTokens::small_icon_size()),
                match self {
                    Self::Ascending => "Ascending",
                    Self::Descending => "Descending",
                },
            )
            .image_tint_follows_text_color(true),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortBy {
    pub column: String,
    pub direction: SortDirection,
}

/// Information required to generate a partition link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PartitionLinksSpec {
    /// Name of the column to generate.
    pub column_name: String,

    /// Name of the existing column containing the partition id.
    pub partition_id_column_name: String,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,

    /// The id of the dataset to use for the links.
    pub dataset_id: EntryId,
}

/// The "blueprint" for a table, a.k.a the specification of how it should look.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableBlueprint {
    pub sort_by: Option<SortBy>,
    pub partition_links: Option<PartitionLinksSpec>,
}
