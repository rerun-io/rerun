use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use re_log_types::{EntryId, StoreId};
use re_protos::cloud::v1alpha1::ext;
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_types_core::{LayerClass, LayerName};

use crate::store::{Error, InMemoryStore, ResolvedStore, StoreSlotId, TASK_ID_SUCCESS, TaskResult};

/// Return type of [`crate::rerun_cloud::RerunCloudHandler::do_register_with_dataset`].
#[derive(Default)]
pub struct RegisterWithDatasetResult {
    /// Recording IDs from the registered RRDs, one per data source.
    ///
    /// Empty string for sources that failed with a schema conflict.
    pub segment_ids: Vec<String>,

    /// Layer name for each registered source.
    pub segment_layers: Vec<LayerName>,

    /// File format of each source (e.g. `"rrd"`).
    pub segment_types: Vec<String>,

    /// Storage URL for each source.
    pub storage_urls: Vec<String>,

    /// Task ID for each source; [`crate::store::TASK_ID_SUCCESS`] for successes,
    /// a unique ID for schema-conflict failures.
    pub task_ids: Vec<String>,
}

/// Register multiple data sources within a dataset.
///
/// These can belong to different layers and different segments.
pub async fn do_register_with_dataset(
    store: &mut InMemoryStore,
    dataset_id: EntryId,
    data_sources: Vec<ext::DataSource>,
    on_duplicate: IfDuplicateBehavior,
) -> tonic::Result<RegisterWithDatasetResult> {
    // Phase 1: Extract store IDs cheaply and check for intra-request duplicates.
    //
    // We extract store IDs from the RRD footer (fast) or by scanning messages
    // for SetStoreInfo (fallback for older files without footers). This avoids
    // full chunk loading on the unhappy path (duplicates found).
    //
    // The `on_duplicate` flag only affects cross-request duplicates (conflicts with
    // already-registered segments), not intra-request duplicates.
    enum ValidatedSource {
        File {
            rrd_path: PathBuf,
            layer_name: LayerName,
            storage_url: url::Url,
        },
        Memory {
            store_slot_id: StoreSlotId,
            resolved: ResolvedStore,
            segment_id: SegmentId,
            layer_name: LayerName,
        },
    }

    let mut seen: BTreeMap<(String, LayerName), Vec<url::Url>> = BTreeMap::new();
    let mut validated_sources: Vec<ValidatedSource> = Vec::new();

    let store_kind = store.dataset(dataset_id)?.store_kind();

    for source in data_sources {
        let ext::DataSource {
            storage_url,
            is_prefix,
            layer,
            kind,
            layer_class,
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

        match layer_class {
            LayerClass::Asset => {
                return Err(tonic::Status::unimplemented(
                    "register_with_dataset: asset layers are not supported",
                ));
            }
            LayerClass::Segment => {}
        }

        let layer = if layer.is_empty() {
            LayerName::base()
        } else {
            layer
        };

        // Handle memory:// URLs (re-registration of existing stores)
        if storage_url.scheme() == "memory" {
            let store_slot_id = parse_memory_url(&storage_url)?;
            let resolved = store.resolve_store(&store_slot_id).ok_or_else(|| {
                tonic::Status::not_found(format!("store not found for memory URL: {storage_url}"))
            })?;
            let store_id = resolved.store_id();
            if store_id.kind() != store_kind {
                continue;
            }
            let segment_id = SegmentId::new(store_id.recording_id().to_string());
            let key = (segment_id.id.clone(), layer.clone());
            seen.entry(key).or_default().push(storage_url.clone());
            validated_sources.push(ValidatedSource::Memory {
                store_slot_id,
                resolved,
                segment_id,
                layer_name: layer,
            });
            continue;
        }

        let Ok(rrd_path) = storage_url.to_file_path() else {
            return if storage_url.scheme() == "file" && storage_url.host().is_some() {
                Err(tonic::Status::not_found(format!(
                    "RRD file not found, file URI should not have a host: {storage_url} (this may be caused by invalid relative-path URI)"
                )))
            } else {
                Err(tonic::Status::not_found(format!(
                    "RRD file not found, could not load URI: {storage_url}"
                )))
            };
        };

        if !rrd_path.exists() {
            return Err(tonic::Status::not_found(format!(
                "RRD file not found, file does not exists: {rrd_path:?}"
            )));
        }

        if !rrd_path.is_file() {
            return Err(tonic::Status::not_found(format!(
                "RRD file not found, path is not a file: {rrd_path:?}"
            )));
        }

        // Extract store IDs cheaply (footer or message scan, no chunk loading)
        let store_ids = load_store_ids(&rrd_path)?;

        for store_id in store_ids {
            if store_id.kind() != store_kind {
                continue;
            }

            let segment_id_str = store_id.recording_id().to_string();
            let key = (segment_id_str, layer.clone());

            seen.entry(key).or_default().push(storage_url.clone());
        }

        validated_sources.push(ValidatedSource::File {
            rrd_path,
            layer_name: layer,
            storage_url,
        });
    }

    // Check for intra-request duplicates
    let duplicates: Vec<_> = seen.iter().filter(|(_, urls)| urls.len() > 1).collect();

    if !duplicates.is_empty() {
        let details: Vec<String> = duplicates
            .iter()
            .map(|((segment_id, layer), urls)| {
                let uri_lines = urls
                    .iter()
                    .map(|u| format!("    {u}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("  segment id: {segment_id}, layer name: {layer}\n{uri_lines}")
            })
            .collect();
        return Err(tonic::Status::invalid_argument(format!(
            "duplicate segment layers in request:\n{}",
            details.join("\n")
        )));
    }

    // Phase 2: Load file sources and unify with memory sources into a common form.
    struct ReadySource {
        store_slot_id: StoreSlotId,
        resolved: ResolvedStore,
        segment_id: SegmentId,
        layer_name: LayerName,
        storage_url: String,
    }

    let mut ready_sources: Vec<ReadySource> = Vec::new();

    for source in validated_sources {
        match source {
            ValidatedSource::Memory {
                store_slot_id,
                resolved,
                segment_id,
                layer_name,
            } => {
                ready_sources.push(ReadySource {
                    storage_url: format!("memory:///store/{store_slot_id}"),
                    store_slot_id,
                    resolved,
                    segment_id,
                    layer_name,
                });
            }

            ValidatedSource::File {
                rrd_path,
                layer_name,
                storage_url,
            } => {
                re_log::info!("Loading RRD: {}", rrd_path.display());

                for (store_id, resolved) in ResolvedStore::load_rrd_file(&rrd_path, store_kind)? {
                    ready_sources.push(ReadySource {
                        store_slot_id: StoreSlotId::new(),
                        resolved,
                        segment_id: SegmentId::new(store_id.recording_id().to_string()),
                        layer_name: layer_name.clone(),
                        storage_url: storage_url.to_string(),
                    });
                }
            }
        }
    }

    // Phase 3: Register all stores in the pool, then add sources to dataset.
    let mut result = RegisterWithDatasetResult::default();
    let mut failed_task_results: Vec<(TaskId, TaskResult)> = vec![];

    for source in &ready_sources {
        store.register_store_with_id(source.store_slot_id, &source.resolved);
    }

    {
        let dataset = store.dataset_mut(dataset_id)?;

        for source in ready_sources {
            let add_result = dataset
                .add_source(
                    source.segment_id.clone(),
                    source.layer_name.clone(),
                    source.store_slot_id,
                    source.resolved,
                    on_duplicate,
                )
                .await;

            match add_result {
                Ok(()) => {
                    result.segment_ids.push(source.segment_id.to_string());
                    result.segment_layers.push(source.layer_name);
                    result.segment_types.push("rrd".to_owned());
                    result.storage_urls.push(source.storage_url);
                    result.task_ids.push(TASK_ID_SUCCESS.to_owned());
                }

                Err(Error::SchemaConflict(msg)) => {
                    result.segment_ids.push(String::new());
                    result.segment_layers.push(source.layer_name);
                    result.segment_types.push("rrd".to_owned());
                    result.storage_urls.push(source.storage_url);

                    let task_id = TaskId::new();
                    result.task_ids.push(task_id.id.clone());
                    failed_task_results.push((task_id, TaskResult::failed(&msg)));
                }

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

/// Extracts unique store IDs from an RRD file without loading chunk data.
///
/// Returns a deduplicated set because a single RRD can contain duplicate
/// `SetStoreInfo` messages for the same store.
fn load_store_ids(rrd_path: &std::path::Path) -> tonic::Result<BTreeSet<StoreId>> {
    let reader = std::io::BufReader::new(
        std::fs::File::open(rrd_path)
            .map_err(|err| tonic::Status::internal(format!("Failed to open RRD file: {err:#}")))?,
    );
    let decoder = re_log_encoding::DecoderApp::decode_lazy(reader);

    let mut store_ids = BTreeSet::new();
    for msg_result in decoder {
        let msg = msg_result.map_err(|err| {
            tonic::Status::internal(format!("Failed to decode RRD message: {err:#}"))
        })?;
        if let re_log_types::LogMsg::SetStoreInfo(info) = msg {
            store_ids.insert(info.info.store_id);
        }
    }

    Ok(store_ids)
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
