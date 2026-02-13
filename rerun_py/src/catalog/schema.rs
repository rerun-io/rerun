use std::collections::HashMap;
use std::str::FromStr as _;

use arrow::datatypes::Schema;
use arrow::ffi::FFI_ArrowSchema;
use pyo3::exceptions::PyLookupError;
use pyo3::types::PyCapsule;
use pyo3::{Bound, PyResult, Python, pyclass, pymethods};
use re_log_types::EntityPath;
use re_sorbet::{BatchType, ComponentColumnSelector, SorbetColumnDescriptors};

use super::component_columns::PyComponentColumnDescriptor;
use super::index_columns::PyIndexColumnDescriptor;
use crate::catalog::{AnyComponentColumn, to_py_err};

#[pyclass(
    frozen,
    eq,
    name = "SchemaInternal",
    module = "rerun_bindings.rerun_bindings"
)]
#[derive(Clone, PartialEq, Eq)]
pub struct PySchemaInternal {
    /// The Sorbet column descriptors.
    pub columns: SorbetColumnDescriptors,

    /// The arrow schema metadata.
    pub metadata: HashMap<String, String>,
}

impl PySchemaInternal {
    /// Convert back the schema to an Arrow schema.
    pub fn into_arrow_schema(self) -> Schema {
        Schema::new_with_metadata(
            self.columns.arrow_fields(BatchType::Dataframe),
            self.metadata,
        )
    }
}

/// The schema representing a set of available columns.
#[pymethods] // NOLINT: ignore[py-mthd-str]
impl PySchemaInternal {
    /// Return a list of all the index columns in the schema.
    fn index_columns(&self) -> Vec<PyIndexColumnDescriptor> {
        self.columns
            .index_columns()
            .map(|c| c.clone().into())
            .collect()
    }

    /// Return a list of all the component columns in the schema.
    fn component_columns(&self) -> Vec<PyComponentColumnDescriptor> {
        self.columns
            .component_columns()
            .map(|c| c.clone().into())
            .collect()
    }

    #[allow(clippy::allow_attributes, rustdoc::broken_intra_doc_links)]
    /// Look up the column descriptor for a specific entity path and component.
    ///
    /// Parameters
    /// ----------
    /// entity_path : str
    ///     The entity path to look up.
    /// component : str
    ///     The component to look up. Example: `Points3D:positions`.
    ///
    /// Returns
    /// -------
    /// Optional[ComponentColumnDescriptor]
    ///     The column descriptor, if it exists.
    fn column_for(
        &self,
        entity_path: &str,
        component: &str,
    ) -> Option<PyComponentColumnDescriptor> {
        let entity_path: EntityPath = entity_path.into();

        let selector = ComponentColumnSelector {
            entity_path,
            component: component.to_owned(),
        };

        self.columns.component_columns().find_map(|col| {
            if col.matches(&selector) {
                Some(col.clone().into())
            } else {
                None
            }
        })
    }

    #[allow(
        clippy::allow_attributes,
        rustdoc::invalid_rust_codeblocks,
        rustdoc::private_doc_tests
    )]
    /// Look up the column descriptor for a specific selector.
    ///
    /// Parameters
    /// ----------
    /// selector: str | ComponentColumnDescriptor | ComponentColumnSelector
    ///     The selector to look up.
    ///
    ///     String arguments are expected to follow the following format:
    ///     `"<entity_path>:<component_type>"`
    ///
    /// Returns
    /// -------
    /// ComponentColumnDescriptor
    ///     The column descriptor, if it exists. Raise an exception otherwise.
    pub fn column_for_selector(
        &self,
        selector: AnyComponentColumn,
    ) -> PyResult<PyComponentColumnDescriptor> {
        match selector {
            AnyComponentColumn::Name(name) => self.resolve_component_column_selector(
                &ComponentColumnSelector::from_str(&name).map_err(to_py_err)?,
            ),

            AnyComponentColumn::ComponentDescriptor(desc) => Ok(desc),

            AnyComponentColumn::ComponentSelector(selector) => {
                self.resolve_component_column_selector(&selector.0)
            }
        }
    }

    fn __arrow_c_schema__<'py>(&'py self, py: Python<'py>) -> PyResult<Bound<'py, PyCapsule>> {
        let schema = self.clone().into_arrow_schema();
        let ffi_schema = FFI_ArrowSchema::try_from(schema).map_err(to_py_err)?;
        let capsule_name = cr"arrow_schema".into();

        PyCapsule::new(py, ffi_schema, Some(capsule_name))
    }
}

impl PySchemaInternal {
    pub fn resolve_component_column_selector(
        &self,
        column_selector: &ComponentColumnSelector,
    ) -> PyResult<PyComponentColumnDescriptor> {
        let desc = self
            .columns
            .resolve_component_column_selector(column_selector)
            .ok_or_else(|| {
                PyLookupError::new_err(format!(
                    "Could not find column for selector {column_selector}"
                ))
            })?;

        Ok(PyComponentColumnDescriptor(desc.clone()))
    }
}
