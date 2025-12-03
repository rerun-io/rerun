use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field, Fields, Schema};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 1);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 2);

    fn migrate(mut batch: RecordBatch) -> RecordBatch {
        batch = migrate_pose_components(batch);
        batch = migrate_transform3d_axis_length(batch);
        migrate_coordinate_frame(batch)
    }
}

/// Migrate deprecated Pose* components to their regular equivalents.
///
/// The `InstancePoses3D` archetype now uses the regular transformation components
/// (Translation3D, RotationAxisAngle, etc.) instead of the redundant Pose* variants.
#[tracing::instrument(level = "trace", skip_all)]
fn migrate_pose_components(batch: RecordBatch) -> RecordBatch {
    let schema = batch.schema();

    // Check if any migration is needed
    let needs_migration = schema.fields().iter().any(|field| {
        field
            .metadata()
            .get(re_types_core::FIELD_METADATA_KEY_COMPONENT_TYPE)
            .is_some_and(|component| {
                component == "rerun.components.PoseTranslation3D"
                    || component == "rerun.components.PoseRotationAxisAngle"
                    || component == "rerun.components.PoseRotationQuat"
                    || component == "rerun.components.PoseScale3D"
                    || component == "rerun.components.PoseTransformMat3x3"
            })
    });

    if !needs_migration {
        return batch;
    }

    re_log::debug_once!("Migrating Pose* components to regular transformation components");

    // Map old component names to new ones
    fn migrate_component_name(component: &str) -> String {
        match component {
            "rerun.components.PoseTranslation3D" => "rerun.components.Translation3D".to_owned(),
            "rerun.components.PoseRotationAxisAngle" => {
                "rerun.components.RotationAxisAngle".to_owned()
            }
            "rerun.components.PoseRotationQuat" => "rerun.components.RotationQuat".to_owned(),
            "rerun.components.PoseScale3D" => "rerun.components.Scale3D".to_owned(),
            "rerun.components.PoseTransformMat3x3" => "rerun.components.TransformMat3x3".to_owned(),
            _ => component.to_owned(),
        }
    }

    // Create new schema with migrated component names
    let new_fields: Vec<Arc<Field>> = schema
        .fields()
        .iter()
        .map(|field| {
            let mut metadata = field.metadata().clone();
            let mut modified = false;

            // Migrate component metadata
            if let Some(component) = metadata.get(re_types_core::FIELD_METADATA_KEY_COMPONENT) {
                let new_component = migrate_component_name(component);
                if new_component != *component {
                    metadata.insert(
                        re_types_core::FIELD_METADATA_KEY_COMPONENT.to_owned(),
                        new_component,
                    );
                    modified = true;
                }
            }

            // Migrate component type metadata
            if let Some(component_type) =
                metadata.get(re_types_core::FIELD_METADATA_KEY_COMPONENT_TYPE)
            {
                let new_component_type = migrate_component_name(component_type);
                if new_component_type != *component_type {
                    metadata.insert(
                        re_types_core::FIELD_METADATA_KEY_COMPONENT_TYPE.to_owned(),
                        new_component_type,
                    );
                    modified = true;
                }
            }

            if modified {
                Arc::new(field.as_ref().clone().with_metadata(metadata))
            } else {
                field.clone()
            }
        })
        .collect();

    let new_schema = Arc::new(Schema::new_with_metadata(
        new_fields,
        schema.metadata().clone(),
    ));

    // Create new RecordBatch with updated schema
    RecordBatch::try_new_with_options(
        new_schema.clone(),
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
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

#[tracing::instrument(level = "trace", skip_all)]
fn migrate_coordinate_frame(batch: RecordBatch) -> RecordBatch {
    let (schema, columns, row_count) = batch.into_parts();

    let new_fields = schema.fields().iter().map(|field| {
        if let Some(val) = field
            .metadata()
            .get(re_types_core::FIELD_METADATA_KEY_COMPONENT)
            && val == "CoordinateFrame:frame_id"
        {
            let mut new_metadata = field.metadata().clone();
            new_metadata.insert(
                re_types_core::FIELD_METADATA_KEY_COMPONENT.into(),
                "CoordinateFrame:frame".into(),
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
