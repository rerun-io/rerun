//! Breaking changes:
//! * `Blob` is encoded as `Binary` instead of `List[u8]`
use std::sync::Arc;

use arrow::{
    array::{
        Array, ArrayRef, AsArray as _, BinaryArray, ListArray, RecordBatch, RecordBatchOptions,
        UInt8Array,
    },
    datatypes::{DataType, Field, FieldRef, Schema},
};

use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 1);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 2);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        migrate_blobs(batch)
    }
}

/// Change datatype from `List[u8]` to `Binary` for blobs
fn migrate_blobs(batch: RecordBatch) -> RecordBatch {
    re_tracing::profile_function!();

    fn is_blob_field(field: &Field) -> bool {
        let components_with_blobs = [
            "rerun.components.Blob",
            "rerun.components.ImageBuffer",
            "rerun.components.VideoSample",
        ];

        let Some(component_type) = field.metadata().get("rerun:component_type") else {
            return false;
        };

        if !components_with_blobs.contains(&component_type.as_str()) {
            return false;
        }

        let DataType::List(list_field) = field.data_type() else {
            return false;
        };

        let DataType::List(innermost_field) = list_field.data_type() else {
            return false;
        };

        innermost_field.data_type() == &DataType::UInt8
    }

    let needs_migration = batch
        .schema()
        .fields()
        .iter()
        .any(|field| is_blob_field(field));

    if !needs_migration {
        return batch;
    }

    let num_columns = batch.num_columns();
    let mut fields: Vec<FieldRef> = Vec::with_capacity(num_columns);
    let mut columns: Vec<ArrayRef> = Vec::with_capacity(num_columns);

    for (field, array) in itertools::izip!(batch.schema().fields(), batch.columns()) {
        if is_blob_field(field) {
            if let Some(new_array) = convert_list_list_u8_to_list_binary(array.as_ref()) {
                let new_field = Field::new(
                    field.name(),
                    new_array.data_type().clone(),
                    field.is_nullable(),
                )
                .with_metadata(field.metadata().clone());

                fields.push(new_field.into());
                columns.push(Arc::new(new_array));

                re_log::debug_once!("Migrated {} from List[u8] to Binary", field.name());
                continue;
            } else {
                re_log::warn_once!("Failed to convert {} to Binary", field.name());
            }
        }

        fields.push(field.clone());
        columns.push(array.clone());
    }

    let schema = Arc::new(Schema::new_with_metadata(
        fields,
        batch.schema().metadata.clone(),
    ));

    RecordBatch::try_new_with_options(
        schema.clone(),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(schema))
}

/// `List[List[u8]]` -> `List[Binary]`
fn convert_list_list_u8_to_list_binary(list_array: &dyn Array) -> Option<ListArray> {
    // The outer `List[List[u8]]`
    let list_array = list_array.as_list_opt()?;

    // The inner List[u8] array
    let inner_list_array = list_array.values().as_list_opt()?;

    // The underlying u8 values
    let u8_array: &UInt8Array = inner_list_array.values().as_primitive_opt()?;

    // Create the binary array reusing existing buffers
    let binary_array = BinaryArray::try_new(
        inner_list_array.offsets().clone(),
        u8_array.values().clone().into_inner(),
        inner_list_array.nulls().cloned(),
    )
    .ok()?;

    // Create the outer list array with binary inner type
    let outer_list = ListArray::try_new(
        Arc::new(Field::new(
            "item",
            DataType::Binary,
            list_array.is_nullable(),
        )),
        list_array.offsets().clone(),
        Arc::new(binary_array),
        list_array.nulls().cloned(),
    )
    .ok()?;

    debug_assert_eq!(list_array.len(), outer_list.len());

    Some(outer_list)
}
