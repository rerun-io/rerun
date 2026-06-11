use std::sync::Arc;

use datafusion::catalog::TableProvider;
use datafusion_ffi::proto::logical_extension_codec::FFI_LogicalExtensionCodec;
use datafusion_ffi::table_provider::FFI_TableProvider;
use pyo3::prelude::{PyAnyMethods as _, PyCapsuleMethods as _};
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyAny, PyResult, pyclass, pymethods};

use crate::utils::get_tokio_runtime;

/// Adapter to expose a [`TableProvider`] to the Python side via the DataFusion FFI capsule protocol.
#[pyclass(
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

#[pymethods]
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

    let capsule = capsule.cast::<PyCapsule>()?;
    let codec_ptr = capsule
        .pointer_checked(Some(c"datafusion_logical_extension_codec"))?
        .cast::<FFI_LogicalExtensionCodec>();
    // Safety: `pointer_checked` has verified the capsule name matches
    // `datafusion_logical_extension_codec` and that the pointer is non-null. We trust
    // datafusion-python to have stored a valid, initialized `FFI_LogicalExtensionCodec`
    // behind a capsule of that name; if it hasn't, something is very wrong with
    // datafusion-python.
    let codec = unsafe { codec_ptr.as_ref() };

    Ok(codec.clone())
}
