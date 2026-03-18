use std::sync::Arc;

use crate::utils::get_tokio_runtime;
use datafusion::catalog::TableProvider;
use datafusion_ffi::proto::logical_extension_codec::FFI_LogicalExtensionCodec;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::prelude::{PyAnyMethods as _, PyCapsuleMethods as _};
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyAny, PyResult, pyclass, pymethods};

/// Adapter to expose a [`TableProvider`] to the Python side via the DataFusion FFI capsule protocol.
#[pyclass( // NOLINT: ignore[py-cls-eq] non-trivial implementation
    frozen,
    name = "TableProviderAdapterInternal",
    module = "rerun_bindings.rerun_bindings"
)]
pub struct PyTableProviderAdapterInternal {
    provider: Arc<dyn TableProvider + Send>,
    streaming: bool,
}

impl PyTableProviderAdapterInternal {
    pub fn new(provider: Arc<dyn TableProvider + Send>, streaming: bool) -> Self {
        Self {
            provider,
            streaming,
        }
    }
}

#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PyTableProviderAdapterInternal {
    fn __datafusion_table_provider__<'py>(
        &self,
        session: &Bound<'py, PyAny>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let capsule_name = cr"datafusion_table_provider".into();

        let runtime = get_tokio_runtime().handle().clone();
        let codec = ffi_logical_codec_from_pycapsule(session)?;
        let provider = FFI_TableProvider::new_with_ffi_codec(
            Arc::clone(&self.provider),
            self.streaming,
            Some(runtime),
            codec,
        );

        PyCapsule::new(session.py(), provider, Some(capsule_name))
    }
}

#[expect(unsafe_code)]
pub(crate) fn ffi_logical_codec_from_pycapsule(
    obj: &Bound<'_, PyAny>,
) -> PyResult<FFI_LogicalExtensionCodec> {
    let attr_name = "__datafusion_logical_extension_codec__";
    let capsule = if obj.hasattr(attr_name)? {
        obj.getattr(attr_name)?.call0()?
    } else {
        obj.to_owned()
    };

    let capsule = capsule.downcast::<PyCapsule>()?;
    // Safety: If we cannot downcast this then there is something very wrong with datafusion-python
    let codec = unsafe { capsule.reference::<FFI_LogicalExtensionCodec>() };

    Ok(codec.clone())
}
