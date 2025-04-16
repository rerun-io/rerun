use datafusion::logical_expr::ScalarUDF;
use datafusion_ffi::udf::FFI_ScalarUDF;
use pyo3::types::PyCapsule;
use pyo3::{pyclass, pymethods, Bound, PyResult, Python};
use re_datafusion::functions::bounded_image_extraction::BoundedImageExtractionUdf;
use std::sync::Arc;

#[pyclass(name = "BoundedImageExtractionUDF")]
pub struct PyBoundedImageExtractionUdf {
    inner: Arc<ScalarUDF>,
}

#[pymethods]
impl PyBoundedImageExtractionUdf {
    #[new]
    pub fn new(entity_path: &str, class_of_interest: u16) -> Self {
        let udf = ScalarUDF::from(BoundedImageExtractionUdf::new(
            entity_path,
            class_of_interest,
        ));
        Self {
            inner: Arc::new(udf),
        }
    }

    pub fn __datafusion_scalar_udf__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let name = cr"datafusion_scalar_udf".into();

        let udf = FFI_ScalarUDF::from(Arc::clone(&self.inner));

        PyCapsule::new(py, udf, Some(name))
    }
}
