#![allow(non_snake_case)]

//! These are the migrations that are introduced for each Sorbet version.

use std::sync::Arc;

use arrow::array::{RecordBatch, RecordBatchOptions};

use re_log::ResultExt as _;

use crate::SorbetSchema;

mod make_list_arrays;

// We introduce artificial versions here for consistency. `v0_0_1` corresponds to
// Rerun versions pre-`v0.23` and `v0_0_2` corresponds to Rerun version
// `v0.23`. Starting with Rerun `v0.24`, we will track the Sorbet version
// separately, starting at `v0.1.0`.

mod v0_0_1__to__v0_0_2;
mod v0_0_2__to__v0_1_0;

/// This trait needs to be implemented by any new migrations. It ensures that
/// all migrations adhere to the same contract.
trait Migration {
    const TARGET_VERSION: semver::Version;

    /// The Sorbet version that corresponds to this record batch.
    fn version(batch: &RecordBatch) -> semver::Version {
        let Some(version_found) = batch
            .schema_ref()
            .metadata()
            .get(SorbetSchema::METADATA_KEY_VERSION)
        else {
            re_log::debug_once!("Encountered batch without 'sorbet:version' metadata.");
            // We still do our best effort and try to perform the migration.
            return semver::Version::new(0, 0, 0);
        };

        match semver::Version::parse(version_found) {
            Ok(version_found) => version_found,
            Err(err) => {
                re_log::error_once!("Could not parse 'sorbet:version': {err}");
                // We still do our best effort and try to perform the migration.
                semver::Version::new(0, 0, 0)
            }
        }
    }

    /// Migrates a record batch from one Sorbet version to the next.
    fn migrate(batch: RecordBatch) -> RecordBatch;
}

fn maybe_apply<M: Migration>(mut batch: RecordBatch) -> RecordBatch {
    let version = M::version(&batch);
    if version < M::TARGET_VERSION {
        re_log::debug!("Migrating from `v{version}` to `v{}`", M::TARGET_VERSION);
        batch = M::migrate(batch);

        let mut metadata = batch.schema().metadata().clone();
        metadata.insert("sorbet:version".to_owned(), M::TARGET_VERSION.to_string());
        let schema = Arc::new(arrow::datatypes::Schema::new_with_metadata(
            batch.schema().fields.clone(),
            metadata,
        ));

        // TODO(grtlr): clean this up when we have mutable record batches in `arrow-rs`.
        RecordBatch::try_new_with_options(
            schema.clone(),
            batch.columns().to_vec(),
            &RecordBatchOptions::default().with_row_count(Some(batch.num_rows())),
        )
        .ok_or_log_error()
        .unwrap_or_else(|| RecordBatch::new_empty(schema))
    } else {
        batch
    }
}

/// Migrate a sorbet record batch of unknown version to the latest version.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_record_batch(mut batch: RecordBatch) -> RecordBatch {
    use self::make_list_arrays::make_all_data_columns_list_arrays;

    re_tracing::profile_function!();

    // Perform migrations if necesarry.
    batch = maybe_apply::<v0_0_1__to__v0_0_2::Migration>(batch);
    batch = maybe_apply::<v0_0_2__to__v0_1_0::Migration>(batch);

    batch = make_all_data_columns_list_arrays(&batch);

    batch
}
