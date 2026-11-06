use std::collections::BTreeSet;
use std::num::NonZeroU64;

use arrow::array::RecordBatch;
use arrow::datatypes::Schema as ArrowSchema;
use re_log_types::{EntryId, EntryIdOrName};
use re_protos::EntryName;
use re_protos::cloud::v1alpha1::ext::{
    CreateDatasetEntryResponse, DataSource, DatasetDetails, DatasetEntry, ReadDatasetEntryResponse,
    RegisterWithDatasetRequest, RegisterWithDatasetTaskDescriptor, UnregisterFromDatasetRequest,
    UpdateDatasetEntryRequest, UpdateDatasetEntryResponse,
};
use re_protos::cloud::v1alpha1::{
    CreateDatasetEntryRequest, GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse,
    GetDatasetSchemaRequest, GetSegmentTableSchemaRequest, GetSegmentTableSchemaResponse,
    LayerNameFilter, LayerNameList, ReadDatasetEntryRequest, ScanDatasetManifestRequest,
    ScanSegmentTableRequest, SegmentIdFilter, SegmentIdList, layer_name_filter, segment_id_filter,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::missing_field;
use re_types_core::LayerName;
use tokio_stream::StreamExt as _;
use tonic::codegen::{Body, StdError};

use super::RedapClient;
use super::client::TonicResponseExt;
use crate::{ApiError, ApiResponseStream, ApiResult, TraceId, extract_trace_id};

/// A dataset's Arrow schema, as returned by `/GetDatasetSchema`.
pub struct DatasetSchema {
    /// The union of the schemas of every registered layer of every segment in the dataset.
    pub schema: ArrowSchema,

    /// The dataset revision the schema describes.
    ///
    /// `None` if the server does not report revisions, or if the dataset has no promoted revision
    /// yet (which the server reports as zero).
    pub revision: Option<NonZeroU64>,
}

/// Rows of a dataset's manifest, as returned by `/ScanDatasetManifest`.
#[derive(Debug)]
pub struct DatasetManifest {
    /// One row per (layer, segment), spread over as many batches as the server sends.
    ///
    /// See `ScanDatasetManifestDataframe` for the columns.
    pub batches: Vec<RecordBatch>,
}

impl<T> RedapClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body> + Clone,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Get the Arrow schema for a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_schema(&mut self, entry_id: EntryId) -> ApiResult<DatasetSchema> {
        let client = self.inner().clone();
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            crate::rpc_retry::retry(|| {
                let mut client = client.clone();
                async move {
                    client
                        .get_dataset_schema(
                            tonic::Request::new(GetDatasetSchemaRequest {}).with_entry_id(entry_id),
                        )
                        .await
                }
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetDatasetSchema"))?,
        );
        crate::dataset_revisions().observe_meta(inner.meta.as_ref());
        let revision = inner
            .meta
            .as_ref()
            .and_then(|meta| NonZeroU64::new(meta.dataset_revision));
        let schema = inner.schema().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /GetDatasetSchema response",
            )
        })?;

        Ok(DatasetSchema { schema, revision })
    }

    /// Create a new dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn create_dataset_entry(
        &mut self,
        name: EntryName,
        entry_id: Option<EntryId>,
    ) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .create_dataset_entry(CreateDatasetEntryRequest {
                    name: Some(name.to_string()),
                    id: entry_id.map(Into::into),
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/CreateDatasetEntry"))?,
        );
        let response: CreateDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /CreateDatasetEntry response",
            )
        })?;

        Ok(response.dataset)
    }

    /// Get information on a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn read_dataset_entry(&mut self, entry_id: EntryId) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .read_dataset_entry(
                    tonic::Request::new(ReadDatasetEntryRequest {}).with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/ReadDatasetEntry"))?,
        );
        let response: ReadDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /ReadDatasetEntry response",
            )
        })?;

        Ok(response.dataset_entry)
    }

    /// Update the details of a dataset entry.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> ApiResult<DatasetEntry> {
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            self.inner()
                .update_dataset_entry(
                    tonic::Request::new(
                        UpdateDatasetEntryRequest {
                            id: entry_id,
                            dataset_details,
                        }
                        .into(),
                    )
                    .with_entry_id(entry_id),
                )
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/UpdateDatasetEntry"))?,
        );
        let response: UpdateDatasetEntryResponse = inner.try_into().map_err(|err| {
            ApiError::deserialization_with_source(
                &self.origin,
                trace_id,
                err,
                "failed parsing /UpdateDatasetEntry response",
            )
        })?;

        Ok(response.dataset_entry)
    }

    //TODO(ab): accept entry name
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_segment_table_schema(&mut self, entry_id: EntryId) -> ApiResult<ArrowSchema> {
        let client = self.inner().clone();
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            crate::rpc_retry::retry(|| {
                let mut client = client.clone();
                async move {
                    client
                        .get_segment_table_schema(
                            tonic::Request::new(GetSegmentTableSchemaRequest {})
                                .with_entry_id(entry_id),
                        )
                        .await
                }
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "GetSegmentTableSchema failed"))?,
        );
        crate::dataset_revisions().observe_meta(inner.meta.as_ref());
        inner
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetSegmentTableSchemaResponse, "schema");
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "missing field in /GetSegmentTableSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetSegmentTableSchema response",
                )
            })
    }

    /// Scan the segment table with server-side column projection.
    ///
    /// The segment filter is only a hint; callers must apply it locally for correctness.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_segment_table(
        &self,
        entry: impl Into<EntryIdOrName>,
        request: ScanSegmentTableRequest,
    ) -> ApiResult<ApiResponseStream<RecordBatch>>
    where
        T: Clone,
    {
        let metadata = match entry.into() {
            EntryIdOrName::Id(id) => tonic::Request::new(()).with_entry_id(id),
            EntryIdOrName::Name(name) => {
                let name = EntryName::new(name)
                    .map_err(|err| ApiError::invalid_arguments(&self.origin, err.to_string()))?;
                tonic::Request::new(()).with_entry_name(name)
            }
        }
        .into_parts()
        .0;

        // Retry only the *open*: the server rejects `ScanSegmentTable` with `ResourceExhausted`
        // fail-fast at admission control, before the stream exists, so re-opening is idempotent.
        // Consumption by the caller is intentionally outside the retry.
        // Once the stream is open it can't return `ResourceExhausted`.
        let response = crate::with_retry_resource_exhausted("/ScanSegmentTable", || {
            let client = self.clone();
            let request = request.clone();
            let metadata = &metadata;
            async move {
                crate::rpc_retry::retry(|| {
                    let mut client = client.clone();
                    let request = request.clone();
                    async move {
                        crate::rpc_retry::open_stream(
                            client
                                .inner()
                                .scan_segment_table(
                                    tonic::Request::new(request).with_metadata(metadata),
                                )
                                .await?,
                        )
                        .await
                    }
                })
                .await
                .map_err(|err| ApiError::tonic(&self.origin, err, "/ScanSegmentTable"))
            }
        })
        .await?;

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/ScanSegmentTable",
        )
        .inspect_first(|response| {
            crate::dataset_revisions().observe_meta(response.meta.as_ref());
        });
        let trace_id = stream.trace_id();
        let origin = self.origin.clone();

        Ok(ApiResponseStream::new(
            self.origin.clone(),
            stream.map(move |response| {
                response?
                    .data()
                    .map_err(|err| {
                        ApiError::deserialization_with_source(
                            &origin,
                            trace_id,
                            err,
                            "failed parsing item from /ScanSegmentTable stream",
                        )
                    })?
                    .try_into()
                    .map_err(|err| {
                        ApiError::deserialization_with_source(
                            &origin,
                            trace_id,
                            err,
                            "failed decoding item from /ScanSegmentTable stream",
                        )
                    })
            }),
            trace_id,
        ))
    }

    /// Submit dataset registration and decode the server's task descriptors.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn register_with_dataset(
        &mut self,
        dataset_id: EntryId,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) -> ApiResult<(Option<TraceId>, Vec<RegisterWithDatasetTaskDescriptor>)> {
        let request = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id);
        let response = self
            .inner()
            .register_with_dataset(request.map(Into::into))
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/RegisterWithDataset"))?;
        let trace_id = extract_trace_id(response.metadata());
        let descriptors = crate::registration_response::parse_task_descriptors(
            &self.origin,
            trace_id,
            response.into_inner().data,
        )?;
        Ok((trace_id, descriptors))
    }

    //TODO(ab): accept entry name
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn get_dataset_manifest_schema(
        &mut self,
        entry_id: EntryId,
    ) -> ApiResult<ArrowSchema> {
        let client = self.inner().clone();
        let (inner, trace_id) = TonicResponseExt::into_inner_and_trace_id(
            crate::rpc_retry::retry(|| {
                let mut client = client.clone();
                async move {
                    client
                        .get_dataset_manifest_schema(
                            tonic::Request::new(GetDatasetManifestSchemaRequest {})
                                .with_entry_id(entry_id),
                        )
                        .await
                }
            })
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/GetDatasetManifestSchema"))?,
        );
        crate::dataset_revisions().observe_meta(inner.meta.as_ref());
        inner
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetDatasetManifestSchemaResponse, "schema");
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "missing field in /GetDatasetManifestSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::deserialization_with_source(
                    &self.origin,
                    trace_id,
                    err,
                    "failed parsing /GetDatasetManifestSchema response",
                )
            })
    }

    /// Scan a dataset's manifest, keeping only the given columns.
    ///
    /// `segment_ids` and `layer_names` are only hints to the server; callers must filter the rows
    /// locally.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn scan_dataset_manifest(
        &mut self,
        entry_id: EntryId,
        columns: impl IntoIterator<Item = impl Into<String>>,
        segment_ids: Option<&BTreeSet<SegmentId>>,
        layer_names: Option<&BTreeSet<LayerName>>,
    ) -> ApiResult<DatasetManifest> {
        let mut request = ScanDatasetManifestRequest::with_columns(columns);
        request.segment_id_filter = segment_ids.map(|segment_ids| SegmentIdFilter {
            strategy: Some(segment_id_filter::Strategy::ScanOnly(SegmentIdList {
                segment_ids: segment_ids.iter().cloned().map(Into::into).collect(),
            })),
        });
        request.layer_name_filter = layer_names.map(|layer_names| LayerNameFilter {
            strategy: Some(layer_name_filter::Strategy::ScanOnly(LayerNameList {
                layer_names: layer_names.iter().map(ToString::to_string).collect(),
            })),
        });
        let client = self.inner().clone();
        let response = crate::rpc_retry::retry(|| {
            let mut client = client.clone();
            let request = request.clone();
            async move {
                crate::rpc_retry::open_stream(
                    client
                        .scan_dataset_manifest(
                            tonic::Request::new(request.clone()).with_entry_id(entry_id),
                        )
                        .await?,
                )
                .await
            }
        })
        .await
        .map_err(|err| ApiError::tonic(&self.origin, err, "/ScanDatasetManifest"))?;

        let mut stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/ScanDatasetManifest",
        )
        .inspect_first(|response| {
            crate::dataset_revisions().observe_meta(response.meta.as_ref());
        });
        let trace_id = stream.trace_id();

        let mut batches = Vec::new();

        while let Some(response) = stream.next().await {
            let response = response?;

            let batch: RecordBatch = response
                .data()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed parsing item from /ScanDatasetManifest stream",
                    )
                })?
                .try_into()
                .map_err(|err| {
                    ApiError::deserialization_with_source(
                        &self.origin,
                        trace_id,
                        err,
                        "failed decoding item from /ScanDatasetManifest stream",
                    )
                })?;

            batches.push(batch);
        }

        Ok(DatasetManifest { batches })
    }

    /// Unregisters segments and layers from the dataset.
    ///
    /// This is an asynchronous operation, and returns a list of task ids.
    ///
    /// This method acts as a *product* filter:
    /// * empty `segments_to_drop` + empty `layers_to_drop`: invalid argument error
    /// * empty `segments_to_drop` + non-empty `layers_to_drop`: remove specified layers for *all* segments
    /// * non-empty `segments_to_drop` + empty `layers_to_drop`: remove *all* layers for specified segments
    /// * non-empty `segments_to_drop` + non-empty `layers_to_drop`: delete *all* specified layers for *all* specified segments
    ///
    /// If `force`, deletion will go through regardless of the segments/layers' current statuses.
    /// This is only useful in the very specific, catatrophic scenario where the contents of the
    /// task queue were lost and some tasks are now stuck in `status=pending` forever.
    /// Do not use this unless you know exactly what you're doing.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn unregister_from_dataset(
        &mut self,
        dataset_id: EntryId,
        segments_to_drop: Vec<SegmentId>,
        layers_to_drop: Vec<LayerName>,
        force: bool,
    ) -> ApiResult<(Option<TraceId>, Vec<TaskId>)> {
        let req = tonic::Request::new(
            UnregisterFromDatasetRequest {
                segments_to_drop,
                layers_to_drop,
                force,
            }
            .into(),
        )
        .with_entry_id(dataset_id);

        use futures::TryStreamExt as _;
        let response = self
            .inner()
            .unregister_from_dataset(req)
            .await
            .map_err(|err| ApiError::tonic(&self.origin, err, "/UnregisterFromDataset"))?;

        let trace_id = extract_trace_id(response.metadata());

        let stream = ApiResponseStream::from_tonic_response(
            self.origin.clone(),
            response,
            "/UnregisterFromDataset",
        );
        let responses: Vec<_> = stream.try_collect().await?;

        let tasks = responses
            .into_iter()
            .filter_map(|resp| resp.task_id)
            .collect();

        Ok((trace_id, tasks))
    }
}
