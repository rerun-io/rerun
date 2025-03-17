#![allow(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value
#![allow(unsafe_op_in_unsafe_fn)] // False positive due to #[pyfunction] macro

use pyo3::{exceptions::PyRuntimeError, prelude::*, Bound, PyResult};

use re_protos::catalog::v1alpha1::{
    catalog_service_client::CatalogServiceClient, CreateDatasetEntryRequest, DatasetEntry,
    DeleteDatasetEntryRequest, EntryDetails, EntryFilter, FindEntriesRequest,
};
use re_protos::common::v1alpha1::Tuid;

/// Register the `rerun.catalog` module.
pub(crate) fn register(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyEntryId>()?;
    m.add_class::<PyCatalogClient>()?;

    m.add_function(wrap_pyfunction!(connect, m)?)?;

    Ok(())
}

async fn connect_async(addr: String) -> PyResult<CatalogServiceClient<tonic::transport::Channel>> {
    #[cfg(not(target_arch = "wasm32"))]
    let tonic_client = tonic::transport::Endpoint::new(addr)
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
        .connect()
        .await
        .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

    Ok(CatalogServiceClient::new(tonic_client))
}

#[pyfunction]
pub fn connect(addr: String) -> PyResult<PyCatalogClient> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    let client = runtime.block_on(connect_async(addr))?;

    Ok(PyCatalogClient { runtime, client })
}

/// A unique identifier for an entry in the catalog.
#[pyclass(name = "EntryId")]
#[derive(Clone)]
pub struct PyEntryId {
    id: Tuid,
}

impl From<Tuid> for PyEntryId {
    fn from(id: Tuid) -> Self {
        Self { id }
    }
}

/// A connection to a remote storage node.
#[pyclass(name = "CatalogClient")]
pub struct PyCatalogClient {
    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    runtime: tokio::runtime::Runtime,

    /// The actual tonic connection.
    client: CatalogServiceClient<tonic::transport::Channel>,
}

#[pymethods]
impl PyCatalogClient {
    // TODO: Create and return a dataset object
    fn create_dataset(&mut self, name: &str) -> PyResult<PyEntryId> {
        self.runtime.block_on(async {
            let resp = self
                .client
                .create_dataset_entry(CreateDatasetEntryRequest {
                    dataset: Some(DatasetEntry {
                        details: Some(EntryDetails {
                            name: Some(name.to_owned()),
                            ..Default::default()
                        }),
                        dataset_handle: None,
                    }),
                })
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            // TODO: This stuff is so ugly
            resp.dataset
                .ok_or(PyRuntimeError::new_err("No dataset in response"))?
                .details
                .ok_or(PyRuntimeError::new_err("No details in response"))?
                .id
                .ok_or(PyRuntimeError::new_err("No id in details"))
                .map(|id| PyEntryId { id })
        })
    }

    // TODO: Create and return entry objects
    fn list_entries(&mut self) -> PyResult<Vec<PyEntryId>> {
        self.runtime.block_on(async {
            let resp = self
                .client
                .find_entries(FindEntriesRequest {
                    filter: Some(EntryFilter::default()),
                })
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            resp.entries
                .into_iter()
                .map(|entry| {
                    Ok(PyEntryId {
                        id: entry
                            .id
                            .map(Into::into)
                            .ok_or(PyRuntimeError::new_err("No id in details"))?,
                    })
                })
                .collect()
        })
    }

    // TODO: Create and return entry objects
    fn delete_dataset(&mut self, id: PyEntryId) -> PyResult<()> {
        self.runtime.block_on(async {
            let _resp = self
                .client
                .delete_dataset_entry(DeleteDatasetEntryRequest { id: Some(id.id) })
                .await
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?
                .into_inner();

            Ok(())
        })
    }
}
