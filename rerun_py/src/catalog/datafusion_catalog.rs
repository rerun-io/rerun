use std::sync::Arc;

use datafusion::catalog::CatalogProvider;
use datafusion_ffi::catalog_provider::FFI_CatalogProvider;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyResult, Python, pyclass, pymethods};

use re_datafusion::RedapCatalogProvider;
use re_redap_client::ConnectionClient;

use crate::utils::get_tokio_runtime;

#[pyclass(frozen, name = "DataFusionCatalog")]
pub(crate) struct PyDataFusionCatalogProvider {
    pub provider: Arc<RedapCatalogProvider>,
}

impl PyDataFusionCatalogProvider {
    pub fn new(name: Option<String>, client: ConnectionClient) -> Self {
        let runtime = get_tokio_runtime().handle().clone();
        let provider = Arc::new(RedapCatalogProvider::new(name.as_deref(), client, runtime));
        Self { provider }
    }
}

#[pymethods]
impl PyDataFusionCatalogProvider {
    /// Returns a DataFusion catalog provider capsule.
    fn __datafusion_catalog_provider__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_catalog_provider".into();

        let provider = Arc::clone(&self.provider) as Arc<dyn CatalogProvider>;

        let runtime = get_tokio_runtime().handle().clone();
        let provider = FFI_CatalogProvider::new(provider, Some(runtime));

        PyCapsule::new(py, provider, Some(capsule_name))
    }
}
