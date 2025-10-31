#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod connection_handle;
mod dataframe_query;
mod dataframe_rendering;
mod datafusion_catalog;
mod datafusion_table;
mod dataset_entry;
mod entry;
mod errors;
mod indexes;
mod table_entry;
mod task;

use errors::{AlreadyExistsError, NotFoundError};
use pyo3::{Bound, PyResult, prelude::*};

use crate::catalog::dataframe_query::PyDataframeQueryView;

pub use self::{
    catalog_client::PyCatalogClientInternal,
    connection_handle::ConnectionHandle,
    dataframe_rendering::PyRerunHtmlTable,
    datafusion_table::PyDataFusionTable,
    dataset_entry::PyDatasetEntry,
    entry::{PyEntry, PyEntryId, PyEntryKind},
    errors::to_py_err,
    indexes::{
        PyIndexConfig, PyIndexProperties, PyIndexingResult, PyVectorDistanceMetric,
        VectorDistanceMetricLike, VectorLike,
    },
    table_entry::{PyTableEntry, PyTableInsertMode},
    task::{PyTask, PyTasks},
};

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClientInternal>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntry>()?;
    m.add_class::<PyDatasetEntry>()?;
    m.add_class::<PyTableEntry>()?;
    m.add_class::<PyTableInsertMode>()?;
    m.add_class::<PyTask>()?;
    m.add_class::<PyTasks>()?;
    m.add_class::<PyDataFusionTable>()?;
    m.add_class::<PyDataframeQueryView>()?;
    m.add_class::<PyRerunHtmlTable>()?;

    // indexing
    m.add_class::<PyIndexingResult>()?;
    m.add_class::<PyIndexConfig>()?;
    m.add_class::<PyIndexProperties>()?;
    m.add_class::<PyVectorDistanceMetric>()?;

    // register exceptions generated with the [`pyo3::create_exception!`] macro
    m.add("NotFoundError", _py.get_type::<NotFoundError>())?;
    m.add("AlreadyExistsError", _py.get_type::<AlreadyExistsError>())?;

    Ok(())
}
