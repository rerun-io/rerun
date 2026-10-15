//! The high-level Rust client for a catalog origin.
//!
//! Dataset and segment operations take explicit entry selectors and all other required parameters.

use std::collections::HashSet;

use arrow::datatypes::Schema as ArrowSchema;

use re_log_types::{EntryId, EntryIdOrName, EntryName};
use re_protos::cloud::v1alpha1::ext::{
    DataSource, RegisterWithDatasetRequest, ScanSegmentTableDataframe,
};
use re_protos::cloud::v1alpha1::{
    ScanSegmentTableRequest, SegmentIdFilter, SegmentIdList, segment_id_filter,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_types_core::LayerName;
use tokio_stream::StreamExt as _;

use crate::dataset_info::{DatasetInfo, DatasetInfoBuilder};
use crate::registration_handle::parse_task_descriptors;
use crate::{
    ApiError, ApiResult, Asset, AssetRegistrationError, Connection, ConnectionClient,
    ConnectionRegistryHandle, RegistrationHandle, TraceId, extract_trace_id,
};

/// High-level Rust client for one catalog origin.
///
/// Dataset and segment operations take explicit entry selectors rather than resource handles.
///
/// The registry owns shared connection and credential state. This handle supplies the origin and
/// owns origin-sensitive workflows, such as registration. Use [`Self::client`] to acquire a
/// short-lived [`ConnectionClient`] for individual RPCs.
#[derive(Clone)]
pub struct ConnectionHandle {
    origin: re_uri::Origin,
    registry: ConnectionRegistryHandle,
}

impl ConnectionHandle {
    pub fn new(registry: ConnectionRegistryHandle, origin: re_uri::Origin) -> Self {
        Self { origin, registry }
    }

    pub async fn connection(&self) -> ApiResult<Connection> {
        self.registry.connection(self.origin.clone()).await
    }

    pub async fn client(&self) -> ApiResult<ConnectionClient> {
        Ok(self.connection().await?.client)
    }

    /// The dataset-wide schema, which may retain fields from removed layers.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_schema(&self, dataset_id: EntryId) -> ApiResult<ArrowSchema> {
        self.client().await?.get_dataset_schema(dataset_id).await
    }

    /// The union of the schemas of a segment's registered layers, without referenced assets.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_schema(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<ArrowSchema> {
        // TODO(RR-5928): Fetch the segment schema without the full chunk manifest.
        let manifest = self
            .client()
            .await?
            .get_rrd_manifest(dataset_id, segment_id)
            .await?;
        Ok(manifest.sorbet_schema)
    }

    /// Validate that a segment exists without fetching its layer metadata.
    // TODO(RR-5930): Replace the projected scan with a dedicated segment-existence RPC.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn ensure_segment_exists(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult {
        let mut request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
        ]);
        request.segment_id_filter = Some(SegmentIdFilter {
            strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: vec![segment_id.clone().into()],
            })),
        });
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(dataset_id, request)
            .await?;
        let trace_id = stream.trace_id();

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            let ids = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch)
                .map_err(|err| {
                    ApiError::deserialization_quiver_from(
                        &self.origin,
                        trace_id,
                        err,
                        "/ScanSegmentTable",
                    )
                })?;
            // Servers may ignore the filter hint, so match the ID locally.
            if ids.iter().any(|id| id == segment_id.as_str()) {
                return Ok(());
            }
        }

        Err(ApiError::not_found(
            &self.origin,
            re_error::format_with_details(
                "Segment not found",
                format!("Dataset: {dataset_id}\nSegment: {segment_id}"),
            ),
        )
        .with_trace_id(trace_id))
    }

    /// Read a segment's layer names from the segment table.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_layers(
        &self,
        dataset: impl Into<EntryIdOrName>,
        segment_id: SegmentId,
    ) -> ApiResult<HashSet<LayerName>> {
        let dataset = dataset.into();
        let mut request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES_NAME,
        ]);
        request.segment_id_filter = Some(SegmentIdFilter {
            strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: vec![segment_id.clone().into()],
            })),
        });
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(dataset.clone(), request)
            .await?;
        let trace_id = stream.trace_id();
        let decode_failed = |err| {
            ApiError::deserialization_quiver_from(&self.origin, trace_id, err, "/ScanSegmentTable")
        };

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            let ids = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch)
                .map_err(decode_failed)?;
            let layers = ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES
                .extract(&batch)
                .map_err(decode_failed)?;

            // Servers may ignore the filter hint, so match the ID locally.
            for (id, layers) in std::iter::zip(ids.iter_owned(), layers.iter_owned()) {
                if id == segment_id {
                    return Ok(layers.into_iter().collect());
                }
            }
        }

        Err(ApiError::not_found(
            &self.origin,
            re_error::format_with_details(
                "Segment not found",
                format!("Dataset: {dataset:?}\nSegment: {segment_id}"),
            ),
        )
        .with_trace_id(trace_id))
    }

    /// An overview of a dataset, from its schemas and a projected scan of its segment table.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_info(&self, dataset_id: EntryId) -> ApiResult<DatasetInfo> {
        let mut client = self.client().await?;
        let segment_table_schema = client.get_segment_table_schema(dataset_id).await?;
        let dataset_schema = client.get_dataset_schema(dataset_id).await?;

        let mut builder = DatasetInfoBuilder::new(&segment_table_schema);
        let request = ScanSegmentTableRequest::with_columns(builder.columns());
        let mut stream = client.scan_segment_table(dataset_id, request).await?;
        let trace_id = stream.trace_id();

        while let Some(batch) = stream.next().await {
            builder.add_batch(&self.origin, trace_id, &batch?)?;
        }

        builder.finish(&self.origin, &dataset_schema)
    }

    /// Discover distinct layer names from current segment-table metadata.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_layers(&self, dataset_id: EntryId) -> ApiResult<HashSet<LayerName>> {
        let request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES_NAME,
        ]);
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(dataset_id, request)
            .await?;
        let trace_id = stream.trace_id();
        let mut names = HashSet::new();

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            let layers = ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES
                .extract(&batch)
                .map_err(|err| {
                    ApiError::deserialization_quiver_from(
                        &self.origin,
                        trace_id,
                        err,
                        "/ScanSegmentTable",
                    )
                })?;
            for layers in layers.iter_owned() {
                names.extend(layers);
            }
        }

        Ok(names)
    }

    /// Read the IDs of segments currently containing this layer.
    ///
    /// A missing layer returns an empty list; a missing dataset remains an error.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_layer_segment_ids(
        &self,
        dataset_id: EntryId,
        layer: LayerName,
    ) -> ApiResult<Vec<SegmentId>> {
        let request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
            ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES_NAME,
        ]);
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(dataset_id, request)
            .await?;
        let trace_id = stream.trace_id();
        let decode_failed = |err| {
            ApiError::deserialization_quiver_from(&self.origin, trace_id, err, "/ScanSegmentTable")
        };
        let mut segment_ids = Vec::new();

        while let Some(batch) = stream.next().await {
            let batch = batch?;
            let ids = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&batch)
                .map_err(decode_failed)?;
            let layers = ScanSegmentTableDataframe::COLUMN_RERUN_LAYER_NAMES
                .extract(&batch)
                .map_err(decode_failed)?;

            for (id, layers) in std::iter::zip(ids.iter_owned(), layers.iter_owned()) {
                if layers.contains(&layer) {
                    segment_ids.push(id);
                }
            }
        }

        Ok(segment_ids)
    }

    /// Unregister exactly one segment/layer pair without deleting its source objects.
    ///
    /// Missing segments or layers are a no-op. Returns the request trace ID and task IDs;
    /// callers can use [`ConnectionClient::wait_for_tasks`] to wait for completion.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn delete_segment_layer(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        layer: LayerName,
    ) -> ApiResult<(Option<TraceId>, Vec<TaskId>)> {
        self.client()
            .await?
            .unregister_from_dataset(dataset_id, vec![segment_id], vec![layer], false)
            .await
    }

    /// Scan an asset dataset's manifest, one [`Asset`] per asset, sorted by asset id.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_asset_dataset(&self, asset_dataset: EntryId) -> ApiResult<Vec<Asset>> {
        let batches = self
            .client()
            .await?
            .scan_dataset_manifest(asset_dataset, crate::asset::ASSET_COLUMNS)
            .await?;

        crate::asset::assets_from_manifest(&self.origin, &batches)
    }

    /// Register an existing `.rrd` that the server can reach as an asset of `dataset_id`, and wait
    /// for the registration to finish.
    ///
    /// Registering an asset that is already registered replaces it.
    ///
    /// The server has restrictions on what an asset can contain; See
    /// [`re_protos::common::v1alpha1::ext::DatasetKind::limits`].
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_asset(
        &self,
        dataset_id: EntryId,
        asset_uri: impl AsRef<str>,
        timeout: std::time::Duration,
    ) -> Result<SegmentId, AssetRegistrationError> {
        let data_source = crate::asset_data_source(self.origin(), asset_uri)
            .map_err(AssetRegistrationError::without_asset)?;

        let asset_dataset = self
            .client()
            .await
            .map_err(AssetRegistrationError::without_asset)?
            .ensure_asset_dataset(dataset_id)
            .await
            .map_err(AssetRegistrationError::without_asset)?;

        let registration = self
            .register_with_dataset(
                asset_dataset,
                vec![data_source],
                IfDuplicateBehavior::Overwrite,
            )
            .await
            .map_err(AssetRegistrationError::without_asset)?;

        // The server writes the asset's row before it reads the `.rrd`, so its id is known even
        // when reading it fails further down.
        let asset_id = registration
            .descriptors()
            .first()
            .map(|descriptor| descriptor.segment_id.clone());

        let registered = registration
            .wait(timeout)
            .await
            .map_err(|err| AssetRegistrationError::new(asset_id.clone(), err))?
            .into_iter()
            .next();

        registered.ok_or_else(|| {
            AssetRegistrationError::new(
                asset_id,
                ApiError::internal(
                    self.origin(),
                    "the server registered the asset but returned no segment",
                ),
            )
        })
    }

    /// Unregister an asset previously registered with [`Self::register_asset`], and wait for the
    /// unregistration to finish.
    ///
    /// The server drops only the layers of the asset it is done registering. `force` drops them
    /// whatever their status, which is the only way to clear an asset whose registration failed.
    ///
    /// Unregistering an asset that isn't registered does nothing.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn unregister_asset(
        &self,
        dataset_id: EntryId,
        asset_segment_id: SegmentId,
        force: bool,
        timeout: std::time::Duration,
    ) -> ApiResult {
        let mut client = self.client().await?;

        // No asset dataset means no asset was ever registered, so there is nothing to drop.
        let Some(asset_dataset) = client.asset_dataset(dataset_id).await? else {
            return Ok(());
        };

        let (trace_id, task_ids) = client
            .unregister_from_dataset(asset_dataset, vec![asset_segment_id], vec![], force)
            .await?;

        client.wait_for_tasks(task_ids, trace_id, timeout).await
    }

    /// Initiate asynchronous registration of the provided data sources with a dataset.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_with_dataset(
        &self,
        dataset_id: EntryId,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) -> ApiResult<RegistrationHandle> {
        let req = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id);

        let response = self
            .client()
            .await?
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/RegisterWithDataset"))?;
        let trace_id = extract_trace_id(response.metadata());
        let descriptors =
            parse_task_descriptors(&self.origin, trace_id, response.into_inner().data)?;

        Ok(RegistrationHandle::new(self.clone(), trace_id, descriptors))
    }

    /// Ensure a dataset exists and register `data_sources` with it.
    ///
    /// `timeout` applies to the task-completion query.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn ensure_dataset_and_register(
        &self,
        dataset_name: &EntryName,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
        timeout: std::time::Duration,
    ) -> ApiResult<(EntryId, SegmentId)> {
        let dataset_id = self
            .client()
            .await?
            .find_or_create_dataset(dataset_name)
            .await?;
        let registration = self
            .register_with_dataset(dataset_id, data_sources, on_duplicate)
            .await?;
        let segment_id = registration
            .wait(timeout)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| {
                ApiError::invalid_arguments(
                    &self.origin,
                    "server registered the file but returned no segments",
                )
            })?;

        Ok((dataset_id, segment_id))
    }

    pub fn origin(&self) -> &re_uri::Origin {
        &self.origin
    }

    pub fn connection_registry(&self) -> &ConnectionRegistryHandle {
        &self.registry
    }
}

impl std::fmt::Debug for ConnectionHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConnectionHandle")
            .field("origin", &self.origin)
            .finish_non_exhaustive()
    }
}
