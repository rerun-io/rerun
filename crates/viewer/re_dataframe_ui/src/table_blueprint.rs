use std::str::FromStr as _;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_log_types::{EntityPath, EntryId};
use re_sdk_types::blueprint::{
    archetypes::TableBlueprint as TableBlueprintArchetype, components::ColumnName,
};
use re_sorbet::{BatchType, ColumnDescriptorRef};
use re_types_core::Archetype as _;
use re_ui::UiExt as _;
use re_viewer_context::{VariantName, blueprint_timeline};

use crate::DisplayRecordBatch;
use crate::datafusion_table_widget::Columns;
use crate::display_record_batch::DisplayColumn;
use crate::filters::ColumnFilter;
use crate::re_table_utils::TableConfig;

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
///
/// This is the single source of truth for table configuration. Fields can be populated
/// from the embedded `.fbs` `TableBlueprint` archetype or set programmatically.
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

    /// The name of the column containing recording URIs for segment previews.
    pub segment_preview_column: Option<String>,

    /// The name of the boolean column used for flag annotations.
    ///
    /// The column must exist in the table and be of boolean type.
    /// Populated from schema metadata ([`crate::experimental_field_metadata::IS_FLAG_COLUMN`])
    /// or the embedded `.fbs` `TableBlueprint` archetype.
    pub flag_column: Option<String>,

    /// The name of the column to use as the card title in grid view.
    ///
    /// If unset, the first visible string column is used.
    /// Populated from schema metadata ([`crate::experimental_field_metadata::IS_GRID_VIEW_CARD_TITLE`])
    /// or the embedded `.fbs` `TableBlueprint` archetype.
    pub grid_view_card_title: Option<String>,

    /// The name of the column containing URLs to open when a card is clicked in grid view.
    ///
    /// If unset, the first column whose values parse as a Rerun URI pointing to the same
    /// Rerun server is used (resolved ad-hoc in the grid view). If no such column exists,
    /// clicking a card does not navigate anywhere.
    /// Populated from the embedded `.fbs` `TableBlueprint` archetype.
    pub url_column: Option<String>,
}

impl TableBlueprint {
    /// Populate fields from an embedded `.fbs` `TableBlueprint` archetype stored in a blueprint
    /// [`EntityDb`].
    pub fn populate_from_embedded_blueprint(&mut self, blueprint_db: &EntityDb) {
        let blueprint_query = LatestAtQuery::latest(blueprint_timeline());
        let engine = blueprint_db.storage_engine();
        let results = engine.cache().latest_at(
            &blueprint_query,
            &"/table".into(),
            TableBlueprintArchetype::all_component_identifiers(),
        );

        self.segment_preview_column = results
            .component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_segment_preview_column().component,
            )
            .map(|name| name.0.to_string());

        self.flag_column = results
            .component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_flag_column().component,
            )
            .map(|name| name.0.to_string());

        self.grid_view_card_title = results
            .component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_grid_view_card_title().component,
            )
            .map(|name| name.0.to_string());

        self.url_column = results
            .component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_url_column().component,
            )
            .map(|name| name.0.to_string());
    }

    /// Fill in unset fields with defaults inferred from the table's runtime state.
    ///
    /// Call after [`Self::populate_from_embedded_blueprint`]. Fields already set by the user
    /// or the embedded blueprint are left untouched.
    ///
    /// Sources applied (in order, first match wins per field):
    /// 1. Per-field Arrow schema metadata (see [`crate::experimental_field_metadata`]).
    /// 2. Structural heuristics over the loaded columns/data.
    pub fn apply_heuristics(
        &mut self,
        schema: &arrow::datatypes::Schema,
        columns: &Columns<'_>,
        display_record_batches: &[DisplayRecordBatch],
        table_config: &TableConfig,
        current_server_origin: Option<&re_uri::Origin>,
    ) {
        if self.flag_column.is_none() {
            self.flag_column =
                find_field_with_flag(schema, crate::experimental_field_metadata::IS_FLAG_COLUMN)
                    .map(str::to_owned);
        }

        if self.grid_view_card_title.is_none() {
            self.grid_view_card_title = find_field_with_flag(
                schema,
                crate::experimental_field_metadata::IS_GRID_VIEW_CARD_TITLE,
            )
            .map(str::to_owned)
            .or_else(|| {
                table_config.visible_column_indexes().find_map(|col_idx| {
                    let col = columns.columns.get(col_idx)?;
                    matches!(
                        &col.desc,
                        ColumnDescriptorRef::Component(c)
                            if c.store_datatype == arrow::datatypes::DataType::Utf8
                    )
                    .then(|| col.display_name())
                })
            });
        }

        if self.url_column.is_none() {
            self.url_column = columns
                .columns
                .iter()
                .enumerate()
                .filter(|(_, c)| {
                    matches!(
                        &c.desc,
                        ColumnDescriptorRef::Component(c)
                            if c.store_datatype == arrow::datatypes::DataType::Utf8
                    )
                })
                .find_map(|(idx, col)| {
                    let sample = display_record_batches.iter().find_map(|batch| {
                        let DisplayColumn::Component(comp) = batch.columns().get(idx)? else {
                            return None;
                        };
                        (0..batch.num_rows()).find_map(|row| comp.string_value_at(row))
                    })?;
                    let uri = re_uri::RedapUri::from_str(&sample).ok()?;
                    current_server_origin
                        .is_none_or(|origin| uri.origin() == origin)
                        .then(|| col.display_name())
                });
        }
    }
}

/// Return the name of the single field with `metadata[key] == "true"`, warning if multiple match.
fn find_field_with_flag<'a>(schema: &'a arrow::datatypes::Schema, key: &str) -> Option<&'a str> {
    let mut found = None;
    for field in schema.fields() {
        if field.metadata().get(key).map(String::as_str) == Some("true") {
            if found.is_some() {
                re_log::warn_once!(
                    "Multiple fields have {key:?} metadata set; using the first one"
                );
                break;
            }
            found = Some(field.name().as_str());
        }
    }
    found
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
