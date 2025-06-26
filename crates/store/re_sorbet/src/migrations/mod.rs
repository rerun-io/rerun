#![allow(non_snake_case)]

//! These are the migrations that are introduced for each Sorbet version.

use arrow::array::RecordBatch;
use semver::Version;

mod make_list_arrays;

// We introduce artificial versions here for consistency. `v0_0_1` corresponds to
// Rerun versions pre-`v0.23` and `v0_0_2` corresponds to Rerun version
// `v0.23`. Starting with Rerun `v0.24`, we will track the Sorbet version
// separately, starting at `v0.1.0`.

mod v0_0_1__to__v0_0_2;
mod v0_0_2__to__v0_1_0;

// TODO(grtlr): Eventually, we should have a trait the abstracts over
// migrations so that they will be easier to manage. But let's follow
// the rule of three here.

fn needs_migration_to_version(batch: &RecordBatch, target_semver: &'static str) -> bool {
    // Expect is fine here, because `target_semver` is a static string.
    let target_version = Version::parse(target_semver).expect("has to be a valid semver");

    let Some(version_found) = batch.schema_ref().metadata().get("sorbet:version") else {
        re_log::debug!("Encountered batch without 'sorbet:version' metadata.");
        // We still do our best effort and try to perform the migration.
        return true;
    };

    match Version::parse(version_found) {
        Ok(version_found) => version_found < target_version,
        Err(err) => {
            re_log::error!("Could not parse 'sorbet:version': {err}");
            false
        }
    }
}

/// Migrate a sorbet record batch of unknown version to the latest version.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_record_batch(mut batch: RecordBatch) -> RecordBatch {
    use self::make_list_arrays::make_all_data_columns_list_arrays;

    re_tracing::profile_function!();

    if v0_0_1__to__v0_0_2::matches_schema(&batch) {
        // Corresponds to migrations from pre-`v0.23` to `v0.23`:
        batch = v0_0_1__to__v0_0_2::reorder_columns(&batch);
        batch = v0_0_1__to__v0_0_2::migrate_tuids(&batch);
        batch = v0_0_1__to__v0_0_2::migrate_record_batch(&batch);
    }

    if needs_migration_to_version(&batch, "0.1.0") {
        // Corresponds to migrations from `v0.23` to `v0.24`:
        batch = v0_0_2__to__v0_1_0::rewire_tagged_components(&batch);
    }

    batch = make_all_data_columns_list_arrays(&batch, v0_0_2__to__v0_1_0::is_component_column);

    batch
}
