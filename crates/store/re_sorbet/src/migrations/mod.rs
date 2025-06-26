//! These are the migrations that were introduced for each version.
//!
//! For example [`v0_24`] contains the migrations that are needed
//! to go from [`v0_23`] to [`v0_24`].

use arrow::array::RecordBatch;

mod make_list_arrays;
mod v0_23;
mod v0_24;

// TODO(grtlr): Eventually, we should have a trait the abstracts over
// migrations so that they will be easier to manage. But let's follow
// the rule of three here.

/// Migrate a sorbet record batch of unknown version to the latest version.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_record_batch(mut batch: RecordBatch) -> RecordBatch {
    use self::make_list_arrays::make_all_data_columns_list_arrays;

    re_tracing::profile_function!();

    if v0_23::matches_schema(&batch) {
        // Migrations from pre-`v0.23` to `v0.23`:
        batch = v0_23::reorder_columns(&batch);
        batch = v0_23::migrate_tuids(&batch);
        batch = v0_23::migrate_record_batch(&batch);
    }

    if true {
        // TODO(#10322): only do this if needed
        // Migrations from `v0.23` to `v0.24`:
        batch = v0_24::rewire_tagged_components(&batch);
    }

    batch = make_all_data_columns_list_arrays(&batch, v0_24::is_component_column);

    batch
}
