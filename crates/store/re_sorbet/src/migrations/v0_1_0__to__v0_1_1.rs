use std::sync::Arc;

use arrow::array::{Array, RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field, Schema};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 0);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 1);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        drop_indicators(batch)
    }
}

/// Drop indicator columns and `is_indicator` metadata.
#[tracing::instrument(level = "trace", skip_all)]
fn drop_indicators(batch: RecordBatch) -> RecordBatch {
    let schema = batch.schema();

    // Find indices of columns to keep (those without is_indicator metadata)
    let keep_indices: Vec<usize> = schema
        .fields()
        .iter()
        .enumerate()
        .filter_map(|(i, field)| {
            if let Some(val) = field.metadata().get("rerun:is_indicator") {
                if val == "true" {
                    re_log::debug_once!(
                        "Dropping column '{}' because 'rerun:is_indicator' is '{val}'.",
                        field.name()
                    );
                    None // Drop
                } else {
                    re_log::debug_once!(
                        "Keeping column '{}' where 'rerun:is_indicator' is '{val}'.",
                        field.name()
                    );
                    Some(i) // Keep
                }
            } else if field.metadata().get("rerun:component").is_some_and(|val| {
                val.starts_with("rerun.components.") && val.ends_with("Indicator")
            }) {
                let Some(indicator) = field.metadata().get("rerun:component") else {
                    debug_assert!(
                        false,
                        "missing 'rerun:component' entry that should be present"
                    );
                    return Some(i);
                };
                re_log::debug_once!("Dropping column because 'rerun:component' is '{indicator}'",);
                None // Drop
            } else {
                Some(i) // Keep
            }
        })
        .collect();

    // Early return if no columns need to be dropped
    if keep_indices.len() == schema.fields().len() {
        return batch;
    }

    // Create new schema with filtered fields
    let new_fields: Vec<Arc<Field>> = keep_indices
        .iter()
        .map(|&i| schema.field(i).clone().into())
        .collect();

    let new_schema = Arc::new(Schema::new_with_metadata(
        new_fields,
        schema.metadata().clone(),
    ));

    // Filter columns to match new schema
    let new_columns: Vec<Arc<dyn Array>> = keep_indices
        .iter()
        .map(|&i| batch.column(i).clone())
        .collect();

    // Create new RecordBatch using provided performance optimization
    RecordBatch::try_new_with_options(
        new_schema.clone(),
        new_columns,
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
}
