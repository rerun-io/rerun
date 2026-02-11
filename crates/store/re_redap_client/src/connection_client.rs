use arrow::array::RecordBatch;
use arrow::datatypes::{Schema as ArrowSchema, SchemaRef};
use itertools::Itertools as _;
use re_arrow_util::ArrowArrayDowncastRef as _;
use re_log_encoding::{RawRrdManifest, ToApplication as _};
use re_log_types::EntryId;
use re_protos::cloud::v1alpha1::ext::{
    CreateDatasetEntryResponse, CreateTableEntryRequest, DataSource, DataSourceKind,
    DatasetDetails, DatasetEntry, EntryDetails, EntryDetailsUpdate, LanceTable, ProviderDetails,
    QueryDatasetRequest, QueryTasksOnCompletionRequest, QueryTasksRequest,
    ReadDatasetEntryResponse, ReadTableEntryResponse, RegisterTableResponse,
    RegisterWithDatasetRequest, RegisterWithDatasetTaskDescriptor, TableEntry, TableInsertMode,
    UnregisterFromDatasetRequest, UpdateDatasetEntryRequest, UpdateDatasetEntryResponse,
    UpdateEntryRequest, UpdateEntryResponse,
};
use re_protos::cloud::v1alpha1::rerun_cloud_service_client::RerunCloudServiceClient;
use re_protos::cloud::v1alpha1::{
    CreateDatasetEntryRequest, DeleteEntryRequest, EntryFilter, EntryKind, FetchChunksRequest,
    FindEntriesRequest, GetDatasetManifestSchemaRequest, GetDatasetManifestSchemaResponse,
    GetDatasetSchemaRequest, GetRrdManifestResponse, GetSegmentTableSchemaRequest,
    GetSegmentTableSchemaResponse, QueryDatasetResponse, QueryTasksOnCompletionResponse,
    QueryTasksResponse, ReadDatasetEntryRequest, ReadTableEntryRequest,
    RegisterWithDatasetResponse, ScanSegmentTableRequest, ScanSegmentTableResponse,
    UnregisterFromDatasetResponse, VersionRequest, WriteTableRequest,
};
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, ScanParameters, SegmentId};
use re_protos::common::v1alpha1::{DataframePart, TaskId};
use re_protos::external::prost::bytes::Bytes;
use re_protos::headers::RerunHeadersInjectorExt as _;
use re_protos::{TypeConversionError, invalid_schema, missing_column, missing_field};
use tokio_stream::{Stream, StreamExt as _};
use tonic::codegen::{Body, StdError};
use tonic::{IntoStreamingRequest as _, Status};
use url::Url;

use crate::{ApiError, ApiResult};

pub type ResponseStream<T> = std::pin::Pin<Box<dyn Stream<Item = tonic::Result<T>> + Send>>;

pub type FetchChunksResponseStream =
    ResponseStream<re_protos::cloud::v1alpha1::FetchChunksResponse>;

pub type QueryDatasetResponseStream =
    ResponseStream<re_protos::cloud::v1alpha1::QueryDatasetResponse>;

pub struct SegmentQueryParams {
    pub dataset_id: EntryId,
    pub segment_id: SegmentId,
    pub include_static_data: bool,
    pub include_temporal_data: bool,
    pub query: Option<re_protos::cloud::v1alpha1::Query>,
}

/// Expose an ergonomic API over the gRPC redap client.
///
/// Implementation note: this type is generic so that it can be used with several client types. This
/// is useful for other projects which might have different type (e.g. due to instrumentation).
/// For the viewer, use [`crate::ConnectionClient`].
//TODO(ab): this should NOT be `Clone`, to discourage callsites from holding on to a client for too
//long. However we have a bunch of places that needs to be fixed before we can do that.
#[derive(Debug, Clone)]
pub struct GenericConnectionClient<T>(RerunCloudServiceClient<T>);

impl<T> GenericConnectionClient<T> {
    /// Create a new [`Self`].
    ///
    /// This should not be used in the viewer, use [`crate::ConnectionRegistryHandle::client`]
    /// instead.
    pub fn new(client: RerunCloudServiceClient<T>) -> Self {
        Self(client)
    }

    /// Get a mutable reference to the underlying `RedapClient`.
    //TODO(#10188): this should disappear once we have wrapper for all endpoints and the client code
    //is using them.
    pub fn inner(&mut self) -> &mut RerunCloudServiceClient<T> {
        &mut self.0
    }
}

// ---

impl<T> GenericConnectionClient<T>
where
    T: tonic::client::GrpcService<tonic::body::Body>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + std::marker::Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + std::marker::Send,
{
    /// Uses the `/Version` endpoint for testing roundtrip time.
    pub async fn ping(&mut self) -> ApiResult<()> {
        self.inner()
            .version(VersionRequest {})
            .await
            .map_err(|err| ApiError::tonic(err, "/Version failed"))
            .map(|_| ())
    }

    /// Find all entries matching the given filter.
    pub async fn find_entries(&mut self, filter: EntryFilter) -> ApiResult<Vec<EntryDetails>> {
        let result = self
            .inner()
            .find_entries(FindEntriesRequest {
                filter: Some(filter),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/FindEntries failed"))?
            .into_inner()
            .entries;

        result
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<EntryDetails>, _>>()
            .map_err(|err| {
                ApiError::serialization_with_source(err, "failed parsing /FindEntries response")
            })
    }

    /// Delete the provided entry.
    pub async fn delete_entry(&mut self, entry_id: EntryId) -> ApiResult {
        self.inner()
            .delete_entry(DeleteEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/DeleteEntry failed"))?;

        Ok(())
    }

    /// Update the provided entry.
    pub async fn update_entry(
        &mut self,
        entry_id: EntryId,
        entry_details_update: EntryDetailsUpdate,
    ) -> ApiResult<EntryDetails> {
        let response: UpdateEntryResponse = self
            .inner()
            .update_entry(tonic::Request::new(
                UpdateEntryRequest {
                    id: entry_id,
                    entry_details_update,
                }
                .into(),
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/UpdateEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(err, "failed parsing /UpdateEntry response")
            })?;

        Ok(response.entry_details)
    }

    /// Get the Arrow schema for a dataset entry.
    pub async fn get_dataset_schema(&mut self, entry_id: EntryId) -> ApiResult<ArrowSchema> {
        self.inner()
            .get_dataset_schema(
                tonic::Request::new(GetDatasetSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetDatasetSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/GetDatasetSchema failed"))?
            .into_inner()
            .schema()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /GetDatasetSchema response",
                )
            })
    }

    /// Create a new dataset entry.
    pub async fn create_dataset_entry(
        &mut self,
        name: String,
        entry_id: Option<EntryId>,
    ) -> ApiResult<DatasetEntry> {
        let response: CreateDatasetEntryResponse = self
            .inner()
            .create_dataset_entry(CreateDatasetEntryRequest {
                name: Some(name),
                id: entry_id.map(Into::into),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/CreateDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /CreateDatasetEntry response",
                )
            })?;

        Ok(response.dataset)
    }

    /// Get information on a dataset entry.
    pub async fn read_dataset_entry(&mut self, entry_id: EntryId) -> ApiResult<DatasetEntry> {
        let response: ReadDatasetEntryResponse = self
            .inner()
            .read_dataset_entry(
                tonic::Request::new(ReadDatasetEntryRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /ReadDatasetEntry request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/ReadDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /ReadDatasetEntry response",
                )
            })?;

        Ok(response.dataset_entry)
    }

    /// Update the details of a dataset entry.
    pub async fn update_dataset_entry(
        &mut self,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> ApiResult<DatasetEntry> {
        let response: UpdateDatasetEntryResponse = self
            .inner()
            .update_dataset_entry(tonic::Request::new(
                UpdateDatasetEntryRequest {
                    id: entry_id,
                    dataset_details,
                }
                .into(),
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/UpdateDatasetEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /UpdateDatasetEntry response",
                )
            })?;

        Ok(response.dataset_entry)
    }

    /// Get information on a table entry.
    pub async fn read_table_entry(&mut self, entry_id: EntryId) -> ApiResult<TableEntry> {
        let response: ReadTableEntryResponse = self
            .inner()
            .read_table_entry(ReadTableEntryRequest {
                id: Some(entry_id.into()),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/ReadTableEntry failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(err, "failed parsing /ReadTableEntry response")
            })?;

        Ok(response.table_entry)
    }

    //TODO(ab): accept entry name
    pub async fn get_segment_table_schema(&mut self, entry_id: EntryId) -> ApiResult<ArrowSchema> {
        self.inner()
            .get_segment_table_schema(
                tonic::Request::new(GetSegmentTableSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetSegmentTableSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "GetSegmentTableSchema failed"))?
            .into_inner()
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetSegmentTableSchemaResponse, "schema");
                ApiError::serialization_with_source(
                    err,
                    "missing field in /GetSegmentTableSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /GetSegmentTableSchema response",
                )
            })
    }

    /// Get a list of segment IDs for the given dataset entry ID.
    //TODO(ab): is there a way—and a reason—to not collect and instead return a stream?
    pub async fn get_dataset_segment_ids(
        &mut self,
        entry_id: EntryId,
    ) -> ApiResult<Vec<SegmentId>> {
        const COLUMN_NAME: &str = ScanSegmentTableResponse::FIELD_SEGMENT_ID;

        let mut stream = self
            .inner()
            .scan_segment_table(
                tonic::Request::new(ScanSegmentTableRequest {
                    columns: vec![COLUMN_NAME.to_owned()],
                })
                .with_entry_id(entry_id)
                .map_err(|err| ApiError::tonic(err, "failed building /ScanSegmentTable request"))?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/ScanSegmentTable failed"))?
            .into_inner();

        let mut segment_ids = Vec::new();

        while let Some(resp) = stream.next().await {
            let record_batch: RecordBatch = resp
                .map_err(|err| {
                    ApiError::tonic(err, "failed receiving item from /ScanSegmentTable stream")
                })?
                .data()
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        err,
                        "failed parsing item from /ScanSegmentTable stream",
                    )
                })?
                .try_into()
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        err,
                        "failed decoding item from /ScanSegmentTable stream",
                    )
                })?;

            let segment_id_col = record_batch.column_by_name(COLUMN_NAME).ok_or_else(|| {
                let err = missing_column!(ScanSegmentTableResponse, COLUMN_NAME);
                ApiError::serialization_with_source(
                    err,
                    "missing column from item in /ScanSegmentTable stream",
                )
            })?;

            let segment_id_array = segment_id_col
                .try_downcast_array_ref::<arrow::array::StringArray>()
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        err,
                        "unexpected types in item in /ScanSegmentTable stream",
                    )
                })?;

            segment_ids.extend(
                segment_id_array
                    .iter()
                    .filter_map(|opt| opt.map(|s| SegmentId::new(s.to_owned()))),
            );
        }

        Ok(segment_ids)
    }

    //TODO(ab): accept entry name
    pub async fn get_dataset_manifest_schema(
        &mut self,
        entry_id: EntryId,
    ) -> ApiResult<ArrowSchema> {
        self.inner()
            .get_dataset_manifest_schema(
                tonic::Request::new(GetDatasetManifestSchemaRequest {})
                    .with_entry_id(entry_id)
                    .map_err(|err| {
                        ApiError::tonic(err, "failed building /GetDatasetManifestSchema request")
                    })?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/GetDatasetManifestSchema failed"))?
            .into_inner()
            .schema
            .ok_or_else(|| {
                let err = missing_field!(GetDatasetManifestSchemaResponse, "schema");
                ApiError::serialization_with_source(
                    err,
                    "missing field in /GetDatasetManifestSchema response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed parsing /GetDatasetManifestSchema response",
                )
            })
    }

    /// Get the full [`RawRrdManifest`] of a recording.
    pub async fn get_rrd_manifest(
        &mut self,
        dataset_id: EntryId,
        segment_id: SegmentId,
    ) -> ApiResult<RawRrdManifest> {
        // TODO(cmc): at some point we should probably continue the stream all the way down, but
        // for now we simplify downstream's life by concatenating everything in here.
        let mut rrd_manifest_parts = Vec::new();

        let responses = self
            .inner()
            .get_rrd_manifest(
                tonic::Request::new(re_protos::cloud::v1alpha1::GetRrdManifestRequest {
                    segment_id: Some(segment_id.clone().into()),
                })
                .with_entry_id(dataset_id)
                .map_err(|err| ApiError::tonic(err, "failed building /GetRrdManifest request"))?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/GetRrdManifest failed"))?
            .into_inner();

        futures::pin_mut!(responses);
        while let Some(resp) = responses.next().await {
            let rrd_manifest_part = resp
                .map_err(|err| {
                    ApiError::connection_with_source(
                        err,
                        "failed fetching /GetRrdManifest response part",
                    )
                })?
                .rrd_manifest
                .ok_or_else(|| {
                    let err = missing_field!(GetRrdManifestResponse, "rrd_manifest");
                    ApiError::serialization_with_source(
                        err,
                        "missing field in /GetRrdManifest response",
                    )
                })?
                .to_application(())
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        err,
                        "failed parsing /GetRrdManifest response",
                    )
                })?;

            rrd_manifest_parts.push(rrd_manifest_part);
        }

        let Some(mut rrd_manifest) = rrd_manifest_parts.first().cloned() else {
            return Err(ApiError::serialization(
                "failed to parse the response for /GetRrdManifest (no data)",
            ));
        };

        let data_parts = rrd_manifest_parts.into_iter().map(|p| p.data).collect_vec();
        rrd_manifest.data =
            re_arrow_util::concat_polymorphic_batches(&data_parts).map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed concatenating /GetRrdManifest response parts",
                )
            })?;

        Ok(rrd_manifest)
    }

    /// Fetches all chunks ids for a specified segment.
    ///
    /// You can include/exclude static/temporal chunks,
    /// and limit the query to a time range.
    pub async fn query_dataset_raw(
        &mut self,
        params: SegmentQueryParams,
    ) -> ApiResult<QueryDatasetResponseStream> {
        let SegmentQueryParams {
            dataset_id,
            segment_id,
            include_static_data,
            include_temporal_data,
            query,
        } = params;

        let query_request = QueryDatasetRequest {
            segment_ids: vec![segment_id],
            chunk_ids: vec![],
            entity_paths: vec![],
            select_all_entity_paths: true,
            fuzzy_descriptors: vec![],
            exclude_static_data: !include_static_data,
            exclude_temporal_data: !include_temporal_data,
            query: query
                .map(|q| q.try_into())
                .transpose()
                .map_err(|err| ApiError::tonic(err, "failed building /QueryDataset request"))?,
            scan_parameters: Some(ScanParameters {
                columns: FetchChunksRequest::required_column_names(),
                ..Default::default()
            }),
        };

        Ok(Box::pin(
            self.inner()
                .query_dataset(
                    tonic::Request::new(query_request.into())
                        .with_entry_id(dataset_id)
                        .map_err(|err| {
                            ApiError::tonic(err, "failed building /QueryDataset request")
                        })?,
                )
                .await
                .map_err(|err| ApiError::tonic(err, "/QueryDataset failed"))?
                .into_inner(),
        ))
    }

    /// Fetches all chunks ids for a specified segment.
    ///
    /// You can include/exclude static/temporal chunks,
    /// and limit the query to a time range.
    ///
    /// You can pass on the results to [`Self::query_dataset_chunk_index`].
    pub async fn query_dataset_chunk_index(
        &mut self,
        params: SegmentQueryParams,
    ) -> ApiResult<Vec<RecordBatch>> {
        self.query_dataset_raw(params)
            .await?
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                ApiError::tonic(
                    err,
                    "failed receiving items in /QueryDataset response stream",
                )
            })?
            .into_iter()
            .map(|resp| {
                resp.data.ok_or_else(|| {
                    let err = missing_field!(QueryDatasetResponse, "data");
                    ApiError::serialization_with_source(
                        err,
                        "missing field in item in /QueryDataset response stream",
                    )
                })
            })
            .map(|batch| {
                arrow::array::RecordBatch::try_from(batch?).map_err(|err| {
                    ApiError::serialization_with_source(err, "failed converting to RecordBatch")
                })
            })
            .collect()
    }

    /// Input should be same schema as what [`Self::query_dataset_chunk_index`] returns.
    pub async fn fetch_segment_chunks_by_id(
        &mut self,
        record_batch: &RecordBatch,
    ) -> ApiResult<FetchChunksResponseStream> {
        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: vec![DataframePart::from(record_batch)],
        };

        let fetch_chunks_response_stream = self
            .inner()
            .fetch_chunks(fetch_chunks_request)
            .await
            .map_err(|err| ApiError::tonic(err, "/FetchChunks failed"))?
            .into_inner();

        Ok(Box::pin(fetch_chunks_response_stream))
    }

    /// Fetches chunks for a specified partition and query.
    ///
    /// Convenience for [`Self::query_dataset_chunk_index`] + [`Self::fetch_segment_chunks_by_id`].
    pub async fn fetch_segment_chunks_by_query(
        &mut self,
        params: SegmentQueryParams,
    ) -> ApiResult<FetchChunksResponseStream> {
        let chunk_info_batches = self
            .query_dataset_raw(params)
            .await?
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| {
                ApiError::tonic(
                    err,
                    "failed receiving items in /QueryDataset response stream",
                )
            })?
            .into_iter()
            .map(|resp| {
                resp.data.ok_or_else(|| {
                    let err = missing_field!(QueryDatasetResponse, "data");
                    ApiError::serialization_with_source(
                        err,
                        "missing field in item in /QueryDataset response stream",
                    )
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        if chunk_info_batches.is_empty() {
            let empty_stream = tokio_stream::empty();
            return Ok(Box::pin(empty_stream));
        }

        let fetch_chunks_request = FetchChunksRequest {
            chunk_infos: chunk_info_batches,
        };

        let fetch_chunks_response_stream = self
            .inner()
            .fetch_chunks(fetch_chunks_request)
            .await
            .map_err(|err| ApiError::tonic(err, "/FetchChunks failed"))?
            .into_inner();

        Ok(Box::pin(fetch_chunks_response_stream))
    }

    /// Initiate registration of the provided recording URIs with a dataset and return the
    /// corresponding task descriptors.
    ///
    /// NOTE: The server may pool multiple registrations into a single task. The result always has
    /// the same length as the output, so task ids may be duplicated.
    pub async fn register_with_dataset(
        &mut self,
        dataset_id: EntryId,
        data_sources: Vec<DataSource>,
        on_duplicate: IfDuplicateBehavior,
    ) -> ApiResult<Vec<RegisterWithDatasetTaskDescriptor>> {
        let req = tonic::Request::new(RegisterWithDatasetRequest {
            data_sources,
            on_duplicate,
        })
        .with_entry_id(dataset_id)
        .map_err(|err| ApiError::tonic(err, "failed building /RegisterWithDataset request"))?;

        let response: RecordBatch = self
            .inner()
            .register_with_dataset(req.map(Into::into))
            .await
            .map_err(|err| ApiError::tonic(err, "/RegisterWithDataset failed"))?
            .into_inner()
            .data
            .ok_or_else(|| {
                let err = missing_field!(RegisterWithDatasetResponse, "data");
                ApiError::serialization_with_source(
                    err,
                    "missing field in /RegisterWithDataset response",
                )
            })?
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed decoding /RegisterWithDataset response",
                )
            })?;

        // TODO(andrea): why is the schema completely off?
        #[expect(clippy::overly_complex_bool_expr)]
        if false
            && !response
                .schema()
                .contains(&RegisterWithDatasetResponse::schema())
        {
            let err = invalid_schema!(RegisterWithDatasetResponse);
            return Err(ApiError::serialization_with_source(
                err,
                "invalid schema in /RegisterWithDataset response",
            ));
        }

        let get_string_array = |column_name: &'static str| {
            response
                .column_by_name(column_name)
                .and_then(|column| {
                    column
                        .try_downcast_array_ref::<arrow::array::StringArray>()
                        .ok()
                })
                .ok_or_else(|| {
                    let err = missing_column!(RegisterWithDatasetResponse, column_name);
                    ApiError::serialization_with_source(
                        err,
                        "missing column in /RegisterWithDataset response",
                    )
                })
        };

        let segment_id_column = get_string_array(RegisterWithDatasetResponse::FIELD_SEGMENT_ID)?;
        let segment_type_column = DataSourceKind::many_from_arrow(
            response
                .column_by_name(RegisterWithDatasetResponse::FIELD_SEGMENT_TYPE)
                .ok_or_else(|| {
                    let err = missing_column!(
                        RegisterWithDatasetResponse,
                        RegisterWithDatasetResponse::FIELD_SEGMENT_TYPE
                    );
                    ApiError::serialization_with_source(
                        err,
                        "missing column in /RegisterWithDataset response",
                    )
                })?,
        )
        .map_err(|err| {
            ApiError::serialization_with_source(err, "failed parsing /RegisterWithDataset response")
        })?;
        let storage_url_column = get_string_array(RegisterWithDatasetResponse::FIELD_STORAGE_URL)?;
        let task_id_column = get_string_array(RegisterWithDatasetResponse::FIELD_TASK_ID)?;

        itertools::izip!(
            segment_id_column,
            segment_type_column,
            storage_url_column,
            task_id_column,
        )
        .map(|(segment_id, segment_type, storage_url, task_id)| {
            Ok(RegisterWithDatasetTaskDescriptor {
                segment_id: SegmentId::new(
                    segment_id
                        .ok_or_else(|| {
                            let err = missing_field!(RegisterWithDatasetResponse, "segment_id");
                            ApiError::serialization_with_source(
                                err,
                                "missing field in /RegisterWithDataset response",
                            )
                        })?
                        .to_owned(),
                ),
                segment_type,
                storage_url: url::Url::parse(storage_url.ok_or_else(|| {
                    let err = missing_field!(RegisterWithDatasetResponse, "storage_url");
                    ApiError::serialization_with_source(
                        err,
                        "missing field in /RegisterWithDataset response",
                    )
                })?)
                .map_err(|err| {
                    ApiError::serialization_with_source(
                        TypeConversionError::UrlParseError(err),
                        "failed to parse /RegisterWithDataset response",
                    )
                })?,
                task_id: TaskId {
                    id: task_id
                        .ok_or_else(|| {
                            let err = missing_field!(RegisterWithDatasetResponse, "task_id");
                            ApiError::serialization_with_source(
                                err,
                                "missing field in /RegisterWithDataset response",
                            )
                        })?
                        .to_owned(),
                },
            })
        })
        .collect()
    }

    /// Unregisters segments and layers from the dataset.
    ///
    /// Excluding IO errors, this will always succeed as long the target dataset exists.
    /// Corollary: unregistering data that doesn't exist is a no-op.
    ///
    /// This always returns a subset of the data from `ScanDatasetManifest`, and therefore the data will
    /// also follow the schema returned by [`Self::get_dataset_manifest_schema`].
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
    pub async fn unregister_from_dataset(
        &mut self,
        dataset_id: EntryId,
        segments_to_drop: Vec<String>,
        layers_to_drop: Vec<String>,
        force: bool,
    ) -> ApiResult<Vec<RecordBatch>> {
        let req = tonic::Request::new(
            UnregisterFromDatasetRequest {
                segments_to_drop: segments_to_drop.into_iter().map(Into::into).collect(),
                layers_to_drop,
                force,
            }
            .into(),
        )
        .with_entry_id(dataset_id)
        .map_err(|err| ApiError::tonic(err, "failed building /UnregisterFromDataset request"))?;

        use futures::TryStreamExt as _;
        let responses: Vec<_> = self
            .inner()
            .unregister_from_dataset(req)
            .await
            .map_err(|err| ApiError::tonic(err, "/UnregisterFromDataset failed"))?
            .into_inner()
            .try_collect()
            .await
            .map_err(|err| ApiError::tonic(err, "/UnregisterFromDataset failed"))?;

        let batches: ApiResult<Vec<RecordBatch>> = responses
            .into_iter()
            .map(|resp| {
                resp.data
                    .ok_or_else(|| {
                        let err = missing_field!(UnregisterFromDatasetResponse, "data");
                        ApiError::serialization_with_source(
                            err,
                            "missing field in /UnregisterFromDataset response",
                        )
                    })?
                    .try_into()
                    .map_err(|err| {
                        ApiError::serialization_with_source(
                            err,
                            "failed decoding /UnregisterFromDataset response",
                        )
                    })
            })
            .collect();

        batches
    }

    /// Register a foreign Lance table to a new table entry in the catalog.
    //TODO(ab): in the future, we will probably support my types of tables (parquet on S3, etc.)
    pub async fn register_table(&mut self, name: String, url: url::Url) -> ApiResult<TableEntry> {
        let request = re_protos::cloud::v1alpha1::ext::RegisterTableRequest {
            name,
            provider_details: ProviderDetails::LanceTable(LanceTable { table_url: url }),
        };

        let response: RegisterTableResponse = self
            .inner()
            .register_table(tonic::Request::new(request.try_into().map_err(|err| {
                ApiError::serialization_with_source(err, "failed building /RegisterTable request")
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "/RegisterTable failed"))?
            .into_inner()
            .try_into()
            .map_err(|err| {
                ApiError::serialization_with_source(err, "failed parsing /RegisterTable response")
            })?;

        Ok(response.table_entry)
    }

    #[expect(clippy::fn_params_excessive_bools)] // TODO(emilk): remove bool parameters
    pub async fn do_maintenance(
        &mut self,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> ApiResult {
        self.inner()
            .do_maintenance(
                tonic::Request::new(
                    re_protos::cloud::v1alpha1::ext::DoMaintenanceRequest {
                        optimize_indexes,
                        retrain_indexes,
                        compact_fragments,
                        cleanup_before,
                        unsafe_allow_recent_cleanup,
                    }
                    .into(),
                )
                .with_entry_id(dataset_id)
                .map_err(|err| ApiError::tonic(err, "failed building /DoMaintenance request"))?,
            )
            .await
            .map_err(|err| ApiError::tonic(err, "/DoMaintenance failed"))?;

        Ok(())
    }

    pub async fn do_global_maintenance(&mut self) -> ApiResult {
        self.inner()
            .do_global_maintenance(tonic::Request::new(
                re_protos::cloud::v1alpha1::DoGlobalMaintenanceRequest {},
            ))
            .await
            .map_err(|err| ApiError::tonic(err, "/DoGlobalMaintenance failed"))?;

        Ok(())
    }

    pub async fn get_table_names(&mut self) -> ApiResult<Vec<String>> {
        Ok(self
            .find_entries(re_protos::cloud::v1alpha1::EntryFilter {
                entry_kind: Some(EntryKind::Table.into()),
                ..Default::default()
            })
            .await?
            .into_iter()
            .map(|entry| entry.name.clone())
            .collect())
    }

    // -- Tasks API --
    pub async fn query_tasks_on_completion(
        &mut self,
        task_ids: Vec<TaskId>,
        timeout: std::time::Duration,
    ) -> ApiResult<tonic::Streaming<QueryTasksOnCompletionResponse>> {
        let q = QueryTasksOnCompletionRequest { task_ids, timeout };
        let response = self
            .inner()
            .query_tasks_on_completion(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(
                    err,
                    "failed building /QueryTasksOnCompletion request",
                )
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "/QueryTasksOnCompletion failed"))?
            .into_inner();
        Ok(response)
    }

    pub async fn query_tasks(&mut self, task_ids: Vec<TaskId>) -> ApiResult<QueryTasksResponse> {
        let q = QueryTasksRequest { task_ids };
        let response = self
            .inner()
            .query_tasks(tonic::Request::new(q.try_into().map_err(|err| {
                ApiError::serialization_with_source(err, "failed building /QueryTasks request")
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "/QueryTasks failed"))?
            .into_inner();
        Ok(response)
    }

    pub async fn get_entry_id(
        &mut self,
        entry_name: &str,
        entry_kind: Option<EntryKind>,
    ) -> ApiResult<Option<EntryId>> {
        self.inner()
            .find_entries(FindEntriesRequest {
                filter: Some(EntryFilter {
                    id: None,
                    name: Some(entry_name.to_owned()),
                    entry_kind: entry_kind.map(|kind| kind.into()),
                }),
            })
            .await
            .map_err(|err| ApiError::tonic(err, "/FindEntries failed"))?
            .into_inner()
            .entries
            .first()
            .and_then(|entry| entry.id)
            .map(|id| {
                EntryId::try_from(id)
                    .map_err(|err| ApiError::serialization_with_source(err, "/FindEntries failed"))
            })
            .transpose()
    }

    pub async fn write_table(
        &mut self,
        stream: impl Stream<Item = RecordBatch> + Send + 'static,
        table_id: EntryId,
        insert_mode: TableInsertMode,
    ) -> ApiResult {
        let insert_mode = re_protos::cloud::v1alpha1::TableInsertMode::from(insert_mode).into();
        let stream = stream
            .map(move |batch| WriteTableRequest {
                dataframe_part: Some(batch.into()),
                insert_mode,
            })
            .into_streaming_request()
            .with_entry_id(table_id)
            .map_err(|err| ApiError::tonic(err, "/WriteTable failed"))?;

        self.inner()
            .write_table(stream)
            .await
            .map(|_| ())
            .map_err(|err| ApiError::tonic(err, "/WriteTable failed"))
    }

    pub async fn create_table_entry(
        &mut self,
        name: &str,
        url: Option<Url>,
        schema: SchemaRef,
    ) -> ApiResult<TableEntry> {
        let provider_details =
            url.map(|url| ProviderDetails::LanceTable(LanceTable { table_url: url }));
        let request = CreateTableEntryRequest {
            name: name.to_owned(),
            schema: schema.as_ref().clone(),
            provider_details,
        };

        let resp = self
            .inner()
            .create_table_entry(tonic::Request::new(request.try_into().map_err(|err| {
                ApiError::internal_with_source(err, "/CreateTableEntry failed")
            })?))
            .await
            .map_err(|err| ApiError::tonic(err, "failed to create table"))?
            .into_inner();

        resp.table
            .ok_or_else(|| {
                ApiError::tonic(
                    Status::invalid_argument("entry ID not set in response"),
                    "/CreateTable failed",
                )
            })?
            .try_into()
            .map_err(|err| ApiError::internal_with_source(err, "/CreateTable failed"))
    }
}
