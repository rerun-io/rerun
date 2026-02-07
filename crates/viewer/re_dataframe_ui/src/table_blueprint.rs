use re_log_types::{EntityPath, EntryId};
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_ui::UiExt as _;
use re_viewer_context::VariantName;

use crate::filters::ColumnFilter;

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
    pub column_physical_name: String,
    pub direction: SortDirection,
}

impl SortBy {
    pub fn ascending(col_name: impl Into<String>) -> Self {
        Self {
            column_physical_name: col_name.into(),
            direction: SortDirection::Ascending,
        }
    }

    pub fn descending(col_name: impl Into<String>) -> Self {
        Self {
            column_physical_name: col_name.into(),
            direction: SortDirection::Descending,
        }
    }
}

/// Information required to generate a segment link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SegmentLinksSpec {
    /// Name of the column to generate.
    pub column_name: String,

    /// Name of the existing column containing the segment id.
    pub segment_id_column_name: String,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,

    /// The id of the dataset to use for the links.
    pub dataset_id: EntryId,
}

/// Information required to generate an entry link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryLinksSpec {
    /// Name of the column to generate.
    pub column_name: String,

    /// Name of the existing column containing the entry id.
    pub entry_id_column_name: String,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,
}

/// The "blueprint" for a table, a.k.a the specification of how it should look.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableBlueprint {
    pub sort_by: Option<SortBy>,
    pub segment_links: Option<SegmentLinksSpec>,
    pub entry_links: Option<EntryLinksSpec>,

    /// Always-on filter specified by calling code.
    ///
    /// For example, exclude blueprint dataset from the entries table.
    pub prefilter: Option<datafusion::logical_expr::Expr>,

    /// Filters specified by the user in the UI.
    pub column_filters: Vec<ColumnFilter>,
}

/// The blueprint for a specific column.
// TODO(ab): these should eventually be stored in `TableBlueprint`, but is currently not strictly
// necessary since we don't need to store the column blueprint for now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnBlueprint {
    /// The name to use for this column in the UI.
    ///
    /// If `None`, the column will be named using [`default_display_name_for_column`].
    pub display_name: Option<String>,
    pub default_visibility: bool,
    pub variant_ui: Option<VariantName>,
    pub sort_key: i64,
}

impl Default for ColumnBlueprint {
    fn default() -> Self {
        Self {
            display_name: None,
            default_visibility: true,
            variant_ui: None,
            sort_key: 0,
        }
    }
}

impl ColumnBlueprint {
    /// Same as [`Self::default()`], but returns a reference to a static instance.
    pub fn default_ref() -> &'static Self {
        use std::sync::LazyLock;
        static DEFAULT: LazyLock<ColumnBlueprint> = LazyLock::new(ColumnBlueprint::default);
        &DEFAULT
    }

    /// Set the name to use for this column in the UI.
    pub fn display_name(self, name: impl Into<String>) -> Self {
        Self {
            display_name: Some(name.into()),
            ..self
        }
    }

    /// Set the default visibility of this column.
    pub fn default_visibility(self, initial_visibility: bool) -> Self {
        Self {
            default_visibility: initial_visibility,
            ..self
        }
    }

    /// Set the alternate UI to use for this column
    pub fn variant_ui(self, variant_ui: impl Into<VariantName>) -> Self {
        Self {
            variant_ui: Some(variant_ui.into()),
            ..self
        }
    }

    /// Customize the order of the columns in the UI.
    ///
    /// Default is `0`. The lower the number, the earlier the column will be shown.
    ///
    /// Order of columns with identical sort keys will depend on the order of columns in the
    /// datafusion query.
    pub fn sort_key(self, sort_key: i64) -> Self {
        Self { sort_key, ..self }
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

#[test]
fn test_default_column_display_name() {
    // Built-in recording property:
    assert_eq!(
        default_display_name_for_column(&ColumnDescriptorRef::Component(
            &re_sorbet::ComponentColumnDescriptor {
                store_datatype: arrow::datatypes::DataType::Binary, // ignored
                entity_path: EntityPath::properties(),
                component: "RecordingInfo:start_time".into(),
                component_type: Some("rerun.components.Timestamp".into()),
                archetype: Some("rerun.archetypes.RecordingInfo".into()),
                is_static: false,
                is_tombstone: false,
                is_semantically_empty: false
            },
        )),
        "property:RecordingInfo:start_time"
    );

    // User-defined recoding property:
    assert_eq!(
        default_display_name_for_column(&ColumnDescriptorRef::Component(
            &re_sorbet::ComponentColumnDescriptor {
                store_datatype: arrow::datatypes::DataType::Binary, // ignored
                component_type: None,
                entity_path: EntityPath::properties() / "episode",
                archetype: None,
                component: "building".into(),
                is_static: false,
                is_tombstone: false,
                is_semantically_empty: false
            },
        )),
        "property:episode:building"
    );
}
