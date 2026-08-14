use arrow::datatypes::{Schema as ArrowSchema, SchemaRef};
use arrow::ffi_stream::ArrowArrayStreamReader;
use itertools::Itertools as _;
use pyo3::exceptions::PyValueError;
use pyo3::{PyResult, Python};
use re_log::external::log::warn;
use re_log_types::{EntryId, EntryName};
use re_protos::cloud::v1alpha1::EntryFilter;
use re_protos::cloud::v1alpha1::ext as cloud_ext;
use re_protos::cloud::v1alpha1::ext::{
    DataSource, DatasetDetails, DatasetEntry, EntryDetails, TableDetails, TableEntry,
    VersionResponse,
};
use re_protos::common::v1alpha1::TaskId;
use re_protos::common::v1alpha1::ext::{IfDuplicateBehavior, SegmentId};
use re_redap_client::{
    ConnectionClient, ConnectionHandle, ConnectionRegistryHandle, RegistrationHandle, TraceId,
};
use re_types_core::LayerName;

use crate::catalog::table_entry::PyTableInsertModeInternal;
use crate::catalog::to_py_err;
use crate::utils::wait_for_future;

/// Connection handle to a catalog service.
#[derive(Clone)]
pub(crate) struct PyConnectionHandle {
    inner: ConnectionHandle,
}

impl PyConnectionHandle {
    pub fn new(connection_registry: ConnectionRegistryHandle, origin: re_uri::Origin) -> Self {
        Self {
            inner: connection_registry.connection_handle(origin),
        }
    }

    pub async fn client(&self) -> PyResult<ConnectionClient> {
        self.inner.client().await.map_err(to_py_err)
    }

    pub fn inner(&self) -> &ConnectionHandle {
        &self.inner
    }

    pub fn origin(&self) -> &re_uri::Origin {
        self.inner.origin()
    }
}

impl PyConnectionHandle {
    #[tracing::instrument(level = "info", skip_all)]
    pub fn version_info(&self, py: Python<'_>) -> PyResult<VersionResponse> {
        wait_for_future(py, async {
            self.client().await?.version_info().await.map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn rtt(&self, py: Python<'_>, num_pings: usize) -> PyResult<std::time::Duration> {
        wait_for_future(py, async {
            self.client().await?.rtt(num_pings).await.map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn bandwidth_bytes_per_sec(
        &self,
        py: Python<'_>,
        num_bytes: u64,
        rtt: std::time::Duration,
    ) -> PyResult<Option<f64>> {
        wait_for_future(py, async {
            self.client()
                .await?
                .bandwidth_bytes_per_sec(num_bytes, rtt)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn find_entries(&self, py: Python<'_>, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        wait_for_future(py, async {
            self.client()
                .await?
                .find_entries(filter)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn delete_entry(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<()> {
        wait_for_future(py, async {
            self.client()
                .await?
                .delete_entry(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn update_entry(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        entry_details_update: cloud_ext::EntryDetailsUpdate,
    ) -> PyResult<cloud_ext::EntryDetails> {
        wait_for_future(py, async {
            self.client()
                .await?
                .update_entry(entry_id, entry_details_update)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn create_dataset(&self, py: Python<'_>, name: String) -> PyResult<DatasetEntry> {
        let name = EntryName::new(name).map_err(|err| PyValueError::new_err(err.to_string()))?;
        wait_for_future(py, async {
            self.client()
                .await?
                .create_dataset_entry(name, None)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn read_dataset(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<DatasetEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .read_dataset_entry(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn update_dataset(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        dataset_details: DatasetDetails,
    ) -> PyResult<DatasetEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .update_dataset_entry(entry_id, dataset_details)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn get_dataset_segment_ids(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
    ) -> PyResult<Vec<String>> {
        wait_for_future(py, async {
            Ok(self
                .client()
                .await?
                .get_dataset_segment_ids(entry_id)
                .await
                .map_err(to_py_err)?
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>())
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_table(
        &self,
        py: Python<'_>,
        name: EntryName,
        url: url::Url,
    ) -> PyResult<TableEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .register_table(name, url)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn create_table_entry(
        &self,
        py: Python<'_>,
        name: &EntryName,
        schema: SchemaRef,
        url: Option<url::Url>,
    ) -> PyResult<TableEntry> {
        let entry_id = wait_for_future(py, async {
            self.client()
                .await?
                .create_table_entry(name.clone(), url, schema)
                .await
                .map_err(to_py_err)
        })?;

        self.read_table(py, entry_id.details.id)
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn read_table(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<TableEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .read_table_entry(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn update_table(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        table_details: TableDetails,
    ) -> PyResult<TableEntry> {
        wait_for_future(py, async {
            self.client()
                .await?
                .update_table_entry(entry_id, table_details)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn write_table(
        &self,
        py: Python<'_>,
        entry_id: EntryId,
        stream: ArrowArrayStreamReader,
        insert_mode: PyTableInsertModeInternal,
    ) -> PyResult<()> {
        wait_for_future(py, async {
            // Since the errors occur during streaming, we cannot let this method
            // fail without doing a collect operation. Instead, we log a warning to
            // the user.
            let stream = futures::stream::iter(stream.filter_map(move |rb| match rb {
                Ok(rb) => Some(rb),
                Err(err) => {
                    warn!("write_table input stream contains an error. {err}");
                    None
                }
            }));

            self.client()
                .await?
                .write_table(stream, entry_id, insert_mode.into())
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn get_dataset_schema(&self, py: Python<'_>, entry_id: EntryId) -> PyResult<ArrowSchema> {
        wait_for_future(py, async {
            self.client()
                .await?
                .get_dataset_schema(entry_id)
                .await
                .map_err(to_py_err)
        })
    }

    /// Initiate registration of the provided recording URIs with a dataset and return the
    /// corresponding task descriptors.
    ///
    /// Custom layers can be specified via `recording_layers`:
    /// * When empty, this defaults to `["base"]`.
    /// * If longer than `recording_uris`, `recording_layers` will be truncated.
    /// * If shorter than `recording_uris`, `recording_layers` will be extended by repeating its last value.
    ///   I.e. an empty `recording_layers` will result in `"base"` begin repeated `len(recording_layers)` times.
    ///
    /// NOTE: The server may pool multiple registrations into a single task. The result always has
    /// the same length as the output, so task ids may be duplicated.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_with_dataset(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        recording_uris: Vec<String>,
        recording_layers: Vec<LayerName>,
        on_duplicate: IfDuplicateBehavior,
    ) -> PyResult<RegistrationHandle> {
        let last_layer = recording_layers
            .last()
            .cloned()
            .unwrap_or_else(LayerName::base);

        let data_sources = std::iter::zip(
            &recording_uris,
            std::iter::chain(
                recording_layers,
                std::iter::repeat_with(|| last_layer.clone()),
            ),
        )
        .map(|(url, layer)| DataSource::new_rrd_layer(layer, url))
        .try_collect()
        .map_err(to_py_err)?;

        wait_for_future(py, async {
            self.inner
                .register_with_dataset(dataset_id, data_sources, on_duplicate)
                .await
                .map_err(to_py_err)
        })
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
    pub fn unregister_from_dataset(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        segments_to_drop: Vec<SegmentId>,
        layers_to_drop: Vec<LayerName>,
        force: bool,
    ) -> PyResult<(Option<TraceId>, Vec<TaskId>)> {
        wait_for_future(py, async {
            self.client()
                .await?
                .unregister_from_dataset(dataset_id, segments_to_drop, layers_to_drop, force)
                .await
                .map_err(to_py_err)
        })
    }

    /// Initiate registration of all the recordings within provided object store prefix (aka directory)
    /// and return the corresponding task descriptors.
    ///
    /// A custom layer can be specified via `recordings_layer`:
    /// * When empty, this defaults to `["base"]`.
    #[tracing::instrument(level = "info", skip_all)]
    pub fn register_with_dataset_prefix(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        recordings_prefix: String,
        recordings_layer: LayerName,
        on_duplicate: IfDuplicateBehavior,
    ) -> PyResult<RegistrationHandle> {
        let data_source = DataSource::new_rrd_layer_prefix(recordings_layer, recordings_prefix)
            .map_err(to_py_err)?;
        let data_sources = vec![data_source];

        wait_for_future(py, async {
            self.inner
                .register_with_dataset(dataset_id, data_sources, on_duplicate)
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    #[expect(clippy::fn_params_excessive_bools)]
    pub fn do_maintenance(
        &self,
        py: Python<'_>,
        dataset_id: EntryId,
        optimize_indexes: bool,
        retrain_indexes: bool,
        compact_fragments: bool,
        cleanup_before: Option<jiff::Timestamp>,
        unsafe_allow_recent_cleanup: bool,
    ) -> PyResult<()> {
        wait_for_future(py, async {
            self.client()
                .await?
                .do_maintenance(
                    dataset_id,
                    optimize_indexes,
                    retrain_indexes,
                    compact_fragments,
                    cleanup_before,
                    unsafe_allow_recent_cleanup,
                )
                .await
                .map_err(to_py_err)
        })
    }

    #[tracing::instrument(level = "info", skip_all)]
    pub fn do_global_maintenance(&self, py: Python<'_>) -> PyResult<()> {
        wait_for_future(py, async {
            self.client()
                .await?
                .do_global_maintenance()
                .await
                .map_err(to_py_err)
        })
    }
}
