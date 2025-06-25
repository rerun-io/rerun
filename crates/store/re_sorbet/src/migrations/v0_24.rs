use std::{collections::HashMap, sync::Arc};

use arrow::{
    array::{RecordBatch as ArrowRecordBatch, RecordBatchOptions as ArrowRecordBatchOptions},
    datatypes::{FieldRef as ArrowFieldRef, Fields, Schema as ArrowSchema},
};
use re_log::ResultExt as _;

// We might have to move the definitions here, if we ever change the metadata key again.
use crate::ColumnKind;

fn trim_archetype_prefix(name: &str) -> &str {
    name.trim()
        .trim_start_matches("rerun.archetypes.")
        .trim_start_matches("rerun.blueprint.archetypes.")
}

pub fn is_component_column(field: &&ArrowFieldRef) -> bool {
    ColumnKind::try_from(field.as_ref()).is_ok_and(|kind| kind == ColumnKind::Component)
}

/// Ensures that incoming data is properly tagged and rewires to our now component descriptor format.
#[tracing::instrument(level = "trace", skip_all)]
pub fn rewire_tagged_components(batch: &ArrowRecordBatch) -> ArrowRecordBatch {
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
            rename_key(&mut metadata, "rerun.index_name", "rerun:index_name");
            rename_key(&mut metadata, "rerun.entity_path", "rerun:entity_path");
            rename_key(&mut metadata, "rerun.kind", "rerun:kind");
            rename_key(&mut metadata, "rerun.is_static", "rerun:is_static");
            rename_key(&mut metadata, "rerun.is_indicator", "rerun:is_indicator");
            rename_key(&mut metadata, "rerun.is_tombstone", "rerun:is_tombstone");
            rename_key(&mut metadata, "rerun.is_semantically_empty", "rerun:is_semantically_empty");
            rename_key(&mut metadata, "rerun.is_sorted", "rerun:is_sorted");

            // If component is present, we are encountering a legacy component descriptor.
            if let Some(component) = metadata.remove("rerun.component") {
                let (archetype, component, component_type) = match (
                    metadata.remove("rerun.archetype"),
                    metadata.remove("rerun.archetype_field"),
                ) {
                    (None, None) => {
                        // We likely encountered data that was logged via `AnyValues` and do our best effort to convert it.
                        re_log::debug!(
                            "Moving stray component type to component field: {component}"
                        );
                        (None, component, None)
                    }
                    (None, Some(archetype_field)) => (None, archetype_field, Some(component)),
                    (maybe_archetype, None) if component.ends_with("Indicator") => {
                        // TODO(#8129): For now, this renames the indicator column metadata. Eventually, we want to remove the column altogether.
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

            if field.name().ends_with("Indicator") {
                // TODO(#8129): Remove indicator components
                if let Some(archetype) = metadata.remove("rerun.archetype") {
                    metadata.insert("rerun:component".to_owned(), format!("{archetype}Indicator"));
                }
                metadata.remove("rerun.archetype_field");
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
        .map(|(key, value)| {
            if key.starts_with("rerun.") {
                re_log::debug_once!("Migrating batch metadata key {key}");
            }
            (key.replace("rerun.", "rerun:"), value.clone())
        })
        .collect();

    let schema = Arc::new(ArrowSchema::new_with_metadata(fields, metadata));

    ArrowRecordBatch::try_new_with_options(
        schema.clone(),
        batch.columns().to_vec(),
        &ArrowRecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| ArrowRecordBatch::new_empty(schema))
}
