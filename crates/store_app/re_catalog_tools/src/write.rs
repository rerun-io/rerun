//! The tools that change a catalog: create and rename entries, and register recordings.
//!
//! None of them removes or replaces data, since a catalog has no way to undo that.

use std::time::Duration;

use serde_json::{Value, json};

use re_catalog_client::{BlueprintKind, BlueprintName, CatalogClient, RegistrationError};
use re_log_types::EntryName;
use re_protos::cloud::v1alpha1::ext::{DataSource, EntryDetails, EntryDetailsUpdate};
use re_protos::common::v1alpha1::ext::IfDuplicateBehavior;
use re_redap_client::{ApiError, ApiErrorKind};
use re_types_core::{LayerName, SegmentId};

use crate::CatalogToolError;
use crate::args::{
    CreateDatasetArgs, OnDuplicateArg, RegisterRecordingsArgs, RegisterTableBlueprintArgs,
    RenameEntryArgs,
};
use crate::metadata::{dataset_id, find_entry};

/// How long a write waits for the server to register recordings.
///
/// The same budget as a query: within the tool-call timeouts of common MCP clients. Work that
/// takes longer keeps running on the server, and the result says so.
const REGISTRATION_WAIT: Duration = Duration::from_secs(120);

/// The most URLs one call takes, so one call cannot start an unbounded amount of work on the
/// server.
const MAX_URLS_PER_CALL: usize = 1000;

pub async fn create_dataset(
    catalog: &CatalogClient,
    args: CreateDatasetArgs,
) -> Result<Value, CatalogToolError> {
    let CreateDatasetArgs { name } = args;
    let entry_name = visible_entry_name(&name)?;
    let dataset = catalog
        .create_dataset(entry_name, None)
        .await
        .map_err(|err| name_taken_or(err, &name, "create the dataset"))?;
    Ok(json!({
        "created": dataset.details.name.to_string(),
        "id": dataset.details.id.to_string(),
    }))
}

pub async fn rename_entry(
    catalog: &CatalogClient,
    args: RenameEntryArgs,
) -> Result<Value, CatalogToolError> {
    let RenameEntryArgs { entry, new_name } = args;
    let update = EntryDetailsUpdate {
        name: Some(visible_entry_name(&new_name)?),
    };
    let details = writable_entry(catalog, &entry).await?;
    let renamed = catalog
        .update_entry(details.id, update)
        .await
        .map_err(|err| name_taken_or(err, &new_name, "rename the entry"))?;
    Ok(json!({
        "old_name": details.name.to_string(),
        "new_name": renamed.name.to_string(),
        "id": renamed.id.to_string(),
    }))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn upload_recording(
    catalog: &CatalogClient,
    args: crate::args::UploadRecordingArgs,
) -> Result<Value, CatalogToolError> {
    let crate::args::UploadRecordingArgs {
        dataset,
        path,
        layer,
        on_duplicate,
    } = args;
    check_rrd_file(&path)?;
    let layer = layer_name(layer)?;
    let dataset_id = dataset_id(catalog, &dataset).await?;
    let write = catalog
        .write_segment_path(
            dataset_id,
            path.clone(),
            None,
            layer.clone(),
            None,
            on_duplicate.into(),
        )
        .await
        .map_err(|source| upload_error(path, source))?;
    let segment_ids: Vec<SegmentId> = write.segment_ids().cloned().collect();
    let outcome = write.wait(REGISTRATION_WAIT).await;
    registration_result(&layer, on_duplicate, &segment_ids, outcome)
}

pub async fn register_recordings(
    catalog: &CatalogClient,
    args: RegisterRecordingsArgs,
) -> Result<Value, CatalogToolError> {
    let RegisterRecordingsArgs {
        dataset,
        urls,
        layer,
        on_duplicate,
    } = args;
    check_count("urls", urls.len())?;
    let layer = layer_name(layer)?;
    let data_sources = urls
        .iter()
        .map(|url| {
            DataSource::new_rrd_layer(layer.clone(), url, None).map_err(|source| {
                CatalogToolError::InvalidStorageUrl {
                    url: url.clone(),
                    source,
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let dataset_id = dataset_id(catalog, &dataset).await?;
    let registration = catalog
        .register_with_dataset(dataset_id, data_sources, on_duplicate.into())
        .await
        .map_err(|err| CatalogToolError::catalog("register the recordings", err))?;
    let segment_ids: Vec<SegmentId> = registration
        .descriptors()
        .iter()
        .map(|descriptor| descriptor.segment_id.clone())
        .collect();
    let outcome = registration.wait(REGISTRATION_WAIT).await;
    registration_result(&layer, on_duplicate, &segment_ids, outcome)
}

pub async fn register_table_blueprint(
    catalog: &CatalogClient,
    args: RegisterTableBlueprintArgs,
) -> Result<Value, CatalogToolError> {
    let RegisterTableBlueprintArgs {
        dataset,
        url,
        name,
        set_default,
    } = args;
    let blueprint_name = BlueprintName::try_new(name.clone())
        .map_err(|source| CatalogToolError::InvalidBlueprintName { source })?;
    let dataset_id = dataset_id(catalog, &dataset).await?;
    let blueprint_dataset = catalog
        .read_dataset(dataset_id)
        .await
        .map_err(|err| CatalogToolError::catalog("read the dataset", err))?
        .dataset_details
        .blueprint_dataset
        .ok_or_else(|| CatalogToolError::NoBlueprintDataset {
            dataset: dataset.clone(),
        })?;

    // Registering under a name in use would replace that blueprint's data.
    let taken = catalog
        .get_blueprint_by_name(blueprint_dataset, &blueprint_name)
        .await
        .map_err(|err| CatalogToolError::catalog("look up the blueprint name", err))?;
    if taken.is_some() {
        return Err(CatalogToolError::BlueprintNameTaken { name });
    }

    let blueprint = catalog
        .register_blueprint(
            blueprint_dataset,
            &url,
            Some(&blueprint_name),
            BlueprintKind::Table,
            None,
            REGISTRATION_WAIT,
        )
        .await
        .map_err(|err| CatalogToolError::catalog("register the blueprint", err))?;
    if set_default {
        catalog
            .set_dataset_default_blueprint(
                dataset_id,
                BlueprintKind::Table,
                Some(blueprint.segment_id.clone()),
            )
            .await
            .map_err(|err| {
                CatalogToolError::catalog("make the blueprint the segment table's default", err)
            })?;
    }
    Ok(json!({
        "blueprint": blueprint_name.as_str(),
        "segment_id": blueprint.segment_id.to_string(),
        "default": set_default,
    }))
}

/// The tool result for a registration of `segment_ids` as `layer`.
///
/// A registration still running when the wait ends is not a failure: the agent can check on it
/// with `catalog_list_segments`.
fn registration_result(
    layer: &LayerName,
    on_duplicate: OnDuplicateArg,
    segment_ids: &[SegmentId],
    outcome: Result<Vec<SegmentId>, RegistrationError>,
) -> Result<Value, CatalogToolError> {
    let names = |ids: &[SegmentId]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    match outcome {
        Ok(registered) => Ok(json!({
            "segment_ids": names(&registered),
            "layer": layer.as_str(),
            "on_duplicate": on_duplicate.name(),
        })),
        Err(RegistrationError::Query(err)) if err.kind == ApiErrorKind::Timeout => Ok(json!({
            "registering": names(segment_ids),
            "layer": layer.as_str(),
            "next_step": "The server is still registering them. Check with \
                          `catalog_list_segments` later.",
        })),
        Err(RegistrationError::Query(err)) => Err(CatalogToolError::RegistrationUnknown {
            source: Box::new(err),
        }),
        Err(RegistrationError::Failed(err)) => Err(CatalogToolError::RegistrationFailed {
            source: Box::new(err),
            suggest_skip: matches!(on_duplicate, OnDuplicateArg::Error),
        }),
    }
}

/// Refuse anything but an `.rrd` file at an absolute path, before anything is sent.
///
/// The header is checked as well as the name, so an agent cannot upload any file the user can
/// read by naming it `.rrd`.
#[cfg(not(target_arch = "wasm32"))]
fn check_rrd_file(path: &std::path::Path) -> Result<(), CatalogToolError> {
    use std::io::Read as _;

    let not_an_rrd = || CatalogToolError::NotAnRrdPath {
        path: path.to_owned(),
    };
    let has_rrd_extension = path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("rrd"));
    if !path.is_absolute() || !has_rrd_extension {
        return Err(not_an_rrd());
    }
    let mut fourcc = [0_u8; 4];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut fourcc))
        .map_err(|source| CatalogToolError::ReadFile {
            path: path.to_owned(),
            source,
        })?;
    if re_log_encoding::rrd::is_rrd_fourcc(fourcc) {
        Ok(())
    } else {
        Err(not_an_rrd())
    }
}

/// A failed upload of the file at `path`, saying what to do when the server takes no uploads.
#[cfg(not(target_arch = "wasm32"))]
fn upload_error(
    path: std::path::PathBuf,
    source: Box<re_catalog_client::WriteError>,
) -> CatalogToolError {
    if let re_catalog_client::WriteError::Upload(re_catalog_client::StagingError::Api(err)) =
        source.as_ref()
        && err.kind == ApiErrorKind::Unimplemented
    {
        CatalogToolError::UploadsUnsupported
    } else {
        CatalogToolError::Upload { path, source }
    }
}

/// The entry `entry` names, refusing Rerun's own hidden ones, e.g. `__entries` and the blueprint
/// and asset datasets.
async fn writable_entry(
    catalog: &CatalogClient,
    entry: &str,
) -> Result<EntryDetails, CatalogToolError> {
    let details =
        find_entry(catalog, entry, &[])
            .await?
            .ok_or_else(|| CatalogToolError::NoSuchEntry {
                entry: entry.to_owned(),
            })?;
    if details.name.is_hidden() {
        return Err(CatalogToolError::HiddenEntry {
            name: details.name.to_string(),
        });
    }
    Ok(details)
}

/// `name` as an entry name, refusing the names of Rerun's own hidden entries.
fn visible_entry_name(name: &str) -> Result<EntryName, CatalogToolError> {
    let entry_name = EntryName::new(name).map_err(|source| CatalogToolError::InvalidEntryName {
        name: name.to_owned(),
        source,
    })?;
    if entry_name.is_hidden() {
        return Err(CatalogToolError::HiddenEntry {
            name: name.to_owned(),
        });
    }
    Ok(entry_name)
}

fn name_taken_or(err: ApiError, name: &str, operation: &'static str) -> CatalogToolError {
    if err.kind == ApiErrorKind::AlreadyExists {
        CatalogToolError::EntryNameTaken {
            name: name.to_owned(),
        }
    } else {
        CatalogToolError::catalog(operation, err)
    }
}

fn layer_name(layer: Option<String>) -> Result<LayerName, CatalogToolError> {
    match layer {
        None => Ok(LayerName::base()),
        Some(layer) => LayerName::try_new(layer.clone())
            .map_err(|source| CatalogToolError::InvalidLayer { layer, source }),
    }
}

fn check_count(field: &'static str, count: usize) -> Result<(), CatalogToolError> {
    if count == 0 {
        Err(CatalogToolError::EmptyList { field })
    } else if MAX_URLS_PER_CALL < count {
        Err(CatalogToolError::TooManyItems {
            field,
            count,
            max: MAX_URLS_PER_CALL,
        })
    } else {
        Ok(())
    }
}

impl OnDuplicateArg {
    fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Skip => "skip",
        }
    }
}

impl From<OnDuplicateArg> for IfDuplicateBehavior {
    fn from(on_duplicate: OnDuplicateArg) -> Self {
        match on_duplicate {
            OnDuplicateArg::Error => Self::Error,
            OnDuplicateArg::Skip => Self::Skip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> re_uri::Origin {
        re_uri::Origin::http_local_host(1)
    }

    #[test]
    fn a_registration_still_running_is_not_a_failure() {
        let timeout = ApiError::tonic(
            &origin(),
            tonic::Status::deadline_exceeded("deadline"),
            "/QueryTasksOnCompletion",
        );
        assert_eq!(timeout.kind, ApiErrorKind::Timeout);
        let result = registration_result(
            &LayerName::base(),
            OnDuplicateArg::Error,
            &[SegmentId::from("a")],
            Err(RegistrationError::Query(timeout)),
        )
        .expect("still registering");
        assert_eq!(result["registering"], json!(["a"]));
    }

    #[test]
    fn a_lost_connection_while_waiting_is_an_unknown_outcome() {
        let lost = ApiError::connection(&origin(), "connection reset");
        let result = registration_result(
            &LayerName::base(),
            OnDuplicateArg::Error,
            &[SegmentId::from("a")],
            Err(RegistrationError::Query(lost)),
        );
        assert!(
            matches!(result, Err(CatalogToolError::RegistrationUnknown { .. })),
            "{result:?}"
        );
    }

    #[test]
    fn counts_are_bounded() {
        assert!(check_count("urls", 0).is_err());
        assert!(check_count("urls", MAX_URLS_PER_CALL).is_ok());
        assert!(check_count("urls", MAX_URLS_PER_CALL + 1).is_err());
    }
}
