#![allow(unsafe_op_in_unsafe_fn)]

use std::collections::BTreeSet;

use arrow::{
    array::{RecordBatch, RecordBatchIterator, RecordBatchReader},
    datatypes::Schema,
    ffi_stream::ArrowArrayStreamReader,
    pyarrow::PyArrowType,
};
// False positive due to #[pyfunction] macro
use pyo3::{
    exceptions::{PyRuntimeError, PyTypeError, PyValueError},
    prelude::*,
    types::PyDict,
    Bound, PyResult,
};
use re_chunk::{Chunk, TransportChunk};
use re_chunk_store::ChunkStore;
use re_dataframe::{ChunkStoreHandle, QueryExpression, SparseFillStrategy, ViewContentsSelector};
use re_grpc_client::TonicStatusError;
use re_log_encoding::codec::wire::{decode, encode};
use re_log_types::{EntityPathFilter, StoreInfo, StoreSource};
use re_protos::{
    common::v0::{EncoderVersion, RecordingId},
    remote_store::v0::{
        storage_node_client::StorageNodeClient, CatalogFilter, DataframePart,
        FetchRecordingRequest, QueryCatalogRequest, QueryRequest, RecordingType,
        RegisterRecordingRequest, UpdateCatalogRequest,
    },
};
use re_sdk::{ApplicationId, ComponentName, StoreId, StoreKind, Time, Timeline};
use tokio_stream::StreamExt;

use crate::dataframe::{ComponentLike, PyRecording, PyRecordingHandle, PyRecordingView};

/// Register the `rerun.remote` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyStorageNodeClient>()?;

    m.add_function(wrap_pyfunction!(connect, m)?)?;

    Ok(())
}

async fn connect_async(addr: String) -> PyResult<StorageNodeClient<tonic::transport::Channel>> {
    #[cfg(not(target_arch = "wasm32"))]
    let tonic_client = tonic::transport::Endpoint::new(addr)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .connect()
        .await
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(StorageNodeClient::new(tonic_client))
}

/// Load a rerun archive from an RRD file.
///
/// Required-feature: `remote`
///
/// Parameters
/// ----------
/// addr : str
///     The address of the storage node to connect to.
///
/// Returns
/// -------
/// StorageNodeClient
///     The connected client.
#[pyfunction]
pub fn connect(addr: String) -> PyResult<PyStorageNodeClient> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let client = runtime.block_on(connect_async(addr))?;

    Ok(PyStorageNodeClient { runtime, client })
}

/// A connection to a remote storage node.
#[pyclass(name = "StorageNodeClient")]
pub struct PyStorageNodeClient {
    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    runtime: tokio::runtime::Runtime,

    /// The actual tonic connection.
    client: StorageNodeClient<tonic::transport::Channel>,
}

impl PyStorageNodeClient {
    /// Get the [`StoreInfo`] for a single recording in the storage node.
    fn get_store_info(&mut self, id: &str) -> PyResult<StoreInfo> {
        let store_info = self
            .runtime
            .block_on(async {
                let resp = self
                    .client
                    .query_catalog(QueryCatalogRequest {
                        column_projection: None, // fetch all columns
                        filter: Some(CatalogFilter {
                            recording_ids: vec![RecordingId { id: id.to_owned() }],
                        }),
                    })
                    .await
                    .map_err(re_grpc_client::TonicStatusError)?
                    .into_inner()
                    .map(|resp| {
                        resp.and_then(|r| {
                            decode(r.encoder_version(), &r.payload)
                                .map_err(|err| tonic::Status::internal(err.to_string()))
                        })
                    })
                    .collect::<Result<Vec<_>, tonic::Status>>()
                    .await
                    .map_err(re_grpc_client::TonicStatusError)?;

                if resp.len() != 1 || resp[0].num_rows() != 1 {
                    return Err(re_grpc_client::StreamError::ChunkError(
                        re_chunk::ChunkError::Malformed {
                            reason: format!(
                                "expected exactly one recording with id {id}, got {}",
                                resp.len()
                            ),
                        },
                    ));
                }

                re_grpc_client::store_info_from_catalog_chunk(&resp[0], id)
            })
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(store_info)
    }

    /// Execute a [`QueryExpression`] for a single recording in the storage node.
    pub(crate) fn exec_query(
        &mut self,
        id: StoreId,
        query: QueryExpression,
    ) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let query: re_protos::common::v0::Query = query.into();

        let batches = self.runtime.block_on(async {
            // TODO(#8536): Avoid the need to collect here.
            // This means we shouldn't be blocking on
            let batches = self
                .client
                .query(QueryRequest {
                    recording_id: Some(id.into()),
                    query: Some(query.clone()),
                })
                .await
                .map_err(TonicStatusError)?
                .into_inner()
                .map(|resp| {
                    resp.and_then(|r| {
                        decode(r.encoder_version(), &r.payload)
                            .map_err(|err| tonic::Status::internal(err.to_string()))
                    })
                })
                .collect::<Result<Vec<_>, tonic::Status>>()
                .await
                .map_err(TonicStatusError)?;

            let schema = batches
                .first()
                .map(|batch| batch.schema.clone())
                .unwrap_or_else(|| arrow2::datatypes::Schema::from(vec![]));

            let fields: Vec<arrow::datatypes::Field> =
                schema.fields.iter().map(|f| f.clone().into()).collect();
            let metadata = schema.metadata.clone().into_iter().collect();
            let schema = arrow::datatypes::Schema::new(fields).with_metadata(metadata);

            Ok(RecordBatchIterator::new(
                batches.into_iter().map(|tc| tc.try_to_arrow_record_batch()),
                std::sync::Arc::new(schema),
            ))
        });

        let result =
            batches.map_err(|err: TonicStatusError| PyRuntimeError::new_err(err.to_string()))?;

        Ok(PyArrowType(Box::new(result)))
    }
}

#[pymethods]
impl PyStorageNodeClient {
    /// Get the metadata for all recordings in the storage node.
    fn query_catalog(&mut self) -> PyResult<PyArrowType<Box<dyn RecordBatchReader + Send>>> {
        let reader = self.runtime.block_on(async {
            // TODO(jleibs): Support column projection and filtering
            let request = QueryCatalogRequest {
                column_projection: None,
                filter: None,
            };

            let transport_chunks = self
                .client
                .query_catalog(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner()
                .map(|resp| {
                    resp.and_then(|r| {
                        decode(r.encoder_version(), &r.payload)
                            .map_err(|err| tonic::Status::internal(err.to_string()))
                    })
                })
                .collect::<Result<Vec<_>, _>>()
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let record_batches: Vec<Result<RecordBatch, arrow::error::ArrowError>> =
                transport_chunks
                    .into_iter()
                    .map(|tc| tc.try_to_arrow_record_batch())
                    .collect();

            // TODO(jleibs): surfacing this schema is awkward. This should be more explicit in
            // the gRPC APIs somehow.
            let schema = record_batches
                .first()
                .and_then(|batch| batch.as_ref().ok().map(|batch| batch.schema()))
                .unwrap_or(std::sync::Arc::new(Schema::empty()));

            let reader = RecordBatchIterator::new(record_batches, schema);

            Ok::<_, PyErr>(reader)
        })?;

        Ok(PyArrowType(Box::new(reader)))
    }

    /// Register a recording along with some metadata.
    ///
    /// Parameters
    /// ----------
    /// storage_url : str
    ///     The URL to the storage location.
    /// metadata : Optional[Table | RecordBatch]
    ///     A pyarrow Table or RecordBatch containing the metadata to update.
    ///     This Table must contain only a single row.
    #[pyo3(signature = (
        storage_url,
        metadata = None
    ))]
    fn register(&mut self, storage_url: &str, metadata: Option<MetadataLike>) -> PyResult<String> {
        self.runtime.block_on(async {
            let storage_url = url::Url::parse(storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let _obj = object_store::ObjectStoreScheme::parse(&storage_url)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let metadata = metadata
                .map(|metadata| {
                    let metadata = metadata.into_record_batch()?;

                    if metadata.num_rows() != 1 {
                        return Err(PyRuntimeError::new_err(
                            "Metadata must contain exactly one row",
                        ));
                    }

                    let metadata_tc = TransportChunk::from_arrow_record_batch(&metadata);

                    encode(EncoderVersion::V0, &metadata_tc)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))
                })
                .transpose()?
                .map(|payload| DataframePart {
                    encoder_version: EncoderVersion::V0 as i32,
                    payload,
                });

            let request = RegisterRecordingRequest {
                // TODO(jleibs): Description should really just be in the metadata
                description: Default::default(),
                storage_url: storage_url.to_string(),
                metadata,
                typ: RecordingType::Rrd.into(),
            };

            let resp = self
                .client
                .register_recording(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();
            let metadata = decode(resp.encoder_version(), &resp.payload)
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            let recording_id = metadata
                .all_columns()
                .find(|(field, _data)| field.name == "id")
                .map(|(_field, data)| data)
                .ok_or(PyRuntimeError::new_err("No id"))?
                .as_any()
                .downcast_ref::<arrow2::array::Utf8Array<i32>>()
                .ok_or(PyRuntimeError::new_err("Id is not a string"))?
                .value(0)
                .to_owned();

            Ok(recording_id)
        })
    }

    /// Update the catalog metadata for one or more recordings.
    ///
    /// The updates are provided as a pyarrow Table or RecordBatch containing the metadata to update.
    /// The Table must contain an 'id' column, which is used to specify the recording to update for each row.
    ///
    /// Parameters
    /// ----------
    /// metadata : Table | RecordBatch
    ///     A pyarrow Table or RecordBatch containing the metadata to update.
    #[pyo3(signature = (
        metadata
    ))]
    #[allow(clippy::needless_pass_by_value)]
    fn update_catalog(&mut self, metadata: MetadataLike) -> PyResult<()> {
        self.runtime.block_on(async {
            let metadata = metadata.into_record_batch()?;

            // TODO(jleibs): This id name should probably come from `re_protos`
            if metadata.schema().column_with_name("id").is_none() {
                return Err(PyRuntimeError::new_err(
                    "Metadata must contain an 'id' column",
                ));
            }

            let metadata_tc = TransportChunk::from_arrow_record_batch(&metadata);

            let request = UpdateCatalogRequest {
                metadata: Some(DataframePart {
                    encoder_version: EncoderVersion::V0 as i32,
                    payload: encode(EncoderVersion::V0, &metadata_tc)
                        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?,
                }),
            };

            self.client
                .update_catalog(request)
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

            Ok(())
        })
    }

    /// Open a [`Recording`][rerun.dataframe.Recording] by id to use with the dataframe APIs.
    ///
    /// This will run queries against the remote storage node and stream the results. Faster for small
    /// numbers of queries with small results.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     The id of the recording to open.
    ///
    /// Returns
    /// -------
    /// Recording
    ///     The opened recording.
    #[pyo3(signature = (
        id,
    ))]
    fn open_recording(slf: Bound<'_, Self>, id: &str) -> PyResult<PyRemoteRecording> {
        let mut borrowed_self = slf.borrow_mut();

        let store_info = borrowed_self.get_store_info(id)?;

        let client = slf.unbind();

        Ok(PyRemoteRecording {
            client: std::sync::Arc::new(client),
            store_info,
        })
    }

    /// Download a [`Recording`][rerun.dataframe.Recording] by id to use with the dataframe APIs.
    ///
    /// This will download the full recording to memory and run queries against a local chunk store.
    ///
    /// Parameters
    /// ----------
    /// id : str
    ///     The id of the recording to open.
    ///
    /// Returns
    /// -------
    /// Recording
    ///     The opened recording.
    #[pyo3(signature = (
        id,
    ))]
    fn download_recording(&mut self, id: &str) -> PyResult<PyRecording> {
        use tokio_stream::StreamExt as _;
        let store = self.runtime.block_on(async {
            let mut resp = self
                .client
                .fetch_recording(FetchRecordingRequest {
                    recording_id: Some(RecordingId { id: id.to_owned() }),
                })
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            // TODO(jleibs): Does this come from RDP?
            let store_id = StoreId::from_string(StoreKind::Recording, id.to_owned());

            let store_info = StoreInfo {
                application_id: ApplicationId::from("rerun_data_platform"),
                store_id: store_id.clone(),
                cloned_from: None,
                is_official_example: false,
                started: Time::now(),
                store_source: StoreSource::Unknown,
                store_version: None,
            };

            let mut store = ChunkStore::new(store_id, Default::default());
            store.set_info(store_info);

            while let Some(result) = resp.next().await {
                let response = result.map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
                let tc = match decode(EncoderVersion::V0, &response.payload) {
                    Ok(tc) => tc,
                    Err(err) => {
                        return Err(PyRuntimeError::new_err(err.to_string()));
                    }
                };
                let chunk = Chunk::from_transport(&tc)
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

                store
                    .insert_chunk(&std::sync::Arc::new(chunk))
                    .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;
            }

            Ok(store)
        });

        let handle = ChunkStoreHandle::new(store?);

        let cache =
            re_dataframe::QueryCacheHandle::new(re_dataframe::QueryCache::new(handle.clone()));

        Ok(PyRecording {
            store: handle,
            cache,
        })
    }
}

/// A type alias for metadata.
#[derive(FromPyObject)]
enum MetadataLike {
    RecordBatch(PyArrowType<RecordBatch>),
    Reader(PyArrowType<ArrowArrayStreamReader>),
}

impl MetadataLike {
    fn into_record_batch(self) -> PyResult<RecordBatch> {
        let (schema, batches) = match self {
            Self::RecordBatch(record_batch) => (record_batch.0.schema(), vec![record_batch.0]),
            Self::Reader(reader) => (
                reader.0.schema(),
                reader.0.collect::<Result<Vec<_>, _>>().map_err(|err| {
                    PyRuntimeError::new_err(format!("Failed to read RecordBatches: {err}"))
                })?,
            ),
        };

        arrow::compute::concat_batches(&schema, &batches)
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))
    }
}

/// A single Rerun recording.
///
/// This can be loaded from an RRD file using [`load_recording()`][rerun.dataframe.load_recording].
///
/// A recording is a collection of data that was logged to Rerun. This data is organized
/// as a column for each index (timeline) and each entity/component pair that was logged.
///
/// You can examine the [`.schema()`][rerun.dataframe.Recording.schema] of the recording to see
/// what data is available, or create a [`RecordingView`][rerun.dataframe.RecordingView] to
/// to retrieve the data.
#[pyclass(name = "RemoteRecording")]
pub struct PyRemoteRecording {
    pub(crate) client: std::sync::Arc<Py<PyStorageNodeClient>>,
    pub(crate) store_info: StoreInfo,
}

impl PyRemoteRecording {
    /// Convert a `ViewContentsLike` into a `ViewContentsSelector`.
    ///
    /// ```python
    /// ViewContentsLike = Union[str, Dict[str, Union[ComponentLike, Sequence[ComponentLike]]]]
    /// ```
    ///
    // TODO(jleibs): This needs access to the schema to resolve paths and components
    fn extract_contents_expr(
        expr: &Bound<'_, PyAny>,
    ) -> PyResult<re_chunk_store::ViewContentsSelector> {
        if let Ok(expr) = expr.extract::<String>() {
            let path_filter =
            EntityPathFilter::parse_strict(&expr, &Default::default()).map_err(|err| {
                PyValueError::new_err(format!(
                    "Could not interpret `contents` as a ViewContentsLike. Failed to parse {expr}: {err}.",
                ))
            })?;

            for (rule, _) in path_filter.rules() {
                if rule.include_subtree {
                    return Err(PyValueError::new_err(
                        "SubTree path expressions (/**) are not allowed yet for remote recordings.",
                    ));
                }
            }

            // Since these are all exact rules, just include them directly
            let contents = path_filter
                .rules()
                .map(|(rule, _)| (rule.path.clone(), None))
                .collect();

            Ok(contents)
        } else if let Ok(dict) = expr.downcast::<PyDict>() {
            // `Union[ComponentLike, Sequence[ComponentLike]]]`

            let mut contents = ViewContentsSelector::default();

            for (key, value) in dict {
                let key = key.extract::<String>().map_err(|_err| {
                    PyTypeError::new_err(
                        format!("Could not interpret `contents` as a ViewContentsLike. Key: {key} is not a path expression."),
                    )
                })?;

                let path_filter = EntityPathFilter::parse_strict(&key, &Default::default()).map_err(|err| {
                    PyValueError::new_err(format!(
                        "Could not interpret `contents` as a ViewContentsLike. Failed to parse {key}: {err}.",
                    ))
                })?;

                for (rule, _) in path_filter.rules() {
                    if rule.include_subtree {
                        return Err(PyValueError::new_err(
                            "SubTree path expressions (/**) are not allowed yet for remote recordings.",
                        ));
                    }
                }

                let component_strs: BTreeSet<String> = if let Ok(component) =
                    value.extract::<ComponentLike>()
                {
                    std::iter::once(component.0).collect()
                } else if let Ok(components) = value.extract::<Vec<ComponentLike>>() {
                    components.into_iter().map(|c| c.0).collect()
                } else {
                    return Err(PyTypeError::new_err(
                            format!("Could not interpret `contents` as a ViewContentsLike. Value: {value} is not a ComponentLike or Sequence[ComponentLike]."),
                        ));
                };

                contents.append(
                    &mut path_filter
                        .rules()
                        .map(|(rule, _)| {
                            let components = component_strs
                                .iter()
                                .map(|component_name| ComponentName::from(component_name.clone()))
                                .collect();
                            (rule.path.clone(), Some(components))
                        })
                        .collect(),
                );
            }

            Ok(contents)
        } else {
            return Err(PyTypeError::new_err(
                "Could not interpret `contents` as a ViewContentsLike. Top-level type must be a string or a dictionary.",
            ));
        }
    }
}

#[pymethods]
impl PyRemoteRecording {
    #[allow(rustdoc::private_doc_tests, rustdoc::invalid_rust_codeblocks)]
    /// Create a [`RecordingView`][rerun.dataframe.RecordingView] of the recording according to a particular index and content specification.
    ///
    /// The only type of index currently supported is the name of a timeline.
    ///
    /// The view will only contain a single row for each unique value of the index
    /// that is associated with a component column that was included in the view.
    /// Component columns that are not included via the view contents will not
    /// impact the rows that make up the view. If the same entity / component pair
    /// was logged to a given index multiple times, only the most recent row will be
    /// included in the view, as determined by the `row_id` column. This will
    /// generally be the last value logged, as row_ids are guaranteed to be
    /// monotonically increasing when data is sent from a single process.
    ///
    /// Parameters
    /// ----------
    /// index : str
    ///     The index to use for the view. This is typically a timeline name.
    /// contents : ViewContentsLike
    ///     The content specification for the view.
    ///
    ///     This can be a single string content-expression such as: `"world/cameras/**"`, or a dictionary
    ///     specifying multiple content-expressions and a respective list of components to select within
    ///     that expression such as `{"world/cameras/**": ["ImageBuffer", "PinholeProjection"]}`.
    /// include_semantically_empty_columns : bool, optional
    ///     Whether to include columns that are semantically empty, by default `False`.
    ///
    ///     Semantically empty columns are components that are `null` or empty `[]` for every row in the recording.
    /// include_indicator_columns : bool, optional
    ///     Whether to include indicator columns, by default `False`.
    ///
    ///     Indicator columns are components used to represent the presence of an archetype within an entity.
    /// include_tombstone_columns : bool, optional
    ///     Whether to include tombstone columns, by default `False`.
    ///
    ///     Tombstone columns are components used to represent clears. However, even without the clear
    ///     tombstone columns, the view will still apply the clear semantics when resolving row contents.
    ///
    /// Returns
    /// -------
    /// RecordingView
    ///     The view of the recording.
    ///
    /// Examples
    /// --------
    /// All the data in the recording on the timeline "my_index":
    /// ```python
    /// recording.view(index="my_index", contents="/**")
    /// ```
    ///
    /// Just the Position3D components in the "points" entity:
    /// ```python
    /// recording.view(index="my_index", contents={"points": "Position3D"})
    /// ```
    #[allow(clippy::fn_params_excessive_bools)]
    #[pyo3(signature = (
        *,
        index,
        contents,
        include_semantically_empty_columns = false,
        include_indicator_columns = false,
        include_tombstone_columns = false,
    ))]
    fn view(
        slf: Bound<'_, Self>,
        index: &str,
        contents: &Bound<'_, PyAny>,
        include_semantically_empty_columns: bool,
        include_indicator_columns: bool,
        include_tombstone_columns: bool,
    ) -> PyResult<PyRecordingView> {
        // TODO(jleibs): We should be able to use this to resolve the timeline / contents
        //let borrowed_self = slf.borrow();

        // TODO(jleibs): Need to get this from the remote schema
        //let timeline = borrowed_self.store.read().resolve_time_selector(&selector);
        let timeline = Timeline::new_sequence(index);

        let contents = Self::extract_contents_expr(contents)?;

        let query = QueryExpression {
            view_contents: Some(contents),
            include_semantically_empty_columns,
            include_indicator_columns,
            include_tombstone_columns,
            filtered_index: Some(timeline),
            filtered_index_range: None,
            filtered_index_values: None,
            using_index_values: None,
            filtered_is_not_null: None,
            sparse_fill_strategy: SparseFillStrategy::None,
            selection: None,
        };

        let recording = slf.unbind();

        Ok(PyRecordingView {
            recording: PyRecordingHandle::Remote(std::sync::Arc::new(recording)),
            query_expression: query,
        })
    }

    /// The recording ID of the recording.
    fn recording_id(&self) -> String {
        self.store_info.store_id.id.as_str().to_owned()
    }

    /// The application ID of the recording.
    fn application_id(&self) -> String {
        self.store_info.application_id.to_string()
    }
}
