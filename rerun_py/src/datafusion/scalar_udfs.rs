use datafusion::logical_expr::ScalarUDF;
use datafusion_ffi::udf::FFI_ScalarUDF;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyResult, Python, pyclass, pyfunction, pymethods};
use re_datafusion::functions::{DepthImageToPointCloudUdf, IntersectionOverUnionUdf};
use std::sync::Arc;

#[pyclass(name = "RerunScalarUDF")]
pub struct PyRerunScalarUDF {
    pub(crate) inner: Arc<ScalarUDF>,
}

#[pymethods]
impl PyRerunScalarUDF {
    pub fn __datafusion_scalar_udf__<'py>(
        &self,
        py: Python<'py>,
    ) -> PyResult<Bound<'py, PyCapsule>> {
        let name = cr"datafusion_scalar_udf".into();

        let udf = FFI_ScalarUDF::from(Arc::clone(&self.inner));

        PyCapsule::new(py, udf, Some(name))
    }
}

#[pyfunction]
pub fn depth_image_to_point_cloud_udf() -> PyRerunScalarUDF {
    let udf = DepthImageToPointCloudUdf::default();
    let inner = Arc::new(ScalarUDF::new_from_impl(udf));
    PyRerunScalarUDF { inner }
}

#[pyfunction]
pub fn intersection_over_union_udf() -> PyRerunScalarUDF {
    let udf = IntersectionOverUnionUdf::default();
    let inner = Arc::new(ScalarUDF::new_from_impl(udf));
    PyRerunScalarUDF { inner }
}
