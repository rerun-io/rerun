use datafusion::logical_expr::ScalarUDF;
use datafusion_ffi::udf::FFI_ScalarUDF;
use pyo3::types::PyCapsule;
use pyo3::{pyclass, pymethods, Bound, PyResult, Python};
use re_datafusion::functions::{
    BoundedImageExtractionUdf, DepthImageToPointCloudUdf, SetEntityPathUdf,
};
use std::sync::Arc;

#[pyclass(name = "BoundedImageExtractionUDF")]
pub struct PyBoundedImageExtractionUdf {
    inner: Arc<ScalarUDF>,
}

#[pymethods]
impl PyBoundedImageExtractionUdf {
    #[new]
    pub fn new() -> Self {
        let udf = ScalarUDF::from(BoundedImageExtractionUdf::default());
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

#[pyclass(name = "DepthImageToPointCloudUDF")]
pub struct PyDepthImageToPointCloudUdf {
    inner: Arc<ScalarUDF>,
}

#[pymethods]
impl PyDepthImageToPointCloudUdf {
    #[new]
    pub fn new(entity_path: &str) -> Self {
        let udf = ScalarUDF::from(DepthImageToPointCloudUdf::new(entity_path));
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

#[pyclass(name = "SetEntityPathUDF")]
pub struct PySetEntityPathUdf {
    inner: Arc<ScalarUDF>,
}

// TODO turn these into a proc macro

#[pymethods]
impl PySetEntityPathUdf {
    #[new]
    pub fn new(entity_path: &str) -> Self {
        let udf = ScalarUDF::from(SetEntityPathUdf::new(entity_path));
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
