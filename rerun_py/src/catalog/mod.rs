#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod component_columns;
mod connection_handle;
mod dataframe_query;
mod dataframe_rendering;
mod datafusion_catalog;
mod datafusion_table;
mod dataset_entry;
mod entry;
mod errors;
mod index_columns;
mod indexes;
mod schema;
mod table_entry;
mod task;
mod trace_context;

use errors::{AlreadyExistsError, NotFoundError};
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub use self::catalog_client::PyCatalogClientInternal;
pub use self::component_columns::{PyComponentColumnDescriptor, PyComponentColumnSelector};
pub use self::connection_handle::ConnectionHandle;
pub use self::dataframe_query::PyDataframeQueryView;
pub use self::dataframe_rendering::PyRerunHtmlTable;
pub use self::datafusion_table::PyDataFusionTable;
pub use self::dataset_entry::PyDatasetEntryInternal;
pub use self::entry::{PyEntryDetails, PyEntryId, PyEntryKind};
pub use self::errors::to_py_err;
pub use self::index_columns::{PyIndexColumnDescriptor, PyIndexColumnSelector};
pub use self::indexes::{
    PyIndexConfig, PyIndexProperties, PyIndexingResult, PyVectorDistanceMetric,
    VectorDistanceMetricLike, VectorLike,
};
pub use self::schema::PySchemaInternal;
pub use self::table_entry::{PyTableEntryInternal, PyTableInsertMode};
pub use self::task::{PyTask, PyTasks};

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClientInternal>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntryDetails>()?;
    m.add_class::<PyDatasetEntryInternal>()?;
    m.add_class::<PyTableEntryInternal>()?;
    m.add_class::<PyTableInsertMode>()?;
    m.add_class::<PyTask>()?;
    m.add_class::<PyTasks>()?;
    m.add_class::<PyDataFusionTable>()?;
    m.add_class::<PyDataframeQueryView>()?;
    m.add_class::<PyRerunHtmlTable>()?;

    // schema
    m.add_class::<PySchemaInternal>()?;
    m.add_class::<PyIndexColumnDescriptor>()?;
    m.add_class::<PyIndexColumnSelector>()?;
    m.add_class::<PyComponentColumnDescriptor>()?;
    m.add_class::<PyComponentColumnSelector>()?;

    // indexing
    m.add_class::<PyIndexingResult>()?;
    m.add_class::<PyIndexConfig>()?;
    m.add_class::<PyIndexProperties>()?;
    m.add_class::<PyVectorDistanceMetric>()?;

    // register exceptions generated with the [`pyo3::create_exception!`] macro
    m.add("NotFoundError", _py.get_type::<NotFoundError>())?;
    m.add("AlreadyExistsError", _py.get_type::<AlreadyExistsError>())?;

    m.add_function(wrap_pyfunction!(trace_context::rerun_trace_context, m)?)?;

    Ok(())
}
