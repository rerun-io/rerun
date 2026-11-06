//! Catalog reads over a whole dataset or a selection of its segments.

use std::collections::{BTreeSet, HashSet};

use arrow::datatypes::Schema as ArrowSchema;
use tokio_stream::StreamExt as _;

use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::ScanSegmentTableDataframe;
use re_protos::cloud::v1alpha1::{
    DatasetQueryScope, ScanSegmentTableRequest, SegmentIdFilter, SegmentIdList, segment_id_filter,
};
use re_protos::common::v1alpha1::ext::SegmentId;
use re_redap_client::{
    ApiError, ApiErrorKind, ApiResult, ConnectionClient, DatasetSchema, OnMissingGetSchemas,
};
use re_types_core::LayerName;

use crate::CatalogClient;
use crate::DatasetSelection;
use crate::dataset_info::{DatasetInfo, DatasetInfoBuilder};

impl CatalogClient {
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
        request.segment_id_filter = segment_filter.map(scan_only);

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

    /// An overview of what `selection` keeps, from schemas and a projected scan of the segment
    /// table.
    ///
    /// Pass a dataset's [`EntryId`] for an overview of the whole dataset.
    ///
    /// A segment-filtered selection fails with [`re_redap_client::ApiErrorKind::Unimplemented`]
    /// against servers older than Rerun 0.41 or Rerun Hub 0.20.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_info(
        &self,
        selection: impl Into<DatasetSelection>,
    ) -> ApiResult<DatasetInfo> {
        let selection = selection.into();
        let dataset_id = selection.dataset_id();
        let mut client = self.client().await?;
        let segment_table_schema = client.get_segment_table_schema(dataset_id).await?;
        let schema = selected_segments_schema(&mut client, &selection, "the info").await?;

        let selects_no_segments = selection.selects_no_segments();
        let segment_id_filter = selection.segment_filter().map(scan_only);
        let mut builder = DatasetInfoBuilder::new(selection, &segment_table_schema);

        if !selects_no_segments {
            let mut request = ScanSegmentTableRequest::with_columns(builder.columns());
            request.segment_id_filter = segment_id_filter;
            let mut stream = client.scan_segment_table(dataset_id, request).await?;
            let trace_id = stream.trace_id();

            while let Some(batch) = stream.next().await {
                builder.add_batch(self.origin(), trace_id, &batch?)?;
            }
        }

        builder.finish(self.origin(), &schema)
    }

    /// The union of the schemas of every registered layer of every segment in the dataset.
    ///
    /// With [`DatasetQueryScope::Indexed`], a server that applies a chunk indexing policy returns only
    /// the indexed part; other servers return the full schema.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_schema(
        &self,
        dataset_id: EntryId,
        scope: DatasetQueryScope,
    ) -> ApiResult<DatasetSchema> {
        self.client()
            .await?
            .get_dataset_schema(dataset_id, scope)
            .await
    }

    /// The schema of the data `selection` keeps: the union of the schemas of its segments, without
    /// the component columns of the entities its content filters leave out.
    ///
    /// A segment-filtered selection fails with [`re_redap_client::ApiErrorKind::Unimplemented`]
    /// against servers older than Rerun 0.41 or Rerun Hub 0.20, and the order of its fields is
    /// unspecified.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_selection_schema(
        &self,
        selection: &DatasetSelection,
    ) -> ApiResult<ArrowSchema> {
        let mut client = self.client().await?;
        let schema = selected_segments_schema(&mut client, selection, "the schema").await?;
        Ok(with_selected_contents(schema, selection))
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
}

/// The union of the schemas of the segments `selection` keeps, whatever its content filters.
///
/// Against servers without `/GetSchemas`, a segment-filtered selection fails with an
/// [`ApiErrorKind::Unimplemented`] error that says `what` requires a newer server.
async fn selected_segments_schema(
    client: &mut ConnectionClient,
    selection: &DatasetSelection,
    what: &str,
) -> ApiResult<ArrowSchema> {
    let dataset_id = selection.dataset_id();
    let Some(segment_ids) = selection.segment_filter() else {
        return Ok(client
            .get_dataset_schema(dataset_id, DatasetQueryScope::Full)
            .await?
            .schema);
    };

    // Falling back to one `/GetRrdManifest` per segment would take minutes for a thousand
    // segments.
    let schema = client
        .get_segments_schema(dataset_id, segment_ids, OnMissingGetSchemas::Fail)
        .await
        .map_err(|mut err| {
            if err.kind == ApiErrorKind::Unimplemented {
                err.message = format!(
                    "{what} of a segment selection requires a server running Rerun 0.41 or \
                     Rerun Hub 0.20 or later"
                );
            }
            err
        })?;
    Ok(schema.unwrap_or_else(ArrowSchema::empty))
}

/// `schema` without the component columns of the entities `selection` leaves out.
fn with_selected_contents(schema: ArrowSchema, selection: &DatasetSelection) -> ArrowSchema {
    let fields: Vec<_> = schema
        .fields()
        .iter()
        .filter(|field| {
            re_sorbet::ColumnDescriptor::try_from_arrow_field(None, field)
                .ok()
                .and_then(|column| column.entity_path().cloned())
                .is_none_or(|entity_path| selection.selects_entity(&entity_path))
        })
        .cloned()
        .collect();
    ArrowSchema::new_with_metadata(fields, schema.metadata)
}

/// A segment-ID filter hint that keeps only `segment_ids`.
fn scan_only(segment_ids: &BTreeSet<SegmentId>) -> SegmentIdFilter {
    SegmentIdFilter {
        strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
            segment_ids: segment_ids.iter().cloned().map(Into::into).collect(),
        })),
    }
}
