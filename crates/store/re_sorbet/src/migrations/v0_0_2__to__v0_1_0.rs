use std::collections::HashMap;
use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Fields, Schema};
use re_log::ResultExt as _;

fn trim_archetype_prefix(name: &str) -> &str {
    name.trim()
        .trim_start_matches("rerun.archetypes.")
        .trim_start_matches("rerun.blueprint.archetypes.")
}

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 0, 2);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 0);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        let mut batch = rewire_tagged_components(&batch);
        port_recording_info(&mut batch);
        batch
    }
}

/// Ensures that incoming data is properly tagged and rewires to our now component descriptor format.
#[tracing::instrument(level = "trace", skip_all)]
fn rewire_tagged_components(batch: &RecordBatch) -> RecordBatch {
    re_tracing::profile_function!();

    let needs_rewiring = batch
        .schema()
        .metadata()
        .keys()
        .any(|key| key.starts_with("rerun."))
        || batch
            .schema()
            .fields()
            .iter()
            .any(|field| field.metadata().keys().any(|key| key.starts_with("rerun.")));

    if !needs_rewiring {
        return batch.clone();
    }

    let fields = batch
        .schema()
        .fields()
        .into_iter()
        .map(|field| {
            let mut field = field.as_ref().clone();
            let mut metadata = field.metadata().clone();

            // Not `colonize` - we are friendly folks at Rerun.
            fn rename_key(metadata: &mut HashMap<String, String>, legacy_key: &str, new_key: &str) {
                if let Some(value) = metadata.remove(legacy_key) {
                    metadata.insert(new_key.to_owned(), value);
                }
            }

            // These metadata fields don't require value changes.
            rename_key(&mut metadata, "rerun.index_name", crate::metadata::SORBET_INDEX_NAME);
            rename_key(&mut metadata, "rerun.entity_path", crate::metadata::SORBET_ENTITY_PATH);
            rename_key(&mut metadata, "rerun.kind", crate::metadata::RERUN_KIND);
            rename_key(&mut metadata, "rerun.is_static", "rerun:is_static");
            rename_key(&mut metadata, "rerun.is_indicator", "rerun:is_indicator");
            rename_key(&mut metadata, "rerun.is_tombstone", "rerun:is_tombstone");
            rename_key(&mut metadata, "rerun.is_semantically_empty", "rerun:is_semantically_empty");
            rename_key(&mut metadata, "rerun.is_sorted", "rerun:is_sorted");

            if field.name().ends_with("Indicator") {
                let field_name = field.name();
                re_log::debug_once!(
                    "Moving indicator from field to component metadata field: {field_name}"
                );

                // A lot of defensive code to handle different legacy formats of the indicator component,
                // including blueprint indicators:
                if let Some(component) = metadata.remove("rerun.component") {
                    debug_assert!(
                        component.ends_with("Indicator"),
                        "Expected component to end with 'Indicator', got: {component:?}"
                    );
                    metadata.insert("rerun:component".to_owned(), component);
                } else if field_name.starts_with("rerun.") {
                    // Long name
                    metadata.insert("rerun:component".to_owned(), field_name.clone());
                } else {
                    // Short name: expand it to be long
                    metadata.insert("rerun:component".to_owned(), format!("rerun.components.{field_name}"));
                }

                // Remove everything else.
                metadata.remove("rerun.archetype");
                metadata.remove("rerun.archetype_field");
                metadata.remove("rerun.component");
            } else if let Some(component) = metadata.remove("rerun.component") {
                // If component is present, we are encountering a legacy component descriptor.
                let (archetype, component, component_type) = match (
                    metadata.remove("rerun.archetype"),
                    metadata.remove("rerun.archetype_field"),
                ) {
                    (None, None) => {
                        // We likely encountered data that was logged via `AnyValues` and do our best effort to convert it.
                        re_log::debug_once!(
                            "Moving stray component type to component field: {component}"
                        );
                        (None, component, None)
                    }
                    (None, Some(archetype_field)) => (None, archetype_field, Some(component)),
                    (maybe_archetype, None) if component.ends_with("Indicator") => {
                        re_log::debug_once!(
                            "Moving indicator to component field: {component}"
                        );

                        // We also strip the archetype name from any indicators.
                        // It turns out that too narrow indicator descriptors cause problems while querying.
                        // More information: <https://github.com/rerun-io/rerun/pull/9938#issuecomment-2888808593>
                        if let Some(archetype) = maybe_archetype {
                            re_log::debug_once!(
                                "Stripped archetype name from indicator: {archetype}"
                            );
                        }
                        (None, component, None)
                    }
                    (Some(archetype), Some(archetype_field)) => {
                        let new_component =
                            format!("{}:{archetype_field}", trim_archetype_prefix(&archetype));
                        (Some(archetype), new_component, Some(component))
                    }
                    (Some(archetype), None) => {
                        re_log::debug!("Encountered archetype {archetype} without archetype field name, duplicating component {component} to archetype field");
                        (Some(archetype), component.clone(), Some(component))
                    },
                };

                if let Some(archetype) = archetype {
                    metadata.insert("rerun:archetype".to_owned(), archetype);
                }
                metadata.insert("rerun:component".to_owned(), component);
                if let Some(component_type) = component_type {
                    metadata.insert("rerun:component_type".to_owned(), component_type);
                }
            }

            for (key, value) in &metadata {
                debug_assert!(!key.starts_with("rerun."), "Metadata `{key}` (with value `{value}`) was not migrated to colon syntax.");
            }

            field.set_metadata(metadata);
            Arc::new(field)
        })
        .collect::<Fields>();

    let metadata = batch
        .schema()
        .metadata()
        .iter()
        .filter_map(|(key, value)| {
            if key.as_str() == "rerun.version" {
                // Note that the `Migration` trait takes care of setting the sorbet version.
                re_log::debug_once!("Dropping 'rerun.version' from metadata.");
                return None;
            }

            if key.starts_with("rerun.") {
                re_log::debug_once!("Migrating batch metadata key '{key}'");
            }
            Some((key.replace("rerun.", "rerun:"), value.clone()))
        })
        .collect();

    let schema = Arc::new(Schema::new_with_metadata(fields, metadata));

    RecordBatch::try_new_with_options(
        schema.clone(),
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(schema))
}

/// Look for old `RecordingProperties` at `/__properties/recording`
/// and rename it to `RecordingInfo` and move it to `/__properties`.
///
/// User properties are still on `/__properties/$FOO` with column name `property:$FOO:…` - no change there.
fn port_recording_info(batch: &mut RecordBatch) {
    re_tracing::profile_function!();

    // We renamed `RecordingProperties` to `RecordingInfo`,
    // and moved it from `/__properties/recording` to `/__properties`.
    if let Some(entity_path) = batch
        .schema_metadata_mut()
        .get_mut(crate::metadata::SORBET_ENTITY_PATH)
        && entity_path == "/__properties/recording"
    {
        *entity_path = "/__properties".to_owned();
    }

    fn migrate_column_name(name: &str) -> String {
        name.replace("RecordingProperties", "RecordingInfo")
            .replace(
                "property:recording:RecordingInfo:",
                "property:RecordingInfo:",
            )
    }

    let modified_fields: arrow::datatypes::Fields = batch
        .schema()
        .fields()
        .iter()
        .map(|field| {
            // Migrate field name:
            let mut field = arrow::datatypes::Field::new(
                migrate_column_name(field.name()),
                field.data_type().clone(),
                field.is_nullable(),
            )
            .with_metadata(field.metadata().clone());

            // Migrate per-column entity paths (if any):
            if let Some(entity_path) = field
                .metadata_mut()
                .get_mut(crate::metadata::SORBET_ENTITY_PATH)
                && entity_path == "/__properties/recording"
            {
                *entity_path = "/__properties".to_owned();
            }

            // Rename `RecordingProperties` to `RecordingInfo` in metadata keys:
            for value in field.metadata_mut().values_mut() {
                *value = value.replace("RecordingProperties", "RecordingInfo");
            }

            Arc::new(field)
        })
        .collect();

    *batch = RecordBatch::try_new_with_options(
        Arc::new(Schema::new_with_metadata(
            modified_fields,
            batch.schema().metadata().clone(),
        )),
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .expect("Can't fail - we've only modified metadata");
}
