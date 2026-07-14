use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};
use arrow::datatypes::{Field, Schema};
use re_log::ResultExt as _;

pub struct Migration;

impl super::Migration for Migration {
    const SOURCE_VERSION: semver::Version = semver::Version::new(0, 1, 2);
    const TARGET_VERSION: semver::Version = semver::Version::new(0, 1, 3);

    fn migrate(batch: RecordBatch) -> RecordBatch {
        migrate_series_visible(batch)
    }
}

/// Migrate deprecated `SeriesVisible` component to `Visible`.
///
/// `SeriesVisible` was a temporary workaround that duplicated `Visible` for time series.
/// Both have identical Arrow structure (transparent wrapper around `Bool`), so only the
/// metadata needs to change.
#[tracing::instrument(level = "trace", skip_all)]
fn migrate_series_visible(batch: RecordBatch) -> RecordBatch {
    let schema = batch.schema();

    let needs_migration = schema.fields().iter().any(|field| {
        field
            .metadata()
            .get("rerun:component_type")
            .is_some_and(|component| component == "rerun.components.SeriesVisible")
    });

    if !needs_migration {
        return batch;
    }

    re_log::debug_once!("Migrating SeriesVisible component to Visible");

    let new_fields: Vec<Arc<Field>> = schema
        .fields()
        .iter()
        .map(|field| {
            let metadata = field.metadata();
            if let Some(component_type) = metadata.get("rerun:component_type")
                && component_type == "rerun.components.SeriesVisible"
            {
                let mut metadata = metadata.clone();
                metadata.insert(
                    "rerun:component_type".to_owned(),
                    "rerun.components.Visible".to_owned(),
                );
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

    RecordBatch::try_new_with_options(
        new_schema.clone(),
        batch.columns().to_vec(),
        &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
    )
    .ok_or_log_error()
    .unwrap_or_else(|| RecordBatch::new_empty(new_schema))
}
