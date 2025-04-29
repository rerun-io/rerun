use pyo3::prelude::{PyModule, PyModuleMethods};
use pyo3::{wrap_pyfunction, Bound, PyResult, Python};

mod scalar_udfs;

use scalar_udfs::{bounded_image_extraction_udf, depth_image_to_point_cloud_udf};

pub(crate) fn register(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    let datafusion_module = PyModule::new(py, "datafusion")?;
    m.add_submodule(&datafusion_module)?;

    datafusion_module.add_function(wrap_pyfunction!(
        bounded_image_extraction_udf,
        &datafusion_module
    )?)?;
    datafusion_module.add_function(wrap_pyfunction!(
        depth_image_to_point_cloud_udf,
        &datafusion_module
    )?)?;

    Ok(())
}
