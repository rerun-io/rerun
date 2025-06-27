#![expect(non_snake_case)]

//! These are the migrations that are introduced for each Sorbet version.

use arrow::array::RecordBatch;

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

    /// Migrates a record batch from one Sorbet version to the next.
    fn migrate(batch: RecordBatch) -> RecordBatch;
}

/// The Sorbet version that corresponds to this record batch.
fn get_or_guess_version(batch: &RecordBatch) -> semver::Version {
    if let Some(version_found) = batch
        .schema_ref()
        .metadata()
        .get(SorbetSchema::METADATA_KEY_VERSION)
    {
        // This is the happy path going forward.
        match semver::Version::parse(version_found) {
            Ok(version_found) => version_found,
            Err(err) => {
                re_log::error_once!("Could not parse 'sorbet:version: {version_found}': {err}");
                // We return an unreasonable large version number here to make
                // sure none of the migration code messes with the record batch.
                // This might be important for downstream handling (such as
                // printing).
                semver::Version::new(u64::MAX, u64::MAX, u64::MAX)
            }
        }
    } else {
        // This means earlier than Rerun `v0.24`.
        re_log::debug_once!("Encountered batch without 'sorbet:version' metadata.");

        if batch
            .schema()
            .metadata()
            .keys()
            .any(|key| key.starts_with("rerun."))
        {
            // This means Rerun `v0.23` or earlier.
            semver::Version::new(0, 0, 1)
        } else if batch
            .schema()
            .metadata()
            .keys()
            .any(|key| key.starts_with("rerun:"))
        {
            // This means from `main` between `v0.23` and `v0.24`. The
            // migration code from `v0.0.2` to `v0.1.0` should be able handle
            // this.
            semver::Version::new(0, 0, 2)
        } else {
            // This must be very old (or unexpected). Again we return a large
            // value from the future to prevent any migrations to run.
            // If we ever want to support even older versions, this would be the place.
            return semver::Version::new(u64::MAX, u64::MAX, u64::MAX);
        }
    }
}

fn maybe_apply<M: Migration>(
    source_version: &semver::Version,
    mut batch: RecordBatch,
) -> RecordBatch {
    if source_version < &M::TARGET_VERSION {
        re_log::debug_once!(
            "Migrating record batch from Sorbet 'v{source_version}' to 'v{}'.",
            M::TARGET_VERSION
        );
        batch = M::migrate(batch);
        batch
            .schema_metadata_mut()
            .insert("sorbet:version".to_owned(), M::TARGET_VERSION.to_string());
        batch
    } else {
        batch
    }
}

/// Migrate a sorbet record batch of unknown version to the latest version.
#[tracing::instrument(level = "trace", skip_all)]
pub fn migrate_record_batch(mut batch: RecordBatch) -> RecordBatch {
    use self::make_list_arrays::make_all_data_columns_list_arrays;

    re_tracing::profile_function!();

    let source_version = get_or_guess_version(&batch);

    // Perform migrations if necessary.
    batch = maybe_apply::<v0_0_1__to__v0_0_2::Migration>(&source_version, batch);
    batch = maybe_apply::<v0_0_2__to__v0_1_0::Migration>(&source_version, batch);

    batch = make_all_data_columns_list_arrays(&batch);

    batch
}
