use std::sync::Arc;

use arrow::{
    array::{RecordBatch, RecordBatchOptions},
    datatypes::{Field, Fields, Schema},
};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 1);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 2);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        migrate_transform3d_axis_length(batch)
    }
}

#[tracing::instrument(level = "trace", skip_all)]
fn migrate_transform3d_axis_length(batch: RecordBatch) -> RecordBatch {
    let (schema, columns, row_count) = batch.into_parts();

    let new_fields = schema.fields().iter().map(|field| {
        if let Some(val) = field
            .metadata()
            .get(re_types_core::FIELD_METADATA_KEY_COMPONENT)
            && val == "Transform3D:axis_length"
        {
            let mut new_metadata = field.metadata().clone();
            new_metadata.insert(
                re_types_core::FIELD_METADATA_KEY_ARCHETYPE.into(),
                "rerun.archetypes.TransformAxes3D".into(),
            );
            new_metadata.insert(
                re_types_core::FIELD_METADATA_KEY_COMPONENT.into(),
                "TransformAxes3D:axis_length".into(),
            );
            Field::new_list_field(field.data_type().clone(), field.is_nullable())
                .with_metadata(new_metadata)
        } else {
            field.as_ref().clone()
        }
    });

    let new_schema =
        Schema::new_with_metadata(new_fields.collect::<Fields>(), schema.metadata().clone());

    RecordBatch::try_new_with_options(
        Arc::new(new_schema.clone()),
        columns,
        &RecordBatchOptions::default().with_row_count(Some(row_count)),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema.into()))
}
