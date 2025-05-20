use re_log_types::{EntityPath, EntryId};
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_ui::UiExt as _;
use re_viewer_context::VariantName;

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
        ui.add(egui::Button::image_and_text(
            self.icon()
                .as_image()
                .tint(ui.design_tokens().label_button_icon_color())
                .fit_to_exact_size(re_ui::DesignTokens::small_icon_size()),
            match self {
                Self::Ascending => "Ascending",
                Self::Descending => "Descending",
            },
        ))
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

//TODO docstring
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnBlueprint {
    pub name: Option<String>,
    pub default_visibility: bool,
    pub alternate_ui: Option<VariantName>,
}

impl Default for ColumnBlueprint {
    fn default() -> Self {
        Self {
            name: None,
            default_visibility: true,
            alternate_ui: None,
        }
    }
}

impl ColumnBlueprint {
    pub fn default_ref() -> &'static Self {
        use std::sync::OnceLock;
        static DEFAULT: OnceLock<ColumnBlueprint> = OnceLock::new();
        DEFAULT.get_or_init(Self::default)
    }

    pub fn name(self, name: impl Into<String>) -> Self {
        Self {
            name: Some(name.into()),
            ..self
        }
    }

    pub fn name_from_descriptor(self, desc: &ColumnDescriptorRef<'_>) -> Self {
        Self {
            name: Some(default_display_name_for_column(desc)),
            ..self
        }
    }

    pub fn default_visibility(self, initial_visibility: bool) -> Self {
        Self {
            default_visibility: initial_visibility,
            ..self
        }
    }

    pub fn alternate_ui(self, alternate_ui: impl Into<VariantName>) -> Self {
        Self {
            alternate_ui: Some(alternate_ui.into()),
            ..self
        }
    }
}

pub fn default_display_name_for_column(desc: &ColumnDescriptorRef<'_>) -> String {
    match desc {
        ColumnDescriptorRef::RowId(_) | ColumnDescriptorRef::Time(_) => desc.display_name(),

        ColumnDescriptorRef::Component(desc) => {
            if desc.entity_path == EntityPath::root() {
                // In most case, user tables don't have any entities, so we filter out the root entity
                // noise in column names.
                desc.column_name(BatchType::Chunk)
            } else {
                desc.column_name(BatchType::Dataframe)
            }
        }
    }
}
