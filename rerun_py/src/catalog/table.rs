use pyo3::{pyclass, pymethods, PyRef, Python};

use re_datafusion::table_entry_provider::TableEntryProvider;

use crate::{catalog::PyEntry, dataframe::PyDataFusionTable, utils::wait_for_future};

#[pyclass(name = "Table", extends=PyEntry)]
pub struct PyTable {}

#[pymethods]
impl PyTable {
    #[getter]
    fn datafusion_provider(self_: PyRef<'_, Self>, py: Python<'_>) -> PyDataFusionTable {
        let super_ = self_.as_super();
        let connection = super_.client.borrow_mut(py).connection().clone();

        let provider = wait_for_future(
            py,
            TableEntryProvider::new(connection.client(), super_.id.borrow(py).id.clone().into())
                .into_provider(),
        );

        PyDataFusionTable { provider }
    }
}
