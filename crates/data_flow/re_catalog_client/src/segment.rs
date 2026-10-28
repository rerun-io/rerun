//! Catalog dataset, segment, and layer operations.

use std::collections::{BTreeSet, HashSet};

use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;
use tokio_stream::StreamExt as _;

use re_log_types::{EntryId, EntryIdOrName};
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::{
    ScanSegmentTableRequest, SegmentIdFilter, SegmentIdList, segment_id_filter,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::{ApiError, ApiResult, DatasetSchema, TraceId};
use re_types_core::LayerName;

use crate::CatalogClient;
use crate::DatasetSelection;
use crate::dataset_info::{DatasetInfo, DatasetInfoBuilder};

impl CatalogClient {
    /// Read one segment's mutable properties, and the revision that gates writes to them.
    ///
    /// `None` means the segment has no properties. See [`Self::set_segment_properties`].
    pub async fn get_segment_properties(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<Option<(RecordBatch, u64)>> {
        self.client()
            .await?
            .get_segment_properties(dataset_id, segment_id)
            .await
    }

    /// Replace one segment's mutable properties, returning the new revision.
    ///
    /// `properties` must be a single-row `RecordBatch` whose columns are named
    /// `property:<key>:<component>`, and it replaces every property on the segment without merging
    /// into the stored ones.
    ///
    /// With `expected_revision` set the write only lands if the stored revision matches, and fails
    /// with `FailedPrecondition` otherwise, leaving the stored properties untouched. `0` means "only
    /// if the segment has no properties yet". Without it the write overwrites whatever is stored.
    pub async fn set_segment_properties(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        properties: &RecordBatch,
        expected_revision: Option<u64>,
    ) -> ApiResult<u64> {
        self.client()
            .await?
            .set_segment_properties(dataset_id, segment_id, properties, expected_revision)
            .await
    }

    /// Drop the properties of several segments, whatever their revision.
    ///
    /// Dropping properties a segment doesn't have does nothing.
    /// Servers without this endpoint leave properties unchanged.
    pub async fn delete_segment_properties(
        &self,
        dataset_id: EntryId,
        segment_ids: Vec<SegmentId>,
    ) -> ApiResult {
        self.client()
            .await?
            .delete_segment_properties(dataset_id, segment_ids)
            .await
    }

    /// Get the IDs of the segments that `selection` keeps, in the server's order.
    ///
    /// Pass a dataset's [`EntryId`] to get all of its segments.
    /// Selected IDs that the dataset does not hold are not returned.
    //TODO(ab): is there a way — and a reason — to not collect and instead return a stream?
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_segment_ids(
        &self,
        selection: impl Into<DatasetSelection>,
    ) -> ApiResult<Vec<SegmentId>> {
        let selection = selection.into();
        if selection.selects_no_segments() {
            return Ok(Vec::new());
        }
        let segment_filter = selection.segment_filter();

        let mut request = ScanSegmentTableRequest::with_columns([
            ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID_NAME,
        ]);
        if let Some(ids) = segment_filter {
            request.segment_id_filter = Some(SegmentIdFilter {
                strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                    segment_ids: ids.iter().cloned().map(Into::into).collect(),
                })),
            });
        }

        let mut stream = self
            .client()
            .await?
            .scan_segment_table(selection.dataset_id(), request)
            .await?;
        let trace_id = stream.trace_id();

        let mut segment_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch = resp?;

            let segment_id_column = ScanSegmentTableDataframe::COLUMN_RERUN_SEGMENT_ID
                .extract(&record_batch)
                .map_err(|err| {
                    ApiError::deserialization_quiver_from(
                        self.origin(),
                        trace_id,
                        err,
                        "/ScanSegmentTable stream",
                    )
                })?;

            // Servers may ignore the filter hint, so filter locally too.
            segment_ids.extend(
                segment_id_column
                    .into_iter_owned()
                    .filter(|id| segment_filter.is_none_or(|ids| ids.contains(id))),
            );
        }

        Ok(segment_ids)
    }

    /// How many segments the given dataset entry holds.
    ///
    /// There is no count endpoint, so this counts the rows of the segment table.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_segment_count(&self, entry_id: EntryId) -> ApiResult<usize> {
        // `rerun_last_updated_at` is the narrowest column of the table, a fixed-width timestamp.
        let mut stream = self
            .client()
            .await?
            .scan_segment_table(
                entry_id,
                ScanSegmentTableRequest::with_columns([
                    ScanSegmentTableDataframe::COLUMN_RERUN_LAST_UPDATED_AT_NAME,
                ]),
            )
            .await?;

        let mut segment_count = 0;

        while let Some(resp) = stream.next().await {
            segment_count += resp?.num_rows();
        }

        Ok(segment_count)
    }

    /// An overview of a dataset, from its schemas and a projected scan of its segment table.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_info(&self, dataset_id: EntryId) -> ApiResult<DatasetInfo> {
        let mut client = self.client().await?;
        let segment_table_schema = client.get_segment_table_schema(dataset_id).await?;
        let dataset_schema = client.get_dataset_schema(dataset_id).await?.schema;

        let mut builder = DatasetInfoBuilder::new(&segment_table_schema);
        let request = ScanSegmentTableRequest::with_columns(builder.columns());
        let mut stream = client.scan_segment_table(dataset_id, request).await?;
        let trace_id = stream.trace_id();

        while let Some(batch) = stream.next().await {
            builder.add_batch(self.origin(), trace_id, &batch?)?;
        }

        builder.finish(self.origin(), &dataset_schema)
    }

    /// The union of the schemas of every registered layer of every segment in the dataset.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_schema(&self, dataset_id: EntryId) -> ApiResult<DatasetSchema> {
        self.client().await?.get_dataset_schema(dataset_id).await
    }

    /// The union of the schemas of a segment's registered layers.
    ///
    /// With `include_assets`, the schemas of the assets that apply to the segment are merged in.
    ///
    /// The order of the fields is unspecified.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_schema(
        &self,
        dataset_id: EntryId,
        segment_id: SegmentId,
        include_assets: bool,
    ) -> ApiResult<ArrowSchema> {
        let mut client = self.client().await?;

        let Some(segment_schema) = client
            .get_segments_schema(dataset_id, &BTreeSet::from([segment_id.clone()]))
            .await?
        else {
            return Err(ApiError::not_found(
                self.origin(),
                format!("segment '{segment_id}' not found in dataset {dataset_id}"),
            ));
        };
        if !include_assets {
            return Ok(segment_schema);
        }

        let Some((asset_dataset, asset_ids)) = client
            .get_assets_for_segment(dataset_id, Some(segment_id))
            .await?
        else {
            return Ok(segment_schema);
        };
        let asset_ids: BTreeSet<_> = asset_ids.into_iter().collect();
        if asset_ids.is_empty() {
            return Ok(segment_schema);
        }

        let Some(asset_schema) = client
            .get_segments_schema(asset_dataset, &asset_ids)
            .await?
        else {
            return Ok(segment_schema);
        };

        re_arrow_util::merge_schema_fields([segment_schema, without_properties(asset_schema)])
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    self.origin(),
                    None,
                    err,
                    "failed merging segment and asset schemas",
                )
            })
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
                        self.origin(),
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
            self.origin(),
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
            ApiError::deserialization_quiver_from(self.origin(), trace_id, err, "/ScanSegmentTable")
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
            self.origin(),
            re_error::format_with_details(
                "Segment not found",
                format!("Dataset: {dataset:?}\nSegment: {segment_id}"),
            ),
        )
        .with_trace_id(trace_id))
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
                        self.origin(),
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
            ApiError::deserialization_quiver_from(self.origin(), trace_id, err, "/ScanSegmentTable")
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
    /// callers can use [`re_redap_client::ConnectionClient::wait_for_tasks`] to wait for completion.
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
}

/// `schema` without the columns of property entities.
fn without_properties(schema: ArrowSchema) -> ArrowSchema {
    let fields: Vec<_> = schema
        .fields()
        .iter()
        .filter(|field| {
            let entity_path = re_sorbet::ColumnDescriptor::try_from_arrow_field(None, field)
                .ok()
                .and_then(|column| column.entity_path().cloned());
            !entity_path.is_some_and(|entity_path| entity_path.is_property())
        })
        .cloned()
        .collect();
    ArrowSchema::new_with_metadata(fields, schema.metadata)
}
