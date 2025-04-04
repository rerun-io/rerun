#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

use arrow::array::RecordBatch;
use pyo3::{prelude::*, Bound, PyResult};

use re_sdk::ViewerClient;

use crate::catalog::to_py_err;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyViewerClient>()?;

    Ok(())
}

/// A connection to an instance of a Rerun viewer.
#[pyclass(name = "ViewerClient")]
pub struct PyViewerClient {
    client: ViewerClient,
}
#[pymethods]
impl PyViewerClient {
    /// Create a new viewer client object.
    #[new]
    fn new(addr: String) -> PyResult<Self> {
        let client = ViewerClient::builder()
            .with_url(addr)
            .map_err(to_py_err)?
            .connect();

        Ok(Self { client })
    }

    fn send_table(
        &self,
        id: String,
        table: arrow::pyarrow::PyArrowType<RecordBatch>,
    ) -> PyResult<()> {
        self.client.send_table(id, table.0);

        Ok(())
    }
}
