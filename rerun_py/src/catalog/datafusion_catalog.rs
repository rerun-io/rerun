use std::sync::Arc;

use crate::catalog::table_provider_adapter::ffi_logical_codec_from_pycapsule;
use crate::utils::get_tokio_runtime;
use datafusion::catalog::CatalogProvider;
use datafusion_ffi::catalog_provider::FFI_CatalogProvider;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyAny, PyResult, pyclass, pymethods};
use re_datafusion::RedapCatalogProvider;
use re_redap_client::ConnectionClient;

#[pyclass(
    frozen,
    eq,
    name = "DataFusionCatalog",
    module = "rerun_bindings.rerun_bindings"
)]
pub(crate) struct PyDataFusionCatalogProvider {
    pub provider: Arc<RedapCatalogProvider>,
}

impl PartialEq for PyDataFusionCatalogProvider {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.provider, &other.provider)
    }
}

impl PyDataFusionCatalogProvider {
    pub fn new(name: Option<String>, client: ConnectionClient) -> Self {
        let runtime = get_tokio_runtime().handle().clone();
        let provider = Arc::new(RedapCatalogProvider::new(name.as_deref(), client, runtime));
        Self { provider }
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyDataFusionCatalogProvider {
    /// Returns a DataFusion catalog provider capsule.
    fn __datafusion_catalog_provider__<'py>(
        &self,
        session: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_catalog_provider".into();

        let provider = Arc::clone(&self.provider) as Arc<dyn CatalogProvider>;

        let runtime = get_tokio_runtime().handle().clone();
        let codec = ffi_logical_codec_from_pycapsule(session)?;
        let provider = FFI_CatalogProvider::new_with_ffi_codec(provider, Some(runtime), codec);

        PyCapsule::new(session.py(), provider, Some(capsule_name))
    }
}
