use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use re_log_types::{EntryId, StoreId, StoreKind};
use re_protos::cloud::v1alpha1::ext;
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_types_core::LayerName;
use url::Url;

#[cfg(not(target_arch = "wasm32"))]
use tokio_util::compat::TokioAsyncReadCompatExt as _;

#[cfg(target_arch = "wasm32")]
use crate::opfs as fs;
use crate::store::{
    Error, InMemoryStore, LayerInfo, ResolvedStore, StoreSlotId, TASK_ID_SUCCESS, TaskResult,
};
#[cfg(not(target_arch = "wasm32"))]
use tokio::fs;

/// Return type of [`do_register_with_dataset`].
#[derive(Default)]
pub struct RegisterWithDatasetResult {
    /// Recording IDs from the registered RRDs, one per data source.
    ///
    /// Empty string for sources that failed with a schema conflict.
    pub segment_ids: Vec<SegmentId>,

    /// Layer name for each registered source.
    pub segment_layers: Vec<LayerName>,

    /// File format of each source (e.g. `"rrd"`).
    pub segment_types: Vec<ext::DataSourceKind>,

    /// Storage URL for each source.
    pub storage_urls: Vec<Url>,

    /// Task ID for each source; [`crate::store::TASK_ID_SUCCESS`] for successes,
    /// a unique ID for schema-conflict failures.
    pub task_ids: Vec<TaskId>,
}

/// A data source that has been validated (paths confirmed to exist, duplicates checked)
/// but not yet loaded into memory.
enum ValidatedSource {
    File {
        rrd_path: PathBuf,
        layer_info: Arc<LayerInfo>,
        storage_url: url::Url,
    },
    Memory {
        store_slot_id: StoreSlotId,
        resolved: ResolvedStore,
        segment_id: SegmentId,
        layer_info: Arc<LayerInfo>,
    },
}

/// A data source that has been fully loaded and is ready to be added to the dataset.
struct ReadySource {
    store_slot_id: StoreSlotId,
    resolved: ResolvedStore,
    segment_id: SegmentId,
    layer_info: Arc<LayerInfo>,
    storage_url: Url,
}

// ---

pub async fn do_register_with_dataset(
    store: &mut InMemoryStore,
    dataset_id: EntryId,
    data_sources: Vec<ext::DataSource>,
    on_duplicate: IfDuplicateBehavior,
) -> tonic::Result<RegisterWithDatasetResult> {
    let (store_kind, validated) = validate_sources(store, dataset_id, data_sources).await?;
    let ready = load_sources(validated, store_kind).await?;
    register_sources(store, dataset_id, ready, on_duplicate).await
}

// ---

/// Phase 1: validate each data source, resolve memory URLs, and check for
/// intra-request duplicates.
///
/// Returns the dataset's [`StoreKind`] alongside the validated sources, since
/// callers need it to filter stores when loading files.
async fn validate_sources(
    store: &InMemoryStore,
    dataset_id: EntryId,
    data_sources: Vec<ext::DataSource>,
) -> tonic::Result<(StoreKind, Vec<ValidatedSource>)> {
    // `seen` tracks (layer_name, segment_id) → URLs to detect intra-request dups.
    // The `on_duplicate` flag only applies to cross-request conflicts.
    let mut seen: BTreeMap<(LayerName, SegmentId), Vec<url::Url>> = BTreeMap::new();
    let mut validated: Vec<ValidatedSource> = Vec::new();

    let store_kind = store.dataset(dataset_id)?.store_kind();

    for source in data_sources {
        let ext::DataSource {
            storage_url,
            is_prefix,
            layer,
            kind,
        } = source;

        // TODO(ab): Should some or all of these errors be returned as task error instead?
        // (No point in doing so unless this is tested in re_redap_tests.)
        if is_prefix {
            return Err(tonic::Status::internal(
                "register_with_dataset: prefix data sources should have been resolved already",
            ));
        }

        match kind {
            ext::DataSourceKind::Rrd => {}
        }

        let layer_name = if layer.is_empty() {
            LayerName::base()
        } else {
            layer
        };

        let layer_info = Arc::new(LayerInfo { name: layer_name });

        if storage_url.scheme() == "memory" {
            validated.push(validate_memory_source(
                store,
                store_kind,
                &storage_url,
                layer_info,
                &mut seen,
            )?);
            continue;
        }

        if let Some(file_source) =
            validate_file_source(store_kind, &storage_url, layer_info, &mut seen).await?
        {
            validated.push(file_source);
        }
    }

    check_intra_request_duplicates(&seen)?;

    Ok((store_kind, validated))
}

fn validate_memory_source(
    store: &InMemoryStore,
    expected_store_kind: StoreKind,
    storage_url: &url::Url,
    layer_info: Arc<LayerInfo>,
    seen: &mut BTreeMap<(LayerName, SegmentId), Vec<url::Url>>,
) -> tonic::Result<ValidatedSource> {
    let store_slot_id = parse_memory_url(storage_url)?;
    let resolved = store.resolve_store(&store_slot_id).ok_or_else(|| {
        tonic::Status::not_found(format!("store not found for memory URL: {storage_url}"))
    })?;
    let store_id = resolved.store_id();
    if store_id.kind() != expected_store_kind {
        return Err(tonic::Status::invalid_argument(format!(
            "memory store has kind {:?}, expected {expected_store_kind:?}",
            store_id.kind()
        )));
    }
    let segment_id = SegmentId::new(store_id.recording_id().to_string());
    seen.entry((layer_info.name.clone(), segment_id.clone()))
        .or_default()
        .push(storage_url.clone());
    Ok(ValidatedSource::Memory {
        store_slot_id,
        resolved,
        segment_id,
        layer_info,
    })
}

/// Returns `None` if the file's store kind doesn't match (silently skipped).
async fn validate_file_source(
    store_kind: StoreKind,
    storage_url: &url::Url,
    layer_info: Arc<LayerInfo>,
    seen: &mut BTreeMap<(LayerName, SegmentId), Vec<url::Url>>,
) -> tonic::Result<Option<ValidatedSource>> {
    let rrd_path = rrd_path_from_url(storage_url)?;
    let metadata = fs::metadata(&rrd_path)
        .await
        .map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => tonic::Status::not_found(format!(
                "RRD file not found, file does not exist: {rrd_path:?}"
            )),
            _ => tonic::Status::internal(format!(
                "Failed to check whether RRD file exists: {err:#}\nFile path: {rrd_path:?}"
            )),
        })?;
    if !metadata.is_file() {
        return Err(tonic::Status::not_found(format!(
            "RRD file not found, path is not a file: {rrd_path:?}"
        )));
    }

    let store_ids = load_store_ids(&rrd_path).await?;

    let mut matched = false;
    for store_id in store_ids {
        if store_id.kind() != store_kind {
            continue;
        }
        matched = true;
        seen.entry((
            layer_info.name.clone(),
            SegmentId::from(store_id.recording_id()),
        ))
        .or_default()
        .push(storage_url.clone());
    }

    if !matched {
        return Ok(None);
    }

    Ok(Some(ValidatedSource::File {
        rrd_path,
        layer_info,
        storage_url: storage_url.clone(),
    }))
}

fn rrd_path_from_url(storage_url: &url::Url) -> tonic::Result<PathBuf> {
    #[cfg(not(target_arch = "wasm32"))]
    let rrd_path = storage_url.to_file_path();

    #[cfg(target_arch = "wasm32")]
    let rrd_path = {
        // NOTE: `Url::to_file_path` is not available on browser Wasm targets, so keep the
        // Wasm conversion here in sync with native file-URL semantics.
        if storage_url.scheme() == "file" && storage_url.host().is_none() {
            let path = storage_url.path().strip_prefix('/').ok_or(());
            path.and_then(|path| {
                use percent_encoding::percent_decode;
                let mut bytes = Vec::with_capacity(storage_url.path().len());
                for segment in path.split('/') {
                    bytes.push(b'/');
                    bytes.extend(percent_decode(segment.as_bytes()));
                }

                String::from_utf8(bytes)
                    .map(PathBuf::from)
                    .map_err(|_err| ())
            })
        } else {
            Err(())
        }
    };

    let Ok(rrd_path) = rrd_path else {
        return if storage_url.scheme() == "file" && storage_url.host().is_some() {
            Err(tonic::Status::not_found(format!(
                "RRD file not found, file URI should not have a host: {storage_url} \
                 (this may be caused by invalid relative-path URI)"
            )))
        } else {
            Err(tonic::Status::not_found(format!(
                "RRD file not found, could not load URI: {storage_url}"
            )))
        };
    };

    Ok(rrd_path)
}

fn check_intra_request_duplicates(
    seen: &BTreeMap<(LayerName, SegmentId), Vec<url::Url>>,
) -> tonic::Result<()> {
    let duplicates: Vec<_> = seen.iter().filter(|(_, urls)| urls.len() > 1).collect();
    if duplicates.is_empty() {
        return Ok(());
    }

    let details: Vec<String> = duplicates
        .iter()
        .map(|((layer, segment_id), urls)| {
            let uri_lines = urls
                .iter()
                .map(|u| format!("    {u}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!("  segment id: {segment_id}, layer name: {layer}\n{uri_lines}")
        })
        .collect();

    Err(tonic::Status::invalid_argument(format!(
        "duplicate segment layers in request:\n{}",
        details.join("\n")
    )))
}

// ---

/// Phase 2: load file-backed sources into memory and unify with already-in-memory sources.
async fn load_sources(
    validated: Vec<ValidatedSource>,
    store_kind: StoreKind,
) -> tonic::Result<Vec<ReadySource>> {
    let mut ready: Vec<ReadySource> = Vec::new();

    for source in validated {
        match source {
            ValidatedSource::Memory {
                store_slot_id,
                resolved,
                segment_id,
                layer_info,
            } => {
                let storage_url =
                    Url::parse(&format!("memory:///store/{store_slot_id}")).map_err(|err| {
                        tonic::Status::internal(format!("failed to build memory URL: {err}"))
                    })?;
                ready.push(ReadySource {
                    store_slot_id,
                    resolved,
                    segment_id,
                    layer_info,
                    storage_url,
                });
            }

            ValidatedSource::File {
                rrd_path,
                layer_info,
                storage_url,
            } => {
                re_log::info!("Loading {rrd_path:?}…");

                let stores = ResolvedStore::load_rrd_file(&rrd_path, store_kind).await?;

                for (store_id, resolved) in stores {
                    ready.push(ReadySource {
                        store_slot_id: StoreSlotId::new(),
                        resolved,
                        segment_id: SegmentId::new(store_id.recording_id().to_string()),
                        layer_info: layer_info.clone(),
                        storage_url: storage_url.clone(),
                    });
                }
            }
        }
    }

    Ok(ready)
}

// ---

/// Phase 3: register stores in the pool and add sources to the dataset.
async fn register_sources(
    store: &mut InMemoryStore,
    dataset_id: EntryId,
    ready: Vec<ReadySource>,
    on_duplicate: IfDuplicateBehavior,
) -> tonic::Result<RegisterWithDatasetResult> {
    let mut result = RegisterWithDatasetResult::default();
    let mut failed_task_results: Vec<(TaskId, TaskResult)> = vec![];

    for source in &ready {
        store.register_store_with_id(source.store_slot_id, &source.resolved);
    }

    {
        let dataset = store.dataset_mut(dataset_id)?;

        for source in ready {
            let add_result = dataset
                .add_source(
                    source.segment_id.clone(),
                    source.layer_info.clone(),
                    source.store_slot_id,
                    source.resolved,
                    on_duplicate,
                )
                .await;

            match add_result {
                Ok(()) => {
                    result.segment_ids.push(source.segment_id);
                    result.segment_layers.push(source.layer_info.name.clone());
                    result.segment_types.push(ext::DataSourceKind::Rrd);
                    result.storage_urls.push(source.storage_url);
                    result.task_ids.push(TaskId {
                        id: TASK_ID_SUCCESS.to_owned(),
                    });
                }

                // Schema conflicts and asset-segment rejections fail just this source's task,
                // matching how the cloud server reports them during registration.
                Err(Error::SchemaConflict(msg) | Error::SegmentRejected(msg)) => {
                    result.segment_ids.push(SegmentId::new(String::new()));
                    result.segment_layers.push(source.layer_info.name.clone());
                    result.segment_types.push(ext::DataSourceKind::Rrd);
                    result.storage_urls.push(source.storage_url);

                    let task_id = TaskId::new();
                    result.task_ids.push(task_id.clone());
                    failed_task_results.push((task_id, TaskResult::failed(&msg)));
                }

                // Everything else, including the synchronous segment-count limit, aborts the batch.
                Err(other_err) => {
                    return Err(other_err.into());
                }
            }
        }
    }

    // Register all task results now that the mutable borrow of dataset is done
    for (task_id, task_result) in failed_task_results {
        store.task_registry().register_failure(task_id, task_result);
    }

    Ok(result)
}

// ---

/// Extracts unique store IDs from an RRD file without loading chunk data.
///
/// Returns a deduplicated set because a single RRD can contain duplicate
/// `SetStoreInfo` messages for the same store.
async fn load_store_ids(rrd_path: &Path) -> tonic::Result<BTreeSet<StoreId>> {
    #[cfg(not(target_arch = "wasm32"))]
    let mut file = fs::File::open(rrd_path)
        .await
        .map_err(|err| {
            tonic::Status::internal(format!(
                "Failed to open RRD file: {err:#}\nFile path: {rrd_path:?}"
            ))
        })?
        .compat();

    #[cfg(target_arch = "wasm32")]
    let mut file = {
        let bytes = fs::read(rrd_path).await.map_err(|err| {
            tonic::Status::internal(format!(
                "Failed to open RRD file: {err:#}\nFile path: {rrd_path:?}"
            ))
        })?;
        // TODO(RR-5154): Avoid buffering the full OPFS file once footer enumeration can use range reads.
        futures::io::Cursor::new(bytes)
    };

    let store_ids = re_log_encoding::enumerate_rrd_stores(&mut file)
        .await
        .map_err(|err| {
            tonic::Status::internal(format!("Failed to enumerate RRD stores: {err:#}"))
        })?;

    Ok(store_ids.into_iter().collect())
}

/// Parses a `memory:///store/{store_slot_id}` URL and returns the [`StoreSlotId`].
fn parse_memory_url(url: &url::Url) -> tonic::Result<StoreSlotId> {
    let path = url.path();
    let slot_id_str = path.strip_prefix("/store/").ok_or_else(|| {
        tonic::Status::invalid_argument(format!(
            "invalid memory URL format, expected memory:///store/{{store_slot_id}}: {url}"
        ))
    })?;
    slot_id_str.parse::<StoreSlotId>().map_err(|err| {
        tonic::Status::invalid_argument(format!(
            "invalid store slot ID in memory URL '{url}': {err}"
        ))
    })
}
