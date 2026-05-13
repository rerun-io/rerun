use std::sync::Arc;

use crate::catalog::table_provider_adapter::ffi_logical_codec_from_pycapsule;
use crate::utils::get_tokio_runtime;
use datafusion::catalog::CatalogProviderList;
use datafusion_ffi::catalog_provider_list::FFI_CatalogProviderList;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyAny, PyResult, pyclass, pymethods};
use re_datafusion::RedapCatalogProviderList;
use re_redap_client::ConnectionClient;

/// PyO3 wrapper exposing a [`RedapCatalogProviderList`] to a Python `datafusion.SessionContext`
/// via `register_catalog_provider_list(...)`.
#[pyclass(
    frozen,
    eq,
    name = "DataFusionCatalogList",
    module = "rerun_bindings.rerun_bindings"
)]
pub(crate) struct PyDataFusionCatalogProviderList {
    pub provider: Arc<RedapCatalogProviderList>,
}

impl PartialEq for PyDataFusionCatalogProviderList {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.provider, &other.provider)
    }
}

impl PyDataFusionCatalogProviderList {
    pub fn new(client: ConnectionClient, origin: re_uri::Origin) -> Self {
        let runtime = get_tokio_runtime().handle().clone();
        let provider = Arc::new(RedapCatalogProviderList::new(client, runtime, Some(origin)));
        Self { provider }
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyDataFusionCatalogProviderList {
    /// Returns a DataFusion catalog provider list capsule.
    fn __datafusion_catalog_provider_list__<'py>(
        &self,
        session: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_catalog_provider_list".into();

        let provider = Arc::clone(&self.provider) as Arc<dyn CatalogProviderList + Send>;

        let runtime = get_tokio_runtime().handle().clone();
        let codec = ffi_logical_codec_from_pycapsule(session)?;
        let provider = FFI_CatalogProviderList::new_with_ffi_codec(provider, Some(runtime), codec);

        PyCapsule::new(session.py(), provider, Some(capsule_name))
    }
}
