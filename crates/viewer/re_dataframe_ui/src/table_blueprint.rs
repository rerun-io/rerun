use std::str::FromStr as _;

use re_chunk_store::LatestAtQuery;
use re_entity_db::EntityDb;
use re_sdk_types::blueprint::{
    archetypes::TableBlueprint as TableBlueprintArchetype, components::ColumnName,
};
use re_sorbet::ColumnDescriptorRef;
use re_types_core::Archetype as _;
use re_viewer_context::blueprint_timeline;

use crate::DisplayRecordBatch;
use crate::datafusion_table_widget::Columns;
use crate::display_record_batch::DisplayColumn;
use crate::re_table_utils::UiTableConfig;

/// Information required to generate a segment link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SegmentLinksSpec {
    /// Name of the column to generate.
    pub column_name: ColumnName,

    /// Name of the existing column containing the segment id.
    pub segment_id_column_name: ColumnName,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,

    /// The id of the dataset to use for the links.
    pub dataset_id: re_log_types::EntryId,
}

/// Information required to generate an entry link column.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EntryLinksSpec {
    /// Name of the column to generate.
    pub column_name: ColumnName,

    /// Name of the existing column containing the entry id.
    pub entry_id_column_name: ColumnName,

    /// Origin to use for the links.
    pub origin: re_uri::Origin,
}

/// The table blueprint defining how the table should be presented.
///
/// Loaded from the registered blueprint data `TableBlueprint` archetype and filled by runtime
/// heuristics.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableBlueprint {
    /// The name of the column containing recording URIs for segment previews.
    pub segment_preview_column: Option<ColumnName>,

    /// The name of the boolean column used for flag annotations.
    ///
    /// The column must exist in the table and be of boolean type.
    /// Populated from schema metadata ([`crate::experimental_field_metadata::IS_FLAG_COLUMN`])
    /// or the registered `.fbs` `TableBlueprint` archetype.
    pub flag_column: Option<ColumnName>,

    /// The name of the column to use as the card title in grid view.
    ///
    /// If unset, the first visible string column is used.
    /// Populated from schema metadata ([`crate::experimental_field_metadata::IS_GRID_VIEW_CARD_TITLE`])
    /// or the registered `.fbs` `TableBlueprint` archetype.
    pub grid_view_card_title: Option<ColumnName>,

    /// The name of the column containing URLs to open when a card is clicked in grid view.
    ///
    /// If unset, the first column whose values parse as a Rerun URI pointing to the same
    /// Rerun server is used (resolved ad-hoc in the grid view). If no such column exists,
    /// clicking a card does not navigate anywhere.
    /// Populated from the registered `.fbs` `TableBlueprint` archetype.
    pub url_column: Option<ColumnName>,
}

impl TableBlueprint {
    /// Load a fresh snapshot from a blueprint store.
    pub fn load(blueprint_db: &EntityDb) -> Self {
        let results = blueprint_db.latest_at(
            &LatestAtQuery::latest(blueprint_timeline()),
            &"/table".into(),
            TableBlueprintArchetype::all_component_identifiers(),
        );

        Self {
            segment_preview_column: results.component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_segment_preview_column().component,
            ),
            flag_column: results.component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_flag_column().component,
            ),
            grid_view_card_title: results.component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_grid_view_card_title().component,
            ),
            url_column: results.component_mono::<ColumnName>(
                TableBlueprintArchetype::descriptor_url_column().component,
            ),
        }
    }

    /// Fill in unset fields with defaults inferred from the table's runtime state.
    ///
    /// Fields already set by the user or the registered blueprint are left untouched.
    ///
    /// Sources applied (in order, first match wins per field):
    /// 1. Per-field Arrow schema metadata (see [`crate::experimental_field_metadata`]).
    /// 2. Structural heuristics over the loaded columns/data.
    pub fn apply_heuristics(
        mut self,
        schema: &arrow::datatypes::Schema,
        columns: &Columns<'_>,
        display_record_batches: &[DisplayRecordBatch],
        table_config: &UiTableConfig,
        current_server_origin: Option<&re_uri::Origin>,
    ) -> Self {
        if self.flag_column.is_none() {
            self.flag_column =
                find_field_with_flag(schema, crate::experimental_field_metadata::IS_FLAG_COLUMN)
                    .map(Into::into);
        }

        if self.grid_view_card_title.is_none() {
            self.grid_view_card_title = find_field_with_flag(
                schema,
                crate::experimental_field_metadata::IS_GRID_VIEW_CARD_TITLE,
            )
            .map(Into::into)
            .or_else(|| {
                table_config.visible_column_names().find_map(|name| {
                    let (_, column) = columns.find_by_physical_name(name)?;
                    matches!(
                        &column.desc,
                        ColumnDescriptorRef::Component(component)
                            if component.store_datatype == arrow::datatypes::DataType::Utf8
                    )
                    .then(|| column.physical_name().clone())
                })
            });
        }

        if self.url_column.is_none() && self.segment_preview_column.is_none() {
            let first_url = columns.columns.iter().enumerate().find_map(|(idx, col)| {
                if !matches!(
                    &col.desc,
                    ColumnDescriptorRef::Component(c)
                        if c.store_datatype == arrow::datatypes::DataType::Utf8
                ) {
                    return None;
                }

                let sample = display_record_batches.iter().find_map(|batch| {
                    let DisplayColumn::Component(comp) = batch.columns().get(idx)? else {
                        return None;
                    };
                    (0..batch.num_rows()).find_map(|row| comp.string_value_at(row))
                })?;
                let uri = re_uri::RedapUri::from_str(&sample).ok()?;

                current_server_origin
                    .is_none_or(|origin| uri.origin() == origin)
                    .then(|| col.physical_name().clone())
            });
            self.url_column.clone_from(&first_url);
            self.segment_preview_column = first_url;
        } else if self.url_column.is_none() && self.segment_preview_column.is_some() {
            self.url_column = self.segment_preview_column.clone();
        }

        self
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use re_chunk_store::external::re_chunk::Chunk;
    use re_entity_db::EntityDb;
    use re_log_types::{StoreId, StoreKind, TimeInt, Timeline};
    use re_sdk_types::blueprint::{
        archetypes::TableBlueprint as TableBlueprintArchetype, components::ColumnName,
    };

    use super::TableBlueprint;

    fn add_blueprint(db: &mut EntityDb, time: i64, blueprint: &TableBlueprintArchetype) {
        let timepoint = [(
            Timeline::new_sequence(re_viewer_context::blueprint_timeline()),
            TimeInt::new_temporal(time),
        )];
        let chunk = Chunk::builder("/table")
            .with_archetype_auto_row(timepoint, blueprint)
            .build()
            .unwrap();
        db.add_chunk(&Arc::new(chunk)).unwrap();
    }

    #[test]
    fn registered_snapshot_decodes_all_roles_and_clears_removed_values() {
        let mut db = EntityDb::new(StoreId::random(StoreKind::Blueprint, "test_app"));
        add_blueprint(
            &mut db,
            1,
            &TableBlueprintArchetype::new()
                .with_segment_preview_column("preview")
                .with_flag_column("flag")
                .with_grid_view_card_title("title")
                .with_url_column("url"),
        );

        assert_eq!(
            TableBlueprint::load(&db),
            TableBlueprint {
                segment_preview_column: Some(ColumnName::from("preview")),
                flag_column: Some(ColumnName::from("flag")),
                grid_view_card_title: Some(ColumnName::from("title")),
                url_column: Some(ColumnName::from("url")),
            }
        );

        add_blueprint(&mut db, 2, &TableBlueprintArchetype::clear_fields());
        assert_eq!(TableBlueprint::load(&db), TableBlueprint::default());
    }
}
