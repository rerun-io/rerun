use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
use std::sync::Arc;

use arrow::array::BinaryArray;
use arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use futures::StreamExt as _;
use nohash_hasher::{IntMap, IntSet};
use re_protos::common::v1alpha1::TaskId;
use tonic::{Code, Request, Response, Status};

use re_arrow_util::RecordBatchExt as _;
use re_chunk_store::{
    Chunk, ChunkId, ChunkStore, ChunkStoreHandle, ChunkTrackingMode, LatestAtQuery, RangeQuery,
};
use re_log_encoding::ToTransport as _;
use re_log_types::{AbsoluteTimeRange, EntityPath, EntryId, StoreId, StoreKind, TimelineName};
#[cfg(not(target_arch = "wasm32"))]
use re_protos::cloud::v1alpha1::ext::{CreateTableEntryResponse, ProviderDetails};
use re_protos::cloud::v1alpha1::ext::{
    QueryDatasetDataframe, QueryTasksDataframe, RegisterWithDatasetDataframe,
    ScanDatasetManifestDataframe,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_server::RerunCloudService;
use re_protos::cloud::v1alpha1::{
    CancelTasksRequest, CancelTasksResponse, DeleteEntryResponse, DoBandwidthTestResponse,
    EntryCreatedEvent, EntryDeletedEvent, EntryDetails, EntryKind, EventKind, FetchChunksRequest,
    GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse, GetDatasetSchemaResponse,
    GetRrdManifestResponse, GetSegmentTableSchemaResponse, QueryDatasetResponse,
    QueryTasksOnCompletionRequest, QueryTasksOnCompletionResponse, QueryTasksRequest,
    QueryTasksResponse, RegisterTableRequest, RegisterTableResponse, ScanDatasetManifestRequest,
    ScanDatasetManifestResponse, ScanSegmentTableResponse, ScanTableResponse, WatchEventsResponse,
    watch_events_response,
};
use re_protos::common::v1alpha1::ext::{DatasetKind, IfDuplicateBehavior, SegmentId};
use re_protos::headers::RerunHeadersExtractorExt as _;
use re_protos::missing_field;
use re_protos::{
    EntryName,
    cloud::v1alpha1::ext::{
        self, CreateDatasetEntryRequest, CreateDatasetEntryResponse, CreateTableEntryRequest,
        DataSource, EntryDetailsUpdate, QueryDatasetRequest, ReadDatasetEntryResponse,
        ReadTableEntryResponse, TableInsertMode, UpdateDatasetEntryRequest,
        UpdateDatasetEntryResponse, UpdateEntryRequest, UpdateEntryResponse,
        UpdateTableEntryRequest, UpdateTableEntryResponse,
    },
};
#[cfg(not(target_arch = "wasm32"))]
use re_tuid::Tuid;
use re_types_core::LayerName;

mod register_with_dataset;
use self::register_with_dataset::{RegisterWithDatasetResult, do_register_with_dataset};

#[cfg(not(target_arch = "wasm32"))]
use crate::NamedPath;
#[cfg(not(target_arch = "wasm32"))]
use crate::OnError;
use crate::store::{
    ChunkKey, Dataset, InMemoryStore, ResolvedStore, StoreSlotId, Table, TaskResult,
};
use crate::store::{LayerInfo, TASK_ID_SUCCESS};

#[derive(Debug)]
#[cfg_attr(target_arch = "wasm32", derive(Clone, Copy, Default))]
pub struct RerunCloudHandlerSettings {
    #[cfg(not(target_arch = "wasm32"))]
    storage_dir: tempfile::TempDir,
}

#[cfg(not(target_arch = "wasm32"))]
impl Default for RerunCloudHandlerSettings {
    fn default() -> Self {
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            storage_dir: create_data_dir().expect("Failed to create data directory"),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn create_data_dir() -> Result<tempfile::TempDir, crate::store::Error> {
    Ok(tempfile::Builder::new().prefix("rerun-data-").tempdir()?)
}

/// Apply a client-supplied SQL boolean `filter` to an in-memory `RecordBatch`.
///
/// The SQL references the batch's own (public) column names. An empty filter is a no-op. Used to
/// serve the `filter` field of `ScanSegmentTable` / `ScanDatasetManifest`.
fn apply_sql_filter(batch: RecordBatch, filter_sql: &str) -> tonic::Result<RecordBatch> {
    if filter_sql.is_empty() {
        return Ok(batch);
    }

    let df_schema = datafusion::common::DFSchema::try_from(batch.schema().as_ref().clone())
        .map_err(|err| Status::internal(format!("Unable to build filter schema: {err:#}")))?;

    let expr = SessionContext::new()
        .parse_sql_expr(filter_sql, &df_schema)
        .map_err(|err| {
            Status::invalid_argument(format!("Unable to parse filter SQL {filter_sql:?}: {err}"))
        })?;

    // Neither `parse_sql_expr` nor `create_physical_expr` applies type coercion, so an untyped
    // SQL literal keeps its parsed type and e.g. `uint64_col > 100` (Int64 literal) would fail
    // at evaluation time with a type mismatch. Coerce against the schema here.
    let expr = datafusion::optimizer::simplify_expressions::ExprSimplifier::new(
        datafusion::logical_expr::simplify::SimplifyContext::default(),
    )
    .coerce(expr, &df_schema)
    .map_err(|err| Status::invalid_argument(format!("Unable to coerce filter: {err}")))?;

    let physical =
        datafusion::physical_expr::create_physical_expr(&expr, &df_schema, &Default::default())
            .map_err(|err| Status::invalid_argument(format!("Unable to plan filter: {err}")))?;

    let evaluated = physical
        .evaluate(&batch)
        .and_then(|value| value.into_array(batch.num_rows()))
        .map_err(|err| Status::invalid_argument(format!("Unable to evaluate filter: {err:#}")))?;

    let mask = evaluated
        .as_any()
        .downcast_ref::<arrow::array::BooleanArray>()
        .ok_or_else(|| Status::invalid_argument("filter expression is not a boolean"))?;

    arrow::compute::filter_record_batch(&batch, mask)
        .map_err(|err| Status::internal(format!("Unable to apply filter: {err:#}")))
}

#[derive(Default)]
pub struct RerunCloudHandlerBuilder {
    settings: RerunCloudHandlerSettings,
    store: InMemoryStore,
}

impl RerunCloudHandlerBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn with_directory_as_dataset(
        mut self,
        directory: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
        on_error: crate::OnError,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_dataset(directory, on_duplicate, on_error)
            .await?;

        Ok(self)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn with_rrds_as_dataset(
        mut self,
        dataset_name: EntryName,
        rrd_paths: Vec<PathBuf>,
        on_duplicate: IfDuplicateBehavior,
        on_error: crate::OnError,
    ) -> Result<Self, crate::store::Error> {
        let dataset_id = self.store.create_dataset(dataset_name, None)?;

        for rrd_path in rrd_paths {
            let load_result = self
                .store
                .register_rrd_to_dataset(
                    dataset_id,
                    &rrd_path,
                    None,
                    on_duplicate,
                    StoreKind::Recording,
                )
                .await;
            match load_result {
                Ok(_segment_ids) => {}
                Err(err) => match on_error {
                    OnError::Continue => {
                        re_log::warn!("Failed loading file {}: {err}", rrd_path.display());
                    }
                    OnError::Abort => {
                        return Err(err);
                    }
                },
            }
        }

        Ok(self)
    }

    #[cfg(all(feature = "lance", not(target_arch = "wasm32")))]
    pub async fn with_directory_as_table(
        mut self,
        path: &NamedPath,
        on_duplicate: IfDuplicateBehavior,
    ) -> Result<Self, crate::store::Error> {
        self.store
            .load_directory_as_table(path, on_duplicate)
            .await?;

        Ok(self)
    }

    pub fn with_eager_chunk_store_config(
        mut self,
        config: re_chunk_store::ChunkStoreConfig,
    ) -> Self {
        self.store.set_eager_chunk_store_config(config);
        self
    }

    pub fn build(self) -> RerunCloudHandler {
        RerunCloudHandler::new(self.settings, self.store)
    }
}

// ---

pub struct RerunCloudHandler {
    #[cfg(not(target_arch = "wasm32"))]
    settings: RerunCloudHandlerSettings,
    eager_chunk_store_config: re_chunk_store::ChunkStoreConfig,
    store: tokio::sync::RwLock<InMemoryStore>,
    events_tx: tokio::sync::broadcast::Sender<WatchEventsResponse>,
}

impl RerunCloudHandler {
    pub fn new(settings: RerunCloudHandlerSettings, store: InMemoryStore) -> Self {
        #[cfg(target_arch = "wasm32")]
        let _ = settings;
        let eager_chunk_store_config = store.eager_chunk_store_config();
        let (events_tx, _) = tokio::sync::broadcast::channel(1024);
        Self {
            #[cfg(not(target_arch = "wasm32"))]
            settings,
            eager_chunk_store_config,
            store: tokio::sync::RwLock::new(store),
            events_tx,
        }
    }

    /// Broadcast a catalog event to all `WatchEvents` subscribers.
    fn notify(&self, kind: watch_events_response::Kind) {
        // A send error just means there are no subscribers, which is fine.
        let _ = self
            .events_tx
            .send(WatchEventsResponse { kind: Some(kind) })
            .ok();
    }

    /// Returns all the chunk stores of the specified dataset and segment ids. If `segment_ids`
    /// is `None`, return stores of all segments.
    ///
    /// Returns (segment id, layer name, store) tuples.
    async fn get_chunk_stores(
        &self,
        dataset_id: EntryId,
        segment_ids: Option<&[SegmentId]>,
    ) -> tonic::Result<Vec<(SegmentId, LayerName, StoreSlotId, ResolvedStore)>> {
        let store = self.store.read().await;
        let dataset = store.dataset(dataset_id)?;

        Ok(dataset
            .segments_from_ids(segment_ids)
            .flat_map(|(segment_id, segment)| {
                segment.iter_sources().map(|(layer_name, source)| {
                    (
                        segment_id.clone(),
                        layer_name.clone(),
                        source.store_slot_id(),
                        source.resolved_store().clone(),
                    )
                })
            })
            .collect())
    }

    #[cfg_attr(target_arch = "wasm32", expect(clippy::unused_async))]
    async fn resolve_data_sources(data_sources: &[DataSource]) -> tonic::Result<Vec<DataSource>> {
        let mut resolved = Vec::<DataSource>::with_capacity(data_sources.len());
        for source in data_sources {
            if source.is_prefix {
                #[cfg(target_arch = "wasm32")]
                {
                    // TODO(RR-5155): Support enumerating OPFS directories for prefix registration.
                    return Err(tonic::Status::invalid_argument(
                        "prefix data sources are not supported on wasm",
                    ));
                }

                #[cfg(not(target_arch = "wasm32"))]
                {
                    if source.storage_url.scheme() == "memory" {
                        return Err(tonic::Status::invalid_argument(
                            "memory:// URLs cannot be used as prefix data sources",
                        ));
                    }
                    let path = source.storage_url.to_file_path().map_err(|_err| {
                        tonic::Status::invalid_argument(format!(
                            "getting file path from {:?}",
                            source.storage_url
                        ))
                    })?;
                    let meta =
                        tokio::fs::metadata(&path)
                            .await
                            .map_err(|err| match err.kind() {
                                std::io::ErrorKind::NotFound => tonic::Status::invalid_argument(
                                    format!("Directory not found: {path:?}"),
                                ),
                                _ => tonic::Status::invalid_argument(format!(
                                    "Failed to read directory metadata {path:?}: {err:#}"
                                )),
                            })?;
                    if !meta.is_dir() {
                        return Err(tonic::Status::invalid_argument(format!(
                            "expected prefix / directory but got an object ({path:?})"
                        )));
                    }

                    // Recursively walk the directory and grab all '.rrd' files
                    let mut dirs_to_visit = vec![path];
                    let mut files = Vec::new();

                    while let Some(current_dir) = dirs_to_visit.pop() {
                        let mut entries =
                            tokio::fs::read_dir(&current_dir).await.map_err(|err| {
                                tonic::Status::internal(format!(
                                    "Failed to read directory {current_dir:?}: {err:#}"
                                ))
                            })?;

                        while let Some(entry) = entries.next_entry().await.map_err(|err| {
                            tonic::Status::internal(format!(
                                "Failed to read directory entry: {err:#}"
                            ))
                        })? {
                            let entry_path = entry.path();
                            let file_type = entry.file_type().await.map_err(|err| {
                                tonic::Status::internal(format!(
                                    "Failed to read directory entry metadata: {err:#}"
                                ))
                            })?;

                            if file_type.is_dir() {
                                dirs_to_visit.push(entry_path);
                            } else if let Some(extension) = entry_path.extension()
                                && extension == "rrd"
                            {
                                files.push(entry_path);
                            }
                        }
                    }

                    if files.is_empty() {
                        return Err(tonic::Status::invalid_argument(format!(
                            "no rrd files found in {:?}",
                            source.storage_url
                        )));
                    }

                    for file_path in files {
                        let mut file_url = source.storage_url.clone();
                        file_url.set_path(&file_path.to_string_lossy());
                        resolved.push(DataSource {
                            storage_url: file_url,
                            is_prefix: false,
                            ..source.clone()
                        });
                    }
                }
            } else {
                resolved.push(source.clone());
            }
        }

        Ok(resolved)
    }
}

impl std::fmt::Debug for RerunCloudHandler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RerunCloudHandler").finish()
    }
}

macro_rules! decl_stream {
    ($stream:ident<manifest:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };

    ($stream:ident<rerun_cloud:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };

    ($stream:ident<tasks:$resp:ident>) => {
        pub type $stream = std::pin::Pin<
            Box<
                dyn futures::Stream<Item = tonic::Result<re_protos::cloud::v1alpha1::$resp>> + Send,
            >,
        >;
    };
}

decl_stream!(DoBandwidthTestResponseStream<rerun_cloud:DoBandwidthTestResponse>);
decl_stream!(WatchEventsResponseStream<rerun_cloud:WatchEventsResponse>);
decl_stream!(FetchChunksResponseStream<manifest:FetchChunksResponse>);
decl_stream!(GetAssetsForSegmentResponseStream<rerun_cloud:GetAssetsForSegmentResponse>);
decl_stream!(GetRrdManifestResponseStream<manifest:GetRrdManifestResponse>);
decl_stream!(QueryDatasetResponseStream<manifest:QueryDatasetResponse>);
decl_stream!(QueryTasksOnCompletionResponseStream<tasks:QueryTasksOnCompletionResponse>);
decl_stream!(ScanDatasetManifestResponseStream<manifest:ScanDatasetManifestResponse>);
decl_stream!(ScanSegmentTableResponseStream<manifest:ScanSegmentTableResponse>);
decl_stream!(ScanTableResponseStream<rerun_cloud:ScanTableResponse>);
decl_stream!(UnregisterFromDatasetResponseStream<manifest:UnregisterFromDatasetResponse>);

impl RerunCloudHandler {
    async fn find_datasets(
        &self,
        entry_id: Option<EntryId>,
        name: Option<EntryName>,
        store_kind: Option<StoreKind>,
    ) -> tonic::Result<Vec<EntryDetails>> {
        let store = self.store.read().await;

        let dataset = match (entry_id, name) {
            (None, None) => None,

            (Some(entry_id), None) => Some(store.dataset(entry_id)?),

            (None, Some(name)) => Some(store.dataset_by_name(&name)?),

            (Some(entry_id), Some(name)) => {
                let dataset = store.dataset_by_name(&name)?;
                if dataset.id() != entry_id {
                    return Err(tonic::Status::not_found(format!(
                        "Dataset with ID {entry_id} not found"
                    )));
                }
                Some(dataset)
            }
        };

        let dataset_iter = if let Some(dataset) = dataset {
            itertools::Either::Left(std::iter::once(dataset))
        } else {
            itertools::Either::Right(store.iter_datasets())
        };

        Ok(dataset_iter
            .filter(|dataset| {
                store_kind.is_none_or(|store_kind| dataset.store_kind() == store_kind)
            })
            .map(Dataset::as_entry_details)
            .map(Into::into)
            .collect())
    }

    async fn find_tables(
        &self,
        entry_id: Option<EntryId>,
        name: Option<EntryName>,
    ) -> tonic::Result<Vec<EntryDetails>> {
        let store = self.store.read().await;

        let table = match (entry_id, name) {
            (None, None) => None,

            (Some(entry_id), None) => {
                let Some(table) = store.table(entry_id) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with ID {entry_id} not found"
                    )));
                };
                Some(table)
            }

            (None, Some(name)) => {
                let Some(table) = store.table_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with name {name} not found"
                    )));
                };
                Some(table)
            }

            (Some(entry_id), Some(name)) => {
                let Some(table) = store.table_by_name(&name) else {
                    return Err(tonic::Status::not_found(format!(
                        "Table with name {name} not found"
                    )));
                };
                if table.id() != entry_id {
                    return Err(tonic::Status::not_found(format!(
                        "Table with ID {entry_id} not found"
                    )));
                }
                Some(table)
            }
        };

        let table_iter = if let Some(table) = table {
            itertools::Either::Left(std::iter::once(table))
        } else {
            itertools::Either::Right(store.iter_tables())
        };

        Ok(table_iter
            .map(Table::as_entry_details)
            .map(Into::into)
            .collect())
    }
}

/// Verifies that the referenced blueprint dataset (if any) exists and is itself a blueprint dataset.
///
/// Internal consistency of the `DatasetDetails`/`TableDetails` is checked separately via their
/// `validate_consistency` methods.
fn validate_blueprint_dataset(
    store: &InMemoryStore,
    blueprint_dataset: Option<EntryId>,
    entry_kind: &str,
) -> tonic::Result<()> {
    let Some(blueprint_dataset) = blueprint_dataset else {
        return Ok(());
    };

    let blueprint_dataset = store.dataset(blueprint_dataset).map_err(|err| {
        tonic::Status::invalid_argument(format!(
            "{entry_kind} blueprint dataset does not exist: {err}"
        ))
    })?;

    if blueprint_dataset.store_kind() != StoreKind::Blueprint {
        return Err(tonic::Status::invalid_argument(format!(
            "{entry_kind} blueprint dataset must be a blueprint dataset"
        )));
    }

    Ok(())
}

/// Same as [`validate_blueprint_dataset`], for the asset dataset.
fn validate_asset_dataset(
    store: &InMemoryStore,
    asset_dataset: Option<EntryId>,
) -> tonic::Result<()> {
    let Some(asset_dataset) = asset_dataset else {
        return Ok(());
    };

    let asset_dataset = store.dataset(asset_dataset).map_err(|err| {
        tonic::Status::invalid_argument(format!("asset dataset does not exist: {err}"))
    })?;

    let kind = asset_dataset.dataset_kind();
    if kind != DatasetKind::Asset {
        return Err(tonic::Status::invalid_argument(format!(
            "asset dataset reference must point to an asset dataset, this is a {kind:?} dataset"
        )));
    }

    Ok(())
}

#[tonic::async_trait]
impl RerunCloudService for RerunCloudHandler {
    async fn version(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::VersionRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::VersionResponse>> {
        let re_protos::cloud::v1alpha1::VersionRequest {} = request.into_inner();

        // NOTE: Reminder that this is only fully filled iff CI=1.
        let build_info = re_build_info::build_info!();

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::VersionResponse {
                build_info: Some(build_info.into()),
                version: re_build_info::exposed_version!().to_owned(),
                cloud_provider: None,
                cloud_region: None,
                features: re_protos::cloud::v1alpha1::features::all_supported_features(),
            },
        ))
    }

    async fn who_am_i(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::WhoAmIRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::WhoAmIResponse>> {
        // The local server has no authentication, so grant full access.
        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WhoAmIResponse {
                user_id: None,
                can_read: true,
                can_write: true,
            },
        ))
    }

    type DoBandwidthTestStream = DoBandwidthTestResponseStream;

    async fn do_bandwidth_test(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::DoBandwidthTestRequest>,
    ) -> tonic::Result<tonic::Response<Self::DoBandwidthTestStream>> {
        let re_protos::cloud::v1alpha1::DoBandwidthTestRequest { num_bytes } = request.into_inner();
        let max = ext::MAX_BANDWIDTH_TEST_BYTES;
        if num_bytes > max {
            return Err(Status::invalid_argument(format!(
                "num_bytes ({num_bytes}) exceeds the maximum of {max}"
            )));
        }
        Ok(tonic::Response::new(
            Box::pin(bandwidth_test_stream(num_bytes)) as Self::DoBandwidthTestStream,
        ))
    }

    type WatchEventsStream = WatchEventsResponseStream;

    async fn watch_events(
        &self,
        request: Request<re_protos::cloud::v1alpha1::WatchEventsRequest>,
    ) -> tonic::Result<tonic::Response<Self::WatchEventsStream>> {
        let rx = self.events_tx.subscribe();

        let kinds = request.into_inner().kinds;

        let stream = futures::stream::unfold((rx, kinds), |(mut rx, kinds)| async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if kinds.is_empty() {
                            return Some((Ok(event), (rx, kinds)));
                        }

                        let subscribed = event.kind.is_some_and(|kind| kind.is_entry_kind())
                            && kinds.contains(&EventKind::entry());

                        if subscribed {
                            return Some((Ok(event), (rx, kinds)));
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::WatchEventsStream
        ))
    }

    // --- Catalog ---

    async fn find_entries(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FindEntriesRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::FindEntriesResponse>> {
        let filter = request.into_inner().filter.unwrap_or_default();

        let entry_id = filter.id.map(TryInto::try_into).transpose()?;
        let name = filter
            .name
            .map(EntryName::new)
            .transpose()
            .map_err(|err| Status::invalid_argument(err.to_string()))?;

        // `entry_kinds` (new, repeated) always wins over the legacy singular `entry_kind` when
        // both are set. `ENTRY_KIND_UNSPECIFIED` is rejected outright; unknown *positive* values
        // (kinds newer than this server knows about) are intentionally allowed through and
        // simply match no entry, so a client requesting them degrades gracefully instead of
        // erroring out (forward compat, mirrors Rerun Hub).
        if filter
            .entry_kinds
            .contains(&(EntryKind::Unspecified as i32))
        {
            return Err(Status::invalid_argument(
                "find_entries: entry_kinds must not contain ENTRY_KIND_UNSPECIFIED",
            ));
        }

        // The effective set of raw `EntryKind` values to match against. `None` for the
        // kind-less default.
        let effective_kinds: Option<Vec<i32>> = if !filter.entry_kinds.is_empty() {
            Some(filter.entry_kinds)
        } else if let Some(kind) = filter.entry_kind {
            // Legacy singular field (pre hub 0.15)
            let kind = EntryKind::try_from(kind).map_err(|err| {
                Status::invalid_argument(format!("find_entries: invalid entry kind {err}"))
            })?;
            if kind == EntryKind::Unspecified {
                return Err(Status::invalid_argument(
                    "find_entries: entry kind unspecified",
                ));
            }
            Some(vec![kind as i32])
        } else {
            None
        };

        let matches_kind = |raw_kind: i32| match &effective_kinds {
            Some(kinds) => kinds.contains(&raw_kind),
            // When neither the new `entry_kinds` nor legacy `entry_kind` (singular)
            // are specified we fall back to the legacy default.
            //
            // See RR-5186.
            None => EntryKind::try_from(raw_kind).is_ok_and(EntryKind::is_legacy_default_kind),
        };

        let soften_not_found = |result: tonic::Result<Vec<EntryDetails>>| match result {
            Ok(entries) => Ok(entries),
            // this is a find. Degrade a NotFound to an empty result set.
            Err(err) if err.code() == Code::NotFound => Ok(vec![]),
            Err(err) => Err(err),
        };

        let mut entries = if effective_kinds.is_some() {
            // `Dataset` and `AssetDataset` are both backed by `StoreKind::Recording`, so a
            // request for just one of them still has to fetch the whole recording family and
            // filter by actual kind below (an asset dataset otherwise leaks into
            // `entry_kind=Dataset` results).
            let mut entries = Vec::new();
            if matches_kind(EntryKind::Dataset as i32)
                || matches_kind(EntryKind::AssetDataset as i32)
            {
                let result = self
                    .find_datasets(entry_id, name.clone(), Some(StoreKind::Recording))
                    .await;
                entries.extend(soften_not_found(result)?);
            }
            if matches_kind(EntryKind::BlueprintDataset as i32) {
                let result = self
                    .find_datasets(entry_id, name.clone(), Some(StoreKind::Blueprint))
                    .await;
                entries.extend(soften_not_found(result)?);
            }
            if matches_kind(EntryKind::Table as i32) {
                let result = self.find_tables(entry_id, name.clone()).await;
                entries.extend(soften_not_found(result)?);
            }
            entries
        } else {
            let datasets = self.find_datasets(entry_id, name.clone(), None).await;
            let mut datasets = soften_not_found(datasets)?;
            let tables = self.find_tables(entry_id, name.clone()).await;
            datasets.extend(soften_not_found(tables)?);
            datasets
        };

        entries.retain(|entry| matches_kind(entry.entry_kind));

        let response = re_protos::cloud::v1alpha1::FindEntriesResponse { entries };

        Ok(tonic::Response::new(response))
    }

    async fn create_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::CreateDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::CreateDatasetEntryResponse>>
    {
        let CreateDatasetEntryRequest {
            name: dataset_name,
            id: dataset_id,
        } = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        let dataset_id = store.create_dataset(dataset_name, dataset_id)?;
        let dataset = store.dataset(dataset_id)?;

        self.notify(watch_events_response::Kind::EntryCreated(
            EntryCreatedEvent {
                id: Some(dataset_id.into()),
            },
        ));

        Ok(tonic::Response::new(
            CreateDatasetEntryResponse {
                dataset: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn read_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::ReadDatasetEntryResponse>> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;

        Ok(tonic::Response::new(
            ReadDatasetEntryResponse {
                dataset_entry: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn update_dataset_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::UpdateDatasetEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateDatasetEntryResponse>>
    {
        let request: UpdateDatasetEntryRequest = request.into_inner().try_into()?;

        request
            .dataset_details
            .validate_consistency()
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;

        let mut store = self.store.write().await;
        validate_blueprint_dataset(&store, request.dataset_details.blueprint_dataset, "dataset")?;

        let mut dataset_details = request.dataset_details;

        // The asset dataset reference is server-managed: unless the client explicitly points it
        // at a new asset dataset, keep the stored one. Recording datasets created before asset
        // datasets were introduced have none, so create the missing one on demand, and replace a
        // reference left dangling by a deleted asset dataset the same way.
        let dataset = store.dataset(request.id)?;
        let stored_asset_dataset = dataset.dataset_details().asset_dataset;
        let dataset_kind = dataset.dataset_kind();
        let client_chosen_asset_dataset = dataset_details.asset_dataset.is_some()
            && dataset_details.asset_dataset != stored_asset_dataset;
        if client_chosen_asset_dataset {
            validate_asset_dataset(&store, dataset_details.asset_dataset)?;
        } else if dataset_kind == DatasetKind::Recording {
            let existing = stored_asset_dataset.filter(|id| store.dataset(*id).is_ok());
            dataset_details.asset_dataset = Some(match existing {
                Some(existing) => existing,
                None => store.create_asset_dataset_for_entry(request.id)?,
            });
        }

        let dataset = store.dataset_mut(request.id)?;

        dataset.set_dataset_details(dataset_details);

        Ok(tonic::Response::new(
            UpdateDatasetEntryResponse {
                dataset_entry: dataset.as_dataset_entry(),
            }
            .into(),
        ))
    }

    async fn read_table_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ReadTableEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::ReadTableEntryResponse>> {
        let store = self.store.read().await;

        let id = request
            .into_inner()
            .id
            .ok_or_else(|| Status::invalid_argument("No table entry ID provided"))?
            .try_into()?;

        let table = store.table(id).ok_or_else(|| {
            tonic::Status::not_found(format!("table with entry ID '{id}' not found"))
        })?;

        Ok(tonic::Response::new(
            ReadTableEntryResponse {
                table_entry: table.as_table_entry(),
            }
            .try_into()?,
        ))
    }

    async fn update_table_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::UpdateTableEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateTableEntryResponse>> {
        let request: UpdateTableEntryRequest = request.into_inner().try_into()?;

        let mut store = self.store.write().await;
        store.table(request.id).ok_or_else(|| {
            tonic::Status::not_found(format!("table with entry ID '{}' not found", request.id))
        })?;

        let mut table_details = request.table_details;
        // Backwards compatibility: tables created before table blueprints had no associated
        // blueprint dataset. If a client updates such a table without providing one, create the
        // missing dataset on demand.
        if table_details.blueprint_dataset.is_none()
            && table_details.default_blueprint_segment.is_some()
        {
            table_details.blueprint_dataset = Some(
                match store
                    .table(request.id)
                    .and_then(|table| table.table_details().blueprint_dataset)
                {
                    Some(blueprint_dataset) => blueprint_dataset,
                    None => store.create_blueprint_dataset_for_entry(request.id)?,
                },
            );
        }

        table_details
            .validate_consistency()
            .map_err(|err| tonic::Status::invalid_argument(err.to_string()))?;
        validate_blueprint_dataset(&store, table_details.blueprint_dataset, "table")?;

        let table = store.table_mut(request.id).ok_or_else(|| {
            tonic::Status::not_found(format!("table with entry ID '{}' not found", request.id))
        })?;
        table.set_table_details(table_details);

        Ok(tonic::Response::new(
            UpdateTableEntryResponse {
                table_entry: table.as_table_entry(),
            }
            .try_into()?,
        ))
    }

    async fn delete_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::DeleteEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DeleteEntryResponse>> {
        let entry_id = request.into_inner().try_into()?;

        self.store.write().await.delete_entry(entry_id)?;

        self.notify(watch_events_response::Kind::EntryDeleted(
            EntryDeletedEvent {
                id: Some(entry_id.into()),
            },
        ));

        Ok(tonic::Response::new(DeleteEntryResponse {}))
    }

    async fn update_entry(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::UpdateEntryRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::UpdateEntryResponse>> {
        let UpdateEntryRequest {
            id: entry_id,
            entry_details_update: EntryDetailsUpdate { name },
        } = request.into_inner().try_into()?;

        let mut store = self.store.write().await;

        if let Some(name) = name {
            store.rename_entry(entry_id, name)?;
        }

        Ok(tonic::Response::new(
            UpdateEntryResponse {
                entry_details: store.entry_details(entry_id)?,
            }
            .into(),
        ))
    }

    // --- Manifest Registry ---
    async fn register_with_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::RegisterWithDatasetRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::RegisterWithDatasetResponse>>
    {
        let mut store = self.store.write().await;

        let dataset_id = get_entry_id_from_headers(&store, &request)?;

        let ext::RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        } = request.into_inner().try_into()?;

        let data_sources = Self::resolve_data_sources(&data_sources).await?;
        if data_sources.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "no data sources to register",
            ));
        }

        let RegisterWithDatasetResult {
            segment_ids,
            segment_layers,
            segment_types,
            storage_urls,
            task_ids,
        } = do_register_with_dataset(&mut store, dataset_id, data_sources, on_duplicate).await?;

        let record_batch = RegisterWithDatasetDataframe {
            rerun_segment_id: segment_ids.into(),
            rerun_segment_layer: segment_layers.into(),
            rerun_segment_type: segment_types.into(),
            rerun_storage_url: storage_urls.into(),
            rerun_task_id: task_ids.into(),
        }
        .into_record_batch()
        .map_err(|err| tonic::Status::internal(format!("Failed to create dataframe: {err:#}")))?;
        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::RegisterWithDatasetResponse {
                data: Some(record_batch.into()),
            },
        ))
    }

    type UnregisterFromDatasetStream = UnregisterFromDatasetResponseStream;

    async fn unregister_from_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::UnregisterFromDatasetRequest>,
    ) -> tonic::Result<Response<Self::UnregisterFromDatasetStream>> {
        let mut store = self.store.write().await;

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        request.get_ref().sanity_check()?;

        let dataset = store.dataset_mut(entry_id)?;

        let ext::UnregisterFromDatasetRequest {
            segments_to_drop,
            layers_to_drop,
            force: _, // OSS doesn't even have statuses
        } = request.into_inner().try_into()?;

        // As per our proto conventions, an empty list means "all":
        let segments_to_drop: Option<HashSet<&SegmentId>> =
            (!segments_to_drop.is_empty()).then(|| segments_to_drop.iter().collect());
        let layers_to_drop: Option<HashSet<&LayerName>> =
            (!layers_to_drop.is_empty()).then(|| layers_to_drop.iter().collect());

        _ = dataset
            .remove_layers(segments_to_drop.as_ref(), layers_to_drop.as_ref())
            .await?;

        store.cleanup_store_pool();

        let stream = futures::stream::once(async move {
            Ok(re_protos::cloud::v1alpha1::UnregisterFromDatasetResponse {
                data: Some(ScanDatasetManifestDataframe::empty_record_batch().into()),
                task_id: Some(TaskId {
                    id: TASK_ID_SUCCESS.to_owned(),
                }),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::UnregisterFromDatasetStream
        ))
    }

    // TODO(RR-2017): This endpoint is in need of a deep redesign. For now it defaults to
    // overwriting the "base" layer.
    async fn write_chunks(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteChunksRequest>>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::WriteChunksResponse>> {
        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let mut request = request.into_inner();

        let mut chunk_stores: HashMap<_, _> = HashMap::default();

        while let Some(chunk_msg) = request.next().await {
            let chunk_msg = chunk_msg?;

            let chunk_batch: RecordBatch = chunk_msg
                .chunk
                .ok_or_else(|| tonic::Status::invalid_argument("no chunk in WriteChunksRequest"))?
                .try_into()
                .map_err(|err| {
                    tonic::Status::internal(format!("Could not decode chunk: {err:#}"))
                })?;

            // Support both new "rerun:segment_id" and legacy "rerun:partition_id" keys
            let schema = chunk_batch.schema();
            let metadata = schema.metadata();
            let segment_id: SegmentId = metadata
                .get("rerun:segment_id")
                .or_else(|| metadata.get("rerun:partition_id"))
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(
                        "Received chunk without 'rerun:segment_id' metadata",
                    )
                })?
                .clone()
                .into();

            let chunk = Arc::new(Chunk::from_record_batch(&chunk_batch).map_err(|err| {
                tonic::Status::internal(format!("error decoding chunk from record batch: {err:#}"))
            })?);

            chunk_stores
                .entry(segment_id.clone())
                .or_insert_with(|| {
                    ChunkStore::new(
                        StoreId::new(
                            StoreKind::Recording,
                            entry_id.to_string(),
                            segment_id.clone(),
                        ),
                        self.eager_chunk_store_config.clone(),
                    )
                })
                .insert_chunk(&chunk)
                .map_err(|err| {
                    tonic::Status::internal(format!("error adding chunk to store: {err:#}"))
                })?;
        }

        let mut store = self.store.write().await;

        // Build handles and register in pool first
        let handles: Vec<_> = chunk_stores
            .into_iter()
            .map(|(segment_id, chunk_store)| {
                let resolved = ResolvedStore::Eager(ChunkStoreHandle::new(chunk_store));
                let store_slot_id = store.register_store(&resolved);
                (segment_id, store_slot_id, resolved)
            })
            .collect();

        let dataset = store.dataset_mut(entry_id)?;

        for (entity_path, store_slot_id, resolved) in handles {
            dataset
                .add_source(
                    entity_path,
                    Arc::new(LayerInfo {
                        name: LayerName::base(),
                    }),
                    store_slot_id,
                    resolved,
                    IfDuplicateBehavior::Error,
                )
                .await?;
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteChunksResponse {},
        ))
    }

    async fn write_table(
        &self,
        request: tonic::Request<tonic::Streaming<re_protos::cloud::v1alpha1::WriteTableRequest>>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::WriteTableResponse>> {
        // Limit the scope of the lock here to prevent deadlocks
        // when reading and writing to the same table
        let entry_id = {
            let store = self.store.read().await;
            get_entry_id_from_headers(&store, &request)?
        };

        let mut request = request.into_inner();

        while let Some(write_msg) = request.next().await {
            let write_msg = write_msg?;

            let rb = write_msg
                .dataframe_part
                .ok_or_else(|| {
                    tonic::Status::invalid_argument("no data frame in WriteTableRequest")
                })?
                .try_into()
                .map_err(|err| {
                    tonic::Status::internal(format!("Could not decode chunk: {err:#}"))
                })?;

            let insert_op = TableInsertMode::try_from(write_msg.insert_mode)
                .map_err(|err| Status::invalid_argument(err.to_string()))?;

            #[cfg(feature = "lance")]
            {
                let mut store = self.store.write().await;
                let Some(table) = store.table_mut(entry_id) else {
                    return Err(tonic::Status::not_found("table not found"));
                };
                table.write_table(rb, insert_op).await.map_err(|err| {
                    tonic::Status::internal(format!("error writing to table: {err:#}"))
                })?;
            }

            #[cfg(not(feature = "lance"))]
            {
                let mut table = {
                    let store = self.store.read().await;
                    store
                        .table(entry_id)
                        .cloned()
                        .ok_or_else(|| tonic::Status::not_found("table not found"))?
                };
                table.write_table(rb, insert_op).await.map_err(|err| {
                    tonic::Status::internal(format!("error writing to table: {err:#}"))
                })?;
            }
        }

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::WriteTableResponse {},
        ))
    }

    /* Query schemas */

    async fn get_segment_table_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetSegmentTableSchemaRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::GetSegmentTableSchemaResponse>>
    {
        let store = self.store.read().await;

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.segment_table().map_err(|err| {
            tonic::Status::internal(format!("Unable to read segment table: {err:#}"))
        })?;

        Ok(tonic::Response::new(GetSegmentTableSchemaResponse {
            schema: Some(
                record_batch
                    .schema_ref()
                    .as_ref()
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!(
                            "unable to serialize Arrow schema: {err:#}"
                        ))
                    })?,
            ),
        }))
    }

    type ScanSegmentTableStream = ScanSegmentTableResponseStream;

    async fn scan_segment_table(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ScanSegmentTableRequest>,
    ) -> tonic::Result<tonic::Response<Self::ScanSegmentTableStream>> {
        let (mut record_batch, request) = {
            let store = self.store.read().await;
            let entry_id = get_entry_id_from_headers(&store, &request)?;
            let dataset = store.dataset(entry_id)?;
            let record_batch = dataset.segment_table().map_err(|err| {
                tonic::Status::internal(format!("Unable to read segment table: {err:#}"))
            })?;
            (record_batch, request.into_inner())
        };

        // Filter before projection so the filter can reference columns that aren't projected out.
        record_batch = apply_sql_filter(record_batch, &request.sql_filter)?;

        // project columns
        if !request.columns.is_empty() {
            record_batch = record_batch
                .project_columns(request.columns.iter().map(|s| s.as_str()))
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!("Unable to project columns: {err:#}"))
                })?;
        }

        let stream = futures::stream::once(async move {
            Ok(ScanSegmentTableResponse {
                data: Some(record_batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanSegmentTableStream
        ))
    }

    async fn get_dataset_manifest_schema(
        &self,
        request: Request<GetDatasetManifestSchemaRequest>,
    ) -> tonic::Result<Response<GetDatasetManifestSchemaResponse>> {
        let store = self.store.read().await;

        let entry_id = get_entry_id_from_headers(&store, &request)?;
        let dataset = store.dataset(entry_id)?;
        let record_batch = dataset.dataset_manifest()?;

        Ok(tonic::Response::new(GetDatasetManifestSchemaResponse {
            schema: Some(
                record_batch
                    .schema_ref()
                    .as_ref()
                    .try_into()
                    .map_err(|err| {
                        tonic::Status::internal(format!(
                            "unable to serialize Arrow schema: {err:#}"
                        ))
                    })?,
            ),
        }))
    }

    type ScanDatasetManifestStream = ScanDatasetManifestResponseStream;

    async fn scan_dataset_manifest(
        &self,
        request: Request<ScanDatasetManifestRequest>,
    ) -> tonic::Result<Response<Self::ScanDatasetManifestStream>> {
        let (mut record_batch, request) = {
            let store = self.store.read().await;
            let entry_id = get_entry_id_from_headers(&store, &request)?;
            let dataset = store.dataset(entry_id)?;
            let record_batch = dataset.dataset_manifest()?;
            (record_batch, request.into_inner())
        };

        // Filter before projection so the filter can reference columns that aren't projected out.
        record_batch = apply_sql_filter(record_batch, &request.sql_filter)?;

        // project columns
        if !request.columns.is_empty() {
            record_batch = record_batch
                .project_columns(request.columns.iter().map(|s| s.as_str()))
                .map_err(|err| {
                    tonic::Status::invalid_argument(format!("Unable to project columns: {err:#}"))
                })?;
        }

        let stream = futures::stream::once(async move {
            Ok(ScanDatasetManifestResponse {
                data: Some(record_batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::ScanDatasetManifestStream
        ))
    }

    async fn get_dataset_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetDatasetSchemaRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::GetDatasetSchemaResponse>> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let dataset = store.dataset(entry_id)?;
        let schema = dataset.schema().map_err(|err| {
            tonic::Status::internal(format!("Unable to read dataset schema: {err:#}"))
        })?;

        Ok(tonic::Response::new(GetDatasetSchemaResponse {
            schema: Some((&schema).try_into().map_err(|err| {
                tonic::Status::internal(format!("Unable to serialize Arrow schema: {err:#}"))
            })?),
        }))
    }

    type GetRrdManifestStream = GetRrdManifestResponseStream;

    async fn get_rrd_manifest(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetRrdManifestRequest>,
    ) -> tonic::Result<tonic::Response<Self::GetRrdManifestStream>> {
        let store = self.store.read().await;
        let entry_id = get_entry_id_from_headers(&store, &request)?;

        let request = request.into_inner();
        let segment_id = request
            .segment_id
            .ok_or_else(|| {
                missing_field!(
                    re_protos::cloud::v1alpha1::GetRrdManifestRequest,
                    "segment_id"
                )
            })?
            .try_into()?;

        let dataset = store.dataset(entry_id)?;
        let rrd_manifest = dataset.rrd_manifest(&segment_id)?;

        let rrd_manifest_stream =
            futures::stream::once(futures::future::ok(GetRrdManifestResponse {
                rrd_manifest: Some(rrd_manifest.to_transport(()).map_err(|err| {
                    tonic::Status::internal(format!("Unable to compute RRD manifest: {err:#}"))
                })?),
            }));

        Ok(tonic::Response::new(
            Box::pin(rrd_manifest_stream) as Self::GetRrdManifestStream
        ))
    }

    type GetAssetsForSegmentStream = GetAssetsForSegmentResponseStream;

    async fn get_assets_for_segment(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetAssetsForSegmentRequest>,
    ) -> tonic::Result<tonic::Response<Self::GetAssetsForSegmentStream>> {
        let store = self.store.read().await;

        let dataset_id = get_entry_id_from_headers(&store, &request)?;

        let dataset = store.dataset(dataset_id)?;

        let dataset_kind = dataset.dataset_kind();
        if dataset_kind != DatasetKind::Recording {
            return Err(tonic::Status::invalid_argument(format!(
                "assets can only be queried on recording datasets, this is a {dataset_kind:?} dataset"
            )));
        }

        // Datasets created before asset datasets were introduced don't have one, which simply
        // means no assets were ever registered. One is created on demand when the dataset entry
        // is next updated.
        let Some(asset_dataset) = dataset.dataset_details().asset_dataset else {
            return Ok(tonic::Response::new(
                Box::pin(futures::stream::empty()) as Self::GetAssetsForSegmentStream
            ));
        };

        // TODO(RR-4979): Filter by properties here.
        let asset_segment_ids = store
            .dataset(asset_dataset)?
            .segments()
            .keys()
            .cloned()
            .map(Into::into)
            .collect();

        let response = futures::stream::once(futures::future::ok(
            re_protos::cloud::v1alpha1::GetAssetsForSegmentResponse {
                assets_entry: Some(asset_dataset.into()),
                asset_segment_ids,
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(response) as Self::GetAssetsForSegmentStream
        ))
    }

    /* Queries */

    type QueryDatasetStream = QueryDatasetResponseStream;

    async fn query_dataset(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::QueryDatasetRequest>,
    ) -> tonic::Result<tonic::Response<Self::QueryDatasetStream>> {
        if !request.get_ref().chunk_ids.is_empty() {
            return Err(tonic::Status::unimplemented(
                "query_dataset: querying specific chunk ids is not implemented",
            ));
        }

        let entry_id = get_entry_id_from_headers(&*self.store.read().await, &request)?;

        let QueryDatasetRequest {
            segment_ids,
            entity_paths,
            select_all_entity_paths,

            //TODO(RR-2613): we must do a much better job at handling these
            chunk_ids: requested_chunk_ids,
            fuzzy_descriptors: _,
            exclude_static_data,
            exclude_temporal_data,
            scan_parameters,
            query,
            generate_direct_urls: _,
        } = request.into_inner().try_into()?;

        if scan_parameters.is_some() {
            // Logged at a low debug-level, because of https://github.com/rerun-io/rerun/pull/12578
            re_log::debug_once!("   scan_parameters are not yet implemented and will be ignored");
        }

        let entity_paths: IntSet<EntityPath> = entity_paths.into_iter().collect();
        if select_all_entity_paths && !entity_paths.is_empty() {
            return Err(tonic::Status::invalid_argument(
                "cannot specify entity paths if `select_all_entity_paths` is true",
            ));
        }

        // RR-4355: per-segment index value pushdown.
        //
        // If the request has `query.latest_at.per_segment_values`, build a
        // map keyed by segment id so the per-segment chunk-fetch loop below
        // can apply it. The ext `try_from` already validated that lengths
        // match `segment_ids` and that there are no duplicates.
        let per_segment_index_values: Option<BTreeMap<SegmentId, Vec<re_log_types::TimeInt>>> =
            match query.as_ref().and_then(|q| q.latest_at.as_ref()) {
                Some(la) if !la.per_segment_values.is_empty() => Some(
                    std::iter::zip(&segment_ids, &la.per_segment_values)
                        .map(|(sid, values)| {
                            (
                                sid.clone(),
                                values
                                    .iter()
                                    .map(|v| re_log_types::TimeInt::new_temporal(*v))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
                _ => None,
            };

        // As per our proto conventions, an empty list means "all":
        let segments_of_interest = (!segment_ids.is_empty()).then_some(segment_ids.as_slice());

        let chunk_stores = self
            .get_chunk_stores(entry_id, segments_of_interest)
            .await?;

        if chunk_stores.is_empty() {
            let stream = futures::stream::iter([{
                let batch = QueryDatasetDataframe::empty_record_batch();
                let data = Some(batch.into());
                Ok(QueryDatasetResponse { data })
            }]);

            return Ok(tonic::Response::new(
                Box::pin(stream) as Self::QueryDatasetStream
            ));
        }

        // Compute the union of timelines across every (segment, layer) touched by this query, so
        // every response we emit below carries the same `{timeline}:start` columns and the client
        // can concatenate them. Individual responses fill in `None` for timelines their chunks
        // don't contain.
        let all_timelines: BTreeMap<String, arrow::datatypes::DataType> = chunk_stores
            .iter()
            .flat_map(|(_, _, _, resolved)| {
                resolved
                    .schema()
                    .timelines()
                    .into_values()
                    .map(|tl| (tl.name().as_str().to_owned(), tl.datatype()))
                    .collect::<Vec<_>>()
            })
            .collect();

        let stream = futures::stream::iter(chunk_stores.into_iter().map(
            move |(segment_id, layer_name, store_slot_id, resolved)| {
                // Build metadata for all relevant chunks (physical + virtual).

                let metadata_vec: Vec<ChunkMetadata> = if let Some(query) = &query {
                    // RR-4355: per-segment index values pushdown.
                    //
                    // When the request carries `per_segment_values`, fan out
                    // `get_chunks_for_query_results` once per value for this
                    // segment with a synthesized latest-at, then dedup. Per
                    // the proto contract (`cloud.proto`):
                    //   "An empty values list for a segment means no temporal
                    //    chunks are returned for that segment (only static
                    //    data)."
                    // For the empty case we run a single static-only query
                    // instead of returning nothing, so static chunks still
                    // surface.
                    let (chunks, missing_virtual) = if let Some(map) = &per_segment_index_values {
                        if let Some(values) = map.get(&segment_id) {
                            let synthesized: Vec<re_log_types::TimeInt> = if values.is_empty() {
                                vec![re_log_types::TimeInt::STATIC]
                            } else {
                                values.clone()
                            };
                            let mut all_chunks: Vec<Arc<Chunk>> = Vec::new();
                            let mut all_missing: BTreeSet<ChunkId> = BTreeSet::new();
                            let mut seen: BTreeSet<ChunkId> = BTreeSet::new();
                            for v in &synthesized {
                                let mut q = query.clone();
                                if let Some(la) = q.latest_at.as_mut() {
                                    la.at = *v;
                                    la.per_segment_values = Vec::new();
                                }
                                let (cs, missing) = get_chunks_for_query_results(
                                    &resolved,
                                    &entity_paths,
                                    select_all_entity_paths,
                                    &q,
                                );
                                for c in cs {
                                    if seen.insert(c.id()) {
                                        all_chunks.push(c);
                                    }
                                }
                                all_missing.extend(missing);
                            }
                            for id in &seen {
                                all_missing.remove(id);
                            }
                            (all_chunks, all_missing.into_iter().collect())
                        } else {
                            (Vec::new(), Vec::new())
                        }
                    } else {
                        get_chunks_for_query_results(
                            &resolved,
                            &entity_paths,
                            select_all_entity_paths,
                            query,
                        )
                    };

                    let mut metas: Vec<_> = chunks
                        .iter()
                        .map(|c| ChunkMetadata::from_chunk(c))
                        .collect();
                    if let ResolvedStore::Lazy(lazy) = &resolved {
                        for chunk_id in &missing_virtual {
                            if let Some(idx) = lazy.chunk_row_index(chunk_id) {
                                metas.push(ChunkMetadata::from_manifest(
                                    lazy.manifest(),
                                    *chunk_id,
                                    idx,
                                    lazy.timeline_ranges().get(chunk_id),
                                ));
                            }
                        }
                    }
                    metas
                } else {
                    match &resolved {
                        ResolvedStore::Eager(h) => h
                            .read()
                            .iter_physical_chunks()
                            .map(|c| ChunkMetadata::from_chunk(c))
                            .collect(),
                        ResolvedStore::Lazy(lazy) => lazy
                            .manifest()
                            .col_chunk_ids()
                            .iter()
                            .enumerate()
                            .map(|(idx, &chunk_id)| {
                                ChunkMetadata::from_manifest(
                                    lazy.manifest(),
                                    chunk_id,
                                    idx,
                                    lazy.timeline_ranges().get(&chunk_id),
                                )
                            })
                            .collect(),
                    }
                };

                let num_chunks = metadata_vec.len();

                let mut chunk_ids = Vec::with_capacity(num_chunks);
                let mut chunk_segment_ids = Vec::with_capacity(num_chunks);
                let mut chunk_keys = Vec::with_capacity(num_chunks);
                let mut chunk_entity_path = Vec::with_capacity(num_chunks);
                let mut chunk_is_static = Vec::with_capacity(num_chunks);
                let mut chunk_byte_sizes = Vec::with_capacity(num_chunks);
                let mut chunk_byte_sizes_uncompressed = Vec::with_capacity(num_chunks);
                let mut chunk_direct_urls = Vec::with_capacity(num_chunks);
                let mut chunk_direct_url_expiry = Vec::with_capacity(num_chunks);

                // Seed with the full set of timelines the query can see so the response schema
                // matches every other response in this stream, even for segments/layers whose
                // chunks don't use all those timelines.
                let mut timelines: BTreeMap<
                    String,
                    (arrow::datatypes::DataType, Vec<Option<i64>>),
                > = all_timelines
                    .iter()
                    .map(|(name, dtype)| {
                        (
                            name.clone(),
                            (dtype.clone(), Vec::with_capacity(num_chunks)),
                        )
                    })
                    .collect();

                for meta in &metadata_vec {
                    if !select_all_entity_paths && !entity_paths.contains(&meta.entity_path) {
                        continue;
                    }

                    if !requested_chunk_ids.is_empty()
                        && !requested_chunk_ids.contains(&meta.chunk_id)
                    {
                        continue;
                    }

                    // Filter by static/temporal data
                    if exclude_static_data && meta.is_static {
                        continue;
                    }
                    if exclude_temporal_data && !meta.is_static {
                        continue;
                    }

                    let mut missing_timelines: BTreeSet<String> =
                        timelines.keys().cloned().collect();
                    for (timeline_name, range) in &meta.timelines {
                        let timeline_name = timeline_name.as_str();
                        missing_timelines.remove(timeline_name);

                        let timeline_data = timelines
                            .get_mut(timeline_name)
                            .expect("timeline was pre-seeded from chunk stores");

                        timeline_data.1.push(Some(range.min().as_i64()));
                    }
                    for timeline_name in missing_timelines {
                        let timeline_data = timelines
                            .get_mut(&timeline_name)
                            .expect("timeline_names already checked");

                        timeline_data.1.push(None);
                    }

                    chunk_segment_ids.push(segment_id.clone());
                    chunk_ids.push(meta.chunk_id);
                    chunk_entity_path.push(meta.entity_path.clone());
                    chunk_is_static.push(meta.is_static);
                    chunk_byte_sizes.push(meta.byte_size);
                    // OSS server stores decoded data, so compressed == uncompressed.
                    chunk_byte_sizes_uncompressed.push(Some(meta.byte_size));

                    chunk_keys.push(
                        ChunkKey {
                            chunk_id: meta.chunk_id,
                            store_slot_id,
                        }
                        .encode()?,
                    );

                    chunk_direct_urls.push(None);
                    chunk_direct_url_expiry.push(None);
                }

                let chunk_layer_names = vec![layer_name.clone(); chunk_ids.len()];
                let chunk_key_refs = chunk_keys.iter().map(|v| v.as_slice()).collect();
                let batch = QueryDatasetResponse::create_dataframe_with_timelines(
                    chunk_ids,
                    chunk_segment_ids,
                    chunk_layer_names,
                    chunk_key_refs,
                    chunk_entity_path,
                    chunk_is_static,
                    chunk_byte_sizes,
                    chunk_byte_sizes_uncompressed,
                    chunk_direct_urls,
                    chunk_direct_url_expiry,
                    &timelines,
                )
                .map_err(|err| {
                    tonic::Status::internal(format!("Failed to create dataframe: {err:#}"))
                })?;

                let data = Some(batch.into());

                Ok(QueryDatasetResponse { data })
            },
        ));

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::QueryDatasetStream
        ))
    }

    type FetchChunksStream = FetchChunksResponseStream;

    // NOTE: OSS server does not detect source drift (a registered rrd file
    // being mutated after registration) which Rerun Hub implements.
    // Consider if worth having parity (RR-4577).
    async fn fetch_chunks(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::FetchChunksRequest>,
    ) -> tonic::Result<tonic::Response<Self::FetchChunksStream>> {
        // worth noting that FetchChunks is not per-dataset request, it simply contains chunk infos
        let request = request.into_inner();

        let mut chunk_keys = vec![];
        for chunk_info_data in request.chunk_infos {
            let chunk_info_batch: RecordBatch = chunk_info_data.try_into().map_err(|err| {
                tonic::Status::internal(format!("Failed to decode chunk_info: {err:#}"))
            })?;

            let schema = chunk_info_batch.schema();

            let chunk_key_col_idx = schema
                .column_with_name(FetchChunksRequest::FIELD_CHUNK_KEY)
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "Missing {} column",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?
                .0;

            let chunk_keys_arr = chunk_info_batch
                .column(chunk_key_col_idx)
                .as_any()
                .downcast_ref::<BinaryArray>()
                .ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "{} must be binary array",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?;

            for chunk_key in chunk_keys_arr {
                let chunk_key = chunk_key.ok_or_else(|| {
                    tonic::Status::invalid_argument(format!(
                        "{} must not be null",
                        FetchChunksRequest::FIELD_CHUNK_KEY
                    ))
                })?;

                let chunk_key = ChunkKey::decode(chunk_key)?;
                chunk_keys.push(chunk_key);
            }
        }

        let chunks = self
            .store
            .read()
            .await
            .chunks_from_chunk_keys(&chunk_keys)?;

        let stream = futures::stream::iter(chunks).map(|(store_id, chunk)| {
            let arrow_msg = re_log_types::ArrowMsg {
                chunk_id: *chunk.id(),
                batch: chunk.to_record_batch().map_err(|err| {
                    tonic::Status::internal(format!(
                        "failed to convert chunk to record batch: {err:#}"
                    ))
                })?,
                on_release: None,
            };

            let compression = re_log_encoding::Compression::Off;

            let encoded_chunk = arrow_msg
                .to_transport((store_id, compression))
                .map_err(|err| tonic::Status::internal(format!("encoding failed: {err:#}")))?;

            Ok(re_protos::cloud::v1alpha1::FetchChunksResponse {
                chunks: vec![encoded_chunk],
            })
        });

        Ok(tonic::Response::new(
            Box::pin(stream) as Self::FetchChunksStream
        ))
    }

    // --- Table APIs ---

    async fn register_table(
        &self,
        request: tonic::Request<RegisterTableRequest>,
    ) -> tonic::Result<tonic::Response<RegisterTableResponse>> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = request;
            return Err(tonic::Status::unimplemented(
                "register_table is not supported on wasm",
            ));
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            #[cfg_attr(not(feature = "lance"), expect(unused_mut))]
            let mut store = self.store.write().await;
            let request = request.into_inner();
            let Some(provider_details) = request.provider_details else {
                return Err(tonic::Status::invalid_argument("Missing provider details"));
            };
            #[cfg_attr(not(feature = "lance"), expect(unused_variables))]
            let lance_table = match ProviderDetails::try_from(&provider_details) {
                Ok(ProviderDetails::LanceTable(lance_table)) => lance_table.table_url,
                Ok(ProviderDetails::SystemTable(_)) => Err(Status::invalid_argument(
                    "System tables cannot be registered",
                ))?,
                Err(err) => return Err(err.into()),
            }
            .to_file_path()
            .map_err(|()| tonic::Status::invalid_argument("Invalid lance table path"))?;

            #[cfg(feature = "lance")]
            let entry_id = {
                let named_path = NamedPath {
                    name: Some(request.name.clone()),
                    path: lance_table,
                };

                store
                    .load_directory_as_table(&named_path, IfDuplicateBehavior::Error)
                    .await?
            };

            #[cfg(not(feature = "lance"))]
            let entry_id = EntryId::new();

            let table_entry = store
                .table(entry_id)
                .ok_or_else(|| Status::internal("table missing that was just registered"))?
                .as_table_entry();

            let response = RegisterTableResponse {
                table_entry: Some(table_entry.try_into()?),
            };

            self.notify(watch_events_response::Kind::EntryCreated(
                EntryCreatedEvent {
                    id: Some(entry_id.into()),
                },
            ));

            Ok(response.into())
        }
    }

    async fn get_table_schema(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::GetTableSchemaRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::GetTableSchemaResponse>> {
        let store = self.store.read().await;
        let Some(entry_id) = request.into_inner().table_id else {
            return Err(Status::not_found("Table ID not specified in request"));
        };
        let entry_id = entry_id.try_into()?;

        let table = store
            .table(entry_id)
            .ok_or_else(|| Status::not_found(format!("Entry with ID {entry_id} not found")))?;

        let schema = table.schema();

        Ok(tonic::Response::new(
            re_protos::cloud::v1alpha1::GetTableSchemaResponse {
                schema: Some(schema.as_ref().try_into().map_err(|err| {
                    Status::internal(format!("Unable to serialize Arrow schema: {err:#}"))
                })?),
            },
        ))
    }

    type ScanTableStream = ScanTableResponseStream;

    async fn scan_table(
        &self,
        request: tonic::Request<re_protos::cloud::v1alpha1::ScanTableRequest>,
    ) -> tonic::Result<tonic::Response<Self::ScanTableStream>> {
        let Some(entry_id) = request.into_inner().table_id else {
            return Err(Status::not_found("Table ID not specified in request"));
        };
        let entry_id = entry_id.try_into()?;

        let provider = {
            let store = self.store.read().await;
            let table = store
                .table(entry_id)
                .ok_or_else(|| Status::not_found(format!("Entry with ID {entry_id} not found")))?;
            table.provider()
        };

        let ctx = SessionContext::default();
        let plan = provider
            .scan(&ctx.state(), None, &[], None)
            .await
            .map_err(|err| Status::internal(format!("failed to scan table: {err:#}")))?;

        let stream = plan
            .execute(0, ctx.task_ctx())
            .map_err(|err| tonic::Status::from_error(Box::new(err)))?;

        let resp_stream = stream.map(|batch| {
            let batch = batch.map_err(|err| tonic::Status::from_error(Box::new(err)))?;
            Ok(ScanTableResponse {
                dataframe_part: Some(batch.into()),
            })
        });

        Ok(tonic::Response::new(
            Box::pin(resp_stream) as Self::ScanTableStream
        ))
    }

    // --- Tasks service ---

    async fn query_tasks(
        &self,
        request: tonic::Request<QueryTasksRequest>,
    ) -> tonic::Result<tonic::Response<QueryTasksResponse>> {
        let task_ids = request.into_inner().ids;
        let store = self.store.read().await;

        let mut ids = Vec::with_capacity(task_ids.len());
        let mut exec_statuses = Vec::with_capacity(task_ids.len());
        let mut msgs = Vec::with_capacity(task_ids.len());

        for task_id in task_ids {
            // Look up the task in the registry, falling back to success for unknown IDs
            // (including legacy dummy IDs and stale task IDs)
            let result = store
                .task_registry()
                .get(&task_id)
                .unwrap_or_else(TaskResult::success);

            ids.push(task_id);
            exec_statuses.push(result.exec_status);
            msgs.push(if result.msgs.is_empty() {
                None
            } else {
                Some(result.msgs)
            });
        }

        let num_tasks = ids.len();
        let rb = QueryTasksDataframe {
            task_id: ids.into(),
            kind: vec![None::<String>; num_tasks].into(),
            data: vec![None::<String>; num_tasks].into(),
            exec_status: exec_statuses.into(),
            msgs: msgs.into(),
            blob_len: vec![None::<u64>; num_tasks].into(),
            lease_owner: vec![None::<String>; num_tasks].into(),
            lease_expiration: vec![None::<i64>; num_tasks].into(),
            attempts: vec![1_u8; num_tasks].into(),
            creation_time: vec![None::<i64>; num_tasks].into(),
            last_update_time: vec![None::<i64>; num_tasks].into(),
        }
        .into_record_batch()
        .map_err(|err| tonic::Status::internal(format!("Failed to create dataframe: {err:#}")))?;

        // All tasks finish immediately in the OSS server
        Ok(tonic::Response::new(QueryTasksResponse {
            data: Some(rb.into()),
        }))
    }

    type QueryTasksOnCompletionStream = QueryTasksOnCompletionResponseStream;

    async fn query_tasks_on_completion(
        &self,
        request: tonic::Request<QueryTasksOnCompletionRequest>,
    ) -> tonic::Result<tonic::Response<Self::QueryTasksOnCompletionStream>> {
        let task_ids = request.into_inner().ids;

        // All tasks finish immediately in the OSS server, so we can delegate to `query_tasks
        let response_data = self
            .query_tasks(tonic::Request::new(QueryTasksRequest { ids: task_ids }))
            .await?
            .into_inner()
            .data;

        Ok(tonic::Response::new(
            Box::pin(futures::stream::once(async move {
                Ok(QueryTasksOnCompletionResponse {
                    data: response_data,
                })
            })) as Self::QueryTasksOnCompletionStream,
        ))
    }

    async fn cancel_tasks(
        &self,
        _request: tonic::Request<CancelTasksRequest>,
    ) -> tonic::Result<tonic::Response<CancelTasksResponse>> {
        // Cancelling tasks is a noop in the OSS server
        Ok(tonic::Response::new(CancelTasksResponse {}))
    }

    async fn do_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoMaintenanceRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DoMaintenanceResponse>> {
        Err(tonic::Status::unimplemented(
            "do_maintenance not implemented",
        ))
    }

    async fn do_global_maintenance(
        &self,
        _request: tonic::Request<re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest>,
    ) -> tonic::Result<tonic::Response<re_protos::cloud::v1alpha1::DoGlobalMaintenanceResponse>>
    {
        Err(tonic::Status::unimplemented(
            "do_global_maintenance not implemented",
        ))
    }

    async fn create_table_entry(
        &self,
        request: Request<re_protos::cloud::v1alpha1::CreateTableEntryRequest>,
    ) -> tonic::Result<Response<re_protos::cloud::v1alpha1::CreateTableEntryResponse>> {
        let request: CreateTableEntryRequest = request.into_inner().try_into()?;
        let table_name = request.name;

        let schema = Arc::new(request.schema);

        #[cfg(target_arch = "wasm32")]
        let Some(details) = request.provider_details else {
            return Err(tonic::Status::unimplemented(
                "filesystem-backed table creation is not supported on wasm",
            ));
        };

        #[cfg(not(target_arch = "wasm32"))]
        let details = if let Some(details) = request.provider_details {
            details
        } else {
            // Create a directory in the storage directory. We use a tuid to avoid collisions
            // and avoid any sanitization issue with the provided table name.
            let table_path = self
                .settings
                .storage_dir
                .path()
                .join(format!("lance-{}", Tuid::new()));
            ProviderDetails::LanceTable(ext::LanceTable {
                table_url: url::Url::from_directory_path(table_path).map_err(|_err| {
                    Status::internal(format!(
                        "Failed to create table directory in {:?}",
                        self.settings.storage_dir.path()
                    ))
                })?,
            })
        };

        #[cfg(target_arch = "wasm32")]
        {
            let _ = (table_name, schema, details);
            return Err(tonic::Status::unimplemented(
                "filesystem-backed table creation is not supported on wasm",
            ));
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let table = match details {
                ProviderDetails::LanceTable(table) => {
                    self.store
                        .write()
                        .await
                        .create_table_entry(table_name, &table.table_url, schema)
                        .await?
                }
                ProviderDetails::SystemTable(_) => {
                    return Err(tonic::Status::invalid_argument(
                        "Creating system tables is not supported",
                    ));
                }
            };

            self.notify(watch_events_response::Kind::EntryCreated(
                EntryCreatedEvent {
                    id: Some(table.details.id.into()),
                },
            ));

            Ok(Response::new(
                CreateTableEntryResponse { table }.try_into()?,
            ))
        }
    }
}

/// Retrieves the entry ID based on HTTP headers.
fn get_entry_id_from_headers<T>(
    store: &InMemoryStore,
    req: &tonic::Request<T>,
) -> tonic::Result<EntryId> {
    if let Some(entry_id) = req.entry_id()? {
        Ok(entry_id)
    } else if let Some(dataset_name) = req.entry_name()? {
        Ok(store.dataset_by_name(&dataset_name)?.id())
    } else {
        const HEADERS: &[&str] = &[
            re_protos::headers::RERUN_HTTP_HEADER_ENTRY_ID,
            re_protos::headers::RERUN_HTTP_HEADER_ENTRY_NAME,
        ];
        Err(tonic::Status::invalid_argument(format!(
            "missing mandatory {HEADERS:?} HTTP headers"
        )))
    }
}

/// Return the equivalent latest at query
fn latest_at_or_static(latest_at: &ext::QueryLatestAt) -> LatestAtQuery {
    match &latest_at.index {
        Some(index) => LatestAtQuery::new(*index, latest_at.at),
        None => LatestAtQuery::new_static(),
    }
}

/// Metadata for a single chunk, extractable from either a physical `Chunk` or a manifest.
struct ChunkMetadata {
    chunk_id: ChunkId,
    entity_path: EntityPath,
    is_static: bool,
    byte_size: u64,
    timelines: IntMap<TimelineName, AbsoluteTimeRange>,
}

impl ChunkMetadata {
    fn from_chunk(chunk: &Chunk) -> Self {
        let timelines = chunk
            .timelines()
            .values()
            .map(|col| (*col.timeline().name(), col.time_range()))
            .collect();
        Self {
            chunk_id: chunk.id(),
            entity_path: chunk.entity_path().clone(),
            is_static: chunk.is_static(),
            byte_size: re_byte_size::SizeBytes::total_size_bytes(chunk),
            timelines,
        }
    }

    fn from_manifest(
        manifest: &re_log_encoding::RrdManifest,
        chunk_id: ChunkId,
        row_idx: usize,
        chunk_timelines: Option<&IntMap<TimelineName, AbsoluteTimeRange>>,
    ) -> Self {
        Self {
            chunk_id,
            entity_path: EntityPath::from(manifest.col_chunk_entity_path_raw().value(row_idx)),
            is_static: manifest.col_chunk_is_static_raw().value(row_idx),
            byte_size: manifest.col_chunk_byte_size_uncompressed()[row_idx],
            timelines: chunk_timelines.cloned().unwrap_or_default(),
        }
    }
}

/// Returns physical chunks and missing virtual chunk IDs for a query.
fn get_chunks_for_query_results(
    resolved: &ResolvedStore,
    entity_paths: &IntSet<EntityPath>,
    select_all_entity_paths: bool,
    query: &ext::Query,
) -> (Vec<Arc<Chunk>>, Vec<ChunkId>) {
    // Contract: a Query with neither `latest_at` nor `range` means "all chunks", regardless of
    // entity filter. This is exercised by the shared `re_redap_tests::query_dataset` "default" test
    // case.
    if query.latest_at.is_none() && query.range.is_none() {
        return match resolved {
            ResolvedStore::Eager(h) => (h.read().iter_physical_chunks().cloned().collect(), vec![]),
            ResolvedStore::Lazy(lazy) => (vec![], lazy.manifest().col_chunk_ids().to_vec()),
        };
    }

    let paths = if select_all_entity_paths {
        resolved.all_entities()
    } else if entity_paths.is_empty() {
        // Per `cloud.proto`: `(select_all_entity_paths=false, entity_paths=[])`
        // is a valid query that selects no entities and yields no results.
        return (Vec::new(), Vec::new());
    } else {
        entity_paths.clone()
    };

    let mut all_chunks: Vec<Arc<Chunk>> = vec![];
    let mut all_missing: BTreeSet<ChunkId> = BTreeSet::new();
    let mut seen_physical: BTreeSet<ChunkId> = BTreeSet::new();

    for entity_path in &paths {
        if let Some(latest_at) = &query.latest_at {
            let latest_at_q = latest_at_or_static(latest_at);
            let results = resolved.latest_at_relevant_chunks_for_all_components(
                ChunkTrackingMode::Report,
                &latest_at_q,
                entity_path,
                true,
            );
            for chunk in results.chunks {
                if seen_physical.insert(chunk.id()) {
                    all_chunks.push(chunk);
                }
            }
            all_missing.extend(results.missing_virtual);
        }
        if let Some(range) = &query.range {
            let range_q = RangeQuery::new(range.index, range.index_range);
            let results = resolved.range_relevant_chunks_for_all_components(
                ChunkTrackingMode::Report,
                &range_q,
                entity_path,
                true,
            );
            for chunk in results.chunks {
                if seen_physical.insert(chunk.id()) {
                    all_chunks.push(chunk);
                }
            }
            // Range tightening for virtual chunks. `range_relevant_chunks_for_all_components`
            // post-filters physical chunks against the per-chunk timeline range, but the
            // start-time-indexed scan that produces `missing_virtual` can pull in chunks
            // whose actual time range falls outside the query (the index lookup widens by
            // the longest chunk interval). Without this drop, lazy stores leak those
            // chunks to the client and rows outside the requested range show up in the
            // result set. Latest-at is unaffected because it doesn't fan out via
            // `missing_virtual` here.
            for chunk_id in results.missing_virtual {
                let keep = match resolved {
                    // Eager stores already went through the physical post-filter above.
                    ResolvedStore::Eager(_) => true,
                    ResolvedStore::Lazy(lazy) => match lazy.timeline_ranges().get(&chunk_id) {
                        // No temporal entry => static chunk; let it through (matches the
                        // `chunk.is_static() && include_static` branch of the physical filter).
                        None => true,
                        Some(per_timeline) => per_timeline
                            .get(&range.index)
                            .is_some_and(|time_range| time_range.intersects(range.index_range)),
                    },
                };
                if keep {
                    all_missing.insert(chunk_id);
                }
            }
        }
    }

    // Remove any virtual IDs that turned out to be physical in another entity's result.
    for id in &seen_physical {
        all_missing.remove(id);
    }

    (all_chunks, all_missing.into_iter().collect())
}

/// Streams `num_bytes` of pseudo-random (incompressible) bytes back to the client,
/// split into ~1 MiB chunks.
fn bandwidth_test_stream(
    num_bytes: u64,
) -> impl futures::Stream<Item = tonic::Result<DoBandwidthTestResponse>> + Send {
    futures::stream::iter(ext::BandwidthTestPayloadIter::new(num_bytes).map(Ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    use futures::TryStreamExt as _;
    use re_protos::cloud::v1alpha1::GetAssetsForSegmentRequest;
    use re_protos::headers::RerunHeadersInjectorExt as _;

    /// Datasets created before asset datasets were introduced don't have one. Querying assets on
    /// such a dataset returns no assets, and updating its entry creates the missing asset dataset.
    #[tokio::test]
    async fn legacy_dataset_without_asset_dataset() {
        let handler = RerunCloudHandlerBuilder::new().build();

        let dataset_id = EntryId::new();
        handler
            .store
            .write()
            .await
            .create_dataset_impl(
                EntryName::new("legacy_dataset").unwrap(),
                dataset_id,
                DatasetKind::Recording,
                None,
            )
            .unwrap();

        let responses: Vec<_> = handler
            .get_assets_for_segment(
                tonic::Request::new(GetAssetsForSegmentRequest {}).with_entry_id(dataset_id),
            )
            .await
            .expect("querying assets should succeed without an asset dataset")
            .into_inner()
            .try_collect()
            .await
            .unwrap();
        assert!(
            responses.is_empty(),
            "a dataset without an asset dataset should have no assets"
        );

        let updated: ext::DatasetEntry = handler
            .update_dataset_entry(tonic::Request::new(
                UpdateDatasetEntryRequest {
                    id: dataset_id,
                    dataset_details: Default::default(),
                }
                .into(),
            ))
            .await
            .expect("updating the entry should succeed")
            .into_inner()
            .dataset
            .unwrap()
            .try_into()
            .unwrap();

        let asset_dataset_id = updated
            .dataset_details
            .asset_dataset
            .expect("updating the entry should create the missing asset dataset");
        let store = handler.store.read().await;
        assert_eq!(
            store.dataset(asset_dataset_id).unwrap().dataset_kind(),
            DatasetKind::Asset,
        );
    }
}
