#![expect(clippy::needless_pass_by_value)] // A lot of arguments to #[pyfunction] need to be by value

mod catalog_client;
mod component_columns;
mod connection_handle;
mod dataframe_rendering;
mod datafusion_catalog;
mod dataset_entry;
mod dataset_view;
mod entry;
mod errors;
mod index_columns;
mod indexes;
mod registration_handle;
mod schema;
mod segment_url_udf;
mod table_entry;
mod table_provider_adapter;
mod trace_context;
mod type_aliases;

use errors::{AlreadyExistsError, NotFoundError};
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};

pub use self::catalog_client::PyCatalogClientInternal;
pub use self::component_columns::{PyComponentColumnDescriptor, PyComponentColumnSelector};
pub use self::connection_handle::ConnectionHandle;
pub use self::dataframe_rendering::PyRerunHtmlTable;
pub use self::dataset_entry::PyDatasetEntryInternal;
pub use self::dataset_view::PyDatasetViewInternal;
pub use self::entry::{PyEntryDetails, PyEntryId, PyEntryKind};
pub use self::errors::to_py_err;
pub use self::index_columns::{PyIndexColumnDescriptor, PyIndexColumnSelector};
pub use self::indexes::{
    PyIndexConfig, PyIndexProperties, PyIndexingResult, PyVectorDistanceMetric,
    VectorDistanceMetricLike, VectorLike,
};
pub use self::registration_handle::{PyRegistrationHandleInternal, PyRegistrationIterator};
pub use self::schema::PySchemaInternal;
pub use self::segment_url_udf::PySegmentUrlUdfInternal;
pub use self::table_entry::{PyTableEntryInternal, PyTableInsertModeInternal};
pub use self::table_provider_adapter::PyTableProviderAdapterInternal;
pub use self::type_aliases::{AnyComponentColumn, IndexValuesLike, PyIndexValuesLikeInternal};

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClientInternal>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntryDetails>()?;
    m.add_class::<PyDatasetEntryInternal>()?;
    m.add_class::<PyTableEntryInternal>()?;
    m.add_class::<PyTableInsertModeInternal>()?;
    m.add_class::<PyRegistrationHandleInternal>()?;
    m.add_class::<PyRegistrationIterator>()?;
    m.add_class::<PyTableProviderAdapterInternal>()?;
    m.add_class::<PySegmentUrlUdfInternal>()?;
    m.add_class::<PyDatasetViewInternal>()?;
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

    // testing
    m.add_class::<PyIndexValuesLikeInternal>()?;

    // register exceptions generated with the [`pyo3::create_exception!`] macro
    m.add("NotFoundError", _py.get_type::<NotFoundError>())?;
    m.add("AlreadyExistsError", _py.get_type::<AlreadyExistsError>())?;

    m.add_function(wrap_pyfunction!(trace_context::rerun_trace_context, m)?)?;

    Ok(())
}
