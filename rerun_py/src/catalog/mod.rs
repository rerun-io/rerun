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
mod object_store;
mod registration_handle;
mod schema;
mod segment_url_udf;
mod table_entry;
mod table_provider_adapter;
mod type_aliases;
mod unregistration_handle;

use errors::{AlreadyExistsError, NotFoundError};
use pyo3::prelude::*;
use pyo3::{Bound, PyResult};
use re_protos::cloud::v1alpha1::ext::{
    ASSET_MODE_COMPONENT, ASSET_PROPERTY, ASSET_SEGMENTS_COMPONENT, AssetMode,
};

pub use self::catalog_client::PyCatalogClientInternal;
pub use self::component_columns::{PyComponentColumnDescriptor, PyComponentColumnSelector};
pub(crate) use self::connection_handle::PyConnectionHandle;
pub use self::dataframe_rendering::PyRerunHtmlTable;
pub use self::dataset_entry::PyDatasetEntryInternal;
pub use self::dataset_view::PyDatasetViewInternal;
pub use self::entry::{PyEntryDetails, PyEntryId, PyEntryKind};
pub use self::errors::to_py_err;
pub use self::index_columns::{PyIndexColumnDescriptor, PyIndexColumnSelector};
pub use self::object_store::{AnyObjectStoreAuthenticator, BearerTokenObjectStoreAuthenticator};
pub use self::registration_handle::{PyRegistrationHandleInternal, PyRegistrationIterator};
pub use self::schema::PySchemaInternal;
pub use self::segment_url_udf::PySegmentUrlUdfInternal;
pub use self::table_entry::{PyTableEntryInternal, PyTableInsertModeInternal};
pub use self::table_provider_adapter::PyTableProviderAdapterInternal;
pub use self::type_aliases::{AnyComponentColumn, IndexValuesLike, PyIndexValuesLikeInternal};
pub use self::unregistration_handle::PyUnregistrationHandleInternal;

/// Register the `rerun.catalog` module.
pub(crate) fn register(_py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyCatalogClientInternal>()?;
    m.add_class::<BearerTokenObjectStoreAuthenticator>()?;

    m.add_class::<PyEntryId>()?;
    m.add_class::<PyEntryKind>()?;
    m.add_class::<PyEntryDetails>()?;
    m.add_class::<PyDatasetEntryInternal>()?;
    m.add_class::<PyTableEntryInternal>()?;
    m.add_class::<PyTableInsertModeInternal>()?;
    m.add_class::<PyRegistrationHandleInternal>()?;
    m.add_class::<PyUnregistrationHandleInternal>()?;
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

    // testing
    m.add_class::<PyIndexValuesLikeInternal>()?;

    // register exceptions generated with the [`pyo3::create_exception!`] macro
    m.add("NotFoundError", _py.get_type::<NotFoundError>())?;
    m.add("AlreadyExistsError", _py.get_type::<AlreadyExistsError>())?;

    // The `asset` property layout, so the SDK writes what `GetAssetsForSegment` reads.
    m.add("ASSET_PROPERTY", ASSET_PROPERTY)?;
    m.add("ASSET_MODE_COMPONENT", ASSET_MODE_COMPONENT)?;
    m.add("ASSET_SEGMENTS_COMPONENT", ASSET_SEGMENTS_COMPONENT)?;
    m.add("ASSET_MODE_OPT_IN", AssetMode::OptIn.as_str())?;
    m.add("ASSET_MODE_OPT_OUT", AssetMode::OptOut.as_str())?;

    Ok(())
}
