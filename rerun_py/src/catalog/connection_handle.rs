//! Client connection handle which ch

use std::sync::Arc;

use pyo3::{exceptions::PyRuntimeError, PyResult};

use re_grpc_client::redap::catalog_client;
use re_protos::catalog::v1alpha1::catalog_service_client::CatalogServiceClient;
use re_protos::catalog::v1alpha1::{
    CreateDatasetEntryRequest, DatasetEntry, DeleteDatasetEntryRequest, EntryDetails, EntryFilter,
};

use crate::catalog::PyEntryId;

#[derive(Clone)]
/// Cheap-to-clone connection handle to a catalog service.
pub struct CatalogConnectionHandle {
    //TODO?
    #[expect(dead_code)]
    origin: re_uri::Origin,

    /// A tokio runtime for async operations. This connection will currently
    /// block the Python interpreter while waiting for responses.
    /// This runtime must be persisted for the lifetime of the connection.
    //TODO(ab): this should be a tokio runtime handle, not the runtime itself
    runtime: Arc<tokio::runtime::Runtime>,

    /// The actual tonic connection.
    client: CatalogServiceClient<tonic::transport::Channel>,
}

impl CatalogConnectionHandle {
    pub fn new(origin: re_uri::Origin) -> PyResult<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        let client = runtime
            .block_on(catalog_client(origin.clone()))
            //TODO: proper error management
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(Self {
            origin,
            runtime: Arc::new(runtime),
            client,
        })
    }
}

// TODO(ab): all these request wrapper should be implemented in a more general client wrapper also
// used in e.g. the redap browser, etc. The present connection handle should just forward them.
impl CatalogConnectionHandle {
    //TODO(ab): return nicer wrapper object over the gRPC message
    pub fn find_entries(&mut self, filter: EntryFilter) -> PyResult<Vec<EntryDetails>> {
        let response =
            self.runtime
                .block_on(self.client.find_entries(
                    re_protos::catalog::v1alpha1::FindEntriesRequest {
                        filter: Some(filter),
                    },
                ))
                // TODO(ab): error management
                .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

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
            // TODO(ab): error management
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        response
            .into_inner()
            .dataset
            .ok_or(PyRuntimeError::new_err("No dataset in response"))
    }

    pub fn delete_dataset(&mut self, entry_id: PyEntryId) -> PyResult<()> {
        let _response = self
            .runtime
            .block_on(self.client.delete_dataset_entry(DeleteDatasetEntryRequest {
                id: Some(entry_id.id.into()),
            }))
            // TODO(ab): error management
            .map_err(|err| PyRuntimeError::new_err(err.to_string()))?;

        Ok(())
    }
}
