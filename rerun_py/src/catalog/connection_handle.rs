//! Client connection handle which ch

use std::sync::Arc;

use pyo3::exceptions::PyConnectionError;
use pyo3::{create_exception, exceptions::PyRuntimeError, PyResult};

use re_grpc_client::redap::catalog_client;
use re_protos::catalog::v1alpha1::catalog_service_client::CatalogServiceClient;
use re_protos::catalog::v1alpha1::{
    CreateDatasetEntryRequest, DatasetEntry, DeleteDatasetEntryRequest, EntryDetails, EntryFilter,
    ReadDatasetEntryRequest,
};
use re_tuid::Tuid;

use crate::catalog::to_py_err;

create_exception!(catalog, ConnectionError, PyConnectionError);

/// Connection handle to a catalog service.
#[derive(Clone)]
pub struct ConnectionHandle {
    #[expect(dead_code)]
    origin: re_uri::Origin,

    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    runtime: Arc<tokio::runtime::Runtime>,

    /// The actual tonic connection.
    client: CatalogServiceClient<tonic::transport::Channel>,
}

impl ConnectionHandle {
    pub fn new(origin: re_uri::Origin, runtime: tokio::runtime::Runtime) -> PyResult<Self> {
        let client = runtime
            .block_on(catalog_client(origin.clone()))
            .map_err(to_py_err)?;

        Ok(Self {
            origin,
            runtime: Arc::new(runtime),
            client,
        })
    }
}

// TODO(ab): all these request wrapper should be implemented in a more general client wrapper also
// used in e.g. the redap browser, etc. The present connection handle should just forward them.
impl ConnectionHandle {
    //TODO(ab): return nicer wrapper object over the gRPC message
    pub fn find_entries(&mut self, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        let response =
            self.runtime
                .block_on(self.client.find_entries(
                    re_protos::catalog::v1alpha1::FindEntriesRequest {
                        filter: Some(filter),
                    },
                ))
                .map_err(to_py_err)?;

        let entries = response.into_inner().entries;

        Ok(entries)
    }

    //TODO(ab): return nicer wrapper object over the gRPC message
    pub fn create_dataset(&mut self, entry: DatasetEntry) -> PyResult<DatasetEntry> {
        let response = self
            .runtime
            .block_on(self.client.create_dataset_entry(CreateDatasetEntryRequest {
                dataset: Some(entry),
            }))
            .map_err(to_py_err)?;

        response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))
    }

    pub fn read_dataset(&mut self, entry_id: Tuid) -> PyResult<DatasetEntry> {
        let response = self
            .runtime
            .block_on(self.client.read_dataset_entry(ReadDatasetEntryRequest {
                id: Some(entry_id.into()),
            }))
            .map_err(to_py_err)?;

        response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))
    }

    pub fn delete_dataset(&mut self, entry_id: Tuid) -> PyResult<()> {
        let _response = self
            .runtime
            .block_on(self.client.delete_dataset_entry(DeleteDatasetEntryRequest {
                id: Some(entry_id.into()),
            }))
            .map_err(to_py_err)?;

        Ok(())
    }
}
