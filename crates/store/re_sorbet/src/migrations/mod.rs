#![expect(non_snake_case)]

//! These are the migrations that are introduced for each Sorbet version.

use std::cmp::Ordering;

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
    /// The Sorbet version that the record batch should currently have.
    const SOURCE_VERSION: semver::Version;

    /// The Sorbet version for the result of the migration.
    const TARGET_VERSION: semver::Version;

    /// Migrates a record batch from one Sorbet version to the next.
    fn migrate(batch: RecordBatch) -> RecordBatch;
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("could not parse 'sorbet:version: {value}': {err}")]
    InvalidSemVer { value: String, err: semver::Error },

    #[error("could not determine Sorbet version")]
    MissingVersion,
}

/// The Sorbet version that corresponds to this record batch.
fn get_or_guess_version(batch: &RecordBatch) -> Result<semver::Version, Error> {
    if let Some(version_found) = batch
        .schema_ref()
        .metadata()
        .get(SorbetSchema::METADATA_KEY_VERSION)
    {
        // This is the happy path going forward.
        semver::Version::parse(version_found).map_err(|err| Error::InvalidSemVer {
            value: version_found.to_owned(),
            err,
        })
    } else {
        // This means earlier than Rerun `v0.24`.
        re_log::debug_once!("Encountered batch without 'sorbet:version' metadata.");

        if batch
            .schema()
            .metadata()
            .keys()
            .any(|key| key.starts_with("rerun."))
        {
            re_log::debug_once!(
                "Found 'rerun.' prefixed metadata. This means Rerun `v0.23` or earlier."
            );
            Ok(semver::Version::new(0, 0, 1))
        } else if batch.schema().metadata().get("rerun:version").is_some() {
            re_log::debug_once!(
                "Found 'rerun:' prefixed metadata. This means 'nightly' between 'v0.23' and 'v0.24'."
            );
            // The migration code from `v0.0.2` to `v0.1.0` should be able handle this.
            Ok(semver::Version::new(0, 0, 2))
        } else {
            Err(Error::MissingVersion)
        }
    }
}

fn maybe_apply<M: Migration>(
    batch_version: &semver::Version,
    mut batch: RecordBatch,
) -> RecordBatch {
    if batch_version < &M::TARGET_VERSION {
        re_log::debug_once!(
            "Migrating record batch from Sorbet 'v{batch_version}' to 'v{}'.",
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

    batch = match get_or_guess_version(&batch) {
        Ok(batch_version) => match batch_version.cmp(&SorbetSchema::METADATA_VERSION) {
            Ordering::Equal => {
                // Provide this code path as an early out to avoid unnecessary comparisons.
                re_log::trace!("Batch version matches Sorbet version.");
                batch
            }
            Ordering::Less => {
                let first_supported = v0_0_1__to__v0_0_2::Migration::SOURCE_VERSION;
                if batch_version < first_supported {
                    re_log::warn_once!(
                        "Sorbet version 'v{batch_version}' is to old. Only versions '>={first_supported}' are supported."
                    );
                    batch
                } else {
                    re_log::trace!("Performing migrations...");
                    batch = maybe_apply::<v0_0_1__to__v0_0_2::Migration>(&batch_version, batch);
                    batch = maybe_apply::<v0_0_2__to__v0_1_0::Migration>(&batch_version, batch);
                    batch
                }
            }
            Ordering::Greater => {
                re_log::warn_once!(
                    "Found Sorbet version 'v{batch_version}' that is newer then current supported version 'v{}'. Consider updating Rerun!",
                    SorbetSchema::METADATA_VERSION
                );
                batch
            }
        },
        Err(Error::MissingVersion) => {
            // TODO(#10421): We need to handle arbitrary record batches and
            // we don't want to spam the viewer with useless warnings.
            re_log::debug_once!(
                "Encountered record batch without 'sorbet:version' metadata. Data will not be migrated."
            );
            batch
        }
        Err(err) => {
            re_log::error_once!("Skipping migrations due to error: {err}");
            batch
        }
    };

    batch = make_all_data_columns_list_arrays(&batch);

    batch
}
