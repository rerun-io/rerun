use std::str::FromStr as _;

use itertools::Itertools as _;
use pyo3::exceptions::PyValueError;
use pyo3::{IntoPyObjectExt as _, Py, PyObject, PyRef, PyRefMut, PyResult, pyclass, pymethods};

use re_log_types::EntityPath;
use re_sorbet::{ColumnDescriptor, ComponentColumnSelector, SorbetColumnDescriptors};

use super::{
    AnyComponentColumn, component_columns::PyComponentColumnDescriptor,
    index_columns::PyIndexColumnDescriptor,
};
use crate::catalog::to_py_err;

#[pyclass]
pub struct SchemaIterator {
    iter: std::vec::IntoIter<PyObject>,
}

#[pymethods]
impl SchemaIterator {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(mut slf: PyRefMut<'_, Self>) -> Option<PyObject> {
        slf.iter.next()
    }
}

#[pyclass(frozen, name = "Schema")]
#[derive(Clone)]
//TODO(#9457): improve this object and use it for `Dataset.schema()`.
pub struct PySchema {
    pub schema: SorbetColumnDescriptors,
}

/// The schema representing a set of available columns.
///
/// Can be returned by [`Recording.schema()`][rerun.dataframe.Recording.schema] or
/// [`RecordingView.schema()`][rerun.dataframe.RecordingView.schema].
#[pymethods]
impl PySchema {
    fn __repr__(&self) -> String {
        self.component_columns()
            .iter()
            .map(|col| col.__repr__())
            .join("\n")
    }

    /// Iterate over all the column descriptors in the schema, ignoring `RowId`.
    fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<SchemaIterator>> {
        let py = slf.py();
        let iter = SchemaIterator {
            iter: slf
                .schema
                .iter()
                .filter_map(|col| match col.clone() {
                    ColumnDescriptor::RowId(_) => None, // TODO(#9922)
                    ColumnDescriptor::Time(col) => {
                        Some(PyIndexColumnDescriptor(col).into_py_any(py))
                    }
                    ColumnDescriptor::Component(col) => {
                        Some(PyComponentColumnDescriptor(col).into_py_any(py))
                    }
                })
                .collect::<PyResult<Vec<_>>>()?
                .into_iter(),
        };
        Py::new(slf.py(), iter)
    }

    /// Return a list of all the index columns in the schema.
    fn index_columns(&self) -> Vec<PyIndexColumnDescriptor> {
        self.schema
            .index_columns()
            .map(|c| c.clone().into())
            .collect()
    }

    /// Return a list of all the component columns in the schema.
    fn component_columns(&self) -> Vec<PyComponentColumnDescriptor> {
        self.schema
            .component_columns()
            .map(|c| c.clone().into())
            .collect()
    }

    #[expect(rustdoc::broken_intra_doc_links)]
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

        self.schema.component_columns().find_map(|col| {
            if col.matches(&selector) {
                Some(col.clone().into())
            } else {
                None
            }
        })
    }

    #[expect(rustdoc::private_doc_tests)]
    #[allow(rustdoc::invalid_rust_codeblocks)]
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
}

impl PySchema {
    pub fn resolve_component_column_selector(
        &self,
        column_selector: &ComponentColumnSelector,
    ) -> PyResult<PyComponentColumnDescriptor> {
        let desc = self
            .schema
            .resolve_component_column_selector(column_selector)
            .ok_or(PyValueError::new_err(format!(
                "Could not find column for selector {column_selector}"
            )))?;

        Ok(PyComponentColumnDescriptor(desc.clone()))
    }
}
