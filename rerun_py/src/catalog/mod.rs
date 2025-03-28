#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod connection_handle;
mod dataset;
mod entry;
mod errors;

use pyo3::{prelude::*, Bound, PyResult};

pub use catalog_client::PyCatalogClient;
pub use connection_handle::ConnectionHandle;
pub use dataset::PyDataset;
pub use entry::{PyEntry, PyEntryId, PyEntryKind};
pub use errors::to_py_err;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClient>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntry>()?;

    m.add_class::<PyDataset>()?;

    Ok(())
}
