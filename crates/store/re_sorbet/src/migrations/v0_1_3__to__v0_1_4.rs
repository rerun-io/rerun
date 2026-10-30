use std::sync::Arc;

use arrow::array::{
    Array as _, ArrayRef, AsArray as _, ListArray, RecordBatch, RecordBatchOptions, StringArray,
    StructArray,
};
use arrow::datatypes::{DataType, Field, FieldRef, Fields, Schema, UInt8Type};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 3);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 4);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        migrate_table_cell_kind(batch)
    }

    fn migrate_rrd_manifest(batch: RecordBatch) -> RecordBatch {
        migrate_rrd_manifest_cell_kind(batch)
    }
}

const CELL_KIND_COMPONENT: &str = "TableColumn:cell_kind";
const COLUMN_TYPE_COMPONENT: &str = "TableColumn:column_type";

fn is_cell_kind_field(field: &Field) -> bool {
    field
        .metadata()
        .get("rerun:component_type")
        .is_some_and(|component_type| component_type == "rerun.blueprint.components.TableCellKind")
}

/// The component type for a `TableCellKind` value.
///
/// Returns `None` for `Auto`, `Preview` and unknown values.
fn component_type_for_cell_kind(cell_kind: u8) -> Option<&'static str> {
    match cell_kind {
        2 => Some("rerun.components.Url"),
        3 => Some("rerun.components.Thumbnail"),
        4 => Some("rerun.components.Flag"),
        6 => Some("rerun.components.EntryKind"),
        _ => None,
    }
}

/// Migrate the `TableColumn:cell_kind` blueprint component to `TableColumn:column_type`.
///
/// Each `TableCellKind` value becomes a `ColumnType` with the matching component type.
#[tracing::instrument(level = "trace", skip_all)]
fn migrate_table_cell_kind(batch: RecordBatch) -> RecordBatch {
    if !batch
        .schema()
        .fields()
        .iter()
        .any(|field| is_cell_kind_field(field))
    {
        return batch;
    }

    re_log::debug_once!("Migrating TableCellKind component to ColumnType");

    let (schema, columns, row_count) = batch.into_parts();

    let (new_fields, new_columns): (Vec<FieldRef>, Vec<ArrayRef>) =
        std::iter::zip(schema.fields().iter(), columns)
            .map(|(field, column)| {
                if is_cell_kind_field(field) {
                    if let Some(migrated) = migrate_cell_kind_column(field, &column) {
                        return migrated;
                    }
                    re_log::warn_once!(
                        "Unexpected datatype {} for {CELL_KIND_COMPONENT}, it won't be migrated",
                        column.data_type()
                    );
                }
                (field.clone(), column)
            })
            .unzip();

    let new_schema = Arc::new(Schema::new_with_metadata(
        new_fields,
        schema.metadata().clone(),
    ));

    RecordBatch::try_new_with_options(
        new_schema.clone(),
        new_columns,
        &RecordBatchOptions::default().with_row_count(Some(row_count)),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
}

/// Converts a `TableCellKind` column to a `ColumnType` column.
///
/// Returns `None` if the column is not a list of `u8`.
fn migrate_cell_kind_column(field: &Field, column: &ArrayRef) -> Option<(FieldRef, ArrayRef)> {
    let list = column.as_list_opt::<i32>()?;
    let cell_kinds = list.values().as_primitive_opt::<UInt8Type>()?;

    let component_types: StringArray = cell_kinds
        .iter()
        .map(|cell_kind| cell_kind.and_then(component_type_for_cell_kind))
        .collect();

    let struct_fields = Fields::from(vec![
        Field::new("component", DataType::Utf8, true),
        Field::new("component_type_name", DataType::Utf8, true),
    ]);
    let column_types = StructArray::try_new(
        struct_fields.clone(),
        vec![
            Arc::new(StringArray::new_null(component_types.len())),
            Arc::new(component_types),
        ],
        cell_kinds.nulls().cloned(),
    )
    .ok_or_log_error()?;

    let new_list = ListArray::try_new(
        Arc::new(Field::new_list_field(DataType::Struct(struct_fields), true)),
        list.offsets().clone(),
        Arc::new(column_types),
        list.nulls().cloned(),
    )
    .ok_or_log_error()?;

    let new_field = column_type_field(field, new_list.data_type().clone());
    Some((new_field, Arc::new(new_list)))
}

/// A copy of a `TableColumn:cell_kind` field, renamed to `TableColumn:column_type` and with the given datatype.
fn column_type_field(field: &Field, data_type: DataType) -> FieldRef {
    let mut metadata = field.metadata().clone();
    metadata.insert(
        "rerun:component".to_owned(),
        COLUMN_TYPE_COMPONENT.to_owned(),
    );
    metadata.insert(
        "rerun:component_type".to_owned(),
        "rerun.blueprint.components.ColumnType".to_owned(),
    );

    Arc::new(
        Field::new(
            field
                .name()
                .replace(CELL_KIND_COMPONENT, COLUMN_TYPE_COMPONENT),
            data_type,
            field.is_nullable(),
        )
        .with_metadata(metadata),
    )
}

/// Rename the per-component index columns of `TableColumn:cell_kind` in an RRD manifest to `TableColumn:column_type`.
#[tracing::instrument(level = "trace", skip_all)]
fn migrate_rrd_manifest_cell_kind(batch: RecordBatch) -> RecordBatch {
    if !batch
        .schema()
        .fields()
        .iter()
        .any(|field| is_cell_kind_field(field))
    {
        return batch;
    }

    re_log::debug_once!("Migrating TableCellKind index columns of an RRD manifest to ColumnType");

    let (schema, columns, row_count) = batch.into_parts();

    let new_fields: Vec<FieldRef> = schema
        .fields()
        .iter()
        .map(|field| {
            if is_cell_kind_field(field) {
                column_type_field(field, field.data_type().clone())
            } else {
                field.clone()
            }
        })
        .collect();

    let new_schema = Arc::new(Schema::new_with_metadata(
        new_fields,
        schema.metadata().clone(),
    ));

    RecordBatch::try_new_with_options(
        new_schema.clone(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(row_count)),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use arrow::array::{Int64Array, ListBuilder, UInt8Builder};

    use super::*;

    /// A blueprint chunk has a `TableColumn:cell_kind` column with every `TableCellKind` value and a null row.
    /// The migration turns it into a `TableColumn:column_type` column with the matching component types.
    /// Other columns stay unchanged.
    #[test]
    fn cell_kind_becomes_column_type() {
        let mut builder = ListBuilder::new(UInt8Builder::new());
        for cell_kind in 1..=6 {
            builder.values().append_value(cell_kind);
            builder.append(true);
        }
        builder.append_null();
        let cell_kinds = Arc::new(builder.finish()) as ArrayRef;

        let cell_kind_field = Field::new(CELL_KIND_COMPONENT, cell_kinds.data_type().clone(), true)
            .with_metadata(HashMap::from([
                (
                    "rerun:archetype".to_owned(),
                    "rerun.blueprint.archetypes.TableColumn".to_owned(),
                ),
                ("rerun:component".to_owned(), CELL_KIND_COMPONENT.to_owned()),
                (
                    "rerun:component_type".to_owned(),
                    "rerun.blueprint.components.TableCellKind".to_owned(),
                ),
            ]));
        let other_field = Field::new("other", DataType::Int64, false);
        let other = Arc::new(Int64Array::from_iter_values(0..7)) as ArrayRef;

        let batch = RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(
                vec![cell_kind_field, other_field.clone()],
                HashMap::default(),
            )),
            vec![cell_kinds, other.clone()],
            &RecordBatchOptions::default().with_row_count(Some(7)),
        )
        .unwrap();

        let migrated = migrate_table_cell_kind(batch);

        let schema = migrated.schema();
        let field = schema.field(0);
        assert_eq!(field.name(), COLUMN_TYPE_COMPONENT);
        assert_eq!(
            field.metadata().get("rerun:component").map(String::as_str),
            Some(COLUMN_TYPE_COMPONENT)
        );
        assert_eq!(
            field
                .metadata()
                .get("rerun:component_type")
                .map(String::as_str),
            Some("rerun.blueprint.components.ColumnType")
        );
        assert_eq!(
            field.metadata().get("rerun:archetype").map(String::as_str),
            Some("rerun.blueprint.archetypes.TableColumn")
        );
        assert_eq!(schema.field(1), &other_field);
        assert_eq!(migrated.column(1), &other);

        let list = migrated.column(0).as_list::<i32>();
        assert!(list.is_null(6));

        let column_types = list.values().as_struct();
        let components = column_types
            .column_by_name("component")
            .unwrap()
            .as_string::<i32>();
        let types = column_types
            .column_by_name("component_type_name")
            .unwrap()
            .as_string::<i32>();
        assert_eq!(components.null_count(), components.len());
        assert_eq!(
            types.iter().collect::<Vec<_>>(),
            vec![
                None,
                Some("rerun.components.Url"),
                Some("rerun.components.Thumbnail"),
                Some("rerun.components.Flag"),
                None,
                Some("rerun.components.EntryKind"),
            ]
        );
    }

    /// An RRD manifest has per-component index columns for `TableColumn:cell_kind` next to a global column.
    /// The migration renames them to `TableColumn:column_type` and keeps their data.
    #[test]
    fn rrd_manifest_cell_kind_index_columns_are_renamed() {
        let cell_kind_metadata = HashMap::from([
            ("rerun:index".to_owned(), "log_time".to_owned()),
            ("rerun:component".to_owned(), CELL_KIND_COMPONENT.to_owned()),
            (
                "rerun:component_type".to_owned(),
                "rerun.blueprint.components.TableCellKind".to_owned(),
            ),
        ]);
        let start_field = Field::new(
            format!("log_time:{CELL_KIND_COMPONENT}:start"),
            DataType::Int64,
            true,
        )
        .with_metadata(cell_kind_metadata.clone());
        let end_field = Field::new(
            format!("log_time:{CELL_KIND_COMPONENT}:end"),
            DataType::Int64,
            true,
        )
        .with_metadata(cell_kind_metadata);
        let global_field = Field::new("chunk_byte_size", DataType::Int64, false);

        let start = Arc::new(Int64Array::from(vec![1])) as ArrayRef;
        let end = Arc::new(Int64Array::from(vec![2])) as ArrayRef;
        let global = Arc::new(Int64Array::from(vec![3])) as ArrayRef;

        let batch = RecordBatch::try_new_with_options(
            Arc::new(Schema::new_with_metadata(
                vec![start_field, end_field, global_field.clone()],
                HashMap::default(),
            )),
            vec![start.clone(), end.clone(), global.clone()],
            &RecordBatchOptions::default().with_row_count(Some(1)),
        )
        .unwrap();

        let migrated = migrate_rrd_manifest_cell_kind(batch);

        let schema = migrated.schema();
        assert_eq!(
            schema.field(0).name(),
            &format!("log_time:{COLUMN_TYPE_COMPONENT}:start")
        );
        assert_eq!(
            schema.field(1).name(),
            &format!("log_time:{COLUMN_TYPE_COMPONENT}:end")
        );
        for field in [schema.field(0), schema.field(1)] {
            assert_eq!(
                field.metadata().get("rerun:component").map(String::as_str),
                Some(COLUMN_TYPE_COMPONENT)
            );
            assert_eq!(
                field
                    .metadata()
                    .get("rerun:component_type")
                    .map(String::as_str),
                Some("rerun.blueprint.components.ColumnType")
            );
            assert_eq!(
                field.metadata().get("rerun:index").map(String::as_str),
                Some("log_time")
            );
        }
        assert_eq!(schema.field(2), &global_field);
        assert_eq!(migrated.column(0), &start);
        assert_eq!(migrated.column(1), &end);
        assert_eq!(migrated.column(2), &global);
    }
}
